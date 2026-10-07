use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use async_trait::async_trait;
use gaming_cafe_api::error::AppError;
use gaming_cafe_api::tenancy::{
    target_schema_version, tenant_path, write_outbox_event_on_connection, MigrationContext,
    MigrationHook, MigrationOrchestrator, MigrationOrchestratorConfig, MigrationState,
    NewOutboxEvent, PendingTenantMigration, TenantCommitNotifier, TenantDbConfig, TenantDbManager,
    TenantLease,
};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Connection, SqliteConnection};
use uuid::Uuid;

#[derive(Default)]
struct FakeLease {
    generations: RwLock<HashMap<Uuid, i64>>,
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
            Err(AppError::Forbidden("tenant lease changed".into()))
        }
    }
}

#[derive(Default)]
struct RecordingNotifier {
    registered: Mutex<Vec<(Uuid, i64)>>,
    committed: Mutex<Vec<(Uuid, Vec<i64>)>>,
}

impl TenantCommitNotifier for RecordingNotifier {
    fn registered(&self, tenant_id: Uuid, current_sequence: i64) {
        self.registered
            .lock()
            .unwrap()
            .push((tenant_id, current_sequence));
    }

    fn committed(&self, tenant_id: Uuid, sequences: &[i64]) {
        self.committed
            .lock()
            .unwrap()
            .push((tenant_id, sequences.to_vec()));
    }
}

struct FakeMigrationState {
    migrations: Mutex<HashMap<Uuid, PendingTenantMigration>>,
    fail_record_tenant: Uuid,
    failed_record_once: AtomicBool,
}

#[async_trait]
impl MigrationState for FakeMigrationState {
    async fn pending(
        &self,
        _cell_id: Uuid,
        target_version: i64,
    ) -> Result<Vec<PendingTenantMigration>, AppError> {
        Ok(self
            .migrations
            .lock()
            .unwrap()
            .values()
            .filter(|migration| migration.schema_version < target_version)
            .copied()
            .collect())
    }

    async fn record_version(
        &self,
        _cell_id: Uuid,
        migration: PendingTenantMigration,
        version: i64,
    ) -> Result<(), AppError> {
        if migration.tenant_id == self.fail_record_tenant
            && !self.failed_record_once.swap(true, Ordering::SeqCst)
        {
            return Err(AppError::Internal(
                "injected control-plane record failure".into(),
            ));
        }
        let mut migrations = self.migrations.lock().unwrap();
        let current = migrations
            .get_mut(&migration.tenant_id)
            .ok_or_else(|| AppError::NotFound("tenant migration state missing".into()))?;
        if current.ownership_generation != migration.ownership_generation {
            return Err(AppError::Conflict("tenant ownership changed".into()));
        }
        current.schema_version = version;
        Ok(())
    }
}

struct TrackingHook {
    fail_tenant: Uuid,
    failed_once: AtomicBool,
    active: AtomicUsize,
    max_active: AtomicUsize,
    before_count: AtomicUsize,
    after_count: AtomicUsize,
}

impl TrackingHook {
    fn new(fail_tenant: Uuid) -> Self {
        Self {
            fail_tenant,
            failed_once: AtomicBool::new(false),
            active: AtomicUsize::new(0),
            max_active: AtomicUsize::new(0),
            before_count: AtomicUsize::new(0),
            after_count: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl MigrationHook for TrackingHook {
    async fn before(
        &self,
        context: MigrationContext,
        _connection: &mut SqliteConnection,
    ) -> Result<(), AppError> {
        self.before_count.fetch_add(1, Ordering::Relaxed);
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_active.fetch_max(active, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(5)).await;
        self.active.fetch_sub(1, Ordering::SeqCst);
        if context.tenant_id == self.fail_tenant && !self.failed_once.swap(true, Ordering::SeqCst) {
            return Err(AppError::Internal("injected pre-migration failure".into()));
        }
        Ok(())
    }

    async fn after(
        &self,
        _context: MigrationContext,
        _connection: &mut SqliteConnection,
    ) -> Result<(), AppError> {
        self.after_count.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

#[tokio::test]
async fn migrates_fifty_tenants_with_bounded_concurrency_and_resumes_failure() {
    let root = test_root();
    tokio::fs::create_dir_all(&root).await.unwrap();
    let tenant_ids = provision_empty_tenants(&root, 50).await;
    let fail_tenant = tenant_ids[17];
    let fail_record_tenant = tenant_ids[23];
    let lease = Arc::new(FakeLease::default());
    let migrations = tenant_ids
        .iter()
        .map(|tenant_id| {
            lease.generations.write().unwrap().insert(*tenant_id, 1);
            (
                *tenant_id,
                PendingTenantMigration {
                    tenant_id: *tenant_id,
                    ownership_generation: 1,
                    schema_version: 0,
                },
            )
        })
        .collect();
    let state = Arc::new(FakeMigrationState {
        migrations: Mutex::new(migrations),
        fail_record_tenant,
        failed_record_once: AtomicBool::new(false),
    });
    let notifier = Arc::new(RecordingNotifier::default());
    let manager = Arc::new(
        TenantDbManager::new(
            TenantDbConfig {
                root: root.clone(),
                read_connections: 1,
                busy_timeout: Duration::from_millis(100),
                idle_timeout: Duration::from_secs(60),
                reaper_interval: Duration::from_secs(1),
            },
            lease,
        )
        .unwrap()
        .with_commit_notifier(notifier.clone()),
    );
    let hook = Arc::new(TrackingHook::new(fail_tenant));
    let orchestrator = MigrationOrchestrator::new(
        Uuid::new_v4(),
        manager.clone(),
        state.clone(),
        vec![hook.clone()],
        MigrationOrchestratorConfig { max_concurrency: 4 },
    )
    .unwrap();

    let first = orchestrator.run_pending().await.unwrap();
    assert_eq!(first.len(), 50);
    assert_eq!(
        first.iter().filter(|outcome| outcome.succeeded()).count(),
        48
    );
    assert_eq!(
        first.iter().filter(|outcome| !outcome.succeeded()).count(),
        2
    );
    assert!(hook.max_active.load(Ordering::SeqCst) > 1);
    assert!(hook.max_active.load(Ordering::SeqCst) <= 4);

    let migrated_tenant = tenant_ids[0];
    assert!(notifier
        .registered
        .lock()
        .unwrap()
        .contains(&(migrated_tenant, 0)));
    let migrated_handle = manager.open(migrated_tenant).await.unwrap();
    migrated_handle
        .with_immediate_writer(|connection| {
            Box::pin(async move {
                write_outbox_event_on_connection(
                    connection,
                    NewOutboxEvent {
                        location_id: None,
                        aggregate_type: "device".into(),
                        aggregate_id: Uuid::new_v4(),
                        event_type: "device.status_changed".into(),
                        schema_version: 1,
                        deleted: false,
                        payload: serde_json::json!({"status":"available"}),
                    },
                )
                .await?;
                Ok(())
            })
        })
        .await
        .unwrap();
    assert!(notifier
        .committed
        .lock()
        .unwrap()
        .contains(&(migrated_tenant, vec![1])));

    let resumed = orchestrator.run_pending().await.unwrap();
    assert_eq!(resumed.len(), 2);
    assert!(resumed.iter().all(|outcome| outcome.succeeded()));
    assert!(resumed
        .iter()
        .any(|outcome| outcome.tenant_id == fail_tenant));
    assert!(resumed
        .iter()
        .any(|outcome| outcome.tenant_id == fail_record_tenant));
    assert_eq!(hook.before_count.load(Ordering::Relaxed), 51);
    assert_eq!(hook.after_count.load(Ordering::Relaxed), 51);
    assert!(state
        .migrations
        .lock()
        .unwrap()
        .values()
        .all(|migration| migration.schema_version == target_schema_version()));

    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(tenant_path(&root, fail_tenant)),
    )
    .await
    .unwrap();
    let version: i64 =
        sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations WHERE success")
            .fetch_one(&mut connection)
            .await
            .unwrap();
    assert_eq!(version, target_schema_version());

    tokio::fs::remove_dir_all(&root).await.unwrap();
}

async fn provision_empty_tenants(root: &Path, count: usize) -> Vec<Uuid> {
    let mut tenant_ids = Vec::with_capacity(count);
    for _ in 0..count {
        let tenant_id = Uuid::new_v4();
        let path = tenant_path(root, tenant_id);
        tokio::fs::create_dir_all(path.parent().unwrap())
            .await
            .unwrap();
        SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true),
        )
        .await
        .unwrap()
        .close()
        .await
        .unwrap();
        tenant_ids.push(tenant_id);
    }
    tenant_ids
}

fn test_root() -> PathBuf {
    std::env::temp_dir().join(format!("arena360-migration-test-{}", Uuid::new_v4()))
}
