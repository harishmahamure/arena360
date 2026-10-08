//! Durable staged migration cohorts; failure closes admission across cells.
use super::{migration::MigrationAdmission, MigrationState, PendingTenantMigration};
use crate::error::AppError;
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use std::{collections::HashSet, sync::Arc, time::Duration};
use uuid::Uuid;
const ELIGIBLE:&str="GREATEST(r.canary_count,ceil(r.tenant_count::numeric*(CASE r.phase WHEN 0 THEN 0 WHEN 1 THEN 1 WHEN 2 THEN 10 WHEN 3 THEN 25 ELSE 100 END)/100))";
#[derive(Clone)]
pub struct State {
    pub pool: PgPool,
}
fn invalid(s: &str) -> AppError {
    AppError::Conflict(s.into())
}
pub async fn create(
    pool: &PgPool,
    version: i64,
    canaries: &[Uuid],
    soak_seconds: i32,
) -> Result<Uuid, AppError> {
    if version != super::target_schema_version()
        || canaries.is_empty()
        || canaries.iter().collect::<HashSet<_>>().len() != canaries.len()
        || !(0..=86400).contains(&soak_seconds)
    {
        return Err(invalid(
            "Use this binary's schema version, unique canary tenants, and 0..86400 soak seconds",
        ));
    }
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('schema-rollout-create',0))")
        .execute(&mut *tx)
        .await?;
    let tenants:Vec<Uuid>=sqlx::query_scalar("SELECT id FROM tenants WHERE owner_cell IS NOT NULL AND schema_version<$1 AND state NOT IN ('PROVISIONING','FAILED','DELETED','COLD') ORDER BY (id=ANY($2)) DESC,md5(id::text),id FOR UPDATE").bind(version).bind(canaries).fetch_all(&mut *tx).await?;
    if canaries.iter().any(|id| !tenants.contains(id)) {
        return Err(invalid(
            "Every canary must be an assigned tenant needing this migration",
        ));
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO schema_rollouts(id,target_version,canary_count,tenant_count,soak_seconds) VALUES($1,$2,$3,$4,$5)").bind(id).bind(version).bind(canaries.len() as i64).bind(tenants.len() as i64).bind(soak_seconds).execute(&mut *tx).await?;
    for (n, tenant) in tenants.iter().enumerate() {
        sqlx::query(
            "INSERT INTO schema_rollout_tenants(rollout_id,tenant_id,ordinal) VALUES($1,$2,$3)",
        )
        .bind(id)
        .bind(tenant)
        .bind((n + 1) as i64)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(id)
}
pub async fn status(pool: &PgPool, id: Uuid) -> Result<serde_json::Value, AppError> {
    sqlx::query_scalar("SELECT to_jsonb(r)||jsonb_build_object('cohort',CASE phase WHEN 0 THEN 'canary' WHEN 1 THEN '1%' WHEN 2 THEN '10%' WHEN 3 THEN '25%' ELSE '100%' END,'succeeded',(SELECT count(*) FROM schema_rollout_tenants WHERE rollout_id=r.id AND state='SUCCEEDED'),'failed',(SELECT count(*) FROM schema_rollout_tenants WHERE rollout_id=r.id AND state='FAILED')) FROM schema_rollouts r WHERE id=$1").bind(id).fetch_optional(pool).await?.ok_or_else(||AppError::NotFound("Schema rollout not found".into()))
}
pub async fn resume(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    let changed=sqlx::query("UPDATE schema_rollouts SET state='RUNNING',last_error=NULL,stage_ready_at=NULL,updated_at=clock_timestamp() WHERE id=$1 AND state='HALTED'").bind(id).execute(&mut *tx).await?;
    if changed.rows_affected() != 1 {
        return Err(invalid("Only a halted rollout can resume"));
    }
    sqlx::query("UPDATE schema_rollout_tenants SET state='QUEUED',last_error=NULL WHERE rollout_id=$1 AND state='FAILED'").bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
pub async fn advance(pool: &PgPool, version: i64) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    let row=sqlx::query("SELECT id,phase FROM schema_rollouts WHERE target_version=$1 AND state='RUNNING' FOR UPDATE").bind(version).fetch_optional(&mut *tx).await?;
    let Some(row) = row else {
        tx.rollback().await?;
        return Ok(());
    };
    let id: Uuid = row.get(0);
    let phase: i32 = row.get(1);
    let ready:bool=sqlx::query_scalar(&format!("SELECT NOT EXISTS(SELECT 1 FROM schema_rollout_tenants m JOIN schema_rollouts r ON r.id=m.rollout_id WHERE r.id=$1 AND m.ordinal<={ELIGIBLE} AND m.state<>'SUCCEEDED')")).bind(id).fetch_one(&mut *tx).await?;
    if ready {
        sqlx::query("UPDATE schema_rollouts SET stage_ready_at=COALESCE(stage_ready_at,clock_timestamp()) WHERE id=$1").bind(id).execute(&mut *tx).await?;
        let elapsed:bool=sqlx::query_scalar("SELECT stage_ready_at+soak_seconds*INTERVAL '1 second'<=clock_timestamp() FROM schema_rollouts WHERE id=$1").bind(id).fetch_one(&mut *tx).await?;
        if elapsed {
            sqlx::query("UPDATE schema_rollouts SET phase=$2,state=$3,stage_ready_at=NULL,updated_at=clock_timestamp() WHERE id=$1").bind(id).bind((phase+1).min(4)).bind(if phase==4{"COMPLETE"}else{"RUNNING"}).execute(&mut *tx).await?;
        }
    }
    tx.commit().await?;
    Ok(())
}
#[async_trait]
impl MigrationState for State {
    async fn pending(
        &self,
        cell: Uuid,
        target: i64,
    ) -> Result<Vec<PendingTenantMigration>, AppError> {
        Ok(sqlx::query_as(&format!("SELECT t.id AS tenant_id,t.ownership_generation,t.schema_version FROM tenants t JOIN tenant_leases l ON l.tenant_id=t.id JOIN schema_rollout_tenants m ON m.tenant_id=t.id JOIN schema_rollouts r ON r.id=m.rollout_id WHERE r.target_version=$2 AND r.state='RUNNING' AND m.state IN ('QUEUED','RUNNING') AND m.ordinal<={ELIGIBLE} AND t.state='ACTIVE' AND t.owner_cell=$1 AND l.owner_cell=$1 AND l.ownership_generation=t.ownership_generation AND l.expires_at>clock_timestamp()+INTERVAL '30 seconds' ORDER BY m.ordinal")).bind(cell).bind(target).fetch_all(&self.pool).await?)
    }
    async fn admit(
        &self,
        cell: Uuid,
        migration: PendingTenantMigration,
        target: i64,
    ) -> Result<MigrationAdmission, AppError> {
        let mut guard = self.pool.begin().await?;
        let locked: bool =
            sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1,0))")
                .bind(format!("schema-migration:{}:{target}", migration.tenant_id))
                .fetch_one(&mut *guard)
                .await?;
        if !locked {
            return Err(invalid("Tenant migration is already running"));
        }
        let mut tx = self.pool.begin().await?;
        // Global admission and failure share the rollout row lock.
        let rollout: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM schema_rollouts WHERE target_version=$1 AND state='RUNNING' FOR UPDATE",
        )
        .bind(target)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(id) = rollout else {
            return Err(invalid("Schema rollout is halted or unavailable"));
        };
        let changed=sqlx::query(&format!("UPDATE schema_rollout_tenants m SET state='RUNNING',attempts=attempts+1 FROM schema_rollouts r,tenants t,tenant_leases l WHERE m.rollout_id=r.id AND r.id=$1 AND m.tenant_id=$2 AND t.id=m.tenant_id AND t.owner_cell=$3 AND t.ownership_generation=$4 AND t.state='ACTIVE' AND l.tenant_id=t.id AND l.owner_cell=$3 AND l.ownership_generation=$4 AND l.expires_at>clock_timestamp()+INTERVAL '30 seconds' AND m.state IN ('QUEUED','RUNNING') AND m.ordinal<={ELIGIBLE}"))
            .bind(id).bind(migration.tenant_id).bind(cell).bind(migration.ownership_generation).execute(&mut *tx).await?;
        if changed.rows_affected() != 1 {
            return Err(invalid(
                "Migration cohort or ownership changed before admission",
            ));
        }
        tx.commit().await?;
        Ok(MigrationAdmission {
            _guard: Some(Box::new(guard)),
        })
    }
    async fn record_version(
        &self,
        cell: Uuid,
        migration: PendingTenantMigration,
        version: i64,
    ) -> Result<(), AppError> {
        let mut tx = self.pool.begin().await?;
        let changed=sqlx::query("UPDATE tenants SET schema_version=$1,updated_at=clock_timestamp() WHERE id=$2 AND owner_cell=$3 AND ownership_generation=$4 AND state NOT IN ('DELETED','FAILED') AND schema_version<=$1").bind(version).bind(migration.tenant_id).bind(cell).bind(migration.ownership_generation).execute(&mut *tx).await?;
        if changed.rows_affected() != 1 {
            return Err(invalid("Migration ownership changed before completion"));
        }
        let changed=sqlx::query("UPDATE schema_rollout_tenants m SET state='SUCCEEDED',last_error=NULL,completed_at=clock_timestamp() FROM schema_rollouts r WHERE m.rollout_id=r.id AND r.target_version=$1 AND m.tenant_id=$2 AND m.state='RUNNING'").bind(version).bind(migration.tenant_id).execute(&mut *tx).await?;
        if changed.rows_affected() != 1 {
            return Err(invalid("Migration admission was superseded"));
        }
        tx.commit().await?;
        Ok(())
    }
    async fn record_failure(
        &self,
        _cell: Uuid,
        migration: PendingTenantMigration,
        target: i64,
        error: &str,
    ) -> Result<(), AppError> {
        let mut tx = self.pool.begin().await?;
        let id:Option<Uuid>=sqlx::query_scalar("UPDATE schema_rollouts SET state='HALTED',last_error=$2,stage_ready_at=NULL,updated_at=clock_timestamp() WHERE target_version=$1 AND state<>'COMPLETE' RETURNING id").bind(target).bind(error).fetch_optional(&mut *tx).await?;
        if let Some(id) = id {
            sqlx::query("UPDATE schema_rollout_tenants SET state='FAILED',last_error=$3 WHERE rollout_id=$1 AND tenant_id=$2 AND state='RUNNING'").bind(id).bind(migration.tenant_id).bind(error).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
pub fn spawn(pool: PgPool, orchestrator: Arc<super::MigrationOrchestrator>) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(10));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            if let Err(e) = advance(&pool, super::target_schema_version()).await {
                tracing::warn!(%e,"Schema rollout advancement unavailable");
                continue;
            }
            match orchestrator.run_pending().await {
                Ok(outcomes) => {
                    for outcome in outcomes {
                        if let Some(error) = outcome.error {
                            tracing::warn!(tenant=%outcome.tenant_id,%error,"Staged migration incomplete");
                        }
                    }
                }
                Err(e) => tracing::warn!(%e,"Schema rollout queue unavailable"),
            }
        }
    });
}
