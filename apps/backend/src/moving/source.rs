//! Source-side bounded cutover. Target preparation runs on the other cell.
use super::control::{self, Move};
use crate::{
    background::Priority,
    control::{LeaseClient, LeaseGrant},
    error::AppError,
    replication::worker::Worker,
    tenancy::TenantDb,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;
/// Asynchronous cutover has a five-second deadline. Synchronous SQLite/file
/// work is not preempted; publish measured elapsed time for capacity checks.
/// A missed deadline resumes source writes without transferring ownership.
pub const WRITE_GATE_LIMIT: Duration = Duration::from_secs(5);
pub async fn cutover(
    pool: &sqlx::PgPool,
    db: Arc<TenantDb>,
    worker: Arc<Worker>,
    leases: Arc<LeaseClient>,
    id: Uuid,
) -> Result<LeaseGrant, AppError> {
    let mut serial = pool.begin().await?;
    let locked: bool =
        sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1,0))")
            .bind(format!("move-source:{id}"))
            .fetch_one(&mut *serial)
            .await?;
    if !locked {
        return Err(AppError::Conflict(
            "Source cutover is already running".into(),
        ));
    }
    let job = control::get(pool, id).await?;
    if job.phase != "COPYING"
        || job.tenant_id != db.tenant_id()
        || job.source_cell != leases.cell_id()
        || job.source_ownership_generation != db.ownership_generation()
        || job.target_capture_number.is_none()
        || job.target_prepared_at.is_none()
    {
        return Err(AppError::Conflict(
            "Move is not ready for this source writer".into(),
        ));
    }
    if !control::target_recent(pool, id).await? {
        return Err(AppError::Conflict("Move target readiness is stale".into()));
    }
    let generation = job
        .source_replication_generation
        .ok_or_else(|| AppError::Conflict("Move has no verified source generation".into()))?;
    let baseline:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM snapshot_manifests WHERE generation_id=$1 AND verified_at IS NOT NULL AND retired_at IS NULL AND source_checksum_sha256 IS NOT NULL)").bind(generation).fetch_one(pool).await?;
    if !baseline {
        return Err(AppError::Conflict("Move source baseline is missing".into()));
    }
    let _permit = match db.background_jobs() {
        Some(j) => Some(j.acquire(Priority::CriticalRecovery).await?),
        None => None,
    };
    // Match normal snapshot/upload lock ordering: publication, then writer.
    let gate = worker.gate(job.tenant_id).await;
    let _serial = gate.lock().await;
    let started = Instant::now();
    let moved = job.clone();
    let conn_pool = pool.clone();
    let tenant_db = db.clone();
    let client = leases.clone();
    let result=tokio::time::timeout(WRITE_GATE_LIMIT,db.with_writer(move|connection|Box::pin(async move {
        crate::tenancy::spool_connection_wal(connection,tenant_db.path(),moved.source_ownership_generation).await?;
        while worker.ship_locked(tenant_db.clone(),true,Some(generation)).await?>0 {}
        let sequence_path=tenant_db.path().parent().unwrap().join("replication/capture-sequence");
        let final_capture=match std::fs::read_to_string(sequence_path) {Ok(n)=>n.parse::<i64>().map_err(|e|AppError::Internal(e.to_string()))?,Err(e) if e.kind()==std::io::ErrorKind::NotFound=>0,Err(e)=>return Err(AppError::Internal(e.to_string()))};
        let mut tx=conn_pool.begin().await?;control::lock_owned(&mut tx,&moved,true).await?;
        let selected:bool=sqlx::query_scalar("SELECT current_replication_generation=$2 FROM tenants WHERE id=$1").bind(moved.tenant_id).bind(generation).fetch_one(&mut *tx).await?;
        if !selected {return Err(AppError::Conflict("Move source generation changed at cutover".into()));}
        sqlx::query("UPDATE tenant_moves SET phase='CUTOVER',final_capture_number=$2,target_prepared_at=NULL,cutover_started_at=clock_timestamp(),last_error=NULL,updated_at=clock_timestamp() WHERE id=$1").bind(moved.id).bind(final_capture).execute(&mut *tx).await?;
        sqlx::query("UPDATE tenants SET state='CUTOVER',updated_at=clock_timestamp() WHERE id=$1").bind(moved.tenant_id).execute(&mut *tx).await?;
        tx.commit().await?;
        loop {
            let ready:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenant_moves WHERE id=$1 AND phase='CUTOVER' AND target_capture_number=final_capture_number AND target_prepared_at IS NOT NULL AND target_image_checksum IS NOT NULL)").bind(moved.id).fetch_one(&conn_pool).await?;
            if ready {break;}tokio::time::sleep(Duration::from_millis(25)).await;
        }
        client.handoff_move(moved.tenant_id,moved.target_cell,moved.id).await
    }))).await;
    let result = match result {
        Ok(r) => r,
        Err(_) => Err(AppError::Conflict(
            "Move exceeded the five-second write gate; source remains authoritative".into(),
        )),
    };
    let elapsed = started.elapsed().as_millis() as i64;
    // A lost COMMIT response must never cause ownership to be transferred back.
    let current = control::get(pool, id).await?;
    if current.phase == "VERIFYING" || current.phase == "ACTIVE" {
        sqlx::query("UPDATE tenant_moves SET write_gate_milliseconds=$2 WHERE id=$1")
            .bind(id)
            .bind(elapsed)
            .execute(pool)
            .await?;
        return sqlx::query_as("SELECT tenant_id,owner_cell,ownership_generation,expires_at FROM tenant_leases WHERE tenant_id=$1 AND owner_cell=$2 AND ownership_generation=$3").bind(job.tenant_id).bind(job.target_cell).bind(job.source_ownership_generation+1).fetch_optional(pool).await?.ok_or_else(||AppError::Conflict("Completed move ownership was superseded".into()));
    }
    if let Err(error) = &result {
        rollback_cutover(pool, &job, &leases, error.to_string(), elapsed).await?;
    }
    result
}
async fn rollback_cutover(
    pool: &sqlx::PgPool,
    old: &Move,
    leases: &LeaseClient,
    error: String,
    elapsed: i64,
) -> Result<(), AppError> {
    // Renewal cannot steal a generation. It also reconciles a local fence when
    // PostgreSQL definitively still assigns this exact generation to the source.
    leases
        .resume_move_source(old.tenant_id, old.source_ownership_generation)
        .await?;
    let current = control::get(pool, old.id).await?;
    let mut tx = pool.begin().await?;
    control::lock_owned(&mut tx, &current, true).await?;
    if !matches!(current.phase.as_str(), "COPYING" | "CUTOVER") {
        return Err(AppError::Conflict(
            "Move cannot roll back its current phase".into(),
        ));
    }
    let selected: Option<Uuid> =
        sqlx::query_scalar("SELECT current_replication_generation FROM tenants WHERE id=$1")
            .bind(old.tenant_id)
            .fetch_one(&mut *tx)
            .await?;
    let phase = if selected == old.source_replication_generation {
        "COPYING"
    } else {
        "PREPARING_MOVE"
    };
    sqlx::query("UPDATE tenant_moves SET phase=$2,final_capture_number=NULL,target_prepared_at=NULL,last_error=$3,write_gate_milliseconds=$4,retry_after=clock_timestamp()+INTERVAL '30 seconds',updated_at=clock_timestamp() WHERE id=$1").bind(old.id).bind(phase).bind(error).bind(elapsed).execute(&mut *tx).await?;
    sqlx::query("UPDATE tenants SET state=$2,updated_at=clock_timestamp() WHERE id=$1")
        .bind(old.tenant_id)
        .bind(phase)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
