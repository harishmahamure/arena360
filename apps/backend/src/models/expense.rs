use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Expense {
    pub id: Uuid,
    pub category_id: Uuid,
    pub vendor_id: Option<Uuid>,
    pub amount: f64,
    pub payment_method: String,
    pub payment_account: Option<String>,
    pub description: Option<String>,
    pub receipt_url: Option<String>,
    pub expense_date: DateTime<Utc>,
    pub is_recurring: bool,
    pub recurrence_pattern: Option<String>,
    pub next_recurrence_date: Option<DateTime<Utc>>,
    pub approval_status: String,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub rejection_reason: Option<String>,
    pub shift_id: Option<Uuid>,
    pub cash_register_entry_id: Option<Uuid>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateExpenseDto {
    pub category_id: Uuid,
    pub vendor_id: Option<Uuid>,
    pub amount: f64,
    pub payment_method: String,
    pub payment_account: Option<String>,
    pub description: Option<String>,
    pub receipt_url: Option<String>,
    pub expense_date: Option<DateTime<Utc>>,
    pub is_recurring: Option<bool>,
    pub recurrence_pattern: Option<String>,
    pub shift_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateExpenseDto {
    pub category_id: Option<Uuid>,
    pub vendor_id: Option<Uuid>,
    pub amount: Option<f64>,
    pub payment_method: Option<String>,
    pub payment_account: Option<String>,
    pub description: Option<String>,
    pub receipt_url: Option<String>,
    pub expense_date: Option<DateTime<Utc>>,
    pub is_recurring: Option<bool>,
    pub recurrence_pattern: Option<String>,
    pub shift_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApproveExpenseDto {
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RejectExpenseDto {
    pub rejection_reason: String,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseFilterDto {
    pub category_id: Option<Uuid>,
    pub vendor_id: Option<Uuid>,
    pub approval_status: Option<String>,
    pub payment_method: Option<String>,
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
    pub min_amount: Option<f64>,
    pub max_amount: Option<f64>,
    pub shift_id: Option<Uuid>,
    pub is_recurring: Option<bool>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseSummaryDto {
    pub category_name: String,
    pub budget_amount: Option<f64>,
    pub budget_period: Option<String>,
    pub total_spent: f64,
    pub remaining_budget: Option<f64>,
    pub expense_count: i64,
}
