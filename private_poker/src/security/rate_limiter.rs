//! Rate limiting with exponential backoff for security endpoints.

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
            max_attempts: 5,
            window_secs: 300,      // 5 minutes
            lockout_secs: 900,      // 15 minutes
            exponential_backoff: true,
        }
    }

    /// Configuration for registration endpoint
    pub fn register() -> Self {
        Self {
            max_attempts: 3,
            window_secs: 3600,     // 1 hour
            lockout_secs: 3600,     // 1 hour
            exponential_backoff: false,
        }
    }

    /// Configuration for password reset endpoint
    pub fn password_reset() -> Self {
        Self {
            max_attempts: 3,
            window_secs: 3600,     // 1 hour
            lockout_secs: 7200,     // 2 hours
            exponential_backoff: true,
        }
    }

    /// Configuration for chat messages
    pub fn chat() -> Self {
        Self {
            max_attempts: 10,
            window_secs: 60,       // 1 minute
            lockout_secs: 300,      // 5 minutes
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
        configs.insert("password_reset".to_string(), RateLimitConfig::password_reset());
        configs.insert("chat".to_string(), RateLimitConfig::chat());

        Self {
            pool,
            cache: Arc::new(RwLock::new(HashMap::new())),
            configs,
        }
    }

    /// Check if an action is allowed
    ///
    /// # Arguments
    ///
    /// * `endpoint` - Endpoint name (e.g., "login", "register")
    /// * `identifier` - Unique identifier (IP address or username)
    ///
    /// # Returns
    ///
    /// * `Result<bool, String>` - Whether action is allowed or error
    pub async fn check_rate_limit(
        &self,
        endpoint: &str,
        identifier: &str,
    ) -> Result<RateLimitResult, String> {
        let config = self
            .configs
            .get(endpoint)
            .ok_or_else(|| format!("Unknown endpoint: {}", endpoint))?;

        let key = format!("{}:{}", endpoint, identifier);

        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(attempt) = cache.get(&key) {
                if let Some(locked_until) = attempt.locked_until {
                    if Utc::now() < locked_until {
                        let retry_after = (locked_until - Utc::now()).num_seconds() as u64;
                        return Ok(RateLimitResult::Locked { retry_after });
                    }
                }
            }
        }

        // Load from database
        let attempt = self.load_attempt(endpoint, identifier).await?;

        // Check if locked
        if let Some(locked_until) = attempt.locked_until {
            if Utc::now() < locked_until {
                let retry_after = (locked_until - Utc::now()).num_seconds() as u64;
                return Ok(RateLimitResult::Locked { retry_after });
            }
        }

        // Check if window expired
        let window_duration = Duration::seconds(config.window_secs as i64);
        if Utc::now() - attempt.window_start > window_duration {
            // Reset window
            let new_attempt = RateLimitAttempt {
                attempts: 0,
                window_start: Utc::now(),
                locked_until: None,
                consecutive_violations: 0,
            };

            self.cache.write().await.insert(key.clone(), new_attempt.clone());
            return Ok(RateLimitResult::Allowed {
                remaining: config.max_attempts,
            });
        }

        // Check if limit exceeded
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
                attempts: attempt.attempts,
                window_start: attempt.window_start,
                locked_until: Some(locked_until),
                consecutive_violations: attempt.consecutive_violations + 1,
            };

            self.cache.write().await.insert(key.clone(), new_attempt.clone());
            self.save_attempt(endpoint, identifier, &new_attempt).await?;

            return Ok(RateLimitResult::Locked {
                retry_after: lockout_duration,
            });
        }

        // Allowed
        let remaining = config.max_attempts - attempt.attempts;
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
    /// * `Result<(), String>` - Success or error
    pub async fn record_attempt(&self, endpoint: &str, identifier: &str) -> Result<(), String> {
        let key = format!("{}:{}", endpoint, identifier);

        let mut cache = self.cache.write().await;
        let attempt = cache.entry(key.clone()).or_insert_with(|| RateLimitAttempt {
            attempts: 0,
            window_start: Utc::now(),
            locked_until: None,
            consecutive_violations: 0,
        });

        attempt.attempts += 1;
        let updated_attempt = attempt.clone();
        drop(cache);

        self.save_attempt(endpoint, identifier, &updated_attempt).await?;

        Ok(())
    }

    /// Reset rate limit for an identifier
    ///
    /// # Arguments
    ///
    /// * `endpoint` - Endpoint name
    /// * `identifier` - Unique identifier
    pub async fn reset(&self, endpoint: &str, identifier: &str) -> Result<(), String> {
        let key = format!("{}:{}", endpoint, identifier);
        self.cache.write().await.remove(&key);

        sqlx::query("DELETE FROM rate_limit_attempts WHERE endpoint = $1 AND identifier = $2")
            .bind(endpoint)
            .bind(identifier)
            .execute(self.pool.as_ref())
            .await
            .map_err(|e| format!("Database error: {}", e))?;

        Ok(())
    }

    /// Load attempt from database
    async fn load_attempt(
        &self,
        endpoint: &str,
        identifier: &str,
    ) -> Result<RateLimitAttempt, String> {
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
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        if let Some(row) = row {
            Ok(RateLimitAttempt {
                attempts: row.get::<i32, _>("attempts") as u32,
                window_start: row.get::<chrono::NaiveDateTime, _>("window_start").and_utc(),
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
    ) -> Result<(), String> {
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
        .map_err(|e| format!("Database error: {}", e))?;

        Ok(())
    }

    /// Clean up expired rate limit records
    pub async fn cleanup_expired(&self) -> Result<u64, String> {
        let result = sqlx::query(
            "DELETE FROM rate_limit_attempts WHERE locked_until < NOW() AND locked_until IS NOT NULL"
        )
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| format!("Database error: {}", e))?;

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
