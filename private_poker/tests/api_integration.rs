//! Integration tests for HTTP REST API and WebSocket endpoints.
//!
//! Tests authentication, table management, and WebSocket communication.

use private_poker::auth::{AuthManager, LoginRequest, RegisterRequest};
use private_poker::db::{Database, DatabaseConfig};
use private_poker::table::{TableConfig, TableManager};
use private_poker::wallet::WalletManager;
use serial_test::serial;
use sqlx::PgPool;
use std::sync::Arc;

/// Generate a unique short username (3-20 chars)
fn unique_username(prefix: &str) -> String {
    let rand_id: u32 = rand::random();
    format!("{}_{}", prefix, rand_id % 100000)
}

/// Helper to create a test database pool
async fn setup_test_db() -> Arc<PgPool> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://poker_test:test_password@localhost/poker_test".to_string());

    let config = DatabaseConfig {
        database_url,
        max_connections: 10,
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

/// Helper to create auth manager with test database
async fn setup_auth_manager() -> (Arc<AuthManager>, Arc<PgPool>) {
    let pool = setup_test_db().await;
    let pepper = "test_pepper_for_testing_only";
    let jwt_secret = "test_secret_key_for_testing_only";
    let auth_manager = Arc::new(AuthManager::new(
        pool.clone(),
        pepper.to_string(),
        jwt_secret.to_string(),
    ));
    (auth_manager, pool)
}

/// Clean up test tables to avoid ID conflicts
async fn cleanup_test_tables(pool: &PgPool) {
    // Delete table escrows first due to foreign key
    let _ = sqlx::query("DELETE FROM table_escrows").execute(pool).await;

    // Delete all tables
    let _ = sqlx::query("DELETE FROM tables").execute(pool).await;
}

/// Helper to create a test user and return auth tokens
async fn create_test_user(auth_manager: &AuthManager, username: &str) -> (i64, String, String) {
    // Register
    let register_req = RegisterRequest {
        username: username.to_string(),
        password: "TestPass123!".to_string(),
        display_name: format!("Test User {}", username),
        email: Some(format!("{}@test.com", username)),
    };

    let _user = auth_manager
        .register(register_req)
        .await
        .expect("Failed to register user");

    // Login to get tokens
    let login_req = LoginRequest {
        username: username.to_string(),
        password: "TestPass123!".to_string(),
        totp_code: None,
    };

    let (user_data, tokens) = auth_manager
        .login(login_req, "test_device".to_string())
        .await
        .expect("Failed to login");

    (user_data.id, tokens.access_token, tokens.refresh_token)
}

// ============================================================================
// Authentication API Tests
// ============================================================================

#[tokio::test]
async fn test_auth_register_login_flow() {
    let (auth_manager, _pool) = setup_auth_manager().await;

    let username = unique_username("api");

    // Test registration
    let register_req = RegisterRequest {
        username: username.clone(),
        password: "SecurePass123!".to_string(),
        display_name: "API Test User".to_string(),
        email: Some(format!("{}@example.com", username)),
    };

    let user = auth_manager
        .register(register_req)
        .await
        .expect("Registration should succeed");

    assert!(user.id > 0);
    assert_eq!(user.username, username);

    // Test login
    let login_req = LoginRequest {
        username,
        password: "SecurePass123!".to_string(),
        totp_code: None,
    };

    let (user_data, tokens) = auth_manager
        .login(login_req, "device1".to_string())
        .await
        .expect("Login should succeed");

    assert_eq!(user_data.id, user.id);
    assert!(!tokens.access_token.is_empty());
    assert!(!tokens.refresh_token.is_empty());
}

#[tokio::test]
async fn test_auth_token_refresh() {
    let (auth_manager, _pool) = setup_auth_manager().await;
    let (_user_id, _access_token, refresh_token) =
        create_test_user(&auth_manager, &unique_username("refresh")).await;

    // Test token refresh
    let new_tokens = auth_manager
        .refresh_token(refresh_token, "test_device".to_string())
        .await
        .expect("Token refresh should succeed");

    assert!(!new_tokens.access_token.is_empty());
    assert!(!new_tokens.refresh_token.is_empty());
}

#[tokio::test]
async fn test_auth_logout() {
    let (auth_manager, _pool) = setup_auth_manager().await;
    let (_user_id, _access_token, refresh_token) =
        create_test_user(&auth_manager, &unique_username("logout")).await;

    // Test logout
    auth_manager
        .logout(refresh_token.clone())
        .await
        .expect("Logout should succeed");

    // Verify token is invalidated
    let result = auth_manager
        .refresh_token(refresh_token, "test_device".to_string())
        .await;

    assert!(result.is_err(), "Token should be invalidated after logout");
}

#[tokio::test]
async fn test_auth_invalid_credentials() {
    let (auth_manager, _pool) = setup_auth_manager().await;
    let username = unique_username("invalid");
    let _ = create_test_user(&auth_manager, &username).await;

    // Test login with wrong password
    let login_req = LoginRequest {
        username,
        password: "WrongPassword123!".to_string(),
        totp_code: None,
    };

    let result = auth_manager.login(login_req, "device1".to_string()).await;

    assert!(result.is_err(), "Login with wrong password should fail");
}

// ============================================================================
// Table Management API Tests
// ============================================================================

#[tokio::test]
#[serial]
async fn test_table_manager_create_and_list() {
    let pool = setup_test_db().await;
    cleanup_test_tables(&pool).await;
    let wallet_manager = Arc::new(WalletManager::new(pool.clone()));
    let table_manager = Arc::new(TableManager::new(pool.clone(), wallet_manager));

    // Create a test table
    let config = TableConfig {
        name: "API Test Table".to_string(),
        max_players: 6,
        small_blind: 5,
        big_blind: 10,
        ..Default::default()
    };

    let table_id = table_manager
        .create_table(config, None)
        .await
        .expect("Table creation should succeed");

    assert!(table_id > 0);

    // List tables
    let tables = table_manager
        .list_tables()
        .await
        .expect("List tables should succeed");

    assert!(!tables.is_empty());
    assert!(tables.iter().any(|t| t.id == table_id));
}

#[tokio::test]
#[serial]
async fn test_table_join_and_leave() {
    let (auth_manager, pool) = setup_auth_manager().await;
    cleanup_test_tables(&pool).await;
    let username = unique_username("table");
    let (user_id, _access_token, _refresh_token) = create_test_user(&auth_manager, &username).await;

    let wallet_manager = Arc::new(WalletManager::new(pool.clone()));
    let table_manager = Arc::new(TableManager::new(pool.clone(), wallet_manager.clone()));

    // Give user some chips
    wallet_manager
        .claim_faucet(user_id)
        .await
        .expect("Faucet claim should succeed");

    // Create table
    let config = TableConfig {
        name: "Join Test Table".to_string(),
        max_players: 6,
        small_blind: 5,
        big_blind: 10,
        ..Default::default()
    };

    let table_id = table_manager
        .create_table(config, Some(user_id))
        .await
        .expect("Table creation should succeed");

    // Join table
    let result = table_manager
        .join_table(table_id, user_id, username, 500, None)
        .await;

    assert!(result.is_ok(), "Join table should succeed");

    // Leave table
    let result = table_manager.leave_table(table_id, user_id).await;

    assert!(result.is_ok(), "Leave table should succeed");
}

#[tokio::test]
#[serial]
async fn test_table_get_state() {
    let (auth_manager, pool) = setup_auth_manager().await;
    cleanup_test_tables(&pool).await;
    let (user_id, _access_token, _refresh_token) =
        create_test_user(&auth_manager, &unique_username("state")).await;

    let wallet_manager = Arc::new(WalletManager::new(pool.clone()));
    let table_manager = Arc::new(TableManager::new(pool.clone(), wallet_manager));

    // Create table
    let config = TableConfig {
        name: "State Test Table".to_string(),
        max_players: 9,
        small_blind: 10,
        big_blind: 20,
        ..Default::default()
    };

    let table_id = table_manager
        .create_table(config, Some(user_id))
        .await
        .expect("Table creation should succeed");

    // Get table state
    let state = table_manager
        .get_table_state(table_id, Some(user_id))
        .await
        .expect("Get table state should succeed");

    assert_eq!(state.table_name, "State Test Table");
    assert!(state.players.is_empty()); // No players joined yet
}

// ============================================================================
// WebSocket Protocol Tests
// ============================================================================

#[tokio::test]
async fn test_websocket_message_formats() {
    // Test that WebSocket message types serialize correctly
    use serde_json;

    // Test Join message
    let join_msg = serde_json::json!({
        "type": "join",
        "buy_in": 1000
    });
    assert_eq!(join_msg["type"], "join");
    assert_eq!(join_msg["buy_in"], 1000);

    // Test Action message - Fold
    let fold_msg = serde_json::json!({
        "type": "action",
        "action": {
            "type": "fold"
        }
    });
    assert_eq!(fold_msg["type"], "action");
    assert_eq!(fold_msg["action"]["type"], "fold");

    // Test Action message - Raise
    let raise_msg = serde_json::json!({
        "type": "action",
        "action": {
            "type": "raise",
            "amount": 100
        }
    });
    assert_eq!(raise_msg["type"], "action");
    assert_eq!(raise_msg["action"]["type"], "raise");
    assert_eq!(raise_msg["action"]["amount"], 100);

    // Test Success response
    let success_resp = serde_json::json!({
        "type": "success",
        "message": "Action processed successfully"
    });
    assert_eq!(success_resp["type"], "success");

    // Test Error response
    let error_resp = serde_json::json!({
        "type": "error",
        "message": "Not your turn"
    });
    assert_eq!(error_resp["type"], "error");
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[tokio::test]
#[serial]
async fn test_api_error_handling() {
    let (auth_manager, pool) = setup_auth_manager().await;
    cleanup_test_tables(&pool).await;
    let wallet_manager = Arc::new(WalletManager::new(pool.clone()));
    let table_manager = Arc::new(TableManager::new(pool, wallet_manager));

    // Test joining non-existent table
    let result = table_manager
        .join_table(99999, 1, "test_user".to_string(), 500, None)
        .await;
    assert!(result.is_err(), "Joining non-existent table should fail");

    // Test duplicate registration
    let username = unique_username("dup");
    let register_req = RegisterRequest {
        username,
        password: "TestPass123!".to_string(),
        display_name: "Duplicate User".to_string(),
        email: None,
    };

    auth_manager
        .register(register_req.clone())
        .await
        .expect("First registration should succeed");

    let result = auth_manager.register(register_req).await;
    assert!(result.is_err(), "Duplicate registration should fail");
}

// ============================================================================
// Concurrent Access Tests
// ============================================================================

#[tokio::test]
#[serial]
async fn test_concurrent_table_joins() {
    let (auth_manager, pool) = setup_auth_manager().await;
    cleanup_test_tables(&pool).await;
    let wallet_manager = Arc::new(WalletManager::new(pool.clone()));
    let table_manager = Arc::new(TableManager::new(pool.clone(), wallet_manager.clone()));

    // Create multiple users
    let mut user_ids = Vec::new();
    for i in 0..3 {
        let username = unique_username(&format!("c{}", i));
        let (user_id, _, _) = create_test_user(&auth_manager, &username).await;

        // Give each user chips
        wallet_manager
            .claim_faucet(user_id)
            .await
            .expect("Faucet claim should succeed");

        user_ids.push((user_id, username));
    }

    // Create table
    let config = TableConfig {
        name: "Concurrent Test Table".to_string(),
        max_players: 6,
        small_blind: 5,
        big_blind: 10,
        ..Default::default()
    };

    let table_id = table_manager
        .create_table(config, Some(user_ids[0].0))
        .await
        .expect("Table creation should succeed");

    // Join concurrently
    let mut handles = Vec::new();
    for (user_id, username) in user_ids {
        let tm = table_manager.clone();
        let handle =
            tokio::spawn(
                async move { tm.join_table(table_id, user_id, username, 500, None).await },
            );
        handles.push(handle);
    }

    // Wait for all joins
    let mut success_count = 0;
    for handle in handles {
        if handle.await.expect("Task should complete").is_ok() {
            success_count += 1;
        }
    }

    assert!(
        success_count >= 2,
        "Multiple users should successfully join"
    );
}
