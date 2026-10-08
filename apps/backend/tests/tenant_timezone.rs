mod support;
use gaming_cafe_api::control::timezone::project;
use support::TenantFixture;
#[tokio::test]
async fn local_timezone_projection_rejects_stale_conflicting_and_invalid_updates() {
    let f = TenantFixture::new().await;
    assert!(project(f.db.clone(), "Asia/Kolkata".into(), 1)
        .await
        .unwrap());
    assert!(!project(f.db.clone(), "Asia/Kolkata".into(), 1)
        .await
        .unwrap());
    assert!(!project(f.db.clone(), "UTC".into(), 0).await.unwrap());
    assert!(project(f.db.clone(), "UTC".into(), 1).await.is_err());
    assert!(project(f.db.clone(), "invalid-zone".into(), 2)
        .await
        .is_err());
    assert_eq!(f.db.timezone().await.unwrap(), "Asia/Kolkata");
    f.revoke();
    assert!(project(f.db.clone(), "Asia/Kolkata".into(), 1)
        .await
        .is_err());
    assert!(project(f.db.clone(), "UTC".into(), 2).await.is_err());
    f.close().await;
}
#[cfg(feature = "duckdb-analytics")]
#[tokio::test]
async fn live_and_restart_drift_preserve_facts_but_require_rebuilding() {
    use gaming_cafe_api::analytics::{
        consumer::{apply_batch, BatchOutcome},
        retention::{run, RetentionOutcome},
        tenant_db::{error, TenantAnalytics},
    };
    let f = TenantFixture::new().await;
    let a = TenantAnalytics::open(f.db.clone()).await.unwrap();
    a.write(|tx|{tx.execute_batch("UPDATE _ingest_state SET status='READY',last_sequence=42; INSERT INTO users(id,username,role,is_active) VALUES(uuid(),'preserved-player','player',true)").map_err(error)?;Ok(())}).await.unwrap();
    project(f.db.clone(), "Asia/Kolkata".into(), 1)
        .await
        .unwrap();
    assert_eq!(
        apply_batch(a.clone(), vec![]).await.unwrap(),
        BatchOutcome::RebuildRequired
    );
    assert_eq!(
        run(a.clone(), chrono::Utc::now()).await.unwrap(),
        RetentionOutcome::Skipped
    );
    drop(a);
    assert!(TenantAnalytics::open(f.db.clone()).await.is_err());
    let a = TenantAnalytics::open_for_ingestion(f.db.clone())
        .await
        .unwrap();
    assert_eq!(a.read(|tx|tx.query_row("SELECT status,timezone,CAST(last_sequence AS BIGINT),(SELECT count(*) FROM users) FROM _ingest_state",[],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?))).map_err(error)).await.unwrap(),("REBUILDING".into(),"UTC".into(),42,1));
    drop(a);
    f.close().await;
}
#[cfg(feature = "duckdb-analytics")]
#[tokio::test]
#[ignore = "requires disposable JetStream and the matching signed SQLite extension"]
async fn live_timezone_rebuild_derives_new_calendar_labels_without_changing_utc_facts() {
    use async_nats::jetstream::stream::{Config, RetentionPolicy, StorageType};
    use gaming_cafe_api::{
        analytics::{
            consumer::{BatchOutcome, JetStreamConsumer},
            publisher::TENANT_EVENT_STREAM,
            rebuild::{rebuild, rebuild_with_hook},
            tenant_db::{error, TenantAnalytics},
        },
        metrics::Metrics,
    };
    let url = std::env::var("NATS_TEST_URL").unwrap();
    let context = async_nats::jetstream::new(async_nats::connect(&url).await.unwrap());
    context
        .create_stream(Config {
            name: TENANT_EVENT_STREAM.into(),
            subjects: vec!["arena.tenant.*.events.v1".into()],
            storage: StorageType::File,
            retention: RetentionPolicy::Limits,
            max_age: std::time::Duration::from_secs(7 * 86400),
            ..Default::default()
        })
        .await
        .unwrap();
    let s = support::SessionFixture::new().await;
    let f = &s.tenant;
    f.db.with_immediate_writer(|c| {
        Box::pin(async move {
            sqlx::query("UPDATE transactions SET transaction_date='2026-09-30T20:30:00.000000Z'")
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    let a = TenantAnalytics::open(f.db.clone()).await.unwrap();
    rebuild(f.db.clone(), a.clone(), &context).await.unwrap();
    assert_eq!(
        a.read(|tx| tx
            .query_row(
                "SELECT CAST(local_date AS VARCHAR) FROM transactions LIMIT 1",
                [],
                |r| r.get::<_, String>(0)
            )
            .map_err(error))
            .await
            .unwrap(),
        "2026-09-30"
    );
    a.write(|tx|{tx.execute_batch("INSERT INTO monthly_summary VALUES(DATE '2020-01-01','00000000-0000-0000-0000-000000000000',99,0,99,1,0,0,0)").map_err(error)?;Ok(())}).await.unwrap();
    let mut consumer = JetStreamConsumer::connect(&context, f.db.clone(), a.clone())
        .await
        .unwrap();
    project(f.db.clone(), "Asia/Kolkata".into(), 1)
        .await
        .unwrap();
    assert_eq!(
        consumer.poll(&Metrics::default()).await.unwrap(),
        BatchOutcome::RebuildRequired
    );
    rebuild(f.db.clone(), a.clone(), &context).await.unwrap();
    let facts=a.read(|tx|tx.query_row("SELECT CAST(local_date AS VARCHAR),CAST(occurred_at AS VARCHAR),(SELECT timezone FROM _ingest_state),(SELECT count(*) FROM monthly_summary WHERE month=DATE '2020-01-01') FROM transactions LIMIT 1",[],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,i64>(3)?))).map_err(error)).await.unwrap();
    assert_eq!(
        facts,
        (
            "2026-10-01".into(),
            "2026-09-30 20:30:00".into(),
            "Asia/Kolkata".into(),
            0
        )
    );
    // A change while the snapshot is building prevents publication of stale labels.
    assert!(
        rebuild_with_hook(f.db.clone(), a.clone(), &context, || async {
            project(f.db.clone(), "UTC".into(), 2).await?;
            Ok(())
        })
        .await
        .is_err()
    );
    rebuild(f.db.clone(), a.clone(), &context).await.unwrap();
    assert_eq!(
        a.read(|tx| tx
            .query_row(
                "SELECT CAST(local_date AS VARCHAR) FROM transactions LIMIT 1",
                [],
                |r| r.get::<_, String>(0)
            )
            .map_err(error))
            .await
            .unwrap(),
        "2026-09-30"
    );
    drop(consumer);
    drop(a);
    s.tenant.close().await;
}
