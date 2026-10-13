//! Canonical tenant events read directly from the local SQLite outbox.
use crate::{error::AppError, tenancy::TenantDb};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TenantEvent {
    pub sequence: i64,
    pub event_id: String,
    #[sqlx(skip)]
    pub tenant_id: Uuid,
    pub location_id: Option<String>,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub event_type: String,
    pub occurred_at: String,
    pub schema_version: i64,
    pub deleted: bool,
    #[sqlx(json)]
    pub payload: Value,
    #[serde(default)]
    #[sqlx(json(nullable))]
    pub analytics_snapshot: Option<crate::tenancy::analytics_snapshot::AnalyticsSnapshot>,
}

/// Read a bounded batch without holding the foreground writer. The database
/// supplies the tenant identity; events cannot originate from another tenant.
pub async fn read_batch(
    db: &TenantDb,
    after: i64,
    through: i64,
    limit: i64,
) -> Result<Vec<TenantEvent>, AppError> {
    db.ensure_current_owner()?;
    let mut events: Vec<TenantEvent> = sqlx::query_as(
        "SELECT sequence,event_id,location_id,aggregate_type,aggregate_id,event_type,occurred_at,schema_version,deleted,payload,analytics_snapshot FROM outbox_events WHERE sequence>? AND sequence<=? ORDER BY sequence LIMIT ?"
    ).bind(after).bind(through).bind(limit).fetch_all(&db.background_read_pool()?).await?;
    db.ensure_current_owner()?;
    for event in &mut events {
        event.tenant_id = db.tenant_id();
    }
    Ok(events)
}
