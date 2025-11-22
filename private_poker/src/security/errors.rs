//! Error types for security module

use thiserror::Error;

/// Result type for anti-collusion operations
pub type AntiCollusionResult<T> = Result<T, AntiCollusionError>;

/// Anti-collusion detection errors
#[derive(Debug, Error)]
pub enum AntiCollusionError {
    /// Database error
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Flag not found
    #[error("Collusion flag not found: {0}")]
    FlagNotFound(i64),

    /// User not found
    #[error("User not found: {0}")]
    UserNotFound(i64),

    /// Table not found
    #[error("Table not found: {0}")]
    TableNotFound(i64),

    /// Invalid flag type
    #[error("Invalid flag type: {0}")]
    InvalidFlagType(String),

    /// Invalid severity
    #[error("Invalid severity: {0}")]
    InvalidSeverity(String),
}

/// Result type for rate limiting operations
pub type RateLimiterResult<T> = Result<T, RateLimitError>;

/// Rate limiting errors
#[derive(Debug, Error)]
pub enum RateLimitError {
    /// Database error
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Rate limit exceeded
    #[error("Rate limit exceeded for {endpoint}: locked until {locked_until}")]
    Exceeded {
        endpoint: String,
        locked_until: chrono::DateTime<chrono::Utc>,
    },

    /// Invalid endpoint configuration
    #[error("Invalid endpoint configuration: {0}")]
    InvalidEndpoint(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),
}
