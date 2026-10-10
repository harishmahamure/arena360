//! Fenced tenant reporting directly from a consistent operational SQLite snapshot.
use crate::{error::AppError, tenancy::TenantDb};
use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use sqlite_reporting::{Parameter as SqlValue, Snapshot};
use std::{marker::PhantomData, sync::Arc};
use uuid::Uuid;
pub fn unavailable(reason: &str) -> AppError {
    AppError::Api {
        code: "ANALYTICS_UNAVAILABLE".into(),
        status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
        details: Some(
            serde_json::json!({"message":"report temporarily unavailable","reason":reason}),
        ),
    }
}
fn error(e: sqlite_reporting::Error) -> AppError {
    match e {
        sqlite_reporting::Error::Busy | sqlite_reporting::Error::Timeout => {
            unavailable(&e.to_string())
        }
        sqlite_reporting::Error::TooLarge => AppError::BadRequest(e.to_string()),
        _ => AppError::Internal(format!("SQLite report: {e}")),
    }
}
#[derive(Clone)]
pub struct ReportReader {
    db: Arc<TenantDb>,
    snapshot: Snapshot,
    locations: Option<Vec<Uuid>>,
    now: DateTime<Utc>,
}
impl ReportReader {
    pub async fn new(db: Arc<TenantDb>) -> Result<Self, AppError> {
        db.ensure_current_owner()?;
        // Touch the owner handle so idle eviction cannot replace the file during this request.
        let _ = db.read_pool()?;
        let snapshot = Snapshot::open(db.path().to_owned()).await.map_err(error)?;
        db.ensure_current_owner()?;
        if snapshot.schema < 18 {
            return Err(unavailable(
                "tenant reporting schema migration 0018 is pending",
            ));
        }
        Ok(Self {
            db,
            snapshot,
            locations: None,
            now: Utc::now(),
        })
    }
    pub fn timezone(&self) -> chrono_tz::Tz {
        self.snapshot.timezone
    }
    pub fn now(&self) -> DateTime<Utc> {
        self.now
    }
    pub fn at(mut self, now: DateTime<Utc>) -> Self {
        self.now = now;
        self
    }
    pub fn scoped(mut self, locations: Option<Vec<Uuid>>) -> Self {
        self.locations = locations.map(|mut ids| {
            ids.sort();
            ids.dedup();
            ids
        });
        self
    }
    pub fn cache_key(&self) -> String {
        format!(
            "sqlite:{}:{}:{}:{}:{}:venues:{:?}",
            self.db.tenant_id(),
            self.db.ownership_generation(),
            self.snapshot.schema,
            self.timezone(),
            self.snapshot.sequence,
            self.locations
        )
    }
    pub async fn ensure_ready(&self) -> Result<(), AppError> {
        self.db.ensure_current_owner()?;
        self.snapshot.ensure_ready().map_err(error)
    }
    pub fn check_window(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<(), AppError> {
        if end < start {
            return Err(AppError::BadRequest(
                "Report end must follow its start".into(),
            ));
        }
        if (end - start).num_days() > 732 {
            return Err(AppError::BadRequest(
                "Choose a report range, including comparisons, of at most 732 days".into(),
            ));
        }
        Ok(())
    }
    fn scope_sql(&self, sql: &str) -> String {
        let venues = self
            .locations
            .as_ref()
            .map(|ids| {
                ids.iter()
                    .map(|id| format!("'{id}'"))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .filter(|ids| !ids.is_empty())
            .unwrap_or_else(|| "NULL".into());
        let scope = include_str!("sqlite_scope.sql")
            .replace("__ALL__", if self.locations.is_none() { "1" } else { "0" })
            .replace("__VENUES__", &venues);
        let sources = include_str!("sqlite_sources.sql");
        let sql = sql.trim_start();
        if sql.get(..4).is_some_and(|p| p.eq_ignore_ascii_case("WITH")) {
            format!("WITH {sources}, {scope}, {}", &sql[4..])
        } else {
            format!("WITH {sources}, {scope} {sql}")
        }
    }
    pub fn query<T>(&self, sql: impl Into<String>) -> Query<T> {
        Query {
            reader: self.clone(),
            sql: sql.into(),
            params: Vec::new(),
            result: PhantomData,
        }
    }
}
pub trait Parameter {
    fn value(self) -> SqlValue;
}
impl Parameter for DateTime<Utc> {
    fn value(self) -> SqlValue {
        SqlValue::Text(self.to_rfc3339_opts(chrono::SecondsFormat::Micros, true))
    }
}
impl Parameter for Uuid {
    fn value(self) -> SqlValue {
        SqlValue::Text(self.to_string())
    }
}
impl Parameter for String {
    fn value(self) -> SqlValue {
        SqlValue::Text(self)
    }
}
impl Parameter for &str {
    fn value(self) -> SqlValue {
        SqlValue::Text(self.to_owned())
    }
}
impl Parameter for i64 {
    fn value(self) -> SqlValue {
        SqlValue::Integer(self)
    }
}
pub struct Query<T> {
    reader: ReportReader,
    sql: String,
    params: Vec<SqlValue>,
    result: PhantomData<T>,
}
impl<T: DeserializeOwned + Send + 'static> Query<T> {
    pub fn bind(mut self, value: impl Parameter) -> Self {
        self.params.push(value.value());
        self
    }
    pub async fn fetch_all(self) -> Result<Vec<T>, AppError> {
        self.reader.ensure_ready().await?;
        let rows = self
            .reader
            .snapshot
            .query(self.reader.scope_sql(&self.sql), self.params)
            .await
            .map_err(error)?;
        self.reader.ensure_ready().await?;
        Ok(rows)
    }
    pub async fn fetch_optional(self) -> Result<Option<T>, AppError> {
        Ok(self.fetch_all().await?.into_iter().next())
    }
    pub async fn fetch_one(self) -> Result<T, AppError> {
        self.fetch_optional()
            .await?
            .ok_or_else(|| AppError::Internal("Missing report result".into()))
    }
}
