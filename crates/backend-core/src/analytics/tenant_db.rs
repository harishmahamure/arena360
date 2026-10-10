//! Derived tenant storage. SQLite ownership fences all opens, reads and commits.
use crate::{error::AppError, tenancy::TenantDb};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use duckdb::{params, Connection, Transaction};
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
const MIGRATIONS: &[(i64, &str)] = &[
    (
        1,
        include_str!("../../migrations/analytics/0001_initial.sql"),
    ),
    (
        2,
        include_str!("../../migrations/analytics/0002_rebuild_replay_floor.sql"),
    ),
    (3, include_str!("../../migrations/analytics/0003_report_transaction_created_at.sql")),
];
pub const SCHEMA_VERSION: i64 = 3;

pub struct TenantAnalytics {
    db: Arc<TenantDb>,
    connection: Mutex<Option<Connection>>,
    closed: std::sync::atomic::AtomicBool,
    pub(crate) rebuild_lock: tokio::sync::Mutex<()>,
    _rebuild_files: Option<Arc<super::rebuild::RebuildFiles>>,
    path: PathBuf,
}
impl TenantAnalytics {
    pub async fn open(db: Arc<TenantDb>) -> Result<Arc<Self>, AppError> {
        Self::open_internal(db, false).await
    }
    async fn open_internal(db: Arc<TenantDb>, require_latest: bool) -> Result<Arc<Self>, AppError> {
        let _job = match db.background_jobs() {
            Some(jobs) => Some(
                jobs.acquire(crate::background::Priority::AnalyticsIngestion)
                    .await?,
            ),
            None => None,
        };
        db.ensure_current_owner()?;
        let timezone: String =
            sqlx::query_scalar("SELECT timezone FROM tenant_runtime WHERE singleton=1")
                .fetch_one(&db.background_read_pool()?)
                .await?;
        blocking(move || {
            db.ensure_current_owner()?;
            let path = db.path().with_file_name("analytics.duckdb");
            let config = duckdb::Config::default()
                .threads(1)
                .map_err(error)?
                .max_memory("128MB")
                .map_err(error)?;
            let mut connection = Connection::open_with_flags(&path, config).map_err(error)?;
            // Validated embedded migrations preserve sealed monthly rows. Changed
            // history/corruption is quarantined; a known upgrade sets REBUILDING
            // before any reader or consumer can use facts in the old projection.
            migrate_with_policy(&mut connection, &db, &timezone, Utc::now(), require_latest)?;
            Ok(Arc::new(Self {
                db,
                connection: Mutex::new(Some(connection)),
                closed: std::sync::atomic::AtomicBool::new(false),
                rebuild_lock: tokio::sync::Mutex::new(()),
                _rebuild_files: None,
                path,
            }))
        })
        .await
    }
    /// Ingestion can rebuild corrupt/outdated derived files from authoritative SQLite.
    /// A newer binary's schema is preserved and requires upgrading this binary.
    pub async fn open_for_ingestion(db: Arc<TenantDb>) -> Result<Arc<Self>, AppError> {
        match Self::open_internal(db.clone(), true).await {
            Ok(handle) => Ok(handle),
            Err(AppError::Internal(message))
                if !message.contains("newer than this binary")
                    && (message.starts_with("DuckDB:")
                        || message.contains("migration history/checksum mismatch")
                        || message.contains("state disagrees with migration history")
                        || message.contains("schema requires rebuild")) =>
            {
                db.ensure_current_owner()?;
                let path = db.path().with_file_name("analytics.duckdb");
                if !path.exists() {
                    return Err(AppError::Internal(message));
                }
                let quarantine = db.path().with_file_name(format!(
                    "analytics.quarantined-{}.duckdb",
                    uuid::Uuid::new_v4()
                ));
                std::fs::rename(&path, &quarantine)
                    .map_err(|e| AppError::Internal(e.to_string()))?;
                let wal = path.with_extension("duckdb.wal");
                if wal.exists() {
                    std::fs::rename(wal, quarantine.with_extension("duckdb.wal"))
                        .map_err(|e| AppError::Internal(e.to_string()))?;
                }
                db.ensure_current_owner()?;
                tracing::warn!(tenant=%db.tenant_id(),%message,"Derived analytics file quarantined for rebuild");
                Self::open(db).await
            }
            Err(error) => Err(error),
        }
    }
    pub(crate) async fn shadow(
        db: Arc<TenantDb>,
        files: Arc<super::rebuild::RebuildFiles>,
        timezone: String,
    ) -> Result<Arc<Self>, AppError> {
        blocking(move || {
            db.ensure_current_owner()?;
            let path = files.root.join("analytics.duckdb");
            let config = duckdb::Config::default()
                .threads(1)
                .map_err(error)?
                .max_memory("128MB")
                .map_err(error)?;
            let mut connection = Connection::open_with_flags(&path, config).map_err(error)?;
            migrate_at(&mut connection, &db, &timezone, Utc::now())?;
            Ok(Arc::new(Self {
                db,
                connection: Mutex::new(Some(connection)),
                closed: std::sync::atomic::AtomicBool::new(false),
                path,
                rebuild_lock: tokio::sync::Mutex::new(()),
                _rebuild_files: Some(files),
            }))
        })
        .await
    }
    /// Close both databases before the atomic rename; no connection keeps a stale WAL path.
    pub(crate) async fn install(self: &Arc<Self>, shadow: Arc<Self>) -> Result<(), AppError> {
        let this = self.clone();
        blocking(move || {
            this.db.ensure_current_owner()?;
            if this.tenant_id() != shadow.tenant_id() {
                return Err(AppError::Forbidden("Foreign rebuild".into()));
            }
            let mut live = this
                .connection
                .lock()
                .map_err(|_| AppError::Internal("Analytics lock poisoned".into()))?;
            let mut staged = shadow
                .connection
                .lock()
                .map_err(|_| AppError::Internal("Analytics lock poisoned".into()))?;
            staged
                .as_ref()
                .ok_or_else(|| AppError::Internal("Rebuild already installed".into()))?
                .execute_batch("CHECKPOINT")
                .map_err(error)?;
            live.as_ref()
                .ok_or_else(|| AppError::Internal("Analytics file unavailable".into()))?
                .execute_batch("CHECKPOINT")
                .map_err(error)?;
            this.db.ensure_current_owner()?;
            drop(staged.take());
            drop(live.take());
            let previous = shadow.path.with_file_name("previous.duckdb");
            let switch = (|| {
                std::fs::hard_link(&this.path, &previous)
                    .map_err(|e| AppError::Internal(e.to_string()))?;
                if let Err(e) = std::fs::rename(&shadow.path, &this.path) {
                    return Err(AppError::Internal(e.to_string()));
                }
                let config = duckdb::Config::default()
                    .threads(1)
                    .map_err(error)?
                    .max_memory("128MB")
                    .map_err(error)?;
                let connection = Connection::open_with_flags(&this.path, config).map_err(error)?;
                this.db.ensure_current_owner()?;
                *live = Some(connection);
                Ok(())
            })();
            if switch.is_err() {
                if previous.exists() {
                    std::fs::rename(&previous, &this.path)
                        .map_err(|e| AppError::Internal(e.to_string()))?;
                }
                this.db.ensure_current_owner()?;
                let config = duckdb::Config::default()
                    .threads(1)
                    .map_err(error)?
                    .max_memory("128MB")
                    .map_err(error)?;
                *live = Some(Connection::open_with_flags(&this.path, config).map_err(error)?);
            }
            switch
        })
        .await
    }
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
    /// Drop the connection under the same lock used by reads, writes and installs.
    /// No ownership check: fencing must still allow resource cleanup.
    pub(crate) async fn close(self: &Arc<Self>) -> Result<(), AppError> {
        self.closed.store(true, std::sync::atomic::Ordering::Release);
        let this = self.clone();
        blocking(move || {
            let mut connection = this.connection.lock()
                .map_err(|_| AppError::Internal("Analytics connection lock poisoned".into()))?;
            drop(connection.take());
            Ok(())
        }).await
    }
    pub(crate) fn is_closed(&self) -> bool {
        self.closed.load(std::sync::atomic::Ordering::Acquire)
    }
    /// Changed calendars cannot serve facts or accept events until the shadow rebuild.
    /// Keep the stored timezone until replacement so old labels are never relabelled in place.
    pub async fn ensure_timezone(self: &Arc<Self>) -> Result<bool, AppError> {
        let timezone: String =
            sqlx::query_scalar("SELECT timezone FROM tenant_runtime WHERE singleton=1")
                .fetch_one(&self.db.background_read_pool()?)
                .await?;
        self.write(move |tx| {
            let stored: String = tx
                .query_row("SELECT timezone FROM _ingest_state", [], |r| r.get(0))
                .map_err(error)?;
            if stored == timezone {
                return Ok(true);
            }
            tx.execute_batch("UPDATE _ingest_state SET status='REBUILDING'")
                .map_err(error)?;
            Ok(false)
        })
        .await
    }
    pub(crate) fn owner(&self) -> &Arc<TenantDb> {
        &self.db
    }
    pub fn tenant_id(&self) -> uuid::Uuid {
        self.db.tenant_id()
    }
    /// A callback sees one snapshot and its transaction always rolls back.
    pub async fn read<T: Send + 'static, F>(self: &Arc<Self>, operation: F) -> Result<T, AppError>
    where
        F: FnOnce(&Transaction<'_>) -> Result<T, AppError> + Send + 'static,
    {
        let this = self.clone();
        blocking(move || {
            this.db.ensure_current_owner()?;
            let mut connection = this
                .connection
                .lock()
                .map_err(|_| AppError::Internal("Analytics connection lock poisoned".into()))?;
            if this.is_closed() {
                return Err(AppError::Internal("Analytics connection closed".into()));
            }
            let tx = connection
                .as_mut()
                .ok_or_else(|| {
                    AppError::Internal("Analytics file unavailable during switch".into())
                })?
                .transaction()
                .map_err(error)?;
            let value = operation(&tx)?;
            tx.rollback().map_err(error)?;
            this.db.ensure_current_owner()?;
            if this.is_closed() {
                return Err(AppError::Internal("Analytics connection closed".into()));
            }
            Ok(value)
        })
        .await
    }
    /// The caller's changes and ingestion checkpoint commit together after fencing.
    pub async fn write<T: Send + 'static, F>(self: &Arc<Self>, operation: F) -> Result<T, AppError>
    where
        F: FnOnce(&Transaction<'_>) -> Result<T, AppError> + Send + 'static,
    {
        let this = self.clone();
        blocking(move || {
            this.db.ensure_current_owner()?;
            let mut connection = this
                .connection
                .lock()
                .map_err(|_| AppError::Internal("Analytics connection lock poisoned".into()))?;
            if this.is_closed() {
                return Err(AppError::Internal("Analytics connection closed".into()));
            }
            let tx = connection
                .as_mut()
                .ok_or_else(|| {
                    AppError::Internal("Analytics file unavailable during switch".into())
                })?
                .transaction()
                .map_err(error)?;
            let value = operation(&tx)?;
            this.db.ensure_current_owner()?;
            if this.is_closed() {
                return Err(AppError::Internal("Analytics connection closed".into()));
            }
            tx.commit().map_err(error)?;
            Ok(value)
        })
        .await
    }
}
async fn blocking<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|e| AppError::Internal(format!("Analytics task failed: {e}")))?
}
pub fn error(error: duckdb::Error) -> AppError {
    AppError::Internal(format!("DuckDB: {error}"))
}

/// All DDL, checksums and initial state are transactional. Existing facts/checkpoints
/// survive repeated opens; unknown versions, changed SQL and time-zone drift fail closed.
pub fn migrate(
    connection: &mut Connection,
    owner: &TenantDb,
    timezone: &str,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    migrate_with_policy(connection, owner, timezone, now, false)
}
fn migrate_with_policy(
    connection: &mut Connection,
    owner: &TenantDb,
    timezone: &str,
    now: DateTime<Utc>,
    allow_timezone_rebuild: bool,
) -> Result<(), AppError> {
    owner.ensure_current_owner()?;
    let expected = owner.path().with_file_name("analytics.duckdb");
    if connection.path() != Some(expected.as_path()) {
        return Err(AppError::Forbidden(
            "Foreign analytics database path".into(),
        ));
    }
    migrate_at_with_policy(connection, owner, timezone, now, allow_timezone_rebuild)
}
pub(crate) fn migrate_at(
    connection: &mut Connection,
    owner: &TenantDb,
    timezone: &str,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    migrate_at_with_policy(connection, owner, timezone, now, false)
}
fn migrate_at_with_policy(
    connection: &mut Connection,
    owner: &TenantDb,
    timezone: &str,
    now: DateTime<Utc>,
    allow_timezone_rebuild: bool,
) -> Result<(), AppError> {
    owner.ensure_current_owner()?;
    let zone = timezone
        .parse::<chrono_tz::Tz>()
        .map_err(|_| AppError::Internal("Invalid analytics timezone".into()))?;
    let tx = connection.transaction().map_err(error)?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS _schema_migrations(version INTEGER PRIMARY KEY,checksum VARCHAR NOT NULL,applied_at TIMESTAMP NOT NULL)").map_err(error)?;
    let recorded = {
        let mut statement = tx
            .prepare("SELECT version,checksum FROM _schema_migrations ORDER BY version")
            .map_err(error)?;
        let rows = statement
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
            .map_err(error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)?;
        rows
    };
    for (index, (version, checksum)) in recorded.iter().enumerate() {
        let Some((expected, sql)) = MIGRATIONS.get(index) else {
            return Err(AppError::Internal(
                "Analytics schema is newer than this binary".into(),
            ));
        };
        if version != expected || checksum != &hex::encode(Sha256::digest(sql.as_bytes())) {
            return Err(AppError::Internal(
                "Analytics migration history/checksum mismatch".into(),
            ));
        }
    }
    let previous = recorded.last().map_or(0, |r| r.0);
    for (version, sql) in MIGRATIONS.iter().skip(recorded.len()) {
        tx.execute_batch(sql).map_err(error)?;
        tx.execute(
            "INSERT INTO _schema_migrations VALUES(?,?,CAST(? AS TIMESTAMP))",
            params![
                version,
                hex::encode(Sha256::digest(sql.as_bytes())),
                crate::time::format_sqlite_timestamp(&now)
                    .map_err(|e| AppError::Internal(e.to_string()))?
            ],
        )
        .map_err(error)?;
    }
    let count: i64 = tx
        .query_row("SELECT COUNT(*) FROM _ingest_state", [], |r| r.get(0))
        .map_err(error)?;
    if count == 0 {
        let local = now.with_timezone(&zone).date_naive();
        let month = local.year() * 12 + local.month0() as i32 - 18;
        let start =
            NaiveDate::from_ymd_opt(month.div_euclid(12), month.rem_euclid(12) as u32 + 1, 1)
                .ok_or_else(|| AppError::Internal("Invalid analytics hot window".into()))?;
        tx.execute("INSERT INTO _ingest_state(id,schema_version,status,last_sequence,hot_window_start,timezone,updated_at) VALUES(1,?,'REBUILDING',0,CAST(? AS DATE),?,CAST(? AS TIMESTAMP))",params![SCHEMA_VERSION,start.to_string(),timezone,crate::time::format_sqlite_timestamp(&now).map_err(|e|AppError::Internal(e.to_string()))?]).map_err(error)?;
    } else {
        let (version, stored_zone): (i64, String) = tx
            .query_row(
                "SELECT schema_version,timezone FROM _ingest_state WHERE id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(error)?;
        if version != previous {
            return Err(AppError::Internal(
                "Analytics state disagrees with migration history".into(),
            ));
        }
        if stored_zone != timezone && !allow_timezone_rebuild {
            return Err(AppError::Conflict(
                "Analytics timezone requires a rebuild".into(),
            ));
        }
        if stored_zone != timezone {
            tx.execute_batch("UPDATE _ingest_state SET status='REBUILDING'")
                .map_err(error)?;
        }
        tx.execute(
            "UPDATE _ingest_state SET schema_version=?,status=CASE WHEN schema_version<>? THEN 'REBUILDING' ELSE status END WHERE id=1",
            params![SCHEMA_VERSION,SCHEMA_VERSION],
        )
        .map_err(error)?;
    }
    owner.ensure_current_owner()?;
    tx.commit().map_err(error)?;
    Ok(())
}
