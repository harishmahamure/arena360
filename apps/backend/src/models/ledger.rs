use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

pub mod ledger_reason {
    pub const PURCHASE: &str = "purchase";
    pub const RECHARGE: &str = "recharge";
    pub const SESSION_USAGE: &str = "session_usage";
    pub const EXPIRY: &str = "expiry";
    pub const ADJUSTMENT: &str = "adjustment";
    pub const MIGRATION: &str = "migration";
    pub const STAFF_ALLOWANCE_GRANT: &str = "staff_allowance_grant";
    pub const STAFF_ALLOWANCE_RENEWAL: &str = "staff_allowance_renewal";
}

#[derive(Debug, Clone, FromRow, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlayerPlanLedger {
    pub id: Uuid,
    pub balance_id: Uuid,
    pub player_id: Uuid,
    pub delta_minutes: i32,
    pub reason: String,
    pub transaction_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub balance_after: i32,
    pub expiry_after: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
}
