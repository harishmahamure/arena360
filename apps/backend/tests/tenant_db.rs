use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use gaming_cafe_api::error::AppError;
use gaming_cafe_api::tenancy::{
    retry_foreground, tenant_path, SqliteBusyMetrics, SqliteRetryConfig, TenantDbConfig,
    TenantDbManager, TenantLease,
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Connection, SqliteConnection};
use uuid::Uuid;

#[derive(Default)]
struct FakeLease {
    generations: RwLock<HashMap<Uuid, i64>>,
}

impl FakeLease {
    fn set(&self, tenant_id: Uuid, generation: Option<i64>) {
        let mut generations = self.generations.write().unwrap();
        match generation {
            Some(generation) => {
                generations.insert(tenant_id, generation);
            }
            None => {
                generations.remove(&tenant_id);
            }
        }
    }
}

impl TenantLease for FakeLease {
    fn writable_generation(&self, tenant_id: Uuid) -> Result<i64, AppError> {
        self.generations
            .read()
            .unwrap()
            .get(&tenant_id)
            .copied()
            .ok_or_else(|| AppError::Forbidden("tenant lease is not writable".into()))
    }

    fn ensure_writable(&self, tenant_id: Uuid, expected_generation: i64) -> Result<(), AppError> {
        if self.writable_generation(tenant_id)? == expected_generation {
            Ok(())
        } else {
            Err(AppError::Forbidden(
                "tenant lease generation changed".into(),
            ))
        }
    }
}

#[tokio::test]
async fn manager_requires_a_lease_and_reuses_one_handle_per_generation() {
    let (root, tenant_id) = provision_tenant().await;
    let lease = Arc::new(FakeLease::default());
    let manager = Arc::new(
        TenantDbManager::new(test_config(root.clone()), lease.clone()).expect("valid config"),
    );

    assert!(matches!(
        manager.open(tenant_id).await,
        Err(AppError::Forbidden(_))
    ));

    lease.set(tenant_id, Some(7));
    let (first, second) = tokio::join!(manager.open(tenant_id), manager.open(tenant_id));
    let first = first.unwrap();
    let second = second.unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(manager.open_count().await, 1);
    assert_eq!(first.ownership_generation(), 7);

    let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&first.read_pool().unwrap())
        .await
        .unwrap();
    assert_eq!(journal_mode, "wal");
    first
        .with_writer(|connection| {
            Box::pin(async move {
                sqlx::query("CREATE TABLE write_probe (value INTEGER NOT NULL)")
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .await
        .unwrap();

    lease.set(tenant_id, Some(8));
    assert!(matches!(
        first
            .with_writer(|_| Box::pin(async { Ok::<_, AppError>(()) }))
            .await,
        Err(AppError::Forbidden(_))
    ));
    assert_eq!(manager.reap_idle().await.unwrap(), 1);
    assert_eq!(manager.open_count().await, 0);

    let replacement = manager.open(tenant_id).await.unwrap();
    assert_eq!(replacement.ownership_generation(), 8);
    assert!(!Arc::ptr_eq(&first, &replacement));

    replacement.close().await.unwrap();
    remove_test_root(&root).await;
}

#[tokio::test]
async fn manager_reaps_idle_handles() {
    let (root, tenant_id) = provision_tenant().await;
    let lease = Arc::new(FakeLease::default());
    lease.set(tenant_id, Some(1));
    let mut config = test_config(root.clone());
    config.idle_timeout = Duration::from_millis(5);
    let manager = TenantDbManager::new(config, lease).unwrap();

    let handle = manager.open(tenant_id).await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert_eq!(manager.reap_idle().await.unwrap(), 1);
    assert!(handle.read_pool().is_err());

    remove_test_root(&root).await;
}

#[tokio::test]
async fn foreground_retry_waits_for_a_transient_sqlite_lock() {
    let root = test_root();
    tokio::fs::create_dir_all(&root).await.unwrap();
    let path = root.join("retry.sqlite");
    let options = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true)
        .busy_timeout(Duration::from_millis(1));
    let mut lock_holder = SqliteConnection::connect_with(&options).await.unwrap();
    sqlx::query("CREATE TABLE retry_test (value INTEGER NOT NULL)")
        .execute(&mut lock_holder)
        .await
        .unwrap();
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut lock_holder)
        .await
        .unwrap();

    let contender = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    let release = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(15)).await;
        sqlx::query("COMMIT")
            .execute(&mut lock_holder)
            .await
            .unwrap();
    });
    let metrics = SqliteBusyMetrics::default();
    let config = SqliteRetryConfig {
        foreground_attempts: 10,
        foreground_delay: Duration::from_millis(3),
        foreground_jitter: Duration::ZERO,
        ..SqliteRetryConfig::default()
    };
    retry_foreground(config, &metrics, || {
        sqlx::query("INSERT INTO retry_test(value) VALUES (1)").execute(&contender)
    })
    .await
    .unwrap();
    release.await.unwrap();
    assert!(metrics.count() > 0);
    assert!(metrics.wait() > Duration::ZERO);

    contender.close().await;
    remove_test_root(&root).await;
}

fn test_config(root: PathBuf) -> TenantDbConfig {
    TenantDbConfig {
        root,
        read_connections: 2,
        busy_timeout: Duration::from_millis(25),
        idle_timeout: Duration::from_secs(60),
        reaper_interval: Duration::from_secs(1),
    }
}

async fn provision_tenant() -> (PathBuf, Uuid) {
    let root = test_root();
    let tenant_id = Uuid::new_v4();
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
    pool.close().await;
    (root, tenant_id)
}

fn test_root() -> PathBuf {
    std::env::temp_dir().join(format!("arena360-tenant-db-test-{}", Uuid::new_v4()))
}

async fn remove_test_root(root: &Path) {
    tokio::fs::remove_dir_all(root).await.unwrap();
}
