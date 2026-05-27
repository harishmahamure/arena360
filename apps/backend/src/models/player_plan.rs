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

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlayerPlanPlayerSummary {
    pub id: Uuid,
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlayerPlanPlanSummary {
    pub id: Uuid,
    pub name: String,
    pub plan_type: String,
    pub price: f64,
    pub time_credits: i32,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlayerPlanResponse {
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub player: Option<PlayerPlanPlayerSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<PlayerPlanPlanSummary>,
}

impl From<PlayerPlan> for PlayerPlanResponse {
    fn from(value: PlayerPlan) -> Self {
        Self {
            id: value.id,
            player_id: value.player_id,
            plan_id: value.plan_id,
            purchase_date: value.purchase_date,
            activation_date: value.activation_date,
            expiry_date: value.expiry_date,
            remaining_usage_count: value.remaining_usage_count,
            remaining_time_credits: value.remaining_time_credits,
            status: value.status,
            moved_to_plan_id: value.moved_to_plan_id,
            moved_credits_count: value.moved_credits_count,
            created_at: value.created_at,
            updated_at: value.updated_at,
            deleted_at: value.deleted_at,
            player: None,
            plan: None,
        }
    }
}

#[derive(Debug, FromRow)]
pub struct PlayerPlanRow {
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
    pub player_username: Option<String>,
    pub player_first_name: Option<String>,
    pub player_last_name: Option<String>,
    pub plan_name: Option<String>,
    pub plan_type: Option<String>,
    pub plan_price: Option<f64>,
    pub plan_time_credits: Option<i32>,
}

impl PlayerPlanRow {
    pub fn into_response(self) -> PlayerPlanResponse {
        let player = self.player_username.map(|username| PlayerPlanPlayerSummary {
            id: self.player_id,
            username,
            first_name: self.player_first_name,
            last_name: self.player_last_name,
        });
        let plan = self.plan_name.map(|name| PlayerPlanPlanSummary {
            id: self.plan_id,
            name,
            plan_type: self.plan_type.unwrap_or_else(|| "time_based".to_string()),
            price: self.plan_price.unwrap_or(0.0),
            time_credits: self.plan_time_credits.unwrap_or(0),
        });

        PlayerPlanResponse {
            id: self.id,
            player_id: self.player_id,
            plan_id: self.plan_id,
            purchase_date: self.purchase_date,
            activation_date: self.activation_date,
            expiry_date: self.expiry_date,
            remaining_usage_count: self.remaining_usage_count,
            remaining_time_credits: self.remaining_time_credits,
            status: self.status,
            moved_to_plan_id: self.moved_to_plan_id,
            moved_credits_count: self.moved_credits_count,
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
