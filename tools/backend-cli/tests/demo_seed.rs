//! Exercise the production binary against an isolated control database and temporary tenant file.
use chrono::Utc;
use gaming_cafe_api::tenancy::tenant_path;
use serde_json::Value;
use sqlx::{
    postgres::PgPoolOptions,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::path::Path;
use uuid::Uuid;

async fn run(root: &Path, url: &str, cell: Uuid, owner: Uuid, slug: &str) -> Value {
    let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_demo_seed"))
        .args([
            "--date",
            &Utc::now().date_naive().to_string(),
            "--tenant-slug",
            slug,
        ])
        .env("CONTROL_DATABASE_URL", url)
        .env("DEMO_CONTROL_DATABASE_URL", url)
        .env("ARENA_CELL_ID", cell.to_string())
        .env("TENANT_DATA_DIR", root)
        .env("DEMO_OWNER_USER_ID", owner.to_string())
        .env("DEMO_PLAYER_PASSWORD", "Demo-test-password-1")
        .output()
        .await
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[tokio::test]
#[ignore = "requires an isolated control-plane database"]
async fn service_seed_reconciles_and_repeats_without_overwriting() {
    let url =
        std::env::var("CONTROL_TEST_DATABASE_URL").expect("CONTROL_TEST_DATABASE_URL required");
    let pool = PgPoolOptions::new()
        .max_connections(3)
        .connect(&url)
        .await
        .unwrap();
    gaming_cafe_api::control::migrate(&pool).await.unwrap();
    let suffix = Uuid::new_v4().simple().to_string();
    let slug = format!("arena360-demo-{}", &suffix[..16]);
    let root = std::env::temp_dir().join(format!("arena360-demo-{suffix}"));
    let cell: Uuid =
        sqlx::query_scalar("INSERT INTO cells(name,address) VALUES($1,$2) RETURNING id")
            .bind(format!("demo-{suffix}"))
            .bind(format!("http://demo-{suffix}.invalid"))
            .fetch_one(&pool)
            .await
            .unwrap();
    let hash = bcrypt::hash("Existing-owner-password", 4).unwrap();
    let owner: Uuid =
        sqlx::query_scalar("INSERT INTO users(username,password_hash) VALUES($1,$2) RETURNING id")
            .bind(format!("demo-owner-{suffix}"))
            .bind(&hash)
            .fetch_one(&pool)
            .await
            .unwrap();
    let seeded = run(&root, &url, cell, owner, &slug).await;
    assert_eq!(seeded["status"], "seeded");
    assert_eq!(seeded["ownerUserId"], owner.to_string());
    assert_eq!(seeded["playersCanLogin"], true);
    assert_eq!(seeded["counts"]["devices"], 12);
    assert_eq!(seeded["counts"]["transactions"], 430);
    assert_eq!(seeded["counts"]["usage_sessions"], 275);
    assert_eq!(seeded["counts"]["kiosk_orders"], 2);
    assert!(seeded["counts"]["outbox_events"].as_i64().unwrap() > 1000);
    let tenant = Uuid::parse_str(seeded["tenantId"].as_str().unwrap()).unwrap();
    let local = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(tenant_path(&root, tenant))
                .read_only(true),
        )
        .await
        .unwrap();
    let wallet_errors: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM player_plan_balances b WHERE remaining_minutes != (SELECT COALESCE(SUM(delta_minutes),0) FROM player_plan_ledger WHERE balance_id=b.id)").fetch_one(&local).await.unwrap();
    assert_eq!(wallet_errors, 0);
    let stock_errors: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM location_stock s WHERE quantity_pieces != (SELECT COALESCE(SUM(delta),0) FROM stock_movements WHERE inventory_location_id=s.inventory_location_id AND product_id=s.product_id)").fetch_one(&local).await.unwrap();
    assert_eq!(stock_errors, 0);
    let active: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM usage_sessions WHERE end_time IS NULL")
            .fetch_one(&local)
            .await
            .unwrap();
    assert_eq!(active, 5);
    let cash_errors: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cash_registers WHERE status='closed' AND (variance != 0 OR closing_balance - expected_closing != (SELECT COALESCE(SUM(amount),0) FROM cash_deposits WHERE cash_register_id=cash_registers.id AND status='approved'))").fetch_one(&local).await.unwrap();
    assert_eq!(cash_errors, 0);
    let events: Vec<(String, String)> =
        sqlx::query_as("SELECT event_type,payload FROM outbox_events")
            .fetch_all(&local)
            .await
            .unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|(kind, _)| kind == "tenant.demo_seeded")
            .count(),
        1
    );
    for (_, payload) in &events {
        assert!(
            !payload.contains("password_hash")
                && !payload.contains("passwordHash")
                && !payload.contains("$2")
                && !payload.contains("Demo-test-password-1")
        );
    }
    let state: String = sqlx::query_scalar("SELECT state FROM demo_seed_runs")
        .fetch_one(&local)
        .await
        .unwrap();
    assert_eq!(state, "COMPLETE");
    let mut repeated = run(&root, &url, cell, owner, &slug).await;
    assert_eq!(repeated["status"], "already-seeded");
    repeated["status"] = seeded["status"].clone();
    assert_eq!(repeated, seeded);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM outbox_events")
        .fetch_one(&local)
        .await
        .unwrap();
    assert_eq!(count, events.len() as i64);
    assert_eq!(count, seeded["counts"]["outbox_events"].as_i64().unwrap());
    let preserved: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id=$1")
        .bind(owner)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(preserved, hash);
    #[cfg(feature = "duckdb-analytics")]
    if let Ok(nats) = std::env::var("NATS_TEST_URL") {
        verify_analytics(&pool, &local, &root, tenant, cell, &nats).await;
    } else {
        eprintln!("Demo analytics parity skipped: disposable JetStream runtime unavailable.");
    }
    local.close().await;
    let staff = Uuid::parse_str(seeded["staffUserId"].as_str().unwrap()).unwrap();
    sqlx::query("DELETE FROM tenants WHERE id=$1")
        .bind(tenant)
        .execute(&pool)
        .await
        .unwrap();
    for user in [owner, staff] {
        sqlx::query("DELETE FROM users WHERE id=$1")
            .bind(user)
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("DELETE FROM cells WHERE id=$1")
        .bind(cell)
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[cfg(feature = "duckdb-analytics")]
async fn verify_analytics(
    pool: &sqlx::PgPool,
    source: &sqlx::SqlitePool,
    root: &Path,
    tenant: Uuid,
    cell: Uuid,
    nats: &str,
) {
    use async_nats::jetstream::stream::{Config, RetentionPolicy, StorageType};
    use gaming_cafe_api::{
        analytics::{
            publisher::TENANT_EVENT_STREAM,
            rebuild::rebuild,
            tenant_db::{error, TenantAnalytics},
        },
        control::{LeaseClient, LeaseConfig},
        tenancy::{
            analytics_snapshot::{ColumnKind, TABLES},
            scale4_to_decimal, TenantDbConfig, TenantDbManager,
        },
    };
    use std::sync::Arc;
    let leases = Arc::new(LeaseClient::new(pool.clone(), cell, LeaseConfig::default()).unwrap());
    leases.acquire_assigned(tenant).await.unwrap();
    let manager = TenantDbManager::new(
        TenantDbConfig {
            root: root.to_owned(),
            ..Default::default()
        },
        leases,
    )
    .unwrap();
    let db = manager.open(tenant).await.unwrap();
    let analytics = TenantAnalytics::open(db.clone()).await.unwrap();
    let context = async_nats::jetstream::new(async_nats::connect(nats).await.unwrap());
    context
        .create_stream(Config {
            name: TENANT_EVENT_STREAM.into(),
            subjects: vec!["arena.tenant.*.events.v1".into()],
            retention: RetentionPolicy::Limits,
            storage: StorageType::File,
            max_age: std::time::Duration::from_secs(7 * 86400),
            ..Default::default()
        })
        .await
        .unwrap();
    rebuild(db.clone(), analytics.clone(), &context)
        .await
        .unwrap();
    let mut projected = 0;
    for spec in TABLES {
        let expected: i64 =
            sqlx::query_scalar(&format!("SELECT count(*) FROM ({})", spec.select()))
                .fetch_one(source)
                .await
                .unwrap();
        let table = spec.name;
        let actual = analytics
            .read(move |tx| {
                tx.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| {
                    r.get::<_, i64>(0)
                })
                .map_err(error)
            })
            .await
            .unwrap();
        assert_eq!(actual, expected, "{table} row count");
        projected += actual;
        for column in spec.columns.iter().filter(|c| c.kind == ColumnKind::Money) {
            let expected: i64 = sqlx::query_scalar(&format!(
                "SELECT coalesce(sum({}),0) FROM {} WHERE {}",
                column.source, spec.source, spec.predicate
            ))
            .fetch_one(source)
            .await
            .unwrap();
            let name = column.name;
            let actual = analytics
                .read(move |tx| {
                    tx.query_row(
                        &format!("SELECT CAST(coalesce(sum({name}),0) AS VARCHAR) FROM {table}"),
                        [],
                        |r| r.get::<_, String>(0),
                    )
                    .map_err(error)
                })
                .await
                .unwrap();
            assert_eq!(
                actual.parse::<rust_decimal::Decimal>().unwrap(),
                scale4_to_decimal(expected),
                "{table}.{name} exact total"
            );
        }
    }
    let expected_seconds:i64=sqlx::query_scalar("SELECT coalesce(sum(unixepoch(end_time)-unixepoch(start_time)),0) FROM usage_sessions WHERE end_time IS NOT NULL AND deleted_at IS NULL").fetch_one(source).await.unwrap();
    let seconds = analytics
        .read(|tx| {
            tx.query_row(
                "SELECT coalesce(sum(occupied_seconds),0) FROM session_hours",
                [],
                |r| r.get::<_, i64>(0),
            )
            .map_err(error)
        })
        .await
        .unwrap();
    assert_eq!(seconds, expected_seconds);
    let watermark: i64 =
        sqlx::query_scalar("SELECT seq FROM sqlite_sequence WHERE name='outbox_events'")
            .fetch_one(source)
            .await
            .unwrap();
    let checkpoint = analytics
        .read(|tx| {
            tx.query_row(
                "SELECT CAST(last_sequence AS BIGINT),status FROM _ingest_state",
                [],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
            )
            .map_err(error)
        })
        .await
        .unwrap();
    assert_eq!(checkpoint, (watermark, "READY".into()));
    eprintln!("Demo DuckDB parity: {projected} rows across 27 projections, exact money totals and {seconds} occupied seconds match SQLite.");
    drop(analytics);
    db.close().await.unwrap();
}
