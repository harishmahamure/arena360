use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::Utc;
use futures::future::BoxFuture;
use gaming_cafe_api::config::{Roles, Settings};
use gaming_cafe_api::dto::{KioskRegisterDto, LoginDto};
use gaming_cafe_api::error::AppError;
use gaming_cafe_api::models::{AssignPlanDto, Device, UpdateUserDto, UserFilterDto};
use gaming_cafe_api::repositories::{
    StaffProjectionResult, TenantCreatePlayer, TenantLocationRoleGrant, TenantStaffProjection,
    TenantUserRepository,
};
use gaming_cafe_api::services::{AuthService, PlayerPlanService, UserService};
use gaming_cafe_api::tenancy::{
    tenant_path, TenantDb, TenantDbConfig, TenantDbManager, TenantLease,
};
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use uuid::Uuid;

#[derive(Default)]
struct Lease {
    generations: RwLock<HashMap<Uuid, i64>>,
    ensure_calls: AtomicUsize,
    fail_on_call: AtomicUsize,
}

impl Lease {
    fn fail_next_commit(&self) {
        self.ensure_calls.store(0, Ordering::SeqCst);
        self.fail_on_call.store(3, Ordering::SeqCst);
    }
}

impl TenantLease for Lease {
    fn writable_generation(&self, tenant_id: Uuid) -> Result<i64, AppError> {
        self.generations
            .read()
            .map_err(|_| AppError::Internal("lease lock poisoned".into()))?
            .get(&tenant_id)
            .copied()
            .ok_or_else(|| AppError::Forbidden("tenant lease is not writable".into()))
    }

    fn ensure_writable(&self, tenant_id: Uuid, generation: i64) -> Result<(), AppError> {
        let call = self.ensure_calls.fetch_add(1, Ordering::SeqCst) + 1;
        if self.fail_on_call.load(Ordering::SeqCst) == call {
            self.generations
                .write()
                .map_err(|_| AppError::Internal("lease lock poisoned".into()))?
                .insert(tenant_id, generation + 1);
        }
        if self.writable_generation(tenant_id)? == generation {
            Ok(())
        } else {
            Err(AppError::Forbidden(
                "tenant lease generation changed".into(),
            ))
        }
    }
}

#[tokio::test]
#[ignore = "requires isolated CONTROL_TEST_DATABASE_URL"]
async fn control_identity_and_local_membership_commands_recover_and_preserve_grants() {
    use gaming_cafe_api::{
        control::staff_projection::sync_tenant,
        repositories::{TenantAccessRepository, TenantMemberDto},
    };
    let f = Fixture::new().await;
    let pool = PgPoolOptions::new()
        .connect(&std::env::var("CONTROL_TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    gaming_cafe_api::control::migrate(&pool).await.unwrap();
    sqlx::query("INSERT INTO tenants(id,slug,name,timezone) VALUES($1,$2,'Projection test','UTC')")
        .bind(f.tenant_id)
        .bind(format!("projection-{}", f.tenant_id))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO subscriptions(tenant_id,plan_code,status,starts_at,ends_at) VALUES($1,'trial','TRIAL',NOW(),NOW()+INTERVAL '30 days')").bind(f.tenant_id).execute(&pool).await.unwrap();
    let cell = Uuid::now_v7();
    sqlx::query("INSERT INTO cells(id,name,address) VALUES($1,$2,$3)")
        .bind(cell)
        .bind(format!("cell-{cell}"))
        .bind(format!("http://{cell}"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE tenants SET owner_cell=$2,ownership_generation=1,state='ACTIVE' WHERE id=$1",
    )
    .bind(f.tenant_id)
    .bind(cell)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO tenant_leases(tenant_id,owner_cell,ownership_generation,expires_at) VALUES($1,$2,1,now()+interval '1 hour')").bind(f.tenant_id).bind(cell).execute(&pool).await.unwrap();
    let user = f.staff_id;
    let manager = Uuid::now_v7();
    for (id, role) in [(manager, "admin"), (user, "staff")] {
        sqlx::query(
            "INSERT INTO users(id,username,password_hash) VALUES($1,$2,'secret-never-projected')",
        )
        .bind(id)
        .bind(format!("global-{id}"))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO organization_memberships(tenant_id,user_id,role) VALUES($1,$2,$3)",
        )
        .bind(f.tenant_id)
        .bind(id)
        .bind(role)
        .execute(&pool)
        .await
        .unwrap();
    }
    sync_tenant(&pool, f.db.clone()).await.unwrap();
    let repo = TenantUserRepository::new(f.db.clone());
    assert!(repo
        .require_active_staff(user)
        .await
        .unwrap()
        .password_hash
        .is_none());
    let access = TenantAccessRepository::new(f.db.clone());
    let edit = |active, revision| TenantMemberDto {
        role_ids: vec![Uuid::from_u128(12)],
        location_ids: Some(vec![f.location_a]),
        location_roles: None,
        active,
        expected_revision: revision,
    };
    access
        .save_member_assignments(user, edit(false, 0), manager)
        .await
        .unwrap();
    assert!(repo.require_active_staff(user).await.is_err());
    sync_tenant(&pool, f.db.clone()).await.unwrap();
    let active: bool = sqlx::query_scalar(
        "SELECT is_active FROM organization_memberships WHERE tenant_id=$1 AND user_id=$2",
    )
    .bind(f.tenant_id)
    .bind(user)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!active);
    access
        .save_member_assignments(user, edit(true, 1), manager)
        .await
        .unwrap();
    // Pending activation survives a control-plane outage and stays disabled.
    let unavailable = PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(100))
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    assert!(sync_tenant(&unavailable, f.db.clone()).await.is_err());
    assert!(repo.require_active_staff(user).await.is_err());
    sync_tenant(&pool, f.db.clone()).await.unwrap();
    assert!(repo.require_active_staff(user).await.is_ok());
    sqlx::query("UPDATE users SET first_name='Global refresh' WHERE id=$1")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    sync_tenant(&pool, f.db.clone()).await.unwrap();
    assert_eq!(
        repo.require_active_staff(user)
            .await
            .unwrap()
            .first_name
            .as_deref(),
        Some("Global refresh")
    );
    let key: String = sqlx::query_scalar("SELECT r.system_key FROM access_assignments a JOIN access_roles r ON r.id=a.role_id WHERE a.user_id=?")
        .bind(user.to_string()).fetch_one(&f.db.read_pool().unwrap()).await.unwrap();
    assert_eq!(key, "finance");
    let other = Uuid::now_v7();
    sqlx::query("INSERT INTO tenants(id,slug,name,timezone) VALUES($1,$2,'Other tenant','UTC')")
        .bind(other)
        .bind(format!("other-{other}"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO subscriptions(tenant_id,plan_code,status,starts_at,ends_at) VALUES($1,'trial','TRIAL',NOW(),NOW()+INTERVAL '30 days')").bind(other).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO organization_memberships(tenant_id,user_id,role,created_at) VALUES($1,$2,'admin','2000-01-01')").bind(other).bind(user).execute(&pool).await.unwrap();
    let auth_settings = test_settings();
    let auth = AuthService::new(auth_settings.clone()).with_control_pool(Some(pool.clone()));
    let response = auth
        .issue_tenant_auth_response(f.db.clone(), user)
        .await
        .unwrap();
    let mut validation = jsonwebtoken::Validation::default();
    validation.set_audience(&["gamezone"]);
    let claims = jsonwebtoken::decode::<gaming_cafe_api::dto::JwtUserClaims>(
        &response.accessToken,
        &jsonwebtoken::DecodingKey::from_secret(auth_settings.jwt_secret.as_bytes()),
        &validation,
    )
    .unwrap()
    .claims;
    assert_eq!(claims.tenantId, f.tenant_id.to_string());
    assert_eq!(claims.roles, vec!["staff"]);
    assert_eq!(claims.allowedTenants[0], f.tenant_id.to_string());
    assert!(claims.permissions.contains(&"finance:read".into()));
    assert!(claims
        .permissions
        .contains(&gaming_cafe_api::access::MANAGED.into()));
    assert!(!claims.permissions.contains(&"access:manage".into()));
    // A broken unrelated profile must not prevent this user's fresh login.
    let collision_name = format!("collision-{manager}");
    repo.create_player(TenantCreatePlayer {
        username: collision_name.clone(),
        password_hash: "local-secret".into(),
        phone_number: "8888888888".into(),
        first_name: None,
        last_name: None,
        actor_id: None,
    })
    .await
    .unwrap();
    sqlx::query("UPDATE users SET username=$2 WHERE id=$1")
        .bind(manager)
        .bind(&collision_name)
        .execute(&pool)
        .await
        .unwrap();
    gaming_cafe_api::control::staff_projection::sync_user(&pool, f.db.clone(), user)
        .await
        .unwrap();
    assert!(sync_tenant(&pool, f.db.clone()).await.is_err());
    auth.issue_tenant_auth_response(f.db.clone(), user)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET username=$2 WHERE id=$1")
        .bind(manager)
        .bind(format!("global-{manager}"))
        .execute(&pool)
        .await
        .unwrap();
    sync_tenant(&pool, f.db.clone()).await.unwrap();

    // One failed queued command must not block another member's revocation/profile update.
    sqlx::query("DELETE FROM staff_projection_changes WHERE tenant_id=$1 AND user_id=$2")
        .bind(f.tenant_id)
        .bind(manager)
        .execute(&pool)
        .await
        .unwrap();
    f.db.with_immediate_writer(move |c|Box::pin(async move {
        sqlx::query("INSERT INTO staff_membership_commands(user_id,desired_active,identity_revision,access_revision) SELECT id,0,identity_revision,access_revision+1 FROM users WHERE id=?")
            .bind(manager.to_string()).execute(c).await?;
        Ok(())
    })).await.unwrap();
    sqlx::query("UPDATE users SET first_name='Healthy projection' WHERE id=$1")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    assert!(sync_tenant(&pool, f.db.clone()).await.is_err());
    assert_eq!(
        repo.require_active_staff(user)
            .await
            .unwrap()
            .first_name
            .as_deref(),
        Some("Healthy projection")
    );
    f.db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("DELETE FROM staff_membership_commands WHERE user_id=?")
                .bind(manager.to_string())
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    sqlx::query(
        "UPDATE organization_memberships SET updated_at=now() WHERE tenant_id=$1 AND user_id=$2",
    )
    .bind(f.tenant_id)
    .bind(manager)
    .execute(&pool)
    .await
    .unwrap();
    sync_tenant(&pool, f.db.clone()).await.unwrap();

    // Authentication finalization forwards a proof token to the owner, never credentials.
    use axum::{
        http::{header, HeaderMap},
        routing::post,
        Json, Router,
    };
    let proof = response.accessToken.clone();
    let payload = serde_json::json!({"data":response});
    let endpoint = move |headers: HeaderMap| {
        let proof = proof.clone();
        let payload = payload.clone();
        async move {
            assert_eq!(
                headers.get(header::AUTHORIZATION).unwrap(),
                format!("Bearer {proof}").as_str()
            );
            assert_eq!(
                headers
                    .get(gaming_cafe_api::routing::ROUTED_HEADER)
                    .unwrap(),
                "1"
            );
            Json(payload)
        }
    };
    let owner = Router::new()
        .route("/auth/refresh", post(endpoint.clone()))
        .route("/auth/staff-shift", post(endpoint.clone()))
        .route("/auth/admin-shift-close", post(endpoint));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, owner).await.unwrap();
    });
    sqlx::query("UPDATE cells SET address=$2 WHERE id=$1")
        .bind(cell)
        .bind(address)
        .execute(&pool)
        .await
        .unwrap();
    let cache = Arc::new(gaming_cafe_api::routing::RoutingCache::new(pool.clone()));
    let remote = gaming_cafe_api::routing::TenantRouter::new(cache.clone(), None).unwrap();
    for shift in [false, true] {
        let finalized = remote
            .finish_auth(f.tenant_id, &response.accessToken, shift)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(finalized.user.id, response.user.id);
        assert_eq!(finalized.accessToken, response.accessToken);
    }
    assert_eq!(
        remote
            .finish_admin_login(f.tenant_id, &response.accessToken)
            .await
            .unwrap()
            .unwrap()
            .accessToken,
        response.accessToken
    );
    let local = gaming_cafe_api::routing::TenantRouter::new(cache, Some(cell)).unwrap();
    assert!(local
        .finish_auth(f.tenant_id, &response.accessToken, false)
        .await
        .unwrap()
        .is_none());
    server.abort();
    access
        .save_member_assignments(user, edit(false, 2), manager)
        .await
        .unwrap();
    sync_tenant(&pool, f.db.clone()).await.unwrap();
    access
        .save_member_assignments(user, edit(true, 3), manager)
        .await
        .unwrap();
    // A newer control-plane revocation cannot be undone by this queued activation.
    sqlx::query(
        "UPDATE organization_memberships SET is_active=false WHERE tenant_id=$1 AND user_id=$2",
    )
    .bind(f.tenant_id)
    .bind(user)
    .execute(&pool)
    .await
    .unwrap();
    sync_tenant(&pool, f.db.clone()).await.unwrap();
    assert!(repo.require_active_staff(user).await.is_err());
    let conflicts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM outbox_events WHERE event_type='access.membership_conflict'",
    )
    .fetch_one(&f.db.read_pool().unwrap())
    .await
    .unwrap();
    assert_eq!(conflicts, 1);
    access
        .save_member_assignments(user, edit(true, 4), manager)
        .await
        .unwrap();
    sync_tenant(&pool, f.db.clone()).await.unwrap();
    assert!(repo.require_active_staff(user).await.is_ok());
    let identities = gaming_cafe_api::control::identity::IdentityRepository::new(pool.clone());
    let setup = identities.setup_totp(f.tenant_id, user).await.unwrap();
    let state: (bool, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT totp_enabled,totp_secret,totp_pending_secret FROM users WHERE id=$1",
    )
    .bind(user)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(state, (false, None, Some(setup.secret.clone())));
    assert!(identities
        .verify_totp_setup(f.tenant_id, user, "invalid")
        .await
        .is_err());
    let generator = totp_rs::TOTP::new(
        totp_rs::Algorithm::SHA1,
        6,
        1,
        30,
        totp_rs::Secret::Encoded(setup.secret).to_bytes().unwrap(),
        Some("GameZone".into()),
        format!("global-{user}"),
    )
    .unwrap();
    identities
        .verify_totp_setup(f.tenant_id, user, &generator.generate_current().unwrap())
        .await
        .unwrap();
    sync_tenant(&pool, f.db.clone()).await.unwrap();
    let enabled: bool = sqlx::query_scalar("SELECT totp_enabled FROM users WHERE id=$1")
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(enabled);
    identities.disable_totp(f.tenant_id, user).await.unwrap();
    let created = identities
        .create_disabled_staff(
            f.tenant_id,
            manager,
            &format!("new-{user}"),
            "staff creation password",
        )
        .await
        .unwrap();
    assert_eq!(
        identities
            .create_disabled_staff(
                f.tenant_id,
                manager,
                &format!("new-{user}"),
                "staff creation password"
            )
            .await
            .unwrap(),
        created
    );
    assert!(identities
        .create_disabled_staff(
            f.tenant_id,
            user,
            &format!("new-{user}"),
            "staff creation password"
        )
        .await
        .is_err());
    sync_tenant(&pool, f.db.clone()).await.unwrap();
    assert!(repo.require_active_staff(created).await.is_err());
    access
        .save_member_assignments(created, edit(true, 0), manager)
        .await
        .unwrap();
    sync_tenant(&pool, f.db.clone()).await.unwrap();
    assert!(repo.require_active_staff(created).await.is_ok());
    let pending: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pending_staff_creations WHERE user_id=$1")
            .bind(created)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(pending, 0);
    sqlx::query("UPDATE tenants SET state='FAILED' WHERE id=$1")
        .bind(f.tenant_id)
        .execute(&pool)
        .await
        .unwrap();
    sync_tenant(&pool, f.db.clone()).await.unwrap();
    assert!(repo.require_active_staff(user).await.is_err());
    assert!(repo.require_active_staff(created).await.is_err());
    sqlx::query("UPDATE tenants SET state='ACTIVE' WHERE id=$1")
        .bind(f.tenant_id)
        .execute(&pool)
        .await
        .unwrap();
    sync_tenant(&pool, f.db.clone()).await.unwrap();
    assert!(repo.require_active_staff(user).await.is_ok());
    sqlx::query("DELETE FROM organization_memberships WHERE tenant_id=$1 AND user_id=$2")
        .bind(f.tenant_id)
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    sync_tenant(&pool, f.db.clone()).await.unwrap();
    assert!(repo.find_by_id(user).await.unwrap().is_none());
    sqlx::query("DELETE FROM tenants WHERE id=$1")
        .bind(f.tenant_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM cells WHERE id=$1")
        .bind(cell)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM tenants WHERE id=$1")
        .bind(other)
        .execute(&pool)
        .await
        .unwrap();
    for id in [user, manager, created] {
        sqlx::query("DELETE FROM users WHERE id=$1")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
    }
    pool.close().await;
    f.close().await;
}

#[tokio::test]
async fn identity_refresh_preserves_local_grants_and_rejects_stale_reactivation() {
    let f = Fixture::new().await;
    let repo = TenantUserRepository::new(f.db.clone());
    let mut p = projection(
        f.staff_id,
        "identity-staff",
        "staff",
        1,
        true,
        false,
        vec![TenantLocationRoleGrant {
            system_key: "finance".into(),
            location_id: f.location_a,
        }],
    );
    p.global_access_role_system_keys = vec!["finance".into()];
    repo.project_staff(p.clone()).await.unwrap();
    let before = counts(&f).await;
    p.first_name = Some("Changed".into());
    assert_eq!(
        repo.project_identity(p.clone(), 10).await.unwrap(),
        StaffProjectionResult::Applied
    );
    let after = counts(&f).await;
    assert_eq!((before.1, before.2), (after.1, after.2));
    let role: String = sqlx::query_scalar("SELECT r.system_key FROM access_assignments a JOIN access_roles r ON r.id=a.role_id WHERE a.user_id=?")
        .bind(f.staff_id.to_string()).fetch_one(&f.db.read_pool().unwrap()).await.unwrap();
    assert_eq!(role, "finance");
    assert!(repo
        .find_by_id(f.staff_id)
        .await
        .unwrap()
        .unwrap()
        .password_hash
        .is_none());
    repo.create_player(TenantCreatePlayer {
        username: "colliding-player".into(),
        password_hash: hash("player password", 4).unwrap(),
        phone_number: "7000000000".into(),
        first_name: None,
        last_name: None,
        actor_id: None,
    })
    .await
    .unwrap();
    let previous_username = p.username.clone();
    p.username = "colliding-player".into();
    p.is_active = false;
    repo.project_identity(p.clone(), 11).await.unwrap();
    assert!(repo.require_active_staff(f.staff_id).await.is_err());
    p.username = previous_username;
    p.is_active = true;
    assert_eq!(
        repo.project_identity(p.clone(), 10).await.unwrap(),
        StaffProjectionResult::Unchanged
    );
    assert!(repo.require_active_staff(f.staff_id).await.is_err());
    repo.project_identity(p.clone(), 12).await.unwrap();
    assert!(repo.require_active_staff(f.staff_id).await.is_ok());
    let before = counts(&f).await;
    f.lease.fail_next_commit();
    p.first_name = Some("Must rollback".into());
    assert!(repo.project_identity(p, 13).await.is_err());
    assert_eq!(counts(&f).await, before);
    assert_eq!(
        repo.find_by_id(f.staff_id)
            .await
            .unwrap()
            .unwrap()
            .first_name
            .as_deref(),
        Some("Changed")
    );
    f.close().await;
}

#[tokio::test]
async fn players_are_isolated_and_support_safe_lifecycle() {
    let first = Fixture::new().await;
    let second = Fixture::new().await;
    let first_repo = TenantUserRepository::new(first.db.clone());
    let second_repo = TenantUserRepository::new(second.db.clone());
    let password_hash = hash("correct horse", DEFAULT_COST).unwrap();
    let player = first_repo
        .create_player(TenantCreatePlayer {
            username: "CasePlayer".into(),
            password_hash,
            phone_number: "9999999999".into(),
            first_name: Some("Case".into()),
            last_name: Some("Player".into()),
            actor_id: None,
        })
        .await
        .unwrap();

    assert!(second_repo.find_by_id(player.id).await.unwrap().is_none());
    assert_eq!(
        first_repo
            .list(&UserFilterDto {
                username: Some("player".into()),
                sort_by: Some("username".into()),
                sort_order: Some("ASC".into()),
                ..Default::default()
            })
            .await
            .unwrap()
            .total,
        1
    );
    assert!(first_repo
        .find_by_id(player.id)
        .await
        .unwrap()
        .unwrap()
        .password_hash
        .is_none());
    assert!(first_repo
        .find_player_for_auth("caseplayer")
        .await
        .unwrap()
        .unwrap()
        .password_hash
        .is_some());

    let updated = first_repo
        .update(
            player.id,
            &UpdateUserDto {
                username: Some("RenamedPlayer".into()),
                phone_number: Some("8888888888".into()),
                first_name: Some("Renamed".into()),
                last_name: None,
                role: Some("admin".into()),
                is_active: Some(true),
            },
            None,
        )
        .await
        .unwrap();
    assert_eq!(updated.role.as_deref(), Some("player"));
    first_repo
        .set_avatar(player.id, Some("https://cdn.invalid/avatar.png"))
        .await
        .unwrap();
    let replacement = hash("new password", DEFAULT_COST).unwrap();
    first_repo
        .update_password(player.id, &replacement)
        .await
        .unwrap();
    let auth = first_repo
        .find_player_for_auth("renamedplayer")
        .await
        .unwrap()
        .unwrap();
    assert!(verify("new password", auth.password_hash.as_deref().unwrap()).unwrap());

    first_repo.soft_delete(player.id).await.unwrap();
    assert!(first_repo.find_by_id(player.id).await.unwrap().is_none());
    assert!(first_repo
        .find_player_for_auth("renamedplayer")
        .await
        .unwrap()
        .is_none());
    first_repo
        .create_player(TenantCreatePlayer {
            username: "renamedplayer".into(),
            password_hash: hash("another password", DEFAULT_COST).unwrap(),
            phone_number: "7777777777".into(),
            first_name: None,
            last_name: None,
            actor_id: None,
        })
        .await
        .unwrap();

    first.close().await;
    second.close().await;
}

#[tokio::test]
async fn live_names_conflict_and_require_methods_enforce_role_and_state() {
    let fixture = Fixture::new().await;
    let repo = TenantUserRepository::new(fixture.db.clone());
    let staff = projection(
        fixture.staff_id,
        "SharedName",
        "staff",
        1,
        true,
        false,
        vec![],
    );
    repo.project_staff(staff).await.unwrap();
    let result = repo
        .create_player(TenantCreatePlayer {
            username: "sharedname".into(),
            password_hash: hash("password 123", DEFAULT_COST).unwrap(),
            phone_number: "9999999999".into(),
            first_name: None,
            last_name: None,
            actor_id: None,
        })
        .await;
    assert!(matches!(result, Err(AppError::Conflict(_))));
    assert!(repo.require_active_staff(fixture.staff_id).await.is_ok());
    assert!(repo.require_active_player(fixture.staff_id).await.is_err());

    repo.project_staff(projection(
        fixture.staff_id,
        "SharedName",
        "staff",
        2,
        false,
        false,
        vec![],
    ))
    .await
    .unwrap();
    assert!(repo.require_active_staff(fixture.staff_id).await.is_err());
    fixture.close().await;
}

#[tokio::test]
async fn staff_projection_never_reuses_a_soft_deleted_player_id() {
    let fixture = Fixture::new().await;
    let repo = TenantUserRepository::new(fixture.db.clone());
    let player = repo
        .create_player(TenantCreatePlayer {
            username: "deleted-player-id".into(),
            password_hash: hash("password 123", DEFAULT_COST).unwrap(),
            phone_number: "9999999999".into(),
            first_name: None,
            last_name: None,
            actor_id: None,
        })
        .await
        .unwrap();
    repo.soft_delete(player.id).await.unwrap();
    let before = counts(&fixture).await;
    let result = repo
        .project_staff(projection(
            player.id,
            "replacement-staff",
            "staff",
            2,
            true,
            false,
            vec![],
        ))
        .await;
    assert!(matches!(result, Err(AppError::Conflict(_))));
    assert_eq!(counts(&fixture).await, before);
    fixture.close().await;
}

#[tokio::test]
async fn staff_projection_is_revisioned_atomic_and_exactly_scoped() {
    let fixture = Fixture::new().await;
    let repo = TenantUserRepository::new(fixture.db.clone());
    let initial = projection(
        fixture.staff_id,
        "operator",
        "staff",
        1,
        true,
        false,
        vec![TenantLocationRoleGrant {
            system_key: "staff".into(),
            location_id: fixture.location_a,
        }],
    );
    assert_eq!(
        repo.project_staff(initial.clone()).await.unwrap(),
        StaffProjectionResult::Applied
    );
    let after_first = counts(&fixture).await;
    assert_eq!(
        repo.project_staff(TenantStaffProjection {
            global_access_role_system_keys: vec!["staff".into(), "staff".into()],
            location_grants: vec![
                TenantLocationRoleGrant {
                    system_key: "staff".into(),
                    location_id: fixture.location_a,
                },
                TenantLocationRoleGrant {
                    system_key: "staff".into(),
                    location_id: fixture.location_a,
                },
            ],
            ..initial.clone()
        })
        .await
        .unwrap(),
        StaffProjectionResult::Unchanged
    );
    assert_eq!(counts(&fixture).await, after_first);
    assert!(matches!(
        repo.project_staff(TenantStaffProjection {
            member_revision: 0,
            ..initial.clone()
        })
        .await,
        Err(AppError::BadRequest(_))
    ));
    assert_eq!(
        repo.location_ids_for_permission(fixture.staff_id, "sessions:write")
            .await
            .unwrap(),
        vec![fixture.location_a]
    );
    assert!(repo
        .location_ids_for_permission(fixture.staff_id, "finance:read")
        .await
        .unwrap()
        .is_empty());

    let replaced = TenantStaffProjection {
        member_revision: 2,
        first_name: Some("Updated".into()),
        global_access_role_system_keys: vec!["finance".into()],
        location_grants: vec![TenantLocationRoleGrant {
            system_key: "finance".into(),
            location_id: fixture.location_b,
        }],
        ..initial
    };
    repo.project_staff(replaced).await.unwrap();
    assert_eq!(
        repo.location_ids_for_permission(fixture.staff_id, "finance:read")
            .await
            .unwrap(),
        vec![fixture.location_b]
    );
    assert!(repo
        .location_ids_for_permission(fixture.staff_id, "sessions:write")
        .await
        .unwrap()
        .is_empty());

    let before = counts(&fixture).await;
    let unknown = repo
        .project_staff(projection(
            fixture.staff_id,
            "operator",
            "staff",
            3,
            true,
            false,
            vec![TenantLocationRoleGrant {
                system_key: "unknown-custom-role".into(),
                location_id: fixture.location_a,
            }],
        ))
        .await;
    assert!(matches!(unknown, Err(AppError::Conflict(_))));
    assert_eq!(counts(&fixture).await, before);
    fixture.close().await;
}

#[tokio::test]
async fn projection_rejects_non_global_or_template_location_roles_atomically() {
    let fixture = Fixture::new().await;
    let repo = TenantUserRepository::new(fixture.db.clone());
    let before = counts(&fixture).await;
    let subset = repo
        .project_staff(TenantStaffProjection {
            global_access_role_system_keys: vec!["staff".into()],
            location_grants: vec![TenantLocationRoleGrant {
                system_key: "finance".into(),
                location_id: fixture.location_a,
            }],
            ..projection(
                fixture.staff_id,
                "subset-staff",
                "staff",
                1,
                true,
                false,
                vec![],
            )
        })
        .await;
    assert!(matches!(subset, Err(AppError::Conflict(_))));
    assert_eq!(counts(&fixture).await, before);

    let template = repo
        .project_staff(TenantStaffProjection {
            global_access_role_system_keys: vec!["template-only".into()],
            ..projection(
                fixture.staff_id,
                "template-staff",
                "staff",
                1,
                true,
                false,
                vec![],
            )
        })
        .await;
    assert!(matches!(template, Err(AppError::Conflict(_))));
    assert_eq!(counts(&fixture).await, before);
    fixture.close().await;
}

#[tokio::test]
async fn equal_revision_requires_exact_canonical_projection_state() {
    let fixture = Fixture::new().await;
    let repo = TenantUserRepository::new(fixture.db.clone());
    let initial = projection(
        fixture.staff_id,
        "revision-staff",
        "staff",
        1,
        true,
        false,
        vec![TenantLocationRoleGrant {
            system_key: "staff".into(),
            location_id: fixture.location_a,
        }],
    );
    repo.project_staff(initial.clone()).await.unwrap();
    let before = counts(&fixture).await;
    let divergent_profile = repo
        .project_staff(TenantStaffProjection {
            first_name: Some("Divergent".into()),
            ..initial.clone()
        })
        .await;
    assert!(matches!(divergent_profile, Err(AppError::Conflict(_))));
    assert_eq!(counts(&fixture).await, before);
    assert_eq!(
        repo.find_by_id(fixture.staff_id)
            .await
            .unwrap()
            .unwrap()
            .first_name
            .as_deref(),
        Some("Projected")
    );

    let divergent_assignments = repo
        .project_staff(TenantStaffProjection {
            global_access_role_system_keys: vec!["staff".into(), "finance".into()],
            ..initial
        })
        .await;
    assert!(matches!(divergent_assignments, Err(AppError::Conflict(_))));
    assert_eq!(counts(&fixture).await, before);
    fixture.close().await;
}

#[tokio::test]
async fn admin_visibility_and_inactive_or_deleted_staff_access_are_safe() {
    let fixture = Fixture::new().await;
    let repo = TenantUserRepository::new(fixture.db.clone());
    let admin_id = Uuid::now_v7();
    repo.project_staff(projection(
        admin_id,
        "administrator",
        "admin",
        1,
        true,
        false,
        vec![],
    ))
    .await
    .unwrap();
    assert_eq!(
        repo.location_ids_for_permission(admin_id, "anything:read")
            .await
            .unwrap(),
        vec![fixture.location_a, fixture.location_b]
    );

    repo.project_staff(projection(
        fixture.staff_id,
        "operator",
        "staff",
        1,
        true,
        false,
        vec![TenantLocationRoleGrant {
            system_key: "staff".into(),
            location_id: fixture.location_a,
        }],
    ))
    .await
    .unwrap();
    repo.project_staff(projection(
        fixture.staff_id,
        "operator",
        "staff",
        2,
        true,
        true,
        vec![],
    ))
    .await
    .unwrap();
    assert!(repo
        .location_ids_for_permission(fixture.staff_id, "sessions:write")
        .await
        .unwrap()
        .is_empty());
    let event: (bool, String) = sqlx::query_as(
        "SELECT deleted,payload FROM outbox_events WHERE aggregate_id=? \
         ORDER BY sequence DESC LIMIT 1",
    )
    .bind(fixture.staff_id.to_string())
    .fetch_one(&fixture.db.read_pool().unwrap())
    .await
    .unwrap();
    assert!(event.0);
    let payload: Value = serde_json::from_str(&event.1).unwrap();
    assert_eq!(payload["deleted"], true);
    fixture.close().await;
}

#[tokio::test]
async fn higher_revision_revocation_ignores_stale_grants_and_profile_collisions() {
    let fixture = Fixture::new().await;
    let repo = TenantUserRepository::new(fixture.db.clone());
    repo.project_staff(projection(
        fixture.staff_id,
        "revoked-staff",
        "staff",
        1,
        true,
        false,
        vec![TenantLocationRoleGrant {
            system_key: "staff".into(),
            location_id: fixture.location_a,
        }],
    ))
    .await
    .unwrap();
    repo.create_player(TenantCreatePlayer {
        username: "profile-collision".into(),
        password_hash: hash("password 123", DEFAULT_COST).unwrap(),
        phone_number: "9999999999".into(),
        first_name: None,
        last_name: None,
        actor_id: None,
    })
    .await
    .unwrap();
    let location_a = fixture.location_a;
    fixture
        .db
        .with_immediate_writer(move |connection| -> BoxFuture<'_, Result<(), AppError>> {
            Box::pin(async move {
                sqlx::query("UPDATE venue_locations SET is_active=0 WHERE id=?")
                    .bind(location_a.to_string())
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .await
        .unwrap();

    let before_deactivate = counts(&fixture).await;
    repo.project_staff(TenantStaffProjection {
        username: "profile-collision".into(),
        member_revision: 2,
        is_active: false,
        global_access_role_system_keys: vec!["stale-role".into()],
        location_grants: vec![TenantLocationRoleGrant {
            system_key: "other-stale-role".into(),
            location_id: Uuid::now_v7(),
        }],
        ..projection(
            fixture.staff_id,
            "ignored",
            "staff",
            2,
            false,
            false,
            vec![],
        )
    })
    .await
    .unwrap();
    let after_deactivate = counts(&fixture).await;
    assert_eq!(after_deactivate.1, 0);
    assert_eq!(after_deactivate.2, 0);
    assert_eq!(after_deactivate.3, before_deactivate.3 + 1);
    let state: (String, bool, i64, Option<String>) = sqlx::query_as(
        "SELECT username,is_active,member_revision,deleted_at FROM users WHERE id=?",
    )
    .bind(fixture.staff_id.to_string())
    .fetch_one(&fixture.db.read_pool().unwrap())
    .await
    .unwrap();
    assert_eq!(state, ("revoked-staff".into(), false, 2, None));
    let deactivated_counts = counts(&fixture).await;
    assert_eq!(
        repo.project_staff(TenantStaffProjection {
            username: "different-stale-profile".into(),
            first_name: Some("Ignored".into()),
            permissions: vec!["ignored:permission".into()],
            member_revision: 2,
            is_active: false,
            global_access_role_system_keys: vec!["another-stale-role".into()],
            location_grants: vec![TenantLocationRoleGrant {
                system_key: "another-stale-role".into(),
                location_id: Uuid::now_v7(),
            }],
            ..projection(
                fixture.staff_id,
                "ignored",
                "staff",
                2,
                false,
                false,
                vec![],
            )
        })
        .await
        .unwrap(),
        StaffProjectionResult::Unchanged
    );
    assert_eq!(counts(&fixture).await, deactivated_counts);

    repo.project_staff(TenantStaffProjection {
        member_revision: 3,
        global_access_role_system_keys: vec!["finance".into()],
        location_grants: vec![TenantLocationRoleGrant {
            system_key: "finance".into(),
            location_id: fixture.location_b,
        }],
        ..projection(
            fixture.staff_id,
            "revoked-staff",
            "staff",
            3,
            true,
            false,
            vec![],
        )
    })
    .await
    .unwrap();
    let before_delete = counts(&fixture).await;
    repo.project_staff(TenantStaffProjection {
        username: "profile-collision".into(),
        member_revision: 4,
        global_access_role_system_keys: vec!["missing-role".into()],
        location_grants: vec![TenantLocationRoleGrant {
            system_key: "missing-location-role".into(),
            location_id: Uuid::now_v7(),
        }],
        ..projection(fixture.staff_id, "ignored", "staff", 4, true, true, vec![])
    })
    .await
    .unwrap();
    let after_delete = counts(&fixture).await;
    assert_eq!(after_delete.1, 0);
    assert_eq!(after_delete.2, 0);
    assert_eq!(after_delete.3, before_delete.3 + 1);
    let deleted: (bool, i64, bool) = sqlx::query_as(
        "SELECT is_active,member_revision,deleted_at IS NOT NULL FROM users WHERE id=?",
    )
    .bind(fixture.staff_id.to_string())
    .fetch_one(&fixture.db.read_pool().unwrap())
    .await
    .unwrap();
    assert_eq!(deleted, (false, 4, true));
    let deleted_counts = counts(&fixture).await;
    assert_eq!(
        repo.project_staff(TenantStaffProjection {
            username: "different-deleted-profile".into(),
            last_name: Some("Ignored".into()),
            permissions: vec!["ignored:deleted".into()],
            member_revision: 4,
            deleted: true,
            global_access_role_system_keys: vec!["deleted-stale-role".into()],
            location_grants: vec![TenantLocationRoleGrant {
                system_key: "other-deleted-stale-role".into(),
                location_id: Uuid::now_v7(),
            }],
            ..projection(fixture.staff_id, "ignored", "admin", 4, true, true, vec![],)
        })
        .await
        .unwrap(),
        StaffProjectionResult::Unchanged
    );
    assert_eq!(counts(&fixture).await, deleted_counts);
    fixture.close().await;
}

#[tokio::test]
async fn first_seen_revocations_ignore_stale_access_payloads_and_fence_older_activation() {
    let fixture = Fixture::new().await;
    let repo = TenantUserRepository::new(fixture.db.clone());
    repo.create_player(TenantCreatePlayer {
        username: "live-name-reused-by-tombstone".into(),
        password_hash: hash("password 123", DEFAULT_COST).unwrap(),
        phone_number: "9999999999".into(),
        first_name: None,
        last_name: None,
        actor_id: None,
    })
    .await
    .unwrap();

    let deleted_id = Uuid::now_v7();
    let before_deleted = counts(&fixture).await;
    let deleted = TenantStaffProjection {
        global_access_role_system_keys: vec!["unknown-template-or-stale".into()],
        location_grants: vec![TenantLocationRoleGrant {
            system_key: "different-unknown-role".into(),
            location_id: Uuid::now_v7(),
        }],
        ..projection(
            deleted_id,
            "live-name-reused-by-tombstone",
            "staff",
            5,
            true,
            true,
            vec![],
        )
    };
    assert_eq!(
        repo.project_staff(deleted.clone()).await.unwrap(),
        StaffProjectionResult::Applied
    );
    let after_deleted = counts(&fixture).await;
    assert_eq!(after_deleted.0, before_deleted.0 + 1);
    assert_eq!(after_deleted.1, 0);
    assert_eq!(after_deleted.2, 0);
    assert_eq!(after_deleted.3, before_deleted.3 + 1);
    let deleted_state: (bool, i64, bool) = sqlx::query_as(
        "SELECT is_active,member_revision,deleted_at IS NOT NULL FROM users WHERE id=?",
    )
    .bind(deleted_id.to_string())
    .fetch_one(&fixture.db.read_pool().unwrap())
    .await
    .unwrap();
    assert_eq!(deleted_state, (false, 5, true));
    assert!(matches!(
        repo.project_staff(TenantStaffProjection {
            member_revision: 4,
            is_active: true,
            deleted: false,
            global_access_role_system_keys: vec!["staff".into()],
            location_grants: vec![],
            ..deleted.clone()
        })
        .await,
        Err(AppError::Conflict(_))
    ));
    let deleted_replay_counts = counts(&fixture).await;
    assert_eq!(
        repo.project_staff(TenantStaffProjection {
            username: "ignored-deleted-replay".into(),
            first_name: Some("Changed".into()),
            permissions: vec!["changed:permission".into()],
            ..deleted
        })
        .await
        .unwrap(),
        StaffProjectionResult::Unchanged
    );
    assert_eq!(counts(&fixture).await, deleted_replay_counts);

    let inactive_id = Uuid::now_v7();
    let before_inactive = counts(&fixture).await;
    let inactive = TenantStaffProjection {
        global_access_role_system_keys: vec!["template-only".into(), "missing-role".into()],
        location_grants: vec![TenantLocationRoleGrant {
            system_key: "missing-location-role".into(),
            location_id: Uuid::now_v7(),
        }],
        ..projection(
            inactive_id,
            "first-seen-inactive",
            "admin",
            7,
            false,
            false,
            vec![],
        )
    };
    assert_eq!(
        repo.project_staff(inactive.clone()).await.unwrap(),
        StaffProjectionResult::Applied
    );
    let after_inactive = counts(&fixture).await;
    assert_eq!(after_inactive.0, before_inactive.0 + 1);
    assert_eq!(after_inactive.1, 0);
    assert_eq!(after_inactive.2, 0);
    assert_eq!(after_inactive.3, before_inactive.3 + 1);
    let inactive_state: (bool, i64, Option<String>) =
        sqlx::query_as("SELECT is_active,member_revision,deleted_at FROM users WHERE id=?")
            .bind(inactive_id.to_string())
            .fetch_one(&fixture.db.read_pool().unwrap())
            .await
            .unwrap();
    assert_eq!(inactive_state, (false, 7, None));
    assert!(matches!(
        repo.project_staff(TenantStaffProjection {
            member_revision: 6,
            is_active: true,
            global_access_role_system_keys: vec!["admin".into()],
            location_grants: vec![],
            ..inactive.clone()
        })
        .await,
        Err(AppError::Conflict(_))
    ));
    let inactive_replay_counts = counts(&fixture).await;
    assert_eq!(
        repo.project_staff(TenantStaffProjection {
            username: "ignored-inactive-replay".into(),
            role: "staff".into(),
            permissions: vec!["changed:inactive".into()],
            ..inactive
        })
        .await
        .unwrap(),
        StaffProjectionResult::Unchanged
    );
    assert_eq!(counts(&fixture).await, inactive_replay_counts);
    fixture.close().await;
}

#[tokio::test]
async fn user_outbox_is_secret_free_and_lease_fencing_rolls_back_everything() {
    let fixture = Fixture::new().await;
    let repo = TenantUserRepository::new(fixture.db.clone());
    let player = repo
        .create_player(TenantCreatePlayer {
            username: "safe-player".into(),
            password_hash: hash("password 123", DEFAULT_COST).unwrap(),
            phone_number: "9999999999".into(),
            first_name: Some("Safe".into()),
            last_name: None,
            actor_id: None,
        })
        .await
        .unwrap();
    let payload: String = sqlx::query_scalar(
        "SELECT payload FROM outbox_events WHERE aggregate_id=? ORDER BY sequence DESC LIMIT 1",
    )
    .bind(player.id.to_string())
    .fetch_one(&fixture.db.read_pool().unwrap())
    .await
    .unwrap();
    for forbidden in [
        "password",
        "email",
        "phone",
        "permissions",
        "secret",
        "token",
        "otp",
    ] {
        assert!(!payload.to_ascii_lowercase().contains(forbidden));
    }

    let before = counts(&fixture).await;
    fixture.lease.fail_next_commit();
    let result = repo
        .project_staff(projection(
            fixture.staff_id,
            "fenced-staff",
            "staff",
            1,
            true,
            false,
            vec![TenantLocationRoleGrant {
                system_key: "staff".into(),
                location_id: fixture.location_a,
            }],
        ))
        .await;
    assert!(matches!(result, Err(AppError::Forbidden(_))));
    assert_eq!(counts(&fixture).await, before);
    fixture.close().await;
}

#[tokio::test]
async fn tenant_services_need_only_sqlite_and_reject_staff_player_plans() {
    let fixture = Fixture::new().await;
    let users = UserService::new();
    let registered = users
        .register_from_kiosk_tenant(
            fixture.db.clone(),
            KioskRegisterDto {
                username: "service-player".into(),
                password: "password 123".into(),
                phoneNumber: "9999999999".into(),
                firstName: None,
                lastName: None,
            },
        )
        .await
        .unwrap();
    assert!(TenantUserRepository::new(fixture.db.clone())
        .find_player_for_auth(&registered.username)
        .await
        .unwrap()
        .is_some());
    let registered_id = Uuid::parse_str(&registered.userId).unwrap();
    users
        .update_tenant(
            fixture.db.clone(),
            registered_id,
            UpdateUserDto {
                username: None,
                phone_number: None,
                first_name: Some("Updated".into()),
                last_name: None,
                role: Some("player".into()),
                is_active: None,
            },
            None,
        )
        .await
        .unwrap();
    let invalid_role = users
        .update_tenant(
            fixture.db.clone(),
            registered_id,
            UpdateUserDto {
                username: None,
                phone_number: None,
                first_name: None,
                last_name: None,
                role: Some("staff".into()),
                is_active: None,
            },
            None,
        )
        .await;
    assert!(matches!(invalid_role, Err(AppError::BadRequest(_))));

    TenantUserRepository::new(fixture.db.clone())
        .project_staff(projection(
            fixture.staff_id,
            "plan-staff",
            "staff",
            1,
            true,
            false,
            vec![],
        ))
        .await
        .unwrap();
    let plan_id = Uuid::now_v7();
    let at = gaming_cafe_api::time::format_sqlite_timestamp(&Utc::now()).unwrap();
    fixture
        .db
        .with_immediate_writer(move |connection| -> BoxFuture<'_, Result<(), AppError>> {
            Box::pin(async move {
                sqlx::query(
                    "INSERT INTO plans(id,name,price,plan_type,validity_days,time_credits,is_active,\
                     created_at,updated_at) VALUES(?, 'Test plan', 10000, 'time_based', 30, 60, 1, ?, ?)",
                )
                .bind(plan_id.to_string())
                .bind(&at)
                .bind(&at)
                .execute(connection)
                .await?;
                Ok(())
            })
        })
        .await
        .unwrap();
    let result = PlayerPlanService::new()
        .assign_plan_to_player_tenant(
            fixture.db.clone(),
            AssignPlanDto {
                player_id: fixture.staff_id,
                plan_id,
                transaction_id: None,
                purchase_date: None,
            },
            None,
        )
        .await;
    assert!(matches!(result, Err(AppError::NotFound(_))));
    fixture.close().await;
}

#[tokio::test]
async fn owning_cell_realtime_websocket_delivers_acks_and_replays_without_operational_postgres() {
    use axum::{
        extract::{ws::WebSocketUpgrade, State},
        response::IntoResponse,
        routing::get,
        Router,
    };
    use futures::{SinkExt, StreamExt};
    use gaming_cafe_api::{
        metrics::Metrics,
        proto::arena360::v1 as pb,
        realtime::{registry::ConnectionRegistry, Dispatcher, RealtimeHub},
        tenancy::{write_outbox_event_on_connection, NewOutboxEvent},
    };
    use prost::Message;
    use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message as WsMessage};
    type WsState = (
        Arc<TenantDb>,
        gaming_cafe_api::dto::JwtUserClaims,
        Arc<ConnectionRegistry>,
        Arc<Metrics>,
    );
    async fn upgrade(
        State((db, claims, registry, metrics)): State<WsState>,
        ws: WebSocketUpgrade,
    ) -> axum::response::Response {
        ws.protocols(["arena360.protobuf.v1"])
            .on_upgrade(move |socket| {
                gaming_cafe_api::realtime::connection::run(socket, claims, db, registry, metrics)
            })
            .into_response()
    }
    async fn receive(
        socket: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) -> pb::server_frame::Frame {
        let message = tokio::time::timeout(Duration::from_secs(5), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let WsMessage::Binary(bytes) = message else {
            panic!("Expected binary server frame: {message:?}");
        };
        pb::ServerFrame::decode(bytes).unwrap().frame.unwrap()
    }
    async fn commit_event(db: Arc<TenantDb>, location: Uuid) {
        db.with_immediate_writer(move |c| {
            Box::pin(async move {
                write_outbox_event_on_connection(
                    c,
                    NewOutboxEvent {
                        aggregate_type: "session".into(),
                        aggregate_id: Uuid::now_v7(),
                        event_type: "session.started".into(),
                        payload: serde_json::json!({"id":Uuid::now_v7(),"deviceId":Uuid::now_v7()}),
                        location_id: Some(location),
                        schema_version: 1,
                        deleted: false,
                    },
                )
                .await?;
                Ok(())
            })
        })
        .await
        .unwrap();
    }
    let f = Fixture::new().await;
    TenantUserRepository::new(f.db.clone())
        .project_staff(projection(
            f.staff_id,
            "websocket-admin",
            "admin",
            1,
            true,
            false,
            vec![],
        ))
        .await
        .unwrap();
    f.db.with_immediate_writer(|c|Box::pin(async move {
        sqlx::query("UPDATE access_roles SET permissions='[\"events:staff\",\"access:manage\"]' WHERE system_key='admin'").execute(c).await?;Ok(())
    })).await.unwrap();
    let claims:gaming_cafe_api::dto::JwtUserClaims=serde_json::from_value(serde_json::json!({
        "sub":f.staff_id,"userId":f.staff_id,"tenantId":f.tenant_id,"allowedTenants":[f.tenant_id],"orgIds":[f.tenant_id],
        "roles":["admin"],"permissions":[],"iss":"gamezone","aud":"gamezone","appId":"test","exp":Utc::now().timestamp()+300
    })).unwrap();
    let registry = Arc::new(ConnectionRegistry::default());
    let metrics = Arc::new(Metrics::default());
    let dispatcher = tokio::spawn(
        Dispatcher::new(
            registry.clone(),
            RealtimeHub::new(8),
            Some(f.manager.clone()),
            metrics.clone(),
        )
        .run(),
    );
    let app = Router::new().route("/realtime", get(upgrade)).with_state((
        f.db.clone(),
        claims,
        registry,
        metrics,
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/realtime", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let mut request = url.clone().into_client_request().unwrap();
    request.headers_mut().insert(
        "sec-websocket-protocol",
        "arena360.protobuf.v1".parse().unwrap(),
    );
    let (mut socket, _) = tokio_tungstenite::connect_async(request.clone())
        .await
        .unwrap();
    assert!(matches!(
        receive(&mut socket).await,
        pb::server_frame::Frame::Welcome(_)
    ));
    let subscribe = pb::ClientFrame {
        frame: Some(pb::client_frame::Frame::Subscribe(pb::Subscribe {
            channels: vec!["staff".into()],
        })),
    }
    .encode_to_vec();
    socket
        .send(WsMessage::Binary(subscribe.clone().into()))
        .await
        .unwrap();
    assert!(matches!(
        receive(&mut socket).await,
        pb::server_frame::Frame::Subscribed(_)
    ));
    commit_event(f.db.clone(), f.location_a).await;
    let pb::server_frame::Frame::Event(first) = receive(&mut socket).await else {
        panic!("Expected live event");
    };
    assert_eq!(first.channel, "staff");
    let ack = pb::ClientFrame {
        frame: Some(pb::client_frame::Frame::Ack(pb::Ack {
            msg_id: first.msg_id,
        })),
    }
    .encode_to_vec();
    socket.send(WsMessage::Binary(ack.into())).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2),async {
        loop {
            let acked:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM realtime_deliveries WHERE outbox_id=? AND ack_at IS NOT NULL)").bind(first.msg_id).fetch_one(&f.db.read_pool().unwrap()).await.unwrap();
            if acked {break;} tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    // An authorization read failure is retryable; it must not discard the durable event.
    let location = f.location_a;
    f.db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("ALTER TABLE access_roles RENAME TO temporarily_unavailable_roles")
                .execute(&mut *c)
                .await?;
            write_outbox_event_on_connection(
                c,
                NewOutboxEvent {
                    aggregate_type: "session".into(),
                    aggregate_id: Uuid::now_v7(),
                    event_type: "session.started".into(),
                    payload: serde_json::json!({"id":Uuid::now_v7(),"deviceId":Uuid::now_v7()}),
                    location_id: Some(location),
                    schema_version: 1,
                    deleted: false,
                },
            )
            .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(1500), socket.next())
            .await
            .is_err()
    );
    let pending: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM realtime_outbox WHERE channel='staff' AND dispatched_at IS NULL",
    )
    .fetch_one(&f.db.read_pool().unwrap())
    .await
    .unwrap();
    assert!(pending > 0);
    f.db.with_immediate_writer(|c| {
        Box::pin(async move {
            sqlx::query("ALTER TABLE temporarily_unavailable_roles RENAME TO access_roles")
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    let pb::server_frame::Frame::Event(second) = receive(&mut socket).await else {
        panic!("Expected second event");
    };
    socket.close(None).await.unwrap();
    let (mut reconnected, _) = tokio_tungstenite::connect_async(request.clone())
        .await
        .unwrap();
    assert!(matches!(
        receive(&mut reconnected).await,
        pb::server_frame::Frame::Welcome(_)
    ));
    let pb::server_frame::Frame::Event(replayed) = receive(&mut reconnected).await else {
        panic!("Expected durable replay");
    };
    assert_eq!(replayed.msg_id, second.msg_id);
    reconnected.close(None).await.unwrap();
    f.db.with_immediate_writer(|c| {
        Box::pin(async move {
            sqlx::query("UPDATE access_roles SET permissions='[]' WHERE system_key='admin'")
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    let (mut revoked, _) = tokio_tungstenite::connect_async(request).await.unwrap();
    assert!(matches!(
        receive(&mut revoked).await,
        pb::server_frame::Frame::Welcome(_)
    ));
    revoked
        .send(WsMessage::Binary(subscribe.into()))
        .await
        .unwrap();
    let pb::server_frame::Frame::Error(error) = receive(&mut revoked).await else {
        panic!("Revoked access must deny replay and subscription");
    };
    assert_eq!(error.code, "FORBIDDEN_CHANNEL");
    revoked.close(None).await.unwrap();
    // More than one replay page of revoked deliveries must not hide a newer personal event.
    let subscriber = f.staff_id;
    let template = second.msg_id;
    f.db.with_immediate_writer(move |c|Box::pin(async move {
        sqlx::query("WITH RECURSIVE n(value) AS (SELECT 1 UNION ALL SELECT value+1 FROM n WHERE value<500) INSERT INTO realtime_outbox(source_sequence,projection_index,channel,event_type,payload,durable,created_at,dispatched_at) SELECT 100000+n.value,0,o.channel,o.event_type,o.payload,1,o.created_at,o.dispatched_at FROM n CROSS JOIN realtime_outbox o WHERE o.id=?")
            .bind(template).execute(&mut *c).await?;
        sqlx::query("INSERT INTO realtime_deliveries(outbox_id,subscriber_id,delivered_at) SELECT id,?,created_at FROM realtime_outbox WHERE source_sequence BETWEEN 100001 AND 100500")
            .bind(subscriber.to_string()).execute(&mut *c).await?;
        write_outbox_event_on_connection(c,NewOutboxEvent{aggregate_type:"notification".into(),aggregate_id:Uuid::now_v7(),event_type:"notification.created".into(),
            payload:serde_json::json!({"userId":subscriber,"text":"personal message after revoked deliveries"}),location_id:None,schema_version:1,deleted:false}).await?;
        Ok(())
    })).await.unwrap();
    let transport = gaming_cafe_api::realtime::tenant_transport::TenantTransport::new(f.db.clone());
    transport.project_pending().await.unwrap();
    let personal = transport
        .pending()
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.event.channel == format!("user:{}", f.staff_id))
        .unwrap();
    transport
        .record_deliveries(personal.event.id, vec![f.staff_id])
        .await
        .unwrap();
    transport.mark_dispatched(personal.event.id).await.unwrap();
    let mut request = url.into_client_request().unwrap();
    request.headers_mut().insert(
        "sec-websocket-protocol",
        "arena360.protobuf.v1".parse().unwrap(),
    );
    let (mut paged, _) = tokio_tungstenite::connect_async(request).await.unwrap();
    assert!(matches!(
        receive(&mut paged).await,
        pb::server_frame::Frame::Welcome(_)
    ));
    let pb::server_frame::Frame::Event(allowed) = receive(&mut paged).await else {
        panic!("Allowed personal event must survive revoked replay pages");
    };
    assert_eq!(allowed.msg_id, personal.event.id);
    paged.close(None).await.unwrap();
    dispatcher.abort();
    let _ = dispatcher.await;
    server.abort();
    let _ = server.await;
    f.close().await;
}

#[tokio::test]
async fn tenant_realtime_recovers_projection_and_rechecks_venue_room_and_account_access() {
    use gaming_cafe_api::{
        realtime::{
            rooms::{CreateRoomDto, TenantRoomService},
            tenant_transport::{can_receive, current_claims, TenantTransport},
        },
        tenancy::{write_outbox_event_on_connection, NewOutboxEvent},
    };
    let f = Fixture::new().await;
    let repo = TenantUserRepository::new(f.db.clone());
    f.db.with_immediate_writer(|c|Box::pin(async move {
        sqlx::query("UPDATE access_roles SET permissions='[\"sessions:read\",\"sessions:write\",\"events:staff\"]' WHERE system_key='staff'").execute(c).await?;
        Ok(())
    })).await.unwrap();
    repo.project_staff(projection(
        f.staff_id,
        "realtime-staff",
        "staff",
        1,
        true,
        false,
        vec![TenantLocationRoleGrant {
            system_key: "staff".into(),
            location_id: f.location_a,
        }],
    ))
    .await
    .unwrap();
    let claims:gaming_cafe_api::dto::JwtUserClaims=serde_json::from_value(serde_json::json!({
        "sub":f.staff_id,"userId":f.staff_id,"tenantId":f.tenant_id,"allowedTenants":[f.tenant_id],"orgIds":[f.tenant_id],
        "roles":["staff"],"permissions":[],"iss":"gamezone","aud":"gamezone","appId":"test","exp":Utc::now().timestamp()+300
    })).unwrap();
    let locations = [f.location_a, f.location_b];
    f.db.with_immediate_writer(move |c|Box::pin(async move {
        for location in locations {
            write_outbox_event_on_connection(c,NewOutboxEvent{aggregate_type:"session".into(),aggregate_id:Uuid::now_v7(),
                event_type:"session.started".into(),payload:serde_json::json!({"id":Uuid::now_v7(),"deviceId":Uuid::now_v7(),"sourceTenantId":Uuid::now_v7()}),
                location_id:Some(location),schema_version:1,deleted:false}).await?;
        }
        Ok(())
    })).await.unwrap();
    let transport = TenantTransport::new(f.db.clone());
    transport.project_pending().await.unwrap();
    let pending = transport.pending().await.unwrap();
    assert_eq!(pending.len(), 4);
    let row = pending
        .iter()
        .find(|r| r.event.channel == "staff" && r.location_id == Some(f.location_a))
        .unwrap()
        .clone();
    let other_venue = pending
        .iter()
        .find(|r| r.event.channel == "staff" && r.location_id == Some(f.location_b))
        .unwrap();
    assert!(can_receive(f.db.clone(), &claims, &row).await.unwrap());
    assert!(!can_receive(f.db.clone(), &claims, other_venue)
        .await
        .unwrap());
    assert_eq!(row.event.payload["sourceTenantId"], f.tenant_id.to_string());
    let mut foreign = claims.clone();
    foreign.tenantId = Uuid::now_v7().to_string();
    assert!(!can_receive(f.db.clone(), &foreign, &row).await.unwrap());
    let restarted = TenantTransport::new(f.db.clone());
    restarted.project_pending().await.unwrap();
    assert_eq!(restarted.pending().await.unwrap().len(), 4);
    restarted
        .record_deliveries(row.event.id, vec![f.staff_id, f.staff_id])
        .await
        .unwrap();
    restarted.mark_dispatched(row.event.id).await.unwrap();
    assert_eq!(restarted.replay(f.staff_id).await.unwrap().len(), 1);
    restarted.ack(row.event.id, Uuid::now_v7()).await.unwrap();
    assert_eq!(restarted.replay(f.staff_id).await.unwrap().len(), 1);
    restarted.ack(row.event.id, f.staff_id).await.unwrap();
    assert!(restarted.replay(f.staff_id).await.unwrap().is_empty());

    let rooms = TenantRoomService::new(f.db.clone());
    let room = rooms
        .create(
            CreateRoomDto {
                name: "Local chat".into(),
                description: None,
            },
            f.staff_id,
        )
        .await
        .unwrap();
    rooms.add_member(room.id, f.staff_id).await.unwrap();
    let channel = "room:Local chat".to_string();
    assert!(restarted
        .publish_chat(&claims, channel.clone(), serde_json::json!(5))
        .await
        .is_err());
    restarted
        .publish_chat(
            &claims,
            channel.clone(),
            serde_json::json!({"text":"hello","sender_id":"forged","sourceTenantId":"forged"}),
        )
        .await
        .unwrap();
    // Losing ownership at commit leaves both projection rows and cursor unchanged.
    let cursor: i64 = sqlx::query_scalar("SELECT sequence FROM realtime_projection_cursor")
        .fetch_one(&f.db.read_pool().unwrap())
        .await
        .unwrap();
    f.lease.fail_next_commit();
    assert!(restarted.project_pending().await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT sequence FROM realtime_projection_cursor")
            .fetch_one(&f.db.read_pool().unwrap())
            .await
            .unwrap(),
        cursor
    );
    assert!(current_claims(f.db.clone(), &claims).await.is_err());
    f.lease.generations.write().unwrap().insert(f.tenant_id, 1);
    restarted.project_pending().await.unwrap();
    let chat = restarted
        .pending()
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.event.channel == channel)
        .unwrap();
    assert_eq!(chat.event.event_type, "chat.message");
    assert_eq!(chat.event.payload["sender_id"], f.staff_id.to_string());
    assert!(can_receive(f.db.clone(), &claims, &chat).await.unwrap());
    rooms.remove_member(room.id, f.staff_id).await.unwrap();
    assert!(!can_receive(f.db.clone(), &claims, &chat).await.unwrap());
    assert!(restarted
        .publish_chat(&claims, channel, serde_json::json!({"text":"forbidden"}))
        .await
        .is_err());
    repo.project_staff(projection(
        f.staff_id,
        "realtime-staff",
        "staff",
        2,
        true,
        false,
        vec![],
    ))
    .await
    .unwrap();
    assert!(!can_receive(f.db.clone(), &claims, &row).await.unwrap());
    repo.project_identity(
        projection(
            f.staff_id,
            "realtime-staff",
            "staff",
            3,
            false,
            false,
            vec![],
        ),
        3,
    )
    .await
    .unwrap();
    assert!(!can_receive(f.db.clone(), &claims, &row).await.unwrap());
    let canonical: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM outbox_events")
        .fetch_one(&f.db.read_pool().unwrap())
        .await
        .unwrap();
    f.db.with_immediate_writer(|c| {
        Box::pin(async move {
            sqlx::query("UPDATE realtime_outbox SET created_at='2000-01-01T00:00:00.000000Z'")
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    assert!(restarted.cleanup(7).await.unwrap() > 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox_events")
            .fetch_one(&f.db.read_pool().unwrap())
            .await
            .unwrap(),
        canonical
    );
    f.close().await;
}

#[tokio::test]
async fn realtime_rooms_are_tenant_local_and_commit_membership_with_outbox() {
    use gaming_cafe_api::realtime::rooms::{CreateRoomDto, TenantRoomService};
    let f = Fixture::new().await;
    TenantUserRepository::new(f.db.clone())
        .project_staff(projection(
            f.staff_id,
            "room-admin",
            "admin",
            1,
            true,
            false,
            vec![],
        ))
        .await
        .unwrap();
    let rooms = TenantRoomService::new(f.db.clone());
    let room = rooms
        .create(
            CreateRoomDto {
                name: " Support ".into(),
                description: None,
            },
            f.staff_id,
        )
        .await
        .unwrap();
    assert_eq!(room.name, "Support");
    assert!(rooms
        .create(
            CreateRoomDto {
                name: "Support".into(),
                description: None
            },
            f.staff_id
        )
        .await
        .is_err());
    assert!(rooms.add_member(room.id, Uuid::now_v7()).await.is_err());
    assert!(!rooms.is_member(room.id, f.staff_id).await.unwrap());
    rooms.add_member(room.id, f.staff_id).await.unwrap();
    rooms.add_member(room.id, f.staff_id).await.unwrap();
    assert_eq!(rooms.list_for_user(f.staff_id).await.unwrap().len(), 1);
    let outbox: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM outbox_events WHERE aggregate_type='realtime_room'",
    )
    .fetch_one(&f.db.read_pool().unwrap())
    .await
    .unwrap();
    assert_eq!(outbox, 2);
    let other = Fixture::new().await;
    assert!(TenantRoomService::new(other.db.clone())
        .add_member(room.id, other.staff_id)
        .await
        .is_err());
    f.lease.fail_next_commit();
    assert!(rooms.remove_member(room.id, f.staff_id).await.is_err());
    assert!(rooms.is_member(room.id, f.staff_id).await.unwrap());
    f.lease.generations.write().unwrap().insert(f.tenant_id, 1);
    rooms.remove_member(room.id, f.staff_id).await.unwrap();
    assert!(!rooms.is_member(room.id, f.staff_id).await.unwrap());
    other.close().await;
    f.close().await;
}

#[tokio::test]
async fn local_staff_refresh_survives_control_outage_and_preserves_signed_tenant_bounds() {
    let f = Fixture::new().await;
    let repo = TenantUserRepository::new(f.db.clone());
    repo.project_staff(projection(
        f.staff_id,
        "local-staff",
        "staff",
        1,
        true,
        false,
        vec![],
    ))
    .await
    .unwrap();
    let unavailable = PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(100))
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    let settings = test_settings();
    let auth = AuthService::new(settings.clone()).with_control_pool(Some(unavailable));
    let other = Uuid::now_v7().to_string();
    let allowed = vec![
        other.clone(),
        f.tenant_id.to_string(),
        other.clone(),
        "invalid".into(),
    ];
    let response = auth
        .issue_local_tenant_auth_response(f.db.clone(), f.staff_id, &allowed)
        .await
        .unwrap();
    let mut validation = jsonwebtoken::Validation::default();
    validation.set_audience(&["gamezone"]);
    let claims = jsonwebtoken::decode::<gaming_cafe_api::dto::JwtUserClaims>(
        &response.accessToken,
        &jsonwebtoken::DecodingKey::from_secret(settings.jwt_secret.as_bytes()),
        &validation,
    )
    .unwrap()
    .claims;
    assert_eq!(claims.tenantId, f.tenant_id.to_string());
    assert_eq!(claims.allowedTenants, vec![f.tenant_id.to_string(), other]);
    assert_eq!(claims.roles, vec!["staff"]);
    assert!(claims.permissions.contains(&"sessions:write".into()));
    assert!(!claims.permissions.contains(&"access:manage".into()));
    assert!(auth
        .issue_local_tenant_auth_response(f.db.clone(), f.staff_id, &[])
        .await
        .is_err());
    repo.project_identity(
        projection(f.staff_id, "local-staff", "staff", 2, false, false, vec![]),
        2,
    )
    .await
    .unwrap();
    assert!(auth
        .issue_local_tenant_auth_response(f.db.clone(), f.staff_id, &allowed)
        .await
        .is_err());
    f.close().await;
}

#[tokio::test]
async fn unchanged_identity_poll_does_not_prevent_idle_handle_eviction() {
    let f = Fixture::new_with_idle(Duration::from_millis(10)).await;
    let repo = TenantUserRepository::new(f.db.clone());
    let p = projection(f.staff_id, "idle-staff", "staff", 1, true, false, vec![]);
    repo.project_identity(p.clone(), 1).await.unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(
        repo.project_identity(p, 1).await.unwrap(),
        StaffProjectionResult::Unchanged
    );
    assert!(
        gaming_cafe_api::repositories::TenantPricingPolicyRepository::new(f.db.clone())
            .activate_due()
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(f.manager.reap_idle().await.unwrap(), 1);
    assert!(f.db.read_pool().is_err());
    f.close().await;
}

#[tokio::test]
#[ignore = "requires isolated CONTROL_TEST_DATABASE_URL"]
async fn staff_kiosk_login_uses_global_credentials_and_local_allowance_with_player_capabilities() {
    use gaming_cafe_api::repositories::{
        TenantBalanceRepository, TenantDeviceRepository, TenantSessionRepository,
        TenantShiftRepository,
    };
    let f = Fixture::new().await;
    let control = PgPoolOptions::new()
        .connect(&std::env::var("CONTROL_TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    gaming_cafe_api::control::migrate(&control).await.unwrap();
    let cell = Uuid::now_v7();
    let staff = f.staff_id;
    let username = format!("kiosk-staff-{staff}");
    sqlx::query("INSERT INTO cells(id,name,address) VALUES($1,$2,$3)")
        .bind(cell)
        .bind(format!("cell-{cell}"))
        .bind(format!("http://{cell}"))
        .execute(&control)
        .await
        .unwrap();
    sqlx::query("INSERT INTO tenants(id,slug,name,timezone,owner_cell,ownership_generation,state) VALUES($1,$2,'Kiosk staff test','UTC',$3,1,'ACTIVE')")
        .bind(f.tenant_id).bind(format!("kiosk-{}",f.tenant_id)).bind(cell).execute(&control).await.unwrap();
    sqlx::query("INSERT INTO subscriptions(tenant_id,plan_code,status,starts_at,ends_at) VALUES($1,'trial','TRIAL',NOW(),NOW()+INTERVAL '30 days')").bind(f.tenant_id).execute(&control).await.unwrap();
    sqlx::query("INSERT INTO users(id,username,password_hash) VALUES($1,$2,$3)")
        .bind(staff)
        .bind(&username)
        .bind(hash("playing-password", DEFAULT_COST).unwrap())
        .execute(&control)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO organization_memberships(tenant_id,user_id,role) VALUES($1,$2,'staff')",
    )
    .bind(f.tenant_id)
    .bind(staff)
    .execute(&control)
    .await
    .unwrap();
    let device = TenantDeviceRepository::new(f.db.clone()).create(&serde_json::from_value(serde_json::json!({
        "name":"Staff kiosk","locationId":f.location_a,"deviceType":"PC","deviceSubType":"HIGH_END_PCS","registrationStatus":"registered"
    })).unwrap(),None).await.unwrap();
    let pg = PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    pg.close().await;
    let settings = test_settings();
    let auth = AuthService::new(settings.clone()).with_control_pool(Some(control.clone()));
    let login = || LoginDto {
        username: username.clone(),
        password: "playing-password".into(),
    };
    let absent = auth
        .login_player_tenant(f.db.clone(), &device, login(), "UTC".into())
        .await
        .unwrap_err();
    assert!(matches!(absent,AppError::Api {ref code,..} if code=="STAFF_ALLOWANCE_NONE"));
    let allowance = TenantBalanceRepository::new(f.db.clone())
        .grant_staff_allowance(staff, 120, 30, None)
        .await
        .unwrap();
    let response = auth
        .login_player_tenant(f.db.clone(), &device, login(), "UTC".into())
        .await
        .unwrap();
    assert_eq!(response.user.role, "staff");
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.set_audience(&["gamezone"]);
    let claims = jsonwebtoken::decode::<gaming_cafe_api::dto::JwtUserClaims>(
        &response.accessToken,
        &jsonwebtoken::DecodingKey::from_secret(settings.jwt_secret.as_bytes()),
        &validation,
    )
    .unwrap()
    .claims;
    assert_eq!(claims.roles, vec!["player"]);
    assert!(claims.permissions.is_empty());
    assert_eq!(claims.tenantId, f.tenant_id.to_string());
    let current =
        gaming_cafe_api::realtime::tenant_transport::current_claims(f.db.clone(), &claims)
            .await
            .unwrap();
    assert_eq!(current.roles, vec!["player"]);
    assert!(current.permissions.is_empty());
    assert!(gaming_cafe_api::realtime::tenant_transport::check_channel(
        f.db.clone(),
        &current,
        &gaming_cafe_api::realtime::channel::ChannelId::Admin
    )
    .await
    .is_err());
    let notification = Uuid::now_v7();
    f.db.with_immediate_writer(move |c| -> BoxFuture<'_,Result<(),AppError>> {Box::pin(async move {
        gaming_cafe_api::tenancy::write_outbox_event_on_connection(c,gaming_cafe_api::tenancy::NewOutboxEvent {
            aggregate_type:"notification".into(),aggregate_id:notification,event_type:"notification.created".into(),location_id:Some(device.location_id),
            schema_version:1,deleted:false,payload:serde_json::json!({"userId":staff,"notificationId":notification,"kind":"kiosk_order_placed"}),
        }).await?;Ok(())
    })}).await.unwrap();
    let transport = gaming_cafe_api::realtime::tenant_transport::TenantTransport::new(f.db.clone());
    transport.project_pending().await.unwrap();
    let inbox_event = transport
        .pending()
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.event.payload["notificationId"] == notification.to_string())
        .unwrap();
    assert!(!gaming_cafe_api::realtime::tenant_transport::can_receive(
        f.db.clone(),
        &claims,
        &inbox_event
    )
    .await
    .unwrap());
    transport
        .record_deliveries(inbox_event.event.id, vec![staff])
        .await
        .unwrap();
    transport
        .ack_current(inbox_event.event.id, &claims)
        .await
        .unwrap();
    assert!(transport
        .replay(staff)
        .await
        .unwrap()
        .iter()
        .any(|row| row.event.id == inbox_event.event.id));
    transport
        .publish_chat(
            &claims,
            format!("user:{staff}"),
            serde_json::json!({"body":"own message"}),
        )
        .await
        .unwrap();
    transport.project_pending().await.unwrap();
    let own_chat = transport
        .pending()
        .await
        .unwrap()
        .into_iter()
        .find(|row| {
            row.event.channel == format!("user:{staff}") && row.event.event_type == "chat.message"
        })
        .unwrap();
    transport
        .record_deliveries(own_chat.event.id, vec![staff])
        .await
        .unwrap();
    transport
        .ack_current(own_chat.event.id, &claims)
        .await
        .unwrap();
    assert!(!transport
        .replay(staff)
        .await
        .unwrap()
        .iter()
        .any(|row| row.event.id == own_chat.event.id));
    let projected = TenantUserRepository::new(f.db.clone())
        .find_by_id(staff)
        .await
        .unwrap()
        .unwrap();
    assert!(projected.password_hash.is_none());
    assert!(projected.totp_secret.is_none());
    let sessions = TenantSessionRepository::new(f.db.clone());
    let session = sessions
        .start(
            staff,
            allowance.id,
            device.id,
            f.location_a,
            None,
            Utc::now(),
            None,
            serde_json::json!({}),
        )
        .await
        .unwrap();
    assert!(
        matches!(TenantShiftRepository::new(f.db.clone()).create(staff,f.location_a,None,staff).await,Err(AppError::Api {ref code,..}) if code=="STAFF_GAMING_SESSION_ACTIVE")
    );
    sessions
        .charge(
            session.session.id,
            0,
            Some((Utc::now(), 1, "voluntary".into())),
            None,
        )
        .await
        .unwrap();
    let shift = TenantShiftRepository::new(f.db.clone())
        .create(staff, f.location_a, None, staff)
        .await
        .unwrap();
    let on_shift = auth
        .login_player_tenant(f.db.clone(), &device, login(), "UTC".into())
        .await
        .unwrap_err();
    assert!(matches!(on_shift,AppError::Api {ref code,..} if code=="STAFF_SHIFT_ACTIVE"));
    assert!(sessions
        .start(
            staff,
            allowance.id,
            device.id,
            f.location_a,
            None,
            Utc::now(),
            None,
            serde_json::json!({})
        )
        .await
        .is_err());
    assert!(
        gaming_cafe_api::realtime::tenant_transport::current_claims(f.db.clone(), &claims)
            .await
            .is_err()
    );
    TenantShiftRepository::new(f.db.clone())
        .force_close(shift.id, staff)
        .await
        .unwrap();
    let wrong = auth
        .login_player_tenant(
            f.db.clone(),
            &device,
            LoginDto {
                username: username.clone(),
                password: "wrong".into(),
            },
            "UTC".into(),
        )
        .await
        .unwrap_err();
    assert!(matches!(wrong,AppError::Api {ref code,..} if code=="AUTH_INVALID_CREDENTIALS"));
    sqlx::query(
        "UPDATE organization_memberships SET is_active=false WHERE tenant_id=$1 AND user_id=$2",
    )
    .bind(f.tenant_id)
    .bind(staff)
    .execute(&control)
    .await
    .unwrap();
    assert!(auth
        .login_player_tenant(f.db.clone(), &device, login(), "UTC".into())
        .await
        .is_err());
    gaming_cafe_api::control::staff_projection::sync_user(&control, f.db.clone(), staff)
        .await
        .unwrap();
    assert!(
        gaming_cafe_api::realtime::tenant_transport::current_claims(f.db.clone(), &claims)
            .await
            .is_err()
    );
    sqlx::query("DELETE FROM organization_memberships WHERE tenant_id=$1")
        .bind(f.tenant_id)
        .execute(&control)
        .await
        .unwrap();
    sqlx::query("DELETE FROM tenants WHERE id=$1")
        .bind(f.tenant_id)
        .execute(&control)
        .await
        .unwrap();
    sqlx::query("DELETE FROM users WHERE id=$1")
        .bind(staff)
        .execute(&control)
        .await
        .unwrap();
    sqlx::query("DELETE FROM cells WHERE id=$1")
        .bind(cell)
        .execute(&control)
        .await
        .unwrap();
    control.close().await;
    f.close().await;
}

#[tokio::test]
async fn staged_tenant_login_authenticates_only_tenant_players() {
    let fixture = Fixture::new().await;
    let repo = TenantUserRepository::new(fixture.db.clone());
    let player = repo
        .create_player(TenantCreatePlayer {
            username: "login-player".into(),
            password_hash: hash("password 123", DEFAULT_COST).unwrap(),
            phone_number: "9999999999".into(),
            first_name: None,
            last_name: None,
            actor_id: None,
        })
        .await
        .unwrap();
    let balance_id = Uuid::now_v7();
    let at = gaming_cafe_api::time::format_sqlite_timestamp(&Utc::now()).unwrap();
    let expiry =
        gaming_cafe_api::time::format_sqlite_timestamp(&(Utc::now() + chrono::Duration::days(1)))
            .unwrap();
    fixture
        .db
        .with_immediate_writer(move |connection| -> BoxFuture<'_, Result<(), AppError>> {
            Box::pin(async move {
                sqlx::query(
                    "INSERT INTO player_plan_balances(id,player_id,device_type,device_sub_type,\
                     kind,remaining_minutes,expiry_date,status,created_at,updated_at) \
                     VALUES(?,?,'PC','HIGH_END_PCS','time',60,?,'active',?,?)",
                )
                .bind(balance_id.to_string())
                .bind(player.id.to_string())
                .bind(expiry)
                .bind(&at)
                .bind(&at)
                .execute(connection)
                .await?;
                Ok(())
            })
        })
        .await
        .unwrap();

    repo.project_staff(projection(
        fixture.staff_id,
        "login-staff",
        "staff",
        1,
        true,
        false,
        vec![],
    ))
    .await
    .unwrap();
    let auth = AuthService::new(test_settings());
    let now = Utc::now();
    let device = Device {
        id: Uuid::now_v7(),
        organization_id: fixture.tenant_id,
        location_id: fixture.location_a,
        name: "Login kiosk".into(),
        serial_number: None,
        local_ip_address: None,
        device_type: "PC".into(),
        device_sub_type: "HIGH_END_PCS".into(),
        location: None,
        status: "available".into(),
        registered_kiosk: None,
        registration_status: "registered".into(),
        created_by: None,
        updated_by: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };
    let response = auth
        .login_player_tenant(
            fixture.db.clone(),
            &device,
            LoginDto {
                username: "LOGIN-PLAYER".into(),
                password: "password 123".into(),
            },
            "UTC".into(),
        )
        .await
        .unwrap();
    assert_eq!(response.user.username, "login-player");
    for (username, password) in [
        ("login-player", "wrong password"),
        ("login-staff", "password 123"),
        ("missing-player", "password 123"),
    ] {
        let error = auth
            .login_player_tenant(
                fixture.db.clone(),
                &device,
                LoginDto {
                    username: username.into(),
                    password: password.into(),
                },
                "UTC".into(),
            )
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            AppError::Api { ref code, .. } if code == "AUTH_INVALID_CREDENTIALS"
        ));
    }
    fixture.close().await;
}

fn test_settings() -> Arc<Settings> {
    Arc::new(Settings {
        roles: Roles::ALL,
        cell_id: None,
        tenant_data_dir: PathBuf::from("unused"),
        control_database_url: None,
        database_min_connections: 0,
        database_max_connections: 1,
        database_acquire_timeout_seconds: 1,
        database_idle_timeout_seconds: 1,
        database_max_lifetime_seconds: 1,
        nats_url: None,
        redis_url: None,
        jwt_secret: "tenant-login-test-secret-at-least-32-characters".into(),
        jwt_access_expiration: "15m".into(),
        jwt_player_expiration: "24h".into(),
        jwt_device_expiration: "365d".into(),
        bcrypt_salt_rounds: 10,
        port: 3000,
        cafe_timezone: "UTC".into(),
        zeptomail_token: None,
        legacy_rest_enabled: false,
        trusted_proxy_cidrs: vec![],
        max_concurrent_requests: 16,
    })
}

fn projection(
    user_id: Uuid,
    username: &str,
    role: &str,
    member_revision: i64,
    is_active: bool,
    deleted: bool,
    location_grants: Vec<TenantLocationRoleGrant>,
) -> TenantStaffProjection {
    TenantStaffProjection {
        user_id,
        email: Some(format!("{username}@example.invalid")),
        username: username.into(),
        first_name: Some("Projected".into()),
        last_name: Some("Staff".into()),
        phone_number: Some("9999999999".into()),
        avatar_url: None,
        role: role.into(),
        permissions: vec!["safe:profile".into()],
        is_active,
        deleted,
        member_revision,
        global_access_role_system_keys: vec![role.into()],
        location_grants,
    }
}

async fn counts(fixture: &Fixture) -> (i64, i64, i64, i64) {
    let pool = fixture.db.read_pool().unwrap();
    let users = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    let access = sqlx::query_scalar("SELECT COUNT(*) FROM access_assignments")
        .fetch_one(&pool)
        .await
        .unwrap();
    let locations = sqlx::query_scalar("SELECT COUNT(*) FROM location_role_assignments")
        .fetch_one(&pool)
        .await
        .unwrap();
    let outbox = sqlx::query_scalar("SELECT COUNT(*) FROM outbox_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    (users, access, locations, outbox)
}

struct Fixture {
    root: PathBuf,
    tenant_id: Uuid,
    db: Arc<TenantDb>,
    lease: Arc<Lease>,
    manager: Arc<TenantDbManager>,
    staff_id: Uuid,
    location_a: Uuid,
    location_b: Uuid,
}

impl Fixture {
    async fn new() -> Self {
        Self::new_with_idle(Duration::from_secs(60)).await
    }
    async fn new_with_idle(idle_timeout: Duration) -> Self {
        let root = std::env::temp_dir().join(format!("arena360-user-repo-{}", Uuid::now_v7()));
        let tenant_id = Uuid::now_v7();
        let path = tenant_path(&root, tenant_id);
        tokio::fs::create_dir_all(path.parent().unwrap())
            .await
            .unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        gaming_cafe_api::tenancy::migrate(&pool).await.unwrap();
        let at = gaming_cafe_api::time::format_sqlite_timestamp(&Utc::now()).unwrap();
        let location_a = Uuid::from_u128(1);
        let location_b = Uuid::from_u128(2);
        for (id, slug) in [(location_a, "alpha"), (location_b, "beta")] {
            sqlx::query(
                "INSERT INTO venue_locations(id,slug,name,created_at,updated_at) VALUES(?,?,?,?,?)",
            )
            .bind(id.to_string())
            .bind(slug)
            .bind(slug)
            .bind(&at)
            .bind(&at)
            .execute(&pool)
            .await
            .unwrap();
        }
        for (id, key, permissions, is_template) in [
            (
                Uuid::from_u128(10),
                "staff",
                r#"["sessions:read","sessions:write"]"#,
                false,
            ),
            (Uuid::from_u128(11), "admin", r#"["access:manage"]"#, false),
            (Uuid::from_u128(12), "finance", r#"["finance:read"]"#, false),
            (
                Uuid::from_u128(13),
                "template-only",
                r#"["finance:read"]"#,
                true,
            ),
        ] {
            sqlx::query(
                "INSERT INTO access_roles(id,system_key,name,permissions,is_template,created_at,updated_at) \
                 VALUES(?,?,?,?,?,?,?)",
            )
            .bind(id.to_string())
            .bind(key)
            .bind(key)
            .bind(permissions)
            .bind(is_template)
            .bind(&at)
            .bind(&at)
            .execute(&pool)
            .await
            .unwrap();
        }
        pool.close().await;
        let lease = Arc::new(Lease::default());
        lease.generations.write().unwrap().insert(tenant_id, 1);
        let manager = TenantDbManager::new(
            TenantDbConfig {
                root: root.clone(),
                read_connections: 2,
                busy_timeout: Duration::from_millis(250),
                idle_timeout,
                reaper_interval: Duration::from_secs(1),
            },
            lease.clone(),
        )
        .unwrap();
        let db = manager.open(tenant_id).await.unwrap();
        Self {
            root,
            tenant_id,
            db,
            lease,
            manager: Arc::new(manager),
            staff_id: Uuid::now_v7(),
            location_a,
            location_b,
        }
    }

    async fn close(self) {
        self.db.close().await.unwrap();
        tokio::fs::remove_dir_all(self.root).await.unwrap();
    }
}
