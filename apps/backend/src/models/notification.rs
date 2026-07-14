use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

pub mod activity_kind {
    pub const TRANSACTION_SALE: &str = "transaction_sale";
    pub const PLAN_SALE: &str = "plan_sale";
    pub const CREDIT_SETTLEMENT: &str = "credit_settlement";
    pub const APPROVAL_REQUESTED: &str = "approval_requested";
    pub const APPROVAL_DECIDED: &str = "approval_decided";
    pub const SESSION_STARTED: &str = "session_started";
    pub const SESSION_ENDED: &str = "session_ended";
    pub const DEVICE_STATUS_CHANGED: &str = "device_status_changed";
    pub const SHIFT_CLOCK_IN: &str = "shift_clock_in";
    pub const SHIFT_CLOCK_OUT: &str = "shift_clock_out";
    pub const SHIFT_HANDOVER: &str = "shift_handover";
    pub const CASH_REGISTER_OPENED: &str = "cash_register_opened";
    pub const CASH_REGISTER_CLOSED: &str = "cash_register_closed";
    pub const CASH_DEPOSIT_INITIATED: &str = "cash_deposit_initiated";
    pub const INVENTORY_TRANSFER_REQUESTED: &str = "inventory_transfer_requested";
    pub const INVENTORY_WASTE_RECORDED: &str = "inventory_waste_recorded";
    pub const KIOSK_ORDER_PLACED: &str = "kiosk_order_placed";
    pub const KIOSK_ORDER_FULFILLED: &str = "kiosk_order_fulfilled";
    pub const KIOSK_ORDER_CANCELLED: &str = "kiosk_order_cancelled";

    pub const STAFF_SHARED: &[&str] = &[
        SESSION_STARTED,
        SESSION_ENDED,
        DEVICE_STATUS_CHANGED,
        KIOSK_ORDER_PLACED,
        KIOSK_ORDER_FULFILLED,
        KIOSK_ORDER_CANCELLED,
    ];

    /// Actionable alerts for staff bell badge and default inbox filter.
    pub const STAFF_IMPORTANT: &[&str] = &[
        APPROVAL_REQUESTED,
        APPROVAL_DECIDED,
        KIOSK_ORDER_PLACED,
        KIOSK_ORDER_CANCELLED,
        CASH_DEPOSIT_INITIATED,
        INVENTORY_TRANSFER_REQUESTED,
    ];
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityLog {
    pub id: Uuid,
    pub kind: String,
    pub title: String,
    pub summary: Option<String>,
    pub payload: Value,
    pub actor_user_id: Option<Uuid>,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserNotification {
    pub id: Uuid,
    pub activity_id: Uuid,
    pub user_id: Uuid,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NotificationItem {
    pub id: Uuid,
    pub activity_id: Uuid,
    pub kind: String,
    pub title: String,
    pub summary: Option<String>,
    pub payload: Value,
    pub actor_user_id: Option<Uuid>,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct NotificationFilterDto {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub unread_only: Option<bool>,
    pub important_only: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ActivityLogFilterDto {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub kind: Option<String>,
    pub actor_user_id: Option<Uuid>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UnreadCountDto {
    pub count: i64,
}
