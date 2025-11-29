//! Integration tests for authentication system.
//!
//! Tests registration, login, 2FA, session management, and password reset flows.

use private_poker::auth::{AuthError, AuthManager, LoginRequest, RegisterRequest};
use private_poker::db::{Database, DatabaseConfig};
use sqlx::PgPool;
use std::sync::Arc;

/// Helper to create a test database pool
async fn setup_test_db() -> Arc<PgPool> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://poker_test:test_password@localhost/poker_test".to_string());

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

/// Helper to create test auth manager with pool
async fn setup_auth_manager() -> (AuthManager, Arc<PgPool>) {
    let pool = setup_test_db().await;
    let auth = AuthManager::new(
        pool.clone(),
        "test_pepper_key".to_string(),
        "test_jwt_secret".to_string(),
    );
    (auth, pool)
}

/// Helper to clean up test user
async fn cleanup_user(pool: &PgPool, username: &str) {
    let _ = sqlx::query("DELETE FROM users WHERE username = $1")
        .bind(username)
        .execute(pool)
        .await;
}

#[tokio::test]
async fn test_register_new_user() {
    let (auth, pool) = setup_auth_manager().await;
    let username = "test_register_user";
    cleanup_user(pool.as_ref(), username).await;

    let result = auth
        .register(RegisterRequest {
            username: username.to_string(),
            password: "SecurePass123!".to_string(),
            display_name: username.to_string(),
            email: None,
        })
        .await;

    assert!(result.is_ok(), "Registration should succeed");
    let user = result.unwrap();
    assert!(user.id > 0, "User ID should be positive");

    cleanup_user(pool.as_ref(), username).await;
}

#[tokio::test]
async fn test_register_duplicate_username() {
    let (auth, pool) = setup_auth_manager().await;
    let username = "test_duplicate_user";
    cleanup_user(pool.as_ref(), username).await;

    // First registration
    auth.register(RegisterRequest {
        username: username.to_string(),
        password: "Password123!".to_string(),
        display_name: username.to_string(),
        email: None,
    })
    .await
    .expect("First registration should succeed");

    // Second registration with same username
    let result = auth
        .register(RegisterRequest {
            username: username.to_string(),
            password: "Password456!".to_string(),
            display_name: username.to_string(),
            email: None,
        })
        .await;

    assert!(result.is_err(), "Duplicate registration should fail");
    assert!(
        matches!(result.unwrap_err(), AuthError::UsernameTaken),
        "Should return UsernameTaken error"
    );

    cleanup_user(pool.as_ref(), username).await;
}

#[tokio::test]
async fn test_register_weak_password() {
    let (auth, pool) = setup_auth_manager().await;
    let username = "test_weak_password";
    cleanup_user(pool.as_ref(), username).await;

    let result = auth
        .register(RegisterRequest {
            username: username.to_string(),
            password: "weak".to_string(),
            display_name: username.to_string(),
            email: None,
        })
        .await;

    assert!(result.is_err(), "Weak password should be rejected");
    assert!(
        matches!(result.unwrap_err(), AuthError::WeakPassword(_)),
        "Should return WeakPassword error"
    );

    cleanup_user(pool.as_ref(), username).await;
}

#[tokio::test]
async fn test_login_success() {
    let (auth, pool) = setup_auth_manager().await;
    let username = "test_login_user";
    let password = "SecurePass123!";
    cleanup_user(pool.as_ref(), username).await;

    // Register user
    auth.register(RegisterRequest {
        username: username.to_string(),
        password: password.to_string(),
        display_name: username.to_string(),
        email: None,
    })
    .await
    .expect("Registration should succeed");

    // Login
    let device_fp = "device123".to_string();
    let result = auth
        .login(
            LoginRequest {
                username: username.to_string(),
                password: password.to_string(),
                totp_code: None,
            },
            device_fp,
        )
        .await;

    assert!(result.is_ok(), "Login should succeed");
    let (user, tokens) = result.unwrap();
    assert_eq!(user.username, username);
    assert!(!tokens.access_token.is_empty());
    assert!(!tokens.refresh_token.is_empty());

    cleanup_user(pool.as_ref(), username).await;
}

#[tokio::test]
async fn test_login_wrong_password() {
    let (auth, pool) = setup_auth_manager().await;
    let username = "test_wrong_password";
    cleanup_user(pool.as_ref(), username).await;

    // Register user
    auth.register(RegisterRequest {
        username: username.to_string(),
        password: "CorrectPass123!".to_string(),
        display_name: username.to_string(),
        email: None,
    })
    .await
    .expect("Registration should succeed");

    // Login with wrong password
    let result = auth
        .login(
            LoginRequest {
                username: username.to_string(),
                password: "WrongPass123!".to_string(),
                totp_code: None,
            },
            "device123".to_string(),
        )
        .await;

    assert!(result.is_err(), "Login with wrong password should fail");
    assert!(
        matches!(result.unwrap_err(), AuthError::InvalidPassword),
        "Should return InvalidPassword error"
    );

    cleanup_user(pool.as_ref(), username).await;
}

#[tokio::test]
async fn test_login_nonexistent_user() {
    let (auth, _pool) = setup_auth_manager().await;
    let username = "nonexistent_user_12345";

    let result = auth
        .login(
            LoginRequest {
                username: username.to_string(),
                password: "SomePass123!".to_string(),
                totp_code: None,
            },
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
    let (auth, pool) = setup_auth_manager().await;
    let username = "test_refresh_token";
    let password = "SecurePass123!";
    cleanup_user(pool.as_ref(), username).await;

    // Register and login
    auth.register(RegisterRequest {
        username: username.to_string(),
        password: password.to_string(),
        display_name: username.to_string(),
        email: None,
    })
    .await
    .expect("Registration should succeed");

    let device_fp = "device123".to_string();
    let (_user, tokens) = auth
        .login(
            LoginRequest {
                username: username.to_string(),
                password: password.to_string(),
                totp_code: None,
            },
            device_fp.clone(),
        )
        .await
        .expect("Login should succeed");

    // Wait 1 second to ensure different timestamp
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Refresh token
    let result = auth
        .refresh_token(tokens.refresh_token.clone(), device_fp)
        .await;

    assert!(result.is_ok(), "Token refresh should succeed");
    let new_tokens = result.unwrap();
    assert!(!new_tokens.access_token.is_empty());
    assert_ne!(
        tokens.access_token, new_tokens.access_token,
        "New access token should be different"
    );

    cleanup_user(pool.as_ref(), username).await;
}

#[tokio::test]
async fn test_logout() {
    let (auth, pool) = setup_auth_manager().await;
    let username = "test_logout_user";
    let password = "SecurePass123!";
    cleanup_user(pool.as_ref(), username).await;

    // Register and login
    auth.register(RegisterRequest {
        username: username.to_string(),
        password: password.to_string(),
        display_name: username.to_string(),
        email: None,
    })
    .await
    .expect("Registration should succeed");

    let device_fp = "device123".to_string();
    let (_user, tokens) = auth
        .login(
            LoginRequest {
                username: username.to_string(),
                password: password.to_string(),
                totp_code: None,
            },
            device_fp.clone(),
        )
        .await
        .expect("Login should succeed");

    // Logout
    let result = auth.logout(tokens.refresh_token.clone()).await;
    assert!(result.is_ok(), "Logout should succeed");

    // Try to use refresh token after logout
    let refresh_result = auth.refresh_token(tokens.refresh_token, device_fp).await;
    assert!(
        refresh_result.is_err(),
        "Refresh token should be invalid after logout"
    );

    cleanup_user(pool.as_ref(), username).await;
}

#[tokio::test]
async fn test_concurrent_registrations() {
    let (auth_inner, pool) = setup_auth_manager().await;
    let auth = Arc::new(auth_inner);
    let base_username = "concurrent_user_";

    let mut handles = vec![];

    // Spawn 10 concurrent registration tasks
    for i in 0..10 {
        let auth_clone = Arc::clone(&auth);
        let username = format!("{}{}", base_username, i);

        let handle = tokio::spawn(async move {
            auth_clone
                .register(RegisterRequest {
                    username: username.clone(),
                    password: "SecurePass123!".to_string(),
                    display_name: username.clone(),
                    email: None,
                })
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
        cleanup_user(pool.as_ref(), &username).await;
    }

    // At least some registrations should succeed (rate limiting may block some)
    assert!(
        success_count >= 3,
        "At least 3 concurrent registrations should succeed (got {})",
        success_count
    );
}

#[tokio::test]
async fn test_session_validation() {
    let (auth, pool) = setup_auth_manager().await;
    let username = "test_session_valid";
    let password = "SecurePass123!";
    cleanup_user(pool.as_ref(), username).await;

    // Register and login
    auth.register(RegisterRequest {
        username: username.to_string(),
        password: password.to_string(),
        display_name: username.to_string(),
        email: None,
    })
    .await
    .expect("Registration should succeed");

    let device_fp = "device123".to_string();
    let (user, tokens) = auth
        .login(
            LoginRequest {
                username: username.to_string(),
                password: password.to_string(),
                totp_code: None,
            },
            device_fp,
        )
        .await
        .expect("Login should succeed");

    // Verify access token
    let validation_result = auth.verify_access_token(&tokens.access_token);
    assert!(validation_result.is_ok(), "Access token should be valid");

    let claims = validation_result.unwrap();
    assert_eq!(claims.sub, user.id, "User ID should match");

    cleanup_user(pool.as_ref(), username).await;
}

#[tokio::test]
async fn test_invalid_token() {
    let (auth, _pool) = setup_auth_manager().await;

    let invalid_token = "invalid.jwt.token";
    let result = auth.verify_access_token(invalid_token);

    assert!(result.is_err(), "Invalid token should fail validation");
}

#[tokio::test]
async fn test_multiple_sessions_same_user() {
    let (auth, pool) = setup_auth_manager().await;
    let username = "test_multi_session";
    let password = "SecurePass123!";
    cleanup_user(pool.as_ref(), username).await;

    // Register user
    auth.register(RegisterRequest {
        username: username.to_string(),
        password: password.to_string(),
        display_name: username.to_string(),
        email: None,
    })
    .await
    .expect("Registration should succeed");

    // Login from device 1
    let (_user, tokens1) = auth
        .login(
            LoginRequest {
                username: username.to_string(),
                password: password.to_string(),
                totp_code: None,
            },
            "device1".to_string(),
        )
        .await
        .expect("Login from device 1 should succeed");

    // Wait 1 second to ensure different timestamp
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Login from device 2
    let (_user2, tokens2) = auth
        .login(
            LoginRequest {
                username: username.to_string(),
                password: password.to_string(),
                totp_code: None,
            },
            "device2".to_string(),
        )
        .await
        .expect("Login from device 2 should succeed");

    // Both tokens should be different
    assert_ne!(tokens1.access_token, tokens2.access_token);
    assert_ne!(tokens1.refresh_token, tokens2.refresh_token);

    // Both tokens should be valid
    assert!(auth.verify_access_token(&tokens1.access_token).is_ok());
    assert!(auth.verify_access_token(&tokens2.access_token).is_ok());

    cleanup_user(pool.as_ref(), username).await;
}
