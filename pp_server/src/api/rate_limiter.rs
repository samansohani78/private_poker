//! Rate limiter for WebSocket message handling.
//!
//! Prevents DoS attacks by limiting the number of messages a client can send
//! within specific time windows.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Rate limiter using a sliding window algorithm
#[derive(Debug)]
pub struct RateLimiter {
    /// Timestamps of recent requests
    timestamps: VecDeque<Instant>,
    /// Maximum number of requests allowed in the window
    max_requests: usize,
    /// Time window for rate limiting
    window: Duration,
}

impl RateLimiter {
    /// Create a new rate limiter
    ///
    /// # Arguments
    ///
    /// * `max_requests` - Maximum number of requests allowed in the time window
    /// * `window` - Time window duration
    ///
    /// # Example
    ///
    /// ```
    /// use pp_server::api::rate_limiter::RateLimiter;
    /// use std::time::Duration;
    ///
    /// // Allow 10 requests per second
    /// let limiter = RateLimiter::new(10, Duration::from_secs(1));
    /// ```
    pub fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            timestamps: VecDeque::with_capacity(max_requests),
            max_requests,
            window,
        }
    }

    /// Create a rate limiter for burst protection (10 messages per second)
    pub fn burst() -> Self {
        Self::new(10, Duration::from_secs(1))
    }

    /// Create a rate limiter for sustained usage (100 messages per minute)
    pub fn sustained() -> Self {
        Self::new(100, Duration::from_secs(60))
    }

    /// Check if a request should be allowed
    ///
    /// Returns `true` if the request is allowed, `false` if rate limit exceeded.
    ///
    /// # Example
    ///
    /// ```
    /// # use pp_server::api::rate_limiter::RateLimiter;
    /// # use std::time::Duration;
    /// let mut limiter = RateLimiter::new(5, Duration::from_secs(1));
    ///
    /// // First 5 requests allowed
    /// for _ in 0..5 {
    ///     assert!(limiter.check());
    /// }
    ///
    /// // 6th request blocked
    /// assert!(!limiter.check());
    /// ```
    pub fn check(&mut self) -> bool {
        let now = Instant::now();

        // Remove timestamps outside the window
        while let Some(ts) = self.timestamps.front() {
            if now.duration_since(*ts) > self.window {
                self.timestamps.pop_front();
            } else {
                break;
            }
        }

        // Check if limit exceeded
        if self.timestamps.len() >= self.max_requests {
            return false;
        }

        // Record this request
        self.timestamps.push_back(now);
        true
    }

    /// Get the number of requests in the current window
    #[allow(dead_code)]
    pub fn current_count(&self) -> usize {
        self.timestamps.len()
    }

    /// Get the number of remaining requests allowed in the current window
    #[allow(dead_code)]
    pub fn remaining(&self) -> usize {
        self.max_requests.saturating_sub(self.timestamps.len())
    }

    /// Get the time until the window resets (when the oldest request expires)
    ///
    /// Returns `None` if there are no requests in the current window.
    #[allow(dead_code)]
    pub fn reset_in(&self) -> Option<Duration> {
        self.timestamps.front().map(|oldest| {
            let elapsed = Instant::now().duration_since(*oldest);
            self.window.saturating_sub(elapsed)
        })
    }

    /// Reset the rate limiter (clear all timestamps)
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.timestamps.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_rate_limiter_allows_within_limit() {
        let mut limiter = RateLimiter::new(5, Duration::from_secs(1));

        for _ in 0..5 {
            assert!(limiter.check(), "Should allow requests within limit");
        }
    }

    #[test]
    fn test_rate_limiter_blocks_over_limit() {
        let mut limiter = RateLimiter::new(3, Duration::from_secs(1));

        // First 3 allowed
        for _ in 0..3 {
            assert!(limiter.check());
        }

        // 4th blocked
        assert!(!limiter.check(), "Should block request over limit");
    }

    #[test]
    fn test_rate_limiter_window_expiry() {
        let mut limiter = RateLimiter::new(2, Duration::from_millis(100));

        // Use up limit
        assert!(limiter.check());
        assert!(limiter.check());
        assert!(!limiter.check());

        // Wait for window to expire
        thread::sleep(Duration::from_millis(150));

        // Should allow again
        assert!(limiter.check(), "Should allow after window expires");
    }

    #[test]
    fn test_rate_limiter_current_count() {
        let mut limiter = RateLimiter::new(10, Duration::from_secs(1));

        assert_eq!(limiter.current_count(), 0);

        limiter.check();
        assert_eq!(limiter.current_count(), 1);

        limiter.check();
        limiter.check();
        assert_eq!(limiter.current_count(), 3);
    }

    #[test]
    fn test_rate_limiter_reset() {
        let mut limiter = RateLimiter::new(2, Duration::from_secs(1));

        limiter.check();
        limiter.check();
        assert!(!limiter.check());

        limiter.reset();
        assert!(limiter.check(), "Should allow after reset");
    }

    #[test]
    fn test_burst_limiter() {
        let mut limiter = RateLimiter::burst();

        for _ in 0..10 {
            assert!(limiter.check());
        }

        assert!(!limiter.check(), "Burst limiter should block 11th request");
    }

    #[test]
    fn test_sustained_limiter() {
        let mut limiter = RateLimiter::sustained();

        for _ in 0..100 {
            assert!(limiter.check());
        }

        assert!(
            !limiter.check(),
            "Sustained limiter should block 101st request"
        );
    }

    #[test]
    fn test_remaining_count() {
        let mut limiter = RateLimiter::new(5, Duration::from_secs(1));

        assert_eq!(limiter.remaining(), 5, "Should have 5 remaining initially");

        limiter.check();
        assert_eq!(limiter.remaining(), 4, "Should have 4 remaining after 1 request");

        limiter.check();
        limiter.check();
        assert_eq!(limiter.remaining(), 2, "Should have 2 remaining after 3 requests");

        limiter.check();
        limiter.check();
        assert_eq!(limiter.remaining(), 0, "Should have 0 remaining after 5 requests");
    }

    #[test]
    fn test_reset_in() {
        let mut limiter = RateLimiter::new(5, Duration::from_secs(1));

        // No requests yet
        assert!(limiter.reset_in().is_none(), "Should be None with no requests");

        // Make a request
        limiter.check();
        let reset_time = limiter.reset_in();
        assert!(reset_time.is_some(), "Should have reset time after request");

        // Reset time should be approximately 1 second (allowing some tolerance)
        let reset_duration = reset_time.unwrap();
        assert!(
            reset_duration <= Duration::from_secs(1),
            "Reset time should be at most 1 second"
        );
    }

    #[test]
    fn test_remaining_after_window_expiry() {
        let mut limiter = RateLimiter::new(2, Duration::from_millis(100));

        limiter.check();
        limiter.check();
        assert_eq!(limiter.remaining(), 0);

        // Wait for window to expire
        thread::sleep(Duration::from_millis(150));

        // Check should clean up old timestamps
        limiter.check();
        assert_eq!(limiter.remaining(), 1, "Should have capacity after window expires");
    }
}
