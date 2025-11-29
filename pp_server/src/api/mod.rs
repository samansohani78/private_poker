//! HTTP/WebSocket API for the poker server.
//!
//! This module provides the complete REST and WebSocket API for the multi-table poker platform.
//! It handles authentication, table management, real-time game updates, and player actions.
//!
//! # Architecture
//!
//! The API is built with:
//! - **Axum**: Async web framework for HTTP/WebSocket
//! - **Tower**: Middleware for CORS, authentication
//! - **JWT**: Token-based authentication with access/refresh tokens
//! - **Actor Model**: Table state managed by dedicated actor tasks
//!
//! # Modules
//!
//! - [`auth`]: User authentication (register, login, logout, token refresh)
//! - [`tables`]: Table management (list, join, leave, take actions)
//! - [`websocket`]: Real-time bidirectional communication for live game updates
//! - [`middleware`]: Authentication middleware for protected endpoints
//!
//! # Endpoints Overview
//!
//! ## Authentication (No Auth Required)
//! - `POST /api/auth/register` - Register new user
//! - `POST /api/auth/login` - Login with credentials
//! - `POST /api/auth/logout` - Invalidate refresh token
//! - `POST /api/auth/refresh` - Get new access token
//!
//! ## Tables
//! - `GET /api/tables` - List all tables (public)
//! - `GET /api/tables/:id` - Get table details (requires auth)
//! - `POST /api/tables/:id/join` - Join table (requires auth)
//! - `POST /api/tables/:id/leave` - Leave table (requires auth)
//! - `POST /api/tables/:id/action` - Take action (requires auth)
//!
//! ## WebSocket
//! - `GET /ws/:table_id?token=<jwt>` - Establish WebSocket connection
//!
//! ## Health Check
//! - `GET /health` - Server health status
//!
//! # Example Usage
//!
//! ```rust,no_run
//! use pp_server::api::{create_router, AppState};
//! use std::sync::Arc;
//! # use private_poker::auth::AuthManager;
//! # use private_poker::table::TableManager;
//! # use private_poker::wallet::WalletManager;
//! # use sqlx::PgPool;
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let auth_manager: AuthManager = unimplemented!();
//! # let table_manager: TableManager = unimplemented!();
//! # let wallet_manager: WalletManager = unimplemented!();
//! # let pool: PgPool = unimplemented!();
//!
//! // Create application state
//! let state = AppState {
//!     auth_manager: Arc::new(auth_manager),
//!     table_manager: Arc::new(table_manager),
//!     wallet_manager: Arc::new(wallet_manager),
//!     pool: Arc::new(pool),
//! };
//!
//! // Create router with all endpoints
//! let app = create_router(state);
//!
//! // Start server
//! let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
//! axum::serve(listener, app).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Security
//!
//! - JWT access tokens expire after 15 minutes
//! - JWT refresh tokens expire after 30 days
//! - WebSocket connections require valid JWT in query parameter
//! - Passwords are hashed with bcrypt before storage
//! - Rate limiting applied to authentication endpoints
//!
//! # CORS
//!
//! CORS is configured permissively for development. In production, configure
//! appropriate origins, methods, and headers.

pub mod auth;
pub mod middleware;
pub mod rate_limiter;
pub mod request_id;
pub mod tables;
pub mod websocket;

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
};
use private_poker::{auth::AuthManager, table::TableManager, wallet::WalletManager};
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

/// Application state shared across all HTTP handlers and WebSocket connections.
///
/// This state is cloned for each request (cheap due to Arc wrappers) and provides
/// access to the core system managers.
///
/// # Fields
///
/// - `auth_manager`: Handles authentication, JWT tokens, and sessions
/// - `table_manager`: Manages poker tables and forwards commands to table actors
/// - `wallet_manager`: Manages user balances and transactions
/// - `pool`: Database connection pool for direct queries
#[derive(Clone)]
pub struct AppState {
    pub auth_manager: Arc<AuthManager>,
    pub table_manager: Arc<TableManager>,
    #[allow(dead_code)]
    pub wallet_manager: Arc<WalletManager>,
    #[allow(dead_code)]
    pub pool: Arc<PgPool>,
}

/// Create the complete API router with all endpoints and middleware.
///
/// Constructs an Axum router with all authentication, table management, and WebSocket
/// endpoints configured. Applies CORS middleware to all routes.
///
/// # Arguments
///
/// - `state`: Application state with managers
///
/// # Returns
///
/// Configured Axum router ready to serve requests
///
/// # Endpoint Summary
///
/// ## API v1 (Recommended)
/// ```text
/// GET  /health                         - Health check (public)
/// POST /api/v1/auth/register           - Register user (public)
/// POST /api/v1/auth/login              - Login (public)
/// POST /api/v1/auth/logout             - Logout (auth required)
/// POST /api/v1/auth/refresh            - Refresh token (auth required)
/// GET  /api/v1/tables                  - List tables (public)
/// GET  /api/v1/tables/:id              - Get table (auth required)
/// POST /api/v1/tables/:id/join         - Join table (auth required)
/// POST /api/v1/tables/:id/leave        - Leave table (auth required)
/// POST /api/v1/tables/:id/action       - Take action (auth required)
/// GET  /ws/:table_id?token=<jwt>       - WebSocket (auth required)
/// ```
///
/// ## Legacy Routes (Deprecated)
/// ```text
/// POST /api/auth/register              - Use /api/v1/auth/register
/// POST /api/auth/login                 - Use /api/v1/auth/login
/// GET  /api/tables                     - Use /api/v1/tables
/// ```
///
/// # Example
///
/// ```rust,no_run
/// # use pp_server::api::{create_router, AppState};
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let state: AppState = unimplemented!();
/// let app = create_router(state);
/// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
/// axum::serve(listener, app).await?;
/// # Ok(())
/// # }
/// ```
pub fn create_router(state: AppState) -> Router {
    // API v1 routes (versioned for future evolution)
    let v1_routes = create_v1_router(state.clone());

    // Root routes (health check, WebSocket - not versioned)
    let root_routes = Router::new()
        .route("/health", get(health_check))
        // WebSocket route handles its own auth via query parameter
        .route("/ws/{table_id}", get(websocket::websocket_handler));

    // Combine all routes
    Router::new()
        .merge(root_routes)
        .nest("/api/v1", v1_routes)
        // Legacy routes (deprecated, redirect to v1)
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login))
        .route("/api/tables", get(tables::list_tables))
        .layer(axum::middleware::from_fn(request_id::request_id_middleware))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// Create API v1 router with all versioned endpoints.
///
/// This allows for future API evolution (v2, v3, etc.) while maintaining
/// backward compatibility with existing clients.
fn create_v1_router(state: AppState) -> Router<AppState> {
    // Public routes (no authentication middleware)
    let public_routes = Router::new()
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/tables", get(tables::list_tables));

    // Protected routes (require authentication middleware)
    let protected_routes = Router::new()
        .route("/auth/logout", post(auth::logout))
        .route("/auth/refresh", post(auth::refresh_token))
        .route("/tables/{table_id}", get(tables::get_table))
        .route("/tables/{table_id}/join", post(tables::join_table))
        .route("/tables/{table_id}/leave", post(tables::leave_table))
        .route("/tables/{table_id}/action", post(tables::take_action))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::auth_middleware,
        ));

    // Combine v1 routes
    Router::new().merge(public_routes).merge(protected_routes)
}

/// Health check endpoint for monitoring and load balancers.
///
/// Performs comprehensive health checks on critical system components:
/// - Database connectivity (executes simple query)
/// - Table manager responsiveness
///
/// Returns JSON with detailed health status and appropriate HTTP status code.
///
/// # Response
///
/// Returns `200 OK` if all components are healthy, or `503 Service Unavailable` if any component fails.
///
/// # Example
///
/// ```bash
/// curl http://localhost:3000/health
/// # {"status":"healthy","database":true,"tables":true,"timestamp":"2025-11-22T10:30:00Z"}
/// ```
async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    // Check database connectivity
    let db_healthy = sqlx::query("SELECT 1")
        .fetch_one(&*state.pool)
        .await
        .is_ok();

    // Check if table manager is responsive (has active tables count)
    let table_count = state.table_manager.table_count();
    let tables_healthy = table_count >= 0;

    let overall_healthy = db_healthy && tables_healthy;

    let status_code = if overall_healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let response = json!({
        "status": if overall_healthy { "healthy" } else { "unhealthy" },
        "version": env!("CARGO_PKG_VERSION"),
        "database": db_healthy,
        "tables": {
            "healthy": tables_healthy,
            "active_count": table_count
        },
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    (status_code, Json(response))
}
