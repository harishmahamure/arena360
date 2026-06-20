use chrono::{DateTime, NaiveTime, Utc};

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub price: f64,
    pub plan_type: String,
    pub validity_days: i32,
    pub time_window_start: Option<NaiveTime>,
    pub time_window_end: Option<NaiveTime>,
    pub time_credits: i32,
    pub is_active: bool,
    pub device_type: Option<String>,
    pub device_sub_type: Option<String>,
    pub allowed_days: Option<Value>,
    pub allowed_months: Option<Value>,
    pub dynamic_deduction_enabled: bool,
    pub deduction_profile: Option<Value>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreatePlanDto {
    pub name: String,
    pub description: Option<String>,
    pub price: f64,
    pub plan_type: String,
    pub validity_days: Option<i32>,
    pub time_window_start: Option<String>,
    pub time_window_end: Option<String>,
    pub time_credits: Option<i32>,
    pub is_active: Option<bool>,
    pub device_type: Option<String>,
    pub device_sub_type: Option<String>,
    pub allowed_days: Option<Value>,
    pub allowed_months: Option<Value>,
    pub dynamic_deduction_enabled: Option<bool>,
    pub deduction_profile: Option<Value>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePlanDto {
    pub name: Option<String>,
    pub description: Option<String>,
    pub price: Option<f64>,
    pub plan_type: Option<String>,
    pub validity_days: Option<i32>,
    pub time_window_start: Option<String>,
    pub time_window_end: Option<String>,
    pub time_credits: Option<i32>,
    pub is_active: Option<bool>,
    pub device_type: Option<String>,
    pub device_sub_type: Option<String>,
    pub allowed_days: Option<Value>,
    pub allowed_months: Option<Value>,
    pub dynamic_deduction_enabled: Option<bool>,
    pub deduction_profile: Option<Value>,
}

#[derive(Debug, Deserialize, Serialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct PlanFilterDto {
    pub search: Option<String>,
    pub plan_type: Option<String>,
    pub is_active: Option<i64>,
    pub min_price: Option<f64>,
    pub max_price: Option<f64>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub device_type: Option<String>,
    pub device_sub_type: Option<String>,
}

impl PlanFilterDto {
    pub fn is_active_bool(&self) -> Option<bool> {
        self.is_active.map(|v| v != 0)
    }
}

pub fn parse_time(value: &str) -> Result<NaiveTime, crate::error::AppError> {
    NaiveTime::parse_from_str(value, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(value, "%H:%M"))
        .map_err(|_| crate::error::AppError::BadRequest(format!("Invalid time format: {value}")))
}

pub fn parse_deduction_profile(
    value: &Value,
) -> Result<crate::models::deduction_profile::DeductionProfile, AppError> {
    let profile: crate::models::deduction_profile::DeductionProfile =
        serde_json::from_value(value.clone()).map_err(|e| {
            AppError::BadRequest(format!("Invalid deductionProfile JSON: {e}"))
        })?;
    profile
        .validate()
        .map_err(AppError::BadRequest)?;
    Ok(profile)
}
