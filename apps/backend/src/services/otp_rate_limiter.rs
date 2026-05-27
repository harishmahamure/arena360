use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::error::AppError;

const MAX_ATTEMPTS: usize = 5;
const WINDOW: Duration = Duration::from_secs(15 * 60);

pub struct OtpRateLimiter {
    attempts: Mutex<HashMap<String, Vec<Instant>>>,
}

impl OtpRateLimiter {
    pub fn new() -> Self {
        Self {
            attempts: Mutex::new(HashMap::new()),
        }
    }

    pub fn check_and_record(&self, username: &str) -> Result<(), AppError> {
        let mut map = self
            .attempts
            .lock()
            .map_err(|_| AppError::Internal("Rate limiter lock poisoned".to_string()))?;

        let now = Instant::now();
        let entry = map.entry(username.to_lowercase()).or_default();
        entry.retain(|t| now.duration_since(*t) < WINDOW);

        if entry.len() >= MAX_ATTEMPTS {
            return Err(AppError::TooManyRequests(
                "Too many OTP requests. Please try again in 15 minutes.".to_string(),
            ));
        }

        entry.push(now);
        Ok(())
    }
}

impl Default for OtpRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_five_then_blocks() {
        let limiter = OtpRateLimiter::new();
        for _ in 0..5 {
            assert!(limiter.check_and_record("admin").is_ok());
        }
        assert!(limiter.check_and_record("admin").is_err());
    }
}
