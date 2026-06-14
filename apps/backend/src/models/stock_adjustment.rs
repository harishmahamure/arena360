use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StockAdjustment {
    pub id: Uuid,
    pub location_id: Uuid,
    pub notes: String,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StockAdjustmentLine {
    pub id: Uuid,
    pub adjustment_id: Uuid,
    pub product_id: Uuid,
    pub previous_pieces: i32,
    pub counted_pieces: i32,
    pub delta_pieces: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StockAdjustmentWithLines {
    #[serde(flatten)]
    pub adjustment: StockAdjustment,
    pub lines: Vec<StockAdjustmentLine>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateStockAdjustmentLineDto {
    pub product_id: Uuid,
    pub counted_pieces: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateStockAdjustmentDto {
    pub location_id: Uuid,
    pub notes: String,
    pub lines: Vec<CreateStockAdjustmentLineDto>,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct StockAdjustmentFilterDto {
    pub location_id: Option<Uuid>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}
