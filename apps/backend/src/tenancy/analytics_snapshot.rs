//! Explicit, secret-free projection shared by live events and consistent rebuilds.
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqliteConnection;
use uuid::Uuid;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnKind {
    Uuid,
    Text,
    Timestamp,
    Date,
    Integer,
    Boolean,
    Money,
    StockKey,
}
pub struct Column {
    pub name: &'static str,
    pub source: &'static str,
    pub kind: ColumnKind,
}
pub struct Table {
    pub name: &'static str,
    pub source: &'static str,
    pub predicate: &'static str,
    pub columns: &'static [Column],
    pub parent: Option<&'static str>,
    pub time: Option<&'static str>,
}
pub static TABLES: &[Table] = &[
    Table {
        name: "users",
        source: "users r",
        predicate: "r.deleted_at IS NULL",
        parent: None,
        time: None,
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "username",
                source: "r.username",
                kind: ColumnKind::Text,
            },
            Column {
                name: "first_name",
                source: "r.first_name",
                kind: ColumnKind::Text,
            },
            Column {
                name: "last_name",
                source: "r.last_name",
                kind: ColumnKind::Text,
            },
            Column {
                name: "role",
                source: "r.role",
                kind: ColumnKind::Text,
            },
            Column {
                name: "is_active",
                source: "r.is_active",
                kind: ColumnKind::Boolean,
            },
            Column {
                name: "credit_limit",
                source: "r.credit_limit",
                kind: ColumnKind::Money,
            },
            Column {
                name: "created_at",
                source: "r.created_at",
                kind: ColumnKind::Timestamp,
            },
        ],
    },
    Table {
        name: "venue_locations",
        source: "venue_locations r",
        predicate: "1=1",
        parent: None,
        time: None,
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "name",
                source: "r.name",
                kind: ColumnKind::Text,
            },
        ],
    },
    Table {
        name: "devices",
        source: "devices r",
        predicate: "r.deleted_at IS NULL",
        parent: None,
        time: None,
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "name",
                source: "r.name",
                kind: ColumnKind::Text,
            },
            Column {
                name: "status",
                source: "r.status",
                kind: ColumnKind::Text,
            },
            Column {
                name: "area_label",
                source: "r.location",
                kind: ColumnKind::Text,
            },
            Column {
                name: "device_type",
                source: "r.device_type",
                kind: ColumnKind::Text,
            },
            Column {
                name: "location_id",
                source: "r.location_id",
                kind: ColumnKind::Uuid,
            },
        ],
    },
    Table {
        name: "plans",
        source: "plans r",
        predicate: "r.deleted_at IS NULL",
        parent: None,
        time: None,
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "name",
                source: "r.name",
                kind: ColumnKind::Text,
            },
            Column {
                name: "price",
                source: "r.price",
                kind: ColumnKind::Money,
            },
            Column {
                name: "time_credit_minutes",
                source: "r.time_credits",
                kind: ColumnKind::Integer,
            },
            Column {
                name: "plan_type",
                source: "r.plan_type",
                kind: ColumnKind::Text,
            },
            Column {
                name: "validity_days",
                source: "r.validity_days",
                kind: ColumnKind::Integer,
            },
        ],
    },
    Table {
        name: "products",
        source: "products r",
        predicate: "r.deleted_at IS NULL",
        parent: None,
        time: None,
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "name",
                source: "r.name",
                kind: ColumnKind::Text,
            },
            Column {
                name: "purchase_price_per_box",
                source: "r.purchase_price_per_box",
                kind: ColumnKind::Money,
            },
            Column {
                name: "purchase_price",
                source: "NULL",
                kind: ColumnKind::Money,
            },
            Column {
                name: "units_per_purchase_unit",
                source: "r.units_per_purchase_unit",
                kind: ColumnKind::Integer,
            },
        ],
    },
    Table {
        name: "vendors",
        source: "vendors r",
        predicate: "1=1",
        parent: None,
        time: None,
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "name",
                source: "r.name",
                kind: ColumnKind::Text,
            },
        ],
    },
    Table {
        name: "inventory_locations",
        source: "inventory_locations r",
        predicate: "r.deleted_at IS NULL",
        parent: None,
        time: None,
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "name",
                source: "r.name",
                kind: ColumnKind::Text,
            },
            Column {
                name: "venue_location_id",
                source: "r.venue_location_id",
                kind: ColumnKind::Uuid,
            },
        ],
    },
    Table {
        name: "expense_categories",
        source: "expense_categories r",
        predicate: "1=1",
        parent: None,
        time: None,
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "name",
                source: "r.name",
                kind: ColumnKind::Text,
            },
            Column {
                name: "is_active",
                source: "r.is_active",
                kind: ColumnKind::Boolean,
            },
            Column {
                name: "budget_amount",
                source: "r.budget_amount",
                kind: ColumnKind::Money,
            },
            Column {
                name: "budget_period",
                source: "r.budget_period",
                kind: ColumnKind::Text,
            },
        ],
    },
    Table {
        name: "transactions",
        source: "transactions r",
        predicate: "r.deleted_at IS NULL",
        parent: None,
        time: Some("occurred_at"),
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "occurred_at",
                source: "r.transaction_date",
                kind: ColumnKind::Timestamp,
            },
            Column {
                name: "local_date",
                source: "NULL",
                kind: ColumnKind::Date,
            },
            Column {
                name: "location_id",
                source: "r.location_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "player_id",
                source: "r.player_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "plan_id",
                source: "r.plan_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "shift_id",
                source: "r.shift_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "created_by",
                source: "r.created_by",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "transaction_type",
                source: "r.transaction_type",
                kind: ColumnKind::Text,
            },
            Column {
                name: "payment_method",
                source: "r.payment_method",
                kind: ColumnKind::Text,
            },
            Column {
                name: "payment_status",
                source: "r.payment_status",
                kind: ColumnKind::Text,
            },
            Column {
                name: "amount",
                source: "r.amount",
                kind: ColumnKind::Money,
            },
            Column {
                name: "paid_amount",
                source: "r.paid_amount",
                kind: ColumnKind::Money,
            },
            Column {
                name: "cash_amount",
                source: "r.cash_amount",
                kind: ColumnKind::Money,
            },
            Column {
                name: "online_amount",
                source: "r.online_amount",
                kind: ColumnKind::Money,
            },
        ],
    },
    Table {
        name: "transaction_lines",
        source: "transaction_products r",
        predicate: "1=1",
        parent: Some("transaction_id"),
        time: None,
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "transaction_id",
                source: "r.transaction_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "product_id",
                source: "COALESCE(r.product_id,'00000000-0000-0000-0000-000000000000')",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "quantity",
                source: "r.quantity",
                kind: ColumnKind::Integer,
            },
            Column {
                name: "unit_price",
                source: "r.unit_price",
                kind: ColumnKind::Money,
            },
        ],
    },
    Table {
        name: "sessions",
        source: "usage_sessions r JOIN player_plan_balances b ON b.id=r.balance_id",
        predicate: "r.deleted_at IS NULL",
        parent: None,
        time: Some("start_time"),
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "device_id",
                source: "r.device_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "balance_id",
                source: "r.balance_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "player_id",
                source: "r.player_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "is_staff_allowance",
                source: "b.kind = 'staff_allowance'",
                kind: ColumnKind::Boolean,
            },
            Column {
                name: "location_id",
                source: "r.location_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "start_time",
                source: "r.start_time",
                kind: ColumnKind::Timestamp,
            },
            Column {
                name: "end_time",
                source: "r.end_time",
                kind: ColumnKind::Timestamp,
            },
            Column {
                name: "start_local_date",
                source: "NULL",
                kind: ColumnKind::Date,
            },
            Column {
                name: "duration_minutes",
                source: "r.duration_minutes",
                kind: ColumnKind::Integer,
            },
            Column {
                name: "shift_id",
                source: "r.shift_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "created_by",
                source: "r.created_by",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "source_plan_id",
                source: "r.source_plan_id_at_start",
                kind: ColumnKind::Uuid,
            },
        ],
    },
    Table {
        name: "wallets",
        source: "player_plan_balances r",
        predicate: "r.deleted_at IS NULL",
        parent: None,
        time: None,
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "player_id",
                source: "r.player_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "status",
                source: "r.status",
                kind: ColumnKind::Text,
            },
            Column {
                name: "kind",
                source: "r.kind",
                kind: ColumnKind::Text,
            },
            Column {
                name: "remaining_minutes",
                source: "r.remaining_minutes",
                kind: ColumnKind::Integer,
            },
            Column {
                name: "expiry_date",
                source: "r.expiry_date",
                kind: ColumnKind::Timestamp,
            },
            Column {
                name: "source_plan_id",
                source: "r.source_plan_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "created_at",
                source: "r.created_at",
                kind: ColumnKind::Timestamp,
            },
        ],
    },
    Table {
        name: "shifts",
        source: "shifts r",
        predicate: "1=1",
        parent: None,
        time: None,
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "user_id",
                source: "r.user_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "location_id",
                source: "r.location_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "clock_in",
                source: "r.clock_in",
                kind: ColumnKind::Timestamp,
            },
            Column {
                name: "clock_out",
                source: "r.clock_out",
                kind: ColumnKind::Timestamp,
            },
            Column {
                name: "status",
                source: "r.status",
                kind: ColumnKind::Text,
            },
        ],
    },
    Table {
        name: "credit_settlements",
        source: "credit_settlements r",
        predicate: "1=1",
        parent: None,
        time: Some("settled_at"),
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "player_id",
                source: "r.player_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "shift_id",
                source: "r.shift_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "location_id",
                source: "(SELECT s.location_id FROM shifts s WHERE s.id=r.shift_id)",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "amount",
                source: "r.amount",
                kind: ColumnKind::Money,
            },
            Column {
                name: "payment_method",
                source: "r.payment_method",
                kind: ColumnKind::Text,
            },
            Column {
                name: "cash_amount",
                source: "r.cash_amount",
                kind: ColumnKind::Money,
            },
            Column {
                name: "online_amount",
                source: "r.online_amount",
                kind: ColumnKind::Money,
            },
            Column {
                name: "settled_at",
                source: "r.settled_at",
                kind: ColumnKind::Timestamp,
            },
        ],
    },
    Table {
        name: "credit_settlement_items",
        source: "credit_settlement_items r",
        predicate: "1=1",
        parent: Some("settlement_id"),
        time: None,
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "settlement_id",
                source: "r.settlement_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "transaction_id",
                source: "r.transaction_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "amount_applied",
                source: "r.amount_applied",
                kind: ColumnKind::Money,
            },
        ],
    },
    Table {
        name: "expenses",
        source: "expenses r",
        predicate: "r.deleted_at IS NULL",
        parent: None,
        time: Some("expense_date"),
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "category_id",
                source: "r.category_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "shift_id",
                source: "r.shift_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "location_id",
                source: "r.location_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "amount",
                source: "r.amount",
                kind: ColumnKind::Money,
            },
            Column {
                name: "approval_status",
                source: "r.approval_status",
                kind: ColumnKind::Text,
            },
            Column {
                name: "expense_date",
                source: "r.expense_date",
                kind: ColumnKind::Timestamp,
            },
            Column {
                name: "local_date",
                source: "NULL",
                kind: ColumnKind::Date,
            },
        ],
    },
    Table {
        name: "cash_registers",
        source: "cash_registers r",
        predicate: "1=1",
        parent: None,
        time: Some("created_at"),
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "shift_id",
                source: "r.shift_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "location_id",
                source: "(SELECT s.location_id FROM shifts s WHERE s.id=r.shift_id)",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "status",
                source: "r.status",
                kind: ColumnKind::Text,
            },
            Column {
                name: "variance",
                source: "r.variance",
                kind: ColumnKind::Money,
            },
            Column {
                name: "closing_balance",
                source: "r.closing_balance",
                kind: ColumnKind::Money,
            },
            Column {
                name: "expected_closing",
                source: "r.expected_closing",
                kind: ColumnKind::Money,
            },
            Column {
                name: "created_at",
                source: "r.created_at",
                kind: ColumnKind::Timestamp,
            },
            Column {
                name: "updated_at",
                source: "r.updated_at",
                kind: ColumnKind::Timestamp,
            },
        ],
    },
    Table {
        name: "cash_deposits",
        source: "cash_deposits r",
        predicate: "1=1",
        parent: None,
        time: Some("created_at"),
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "shift_id",
                source: "r.shift_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "location_id",
                source: "(SELECT s.location_id FROM shifts s WHERE s.id=r.shift_id)",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "status",
                source: "r.status",
                kind: ColumnKind::Text,
            },
            Column {
                name: "amount",
                source: "r.amount",
                kind: ColumnKind::Money,
            },
            Column {
                name: "deposit_type",
                source: "r.deposit_type",
                kind: ColumnKind::Text,
            },
            Column {
                name: "created_at",
                source: "r.created_at",
                kind: ColumnKind::Timestamp,
            },
        ],
    },
    Table {
        name: "location_stock",
        source: "location_stock r",
        predicate: "1=1",
        parent: None,
        time: None,
        columns: &[
            Column {
                name: "id",
                source: "r.inventory_location_id || ':' || r.product_id",
                kind: ColumnKind::StockKey,
            },
            Column {
                name: "location_id",
                source: "r.inventory_location_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "product_id",
                source: "r.product_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "quantity_pieces",
                source: "r.quantity_pieces",
                kind: ColumnKind::Integer,
            },
        ],
    },
    Table {
        name: "reorder_rules",
        source: "inventory_reorder_rules r",
        predicate: "1=1",
        parent: None,
        time: None,
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "location_id",
                source: "r.inventory_location_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "product_id",
                source: "r.product_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "is_active",
                source: "r.is_active",
                kind: ColumnKind::Boolean,
            },
            Column {
                name: "minimum_pieces",
                source: "r.minimum_pieces",
                kind: ColumnKind::Integer,
            },
        ],
    },
    Table {
        name: "purchase_orders",
        source: "purchase_orders r",
        predicate: "1=1",
        parent: None,
        time: None,
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "status",
                source: "r.status",
                kind: ColumnKind::Text,
            },
            Column {
                name: "destination_location_id",
                source: "r.destination_location_id",
                kind: ColumnKind::Uuid,
            },
        ],
    },
    Table {
        name: "stock_transfer_requests",
        source: "stock_transfer_requests r",
        predicate: "1=1",
        parent: None,
        time: None,
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "status",
                source: "r.status",
                kind: ColumnKind::Text,
            },
            Column {
                name: "from_location_id",
                source: "r.from_location_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "to_location_id",
                source: "r.to_location_id",
                kind: ColumnKind::Uuid,
            },
        ],
    },
    Table {
        name: "stock_receipts",
        source: "stock_receipts r",
        predicate: "1=1",
        parent: None,
        time: Some("received_at"),
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "vendor_id",
                source: "r.vendor_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "location_id",
                source: "r.inventory_location_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "received_at",
                source: "r.created_at",
                kind: ColumnKind::Timestamp,
            },
        ],
    },
    Table {
        name: "stock_receipt_lines",
        source: "stock_receipt_lines r",
        predicate: "1=1",
        parent: Some("receipt_id"),
        time: None,
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "receipt_id",
                source: "r.receipt_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "product_id",
                source: "r.product_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "box_quantity",
                source: "r.box_quantity",
                kind: ColumnKind::Integer,
            },
            Column {
                name: "pieces_added",
                source: "r.pieces_added",
                kind: ColumnKind::Integer,
            },
        ],
    },
    Table {
        name: "stock_waste_events",
        source: "stock_waste_events r",
        predicate: "1=1",
        parent: None,
        time: None,
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "location_id",
                source: "r.inventory_location_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "status",
                source: "r.status",
                kind: ColumnKind::Text,
            },
            Column {
                name: "approved_at",
                source: "r.approved_at",
                kind: ColumnKind::Timestamp,
            },
        ],
    },
    Table {
        name: "stock_waste_lines",
        source: "stock_waste_lines r",
        predicate: "1=1",
        parent: Some("waste_event_id"),
        time: None,
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "waste_event_id",
                source: "r.waste_event_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "product_id",
                source: "r.product_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "reason_code",
                source: "r.reason_code",
                kind: ColumnKind::Text,
            },
            Column {
                name: "quantity_pieces",
                source: "r.quantity_pieces",
                kind: ColumnKind::Integer,
            },
        ],
    },
    Table {
        name: "stock_movements",
        source: "stock_movements r",
        predicate: "1=1",
        parent: None,
        time: Some("created_at"),
        columns: &[
            Column {
                name: "id",
                source: "r.id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "location_id",
                source: "r.inventory_location_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "product_id",
                source: "r.product_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "delta",
                source: "r.delta",
                kind: ColumnKind::Integer,
            },
            Column {
                name: "movement_type",
                source: "r.movement_type",
                kind: ColumnKind::Text,
            },
            Column {
                name: "reference_id",
                source: "r.reference_id",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "reference_type",
                source: "r.reference_type",
                kind: ColumnKind::Text,
            },
            Column {
                name: "created_by",
                source: "r.created_by",
                kind: ColumnKind::Uuid,
            },
            Column {
                name: "created_at",
                source: "r.created_at",
                kind: ColumnKind::Timestamp,
            },
        ],
    },
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalyticsSnapshot {
    pub version: u32,
    pub changes: Vec<Change>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub table: String,
    pub key_column: String,
    pub key: String,
    pub rows: Vec<Value>,
}
pub fn table(name: &str) -> Result<&'static Table, AppError> {
    TABLES
        .iter()
        .find(|t| t.name == name)
        .ok_or_else(|| AppError::Internal("Unknown analytical table".into()))
}
impl Table {
    pub fn select(&self) -> String {
        let fields = self
            .columns
            .iter()
            .flat_map(|c| [format!("'{}'", c.name), c.source.to_owned()])
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "SELECT json_object({fields}) FROM {} WHERE {}",
            self.source, self.predicate
        )
    }
}
/// Stable derived identity for the operational stock table's composite key.
pub fn stock_id(location: Uuid, product: Uuid) -> Uuid {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(format!("arena360:location_stock:{location}:{product}"));
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
/// Money travels as exact decimal text, never through a JSON floating-point DTO.
pub fn normalize(table: &Table, mut row: Value) -> Result<Value, AppError> {
    for c in table.columns {
        let value = &mut row[c.name];
        if value.is_null() {
            continue;
        }
        match c.kind {
            ColumnKind::Money => {
                *value =
                    Value::String(
                        crate::tenancy::scale4_to_decimal(value.as_i64().ok_or_else(|| {
                            AppError::Internal("Invalid scale-4 snapshot".into())
                        })?)
                        .to_string(),
                    )
            }
            ColumnKind::Boolean => {
                *value = Value::Bool(match value.as_i64() {
                    Some(0) => false,
                    Some(1) => true,
                    _ => return Err(AppError::Internal("Invalid snapshot boolean".into())),
                })
            }
            ColumnKind::StockKey => {
                let (a, b) = value
                    .as_str()
                    .and_then(|s| s.split_once(':'))
                    .ok_or_else(|| AppError::Internal("Invalid stock key".into()))?;
                let parse =
                    |s: &str| Uuid::parse_str(s).map_err(|e| AppError::Internal(e.to_string()));
                *value = Value::String(stock_id(parse(a)?, parse(b)?).to_string());
            }
            _ => {}
        }
    }
    crate::time::validate_utc_timestamps(&row).map_err(AppError::Internal)?;
    Ok(row)
}
async fn rows(
    connection: &mut SqliteConnection,
    spec: &Table,
    predicate: &str,
    key: &str,
) -> Result<Vec<Value>, AppError> {
    let raw: Vec<String> = sqlx::query_scalar(&format!(
        "{} AND ({predicate}) ORDER BY r.id",
        spec.select()
    ))
    .bind(key)
    .fetch_all(connection)
    .await?;
    raw.into_iter()
        .map(|r| {
            normalize(
                spec,
                serde_json::from_str(&r).map_err(|e| AppError::Internal(e.to_string()))?,
            )
        })
        .collect()
}
fn aggregate_table(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "user" => "users",
        "venue_location" => "venue_locations",
        "device" => "devices",
        "plan" => "plans",
        "product" => "products",
        "vendor" => "vendors",
        "inventory_location" => "inventory_locations",
        "expense_category" => "expense_categories",
        "transaction" => "transactions",
        "session" => "sessions",
        "balance" => "wallets",
        "shift" => "shifts",
        "credit_settlement" => "credit_settlements",
        "expense" => "expenses",
        "cash_register" => "cash_registers",
        "cash_deposit" => "cash_deposits",
        "inventory_reorder_rule" => "reorder_rules",
        "purchase_order" => "purchase_orders",
        "stock_transfer" => "stock_transfer_requests",
        "stock_receipt" => "stock_receipts",
        "stock_waste" => "stock_waste_events",
        "stock_movement" => "stock_movements",
        _ => return None,
    })
}
/// Capture inside the business transaction. Public/realtime payloads stay unchanged.
/// None means an older/synthetic event lacks a full snapshot and requires a rebuild.
pub async fn capture(
    connection: &mut SqliteConnection,
    kind: &str,
    id: Uuid,
    deleted: bool,
) -> Result<Option<AnalyticsSnapshot>, AppError> {
    let mut snapshot = AnalyticsSnapshot {
        version: 1,
        changes: Vec::new(),
    };
    let Some(name) = aggregate_table(kind) else {
        return Ok(Some(snapshot));
    };
    let spec = table(name)?;
    let key = id.to_string();
    let root = if deleted {
        Vec::new()
    } else {
        rows(connection, spec, "r.id=?", &key).await?
    };
    if !deleted && root.is_empty() {
        return Ok(None);
    }
    snapshot.changes.push(Change {
        table: name.into(),
        key_column: "id".into(),
        key: key.clone(),
        rows: root,
    });
    let child = match kind {
        "transaction" => Some(("transaction_lines", "transaction_id")),
        "credit_settlement" => Some(("credit_settlement_items", "settlement_id")),
        "stock_receipt" => Some(("stock_receipt_lines", "receipt_id")),
        "stock_waste" => Some(("stock_waste_lines", "waste_event_id")),
        _ => None,
    };
    if let Some((name, column)) = child {
        let child_rows = if deleted {
            Vec::new()
        } else {
            rows(connection, table(name)?, &format!("r.{column}=?"), &key).await?
        };
        snapshot.changes.push(Change {
            table: name.into(),
            key_column: column.into(),
            key: key.clone(),
            rows: child_rows,
        });
    }
    if kind == "stock_movement" && !deleted {
        let spec = table("location_stock")?;
        let raw:Vec<String>=sqlx::query_scalar(&format!("{} AND (r.inventory_location_id,r.product_id) IN (SELECT inventory_location_id,product_id FROM stock_movements WHERE id=?)",spec.select())).bind(&key).fetch_all(&mut *connection).await?;
        for raw in raw {
            let row = normalize(
                spec,
                serde_json::from_str(&raw).map_err(|e| AppError::Internal(e.to_string()))?,
            )?;
            snapshot.changes.push(Change {
                table: spec.name.into(),
                key_column: "id".into(),
                key: row["id"].as_str().unwrap().into(),
                rows: vec![row],
            });
        }
    }
    Ok(Some(snapshot))
}
