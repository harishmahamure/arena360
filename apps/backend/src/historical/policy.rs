//! Historical fact families; dimensions and unfinished operational work stay live.
use super::raw::{identifier, literal};
use crate::error::AppError;
pub struct Table {
    pub name: &'static str,
    pub time: Option<&'static str>,
    pub closed: &'static str,
    pub parent: Option<(&'static str, &'static str)>,
}
macro_rules! root {
    ($name:literal,$time:literal,$closed:literal) => {
        Table {
            name: $name,
            time: Some($time),
            closed: $closed,
            parent: None,
        }
    };
}
macro_rules! child {
    ($name:literal,$parent:literal,$key:literal) => {
        Table {
            name: $name,
            time: None,
            closed: "1=1",
            parent: Some(($parent, $key)),
        }
    };
}
pub static TABLES: &[Table] = &[
    root!(
        "transactions",
        "transaction_date",
        "r.payment_status IN ('completed','failed','refunded') OR r.deleted_at IS NOT NULL"
    ),
    child!("transaction_products", "transactions", "transaction_id"),
    child!(
        "transaction_product_options",
        "transaction_products",
        "transaction_product_id"
    ),
    root!("usage_sessions", "start_time", "r.end_time IS NOT NULL"),
    root!("player_plan_ledger", "created_at", "1=1"),
    root!("shifts", "clock_in", "r.status='closed'"),
    root!("credit_settlements", "settled_at", "1=1"),
    child!(
        "credit_settlement_items",
        "credit_settlements",
        "settlement_id"
    ),
    root!(
        "kiosk_orders",
        "created_at",
        "r.status IN ('fulfilled','cancelled')"
    ),
    child!("kiosk_order_items", "kiosk_orders", "order_id"),
    root!(
        "purchase_orders",
        "created_at",
        "r.status IN ('received','rejected','cancelled')"
    ),
    child!(
        "purchase_order_lines",
        "purchase_orders",
        "purchase_order_id"
    ),
    root!("stock_receipts", "receipt_date", "1=1"),
    child!("stock_receipt_lines", "stock_receipts", "receipt_id"),
    root!(
        "stock_transfer_requests",
        "created_at",
        "r.status IN ('fulfilled','rejected')"
    ),
    child!(
        "stock_transfer_lines",
        "stock_transfer_requests",
        "transfer_request_id"
    ),
    root!("stock_waste_events", "approved_at", "r.status='approved'"),
    child!("stock_waste_lines", "stock_waste_events", "waste_event_id"),
    root!("stock_adjustments", "created_at", "1=1"),
    child!(
        "stock_adjustment_lines",
        "stock_adjustments",
        "adjustment_id"
    ),
    root!("stock_movements", "created_at", "1=1"),
    root!(
        "cash_registers",
        "created_at",
        "r.status IN ('closed','reconciled')"
    ),
    root!("cash_register_entries", "created_at", "1=1"),
    root!(
        "cash_deposits",
        "created_at",
        "r.status IN ('approved','rejected')"
    ),
    root!(
        "expenses",
        "expense_date",
        "r.approval_status IN ('approved','rejected')"
    ),
    root!(
        "kitchen_tickets",
        "created_at",
        "r.status IN ('served','cancelled')"
    ),
    child!("kitchen_ticket_events", "kitchen_tickets", "ticket_id"),
    root!("activity_log", "created_at", "1=1"),
    root!("user_notifications", "created_at", "r.read_at IS NOT NULL"),
];
pub fn table(name: &str) -> Result<&'static Table, AppError> {
    TABLES
        .iter()
        .find(|t| t.name == name)
        .ok_or_else(|| AppError::BadRequest("Unknown archive source table".into()))
}
pub fn predicate(spec: &Table, start: &str, end: &str, cutoff: &str) -> Result<String, AppError> {
    let mut filter = format!("({})", spec.closed);
    if let Some(time) = spec.time {
        filter.push_str(&format!(
            " AND r.{}>={} AND r.{}<{}",
            identifier(time)?,
            literal(start),
            identifier(time)?,
            literal(end)
        ));
    }
    let finish = match spec.name {
        "usage_sessions" => Some("end_time"),
        "shifts" => Some("clock_out"),
        _ => None,
    };
    if let Some(finish) = finish {
        filter.push_str(&format!(
            " AND r.{}<={}",
            identifier(finish)?,
            literal(cutoff)
        ));
    }
    if let Some((parent, key)) = spec.parent {
        let parent_filter = predicate(table(parent)?, start, end, cutoff)?;
        filter.push_str(&format!(
            " AND r.{} IN (SELECT r.id FROM {} r WHERE {parent_filter})",
            identifier(key)?,
            identifier(parent)?
        ));
    }
    Ok(filter)
}
#[derive(Clone)]
pub struct Reference {
    pub child: String,
    pub column: String,
    pub parent: String,
}
pub async fn references(pool: &sqlx::SqlitePool) -> Result<Vec<Reference>, AppError> {
    use sqlx::Row;
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%'",
    )
    .fetch_all(pool)
    .await?;
    let mut refs = vec![];
    for name in names {
        for row in sqlx::query(&format!("PRAGMA foreign_key_list({})", identifier(&name)?))
            .fetch_all(pool)
            .await?
        {
            let parent: String = row.get("table");
            let target: Option<String> = row.try_get("to").ok();
            if target.as_deref() == Some("id") && TABLES.iter().any(|t| t.name == parent) {
                refs.push(Reference {
                    child: name.clone(),
                    column: row.get("from"),
                    parent,
                });
            }
        }
    }
    Ok(refs)
}
pub fn deletion_order(refs: &[Reference]) -> Result<Vec<&'static Table>, AppError> {
    let mut remaining = TABLES.iter().collect::<Vec<_>>();
    let mut output = vec![];
    while !remaining.is_empty() {
        let position = remaining
            .iter()
            .position(|parent| {
                !refs
                    .iter()
                    .any(|r| r.parent == parent.name && remaining.iter().any(|c| c.name == r.child))
            })
            .ok_or_else(|| {
                AppError::Conflict("Archive fact dependencies contain a cycle".into())
            })?;
        output.push(remaining.remove(position));
    }
    Ok(output)
}
