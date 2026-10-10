use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::error::AppError;

static JITTER_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy)]
pub struct SqliteRetryConfig {
    pub foreground_attempts: u32,
    pub foreground_delay: Duration,
    pub foreground_jitter: Duration,
    pub background_initial_delay: Duration,
    pub background_max_delay: Duration,
    pub background_jitter: Duration,
}

impl SqliteRetryConfig {
    pub fn validate(self) -> Result<Self, AppError> {
        if self.foreground_attempts == 0
            || self.foreground_delay.is_zero()
            || self.background_initial_delay.is_zero()
            || self.background_max_delay < self.background_initial_delay
        {
            return Err(AppError::Internal(
                "invalid SQLite retry configuration".into(),
            ));
        }
        Ok(self)
    }
}

impl Default for SqliteRetryConfig {
    fn default() -> Self {
        Self {
            foreground_attempts: 3,
            foreground_delay: Duration::from_millis(10),
            foreground_jitter: Duration::from_millis(15),
            background_initial_delay: Duration::from_millis(250),
            background_max_delay: Duration::from_secs(30),
            background_jitter: Duration::from_millis(100),
        }
    }
}

#[derive(Debug, Default)]
pub struct SqliteBusyMetrics {
    count: AtomicU64,
    wait_micros: AtomicU64,
}

impl SqliteBusyMetrics {
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    pub fn wait(&self) -> Duration {
        Duration::from_micros(self.wait_micros.load(Ordering::Relaxed))
    }

    fn record(&self, delay: Duration) {
        self.count.fetch_add(1, Ordering::Relaxed);
        self.wait_micros.fetch_add(
            u64::try_from(delay.as_micros()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
    }
}

pub async fn retry_foreground<T, F, Fut>(
    config: SqliteRetryConfig,
    metrics: &SqliteBusyMetrics,
    mut operation: F,
) -> Result<T, AppError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, sqlx::Error>>,
{
    let config = config.validate()?;
    let mut attempt = 0;
    loop {
        match operation().await {
            Err(error) if is_sqlite_busy(&error) && attempt + 1 < config.foreground_attempts => {
                attempt += 1;
                let delay = config
                    .foreground_delay
                    .saturating_add(jitter(config.foreground_jitter));
                metrics.record(delay);
                tokio::time::sleep(delay).await;
            }
            result => return result.map_err(AppError::Database),
        }
    }
}

pub struct BackgroundBackoff {
    config: SqliteRetryConfig,
    failures: u32,
    metrics: Arc<SqliteBusyMetrics>,
}

impl BackgroundBackoff {
    pub fn new(
        config: SqliteRetryConfig,
        metrics: Arc<SqliteBusyMetrics>,
    ) -> Result<Self, AppError> {
        Ok(Self {
            config: config.validate()?,
            failures: 0,
            metrics,
        })
    }

    pub fn reset(&mut self) {
        self.failures = 0;
    }

    pub fn next_delay(&mut self) -> Duration {
        let exponent = self.failures.min(31);
        self.failures = self.failures.saturating_add(1);
        let multiplier = 1_u32.checked_shl(exponent).unwrap_or(u32::MAX);
        let base = self
            .config
            .background_initial_delay
            .saturating_mul(multiplier)
            .min(self.config.background_max_delay);
        let delay = base
            .saturating_add(jitter(self.config.background_jitter))
            .min(self.config.background_max_delay);
        self.metrics.record(delay);
        delay
    }
}

pub fn is_sqlite_busy(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(|database| database.code())
        .is_some_and(|code| {
            matches!(code.as_ref(), "SQLITE_BUSY" | "SQLITE_LOCKED")
                || code
                    .parse::<i32>()
                    .is_ok_and(|code| matches!(code & 0xff, 5 | 6))
        })
}

fn jitter(max: Duration) -> Duration {
    if max.is_zero() {
        return Duration::ZERO;
    }
    let sequence = JITTER_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let mixed = sequence
        .wrapping_mul(6_364_136_223_846_793_005)
        .rotate_left(17);
    let max_micros = u64::try_from(max.as_micros()).unwrap_or(u64::MAX);
    Duration::from_micros(mixed % max_micros.saturating_add(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_backoff_is_bounded_and_resettable() {
        let metrics = Arc::new(SqliteBusyMetrics::default());
        let config = SqliteRetryConfig {
            background_initial_delay: Duration::from_millis(10),
            background_max_delay: Duration::from_millis(40),
            background_jitter: Duration::ZERO,
            ..SqliteRetryConfig::default()
        };
        let mut backoff = BackgroundBackoff::new(config, metrics.clone()).unwrap();
        assert_eq!(backoff.next_delay(), Duration::from_millis(10));
        assert_eq!(backoff.next_delay(), Duration::from_millis(20));
        assert_eq!(backoff.next_delay(), Duration::from_millis(40));
        assert_eq!(backoff.next_delay(), Duration::from_millis(40));
        backoff.reset();
        assert_eq!(backoff.next_delay(), Duration::from_millis(10));
        assert_eq!(metrics.count(), 5);
    }
}
