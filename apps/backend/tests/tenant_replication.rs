use async_trait::async_trait;
use gaming_cafe_api::{
    error::AppError,
    metrics::Metrics,
    replication::{
        crypto::{self, TenantKeys},
        ledger::{Ledger, Segment},
        wal::{self, Capture},
        worker::{self, Worker},
    },
    tenancy::{tenant_path, TenantDb, TenantDbConfig, TenantDbManager, TenantLease},
};
use object_store::{memory::InMemory, path::Path as ObjectPath, ObjectStoreExt};
use sqlx::{sqlite::SqliteConnectOptions, Connection, SqliteConnection};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use uuid::Uuid;

struct Lease(AtomicBool);
impl TenantLease for Lease {
    fn writable_generation(&self, _: Uuid) -> Result<i64, AppError> {
        if self.0.load(Ordering::SeqCst) {
            Ok(1)
        } else {
            Err(AppError::Forbidden("fenced".into()))
        }
    }
    fn ensure_writable(&self, t: Uuid, g: i64) -> Result<(), AppError> {
        if self.writable_generation(t)? == g {
            Ok(())
        } else {
            Err(AppError::Forbidden("wrong generation".into()))
        }
    }
}
async fn database() -> (PathBuf, Arc<TenantDb>, Arc<Lease>) {
    let root = std::env::temp_dir().join(format!("arena360-replication-{}", Uuid::new_v4()));
    let tenant = Uuid::new_v4();
    let path = tenant_path(&root, tenant);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TABLE durable(value TEXT NOT NULL)")
        .execute(&mut connection)
        .await
        .unwrap();
    connection.close().await.unwrap();
    let lease = Arc::new(Lease(AtomicBool::new(true)));
    let manager = TenantDbManager::new(
        TenantDbConfig {
            root: root.clone(),
            ..Default::default()
        },
        lease.clone(),
    )
    .unwrap();
    (root, manager.open(tenant).await.unwrap(), lease)
}
async fn insert(db: &TenantDb, value: &str) {
    let value = value.to_owned();
    db.with_writer(move |c| {
        Box::pin(async move {
            sqlx::query("INSERT INTO durable VALUES(?)")
                .bind(value)
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
}
struct TestLedger {
    generation: Uuid,
    entries: Mutex<Vec<Segment>>,
    fail_manifest: AtomicBool,
    fence: Option<Arc<Lease>>,
}
impl Default for TestLedger {
    fn default() -> Self {
        Self {
            generation: Uuid::new_v4(),
            entries: Mutex::new(vec![]),
            fail_manifest: AtomicBool::new(false),
            fence: None,
        }
    }
}
#[async_trait]
impl Ledger for TestLedger {
    async fn reserve(&self, db: &TenantDb, capture: &Capture) -> Result<Segment, AppError> {
        let mut entries = self.entries.lock().unwrap();
        if let Some(existing) = entries
            .iter()
            .find(|s| s.capture.checksum == capture.checksum)
        {
            return Ok(existing.clone());
        }
        let number = entries.len() as i64 + 1;
        let entry = Segment {
            generation: self.generation,
            number,
            object_key: format!(
                "tenants/{}/replication/generations/{}/wal/{number:010}.wal.zst",
                db.tenant_id(),
                self.generation
            ),
            capture: capture.clone(),
            verified: false,
        };
        entries.push(entry.clone());
        Ok(entry)
    }
    async fn manifest(
        &self,
        _: &TenantDb,
        segment: &Segment,
        _: &str,
        _: i64,
    ) -> Result<Vec<u8>, AppError> {
        if self.fail_manifest.load(Ordering::SeqCst) {
            return Err(AppError::Internal("manifest unavailable".into()));
        }
        if let Some(lease) = &self.fence {
            lease.0.store(false, Ordering::SeqCst);
        }
        Ok(serde_json::to_vec(&serde_json::json!({"segments":[segment]})).unwrap())
    }
    async fn verified(
        &self,
        _: &TenantDb,
        segment: &Segment,
        _: &str,
        _: i64,
    ) -> Result<(), AppError> {
        self.entries
            .lock()
            .unwrap()
            .iter_mut()
            .find(|s| s.number == segment.number)
            .unwrap()
            .verified = true;
        Ok(())
    }
}
fn worker(root: &std::path::Path, db: &TenantDb, ledger: Arc<TestLedger>) -> Worker {
    let keys = TenantKeys::new(root.join("secrets"));
    wal::durable_create(&keys.path(db.tenant_id()), &[42u8; 32]).unwrap();
    Worker {
        store: Arc::new(InMemory::new()),
        ledger,
        keys,
        metrics: Arc::new(Metrics::default()),
    }
}

#[tokio::test]
async fn durable_capture_precedes_checkpoint_and_restores_committed_pages() {
    let (root, db, _) = database().await;
    let restore = root.join("restore.sqlite");
    std::fs::copy(db.path(), &restore).unwrap();
    insert(&db, "payment committed").await;
    let path = db.spool_wal().await.unwrap().unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let capture: Capture =
        serde_json::from_slice(&std::fs::read(path.with_extension("json")).unwrap()).unwrap();
    wal::validate(&bytes, capture.frames).unwrap();
    assert_eq!(
        std::fs::metadata(format!("{}-wal", db.path().display()))
            .unwrap()
            .len(),
        0
    );
    std::fs::write(format!("{}-wal", restore.display()), bytes).unwrap();
    let mut restored =
        SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&restore))
            .await
            .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT value FROM durable")
            .fetch_one(&mut restored)
            .await
            .unwrap(),
        "payment committed"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_one(&mut restored)
            .await
            .unwrap(),
        "ok"
    );
    restored.close().await.unwrap();
    db.close().await.unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn rollback_spill_is_excluded_and_reader_blocked_checkpoints_deduplicate() {
    let (root, db, _) = database().await;
    insert(&db, "keep").await;
    let mut reader = db.read_pool().unwrap().acquire().await.unwrap();
    sqlx::query("BEGIN").execute(&mut *reader).await.unwrap();
    let _: i64 = sqlx::query_scalar("SELECT count(*) FROM durable")
        .fetch_one(&mut *reader)
        .await
        .unwrap();
    db.with_writer(|c| {
        Box::pin(async move {
            sqlx::query("PRAGMA cache_size=10").execute(&mut *c).await?;
            sqlx::query("BEGIN IMMEDIATE").execute(&mut *c).await?;
            sqlx::query("INSERT INTO durable SELECT hex(zeroblob(8192)) FROM json_each(?)")
                .bind(serde_json::to_string(&vec![1; 100]).unwrap())
                .execute(&mut *c)
                .await?;
            sqlx::query("ROLLBACK").execute(c).await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    let a = db.spool_wal().await.unwrap().unwrap();
    let b = db.spool_wal().await.unwrap().unwrap();
    assert_eq!(a, b);
    assert_eq!(worker::pending(&db).unwrap().len(), 1);
    let capture: Capture =
        serde_json::from_slice(&std::fs::read(a.with_extension("json")).unwrap()).unwrap();
    assert!(capture.frames < 100);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM durable")
            .fetch_one(&mut *reader)
            .await
            .unwrap(),
        1
    );
    sqlx::query("ROLLBACK").execute(&mut *reader).await.unwrap();
    drop(reader);
    db.close().await.unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn spool_failure_withholds_checkpoint_and_close_cannot_discard_wal() {
    let (root, db, _) = database().await;
    insert(&db, "survives").await;
    let replication = db.path().parent().unwrap().join("replication");
    std::fs::write(&replication, b"blocked directory").unwrap();
    assert!(db.spool_wal().await.is_err());
    assert!(
        std::fs::metadata(format!("{}-wal", db.path().display()))
            .unwrap()
            .len()
            > 32
    );
    let wal_path = format!("{}-wal", db.path().display());
    assert!(db.close().await.is_err());
    drop(db);
    assert!(std::fs::metadata(wal_path).unwrap().len() > 32);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn failed_manifest_retries_same_ciphertext_then_deletes_only_after_verification() {
    let (root, db, _) = database().await;
    insert(&db, "retry").await;
    let path = db.spool_wal().await.unwrap().unwrap();
    let ledger = Arc::new(TestLedger::default());
    ledger.fail_manifest.store(true, Ordering::SeqCst);
    let worker = worker(&root, &db, ledger.clone());
    assert!(worker.ship(db.clone(), true).await.is_err());
    assert!(path.exists());
    let entry = ledger.entries.lock().unwrap()[0].clone();
    let remote = worker
        .store
        .get(&ObjectPath::from(entry.object_key.clone()))
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    ledger.fail_manifest.store(false, Ordering::SeqCst);
    assert_eq!(worker.ship(db.clone(), true).await.unwrap(), 1);
    let retried = worker
        .store
        .get(&ObjectPath::from(entry.object_key.clone()))
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert_eq!(remote, retried);
    assert!(!path.exists());
    assert!(ledger.entries.lock().unwrap()[0].verified);
    db.close().await.unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn upload_collision_and_ownership_loss_preserve_spool() {
    let (root, db, lease) = database().await;
    insert(&db, "fencing").await;
    let path = db.spool_wal().await.unwrap().unwrap();
    let ledger = Arc::new(TestLedger {
        fence: Some(lease.clone()),
        ..Default::default()
    });
    let worker = worker(&root, &db, ledger.clone());
    assert!(worker.ship(db.clone(), true).await.is_err());
    assert!(path.exists());
    assert!(!ledger.entries.lock().unwrap()[0].verified);
    assert!(worker.put_verified("immutable", b"one").await.is_ok());
    assert!(worker.put_verified("immutable", b"two").await.is_err());
    db.close().await.unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn encryption_authenticates_tenant_path_and_key_and_bounds_decompression() {
    let key = [7u8; 32];
    let encoded = crypto::encode(&key, "tenant/one", b"secret payment").unwrap();
    assert_eq!(
        crypto::decode(&key, "tenant/one", &encoded, 100).unwrap(),
        b"secret payment"
    );
    assert!(crypto::decode(&key, "tenant/two", &encoded, 100).is_err());
    assert!(crypto::decode(&[8u8; 32], "tenant/one", &encoded, 100).is_err());
    assert!(crypto::decode(&key, "tenant/one", &encoded, 2).is_err());
    let mut corrupt = encoded;
    corrupt[25] ^= 1;
    assert!(crypto::decode(&key, "tenant/one", &corrupt, 100).is_err());
}

#[tokio::test]
async fn restart_reopens_pending_spool_and_capture_order_survives_clock_changes() {
    let (root, db, lease) = database().await;
    insert(&db, "first").await;
    let first = db.spool_wal().await.unwrap().unwrap();
    db.close().await.unwrap();
    let manager = TenantDbManager::new(
        TenantDbConfig {
            root: root.clone(),
            ..Default::default()
        },
        lease,
    )
    .unwrap();
    manager.open_spooled().await.unwrap();
    assert_eq!(manager.open_count().await, 1);
    let resumed = manager.open(db.tenant_id()).await.unwrap();
    insert(&resumed, "second").await;
    let second = resumed.spool_wal().await.unwrap().unwrap();
    let path = second.with_extension("json");
    let mut capture: Capture = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    capture.captured_at = "2000-01-01T00:00:00.000000Z".into();
    std::fs::write(&path, serde_json::to_vec(&capture).unwrap()).unwrap();
    let pending = worker::pending(&resumed).unwrap();
    assert_eq!(pending[0].0, first);
    assert_eq!(pending[1].0, second);
    assert!(pending[0].1.capture_number < pending[1].1.capture_number);
    resumed.close().await.unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore = "requires an isolated control-plane database"]
async fn postgres_manifest_reservations_are_idempotent_ordered_and_lease_fenced() {
    use gaming_cafe_api::replication::ledger::PostgresLedger;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(3)
        .connect(&std::env::var("CONTROL_TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    gaming_cafe_api::control::migrate(&pool).await.unwrap();
    let (root, db, _) = database().await;
    let tenant = db.tenant_id();
    let cell = Uuid::new_v4();
    sqlx::query("INSERT INTO cells(id,name,address) VALUES($1,$2,$3)")
        .bind(cell)
        .bind(format!("replication-{cell}"))
        .bind(format!("http://{cell}.invalid"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO tenants(id,slug,name,timezone,owner_cell,ownership_generation,state) VALUES($1,$2,'Replication test','UTC',$3,1,'ACTIVE')").bind(tenant).bind(format!("replication-{tenant}")).bind(cell).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tenant_leases(tenant_id,owner_cell,ownership_generation,expires_at) VALUES($1,$2,1,NOW()+INTERVAL '5 minutes')").bind(tenant).bind(cell).execute(&pool).await.unwrap();
    let ledger = Arc::new(PostgresLedger {
        pool: pool.clone(),
        cell_id: cell,
    });
    let keys = TenantKeys::new(root.join("secrets"));
    wal::durable_create(&keys.path(tenant), &[42u8; 32]).unwrap();
    let worker = Worker {
        store: Arc::new(InMemory::new()),
        ledger: ledger.clone(),
        keys,
        metrics: Arc::new(Metrics::default()),
    };
    insert(&db, "one").await;
    db.spool_wal().await.unwrap();
    let capture = worker::pending(&db).unwrap()[0].1.clone();
    let first = ledger.reserve(&db, &capture).await.unwrap();
    let retry = ledger.reserve(&db, &capture).await.unwrap();
    assert_eq!(first.number, retry.number);
    assert_eq!(first.generation, retry.generation);
    assert_eq!(worker.ship(db.clone(), true).await.unwrap(), 1);
    insert(&db, "two").await;
    db.spool_wal().await.unwrap();
    let capture = worker::pending(&db).unwrap()[0].1.clone();
    let second = ledger.reserve(&db, &capture).await.unwrap();
    assert_eq!(second.number, 2);
    assert_eq!(second.generation, first.generation);
    sqlx::query("UPDATE tenant_leases SET expires_at=NOW()+INTERVAL '10 seconds',renewed_at=NOW() WHERE tenant_id=$1").bind(tenant).execute(&pool).await.unwrap();
    assert!(ledger.reserve(&db, &capture).await.is_err());
    assert!(ledger
        .verified(&db, &second, &"a".repeat(64), 10)
        .await
        .is_err());
    assert_eq!(worker::pending(&db).unwrap().len(), 1);
    sqlx::query("UPDATE tenant_leases SET expires_at=NOW()+INTERVAL '5 minutes',renewed_at=NOW() WHERE tenant_id=$1").bind(tenant).execute(&pool).await.unwrap();
    assert_eq!(worker.ship(db.clone(), true).await.unwrap(), 1);
    let verified:i64=sqlx::query_scalar("SELECT count(*) FROM replication_segments WHERE generation_id=$1 AND verified_at IS NOT NULL").bind(first.generation).fetch_one(&pool).await.unwrap();
    assert_eq!(verified, 2);
    use gaming_cafe_api::replication::ledger::GenerationReason;
    let transition = Uuid::new_v4();
    let restored = ledger
        .start_generation(&db, GenerationReason::Restore, transition)
        .await
        .unwrap();
    assert_ne!(restored, first.generation);
    assert_eq!(
        ledger
            .start_generation(&db, GenerationReason::Restore, transition)
            .await
            .unwrap(),
        restored
    );
    let old_state: String =
        sqlx::query_scalar("SELECT state FROM replication_generations WHERE id=$1")
            .bind(first.generation)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(old_state, "RESTORED");
    assert!(ledger
        .verified(&db, &second, &"a".repeat(64), 10)
        .await
        .is_err());
    insert(&db, "three").await;
    db.spool_wal().await.unwrap();
    let three = worker::pending(&db).unwrap()[0].1.clone();
    let third = ledger.reserve(&db, &three).await.unwrap();
    assert_eq!(third.generation, restored);
    assert_eq!(third.number, 1);
    let mut missing = three.clone();
    missing.capture_number += 2;
    missing.checksum = "b".repeat(64);
    let gap = ledger.reserve(&db, &missing).await.unwrap();
    assert_ne!(gap.generation, restored);
    assert_eq!(gap.number, 1);
    let state: String = sqlx::query_scalar("SELECT state FROM replication_generations WHERE id=$1")
        .bind(restored)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, "GAPPED");
    assert!(ledger
        .start_generation(&db, GenerationReason::Restore, transition)
        .await
        .is_err());
    let current: Uuid =
        sqlx::query_scalar("SELECT current_replication_generation FROM tenants WHERE id=$1")
            .bind(tenant)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(current, gap.generation);
    // Authoritative ownership can change while this process still has an old
    // locally cached lease. PostgreSQL must reject its old manifest writes.
    sqlx::query("UPDATE tenants SET ownership_generation=2 WHERE id=$1")
        .bind(tenant)
        .execute(&pool)
        .await
        .unwrap();
    assert!(ledger
        .verified(&db, &second, &"a".repeat(64), 10)
        .await
        .is_err());
    db.close().await.unwrap();
    sqlx::query("DELETE FROM tenants WHERE id=$1")
        .bind(tenant)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM cells WHERE id=$1")
        .bind(cell)
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    std::fs::remove_dir_all(root).unwrap();
}
