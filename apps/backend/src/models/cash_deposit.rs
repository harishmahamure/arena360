use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CashDeposit {
    pub id: Uuid,
    pub cash_register_id: Uuid,
    pub shift_id: Uuid,
    pub initiated_by: Uuid,
    pub approved_by: Option<Uuid>,
    pub amount: f64,
    pub denominations: serde_json::Value,
    pub deposit_type: Option<String>,
    pub status: String,
    pub approved_at: Option<DateTime<Utc>>,
    pub rejection_reason: Option<String>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InitiateDepositDto {
    pub cash_register_id: Uuid,
    pub shift_id: Uuid,
    pub amount: f64,
    pub denominations: serde_json::Value,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApproveDepositDto {
    pub deposit_type: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RejectDepositDto {
    pub rejection_reason: String,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct CashDepositFilterDto {
    pub shift_id: Option<Uuid>,
    pub cash_register_id: Option<Uuid>,
    pub status: Option<String>,
    pub initiated_by: Option<Uuid>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}
