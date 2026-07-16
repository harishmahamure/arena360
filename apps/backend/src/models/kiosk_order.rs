use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

pub mod kiosk_order_status {
    pub const PENDING: &str = "pending";
    pub const PREPARING: &str = "preparing";
    pub const FULFILLED: &str = "fulfilled";
    pub const CANCELLED: &str = "cancelled";

    pub const OPEN: &[&str] = &[PENDING, PREPARING];
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KioskOrder {
    pub id: Uuid,
    pub session_id: Uuid,
    pub player_id: Uuid,
    pub device_id: Uuid,
    pub status: String,
    pub player_note: Option<String>,
    pub transaction_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub fulfilled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KioskOrderItem {
    pub id: Uuid,
    pub order_id: Uuid,
    pub product_id: Uuid,
    pub quantity: i32,
    pub product_name: String,
    pub unit_price: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KioskOrderItemResponse {
    pub id: Uuid,
    pub product_id: Uuid,
    pub quantity: i32,
    pub product_name: String,
    pub unit_price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KioskOrderWithItems {
    pub id: Uuid,
    pub session_id: Uuid,
    pub player_id: Uuid,
    pub device_id: Uuid,
    pub status: String,
    pub player_note: Option<String>,
    pub transaction_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub fulfilled_at: Option<DateTime<Utc>>,
    pub line_items: Vec<KioskOrderItemResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub player_username: Option<String>,
}

impl KioskOrderWithItems {
    pub fn from_parts(
        order: KioskOrder,
        items: Vec<KioskOrderItem>,
        device_name: Option<String>,
        player_username: Option<String>,
    ) -> Self {
        Self {
            id: order.id,
            session_id: order.session_id,
            player_id: order.player_id,
            device_id: order.device_id,
            status: order.status,
            player_note: order.player_note,
            transaction_id: order.transaction_id,
            created_at: order.created_at,
            updated_at: order.updated_at,
            fulfilled_at: order.fulfilled_at,
            line_items: items
                .into_iter()
                .map(|i| KioskOrderItemResponse {
                    id: i.id,
                    product_id: i.product_id,
                    quantity: i.quantity,
                    product_name: i.product_name,
                    unit_price: i.unit_price,
                })
                .collect(),
            device_name,
            player_username,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KioskMenuProduct {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub category: String,
    pub price: f64,
    pub stock_available: i32,
    pub in_stock: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateKioskOrderLineItemDto {
    pub product_id: Uuid,
    pub quantity: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateKioskOrderDto {
    pub line_items: Vec<CreateKioskOrderLineItemDto>,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateKioskOrderDto {
    pub status: String,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct KioskOrderFilterDto {
    pub status: Option<String>,
    pub device_id: Option<Uuid>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConvertKioskOrderDto {
    pub payment_method: String,
    pub payment_status: Option<String>,
    pub notes: Option<String>,
    pub online_payment_ref_last4: Option<String>,
    pub cash_amount: Option<f64>,
    pub online_amount: Option<f64>,
    pub sale_location_id: Option<Uuid>,
}
