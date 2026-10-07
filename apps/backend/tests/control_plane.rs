//! Run against an empty database migrated with `migrations/control`:
//! CONTROL_TEST_DATABASE_URL=... cargo test --test control_plane -- --ignored

mod support;
use chrono::{Duration, Utc};
use gaming_cafe_api::control::{
    entitlement::EntitlementCache, CreateTenant, LeaseClient, LeaseConfig, LeaseRepository,
    Repository,
};
use gaming_cafe_api::{
    config::{Roles, Settings},
    dto::{JwtUserClaims, PanelLoginResponseDto, PanelMfaDto, StaffLoginDto},
    middleware::auth::control_panel_session_active,
    routing::{RoutingCache, TenantRouter, ROUTING_CHANGED_CHANNEL},
    services::AuthService,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use std::time::Duration as StdDuration;

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
    gaming_cafe_api::control::migrate(&pool).await.unwrap();
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
           VALUES ($1, $2, 'JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP', FALSE)
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

    let settings = Arc::new(test_settings(database_url));
    let auth = AuthService::new(settings.clone()).with_control_pool(Some(pool.clone()));

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
            username: username.clone(),
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
    let PanelLoginResponseDto::MfaRequired {
        challenge_token, ..
    } = panel
    else {
        panic!("MFA required");
    };
    for _ in 0..5 {
        let error = auth
            .verify_panel_mfa(PanelMfaDto {
                challengeToken: challenge_token.clone(),
                code: "invalid".into(),
            })
            .await
            .unwrap_err();
        assert!(error.to_string().contains("AUTH_INVALID_MFA"), "{error}");
    }
    let error = auth
        .verify_panel_mfa(PanelMfaDto {
            challengeToken: challenge_token.clone(),
            code: "invalid".into(),
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("AUTH_CHALLENGE_EXPIRED"));
    let panel = auth
        .login_panel(StaffLoginDto {
            username: username.clone(),
            password: password.into(),
            totp: None,
        })
        .await
        .unwrap();
    let PanelLoginResponseDto::MfaRequired {
        challenge_token, ..
    } = panel
    else {
        panic!("MFA required");
    };
    let totp = totp_rs::TOTP::new(
        totp_rs::Algorithm::SHA1,
        6,
        1,
        30,
        totp_rs::Secret::Encoded("JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP".into())
            .to_bytes()
            .unwrap(),
        Some("GameZone".into()),
        username,
    )
    .unwrap();
    let code = totp.generate_current().unwrap();
    let verified = auth
        .verify_panel_mfa(PanelMfaDto {
            challengeToken: challenge_token.clone(),
            code: code.clone(),
        })
        .await
        .unwrap();
    assert!(matches!(
        verified,
        PanelLoginResponseDto::Authenticated { .. }
    ));
    let replay = auth
        .verify_panel_mfa(PanelMfaDto {
            challengeToken: challenge_token,
            code,
        })
        .await
        .unwrap_err();
    assert!(replay.to_string().contains("AUTH_CHALLENGE_EXPIRED"));
    sqlx::query("DELETE FROM tenants WHERE id=$1")
        .bind(tenant.id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM users WHERE id=$1")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
}

#[tokio::test]
#[ignore = "requires an isolated control-plane database"]
async fn ownership_lease_fences_competing_cells_and_advances_generation() {
    let database_url =
        std::env::var("CONTROL_TEST_DATABASE_URL").expect("CONTROL_TEST_DATABASE_URL is required");
    let pool = PgPoolOptions::new()
        .max_connections(3)
        .connect(&database_url)
        .await
        .unwrap();
    gaming_cafe_api::control::migrate(&pool).await.unwrap();

    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let first_cell: uuid::Uuid =
        sqlx::query_scalar("INSERT INTO cells (name, address) VALUES ($1, $2) RETURNING id")
            .bind(format!("lease-a-{suffix}"))
            .bind(format!("http://lease-a-{suffix}.internal"))
            .fetch_one(&pool)
            .await
            .unwrap();
    let second_cell: uuid::Uuid =
        sqlx::query_scalar("INSERT INTO cells (name, address) VALUES ($1, $2) RETURNING id")
            .bind(format!("lease-b-{suffix}"))
            .bind(format!("http://lease-b-{suffix}.internal"))
            .fetch_one(&pool)
            .await
            .unwrap();
    let tenant = Repository::new(pool.clone())
        .create_tenant(CreateTenant {
            slug: format!("lease-{suffix}"),
            name: "Lease test".into(),
            timezone: "UTC".into(),
            owner_cell: Some(first_cell),
            subscription_plan: "trial".into(),
            entitlements: json!({}),
            trial_ends_at: Utc::now() + Duration::days(30),
            entitlement_grace_until: Utc::now() + Duration::days(37),
        })
        .await
        .unwrap();
    let config = LeaseConfig {
        lease_duration: StdDuration::from_secs(10),
        renewal_interval: StdDuration::from_secs(2),
        fence_before_expiry: StdDuration::from_secs(3),
        reassignment_skew: StdDuration::from_secs(1),
    };
    let first_client = LeaseClient::new(pool.clone(), first_cell, config).unwrap();
    assert!(
        first_client.acquire_assigned(tenant.id).await.is_err(),
        "restart recovery must not activate provisioning tenants"
    );
    let first_grant = first_client.acquire(tenant.id).await.unwrap();
    assert_eq!(first_grant.ownership_generation, 1);
    let restarted = LeaseClient::new(pool.clone(), first_cell, config).unwrap();
    assert_eq!(
        restarted
            .acquire_assigned(tenant.id)
            .await
            .unwrap()
            .ownership_generation,
        1
    );
    assert!(
        LeaseRepository::new(pool.clone())
            .acquire_assigned(tenant.id, second_cell, config)
            .await
            .is_err(),
        "startup must never take another cell's tenant"
    );

    assert_eq!(first_client.writable_generation(tenant.id).unwrap(), 1);
    assert!(first_client.ensure_writable(tenant.id, 2).is_err());

    let repository = LeaseRepository::new(pool.clone());
    assert!(repository
        .acquire(tenant.id, second_cell, config)
        .await
        .is_err());

    sqlx::query(
        "UPDATE tenant_leases \
         SET renewed_at = clock_timestamp() - INTERVAL '3 seconds', \
             expires_at = clock_timestamp() - INTERVAL '2 seconds' \
         WHERE tenant_id = $1",
    )
    .bind(tenant.id)
    .execute(&pool)
    .await
    .unwrap();
    let second_grant = repository
        .acquire(tenant.id, second_cell, config)
        .await
        .unwrap();
    assert_eq!(second_grant.ownership_generation, 2);
    assert!(first_client.renew(tenant.id).await.is_err());
    assert!(first_client.writable_generation(tenant.id).is_err());
    let owner_after_takeover: (uuid::Uuid, i64) =
        sqlx::query_as("SELECT owner_cell, ownership_generation FROM tenants WHERE id = $1")
            .bind(tenant.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(owner_after_takeover, (second_cell, 2));

    let second_client = LeaseClient::new(pool.clone(), second_cell, config).unwrap();
    assert_eq!(
        second_client
            .acquire(tenant.id)
            .await
            .unwrap()
            .ownership_generation,
        2
    );
    let routing = Arc::new(RoutingCache::new(pool.clone()));
    routing.refresh_all().await.unwrap();
    assert_eq!(
        routing
            .resolve(tenant.id)
            .await
            .unwrap()
            .unwrap()
            .owner_cell,
        second_cell
    );
    let second_router = TenantRouter::new(routing.clone(), Some(second_cell)).unwrap();
    let first_router = TenantRouter::new(routing.clone(), Some(first_cell)).unwrap();
    let dedicated_router = TenantRouter::new(routing.clone(), None).unwrap();
    assert!(second_router
        .remote_address(tenant.id)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        first_router
            .remote_address(tenant.id)
            .await
            .unwrap()
            .unwrap(),
        format!("http://lease-b-{suffix}.internal")
    );
    assert_eq!(
        dedicated_router
            .remote_address(tenant.id)
            .await
            .unwrap()
            .unwrap(),
        format!("http://lease-b-{suffix}.internal")
    );
    let mut routing_notifications = sqlx::postgres::PgListener::connect(&database_url)
        .await
        .unwrap();
    routing_notifications
        .listen(ROUTING_CHANGED_CHANNEL)
        .await
        .unwrap();
    let fenced_client = second_client.clone();
    let handoff = second_client
        .handoff(tenant.id, first_cell, move |generation| async move {
            assert_eq!(generation, 2);
            assert!(fenced_client.writable_generation(tenant.id).is_err());
            Ok(())
        })
        .await
        .unwrap();
    let notification =
        tokio::time::timeout(StdDuration::from_secs(1), routing_notifications.recv())
            .await
            .unwrap()
            .unwrap();
    assert_eq!(notification.payload(), tenant.id.to_string());
    routing.invalidate(tenant.id).await;
    assert_eq!(
        routing
            .resolve(tenant.id)
            .await
            .unwrap()
            .unwrap()
            .owner_cell,
        first_cell
    );
    assert!(first_router
        .remote_address(tenant.id)
        .await
        .unwrap()
        .is_none());
    assert_eq!(handoff.owner_cell, first_cell);
    assert_eq!(handoff.ownership_generation, 3);
    let owner_after_handoff: (uuid::Uuid, i64) =
        sqlx::query_as("SELECT owner_cell, ownership_generation FROM tenants WHERE id = $1")
            .bind(tenant.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(owner_after_handoff, (first_cell, 3));
    assert_eq!(
        first_client
            .acquire(tenant.id)
            .await
            .unwrap()
            .ownership_generation,
        3
    );
    assert!(first_client
        .handoff(tenant.id, second_cell, |_| async {
            Err(gaming_cafe_api::error::AppError::Internal(
                "simulated WAL upload failure".into(),
            ))
        })
        .await
        .is_err());
    let owner_after_failed_flush: (uuid::Uuid, i64) =
        sqlx::query_as("SELECT owner_cell, ownership_generation FROM tenants WHERE id = $1")
            .bind(tenant.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(owner_after_failed_flush, (first_cell, 3));
    assert!(first_client.writable_generation(tenant.id).is_err());
}

fn test_settings(database_url: String) -> Settings {
    Settings {
        roles: Roles::ALL,
        cell_id: None,
        tenant_data_dir: std::path::PathBuf::from("data/tenants"),
        control_database_url: Some(database_url.clone()),
        database_min_connections: 0,
        database_max_connections: 3,
        database_acquire_timeout_seconds: 2,
        database_idle_timeout_seconds: 600,
        database_max_lifetime_seconds: 1800,
        nats_url: None,
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

#[tokio::test]
#[ignore="requires an isolated control-plane database"]
async fn timezone_updates_are_revisioned_validated_and_survive_direct_administration(){
 let pool=PgPoolOptions::new().max_connections(2).connect(&std::env::var("CONTROL_TEST_DATABASE_URL").unwrap()).await.unwrap();
 gaming_cafe_api::control::migrate(&pool).await.unwrap();
 let repo=Repository::new(pool.clone());let now=Utc::now();
 let tenant=repo.create_tenant(CreateTenant{slug:format!("timezone-{}",uuid::Uuid::new_v4()),name:"Timezone test".into(),timezone:"UTC".into(),owner_cell:None,subscription_plan:"trial".into(),entitlements:json!({}),trial_ends_at:now+Duration::days(30),entitlement_grace_until:now+Duration::days(37)}).await.unwrap();
 let secret=b"timezone-control-test-secret-32-bytes";
 let old=repo.signed_entitlement(tenant.id,secret).await.unwrap();
 assert!(repo.update_timezone(tenant.id,"invalid-zone").await.is_err());
 repo.update_timezone(tenant.id,"Asia/Kolkata").await.unwrap();
 repo.update_timezone(tenant.id,"Asia/Kolkata").await.unwrap();
 let rev:i64=sqlx::query_scalar("SELECT timezone_revision FROM tenants WHERE id=$1").bind(tenant.id).fetch_one(&pool).await.unwrap();assert_eq!(rev,1);
 let cache=EntitlementCache::new(secret.as_slice());cache.update(&repo.signed_entitlement(tenant.id,secret).await.unwrap()).await.unwrap();assert!(cache.update(&old).await.is_err());
 sqlx::query("UPDATE tenants SET timezone='Europe/Berlin',timezone_revision=0 WHERE id=$1").bind(tenant.id).execute(&pool).await.unwrap();
 let rev:i64=sqlx::query_scalar("SELECT timezone_revision FROM tenants WHERE id=$1").bind(tenant.id).fetch_one(&pool).await.unwrap();assert_eq!(rev,2);
 let mut tx=pool.begin().await.unwrap();sqlx::query("UPDATE tenants SET timezone='UTC' WHERE id=$1").bind(tenant.id).execute(&mut *tx).await.unwrap();tx.rollback().await.unwrap();
 let timezone:String=sqlx::query_scalar("SELECT timezone FROM tenants WHERE id=$1").bind(tenant.id).fetch_one(&pool).await.unwrap();assert_eq!(timezone,"Europe/Berlin");
 let f=support::TenantFixture::new().await;
 sqlx::query("INSERT INTO tenants(id,slug,name,state,timezone) VALUES($1,$2,'Local calendar projection','PROVISIONING','UTC')").bind(f.db.tenant_id()).bind(format!("timezone-local-{}",f.db.tenant_id())).execute(&pool).await.unwrap();
 repo.update_timezone(f.db.tenant_id(),"Europe/Berlin").await.unwrap();
 assert!(gaming_cafe_api::control::timezone::sync(&pool,f.db.clone()).await.unwrap());
 assert!(!gaming_cafe_api::control::timezone::sync(&pool,f.db.clone()).await.unwrap());
 pool.close().await;
 assert!(gaming_cafe_api::control::timezone::sync(&pool,f.db.clone()).await.is_err());
 assert_eq!(f.db.timezone().await.unwrap(),"Europe/Berlin");
 f.player("control-outage-calendar-player").await;
 f.close().await;
}
