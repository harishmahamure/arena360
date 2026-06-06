use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StockReceipt {
    pub id: Uuid,
    pub location_id: Uuid,
    pub vendor_id: Option<Uuid>,
    pub notes: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StockReceiptLine {
    pub id: Uuid,
    pub receipt_id: Uuid,
    pub product_id: Uuid,
    pub box_quantity: i32,
    pub pieces_added: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StockReceiptWithLines {
    #[serde(flatten)]
    pub receipt: StockReceipt,
    pub lines: Vec<StockReceiptLine>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateStockReceiptLineDto {
    pub product_id: Uuid,
    pub box_quantity: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateStockReceiptDto {
    pub location_id: Uuid,
    pub vendor_id: Option<Uuid>,
    pub notes: Option<String>,
    pub lines: Vec<CreateStockReceiptLineDto>,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct StockReceiptFilterDto {
    pub location_id: Option<Uuid>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}
