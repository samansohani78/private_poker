//! Database configuration module.
//!
//! Provides configuration structures for database connection management.

use std::env;

/// Database configuration
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// PostgreSQL connection URL
    pub database_url: String,

    /// Maximum number of connections in the pool
    pub max_connections: u32,

    /// Minimum number of connections in the pool
    pub min_connections: u32,

    /// Connection timeout in seconds
    pub connection_timeout_secs: u64,

    /// Idle connection timeout in seconds
    pub idle_timeout_secs: u64,

    /// Maximum connection lifetime in seconds
    pub max_lifetime_secs: u64,
}

impl DatabaseConfig {
    /// Create configuration from environment variables
    ///
    /// Expected environment variables:
    /// - `DATABASE_URL`: PostgreSQL connection string
    /// - `DB_MAX_CONNECTIONS`: Maximum pool size (default: 20)
    /// - `DB_MIN_CONNECTIONS`: Minimum pool size (default: 5)
    /// - `DB_CONNECTION_TIMEOUT`: Connection timeout in seconds (default: 10)
    /// - `DB_IDLE_TIMEOUT`: Idle timeout in seconds (default: 600)
    /// - `DB_MAX_LIFETIME`: Max lifetime in seconds (default: 1800)
    ///
    /// # Returns
    ///
    /// * `DatabaseConfig` - Configuration from environment
    ///
    /// # Panics
    ///
    /// Panics if `DATABASE_URL` is not set
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            max_connections: env::var("DB_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "20".to_string())
                .parse()
                .expect("DB_MAX_CONNECTIONS must be a valid u32"),
            min_connections: env::var("DB_MIN_CONNECTIONS")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .expect("DB_MIN_CONNECTIONS must be a valid u32"),
            connection_timeout_secs: env::var("DB_CONNECTION_TIMEOUT")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .expect("DB_CONNECTION_TIMEOUT must be a valid u64"),
            idle_timeout_secs: env::var("DB_IDLE_TIMEOUT")
                .unwrap_or_else(|_| "600".to_string())
                .parse()
                .expect("DB_IDLE_TIMEOUT must be a valid u64"),
            max_lifetime_secs: env::var("DB_MAX_LIFETIME")
                .unwrap_or_else(|_| "1800".to_string())
                .parse()
                .expect("DB_MAX_LIFETIME must be a valid u64"),
        }
    }

    /// Create a default configuration for development
    ///
    /// Uses `postgres://postgres@localhost/poker_db` as the database URL
    ///
    /// # Returns
    ///
    /// * `DatabaseConfig` - Default development configuration
    pub fn development() -> Self {
        Self {
            database_url: "postgres://postgres@localhost/poker_db".to_string(),
            max_connections: 20,
            min_connections: 5,
            connection_timeout_secs: 10,
            idle_timeout_secs: 600,
            max_lifetime_secs: 1800,
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self::development()
    }
}
