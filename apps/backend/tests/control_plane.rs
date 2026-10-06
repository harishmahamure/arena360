//! Run against an empty database migrated with `migrations/control`:
//! CONTROL_TEST_DATABASE_URL=... cargo test --test control_plane -- --ignored

use chrono::{Duration, Utc};
use gaming_cafe_api::control::{
    entitlement::EntitlementCache, CreateTenant, Repository,
};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;

#[tokio::test]
#[ignore = "requires an isolated control-plane database"]
async fn provisions_tenant_and_uses_entitlement_after_postgres_stops() {
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(
            &std::env::var("CONTROL_TEST_DATABASE_URL")
                .expect("CONTROL_TEST_DATABASE_URL is required"),
        )
        .await
        .unwrap();
    let repository = Repository::new(pool.clone());
    let now = Utc::now();
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let tenant = repository
        .create_tenant(CreateTenant {
            slug: format!("test-{suffix}"),
            name: "Control test".into(),
            timezone: "Asia/Kolkata".into(),
            owner_cell: None,
            subscription_plan: "trial".into(),
            entitlements: json!({"maxDevices": 20}),
            trial_ends_at: now + Duration::days(30),
            entitlement_grace_until: now + Duration::days(37),
        })
        .await
        .unwrap();
    assert_eq!(tenant.state, "PROVISIONING");

    let secret = b"control-plane-integration-secret-32-bytes";
    let token = repository
        .signed_entitlement(tenant.id, secret)
        .await
        .unwrap();
    let cache = EntitlementCache::new(secret.as_slice());
    cache.update(&token).await.unwrap();

    pool.close().await;
    let cached = cache.get(tenant.id).await.unwrap();
    assert_eq!(cached.timezone, "Asia/Kolkata");
    assert_eq!(cached.entitlements["maxDevices"], 20);
}
