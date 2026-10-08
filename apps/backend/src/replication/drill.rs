//! Weekly restore-only drills: production ownership and images stay live.
use super::{recovery::Recoverer, restore};
use crate::{background::Priority, error::AppError, metrics::Metrics};
use chrono::{DateTime, Utc};
use sqlx::Row;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct TenantResult {
    pub tenant: Uuid,
    pub elapsed_milliseconds: u64,
    pub recovered_at: Option<DateTime<Utc>>,
    pub capture_number: Option<u64>,
    pub source_generation: Option<Uuid>,
    pub image_bytes: Option<u64>,
    pub error: Option<String>,
}
#[derive(Debug, serde::Serialize)]
pub struct Report {
    pub id: Uuid,
    pub cell: Uuid,
    pub passed: bool,
    pub elapsed_milliseconds: u64,
    pub tenants: Vec<TenantResult>,
}
pub struct Drill {
    pub recovery: Arc<Recoverer>,
    pub metrics: Arc<Metrics>,
}
impl Drill {
    /// Cell-wide advisory locking prevents duplicate runs across process restarts.
    /// `force` is for operator/tests; the automatic scheduler always uses false.
    pub async fn run(&self, force: bool) -> Result<Option<Report>, AppError> {
        self.metrics.enable_restore_drills();
        let ledger = &self.recovery.ledger;
        if let Some(row) = sqlx::query("SELECT status,elapsed_milliseconds,extract(epoch FROM finished_at)::bigint FROM cell_restore_drills WHERE cell_id=$1 AND status <> 'RUNNING' ORDER BY started_at DESC LIMIT 1")
            .bind(ledger.cell_id).fetch_optional(&ledger.pool).await? {
            self.metrics.set_restore_drill(row.get::<String,_>(0)=="PASSED",row.get::<i64,_>(1) as u64,row.get::<i64,_>(2) as u64);
        }
        let mut serial = ledger.pool.begin().await?;
        let locked: bool =
            sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1,0))")
                .bind(format!("restore-drill:{}", ledger.cell_id))
                .fetch_one(&mut *serial)
                .await?;
        if !locked {
            serial.rollback().await?;
            return Ok(None);
        }
        if !force {
            let due: bool = sqlx::query_scalar("SELECT COALESCE((SELECT status<>'PASSED' OR started_at<=clock_timestamp()-INTERVAL '7 days' FROM cell_restore_drills WHERE cell_id=$1 ORDER BY started_at DESC LIMIT 1),true) AND NOT EXISTS(SELECT 1 FROM cell_restore_drills WHERE cell_id=$1 AND started_at>clock_timestamp()-INTERVAL '1 hour')")
                .bind(ledger.cell_id).fetch_one(&mut *serial).await?;
            if !due {
                serial.rollback().await?;
                return Ok(None);
            }
        }
        // A prior process died mid-drill. Its partial results remain inspectable.
        sqlx::query("UPDATE cell_restore_drills SET status='FAILED',finished_at=clock_timestamp(),elapsed_milliseconds=(extract(epoch FROM clock_timestamp()-started_at)*1000)::bigint WHERE cell_id=$1 AND status='RUNNING'")
            .bind(ledger.cell_id).execute(&ledger.pool).await?;
        // A killed process cannot run Drop cleanup. This private directory is
        // owned only by drills for this cell, and the advisory lock excludes a
        // live predecessor. Do not follow symlinks or remove unrelated entries.
        let directory = self.recovery.staging_root.join("drills");
        match std::fs::read_dir(&directory) {
            Ok(entries) => {
                for entry in entries.take(10_000) {
                    let entry = entry.map_err(|e| AppError::Internal(e.to_string()))?;
                    let name = entry.file_name();
                    if entry
                        .file_type()
                        .map_err(|e| AppError::Internal(e.to_string()))?
                        .is_dir()
                        && name.to_str().is_some_and(|n| n.starts_with("restore-"))
                    {
                        std::fs::remove_dir_all(entry.path()).map_err(|e| {
                            AppError::Internal(format!("Restore drill orphan cleanup: {e}"))
                        })?;
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(AppError::Internal(e.to_string())),
        }
        let tenants: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM tenants WHERE owner_cell=$1 AND storage_engine='SQLITE' AND state='ACTIVE' ORDER BY id")
            .bind(ledger.cell_id).fetch_all(&ledger.pool).await?;
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO cell_restore_drills(id,cell_id,tenant_count) VALUES($1,$2,$3)")
            .bind(id)
            .bind(ledger.cell_id)
            .bind(tenants.len() as i32)
            .execute(&ledger.pool)
            .await?;
        let start = Instant::now();
        let mut results = Vec::with_capacity(tenants.len());
        for tenant in tenants {
            let start = Instant::now();
            let restored = tokio::time::timeout(Duration::from_secs(600), async {
                let _permit = match self.recovery.databases.background_jobs() {
                    Some(jobs) => Some(jobs.acquire(Priority::Maintenance).await?),
                    None => None,
                };
                restore::restore(
                    tenant,
                    self.recovery.leases.clone(),
                    ledger,
                    self.recovery.store.as_ref(),
                    &self.recovery.keys,
                    &self.recovery.staging_root.join("drills"),
                    None,
                    restore::Limits::default(),
                )
                .await
            })
            .await;
            let mut result = TenantResult {
                tenant,
                elapsed_milliseconds: start.elapsed().as_millis() as u64,
                recovered_at: None,
                capture_number: None,
                source_generation: None,
                image_bytes: None,
                error: None,
            };
            match restored {
                Ok(Ok(image)) => {
                    result.recovered_at=Some(image.recovered_at);
                    result.capture_number=Some(image.capture_number);
                    result.source_generation=Some(image.generation);
                    result.image_bytes=std::fs::metadata(&image.image).ok().map(|m|m.len());
                    if let Err(e)=std::fs::remove_dir_all(image.image.parent().unwrap()) {
                        result.error=Some(format!("Restored successfully but staging cleanup failed: {e}"));
                    }
                },
                Ok(Err(e)) => result.error=Some(e.to_string()),
                Err(_) => result.error=Some("Restore drill exceeded 10-minute per-tenant deadline (including admission wait)".into()),
            }
            results.push(result);
            sqlx::query("UPDATE cell_restore_drills SET results=$2 WHERE id=$1")
                .bind(id)
                .bind(
                    serde_json::to_value(&results)
                        .map_err(|e| AppError::Internal(e.to_string()))?,
                )
                .execute(&ledger.pool)
                .await?;
        }
        let elapsed = start.elapsed().as_millis() as u64;
        let passed = results.iter().all(|r| r.error.is_none());
        let finished:i64=sqlx::query_scalar("UPDATE cell_restore_drills SET status=$2,finished_at=clock_timestamp(),elapsed_milliseconds=$3 WHERE id=$1 RETURNING extract(epoch FROM finished_at)::bigint")
            .bind(id).bind(if passed {"PASSED"}else{"FAILED"}).bind(elapsed as i64).fetch_one(&ledger.pool).await?;
        self.metrics
            .set_restore_drill(passed, elapsed, finished as u64);
        serial.commit().await?;
        Ok(Some(Report {
            id,
            cell: ledger.cell_id,
            passed,
            elapsed_milliseconds: elapsed,
            tenants: results,
        }))
    }
    pub fn spawn(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(60));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            tick.tick().await;
            loop {
                tick.tick().await;
                match self.run(false).await {
                    Ok(Some(report))=>tracing::info!(?report,"Weekly restore-only drill finished; measured time excludes activation and analytics"),
                    Ok(None)=>{},
                    Err(error)=>tracing::warn!(%error,"Weekly restore drill failed to run"),
                }
            }
        });
    }
}
