use crate::error::AppError;
use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use std::{marker::PhantomData, time::Duration};

#[derive(Clone)]
pub struct ClickHouse {
    client: reqwest::Client,
    url: String,
    database: String,
    user: String,
    password: String,
}

pub fn unavailable(error: impl std::fmt::Display) -> AppError {
    tracing::error!(%error, "Analytics operation failed");
    AppError::Api {
        code: "ANALYTICS_UNAVAILABLE".into(),
        status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
        details: None,
    }
}

impl ClickHouse {
    pub fn from_env() -> Self {
        Self::new(
            std::env::var("CLICKHOUSE_URL").unwrap_or_else(|_| "http://127.0.0.1:8123".into()),
            std::env::var("CLICKHOUSE_DATABASE").unwrap_or_else(|_| "arena360".into()),
            std::env::var("CLICKHOUSE_USER").unwrap_or_else(|_| "arena360".into()),
            std::env::var("CLICKHOUSE_PASSWORD").unwrap_or_default(),
        )
    }

    pub fn new(url: String, database: String, user: String, password: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("HTTP client"),
            url,
            database,
            user,
            password,
        }
    }

    pub async fn execute(
        &self,
        sql: &str,
        params: &[(String, String)],
    ) -> Result<String, AppError> {
        let response = self
            .client
            .post(&self.url)
            .basic_auth(&self.user, Some(&self.password))
            .query(&[
                ("database", self.database.as_str()),
                ("wait_end_of_query", "1"),
                ("async_insert", "0"),
                ("output_format_json_quote_64bit_integers", "0"),
                ("date_time_output_format", "iso"),
                ("date_time_input_format", "best_effort"),
                ("join_use_nulls", "1"),
                ("cast_keep_nullable", "1"),
                ("max_execution_time", "25"),
            ])
            .query(params)
            .body(sql.to_owned())
            .send()
            .await
            .map_err(unavailable)?;
        let status = response.status();
        let body = response.text().await.map_err(unavailable)?;
        if !status.is_success() {
            return Err(unavailable(body));
        }
        Ok(body)
    }

    pub async fn ensure_ready(&self) -> Result<(), AppError> {
        let body = self
            .execute(
                "SELECT count() FROM analytics_ready WHERE id=1 FORMAT TabSeparated",
                &[],
            )
            .await?;
        if body.trim() == "0" {
            return Err(unavailable("Initial analytics backfill has not completed"));
        }
        Ok(())
    }

    pub async fn initialize(&self) -> Result<(), AppError> {
        for sql in include_str!("schema.sql")
            .split(';')
            .filter(|s| !s.trim().is_empty())
        {
            self.execute(sql, &[]).await?;
        }
        Ok(())
    }
}

pub struct Query<T> {
    sql: String,
    params: Vec<(&'static str, String)>,
    result: PhantomData<T>,
}

pub trait Parameter {
    fn parameter(self) -> (&'static str, String);
}
impl Parameter for DateTime<Utc> {
    fn parameter(self) -> (&'static str, String) {
        (
            "DateTime64(6, 'UTC')",
            self.format("%Y-%m-%d %H:%M:%S%.6f").to_string(),
        )
    }
}
impl Parameter for uuid::Uuid {
    fn parameter(self) -> (&'static str, String) {
        ("UUID", self.to_string())
    }
}
impl Parameter for String {
    fn parameter(self) -> (&'static str, String) {
        ("String", self)
    }
}
impl Parameter for i64 {
    fn parameter(self) -> (&'static str, String) {
        ("Int64", self.to_string())
    }
}

pub fn query_as<T: DeserializeOwned>(sql: impl Into<String>) -> Query<T> {
    Query {
        sql: sql.into(),
        params: vec![],
        result: PhantomData,
    }
}

impl<T: DeserializeOwned> Query<T> {
    pub fn bind(mut self, value: impl Parameter) -> Self {
        self.params.push(value.parameter());
        self
    }
    pub(crate) fn bind_raw(mut self, value: (&'static str, String)) -> Self {
        self.params.push(value);
        self
    }
    pub async fn fetch_optional(self, client: &ClickHouse) -> Result<Option<T>, AppError> {
        Ok(self.fetch_all(client).await?.into_iter().next())
    }
    pub async fn fetch_all(self, client: &ClickHouse) -> Result<Vec<T>, AppError> {
        client.ensure_ready().await?;
        let mut sql = self.sql;
        let mut params = vec![];
        for (index, (kind, value)) in self.params.into_iter().enumerate().rev() {
            let key = format!("p{}", index + 1);
            sql = sql.replace(&format!("${}", index + 1), &format!("{{{key}:{kind}}}"));
            params.push((format!("param_{key}"), value));
        }
        let body = client
            .execute(&format!("{sql} FORMAT JSONCompactEachRow"), &params)
            .await?;
        body.lines()
            .filter(|l| !l.is_empty())
            .map(|line| serde_json::from_str(line).map_err(unavailable))
            .collect()
    }
    pub async fn fetch_one(self, client: &ClickHouse) -> Result<T, AppError> {
        self.fetch_all(client)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| unavailable("Missing analytics result"))
    }
}
