//! Tenant realtime projection row; canonical events originate in SQLite.
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct OutboxRow {
    pub id: i64,
    pub channel: String,
    pub event_type: String,
    pub payload: Value,
    pub audience_role: Option<String>,
    pub audience_user_id: Option<Uuid>,
    pub audience_room_id: Option<Uuid>,
    pub durable: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub source_tenant_id: Option<Uuid>,
}
