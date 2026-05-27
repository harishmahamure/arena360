use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

pub mod status {
    pub const ACTIVE: &str = "active";
    pub const EXPIRED: &str = "expired";
    pub const EXHAUSTED: &str = "exhausted";
    pub const CANCELLED: &str = "cancelled";
    pub const MOVED_TO_NEXT_PLAN: &str = "moved_to_next_plan";
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlayerPlan {
    pub id: Uuid,
    pub player_id: Uuid,
    pub plan_id: Uuid,
    pub purchase_date: DateTime<Utc>,
    pub activation_date: Option<DateTime<Utc>>,
    pub expiry_date: DateTime<Utc>,
    pub remaining_usage_count: Option<i32>,
    pub remaining_time_credits: Option<i32>,
    pub status: String,
    pub moved_to_plan_id: Option<Uuid>,
    pub moved_credits_count: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct PlayerPlanFilterDto {
    pub player_id: Option<Uuid>,
    pub plan_id: Option<Uuid>,
    pub status: Option<String>,
    pub purchase_date_from: Option<String>,
    pub purchase_date_to: Option<String>,
    pub expiry_date_from: Option<String>,
    pub expiry_date_to: Option<String>,
    pub min_remaining_usage_count: Option<i32>,
    pub min_remaining_time_credits: Option<i32>,
    pub is_expired: Option<bool>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub device_type: Option<String>,
    pub device_sub_type: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssignPlanDto {
    pub player_id: Uuid,
    pub plan_id: Uuid,
    pub transaction_id: Option<Uuid>,
    pub purchase_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

pub struct PlayerPlanCreateValues {
    pub player_id: Uuid,
    pub plan_id: Uuid,
    pub purchase_date: DateTime<Utc>,
    pub expiry_date: DateTime<Utc>,
    pub remaining_usage_count: Option<i32>,
    pub remaining_time_credits: Option<i32>,
    pub status: String,
}

pub struct PlayerPlanUpdateValues {
    pub status: Option<String>,
    pub remaining_time_credits: Option<i32>,
    pub remaining_usage_count: Option<i32>,
    pub activation_date: Option<DateTime<Utc>>,
}
