//! Lease-gated restoration into a private, unpublished SQLite image.
use super::{
    batch,
    crypto::TenantKeys,
    ledger::PostgresLedger,
    snapshot,
    wal::{self, Capture},
};
use crate::{error::AppError, tenancy::TenantLease};
use chrono::{DateTime, Utc};
use futures::TryStreamExt;
use object_store::{path::Path as ObjectPath, ObjectStore, ObjectStoreExt};
use sha2::{Digest, Sha256};
use sqlx::{sqlite::SqliteConnectOptions, Connection, Row, SqliteConnection};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;
fn fail(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("Tenant restore: {e}"))
}
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub snapshot_bytes: u64,
    pub segment_bytes: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            snapshot_bytes: 64 * 1024 * 1024 * 1024,
            segment_bytes: 512 * 1024 * 1024,
        }
    }
}
#[derive(Debug)]
pub struct Restored {
    pub tenant: Uuid,
    pub generation: Uuid,
    pub ownership_generation: i64,
    pub snapshot: Uuid,
    pub image: PathBuf,
    pub capture_number: u64,
    pub recovered_at: DateTime<Utc>,
}
/// No source object is retired while this shared generation pin is held.
/// Restore pins do not block lease renewal or operational writes.
pub(crate) async fn pin(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    generation: Uuid,
) -> Result<(), AppError> {
    sqlx::query("SELECT pg_advisory_xact_lock_shared(hashtextextended($1,0))")
        .bind(generation.to_string())
        .execute(&mut **tx)
        .await?;
    Ok(())
}
async fn current(ledger: &PostgresLedger, tenant: Uuid, ownership: i64) -> Result<Uuid, AppError> {
    sqlx::query_scalar("SELECT t.current_replication_generation FROM tenants t JOIN tenant_leases l ON l.tenant_id=t.id WHERE t.id=$1 AND t.owner_cell=$2 AND l.owner_cell=$2 AND t.ownership_generation=$3 AND l.ownership_generation=$3 AND l.expires_at>clock_timestamp()+INTERVAL '30 seconds' AND t.state <> 'DELETED' AND t.current_replication_generation IS NOT NULL")
        .bind(tenant).bind(ledger.cell_id).bind(ownership).fetch_optional(&ledger.pool).await?.ok_or_else(||AppError::Forbidden("Restore requires a fresh assigned ownership lease and selected generation".into()))
}
async fn download(
    store: &dyn ObjectStore,
    key: &str,
    checksum: &str,
    size: i64,
    target: &Path,
    limit: u64,
) -> Result<(), AppError> {
    if size <= 0 || size as u64 > limit {
        return Err(fail("Object exceeds restore size limit"));
    }
    let remote = store.get(&ObjectPath::from(key)).await.map_err(fail)?;
    if remote.meta.size != size as u64 {
        return Err(fail("Remote object size differs from verified ledger"));
    }
    let mut file = tokio::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(target)
        .await
        .map_err(fail)?;
    let mut stream = remote.into_stream();
    let mut hash = Sha256::new();
    let mut total = 0u64;
    while let Some(bytes) = stream.try_next().await.map_err(fail)? {
        total = total
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| fail("Object size overflow"))?;
        if total > size as u64 {
            return Err(fail("Remote object exceeds verified size"));
        }
        hash.update(&bytes);
        file.write_all(&bytes).await.map_err(fail)?;
    }
    file.sync_all().await.map_err(fail)?;
    if total != size as u64 || hex::encode(hash.finalize()) != checksum {
        return Err(fail("Object checksum mismatch"));
    }
    Ok(())
}
async fn apply(image: &Path, capture: &Capture, bytes: &[u8]) -> Result<(), AppError> {
    if wal::checksum(bytes) != capture.checksum {
        return Err(fail("WAL source checksum mismatch"));
    }
    wal::validate(bytes, capture.frames)?;
    let wal_path = PathBuf::from(format!("{}-wal", image.display()));
    wal::durable_create(&wal_path, bytes)?;
    let mut connection =
        SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(image)).await?;
    let _: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_schema")
        .fetch_one(&mut connection)
        .await?;
    let checkpoint = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .fetch_one(&mut connection)
        .await?;
    if checkpoint.get::<i64, _>(0) != 0 {
        return Err(fail("Restore WAL checkpoint was blocked"));
    }
    connection.close().await?;
    for suffix in ["-wal", "-shm"] {
        match std::fs::remove_file(format!("{}{suffix}", image.display())) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(fail(e)),
        }
    }
    Ok(())
}
/// Uses the control plane's current generation, never a newest remote folder.
/// PIT selects the last complete durable capture at/before the requested UTC
/// instant. Captures are the recovery granularity; WAL has no transaction UTC.
/// The returned image is staged, not installed into an active tenant directory.
pub async fn restore(
    tenant: Uuid,
    lease: Arc<dyn TenantLease>,
    ledger: &PostgresLedger,
    store: &dyn ObjectStore,
    keys: &TenantKeys,
    staging_root: &Path,
    at: Option<DateTime<Utc>>,
    limits: Limits,
) -> Result<Restored, AppError> {
    let ownership = lease.writable_generation(tenant)?;
    let key = keys.read(tenant)?;
    let generation = current(ledger, tenant, ownership).await?;
    let mut tx = ledger.pool.begin().await?;
    pin(&mut tx, generation).await?;
    if current(ledger, tenant, ownership).await? != generation {
        return Err(fail("Generation changed before restore selection"));
    }
    let now: DateTime<Utc> = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&mut *tx)
        .await?;
    if at.is_some_and(|t| t > now || t < now - chrono::Duration::days(90)) {
        return Err(AppError::Conflict(
            "Requested restore instant is outside the retained 90-day window".into(),
        ));
    }
    let source_ownership: i64 =
        sqlx::query_scalar("SELECT ownership_generation FROM replication_generations WHERE id=$1")
            .bind(generation)
            .fetch_one(&mut *tx)
            .await?;
    let row=sqlx::query("SELECT id,object_key,checksum_sha256,encrypted_size_bytes,source_checksum_sha256,capture_number,snapshot_at FROM snapshot_manifests WHERE generation_id=$1 AND verified_at IS NOT NULL AND retired_at IS NULL AND source_checksum_sha256 IS NOT NULL AND capture_number IS NOT NULL AND ($2::timestamptz IS NULL OR snapshot_at <= $2) ORDER BY snapshot_at DESC,capture_number DESC,id DESC LIMIT 1").bind(generation).bind(at).fetch_optional(&mut *tx).await?.ok_or_else(||AppError::Conflict("No verified live snapshot exists before the requested instant in the selected generation".into()))?;
    let snapshot_id: Uuid = row.get(0);
    let object: String = row.get(1);
    let checksum: String = row.get(2);
    let size: i64 = row.get(3);
    let source_checksum: String = row.get(4);
    let mut number = row.get::<i64, _>(5) as u64;
    let mut recovered: DateTime<Utc> = row.get(6);
    let rows=sqlx::query("SELECT segment_number,object_key,capture,checksum_sha256,encrypted_size_bytes,retired_at,verified_at FROM replication_segments WHERE generation_id=$1 ORDER BY segment_number").bind(generation).fetch_all(&mut *tx).await?;
    let directory = staging_root.join(format!("restore-{tenant}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&directory).map_err(fail)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
            .map_err(fail)?;
    }
    let image = directory.join("tenant.db");
    let result = async {
        let encoded = directory.join("snapshot.encoded");
        download(
            store,
            &object,
            &checksum,
            size,
            &encoded,
            limits.snapshot_bytes,
        )
        .await?;
        lease.ensure_writable(tenant, ownership)?;
        let input = std::fs::File::open(&encoded).map_err(fail)?;
        let output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&image)
            .map_err(fail)?;
        let object_copy = object.clone();
        let image_copy = image.clone();
        let limit = limits.snapshot_bytes;
        tokio::task::spawn_blocking(move || -> Result<(), AppError> {
            super::crypto::decode_file(&key, &object_copy, input, &output, limit)?;
            output.sync_all().map_err(fail)?;
            if snapshot::hash_file(&image_copy)? != source_checksum {
                return Err(fail("Snapshot source checksum mismatch"));
            }
            Ok(())
        })
        .await
        .map_err(fail)??;
        std::fs::remove_file(encoded).map_err(fail)?;
        let mut stopped = false;
        for row in rows {
            let capture: Capture = serde_json::from_value(row.get(2)).map_err(fail)?;
            if capture.ownership_generation != source_ownership {
                return Err(fail("WAL belongs to a different generation owner"));
            }
            if capture.range_end() <= number {
                continue;
            }
            if row.get::<Option<DateTime<Utc>>, _>(6).is_none() {
                break;
            }
            if row.get::<Option<DateTime<Utc>>, _>(5).is_some() {
                return Err(fail("Required WAL is retired; restore chain is incomplete"));
            }
            if capture.capture_number
                > number
                    .checked_add(1)
                    .ok_or_else(|| fail("Capture number overflow"))?
            {
                return Err(fail("Required WAL capture is missing"));
            }
            let object: String = row.get(1);
            let checksum: String = row.get(3);
            let size: i64 = row.get(4);
            let encoded = directory.join("wal.encoded");
            download(
                store,
                &object,
                &checksum,
                size,
                &encoded,
                limits.segment_bytes,
            )
            .await?;
            lease.ensure_writable(tenant, ownership)?;
            let bytes = std::fs::read(&encoded).map_err(fail)?;
            let raw = super::crypto::decode(&key, &object, &bytes, limits.segment_bytes as usize)?;
            if wal::checksum(&raw) != capture.checksum {
                return Err(fail("WAL batch checksum mismatch"));
            }
            let records = match capture.version {
                1 => vec![(capture.clone(), raw)],
                2 => batch::decode(&raw)?,
                _ => return Err(fail("Unsupported capture format")),
            };
            if records.first().unwrap().0.capture_number != capture.capture_number
                || records.last().unwrap().0.capture_number != capture.range_end()
            {
                return Err(fail("WAL batch range mismatch"));
            }
            for (c, bytes) in records {
                if c.capture_number <= number {
                    continue;
                }
                let instant = crate::time::parse_sqlite_timestamp(&c.captured_at).map_err(fail)?;
                if at.is_some() && instant < recovered {
                    return Err(fail(
                        "Nonmonotonic capture UTC prevents point-in-time selection",
                    ));
                }
                if at.is_some_and(|t| instant > t) {
                    stopped = true;
                    break;
                }
                if c.capture_number != number + 1 {
                    return Err(fail("WAL capture order is incomplete"));
                }
                apply(&image, &c, &bytes).await.map_err(|e| {
                    fail(format!("Capture {} replay failed: {e}", c.capture_number))
                })?;
                number = c.capture_number;
                recovered = instant;
            }
            std::fs::remove_file(encoded).map_err(fail)?;
            if stopped {
                break;
            }
        }
        let mut connection =
            SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&image)).await?;
        let checks: Vec<String> = sqlx::query_scalar("PRAGMA integrity_check")
            .fetch_all(&mut connection)
            .await?;
        if checks != ["ok"] {
            return Err(fail(format!("SQLite integrity check failed: {checks:?}")));
        }
        connection.close().await?;
        std::fs::File::open(&image)
            .and_then(|f| f.sync_all())
            .map_err(fail)?;
        std::fs::File::open(&directory)
            .and_then(|f| f.sync_all())
            .map_err(fail)?;
        lease.ensure_writable(tenant, ownership)?;
        if current(ledger, tenant, ownership).await? != generation {
            return Err(fail("Generation changed during restore"));
        }
        Ok::<_, AppError>(())
    }
    .await;
    if let Err(error) = result {
        let _ = std::fs::remove_dir_all(&directory);
        return Err(error);
    }
    tx.commit().await?;
    Ok(Restored {
        tenant,
        generation,
        ownership_generation: ownership,
        snapshot: snapshot_id,
        image,
        capture_number: number,
        recovered_at: recovered,
    })
}
