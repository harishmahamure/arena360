use std::sync::Arc;
use uuid::Uuid;

use crate::cache::{self, keys, CacheService};
use crate::error::AppError;

const MAX_ATTEMPTS: usize = 5;

pub struct KioskRegistrationRateLimiter {
    cache: Arc<dyn CacheService>,
}

impl KioskRegistrationRateLimiter {
    pub fn new(cache: Arc<dyn CacheService>) -> Self {
        Self { cache }
    }

    pub async fn check_and_record(&self, device_id: Uuid) -> Result<(), AppError> {
        let key = keys::kiosk_register_rate_limit(&device_id);

        if !self.cache.is_available() {
            return Ok(());
        }

        let current: Option<i64> = cache::get_json(&*self.cache, &key).await?;
        let attempts = current.unwrap_or(0) + 1;
        if attempts as usize > MAX_ATTEMPTS {
            return Err(AppError::too_many_requests_code("REGISTRATION_RATE_LIMITED"));
        }

        cache::set_json(
            &*self.cache,
            &key,
            &attempts,
            keys::ttl::KIOSK_REGISTER_RATE_LIMIT,
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
        let limiter = KioskRegistrationRateLimiter::new(Arc::new(cache::NoopCache));
        assert!(limiter
            .check_and_record(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap())
            .await
            .is_ok());
    }
}
