use std::future::Future;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Serialize;
use tracing::warn;

use crate::error::AppError;

pub mod keys;

#[async_trait]
pub trait CacheService: Send + Sync {
    async fn get_value(&self, key: &str) -> Result<Option<serde_json::Value>, AppError>;

    async fn set_value(
        &self,
        key: &str,
        value: &serde_json::Value,
        ttl: Duration,
    ) -> Result<(), AppError>;

    async fn delete(&self, keys: &[&str]) -> Result<(), AppError>;

    async fn invalidate_prefix(&self, prefix: &str) -> Result<(), AppError>;
}

pub struct NoopCache;

#[async_trait]
impl CacheService for NoopCache {
    async fn get_value(&self, _key: &str) -> Result<Option<serde_json::Value>, AppError> {
        Ok(None)
    }

    async fn set_value(
        &self,
        _key: &str,
        _value: &serde_json::Value,
        _ttl: Duration,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn delete(&self, _keys: &[&str]) -> Result<(), AppError> {
        Ok(())
    }

    async fn invalidate_prefix(&self, _prefix: &str) -> Result<(), AppError> {
        Ok(())
    }
}

/// Operational values are always read from the authoritative database.
pub async fn create_cache() -> Arc<dyn CacheService> {
    Arc::new(NoopCache)
}

pub async fn get_json<T: DeserializeOwned>(
    cache: &dyn CacheService,
    key: &str,
) -> Result<Option<T>, AppError> {
    match cache.get_value(key).await? {
        Some(value) => serde_json::from_value(value)
            .map(Some)
            .map_err(|e| AppError::Internal(format!("Cache deserialize error: {e}"))),
        None => Ok(None),
    }
}

pub async fn set_json<T: Serialize>(
    cache: &dyn CacheService,
    key: &str,
    value: &T,
    ttl: Duration,
) -> Result<(), AppError> {
    let json = serde_json::to_value(value)
        .map_err(|e| AppError::Internal(format!("Cache serialize error: {e}")))?;
    cache.set_value(key, &json, ttl).await
}

/// Cache-aside helper: read from cache, or compute and store.
pub async fn get_or_set<T, F, Fut>(
    cache: &dyn CacheService,
    key: &str,
    ttl: Duration,
    fetch: F,
) -> Result<T, AppError>
where
    T: Serialize + DeserializeOwned + Send,
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T, AppError>>,
{
    match get_json::<T>(cache, key).await {
        Ok(Some(cached)) => return Ok(cached),
        Ok(None) => {}
        Err(error) => warn!(%error, key, "Cache read failed; falling back to source of truth"),
    }

    static FLIGHTS: OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    > = OnceLock::new();
    let flight = {
        let flights = FLIGHTS.get_or_init(Default::default);
        let mut flights = flights.lock().expect("single-flight lock poisoned");
        flights
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    };
    let guard = flight.lock().await;
    match get_json::<T>(cache, key).await {
        Ok(Some(cached)) => {
            drop(guard);
            remove_flight(FLIGHTS.get(), key, &flight);
            return Ok(cached);
        }
        Ok(None) => {}
        Err(error) => warn!(%error, key, "Cache recheck failed; falling back to source of truth"),
    }

    let value = match fetch().await {
        Ok(value) => value,
        Err(error) => {
            drop(guard);
            remove_flight(FLIGHTS.get(), key, &flight);
            return Err(error);
        }
    };
    let base_ms = ttl.as_millis() as u64;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64;
    let jitter_percent = 90 + (nanos % 21);
    let jittered_ttl = Duration::from_millis((base_ms * jitter_percent / 100).max(1));
    let cache_result = set_json(cache, key, &value, jittered_ttl).await;
    drop(guard);
    remove_flight(FLIGHTS.get(), key, &flight);
    if let Err(error) = cache_result {
        warn!(%error, key, "Cache write failed; returning source-of-truth value");
    }
    Ok(value)
}

type FlightMap = std::sync::Mutex<std::collections::HashMap<String, Arc<tokio::sync::Mutex<()>>>>;

fn remove_flight(flights: Option<&FlightMap>, key: &str, flight: &Arc<tokio::sync::Mutex<()>>) {
    let Some(flights) = flights else { return };
    let mut flights = flights.lock().expect("single-flight lock poisoned");
    if flights
        .get(key)
        .is_some_and(|current| Arc::ptr_eq(current, flight))
    {
        flights.remove(key);
    }
}

/// Invalidate cached keys.
pub async fn invalidate(cache: &dyn CacheService, keys: &[String]) -> Result<(), AppError> {
    if keys.is_empty() {
        return Ok(());
    }
    let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
    cache.delete(&refs).await?;
    Ok(())
}

/// Bust all dashboard stats snapshots (keyed under `stats:`).
pub async fn invalidate_stats(cache: &dyn CacheService) -> Result<(), AppError> {
    cache
        .invalidate_prefix(crate::cache::keys::STATS_PREFIX)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FailingCache;

    #[async_trait]
    impl CacheService for FailingCache {
        async fn get_value(&self, _key: &str) -> Result<Option<serde_json::Value>, AppError> {
            Err(AppError::Internal("cache unavailable".to_string()))
        }

        async fn set_value(
            &self,
            _key: &str,
            _value: &serde_json::Value,
            _ttl: Duration,
        ) -> Result<(), AppError> {
            Err(AppError::Internal("cache unavailable".to_string()))
        }

        async fn delete(&self, _keys: &[&str]) -> Result<(), AppError> {
            Ok(())
        }

        async fn invalidate_prefix(&self, _prefix: &str) -> Result<(), AppError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn cache_aside_returns_database_value_when_cache_fails() {
        let value = get_or_set(
            &FailingCache,
            "test:postgres-authoritative",
            Duration::from_secs(30),
            || async { Ok::<_, AppError>(serde_json::json!({"source": "postgres"})) },
        )
        .await
        .expect("source-of-truth value should be returned");

        assert_eq!(value, serde_json::json!({"source": "postgres"}));
    }
}
