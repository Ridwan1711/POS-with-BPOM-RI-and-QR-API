//! Fixed-window rate limiter kept in process memory.
//!
//! Used for login attempts (per account email) and for external provider lookups
//! (per provider and per user). For multi-instance deployments, move this to Redis.

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

const MAX_TRACKED_KEYS: usize = 10_000;

struct Window {
    started: Instant,
    span: Duration,
    count: u32,
}

#[derive(Default)]
pub struct RateLimiter {
    windows: Mutex<HashMap<String, Window>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one event for `key`. Returns `Err(retry_after)` when `limit` is already reached.
    pub fn check(&self, key: &str, limit: u32, span: Duration) -> Result<(), Duration> {
        let now = Instant::now();
        let mut windows = self.windows.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

        if windows.len() > MAX_TRACKED_KEYS {
            windows.retain(|_, w| now.duration_since(w.started) < w.span);
        }

        let window = windows.entry(key.to_string()).or_insert(Window {
            started: now,
            span,
            count: 0,
        });
        if now.duration_since(window.started) >= window.span {
            window.started = now;
            window.span = span;
            window.count = 0;
        }

        if window.count >= limit {
            let elapsed = now.duration_since(window.started);
            return Err(window.span.saturating_sub(elapsed));
        }
        window.count += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::RateLimiter;
    use std::time::Duration;

    #[test]
    fn allows_up_to_limit_then_blocks() {
        let limiter = RateLimiter::new();
        let span = Duration::from_secs(60);
        assert!(limiter.check("k", 2, span).is_ok());
        assert!(limiter.check("k", 2, span).is_ok());
        assert!(limiter.check("k", 2, span).is_err());
    }

    #[test]
    fn keys_are_independent() {
        let limiter = RateLimiter::new();
        let span = Duration::from_secs(60);
        assert!(limiter.check("a", 1, span).is_ok());
        assert!(limiter.check("b", 1, span).is_ok());
        assert!(limiter.check("a", 1, span).is_err());
    }
}
