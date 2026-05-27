use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::dto::AuthUserDto;
use crate::models::cash_deposit::CashDeposit;
use crate::models::CashRegister;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Shift {
    pub id: Uuid,
    pub user_id: Uuid,
    pub clock_in: DateTime<Utc>,
    pub clock_out: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub status: String,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClockInDto {
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClockOutDto {
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HandoverDepositDto {
    pub amount: f64,
    pub denominations: serde_json::Value,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShiftCloseDto {
    pub closing_balance: f64,
    pub closing_denominations: Option<serde_json::Value>,
    pub notes: Option<String>,
    pub deposit: Option<HandoverDepositDto>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShiftHandoverDto {
    pub closing_balance: f64,
    pub closing_denominations: Option<serde_json::Value>,
    pub notes: Option<String>,
    pub validator_username: String,
    pub validator_password: String,
    pub validator_totp: String,
    pub deposit: Option<HandoverDepositDto>,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShiftCloseResponseDto {
    pub closedShift: Shift,
    pub cashRegister: Option<CashRegister>,
    pub deposit: Option<CashDeposit>,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShiftHandoverResponseDto {
    pub closedShift: Shift,
    pub cashRegister: Option<CashRegister>,
    pub deposit: Option<CashDeposit>,
    pub newAccessToken: String,
    pub newUser: AuthUserDto,
    pub newShiftId: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CashRegisterSummary {
    pub id: Uuid,
    pub shift_id: Uuid,
    pub opening_balance: f64,
    pub expected_closing: f64,
    pub status: String,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ShiftFilterDto {
    pub user_id: Option<Uuid>,
    pub status: Option<String>,
    pub clock_in_from: Option<DateTime<Utc>>,
    pub clock_in_to: Option<DateTime<Utc>>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}
