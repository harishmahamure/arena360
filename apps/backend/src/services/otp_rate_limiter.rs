use std::sync::Arc;

use crate::cache::{self, keys, CacheService};
use crate::error::AppError;

const MAX_ATTEMPTS: usize = 5;

pub struct OtpRateLimiter {
    cache: Arc<dyn CacheService>,
}

impl OtpRateLimiter {
    pub fn new(cache: Arc<dyn CacheService>) -> Self {
        Self { cache }
    }

    pub async fn check_and_record(&self, username: &str) -> Result<(), AppError> {
        let key = keys::otp_rate_limit(username);

        if !self.cache.is_available() {
            return Ok(());
        }

        let current: Option<i64> = cache::get_json(&*self.cache, &key).await?;
        let attempts = current.unwrap_or(0) + 1;
        if attempts as usize > MAX_ATTEMPTS {
            return Err(AppError::TooManyRequests(
                "Too many OTP requests. Please try again in 15 minutes.".to_string(),
            ));
        }

        cache::set_json(
            &*self.cache,
            &key,
            &attempts,
            keys::ttl::OTP_RATE_LIMIT,
        )
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn noop_cache_allows_requests() {
        let limiter = OtpRateLimiter::new(Arc::new(cache::NoopCache));
        assert!(limiter.check_and_record("admin").await.is_ok());
    }
}
