use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StockWasteEvent {
    pub id: Uuid,
    pub location_id: Uuid,
    pub status: String,
    pub notes: Option<String>,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub rejection_reason: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StockWasteLine {
    pub id: Uuid,
    pub waste_event_id: Uuid,
    pub product_id: Uuid,
    pub quantity_pieces: i32,
    pub reason_code: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StockWasteEventWithLines {
    #[serde(flatten)]
    pub event: StockWasteEvent,
    pub lines: Vec<StockWasteLine>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateStockWasteLineDto {
    pub product_id: Uuid,
    pub quantity_pieces: i32,
    pub reason_code: String,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateStockWasteEventDto {
    pub location_id: Uuid,
    pub notes: Option<String>,
    pub lines: Vec<CreateStockWasteLineDto>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RejectStockWasteDto {
    pub rejection_reason: String,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct StockWasteFilterDto {
    pub status: Option<String>,
    pub location_id: Option<Uuid>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WasteSummaryRow {
    pub reason_code: String,
    pub product_id: Uuid,
    pub product_name: String,
    pub location_id: Uuid,
    pub location_name: String,
    pub total_pieces: i64,
    pub estimated_cost: f64,
}
