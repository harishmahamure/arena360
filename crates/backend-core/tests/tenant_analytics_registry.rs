#![cfg(feature = "duckdb-analytics")]
mod support;
use gaming_cafe_api::analytics::{registry::AnalyticsRegistry, tenant_db::error};
use std::sync::Arc;
use support::TenantFixture;

#[tokio::test]
async fn concurrent_readers_and_ingestion_share_one_connection_per_tenant() {
    let a = TenantFixture::new().await;
    let b = TenantFixture::new().await;
    let registry = Arc::new(AnalyticsRegistry::default());
    let (first, second, foreign) = tokio::join!(
        registry.get(a.db.clone()),
        registry.get(a.db.clone()),
        registry.get(b.db.clone())
    );
    let first = first.unwrap();
    let second = second.unwrap();
    let foreign = foreign.unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    assert!(!Arc::ptr_eq(&first, &foreign));
    first
        .write(|tx| {
            tx.execute_batch("UPDATE _ingest_state SET last_sequence=17")
                .map_err(error)?;
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(
        second
            .read(|tx| tx
                .query_row("SELECT last_sequence FROM _ingest_state", [], |r| r
                    .get::<_, u64>(0))
                .map_err(error))
            .await
            .unwrap(),
        17
    );
    assert_eq!(
        foreign
            .read(|tx| tx
                .query_row("SELECT last_sequence FROM _ingest_state", [], |r| r
                    .get::<_, u64>(0))
                .map_err(error))
            .await
            .unwrap(),
        0
    );
    drop((first, second, foreign));
    a.close().await;
    b.close().await;
}

#[tokio::test]
async fn failed_worker_closes_old_readers_without_closing_its_replacement() {
    let f = TenantFixture::new().await;
    let registry = AnalyticsRegistry::default();
    let old = registry.get(f.db.clone()).await.unwrap();
    registry.invalidate(&old).await.unwrap();
    assert!(old.read(|_| Ok(())).await.is_err());
    let replacement = registry.get(f.db.clone()).await.unwrap();
    assert!(!Arc::ptr_eq(&old, &replacement));
    registry.invalidate(&old).await.unwrap();
    replacement.read(|_| Ok(())).await.unwrap();
    f.revoke();
    assert!(registry.get(f.db.clone()).await.is_err());
    assert!(replacement.read(|_| Ok(())).await.is_err());
    drop((old, replacement));
    f.close().await;
}

#[tokio::test]
async fn reopening_sqlite_after_idle_close_replaces_the_native_connection() {
    let f = TenantFixture::new().await;
    let registry = AnalyticsRegistry::default();
    let old = registry.get(f.db.clone()).await.unwrap();
    f.db.close().await.unwrap();
    let db = f.manager.open(f.db.tenant_id()).await.unwrap();
    assert_eq!(db.ownership_generation(), f.db.ownership_generation());
    let replacement = registry.get(db.clone()).await.unwrap();
    assert!(!Arc::ptr_eq(&old, &replacement));
    assert!(old.read(|_| Ok(())).await.is_err());
    replacement.read(|_| Ok(())).await.unwrap();
    drop((old, replacement));
    db.close().await.unwrap();
    f.close().await;
}
