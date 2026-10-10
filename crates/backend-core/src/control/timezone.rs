//! Revisioned control metadata is projected locally by the current owner. A control
//! outage preserves the last accepted calendar and does not block business requests.
use crate::{
    error::AppError,
    tenancy::{TenantDb, TenantDbManager},
};
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};

pub async fn project(db: Arc<TenantDb>, timezone: String, revision: i64) -> Result<bool, AppError> {
    db.ensure_current_owner()?;
    timezone
        .parse::<chrono_tz::Tz>()
        .map_err(|_| AppError::BadRequest("Invalid tenant timezone".into()))?;
    if revision < 0 {
        return Err(AppError::BadRequest("Invalid timezone revision".into()));
    }
    let current: (String, i64) =
        sqlx::query_as("SELECT timezone,timezone_revision FROM tenant_runtime WHERE singleton=1")
            .fetch_one(&db.background_read_pool()?)
            .await?;
    if current.1 > revision {
        return Ok(false);
    }
    if current.1 == revision {
        if current.0 != timezone {
            return Err(AppError::Conflict(
                "Conflicting timezone projection revision".into(),
            ));
        }
        return Ok(false);
    }
    db.with_immediate_writer(move|c|Box::pin(async move{
  let changed=sqlx::query("UPDATE tenant_runtime SET timezone=?,timezone_revision=? WHERE singleton=1 AND timezone_revision<?").bind(timezone).bind(revision).bind(revision).execute(c).await?.rows_affected();
  Ok(changed>0)
 })).await
}
pub async fn sync(pool: &PgPool, db: Arc<TenantDb>) -> Result<bool, AppError> {
    let (timezone,revision):(String,i64)=sqlx::query_as("SELECT timezone,timezone_revision FROM tenants WHERE id=$1 AND state NOT IN ('DELETED','FAILED')").bind(db.tenant_id()).fetch_one(pool).await?;
    project(db, timezone, revision).await
}
pub fn spawn(pool: PgPool, manager: Arc<TenantDbManager>) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            for db in manager.open_handles().await {
                let tenant = db.tenant_id();
                if let Err(error) = sync(&pool, db).await {
                    tracing::warn!(%tenant,%error,"Tenant timezone projection delayed");
                }
            }
        }
    });
}
