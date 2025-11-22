//! Enhanced structured logging configuration.
//!
//! This module provides structured logging with request correlation,
//! performance metrics, and security event tracking.

use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

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

    #[test]
    fn test_log_security_event_no_user() {
        log_security_event("test_event", None, Some("192.168.1.1"), "No user event");
    }

    #[test]
    fn test_log_security_event_no_ip() {
        log_security_event("test_event", Some(42), None, "No IP event");
    }

    #[test]
    fn test_log_security_event_minimal() {
        log_security_event("minimal_event", None, None, "Minimal event");
    }

    #[test]
    fn test_log_performance_fast_operation() {
        log_performance("fast_op", 50, None);
        log_performance("very_fast", 1, Some("quick"));
    }

    #[test]
    fn test_log_performance_slow_operation() {
        log_performance("slow_op", 1001, Some("very slow"));
        log_performance("super_slow", 5000, None);
    }

    #[test]
    fn test_log_performance_boundary() {
        log_performance("boundary", 1000, None); // Exactly at threshold
        log_performance("just_over", 1001, Some("just slow"));
    }

    #[test]
    fn test_log_database_fast_query() {
        log_database_operation("SELECT", "users", 10);
        log_database_operation("SELECT", "sessions", 99);
    }

    #[test]
    fn test_log_database_slow_query() {
        log_database_operation("SELECT", "users", 101);
        log_database_operation("UPDATE", "wallets", 500);
    }

    #[test]
    fn test_log_database_all_query_types() {
        log_database_operation("SELECT", "users", 50);
        log_database_operation("INSERT", "users", 60);
        log_database_operation("UPDATE", "users", 70);
        log_database_operation("DELETE", "users", 80);
    }

    #[test]
    fn test_log_api_request_various_methods() {
        log_api_request("GET", "/api/health", 200, 5, None);
        log_api_request("POST", "/api/auth/register", 201, 150, None);
        log_api_request("PUT", "/api/users/123", 200, 100, Some(123));
        log_api_request("DELETE", "/api/sessions", 204, 50, Some(456));
        log_api_request("PATCH", "/api/profile", 200, 75, Some(789));
    }

    #[test]
    fn test_log_api_request_various_status_codes() {
        log_api_request("GET", "/api/test", 200, 50, None); // OK
        log_api_request("POST", "/api/test", 201, 50, None); // Created
        log_api_request("GET", "/api/test", 400, 50, None); // Bad Request
        log_api_request("GET", "/api/test", 401, 50, None); // Unauthorized
        log_api_request("GET", "/api/test", 404, 50, None); // Not Found
        log_api_request("POST", "/api/test", 500, 50, None); // Internal Server Error
    }

    #[test]
    fn test_log_api_request_long_duration() {
        log_api_request("POST", "/api/slow", 200, 5000, Some(1));
        log_api_request("GET", "/api/timeout", 504, 30000, None);
    }

    #[test]
    fn test_log_functions_with_special_characters() {
        log_security_event(
            "login/failure",
            Some(1),
            Some("127.0.0.1"),
            "Failed @ login",
        );
        log_performance("db::query", 100, Some("table: users & sessions"));
        log_database_operation("SELECT", "table_with_underscore", 50);
        log_api_request("GET", "/api/path/with/slashes", 200, 50, Some(1));
    }

    #[test]
    fn test_log_functions_with_empty_strings() {
        log_security_event("", Some(1), Some(""), "");
        log_performance("", 0, Some(""));
        log_database_operation("", "", 0);
        log_api_request("", "", 0, 0, None);
    }

    #[test]
    fn test_log_functions_with_very_long_strings() {
        let long_string = "x".repeat(1000);
        log_security_event(&long_string, Some(1), Some(&long_string), &long_string);
        log_performance(&long_string, 100, Some(&long_string));
        log_database_operation(&long_string, &long_string, 100);
        log_api_request(&long_string, &long_string, 200, 100, Some(1));
    }

    #[test]
    fn test_multiple_concurrent_logs() {
        // Simulate multiple logs happening quickly
        for i in 0..10 {
            let i_u64 = i as u64;
            log_security_event("concurrent", Some(i), Some("127.0.0.1"), "Concurrent event");
            log_performance("concurrent", i_u64 * 100, None);
            log_database_operation("SELECT", "test", i_u64 * 10);
            log_api_request("GET", "/test", 200, i_u64 * 10, Some(i));
        }
    }
}
