use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UsageSession {
    pub id: Uuid,
    pub player_plan_id: Uuid,
    pub device_id: Uuid,
    pub game_id: Option<Uuid>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_minutes: Option<i32>,
    pub time_credits_consumed: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionDto {
    pub player_plan_id: Uuid,
    pub device_id: Uuid,
    pub game_id: Option<Uuid>,
    pub start_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EndSessionDto {
    pub end_time: Option<DateTime<Utc>>,
    pub time_credits_consumed: Option<i32>,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct SessionFilterDto {
    pub player_plan_id: Option<Uuid>,
    pub device_id: Option<Uuid>,
    pub game_id: Option<Uuid>,
    pub player_id: Option<Uuid>,
    pub is_active: Option<i32>,
    pub start_time_from: Option<DateTime<Utc>>,
    pub start_time_to: Option<DateTime<Utc>>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}
