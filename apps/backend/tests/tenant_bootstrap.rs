//! CONTROL_TEST_DATABASE_URL=... cargo test --test tenant_bootstrap -- --ignored
use chrono::{Duration, Utc};
use gaming_cafe_api::{
    control::{bootstrap::recover_assigned, CreateTenant, LeaseClient, LeaseConfig, Repository},
    repositories::TenantUserRepository,
    tenancy::{
        PostgresProvisioningControl, ProvisionTenant, TenantDbConfig, TenantDbManager,
        TenantProvisioner,
    },
};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires isolated CONTROL_TEST_DATABASE_URL"]
async fn restart_recovers_only_assigned_files_and_preserves_local_grants() {
    let pool = PgPoolOptions::new()
        .max_connections(3)
        .connect(&std::env::var("CONTROL_TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    gaming_cafe_api::control::migrate(&pool).await.unwrap();
    let cell = Uuid::now_v7();
    let other = Uuid::now_v7();
    for id in [cell, other] {
        sqlx::query("INSERT INTO cells(id,name,address) VALUES($1,$2,$3)")
            .bind(id)
            .bind(format!("boot-{id}"))
            .bind(format!("http://boot-{id}.internal"))
            .execute(&pool)
            .await
            .unwrap();
    }
    let root = std::env::temp_dir().join(format!("arena360-boot-{cell}"));
    let repository = Repository::new(pool.clone());
    let request = |owner, suffix: &str| CreateTenant {
        slug: format!("boot-{cell}-{suffix}"),
        name: "Restart fixture".into(),
        timezone: "UTC".into(),
        owner_cell: Some(owner),
        subscription_plan: "trial".into(),
        entitlements: json!({}),
        trial_ends_at: Utc::now() + Duration::days(30),
        entitlement_grace_until: Utc::now() + Duration::days(37),
    };
    let leases = Arc::new(LeaseClient::new(pool.clone(), cell, LeaseConfig::default()).unwrap());
    let provisioner = TenantProvisioner::new(
        root.clone(),
        Arc::new(PostgresProvisioningControl::new(
            repository.clone(),
            leases.clone(),
        )),
    );
    let tenant = provisioner
        .provision(ProvisionTenant {
            tenant: request(cell, "local"),
            settings: vec![],
        })
        .await
        .unwrap()
        .tenant
        .id;
    let foreign = repository
        .create_tenant(request(other, "foreign"))
        .await
        .unwrap()
        .id;
    LeaseClient::new(pool.clone(), other, LeaseConfig::default())
        .unwrap()
        .acquire(foreign)
        .await
        .unwrap();
    let provisioning = repository
        .create_tenant(request(cell, "pending"))
        .await
        .unwrap()
        .id;
    let user = Uuid::now_v7();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name) VALUES($1,$2,'global-secret','Initial')").bind(user).bind(format!("boot-user-{user}")).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO organization_memberships(tenant_id,user_id,role) VALUES($1,$2,'staff')",
    )
    .bind(tenant)
    .bind(user)
    .execute(&pool)
    .await
    .unwrap();
    let restarted = Arc::new(LeaseClient::new(pool.clone(), cell, LeaseConfig::default()).unwrap());
    assert!(restarted.writable_generation(tenant).is_err());
    let manager = Arc::new(
        TenantDbManager::new(
            TenantDbConfig {
                root: root.clone(),
                ..TenantDbConfig::default()
            },
            restarted.clone(),
        )
        .unwrap(),
    );
    assert_eq!(
        recover_assigned(&pool, &restarted, &manager, &root)
            .await
            .unwrap(),
        1
    );
    assert!(restarted.writable_generation(foreign).is_err());
    assert!(restarted.writable_generation(provisioning).is_err());
    let db = manager.open(tenant).await.unwrap();
    let profile = TenantUserRepository::new(db.clone())
        .require_active_staff(user)
        .await
        .unwrap();
    assert_eq!(profile.first_name.as_deref(), Some("Initial"));
    assert!(profile.password_hash.is_none());
    let custom = Uuid::now_v7();
    db.with_immediate_writer(move |c| Box::pin(async move {
        let at = gaming_cafe_api::tenancy::format_sqlite_timestamp(&Utc::now()).unwrap();
        sqlx::query("INSERT INTO access_roles(id,name,permissions,created_at,updated_at) VALUES(?,'Local finance','[\"finance:read\"]',?,?)").bind(custom.to_string()).bind(&at).bind(&at).execute(&mut *c).await?;
        sqlx::query("DELETE FROM access_assignments WHERE user_id=?").bind(user.to_string()).execute(&mut *c).await?;
        sqlx::query("INSERT INTO access_assignments(user_id,role_id,created_at) VALUES(?,?,?)").bind(user.to_string()).bind(custom.to_string()).bind(at).execute(c).await?;
        Ok(())
    })).await.unwrap();
    sqlx::query("UPDATE users SET first_name='After restart' WHERE id=$1")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        recover_assigned(&pool, &restarted, &manager, &root)
            .await
            .unwrap(),
        1
    );
    let profile = TenantUserRepository::new(db.clone())
        .require_active_staff(user)
        .await
        .unwrap();
    assert_eq!(profile.first_name.as_deref(), Some("After restart"));
    assert!(profile.password_hash.is_none());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM access_assignments WHERE user_id=?")
            .bind(user.to_string())
            .fetch_one(&db.read_pool().unwrap())
            .await
            .unwrap(),
        1
    );
    let retained: String =
        sqlx::query_scalar("SELECT role_id FROM access_assignments WHERE user_id=?")
            .bind(user.to_string())
            .fetch_one(&db.read_pool().unwrap())
            .await
            .unwrap();
    assert_eq!(retained, custom.to_string());
    let missing = repository
        .create_tenant(request(cell, "missing"))
        .await
        .unwrap()
        .id;
    leases.acquire(missing).await.unwrap();
    let missing_restart =
        Arc::new(LeaseClient::new(pool.clone(), cell, LeaseConfig::default()).unwrap());
    assert!(recover_assigned(&pool, &missing_restart, &manager, &root)
        .await
        .is_err());
    assert!(
        missing_restart.writable_generation(missing).is_err(),
        "missing files must not become writable"
    );
    assert!(!gaming_cafe_api::tenancy::tenant_path(&root, missing).exists());
    db.close().await.unwrap();
    for id in [tenant, foreign, provisioning, missing] {
        sqlx::query("DELETE FROM tenants WHERE id=$1")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("DELETE FROM users WHERE id=$1")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    for id in [cell, other] {
        sqlx::query("DELETE FROM cells WHERE id=$1")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
    }
    pool.close().await;
    tokio::fs::remove_dir_all(root).await.unwrap();
}
