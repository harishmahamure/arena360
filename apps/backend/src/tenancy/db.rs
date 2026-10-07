use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use futures::future::BoxFuture;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Connection, SqliteConnection, SqlitePool};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use crate::control::LeaseClient;
use crate::error::AppError;

pub trait TenantLease: Send + Sync {
    fn writable_generation(&self, tenant_id: Uuid) -> Result<i64, AppError>;
    fn ensure_writable(&self, tenant_id: Uuid, expected_generation: i64) -> Result<(), AppError>;
}

pub trait TenantCommitNotifier: Send + Sync {
    fn registered(&self, tenant_id: Uuid, current_sequence: i64);
    fn committed(&self, tenant_id: Uuid, sequences: &[i64]);
}

struct NoopTenantCommitNotifier;

impl TenantCommitNotifier for NoopTenantCommitNotifier {
    fn registered(&self, _tenant_id: Uuid, _current_sequence: i64) {}
    fn committed(&self, _tenant_id: Uuid, _sequences: &[i64]) {}
}

impl TenantLease for LeaseClient {
    fn writable_generation(&self, tenant_id: Uuid) -> Result<i64, AppError> {
        LeaseClient::writable_generation(self, tenant_id)
    }

    fn ensure_writable(&self, tenant_id: Uuid, expected_generation: i64) -> Result<(), AppError> {
        LeaseClient::ensure_writable(self, tenant_id, expected_generation)
    }
}

#[derive(Debug, Clone)]
pub struct TenantDbConfig {
    pub root: PathBuf,
    pub read_connections: u32,
    pub busy_timeout: Duration,
    pub idle_timeout: Duration,
    pub reaper_interval: Duration,
}

impl TenantDbConfig {
    pub fn validate(self) -> Result<Self, AppError> {
        if self.read_connections == 0
            || self.busy_timeout.is_zero()
            || self.idle_timeout.is_zero()
            || self.reaper_interval.is_zero()
        {
            return Err(AppError::Internal(
                "invalid tenant database configuration".into(),
            ));
        }
        Ok(self)
    }
}

impl Default for TenantDbConfig {
    fn default() -> Self {
        Self {
            root: PathBuf::from("data/tenants"),
            read_connections: 4,
            busy_timeout: Duration::from_millis(250),
            idle_timeout: Duration::from_secs(10 * 60),
            reaper_interval: Duration::from_secs(30),
        }
    }
}

pub struct TenantDb {
    tenant_id: Uuid,
    ownership_generation: i64,
    path: PathBuf,
    lease: Arc<dyn TenantLease>,
    notifier: Arc<dyn TenantCommitNotifier>,
    writer: Mutex<Option<SqliteConnection>>,
    readers: SqlitePool,
    last_used: StdMutex<Instant>,
    closed: AtomicBool,
}

impl TenantDb {
    pub async fn timezone(&self) -> Result<String, AppError> {
        let timezone: String =
            sqlx::query_scalar("SELECT timezone FROM tenant_runtime WHERE singleton=1")
                .fetch_optional(&self.read_pool()?)
                .await?
                .ok_or_else(|| {
                    AppError::Internal("Tenant timezone has not been projected".into())
                })?;
        timezone
            .parse::<chrono_tz::Tz>()
            .map_err(|_| AppError::Internal("Invalid projected tenant timezone".into()))?;
        Ok(timezone)
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn ownership_generation(&self) -> i64 {
        self.ownership_generation
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn read_pool(&self) -> Result<SqlitePool, AppError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(AppError::Forbidden("Tenant database is closed".into()));
        }
        self.touch()?;
        Ok(self.readers.clone())
    }

    /// Background polling must not keep an otherwise idle tenant handle alive.
    pub(crate) fn background_read_pool(&self) -> Result<SqlitePool, AppError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(AppError::Forbidden("Tenant database is closed".into()));
        }
        Ok(self.readers.clone())
    }

    pub async fn with_writer<T, F>(&self, operation: F) -> Result<T, AppError>
    where
        F: for<'connection> FnOnce(
            &'connection mut SqliteConnection,
        ) -> BoxFuture<'connection, Result<T, AppError>>,
    {
        self.lease
            .ensure_writable(self.tenant_id, self.ownership_generation)?;
        let mut writer = self.writer.lock().await;
        self.lease
            .ensure_writable(self.tenant_id, self.ownership_generation)?;
        if self.closed.load(Ordering::Acquire) {
            return Err(AppError::Forbidden("Tenant database is closed".into()));
        }
        let connection = writer
            .as_mut()
            .ok_or_else(|| AppError::Forbidden("Tenant writer is closed".into()))?;
        self.touch()?;
        operation(connection).await
    }

    /// Runs a tenant mutation in an immediate SQLite transaction.
    ///
    /// The lease is checked by `with_writer` before the operation and again after the operation,
    /// immediately before commit. A cell fenced while the operation is running therefore rolls
    /// back instead of committing stale-generation data.
    pub async fn with_immediate_writer<T: Send, F>(&self, operation: F) -> Result<T, AppError>
    where
        F: for<'connection> FnOnce(
                &'connection mut SqliteConnection,
            ) -> BoxFuture<'connection, Result<T, AppError>>
            + Send,
    {
        self.lease
            .ensure_writable(self.tenant_id, self.ownership_generation)?;
        let mut writer = self.writer.lock().await;
        self.lease
            .ensure_writable(self.tenant_id, self.ownership_generation)?;
        if self.closed.load(Ordering::Acquire) {
            return Err(AppError::Forbidden("Tenant database is closed".into()));
        }
        let connection = writer
            .as_mut()
            .ok_or_else(|| AppError::Forbidden("Tenant writer is closed".into()))?;
        self.touch()?;
        let previous_sequence: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(sequence), 0) FROM outbox_events")
                .fetch_one(&mut *connection)
                .await?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *connection)
            .await?;
        let value = match operation(connection).await {
            Ok(value) => value,
            Err(error) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *connection).await;
                return Err(error);
            }
        };
        let sequences: Vec<i64> = match sqlx::query_scalar(
            "SELECT sequence FROM outbox_events WHERE sequence > ? ORDER BY sequence",
        )
        .bind(previous_sequence)
        .fetch_all(&mut *connection)
        .await
        {
            Ok(sequences) => sequences,
            Err(error) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *connection).await;
                return Err(error.into());
            }
        };
        if let Err(error) = self
            .lease
            .ensure_writable(self.tenant_id, self.ownership_generation)
        {
            let _ = sqlx::query("ROLLBACK").execute(&mut *connection).await;
            return Err(error);
        }
        if let Err(error) = sqlx::query("COMMIT").execute(&mut *connection).await {
            let _ = sqlx::query("ROLLBACK").execute(&mut *connection).await;
            return Err(error.into());
        }
        self.notifier.committed(self.tenant_id, &sequences);
        Ok(value)
    }

    pub async fn close(&self) -> Result<(), AppError> {
        if self.closed.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        self.readers.close().await;
        if let Some(writer) = self.writer.lock().await.take() {
            writer.close().await?;
        }
        Ok(())
    }

    fn idle_for(&self, now: Instant) -> Result<Duration, AppError> {
        self.last_used
            .lock()
            .map(|last_used| now.saturating_duration_since(*last_used))
            .map_err(|_| AppError::Internal("tenant database activity lock poisoned".into()))
    }

    fn touch(&self) -> Result<(), AppError> {
        *self
            .last_used
            .lock()
            .map_err(|_| AppError::Internal("tenant database activity lock poisoned".into()))? =
            Instant::now();
        Ok(())
    }
}

#[derive(Clone)]
pub struct TenantDbManager {
    config: TenantDbConfig,
    lease: Arc<dyn TenantLease>,
    handles: Arc<RwLock<HashMap<Uuid, Arc<TenantDb>>>>,
    open_gate: Arc<Mutex<()>>,
    notifier: Arc<dyn TenantCommitNotifier>,
}

impl TenantDbManager {
    pub fn new(config: TenantDbConfig, lease: Arc<dyn TenantLease>) -> Result<Self, AppError> {
        Ok(Self {
            config: config.validate()?,
            lease,
            handles: Arc::new(RwLock::new(HashMap::new())),
            open_gate: Arc::new(Mutex::new(())),
            notifier: Arc::new(NoopTenantCommitNotifier),
        })
    }

    pub fn with_commit_notifier(mut self, notifier: Arc<dyn TenantCommitNotifier>) -> Self {
        self.notifier = notifier;
        self
    }

    pub async fn open(&self, tenant_id: Uuid) -> Result<Arc<TenantDb>, AppError> {
        let generation = self.lease.writable_generation(tenant_id)?;
        if let Some(handle) = self.handles.read().await.get(&tenant_id).cloned() {
            if handle.ownership_generation == generation && !handle.closed.load(Ordering::Acquire) {
                handle.touch()?;
                return Ok(handle);
            }
        }

        let _open = self.open_gate.lock().await;
        let generation = self.lease.writable_generation(tenant_id)?;
        if let Some(handle) = self.handles.read().await.get(&tenant_id).cloned() {
            if handle.ownership_generation == generation && !handle.closed.load(Ordering::Acquire) {
                handle.touch()?;
                return Ok(handle);
            }
        }
        if let Some(stale) = self.handles.write().await.remove(&tenant_id) {
            stale.close().await?;
        }

        let handle = Arc::new(
            open_tenant(
                tenant_id,
                generation,
                &self.config,
                self.lease.clone(),
                self.notifier.clone(),
            )
            .await?,
        );
        self.handles.write().await.insert(tenant_id, handle.clone());
        Ok(handle)
    }

    pub async fn reap_idle(&self) -> Result<usize, AppError> {
        let now = Instant::now();
        let handles = self
            .handles
            .read()
            .await
            .iter()
            .map(|(tenant_id, handle)| (*tenant_id, handle.clone()))
            .collect::<Vec<_>>();
        let mut close_ids = Vec::new();
        for (tenant_id, handle) in handles {
            let lease_invalid = self
                .lease
                .ensure_writable(tenant_id, handle.ownership_generation)
                .is_err();
            if lease_invalid || handle.idle_for(now)? >= self.config.idle_timeout {
                close_ids.push(tenant_id);
            }
        }

        let mut closed = Vec::new();
        {
            let mut handles = self.handles.write().await;
            for tenant_id in close_ids {
                if let Some(handle) = handles.remove(&tenant_id) {
                    closed.push(handle);
                }
            }
        }
        let count = closed.len();
        for handle in closed {
            handle.close().await?;
        }
        Ok(count)
    }

    pub fn spawn_reaper(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(self.config.reaper_interval);
            interval.tick().await;
            loop {
                interval.tick().await;
                if let Err(error) = self.reap_idle().await {
                    tracing::warn!(%error, "Tenant database reaper failed");
                }
            }
        })
    }

    pub async fn open_count(&self) -> usize {
        self.handles.read().await.len()
    }

    pub async fn open_handles(&self) -> Vec<Arc<TenantDb>> {
        self.handles
            .read()
            .await
            .values()
            .filter(|handle| !handle.closed.load(Ordering::Acquire))
            .cloned()
            .collect()
    }
}

async fn open_tenant(
    tenant_id: Uuid,
    generation: i64,
    config: &TenantDbConfig,
    lease: Arc<dyn TenantLease>,
    notifier: Arc<dyn TenantCommitNotifier>,
) -> Result<TenantDb, AppError> {
    let path = tenant_path(&config.root, tenant_id);
    if !path.is_file() {
        return Err(AppError::NotFound(format!(
            "Tenant database does not exist: {}",
            path.display()
        )));
    }

    let writer_options = sqlite_options(&path, config.busy_timeout, false);
    let mut writer = SqliteConnection::connect_with(&writer_options).await?;
    let outbox_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_schema
            WHERE type = 'table' AND name = 'outbox_events'
        )",
    )
    .fetch_one(&mut writer)
    .await?;
    let current_sequence: i64 = if outbox_exists {
        // Once the table exists, query failures are real corruption/schema errors and
        // must not be mistaken for an unmigrated database.
        sqlx::query_scalar("SELECT COALESCE(MAX(sequence), 0) FROM outbox_events")
            .fetch_one(&mut writer)
            .await?
    } else {
        // Schema-version-0 files are opened by the migration orchestrator before
        // outbox_events exists. Registering zero preserves no-history startup while
        // allowing this same handle's first post-migration commit to wake realtime.
        0
    };
    notifier.registered(tenant_id, current_sequence);
    let reader_options = sqlite_options(&path, config.busy_timeout, true);
    let readers = SqlitePoolOptions::new()
        .min_connections(0)
        .max_connections(config.read_connections)
        .acquire_timeout(config.busy_timeout)
        .connect_with(reader_options)
        .await?;

    Ok(TenantDb {
        tenant_id,
        ownership_generation: generation,
        path,
        lease,
        notifier,
        writer: Mutex::new(Some(writer)),
        readers,
        last_used: StdMutex::new(Instant::now()),
        closed: AtomicBool::new(false),
    })
}

fn sqlite_options(path: &Path, busy_timeout: Duration, read_only: bool) -> SqliteConnectOptions {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .read_only(read_only)
        .create_if_missing(false)
        .foreign_keys(true)
        .busy_timeout(busy_timeout);
    if read_only {
        options
    } else {
        options
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .pragma("wal_autocheckpoint", "0")
    }
}

pub fn tenant_path(root: &Path, tenant_id: Uuid) -> PathBuf {
    root.join(format!("tenant-{tenant_id}"))
        .join("operational.sqlite")
}
