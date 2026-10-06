use chrono::{TimeZone, Utc};
use gaming_cafe_api::tenancy::{format_sqlite_timestamp, write_outbox_event, NewOutboxEvent};
use serde_json::json;
use sqlx::sqlite::SqlitePoolOptions;
use uuid::Uuid;

#[tokio::test]
async fn business_write_and_outbox_event_commit_or_roll_back_together() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    gaming_cafe_api::tenancy::migrate(&pool).await.unwrap();
    sqlx::query("CREATE TABLE sample_orders (id TEXT PRIMARY KEY, total INTEGER NOT NULL) STRICT")
        .execute(&pool)
        .await
        .unwrap();

    let order_id = Uuid::new_v4();
    let completed_at = Utc
        .with_ymd_and_hms(2026, 10, 6, 15, 30, 0)
        .single()
        .unwrap();
    let event = NewOutboxEvent {
        location_id: Some(Uuid::new_v4()),
        aggregate_type: "order".into(),
        aggregate_id: order_id,
        event_type: "order.completed".into(),
        schema_version: 1,
        deleted: false,
        payload: json!({
            "id": order_id,
            "total": 125_000,
            "completedAt": format_sqlite_timestamp(&completed_at).unwrap()
        }),
    };

    let mut rolled_back = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO sample_orders(id, total) VALUES ($1, $2)")
        .bind(order_id.to_string())
        .bind(125_000_i64)
        .execute(&mut *rolled_back)
        .await
        .unwrap();
    write_outbox_event(&mut rolled_back, event.clone())
        .await
        .unwrap();
    rolled_back.rollback().await.unwrap();
    assert_eq!(row_count(&pool, "sample_orders").await, 0);
    assert_eq!(row_count(&pool, "outbox_events").await, 0);

    let mut committed = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO sample_orders(id, total) VALUES ($1, $2)")
        .bind(order_id.to_string())
        .bind(125_000_i64)
        .execute(&mut *committed)
        .await
        .unwrap();
    let written = write_outbox_event(&mut committed, event.clone())
        .await
        .unwrap();
    committed.commit().await.unwrap();

    assert_eq!(row_count(&pool, "sample_orders").await, 1);
    assert_eq!(row_count(&pool, "outbox_events").await, 1);
    assert_eq!(written.event_id.get_version_num(), 7);
    let row: (String, String, i64, bool, String) = sqlx::query_as(
        r#"SELECT event_id, aggregate_id, schema_version, deleted, payload
           FROM outbox_events WHERE sequence = $1"#,
    )
    .bind(written.sequence)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, written.event_id.to_string());
    assert_eq!(row.1, order_id.to_string());
    assert_eq!(row.2, 1);
    assert!(!row.3);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&row.4).unwrap(),
        event.payload
    );
}

async fn row_count(pool: &sqlx::SqlitePool, table: &str) -> i64 {
    let query = format!("SELECT COUNT(*) FROM {table}");
    sqlx::query_scalar(&query).fetch_one(pool).await.unwrap()
}
