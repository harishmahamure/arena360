use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

const NOW: &str = "2026-10-07T12:00:00.000000Z";
const VENUE: &str = "0199c041-0000-7000-8000-000000000101";
const STORE: &str = "0199c041-0000-7000-8000-000000000102";
const PRODUCT: &str = "0199c041-0000-7000-8000-000000000103";
const VENDOR: &str = "0199c041-0000-7000-8000-000000000104";
const PO: &str = "0199c041-0000-7000-8000-000000000105";
const PO_LINE: &str = "0199c041-0000-7000-8000-000000000106";
const RECEIPT: &str = "0199c041-0000-7000-8000-000000000107";

#[tokio::test]
async fn back_office_schema_is_strict_local_and_referential() {
    let pool = migrated_pool().await;
    for table in [
        "vendors",
        "expense_categories",
        "purchase_order_number_counter",
        "purchase_orders",
        "purchase_order_lines",
        "inventory_reorder_rules",
        "stock_receipts",
        "stock_receipt_lines",
        "stock_transfer_requests",
        "stock_transfer_lines",
        "stock_waste_events",
        "stock_waste_lines",
        "stock_adjustments",
        "stock_adjustment_lines",
        "cash_registers",
        "cash_register_entries",
        "cash_deposits",
        "expenses",
        "configurations",
        "activity_log",
        "user_notifications",
        "kitchen_menu_settings",
        "kitchen_tickets",
        "kitchen_ticket_events",
    ] {
        let sql: String =
            sqlx::query_scalar("SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = $1")
                .bind(table)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(sql.ends_with("STRICT"), "{table} must be STRICT");

        for column in sqlx::query(&format!("PRAGMA table_info(\"{table}\")"))
            .fetch_all(&pool)
            .await
            .unwrap()
        {
            let name: String = column.get("name");
            assert_ne!(name, "tenant_id");
            assert_ne!(name, "organizationId");
            assert!(!name.contains("totp") && !name.contains("password"));
        }
    }
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn procurement_keeps_line_snapshots_and_rejects_invalid_receipts() {
    let pool = migrated_pool().await;
    for sql in [
        format!("INSERT INTO venue_locations(id, slug, name, created_at, updated_at) VALUES('{VENUE}', 'main', 'Main', '{NOW}', '{NOW}')"),
        format!("INSERT INTO inventory_locations(id, venue_location_id, name, kind, created_at, updated_at) VALUES('{STORE}', '{VENUE}', 'Store', 'store', '{NOW}', '{NOW}')"),
        format!("INSERT INTO products(id, name, day_price, night_price, created_at, updated_at) VALUES('{PRODUCT}', 'Tea', 10000, 10000, '{NOW}', '{NOW}')"),
        format!("INSERT INTO vendors(id, name, created_at, updated_at) VALUES('{VENDOR}', 'Supplier', '{NOW}', '{NOW}')"),
        format!("INSERT INTO purchase_orders(id, po_number, vendor_id, destination_location_id, created_at, updated_at) VALUES('{PO}', 'PO-1', '{VENDOR}', '{STORE}', '{NOW}', '{NOW}')"),
        format!("INSERT INTO purchase_order_lines(id, purchase_order_id, product_id, ordered_boxes, units_per_box_snapshot, box_cost_snapshot, line_subtotal, line_tax, line_total) VALUES('{PO_LINE}', '{PO}', '{PRODUCT}', 2, 12, 100000, 200000, 0, 200000)"),
        format!("INSERT INTO stock_receipts(id, inventory_location_id, vendor_id, purchase_order_id, invoice_reference, receipt_date, created_at) VALUES('{RECEIPT}', '{STORE}', '{VENDOR}', '{PO}', 'INV-1', '{NOW}', '{NOW}')"),
    ] {
        sqlx::query(&sql).execute(&pool).await.unwrap();
    }

    let bad_quantity = format!("INSERT INTO stock_receipt_lines(id, receipt_id, product_id, purchase_order_line_id, box_quantity, accepted_box_quantity, rejected_box_quantity, pieces_added) VALUES('0199c041-0000-7000-8000-000000000108', '{RECEIPT}', '{PRODUCT}', '{PO_LINE}', 2, 2, 1, 24)");
    assert!(sqlx::query(&bad_quantity).execute(&pool).await.is_err());

    let good_line = format!("INSERT INTO stock_receipt_lines(id, receipt_id, product_id, purchase_order_line_id, box_quantity, accepted_box_quantity, rejected_box_quantity, pieces_added, box_cost_snapshot) VALUES('0199c041-0000-7000-8000-000000000108', '{RECEIPT}', '{PRODUCT}', '{PO_LINE}', 2, 1, 1, 12, 100000)");
    sqlx::query(&good_line).execute(&pool).await.unwrap();

    let duplicate_invoice = format!("INSERT INTO stock_receipts(id, inventory_location_id, purchase_order_id, invoice_reference, receipt_date, created_at) VALUES('0199c041-0000-7000-8000-000000000109', '{STORE}', '{PO}', 'INV-1', '{NOW}', '{NOW}')");
    assert!(sqlx::query(&duplicate_invoice)
        .execute(&pool)
        .await
        .is_err());
    let orphan = format!("INSERT INTO stock_transfer_lines(id, transfer_request_id, product_id, quantity_pieces) VALUES('0199c041-0000-7000-8000-000000000110', '{PO}', '{PRODUCT}', 1)");
    assert!(sqlx::query(&orphan).execute(&pool).await.is_err());

    let bad_reorder = format!("INSERT INTO inventory_reorder_rules(id, inventory_location_id, product_id, minimum_pieces, target_pieces, created_at, updated_at) VALUES('0199c041-0000-7000-8000-000000000111', '{STORE}', '{PRODUCT}', 20, 10, '{NOW}', '{NOW}')");
    assert!(sqlx::query(&bad_reorder).execute(&pool).await.is_err());

    let bad_config = format!("INSERT INTO configurations(id, key, value, created_at, updated_at) VALUES('0199c041-0000-7000-8000-000000000112', 'business.name', '{{bad json', '{NOW}', '{NOW}')");
    assert!(sqlx::query(&bad_config).execute(&pool).await.is_err());

    let bad_kitchen = format!("INSERT INTO kitchen_menu_settings(product_id, station, prep_minutes, updated_at) VALUES('{PRODUCT}', 'Bar', 0, '{NOW}')");
    assert!(sqlx::query(&bad_kitchen).execute(&pool).await.is_err());
}

async fn migrated_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .foreign_keys(true),
        )
        .await
        .unwrap();
    gaming_cafe_api::tenancy::migrate(&pool).await.unwrap();
    pool
}
