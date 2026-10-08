//! Per-cell admission control (§54). Foreground writes never queue here. Outbox
//! work has reserved capacity; waiting background work is ordered by priority.
//! Active work is not preempted: callers use bounded batches and fenced commits.
use crate::error::AppError;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Instant,
};
use tokio::sync::{mpsc, oneshot};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Priority {
    Operational = 0,
    Outbox = 1,
    CriticalRecovery = 2,
    Backup = 3,
    AnalyticsIngestion = 4,
    HotBackfill = 5,
    SchemaBackfill = 6,
    ArchiveExport = 7,
    ArchivePurge = 8,
    HistoricalExport = 9,
    Maintenance = 10,
}
const COUNT: usize = 11;
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub outbox_slots: usize,
    pub background_slots: usize,
    pub backfill_slots: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            outbox_slots: 1,
            background_slots: 2,
            backfill_slots: 1,
        }
    }
}
#[derive(Default)]
pub struct JobStats {
    queued: [AtomicU64; COUNT],
    active: [AtomicU64; COUNT],
    started: [AtomicU64; COUNT],
    finished: [AtomicU64; COUNT],
    wait_micros: [AtomicU64; COUNT],
    disk_zone: AtomicU64,
}
impl JobStats {
    pub fn disk_zone(&self)->u64 {self.disk_zone.load(Ordering::Acquire)}
    pub fn disk_blocked(&self,p:Priority)->bool {
        let zone=self.disk_zone();
        (zone>=2 && matches!(p,Priority::HotBackfill|Priority::SchemaBackfill|Priority::ArchiveExport|Priority::HistoricalExport|Priority::Maintenance)) || (zone>=3 && p==Priority::AnalyticsIngestion)
    }
    pub fn queued(&self, p: Priority) -> u64 {
        self.queued[p as usize].load(Ordering::Relaxed)
    }
    pub fn active(&self, p: Priority) -> u64 {
        self.active[p as usize].load(Ordering::Acquire)
    }
    pub fn render(&self) -> String {
        let mut text=String::from("# TYPE arena360_background_queued gauge\n# TYPE arena360_background_active gauge\n# TYPE arena360_background_admitted_total counter\n# TYPE arena360_background_released_total counter\n# TYPE arena360_background_queue_wait_seconds_total counter\n");
        for p in 0..COUNT {
            for (name, value) in [
                ("queued", self.queued[p].load(Ordering::Relaxed)),
                ("active", self.active[p].load(Ordering::Relaxed)),
                ("admitted_total", self.started[p].load(Ordering::Relaxed)),
                ("released_total", self.finished[p].load(Ordering::Relaxed)),
            ] {
                text.push_str(&format!(
                    "arena360_background_{name}{{priority=\"P{p}\"}} {value}\n"
                ));
            }
            text.push_str(&format!(
                "arena360_background_queue_wait_seconds_total{{priority=\"P{p}\"}} {}\n",
                self.wait_micros[p].load(Ordering::Relaxed) as f64 / 1_000_000.0
            ));
        }
        text
    }
}
struct Request {
    reply: oneshot::Sender<JobPermit>,
    queued_at: Instant,
}
enum Command {
    Acquire(Priority, Request),
    Release(Priority),
    Wake,
}
pub struct BackgroundJobs {
    sender: mpsc::UnboundedSender<Command>,
    stats: Arc<JobStats>,
}
#[derive(Debug)]
pub struct JobPermit {
    sender: mpsc::UnboundedSender<Command>,
    priority: Priority,
}
impl Drop for JobPermit {
    fn drop(&mut self) {
        let _ = self.sender.send(Command::Release(self.priority));
    }
}
pub struct OperationalGuard {
    jobs: Arc<BackgroundJobs>,
}
impl Drop for OperationalGuard {
    fn drop(&mut self) {
        self.jobs.stats.active[0].fetch_sub(1, Ordering::Release);
        self.jobs.stats.finished[0].fetch_add(1, Ordering::Relaxed);
        let _ = self.jobs.sender.send(Command::Wake);
    }
}
struct WakeOnDrop(mpsc::UnboundedSender<Command>);
impl Drop for WakeOnDrop {
    fn drop(&mut self) {
        let _ = self.0.send(Command::Wake);
    }
}
impl BackgroundJobs {
    pub fn new(limits: Limits) -> Result<Arc<Self>, AppError> {
        if limits.outbox_slots == 0
            || limits.background_slots == 0
            || limits.backfill_slots == 0
            || limits.backfill_slots > limits.background_slots
        {
            return Err(AppError::Internal("Invalid cell background limits".into()));
        }
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let stats = Arc::new(JobStats::default());
        let state = stats.clone();
        let weak = sender.downgrade();
        tokio::spawn(async move {
            let mut queues: [VecDeque<Request>; COUNT] = std::array::from_fn(|_| VecDeque::new());
            let mut active = [0usize; COUNT];
            while let Some(command) = receiver.recv().await {
                match command {
                    Command::Acquire(p, request) => {
                        state.queued[p as usize].fetch_add(1, Ordering::Relaxed);
                        queues[p as usize].push_back(request);
                    }
                    Command::Release(p) => {
                        active[p as usize] -= 1;
                        state.active[p as usize].fetch_sub(1, Ordering::Release);
                        state.finished[p as usize].fetch_add(1, Ordering::Relaxed);
                    }
                    Command::Wake => {}
                }
                // Cancelled waiters consume neither slots nor queue space, even under load.
                for (p, queue) in queues.iter_mut().enumerate() {
                    queue.retain(|r| {
                        if r.reply.is_closed() {
                            state.queued[p].fetch_sub(1, Ordering::Relaxed);
                            false
                        } else {
                            true
                        }
                    });
                }
                loop {
                    let general: usize = active[2..].iter().sum();
                    let backfills: usize = active[5..].iter().sum();
                    let candidate = (1..COUNT).find(|&p| {
                        !queues[p].is_empty()
                            && !((state.disk_zone()>=2 && matches!(p,5|6|7|9|10)) || (state.disk_zone()>=3 && p==4))
                            && if p == 1 {
                                active[1] < limits.outbox_slots
                            } else {
                                general < limits.background_slots
                                    && (p < 5 || backfills < limits.backfill_slots)
                                    && (p <= 2 || state.active[0].load(Ordering::Acquire) == 0)
                            }
                    });
                    let Some(p) = candidate else {
                        break;
                    };
                    let request = queues[p].pop_front().unwrap();
                    state.queued[p].fetch_sub(1, Ordering::Relaxed);
                    let Some(sender) = weak.upgrade() else {
                        break;
                    };
                    active[p] += 1;
                    state.active[p].fetch_add(1, Ordering::Release);
                    state.started[p].fetch_add(1, Ordering::Relaxed);
                    state.wait_micros[p].fetch_add(
                        request
                            .queued_at
                            .elapsed()
                            .as_micros()
                            .min(u64::MAX as u128) as u64,
                        Ordering::Relaxed,
                    );
                    let priority = match p {
                        1 => Priority::Outbox,
                        2 => Priority::CriticalRecovery,
                        3 => Priority::Backup,
                        4 => Priority::AnalyticsIngestion,
                        5 => Priority::HotBackfill,
                        6 => Priority::SchemaBackfill,
                        7 => Priority::ArchiveExport,
                        8 => Priority::ArchivePurge,
                        9 => Priority::HistoricalExport,
                        _ => Priority::Maintenance,
                    };
                    // If the waiter was cancelled after admission, the returned permit drops
                    // and releases through the actor rather than recursively locking state.
                    let _ = request.reply.send(JobPermit { sender, priority });
                }
            }
        });
        Ok(Arc::new(Self { sender, stats }))
    }
    pub fn set_disk_zone(&self,zone:u64) {
        self.stats.disk_zone.store(zone,Ordering::Release);
        let _=self.sender.send(Command::Wake);
    }
    pub fn stats(&self) -> Arc<JobStats> {
        self.stats.clone()
    }
    pub fn operational(self: &Arc<Self>) -> OperationalGuard {
        self.stats.active[0].fetch_add(1, Ordering::Release);
        self.stats.started[0].fetch_add(1, Ordering::Relaxed);
        OperationalGuard { jobs: self.clone() }
    }
    pub async fn acquire(&self, priority: Priority) -> Result<JobPermit, AppError> {
        if priority == Priority::Operational {
            return Err(AppError::Internal(
                "Operational work must bypass the background queue".into(),
            ));
        }
        let (reply, receive) = oneshot::channel();
        let _wake = WakeOnDrop(self.sender.clone());
        self.sender
            .send(Command::Acquire(
                priority,
                Request {
                    reply,
                    queued_at: Instant::now(),
                },
            ))
            .map_err(|_| AppError::Internal("Cell job scheduler closed".into()))?;
        receive
            .await
            .map_err(|_| AppError::Internal("Cell job admission cancelled".into()))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn jobs() -> Arc<BackgroundJobs> {
        BackgroundJobs::new(Limits {
            outbox_slots: 1,
            background_slots: 1,
            backfill_slots: 1,
        })
        .unwrap()
    }
    async fn queued(jobs: &BackgroundJobs, p: Priority, n: u64) {
        tokio::time::timeout(Duration::from_secs(1), async {
            while jobs.stats.queued(p) != n {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    #[tokio::test]
    async fn waiting_jobs_obey_priority_then_release_capacity() {
        let jobs = jobs();
        let held = jobs.acquire(Priority::HotBackfill).await.unwrap();
        let low_jobs = jobs.clone();
        let low =
            tokio::spawn(async move { low_jobs.acquire(Priority::Maintenance).await.unwrap() });
        queued(&jobs, Priority::Maintenance, 1).await;
        let high_jobs = jobs.clone();
        let high = tokio::spawn(async move {
            high_jobs
                .acquire(Priority::AnalyticsIngestion)
                .await
                .unwrap()
        });
        queued(&jobs, Priority::AnalyticsIngestion, 1).await;
        drop(held);
        let high = tokio::time::timeout(Duration::from_secs(1), high)
            .await
            .unwrap()
            .unwrap();
        assert!(!low.is_finished());
        drop(high);
        drop(
            tokio::time::timeout(Duration::from_secs(1), low)
                .await
                .unwrap()
                .unwrap(),
        );
    }
    #[tokio::test]
    async fn foreground_bypasses_queues_and_outbox_and_recovery_keep_reserved_capacity() {
        let jobs = jobs();
        let foreground = jobs.operational();
        let low_jobs = jobs.clone();
        let low =
            tokio::spawn(async move { low_jobs.acquire(Priority::Maintenance).await.unwrap() });
        queued(&jobs, Priority::Maintenance, 1).await;
        let outbox = tokio::time::timeout(Duration::from_secs(1), jobs.acquire(Priority::Outbox))
            .await
            .unwrap()
            .unwrap();
        let recovery = tokio::time::timeout(
            Duration::from_secs(1),
            jobs.acquire(Priority::CriticalRecovery),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(!low.is_finished());
        drop(outbox);
        drop(foreground);
        assert!(!low.is_finished());
        drop(recovery);
        drop(
            tokio::time::timeout(Duration::from_secs(1), low)
                .await
                .unwrap()
                .unwrap(),
        );
    }
    #[tokio::test]
    async fn cancelled_waiters_and_admitted_jobs_never_leak_capacity() {
        let jobs = jobs();
        let held = jobs.acquire(Priority::HotBackfill).await.unwrap();
        let queued_jobs = jobs.clone();
        let waiter = tokio::spawn(async move {
            queued_jobs
                .acquire(Priority::AnalyticsIngestion)
                .await
                .unwrap()
        });
        queued(&jobs, Priority::AnalyticsIngestion, 1).await;
        waiter.abort();
        assert!(waiter.await.unwrap_err().is_cancelled());
        queued(&jobs, Priority::AnalyticsIngestion, 0).await;
        drop(held);
        for _ in 0..100 {
            let copy = jobs.clone();
            let task = tokio::spawn(async move {
                let _permit = copy.acquire(Priority::Maintenance).await.unwrap();
                tokio::task::yield_now().await;
            });
            tokio::task::yield_now().await;
            task.abort();
            let _ = task.await;
        }
        drop(
            tokio::time::timeout(Duration::from_secs(1), jobs.acquire(Priority::Maintenance))
                .await
                .unwrap()
                .unwrap(),
        );
    }
    #[tokio::test]
    async fn slow_backfill_cannot_take_all_ingestion_or_outbox_slots() {
        let jobs = BackgroundJobs::new(Limits::default()).unwrap();
        let backfill = jobs.acquire(Priority::HotBackfill).await.unwrap();
        let other_jobs = jobs.clone();
        let other =
            tokio::spawn(async move { other_jobs.acquire(Priority::Maintenance).await.unwrap() });
        queued(&jobs, Priority::Maintenance, 1).await;
        let ingest = tokio::time::timeout(
            Duration::from_secs(1),
            jobs.acquire(Priority::AnalyticsIngestion),
        )
        .await
        .unwrap()
        .unwrap();
        let outbox = jobs.acquire(Priority::Outbox).await.unwrap();
        assert!(!other.is_finished());
        drop(ingest);
        drop(outbox);
        drop(backfill);
        drop(other.await.unwrap());
    }
}
