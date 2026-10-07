//! SQLite wallets remain authoritative when cached views are stale or unavailable.
mod support;
use gaming_cafe_api::cache::{keys, CacheService};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use support::SessionFixture;

#[tokio::test]
async fn recharge_writes_latest_wallet_to_raw_cache() {
    let f = SessionFixture::new().await;
    let updated = f.recharge().await;
    let cached = f
        .cache
        .get_value(&keys::balance_raw(&f.balance))
        .await
        .unwrap()
        .unwrap();
    let wallet: gaming_cafe_api::models::PlayerPlanBalance =
        serde_json::from_value(cached).unwrap();
    assert_eq!(wallet.remaining_minutes, updated.remaining_minutes);
    assert_eq!(wallet.remaining_minutes, 240);
    f.close().await;
}
#[tokio::test]
async fn recharge_invalidates_active_scope_cache() {
    let f = SessionFixture::new().await;
    let raw = f
        .balances
        .get_raw_tenant(f.tenant.db.clone(), f.balance)
        .await
        .unwrap();
    let scope = format!(
        "{}:{}:{}",
        raw.device_type.unwrap(),
        raw.device_sub_type.unwrap(),
        raw.kind
    );
    let key = keys::balance_active(&f.player, &scope);
    f.cache
        .set_value(&key, &json!({"stale":true}), Duration::from_secs(60))
        .await
        .unwrap();
    f.recharge().await;
    assert!(f.cache.get_value(&key).await.unwrap().is_none());
    f.close().await;
}
#[tokio::test]
async fn tenant_wallet_read_ignores_a_stale_raw_cache() {
    let f = SessionFixture::new().await;
    let mut raw = f
        .balances
        .get_raw_tenant(f.tenant.db.clone(), f.balance)
        .await
        .unwrap();
    raw.remaining_minutes = 9999;
    f.cache
        .set_value(
            &keys::balance_raw(&f.balance),
            &serde_json::to_value(raw).unwrap(),
            Duration::from_secs(60),
        )
        .await
        .unwrap();
    assert_eq!(
        f.balances
            .get_raw_tenant(f.tenant.db.clone(), f.balance)
            .await
            .unwrap()
            .remaining_minutes,
        120
    );
    f.recharge().await;
    assert_eq!(
        f.balances
            .get_raw_tenant(f.tenant.db.clone(), f.balance)
            .await
            .unwrap()
            .remaining_minutes,
        240
    );
    f.close().await;
}
#[tokio::test]
async fn heartbeat_writes_the_committed_wallet_to_raw_cache() {
    let f = SessionFixture::new().await;
    let session = f
        .start(Some(chrono::Utc::now() - chrono::Duration::minutes(2)))
        .await;
    f.sessions
        .heartbeat_for_player_tenant(f.tenant.db.clone(), session.id, f.player, f.device)
        .await
        .unwrap();
    let cached: gaming_cafe_api::models::PlayerPlanBalance = serde_json::from_value(
        f.cache
            .get_value(&keys::balance_raw(&f.balance))
            .await
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    let stored = f
        .balances
        .get_raw_tenant(f.tenant.db.clone(), f.balance)
        .await
        .unwrap();
    assert_eq!(cached.remaining_minutes, stored.remaining_minutes);
    assert!(stored.remaining_minutes <= 118);
    f.close().await;
}
struct UnavailableCache;
#[async_trait::async_trait]
impl CacheService for UnavailableCache {
    async fn get_value(
        &self,
        _: &str,
    ) -> Result<Option<serde_json::Value>, gaming_cafe_api::error::AppError> {
        Err(gaming_cafe_api::error::AppError::Internal(
            "cache unavailable".into(),
        ))
    }
    async fn set_value(
        &self,
        _: &str,
        _: &serde_json::Value,
        _: Duration,
    ) -> Result<(), gaming_cafe_api::error::AppError> {
        Err(gaming_cafe_api::error::AppError::Internal(
            "cache unavailable".into(),
        ))
    }
    async fn delete(&self, _: &[&str]) -> Result<(), gaming_cafe_api::error::AppError> {
        Err(gaming_cafe_api::error::AppError::Internal(
            "cache unavailable".into(),
        ))
    }
    async fn invalidate_prefix(&self, _: &str) -> Result<(), gaming_cafe_api::error::AppError> {
        Err(gaming_cafe_api::error::AppError::Internal(
            "cache unavailable".into(),
        ))
    }
    async fn publish_invalidation(
        &self,
        _: &[String],
    ) -> Result<(), gaming_cafe_api::error::AppError> {
        Err(gaming_cafe_api::error::AppError::Internal(
            "cache unavailable".into(),
        ))
    }
    async fn consume_ip_token(
        &self,
        _: &str,
        _: u32,
        _: Duration,
    ) -> Result<Option<gaming_cafe_api::cache::RateLimitDecision>, gaming_cafe_api::error::AppError>
    {
        Ok(None)
    }
    fn is_available(&self) -> bool {
        true
    }
}
#[tokio::test]
async fn committed_recharge_succeeds_during_cache_outage() {
    let f = SessionFixture::new().await;
    let service = gaming_cafe_api::services::BalanceService::new(Arc::new(UnavailableCache));
    let updated = service
        .purchase_or_recharge_tenant(
            f.tenant.db.clone(),
            gaming_cafe_api::models::PurchaseBalanceDto {
                player_id: f.player,
                plan_id: f.plan,
                transaction_id: Some(f.tenant.plan_transaction(f.player, f.plan, f.venue).await),
            },
            None,
        )
        .await
        .unwrap();
    assert_eq!(updated.remaining_minutes, 240);
    assert_eq!(
        f.balances
            .get_raw_tenant(f.tenant.db.clone(), f.balance)
            .await
            .unwrap()
            .remaining_minutes,
        240
    );
    f.close().await;
}
