//! Process-local limits. Each service replica enforces its own allowance.
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy)]
pub struct Decision {
    pub allowed: bool,
    pub remaining: u32,
    pub retry_after_ms: u64,
}

struct Bucket {
    tokens: f64,
    updated: Instant,
    expires: Instant,
}

#[derive(Default)]
pub struct RateLimiter {
    buckets: Mutex<HashMap<String, Bucket>>,
    next_cleanup: Mutex<Option<Instant>>,
}

impl RateLimiter {
    pub fn consume(&self, key: &str, capacity: u32, window: Duration) -> Decision {
        self.consume_at(key, capacity, window, Instant::now(), true)
    }

    /// Authentication attempts use a fixed window, without incremental refill.
    pub fn record_attempt(&self, key: &str, capacity: u32, window: Duration) -> Decision {
        self.consume_at(key, capacity, window, Instant::now(), false)
    }

    fn consume_at(
        &self,
        key: &str,
        capacity: u32,
        window: Duration,
        now: Instant,
        refill: bool,
    ) -> Decision {
        assert!(capacity > 0 && !window.is_zero());
        let mut buckets = self.buckets.lock().expect("rate limiter lock poisoned");
        let mut cleanup = self
            .next_cleanup
            .lock()
            .expect("rate limiter cleanup lock poisoned");
        if cleanup.is_none_or(|due| now >= due) {
            buckets.retain(|_, bucket| bucket.expires > now);
            *cleanup = Some(now + Duration::from_secs(60));
        }
        let bucket = buckets.entry(key.to_owned()).or_insert(Bucket {
            tokens: capacity as f64,
            updated: now,
            expires: now + window,
        });
        if now >= bucket.expires {
            bucket.tokens = capacity as f64;
            bucket.updated = now;
            bucket.expires = now + window;
        }
        let rate = capacity as f64 / window.as_secs_f64();
        if refill {
            bucket.tokens = (bucket.tokens
                + now.duration_since(bucket.updated).as_secs_f64() * rate)
                .min(capacity as f64);
            bucket.updated = now;
            bucket.expires = now + window;
        }
        let allowed = bucket.tokens >= 1.0;
        if allowed {
            bucket.tokens -= 1.0;
        }
        let retry_after_ms = if allowed {
            0
        } else if refill {
            (((1.0 - bucket.tokens) / rate) * 1000.0).ceil() as u64
        } else {
            bucket.expires.duration_since(now).as_millis() as u64
        };
        Decision {
            allowed,
            remaining: bucket.tokens.floor() as u32,
            retry_after_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhaustion_refill_and_client_isolation() {
        let limiter = RateLimiter::default();
        let now = Instant::now();
        let window = Duration::from_secs(60);
        for _ in 0..2 {
            assert!(limiter.consume_at("first", 2, window, now, true).allowed);
        }
        let denied = limiter.consume_at("first", 2, window, now, true);
        assert!(!denied.allowed);
        assert_eq!(denied.retry_after_ms, 30_000);
        assert!(limiter.consume_at("second", 2, window, now, true).allowed);
        assert!(
            limiter
                .consume_at("first", 2, window, now + Duration::from_secs(30), true)
                .allowed
        );
        assert_eq!(
            limiter
                .consume_at("first", 2, window, now + Duration::from_secs(120), true)
                .remaining,
            1
        );
    }

    #[test]
    fn authentication_windows_do_not_refill_or_extend_on_rejection() {
        let limiter = RateLimiter::default();
        let now = Instant::now();
        let window = Duration::from_secs(60);
        for _ in 0..5 {
            assert!(
                limiter
                    .consume_at("otp:user", 5, window, now, false)
                    .allowed
            );
        }
        assert!(
            !limiter
                .consume_at("otp:user", 5, window, now + Duration::from_secs(59), false)
                .allowed
        );
        assert!(
            limiter
                .consume_at("otp:user", 5, window, now + window, false)
                .allowed
        );
    }

    #[test]
    fn concurrent_requests_share_one_atomic_allowance() {
        let limiter = std::sync::Arc::new(RateLimiter::default());
        let now = Instant::now();
        let workers = (0..8)
            .map(|_| {
                let limiter = limiter.clone();
                std::thread::spawn(move || {
                    (0..20)
                        .filter(|_| {
                            limiter
                                .consume_at("shared", 100, Duration::from_secs(60), now, true)
                                .allowed
                        })
                        .count()
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            workers
                .into_iter()
                .map(|w| w.join().unwrap())
                .sum::<usize>(),
            100
        );
    }
}
