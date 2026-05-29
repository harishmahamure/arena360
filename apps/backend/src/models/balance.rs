use chrono::{DateTime, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

pub mod balance_status {
    pub const ACTIVE: &str = "active";
    pub const EXPIRED: &str = "expired";
    pub const EXHAUSTED: &str = "exhausted";
    pub const CANCELLED: &str = "cancelled";
}

pub mod plan_kind {
    pub const TIME: &str = "time";
    pub const HAPPY_HOURS: &str = "happy_hours";
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlayerPlanBalance {
    pub id: Uuid,
    pub player_id: Uuid,
    pub device_type: Option<String>,
    pub device_sub_type: Option<String>,
    pub kind: String,
    pub remaining_minutes: i32,
    pub expiry_date: DateTime<Utc>,
    pub window_start: Option<NaiveTime>,
    pub window_end: Option<NaiveTime>,
    pub status: String,
    pub source_plan_id: Option<Uuid>,
    pub allowed_days: Option<Value>,
    pub allowed_months: Option<Value>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BalancePlayerSummary {
    pub id: Uuid,
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BalancePlanSummary {
    pub id: Uuid,
    pub name: String,
    pub plan_type: String,
    pub price: f64,
    pub time_credits: i32,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlayerPlanBalanceResponse {
    pub id: Uuid,
    pub player_id: Uuid,
    pub device_type: Option<String>,
    pub device_sub_type: Option<String>,
    pub kind: String,
    pub remaining_minutes: i32,
    pub expiry_date: DateTime<Utc>,
    pub window_start: Option<NaiveTime>,
    pub window_end: Option<NaiveTime>,
    pub status: String,
    pub source_plan_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_days: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_months: Option<Value>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub player: Option<BalancePlayerSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<BalancePlanSummary>,
}

impl From<PlayerPlanBalance> for PlayerPlanBalanceResponse {
    fn from(b: PlayerPlanBalance) -> Self {
        Self {
            id: b.id,
            player_id: b.player_id,
            device_type: b.device_type,
            device_sub_type: b.device_sub_type,
            kind: b.kind,
            remaining_minutes: b.remaining_minutes,
            expiry_date: b.expiry_date,
            window_start: b.window_start,
            window_end: b.window_end,
            status: b.status,
            source_plan_id: b.source_plan_id,
            allowed_days: b.allowed_days,
            allowed_months: b.allowed_months,
            created_by: b.created_by,
            updated_by: b.updated_by,
            created_at: b.created_at,
            updated_at: b.updated_at,
            deleted_at: b.deleted_at,
            player: None,
            plan: None,
        }
    }
}

#[derive(Debug, FromRow)]
pub struct BalanceRow {
    pub id: Uuid,
    pub player_id: Uuid,
    pub device_type: Option<String>,
    pub device_sub_type: Option<String>,
    pub kind: String,
    pub remaining_minutes: i32,
    pub expiry_date: DateTime<Utc>,
    pub window_start: Option<NaiveTime>,
    pub window_end: Option<NaiveTime>,
    pub status: String,
    pub source_plan_id: Option<Uuid>,
    pub allowed_days: Option<Value>,
    pub allowed_months: Option<Value>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub player_username: Option<String>,
    pub player_first_name: Option<String>,
    pub player_last_name: Option<String>,
    pub plan_name: Option<String>,
    pub plan_type: Option<String>,
    pub plan_price: Option<f64>,
    pub plan_time_credits: Option<i32>,
}

impl BalanceRow {
    pub fn into_response(self) -> PlayerPlanBalanceResponse {
        let player = self
            .player_username
            .map(|username| BalancePlayerSummary {
                id: self.player_id,
                username,
                first_name: self.player_first_name,
                last_name: self.player_last_name,
            });
        let plan = self.plan_name.map(|name| BalancePlanSummary {
            id: self.source_plan_id.unwrap_or_default(),
            name,
            plan_type: self.plan_type.unwrap_or_else(|| "time_based".to_string()),
            price: self.plan_price.unwrap_or(0.0),
            time_credits: self.plan_time_credits.unwrap_or(0),
        });

        PlayerPlanBalanceResponse {
            id: self.id,
            player_id: self.player_id,
            device_type: self.device_type,
            device_sub_type: self.device_sub_type,
            kind: self.kind,
            remaining_minutes: self.remaining_minutes,
            expiry_date: self.expiry_date,
            window_start: self.window_start,
            window_end: self.window_end,
            status: self.status,
            source_plan_id: self.source_plan_id,
            allowed_days: self.allowed_days,
            allowed_months: self.allowed_months,
            created_by: self.created_by,
            updated_by: self.updated_by,
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted_at: self.deleted_at,
            player,
            plan,
        }
    }
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct BalanceFilterDto {
    pub player_id: Option<Uuid>,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub device_type: Option<String>,
    pub device_sub_type: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    #[serde(default)]
    pub usable_only: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseBalanceDto {
    pub player_id: Uuid,
    pub plan_id: Uuid,
    pub transaction_id: Option<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BalanceValidationResult {
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
