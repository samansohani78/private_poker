//! Authentication module providing user registration, login, and session management.
//!
//! This module implements secure authentication with:
//! - Argon2id password hashing with server-side pepper
//! - JWT access tokens (15-minute expiry)
//! - Rotating refresh tokens (7-day expiry)
//! - Two-factor authentication (TOTP)
//! - Device fingerprinting for session security
//!
//! ## Example
//!
//! ```no_run
//! use private_poker::auth::{AuthManager, RegisterRequest};
//! use private_poker::db::Database;
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let db = Database::new(&Default::default()).await?;
//!     let auth = AuthManager::new(
//!         Arc::new(db.pool().clone()),
//!         "secret_pepper".to_string(),
//!         "jwt_secret".to_string()
//!     );
//!
//!     let request = RegisterRequest {
//!         username: "player1".to_string(),
//!         password: "SecurePass123".to_string(),
//!         display_name: "Player One".to_string(),
//!         email: Some("player@example.com".to_string()),
//!     };
//!
//!     let user = auth.register(request).await?;
//!     println!("Registered user: {}", user.username);
//!     Ok(())
//! }
//! ```

pub mod errors;
pub mod manager;
pub mod models;

pub use errors::{AuthError, AuthResult};
pub use manager::AuthManager;
pub use models::{
    AccessTokenClaims, LoginRequest, PasswordResetConfirm, PasswordResetRequest, RegisterRequest,
    Session, SessionTokens, TwoFactorSetup, User, UserId,
};
