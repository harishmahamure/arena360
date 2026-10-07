//! Tenant reads are authoritative; mutations invalidate stale session views.
mod support;
use gaming_cafe_api::{
    cache::{keys, CacheService},
    models::SessionFilterDto,
};
use serde_json::json;
use std::time::Duration;
use support::SessionFixture;

async fn warm(f: &SessionFixture, id: uuid::Uuid) -> Vec<String> {
    let keys = vec![
        keys::sessions_list("active"),
        keys::session_enriched(&id),
        keys::session_device(&f.device),
    ];
    for key in &keys {
        f.cache
            .set_value(key, &json!({"stale":true}), Duration::from_secs(60))
            .await
            .unwrap();
    }
    keys
}
#[tokio::test]
async fn list_active_ignores_stale_shared_cache_and_isolates_tenants() {
    let f = SessionFixture::new().await;
    let other = SessionFixture::new().await;
    let session = f.start(None).await;
    warm(&f, session.id).await;
    let filters = || SessionFilterDto {
        is_active: Some(1),
        ..Default::default()
    };
    let rows = f
        .sessions
        .list_tenant(f.tenant.db.clone(), filters(), vec![f.venue], "UTC".into())
        .await
        .unwrap();
    assert_eq!(rows.total, 1);
    assert_eq!(rows.data[0].id, session.id);
    assert_eq!(
        f.sessions
            .list_tenant(
                other.tenant.db.clone(),
                filters(),
                vec![other.venue],
                "UTC".into()
            )
            .await
            .unwrap()
            .total,
        0
    );
    other.close().await;
    f.close().await;
}
#[tokio::test]
async fn session_heartbeat_invalidates_list_and_detail_cache() {
    let f = SessionFixture::new().await;
    let session = f
        .start(Some(chrono::Utc::now() - chrono::Duration::minutes(2)))
        .await;
    let keys = warm(&f, session.id).await;
    f.sessions
        .heartbeat_for_player_tenant(f.tenant.db.clone(), session.id, f.player, f.device)
        .await
        .unwrap();
    for key in keys {
        assert!(f.cache.get_value(&key).await.unwrap().is_none());
    }
    f.close().await;
}
#[tokio::test]
async fn balance_mutation_invalidates_open_session_views() {
    let f = SessionFixture::new().await;
    let session = f.start(None).await;
    let keys = warm(&f, session.id).await;
    f.recharge().await;
    for key in keys {
        assert!(f.cache.get_value(&key).await.unwrap().is_none());
    }
    f.close().await;
}
#[tokio::test]
async fn heartbeat_detail_reads_committed_wallet_and_charge() {
    let f = SessionFixture::new().await;
    let session = f
        .start(Some(chrono::Utc::now() - chrono::Duration::minutes(2)))
        .await;
    warm(&f, session.id).await;
    f.sessions
        .heartbeat_for_player_tenant(f.tenant.db.clone(), session.id, f.player, f.device)
        .await
        .unwrap();
    let stored = f
        .sessions
        .get_by_id_tenant(f.tenant.db.clone(), session.id, "UTC".into())
        .await
        .unwrap();
    let charged = stored.time_credits_consumed.unwrap();
    assert!(charged >= 2);
    assert_eq!(stored.balance.unwrap().remaining_minutes, 120 - charged);
    assert_eq!(stored.wallet_minutes_at_start, Some(120));
    f.close().await;
}
