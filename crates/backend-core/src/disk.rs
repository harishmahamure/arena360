//! Filesystem pressure, component allocation and reversible background admission.
use crate::{background::BackgroundJobs, error::AppError, metrics::Metrics};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
#[derive(Debug, Clone, Default)]
pub struct Sample {
    pub total: u64,
    pub available: u64,
    pub components: [u64; 6],
    pub scan_complete: bool,
}
const NAMES: [&str; 6] = [
    "sqlite",
    "sqlite_wal",
    "spool",
    "duckdb",
    "temporary",
    "other",
];
impl Sample {
    pub fn used_permille(&self) -> u64 {
        if self.total == 0 {
            1000
        } else {
            ((self.total.saturating_sub(self.available) as u128 * 1000) / self.total as u128) as u64
        }
    }
    pub fn render(&self) -> String {
        let mut text=format!("# TYPE arena360_disk_total_bytes gauge\narena360_disk_total_bytes {}\n# TYPE arena360_disk_available_bytes gauge\narena360_disk_available_bytes {}\n# TYPE arena360_disk_used_ratio gauge\narena360_disk_used_ratio {}\n# TYPE arena360_disk_component_scan_complete gauge\narena360_disk_component_scan_complete {}\n# TYPE arena360_disk_component_allocated_bytes gauge\n",self.total,self.available,self.used_permille() as f64/1000.0,self.scan_complete as u8);
        for (name, bytes) in NAMES.iter().zip(self.components) {
            text.push_str(&format!(
                "arena360_disk_component_allocated_bytes{{component=\"{name}\"}} {bytes}\n"
            ));
        }
        text
    }
}
/// Two percentage points of recovery hysteresis avoid pause/resume oscillation.
pub fn zone(used: u64, previous: u64) -> u64 {
    let raw = if used >= 900 {
        3
    } else if used >= 800 {
        2
    } else if used >= 700 {
        1
    } else {
        0
    };
    if previous <= 3 && raw < previous && used >= [0, 680, 780, 880][previous as usize] {
        previous
    } else {
        raw
    }
}
fn category(path: &Path) -> usize {
    let parts: Vec<_> = path.iter().filter_map(|s| s.to_str()).collect();
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if parts.iter().any(|p| {
        p.starts_with("recovery-staging")
            || p.starts_with("analytics-rebuild")
            || p.starts_with("rebuild-")
            || p.starts_with("archive-staging")
            || p.starts_with("export-staging")
    }) || parts.windows(2).any(|p| p == ["replication", "snapshots"])
        || name.ends_with(".tmp")
    {
        4
    } else if parts
        .windows(2)
        .any(|p| p == ["replication", "spool"] || p == ["replication", "batches"])
    {
        2
    } else if name.ends_with(".duckdb") || name.ends_with(".duckdb.wal") {
        3
    } else if name.ends_with(".sqlite-wal")
        || name.ends_with(".sqlite-shm")
        || name.ends_with(".db-wal")
        || name.ends_with(".db-shm")
    {
        1
    } else if name.ends_with(".sqlite") || name.ends_with(".db") {
        0
    } else {
        5
    }
}
fn fail(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("Disk monitor: {e}"))
}
/// Component scans are bounded and never follow symlinks. Pressure uses the
/// whole filesystem, not just the sum of tenant files. Races with cleanup are normal.
pub fn measure(root: &Path) -> Result<Sample, AppError> {
    let total = fs2::total_space(root).map_err(fail)?;
    let available = fs2::available_space(root).map_err(fail)?;
    if total == 0 {
        return Err(fail("Filesystem reports zero capacity"));
    }
    let mut sample = Sample {
        total,
        available,
        scan_complete: true,
        ..Default::default()
    };
    let mut pending = vec![root.to_owned()];
    let start = Instant::now();
    let mut visited = 0;
    #[cfg(unix)]
    let mut inodes = std::collections::HashSet::new();
    while let Some(directory) = pending.pop() {
        let entries = match std::fs::read_dir(&directory) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                sample.scan_complete = false;
                continue;
            }
        };
        for entry in entries {
            visited += 1;
            if visited > 100_000 || start.elapsed() > Duration::from_secs(2) {
                sample.scan_complete = false;
                return Ok(sample);
            }
            let entry = match entry {
                Ok(e) => e,
                Err(_) => {
                    sample.scan_complete = false;
                    continue;
                }
            };
            let kind = match entry.file_type() {
                Ok(k) => k,
                Err(_) => {
                    sample.scan_complete = false;
                    continue;
                }
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                pending.push(entry.path());
                continue;
            }
            if !kind.is_file() {
                continue;
            }
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(_) => {
                    sample.scan_complete = false;
                    continue;
                }
            };
            #[cfg(unix)]
            let bytes = {
                use std::os::unix::fs::MetadataExt;
                if !inodes.insert((metadata.dev(), metadata.ino())) {
                    continue;
                }
                metadata.blocks().saturating_mul(512)
            };
            #[cfg(not(unix))]
            let bytes = metadata.len();
            let path = entry.path();
            let component = category(path.strip_prefix(root).unwrap_or(&path));
            sample.components[component] = sample.components[component].saturating_add(bytes);
        }
    }
    Ok(sample)
}
pub fn spawn(root: PathBuf, jobs: Arc<BackgroundJobs>, metrics: Arc<Metrics>) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(15));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            let path = root.clone();
            match tokio::task::spawn_blocking(move || measure(&path)).await {
                Ok(Ok(sample)) => {
                    let next = zone(sample.used_permille(), jobs.stats().disk_zone());
                    if next != jobs.stats().disk_zone() {
                        tracing::warn!(
                            zone = next,
                            used_permille = sample.used_permille(),
                            "Cell disk pressure zone changed"
                        );
                    }
                    jobs.set_disk_zone(next);
                    metrics.set_disk(sample, next);
                }
                result => {
                    jobs.set_disk_zone(4);
                    metrics.disk_failed();
                    tracing::warn!(
                        ?result,
                        "Disk measurement unavailable; low-priority admission paused"
                    );
                }
            }
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thresholds_recover_with_hysteresis_and_unknown_can_recover() {
        for (used, expected) in [(699, 0), (700, 1), (799, 1), (800, 2), (899, 2), (900, 3)] {
            assert_eq!(zone(used, 0), expected);
        }
        assert_eq!(zone(890, 3), 3);
        assert_eq!(zone(879, 3), 2);
        assert_eq!(zone(790, 2), 2);
        assert_eq!(zone(779, 2), 1);
        assert_eq!(zone(690, 1), 1);
        assert_eq!(zone(679, 1), 0);
        assert_eq!(zone(600, 4), 0);
    }
    #[test]
    fn measures_all_components_without_following_external_links_or_double_counting() {
        let root = std::env::temp_dir().join(format!("arena360-disk-{}", uuid::Uuid::new_v4()));
        let files = [
            "tenant-x/operational.sqlite",
            "tenant-x/operational.sqlite-wal",
            "tenant-x/replication/spool/1.wal",
            "tenant-x/analytics.duckdb",
            "tenant-x/replication/snapshots/1.db",
            "tenant-x/replication/capture-sequence",
        ];
        for path in files {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, vec![7u8; 8192]).unwrap();
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(std::env::temp_dir(), root.join("external")).unwrap();
            std::fs::hard_link(
                root.join(files[0]),
                root.join("tenant-x/operational-copy.sqlite"),
            )
            .unwrap();
        }
        let rebuild = root.join("tenant-x/rebuild-fixture/source.sqlite");
        std::fs::create_dir_all(rebuild.parent().unwrap()).unwrap();
        std::fs::write(rebuild, vec![8u8; 8192]).unwrap();
        let sample = measure(&root).unwrap();
        assert!(sample.total > 0 && sample.scan_complete);
        assert!(sample.components.iter().all(|&n| n > 0));
        #[cfg(unix)]
        assert_eq!(sample.components[0], sample.components[1]);
        assert!(sample.render().contains("component=\"spool\""));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn pressure_pauses_growth_and_resumes_without_blocking_critical_work_or_purge() {
        use crate::background::{Limits, Priority};
        let jobs = BackgroundJobs::new(Limits::default()).unwrap();
        jobs.set_disk_zone(3);
        for p in [
            Priority::HotBackfill,
            Priority::SchemaBackfill,
            Priority::ArchiveExport,
            Priority::HistoricalExport,
            Priority::Maintenance,
            Priority::AnalyticsIngestion,
        ] {
            assert!(
                tokio::time::timeout(Duration::from_millis(15), jobs.acquire(p))
                    .await
                    .is_err()
            );
        }
        for p in [
            Priority::Outbox,
            Priority::CriticalRecovery,
            Priority::Backup,
            Priority::ArchivePurge,
        ] {
            drop(
                tokio::time::timeout(Duration::from_secs(1), jobs.acquire(p))
                    .await
                    .unwrap()
                    .unwrap(),
            );
        }
        jobs.set_disk_zone(4);
        assert!(jobs.stats().disk_blocked(Priority::AnalyticsIngestion));
        assert!(tokio::time::timeout(
            Duration::from_millis(15),
            jobs.acquire(Priority::HistoricalExport)
        )
        .await
        .is_err());
        let foreground = jobs.operational();
        assert_eq!(jobs.stats().active(Priority::Operational), 1);
        drop(foreground);
        let copy = jobs.clone();
        let paused =
            tokio::spawn(async move { copy.acquire(Priority::HotBackfill).await.unwrap() });
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(!paused.is_finished());
        jobs.set_disk_zone(0);
        drop(
            tokio::time::timeout(Duration::from_secs(1), paused)
                .await
                .unwrap()
                .unwrap(),
        );
    }
}
