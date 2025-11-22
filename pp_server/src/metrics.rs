//! Prometheus metrics for monitoring poker server health and performance.
//!
//! This module provides metrics collection and export via the `/metrics` endpoint.
//! Metrics are exposed in Prometheus text format for scraping by monitoring systems.
//!
//! # Metrics Categories
//!
//! - **HTTP Metrics**: Request counts, duration, status codes
//! - **WebSocket Metrics**: Active connections, messages sent/received
//! - **Game Metrics**: Active tables, players, hands played
//! - **Database Metrics**: Query counts, connection pool status
//! - **Auth Metrics**: Login attempts, active sessions
//!
//! # Example Usage
//!
//! ```rust,no_run
//! use pp_server::metrics;
//! use std::net::SocketAddr;
//!
//! // Initialize metrics exporter
//! let addr: SocketAddr = "127.0.0.1:9090".parse().unwrap();
//! metrics::init_metrics(addr).unwrap();
//!
//! // Record HTTP request
//! metrics::http_requests_total("POST", "/api/auth/login", 200);
//!
//! // Record WebSocket connection
//! metrics::websocket_connections_active(10);
//! ```

#![allow(dead_code)] // Public API for future integration

use metrics_exporter_prometheus::PrometheusBuilder;
use std::net::SocketAddr;

/// Initialize Prometheus metrics exporter.
///
/// Sets up a Prometheus scrape endpoint on the specified address.
/// Metrics will be available at `http://<addr>/metrics`.
///
/// # Arguments
///
/// - `addr`: Address to bind the metrics server to (e.g., `0.0.0.0:9090`)
///
/// # Returns
///
/// Result indicating success or error message
pub fn init_metrics(addr: SocketAddr) -> Result<(), String> {
    PrometheusBuilder::new()
        .with_http_listener(addr)
        .install()
        .map_err(|e| format!("Failed to install Prometheus exporter: {}", e))
}

// ============================================================================
// HTTP Metrics
// ============================================================================

/// Record HTTP request.
///
/// Increments the total HTTP request counter with method, path, and status labels.
pub fn http_requests_total(method: &str, path: &str, status: u16) {
    metrics::counter!("http_requests_total",
        "method" => method.to_string(),
        "path" => path.to_string(),
        "status" => status.to_string()
    )
    .increment(1);
}

/// Record HTTP request duration in milliseconds.
pub fn http_request_duration_ms(method: &str, path: &str, duration_ms: f64) {
    metrics::histogram!("http_request_duration_ms",
        "method" => method.to_string(),
        "path" => path.to_string()
    )
    .record(duration_ms);
}

// ============================================================================
// WebSocket Metrics
// ============================================================================

/// Set current active WebSocket connections count.
pub fn websocket_connections_active(count: u64) {
    metrics::gauge!("websocket_connections_active").set(count as f64);
}

/// Increment total WebSocket connections counter.
pub fn websocket_connections_total() {
    metrics::counter!("websocket_connections_total").increment(1);
}

/// Increment WebSocket messages sent counter.
pub fn websocket_messages_sent() {
    metrics::counter!("websocket_messages_sent").increment(1);
}

/// Increment WebSocket messages received counter.
pub fn websocket_messages_received() {
    metrics::counter!("websocket_messages_received").increment(1);
}

// ============================================================================
// Game Metrics
// ============================================================================

/// Set current active tables count.
pub fn active_tables(count: i32) {
    metrics::gauge!("active_tables").set(count as f64);
}

/// Set current active players count.
pub fn active_players(count: usize) {
    metrics::gauge!("active_players").set(count as f64);
}

/// Increment hands played counter.
pub fn hands_played_total() {
    metrics::counter!("hands_played_total").increment(1);
}

/// Record pot size distribution.
pub fn pot_size_chips(size: i64) {
    metrics::histogram!("pot_size_chips").record(size as f64);
}

// ============================================================================
// Database Metrics
// ============================================================================

/// Record database query duration in milliseconds.
pub fn db_query_duration_ms(query_type: &str, duration_ms: f64) {
    metrics::histogram!("db_query_duration_ms",
        "query_type" => query_type.to_string()
    )
    .record(duration_ms);
}

/// Set current database connection pool size.
pub fn db_connections_active(count: u32) {
    metrics::gauge!("db_connections_active").set(count as f64);
}

// ============================================================================
// Auth Metrics
// ============================================================================

/// Increment login attempts counter.
pub fn login_attempts_total(success: bool) {
    metrics::counter!("login_attempts_total",
        "success" => success.to_string()
    )
    .increment(1);
}

/// Set current active sessions count.
pub fn active_sessions(count: i64) {
    metrics::gauge!("active_sessions").set(count as f64);
}

// ============================================================================
// Rate Limiting Metrics
// ============================================================================

/// Increment rate limit hits counter.
pub fn rate_limit_hits_total(endpoint: &str) {
    metrics::counter!("rate_limit_hits_total",
        "endpoint" => endpoint.to_string()
    )
    .increment(1);
}
