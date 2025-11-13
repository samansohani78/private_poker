//! Authentication API handlers.
//!
//! This module provides HTTP REST endpoints for user authentication including:
//! - User registration with username, password, and optional email
//! - Login with username/password and optional 2FA
//! - Logout to invalidate refresh tokens
//! - Token refresh for obtaining new access tokens
//!
//! All endpoints return JSON responses with either authentication tokens or error messages.
//!
//! # Examples
//!
//! Register a new user:
//! ```bash
//! curl -X POST http://localhost:3000/api/auth/register \
//!   -H "Content-Type: application/json" \
//!   -d '{"username": "player1", "password": "Pass123!", "display_name": "Player One"}'
//! ```
//!
//! Login:
//! ```bash
//! curl -X POST http://localhost:3000/api/auth/login \
//!   -H "Content-Type: application/json" \
//!   -d '{"username": "player1", "password": "Pass123!"}'
//! ```

use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use private_poker::auth::{LoginRequest, RegisterRequest};

use super::AppState;

#[derive(Debug, Deserialize)]
pub struct LoginPayload {
    pub username: String,
    pub password: String,
    pub totp_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterPayload {
    pub username: String,
    pub password: String,
    pub display_name: String,
    pub email: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user_id: i64,
    pub username: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Register a new user account and automatically log them in.
///
/// Creates a new user with the provided credentials and immediately generates
/// authentication tokens for the new account.
///
/// # Request Body
///
/// ```json
/// {
///   "username": "player123",
///   "password": "SecurePass123!",
///   "display_name": "Player One",
///   "email": "player@example.com"  // Optional
/// }
/// ```
///
/// # Response
///
/// On success, returns `200 OK` with authentication tokens:
/// ```json
/// {
///   "access_token": "eyJhbGciOiJIUzI1NiIs...",
///   "refresh_token": "eyJhbGciOiJIUzI1NiIs...",
///   "user_id": 42,
///   "username": "player123"
/// }
/// ```
///
/// # Errors
///
/// - `400 Bad Request`: Username already taken, weak password, or invalid input
/// - `500 Internal Server Error`: Server error during registration or login
///
/// # Security
///
/// - Passwords must meet minimum strength requirements
/// - Usernames must be unique
/// - Passwords are hashed before storage
pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterPayload>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    let request = RegisterRequest {
        username: payload.username.clone(),
        password: payload.password.clone(),
        display_name: payload.display_name,
        email: payload.email,
    };

    match state.auth_manager.register(request).await {
        Ok(_user) => {
            // Login to generate tokens
            let login_request = LoginRequest {
                username: payload.username,
                password: payload.password,
                totp_code: None,
            };

            let device_fp = "web".to_string();

            match state.auth_manager.login(login_request, device_fp).await {
                Ok((user, tokens)) => Ok(Json(AuthResponse {
                    access_token: tokens.access_token,
                    refresh_token: tokens.refresh_token,
                    user_id: user.id,
                    username: user.username,
                })),
                Err(e) => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse { error: e.to_string() }),
                )),
            }
        }
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: e.to_string() }),
        )),
    }
}

/// Authenticate a user and generate session tokens.
///
/// Validates user credentials and returns JWT access and refresh tokens.
/// Access tokens are short-lived (15 minutes) while refresh tokens last longer (30 days).
///
/// # Request Body
///
/// ```json
/// {
///   "username": "player123",
///   "password": "SecurePass123!",
///   "totp_code": null  // Required if 2FA is enabled
/// }
/// ```
///
/// # Response
///
/// On success, returns `200 OK` with tokens:
/// ```json
/// {
///   "access_token": "eyJhbGciOiJIUzI1NiIs...",
///   "refresh_token": "eyJhbGciOiJIUzI1NiIs...",
///   "user_id": 42,
///   "username": "player123"
/// }
/// ```
///
/// # Errors
///
/// - `401 Unauthorized`: Invalid credentials or incorrect 2FA code
///
/// # Security
///
/// - Failed login attempts are rate-limited
/// - Passwords are verified against hashed values
/// - 2FA code is required if enabled for the account
/// - Device fingerprinting is used for session tracking
pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginPayload>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    let request = LoginRequest {
        username: payload.username.clone(),
        password: payload.password,
        totp_code: payload.totp_code,
    };

    let device_fp = "web".to_string();

    match state.auth_manager.login(request, device_fp).await {
        Ok((user, tokens)) => Ok(Json(AuthResponse {
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            user_id: user.id,
            username: user.username,
        })),
        Err(e) => Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse { error: e.to_string() }),
        )),
    }
}

/// Logout and invalidate the current refresh token.
///
/// Terminates the user's session by invalidating their refresh token in the database.
/// The access token will continue to work until it expires naturally (15 minutes).
///
/// # Request Body
///
/// ```json
/// "eyJhbGciOiJIUzI1NiIs..."  // Refresh token string
/// ```
///
/// # Response
///
/// On success, returns `204 No Content` with empty body.
///
/// # Errors
///
/// - `400 Bad Request`: Invalid or already expired token
///
/// # Security
///
/// - Only the specific refresh token is invalidated
/// - Other sessions/devices remain active
/// - Access tokens cannot be invalidated early
pub async fn logout(
    State(state): State<AppState>,
    Json(refresh_token): Json<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    match state.auth_manager.logout(refresh_token).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: e.to_string() }),
        )),
    }
}

/// Refresh an expired access token using a valid refresh token.
///
/// When an access token expires (after 15 minutes), use this endpoint with a valid
/// refresh token to obtain a new access token and refresh token pair.
///
/// # Request Body
///
/// ```json
/// "eyJhbGciOiJIUzI1NiIs..."  // Refresh token string
/// ```
///
/// # Response
///
/// On success, returns `200 OK` with new tokens:
/// ```json
/// {
///   "access_token": "eyJhbGciOiJIUzI1NiIs...",  // New access token
///   "refresh_token": "eyJhbGciOiJIUzI1NiIs...", // New refresh token
///   "user_id": 42,
///   "username": "player123"
/// }
/// ```
///
/// # Errors
///
/// - `401 Unauthorized`: Invalid, expired, or revoked refresh token
/// - `500 Internal Server Error`: Server error during token generation
///
/// # Security
///
/// - Old refresh token is invalidated and replaced with new one
/// - Rotation helps detect token theft
/// - Device fingerprinting must match original login
/// - Refresh tokens expire after 30 days of inactivity
pub async fn refresh_token(
    State(state): State<AppState>,
    Json(old_refresh_token): Json<String>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    let device_fp = "web".to_string();

    match state.auth_manager.refresh_token(old_refresh_token, device_fp).await {
        Ok(tokens) => {
            match state.auth_manager.verify_access_token(&tokens.access_token) {
                Ok(claims) => Ok(Json(AuthResponse {
                    access_token: tokens.access_token,
                    refresh_token: tokens.refresh_token,
                    user_id: claims.sub,
                    username: claims.username,
                })),
                Err(e) => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse { error: e.to_string() }),
                )),
            }
        }
        Err(e) => Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse { error: e.to_string() }),
        )),
    }
}
