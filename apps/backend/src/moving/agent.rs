//! Control-plane move queues are consumed by their explicitly assigned cells.
use super::{control, source, target};
use crate::{
    error::AppError,
    replication::{recovery::Recoverer, snapshot, worker::Worker},
};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;
pub struct Agent {
    pub recovery: Arc<Recoverer>,
    pub worker: Arc<Worker>,
}
impl Agent {
    pub async fn advance(&self, id: Uuid) -> Result<(), AppError> {
        let pool = &self.recovery.ledger.pool;
        let cell = self.recovery.ledger.cell_id;
        let job = control::get(pool, id).await?;
        if job.source_cell == cell {
            match job.phase.as_str() {
                "PREPARING_MOVE" => {
                    let mut serial = pool.begin().await?;
                    let locked: bool = sqlx::query_scalar(
                        "SELECT pg_try_advisory_xact_lock(hashtextextended($1,0))",
                    )
                    .bind(format!("move-source:{id}"))
                    .fetch_one(&mut *serial)
                    .await?;
                    if !locked {
                        return Ok(());
                    }
                    self.recovery
                        .leases
                        .resume_move_source(job.tenant_id, job.source_ownership_generation)
                        .await?;
                    let db = self.recovery.databases.open(job.tenant_id).await?;
                    snapshot::take(
                        db.clone(),
                        &self.recovery.ledger,
                        self.recovery.store.as_ref(),
                        &self.recovery.keys,
                        snapshot::Kind::Baseline,
                    )
                    .await?;
                    while self.worker.ship(db.clone(), true).await? > 0 {}
                    let generation = self.recovery.ledger.ensure_generation(&db).await?;
                    control::copying(pool, &job, generation).await?;
                }
                "COPYING"
                    if job.target_capture_number.is_some() && job.target_prepared_at.is_some() =>
                {
                    if !control::target_recent(pool, id).await? {
                        return Ok(());
                    }
                    self.recovery
                        .leases
                        .resume_move_source(job.tenant_id, job.source_ownership_generation)
                        .await?;
                    let db = self.recovery.databases.open(job.tenant_id).await?;
                    source::cutover(
                        pool,
                        db,
                        self.worker.clone(),
                        self.recovery.leases.clone(),
                        id,
                    )
                    .await?;
                }
                "CUTOVER" => {
                    let mut serial = pool.begin().await?;
                    let locked: bool = sqlx::query_scalar(
                        "SELECT pg_try_advisory_xact_lock(hashtextextended($1,0))",
                    )
                    .bind(format!("move-source:{id}"))
                    .fetch_one(&mut *serial)
                    .await?;
                    if !locked {
                        return Ok(());
                    }
                    // The source process died inside its writer gate. Reconcile
                    // only if PostgreSQL still assigns its exact generation.
                    self.recovery
                        .leases
                        .resume_move_source(job.tenant_id, job.source_ownership_generation)
                        .await?;
                    let mut tx = pool.begin().await?;
                    control::lock_owned(&mut tx, &job, true).await?;
                    sqlx::query("UPDATE tenant_moves SET phase='COPYING',final_capture_number=NULL,target_prepared_at=NULL,last_error='Source restarted during cutover; pre-copy resumes',updated_at=clock_timestamp() WHERE id=$1").bind(id).execute(&mut *tx).await?;
                    sqlx::query("UPDATE tenants SET state='COPYING' WHERE id=$1")
                        .bind(job.tenant_id)
                        .execute(&mut *tx)
                        .await?;
                    tx.commit().await?;
                }
                _ => {}
            }
        } else if job.target_cell == cell {
            match job.phase.as_str() {
                "COPYING" | "CUTOVER" => target::precopy(self.recovery.clone(), id).await?,
                "VERIFYING" => target::activate(self.recovery.clone(), id).await?,
                "ACTIVE" if self.recovery.databases.move_pending(job.tenant_id) => {
                    target::activate(self.recovery.clone(), id).await?
                }
                _ => {}
            }
        } else {
            return Err(AppError::Forbidden(
                "Cell is not assigned to this move".into(),
            ));
        }
        Ok(())
    }
    pub async fn tick(&self) -> Result<(), AppError> {
        let pool = &self.recovery.ledger.pool;
        let ids:Vec<Uuid>=sqlx::query_scalar("SELECT id FROM tenant_moves WHERE (source_cell=$1 OR target_cell=$1) AND phase NOT IN ('CANCELLED','ACTIVE') AND retry_after<=clock_timestamp() ORDER BY created_at LIMIT 32").bind(self.recovery.ledger.cell_id).fetch_all(pool).await?;
        for id in ids {
            let job = control::get(pool, id).await?;
            if job.phase == "ACTIVE" && !self.recovery.databases.move_pending(job.tenant_id) {
                continue;
            }
            if let Err(error) = self.advance(id).await {
                tracing::warn!(move_id=%id,%error,"Tenant move remains pending");
                sqlx::query("UPDATE tenant_moves SET last_error=$2,retry_after=clock_timestamp()+INTERVAL '30 seconds',updated_at=clock_timestamp() WHERE id=$1 AND phase=$3").bind(id).bind(error.to_string()).bind(job.phase).execute(pool).await?;
            }
        }
        Ok(())
    }
    pub async fn resume_sources(&self) -> Result<(), AppError> {
        let jobs:Vec<control::Move>=sqlx::query_as("SELECT m.* FROM tenant_moves m JOIN tenants t ON t.id=m.tenant_id WHERE m.source_cell=$1 AND m.phase IN ('PREPARING_MOVE','COPYING','CUTOVER') AND t.owner_cell=$1 AND t.ownership_generation=m.source_ownership_generation ORDER BY m.created_at")
            .bind(self.recovery.ledger.cell_id).fetch_all(&self.recovery.ledger.pool).await?;
        for job in jobs {
            self.recovery
                .leases
                .resume_move_source(job.tenant_id, job.source_ownership_generation)
                .await?;
        }
        Ok(())
    }
    pub async fn resume_completed(&self) -> Result<(), AppError> {
        let ids:Vec<Uuid>=sqlx::query_scalar("SELECT m.id FROM tenant_moves m JOIN tenants t ON t.id=m.tenant_id WHERE m.target_cell=$1 AND m.phase='ACTIVE' AND t.owner_cell=$1 AND t.ownership_generation=m.source_ownership_generation+1 ORDER BY m.created_at")
            .bind(self.recovery.ledger.cell_id).fetch_all(&self.recovery.ledger.pool).await?;
        for id in ids {
            let job = control::get(&self.recovery.ledger.pool, id).await?;
            if self.recovery.databases.move_pending(job.tenant_id) {
                if let Err(error) = target::activate(self.recovery.clone(), id).await {
                    tracing::warn!(move_id=%id,%error,"Completed move quarantine remains pending");
                }
            }
        }
        Ok(())
    }
    pub async fn cleanup(&self) -> Result<(), AppError> {
        let pool = &self.recovery.ledger.pool;
        let cell = self.recovery.ledger.cell_id;
        let jobs:Vec<control::Move>=sqlx::query_as("SELECT * FROM tenant_moves WHERE ((phase='ACTIVE' AND retain_source_until<=clock_timestamp()) OR phase='CANCELLED') AND (source_cell=$1 OR target_cell=$1) ORDER BY created_at LIMIT 1000").bind(cell).fetch_all(pool).await?;
        for job in jobs {
            let mut tx = pool.begin().await?;
            let owner: Option<(Option<Uuid>, i64)> = sqlx::query_as(
                "SELECT owner_cell,ownership_generation FROM tenants WHERE id=$1 FOR UPDATE",
            )
            .bind(job.tenant_id)
            .fetch_optional(&mut *tx)
            .await?;
            let Some((owner, generation)) = owner else {
                continue;
            };
            let pending:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenant_moves WHERE tenant_id=$1 AND target_cell=$2 AND phase NOT IN ('ACTIVE','CANCELLED'))").bind(job.tenant_id).bind(cell).fetch_one(&mut *tx).await?;
            let newer_departure: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenant_moves WHERE tenant_id=$1 AND source_cell=$2 AND source_ownership_generation>$3 AND phase IN ('VERIFYING','ACTIVE'))")
                .bind(job.tenant_id).bind(cell).bind(job.source_ownership_generation).fetch_one(&mut *tx).await?;
            if owner != Some(cell)
                && !newer_departure
                && !pending
                && (job.phase == "CANCELLED" || generation > job.source_ownership_generation)
            {
                self.recovery
                    .databases
                    .cleanup_move_copy(job.tenant_id, job.id, true, job.phase == "CANCELLED")
                    .await?;
            }
            let cancelled_expired: bool = if job.phase == "CANCELLED" {
                sqlx::query_scalar("SELECT updated_at<clock_timestamp()-INTERVAL '7 days' FROM tenant_moves WHERE id=$1").bind(job.id).fetch_one(&mut *tx).await?
            } else {
                false
            };
            if (job.phase == "ACTIVE" || cancelled_expired) && job.target_cell == cell {
                self.recovery
                    .databases
                    .cleanup_move_copy(job.tenant_id, job.id, false, false)
                    .await?;
            }
            tx.rollback().await?;
            if job.phase == "CANCELLED" {
                let root = self
                    .recovery
                    .staging_root
                    .join("moves")
                    .join(job.id.to_string());
                if root.exists() {
                    std::fs::remove_dir_all(root).map_err(|e| AppError::Internal(e.to_string()))?;
                }
            }
        }
        Ok(())
    }
    pub fn spawn(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_millis(100));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let mut reconcile = tokio::time::interval(Duration::from_secs(60));
            reconcile.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    _=tick.tick()=>if let Err(error)=self.tick().await {tracing::warn!(%error,"Tenant move queue unavailable");tokio::time::sleep(Duration::from_secs(1)).await;},
                    _=reconcile.tick()=>{
                        if let Err(error)=self.resume_completed().await {tracing::warn!(%error,"Move completion reconciliation unavailable");}
                        if let Err(error)=self.cleanup().await {tracing::warn!(%error,"Move copy cleanup unavailable");}
                    },
                }
            }
        });
    }
}
