use gaming_cafe_api::{
    error::AppError,
    tenancy::{tenant_path, TenantDb, TenantDbConfig, TenantDbManager, TenantLease},
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::{path::PathBuf, sync::Arc, time::Duration};
use uuid::Uuid;

struct Lease(Uuid);
impl TenantLease for Lease {
    fn writable_generation(&self, tenant: Uuid) -> Result<i64, AppError> {
        if tenant == self.0 {
            Ok(1)
        } else {
            Err(AppError::Forbidden("foreign test tenant".into()))
        }
    }
    fn ensure_writable(&self, tenant: Uuid, generation: i64) -> Result<(), AppError> {
        if self.writable_generation(tenant)? == generation {
            Ok(())
        } else {
            Err(AppError::Forbidden("stale test generation".into()))
        }
    }
}

/// An isolated, migrated tenant file with a lease scoped to exactly that tenant.
pub struct TenantFixture {
    pub db: Arc<TenantDb>,
    root: PathBuf,
}
impl TenantFixture {
    pub async fn new() -> Self {
        let tenant = Uuid::now_v7();
        let root = std::env::temp_dir().join(format!("arena360-service-test-{tenant}"));
        let path = tenant_path(&root, tenant);
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
        pool.close().await;
        let manager = TenantDbManager::new(
            TenantDbConfig {
                root: root.clone(),
                read_connections: 2,
                busy_timeout: Duration::from_millis(500),
                idle_timeout: Duration::from_secs(60),
                reaper_interval: Duration::from_secs(1),
            },
            Arc::new(Lease(tenant)),
        )
        .unwrap();
        let db = manager.open(tenant).await.unwrap();
        Self { db, root }
    }
    pub async fn player(&self, username: &str) -> Uuid {
        gaming_cafe_api::repositories::TenantUserRepository::new(self.db.clone())
            .create_player(gaming_cafe_api::repositories::TenantCreatePlayer {
                username: username.into(),
                password_hash: bcrypt::hash("initial-password", 4).unwrap(),
                phone_number: "9999999999".into(),
                first_name: None,
                last_name: None,
                actor_id: None,
            })
            .await
            .unwrap()
            .id
    }
    pub async fn close(self) {
        self.db.close().await.unwrap();
        tokio::fs::remove_dir_all(self.root).await.unwrap();
    }
}
