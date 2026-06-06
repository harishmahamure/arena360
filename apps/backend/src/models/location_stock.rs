use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LocationStockRow {
    pub location_id: Uuid,
    pub product_id: Uuid,
    pub quantity_pieces: i32,
    pub product_name: Option<String>,
    pub product_sku: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct LocationStockFilterDto {
    pub location_id: Option<Uuid>,
    pub product_id: Option<Uuid>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}
