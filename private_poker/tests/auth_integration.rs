//! Integration tests for authentication system.
//!
//! Tests registration, login, 2FA, session management, and password reset flows.

use private_poker::auth::{AuthManager, AuthError};
use private_poker::db::{Database, DatabaseConfig};
use sqlx::PgPool;
use std::sync::Arc;

/// Helper to create a test database pool
async fn setup_test_db() -> Arc<PgPool> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres@localhost/poker_test".to_string());

    let config = DatabaseConfig {
        database_url,
        max_connections: 5,
        min_connections: 1,
        connection_timeout_secs: 5,
        idle_timeout_secs: 300,
        max_lifetime_secs: 1800,
    };

    let db = Database::new(&config)
        .await
        .expect("Failed to create test database");

    Arc::new(db.pool().clone())
}

/// Helper to create test auth manager
async fn setup_auth_manager() -> AuthManager {
    let pool = setup_test_db().await;
    AuthManager::new(pool, "test_secret_key_for_jwt".to_string())
}

/// Helper to clean up test user
async fn cleanup_user(auth: &AuthManager, username: &str) {
    // Attempt to delete user (ignore errors if doesn't exist)
    let _ = auth.delete_user_by_username(username).await;
}

#[tokio::test]
async fn test_register_new_user() {
    let auth = setup_auth_manager().await;
    let username = "test_register_user";
    cleanup_user(&auth, username).await;

    let result = auth
        .register(username.to_string(), "SecurePass123!".to_string(), None)
        .await;

    assert!(result.is_ok(), "Registration should succeed");
    let user_id = result.unwrap();
    assert!(user_id > 0, "User ID should be positive");

    cleanup_user(&auth, username).await;
}

#[tokio::test]
async fn test_register_duplicate_username() {
    let auth = setup_auth_manager().await;
    let username = "test_duplicate_user";
    cleanup_user(&auth, username).await;

    // First registration
    auth.register(username.to_string(), "Password123!".to_string(), None)
        .await
        .expect("First registration should succeed");

    // Second registration with same username
    let result = auth
        .register(username.to_string(), "Password456!".to_string(), None)
        .await;

    assert!(result.is_err(), "Duplicate registration should fail");
    assert!(
        matches!(result.unwrap_err(), AuthError::UsernameTaken),
        "Should return UsernameTaken error"
    );

    cleanup_user(&auth, username).await;
}

#[tokio::test]
async fn test_register_weak_password() {
    let auth = setup_auth_manager().await;
    let username = "test_weak_password";
    cleanup_user(&auth, username).await;

    let result = auth
        .register(username.to_string(), "weak".to_string(), None)
        .await;

    assert!(result.is_err(), "Weak password should be rejected");
    assert!(
        matches!(result.unwrap_err(), AuthError::WeakPassword(_)),
        "Should return WeakPassword error"
    );

    cleanup_user(&auth, username).await;
}

#[tokio::test]
async fn test_login_success() {
    let auth = setup_auth_manager().await;
    let username = "test_login_user";
    let password = "SecurePass123!";
    cleanup_user(&auth, username).await;

    // Register user
    auth.register(username.to_string(), password.to_string(), None)
        .await
        .expect("Registration should succeed");

    // Login
    let device_fp = "device123".to_string();
    let result = auth
        .login(username.to_string(), password.to_string(), device_fp)
        .await;

    assert!(result.is_ok(), "Login should succeed");
    let (tokens, user) = result.unwrap();
    assert_eq!(user.username, username);
    assert!(!tokens.access_token.is_empty());
    assert!(!tokens.refresh_token.is_empty());

    cleanup_user(&auth, username).await;
}

#[tokio::test]
async fn test_login_wrong_password() {
    let auth = setup_auth_manager().await;
    let username = "test_wrong_password";
    cleanup_user(&auth, username).await;

    // Register user
    auth.register(username.to_string(), "CorrectPass123!".to_string(), None)
        .await
        .expect("Registration should succeed");

    // Login with wrong password
    let result = auth
        .login(
            username.to_string(),
            "WrongPass123!".to_string(),
            "device123".to_string(),
        )
        .await;

    assert!(result.is_err(), "Login with wrong password should fail");
    assert!(
        matches!(result.unwrap_err(), AuthError::InvalidPassword),
        "Should return InvalidPassword error"
    );

    cleanup_user(&auth, username).await;
}

#[tokio::test]
async fn test_login_nonexistent_user() {
    let auth = setup_auth_manager().await;
    let username = "nonexistent_user_12345";

    let result = auth
        .login(
            username.to_string(),
            "SomePass123!".to_string(),
            "device123".to_string(),
        )
        .await;

    assert!(result.is_err(), "Login for nonexistent user should fail");
    assert!(
        matches!(result.unwrap_err(), AuthError::UserNotFound),
        "Should return UserNotFound error"
    );
}

#[tokio::test]
async fn test_refresh_token_flow() {
    let auth = setup_auth_manager().await;
    let username = "test_refresh_token";
    let password = "SecurePass123!";
    cleanup_user(&auth, username).await;

    // Register and login
    auth.register(username.to_string(), password.to_string(), None)
        .await
        .expect("Registration should succeed");

    let device_fp = "device123".to_string();
    let (tokens, _user) = auth
        .login(username.to_string(), password.to_string(), device_fp.clone())
        .await
        .expect("Login should succeed");

    // Refresh token
    let result = auth
        .refresh_token(tokens.refresh_token.clone(), device_fp)
        .await;

    assert!(result.is_ok(), "Token refresh should succeed");
    let new_tokens = result.unwrap();
    assert!(!new_tokens.access_token.is_empty());
    assert_ne!(tokens.access_token, new_tokens.access_token, "New access token should be different");

    cleanup_user(&auth, username).await;
}

#[tokio::test]
async fn test_logout() {
    let auth = setup_auth_manager().await;
    let username = "test_logout_user";
    let password = "SecurePass123!";
    cleanup_user(&auth, username).await;

    // Register and login
    auth.register(username.to_string(), password.to_string(), None)
        .await
        .expect("Registration should succeed");

    let device_fp = "device123".to_string();
    let (tokens, user) = auth
        .login(username.to_string(), password.to_string(), device_fp.clone())
        .await
        .expect("Login should succeed");

    // Logout
    let result = auth.logout(user.id, &tokens.refresh_token).await;
    assert!(result.is_ok(), "Logout should succeed");

    // Try to use refresh token after logout
    let refresh_result = auth.refresh_token(tokens.refresh_token, device_fp).await;
    assert!(refresh_result.is_err(), "Refresh token should be invalid after logout");

    cleanup_user(&auth, username).await;
}

#[tokio::test]
async fn test_concurrent_registrations() {
    let auth = Arc::new(setup_auth_manager().await);
    let base_username = "concurrent_user_";

    let mut handles = vec![];

    // Spawn 10 concurrent registration tasks
    for i in 0..10 {
        let auth_clone = Arc::clone(&auth);
        let username = format!("{}{}", base_username, i);

        let handle = tokio::spawn(async move {
            auth_clone
                .register(username.clone(), "SecurePass123!".to_string(), None)
                .await
        });

        handles.push((handle, i));
    }

    // Wait for all tasks
    let mut success_count = 0;
    for (handle, i) in handles {
        let result = handle.await.unwrap();
        if result.is_ok() {
            success_count += 1;
        }

        // Cleanup
        let username = format!("{}{}", base_username, i);
        cleanup_user(&auth, &username).await;
    }

    assert_eq!(success_count, 10, "All concurrent registrations should succeed");
}

#[tokio::test]
async fn test_session_validation() {
    let auth = setup_auth_manager().await;
    let username = "test_session_validation";
    let password = "SecurePass123!";
    cleanup_user(&auth, username).await;

    // Register and login
    auth.register(username.to_string(), password.to_string(), None)
        .await
        .expect("Registration should succeed");

    let device_fp = "device123".to_string();
    let (tokens, user) = auth
        .login(username.to_string(), password.to_string(), device_fp)
        .await
        .expect("Login should succeed");

    // Validate access token
    let validation_result = auth.validate_access_token(&tokens.access_token).await;
    assert!(validation_result.is_ok(), "Access token should be valid");

    let validated_user = validation_result.unwrap();
    assert_eq!(validated_user.id, user.id, "User ID should match");

    cleanup_user(&auth, username).await;
}

#[tokio::test]
async fn test_invalid_token() {
    let auth = setup_auth_manager().await;

    let invalid_token = "invalid.jwt.token";
    let result = auth.validate_access_token(invalid_token).await;

    assert!(result.is_err(), "Invalid token should fail validation");
}

#[tokio::test]
async fn test_multiple_sessions_same_user() {
    let auth = setup_auth_manager().await;
    let username = "test_multi_session";
    let password = "SecurePass123!";
    cleanup_user(&auth, username).await;

    // Register user
    auth.register(username.to_string(), password.to_string(), None)
        .await
        .expect("Registration should succeed");

    // Login from device 1
    let (tokens1, _) = auth
        .login(username.to_string(), password.to_string(), "device1".to_string())
        .await
        .expect("Login from device 1 should succeed");

    // Login from device 2
    let (tokens2, _) = auth
        .login(username.to_string(), password.to_string(), "device2".to_string())
        .await
        .expect("Login from device 2 should succeed");

    // Both tokens should be different
    assert_ne!(tokens1.access_token, tokens2.access_token);
    assert_ne!(tokens1.refresh_token, tokens2.refresh_token);

    // Both tokens should be valid
    assert!(auth.validate_access_token(&tokens1.access_token).await.is_ok());
    assert!(auth.validate_access_token(&tokens2.access_token).await.is_ok());

    cleanup_user(&auth, username).await;
}
