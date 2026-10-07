use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseOrder {
    pub id: Uuid,
    pub po_number: String,
    pub vendor_id: Uuid,
    pub destination_location_id: Uuid,
    pub status: String,
    pub expected_delivery_date: Option<NaiveDate>,
    pub subtotal: f64,
    pub discount: f64,
    pub tax: f64,
    pub freight: f64,
    pub total: f64,
    pub notes: Option<String>,
    pub rejection_reason: Option<String>,
    pub version: i32,
    pub created_by: Option<Uuid>,
    pub submitted_by: Option<Uuid>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub ordered_by: Option<Uuid>,
    pub ordered_at: Option<DateTime<Utc>>,
    pub cancelled_by: Option<Uuid>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseOrderLine {
    pub id: Uuid,
    pub purchase_order_id: Uuid,
    pub product_id: Uuid,
    pub product_name: Option<String>,
    pub product_sku: Option<String>,
    pub ordered_boxes: i32,
    pub received_boxes: i32,
    pub units_per_box_snapshot: i32,
    pub box_cost_snapshot: f64,
    pub tax_rate: f64,
    pub line_subtotal: f64,
    pub line_tax: f64,
    pub line_total: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseOrderWithLines {
    #[serde(flatten)]
    pub order: PurchaseOrder,
    pub lines: Vec<PurchaseOrderLine>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseOrderLineInput {
    pub product_id: Uuid,
    pub ordered_boxes: i32,
    pub box_cost: f64,
    pub tax_rate: Option<f64>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreatePurchaseOrderDto {
    pub vendor_id: Uuid,
    pub destination_location_id: Uuid,
    pub expected_delivery_date: Option<NaiveDate>,
    pub discount: Option<f64>,
    pub freight: Option<f64>,
    pub notes: Option<String>,
    pub lines: Vec<PurchaseOrderLineInput>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePurchaseOrderDto {
    pub version: i32,
    pub vendor_id: Option<Uuid>,
    pub destination_location_id: Option<Uuid>,
    pub expected_delivery_date: Option<NaiveDate>,
    pub discount: Option<f64>,
    pub freight: Option<f64>,
    pub notes: Option<String>,
    pub lines: Option<Vec<PurchaseOrderLineInput>>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RejectPurchaseOrderDto {
    pub reason: String,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseOrderFilterDto {
    pub status: Option<String>,
    pub vendor_id: Option<Uuid>,
    pub destination_location_id: Option<Uuid>,
    pub search: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseOrderReceiptLineInput {
    pub purchase_order_line_id: Uuid,
    pub accepted_boxes: i32,
    pub rejected_boxes: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReceivePurchaseOrderDto {
    pub invoice_reference: String,
    pub payment_method: String,
    pub payment_account: Option<String>,
    pub receipt_date: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub lines: Vec<PurchaseOrderReceiptLineInput>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReceivePurchaseOrderResponse {
    pub purchase_order: PurchaseOrderWithLines,
    pub receipt_id: Uuid,
    pub expense_id: Uuid,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InventoryReorderRule {
    pub id: Uuid,
    pub location_id: Uuid,
    pub product_id: Uuid,
    pub product_name: Option<String>,
    pub minimum_pieces: i32,
    pub target_pieces: i32,
    pub preferred_vendor_id: Option<Uuid>,
    pub lead_time_days: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpsertInventoryReorderRuleDto {
    pub location_id: Uuid,
    pub product_id: Uuid,
    pub minimum_pieces: i32,
    pub target_pieces: i32,
    pub preferred_vendor_id: Option<Uuid>,
    pub lead_time_days: Option<i32>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReorderSuggestion {
    pub rule_id: Uuid,
    pub location_id: Uuid,
    pub location_name: String,
    pub product_id: Uuid,
    pub product_name: String,
    pub current_pieces: i32,
    pub minimum_pieces: i32,
    pub target_pieces: i32,
    pub suggested_pieces: i32,
    pub preferred_vendor_id: Option<Uuid>,
    pub lead_time_days: i32,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StockMovementRow {
    pub id: Uuid,
    pub location_id: Uuid,
    pub location_name: String,
    pub product_id: Uuid,
    pub product_name: String,
    pub delta: i32,
    pub movement_type: String,
    pub reference_id: Option<Uuid>,
    pub reference_type: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct StockMovementFilterDto {
    pub location_id: Option<Uuid>,
    pub product_id: Option<Uuid>,
    pub movement_type: Option<String>,
    pub reference_id: Option<Uuid>,
    pub reference_type: Option<String>,
    pub actor_id: Option<Uuid>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InventoryOverviewDto {
    pub total_pieces: i64,
    pub estimated_stock_value: f64,
    pub low_stock_products: i64,
    pub out_of_stock_products: i64,
    pub open_purchase_orders: i64,
    pub pending_transfers: i64,
    pub pending_waste_events: i64,
    pub recent_movements: Vec<StockMovementRow>,
}
