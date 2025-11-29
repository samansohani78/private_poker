//! Rate limiting with exponential backoff for security endpoints.

#![allow(clippy::needless_raw_string_hashes)]

use super::errors::{RateLimitError, RateLimiterResult};
use chrono::{DateTime, Duration, Utc};
use sqlx::{PgPool, Row};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

/// Rate limit configuration for an endpoint
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum attempts allowed in window
    pub max_attempts: u32,

    /// Time window in seconds
    pub window_secs: u64,

    /// Lockout duration in seconds after exceeding limit
    pub lockout_secs: u64,

    /// Whether to use exponential backoff
    pub exponential_backoff: bool,
}

impl RateLimitConfig {
    /// Configuration for login endpoint
    pub fn login() -> Self {
        Self {
            max_attempts: std::env::var("RATE_LIMIT_LOGIN_ATTEMPTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
            window_secs: std::env::var("RATE_LIMIT_LOGIN_WINDOW_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(300),
            lockout_secs: std::env::var("RATE_LIMIT_LOGIN_LOCKOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(900),
            exponential_backoff: true,
        }
    }

    /// Configuration for registration endpoint
    pub fn register() -> Self {
        Self {
            max_attempts: std::env::var("RATE_LIMIT_REGISTER_ATTEMPTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
            window_secs: std::env::var("RATE_LIMIT_REGISTER_WINDOW_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3600),
            lockout_secs: std::env::var("RATE_LIMIT_REGISTER_LOCKOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3600),
            exponential_backoff: false,
        }
    }

    /// Configuration for password reset endpoint
    pub fn password_reset() -> Self {
        Self {
            max_attempts: std::env::var("RATE_LIMIT_RESET_ATTEMPTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
            window_secs: std::env::var("RATE_LIMIT_RESET_WINDOW_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3600),
            lockout_secs: std::env::var("RATE_LIMIT_RESET_LOCKOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(7200),
            exponential_backoff: true,
        }
    }

    /// Configuration for chat messages
    pub fn chat() -> Self {
        Self {
            max_attempts: std::env::var("RATE_LIMIT_CHAT_ATTEMPTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            window_secs: std::env::var("RATE_LIMIT_CHAT_WINDOW_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
            lockout_secs: std::env::var("RATE_LIMIT_CHAT_LOCKOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(300),
            exponential_backoff: false,
        }
    }
}

/// Rate limit attempt record
#[derive(Debug, Clone)]
struct RateLimitAttempt {
    attempts: u32,
    window_start: DateTime<Utc>,
    locked_until: Option<DateTime<Utc>>,
    consecutive_violations: u32,
}

/// Rate limiter with database-backed persistence
pub struct RateLimiter {
    /// Database pool
    pool: Arc<PgPool>,

    /// In-memory cache for fast lookups
    cache: Arc<RwLock<HashMap<String, RateLimitAttempt>>>,

    /// Endpoint configurations
    configs: HashMap<String, RateLimitConfig>,
}

impl RateLimiter {
    /// Create a new rate limiter
    ///
    /// # Arguments
    ///
    /// * `pool` - Database connection pool
    ///
    /// # Returns
    ///
    /// * `RateLimiter` - New rate limiter instance
    pub fn new(pool: Arc<PgPool>) -> Self {
        let mut configs = HashMap::new();
        configs.insert("login".to_string(), RateLimitConfig::login());
        configs.insert("register".to_string(), RateLimitConfig::register());
        configs.insert(
            "password_reset".to_string(),
            RateLimitConfig::password_reset(),
        );
        configs.insert("chat".to_string(), RateLimitConfig::chat());

        Self {
            pool,
            cache: Arc::new(RwLock::new(HashMap::new())),
            configs,
        }
    }

    /// Atomically check rate limit and record attempt (race-free)
    ///
    /// This method combines check and increment in a single atomic operation
    /// to prevent race conditions where multiple concurrent requests could
    /// all pass the check before any of them record an attempt.
    ///
    /// # Arguments
    ///
    /// * `endpoint` - Endpoint name (e.g., "login", "register")
    /// * `identifier` - Unique identifier (IP address or username)
    ///
    /// # Returns
    ///
    /// * `RateLimiterResult<RateLimitResult>` - Whether action is allowed, locked, or error
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use private_poker::security::rate_limiter::{RateLimiter, RateLimitResult};
    /// # async fn example(limiter: &RateLimiter) {
    /// match limiter.check_and_record("login", "192.168.1.1").await {
    ///     Ok(RateLimitResult::Allowed { remaining }) => {
    ///         println!("Request allowed, {} attempts remaining", remaining);
    ///     }
    ///     Ok(RateLimitResult::Locked { retry_after }) => {
    ///         println!("Rate limited, retry after {} seconds", retry_after);
    ///     }
    ///     Err(e) => println!("Error: {}", e),
    /// }
    /// # }
    /// ```
    pub async fn check_and_record(
        &self,
        endpoint: &str,
        identifier: &str,
    ) -> RateLimiterResult<RateLimitResult> {
        let config = self
            .configs
            .get(endpoint)
            .ok_or_else(|| RateLimitError::InvalidEndpoint(endpoint.to_string()))?;

        let key = format!("{}:{}", endpoint, identifier);

        // Acquire write lock upfront for atomic check-then-act
        // This prevents race conditions from concurrent requests
        let mut cache = self.cache.write().await;

        // Load current attempt state (from cache or DB)
        let attempt = if let Some(cached_attempt) = cache.get(&key) {
            cached_attempt.clone()
        } else {
            // Release lock during DB read to avoid holding lock during I/O
            drop(cache);
            let loaded_attempt = self.load_attempt(endpoint, identifier).await?;

            // Reacquire lock to insert into cache
            cache = self.cache.write().await;

            // Check again if another request loaded it while we were waiting
            if let Some(cached_attempt) = cache.get(&key) {
                cached_attempt.clone()
            } else {
                cache.insert(key.clone(), loaded_attempt.clone());
                loaded_attempt
            }
        };

        // Check if locked (still holding write lock)
        if let Some(locked_until) = attempt.locked_until
            && Utc::now() < locked_until
        {
            let retry_after = (locked_until - Utc::now()).num_seconds() as u64;
            return Ok(RateLimitResult::Locked { retry_after });
        }

        // Check if window expired
        let window_duration = Duration::seconds(config.window_secs as i64);
        if Utc::now() - attempt.window_start > window_duration {
            // Reset window and record first attempt
            let new_attempt = RateLimitAttempt {
                attempts: 1, // Record this attempt
                window_start: Utc::now(),
                locked_until: None,
                consecutive_violations: 0,
            };

            cache.insert(key.clone(), new_attempt.clone());
            drop(cache); // Release lock before DB write

            self.save_attempt(endpoint, identifier, &new_attempt).await?;

            return Ok(RateLimitResult::Allowed {
                remaining: config.max_attempts - 1,
            });
        }

        // Check if limit already exceeded
        if attempt.attempts >= config.max_attempts {
            let lockout_duration = if config.exponential_backoff {
                // Exponential backoff: 2^violations * base_lockout
                let multiplier = 2u64.pow(attempt.consecutive_violations.min(5));
                config.lockout_secs * multiplier
            } else {
                config.lockout_secs
            };

            let locked_until = Utc::now() + Duration::seconds(lockout_duration as i64);

            let new_attempt = RateLimitAttempt {
                attempts: attempt.attempts + 1, // Increment anyway to track violations
                window_start: attempt.window_start,
                locked_until: Some(locked_until),
                consecutive_violations: attempt.consecutive_violations + 1,
            };

            cache.insert(key.clone(), new_attempt.clone());
            drop(cache); // Release lock before DB write

            self.save_attempt(endpoint, identifier, &new_attempt).await?;

            return Ok(RateLimitResult::Locked {
                retry_after: lockout_duration,
            });
        }

        // Allowed - atomically increment attempt count
        let new_attempt = RateLimitAttempt {
            attempts: attempt.attempts + 1,
            window_start: attempt.window_start,
            locked_until: None,
            consecutive_violations: attempt.consecutive_violations,
        };

        let remaining = config.max_attempts - new_attempt.attempts;
        cache.insert(key.clone(), new_attempt.clone());
        drop(cache); // Release lock before DB write

        self.save_attempt(endpoint, identifier, &new_attempt).await?;

        Ok(RateLimitResult::Allowed { remaining })
    }

    /// Record an attempt
    ///
    /// # Arguments
    ///
    /// * `endpoint` - Endpoint name
    /// * `identifier` - Unique identifier
    ///
    /// # Returns
    ///
    /// * `RateLimiterResult<()>` - Success or error
    pub async fn record_attempt(&self, endpoint: &str, identifier: &str) -> RateLimiterResult<()> {
        let key = format!("{}:{}", endpoint, identifier);

        let mut cache = self.cache.write().await;
        let attempt = cache
            .entry(key.clone())
            .or_insert_with(|| RateLimitAttempt {
                attempts: 0,
                window_start: Utc::now(),
                locked_until: None,
                consecutive_violations: 0,
            });

        attempt.attempts += 1;
        let updated_attempt = attempt.clone();
        drop(cache);

        self.save_attempt(endpoint, identifier, &updated_attempt)
            .await?;

        Ok(())
    }

    /// Reset rate limit for an identifier
    ///
    /// # Arguments
    ///
    /// * `endpoint` - Endpoint name
    /// * `identifier` - Unique identifier
    pub async fn reset(&self, endpoint: &str, identifier: &str) -> RateLimiterResult<()> {
        let key = format!("{}:{}", endpoint, identifier);
        self.cache.write().await.remove(&key);

        sqlx::query("DELETE FROM rate_limit_attempts WHERE endpoint = $1 AND identifier = $2")
            .bind(endpoint)
            .bind(identifier)
            .execute(self.pool.as_ref())
            .await?;

        Ok(())
    }

    /// Load attempt from database
    async fn load_attempt(
        &self,
        endpoint: &str,
        identifier: &str,
    ) -> RateLimiterResult<RateLimitAttempt> {
        let row = sqlx::query(
            r#"
            SELECT attempts, window_start, locked_until
            FROM rate_limit_attempts
            WHERE endpoint = $1 AND identifier = $2
            "#,
        )
        .bind(endpoint)
        .bind(identifier)
        .fetch_optional(self.pool.as_ref())
        .await?;

        if let Some(row) = row {
            // Safely convert i32 to u32 with validation
            let attempts_i32 = row.get::<i32, _>("attempts");
            if attempts_i32 < 0 {
                return Err(RateLimitError::DatabaseCorruption {
                    message: format!(
                        "Negative attempt count in database: {} for {}:{}",
                        attempts_i32, endpoint, identifier
                    ),
                });
            }
            let attempts = attempts_i32 as u32;

            Ok(RateLimitAttempt {
                attempts,
                window_start: row
                    .get::<chrono::NaiveDateTime, _>("window_start")
                    .and_utc(),
                locked_until: row
                    .get::<Option<chrono::NaiveDateTime>, _>("locked_until")
                    .map(|dt| dt.and_utc()),
                consecutive_violations: 0,
            })
        } else {
            Ok(RateLimitAttempt {
                attempts: 0,
                window_start: Utc::now(),
                locked_until: None,
                consecutive_violations: 0,
            })
        }
    }

    /// Save attempt to database
    async fn save_attempt(
        &self,
        endpoint: &str,
        identifier: &str,
        attempt: &RateLimitAttempt,
    ) -> RateLimiterResult<()> {
        sqlx::query(
            r#"
            INSERT INTO rate_limit_attempts (endpoint, identifier, attempts, window_start, locked_until)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (endpoint, identifier)
            DO UPDATE SET
                attempts = EXCLUDED.attempts,
                window_start = EXCLUDED.window_start,
                locked_until = EXCLUDED.locked_until
            "#,
        )
        .bind(endpoint)
        .bind(identifier)
        .bind(attempt.attempts as i32)
        .bind(attempt.window_start.naive_utc())
        .bind(attempt.locked_until.map(|dt| dt.naive_utc()))
        .execute(self.pool.as_ref())
        .await
        ?;

        Ok(())
    }

    /// Clean up expired rate limit records
    pub async fn cleanup_expired(&self) -> RateLimiterResult<u64> {
        let result = sqlx::query(
            "DELETE FROM rate_limit_attempts WHERE locked_until < NOW() AND locked_until IS NOT NULL"
        )
        .execute(self.pool.as_ref())
        .await
        ?;

        Ok(result.rows_affected())
    }
}

/// Rate limit check result
#[derive(Debug, Clone)]
pub enum RateLimitResult {
    /// Action is allowed
    Allowed { remaining: u32 },

    /// Action is blocked due to rate limit
    Locked { retry_after: u64 },
}

impl RateLimitResult {
    /// Check if action is allowed
    pub fn is_allowed(&self) -> bool {
        matches!(self, RateLimitResult::Allowed { .. })
    }

    /// Get remaining attempts (if allowed)
    pub fn remaining(&self) -> Option<u32> {
        match self {
            RateLimitResult::Allowed { remaining } => Some(*remaining),
            _ => None,
        }
    }

    /// Get retry after seconds (if locked)
    pub fn retry_after(&self) -> Option<u64> {
        match self {
            RateLimitResult::Locked { retry_after } => Some(*retry_after),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::task::JoinSet;

    /// Helper to create a test rate limiter with custom config
    async fn create_test_limiter() -> RateLimiter {
        let pool = Arc::new(
            sqlx::PgPool::connect(&std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://poker_test:test_password@localhost/poker_test".to_string()
            }))
            .await
            .expect("Failed to connect to test database"),
        );

        let mut configs = HashMap::new();
        configs.insert(
            "test_endpoint".to_string(),
            RateLimitConfig {
                max_attempts: 5,
                window_secs: 60,
                lockout_secs: 300,
                exponential_backoff: false,
            },
        );

        RateLimiter {
            pool,
            cache: Arc::new(RwLock::new(HashMap::new())),
            configs,
        }
    }

    #[tokio::test]
    #[ignore = "Requires database setup"]
    async fn test_check_and_record_allows_within_limit() {
        let limiter = create_test_limiter().await;

        // First 5 requests should be allowed
        for i in 1..=5 {
            let result = limiter
                .check_and_record("test_endpoint", "test_user")
                .await
                .unwrap();

            match result {
                RateLimitResult::Allowed { remaining } => {
                    assert_eq!(remaining, 5 - i, "Attempt {}: wrong remaining count", i);
                }
                RateLimitResult::Locked { .. } => {
                    panic!("Attempt {}: should be allowed, got locked", i);
                }
            }
        }

        // 6th request should be locked
        let result = limiter
            .check_and_record("test_endpoint", "test_user")
            .await
            .unwrap();

        assert!(
            matches!(result, RateLimitResult::Locked { .. }),
            "6th attempt should be locked"
        );
    }

    #[tokio::test]
    #[ignore = "Requires database setup"]
    async fn test_concurrent_requests_no_race_condition() {
        let limiter = Arc::new(create_test_limiter().await);
        let identifier = "concurrent_test_user";

        // Spawn 100 concurrent requests
        let mut join_set = JoinSet::new();

        for _ in 0..100 {
            let limiter_clone = Arc::clone(&limiter);
            let id = identifier.to_string();
            join_set.spawn(async move {
                limiter_clone
                    .check_and_record("test_endpoint", &id)
                    .await
            });
        }

        // Collect results
        let mut allowed_count = 0;
        let mut locked_count = 0;

        while let Some(result) = join_set.join_next().await {
            match result.unwrap().unwrap() {
                RateLimitResult::Allowed { .. } => allowed_count += 1,
                RateLimitResult::Locked { .. } => locked_count += 1,
            }
        }

        // With max_attempts = 5, exactly 5 should be allowed
        // The remaining 95 should be locked
        assert_eq!(
            allowed_count, 5,
            "Expected exactly 5 allowed requests (no race condition), got {}",
            allowed_count
        );
        assert_eq!(
            locked_count, 95,
            "Expected 95 locked requests, got {}",
            locked_count
        );
    }

    #[tokio::test]
    #[ignore = "Requires database setup"]
    async fn test_window_reset_allows_new_requests() {
        let pool = Arc::new(
            sqlx::PgPool::connect(&std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://poker_test:test_password@localhost/poker_test".to_string()
            }))
            .await
            .expect("Failed to connect to test database"),
        );

        let mut configs = HashMap::new();
        configs.insert(
            "test_endpoint".to_string(),
            RateLimitConfig {
                max_attempts: 3,
                window_secs: 1, // 1 second window for fast testing
                lockout_secs: 5,
                exponential_backoff: false,
            },
        );

        let limiter = RateLimiter {
            pool,
            cache: Arc::new(RwLock::new(HashMap::new())),
            configs,
        };

        // Use all 3 attempts
        for _ in 0..3 {
            limiter
                .check_and_record("test_endpoint", "test_reset_user")
                .await
                .unwrap();
        }

        // 4th should be locked
        let result = limiter
            .check_and_record("test_endpoint", "test_reset_user")
            .await
            .unwrap();
        assert!(matches!(result, RateLimitResult::Locked { .. }));

        // Wait for lockout to expire (lockout_secs = 5)
        tokio::time::sleep(tokio::time::Duration::from_secs(6)).await;

        // After lockout expires and window expires, should be allowed again (new window)
        let result = limiter
            .check_and_record("test_endpoint", "test_reset_user")
            .await
            .unwrap();
        assert!(
            matches!(result, RateLimitResult::Allowed { remaining: 2 }),
            "After lockout and window reset, should be allowed with 2 remaining"
        );
    }

    #[tokio::test]
    #[ignore = "Requires database setup"]
    async fn test_different_identifiers_independent() {
        let limiter = Arc::new(create_test_limiter().await);

        // User 1 uses all attempts
        for _ in 0..5 {
            limiter
                .check_and_record("test_endpoint", "user1")
                .await
                .unwrap();
        }

        // User 1 should be locked
        let result1 = limiter
            .check_and_record("test_endpoint", "user1")
            .await
            .unwrap();
        assert!(matches!(result1, RateLimitResult::Locked { .. }));

        // User 2 should still be allowed
        let result2 = limiter
            .check_and_record("test_endpoint", "user2")
            .await
            .unwrap();
        assert!(
            matches!(result2, RateLimitResult::Allowed { remaining: 4 }),
            "Different users should have independent rate limits"
        );
    }
}
