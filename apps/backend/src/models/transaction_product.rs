use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransactionProduct {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub product_id: Uuid,
    pub quantity: i32,
    pub unit_price: f64,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransactionProductResponse {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub product_id: Uuid,
    pub quantity: i32,
    pub unit_price: f64,
    pub product_name: String,
    pub product_sku: Option<String>,
    pub product_price: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateLineItemDto {
    pub product_id: Uuid,
    pub quantity: i32,
    pub unit_price: Option<f64>,
}
