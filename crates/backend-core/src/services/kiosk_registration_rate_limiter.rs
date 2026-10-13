use std::sync::Arc;
use uuid::Uuid;

use crate::cache::keys;
use crate::error::AppError;
use crate::services::rate_limiter::RateLimiter;

const MAX_ATTEMPTS: u32 = 5;

pub struct KioskRegistrationRateLimiter {
    limits: Arc<RateLimiter>,
}

impl KioskRegistrationRateLimiter {
    pub fn new(limits: Arc<RateLimiter>) -> Self {
        Self { limits }
    }

    pub async fn check_and_record(&self, device_id: Uuid) -> Result<(), AppError> {
        let key = keys::kiosk_register_rate_limit(&device_id);
        if !self
            .limits
            .record_attempt(&key, MAX_ATTEMPTS, keys::ttl::KIOSK_REGISTER_RATE_LIMIT)
            .allowed
        {
            return Err(AppError::too_many_requests_code(
                "REGISTRATION_RATE_LIMITED",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_limit_allows_initial_request() {
        let limiter = KioskRegistrationRateLimiter::new(Arc::new(RateLimiter::default()));
        assert!(limiter
            .check_and_record(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap())
            .await
            .is_ok());
    }
}
