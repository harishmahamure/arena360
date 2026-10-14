//! Close user applications only after the webview confirms a signed-out station.
//! The worker sleeps indefinitely when disabled and scans every 30 seconds when
//! enabled. A generation invalidates pending close/terminate work on transitions.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{Duration, Instant};

const INTERVAL: Duration = Duration::from_secs(30);

struct Schedule {
    enabled: bool,
    generation: u64,
    next_run: Option<Instant>,
    app_paths: Vec<String>,
}

impl Schedule {
    const fn new() -> Self {
        Self {
            enabled: false,
            generation: 0,
            next_run: None,
            app_paths: Vec::new(),
        }
    }

    fn update(&mut self, enabled: bool, app_paths: Vec<String>, now: Instant) -> bool {
        if self.enabled == enabled && self.app_paths == app_paths {
            return false;
        }
        self.generation = self.generation.wrapping_add(1);
        self.enabled = enabled;
        self.app_paths = app_paths;
        self.next_run = enabled.then_some(now);
        true
    }

    fn take_due(&mut self, now: Instant) -> Option<u64> {
        if !self.enabled || now < self.next_run? {
            return None;
        }
        self.next_run = Some(now + INTERVAL);
        Some(self.generation)
    }

    fn is_current(&self, generation: u64) -> bool {
        self.enabled && self.generation == generation
    }
}

static STATE: Mutex<Schedule> = Mutex::new(Schedule::new());
static WAKE: OnceLock<mpsc::SyncSender<()>> = OnceLock::new();
static SWEEP_RUNNING: AtomicBool = AtomicBool::new(false);

pub(crate) struct CleanupLease<'a> {
    state: &'a Mutex<Schedule>,
    generation: u64,
    pub app_paths: Vec<String>,
}

impl CleanupLease<'_> {
    /// Serialize each destructive action with disable, so none can start after
    /// a login/Setup transition has acknowledged cancellation.
    pub fn with_active<T>(&self, action: impl FnOnce() -> T) -> Option<T> {
        let state = self.state.lock().ok()?;
        if !state.is_current(self.generation)
            || !crate::lockdown::is_locked()
            || crate::process::session_launch_active()
        {
            return None;
        }
        Some(action())
    }

    pub fn is_active(&self) -> bool {
        self.with_active(|| ()).is_some()
    }
}

fn update(enabled: bool, app_paths: Vec<String>) -> Result<(), String> {
    let changed =
        STATE
            .lock()
            .map_err(|e| e.to_string())?
            .update(enabled, app_paths, Instant::now());
    if changed {
        if let Some(sender) = WAKE.get() {
            let _ = sender.try_send(());
        }
    }
    Ok(())
}

pub fn pause() {
    let _ = update(false, Vec::new());
}

#[tauri::command]
pub async fn set_signed_out_cleanup_enabled_cmd(
    enabled: bool,
    app_paths: Vec<String>,
) -> Result<(), String> {
    update(enabled, app_paths)?;
    if !enabled {
        // Cancellation normally completes within 100 ms. Keep the UI thread
        // free while any process-handle wait finishes before sign-in proceeds.
        tauri::async_runtime::spawn_blocking(|| {
            let deadline = Instant::now() + Duration::from_secs(5);
            while SWEEP_RUNNING.load(Ordering::SeqCst) {
                if Instant::now() >= deadline {
                    return Err("Signed-out application cleanup is still stopping".to_string());
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())??;
    }
    Ok(())
}

pub fn start() -> Result<(), String> {
    // Development builds never close unrelated desktop applications.
    #[cfg(all(windows, not(debug_assertions)))]
    {
        let (tx, rx) = mpsc::sync_channel(1);
        if WAKE.set(tx).is_ok() {
            std::thread::Builder::new()
                .name("kiosk-idle-cleanup".into())
                .spawn(move || worker(rx))
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(all(windows, not(debug_assertions)))]
fn worker(rx: mpsc::Receiver<()>) {
    loop {
        let wait = match STATE.lock() {
            Ok(state) => state
                .next_run
                .map(|due| due.saturating_duration_since(Instant::now())),
            Err(_) => return,
        };
        match wait {
            Some(wait) => match rx.recv_timeout(wait) {
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
                _ => {}
            },
            None => {
                if rx.recv().is_err() {
                    return;
                }
            }
        }
        let lease = {
            let Ok(mut state) = STATE.lock() else {
                return;
            };
            let Some(generation) = state.take_due(Instant::now()) else {
                continue;
            };
            // Reserve under the same mutex as disable to avoid an acknowledgement
            // racing a sweep that has been scheduled but has not started yet.
            SWEEP_RUNNING.store(true, Ordering::SeqCst);
            CleanupLease {
                state: &STATE,
                generation,
                app_paths: state.app_paths.clone(),
            }
        };
        struct Sweep;
        impl Drop for Sweep {
            fn drop(&mut self) {
                SWEEP_RUNNING.store(false, Ordering::SeqCst);
            }
        }
        let _sweep = Sweep;
        crate::process::close_signed_out_desktop_apps(&lease);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_for_confirmation_then_runs_immediately_and_every_thirty_seconds() {
        let now = Instant::now();
        let mut schedule = Schedule::new();
        assert_eq!(schedule.take_due(now + INTERVAL * 10), None);
        schedule.update(true, Vec::new(), now);
        assert!(schedule.take_due(now).is_some());
        assert_eq!(schedule.take_due(now + Duration::from_secs(29)), None);
        assert!(schedule.take_due(now + Duration::from_secs(30)).is_some());
        assert_eq!(schedule.take_due(now + Duration::from_secs(59)), None);
        assert!(schedule.take_due(now + Duration::from_secs(60)).is_some());
    }

    #[test]
    fn duplicate_state_sync_does_not_repeat_immediate_cleanup() {
        let now = Instant::now();
        let mut schedule = Schedule::new();
        schedule.update(true, Vec::new(), now);
        let generation = schedule.take_due(now).unwrap();
        assert!(!schedule.update(true, Vec::new(), now + Duration::from_secs(1)));
        assert_eq!(schedule.generation, generation);
        assert_eq!(schedule.take_due(now + Duration::from_secs(1)), None);
    }

    #[test]
    fn disable_cancels_old_sweep_even_after_reenable() {
        let now = Instant::now();
        let state = Mutex::new(Schedule::new());
        state.lock().unwrap().update(true, Vec::new(), now);
        let generation = state.lock().unwrap().take_due(now).unwrap();
        let lease = CleanupLease {
            state: &state,
            generation,
            app_paths: Vec::new(),
        };
        assert!(lease.is_active());
        assert_eq!(
            lease.with_active(|| "allowed while signed out"),
            Some("allowed while signed out")
        );
        let mut terminated = false;
        state.lock().unwrap().update(false, Vec::new(), now);
        assert!(lease.with_active(|| terminated = true).is_none());
        state.lock().unwrap().update(true, Vec::new(), now);
        assert!(lease.with_active(|| terminated = true).is_none());
        assert!(!terminated);
    }
}
