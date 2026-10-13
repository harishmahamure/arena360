mod support;
use gaming_cafe_api::{
    analytics::outbox_retention::prune_batch,
    background::{BackgroundJobs, Limits, Priority},
};
use std::time::Duration;
async fn queued(jobs: &BackgroundJobs, p: Priority) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while jobs.stats().queued(p) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn queued_retention_leaves_operational_writes_free_and_rechecks_ownership() {
    let jobs = BackgroundJobs::new(Limits::default()).unwrap();
    let f = support::TenantFixture::new_with_background_jobs(jobs.clone()).await;
    f.player("first-source-player").await;
    let held = jobs.acquire(Priority::Outbox).await.unwrap();
    let db = f.db.clone();
    let task = tokio::spawn(async move { prune_batch(db, chrono::Utc::now()).await });
    queued(&jobs, Priority::Outbox).await;
    tokio::time::timeout(Duration::from_secs(2), f.player("foreground-writer-player"))
        .await
        .unwrap();
    f.revoke();
    drop(held);
    assert!(task.await.unwrap().is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox_events")
            .fetch_one(&f.db.read_pool().unwrap())
            .await
            .unwrap(),
        2
    );
    f.close().await;
}
#[cfg(feature = "duckdb-analytics")]
#[tokio::test]
async fn queued_ingestion_cannot_commit_after_ownership_changes() {
    use gaming_cafe_api::analytics::{
        consumer::apply_batch,
        tenant_db::{error, TenantAnalytics},
    };
    let jobs = BackgroundJobs::new(Limits::default()).unwrap();
    let f = support::TenantFixture::new_with_background_jobs(jobs.clone()).await;
    let a = TenantAnalytics::open(f.db.clone()).await.unwrap();
    a.write(|tx| {
        tx.execute_batch("UPDATE _ingest_state SET status='READY'")
            .map_err(error)?;
        Ok(())
    })
    .await
    .unwrap();
    let first = jobs.acquire(Priority::AnalyticsIngestion).await.unwrap();
    let second = jobs.acquire(Priority::AnalyticsIngestion).await.unwrap();
    let copy = a.clone();
    let task = tokio::spawn(async move { apply_batch(copy, vec![]).await });
    queued(&jobs, Priority::AnalyticsIngestion).await;
    f.revoke();
    drop(first);
    drop(second);
    assert!(task.await.unwrap().is_err());
    let path = a.path().to_owned();
    drop(a);
    let c = duckdb::Connection::open(path).unwrap();
    assert_eq!(
        c.query_row(
            "SELECT CAST(last_sequence AS BIGINT) FROM _ingest_state",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    drop(c);
    f.close().await;
}
