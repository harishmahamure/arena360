//! Read-only pre-copy authorization derives from the source's real lease and
//! an explicit move assignment. It can never acquire or transfer ownership.
use super::control;
use crate::{
    error::AppError,
    replication::{ledger::PostgresLedger, recovery::Recoverer, restore, wal},
    tenancy::TenantLease,
};
use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;
struct CopyLease {
    tenant: Uuid,
    generation: i64,
    until: Instant,
}
impl TenantLease for CopyLease {
    fn writable_generation(&self, tenant: Uuid) -> Result<i64, AppError> {
        if tenant != self.tenant || Instant::now() >= self.until {
            return Err(AppError::Forbidden("Move copy source lease expired".into()));
        }
        Ok(self.generation)
    }
    fn ensure_writable(&self, tenant: Uuid, generation: i64) -> Result<(), AppError> {
        if self.writable_generation(tenant)? != generation {
            return Err(AppError::Forbidden(
                "Move copy source generation changed".into(),
            ));
        }
        Ok(())
    }
}
async fn permission(
    context: &Recoverer,
    job: &control::Move,
) -> Result<Arc<dyn TenantLease>, AppError> {
    if context.ledger.cell_id != job.target_cell
        || !matches!(job.phase.as_str(), "COPYING" | "CUTOVER")
    {
        return Err(AppError::Forbidden(
            "Cell is not assigned to this move pre-copy".into(),
        ));
    }
    let sent = Instant::now();
    let remaining:Option<i64>=sqlx::query_scalar("SELECT (extract(epoch FROM l.expires_at-clock_timestamp())*1000)::bigint-30000 FROM tenants t JOIN tenant_leases l ON l.tenant_id=t.id JOIN cells c ON c.id=$4 WHERE t.id=$1 AND t.owner_cell=$2 AND l.owner_cell=$2 AND t.ownership_generation=$3 AND l.ownership_generation=$3 AND t.current_replication_generation=$5 AND c.state='ACTIVE' AND t.state IN ('COPYING','CUTOVER') AND t.schema_version=$6 AND l.expires_at>clock_timestamp()+INTERVAL '30 seconds'").bind(job.tenant_id).bind(job.source_cell).bind(job.source_ownership_generation).bind(job.target_cell).bind(job.source_replication_generation).bind(crate::tenancy::target_schema_version()).fetch_optional(&context.ledger.pool).await?;
    let remaining = remaining.filter(|n| *n > 0).ok_or_else(|| {
        AppError::Forbidden("Move source lease or selected backup changed".into())
    })?;
    Ok(Arc::new(CopyLease {
        tenant: job.tenant_id,
        generation: job.source_ownership_generation,
        until: sent + Duration::from_millis(remaining as u64),
    }))
}
fn fail(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("Move pre-copy: {e}"))
}
fn receipt(path: &Path, value: &restore::Restored) -> Result<(), AppError> {
    let temporary = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
    wal::durable_create(&temporary, &serde_json::to_vec(value).map_err(fail)?)?;
    std::fs::rename(temporary, path).map_err(fail)?;
    std::fs::File::open(path.parent().unwrap())
        .and_then(|f| f.sync_all())
        .map_err(fail)?;
    Ok(())
}
pub async fn precopy(context: Arc<Recoverer>, id: Uuid) -> Result<(), AppError> {
    let mut serial = context.ledger.pool.begin().await?;
    let locked: bool =
        sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1,0))")
            .bind(format!("move-target:{id}"))
            .fetch_one(&mut *serial)
            .await?;
    if !locked {
        return Err(AppError::Conflict(
            "Target move preparation is already running".into(),
        ));
    }
    let job = control::get(&context.ledger.pool, id).await?;
    let lease = permission(&context, &job).await?;
    let _permit = match context.databases.background_jobs() {
        Some(j) => Some(
            j.acquire(if job.phase == "CUTOVER" {
                crate::background::Priority::CriticalRecovery
            } else {
                crate::background::Priority::HotBackfill
            })
            .await?,
        ),
        None => None,
    };
    let root = context.staging_root.join("moves").join(id.to_string());
    let manifest = root.join("receipt.json");
    let ledger = PostgresLedger {
        pool: context.ledger.pool.clone(),
        cell_id: job.source_cell,
    };
    let previous = std::fs::read(&manifest)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<restore::Restored>(&bytes).ok());
    let mut image = None;
    if let Some(previous) = previous {
        if previous.image.parent().and_then(|p| p.parent()) != Some(root.as_path()) {
            return Err(fail("Receipt image escaped this move staging directory"));
        }
        // Idle pre-copy only refreshes readiness. Final cutover always hashes
        // and checks integrity again before installing the image.
        if job.phase == "COPYING"
            && Some(previous.generation) == job.source_replication_generation
            && job.target_capture_number == Some(previous.capture_number as i64)
            && job.target_image_checksum.as_deref() == Some(previous.image_checksum.as_str())
            && previous.image.is_file()
        {
            let later: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM replication_segments WHERE generation_id=$1 AND verified_at IS NOT NULL AND retired_at IS NULL AND COALESCE((capture->>'last_capture_number')::bigint,(capture->>'capture_number')::bigint)>$2)")
                .bind(previous.generation).bind(previous.capture_number as i64).fetch_one(&context.ledger.pool).await?;
            if !later {
                control::target_prepared(
                    &context.ledger.pool,
                    &job,
                    previous.capture_number as i64,
                    &previous.image_checksum,
                )
                .await?;
                serial.rollback().await?;
                return Ok(());
            }
        }
        let parent = previous.image.parent().unwrap().to_path_buf();
        match restore::advance(
            previous,
            lease.clone(),
            &ledger,
            context.store.as_ref(),
            &context.keys,
            restore::Limits::default(),
        )
        .await
        {
            Ok(result) => image = Some(result),
            Err(_) => {
                let _ = std::fs::remove_dir_all(parent);
            }
        }
    }
    let image = match image {
        Some(image) => image,
        None => {
            restore::restore(
                job.tenant_id,
                lease.clone(),
                &ledger,
                context.store.as_ref(),
                &context.keys,
                &root,
                None,
                restore::Limits::default(),
            )
            .await?
        }
    };
    if Some(image.generation) != job.source_replication_generation {
        return Err(fail("Pre-copy generation changed"));
    }
    receipt(&manifest, &image)?;
    if job.phase == "CUTOVER" {
        if job.final_capture_number != Some(image.capture_number as i64) {
            return Err(AppError::Conflict(
                "Move awaits the final verified upload".into(),
            ));
        }
        permission(&context, &job).await?;
        // Serialize installation with cancellation, recovery, and cleanup.
        let mut installation = context.ledger.pool.begin().await?;
        control::lock_owned(&mut installation, &job, true).await?;
        context
            .databases
            .prepare_move(
                job.tenant_id,
                id,
                job.source_ownership_generation + 1,
                &image.image,
                image.capture_number,
                &image.image_checksum,
            )
            .await?;
        control::target_prepared_locked(
            &mut installation,
            &job,
            image.capture_number as i64,
            &image.image_checksum,
        )
        .await?;
        installation.commit().await?;
        serial.rollback().await?;
        return Ok(());
    }
    control::target_prepared(
        &context.ledger.pool,
        &job,
        image.capture_number as i64,
        &image.image_checksum,
    )
    .await?;
    serial.rollback().await?;
    Ok(())
}
pub async fn activate(context: Arc<Recoverer>, id: Uuid) -> Result<(), AppError> {
    let mut serial = context.ledger.pool.begin().await?;
    let locked: bool =
        sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1,0))")
            .bind(format!("move-target:{id}"))
            .fetch_one(&mut *serial)
            .await?;
    if !locked {
        return Err(AppError::Conflict(
            "Target move activation is already running".into(),
        ));
    }
    let job = control::get(&context.ledger.pool, id).await?;
    if !matches!(job.phase.as_str(), "VERIFYING" | "ACTIVE")
        || job.target_cell != context.ledger.cell_id
    {
        return Err(AppError::Conflict("This cell has no verifying move".into()));
    }
    let checksum = job
        .target_image_checksum
        .clone()
        .ok_or_else(|| fail("Missing target checksum"))?;
    let grant = context.leases.acquire_assigned(job.tenant_id).await?;
    if grant.ownership_generation != job.source_ownership_generation + 1 {
        return Err(AppError::Conflict(
            "Move target ownership was superseded".into(),
        ));
    }
    if job.phase == "ACTIVE" && !context.databases.move_pending(job.tenant_id) {
        if !context.databases.recovery_path(job.tenant_id).is_file() {
            return Err(fail(
                "Completed move operational image is missing; run normal cell recovery",
            ));
        }
        return Ok(());
    }
    let pool = context.ledger.pool.clone();
    let finished = job.clone();
    context
        .databases
        .activate_move(
            job.tenant_id,
            id,
            grant.ownership_generation,
            &checksum,
            job.phase == "VERIFYING",
            move || Box::pin(async move { control::complete(&pool, &finished).await }),
        )
        .await?;
    let root = context.staging_root.join("moves").join(id.to_string());
    // The operational image is hard-linked; removing staging keeps it alive.
    if root.exists() {
        std::fs::remove_dir_all(root).map_err(fail)?;
    }
    serial.rollback().await?;
    Ok(())
}
