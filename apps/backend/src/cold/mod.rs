//! Cold tenants retain verified remote OLTP and keys; no local files or lease.
use crate::{
    control::{LeaseConfig, LeaseRepository},
    error::AppError,
    replication::{recovery::Recoverer, snapshot, worker::Worker},
    tenancy::TenantDb,
};
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool, Row};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;
#[derive(Clone, Debug, FromRow, serde::Serialize)]
pub struct Job {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub source_cell: Uuid,
    pub source_ownership_generation: i64,
    pub minimum_idle_seconds: i64,
    pub phase: String,
    pub source_cleaned_at: Option<DateTime<Utc>>,
    pub released_at: Option<DateTime<Utc>>,
    pub target_cell: Option<Uuid>,
    pub target_ownership_generation: Option<i64>,
    pub hydration_milliseconds: Option<i64>,
    pub last_error: Option<String>,
}
fn fail(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("Cold lifecycle: {e}"))
}
fn pending() -> AppError {
    AppError::Api {
        code: "TENANT_HYDRATION_PENDING".into(),
        status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
        details: None,
    }
}
pub async fn get(pool: &PgPool, id: Uuid) -> Result<Job, AppError> {
    sqlx::query_as("SELECT * FROM tenant_cold_jobs WHERE id=$1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Cold transition not found".into()))
}
pub async fn enqueue(pool: &PgPool, tenant: Uuid, idle: i64) -> Result<Job, AppError> {
    if !(0..=31536000).contains(&idle) {
        return Err(AppError::BadRequest(
            "Idle seconds must be 0..31536000".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    let row=sqlx::query("SELECT owner_cell,ownership_generation,state,schema_version FROM tenants WHERE id=$1 FOR UPDATE").bind(tenant).fetch_optional(&mut *tx).await?.ok_or_else(||AppError::NotFound("Tenant not found".into()))?;
    let owner: Option<Uuid> = row.get(0);
    let generation: i64 = row.get(1);
    if row.get::<String, _>(2) != "ACTIVE"
        || owner.is_none()
        || row.get::<i64, _>(3) != crate::tenancy::target_schema_version()
    {
        return Err(AppError::Conflict(
            "Cooling requires an assigned ACTIVE tenant on the current schema".into(),
        ));
    }
    let busy:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenant_moves WHERE tenant_id=$1 AND phase NOT IN ('ACTIVE','CANCELLED')) OR EXISTS(SELECT 1 FROM schema_rollout_tenants m JOIN schema_rollouts r ON r.id=m.rollout_id WHERE m.tenant_id=$1 AND m.state<>'SUCCEEDED' AND r.state<>'COMPLETE')").bind(tenant).fetch_one(&mut *tx).await?;
    if busy {
        return Err(AppError::Conflict(
            "Tenant move or schema rollout is pending".into(),
        ));
    }
    let fresh:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenant_leases WHERE tenant_id=$1 AND owner_cell=$2 AND ownership_generation=$3 AND expires_at>clock_timestamp()+INTERVAL '30 seconds')").bind(tenant).bind(owner).bind(generation).fetch_one(&mut *tx).await?;
    if !fresh {
        return Err(AppError::Forbidden(
            "Cooling requires a fresh source lease".into(),
        ));
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO tenant_cold_jobs(id,tenant_id,source_cell,source_ownership_generation,minimum_idle_seconds) VALUES($1,$2,$3,$4,$5)").bind(id).bind(tenant).bind(owner).bind(generation).bind(idle).execute(&mut *tx).await?;
    tx.commit().await?;
    get(pool, id).await
}
pub async fn cancel(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let job = get(pool, id).await?;
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT id FROM tenants WHERE id=$1 FOR UPDATE")
        .bind(job.tenant_id)
        .fetch_one(&mut *tx)
        .await?;
    let changed=sqlx::query("UPDATE tenant_cold_jobs SET phase='CANCELLED',updated_at=clock_timestamp() WHERE id=$1 AND phase='SNAPSHOTTING'").bind(id).execute(&mut *tx).await?.rows_affected();
    if changed != 1 {
        return Err(AppError::Conflict(
            "Only an unreleased cold job can be cancelled".into(),
        ));
    }
    tx.commit().await?;
    Ok(())
}
pub struct Agent {
    pub recovery: Arc<Recoverer>,
    pub worker: Arc<Worker>,
}
impl Agent {
    pub async fn heartbeat(&self) -> Result<(), AppError> {
        sqlx::query("UPDATE cells SET hydration_heartbeat_at=clock_timestamp() WHERE id=$1")
            .bind(self.recovery.ledger.cell_id)
            .execute(&self.recovery.ledger.pool)
            .await?;
        Ok(())
    }
    async fn inactivity(&self, db: &TenantDb, job: &Job) -> Result<(), AppError> {
        let last = db.with_writer(|c|Box::pin(async move{
   let live:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM usage_sessions WHERE end_time IS NULL) OR EXISTS(SELECT 1 FROM shifts WHERE status='active')").fetch_one(&mut *c).await?;
   if live{return Err(AppError::Conflict("Tenant has an open session or shift".into()));}
   let last:Option<String>=sqlx::query_scalar("SELECT MAX(occurred_at) FROM outbox_events").fetch_one(c).await?;Ok(last)
  })).await.and_then(|last|match last{Some(last)=>crate::time::parse_sqlite_timestamp(&last).map(Some).map_err(fail),None=>Ok(None)})?;
        let (created, now): (DateTime<Utc>, DateTime<Utc>) =
            sqlx::query_as("SELECT created_at,clock_timestamp() FROM tenants WHERE id=$1")
                .bind(job.tenant_id)
                .fetch_one(&self.recovery.ledger.pool)
                .await?;
        if now - last.unwrap_or(created) < chrono::Duration::seconds(job.minimum_idle_seconds) {
            return Err(AppError::Conflict(
                "Tenant has recent operational activity".into(),
            ));
        }
        Ok(())
    }
    pub async fn cool(&self, id: Uuid) -> Result<(), AppError> {
        let pool = &self.recovery.ledger.pool;
        let mut serial = pool.begin().await?;
        let locked: bool =
            sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1,0))")
                .bind(format!("cold-source:{id}"))
                .fetch_one(&mut *serial)
                .await?;
        if !locked {
            return Ok(());
        }
        let job = get(pool, id).await?;
        if job.source_cell != self.recovery.ledger.cell_id || job.phase != "SNAPSHOTTING" {
            return Err(AppError::Conflict(
                "Cell has no pending cold source job".into(),
            ));
        }
        let state: Option<(Option<Uuid>, i64, String, i64)> = sqlx::query_as(
            "SELECT owner_cell,ownership_generation,state,schema_version FROM tenants WHERE id=$1",
        )
        .bind(job.tenant_id)
        .fetch_optional(pool)
        .await?;
        if state
            != Some((
                Some(job.source_cell),
                job.source_ownership_generation,
                "ACTIVE".into(),
                crate::tenancy::target_schema_version(),
            ))
        {
            sqlx::query("UPDATE tenant_cold_jobs SET phase='CANCELLED',last_error='Ownership or schema changed before cooling' WHERE id=$1").bind(id).execute(pool).await?;
            serial.rollback().await?;
            return Ok(());
        }
        self.recovery
            .leases
            .resume_move_source(job.tenant_id, job.source_ownership_generation)
            .await?;
        let db = self.recovery.databases.open(job.tenant_id).await?;
        self.inactivity(&db, &job).await?;
        let _permit = match db.background_jobs() {
            Some(j) => Some(j.acquire(crate::background::Priority::Maintenance).await?),
            None => None,
        };
        snapshot::take(
            db.clone(),
            &self.recovery.ledger,
            self.recovery.store.as_ref(),
            &self.recovery.keys,
            snapshot::Kind::Manual,
        )
        .await?;
        let generation = self.recovery.ledger.ensure_generation(&db).await?;
        let gate = self.worker.gate(job.tenant_id).await;
        let _publish = gate.lock().await;
        let context = self.recovery.clone();
        let worker = self.worker.clone();
        let gated = db.clone();
        let transition = job.clone();
        let result=db.with_writer(move|connection|Box::pin(async move{
   let live:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM usage_sessions WHERE end_time IS NULL) OR EXISTS(SELECT 1 FROM shifts WHERE status='active')").fetch_one(&mut *connection).await?;
   if live{return Err(AppError::Conflict("Tenant became active before cooling".into()));}
   let last:Option<String>=sqlx::query_scalar("SELECT MAX(occurred_at) FROM outbox_events").fetch_one(&mut *connection).await?;
   let (created,now):(DateTime<Utc>,DateTime<Utc>)=sqlx::query_as("SELECT created_at,clock_timestamp() FROM tenants WHERE id=$1").bind(transition.tenant_id).fetch_one(&context.ledger.pool).await?;
   let last=last.map(|v|crate::time::parse_sqlite_timestamp(&v).map_err(fail)).transpose()?.unwrap_or(created);
   if now-last<chrono::Duration::seconds(transition.minimum_idle_seconds){return Err(AppError::Conflict("Tenant has recent operational activity".into()));}
   crate::tenancy::spool_connection_wal(connection,gated.path(),transition.source_ownership_generation).await?;
   while worker.ship_locked(gated.clone(),true,Some(generation)).await?>0{}
   let capture=match std::fs::read_to_string(gated.path().parent().unwrap().join("replication/capture-sequence")){Ok(value)=>value.parse::<i64>().map_err(fail)?,Err(e) if e.kind()==std::io::ErrorKind::NotFound=>0,Err(e)=>return Err(fail(e))};
   let mut tx=context.ledger.pool.begin().await?;
   if context.ledger.lock_owner(&gated,&mut tx).await?!=Some(generation){return Err(AppError::Conflict("Cold backup generation changed".into()));}
   let phase:String=sqlx::query_scalar("SELECT phase FROM tenant_cold_jobs WHERE id=$1 FOR UPDATE").bind(transition.id).fetch_one(&mut *tx).await?;
   if phase!="SNAPSHOTTING" {return Err(AppError::Conflict("Cold job was cancelled".into()));}
   let eligible:bool=sqlx::query_scalar("SELECT state='ACTIVE' AND NOT EXISTS(SELECT 1 FROM schema_rollout_tenants m JOIN schema_rollouts r ON r.id=m.rollout_id WHERE m.tenant_id=t.id AND m.state<>'SUCCEEDED' AND r.state<>'COMPLETE') FROM tenants t WHERE id=$1").bind(transition.tenant_id).fetch_one(&mut *tx).await?;
   if !eligible{return Err(AppError::Conflict("Tenant transition changed before cooling".into()));}
   let snapshot:Uuid=sqlx::query_scalar("SELECT id FROM snapshot_manifests WHERE generation_id=$1 AND verified_at IS NOT NULL AND retired_at IS NULL AND source_checksum_sha256 IS NOT NULL AND capture_number<=$2 ORDER BY snapshot_at DESC,capture_number DESC LIMIT 1").bind(generation).bind(capture).fetch_optional(&mut *tx).await?.ok_or_else(||AppError::Conflict("Cold tenant has no verified restorable snapshot".into()))?;
   let marker=gated.path().parent().unwrap().join("replication/cold-pending.json");
   let expected=serde_json::to_vec(&serde_json::json!({"cold_job_id":transition.id,"ownership_generation":transition.source_ownership_generation})).map_err(fail)?;
   if marker.exists() && std::fs::read(&marker).map_err(fail)?!=expected {
    let prior:serde_json::Value=serde_json::from_slice(&std::fs::read(&marker).map_err(fail)?).map_err(fail)?;
    let prior_id=prior.get("cold_job_id").and_then(|v|v.as_str()).and_then(|v|Uuid::parse_str(v).ok()).ok_or_else(||fail("Invalid cold marker"))?;
    let cancelled:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenant_cold_jobs WHERE id=$1 AND tenant_id=$2 AND source_cell=$3 AND phase='CANCELLED')").bind(prior_id).bind(transition.tenant_id).bind(transition.source_cell).fetch_one(&mut *tx).await?;
    if !cancelled{return Err(AppError::Conflict("Another cold job owns the source marker".into()));}
    std::fs::remove_file(&marker).map_err(fail)?;
   }
   if !marker.exists(){crate::replication::wal::durable_create(&marker,&expected)?;}
   context.leases.fence_for_handoff(transition.tenant_id)?;
   sqlx::query("DELETE FROM tenant_leases WHERE tenant_id=$1 AND owner_cell=$2 AND ownership_generation=$3").bind(transition.tenant_id).bind(transition.source_cell).bind(transition.source_ownership_generation).execute(&mut *tx).await?;
   sqlx::query("UPDATE tenants SET state='COLD',owner_cell=NULL,updated_at=clock_timestamp() WHERE id=$1").bind(transition.tenant_id).execute(&mut *tx).await?;
   sqlx::query("UPDATE tenant_cold_jobs SET phase='RELEASED',source_generation=$2,snapshot_id=$3,capture_number=$4,released_at=clock_timestamp(),last_error=NULL,updated_at=clock_timestamp() WHERE id=$1 AND phase='SNAPSHOTTING'").bind(transition.id).bind(generation).bind(snapshot).bind(capture).execute(&mut *tx).await?;
   sqlx::query("SELECT pg_notify($1,$2)").bind(crate::routing::ROUTING_CHANGED_CHANNEL).bind(transition.tenant_id.to_string()).execute(&mut *tx).await?;tx.commit().await?;Ok(())
  })).await;
        // Fencing intentionally makes with_writer's final lease check fail. The
        // durable control result decides whether cleanup or source reconciliation runs.
        let current = get(pool, id).await?;
        if current.released_at.is_some() {
            self.cleanup(id).await?;
            serial.rollback().await?;
            return Ok(());
        }
        self.recovery
            .leases
            .resume_move_source(job.tenant_id, job.source_ownership_generation)
            .await?;
        if let Err(e) = result {
            return Err(e);
        }
        Err(AppError::Conflict(
            "Cold transition was not committed".into(),
        ))
    }
    pub async fn cleanup(&self, id: Uuid) -> Result<(), AppError> {
        let job = get(&self.recovery.ledger.pool, id).await?;
        if job.source_cell != self.recovery.ledger.cell_id
            || job.released_at.is_none()
            || job.source_cleaned_at.is_some()
        {
            return Ok(());
        }
        let mut tx = self.recovery.ledger.pool.begin().await?;
        let (owner, generation): (Option<Uuid>, i64) = sqlx::query_as(
            "SELECT owner_cell,ownership_generation FROM tenants WHERE id=$1 FOR UPDATE",
        )
        .bind(job.tenant_id)
        .fetch_one(&mut *tx)
        .await?;
        if let Some(analytics) = &self.recovery.analytics {
            analytics
                .retire(job.tenant_id, job.source_ownership_generation)
                .await?;
        }
        self.recovery
            .databases
            .cleanup_cold_copy(
                job.tenant_id,
                id,
                job.source_ownership_generation,
                owner != Some(job.source_cell) || generation == job.source_ownership_generation,
            )
            .await?;
        sqlx::query("UPDATE tenant_cold_jobs SET source_cleaned_at=clock_timestamp(),last_error=NULL,updated_at=clock_timestamp() WHERE id=$1").bind(id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn hydrate(&self, tenant: Uuid) -> Result<(), AppError> {
        let pool = &self.recovery.ledger.pool;
        sqlx::query("UPDATE tenant_cold_jobs j SET phase='HYDRATING',target_cell=t.owner_cell,target_ownership_generation=t.ownership_generation,hydration_requested_at=COALESCE(j.hydration_requested_at,t.updated_at),updated_at=clock_timestamp() FROM tenants t WHERE j.tenant_id=t.id AND t.id=$1 AND t.owner_cell=$2 AND t.state='RESTORING' AND j.phase IN ('RELEASED','HYDRATING') AND j.source_cleaned_at IS NOT NULL AND t.ownership_generation>j.source_ownership_generation").bind(tenant).bind(self.recovery.ledger.cell_id).execute(pool).await?;
        let operations = Arc::new(Recoverer {
            ledger: self.recovery.ledger.clone(),
            leases: self.recovery.leases.clone(),
            databases: self.recovery.databases.clone(),
            store: self.recovery.store.clone(),
            keys: self.recovery.keys.clone(),
            staging_root: self.recovery.staging_root.clone(),
            analytics: None,
        });
        operations.recover_tenant(tenant).await?;
        self.reconcile().await?;
        Ok(())
    }
    async fn reconcile(&self) -> Result<(), AppError> {
        // Recovery may finish before the coordinator records its assignment.
        sqlx::query("UPDATE tenant_cold_jobs j SET phase='ACTIVE',target_cell=r.cell_id,target_ownership_generation=r.ownership_generation,hydration_requested_at=r.started_at,operations_ready_at=r.operations_ready_at,hydration_milliseconds=GREATEST(0,(extract(epoch FROM r.operations_ready_at-r.started_at)*1000)::bigint),last_error=NULL,updated_at=clock_timestamp() FROM tenant_recovery_jobs r WHERE j.tenant_id=r.tenant_id AND j.source_generation=r.source_generation AND r.ownership_generation>j.source_ownership_generation AND r.started_at>=j.released_at AND r.operations_ready_at IS NOT NULL AND j.phase='RELEASED' AND j.source_cleaned_at IS NOT NULL").execute(&self.recovery.ledger.pool).await?;
        sqlx::query("UPDATE tenant_cold_jobs j SET phase='ACTIVE',operations_ready_at=r.operations_ready_at,hydration_milliseconds=GREATEST(0,(extract(epoch FROM r.operations_ready_at-j.hydration_requested_at)*1000)::bigint),last_error=NULL,updated_at=clock_timestamp() FROM tenant_recovery_jobs r WHERE j.tenant_id=r.tenant_id AND r.ownership_generation=j.target_ownership_generation AND r.cell_id=j.target_cell AND r.operations_ready_at IS NOT NULL AND j.phase='HYDRATING'").execute(&self.recovery.ledger.pool).await?;
        Ok(())
    }
    pub async fn tick(&self) -> Result<(), AppError> {
        self.reconcile().await?;
        let pool = &self.recovery.ledger.pool;
        let cell = self.recovery.ledger.cell_id;
        let jobs:Vec<Job>=sqlx::query_as("SELECT * FROM tenant_cold_jobs WHERE source_cell=$1 AND ((phase='SNAPSHOTTING' AND retry_after<=clock_timestamp()) OR (released_at IS NOT NULL AND source_cleaned_at IS NULL)) ORDER BY created_at LIMIT 8").bind(cell).fetch_all(pool).await?;
        for job in jobs {
            let result = if job.phase == "SNAPSHOTTING" {
                self.cool(job.id).await
            } else {
                self.cleanup(job.id).await
            };
            if let Err(error) = result {
                tracing::warn!(cold_job=%job.id,%error,"Cold source transition remains pending");
                sqlx::query("UPDATE tenant_cold_jobs SET last_error=$2,retry_after=clock_timestamp()+INTERVAL '30 seconds' WHERE id=$1 AND phase=$3").bind(job.id).bind(error.to_string()).bind(job.phase).execute(pool).await?;
            }
        }
        let tenants:Vec<Uuid>=sqlx::query_scalar("SELECT t.id FROM tenants t JOIN tenant_cold_jobs j ON j.tenant_id=t.id WHERE t.owner_cell=$1 AND t.state='RESTORING' AND j.phase IN ('RELEASED','HYDRATING') AND j.source_cleaned_at IS NOT NULL ORDER BY j.created_at LIMIT 8").bind(cell).fetch_all(pool).await?;
        for tenant in tenants {
            if let Err(error) = self.hydrate(tenant).await {
                tracing::warn!(%tenant,%error,"Cold hydration remains pending");
                sqlx::query("UPDATE tenant_cold_jobs SET last_error=$2 WHERE tenant_id=$1 AND phase IN ('RELEASED','HYDRATING')").bind(tenant).bind(error.to_string()).execute(pool).await?;
            }
        }
        Ok(())
    }
    pub fn spawn(self: Arc<Self>) {
        let heartbeat = self.clone();
        tokio::spawn(async move {
            let mut ticks = tokio::time::interval(Duration::from_secs(10));
            loop {
                ticks.tick().await;
                if let Err(e) = heartbeat.heartbeat().await {
                    tracing::warn!(%e,"Cold hydration heartbeat unavailable");
                }
            }
        });
        tokio::spawn(async move {
            let mut ticks = tokio::time::interval(Duration::from_secs(1));
            ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticks.tick().await;
                if let Err(e) = self.tick().await {
                    tracing::warn!(%e,"Cold lifecycle queue unavailable");
                }
            }
        });
    }
}
#[derive(Clone)]
pub struct Coordinator {
    pub pool: PgPool,
    pub wait_limit: Duration,
}
impl Coordinator {
    pub async fn wake(&self, tenant: Uuid) -> Result<(), AppError> {
        let started = Instant::now();
        loop {
            let row: Option<(String, Option<Uuid>, i64)> = sqlx::query_as(
                "SELECT state,owner_cell,ownership_generation FROM tenants WHERE id=$1",
            )
            .bind(tenant)
            .fetch_optional(&self.pool)
            .await?;
            let Some((state, _, _)) = row else {
                return Err(AppError::NotFound("Tenant not found".into()));
            };
            if state == "ACTIVE" {
                return Ok(());
            }
            let job:Option<Job>=sqlx::query_as("SELECT * FROM tenant_cold_jobs WHERE tenant_id=$1 AND phase IN ('RELEASED','HYDRATING') ORDER BY created_at DESC LIMIT 1").bind(tenant).fetch_optional(&self.pool).await?;
            let Some(job) = job else {
                return Err(pending());
            };
            if state == "COLD" && job.source_cleaned_at.is_some() {
                let mut reservation = self.pool.begin().await?;
                let target:Option<Uuid>=sqlx::query_scalar("SELECT c.id FROM cells c WHERE c.state='ACTIVE' AND c.hydration_heartbeat_at>clock_timestamp()-INTERVAL '30 seconds' ORDER BY (SELECT count(*) FROM tenants t WHERE t.owner_cell=c.id),c.id FOR SHARE OF c SKIP LOCKED LIMIT 1").fetch_optional(&mut *reservation).await?;
                if let Some(target) = target {
                    let lease = LeaseRepository::new(self.pool.clone())
                        .acquire_for_cold(tenant, target, LeaseConfig::default())
                        .await;
                    reservation.rollback().await?;
                    match lease {
                        Ok(grant) => {
                            sqlx::query("UPDATE tenant_cold_jobs SET phase='HYDRATING',target_cell=$2,target_ownership_generation=$3,hydration_requested_at=COALESCE(hydration_requested_at,clock_timestamp()),updated_at=clock_timestamp() WHERE id=$1 AND phase='RELEASED'").bind(job.id).bind(target).bind(grant.ownership_generation).execute(&self.pool).await?;
                        }
                        Err(AppError::Conflict(_)) => {}
                        Err(e) => return Err(e),
                    }
                } else {
                    reservation.rollback().await?;
                    return Err(pending());
                }
            } else if state != "COLD" && state != "RESTORING" {
                return Err(pending());
            }
            if started.elapsed() >= self.wait_limit {
                return Err(pending());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}
