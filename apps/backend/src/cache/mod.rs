use std::future::Future;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use deadpool_redis::{Config as RedisPoolConfig, Pool, Runtime};
use redis::AsyncCommands;
use serde::de::DeserializeOwned;
use serde::Serialize;
use tracing::{debug, warn};

use crate::error::AppError;

pub mod keys;

const DEFAULT_REDIS_URL: &str = "redis://127.0.0.1:6379";

#[derive(Debug, Clone, Copy)]
pub struct RateLimitDecision {
    pub allowed: bool,
    pub remaining: u32,
    pub retry_after_ms: u64,
}

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

    async fn publish_invalidation(&self, keys: &[String]) -> Result<(), AppError>;

    /// Atomically consumes one token. `None` means Redis enforcement is not
    /// available and callers must use the configured fail-open behaviour.
    async fn consume_ip_token(
        &self,
        key: &str,
        capacity: u32,
        refill_window: Duration,
    ) -> Result<Option<RateLimitDecision>, AppError>;

    fn is_available(&self) -> bool;
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

    async fn publish_invalidation(&self, _keys: &[String]) -> Result<(), AppError> {
        Ok(())
    }

    async fn consume_ip_token(
        &self,
        _key: &str,
        _capacity: u32,
        _refill_window: Duration,
    ) -> Result<Option<RateLimitDecision>, AppError> {
        Ok(None)
    }

    fn is_available(&self) -> bool {
        false
    }
}

pub struct RedisCache {
    pool: Pool,
}

impl RedisCache {
    pub async fn connect(redis_url: &str) -> Result<Self, AppError> {
        let cfg = RedisPoolConfig::from_url(redis_url);
        let pool = cfg
            .create_pool(Some(Runtime::Tokio1))
            .map_err(|e| AppError::Internal(format!("Redis pool error: {e}")))?;

        let mut conn = pool
            .get()
            .await
            .map_err(|e| AppError::Internal(format!("Redis connect error: {e}")))?;
        let _: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| AppError::Internal(format!("Redis ping error: {e}")))?;

        Ok(Self { pool })
    }
}

#[async_trait]
impl CacheService for RedisCache {
    async fn get_value(&self, key: &str) -> Result<Option<serde_json::Value>, AppError> {
        let mut conn = match self.pool.get().await {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, key, "Redis get connection failed");
                return Ok(None);
            }
        };

        let raw: Option<String> = conn.get(key).await.map_err(|e| {
            warn!(error = %e, key, "Redis GET failed");
            AppError::Internal(format!("Redis GET failed: {e}"))
        })?;

        match raw {
            Some(s) => {
                debug!(key, "cache hit");
                serde_json::from_str(&s)
                    .map(Some)
                    .map_err(|e| AppError::Internal(format!("Cache deserialize error: {e}")))
            }
            None => {
                debug!(key, "cache miss");
                Ok(None)
            }
        }
    }

    async fn set_value(
        &self,
        key: &str,
        value: &serde_json::Value,
        ttl: Duration,
    ) -> Result<(), AppError> {
        let payload = value.to_string();

        let mut conn = match self.pool.get().await {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, key, "Redis set connection failed");
                return Ok(());
            }
        };

        let _: () = conn
            .set_ex(key, payload, ttl.as_secs())
            .await
            .map_err(|e| AppError::Internal(format!("Redis SET failed: {e}")))?;
        Ok(())
    }

    async fn delete(&self, keys: &[&str]) -> Result<(), AppError> {
        if keys.is_empty() {
            return Ok(());
        }

        let mut conn = match self.pool.get().await {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "Redis delete connection failed");
                return Ok(());
            }
        };

        let _: () = conn
            .del(keys)
            .await
            .map_err(|e| AppError::Internal(format!("Redis DEL failed: {e}")))?;
        Ok(())
    }

    async fn invalidate_prefix(&self, prefix: &str) -> Result<(), AppError> {
        let mut conn = match self.pool.get().await {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, prefix, "Redis scan connection failed");
                return Ok(());
            }
        };

        let pattern = format!("{prefix}*");
        let mut cursor: u64 = 0;
        loop {
            let (next, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await
                .map_err(|e| AppError::Internal(format!("Redis SCAN failed: {e}")))?;

            if !keys.is_empty() {
                let key_refs: Vec<&str> = keys.iter().map(String::as_str).collect();
                let _: () = conn
                    .del(&key_refs)
                    .await
                    .map_err(|e| AppError::Internal(format!("Redis DEL failed: {e}")))?;
            }

            cursor = next;
            if cursor == 0 {
                break;
            }
        }
        Ok(())
    }

    async fn publish_invalidation(&self, keys: &[String]) -> Result<(), AppError> {
        if keys.is_empty() {
            return Ok(());
        }

        let payload = serde_json::to_string(keys)
            .map_err(|e| AppError::Internal(format!("Invalidation serialize error: {e}")))?;

        let mut conn = match self.pool.get().await {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "Redis publish connection failed");
                return Ok(());
            }
        };

        let _: () = conn
            .publish(keys::INVALIDATION_CHANNEL, payload)
            .await
            .map_err(|e| AppError::Internal(format!("Redis PUBLISH failed: {e}")))?;
        Ok(())
    }

    async fn consume_ip_token(
        &self,
        key: &str,
        capacity: u32,
        refill_window: Duration,
    ) -> Result<Option<RateLimitDecision>, AppError> {
        // Redis TIME avoids clock skew between backend pods. The hash stores
        // fractional tokens and the last refill time; the script is atomic.
        const TOKEN_BUCKET_LUA: &str = r#"
local now = redis.call('TIME')
local now_ms = (now[1] * 1000) + math.floor(now[2] / 1000)
local capacity = tonumber(ARGV[1])
local window_ms = tonumber(ARGV[2])
local refill_per_ms = capacity / window_ms
local bucket = redis.call('HMGET', KEYS[1], 'tokens', 'updated_ms')
local tokens = tonumber(bucket[1]) or capacity
local updated_ms = tonumber(bucket[2]) or now_ms
tokens = math.min(capacity, tokens + math.max(0, now_ms - updated_ms) * refill_per_ms)
local allowed = 0
local retry_ms = 0
if tokens >= 1 then
  tokens = tokens - 1
  allowed = 1
else
  retry_ms = math.ceil((1 - tokens) / refill_per_ms)
end
redis.call('HSET', KEYS[1], 'tokens', tokens, 'updated_ms', now_ms)
redis.call('PEXPIRE', KEYS[1], window_ms * 2)
return {allowed, math.floor(tokens), retry_ms}
"#;

        let mut conn = self.pool.get().await.map_err(|error| {
            AppError::Internal(format!("Redis rate-limit connection failed: {error}"))
        })?;
        let result: (i64, i64, i64) = redis::Script::new(TOKEN_BUCKET_LUA)
            .key(key)
            .arg(capacity)
            .arg(refill_window.as_millis() as u64)
            .invoke_async(&mut conn)
            .await
            .map_err(|error| {
                AppError::Internal(format!("Redis rate-limit script failed: {error}"))
            })?;

        Ok(Some(RateLimitDecision {
            allowed: result.0 == 1,
            remaining: result.1.max(0) as u32,
            retry_after_ms: result.2.max(0) as u64,
        }))
    }

    fn is_available(&self) -> bool {
        true
    }
}

pub async fn create_cache(redis_url: Option<&str>) -> Arc<dyn CacheService> {
    let url = redis_url
        .filter(|u| !u.is_empty())
        .unwrap_or(DEFAULT_REDIS_URL);

    match RedisCache::connect(url).await {
        Ok(cache) => {
            tracing::info!("Connected to Redis");
            Arc::new(cache)
        }
        Err(e) => {
            warn!(error = %e, "Redis unavailable; using NoopCache");
            Arc::new(NoopCache)
        }
    }
}

pub async fn ping(cache: &dyn CacheService) -> bool {
    cache.is_available()
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

pub fn spawn_invalidation_listener(cache: Arc<dyn CacheService>, redis_url: Option<String>) {
    if !cache.is_available() {
        return;
    }

    let url = redis_url
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| DEFAULT_REDIS_URL.to_string());

    tokio::spawn(async move {
        loop {
            if let Err(e) = run_invalidation_listener(&url, cache.clone()).await {
                warn!(error = %e, "Cache invalidation listener error; retrying in 5s");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    });
}

async fn run_invalidation_listener(
    redis_url: &str,
    cache: Arc<dyn CacheService>,
) -> Result<(), AppError> {
    let client = redis::Client::open(redis_url)
        .map_err(|e| AppError::Internal(format!("Redis client error: {e}")))?;
    let mut pubsub = client
        .get_async_pubsub()
        .await
        .map_err(|e| AppError::Internal(format!("Redis pubsub error: {e}")))?;

    pubsub
        .subscribe(keys::INVALIDATION_CHANNEL)
        .await
        .map_err(|e| AppError::Internal(format!("Redis subscribe error: {e}")))?;

    let mut stream = pubsub.into_on_message();
    use futures::StreamExt;
    while let Some(msg) = stream.next().await {
        let payload: String = msg
            .get_payload()
            .map_err(|e| AppError::Internal(format!("Invalidation payload error: {e}")))?;
        if let Ok(keys) = serde_json::from_str::<Vec<String>>(&payload) {
            let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
            let _ = cache.delete(&refs).await;
        }
    }

    Ok(())
}

/// Invalidate keys locally and notify other pods.
pub async fn invalidate(cache: &dyn CacheService, keys: &[String]) -> Result<(), AppError> {
    if keys.is_empty() {
        return Ok(());
    }
    let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
    cache.delete(&refs).await?;
    cache.publish_invalidation(keys).await?;
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

        async fn publish_invalidation(&self, _keys: &[String]) -> Result<(), AppError> {
            Ok(())
        }

        async fn consume_ip_token(
            &self,
            _key: &str,
            _capacity: u32,
            _refill_window: Duration,
        ) -> Result<Option<RateLimitDecision>, AppError> {
            Ok(None)
        }

        fn is_available(&self) -> bool {
            true
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
