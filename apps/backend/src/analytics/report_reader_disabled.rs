//! Builds without native DuckDB keep the report API but always return unavailable.
use crate::error::AppError;
use std::marker::PhantomData;

pub fn unavailable(reason: &str) -> AppError {
    AppError::Api {
        code: "ANALYTICS_UNAVAILABLE".into(),
        status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
        details: Some(
            serde_json::json!({"message":"report temporarily rebuilding","reason":reason}),
        ),
    }
}
#[derive(Clone)]
pub struct ReportReader;
impl ReportReader {
    pub fn now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }
    pub fn timezone(&self) -> chrono_tz::Tz {
        chrono_tz::UTC
    }
    pub fn cache_key(&self) -> String {
        "native-analytics-disabled".into()
    }
    pub async fn ensure_ready(&self) -> Result<(), AppError> {
        Err(unavailable("native analytics is disabled"))
    }
    pub fn check_window(
        &self,
        _: chrono::DateTime<chrono::Utc>,
        _: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), AppError> {
        Err(unavailable("native analytics is disabled"))
    }
    pub fn query<T>(&self, _: impl Into<String>) -> Query<T> {
        Query(PhantomData)
    }
}
pub struct Query<T>(PhantomData<T>);
impl<T> Query<T> {
    pub fn bind<V>(self, _: V) -> Self {
        self
    }
    pub async fn fetch_all(self) -> Result<Vec<T>, AppError> {
        Err(unavailable("native analytics is disabled"))
    }
    pub async fn fetch_one(self) -> Result<T, AppError> {
        Err(unavailable("native analytics is disabled"))
    }
    pub async fn fetch_optional(self) -> Result<Option<T>, AppError> {
        Err(unavailable("native analytics is disabled"))
    }
}
