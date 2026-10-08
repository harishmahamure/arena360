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
    background_jobs: Option<Arc<crate::background::BackgroundJobs>>,
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

    pub fn background_jobs(&self) -> Option<&Arc<crate::background::BackgroundJobs>> { self.background_jobs.as_ref() }
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

    pub(crate) fn ensure_current_owner(&self) -> Result<(), AppError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(AppError::Forbidden("Tenant database is closed".into()));
        }
        self.lease.ensure_writable(self.tenant_id,self.ownership_generation)
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
        let _operational = self.background_jobs.as_ref().map(|jobs| jobs.operational());
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
        let _operational = self.background_jobs.as_ref().map(|jobs| jobs.operational());
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
        let mut guard = self.writer.lock().await;
        // Last-connection checkpoints must obey the same durable-copy ordering
        // as scheduled checkpoints, including idle reaping and lease loss.
        if let Some(writer) = guard.as_mut() {
            spool_wal(writer, &self.path, self.ownership_generation).await?;
        }
        if let Some(writer) = guard.take() {
            writer.close().await?;
        }
        Ok(())
    }

    /// Copy the committed WAL under the single writer lock, then checkpoint.
    /// Network I/O is deliberately outside this critical section.
    pub async fn spool_wal(&self) -> Result<Option<PathBuf>, AppError> {
        let _job = match self.background_jobs() {
            Some(jobs) => Some(jobs.acquire(crate::background::Priority::Backup).await?),
            None => None,
        };
        self.ensure_current_owner()?;
        let mut writer = self.writer.lock().await;
        self.ensure_current_owner()?;
        let connection = writer.as_mut().ok_or_else(|| AppError::Forbidden("Tenant writer is closed".into()))?;
        let captured = spool_wal(connection, &self.path, self.ownership_generation).await?;
        self.ensure_current_owner()?;
        Ok(captured)
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
    background_jobs: Option<Arc<crate::background::BackgroundJobs>>,
}

impl TenantDbManager {
    pub fn new(config: TenantDbConfig, lease: Arc<dyn TenantLease>) -> Result<Self, AppError> {
        Ok(Self {
            config: config.validate()?,
            lease,
            handles: Arc::new(RwLock::new(HashMap::new())),
            open_gate: Arc::new(Mutex::new(())),
            notifier: Arc::new(NoopTenantCommitNotifier),
            background_jobs: None,
        })
    }

    pub fn with_commit_notifier(mut self, notifier: Arc<dyn TenantCommitNotifier>) -> Self {
        self.notifier = notifier;
        self
    }

    pub fn with_background_jobs(mut self, jobs: Arc<crate::background::BackgroundJobs>) -> Self {
        self.background_jobs = Some(jobs); self
    }
    pub async fn open(&self, tenant_id: Uuid) -> Result<Arc<TenantDb>, AppError> {
        let generation = self.lease.writable_generation(tenant_id)?;
        if tenant_path(&self.config.root,tenant_id).parent().unwrap().join("replication/recovery-pending.json").exists() {return Err(AppError::Conflict("Tenant recovery is in progress".into()));}
        if let Some(handle) = self.handles.read().await.get(&tenant_id).cloned() {
            if handle.ownership_generation == generation && !handle.closed.load(Ordering::Acquire) {
                handle.touch()?;
                return Ok(handle);
            }
        }

        let _open = self.open_gate.lock().await;
        if self.recovery_pending(tenant_id) {return Err(AppError::Conflict("Tenant recovery is in progress".into()));}
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
                self.background_jobs.clone(),
            )
            .await?,
        );
        self.handles.write().await.insert(tenant_id, handle.clone());
        Ok(handle)
    }

    pub(crate) fn recovery_jobs(&self)->Option<&Arc<crate::background::BackgroundJobs>> {self.background_jobs.as_ref()}
    pub(crate) fn recovery_pending(&self,tenant:Uuid)->bool {tenant_path(&self.config.root,tenant).parent().unwrap().join("replication/recovery-pending.json").exists()}
    pub(crate) fn recovery_path(&self,tenant:Uuid)->PathBuf {tenant_path(&self.config.root,tenant)}
    /// Recovery alone may open a quarantined image. Business opens remain
    /// blocked by the durable marker through failures and process restarts.
    pub(crate) async fn with_recovery_image<F,T>(&self,tenant:Uuid,installed:bool,source:Option<&Path>,capture:u64,transition:Uuid,finish:F)->Result<T,AppError>
    where F:FnOnce(Arc<TenantDb>)->BoxFuture<'static,Result<T,AppError>> {
        let _open=self.open_gate.lock().await;
        let generation=self.lease.writable_generation(tenant)?;
        if self.handles.read().await.contains_key(&tenant) {return Err(AppError::Conflict("Close the active tenant handle before recovery".into()));}
        let path=tenant_path(&self.config.root,tenant);
        let marker=path.parent().unwrap().join("replication/recovery-pending.json");
        crate::replication::wal::durable_create(&marker,&serde_json::to_vec(&serde_json::json!({"transition":transition,"ownership_generation":generation})).map_err(|e|AppError::Internal(e.to_string()))?)?;
        if !installed {
            let source=source.ok_or_else(||AppError::Internal("Recovery image is missing".into()))?;
            let temporary=path.with_extension(format!("{}.tmp",Uuid::new_v4()));
            tokio::fs::copy(source,&temporary).await.map_err(|e|AppError::Internal(e.to_string()))?;
            std::fs::File::open(&temporary).and_then(|f|f.sync_all()).map_err(|e|AppError::Internal(e.to_string()))?;
            self.lease.ensure_writable(tenant,generation)?;
            if path.exists() {
                if crate::replication::snapshot::hash_file(&path)? != crate::replication::snapshot::hash_file(&temporary)? {return Err(AppError::Conflict("Recovery cannot overwrite an existing different tenant image".into()));}
            } else {std::fs::hard_link(&temporary,&path).map_err(|e|AppError::Internal(e.to_string()))?;}
            std::fs::remove_file(&temporary).map_err(|e|AppError::Internal(e.to_string()))?;
            for name in ["capture-sequence","acknowledged-capture"] {crate::replication::wal::durable_create(&marker.parent().unwrap().join(name),capture.to_string().as_bytes())?;}
            std::fs::File::open(path.parent().unwrap()).and_then(|f|f.sync_all()).map_err(|e|AppError::Internal(e.to_string()))?;
        }
        let db=Arc::new(open_tenant(tenant,generation,&self.config,self.lease.clone(),self.notifier.clone(),self.background_jobs.clone()).await?);
        let result=finish(db.clone()).await;
        match result {
            Ok(value)=>{
                self.lease.ensure_writable(tenant,generation)?;
                std::fs::remove_file(&marker).map_err(|e|AppError::Internal(e.to_string()))?;
                std::fs::File::open(marker.parent().unwrap()).and_then(|f|f.sync_all()).map_err(|e|AppError::Internal(e.to_string()))?;
                self.handles.write().await.insert(tenant,db);Ok(value)
            }
            Err(error)=>{let _=db.close().await;Err(error)}
        }
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

    /// A migration hook already holds this handle's writer. Reopening a changed
    /// generation here would try to close that same writer and deadlock.
    pub(crate) async fn migration_handle(&self,tenant:Uuid,generation:i64)->Result<Arc<TenantDb>,AppError> {
        let db=self.handles.read().await.get(&tenant).cloned().ok_or_else(||AppError::Forbidden("Migration handle is unavailable".into()))?;
        if db.ownership_generation()!=generation {return Err(AppError::Conflict("Migration ownership generation changed".into()));}
        db.ensure_current_owner()?;Ok(db)
    }

    /// Closed tenants with unshipped WAL remain eligible for background upload.
    /// This checks an existing lease and never acquires ownership.
    pub async fn open_spooled(&self) -> Result<(), AppError> {
        if !self.config.root.exists() { return Ok(()); }
        for entry in std::fs::read_dir(&self.config.root).map_err(|e|AppError::Internal(e.to_string()))? {
            let entry=entry.map_err(|e|AppError::Internal(e.to_string()))?;
            let name=entry.file_name();
            let Some(tenant)=name.to_str().and_then(|s|s.strip_prefix("tenant-")).and_then(|s|Uuid::parse_str(s).ok()) else {continue;};
            let Ok(generation)=self.lease.writable_generation(tenant) else {continue;};
            if entry.path().join(format!("replication/batch-{generation}.json")).exists() {self.open(tenant).await?;continue;}
            let spool=entry.path().join("replication/spool");
            if !spool.exists() {continue;}
            let mut pending=false;
            for file in std::fs::read_dir(spool).map_err(|e|AppError::Internal(e.to_string()))? {
                let path=file.map_err(|e|AppError::Internal(e.to_string()))?.path();
                if path.extension().and_then(|s|s.to_str())!=Some("wal") {continue;}
                let metadata=std::fs::read(path.with_extension("json")).map_err(|e|AppError::Internal(e.to_string()))?;
                let capture:crate::replication::wal::Capture=serde_json::from_slice(&metadata).map_err(|e|AppError::Internal(e.to_string()))?;
                if capture.ownership_generation==generation {pending=true;break;}
            }
            if pending {self.open(tenant).await?;}
        }
        Ok(())
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
    background_jobs: Option<Arc<crate::background::BackgroundJobs>>,
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
    {
        let mut handle = writer.lock_handle().await?;
        // SAFETY: SQLx's locked handle excludes its worker for this synchronous
        // call. The output pointer is null, as allowed by sqlite3_db_config.
        let result = unsafe { libsqlite3_sys::sqlite3_db_config(
            handle.as_raw_handle().as_ptr(), libsqlite3_sys::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE,
            1i32, std::ptr::null_mut::<i32>()) };
        if result != libsqlite3_sys::SQLITE_OK { return Err(AppError::Internal("Cannot disable SQLite close checkpoint".into())); }
    }
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
        background_jobs,
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

pub(crate) async fn spool_wal(connection: &mut SqliteConnection, path: &Path, generation: i64) -> Result<Option<PathBuf>, AppError> {
    let mut transaction = connection.begin_with("BEGIN IMMEDIATE").await?;
    // Ensure SQLite has initialized the WAL index. The transaction's Drop
    // queues rollback if this future is cancelled during the SQL read.
    sqlx::query("SELECT count(*) FROM sqlite_schema").fetch_one(&mut *transaction).await?;
    // Keep the short filesystem critical section synchronous: cancellation
    // cannot release the writer while a detached task is still copying frames.
    let captured = crate::replication::wal::capture(path, generation)?;
    transaction.rollback().await?;
    {
        // TRUNCATE may report busy when readers pin older frames. That is safe:
        // the complete prefix is already durable, and next capture may repeat it.
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)").fetch_one(connection).await?;
    }
    Ok(captured)
}

pub fn tenant_path(root: &Path, tenant_id: Uuid) -> PathBuf {
    root.join(format!("tenant-{tenant_id}"))
        .join("operational.sqlite")
}
