//! Read-only report snapshots. This crate knows nothing about tenants or HTTP.
mod functions;
pub use rusqlite::types::Value as Parameter;
use rusqlite::{Connection, OpenFlags};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock, Weak},
    time::{Duration, Instant},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub const REPORT_TIMEOUT: Duration = Duration::from_secs(25);
const MAX_ROWS: usize = 10_000;
const MAX_BYTES: usize = 4 * 1024 * 1024;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("report capacity is busy; retry later")]
    Busy,
    #[error("report deadline exceeded")]
    Timeout,
    #[error("report result exceeds the limit; narrow the requested range")]
    TooLarge,
    #[error("{0}")]
    Sql(#[from] rusqlite::Error),
    #[error("{0}")]
    Decode(#[from] serde_json::Error),
    #[error("report task failed: {0}")]
    Task(String),
}
struct ConnectionState {
    connection: Connection,
    _worker: OwnedSemaphorePermit,
    _tenant: OwnedSemaphorePermit,
}
struct Inner {
    state: Mutex<Option<ConnectionState>>,
    deadline: Instant,
}
#[derive(Clone)]
pub struct Snapshot {
    inner: Arc<Inner>,
    pub timezone: chrono_tz::Tz,
    pub sequence: i64,
    pub schema: i64,
}
fn permits(path: &Path) -> Result<(OwnedSemaphorePermit, OwnedSemaphorePermit), Error> {
    static WORKER: OnceLock<Arc<Semaphore>> = OnceLock::new();
    static TENANTS: OnceLock<Mutex<HashMap<PathBuf, Weak<Semaphore>>>> = OnceLock::new();
    let mut tenants = TENANTS
        .get_or_init(Default::default)
        .lock()
        .map_err(|e| Error::Task(e.to_string()))?;
    tenants.retain(|_, gate| gate.strong_count() > 0);
    let gate = tenants
        .get(path)
        .and_then(Weak::upgrade)
        .unwrap_or_else(|| {
            let gate = Arc::new(Semaphore::new(2));
            tenants.insert(path.to_owned(), Arc::downgrade(&gate));
            gate
        });
    let tenant = gate.try_acquire_owned().map_err(|_| Error::Busy)?;
    let worker = WORKER
        .get_or_init(|| Arc::new(Semaphore::new(8)))
        .clone()
        .try_acquire_owned()
        .map_err(|_| Error::Busy)?;
    Ok((worker, tenant))
}
impl Snapshot {
    pub async fn open(path: PathBuf) -> Result<Self, Error> {
        Self::open_with_timeout(path, REPORT_TIMEOUT).await
    }
    async fn open_with_timeout(path: PathBuf, timeout: Duration) -> Result<Self, Error> {
        let (worker, tenant) = permits(&path)?;
        let snapshot = tokio::task::spawn_blocking(move || {
            let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
            connection.busy_timeout(Duration::from_millis(250))?;
            connection.execute_batch("PRAGMA query_only=ON; PRAGMA trusted_schema=OFF; PRAGMA cache_size=-2048; PRAGMA temp_store=FILE; BEGIN DEFERRED;")?;
            // The first read pins the WAL snapshot for every query in a dashboard.
            let (timezone, sequence, schema): (String,i64,i64) = connection.query_row(
                "SELECT timezone,COALESCE((SELECT seq FROM sqlite_sequence WHERE name='outbox_events'),0),(SELECT MAX(version) FROM _sqlx_migrations WHERE success=1) FROM tenant_runtime WHERE singleton=1", [], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)))?;
            let timezone = timezone.parse().map_err(|_| Error::Task("invalid tenant timezone".into()))?;
            functions::register(&connection, timezone)?;
            let deadline = Instant::now() + timeout;
            connection.progress_handler(2000, Some(move || Instant::now() >= deadline));
            Ok::<_,Error>(Self { inner: Arc::new(Inner { state: Mutex::new(Some(ConnectionState {connection,_worker:worker,_tenant:tenant})), deadline }), timezone, sequence, schema })
        }).await.map_err(|e| Error::Task(e.to_string()))??;
        // A reader retained by a cache or abandoned request must not pin the WAL indefinitely.
        let weak = Arc::downgrade(&snapshot.inner);
        tokio::spawn(async move {
            tokio::time::sleep(timeout).await;
            if let Some(inner) = weak.upgrade() {
                let _ = tokio::task::spawn_blocking(move || {
                    if let Ok(mut state) = inner.state.lock() {
                        state.take();
                    }
                })
                .await;
            }
        });
        Ok(snapshot)
    }
    pub fn ensure_ready(&self) -> Result<(), Error> {
        if Instant::now() >= self.inner.deadline {
            Err(Error::Timeout)
        } else {
            Ok(())
        }
    }
    pub async fn query<T: DeserializeOwned + Send + 'static>(
        &self,
        sql: String,
        params: Vec<Parameter>,
    ) -> Result<Vec<T>, Error> {
        self.ensure_ready()?;
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = inner.state.lock().map_err(|e| Error::Task(e.to_string()))?;
            if Instant::now() >= inner.deadline {
                return Err(Error::Timeout);
            }
            let state = guard.as_ref().ok_or(Error::Timeout)?;
            let mut statement = state.connection.prepare(&sql)?;
            if !statement.readonly() {
                return Err(Error::Task("report statement must be read-only".into()));
            }
            // SQLite assigns named parameter slots in occurrence order, not numeric order.
            for (i, value) in params.iter().enumerate() {
                if let Some(slot) = statement.parameter_index(&format!("${}", i + 1))? {
                    statement.raw_bind_parameter(slot, value)?;
                }
            }
            let columns = statement.column_count();
            let mut rows = statement.raw_query();
            let mut result = Vec::new();
            let mut bytes = 0;
            while let Some(row) = rows.next().map_err(|e| {
                if Instant::now() >= inner.deadline {
                    Error::Timeout
                } else {
                    Error::Sql(e)
                }
            })? {
                let mut values = Vec::with_capacity(columns);
                for i in 0..columns {
                    values.push(match row.get_ref(i)? {
                        rusqlite::types::ValueRef::Null => Value::Null,
                        rusqlite::types::ValueRef::Integer(v) => Value::from(v),
                        rusqlite::types::ValueRef::Real(v) => Value::from(v),
                        rusqlite::types::ValueRef::Text(v) => Value::String(
                            std::str::from_utf8(v)
                                .map_err(|e| Error::Task(e.to_string()))?
                                .to_owned(),
                        ),
                        _ => return Err(Error::Task("unsupported report column".into())),
                    });
                }
                let value = Value::Array(values);
                bytes += serde_json::to_vec(&value)?.len();
                if result.len() >= MAX_ROWS || bytes > MAX_BYTES {
                    return Err(Error::TooLarge);
                }
                result.push(serde_json::from_value(value)?);
            }
            Ok(result)
        })
        .await
        .map_err(|e| Error::Task(e.to_string()))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> PathBuf {
        static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "arena360-report-engine-{}-{}-{}.sqlite",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap(),
            ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let db = Connection::open(&path).unwrap();
        db.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE tenant_runtime(singleton INTEGER, timezone TEXT); INSERT INTO tenant_runtime VALUES(1,'Asia/Kolkata'); CREATE TABLE outbox_events(sequence INTEGER PRIMARY KEY AUTOINCREMENT); CREATE TABLE _sqlx_migrations(version INTEGER,success INTEGER); INSERT INTO _sqlx_migrations VALUES(18,1);").unwrap();
        path
    }
    #[tokio::test]
    async fn named_parameters_calendar_and_exact_money() {
        let path = fixture();
        let snapshot = Snapshot::open(path.clone()).await.unwrap();
        let rows:Vec<(String,i64,String,String)>=snapshot.query("SELECT $2,$1,report_local_date('2026-10-01T19:00:00.000000Z'),report_money_text(9007199254740993,4)".into(),vec![Parameter::Integer(7),Parameter::Text("bound-second".into())]).await.unwrap();
        assert_eq!(
            rows,
            vec![(
                "bound-second".into(),
                7,
                "2026-10-02".into(),
                "900719925474.0993".into()
            )]
        );
        let rows:Vec<(String,String,f64)>=snapshot.query("SELECT report_money_text(10050,2),report_money_text(-10050,2),report_allocate(10000,1,3)".into(),vec![]).await.unwrap();
        assert_eq!(rows[0].0, "1.01");
        assert_eq!(rows[0].1, "-1.01");
        assert!((rows[0].2 - 10000.0 / 3.0).abs() < 1e-8);
        drop(snapshot);
        std::fs::remove_file(path).unwrap();
    }
    #[tokio::test]
    async fn tenant_concurrency_admission_and_readonly_enforcement() {
        let path = fixture();
        let one = Snapshot::open(path.clone()).await.unwrap();
        let two = Snapshot::open(path.clone()).await.unwrap();
        assert!(matches!(
            Snapshot::open(path.clone()).await,
            Err(Error::Busy)
        ));
        assert!(one
            .query::<(i64,)>(
                "DELETE FROM tenant_runtime RETURNING singleton".into(),
                vec![]
            )
            .await
            .is_err());
        drop(one);
        let third = Snapshot::open(path.clone()).await.unwrap();
        drop((two, third));
        std::fs::remove_file(path).unwrap();
    }
    #[tokio::test]
    async fn long_queries_are_interrupted_and_result_size_is_bounded() {
        let path = fixture();
        let snapshot = Snapshot::open_with_timeout(path.clone(), Duration::from_millis(100))
            .await
            .unwrap();
        let error=snapshot.query::<(i64,)>("WITH RECURSIVE numbers(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM numbers WHERE n<100000000) SELECT SUM(n) FROM numbers".into(),vec![]).await.unwrap_err();
        assert!(matches!(error, Error::Timeout));
        drop(snapshot);
        let snapshot = Snapshot::open(path.clone()).await.unwrap();
        assert!(matches!(
            snapshot
                .query::<(String,)>("SELECT hex(zeroblob(3000000))".into(), vec![])
                .await,
            Err(Error::TooLarge)
        ));
        drop(snapshot);
        std::fs::remove_file(path).unwrap();
    }
}
