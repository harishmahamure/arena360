use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::models::deduction_profile::DeductionProfile;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UsageSession {
    pub id: Uuid,
    pub balance_id: Option<Uuid>,
    pub device_id: Uuid,
    pub shift_id: Option<Uuid>,
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
    pub balance_id: Uuid,
    pub device_id: Uuid,
    pub shift_id: Option<Uuid>,
    pub start_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EndSessionDto {
    pub end_time: Option<DateTime<Utc>>,
    pub time_credits_consumed: Option<i32>,
    pub staff_totp: Option<String>,
    /// One of: voluntary, auto, force, offline_reconcile. Echoed into the
    /// `session.ended` realtime event. Persistence is gated on ADR-0021.
    pub reason: Option<String>,
}

/// Allowed `session.ended` reasons (ADR-0021). Kept in the service layer so new
/// reasons are a code change, not a migration.
pub const SESSION_END_REASONS: &[&str] = &["voluntary", "auto", "force", "offline_reconcile"];

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct SessionFilterDto {
    pub balance_id: Option<Uuid>,
    pub device_id: Option<Uuid>,
    pub player_id: Option<Uuid>,
    pub shift_id: Option<Uuid>,
    pub is_active: Option<i32>,
    pub start_time_from: Option<DateTime<Utc>>,
    pub start_time_to: Option<DateTime<Utc>>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionDeviceSummary {
    pub id: Uuid,
    pub name: String,
    pub device_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionPlayerSummary {
    pub id: Uuid,
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionPlanSummary {
    pub id: Uuid,
    pub name: String,
    pub plan_type: String,
    pub time_credits: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionBalanceSummary {
    pub id: Uuid,
    pub player_id: Uuid,
    pub kind: String,
    pub remaining_minutes: i32,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub player: Option<SessionPlayerSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<SessionPlanSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deduction_profile: Option<DeductionProfile>,
    pub expiry_date: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UsageSessionResponse {
    pub id: Uuid,
    pub balance_id: Option<Uuid>,
    pub device_id: Uuid,
    pub shift_id: Option<Uuid>,
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
    pub balance: Option<SessionBalanceSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<SessionDeviceSummary>,
    /// IANA timezone for peak/low window math (enriched list/detail only).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub cafe_timezone: String,
}

impl From<UsageSession> for UsageSessionResponse {
    fn from(value: UsageSession) -> Self {
        Self {
            id: value.id,
            balance_id: value.balance_id,
            device_id: value.device_id,
            shift_id: value.shift_id,
            start_time: value.start_time,
            end_time: value.end_time,
            duration_minutes: value.duration_minutes,
            time_credits_consumed: value.time_credits_consumed,
            created_by: value.created_by,
            updated_by: value.updated_by,
            created_at: value.created_at,
            updated_at: value.updated_at,
            deleted_at: value.deleted_at,
            balance: None,
            device: None,
            cafe_timezone: String::new(),
        }
    }
}

#[derive(Debug, FromRow)]
pub struct UsageSessionRow {
    pub id: Uuid,
    pub balance_id: Option<Uuid>,
    pub device_id: Uuid,
    pub shift_id: Option<Uuid>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_minutes: Option<i32>,
    pub time_credits_consumed: Option<i32>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub bal_player_id: Option<Uuid>,
    pub bal_kind: Option<String>,
    pub bal_remaining_minutes: Option<i32>,
    pub bal_status: Option<String>,
    pub bal_source_plan_id: Option<Uuid>,
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
    pub bal_deduction_profile: Option<Value>,
    pub bal_expiry_date: Option<DateTime<Utc>>,
}

fn parse_deduction_profile(value: Option<Value>) -> Option<DeductionProfile> {
    value.and_then(|v| serde_json::from_value(v).ok())
}

impl UsageSessionRow {
    pub fn into_response(self) -> UsageSessionResponse {
        let player = self
            .player_username
            .as_ref()
            .map(|username| SessionPlayerSummary {
                id: self.bal_player_id.unwrap_or_default(),
                username: username.clone(),
                first_name: self.player_first_name.clone(),
                last_name: self.player_last_name.clone(),
            });
        let plan = self.plan_name.as_ref().map(|name| SessionPlanSummary {
            id: self.bal_source_plan_id.unwrap_or_default(),
            name: name.clone(),
            plan_type: self
                .plan_type
                .clone()
                .unwrap_or_else(|| "time_based".to_string()),
            time_credits: self.plan_time_credits.unwrap_or(0),
        });
        let deduction_profile = parse_deduction_profile(self.bal_deduction_profile);
        let balance = self.balance_id.map(|bid| SessionBalanceSummary {
            id: bid,
            player_id: self.bal_player_id.unwrap_or_default(),
            kind: self.bal_kind.unwrap_or_else(|| "time".to_string()),
            remaining_minutes: self.bal_remaining_minutes.unwrap_or(0),
            status: self.bal_status.unwrap_or_else(|| "active".to_string()),
            player,
            plan,
            deduction_profile,
            expiry_date: self.bal_expiry_date.unwrap_or_else(Utc::now),
        });
        let device = self.device_name.map(|name| SessionDeviceSummary {
            id: self.device_id,
            name,
            device_type: self
                .device_type
                .unwrap_or_else(|| super::DEFAULT_DEVICE_TYPE.to_string()),
            location: self.device_location,
            status: self
                .device_status
                .unwrap_or_else(|| "available".to_string()),
        });

        UsageSessionResponse {
            id: self.id,
            balance_id: self.balance_id,
            device_id: self.device_id,
            shift_id: self.shift_id,
            start_time: self.start_time,
            end_time: self.end_time,
            duration_minutes: self.duration_minutes,
            time_credits_consumed: self.time_credits_consumed,
            created_by: self.created_by,
            updated_by: self.updated_by,
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted_at: self.deleted_at,
            balance,
            device,
            cafe_timezone: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_profile_json() -> Value {
        serde_json::json!({
            "peakWindowStart": "18:00:00",
            "peakWindowEnd": "23:00:00",
            "peakRatio": 1.5,
            "lowWindowStart": "07:00:00",
            "lowWindowEnd": "11:00:00",
            "lowRatio": 0.8
        })
    }

    #[test]
    fn into_response_maps_balance_deduction_profile() {
        let now = Utc::now();
        let balance_id = Uuid::new_v4();
        let row = UsageSessionRow {
            id: Uuid::new_v4(),
            balance_id: Some(balance_id),
            device_id: Uuid::new_v4(),
            shift_id: None,
            start_time: now,
            end_time: None,
            duration_minutes: None,
            time_credits_consumed: None,
            created_by: None,
            updated_by: None,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            bal_player_id: Some(Uuid::new_v4()),
            bal_kind: Some("time".to_string()),
            bal_remaining_minutes: Some(300),
            bal_status: Some("active".to_string()),
            bal_source_plan_id: Some(Uuid::new_v4()),
            player_username: Some("player1".to_string()),
            player_first_name: None,
            player_last_name: None,
            plan_name: Some("Peak plan".to_string()),
            plan_type: Some("time_based".to_string()),
            plan_time_credits: Some(300),
            device_name: Some("PC-1".to_string()),
            device_type: Some("PC".to_string()),
            device_location: None,
            device_status: Some("in_use".to_string()),
            bal_deduction_profile: Some(sample_profile_json()),
            bal_expiry_date: Some(now + chrono::Duration::days(7)),
        };

        let response = row.into_response();
        let balance = response.balance.expect("balance summary");
        let profile = balance
            .deduction_profile
            .expect("deduction profile on balance");
        assert!((profile.peak_ratio - 1.5).abs() < f64::EPSILON);
        assert_eq!(balance.id, balance_id);
    }
}
