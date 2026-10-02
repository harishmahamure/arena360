use gaming_cafe_api::analytics::{
    query_as,
    worker::{project, Change},
    ClickHouse,
};
use serde_json::json;
use uuid::Uuid;

#[test]
fn projections_allowlist_fields_and_preserve_decimal_precision() {
    let id = Uuid::new_v4();
    let mut change = Change { schema_version: 1, source_table: "users".into(), row_id: id, version: 42,
        deleted: false, row_data: serde_json::from_str(&format!(r#"{{"id":"{id}","username":"test","role":"player","isActive":true,"creditLimit":999999999999999.1234,"password_hash":"secret","totpSecret":"secret"}}"#)).unwrap() };
    let row = project(&change).unwrap();
    assert!(row.get("password_hash").is_none());
    assert!(row.get("totpSecret").is_none());
    assert_eq!(row["creditLimit"].to_string(), "999999999999999.1234");
    assert_eq!(row["_version"], 42);
    change.deleted = true;
    assert_eq!(project(&change).unwrap()["_deleted"], 1);
    change.schema_version = 2;
    assert!(project(&change).is_err());
    change.schema_version = 1;
    change.row_id = Uuid::new_v4();
    assert!(project(&change).is_err());
    change.source_table = "untrusted; DROP TABLE users".into();
    assert!(project(&change).is_err());
}

#[tokio::test]
async fn unavailable_clickhouse_is_an_error_not_zero_or_postgres_fallback() {
    let ch = ClickHouse::new(
        "http://127.0.0.1:1".into(),
        "unused".into(),
        "unused".into(),
        "".into(),
    );
    let err = query_as::<(i64,)>("SELECT count() FROM users")
        .fetch_one(&ch)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("ANALYTICS_UNAVAILABLE"));
}

/// Use a disposable ClickHouse database; never run against a populated deployment.
#[tokio::test]
#[ignore = "requires ANALYTICS_TEST_CLICKHOUSE_URL and an empty ClickHouse database"]
async fn clickhouse_replay_deletion_precision_and_all_report_queries() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("error")
        .try_init();
    let url = std::env::var("ANALYTICS_TEST_CLICKHOUSE_URL").expect("isolated ClickHouse URL");
    let ch = ClickHouse::new(
        url,
        std::env::var("ANALYTICS_TEST_CLICKHOUSE_DATABASE")
            .unwrap_or_else(|_| "arena360_test".into()),
        std::env::var("ANALYTICS_TEST_CLICKHOUSE_USER").unwrap_or_else(|_| "default".into()),
        std::env::var("ANALYTICS_TEST_CLICKHOUSE_PASSWORD").unwrap_or_default(),
    );
    ch.initialize().await.unwrap();
    assert!(
        ch.ensure_ready().await.is_err(),
        "test requires an empty database"
    );
    ch.execute("INSERT INTO analytics_ready VALUES (1, now64(6))", &[])
        .await
        .unwrap();
    let id = Uuid::new_v4();
    let user = Uuid::new_v4();
    let original = json!({"id":id,"playerId":user,"planId":null,"amount":"0.30","paidAmount":"0.0000",
        "paymentMethod":"cash","paymentStatus":"completed","transactionType":"product_purchase",
        "cashAmount":null,"onlineAmount":null,"createdAt":"2026-10-01T12:00:00Z","updatedAt":"2026-10-01T12:00:00Z",
        "transactionDate":"2026-10-01T12:00:00Z","deletedAt":null});
    for version in [10, 10, 0, 9] {
        let mut change = Change {
            schema_version: 1,
            source_table: "transactions".into(),
            row_id: id,
            version,
            deleted: false,
            row_data: original.clone(),
        };
        if version < 10 {
            change.row_data["amount"] = "99.00".into();
        }
        let row = project(&change).unwrap();
        ch.execute(
            &format!("INSERT INTO transactions_versions FORMAT JSONEachRow\n{row}"),
            &[],
        )
        .await
        .unwrap();
    }
    let category = Uuid::new_v4();
    for (table, data) in [
        (
            "users",
            json!({"id":user,"username":"repeat-player","role":"player","isActive":true,"creditLimit":"100.0000","createdAt":"2026-10-01T12:00:00Z"}),
        ),
        (
            "expense_categories",
            json!({"id":category,"name":"Ingredients","isActive":true,"budgetAmount":"100.0000"}),
        ),
        (
            "expenses",
            json!({"id":Uuid::new_v4(),"categoryId":category,"amount":"0.1234","approvalStatus":"approved","expenseDate":"2026-10-01T12:00:00Z"}),
        ),
        (
            "cash_registers",
            json!({"id":Uuid::new_v4(),"shiftId":Uuid::new_v4(),"status":"closed","variance":"0.1234","createdAt":"2026-10-01T12:00:00Z","updatedAt":"2026-10-01T12:00:00Z"}),
        ),
        (
            "credit_settlements",
            json!({"id":Uuid::new_v4(),"playerId":user,"amount":"0.1234","paymentMethod":"cash","settledAt":"2026-10-01T12:00:00Z"}),
        ),
    ] {
        let row_id = Uuid::parse_str(data["id"].as_str().unwrap()).unwrap();
        let change = Change {
            schema_version: 1,
            source_table: table.into(),
            row_id,
            version: 10,
            deleted: false,
            row_data: data,
        };
        ch.execute(
            &format!(
                "INSERT INTO {table}_versions FORMAT JSONEachRow\n{}",
                project(&change).unwrap()
            ),
            &[],
        )
        .await
        .unwrap();
    }
    let start = "2026-10-01T00:00:00Z".parse().unwrap();
    let end = "2026-10-03T00:00:00Z".parse().unwrap();
    let report = ch.finance_report(start, end).await.unwrap();
    assert_eq!(report["sales"], "0.30");
    assert_eq!(report["saleCount"], 1);
    assert_eq!(report["approvedExpenses"], "0.1234");
    assert_eq!(report["creditCollections"], "0.1234");
    assert_eq!(report["daily"].as_array().unwrap().len(), 2);
    let cache = gaming_cafe_api::cache::create_cache(None).await;
    let stats = gaming_cafe_api::services::StatsService::new(ch.clone(), cache);
    let a = Some("2026-10-01".into());
    let b = Some("2026-10-02".into());
    let dashboard = stats
        .get_dashboard_stats(a.clone(), b.clone(), true)
        .await
        .unwrap();
    assert_eq!(dashboard.transactions.current.total_transactions, 1);
    stats
        .get_staff_dashboard_stats(a.clone(), b.clone(), Some("2026-10-01T00:00:00Z".into()))
        .await
        .unwrap();
    stats
        .get_finance_reconciliation_stats(a.clone(), b.clone(), true)
        .await
        .unwrap();
    stats
        .get_finance_deposit_stats(a.clone(), b.clone(), true)
        .await
        .unwrap();
    stats.get_finance_variance_stats(a, b, true).await.unwrap();
    ch.get_summary_by_category().await.unwrap();
    ch.receipt_summary(None, None, None).await.unwrap();
    ch.waste_summary(None, None, None).await.unwrap();
    ch.overview().await.unwrap();
    ch.get_portfolio_summary().await.unwrap();
    let mut changed = original.clone();
    changed["paymentStatus"] = "refunded".into();
    let event = Change {
        schema_version: 1,
        source_table: "transactions".into(),
        row_id: id,
        version: 11,
        deleted: false,
        row_data: changed,
    };
    ch.execute(
        &format!(
            "INSERT INTO transactions_versions FORMAT JSONEachRow\n{}",
            project(&event).unwrap()
        ),
        &[],
    )
    .await
    .unwrap();
    let report = ch.finance_report(start, end).await.unwrap();
    assert_eq!(report["saleCount"], 0);
    assert_eq!(report["refundedSales"], "0.30");
    let event = Change {
        version: 12,
        deleted: true,
        ..event
    };
    ch.execute(
        &format!(
            "INSERT INTO transactions_versions FORMAT JSONEachRow\n{}",
            project(&event).unwrap()
        ),
        &[],
    )
    .await
    .unwrap();
    // Late replay of a live row cannot resurrect a tombstone.
    let event = Change {
        version: 10,
        deleted: false,
        row_data: original,
        ..event
    };
    ch.execute(
        &format!(
            "INSERT INTO transactions_versions FORMAT JSONEachRow\n{}",
            project(&event).unwrap()
        ),
        &[],
    )
    .await
    .unwrap();
    let (count,): (i64,) = query_as("SELECT count() FROM transactions")
        .fetch_one(&ch)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

struct Worker(std::process::Child);
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
#[ignore = "requires isolated migrated PostgreSQL, empty ClickHouse database, and NATS JetStream"]
async fn committed_changes_flow_through_jetstream_and_survive_worker_restart() {
    use std::time::Duration;
    let pg_url =
        std::env::var("ANALYTICS_TEST_DATABASE_URL").expect("isolated migrated PostgreSQL");
    let ch_url = std::env::var("ANALYTICS_TEST_CLICKHOUSE_URL").expect("isolated ClickHouse");
    let db = std::env::var("ANALYTICS_TEST_CLICKHOUSE_DATABASE").expect("empty test database");
    let nats = std::env::var("ANALYTICS_TEST_NATS_URL").expect("isolated NATS JetStream");
    let ch_user =
        std::env::var("ANALYTICS_TEST_CLICKHOUSE_USER").unwrap_or_else(|_| "default".into());
    let ch_password = std::env::var("ANALYTICS_TEST_CLICKHOUSE_PASSWORD").unwrap_or_default();
    let pg = sqlx::postgres::PgPoolOptions::new()
        .max_connections(3)
        .connect(&pg_url)
        .await
        .unwrap();
    let ch = ClickHouse::new(
        ch_url.clone(),
        db.clone(),
        ch_user.clone(),
        ch_password.clone(),
    );
    ch.initialize().await.unwrap();
    assert!(
        ch.ensure_ready().await.is_err(),
        "test requires an empty ClickHouse database"
    );
    let id = Uuid::new_v4();
    // Rollback must remove its analytics event atomically.
    let mut tx = pg.begin().await.unwrap();
    sqlx::query("INSERT INTO users(id,username,password_hash,role) VALUES($1,$2,'not-for-analytics','player')")
        .bind(id).bind(format!("analytics-{id}")).execute(&mut *tx).await.unwrap();
    let (count,): (i64,) = sqlx::query_as("SELECT count(*) FROM analytics_outbox WHERE row_id=$1")
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(count, 1);
    tx.rollback().await.unwrap();
    let (count,): (i64,) = sqlx::query_as("SELECT count(*) FROM analytics_outbox WHERE row_id=$1")
        .bind(id)
        .fetch_one(&pg)
        .await
        .unwrap();
    assert_eq!(count, 0);
    sqlx::query("INSERT INTO users(id,username,password_hash,role) VALUES($1,$2,'not-for-analytics','player')")
        .bind(id).bind(format!("analytics-{id}")).execute(&pg).await.unwrap();
    let location = Uuid::new_v4();
    let product = Uuid::new_v4();
    sqlx::query("INSERT INTO inventory_locations(id,name,kind) VALUES($1,'Analytics QA','store')")
        .bind(location)
        .execute(&pg)
        .await
        .unwrap();
    sqlx::query(r#"INSERT INTO products(id,name,price,"dayPrice","nightPrice") VALUES($1,'Analytics QA',1,1,1)"#)
        .bind(product)
        .execute(&pg)
        .await
        .unwrap();
    sqlx::query(
        r#"INSERT INTO location_stock("locationId","productId","quantityPieces") VALUES($1,$2,7)"#,
    )
    .bind(location)
    .bind(product)
    .execute(&pg)
    .await
    .unwrap();
    let spawn = |backfill| {
        let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_analytics_worker"));
        cmd.env("ANALYTICS_DATABASE_URL", &pg_url)
            .env("NATS_URL", &nats)
            .env("CLICKHOUSE_URL", &ch_url)
            .env("CLICKHOUSE_DATABASE", &db)
            .env("CLICKHOUSE_USER", &ch_user)
            .env("CLICKHOUSE_PASSWORD", &ch_password)
            .env("RUST_LOG", "warn");
        if backfill {
            cmd.arg("--backfill");
        }
        Worker(cmd.spawn().unwrap())
    };
    let mut worker = spawn(true);
    tokio::time::timeout(Duration::from_secs(45), async {
        loop {
            assert!(
                worker.0.try_wait().unwrap().is_none(),
                "worker exited before backfill completed"
            );
            if ch.ensure_ready().await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
    let (count,): (i64,) = query_as("SELECT count() FROM users WHERE id=$1")
        .bind(id)
        .fetch_one(&ch)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let (pieces,): (i64,) = query_as(
        r#"SELECT "quantityPieces" FROM location_stock WHERE "locationId"=$1 AND "productId"=$2"#,
    )
    .bind(location)
    .bind(product)
    .fetch_one(&ch)
    .await
    .unwrap();
    assert_eq!(
        pieces, 7,
        "snapshot and live event for composite key must deduplicate"
    );
    // A late commit has a lower sequence than an already published transaction.
    let delayed = Uuid::new_v4();
    let fast = Uuid::new_v4();
    let mut delayed_tx = pg.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO users(id,username,password_hash,role) VALUES($1,$2,'unused','player')",
    )
    .bind(delayed)
    .bind(format!("delayed-{delayed}"))
    .execute(&mut *delayed_tx)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO users(id,username,password_hash,role) VALUES($1,$2,'unused','player')",
    )
    .bind(fast)
    .bind(format!("fast-{fast}"))
    .execute(&pg)
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let (count,): (i64,) = query_as("SELECT count() FROM users WHERE id=$1")
                .bind(fast)
                .fetch_one(&ch)
                .await
                .unwrap();
            if count == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
    delayed_tx.commit().await.unwrap();
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let (count,): (i64,) = query_as("SELECT count() FROM users WHERE id=$1")
                .bind(delayed)
                .fetch_one(&ch)
                .await
                .unwrap();
            if count == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
    // A second process must not become a second active consumer.
    let mut second = spawn(false);
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(status) = second.0.try_wait().unwrap() {
                assert!(!status.success());
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
    drop(worker);
    sqlx::query("UPDATE users SET username=$2 WHERE id=$1")
        .bind(id)
        .bind(format!("updated-{id}"))
        .execute(&pg)
        .await
        .unwrap();
    let mut worker = spawn(false);
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            assert!(
                worker.0.try_wait().unwrap().is_none(),
                "worker exited during recovery"
            );
            let (name,): (String,) = query_as("SELECT username FROM users WHERE id=$1")
                .bind(id)
                .fetch_one(&ch)
                .await
                .unwrap();
            if name == format!("updated-{id}") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
    sqlx::query("DELETE FROM users WHERE id=$1")
        .bind(id)
        .execute(&pg)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let (count,): (i64,) = query_as("SELECT count() FROM users WHERE id=$1")
                .bind(id)
                .fetch_one(&ch)
                .await
                .unwrap();
            if count == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
    drop(worker);
}
