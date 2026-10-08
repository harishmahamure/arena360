use crate::error::AppError;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;
#[derive(Debug, Clone, FromRow, serde::Serialize)]
pub struct Move {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub source_cell: Uuid,
    pub target_cell: Uuid,
    pub source_ownership_generation: i64,
    pub source_replication_generation: Option<Uuid>,
    pub phase: String,
    pub target_capture_number: Option<i64>,
    pub final_capture_number: Option<i64>,
    pub target_image_checksum: Option<String>,
    pub target_prepared_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_error: Option<String>,
    pub write_gate_milliseconds: Option<i64>,
}
pub async fn get(pool: &PgPool, id: Uuid) -> Result<Move, AppError> {
    sqlx::query_as("SELECT * FROM tenant_moves WHERE id=$1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Tenant move not found".into()))
}
/// Explicit operator request. Target cells cannot claim tenants through a move
/// they created themselves; ownership is transferred only by the old owner.
pub async fn enqueue(pool: &PgPool, tenant: Uuid, target: Uuid) -> Result<Move, AppError> {
    let mut tx = pool.begin().await?;
    let id = enqueue_locked(&mut tx, tenant, target).await?;
    tx.commit().await?;
    get(pool, id).await
}
pub(crate) async fn enqueue_locked(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant: Uuid,
    target: Uuid,
) -> Result<Uuid, AppError> {
    let target_state: Option<String> =
        sqlx::query_scalar("SELECT state FROM cells WHERE id=$1 FOR SHARE")
            .bind(target)
            .fetch_optional(&mut **tx)
            .await?;
    if target_state.as_deref() != Some("ACTIVE") {
        return Err(AppError::Forbidden(
            "Move target must be an active cell".into(),
        ));
    }
    let source: Option<(Option<Uuid>, i64, String)> = sqlx::query_as(
        "SELECT owner_cell,ownership_generation,state FROM tenants WHERE id=$1 FOR UPDATE",
    )
    .bind(tenant)
    .fetch_optional(&mut **tx)
    .await?;
    let Some((Some(source), generation, state)) = source else {
        return Err(AppError::Conflict("Tenant has no assigned source".into()));
    };
    if state != "ACTIVE" || source == target || generation == i64::MAX {
        return Err(AppError::Conflict(
            "Move requires an active tenant and a different cell".into(),
        ));
    }
    let fresh:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenant_leases WHERE tenant_id=$1 AND owner_cell=$2 AND ownership_generation=$3 AND expires_at>clock_timestamp()+INTERVAL '30 seconds')").bind(tenant).bind(source).bind(generation).fetch_one(&mut **tx).await?;
    if !fresh {
        return Err(AppError::Forbidden(
            "Move requires a fresh source lease".into(),
        ));
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO tenant_moves(id,tenant_id,source_cell,target_cell,source_ownership_generation) VALUES($1,$2,$3,$4,$5)").bind(id).bind(tenant).bind(source).bind(target).bind(generation).execute(&mut **tx).await?;
    sqlx::query(
        "UPDATE tenants SET state='PREPARING_MOVE',updated_at=clock_timestamp() WHERE id=$1",
    )
    .bind(tenant)
    .execute(&mut **tx)
    .await?;
    Ok(id)
}
/// Always lock tenant before move, matching lease handoff lock ordering.
pub(crate) async fn lock_owned<'a>(
    tx: &mut sqlx::Transaction<'a, sqlx::Postgres>,
    job: &Move,
    source: bool,
) -> Result<(), AppError> {
    let expected_cell = if source {
        job.source_cell
    } else {
        job.target_cell
    };
    let expected_generation = job
        .source_ownership_generation
        .checked_add(if source { 0 } else { 1 })
        .ok_or_else(|| AppError::Internal("Move ownership overflow".into()))?;
    let owner: Option<(Option<Uuid>, i64, String)> = sqlx::query_as(
        "SELECT owner_cell,ownership_generation,state FROM tenants WHERE id=$1 FOR UPDATE",
    )
    .bind(job.tenant_id)
    .fetch_optional(&mut **tx)
    .await?;
    let expected_state = if source { job.phase.as_str() } else { "ACTIVE" };
    if owner
        != Some((
            Some(expected_cell),
            expected_generation,
            expected_state.to_owned(),
        ))
    {
        return Err(AppError::Forbidden("Tenant move ownership changed".into()));
    }
    let phase: Option<String> =
        sqlx::query_scalar("SELECT phase FROM tenant_moves WHERE id=$1 FOR UPDATE")
            .bind(job.id)
            .fetch_optional(&mut **tx)
            .await?;
    if phase.as_deref() != Some(job.phase.as_str()) {
        return Err(AppError::Conflict("Tenant move phase changed".into()));
    }
    let fresh:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenant_leases WHERE tenant_id=$1 AND owner_cell=$2 AND ownership_generation=$3 AND expires_at>clock_timestamp()+INTERVAL '30 seconds')").bind(job.tenant_id).bind(expected_cell).bind(expected_generation).fetch_one(&mut **tx).await?;
    if !fresh {
        return Err(AppError::Forbidden("Tenant move lease expired".into()));
    }
    Ok(())
}
pub async fn cancel(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let job = get(pool, id).await?;
    if !matches!(job.phase.as_str(), "PREPARING_MOVE" | "COPYING") {
        return Err(AppError::Conflict(
            "Only a move before cutover can be cancelled".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    lock_owned(&mut tx, &job, true).await?;
    sqlx::query(
        "UPDATE tenant_moves SET phase='CANCELLED',updated_at=clock_timestamp() WHERE id=$1",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE tenants SET state='ACTIVE',updated_at=clock_timestamp() WHERE id=$1")
        .bind(job.tenant_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
pub(crate) async fn copying(pool: &PgPool, job: &Move, generation: Uuid) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    lock_owned(&mut tx, job, true).await?;
    let selected: bool =
        sqlx::query_scalar("SELECT current_replication_generation=$2 FROM tenants WHERE id=$1")
            .bind(job.tenant_id)
            .bind(generation)
            .fetch_one(&mut *tx)
            .await?;
    if !selected {
        return Err(AppError::Conflict("Move backup generation changed".into()));
    }
    sqlx::query("UPDATE tenant_moves SET phase='COPYING',source_replication_generation=$2,target_capture_number=NULL,target_prepared_at=NULL,target_image_checksum=NULL,last_error=NULL,updated_at=clock_timestamp() WHERE id=$1").bind(job.id).bind(generation).execute(&mut *tx).await?;
    sqlx::query("UPDATE tenants SET state='COPYING',updated_at=clock_timestamp() WHERE id=$1")
        .bind(job.tenant_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

pub(crate) async fn target_prepared(
    pool: &PgPool,
    job: &Move,
    capture: i64,
    checksum: &str,
) -> Result<(), AppError> {
    if !matches!(job.phase.as_str(), "COPYING" | "CUTOVER") {
        return Err(AppError::Conflict(
            "Move does not accept a pre-copy receipt".into(),
        ));
    }
    if job.phase == "CUTOVER" && job.final_capture_number != Some(capture) {
        return Err(AppError::Conflict(
            "Move final capture is not complete".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    target_prepared_locked(&mut tx, job, capture, checksum).await?;
    tx.commit().await?;
    Ok(())
}
pub(crate) async fn target_prepared_locked(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    job: &Move,
    capture: i64,
    checksum: &str,
) -> Result<(), AppError> {
    lock_owned(tx, job, true).await?;
    let selected: bool =
        sqlx::query_scalar("SELECT current_replication_generation=$2 FROM tenants WHERE id=$1")
            .bind(job.tenant_id)
            .bind(job.source_replication_generation)
            .fetch_one(&mut **tx)
            .await?;
    if !selected {
        return Err(AppError::Conflict("Move source backup changed".into()));
    }
    sqlx::query("UPDATE tenant_moves SET target_capture_number=$2,target_image_checksum=$3,target_prepared_at=clock_timestamp(),last_error=NULL,updated_at=clock_timestamp() WHERE id=$1")
        .bind(job.id).bind(capture).bind(checksum).execute(&mut **tx).await?;
    Ok(())
}

pub(crate) async fn complete(pool: &PgPool, job: &Move) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    lock_owned(&mut tx, job, false).await?;
    if job.phase == "ACTIVE" {
        tx.rollback().await?;
        return Ok(());
    }
    if job.phase != "VERIFYING" {
        return Err(AppError::Conflict("Move is not verifying".into()));
    }
    sqlx::query("UPDATE tenant_moves SET phase='ACTIVE',completed_at=clock_timestamp(),last_error=NULL,updated_at=clock_timestamp() WHERE id=$1").bind(job.id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

pub(crate) async fn target_recent(pool: &PgPool, id: Uuid) -> Result<bool, AppError> {
    Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenant_moves WHERE id=$1 AND target_prepared_at>clock_timestamp()-INTERVAL '10 seconds')").bind(id).fetch_one(pool).await?)
}
