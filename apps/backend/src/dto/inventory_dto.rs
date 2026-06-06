use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct WasteSummaryFilterDto {
    pub location_id: Option<Uuid>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProductPricingSnapshot {
    pub day_price: f64,
    pub night_price: f64,
    pub effective_price: f64,
    pub is_night_window: bool,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApproveInventoryActionDto {
    pub notes: Option<String>,
}
