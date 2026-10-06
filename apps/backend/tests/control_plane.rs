//! Run against an empty database migrated with `migrations/control`:
//! CONTROL_TEST_DATABASE_URL=... cargo test --test control_plane -- --ignored

use chrono::{Duration, Utc};
use gaming_cafe_api::control::{entitlement::EntitlementCache, CreateTenant, Repository};
use gaming_cafe_api::{
    cache::NoopCache,
    config::{Roles, Settings},
    dto::{JwtUserClaims, PanelLoginResponseDto, StaffLoginDto},
    middleware::auth::control_panel_session_active,
    services::{AuthService, BalanceService, UserService},
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

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

#[tokio::test]
#[ignore = "requires an isolated control-plane database"]
async fn authenticates_staff_and_invalidates_disabled_membership() {
    let database_url =
        std::env::var("CONTROL_TEST_DATABASE_URL").expect("CONTROL_TEST_DATABASE_URL is required");
    let pool = PgPoolOptions::new()
        .max_connections(3)
        .connect(&database_url)
        .await
        .unwrap();
    gaming_cafe_api::control::migrate(&pool).await.unwrap();

    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let tenant = Repository::new(pool.clone())
        .create_tenant(CreateTenant {
            slug: format!("auth-{suffix}"),
            name: "Auth test".into(),
            timezone: "UTC".into(),
            owner_cell: None,
            subscription_plan: "trial".into(),
            entitlements: json!({}),
            trial_ends_at: Utc::now() + Duration::days(30),
            entitlement_grace_until: Utc::now() + Duration::days(37),
        })
        .await
        .unwrap();
    let username = format!("staff-{suffix}");
    let password = "correct horse battery staple";
    let user_id: uuid::Uuid = sqlx::query_scalar(
        r#"INSERT INTO users (username, password_hash, totp_secret, totp_enabled)
           VALUES ($1, $2, 'JBSWY3DPEHPK3PXP', FALSE)
           RETURNING id"#,
    )
    .bind(&username)
    .bind(bcrypt::hash(password, 4).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO organization_memberships
             (tenant_id, user_id, role, permissions)
           VALUES ($1, $2, 'staff', '["shifts:write"]')"#,
    )
    .bind(tenant.id)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    let cache = Arc::new(NoopCache);
    let operational_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy(&database_url)
        .unwrap();
    let users = Arc::new(UserService::new(operational_pool.clone(), cache.clone()));
    let balances = Arc::new(BalanceService::new(operational_pool.clone(), cache));
    let settings = Arc::new(test_settings(database_url));
    let auth = AuthService::new(operational_pool, settings.clone(), balances, users)
        .with_control_pool(Some(pool.clone()));

    let user = auth.authenticate_staff(&username, password).await.unwrap();
    let response = auth.issue_auth_response(&user).await.unwrap();
    let mut validation = Validation::default();
    validation.set_audience(&["gamezone"]);
    let claims = decode::<JwtUserClaims>(
        &response.accessToken,
        &DecodingKey::from_secret(settings.jwt_secret.as_bytes()),
        &validation,
    )
    .unwrap()
    .claims;
    assert_eq!(claims.tenantId, tenant.id.to_string());
    assert_eq!(claims.permissions, vec!["shifts:write"]);
    assert!(control_panel_session_active(&pool, &claims).await.unwrap());

    sqlx::query(
        "UPDATE organization_memberships SET is_active = FALSE \
         WHERE tenant_id = $1 AND user_id = $2",
    )
    .bind(tenant.id)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();
    assert!(!control_panel_session_active(&pool, &claims).await.unwrap());

    sqlx::query(
        "UPDATE organization_memberships SET is_active = TRUE WHERE tenant_id = $1 AND user_id = $2",
    )
    .bind(tenant.id)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE users SET totp_enabled = TRUE WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    let panel = auth
        .login_panel(StaffLoginDto {
            username,
            password: password.into(),
            totp: None,
        })
        .await
        .unwrap();
    assert!(matches!(panel, PanelLoginResponseDto::MfaRequired { .. }));
    let challenge_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM auth_challenges WHERE user_id = $1 AND kind = 'PANEL_MFA'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(challenge_count, 1);
}

fn test_settings(database_url: String) -> Settings {
    Settings {
        roles: Roles::ALL,
        database_url: database_url.clone(),
        control_database_url: Some(database_url.clone()),
        database_listener_url: database_url,
        database_min_connections: 0,
        database_max_connections: 3,
        database_acquire_timeout_seconds: 2,
        database_idle_timeout_seconds: 600,
        database_max_lifetime_seconds: 1800,
        redis_url: None,
        jwt_secret: "control-auth-test-secret-at-least-32-bytes".into(),
        jwt_access_expiration: "15m".into(),
        jwt_player_expiration: "24h".into(),
        jwt_device_expiration: "365d".into(),
        bcrypt_salt_rounds: 4,
        port: 3000,
        cafe_timezone: "UTC".into(),
        zeptomail_token: None,
        legacy_rest_enabled: false,
        trusted_proxy_cidrs: Vec::new(),
        max_concurrent_requests: 256,
    }
}
