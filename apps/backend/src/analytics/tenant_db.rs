//! Derived tenant storage. SQLite ownership fences all opens, reads and commits.
use crate::{error::AppError, tenancy::TenantDb};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use duckdb::{params, Connection, Transaction};
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
const MIGRATIONS: &[(i64, &str)] = &[(
    1,
    include_str!("../../migrations/analytics/0001_initial.sql"),
)];
pub const SCHEMA_VERSION: i64 = 1;

pub struct TenantAnalytics {
    db: Arc<TenantDb>,
    connection: Mutex<Connection>,
    path: PathBuf,
}
impl TenantAnalytics {
    pub async fn open(db: Arc<TenantDb>) -> Result<Arc<Self>, AppError> {
        db.ensure_current_owner()?;
        let timezone = db.timezone().await?;
        blocking(move || {
            db.ensure_current_owner()?;
            let path = db.path().with_file_name("analytics.duckdb");
            let config = duckdb::Config::default()
                .threads(1)
                .map_err(error)?
                .max_memory("128MB")
                .map_err(error)?;
            let mut connection = Connection::open_with_flags(&path, config).map_err(error)?;
            migrate(&mut connection, &db, &timezone, Utc::now())?;
            Ok(Arc::new(Self {
                db,
                connection: Mutex::new(connection),
                path,
            }))
        })
        .await
    }
    pub fn path(&self) -> &std::path::Path {
        &self.path
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
            let tx = connection.transaction().map_err(error)?;
            let value = operation(&tx)?;
            tx.rollback().map_err(error)?;
            this.db.ensure_current_owner()?;
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
            let tx = connection.transaction().map_err(error)?;
            let value = operation(&tx)?;
            this.db.ensure_current_owner()?;
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
    owner.ensure_current_owner()?;
    let expected = owner.path().with_file_name("analytics.duckdb");
    if connection.path() != Some(expected.as_path()) {
        return Err(AppError::Forbidden(
            "Foreign analytics database path".into(),
        ));
    }
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
        if stored_zone != timezone {
            return Err(AppError::Conflict(
                "Analytics timezone requires a rebuild".into(),
            ));
        }
        tx.execute(
            "UPDATE _ingest_state SET schema_version=? WHERE id=1",
            params![SCHEMA_VERSION],
        )
        .map_err(error)?;
    }
    owner.ensure_current_owner()?;
    tx.commit().map_err(error)?;
    Ok(())
}
