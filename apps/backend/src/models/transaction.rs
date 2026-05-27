use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub id: Uuid,
    pub player_id: Uuid,
    pub transaction_type: String,
    pub plan_id: Option<Uuid>,
    pub amount: f64,
    pub cash_amount: Option<f64>,
    pub online_amount: Option<f64>,
    pub payment_method: String,
    pub payment_status: String,
    pub notes: Option<String>,
    pub transaction_date: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateTransactionDto {
    pub player_id: Uuid,
    pub transaction_type: String,
    pub plan_id: Option<Uuid>,
    pub amount: Option<f64>,
    pub payment_method: String,
    pub payment_status: Option<String>,
    pub notes: Option<String>,
    pub transaction_date: Option<DateTime<Utc>>,
    pub cash_amount: Option<f64>,
    pub online_amount: Option<f64>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTransactionDto {
    pub payment_status: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct TransactionFilterDto {
    pub player_id: Option<Uuid>,
    pub transaction_type: Option<String>,
    pub plan_id: Option<Uuid>,
    pub payment_method: Option<String>,
    pub payment_status: Option<String>,
    pub transaction_date_from: Option<DateTime<Utc>>,
    pub transaction_date_to: Option<DateTime<Utc>>,
    pub min_amount: Option<f64>,
    pub max_amount: Option<f64>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}
