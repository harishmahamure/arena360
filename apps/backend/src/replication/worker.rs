use super::{
    crypto::TenantKeys,
    ledger::{Ledger, Segment},
    wal::{self, Capture},
};
use crate::{
    error::AppError,
    metrics::Metrics,
    tenancy::{TenantDb, TenantDbManager},
};
use object_store::{path::Path as ObjectPath, ObjectStore, ObjectStoreExt, PutMode, UpdateVersion};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

fn fail(message: impl Into<String>) -> AppError {
    AppError::Internal(message.into())
}
fn io(e: std::io::Error) -> AppError {
    fail(format!("Replication spool: {e}"))
}
pub struct Worker {
    pub gates: std::sync::Arc<
        tokio::sync::Mutex<
            std::collections::HashMap<uuid::Uuid, std::sync::Arc<tokio::sync::Mutex<()>>>,
        >,
    >,
    pub store: Arc<dyn ObjectStore>,
    pub ledger: Arc<dyn Ledger>,
    pub keys: TenantKeys,
    pub metrics: Arc<Metrics>,
}
fn spool_directory(db: &TenantDb) -> PathBuf {
    db.path().parent().unwrap().join("replication/spool")
}
pub fn pending(db: &TenantDb) -> Result<Vec<(PathBuf, Capture)>, AppError> {
    let directory = spool_directory(db);
    if !directory.exists() {
        return Ok(vec![]);
    }
    let mut captures = vec![];
    for entry in std::fs::read_dir(directory).map_err(io)? {
        let path = entry.map_err(io)?.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wal") {
            continue;
        }
        let capture: Capture =
            serde_json::from_slice(&std::fs::read(path.with_extension("json")).map_err(io)?)
                .map_err(|e| fail(e.to_string()))?;
        // Foreign-generation spool is preserved for incident investigation. A
        // new owner must never publish it into its own generation.
        if capture.ownership_generation != db.ownership_generation() {
            continue;
        }
        captures.push((path, capture));
    }
    captures.sort_by_key(|(_, capture)| capture.capture_number);
    if captures
        .windows(2)
        .any(|pair| pair[0].1.capture_number == pair[1].1.capture_number)
    {
        return Err(fail("Duplicate capture sequence"));
    }
    Ok(captures)
}
impl Worker {
    pub async fn ship(&self, db: Arc<TenantDb>, force: bool) -> Result<usize, AppError> {
        let gate = self
            .gates
            .lock()
            .await
            .entry(db.tenant_id())
            .or_default()
            .clone();
        let _serial = gate.lock().await;
        let captures = pending(&db)?;
        let bytes = captures.iter().try_fold(0u64, |sum, (p, _)| {
            std::fs::metadata(p).map(|m| sum + m.len()).map_err(io)
        })?;
        let age = captures
            .first()
            .map(|(_, c)| {
                crate::time::parse_sqlite_timestamp(&c.captured_at)
                    .map(|t| (chrono::Utc::now() - t).num_milliseconds().max(0) as u64)
            })
            .transpose()
            .map_err(|e| fail(e.to_string()))?
            .unwrap_or(0);
        if !force
            && !super::batch::descriptor(&db).exists()
            && bytes < 8 * 1024 * 1024
            && age < 120_000
        {
            return Ok(0);
        }
        let _job = match db.background_jobs() {
            Some(j) => Some(j.acquire(crate::background::Priority::Backup).await?),
            None => None,
        };
        db.ensure_current_owner()?;
        let Some(batch) = super::batch::prepare(&db, &captures)? else {
            return Ok(0);
        };
        let segment = self.ledger.reserve(&db, &batch.capture).await?;
        let artifact = batch
            .raw
            .with_extension(format!("{}.encoded", segment.generation));
        if !segment.verified {
            self.ledger
                .baseline(
                    db.clone(),
                    segment.generation,
                    self.store.as_ref(),
                    &self.keys,
                )
                .await?;
            let source = super::batch::read(&batch)?;
            if !artifact.exists() {
                wal::durable_create(
                    &artifact,
                    &super::crypto::encode(
                        &self.keys.read(db.tenant_id())?,
                        &segment.object_key,
                        &source,
                    )?,
                )?;
            }
            let encoded = std::fs::read(&artifact).map_err(io)?;
            if super::crypto::decode(
                &self.keys.read(db.tenant_id())?,
                &segment.object_key,
                &encoded,
                512 * 1024 * 1024,
            )? != source
            {
                return Err(fail("Encoded batch mismatch"));
            }
            let digest = wal::checksum(&encoded);
            self.put_verified(&segment.object_key, &encoded).await?;
            db.ensure_current_owner()?;
            let manifest = self
                .ledger
                .manifest(&db, &segment, &digest, encoded.len() as i64)
                .await?;
            self.publish_manifest(&db, &segment, &manifest).await?;
            db.ensure_current_owner()?;
            self.ledger
                .verified(&db, &segment, &digest, encoded.len() as i64)
                .await?;
        }
        db.ensure_current_owner()?;
        super::batch::remove(&db, &batch, &artifact)?;
        Ok(batch.files.len())
    }
    pub async fn put_verified(&self, key: &str, bytes: &[u8]) -> Result<(), AppError> {
        put_verified(self.store.as_ref(), key, bytes).await
    }
    async fn publish_manifest(
        &self,
        db: &TenantDb,
        segment: &Segment,
        bytes: &[u8],
    ) -> Result<(), AppError> {
        publish_document(self.store.as_ref(), db, segment.generation, bytes).await
    }
}

pub async fn put_verified(
    store: &dyn ObjectStore,
    key: &str,
    bytes: &[u8],
) -> Result<(), AppError> {
    let path = ObjectPath::from(key);
    match store
        .put_opts(&path, bytes.to_vec().into(), PutMode::Create.into())
        .await
    {
        Ok(_) => {}
        Err(object_store::Error::AlreadyExists { .. }) => {}
        Err(e) => return Err(fail(format!("Immutable backup upload failed: {e}"))),
    }
    let received = store.get(&path).await.map_err(|e| fail(e.to_string()))?;
    if received.meta.size != bytes.len() as u64 {
        return Err(fail("Backup upload size verification failed"));
    }
    let received = received.bytes().await.map_err(|e| fail(e.to_string()))?;
    if received.as_ref() != bytes {
        return Err(fail("Backup upload checksum verification failed"));
    }
    Ok(())
}
pub async fn publish_document(
    store: &dyn ObjectStore,
    db: &TenantDb,
    generation: uuid::Uuid,
    bytes: &[u8],
) -> Result<(), AppError> {
    let document: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| fail(e.to_string()))?;
    let revision = document["revision"]
        .as_i64()
        .filter(|r| *r > 0)
        .ok_or_else(|| fail("Missing manifest revision"))?;
    let prefix = format!(
        "tenants/{}/replication/generations/{}",
        db.tenant_id(),
        generation
    );
    // Revisions are immutable evidence. The well-known pointer uses CAS so
    // delayed requests cannot replace a newer generation manifest revision.
    put_verified(
        store,
        &format!("{prefix}/manifests/{:010}.json", revision),
        &serde_json::to_vec(&serde_json::json!({"version":1,"revision":revision,"tenant_id":db.tenant_id(),"generation_id":generation,"ownership_generation":db.ownership_generation(),"document_sha256":wal::checksum(bytes),"delta":document["delta"]})).map_err(|e|fail(e.to_string()))?,
    )
    .await?;
    let path = ObjectPath::from(format!("{prefix}/manifest.json"));
    let mode = match store.get(&path).await {
        Ok(existing) => {
            let version = UpdateVersion {
                e_tag: existing.meta.e_tag.clone(),
                version: existing.meta.version.clone(),
            };
            let old = existing.bytes().await.map_err(|e| fail(e.to_string()))?;
            if old.as_ref() == bytes {
                return Ok(());
            }
            let document: serde_json::Value =
                serde_json::from_slice(&old).map_err(|e| fail(e.to_string()))?;
            let previous = document["revision"]
                .as_i64()
                .or_else(|| {
                    document["segments"]
                        .as_array()
                        .and_then(|rows| rows.iter().filter_map(|s| s["number"].as_i64()).max())
                })
                .unwrap_or(i64::MAX);
            if previous >= revision {
                return Err(fail("Remote manifest is ahead or incompatible"));
            }
            PutMode::Update(version)
        }
        Err(object_store::Error::NotFound { .. }) => PutMode::Create,
        Err(e) => return Err(fail(e.to_string())),
    };
    db.ensure_current_owner()?;
    store
        .put_opts(&path, bytes.to_vec().into(), mode.into())
        .await
        .map_err(|e| fail(e.to_string()))?;
    let remote = store
        .get(&path)
        .await
        .map_err(|e| fail(e.to_string()))?
        .bytes()
        .await
        .map_err(|e| fail(e.to_string()))?;
    if remote.as_ref() != bytes {
        return Err(fail("Generation manifest verification failed"));
    }
    Ok(())
}

/// Capture is independent of object storage/control availability. Local NVMe
/// spool survives a process crash and continues growing through network outages.
pub fn spawn_capture(manager: Arc<TenantDbManager>, metrics: Arc<Metrics>, root: PathBuf) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            for db in manager.open_handles().await {
                if let Err(error) = db.spool_wal().await {
                    metrics.replication_failed();
                    tracing::warn!(tenant=%db.tenant_id(),%error,"WAL capture failed; checkpoint withheld");
                }
            }
            match backlog(&root) {
                Ok((bytes, age)) => metrics.set_replication_backlog(bytes, age),
                Err(error) => tracing::warn!(%error,"Cannot measure WAL spool"),
            }
        }
    });
}
pub fn spawn_upload(manager: Arc<TenantDbManager>, worker: Arc<Worker>) {
    spawn_snapshots(manager.clone(), worker.clone());
    tokio::spawn(async move {
        let mut running =
            std::collections::HashMap::<uuid::Uuid, tokio::task::JoinHandle<()>>::new();
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            running.retain(|_, task| !task.is_finished());
            if let Err(error) = manager.open_spooled().await {
                worker.metrics.replication_failed();
                tracing::warn!(%error,"Cannot reopen spooled tenant");
            }
            for db in manager.open_handles().await {
                let tenant = db.tenant_id();
                if running.contains_key(&tenant) {
                    continue;
                }
                let worker = worker.clone();
                running.insert(
                    tenant,
                    tokio::spawn(async move {
                        match tokio::time::timeout(Duration::from_secs(30), worker.ship(db, false))
                            .await
                        {
                            Ok(Ok(_)) => {}
                            result => {
                                worker.metrics.replication_failed();
                                tracing::warn!(%tenant,?result,"WAL upload failed; spool retained");
                            }
                        }
                    }),
                );
            }
        }
    });
}
fn spawn_snapshots(manager: Arc<TenantDbManager>, worker: Arc<Worker>) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            let tenants = match worker.ledger.daily_tenants().await {
                Ok(t) => t,
                Err(error) => {
                    worker.metrics.replication_failed();
                    tracing::warn!(%error,"Daily snapshot registry failed");
                    continue;
                }
            };
            for tenant in tenants {
                let result = tokio::time::timeout(Duration::from_secs(120), async {
                    let db = manager.open(tenant).await?;
                    let gate = worker.gates.lock().await.entry(tenant).or_default().clone();
                    let _serial = gate.lock().await;
                    let _job = match db.background_jobs() {
                        Some(j) => Some(j.acquire(crate::background::Priority::Backup).await?),
                        None => None,
                    };
                    worker
                        .ledger
                        .snapshot(
                            db,
                            worker.store.as_ref(),
                            &worker.keys,
                            super::snapshot::Kind::Daily,
                        )
                        .await
                })
                .await;
                if !matches!(result, Ok(Ok(()))) {
                    worker.metrics.replication_failed();
                    tracing::warn!(%tenant,?result,"Daily snapshot failed; evidence retained");
                }
            }
        }
    });
}
pub fn backlog(root: &Path) -> Result<(u64, u64), AppError> {
    let mut bytes = 0;
    let mut oldest = 0;
    if !root.exists() {
        return Ok((0, 0));
    }
    for tenant in std::fs::read_dir(root).map_err(io)? {
        let spool = tenant.map_err(io)?.path().join("replication/spool");
        if !spool.exists() {
            continue;
        }
        for file in std::fs::read_dir(spool).map_err(io)? {
            let path = file.map_err(io)?.path();
            if path.is_file() {
                bytes += std::fs::metadata(&path).map_err(io)?.len();
            }
            if path.extension().and_then(|s| s.to_str()) != Some("wal") {
                continue;
            }
            let c: Capture =
                serde_json::from_slice(&std::fs::read(path.with_extension("json")).map_err(io)?)
                    .map_err(|e| fail(e.to_string()))?;
            let t = crate::time::parse_sqlite_timestamp(&c.captured_at)
                .map_err(|e| fail(e.to_string()))?;
            oldest = oldest.max((chrono::Utc::now() - t).num_milliseconds().max(0) as u64);
        }
    }
    Ok((bytes, oldest))
}
