//! Authentication error types.

use thiserror::Error;

/// Authentication errors
#[derive(Debug, Error)]
pub enum AuthError {
    /// Database error
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Password hashing failed
    #[error("Password hashing failed")]
    HashingFailed,

    /// Password verification failed
    #[error("Invalid password")]
    InvalidPassword,

    /// User not found
    #[error("User not found")]
    UserNotFound,

    /// Username already exists
    #[error("Username already exists")]
    UsernameTaken,

    /// Email already exists
    #[error("Email already exists")]
    EmailTaken,

    /// Invalid username format
    #[error("Invalid username: {0}")]
    InvalidUsername(String),

    /// Password too weak
    #[error("Password too weak: {0}")]
    WeakPassword(String),

    /// JWT token error
    #[error("JWT error: {0}")]
    JwtError(#[from] jsonwebtoken::errors::Error),

    /// Session not found
    #[error("Session not found")]
    SessionNotFound,

    /// Session expired
    #[error("Session expired")]
    SessionExpired,

    /// Invalid refresh token
    #[error("Invalid refresh token")]
    InvalidRefreshToken,

    /// 2FA required
    #[error("Two-factor authentication required")]
    TwoFactorRequired,

    /// Invalid 2FA code
    #[error("Invalid two-factor authentication code")]
    InvalidTwoFactorCode,

    /// 2FA not enabled
    #[error("Two-factor authentication not enabled")]
    TwoFactorNotEnabled,

    /// Rate limited
    #[error("Too many attempts, please try again later")]
    RateLimited,

    /// Invalid reset code
    #[error("Invalid or expired reset code")]
    InvalidResetCode,
}

impl AuthError {
    /// Get a client-safe error message that doesn't leak sensitive information
    ///
    /// Database and JWT errors are sanitized to prevent information disclosure
    /// about the internal system structure.
    pub fn client_message(&self) -> String {
        match self {
            // Sanitize database errors - don't expose SQL details
            AuthError::Database(_) => "Internal server error".to_string(),
            // Sanitize JWT errors - don't expose token structure
            AuthError::JwtError(_) => "Authentication failed".to_string(),
            // All other errors are safe to expose
            _ => self.to_string(),
        }
    }
}

/// Result type for authentication operations
pub type AuthResult<T> = Result<T, AuthError>;
