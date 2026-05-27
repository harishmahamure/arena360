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
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
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

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionDeviceSummary {
    pub id: Uuid,
    pub name: String,
    pub device_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionPlayerSummary {
    pub id: Uuid,
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionPlanSummary {
    pub id: Uuid,
    pub name: String,
    pub plan_type: String,
    pub time_credits: i32,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionPlayerPlanSummary {
    pub id: Uuid,
    pub player_id: Uuid,
    pub plan_id: Uuid,
    pub remaining_time_credits: Option<i32>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub player: Option<SessionPlayerSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<SessionPlanSummary>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UsageSessionResponse {
    pub id: Uuid,
    pub player_plan_id: Uuid,
    pub device_id: Uuid,
    pub game_id: Option<Uuid>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_minutes: Option<i32>,
    pub time_credits_consumed: Option<i32>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub player_plan: Option<SessionPlayerPlanSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<SessionDeviceSummary>,
}

impl From<UsageSession> for UsageSessionResponse {
    fn from(value: UsageSession) -> Self {
        Self {
            id: value.id,
            player_plan_id: value.player_plan_id,
            device_id: value.device_id,
            game_id: value.game_id,
            start_time: value.start_time,
            end_time: value.end_time,
            duration_minutes: value.duration_minutes,
            time_credits_consumed: value.time_credits_consumed,
            created_by: value.created_by,
            updated_by: value.updated_by,
            created_at: value.created_at,
            updated_at: value.updated_at,
            deleted_at: value.deleted_at,
            player_plan: None,
            device: None,
        }
    }
}

#[derive(Debug, FromRow)]
pub struct UsageSessionRow {
    pub id: Uuid,
    pub player_plan_id: Uuid,
    pub device_id: Uuid,
    pub game_id: Option<Uuid>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_minutes: Option<i32>,
    pub time_credits_consumed: Option<i32>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub pp_player_id: Option<Uuid>,
    pub pp_plan_id: Option<Uuid>,
    pub pp_remaining_time_credits: Option<i32>,
    pub pp_status: Option<String>,
    pub player_username: Option<String>,
    pub player_first_name: Option<String>,
    pub player_last_name: Option<String>,
    pub plan_name: Option<String>,
    pub plan_type: Option<String>,
    pub plan_time_credits: Option<i32>,
    pub device_name: Option<String>,
    pub device_type: Option<String>,
    pub device_location: Option<String>,
    pub device_status: Option<String>,
}

impl UsageSessionRow {
    pub fn into_response(self) -> UsageSessionResponse {
        let player = self
            .player_username
            .as_ref()
            .map(|username| SessionPlayerSummary {
                id: self.pp_player_id.unwrap_or_default(),
                username: username.clone(),
                first_name: self.player_first_name.clone(),
                last_name: self.player_last_name.clone(),
            });
        let plan = self.plan_name.as_ref().map(|name| SessionPlanSummary {
            id: self.pp_plan_id.unwrap_or_default(),
            name: name.clone(),
            plan_type: self
                .plan_type
                .clone()
                .unwrap_or_else(|| "time_based".to_string()),
            time_credits: self.plan_time_credits.unwrap_or(0),
        });
        let player_plan = Some(SessionPlayerPlanSummary {
            id: self.player_plan_id,
            player_id: self.pp_player_id.unwrap_or_default(),
            plan_id: self.pp_plan_id.unwrap_or_default(),
            remaining_time_credits: self.pp_remaining_time_credits,
            status: self.pp_status.unwrap_or_else(|| "active".to_string()),
            player,
            plan,
        });
        let device = self.device_name.map(|name| SessionDeviceSummary {
            id: self.device_id,
            name,
            device_type: self.device_type.unwrap_or_else(|| "other".to_string()),
            location: self.device_location,
            status: self
                .device_status
                .unwrap_or_else(|| "available".to_string()),
        });

        UsageSessionResponse {
            id: self.id,
            player_plan_id: self.player_plan_id,
            device_id: self.device_id,
            game_id: self.game_id,
            start_time: self.start_time,
            end_time: self.end_time,
            duration_minutes: self.duration_minutes,
            time_credits_consumed: self.time_credits_consumed,
            created_by: self.created_by,
            updated_by: self.updated_by,
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted_at: self.deleted_at,
            player_plan,
            device,
        }
    }
}
