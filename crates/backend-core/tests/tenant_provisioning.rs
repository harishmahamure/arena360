use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{Duration, Utc};
use gaming_cafe_api::control::{CreateTenant, Tenant};
use gaming_cafe_api::error::AppError;
use gaming_cafe_api::repositories::{
    TenantSettingsRepository, TenantStaffProjection, TenantUserRepository,
};
use gaming_cafe_api::tenancy::{
    target_schema_version, InitialSettingOverride, ProvisionTenant, ProvisioningControl,
    TenantDbConfig, TenantDbManager, TenantLease, TenantProvisioner,
};
use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use uuid::Uuid;

struct FakeControl {
    tenant: Mutex<Option<Tenant>>,
    tenant_id: Uuid,
    cell_id: Uuid,
    fail_finalize_once: AtomicBool,
    register_count: AtomicUsize,
    acquire_count: AtomicUsize,
}

#[async_trait]
impl ProvisioningControl for FakeControl {
    async fn register(&self, request: CreateTenant) -> Result<Tenant, AppError> {
        self.register_count.fetch_add(1, Ordering::Relaxed);
        let mut tenant = self.tenant.lock().unwrap();
        let existing = tenant.get_or_insert_with(|| Tenant {
            id: self.tenant_id,
            slug: request.slug,
            name: request.name,
            owner_cell: request.owner_cell,
            ownership_generation: 0,
            schema_version: 0,
            state: "PROVISIONING".into(),
            timezone: request.timezone,
        });
        Ok(existing.clone())
    }

    async fn acquire_lease(&self, tenant_id: Uuid) -> Result<i64, AppError> {
        assert_eq!(tenant_id, self.tenant_id);
        self.acquire_count.fetch_add(1, Ordering::Relaxed);
        let mut tenant = self.tenant.lock().unwrap();
        let tenant = tenant.as_mut().unwrap();
        tenant.owner_cell = Some(self.cell_id);
        tenant.ownership_generation = 1;
        Ok(1)
    }

    async fn finalize(
        &self,
        tenant_id: Uuid,
        ownership_generation: i64,
        schema_version: i64,
    ) -> Result<Tenant, AppError> {
        assert_eq!(tenant_id, self.tenant_id);
        assert_eq!(ownership_generation, 1);
        if !self.fail_finalize_once.swap(true, Ordering::SeqCst) {
            return Err(AppError::Internal("injected finalize failure".into()));
        }
        let mut tenant = self.tenant.lock().unwrap();
        let tenant = tenant.as_mut().unwrap();
        tenant.schema_version = schema_version;
        tenant.state = "ACTIVE".into();
        Ok(tenant.clone())
    }
}

impl TenantLease for FakeControl {
    fn writable_generation(&self, tenant_id: Uuid) -> Result<i64, AppError> {
        let tenant = self.tenant.lock().unwrap();
        match tenant.as_ref() {
            Some(tenant) if tenant.id == tenant_id && tenant.state == "ACTIVE" => {
                Ok(tenant.ownership_generation)
            }
            _ => Err(AppError::Forbidden("Tenant is not active".into())),
        }
    }

    fn ensure_writable(&self, tenant_id: Uuid, expected: i64) -> Result<(), AppError> {
        if self.writable_generation(tenant_id)? == expected {
            Ok(())
        } else {
            Err(AppError::Forbidden("Stale tenant generation".into()))
        }
    }
}

#[tokio::test]
async fn provisioning_seeds_defaults_and_resumes_without_overwriting_customizations() {
    let root = std::env::temp_dir().join(format!("arena360-provisioning-{}", Uuid::new_v4()));
    let tenant_id = Uuid::new_v4();
    let cell_id = Uuid::new_v4();
    let control = Arc::new(FakeControl {
        tenant: Mutex::new(None),
        tenant_id,
        cell_id,
        fail_finalize_once: AtomicBool::new(false),
        register_count: AtomicUsize::new(0),
        acquire_count: AtomicUsize::new(0),
    });
    let provisioner = TenantProvisioner::new(root.clone(), control.clone());
    let request = request(cell_id);

    assert!(provisioner.provision(request.clone()).await.is_err());
    let path = gaming_cafe_api::tenancy::tenant_path(&root, tenant_id);
    let pool = open_pool(&path).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM units")
            .fetch_one(&pool)
            .await
            .unwrap(),
        11
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM access_roles")
            .fetch_one(&pool)
            .await
            .unwrap(),
        9
    );
    let roles: Vec<(String, String, bool, String)> = sqlx::query_as(
        "SELECT system_key, name, is_template, permissions FROM access_roles ORDER BY system_key",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let expected = [
        ("admin", "Administrator", false),
        ("location-admin", "Location administrator", false),
        ("staff", "Counter operator", false),
        ("template-auditor", "Auditor", true),
        ("template-counter", "Counter operator", true),
        ("template-finance", "Finance reviewer", true),
        ("template-kitchen", "Kitchen operator", true),
        ("template-location-admin", "Location administrator", true),
        ("template-manager", "Venue manager", true),
    ];
    for ((key, name, template, permissions), expected) in roles.iter().zip(expected) {
        assert_eq!((key.as_str(), name.as_str(), *template), expected);
        let permissions: Vec<String> = serde_json::from_str(permissions).unwrap();
        assert!(!permissions.is_empty());
        assert_eq!(
            permissions.len(),
            permissions.iter().collect::<BTreeSet<_>>().len()
        );
        assert!(
            permissions
                .iter()
                .all(|p| gaming_cafe_api::access::known(p)),
            "{key}"
        );
        if key == "admin" {
            let catalog = gaming_cafe_api::access::catalog();
            let known: BTreeSet<String> = catalog
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|module| module["permissions"].as_array().unwrap())
                .map(|permission| permission["key"].as_str().unwrap().to_owned())
                .collect();
            assert_eq!(permissions.into_iter().collect::<BTreeSet<_>>(), known);
        }
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM setting_overrides")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    sqlx::query("UPDATE units SET name = 'Single item' WHERE unit_type = 'piece'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE access_roles SET name='My counter', permissions='[\"transactions:read\"]', revision=2 WHERE system_key='staff'")
        .execute(&pool).await.unwrap();
    sqlx::query("UPDATE setting_overrides SET value='\"Custom venue\"', revision=2 WHERE key='business.name'")
        .execute(&pool).await.unwrap();
    sqlx::query("DELETE FROM access_roles WHERE system_key='template-kitchen'")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;

    let result = provisioner.provision(request).await.unwrap();
    assert_eq!(result.tenant.state, "ACTIVE");
    assert_eq!(result.tenant.schema_version, target_schema_version());
    assert_eq!(result.ownership_generation, 1);
    assert_eq!(result.path, path);
    assert_eq!(control.register_count.load(Ordering::Relaxed), 2);
    assert_eq!(control.acquire_count.load(Ordering::Relaxed), 2);

    let pool = open_pool(&path).await;
    let piece_name: String = sqlx::query_scalar("SELECT name FROM units WHERE unit_type = 'piece'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(piece_name, "Single item");
    let staff: (String, String, i64) = sqlx::query_as(
        "SELECT name,permissions,revision FROM access_roles WHERE system_key='staff'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        staff,
        ("My counter".into(), "[\"transactions:read\"]".into(), 2)
    );
    let setting: (String, i64) =
        sqlx::query_as("SELECT value,revision FROM setting_overrides WHERE key='business.name'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(setting, ("\"Custom venue\"".into(), 2));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM access_roles")
            .fetch_one(&pool)
            .await
            .unwrap(),
        9
    );
    let restored_template: bool = sqlx::query_scalar(
        "SELECT is_template FROM access_roles WHERE system_key='template-kitchen'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(restored_template);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM units")
            .fetch_one(&pool)
            .await
            .unwrap(),
        11
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM setting_overrides")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    pool.close().await;

    // First identity hydration must assign the provisioned baseline role and let
    // the administrator perform the location setup required by a new tenant.
    let manager = TenantDbManager::new(
        TenantDbConfig {
            root: root.clone(),
            ..TenantDbConfig::default()
        },
        control,
    )
    .unwrap();
    let db = manager.open(tenant_id).await.unwrap();
    let user = Uuid::new_v4();
    TenantUserRepository::new(db.clone())
        .project_identity(
            TenantStaffProjection {
                user_id: user,
                email: None,
                username: "new.admin".into(),
                first_name: None,
                last_name: None,
                phone_number: None,
                avatar_url: None,
                role: "admin".into(),
                permissions: vec![],
                is_active: true,
                deleted: false,
                member_revision: 0,
                global_access_role_system_keys: vec![],
                location_grants: vec![],
            },
            1,
        )
        .await
        .unwrap();
    let settings = TenantSettingsRepository::new(db.clone());
    for permission in ["access:manage", "locations:read", "locations:manage"] {
        settings
            .ensure_access(tenant_id, user, permission)
            .await
            .unwrap();
        assert!(settings
            .effective_permissions(user)
            .await
            .unwrap()
            .contains(&permission.to_owned()));
    }
    db.close().await.unwrap();
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn provisioning_seed_conflicts_roll_back_and_prevent_activation() {
    // A retry must preserve custom rows, but cannot activate with a missing baseline role.
    for incompatible_type in [false, true] {
        let root = std::env::temp_dir().join(format!("arena360-provisioning-{}", Uuid::new_v4()));
        let tenant_id = Uuid::new_v4();
        let cell_id = Uuid::new_v4();
        let control = Arc::new(FakeControl {
            tenant: Mutex::new(None),
            tenant_id,
            cell_id,
            fail_finalize_once: AtomicBool::new(false),
            register_count: AtomicUsize::new(0),
            acquire_count: AtomicUsize::new(0),
        });
        let provisioner = TenantProvisioner::new(root.clone(), control.clone());
        assert!(provisioner.provision(request(cell_id)).await.is_err());
        let path = gaming_cafe_api::tenancy::tenant_path(&root, tenant_id);
        let pool = open_pool(&path).await;
        sqlx::query("DELETE FROM units WHERE unit_type='liter'")
            .execute(&pool)
            .await
            .unwrap();
        if incompatible_type {
            sqlx::query("UPDATE access_roles SET is_template=1 WHERE system_key='admin'")
                .execute(&pool)
                .await
                .unwrap();
        } else {
            // The unrelated name collision used to be swallowed by INSERT OR IGNORE.
            sqlx::query("DELETE FROM access_roles WHERE system_key='location-admin'")
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO access_roles(id,name,permissions,created_at,updated_at) SELECT ?, 'Location administrator', '[\"locations:read\"]', created_at,updated_at FROM access_roles WHERE system_key='admin'")
                .bind(Uuid::new_v4().to_string()).execute(&pool).await.unwrap();
        }
        let error = provisioner.provision(request(cell_id)).await.unwrap_err();
        if incompatible_type {
            assert!(matches!(error, AppError::Conflict(_)));
        } else {
            assert!(matches!(error, AppError::Database(_)));
        }
        assert_eq!(control.acquire_count.load(Ordering::Relaxed), 1);
        assert_eq!(
            control.tenant.lock().unwrap().as_ref().unwrap().state,
            "PROVISIONING"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM units WHERE unit_type='liter'")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0,
            "failed seed transaction must roll back"
        );
        if incompatible_type {
            sqlx::query("UPDATE access_roles SET is_template=0 WHERE system_key='admin'")
                .execute(&pool)
                .await
                .unwrap();
        } else {
            sqlx::query(
                "UPDATE access_roles SET name='Custom location reader' WHERE system_key IS NULL",
            )
            .execute(&pool)
            .await
            .unwrap();
        }
        assert_eq!(
            provisioner
                .provision(request(cell_id))
                .await
                .unwrap()
                .tenant
                .state,
            "ACTIVE"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM units")
                .fetch_one(&pool)
                .await
                .unwrap(),
            11
        );
        pool.close().await;
        tokio::fs::remove_dir_all(root).await.unwrap();
    }
}

#[tokio::test]
async fn provisioning_refuses_an_unmanaged_existing_database() {
    let root = std::env::temp_dir().join(format!("arena360-provisioning-{}", Uuid::new_v4()));
    let tenant_id = Uuid::new_v4();
    let cell_id = Uuid::new_v4();
    let path = gaming_cafe_api::tenancy::tenant_path(&root, tenant_id);
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
    sqlx::query("CREATE TABLE unrelated (value TEXT)")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    let control = Arc::new(FakeControl {
        tenant: Mutex::new(None),
        tenant_id,
        cell_id,
        fail_finalize_once: AtomicBool::new(true),
        register_count: AtomicUsize::new(0),
        acquire_count: AtomicUsize::new(0),
    });
    let provisioner = TenantProvisioner::new(root.clone(), control);

    assert!(matches!(
        provisioner.provision(request(cell_id)).await,
        Err(AppError::Conflict(_))
    ));
    let pool = open_pool(&path).await;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'unrelated')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(exists);
    pool.close().await;
    tokio::fs::remove_dir_all(root).await.unwrap();
}

fn request(cell_id: Uuid) -> ProvisionTenant {
    ProvisionTenant {
        tenant: CreateTenant {
            slug: "provisioning-test".into(),
            name: "Provisioning test".into(),
            timezone: "UTC".into(),
            owner_cell: Some(cell_id),
            subscription_plan: "trial".into(),
            entitlements: json!({}),
            trial_ends_at: Utc::now() + Duration::days(30),
            entitlement_grace_until: Utc::now() + Duration::days(37),
        },
        settings: vec![InitialSettingOverride {
            location_id: None,
            key: "business.name".into(),
            value: json!("Provisioning test"),
        }],
    }
}

async fn open_pool(path: &PathBuf) -> sqlx::SqlitePool {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().filename(path))
        .await
        .unwrap()
}
