//! Phase 1 retention when there is no external event sink. Operational ledgers
//! remain intact; only old events already projected for realtime are removed.
use crate::{
    error::AppError,
    tenancy::{TenantDb, TenantDbManager},
};
use chrono::{DateTime, Duration, Utc};
use std::sync::Arc;
pub async fn prune_batch(db: Arc<TenantDb>, before: DateTime<Utc>) -> Result<u64, AppError> {
    let before = crate::time::format_sqlite_timestamp(&before)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    db.with_immediate_writer(move |c| Box::pin(async move {
        Ok(sqlx::query("DELETE FROM outbox_events WHERE sequence IN (SELECT sequence FROM outbox_events WHERE occurred_at<? AND sequence<=(SELECT sequence FROM realtime_projection_cursor WHERE singleton=1) ORDER BY sequence LIMIT 1000)").bind(before).execute(c).await?.rows_affected())
    })).await
}
pub fn spawn(manager: Arc<TenantDbManager>) {
    let days = std::env::var("OUTBOX_RETENTION_DAYS")
        .unwrap_or_else(|_| "7".into())
        .parse::<i64>()
        .expect("OUTBOX_RETENTION_DAYS must be an integer");
    assert!(
        (1..=365).contains(&days),
        "OUTBOX_RETENTION_DAYS must be between 1 and 365"
    );
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            for db in manager.open_handles().await {
                let before = Utc::now() - Duration::days(days);
                for _ in 0..16 {
                    match prune_batch(db.clone(), before).await {
                        Ok(n) if n < 1000 => break,
                        Ok(_) => tokio::task::yield_now().await,
                        Err(error) => {
                            tracing::warn!(tenant=%db.tenant_id(),%error,"Local outbox retention delayed");
                            break;
                        }
                    }
                }
            }
        }
    });
}
