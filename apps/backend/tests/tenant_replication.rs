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
        atomic::{AtomicBool, AtomicI64, Ordering},
        Arc, Mutex,
    },
};
use uuid::Uuid;

struct Lease(AtomicBool, AtomicI64);
impl TenantLease for Lease {
    fn writable_generation(&self, _: Uuid) -> Result<i64, AppError> {
        if self.0.load(Ordering::SeqCst) {
            Ok(self.1.load(Ordering::SeqCst))
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
    let lease = Arc::new(Lease(AtomicBool::new(true), AtomicI64::new(1)));
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
    async fn baseline(
        &self,
        _db: Arc<TenantDb>,
        _generation: Uuid,
        _store: &dyn object_store::ObjectStore,
        _keys: &TenantKeys,
    ) -> Result<(), AppError> {
        Ok(())
    }
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
        Ok(
            serde_json::to_vec(
                &serde_json::json!({"revision":segment.number,"segments":[segment]}),
            )
            .unwrap(),
        )
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
        gates: Default::default(),
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

#[test]
fn streaming_snapshot_envelope_rejects_truncation_tampering_and_wrong_tenant() {
    let key = [9u8; 32];
    let source = (0..200_000u32)
        .flat_map(|i| i.wrapping_mul(2_654_435_761).to_le_bytes())
        .collect::<Vec<_>>();
    let mut encoded = vec![];
    crypto::encode_file(&key, "snapshot/one", source.as_slice(), &mut encoded).unwrap();
    let mut output = vec![];
    assert_eq!(
        crypto::decode_file(
            &key,
            "snapshot/one",
            encoded.as_slice(),
            &mut output,
            source.len() as u64
        )
        .unwrap(),
        source.len() as u64
    );
    assert_eq!(output, source);
    for invalid in [
        &encoded[..encoded.len() - 20],
        &encoded[..encoded.len() - 1],
    ] {
        assert!(crypto::decode_file(
            &key,
            "snapshot/one",
            invalid,
            std::io::sink(),
            source.len() as u64
        )
        .is_err());
    }
    assert!(crypto::decode_file(
        &key,
        "snapshot/two",
        encoded.as_slice(),
        std::io::sink(),
        source.len() as u64
    )
    .is_err());
    assert!(crypto::decode_file(
        &key,
        "snapshot/one",
        encoded.as_slice(),
        std::io::sink(),
        10
    )
    .is_err());
    encoded[30] ^= 1;
    assert!(crypto::decode_file(
        &key,
        "snapshot/one",
        encoded.as_slice(),
        std::io::sink(),
        source.len() as u64
    )
    .is_err());
}

#[tokio::test]
async fn legacy_wal_manifest_advances_without_reusing_an_immutable_revision() {
    let (root, db, _) = database().await;
    let store = InMemory::new();
    let generation = Uuid::new_v4();
    let prefix = format!(
        "tenants/{}/replication/generations/{generation}",
        db.tenant_id()
    );
    let old = serde_json::to_vec(&serde_json::json!({"segments":[{"number":9}]})).unwrap();
    worker::put_verified(&store, &format!("{prefix}/manifest.json"), &old)
        .await
        .unwrap();
    worker::put_verified(&store, &format!("{prefix}/manifests/0000000009.json"), &old)
        .await
        .unwrap();
    let next = serde_json::to_vec(
        &serde_json::json!({"revision":10,"segments":[{"number":9}],"snapshots":[]}),
    )
    .unwrap();
    worker::publish_document(&store, &db, generation, &next)
        .await
        .unwrap();
    let immutable = store
        .get(&ObjectPath::from(format!(
            "{prefix}/manifests/0000000009.json"
        )))
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert_eq!(immutable.as_ref(), old);
    let current = store
        .get(&ObjectPath::from(format!("{prefix}/manifest.json")))
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert_eq!(current.as_ref(), next);
    db.close().await.unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn two_minute_upload_combines_captures_and_resumes_partial_verified_cleanup() {
    use gaming_cafe_api::replication::batch;
    let (root, db, _) = database().await;
    for value in ["one", "two", "three"] {
        insert(&db, value).await;
        db.spool_wal().await.unwrap();
    }
    let ledger = Arc::new(TestLedger::default());
    let worker = worker(&root, &db, ledger.clone());
    assert_eq!(worker.ship(db.clone(), false).await.unwrap(), 0);
    let pending = worker::pending(&db).unwrap();
    let metadata = pending[0].0.with_extension("json");
    let mut old = pending[0].1.clone();
    old.captured_at = gaming_cafe_api::time::format_sqlite_timestamp(
        &(chrono::Utc::now() - chrono::Duration::seconds(121)),
    )
    .unwrap();
    std::fs::write(metadata, serde_json::to_vec(&old).unwrap()).unwrap();
    let prepared = batch::prepare(&db, &worker::pending(&db).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(prepared.files.len(), 3);
    assert_eq!(worker.ship(db.clone(), false).await.unwrap(), 3);
    assert!(worker::pending(&db).unwrap().is_empty());
    let entry = ledger.entries.lock().unwrap()[0].clone();
    assert_eq!(ledger.entries.lock().unwrap().len(), 1);
    assert_eq!(entry.capture.range_end(), 3);
    let encoded = worker
        .store
        .get(&ObjectPath::from(entry.object_key.clone()))
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    let source = crypto::decode(
        &worker.keys.read(db.tenant_id()).unwrap(),
        &entry.object_key,
        &encoded,
        16 * 1024 * 1024,
    )
    .unwrap();
    let records = batch::decode(&source).unwrap();
    assert_eq!(records.len(), 3);
    assert_eq!(
        records
            .iter()
            .map(|(c, _)| c.capture_number)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    // A crash after source deletion but before descriptor cleanup must not
    // trigger another upload or wait for new writes to resume cleanup.
    wal::durable_create(
        &batch::descriptor(&db),
        &serde_json::to_vec(&prepared).unwrap(),
    )
    .unwrap();
    assert_eq!(worker.ship(db.clone(), false).await.unwrap(), 3);
    assert!(!batch::descriptor(&db).exists());
    assert_eq!(ledger.entries.lock().unwrap().len(), 1);
    db.close().await.unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn acknowledged_reader_pinned_wal_does_not_upload_again_without_new_writes() {
    let (root, db, _) = database().await;
    insert(&db, "one").await;
    let mut reader = db.read_pool().unwrap().acquire().await.unwrap();
    sqlx::query("BEGIN").execute(&mut *reader).await.unwrap();
    let _: i64 = sqlx::query_scalar("SELECT count(*) FROM durable")
        .fetch_one(&mut *reader)
        .await
        .unwrap();
    db.spool_wal().await.unwrap();
    let ledger = Arc::new(TestLedger::default());
    let worker = worker(&root, &db, ledger.clone());
    assert_eq!(worker.ship(db.clone(), true).await.unwrap(), 1);
    assert!(db.spool_wal().await.unwrap().is_none());
    assert!(worker::pending(&db).unwrap().is_empty());
    assert_eq!(worker.ship(db.clone(), true).await.unwrap(), 0);
    assert_eq!(ledger.entries.lock().unwrap().len(), 1);
    sqlx::query("ROLLBACK").execute(&mut *reader).await.unwrap();
    drop(reader);
    db.close().await.unwrap();
    std::fs::remove_dir_all(root).unwrap();
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
    let (root, db, lease) = database().await;
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
        gates: Default::default(),
        store: Arc::new(InMemory::new()),
        ledger: ledger.clone(),
        keys,
        metrics: Arc::new(Metrics::default()),
    };
    let missing_keys = TenantKeys::new(root.join("missing-keys"));
    assert!(gaming_cafe_api::replication::snapshot::take(
        db.clone(),
        &ledger,
        worker.store.as_ref(),
        &missing_keys,
        gaming_cafe_api::replication::snapshot::Kind::Baseline
    )
    .await
    .is_err());
    assert!(!db
        .path()
        .parent()
        .unwrap()
        .join("replication/snapshots")
        .exists());
    insert(&db, "one").await;
    db.spool_wal().await.unwrap();
    let capture = gaming_cafe_api::replication::batch::prepare(&db, &worker::pending(&db).unwrap())
        .unwrap()
        .unwrap()
        .capture;
    let first = ledger.reserve(&db, &capture).await.unwrap();
    let retry = ledger.reserve(&db, &capture).await.unwrap();
    assert_eq!(first.number, retry.number);
    assert_eq!(first.generation, retry.generation);
    assert_eq!(worker.ship(db.clone(), true).await.unwrap(), 1);
    insert(&db, "two").await;
    db.spool_wal().await.unwrap();
    let capture = gaming_cafe_api::replication::batch::prepare(&db, &worker::pending(&db).unwrap())
        .unwrap()
        .unwrap()
        .capture;
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
    use gaming_cafe_api::replication::snapshot::{self, Kind, SnapshotHook};
    snapshot::take(
        db.clone(),
        &ledger,
        worker.store.as_ref(),
        &worker.keys,
        Kind::Daily,
    )
    .await
    .unwrap();
    snapshot::take(
        db.clone(),
        &ledger,
        worker.store.as_ref(),
        &worker.keys,
        Kind::Daily,
    )
    .await
    .unwrap();
    let daily:i64=sqlx::query_scalar("SELECT count(*) FROM snapshot_manifests WHERE generation_id=$1 AND kind='DAILY' AND verified_at IS NOT NULL").bind(first.generation).fetch_one(&pool).await.unwrap();
    assert_eq!(daily, 1);
    let row:(String,String,i64)=sqlx::query_as("SELECT object_key,source_checksum_sha256,capture_number FROM snapshot_manifests WHERE generation_id=$1 AND kind='DAILY'").bind(first.generation).fetch_one(&pool).await.unwrap();
    let bytes = worker
        .store
        .get(&ObjectPath::from(row.0.clone()))
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    let mut image = vec![];
    crypto::decode_file(
        &worker.keys.read(tenant).unwrap(),
        &row.0,
        bytes.as_ref(),
        &mut image,
        64 * 1024 * 1024,
    )
    .unwrap();
    assert_eq!(wal::checksum(&image), row.1);
    assert!(row.2 >= 2);
    let restored_file = root.join("snapshot-restored.sqlite");
    std::fs::write(&restored_file, image).unwrap();
    let mut restored_connection =
        SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&restored_file))
            .await
            .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM durable")
            .fetch_one(&mut restored_connection)
            .await
            .unwrap(),
        2
    );
    restored_connection.close().await.unwrap();
    db.close().await.unwrap();
    let manager = Arc::new(
        TenantDbManager::new(
            TenantDbConfig {
                root: root.clone(),
                ..Default::default()
            },
            lease.clone(),
        )
        .unwrap(),
    );
    let db = manager.open(tenant).await.unwrap();
    let hook = Arc::new(SnapshotHook {
        databases: manager.clone(),
        ledger: ledger.clone(),
        store: worker.store.clone(),
        keys: Arc::new(worker.keys.clone()),
    });
    let orchestrator = gaming_cafe_api::tenancy::MigrationOrchestrator::new(
        cell,
        manager,
        Arc::new(gaming_cafe_api::tenancy::PostgresMigrationState::new(
            pool.clone(),
        )),
        vec![hook.clone()],
        Default::default(),
    )
    .unwrap();
    let migrated = orchestrator.run_pending().await.unwrap();
    assert_eq!(migrated.len(), 1);
    assert!(migrated[0].succeeded(), "{:?}", migrated);
    let migration_snapshots:Vec<(String,i64)>=sqlx::query_as("SELECT kind,schema_version FROM snapshot_manifests WHERE generation_id=$1 AND kind IN ('PRE_MIGRATION','POST_MIGRATION') AND verified_at IS NOT NULL ORDER BY snapshot_at").bind(first.generation).fetch_all(&pool).await.unwrap();
    assert_eq!(
        migration_snapshots,
        vec![
            ("PRE_MIGRATION".into(), 0),
            (
                "POST_MIGRATION".into(),
                gaming_cafe_api::tenancy::target_schema_version()
            )
        ]
    );
    let hook_copy = hook.clone();
    let lease_copy = lease.clone();
    db.with_writer(move |connection| {
        Box::pin(async move {
            use gaming_cafe_api::tenancy::{MigrationContext, MigrationHook};
            lease_copy.1.store(2, Ordering::SeqCst);
            let result = tokio::time::timeout(
                std::time::Duration::from_millis(200),
                hook_copy.before(
                    MigrationContext {
                        tenant_id: tenant,
                        ownership_generation: 1,
                        from_version: 16,
                        to_version: 16,
                    },
                    connection,
                ),
            )
            .await;
            lease_copy.1.store(1, Ordering::SeqCst);
            assert!(result
                .expect("Ownership loss must not deadlock a migration hook")
                .is_err());
            Ok(())
        })
    })
    .await
    .unwrap();
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
    lease.1.store(2, Ordering::SeqCst);
    sqlx::query("UPDATE tenant_leases SET ownership_generation=2,renewed_at=NOW(),expires_at=NOW()+INTERVAL '5 minutes' WHERE tenant_id=$1").bind(tenant).execute(&pool).await.unwrap();
    let next_manager = TenantDbManager::new(
        TenantDbConfig {
            root: root.clone(),
            ..Default::default()
        },
        lease,
    )
    .unwrap();
    let next_db = next_manager.open(tenant).await.unwrap();
    insert(&next_db, "new owner").await;
    next_db.spool_wal().await.unwrap();
    let next = ledger
        .reserve(&next_db, &worker::pending(&next_db).unwrap()[0].1)
        .await
        .unwrap();
    assert_ne!(next.generation, gap.generation);
    assert_eq!(next.number, 1);
    let ownership: i64 =
        sqlx::query_scalar("SELECT ownership_generation FROM replication_generations WHERE id=$1")
            .bind(next.generation)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(ownership, 2);
    next_db.close().await.unwrap();
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

#[tokio::test]
#[ignore = "requires an isolated control-plane database"]
async fn postgres_retention_preserves_restore_dependencies_resumes_intents_and_fences_owner() {
    use gaming_cafe_api::replication::{ledger::PostgresLedger, retention};
    let pool=sqlx::postgres::PgPoolOptions::new().max_connections(3).connect(&std::env::var("CONTROL_TEST_DATABASE_URL").unwrap()).await.unwrap();
    gaming_cafe_api::control::migrate(&pool).await.unwrap();
    let (root,db,lease)=database().await;
    let tenant=db.tenant_id();let cell=Uuid::new_v4();
    sqlx::query("INSERT INTO cells(id,name,address) VALUES($1,$2,$3)").bind(cell).bind(format!("retention-{cell}")).bind(format!("http://{cell}.invalid")).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tenants(id,slug,name,timezone,owner_cell,ownership_generation,state) VALUES($1,$2,'Retention','UTC',$3,1,'ACTIVE')").bind(tenant).bind(format!("retention-{tenant}")).bind(cell).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tenant_leases(tenant_id,owner_cell,ownership_generation,expires_at) VALUES($1,$2,1,NOW()+INTERVAL '5 minutes')").bind(tenant).bind(cell).execute(&pool).await.unwrap();
    let ledger=PostgresLedger{pool:pool.clone(),cell_id:cell};
    let generation=ledger.ensure_generation(&db).await.unwrap();
    let store=InMemory::new();
    let prefix=format!("tenants/{tenant}/replication/generations/{generation}");
    let now=chrono::Utc::now();
    let mut snapshots=vec![];
    for (days,capture) in [(130i64,5i64),(100,10),(20,40)] {
        let id=Uuid::new_v4();let key=format!("{prefix}/snapshots/{id}");
        store.put(&ObjectPath::from(key.clone()),bytes::Bytes::from_static(b"snapshot").into()).await.unwrap();
        sqlx::query("INSERT INTO snapshot_manifests(id,tenant_id,generation_id,kind,schema_version,object_key,checksum_sha256,encrypted_size_bytes,snapshot_at,verified_at,capture_number,source_checksum_sha256) VALUES($1,$2,$3,'MANUAL',0,$4,$5,8,$6,NOW(),$7,$5)").bind(id).bind(tenant).bind(generation).bind(&key).bind("a".repeat(64)).bind(now-chrono::Duration::days(days)).bind(capture).execute(&pool).await.unwrap();
        snapshots.push((id,key));
    }
    let template=Capture{version:2,capture_number:1,ownership_generation:1,captured_at:gaming_cafe_api::time::format_sqlite_timestamp(&(now-chrono::Duration::days(120))).unwrap(),salt:"fixture".into(),page_size:4096,frames:1,checksum:"b".repeat(64),last_capture_number:Some(5),last_captured_at:None};
    let mut segment_keys=vec![];
    for (number,days,last) in [(1i64,120i64,5u64),(2,95,11),(3,89,12)] {
        let key=format!("{prefix}/wal/{number}");let mut capture=template.clone();
        capture.capture_number=if number==1 {1} else {last};
        capture.last_capture_number=Some(last);
        capture.last_captured_at=Some(gaming_cafe_api::time::format_sqlite_timestamp(&(now-chrono::Duration::days(days))).unwrap());
        store.put(&ObjectPath::from(key.clone()),bytes::Bytes::from_static(b"wal").into()).await.unwrap();
        sqlx::query("INSERT INTO replication_segments(generation_id,segment_number,source_checksum,object_key,capture,checksum_sha256,encrypted_size_bytes,verified_at) VALUES($1,$2,$3,$4,$5,$3,3,NOW())").bind(generation).bind(number).bind(format!("{number:064x}")).bind(&key).bind(serde_json::to_value(capture).unwrap()).execute(&pool).await.unwrap();
        segment_keys.push(key);
    }
    // Unverified rows must survive even if their timestamp is ancient.
    let unverified=Uuid::new_v4();
    sqlx::query("INSERT INTO snapshot_manifests(id,tenant_id,generation_id,kind,schema_version,object_key,checksum_sha256,encrypted_size_bytes,snapshot_at,capture_number,source_checksum_sha256) VALUES($1,$2,$3,'MANUAL',0,$4,$5,8,NOW()-INTERVAL '200 days',1,$5)").bind(unverified).bind(tenant).bind(generation).bind(format!("{prefix}/unverified")).bind("c".repeat(64)).execute(&pool).await.unwrap();
    assert_eq!(retention::run(&db,&ledger,&store).await.unwrap(),2);
    assert!(store.head(&ObjectPath::from(snapshots[0].1.clone())).await.is_err());
    assert!(store.head(&ObjectPath::from(segment_keys[0].clone())).await.is_err());
    for key in [&snapshots[1].1,&snapshots[2].1,&segment_keys[1],&segment_keys[2]] {assert!(store.head(&ObjectPath::from(key.clone())).await.is_ok());}
    let retired:bool=sqlx::query_scalar("SELECT retired_at IS NOT NULL AND deleted_at IS NOT NULL FROM snapshot_manifests WHERE id=$1").bind(snapshots[0].0).fetch_one(&pool).await.unwrap();assert!(retired);
    let untouched:bool=sqlx::query_scalar("SELECT retired_at IS NULL FROM snapshot_manifests WHERE id=$1").bind(unverified).fetch_one(&pool).await.unwrap();assert!(untouched);
    // Simulate a crash after remote deletion but before the completion receipt.
    sqlx::query("UPDATE snapshot_manifests SET deleted_at=NULL WHERE id=$1").bind(snapshots[0].0).execute(&pool).await.unwrap();
    assert_eq!(retention::run(&db,&ledger,&store).await.unwrap(),1);
    assert_eq!(retention::run(&db,&ledger,&store).await.unwrap(),0);
    // A stale local lease cannot override authoritative control ownership.
    sqlx::query("UPDATE tenant_leases SET expires_at=NOW()+INTERVAL '20 seconds' WHERE tenant_id=$1").bind(tenant).execute(&pool).await.unwrap();
    assert!(retention::run(&db,&ledger,&store).await.is_err());
    lease.0.store(false,Ordering::SeqCst);
    assert!(retention::run(&db,&ledger,&store).await.is_err());
    lease.0.store(true,Ordering::SeqCst);
    db.close().await.unwrap();
    sqlx::query("DELETE FROM tenants WHERE id=$1").bind(tenant).execute(&pool).await.unwrap();
    sqlx::query("DELETE FROM cells WHERE id=$1").bind(cell).execute(&pool).await.unwrap();
    pool.close().await;std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore = "requires an isolated control-plane database"]
async fn postgres_restore_replays_encrypted_batches_and_stops_at_capture_utc() {
    use gaming_cafe_api::replication::{ledger::PostgresLedger,restore,snapshot};
    let pool=sqlx::postgres::PgPoolOptions::new().max_connections(4).connect(&std::env::var("CONTROL_TEST_DATABASE_URL").unwrap()).await.unwrap();
    gaming_cafe_api::control::migrate(&pool).await.unwrap();
    let (root,db,lease)=database().await;let tenant=db.tenant_id();let cell=Uuid::new_v4();
    sqlx::query("INSERT INTO cells(id,name,address) VALUES($1,$2,$3)").bind(cell).bind(format!("restore-{cell}")).bind(format!("http://{cell}.invalid")).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tenants(id,slug,name,timezone,owner_cell,ownership_generation,state) VALUES($1,$2,'Restore','UTC',$3,1,'ACTIVE')").bind(tenant).bind(format!("restore-{tenant}")).bind(cell).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tenant_leases(tenant_id,owner_cell,ownership_generation,expires_at) VALUES($1,$2,1,NOW()+INTERVAL '5 minutes')").bind(tenant).bind(cell).execute(&pool).await.unwrap();
    let ledger=Arc::new(PostgresLedger{pool:pool.clone(),cell_id:cell});
    let keys=TenantKeys::new(root.join("secrets"));wal::durable_create(&keys.path(tenant),&[42u8;32]).unwrap();
    let store=Arc::new(InMemory::new());
    let worker=Worker{gates:Default::default(),store:store.clone(),ledger:ledger.clone(),keys:keys.clone(),metrics:Arc::new(Metrics::default())};
    snapshot::take(db.clone(),&ledger,store.as_ref(),&keys,snapshot::Kind::Baseline).await.unwrap();
    insert(&db,"first").await;db.spool_wal().await.unwrap();
    let first=worker::pending(&db).unwrap()[0].1.clone();
    let target=gaming_cafe_api::time::parse_sqlite_timestamp(&first.captured_at).unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    insert(&db,"second").await;db.spool_wal().await.unwrap();
    assert_eq!(worker.ship(db.clone(),true).await.unwrap(),2);
    let staging=root.join("restore-staging");
    let full=restore::restore(tenant,lease.clone(),&ledger,store.as_ref(),&keys,&staging,None,restore::Limits::default()).await.unwrap();
    let mut c=SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&full.image)).await.unwrap();
    let values:Vec<String>=sqlx::query_scalar("SELECT value FROM durable ORDER BY rowid").fetch_all(&mut c).await.unwrap();assert_eq!(values,vec!["first","second"]);c.close().await.unwrap();
    assert_eq!(full.capture_number,first.capture_number+1);
    let point=restore::restore(tenant,lease.clone(),&ledger,store.as_ref(),&keys,&staging,Some(target),restore::Limits::default()).await.unwrap();
    let mut c=SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&point.image)).await.unwrap();
    let values:Vec<String>=sqlx::query_scalar("SELECT value FROM durable ORDER BY rowid").fetch_all(&mut c).await.unwrap();assert_eq!(values,vec!["first"]);c.close().await.unwrap();
    assert_eq!(point.recovered_at,target);
    assert!(restore::restore(tenant,lease.clone(),&ledger,store.as_ref(),&keys,&staging,Some(chrono::Utc::now()-chrono::Duration::days(91)),restore::Limits::default()).await.is_err());
    // A shared restore pin must make retention skip this generation.
    let mut pin=pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock_shared(hashtextextended($1,0))").bind(full.generation.to_string()).execute(&mut *pin).await.unwrap();
    sqlx::query("UPDATE snapshot_manifests SET snapshot_at=NOW()-INTERVAL '100 days' WHERE generation_id=$1").bind(full.generation).execute(&pool).await.unwrap();
    let obsolete=Uuid::new_v4();
    sqlx::query("INSERT INTO snapshot_manifests(id,tenant_id,generation_id,kind,schema_version,object_key,checksum_sha256,encrypted_size_bytes,snapshot_at,verified_at,capture_number,source_checksum_sha256) SELECT $1,tenant_id,generation_id,kind,schema_version,object_key||'/obsolete',checksum_sha256,encrypted_size_bytes,NOW()-INTERVAL '200 days',verified_at,capture_number,source_checksum_sha256 FROM snapshot_manifests WHERE id=$2").bind(obsolete).bind(full.snapshot).execute(&pool).await.unwrap();
    assert_eq!(gaming_cafe_api::replication::retention::run(&db,&ledger,store.as_ref()).await.unwrap(),0);
    let untouched:bool=sqlx::query_scalar("SELECT retired_at IS NULL FROM snapshot_manifests WHERE id=$1").bind(obsolete).fetch_one(&pool).await.unwrap();assert!(untouched);
    pin.rollback().await.unwrap();
    assert_eq!(gaming_cafe_api::replication::retention::run(&db,&ledger,store.as_ref()).await.unwrap(),1);
    // Required WAL tombstones and capture gaps cannot silently return a snapshot.
    sqlx::query("UPDATE replication_segments SET retired_at=NOW() WHERE generation_id=$1").bind(full.generation).execute(&pool).await.unwrap();
    assert!(restore::restore(tenant,lease.clone(),&ledger,store.as_ref(),&keys,&staging,None,restore::Limits::default()).await.is_err());
    sqlx::query("UPDATE replication_segments SET retired_at=NULL WHERE generation_id=$1").bind(full.generation).execute(&pool).await.unwrap();
    let original:serde_json::Value=sqlx::query_scalar("SELECT capture FROM replication_segments WHERE generation_id=$1 LIMIT 1").bind(full.generation).fetch_one(&pool).await.unwrap();
    sqlx::query("UPDATE replication_segments SET capture=jsonb_set(capture,'{capture_number}','9999') WHERE generation_id=$1").bind(full.generation).execute(&pool).await.unwrap();
    assert!(restore::restore(tenant,lease.clone(),&ledger,store.as_ref(),&keys,&staging,None,restore::Limits::default()).await.is_err());
    sqlx::query("UPDATE replication_segments SET capture=$2 WHERE generation_id=$1").bind(full.generation).bind(original).execute(&pool).await.unwrap();
    let key:String=sqlx::query_scalar("SELECT object_key FROM replication_segments WHERE generation_id=$1 ORDER BY segment_number LIMIT 1").bind(full.generation).fetch_one(&pool).await.unwrap();
    store.put(&ObjectPath::from(key),bytes::Bytes::from_static(b"corrupt").into()).await.unwrap();
    assert!(restore::restore(tenant,lease.clone(),&ledger,store.as_ref(),&keys,&staging,None,restore::Limits::default()).await.is_err());
    // Only the two completed images survive a failed object verification.
    assert_eq!(std::fs::read_dir(&staging).unwrap().count(),2);
    lease.0.store(false,Ordering::SeqCst);
    assert!(restore::restore(tenant,lease.clone(),&ledger,store.as_ref(),&keys,&staging,None,restore::Limits::default()).await.is_err());
    lease.0.store(true,Ordering::SeqCst);
    sqlx::query("UPDATE tenant_leases SET expires_at=NOW()+INTERVAL '20 seconds' WHERE tenant_id=$1").bind(tenant).execute(&pool).await.unwrap();
    assert!(restore::restore(tenant,lease.clone(),&ledger,store.as_ref(),&keys,&staging,None,restore::Limits::default()).await.is_err());
    db.close().await.unwrap();
    sqlx::query("DELETE FROM tenants WHERE id=$1").bind(tenant).execute(&pool).await.unwrap();
    sqlx::query("DELETE FROM cells WHERE id=$1").bind(cell).execute(&pool).await.unwrap();pool.close().await;std::fs::remove_dir_all(root).unwrap();
}

struct AssertOperationsBeforeAnalytics { pool:sqlx::PgPool, fail:AtomicBool }
#[async_trait]
impl gaming_cafe_api::replication::recovery::RecoveryAnalytics for AssertOperationsBeforeAnalytics {
    async fn rebuild(&self,db:Arc<TenantDb>)->Result<(),AppError> {
        let state:String=sqlx::query_scalar("SELECT state FROM tenants WHERE id=$1").bind(db.tenant_id()).fetch_one(&self.pool).await?;
        assert_eq!(state,"ACTIVE");
        insert(&db,"operations before analytics").await;
        if self.fail.load(Ordering::SeqCst) {Err(AppError::Internal("simulated analytics outage".into()))} else {Ok(())}
    }
}
#[tokio::test]
#[ignore = "requires an isolated control-plane database"]
async fn cell_loss_recovery_reassigns_restores_activates_then_retries_analytics() {
    use gaming_cafe_api::{control::{LeaseClient,LeaseConfig},replication::{ledger::PostgresLedger,recovery::Recoverer,snapshot}};
    let pool=sqlx::postgres::PgPoolOptions::new().max_connections(6).connect(&std::env::var("CONTROL_TEST_DATABASE_URL").unwrap()).await.unwrap();
    gaming_cafe_api::control::migrate(&pool).await.unwrap();
    let (root,db,_)=database().await;let tenant=db.tenant_id();let old=Uuid::new_v4();let target=Uuid::new_v4();
    for cell in [old,target] {sqlx::query("INSERT INTO cells(id,name,address) VALUES($1,$2,$3)").bind(cell).bind(format!("recovery-{cell}")).bind(format!("http://{cell}.invalid")).execute(&pool).await.unwrap();}
    sqlx::query("INSERT INTO tenants(id,slug,name,timezone,owner_cell,ownership_generation,state) VALUES($1,$2,'Cell loss','UTC',$3,1,'ACTIVE')").bind(tenant).bind(format!("cell-loss-{tenant}")).bind(old).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tenant_leases(tenant_id,owner_cell,ownership_generation,expires_at) VALUES($1,$2,1,NOW()+INTERVAL '5 minutes')").bind(tenant).bind(old).execute(&pool).await.unwrap();
    let ledger=Arc::new(PostgresLedger{pool:pool.clone(),cell_id:old});
    let keys=TenantKeys::new(root.join("separate-secrets"));wal::durable_create(&keys.path(tenant),&[42u8;32]).unwrap();
    let store=Arc::new(InMemory::new());
    let worker=Worker{gates:Default::default(),store:store.clone(),ledger:ledger.clone(),keys:keys.clone(),metrics:Arc::new(Metrics::default())};
    snapshot::take(db.clone(),&ledger,store.as_ref(),&keys,snapshot::Kind::Baseline).await.unwrap();
    insert(&db,"replicated payment").await;db.spool_wal().await.unwrap();worker.ship(db.clone(),true).await.unwrap();
    insert(&db,"not yet uploaded").await;
    let old_directory=db.path().parent().unwrap().to_owned();db.close().await.unwrap();std::fs::remove_dir_all(old_directory).unwrap();
    let leases=Arc::new(LeaseClient::new(pool.clone(),target,LeaseConfig::default()).unwrap());
    let manager=Arc::new(TenantDbManager::new(TenantDbConfig{root:root.join("new-cell"),..Default::default()},leases.clone()).unwrap());
    let analytics=Arc::new(AssertOperationsBeforeAnalytics{pool:pool.clone(),fail:AtomicBool::new(true)});
    let recoverer=Arc::new(Recoverer{ledger:Arc::new(PostgresLedger{pool:pool.clone(),cell_id:target}),leases,databases:manager.clone(),store,keys,staging_root:root.join("recovery-staging"),analytics:Some(analytics.clone())});
    assert_eq!(recoverer.affected(old).await.unwrap(),vec![tenant]);
    // The failed cell's unexpired lease cannot be stolen for recovery.
    assert!(recoverer.clone().recover_tenant(tenant).await.is_err());
    sqlx::query("UPDATE tenant_leases SET renewed_at=NOW()-INTERVAL '6 minutes',expires_at=NOW()-INTERVAL '60 seconds' WHERE tenant_id=$1").bind(tenant).execute(&pool).await.unwrap();
    let outcomes=recoverer.clone().recover_cell(old).await.unwrap();assert_eq!(outcomes.len(),1);
    assert!(outcomes[0].operations_ready,"{:?}",outcomes[0]);assert!(!outcomes[0].analytics_ready);assert!(outcomes[0].error.is_some());
    assert_eq!(outcomes[0].ownership_generation,2);
    let restored=manager.open(tenant).await.unwrap();
    let values:Vec<String>=sqlx::query_scalar("SELECT value FROM durable ORDER BY rowid").fetch_all(&restored.read_pool().unwrap()).await.unwrap();
    assert_eq!(values,vec!["replicated payment","operations before analytics"]);
    let phase:String=sqlx::query_scalar("SELECT phase FROM tenant_recovery_jobs WHERE tenant_id=$1").bind(tenant).fetch_one(&pool).await.unwrap();assert_eq!(phase,"COMPLETE");
    analytics.fail.store(false,Ordering::SeqCst);
    let retried=recoverer.clone().recover_tenant(tenant).await.unwrap();assert!(retried.operations_ready&&retried.analytics_ready);
    let jobs:i64=sqlx::query_scalar("SELECT COUNT(*) FROM tenant_recovery_jobs WHERE tenant_id=$1").bind(tenant).fetch_one(&pool).await.unwrap();assert_eq!(jobs,1);
    let generation:i64=sqlx::query_scalar("SELECT ownership_generation FROM tenants WHERE id=$1").bind(tenant).fetch_one(&pool).await.unwrap();assert_eq!(generation,2);
    restored.close().await.unwrap();
    // Crash after ACTIVE/COMPLETE is committed but before marker cleanup/cache publication.
    let job:Uuid=sqlx::query_scalar("SELECT id FROM tenant_recovery_jobs WHERE tenant_id=$1").bind(tenant).fetch_one(&pool).await.unwrap();
    let path=tenant_path(&root.join("new-cell"),tenant);
    let marker=path.parent().unwrap().join("replication/recovery-pending.json");
    wal::durable_create(&marker,&serde_json::to_vec(&serde_json::json!({"transition":job,"ownership_generation":2})).unwrap()).unwrap();
    sqlx::query("UPDATE tenants SET state='RESTORING' WHERE id=$1").bind(tenant).execute(&pool).await.unwrap();
    let fresh_manager=Arc::new(TenantDbManager::new(TenantDbConfig{root:root.join("new-cell"),..Default::default()},recoverer.leases.clone()).unwrap());
    assert!(fresh_manager.open(tenant).await.is_err());
    let routing=gaming_cafe_api::routing::RoutingCache::new(pool.clone());
    assert!(routing.refresh_tenant(tenant).await.unwrap().is_none());
    let fresh=Arc::new(Recoverer{ledger:recoverer.ledger.clone(),leases:recoverer.leases.clone(),databases:fresh_manager.clone(),store:recoverer.store.clone(),keys:recoverer.keys.clone(),staging_root:root.join("recovery-staging"),analytics:None});
    let resumed=fresh.resume_assigned().await.unwrap();assert_eq!(resumed.len(),1);assert!(resumed[0].operations_ready,"{:?}",resumed[0]);
    assert!(!marker.exists());
    let resumed_db=fresh_manager.open(tenant).await.unwrap();resumed_db.close().await.unwrap();
    sqlx::query("DELETE FROM tenants WHERE id=$1").bind(tenant).execute(&pool).await.unwrap();
    for cell in [old,target] {sqlx::query("DELETE FROM cells WHERE id=$1").bind(cell).execute(&pool).await.unwrap();}
    pool.close().await;std::fs::remove_dir_all(root).unwrap();
}
