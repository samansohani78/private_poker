//! Database query timeout helpers
//!
//! Provides timeout wrappers for database operations to prevent indefinite hangs.

use std::time::Duration;
use tokio::time::timeout;

/// Default timeout for database queries (5 seconds)
pub const DEFAULT_QUERY_TIMEOUT: Duration = Duration::from_secs(5);

/// Default timeout for transactions (10 seconds)
pub const DEFAULT_TRANSACTION_TIMEOUT: Duration = Duration::from_secs(10);

/// Default timeout for long-running operations (30 seconds)
pub const LONG_OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

/// Error type for timeout operations
#[derive(Debug, thiserror::Error)]
pub enum TimeoutError {
    /// Operation timed out
    #[error("Database operation timed out after {0:?}")]
    Timeout(Duration),

    /// Database error
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Result type for timeout operations
pub type TimeoutResult<T> = Result<T, TimeoutError>;

/// Execute a query with timeout
///
/// # Arguments
///
/// * `duration` - Timeout duration
/// * `future` - Async operation to execute
///
/// # Returns
///
/// * `TimeoutResult<T>` - Result or timeout error
///
/// # Example
///
/// ```no_run
/// use private_poker::db::timeouts::{with_timeout, DEFAULT_QUERY_TIMEOUT};
/// # use sqlx::PgPool;
/// # async fn example(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
///
/// let result = with_timeout(
///     DEFAULT_QUERY_TIMEOUT,
///     sqlx::query("SELECT * FROM users WHERE id = $1")
///         .bind(1)
///         .fetch_one(pool)
/// ).await?;
///
/// # Ok(())
/// # }
/// ```
pub async fn with_timeout<F, T>(duration: Duration, future: F) -> TimeoutResult<T>
where
    F: std::future::Future<Output = Result<T, sqlx::Error>>,
{
    match timeout(duration, future).await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(e)) => Err(TimeoutError::Database(e)),
        Err(_) => Err(TimeoutError::Timeout(duration)),
    }
}

/// Execute a query with default timeout (5 seconds)
pub async fn with_default_timeout<F, T>(future: F) -> TimeoutResult<T>
where
    F: std::future::Future<Output = Result<T, sqlx::Error>>,
{
    with_timeout(DEFAULT_QUERY_TIMEOUT, future).await
}

/// Execute a long-running query with extended timeout (30 seconds)
pub async fn with_long_timeout<F, T>(future: F) -> TimeoutResult<T>
where
    F: std::future::Future<Output = Result<T, sqlx::Error>>,
{
    with_timeout(LONG_OPERATION_TIMEOUT, future).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_timeout_constants() {
        assert_eq!(DEFAULT_QUERY_TIMEOUT.as_secs(), 5);
        assert_eq!(DEFAULT_TRANSACTION_TIMEOUT.as_secs(), 10);
        assert_eq!(LONG_OPERATION_TIMEOUT.as_secs(), 30);
    }

    #[tokio::test]
    async fn test_timeout_error_display() {
        let err = TimeoutError::Timeout(Duration::from_secs(5));
        assert!(err.to_string().contains("timed out"));
        assert!(err.to_string().contains("5s"));
    }
}
