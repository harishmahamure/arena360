use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StockTransferRequest {
    pub id: Uuid,
    pub from_location_id: Uuid,
    pub to_location_id: Uuid,
    pub status: String,
    pub requested_by: Option<Uuid>,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub rejection_reason: Option<String>,
    pub fulfilled_by: Option<Uuid>,
    pub fulfilled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StockTransferLine {
    pub id: Uuid,
    pub transfer_request_id: Uuid,
    pub product_id: Uuid,
    pub quantity_pieces: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StockTransferRequestWithLines {
    #[serde(flatten)]
    pub request: StockTransferRequest,
    pub lines: Vec<StockTransferLine>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateStockTransferLineDto {
    pub product_id: Uuid,
    pub quantity_pieces: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateStockTransferRequestDto {
    pub from_location_id: Option<Uuid>,
    pub to_location_id: Option<Uuid>,
    pub lines: Vec<CreateStockTransferLineDto>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RejectStockTransferDto {
    pub rejection_reason: String,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct StockTransferFilterDto {
    pub status: Option<String>,
    pub from_location_id: Option<Uuid>,
    pub to_location_id: Option<Uuid>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}
