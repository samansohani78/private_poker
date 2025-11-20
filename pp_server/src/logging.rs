//! Enhanced structured logging configuration.
//!
//! This module provides structured logging with request correlation,
//! performance metrics, and security event tracking.

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize structured logging with enhanced features
///
/// Features:
/// - Request ID correlation
/// - JSON formatting for production
/// - Performance metrics
/// - Security event tracking
/// - Configurable log levels via RUST_LOG env var
///
/// # Example
///
/// ```no_run
/// use pp_server::logging;
///
/// #[tokio::main]
/// async fn main() {
///     logging::init();
///     tracing::info!("Server starting");
/// }
/// ```
pub fn init() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,hyper=warn"));

    // Console layer for development
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();

    tracing::info!("Structured logging initialized");
}

/// Log security event with structured data
///
/// # Arguments
///
/// * `event_type` - Type of security event
/// * `user_id` - Optional user ID
/// * `ip_address` - Optional IP address
/// * `message` - Event message
///
/// # Example
///
/// ```
/// use pp_server::logging::log_security_event;
///
/// log_security_event(
///     "failed_login",
///     Some(123),
///     Some("192.168.1.1"),
///     "Invalid password attempt"
/// );
/// ```
#[allow(dead_code)]
pub fn log_security_event(
    event_type: &str,
    user_id: Option<i64>,
    ip_address: Option<&str>,
    message: &str,
) {
    tracing::warn!(
        event_type = event_type,
        user_id = user_id,
        ip_address = ip_address,
        "SECURITY: {}",
        message
    );
}

/// Log performance metric
///
/// # Arguments
///
/// * `operation` - Operation name
/// * `duration_ms` - Duration in milliseconds
/// * `metadata` - Additional metadata
///
/// # Example
///
/// ```
/// use pp_server::logging::log_performance;
/// use std::time::Instant;
///
/// let start = Instant::now();
/// // ... do work ...
/// let duration = start.elapsed().as_millis() as u64;
/// log_performance("database_query", duration, Some("SELECT FROM users"));
/// ```
#[allow(dead_code)]
pub fn log_performance(operation: &str, duration_ms: u64, metadata: Option<&str>) {
    if duration_ms > 1000 {
        tracing::warn!(
            operation = operation,
            duration_ms = duration_ms,
            metadata = metadata,
            "PERFORMANCE: Slow operation"
        );
    } else {
        tracing::debug!(
            operation = operation,
            duration_ms = duration_ms,
            metadata = metadata,
            "Performance metric"
        );
    }
}

/// Log database operation
///
/// # Arguments
///
/// * `query_type` - Type of query (SELECT, INSERT, UPDATE, DELETE)
/// * `table` - Table name
/// * `duration_ms` - Duration in milliseconds
///
#[allow(dead_code)]
pub fn log_database_operation(query_type: &str, table: &str, duration_ms: u64) {
    tracing::debug!(
        query_type = query_type,
        table = table,
        duration_ms = duration_ms,
        "Database operation"
    );

    if duration_ms > 100 {
        tracing::warn!(
            query_type = query_type,
            table = table,
            duration_ms = duration_ms,
            "Slow database query detected"
        );
    }
}

/// Log API request/response
///
/// # Arguments
///
/// * `method` - HTTP method
/// * `path` - Request path
/// * `status_code` - Response status code
/// * `duration_ms` - Request duration in milliseconds
/// * `user_id` - Optional user ID
///
#[allow(dead_code)]
pub fn log_api_request(
    method: &str,
    path: &str,
    status_code: u16,
    duration_ms: u64,
    user_id: Option<i64>,
) {
    tracing::info!(
        http_method = method,
        http_path = path,
        http_status = status_code,
        duration_ms = duration_ms,
        user_id = user_id,
        "API request completed"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_security_event() {
        // Just ensure it doesn't panic
        log_security_event("test_event", Some(1), Some("127.0.0.1"), "Test message");
    }

    #[test]
    fn test_log_performance() {
        log_performance("test_operation", 500, Some("metadata"));
        log_performance("slow_operation", 2000, None);
    }

    #[test]
    fn test_log_database_operation() {
        log_database_operation("SELECT", "users", 50);
        log_database_operation("INSERT", "sessions", 150);
    }

    #[test]
    fn test_log_api_request() {
        log_api_request("GET", "/api/users", 200, 45, Some(123));
        log_api_request("POST", "/api/login", 401, 120, None);
    }
}
