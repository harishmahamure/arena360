use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const STAFF_ALLOWANCE_PERIOD_DAYS: i64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum StaffGamingAllowanceStatus {
    Active,
    Expired,
    Exhausted,
    None,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetStaffGamingAllowanceDto {
    pub allotted_hours: f64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaffGamingAllowanceSummary {
    pub user_id: uuid::Uuid,
    pub status: StaffGamingAllowanceStatus,
    pub allotted_minutes: i32,
    pub remaining_minutes: i32,
    pub used_minutes: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period_start: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period_end: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance_id: Option<uuid::Uuid>,
}

impl StaffGamingAllowanceSummary {
    pub fn none(user_id: uuid::Uuid) -> Self {
        Self {
            user_id,
            status: StaffGamingAllowanceStatus::None,
            allotted_minutes: 0,
            remaining_minutes: 0,
            used_minutes: 0,
            period_start: None,
            period_end: None,
            balance_id: None,
        }
    }
}
