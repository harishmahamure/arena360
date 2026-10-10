#![cfg(feature = "duckdb-analytics")]
mod support;
use gaming_cafe_api::{
    analytics::publisher::{publish_batch, JetStreamSink, TENANT_EVENT_STREAM},
    historical::{
        hot::{Consumer, Writer},
        objects::{self, Object},
    },
    metrics::Metrics,
    replication::{crypto::TenantKeys, ledger::PostgresLedger, wal},
    tenancy::{write_outbox_event_on_connection, NewOutboxEvent},
};
use object_store::ObjectStoreExt;
use serde_json::json;
use std::{sync::Arc, time::Duration};
use uuid::Uuid;
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires control database and disposable JetStream server"]
async fn monthly_hot_copy_backfills_verifies_and_acks_only_after_publication() {
    let jobs =
        gaming_cafe_api::background::BackgroundJobs::new(gaming_cafe_api::background::Limits {
            outbox_slots: 1,
            background_slots: 1,
            backfill_slots: 1,
        })
        .unwrap();
    let fixture = support::SessionFixture::new_with_background_jobs(jobs).await;
    let db = fixture.tenant.db.clone();
    let tenant = db.tenant_id();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect(&std::env::var("CONTROL_TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    gaming_cafe_api::control::migrate(&pool).await.unwrap();
    let cell = Uuid::new_v4();
    sqlx::query("INSERT INTO cells(id,name,address) VALUES($1,$2,$3)")
        .bind(cell)
        .bind(cell.to_string())
        .bind(format!("http://{cell}.invalid"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO tenants(id,slug,name,timezone,owner_cell,ownership_generation,state) VALUES($1,$2,'Hot','Asia/Kolkata',$3,1,'ACTIVE')").bind(tenant).bind(tenant.to_string()).bind(cell).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tenant_leases(tenant_id,owner_cell,ownership_generation,expires_at) VALUES($1,$2,1,clock_timestamp()+INTERVAL '30 minutes')").bind(tenant).bind(cell).execute(&pool).await.unwrap();
    let first = fixture
        .tenant
        .plan_transaction(fixture.player, fixture.plan, fixture.venue)
        .await;
    let second = fixture
        .tenant
        .plan_transaction(fixture.player, fixture.plan, fixture.venue)
        .await;
    db.with_immediate_writer(move|c|Box::pin(async move{
  sqlx::query("UPDATE tenant_runtime SET timezone='Asia/Kolkata'").execute(&mut *c).await?;
  for (id,at,amount) in [(first,"2026-09-30T18:20:00.000000Z",123456i64),(second,"2026-09-30T18:40:00.000000Z",789012i64)] {
   sqlx::query("UPDATE transactions SET transaction_date=?,created_at=?,updated_at=?,amount=?,paid_amount=?,cash_amount=? WHERE id=?").bind(at).bind(at).bind(at).bind(amount).bind(amount).bind(amount).bind(id.to_string()).execute(&mut *c).await?;
   write_outbox_event_on_connection(c,NewOutboxEvent{aggregate_type:"transaction".into(),aggregate_id:id,event_type:"transaction.updated".into(),location_id:None,schema_version:1,deleted:false,payload:json!({"id":id})}).await?;
  }Ok(())
 })).await.unwrap();
    let root = std::env::temp_dir().join(format!("arena-hot-{tenant}"));
    let keys = TenantKeys::new(root.join("keys"));
    wal::durable_create(&keys.path(tenant), &[77; 32]).unwrap();
    let store = Arc::new(object_store::memory::InMemory::new());
    let writer = Arc::new(Writer {
        metrics: Arc::new(Metrics::default()),
        ledger: Arc::new(PostgresLedger {
            pool: pool.clone(),
            cell_id: cell,
        }),
        store: store.clone(),
        keys: keys.clone(),
        staging_root: root.join("staging"),
    });
    let now = "2026-10-08T00:00:00Z".parse().unwrap();
    let boundary = writer.refresh(db.clone(), now).await.unwrap();
    assert!(boundary > 0);
    assert!(!writer.needs_refresh(&db, now).await.unwrap());
    let mut saved = vec![];
    for (period, id, expected) in [
        ("2026-09-01", first, "12.3456"),
        ("2026-10-01", second, "78.9012"),
    ] {
        let value:serde_json::Value=sqlx::query_scalar("SELECT objects FROM hot_month_manifests WHERE tenant_id=$1 AND period_start=$2::text::date").bind(tenant).bind(period).fetch_one(&pool).await.unwrap();
        let objects: Vec<Object> = serde_json::from_value(value).unwrap();
        let object = objects
            .iter()
            .find(|o| o.table == "transactions")
            .unwrap()
            .clone();
        let file = root.join(format!("{period}.parquet"));
        objects::download(store.as_ref(), &keys.read(tenant).unwrap(), &object, &file)
            .await
            .unwrap();
        let connection = duckdb::Connection::open_in_memory().unwrap();
        let sql = format!(
            "SELECT CAST(amount AS VARCHAR) FROM read_parquet('{}') WHERE id=CAST('{}' AS UUID)",
            file.display(),
            id
        );
        let amount: String = connection.query_row(&sql, [], |r| r.get(0)).unwrap();
        assert_eq!(amount, expected);
        assert!(!object.key.contains("replication"));
        saved.push(object);
    }
    // A retry reuses byte-for-byte verified objects rather than duplicating a month.
    writer.refresh(db.clone(), now).await.unwrap();
    let value: serde_json::Value = sqlx::query_scalar(
        "SELECT objects FROM hot_month_manifests WHERE tenant_id=$1 AND period_start='2026-09-01'",
    )
    .bind(tenant)
    .fetch_one(&pool)
    .await
    .unwrap();
    let repeated: Vec<Object> = serde_json::from_value(value).unwrap();
    assert_eq!(
        repeated
            .iter()
            .find(|o| o.table == "transactions")
            .unwrap()
            .key,
        saved[0].key
    );
    store
        .delete(&object_store::path::Path::from(saved[0].key.as_str()))
        .await
        .unwrap();
    writer.refresh(db.clone(), now).await.unwrap();
    let value: serde_json::Value = sqlx::query_scalar(
        "SELECT objects FROM hot_month_manifests WHERE tenant_id=$1 AND period_start='2026-09-01'",
    )
    .bind(tenant)
    .fetch_one(&pool)
    .await
    .unwrap();
    let repaired: Vec<Object> = serde_json::from_value(value).unwrap();
    let replacement = repaired
        .into_iter()
        .find(|o| o.table == "transactions")
        .unwrap();
    assert_ne!(replacement.key, saved[0].key);
    saved[0] = replacement;
    // Authentication detects a bad key and leaves no decoded output.
    let rejected = root.join("bad.parquet");
    assert!(
        objects::download(store.as_ref(), &[88; 32], &saved[0], &rejected)
            .await
            .is_err()
    );
    assert!(!rejected.exists());
    let url = std::env::var("NATS_TEST_URL").unwrap();
    let context = async_nats::jetstream::new(async_nats::connect(&url).await.unwrap());
    context
        .get_or_create_stream(async_nats::jetstream::stream::Config {
            name: TENANT_EVENT_STREAM.into(),
            subjects: vec!["arena.tenant.*.events.v1".into()],
            max_age: Duration::from_secs(7 * 86400),
            storage: async_nats::jetstream::stream::StorageType::File,
            ..Default::default()
        })
        .await
        .unwrap();
    let sink = JetStreamSink::connect(&url).await.unwrap();
    publish_batch(db.clone(), &sink, &Metrics::default())
        .await
        .unwrap();
    let mut consumer = Consumer::connect(&context, writer.clone(), db.clone())
        .await
        .unwrap();
    assert!(consumer.poll().await.unwrap() > 0);
    let stream = context.get_stream(TENANT_EVENT_STREAM).await.unwrap();
    let mut broker = stream
        .get_consumer::<async_nats::jetstream::consumer::pull::Config>(&format!(
            "hot_{}",
            tenant.simple()
        ))
        .await
        .unwrap();
    assert_eq!(broker.info().await.unwrap().num_ack_pending, 0);
    // A changed source with no key cannot advance its checkpoint or ACK the event.
    db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("UPDATE transactions SET amount=20000,paid_amount=20000,cash_amount=20000 WHERE id=?")
                .bind(first.to_string())
                .execute(&mut *c)
                .await?;
            write_outbox_event_on_connection(
                c,
                NewOutboxEvent {
                    aggregate_type: "transaction".into(),
                    aggregate_id: first,
                    event_type: "transaction.updated".into(),
                    location_id: None,
                    schema_version: 1,
                    deleted: false,
                    payload: json!({"id":first}),
                },
            )
            .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    publish_batch(db.clone(), &sink, &Metrics::default())
        .await
        .unwrap();
    std::fs::remove_file(keys.path(tenant)).unwrap();
    assert!(consumer.poll().await.is_err());
    assert!(broker.info().await.unwrap().num_ack_pending > 0);
    let last: i64 =
        sqlx::query_scalar("SELECT source_watermark FROM hot_tenant_state WHERE tenant_id=$1")
            .bind(tenant)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(last, boundary);
    wal::durable_create(&keys.path(tenant), &[77; 32]).unwrap();
    writer
        .refresh(db.clone(), chrono::Utc::now())
        .await
        .unwrap();
    let metrics = writer.metrics.render();
    assert!(metrics.contains("arena360_historical_job_completed_total{task=\"hot_parquet\"} 4"));
    assert!(metrics.contains("arena360_historical_job_failures_total{task=\"hot_parquet\"} 1"));
    fixture.tenant.revoke();
    assert!(writer.refresh(db, now).await.is_err());
    fixture.close().await;
    std::fs::remove_dir_all(root).unwrap();
}
