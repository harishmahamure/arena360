use std::sync::Arc;

use crate::cache::keys;
use crate::error::AppError;
use crate::services::rate_limiter::RateLimiter;

const MAX_ATTEMPTS: u32 = 5;

pub struct OtpRateLimiter {
    limits: Arc<RateLimiter>,
}

impl OtpRateLimiter {
    pub fn new(limits: Arc<RateLimiter>) -> Self {
        Self { limits }
    }

    pub async fn check_and_record(&self, username: &str) -> Result<(), AppError> {
        let key = keys::otp_rate_limit(username);
        if !self
            .limits
            .record_attempt(&key, MAX_ATTEMPTS, keys::ttl::OTP_RATE_LIMIT)
            .allowed
        {
            return Err(AppError::TooManyRequests(
                "Too many OTP requests. Please try again later.".to_string(),
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
        let limiter = OtpRateLimiter::new(Arc::new(RateLimiter::default()));
        assert!(limiter.check_and_record("admin").await.is_ok());
    }
}
