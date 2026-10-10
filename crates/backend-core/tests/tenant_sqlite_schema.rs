use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Row, SqlitePool};

const EVENT_ID: &str = "0199b3f0-7c6d-7a01-8abc-1234567890ab";
const AGGREGATE_ID: &str = "0199b3f0-7c6d-7a02-8abc-1234567890ab";
const OCCURRED_AT: &str = "2026-10-06T05:06:00.000000Z";

#[tokio::test]
async fn tenant_baseline_has_isolated_strict_outbox_schema() {
    let pool = migrated_pool().await;
    let table_sql: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = 'outbox_events'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(table_sql.ends_with("STRICT"));
    assert!(table_sql.contains("sequence INTEGER PRIMARY KEY AUTOINCREMENT"));

    let columns = sqlx::query("PRAGMA table_info(outbox_events)")
        .fetch_all(&pool)
        .await
        .unwrap();
    let names: Vec<String> = columns.iter().map(|column| column.get("name")).collect();
    assert_eq!(
        names,
        [
            "sequence",
            "event_id",
            "location_id",
            "aggregate_type",
            "aggregate_id",
            "event_type",
            "occurred_at",
            "schema_version",
            "deleted",
            "payload",
            "analytics_snapshot",
        ]
    );
    assert!(!table_sql.to_ascii_lowercase().contains("organizationid"));
    assert!(!names.iter().any(|name| name == "tenant_id"));

    let business_tables: Vec<String> = sqlx::query_scalar(
        r#"SELECT name
           FROM sqlite_schema
           WHERE type = 'table'
             AND name NOT LIKE 'sqlite_%'
             AND name <> '_sqlx_migrations'"#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    for table in business_tables {
        let pragma = format!("PRAGMA table_info(\"{}\")", table.replace('"', "\"\""));
        for column in sqlx::query(&pragma).fetch_all(&pool).await.unwrap() {
            let name: String = column.get("name");
            assert_ne!(name.to_ascii_lowercase(), "organizationid");
            assert_ne!(name.to_ascii_lowercase(), "tenant_id");
        }
    }
}

#[tokio::test]
async fn outbox_constraints_enforce_canonical_storage_values() {
    let pool = migrated_pool().await;
    let first = insert_event(
        &pool,
        EVENT_ID,
        AGGREGATE_ID,
        OCCURRED_AT,
        1,
        0,
        r#"{"id":"snapshot"}"#,
    )
    .await
    .unwrap();
    assert_eq!(first, 1);
    sqlx::query("DELETE FROM outbox_events WHERE sequence = $1")
        .bind(first)
        .execute(&pool)
        .await
        .unwrap();

    let second = insert_event(
        &pool,
        "0199b3f0-7c6d-7a03-8abc-1234567890ab",
        AGGREGATE_ID,
        OCCURRED_AT,
        1,
        1,
        "{}",
    )
    .await
    .unwrap();
    assert_eq!(second, 2, "AUTOINCREMENT must never reuse a sequence");

    for invalid in [
        insert_event(
            &pool,
            "0199B3F0-7C6D-7A04-8ABC-1234567890AB",
            AGGREGATE_ID,
            OCCURRED_AT,
            1,
            0,
            "{}",
        )
        .await,
        insert_event(
            &pool,
            "0199b3f0-7c6d-7a05-8abc-1234567890ab",
            AGGREGATE_ID,
            "2026-10-06T10:36:00+05:30",
            1,
            0,
            "{}",
        )
        .await,
        insert_event(
            &pool,
            "0199b3f0-7c6d-7a06-8abc-1234567890ab",
            AGGREGATE_ID,
            OCCURRED_AT,
            0,
            0,
            "{}",
        )
        .await,
        insert_event(
            &pool,
            "0199b3f0-7c6d-7a07-8abc-1234567890ab",
            AGGREGATE_ID,
            OCCURRED_AT,
            1,
            2,
            "{}",
        )
        .await,
        insert_event(
            &pool,
            "0199b3f0-7c6d-7a08-8abc-1234567890ab",
            AGGREGATE_ID,
            OCCURRED_AT,
            1,
            0,
            "[]",
        )
        .await,
    ] {
        assert!(invalid.is_err());
    }
}

async fn migrated_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    gaming_cafe_api::tenancy::migrate(&pool).await.unwrap();
    pool
}

async fn insert_event(
    pool: &SqlitePool,
    event_id: &str,
    aggregate_id: &str,
    occurred_at: &str,
    schema_version: i64,
    deleted: i64,
    payload: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        r#"INSERT INTO outbox_events
             (event_id, aggregate_type, aggregate_id, event_type,
              occurred_at, schema_version, deleted, payload)
           VALUES ($1, 'session', $2, 'session.updated', $3, $4, $5, $6)
           RETURNING sequence"#,
    )
    .bind(event_id)
    .bind(aggregate_id)
    .bind(occurred_at)
    .bind(schema_version)
    .bind(deleted)
    .bind(payload)
    .fetch_one(pool)
    .await
}
