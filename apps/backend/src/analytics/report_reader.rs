//! Fenced, parameterized report reads from the ingestion connection.
use super::tenant_db::{error, TenantAnalytics, SCHEMA_VERSION};
use crate::error::AppError;
use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use duckdb::{params_from_iter, types::Value as SqlValue};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{marker::PhantomData, sync::Arc};
use uuid::Uuid;

pub fn unavailable(reason: &str) -> AppError {
    AppError::Api {
        code: "ANALYTICS_UNAVAILABLE".into(),
        status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
        details: Some(
            serde_json::json!({"message":"report temporarily rebuilding", "reason":reason}),
        ),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct State {
    timezone: String,
    hot: NaiveDate,
    sequence: u64,
    updated_at: String,
}

fn state(tx: &duckdb::Transaction<'_>) -> Result<State, AppError> {
    let (status, version, timezone, hot, sequence, updated_at): (String, i64, String, String, u64, String) = tx
        .query_row("SELECT status,schema_version,timezone,CAST(hot_window_start AS VARCHAR),last_sequence,CAST(updated_at AS VARCHAR) FROM _ingest_state", [],
            |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?)))
        .map_err(error)?;
    if status != "READY" || version != SCHEMA_VERSION {
        return Err(unavailable("tenant analytics is not ready"));
    }
    timezone
        .parse::<Tz>()
        .map_err(|_| unavailable("invalid tenant calendar"))?;
    Ok(State {
        timezone,
        hot: hot.parse().map_err(|_| unavailable("invalid hot window"))?,
        sequence,
        updated_at,
    })
}

#[derive(Clone)]
pub struct ReportReader {
    analytics: Arc<TenantAnalytics>,
    state: State,
    locations: Option<Vec<Uuid>>,
    now: DateTime<Utc>,
}

impl ReportReader {
    pub async fn new(analytics: Arc<TenantAnalytics>) -> Result<Self, AppError> {
        // This also marks changed calendars REBUILDING before any cached result can be read.
        if !analytics.ensure_timezone().await? {
            return Err(unavailable("tenant calendar changed"));
        }
        let state = analytics.read(state).await?;
        let reader = Self {
            analytics,
            state,
            locations: None,
            now: Utc::now(),
        };
        reader.check_source().await?;
        Ok(reader)
    }

    pub fn timezone(&self) -> Tz {
        self.state
            .timezone
            .parse()
            .expect("validated tenant calendar")
    }
    pub fn now(&self) -> DateTime<Utc> {
        self.now
    }
    /// A fixed observation instant makes current-wallet and open-session parity reproducible.
    pub fn at(mut self, now: DateTime<Utc>) -> Self {
        self.now = now;
        self
    }

    pub fn hot_window_start(&self) -> NaiveDate {
        self.state.hot
    }

    /// Set only after resolving current local permission and venue grants.
    pub fn scoped(mut self, locations: Option<Vec<Uuid>>) -> Self {
        self.locations = locations.map(|mut ids| {
            ids.sort();
            ids.dedup();
            ids
        });
        self
    }

    fn scope_sql(&self, sql: &str) -> Result<String, AppError> {
        let venues = self
            .locations
            .as_ref()
            .map(|ids| {
                ids.iter()
                    .map(|id| format!("UUID '{id}'"))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .filter(|ids| !ids.is_empty())
            .unwrap_or_else(|| "NULL::UUID".into());
        let cut = super::rebuild::boundary(self.state.hot, self.timezone())?;
        let definitions = include_str!("report_scope.sql")
            .replace(
                "__ALL__",
                if self.locations.is_none() {
                    "TRUE"
                } else {
                    "FALSE"
                },
            )
            .replace("__VENUES__", &venues)
            .replace("__HOT_DATE__", &self.state.hot.to_string())
            .replace(
                "__HOT_UTC__",
                &cut.format("%Y-%m-%d %H:%M:%S%.6f").to_string(),
            );
        let sql = sql.replace("__TIMEZONE__", &self.state.timezone);
        let sql = sql.trim_start();
        if sql
            .get(..4)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("WITH"))
        {
            Ok(format!("WITH {definitions}, {}", &sql[4..]))
        } else {
            Ok(format!("WITH {definitions} {sql}"))
        }
    }

    /// Cache identity changes after replay, rebuild, retention or ownership replacement.
    pub fn cache_key(&self) -> String {
        format!(
            "duckdb:{}:{}:{}:{}:{}:{}:{}:venues:{:?}",
            self.analytics.tenant_id(),
            self.analytics.owner().ownership_generation(),
            SCHEMA_VERSION,
            self.state.timezone,
            self.state.hot,
            self.state.sequence,
            self.state.updated_at,
            self.locations
        )
    }

    pub fn check_window(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<(), AppError> {
        if end < start {
            return Err(AppError::BadRequest(
                "Report end must follow its start".into(),
            ));
        }
        if start.with_timezone(&self.timezone()).date_naive() < self.state.hot {
            return Err(AppError::BadRequest(
                "Report range is outside the analytics hot window".into(),
            ));
        }
        Ok(())
    }

    async fn check_source(&self) -> Result<u64, AppError> {
        self.analytics.owner().ensure_current_owner()?;
        let (timezone, watermark): (String, i64) = sqlx::query_as(
            "SELECT timezone,COALESCE((SELECT seq FROM sqlite_sequence WHERE name='outbox_events'),0) FROM tenant_runtime WHERE singleton=1")
            .fetch_one(&self.analytics.owner().read_pool()?).await?;
        if timezone != self.state.timezone
            || watermark < 0
            || (watermark as u64) < self.state.sequence
        {
            return Err(unavailable("source calendar or checkpoint changed"));
        }
        Ok(watermark as u64)
    }

    /// Revalidate immediately before consulting a cache, including the live DuckDB state.
    pub async fn ensure_ready(&self) -> Result<(), AppError> {
        self.check_source().await?;
        let expected = self.state.clone();
        let sequence = self
            .analytics
            .read(move |tx| check_state(tx, &expected))
            .await?;
        self.check_observed(sequence).await
    }

    // Read SQLite again after releasing the native connection. Ingestion may
    // advance while we wait for that connection; validate the snapshot actually
    // observed rather than rejecting valid facts against an earlier watermark.
    async fn check_observed(&self, sequence: u64) -> Result<(), AppError> {
        if sequence > self.check_source().await? {
            return Err(unavailable(
                "analytics checkpoint is ahead of restored source",
            ));
        }
        Ok(())
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

fn check_state(tx: &duckdb::Transaction<'_>, expected: &State) -> Result<u64, AppError> {
    let current = state(tx)?;
    if current.timezone != expected.timezone || current.hot != expected.hot {
        return Err(unavailable("report calendar or hot window changed"));
    }
    Ok(current.sequence)
}

pub trait Parameter {
    fn value(self) -> SqlValue;
}
impl Parameter for DateTime<Utc> {
    fn value(self) -> SqlValue {
        SqlValue::Timestamp(
            duckdb::types::TimeUnit::Microsecond,
            self.timestamp_micros(),
        )
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
        SqlValue::BigInt(self)
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
        self.reader.check_source().await?;
        for value in &self.params {
            if let SqlValue::Timestamp(unit, value) = value {
                let instant = DateTime::from_timestamp_micros(unit.to_micros(*value))
                    .ok_or_else(|| AppError::BadRequest("Invalid report timestamp".into()))?;
                self.reader.check_window(instant, instant)?;
            }
        }
        let sql = self.reader.scope_sql(&self.sql)?;
        let expected = self.reader.state.clone();
        let (results, sequence) =
            self.reader
                .analytics
                .read(move |tx| {
                    let sequence = check_state(tx, &expected)?;
                    let mut statement = tx.prepare(&sql).map_err(error)?;
                    let mut rows = statement
                        .query(params_from_iter(self.params))
                        .map_err(error)?;
                    let mut results = Vec::new();
                    while let Some(row) = rows.next().map_err(error)? {
                        let values = (0..row.as_ref().column_count())
                            .map(|column| {
                                let value = row.get::<_, SqlValue>(column).map_err(error)?;
                                json(value)
                            })
                            .collect::<Result<Vec<_>, AppError>>()?;
                        results.push(serde_json::from_value(Value::Array(values)).map_err(
                            |e| AppError::Internal(format!("Invalid report result: {e}")),
                        )?);
                    }
                    Ok((results, sequence))
                })
                .await?;
        self.reader.check_observed(sequence).await?;
        Ok(results)
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

fn json(value: SqlValue) -> Result<Value, AppError> {
    let invalid = || AppError::Internal("Unsupported report column value".into());
    Ok(match value {
        SqlValue::Null => Value::Null,
        SqlValue::Boolean(v) => Value::Bool(v),
        SqlValue::TinyInt(v) => v.into(),
        SqlValue::SmallInt(v) => v.into(),
        SqlValue::Int(v) => v.into(),
        SqlValue::BigInt(v) => v.into(),
        SqlValue::UTinyInt(v) => v.into(),
        SqlValue::USmallInt(v) => v.into(),
        SqlValue::UInt(v) => v.into(),
        SqlValue::UBigInt(v) => v.into(),
        SqlValue::HugeInt(v) => serde_json::from_str(&v.to_string()).map_err(|_| invalid())?,
        SqlValue::UHugeInt(v) => serde_json::from_str(&v.to_string()).map_err(|_| invalid())?,
        // Preserve the exact decimal representation until the public DTO is decoded.
        SqlValue::Decimal(v) => serde_json::from_str(&v.to_string()).map_err(|_| invalid())?,
        SqlValue::Float(v) if v.is_finite() => serde_json::json!(v),
        SqlValue::Double(v) if v.is_finite() => serde_json::json!(v),
        SqlValue::Text(v) | SqlValue::Enum(v) => Value::String(v),
        SqlValue::Timestamp(unit, v) => Value::String(crate::time::utc_timestamp(
            &DateTime::from_timestamp_micros(unit.to_micros(v)).ok_or_else(invalid)?,
        )),
        SqlValue::Date32(v) => Value::String(
            NaiveDate::from_ymd_opt(1970, 1, 1)
                .unwrap()
                .checked_add_signed(chrono::Duration::days(i64::from(v)))
                .ok_or_else(invalid)?
                .to_string(),
        ),
        SqlValue::List(values) | SqlValue::Array(values) => {
            Value::Array(values.into_iter().map(json).collect::<Result<_, _>>()?)
        }
        _ => return Err(invalid()),
    })
}
