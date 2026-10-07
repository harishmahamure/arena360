use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

const NOW: &str = "2026-10-07T12:00:00.000000Z";
const LOCATION: &str = "0199c041-0000-7000-8000-000000000001";
const PLAYER: &str = "0199c041-0000-7000-8000-000000000002";
const STAFF: &str = "0199c041-0000-7000-8000-000000000003";
const DEVICE: &str = "0199c041-0000-7000-8000-000000000004";
const PLAN: &str = "0199c041-0000-7000-8000-000000000005";
const PRODUCT: &str = "0199c041-0000-7000-8000-000000000006";
const INGREDIENT: &str = "0199c041-0000-7000-8000-000000000007";
const INVENTORY_LOCATION: &str = "0199c041-0000-7000-8000-000000000008";
const BALANCE: &str = "0199c041-0000-7000-8000-000000000009";
const SHIFT: &str = "0199c041-0000-7000-8000-00000000000a";
const SESSION: &str = "0199c041-0000-7000-8000-00000000000b";
const TRANSACTION: &str = "0199c041-0000-7000-8000-00000000000c";

#[tokio::test]
async fn core_schema_is_strict_tenant_local_and_excludes_staff_credentials() {
    let pool = migrated_pool().await;
    let expected = [
        "venue_locations",
        "users",
        "access_assignments",
        "location_role_assignments",
        "devices",
        "plans",
        "plan_locations",
        "products",
        "product_locations",
        "games",
        "pricing_rule_sets",
        "pricing_rule_set_locations",
        "pricing_rule_versions",
        "setting_revisions",
        "product_recipe_items",
        "product_option_groups",
        "product_options",
        "product_option_ingredients",
        "inventory_locations",
        "location_stock",
        "stock_movements",
        "shifts",
        "player_plans",
        "player_plan_balances",
        "transactions",
        "usage_sessions",
        "player_plan_ledger",
        "transaction_products",
        "transaction_product_options",
        "credit_settlements",
        "credit_settlement_items",
        "kiosk_orders",
        "kiosk_order_items",
    ];

    for table in expected {
        let sql: String =
            sqlx::query_scalar("SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = $1")
                .bind(table)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(sql.ends_with("STRICT"), "{table} must be STRICT");

        let pragma = format!("PRAGMA table_info(\"{table}\")");
        for column in sqlx::query(&pragma).fetch_all(&pool).await.unwrap() {
            let name: String = column.get("name");
            let normalized = name.to_ascii_lowercase();
            assert_ne!(normalized, "tenant_id");
            assert_ne!(normalized, "organizationid");
            assert!(
                !normalized.contains("otp")
                    && !normalized.contains("totp")
                    && !normalized.contains("refresh_token")
                    && !normalized.contains("challenge"),
                "credential field {table}.{name} must not be tenant-local"
            );
        }

        let foreign_keys = format!("PRAGMA foreign_key_list(\"{table}\")");
        for foreign_key in sqlx::query(&foreign_keys).fetch_all(&pool).await.unwrap() {
            let parent: String = foreign_key.get("table");
            let from: String = foreign_key.get("from");
            let to: String = foreign_key.get("to");
            assert!(!from.is_empty() && !to.is_empty());
            let parent_exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(
                   SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = $1
                 )",
            )
            .bind(&parent)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert!(
                parent_exists,
                "{table}.{from} references missing {parent}.{to}"
            );
        }

        let indexes = format!("PRAGMA index_list(\"{table}\")");
        for index in sqlx::query(&indexes).fetch_all(&pool).await.unwrap() {
            let origin: String = index.get("origin");
            if origin != "c" {
                continue;
            }
            let name: String = index.get("name");
            let index_info = format!("PRAGMA index_xinfo(\"{}\")", name.replace('"', "\"\""));
            assert!(
                !sqlx::query(&index_info)
                    .fetch_all(&pool)
                    .await
                    .unwrap()
                    .is_empty(),
                "{name} has no indexed expressions"
            );
        }
    }

    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn core_constraints_reject_invalid_identity_credentials_values_and_scopes() {
    let pool = migrated_pool().await;

    rejected(
        &pool,
        &format!(
            "INSERT INTO users (id, username, role, created_at, updated_at)
             VALUES ('{PLAYER}', 'missing-password', 'player', '{NOW}', '{NOW}')"
        ),
    )
    .await;
    rejected(
        &pool,
        &format!(
            "INSERT INTO users
               (id, username, password_hash, role, created_at, updated_at)
             VALUES ('{STAFF}', 'staff-with-password', 'hash', 'staff', '{NOW}', '{NOW}')"
        ),
    )
    .await;
    rejected(
        &pool,
        &format!(
            "INSERT INTO venue_locations (id, slug, name, created_at, updated_at)
             VALUES ('not-a-uuid', 'bad-id', 'Bad ID', '{NOW}', '{NOW}')"
        ),
    )
    .await;
    rejected(
        &pool,
        "INSERT INTO venue_locations (id, slug, name, created_at, updated_at)
         VALUES (
           '0199c041-0000-7000-8000-000000000099', 'bad-time', 'Bad time',
           'xxxxxxxxxxxxxxxxxxxxxxxxxxZ', '2026-10-07T12:00:00.000000Z'
         )",
    )
    .await;

    seed_core(&pool).await;
    rejected(
        &pool,
        &format!(
            "INSERT INTO stock_movements
               (id, inventory_location_id, product_id, delta, movement_type, created_at)
             VALUES (
               '0199c041000070008000000000000090', '{INVENTORY_LOCATION}', '{PRODUCT}',
               1, 'receipt', '{NOW}'
             )"
        ),
    )
    .await;
    rejected(
        &pool,
        &format!(
            "INSERT INTO products
               (id, name, day_price, night_price, created_at, updated_at)
             VALUES (
               '0199c041-0000-7000-8000-000000000090', 'Bad price', -1, 0, '{NOW}', '{NOW}'
             )"
        ),
    )
    .await;
    rejected(
        &pool,
        &format!(
            "INSERT INTO pricing_rule_sets (id, name, created_at, updated_at)
             VALUES ('0199c041-0000-7000-8000-000000000091', 'Rules', '{NOW}', '{NOW}');
             INSERT INTO pricing_rule_versions
               (id, rule_set_id, version, policy, created_at)
             VALUES (
               '0199c041-0000-7000-8000-000000000092',
               '0199c041-0000-7000-8000-000000000091', 1, '[]', '{NOW}'
             )"
        ),
    )
    .await;
    rejected(
        &pool,
        &format!(
            "INSERT INTO product_locations (product_id, location_id)
             VALUES ('{PRODUCT}', '0199c041-0000-7000-8000-000000000099')"
        ),
    )
    .await;
    rejected(
        &pool,
        &format!(
            "UPDATE location_stock SET quantity_pieces = -1
             WHERE inventory_location_id = '{INVENTORY_LOCATION}' AND product_id = '{PRODUCT}'"
        ),
    )
    .await;
}

#[tokio::test]
async fn wallet_session_payment_and_checkout_invariants_are_enforced() {
    let pool = migrated_pool().await;
    seed_core(&pool).await;

    rejected(
        &pool,
        &format!(
            "INSERT INTO player_plan_balances
               (id, player_id, kind, remaining_minutes, expiry_date, created_at, updated_at)
             VALUES (
               '0199c041-0000-7000-8000-000000000020', '{PLAYER}', 'time', 60,
               '2026-11-07T12:00:00.000000Z', '{NOW}', '{NOW}'
             )"
        ),
    )
    .await;

    insert_open_session(&pool, SESSION, DEVICE, BALANCE)
        .await
        .unwrap();
    rejected(
        &pool,
        &format!(
            "INSERT INTO usage_sessions
               (id, player_id, balance_id, device_id, location_id, start_time,
                wallet_minutes_at_start, created_at, updated_at)
             VALUES (
               '0199c041-0000-7000-8000-000000000021', '{PLAYER}', '{BALANCE}', '{DEVICE}',
               '{LOCATION}', '{NOW}', 120, '{NOW}', '{NOW}'
             )"
        ),
    )
    .await;

    rejected(
        &pool,
        &format!(
            "INSERT INTO transactions
               (id, player_id, shift_id, location_id, transaction_type, amount, paid_amount,
                cash_amount, online_amount, payment_method, payment_status, transaction_date,
                created_at, updated_at)
             VALUES (
               '0199c041-0000-7000-8000-000000000022', '{PLAYER}', '{SHIFT}', '{LOCATION}',
               'product_purchase', 100000, 100000, 50000, 0, 'cash', 'completed',
               '{NOW}', '{NOW}', '{NOW}'
             )"
        ),
    )
    .await;

    sqlx::query(&format!(
        "INSERT INTO transactions
           (id, player_id, shift_id, location_id, transaction_type, amount, paid_amount,
            cash_amount, online_amount, payment_method, payment_status, transaction_date,
            created_at, updated_at)
         VALUES (
           '{TRANSACTION}', '{PLAYER}', '{SHIFT}', '{LOCATION}', 'product_purchase',
           100000, 100000, 100000, 0, 'cash', 'completed', '{NOW}', '{NOW}', '{NOW}'
         )"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(&format!(
        "INSERT INTO transaction_products
           (id, transaction_id, product_id, product_name, product_sku, quantity, unit_price,
            created_at, updated_at)
         VALUES (
           '0199c041-0000-7000-8000-000000000023', '{TRANSACTION}', '{PRODUCT}',
           'Cola snapshot', 'COLA', 2, 50000, '{NOW}', '{NOW}'
         )"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(&format!(
        "INSERT INTO kiosk_orders
           (id, session_id, player_id, device_id, status, transaction_id, created_at, updated_at)
         VALUES (
           '0199c041-0000-7000-8000-000000000024', '{SESSION}', '{PLAYER}', '{DEVICE}',
           'fulfilled', '{TRANSACTION}', '{NOW}', '{NOW}'
         )"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(&format!(
        "INSERT INTO kiosk_order_items
           (id, order_id, product_id, product_name, product_sku, quantity, unit_price, created_at)
         VALUES (
           '0199c041-0000-7000-8000-000000000025',
           '0199c041-0000-7000-8000-000000000024', '{PRODUCT}',
           'Cola snapshot', 'COLA', 2, 50000, '{NOW}'
         )"
    ))
    .execute(&pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn recipe_credit_and_required_access_path_indexes_are_present() {
    let pool = migrated_pool().await;
    seed_core(&pool).await;

    sqlx::query(&format!(
        "INSERT INTO product_recipe_items (product_id, ingredient_id, quantity)
         VALUES ('{PRODUCT}', '{INGREDIENT}', 2)"
    ))
    .execute(&pool)
    .await
    .unwrap();
    rejected(
        &pool,
        &format!(
            "INSERT INTO product_recipe_items (product_id, ingredient_id, quantity)
             VALUES ('{PRODUCT}', '{PRODUCT}', 1)"
        ),
    )
    .await;

    sqlx::query(&format!(
        "INSERT INTO transactions
           (id, player_id, shift_id, location_id, transaction_type, amount, paid_amount,
            payment_method, payment_status, transaction_date, created_at, updated_at)
         VALUES (
           '{TRANSACTION}', '{PLAYER}', '{SHIFT}', '{LOCATION}', 'product_purchase',
           100000, 0, 'credit', 'credit', '{NOW}', '{NOW}', '{NOW}'
         )"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(&format!(
        "INSERT INTO credit_settlements
           (id, player_id, settled_by, shift_id, amount, payment_method, cash_amount,
            settled_at, created_at, updated_at)
         VALUES (
           '0199c041-0000-7000-8000-000000000030', '{PLAYER}', '{STAFF}', '{SHIFT}',
           50000, 'cash', 50000, '{NOW}', '{NOW}', '{NOW}'
         )"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(&format!(
        "INSERT INTO credit_settlement_items
           (id, settlement_id, transaction_id, amount_applied, created_at)
         VALUES (
           '0199c041-0000-7000-8000-000000000031',
           '0199c041-0000-7000-8000-000000000030', '{TRANSACTION}', 50000, '{NOW}'
         )"
    ))
    .execute(&pool)
    .await
    .unwrap();

    for index in [
        "player_plan_balances_active_scope_unique",
        "usage_sessions_one_open_device",
        "usage_sessions_one_open_balance",
        "usage_sessions_one_open_player",
        "transactions_location_status_time_live",
        "devices_serial_live_unique",
        "location_stock_product",
        "kiosk_orders_open_status",
    ] {
        let present: bool = sqlx::query_scalar(
            "SELECT EXISTS(
               SELECT 1 FROM sqlite_schema WHERE type = 'index' AND name = $1
             )",
        )
        .bind(index)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(present, "missing required index {index}");
    }
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

async fn rejected(pool: &SqlitePool, sql: &str) {
    assert!(
        sqlx::raw_sql(sql).execute(pool).await.is_err(),
        "statement unexpectedly succeeded: {sql}"
    );
}

async fn seed_core(pool: &SqlitePool) {
    for sql in [
        format!(
            "INSERT INTO venue_locations (id, slug, name, created_at, updated_at)
             VALUES ('{LOCATION}', 'main', 'Main', '{NOW}', '{NOW}')"
        ),
        format!(
            "INSERT INTO users
               (id, username, password_hash, role, created_at, updated_at)
             VALUES ('{PLAYER}', 'player', 'bcrypt-hash', 'player', '{NOW}', '{NOW}')"
        ),
        format!(
            "INSERT INTO users (id, username, role, created_at, updated_at)
             VALUES ('{STAFF}', 'staff', 'staff', '{NOW}', '{NOW}')"
        ),
        format!(
            "INSERT INTO devices (id, location_id, name, serial_number, created_at, updated_at)
             VALUES ('{DEVICE}', '{LOCATION}', 'Station 1', 'SERIAL-1', '{NOW}', '{NOW}')"
        ),
        format!(
            "INSERT INTO plans
               (id, name, price, plan_type, time_credits, created_at, updated_at)
             VALUES ('{PLAN}', 'Hour', 100000, 'time_based', 60, '{NOW}', '{NOW}')"
        ),
        format!(
            "INSERT INTO products
               (id, name, day_price, night_price, sku, created_at, updated_at)
             VALUES ('{PRODUCT}', 'Cola', 50000, 50000, 'COLA', '{NOW}', '{NOW}')"
        ),
        format!(
            "INSERT INTO products
               (id, name, day_price, night_price, sku, is_raw_material, created_at, updated_at)
             VALUES ('{INGREDIENT}', 'Syrup', 0, 0, 'SYRUP', 1, '{NOW}', '{NOW}')"
        ),
        format!(
            "INSERT INTO inventory_locations
               (id, venue_location_id, name, kind, created_at, updated_at)
             VALUES ('{INVENTORY_LOCATION}', '{LOCATION}', 'Store', 'store', '{NOW}', '{NOW}')"
        ),
        format!(
            "INSERT INTO location_stock
               (inventory_location_id, product_id, quantity_pieces, created_at, updated_at)
             VALUES ('{INVENTORY_LOCATION}', '{PRODUCT}', 10, '{NOW}', '{NOW}')"
        ),
        format!(
            "INSERT INTO player_plan_balances
               (id, player_id, kind, remaining_minutes, expiry_date, source_plan_id,
                created_at, updated_at)
             VALUES (
               '{BALANCE}', '{PLAYER}', 'time', 120, '2026-11-07T12:00:00.000000Z',
               '{PLAN}', '{NOW}', '{NOW}'
             )"
        ),
        format!(
            "INSERT INTO shifts
               (id, user_id, location_id, clock_in, created_at, updated_at)
             VALUES ('{SHIFT}', '{STAFF}', '{LOCATION}', '{NOW}', '{NOW}', '{NOW}')"
        ),
    ] {
        sqlx::query(&sql).execute(pool).await.unwrap();
    }
}

async fn insert_open_session(
    pool: &SqlitePool,
    id: &str,
    device_id: &str,
    balance_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO usage_sessions
           (id, player_id, balance_id, device_id, location_id, start_time,
            wallet_minutes_at_start, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, 120, $6, $6)",
    )
    .bind(id)
    .bind(PLAYER)
    .bind(balance_id)
    .bind(device_id)
    .bind(LOCATION)
    .bind(NOW)
    .execute(pool)
    .await?;
    Ok(())
}
