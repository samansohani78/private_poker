//! Authentication middleware for protected endpoints.
//!
//! This module provides Axum middleware for JWT-based authentication.
//! The middleware extracts and validates JWT access tokens from the Authorization header,
//! then injects the authenticated user ID into request extensions for downstream handlers.
//!
//! # Usage
//!
//! Apply to protected routes in the router:
//!
//! ```rust,no_run
//! use axum::{Router, routing::get, middleware};
//! # use pp_server::api::middleware::auth_middleware;
//! # use pp_server::api::AppState;
//! # async fn handler() {}
//! # let state: AppState = unimplemented!();
//!
//! let protected_routes: Router = Router::new()
//!     .route("/api/protected", get(handler))
//!     .layer(middleware::from_fn_with_state(state.clone(), auth_middleware));
//! # let _ = protected_routes;
//! ```
//!
//! # Extracting User ID
//!
//! In handler functions, extract the user ID from request extensions:
//!
//! ```rust,no_run
//! use axum::extract::Extension;
//!
//! async fn protected_handler(Extension(user_id): Extension<i64>) -> String {
//!     format!("Authenticated as user {}", user_id)
//! }
//! # let _ = protected_handler;
//! ```

use axum::{
    extract::{Request, State},
    http::{StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::Response,
};

use super::AppState;

/// Authentication middleware that validates JWT tokens and injects user ID.
///
/// Extracts the JWT access token from the `Authorization: Bearer <token>` header,
/// validates it using the AuthManager, and injects the user ID into request extensions.
///
/// # Request Headers
///
/// Expects:
/// ```text
/// Authorization: Bearer eyJhbGciOiJIUzI1NiIs...
/// ```
///
/// # Behavior
///
/// - **Success**: Token valid → Injects `user_id: i64` into request extensions → Calls next handler
/// - **Missing header**: Returns `401 Unauthorized`
/// - **Invalid format**: Returns `401 Unauthorized`
/// - **Invalid/expired token**: Returns `401 Unauthorized`
///
/// # Example
///
/// ```rust,no_run
/// // In router setup
/// use axum::{Router, routing::get, middleware};
/// # use pp_server::api::middleware::auth_middleware;
/// # use pp_server::api::AppState;
/// # async fn list_tables() {}
/// # async fn get_table() {}
/// # let state: AppState = unimplemented!();
///
/// let app: Router = Router::new()
///     .route("/api/tables", get(list_tables))  // Public
///     .route("/api/tables/{id}", get(get_table))
///     .layer(middleware::from_fn_with_state(state, auth_middleware));  // Protected
/// # let _ = app;
/// ```
#[allow(dead_code)]
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract token from Authorization header
    let auth_header = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));

    let token = match auth_header {
        Some(t) => t,
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    // Verify token and get user ID
    match state.auth_manager.verify_access_token(token) {
        Ok(claims) => {
            // Add user_id to request extensions
            request.extensions_mut().insert(claims.sub);
            Ok(next.run(request).await)
        }
        Err(_) => Err(StatusCode::UNAUTHORIZED),
    }
}
