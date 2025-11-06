//! Integration tests for wallet system.
//!
//! Tests wallet creation, balance management, faucet claims, escrow operations,
//! and ledger integrity.

use private_poker::auth::AuthManager;
use private_poker::db::{Database, DatabaseConfig};
use private_poker::wallet::{WalletManager, WalletError};
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

/// Helper to create test wallet manager
async fn setup_wallet_manager() -> WalletManager {
    let pool = setup_test_db().await;
    WalletManager::new(pool, 1000) // 1000 chips default balance
}

/// Helper to create test user
async fn create_test_user(pool: &PgPool, username: &str) -> i64 {
    let auth = AuthManager::new(Arc::new(pool.clone()), "test_secret".to_string());
    auth.register(username.to_string(), "Password123!".to_string(), None)
        .await
        .expect("User creation should succeed")
}

/// Helper to cleanup test user
async fn cleanup_user(pool: &PgPool, username: &str) {
    let auth = AuthManager::new(Arc::new(pool.clone()), "test_secret".to_string());
    let _ = auth.delete_user_by_username(username).await;
}

#[tokio::test]
async fn test_create_wallet() {
    let wallet_mgr = setup_wallet_manager().await;
    let pool = setup_test_db().await;
    let username = "test_wallet_user";
    cleanup_user(&pool, username).await;

    let user_id = create_test_user(&pool, username).await;

    // Create wallet
    let result = wallet_mgr.create_wallet(user_id).await;
    assert!(result.is_ok(), "Wallet creation should succeed");

    // Get balance
    let balance = wallet_mgr.get_balance(user_id).await.expect("Should get balance");
    assert_eq!(balance, 1000, "Default balance should be 1000");

    cleanup_user(&pool, username).await;
}

#[tokio::test]
async fn test_claim_faucet() {
    let wallet_mgr = setup_wallet_manager().await;
    let pool = setup_test_db().await;
    let username = "test_faucet_user";
    cleanup_user(&pool, username).await;

    let user_id = create_test_user(&pool, username).await;
    wallet_mgr.create_wallet(user_id).await.expect("Wallet creation should succeed");

    // Set balance to 0 to allow faucet claim
    wallet_mgr
        .update_balance(user_id, -1000, "test_setup".to_string())
        .await
        .expect("Should update balance");

    // Claim faucet
    let result = wallet_mgr.claim_faucet(user_id).await;
    assert!(result.is_ok(), "Faucet claim should succeed");

    let (amount, _next_claim) = result.unwrap();
    assert!(amount > 0, "Faucet amount should be positive");

    // Check balance increased
    let balance = wallet_mgr.get_balance(user_id).await.expect("Should get balance");
    assert_eq!(balance, amount, "Balance should equal faucet amount");

    cleanup_user(&pool, username).await;
}

#[tokio::test]
async fn test_faucet_cooldown() {
    let wallet_mgr = setup_wallet_manager().await;
    let pool = setup_test_db().await;
    let username = "test_faucet_cooldown";
    cleanup_user(&pool, username).await;

    let user_id = create_test_user(&pool, username).await;
    wallet_mgr.create_wallet(user_id).await.expect("Wallet creation should succeed");

    // Set balance to 0
    wallet_mgr
        .update_balance(user_id, -1000, "test_setup".to_string())
        .await
        .expect("Should update balance");

    // First claim
    wallet_mgr.claim_faucet(user_id).await.expect("First claim should succeed");

    // Set balance to 0 again
    let balance = wallet_mgr.get_balance(user_id).await.unwrap();
    wallet_mgr
        .update_balance(user_id, -balance, "test_setup".to_string())
        .await
        .expect("Should update balance");

    // Second claim (should fail due to cooldown)
    let result = wallet_mgr.claim_faucet(user_id).await;
    assert!(result.is_err(), "Second claim should fail due to cooldown");

    cleanup_user(&pool, username).await;
}

#[tokio::test]
async fn test_balance_operations() {
    let wallet_mgr = setup_wallet_manager().await;
    let pool = setup_test_db().await;
    let username = "test_balance_ops";
    cleanup_user(&pool, username).await;

    let user_id = create_test_user(&pool, username).await;
    wallet_mgr.create_wallet(user_id).await.expect("Wallet creation should succeed");

    let initial_balance = wallet_mgr.get_balance(user_id).await.expect("Should get balance");

    // Add chips
    wallet_mgr
        .update_balance(user_id, 500, "bonus".to_string())
        .await
        .expect("Should add chips");

    let balance_after_add = wallet_mgr.get_balance(user_id).await.expect("Should get balance");
    assert_eq!(balance_after_add, initial_balance + 500);

    // Remove chips
    wallet_mgr
        .update_balance(user_id, -200, "deduction".to_string())
        .await
        .expect("Should remove chips");

    let balance_after_remove = wallet_mgr.get_balance(user_id).await.expect("Should get balance");
    assert_eq!(balance_after_remove, initial_balance + 500 - 200);

    cleanup_user(&pool, username).await;
}

#[tokio::test]
async fn test_insufficient_balance() {
    let wallet_mgr = setup_wallet_manager().await;
    let pool = setup_test_db().await;
    let username = "test_insufficient_balance";
    cleanup_user(&pool, username).await;

    let user_id = create_test_user(&pool, username).await;
    wallet_mgr.create_wallet(user_id).await.expect("Wallet creation should succeed");

    let balance = wallet_mgr.get_balance(user_id).await.expect("Should get balance");

    // Try to remove more chips than available
    let result = wallet_mgr
        .update_balance(user_id, -(balance + 100), "overdraft".to_string())
        .await;

    assert!(result.is_err(), "Should fail with insufficient balance");
    assert!(
        matches!(result.unwrap_err(), WalletError::InsufficientBalance),
        "Should return InsufficientBalance error"
    );

    cleanup_user(&pool, username).await;
}

#[tokio::test]
async fn test_transaction_history() {
    let wallet_mgr = setup_wallet_manager().await;
    let pool = setup_test_db().await;
    let username = "test_transaction_history";
    cleanup_user(&pool, username).await;

    let user_id = create_test_user(&pool, username).await;
    wallet_mgr.create_wallet(user_id).await.expect("Wallet creation should succeed");

    // Perform several transactions
    wallet_mgr
        .update_balance(user_id, 100, "bonus1".to_string())
        .await
        .expect("Should add chips");

    wallet_mgr
        .update_balance(user_id, -50, "deduction1".to_string())
        .await
        .expect("Should remove chips");

    wallet_mgr
        .update_balance(user_id, 200, "bonus2".to_string())
        .await
        .expect("Should add chips");

    // Get transaction history
    let history = wallet_mgr
        .get_transaction_history(user_id, 100, 0)
        .await
        .expect("Should get history");

    // Should have at least 4 entries: initial wallet creation + 3 transactions
    assert!(history.len() >= 4, "Should have multiple transactions");

    cleanup_user(&pool, username).await;
}

#[tokio::test]
async fn test_concurrent_balance_updates() {
    let wallet_mgr = Arc::new(setup_wallet_manager().await);
    let pool = setup_test_db().await;
    let username = "test_concurrent_balance";
    cleanup_user(&pool, username).await;

    let user_id = create_test_user(&pool, username).await;
    wallet_mgr.create_wallet(user_id).await.expect("Wallet creation should succeed");

    let initial_balance = wallet_mgr.get_balance(user_id).await.expect("Should get balance");

    let mut handles = vec![];

    // Spawn 10 concurrent add operations
    for i in 0..10 {
        let wallet_clone = Arc::clone(&wallet_mgr);
        let handle = tokio::spawn(async move {
            wallet_clone
                .update_balance(user_id, 10, format!("concurrent_{}", i))
                .await
        });
        handles.push(handle);
    }

    // Wait for all operations
    for handle in handles {
        handle.await.unwrap().expect("Balance update should succeed");
    }

    // Final balance should be initial + 100 (10 * 10)
    let final_balance = wallet_mgr.get_balance(user_id).await.expect("Should get balance");
    assert_eq!(final_balance, initial_balance + 100, "Concurrent updates should be atomic");

    cleanup_user(&pool, username).await;
}

#[tokio::test]
async fn test_idempotency_key() {
    let wallet_mgr = setup_wallet_manager().await;
    let pool = setup_test_db().await;
    let username = "test_idempotency";
    cleanup_user(&pool, username).await;

    let user_id = create_test_user(&pool, username).await;
    wallet_mgr.create_wallet(user_id).await.expect("Wallet creation should succeed");

    let initial_balance = wallet_mgr.get_balance(user_id).await.expect("Should get balance");

    let idempotency_key = "unique_transaction_123".to_string();

    // First transaction
    wallet_mgr
        .update_balance(user_id, 100, idempotency_key.clone())
        .await
        .expect("First transaction should succeed");

    let balance_after_first = wallet_mgr.get_balance(user_id).await.expect("Should get balance");
    assert_eq!(balance_after_first, initial_balance + 100);

    // Second transaction with same idempotency key
    wallet_mgr
        .update_balance(user_id, 100, idempotency_key.clone())
        .await
        .expect("Idempotent transaction should succeed");

    let balance_after_second = wallet_mgr.get_balance(user_id).await.expect("Should get balance");

    // Balance should not change (idempotency)
    assert_eq!(
        balance_after_second, balance_after_first,
        "Idempotent transaction should not change balance"
    );

    cleanup_user(&pool, username).await;
}

#[tokio::test]
async fn test_negative_balance_prevention() {
    let wallet_mgr = setup_wallet_manager().await;
    let pool = setup_test_db().await;
    let username = "test_negative_balance";
    cleanup_user(&pool, username).await;

    let user_id = create_test_user(&pool, username).await;
    wallet_mgr.create_wallet(user_id).await.expect("Wallet creation should succeed");

    let balance = wallet_mgr.get_balance(user_id).await.expect("Should get balance");

    // Try to create negative balance
    let result = wallet_mgr
        .update_balance(user_id, -(balance + 1), "overdraft_attempt".to_string())
        .await;

    assert!(result.is_err(), "Negative balance should be prevented");

    // Balance should remain unchanged
    let final_balance = wallet_mgr.get_balance(user_id).await.expect("Should get balance");
    assert_eq!(final_balance, balance, "Balance should not change on failed transaction");

    cleanup_user(&pool, username).await;
}

#[tokio::test]
async fn test_wallet_not_found() {
    let wallet_mgr = setup_wallet_manager().await;

    // Try to get balance for nonexistent user
    let result = wallet_mgr.get_balance(999999).await;

    assert!(result.is_err(), "Should fail for nonexistent wallet");
    assert!(
        matches!(result.unwrap_err(), WalletError::WalletNotFound),
        "Should return WalletNotFound error"
    );
}
