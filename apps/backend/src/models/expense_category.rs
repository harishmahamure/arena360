use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseCategory {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub parent_id: Option<Uuid>,
    pub is_active: bool,
    pub budget_amount: Option<f64>,
    pub budget_period: Option<String>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateExpenseCategoryDto {
    pub name: String,
    pub description: Option<String>,
    pub parent_id: Option<Uuid>,
    pub is_active: Option<bool>,
    pub budget_amount: Option<f64>,
    pub budget_period: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateExpenseCategoryDto {
    pub name: Option<String>,
    pub description: Option<String>,
    pub parent_id: Option<Uuid>,
    pub is_active: Option<bool>,
    pub budget_amount: Option<f64>,
    pub budget_period: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseCategoryFilterDto {
    pub name: Option<String>,
    pub is_active: Option<bool>,
    pub parent_id: Option<Uuid>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}
