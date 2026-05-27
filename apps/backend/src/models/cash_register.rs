use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CashRegister {
    pub id: Uuid,
    pub shift_id: Uuid,
    pub opened_by: Uuid,
    pub closed_by: Option<Uuid>,
    pub opening_balance: f64,
    pub opening_denominations: Option<serde_json::Value>,
    pub closing_balance: Option<f64>,
    pub closing_denominations: Option<serde_json::Value>,
    pub expected_closing: Option<f64>,
    pub variance: Option<f64>,
    pub status: String,
    pub notes: Option<String>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CashRegisterEntry {
    pub id: Uuid,
    pub cash_register_id: Uuid,
    pub entry_type: String,
    pub amount: f64,
    pub reason: Option<String>,
    pub reference_id: Option<Uuid>,
    pub reference_type: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OpenCashRegisterDto {
    pub shift_id: Uuid,
    pub opening_balance: f64,
    pub opening_denominations: Option<serde_json::Value>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CloseCashRegisterDto {
    pub closing_balance: f64,
    pub closing_denominations: Option<serde_json::Value>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateCashRegisterEntryDto {
    pub entry_type: String,
    pub amount: f64,
    pub reason: Option<String>,
    pub reference_id: Option<Uuid>,
    pub reference_type: Option<String>,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct CashRegisterFilterDto {
    pub shift_id: Option<Uuid>,
    pub status: Option<String>,
    pub opened_by: Option<Uuid>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CashRegisterWithEntries {
    #[serde(flatten)]
    pub register: CashRegister,
    pub entries: Vec<CashRegisterEntry>,
}
