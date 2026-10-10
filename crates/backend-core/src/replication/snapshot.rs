use super::{
    crypto::{self, TenantKeys},
    ledger::PostgresLedger,
    wal, worker,
};
use crate::{
    error::AppError,
    tenancy::{MigrationContext, MigrationHook, TenantDb, TenantDbManager},
};
use async_trait::async_trait;
use object_store::ObjectStore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{sqlite::SqliteConnectOptions, Connection, Row, SqliteConnection};
use std::{fs::File, io::Read, path::Path, sync::Arc};
use uuid::Uuid;
fn fail(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("Tenant snapshot: {e}"))
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Kind {
    Baseline,
    Daily,
    PreMigration(i64),
    PostMigration(i64),
    Manual,
}
impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Daily => "DAILY",
            Self::PreMigration(_) => "PRE_MIGRATION",
            Self::PostMigration(_) => "POST_MIGRATION",
            _ => "MANUAL",
        }
    }
    fn suffix(self) -> String {
        match self {
            Self::PreMigration(v) => format!("-pre-migration-v{v}"),
            Self::PostMigration(v) => format!("-post-migration-v{v}"),
            _ => String::new(),
        }
    }
    fn pending_name(self) -> String {
        match self {
            Self::Baseline => "baseline".into(),
            Self::Daily => "daily".into(),
            Self::Manual => "manual".into(),
            _ => self.suffix(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: Uuid,
    pub tenant: Uuid,
    pub generation: Uuid,
    pub ownership_generation: i64,
    pub kind: Kind,
    pub object_key: String,
    pub snapshot_at: chrono::DateTime<chrono::Utc>,
    pub schema_version: i64,
    pub capture_number: i64,
    pub event_sequence: i64,
    pub source_checksum: String,
    pub checksum: String,
    pub encrypted_size: i64,
}
pub fn hash_file(path: &Path) -> Result<String, AppError> {
    let mut file = File::open(path).map_err(fail)?;
    let mut buffer = [0u8; 65536];
    let mut hash = Sha256::new();
    loop {
        let n = file.read(&mut buffer).map_err(fail)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(hex::encode(hash.finalize()))
}
async fn copy(
    connection: &mut SqliteConnection,
    db: &TenantDb,
    target: &Path,
) -> Result<(i64, i64, i64, chrono::DateTime<chrono::Utc>), AppError> {
    db.ensure_current_owner()?;
    crate::tenancy::spool_connection_wal(connection, db.path(), db.ownership_generation()).await?;
    // WAL frames address physical source pages. VACUUM INTO renumbers them
    // and produces a logically valid snapshot that cannot safely replay WAL.
    // The backup API preserves those pages, including the freelist, and sees
    // committed WAL pages even when a reader prevents checkpoint completion.
    let mut destination = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(target)
            .create_if_missing(true),
    )
    .await?;
    {
        let mut source_handle = connection.lock_handle().await?;
        let mut destination_handle = destination.lock_handle().await?;
        // SAFETY: both SQLx handles exclude their worker threads. The backup
        // never escapes this scope, and finish runs on every initialized path.
        let result = unsafe {
            let destination = destination_handle.as_raw_handle().as_ptr();
            let backup = libsqlite3_sys::sqlite3_backup_init(
                destination,
                c"main".as_ptr(),
                source_handle.as_raw_handle().as_ptr(),
                c"main".as_ptr(),
            );
            if backup.is_null() {
                Err(fail(format!(
                    "Cannot initialize page-preserving backup: {}",
                    libsqlite3_sys::sqlite3_errcode(destination)
                )))
            } else {
                let step = libsqlite3_sys::sqlite3_backup_step(backup, -1);
                let finish = libsqlite3_sys::sqlite3_backup_finish(backup);
                if step == libsqlite3_sys::SQLITE_DONE && finish == libsqlite3_sys::SQLITE_OK {
                    Ok(())
                } else {
                    Err(fail(format!(
                        "Page-preserving backup failed: step={step}, finish={finish}"
                    )))
                }
            }
        };
        result?;
    }
    destination.close().await?;
    File::open(target)
        .and_then(|f| f.sync_all())
        .map_err(fail)?;
    let version = if sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='_sqlx_migrations')",
    )
    .fetch_one(&mut *connection)
    .await?
    {
        sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(MAX(version),0) FROM _sqlx_migrations WHERE success=1",
        )
        .fetch_one(&mut *connection)
        .await?
    } else {
        0
    };
    let event = if sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='sqlite_sequence')",
    )
    .fetch_one(&mut *connection)
    .await?
    {
        sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name='outbox_events'),0)",
        )
        .fetch_one(&mut *connection)
        .await?
    } else {
        0
    };
    let number = match std::fs::read_to_string(
        db.path()
            .parent()
            .unwrap()
            .join("replication/capture-sequence"),
    ) {
        Ok(n) => n.parse::<i64>().map_err(fail)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
        Err(e) => return Err(fail(e)),
    };
    db.ensure_current_owner()?;
    Ok((version, event, number, chrono::Utc::now()))
}
pub async fn take(
    db: Arc<TenantDb>,
    ledger: &PostgresLedger,
    store: &dyn ObjectStore,
    keys: &TenantKeys,
    kind: Kind,
) -> Result<(), AppError> {
    take_with_connection(db, ledger, store, keys, kind, None).await
}
async fn take_with_connection(
    db: Arc<TenantDb>,
    ledger: &PostgresLedger,
    store: &dyn ObjectStore,
    keys: &TenantKeys,
    kind: Kind,
    connection: Option<&mut SqliteConnection>,
) -> Result<(), AppError> {
    // Configuration failures must not create a full temporary database copy.
    let key = keys.read(db.tenant_id())?;
    let generation = ledger.ensure_generation(&db).await?;
    if matches!(kind, Kind::Daily) {
        let fresh:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM snapshot_manifests WHERE generation_id=$1 AND kind='DAILY' AND verified_at IS NOT NULL AND retired_at IS NULL AND snapshot_at>clock_timestamp()-INTERVAL '1 day')").bind(generation).fetch_one(&ledger.pool).await?;
        if fresh {
            return Ok(());
        }
    }
    let directory = db.path().parent().unwrap().join("replication/snapshots");
    std::fs::create_dir_all(&directory).map_err(fail)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
            .map_err(fail)?;
    }
    let pending = directory.join(format!("{generation}-{}.json", kind.pending_name()));
    let snapshot: Snapshot = if pending.exists() {
        serde_json::from_slice(&std::fs::read(&pending).map_err(fail)?).map_err(fail)?
    } else {
        let id = Uuid::new_v4();
        let source = directory.join(format!("{id}.sqlite"));
        let (schema_version, event_sequence, capture_number, at) = if let Some(connection) =
            connection
        {
            copy(connection, &db, &source).await?
        } else {
            let db_copy = db.clone();
            let source_copy = source.clone();
            db.with_writer(move |c| Box::pin(async move { copy(c, &db_copy, &source_copy).await }))
                .await?
        };
        let object_key = format!(
            "tenants/{}/replication/generations/{generation}/snapshots/{}{}.db.zst",
            db.tenant_id(),
            at.format("%Y-%m-%dT%H-%M-%S%.9fZ"),
            kind.suffix()
        );
        let encoded = directory.join(format!("{id}.encoded"));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut output = options.open(&encoded).map_err(fail)?;
        crypto::encode_file(
            &key,
            &object_key,
            File::open(&source).map_err(fail)?,
            &mut output,
        )?;
        output.sync_all().map_err(fail)?;
        let snapshot = Snapshot {
            id,
            tenant: db.tenant_id(),
            generation,
            ownership_generation: db.ownership_generation(),
            kind,
            object_key,
            snapshot_at: at,
            schema_version,
            event_sequence,
            capture_number,
            source_checksum: hash_file(&source)?,
            checksum: hash_file(&encoded)?,
            encrypted_size: output
                .metadata()
                .map_err(fail)?
                .len()
                .try_into()
                .map_err(fail)?,
        };
        wal::durable_create(&pending, &serde_json::to_vec(&snapshot).map_err(fail)?)?;
        snapshot
    };
    if snapshot.tenant != db.tenant_id()
        || snapshot.generation != generation
        || snapshot.ownership_generation != db.ownership_generation()
    {
        return Err(fail("Snapshot ownership changed"));
    }
    let encoded = directory.join(format!("{}.encoded", snapshot.id));
    let source = directory.join(format!("{}.sqlite", snapshot.id));
    if hash_file(&encoded)? != snapshot.checksum || hash_file(&source)? != snapshot.source_checksum
    {
        return Err(fail("Snapshot spool checksum mismatch"));
    }
    let mut tx = ledger.pool.begin().await?;
    if ledger.lock_owner(&db, &mut tx).await? != Some(generation) {
        return Err(fail("Generation changed before snapshot reservation"));
    }
    sqlx::query("INSERT INTO snapshot_manifests(id,tenant_id,generation_id,kind,schema_version,object_key,checksum_sha256,encrypted_size_bytes,snapshot_at,capture_number,event_sequence,source_checksum_sha256) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) ON CONFLICT(id) DO NOTHING").bind(snapshot.id).bind(snapshot.tenant).bind(generation).bind(snapshot.kind.name()).bind(snapshot.schema_version).bind(&snapshot.object_key).bind(&snapshot.checksum).bind(snapshot.encrypted_size).bind(snapshot.snapshot_at).bind(snapshot.capture_number).bind(snapshot.event_sequence).bind(&snapshot.source_checksum).execute(&mut *tx).await?;
    db.ensure_current_owner()?;
    tx.commit().await?;
    put_file_verified(store, &snapshot.object_key, &encoded, &snapshot.checksum).await?;
    db.ensure_current_owner()?;
    let document = manifest(&db, ledger, &snapshot).await?;
    worker::publish_document(store, &db, generation, &document).await?;
    let mut tx = ledger.pool.begin().await?;
    if ledger.lock_owner(&db, &mut tx).await? != Some(generation) {
        return Err(fail("Generation changed before snapshot verification"));
    }
    sqlx::query("UPDATE snapshot_manifests SET verified_at=clock_timestamp() WHERE id=$1")
        .bind(snapshot.id)
        .execute(&mut *tx)
        .await?;
    db.ensure_current_owner()?;
    tx.commit().await?;
    for path in [pending, source, encoded] {
        std::fs::remove_file(path).map_err(fail)?;
    }
    File::open(directory)
        .and_then(|f| f.sync_all())
        .map_err(fail)?;
    Ok(())
}

/// The generated artifact is immutable: create_new, a unique snapshot UUID,
/// and no writer after its fsync. Uploads use a conditional PUT for small files
/// or bounded multipart streaming for large files; retries verify existing objects.
async fn put_file_verified(
    store: &dyn ObjectStore,
    key: &str,
    file: &Path,
    checksum: &str,
) -> Result<(), AppError> {
    use futures::TryStreamExt;
    use object_store::{path::Path as ObjectPath, ObjectStoreExt};
    let input = File::open(file).map_err(fail)?;
    let size = input.metadata().map_err(fail)?.len();
    let path = ObjectPath::from(key);
    match crate::historical::objects::put_file(store,&path,file).await {
        Ok(()) | Err(AppError::Conflict(_))=>{},
        Err(error)=>return Err(error),
    }
    let remote = store.get(&path).await.map_err(fail)?;
    if remote.meta.size != size {
        return Err(fail("Snapshot upload size mismatch"));
    }
    let mut stream = remote.into_stream();
    let mut hash = Sha256::new();
    while let Some(chunk) = stream.try_next().await.map_err(fail)? {
        hash.update(&chunk);
    }
    if hex::encode(hash.finalize()) != checksum {
        return Err(fail("Snapshot upload checksum mismatch"));
    }
    Ok(())
}
async fn manifest(
    db: &TenantDb,
    ledger: &PostgresLedger,
    snapshot: &Snapshot,
) -> Result<Vec<u8>, AppError> {
    let mut tx = ledger.pool.begin().await?;
    if ledger.lock_owner(db, &mut tx).await? != Some(snapshot.generation) {
        return Err(fail("Generation changed before manifest"));
    }
    let segments:Vec<serde_json::Value>=sqlx::query_scalar("SELECT jsonb_build_object('number',segment_number,'object_key',object_key,'capture',capture,'checksum_sha256',checksum_sha256,'encrypted_size_bytes',encrypted_size_bytes,'retired_at',retired_at) FROM replication_segments WHERE generation_id=$1 AND verified_at IS NOT NULL ORDER BY segment_number").bind(snapshot.generation).fetch_all(&mut *tx).await?;
    let snapshots:Vec<serde_json::Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',id,'object_key',object_key,'kind',kind,'schema_version',schema_version,'snapshot_at',snapshot_at,'capture_number',capture_number,'event_sequence',event_sequence,'source_checksum_sha256',source_checksum_sha256,'checksum_sha256',checksum_sha256,'encrypted_size_bytes',encrypted_size_bytes,'retired_at',retired_at) FROM snapshot_manifests WHERE generation_id=$1 AND (verified_at IS NOT NULL OR id=$2) ORDER BY snapshot_at,id").bind(snapshot.generation).bind(snapshot.id).fetch_all(&mut *tx).await?;
    let row=sqlx::query("UPDATE replication_generations SET manifest_revision=manifest_revision+1 WHERE id=$1 RETURNING manifest_revision,start_reason").bind(snapshot.generation).fetch_one(&mut *tx).await?;
    let document=serde_json::to_vec(&serde_json::json!({"version":1,"tenant_id":db.tenant_id(),"generation_id":snapshot.generation,"ownership_generation":db.ownership_generation(),"start_reason":row.get::<String,_>(1),"revision":row.get::<i64,_>(0),"delta":{"snapshot":snapshot},"segments":segments,"snapshots":snapshots})).map_err(fail)?;
    db.ensure_current_owner()?;
    tx.commit().await?;
    Ok(document)
}

/// API-0021 hooks operate on the already gated migration writer, avoiding a
/// reentrant writer lock. Verification must finish before migration can proceed.
pub struct SnapshotHook {
    pub databases: Arc<TenantDbManager>,
    pub ledger: Arc<PostgresLedger>,
    pub store: Arc<dyn ObjectStore>,
    pub keys: Arc<TenantKeys>,
}
#[async_trait]
impl MigrationHook for SnapshotHook {
    async fn before(
        &self,
        context: MigrationContext,
        connection: &mut SqliteConnection,
    ) -> Result<(), AppError> {
        let db = self
            .databases
            .migration_handle(context.tenant_id, context.ownership_generation)
            .await?;
        take_with_connection(
            db,
            &self.ledger,
            self.store.as_ref(),
            &self.keys,
            Kind::PreMigration(context.to_version),
            Some(connection),
        )
        .await
    }
    async fn after(
        &self,
        context: MigrationContext,
        connection: &mut SqliteConnection,
    ) -> Result<(), AppError> {
        let db = self
            .databases
            .migration_handle(context.tenant_id, context.ownership_generation)
            .await?;
        take_with_connection(
            db,
            &self.ledger,
            self.store.as_ref(),
            &self.keys,
            Kind::PostMigration(context.to_version),
            Some(connection),
        )
        .await
    }
}
