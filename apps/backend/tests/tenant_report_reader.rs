#![cfg(feature = "duckdb-analytics")]
mod support;
use chrono::{DateTime, NaiveDate, Utc};
use gaming_cafe_api::analytics::{
    registry::AnalyticsRegistry, report_reader::ReportReader, tenant_db::error,
};
use serde::Deserialize;
use serde_json::Value;
use support::TenantFixture;
use uuid::Uuid;

async fn ready(
    f: &TenantFixture,
) -> std::sync::Arc<gaming_cafe_api::analytics::tenant_db::TenantAnalytics> {
    let a = AnalyticsRegistry::default()
        .get(f.db.clone())
        .await
        .unwrap();
    a.write(|tx| {
        tx.execute_batch(
            "UPDATE _ingest_state SET status='READY',hot_window_start=DATE '2025-01-01'",
        )
        .map_err(error)?;
        Ok(())
    })
    .await
    .unwrap();
    a
}

#[tokio::test]
async fn bound_values_and_exact_decimals_decode_without_sql_interpolation() {
    let f = TenantFixture::new().await;
    let a = ready(&f).await;
    let reader = ReportReader::new(a.clone()).await.unwrap();
    #[derive(Deserialize)]
    struct Row {
        id: Uuid,
        text: String,
        money: Value,
        at: DateTime<Utc>,
        date: NaiveDate,
        count: u64,
        absent: Option<String>,
    }
    let id = Uuid::from_u128(42);
    let at: DateTime<Utc> = "2026-10-01T00:00:00.123456Z".parse().unwrap();
    let text = "'; UPDATE _ingest_state SET status='FAILED'; --";
    let row: Row = reader.query("SELECT CAST($1 AS UUID),$2,CAST('922337203685477.5807' AS DECIMAL(19,4)),$3,DATE '2026-10-01',CAST(18446744073709551615 AS UBIGINT),NULL")
        .bind(id).bind(text).bind(at).fetch_one().await.unwrap();
    assert_eq!(row.id, id);
    assert_eq!(row.text, text);
    assert_eq!(row.money.to_string(), "922337203685477.5807");
    assert_eq!(row.at, at);
    assert_eq!(row.date.to_string(), "2026-10-01");
    assert_eq!(row.count, u64::MAX);
    assert!(row.absent.is_none());
    reader.ensure_ready().await.unwrap();
    drop((reader, a));
    f.close().await;
}

#[tokio::test]
async fn readiness_calendar_retention_and_fencing_are_checked_before_cached_reads() {
    let f = TenantFixture::new().await;
    let a = ready(&f).await;
    let reader = ReportReader::new(a.clone()).await.unwrap();
    assert!(reader
        .check_window(
            "2024-12-31T23:59:59Z".parse().unwrap(),
            "2025-01-01T00:00:00Z".parse().unwrap()
        )
        .is_err());
    a.write(|tx| {
        tx.execute_batch("UPDATE _ingest_state SET hot_window_start=DATE '2025-02-01'")
            .map_err(error)?;
        Ok(())
    })
    .await
    .unwrap();
    assert!(reader.ensure_ready().await.is_err());
    assert!(reader
        .query::<(i64,)>("SELECT 1")
        .fetch_one()
        .await
        .is_err());
    let current = ReportReader::new(a.clone()).await.unwrap();
    assert_ne!(reader.cache_key(), current.cache_key());
    a.write(|tx| {
        tx.execute_batch("UPDATE _ingest_state SET status='LAGGING'")
            .map_err(error)?;
        Ok(())
    })
    .await
    .unwrap();
    assert!(current.ensure_ready().await.is_err());
    assert!(ReportReader::new(a.clone()).await.is_err());
    a.write(|tx| {
        tx.execute_batch("UPDATE _ingest_state SET status='READY'")
            .map_err(error)?;
        Ok(())
    })
    .await
    .unwrap();
    f.db.with_immediate_writer(|c| {
        Box::pin(async move {
            sqlx::query("UPDATE tenant_runtime SET timezone='Asia/Kolkata'")
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    assert!(current.ensure_ready().await.is_err());
    assert!(ReportReader::new(a.clone()).await.is_err());
    assert_eq!(
        a.read(|tx| tx
            .query_row("SELECT status FROM _ingest_state", [], |r| r
                .get::<_, String>(0))
            .map_err(error))
            .await
            .unwrap(),
        "REBUILDING"
    );
    f.revoke();
    assert!(current.ensure_ready().await.is_err());
    drop((reader, current, a));
    f.close().await;
}

#[tokio::test]
async fn restored_source_checkpoint_and_tenant_cache_identity_cannot_serve_old_facts() {
    let f = TenantFixture::new().await;
    let other = TenantFixture::new().await;
    let a = ready(&f).await;
    let b = ready(&other).await;
    let reader = ReportReader::new(a.clone()).await.unwrap();
    let foreign = ReportReader::new(b.clone()).await.unwrap();
    assert_ne!(reader.cache_key(), foreign.cache_key());
    a.write(|tx| {
        tx.execute_batch("UPDATE _ingest_state SET last_sequence=1")
            .map_err(error)?;
        Ok(())
    })
    .await
    .unwrap();
    assert!(
        ReportReader::new(a.clone()).await.is_err(),
        "DuckDB ahead of restored SQLite must not serve a report"
    );
    // Existing readers also re-read the current checkpoint before using a cache.
    assert!(reader.ensure_ready().await.is_err());
    drop((reader, foreign, a, b));
    f.close().await;
    other.close().await;
}

#[tokio::test]
async fn venue_predicates_hide_expired_facts_children_and_unassigned_venues() {
    let f = TenantFixture::new().await;
    let a = ready(&f).await;
    a.write(|tx| {
        tx.execute_batch("INSERT INTO transactions(id,occurred_at,local_date,location_id,transaction_type,payment_method,payment_status,amount,paid_amount) VALUES
          (UUID '00000000-0000-0000-0000-000000000001',TIMESTAMP '2026-10-01',DATE '2026-10-01',UUID '00000000-0000-0000-0000-00000000000a','product_purchase','cash','completed',10,10),
          (UUID '00000000-0000-0000-0000-000000000002',TIMESTAMP '2026-10-01',DATE '2026-10-01',UUID '00000000-0000-0000-0000-00000000000b','product_purchase','cash','completed',20,20),
          (UUID '00000000-0000-0000-0000-000000000003',TIMESTAMP '2026-10-01',DATE '2026-10-01',NULL,'product_purchase','cash','completed',30,30),
          (UUID '00000000-0000-0000-0000-000000000004',TIMESTAMP '2024-12-01',DATE '2024-12-01',UUID '00000000-0000-0000-0000-00000000000a','product_purchase','cash','completed',900,900);
          INSERT INTO transaction_lines(id,transaction_id,product_id,quantity,unit_price)
          SELECT id,id,UUID '00000000-0000-0000-0000-00000000000c',1,amount FROM transactions;").map_err(error)?;
        Ok(())
    }).await.unwrap();
    let all = ReportReader::new(a.clone()).await.unwrap();
    let selected = all.clone().scoped(Some(vec![Uuid::from_u128(10)]));
    let empty = all.clone().scoped(Some(vec![]));
    assert_ne!(all.cache_key(), selected.cache_key());
    assert_ne!(all.cache_key(), empty.cache_key());
    assert_eq!(
        selected.cache_key(),
        all.clone()
            .scoped(Some(vec![Uuid::from_u128(10), Uuid::from_u128(10)]))
            .cache_key()
    );
    assert_eq!(
        all.query::<(i64, f64)>("SELECT count(*),sum(amount) FROM report_transactions")
            .fetch_one()
            .await
            .unwrap(),
        (3, 60.0)
    );
    assert_eq!(
        selected
            .query::<(i64, f64)>("SELECT count(*),sum(amount) FROM report_transactions")
            .fetch_one()
            .await
            .unwrap(),
        (1, 10.0)
    );
    assert_eq!(selected.query::<(i64,)>("WITH counts AS (SELECT count(*) n FROM report_transaction_lines) SELECT n FROM counts").fetch_one().await.unwrap().0,1);
    assert_eq!(
        empty
            .query::<(i64,)>("SELECT count(*) FROM report_transaction_lines")
            .fetch_one()
            .await
            .unwrap()
            .0,
        0
    );
    drop((all, selected, empty, a));
    f.close().await;
}
