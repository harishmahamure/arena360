use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{Duration, Utc};
use gaming_cafe_api::control::{CreateTenant, Tenant};
use gaming_cafe_api::error::AppError;
use gaming_cafe_api::tenancy::{
    target_schema_version, InitialSettingOverride, ProvisionTenant, ProvisioningControl,
    TenantProvisioner,
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
        7
    );
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
    tokio::fs::remove_dir_all(root).await.unwrap();
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
