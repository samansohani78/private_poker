//! Integration tests for wallet system with escrow-based ledger.
//!
//! Tests wallet creation, balance management, faucet claims, escrow operations,
//! and ledger integrity using the new escrow-based transfer system.

use private_poker::auth::{AuthManager, RegisterRequest};
use private_poker::db::{Database, DatabaseConfig};
use private_poker::wallet::WalletManager;
use sqlx::PgPool;
use std::sync::Arc;

/// Generate unique idempotency key
fn unique_key(prefix: &str) -> String {
    format!(
        "{}_{}",
        prefix,
        chrono::Utc::now().timestamp_nanos_opt().unwrap()
    )
}

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

/// Helper to create test wallet manager and auth manager
async fn setup_managers() -> (WalletManager, AuthManager, Arc<PgPool>) {
    let pool = setup_test_db().await;
    let wallet_mgr = WalletManager::new(pool.clone());
    let auth_mgr = AuthManager::new(
        pool.clone(),
        "test_pepper".to_string(),
        "test_jwt_secret".to_string(),
    );
    (wallet_mgr, auth_mgr, pool)
}

/// Helper to cleanup test user
async fn cleanup_user(pool: &PgPool, username: &str) {
    let _ = sqlx::query("DELETE FROM users WHERE username = $1")
        .bind(username)
        .execute(pool)
        .await;
}

/// Helper to cleanup test table escrow
async fn cleanup_table_escrow(pool: &PgPool, table_id: i64) {
    let _ = sqlx::query("DELETE FROM table_escrows WHERE table_id = $1")
        .bind(table_id)
        .execute(pool)
        .await;
}

#[tokio::test]
async fn test_get_wallet_auto_created() {
    let (wallet_mgr, auth_mgr, pool) = setup_managers().await;
    let username = "test_wallet_auto";
    cleanup_user(&pool, username).await;

    // Register user (wallet should be auto-created)
    let user = auth_mgr
        .register(RegisterRequest {
            username: username.to_string(),
            password: "SecurePass123!".to_string(),
            display_name: username.to_string(),
            email: None,
        })
        .await
        .expect("Registration should succeed");

    // Get wallet
    let wallet = wallet_mgr
        .get_wallet(user.id)
        .await
        .expect("Should get wallet");

    assert_eq!(wallet.user_id, user.id);
    assert!(wallet.balance >= 0, "Balance should be non-negative");

    cleanup_user(&pool, username).await;
}

#[tokio::test]
async fn test_claim_faucet() {
    let (wallet_mgr, auth_mgr, pool) = setup_managers().await;
    let username = "test_faucet";
    cleanup_user(&pool, username).await;

    // Register user
    let user = auth_mgr
        .register(RegisterRequest {
            username: username.to_string(),
            password: "SecurePass123!".to_string(),
            display_name: username.to_string(),
            email: None,
        })
        .await
        .expect("Registration should succeed");

    // Claim faucet
    let result = wallet_mgr.claim_faucet(user.id).await;
    assert!(
        result.is_ok(),
        "Faucet claim should succeed: {:?}",
        result.err()
    );

    let claim = result.unwrap();
    assert!(claim.amount > 0, "Faucet amount should be positive");

    // Check balance increased
    let wallet = wallet_mgr
        .get_wallet(user.id)
        .await
        .expect("Should get wallet");
    assert!(
        wallet.balance >= claim.amount,
        "Balance should include faucet amount"
    );

    cleanup_user(&pool, username).await;
}

#[tokio::test]
async fn test_faucet_cooldown() {
    let (wallet_mgr, auth_mgr, pool) = setup_managers().await;
    let username = "test_faucet_cooldown";
    cleanup_user(&pool, username).await;

    // Register user
    let user = auth_mgr
        .register(RegisterRequest {
            username: username.to_string(),
            password: "SecurePass123!".to_string(),
            display_name: username.to_string(),
            email: None,
        })
        .await
        .expect("Registration should succeed");

    // First claim
    wallet_mgr
        .claim_faucet(user.id)
        .await
        .expect("First faucet claim should succeed");

    // Second claim immediately should fail
    let result = wallet_mgr.claim_faucet(user.id).await;
    assert!(
        result.is_err(),
        "Second faucet claim should fail due to cooldown"
    );

    cleanup_user(&pool, username).await;
}

#[tokio::test]
async fn test_transfer_to_escrow() {
    let (wallet_mgr, auth_mgr, pool) = setup_managers().await;
    let username = "test_escrow_to";
    let table_id = 999;
    cleanup_user(&pool, username).await;
    cleanup_table_escrow(&pool, table_id).await;

    // Register user and claim faucet
    let user = auth_mgr
        .register(RegisterRequest {
            username: username.to_string(),
            password: "SecurePass123!".to_string(),
            display_name: username.to_string(),
            email: None,
        })
        .await
        .expect("Registration should succeed");

    wallet_mgr
        .claim_faucet(user.id)
        .await
        .expect("Faucet claim should succeed");

    let initial_wallet = wallet_mgr
        .get_wallet(user.id)
        .await
        .expect("Should get wallet");

    // Create table escrow
    sqlx::query("INSERT INTO table_escrows (table_id, balance) VALUES ($1, 0) ON CONFLICT (table_id) DO UPDATE SET balance = 0")
        .bind(table_id)
        .execute(pool.as_ref())
        .await
        .expect("Should create table escrow");

    // Transfer to escrow
    let transfer_amount = 500;
    let idempotency_key = unique_key("test_transfer");

    let result = wallet_mgr
        .transfer_to_escrow(user.id, table_id, transfer_amount, idempotency_key)
        .await;

    assert!(
        result.is_ok(),
        "Transfer to escrow should succeed: {:?}",
        result.err()
    );

    // Check wallet balance decreased
    let updated_wallet = wallet_mgr
        .get_wallet(user.id)
        .await
        .expect("Should get wallet");
    assert_eq!(
        updated_wallet.balance,
        initial_wallet.balance - transfer_amount,
        "Wallet balance should decrease by transfer amount"
    );

    // Check escrow balance increased
    let escrow = wallet_mgr
        .get_escrow(table_id)
        .await
        .expect("Should get escrow");
    assert_eq!(
        escrow.balance, transfer_amount,
        "Escrow balance should equal transfer amount"
    );

    cleanup_table_escrow(&pool, table_id).await;
    cleanup_user(&pool, username).await;
}

#[tokio::test]
async fn test_transfer_from_escrow() {
    let (wallet_mgr, auth_mgr, pool) = setup_managers().await;
    let username = "test_escrow_from";
    let table_id = 998;
    cleanup_user(&pool, username).await;
    cleanup_table_escrow(&pool, table_id).await;

    // Register user and claim faucet
    let user = auth_mgr
        .register(RegisterRequest {
            username: username.to_string(),
            password: "SecurePass123!".to_string(),
            display_name: username.to_string(),
            email: None,
        })
        .await
        .expect("Registration should succeed");

    wallet_mgr
        .claim_faucet(user.id)
        .await
        .expect("Faucet claim should succeed");

    // Create table escrow (using ON CONFLICT to handle duplicates)
    sqlx::query("INSERT INTO table_escrows (table_id, balance) VALUES ($1, 0) ON CONFLICT (table_id) DO UPDATE SET balance = 0")
        .bind(table_id)
        .execute(pool.as_ref())
        .await
        .expect("Should create table escrow");

    // Transfer TO escrow first
    let initial_transfer = 500;
    wallet_mgr
        .transfer_to_escrow(
            user.id,
            table_id,
            initial_transfer,
            unique_key("test_setup"),
        )
        .await
        .expect("Setup transfer should succeed");

    let wallet_before = wallet_mgr
        .get_wallet(user.id)
        .await
        .expect("Should get wallet");

    // Transfer FROM escrow (won some chips at table)
    // Can only return what's in escrow or less
    let return_amount = 400; // Lost 100 chips at table
    let result = wallet_mgr
        .transfer_from_escrow(user.id, table_id, return_amount, unique_key("test_return"))
        .await;

    assert!(
        result.is_ok(),
        "Transfer from escrow should succeed: {:?}",
        result.err()
    );

    // Check wallet balance increased
    let wallet_after = wallet_mgr
        .get_wallet(user.id)
        .await
        .expect("Should get wallet");
    assert_eq!(
        wallet_after.balance,
        wallet_before.balance + return_amount,
        "Wallet should receive return amount"
    );

    cleanup_table_escrow(&pool, table_id).await;
    cleanup_user(&pool, username).await;
}

#[tokio::test]
async fn test_escrow_idempotency() {
    let (wallet_mgr, auth_mgr, pool) = setup_managers().await;
    let username = "test_escrow_idem";
    let table_id = 997;
    cleanup_user(&pool, username).await;
    cleanup_table_escrow(&pool, table_id).await;

    // Register user
    let user = auth_mgr
        .register(RegisterRequest {
            username: username.to_string(),
            password: "SecurePass123!".to_string(),
            display_name: username.to_string(),
            email: None,
        })
        .await
        .expect("Registration should succeed");

    wallet_mgr
        .claim_faucet(user.id)
        .await
        .expect("Faucet claim should succeed");

    // Create table escrow
    sqlx::query("INSERT INTO table_escrows (table_id, balance) VALUES ($1, 0) ON CONFLICT (table_id) DO UPDATE SET balance = 0")
        .bind(table_id)
        .execute(pool.as_ref())
        .await
        .expect("Should create table escrow");

    // First transfer with unique key (but will reuse within this test)
    let idempotency_key = unique_key("duplicate_test");
    let result1 = wallet_mgr
        .transfer_to_escrow(user.id, table_id, 500, idempotency_key.clone())
        .await;
    assert!(
        result1.is_ok(),
        "First transfer should succeed: {:?}",
        result1.err()
    );

    let wallet_after_first = wallet_mgr
        .get_wallet(user.id)
        .await
        .expect("Should get wallet");

    // Second transfer with same idempotency key should be idempotent (no-op or error)
    let result2 = wallet_mgr
        .transfer_to_escrow(user.id, table_id, 500, idempotency_key)
        .await;

    // Either should error or wallet shouldn't change
    let wallet_after_second = wallet_mgr
        .get_wallet(user.id)
        .await
        .expect("Should get wallet");
    if result2.is_ok() {
        assert_eq!(
            wallet_after_first.balance, wallet_after_second.balance,
            "Idempotent transfer should not change balance"
        );
    }

    cleanup_table_escrow(&pool, table_id).await;
    cleanup_user(&pool, username).await;
}

#[tokio::test]
async fn test_insufficient_funds() {
    let (wallet_mgr, auth_mgr, pool) = setup_managers().await;
    let username = "test_insufficient";
    let table_id = 996;
    cleanup_user(&pool, username).await;
    cleanup_table_escrow(&pool, table_id).await;

    // Register user (no faucet claim - low balance)
    let user = auth_mgr
        .register(RegisterRequest {
            username: username.to_string(),
            password: "SecurePass123!".to_string(),
            display_name: username.to_string(),
            email: None,
        })
        .await
        .expect("Registration should succeed");

    // Create table escrow
    sqlx::query("INSERT INTO table_escrows (table_id, balance) VALUES ($1, 0) ON CONFLICT (table_id) DO UPDATE SET balance = 0")
        .bind(table_id)
        .execute(pool.as_ref())
        .await
        .expect("Should create table escrow");

    let wallet = wallet_mgr
        .get_wallet(user.id)
        .await
        .expect("Should get wallet");

    // Try to transfer more than balance
    let result = wallet_mgr
        .transfer_to_escrow(
            user.id,
            table_id,
            wallet.balance + 1000,
            unique_key("test_overspend"),
        )
        .await;

    assert!(
        result.is_err(),
        "Transfer should fail with insufficient funds"
    );

    cleanup_table_escrow(&pool, table_id).await;
    cleanup_user(&pool, username).await;
}

#[tokio::test]
async fn test_escrow_upsert_returns_correct_balance() {
    // Regression test for H1: Escrow upsert bug
    // Verifies that multiple transfers to same escrow accumulate correctly
    let (wallet_mgr, auth_mgr, pool) = setup_managers().await;
    let username = "test_escrow_upsert";
    let table_id = 994;
    cleanup_user(&pool, username).await;
    cleanup_table_escrow(&pool, table_id).await;

    // Register user and claim faucet
    let user = auth_mgr
        .register(RegisterRequest {
            username: username.to_string(),
            password: "SecurePass123!".to_string(),
            display_name: username.to_string(),
            email: None,
        })
        .await
        .expect("Registration should succeed");

    wallet_mgr
        .claim_faucet(user.id)
        .await
        .expect("Faucet claim should succeed");

    // First transfer - creates escrow
    wallet_mgr
        .transfer_to_escrow(user.id, table_id, 100, unique_key("upsert1"))
        .await
        .expect("First transfer should succeed");

    let escrow_after_first = wallet_mgr
        .get_escrow(table_id)
        .await
        .expect("Should get escrow");
    assert_eq!(
        escrow_after_first.balance, 100,
        "Escrow should have 100 after first transfer"
    );

    // Second transfer - updates existing escrow (this is where the bug was)
    wallet_mgr
        .transfer_to_escrow(user.id, table_id, 200, unique_key("upsert2"))
        .await
        .expect("Second transfer should succeed");

    let escrow_after_second = wallet_mgr
        .get_escrow(table_id)
        .await
        .expect("Should get escrow");
    assert_eq!(
        escrow_after_second.balance, 300,
        "Escrow should have 300 (100+200) after second transfer - this verifies the upsert fix"
    );

    // Third transfer to verify accumulation continues working
    wallet_mgr
        .transfer_to_escrow(user.id, table_id, 50, unique_key("upsert3"))
        .await
        .expect("Third transfer should succeed");

    let escrow_after_third = wallet_mgr
        .get_escrow(table_id)
        .await
        .expect("Should get escrow");
    assert_eq!(
        escrow_after_third.balance, 350,
        "Escrow should have 350 (100+200+50) after third transfer"
    );

    cleanup_table_escrow(&pool, table_id).await;
    cleanup_user(&pool, username).await;
}

#[tokio::test]
async fn test_get_transaction_history() {
    let (wallet_mgr, auth_mgr, pool) = setup_managers().await;
    let username = "test_history";
    let table_id = 995;
    cleanup_user(&pool, username).await;
    cleanup_table_escrow(&pool, table_id).await;

    // Register user
    let user = auth_mgr
        .register(RegisterRequest {
            username: username.to_string(),
            password: "SecurePass123!".to_string(),
            display_name: username.to_string(),
            email: None,
        })
        .await
        .expect("Registration should succeed");

    wallet_mgr
        .claim_faucet(user.id)
        .await
        .expect("Faucet claim should succeed");

    // Create table escrow
    sqlx::query("INSERT INTO table_escrows (table_id, balance) VALUES ($1, 0) ON CONFLICT (table_id) DO UPDATE SET balance = 0")
        .bind(table_id)
        .execute(pool.as_ref())
        .await
        .expect("Should create table escrow");

    // Make some transactions
    wallet_mgr
        .transfer_to_escrow(user.id, table_id, 100, unique_key("tx1"))
        .await
        .expect("Transfer 1 should succeed");

    wallet_mgr
        .transfer_to_escrow(user.id, table_id, 200, unique_key("tx2"))
        .await
        .expect("Transfer 2 should succeed");

    // Get transaction history
    let entries = wallet_mgr
        .get_entries(user.id, 10)
        .await
        .expect("Should get transaction history");

    assert!(entries.len() >= 2, "Should have at least 2 transactions");

    // Check transactions are recorded
    let has_tx1 = entries.iter().any(|e| e.amount == -100);
    let has_tx2 = entries.iter().any(|e| e.amount == -200);
    assert!(has_tx1, "Should find first transfer in history");
    assert!(has_tx2, "Should find second transfer in history");

    cleanup_table_escrow(&pool, table_id).await;
    cleanup_user(&pool, username).await;
}

// === Edge Case Tests ===

#[tokio::test]
async fn test_overflow_protection_faucet_at_max() {
    // Test: Claiming faucet when balance is near i64::MAX should prevent overflow
    let (wallet_mgr, auth_mgr, pool) = setup_managers().await;
    let username = "test_overflow_max";
    cleanup_user(&pool, username).await;

    // Register user
    let user = auth_mgr
        .register(RegisterRequest {
            username: username.to_string(),
            password: "SecurePass123!".to_string(),
            display_name: username.to_string(),
            email: None,
        })
        .await
        .expect("Registration should succeed");

    // Manually set balance near i64::MAX
    let near_max = i64::MAX - 500; // Less than faucet amount
    sqlx::query("UPDATE wallets SET balance = $1 WHERE user_id = $2")
        .bind(near_max)
        .bind(user.id)
        .execute(pool.as_ref())
        .await
        .expect("Should update balance");

    // Try to claim faucet - should fail with overflow error
    let result = wallet_mgr.claim_faucet(user.id).await;
    assert!(
        result.is_err(),
        "Faucet claim should fail when it would overflow"
    );

    cleanup_user(&pool, username).await;
}

#[tokio::test]
async fn test_overflow_protection_escrow_transfer() {
    // Test: Transferring from escrow when it would cause overflow
    let (wallet_mgr, _auth_mgr, pool) = setup_managers().await;
    let username = "test_overflow_escrow";
    let table_id = 998;
    cleanup_user(&pool, username).await;
    cleanup_table_escrow(&pool, table_id).await;

    // Create user manually
    let user_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO users (username, password_hash, display_name) VALUES ($1, 'hash', $1) RETURNING id"
    )
    .bind(username)
    .fetch_one(pool.as_ref())
    .await
    .expect("Should create user");

    // Create wallet with near-max balance
    let near_max = i64::MAX - 500;
    sqlx::query("INSERT INTO wallets (user_id, balance) VALUES ($1, $2)")
        .bind(user_id)
        .bind(near_max)
        .execute(pool.as_ref())
        .await
        .expect("Should create wallet");

    // Create table escrow with some balance
    sqlx::query("INSERT INTO table_escrows (table_id, balance) VALUES ($1, 1000) ON CONFLICT (table_id) DO UPDATE SET balance = 1000")
        .bind(table_id)
        .execute(pool.as_ref())
        .await
        .expect("Should create table escrow");

    // Try to transfer from escrow - should fail
    let result = wallet_mgr
        .transfer_from_escrow(user_id, table_id, 1000, unique_key("overflow"))
        .await;

    assert!(
        result.is_err(),
        "Transfer from escrow should fail when it would overflow"
    );

    cleanup_table_escrow(&pool, table_id).await;
    cleanup_user(&pool, username).await;
}

#[tokio::test]
async fn test_underflow_protection_insufficient_funds() {
    // Test: Transferring more than balance should fail
    let (wallet_mgr, auth_mgr, pool) = setup_managers().await;
    let username = "test_underflow";
    let table_id = 999;
    cleanup_user(&pool, username).await;
    cleanup_table_escrow(&pool, table_id).await;

    // Register user
    let user = auth_mgr
        .register(RegisterRequest {
            username: username.to_string(),
            password: "SecurePass123!".to_string(),
            display_name: username.to_string(),
            email: None,
        })
        .await
        .expect("Registration should succeed");

    // Claim faucet to get some balance
    wallet_mgr
        .claim_faucet(user.id)
        .await
        .ok(); // May succeed or fail if already claimed

    // Get current balance
    let wallet = wallet_mgr
        .get_wallet(user.id)
        .await
        .expect("Should get wallet");

    let current_balance = wallet.balance;
    assert!(current_balance > 0, "Should have some balance");

    // Create table escrow
    sqlx::query("INSERT INTO table_escrows (table_id, balance) VALUES ($1, 0) ON CONFLICT (table_id) DO UPDATE SET balance = 0")
        .bind(table_id)
        .execute(pool.as_ref())
        .await
        .expect("Should create table escrow");

    // Try to transfer more than balance
    let result = wallet_mgr
        .transfer_to_escrow(user.id, table_id, current_balance + 1000, unique_key("underflow"))
        .await;

    assert!(
        result.is_err(),
        "Transfer should fail when amount exceeds balance: {:?}",
        result
    );

    cleanup_table_escrow(&pool, table_id).await;
    cleanup_user(&pool, username).await;
}

#[tokio::test]
async fn test_concurrent_faucet_claims() {
    // Test: Concurrent faucet claims should be safe (no corruption)
    let (wallet_mgr, auth_mgr, pool) = setup_managers().await;
    let wallet_mgr = Arc::new(wallet_mgr);
    let username = "test_concur_faucet";
    cleanup_user(&pool, username).await;

    // Register user
    let user = auth_mgr
        .register(RegisterRequest {
            username: username.to_string(),
            password: "SecurePass123!".to_string(),
            display_name: username.to_string(),
            email: None,
        })
        .await
        .expect("Registration should succeed");

    // Get initial balance
    let initial_wallet = wallet_mgr
        .get_wallet(user.id)
        .await
        .expect("Should get wallet");
    let initial_balance = initial_wallet.balance;

    // Try to claim faucet concurrently from multiple threads
    let mut handles = vec![];
    for _ in 0..10 {
        let mgr = wallet_mgr.clone();
        let uid = user.id;
        handles.push(tokio::spawn(async move { mgr.claim_faucet(uid).await }));
    }

    // Collect results
    let mut success_count = 0;
    for handle in handles {
        if handle.await.expect("Task should complete").is_ok() {
            success_count += 1;
        }
    }

    // At least one should succeed
    assert!(
        success_count >= 1,
        "At least one faucet claim should succeed"
    );

    // Verify balance increased (concurrency is safe)
    let final_wallet = wallet_mgr
        .get_wallet(user.id)
        .await
        .expect("Should get wallet");

    assert!(
        final_wallet.balance > initial_balance,
        "Balance should have increased"
    );
    assert!(
        final_wallet.balance % 1000 == 0,
        "Balance should be a multiple of 1000 (faucet amount)"
    );

    cleanup_user(&pool, username).await;
}

#[tokio::test]
async fn test_concurrent_escrow_operations() {
    // Test: Concurrent escrow operations should maintain consistency
    let (wallet_mgr, auth_mgr, pool) = setup_managers().await;
    let wallet_mgr = Arc::new(wallet_mgr);
    let username = "test_concur_escrow";
    let table_id = 1000;
    cleanup_user(&pool, username).await;
    cleanup_table_escrow(&pool, table_id).await;

    // Register user and give them chips
    let user = auth_mgr
        .register(RegisterRequest {
            username: username.to_string(),
            password: "SecurePass123!".to_string(),
            display_name: username.to_string(),
            email: None,
        })
        .await
        .expect("Registration should succeed");

    // Set balance to 10,000
    sqlx::query("UPDATE wallets SET balance = 10000 WHERE user_id = $1")
        .bind(user.id)
        .execute(pool.as_ref())
        .await
        .expect("Should update balance");

    // Create table escrow
    sqlx::query("INSERT INTO table_escrows (table_id, balance) VALUES ($1, 0) ON CONFLICT (table_id) DO UPDATE SET balance = 0")
        .bind(table_id)
        .execute(pool.as_ref())
        .await
        .expect("Should create table escrow");

    // Transfer to escrow concurrently (10 threads × 100 chips each)
    let mut handles = vec![];
    for i in 0..10 {
        let mgr = wallet_mgr.clone();
        let uid = user.id;
        let tid = table_id;
        let key = unique_key(&format!("concurrent_{}", i));
        handles.push(tokio::spawn(async move {
            mgr.transfer_to_escrow(uid, tid, 100, key).await
        }));
    }

    // Wait for all transfers
    let mut success_count = 0;
    for handle in handles {
        if handle.await.expect("Task should complete").is_ok() {
            success_count += 1;
        }
    }

    // All 10 transfers should succeed
    assert_eq!(success_count, 10, "All 10 transfers should succeed");

    // Verify final balances
    let wallet = wallet_mgr
        .get_wallet(user.id)
        .await
        .expect("Should get wallet");
    assert_eq!(
        wallet.balance, 9000,
        "Wallet balance should be 10000 - 1000"
    );

    let escrow_balance: i64 =
        sqlx::query_scalar("SELECT balance FROM table_escrows WHERE table_id = $1")
            .bind(table_id)
            .fetch_one(pool.as_ref())
            .await
            .expect("Should get escrow balance");
    assert_eq!(escrow_balance, 1000, "Escrow balance should be 1000");

    cleanup_table_escrow(&pool, table_id).await;
    cleanup_user(&pool, username).await;
}

#[tokio::test]
async fn test_zero_amount_transfers() {
    // Test: Zero amount transfers should be rejected
    let (wallet_mgr, auth_mgr, pool) = setup_managers().await;
    let username = "test_zero_transfer";
    let table_id = 1001;
    cleanup_user(&pool, username).await;
    cleanup_table_escrow(&pool, table_id).await;

    // Register user
    let user = auth_mgr
        .register(RegisterRequest {
            username: username.to_string(),
            password: "SecurePass123!".to_string(),
            display_name: username.to_string(),
            email: None,
        })
        .await
        .expect("Registration should succeed");

    wallet_mgr
        .claim_faucet(user.id)
        .await
        .expect("Faucet claim should succeed");

    // Create table escrow
    sqlx::query("INSERT INTO table_escrows (table_id, balance) VALUES ($1, 0) ON CONFLICT (table_id) DO UPDATE SET balance = 0")
        .bind(table_id)
        .execute(pool.as_ref())
        .await
        .expect("Should create table escrow");

    // Try zero transfer
    let result = wallet_mgr
        .transfer_to_escrow(user.id, table_id, 0, unique_key("zero"))
        .await;

    assert!(
        result.is_err(),
        "Zero amount transfer should be rejected"
    );

    cleanup_table_escrow(&pool, table_id).await;
    cleanup_user(&pool, username).await;
}

#[tokio::test]
async fn test_negative_amount_rejected() {
    // Test: Negative amounts should be rejected
    let (wallet_mgr, auth_mgr, pool) = setup_managers().await;
    let username = "test_negative";
    let table_id = 1002;
    cleanup_user(&pool, username).await;
    cleanup_table_escrow(&pool, table_id).await;

    // Register user
    let user = auth_mgr
        .register(RegisterRequest {
            username: username.to_string(),
            password: "SecurePass123!".to_string(),
            display_name: username.to_string(),
            email: None,
        })
        .await
        .expect("Registration should succeed");

    wallet_mgr
        .claim_faucet(user.id)
        .await
        .expect("Faucet claim should succeed");

    // Create table escrow
    sqlx::query("INSERT INTO table_escrows (table_id, balance) VALUES ($1, 100) ON CONFLICT (table_id) DO UPDATE SET balance = 100")
        .bind(table_id)
        .execute(pool.as_ref())
        .await
        .expect("Should create table escrow");

    // Try negative transfer from escrow
    let result = wallet_mgr
        .transfer_from_escrow(user.id, table_id, -100, unique_key("negative"))
        .await;

    assert!(
        result.is_err(),
        "Negative amount transfer should be rejected"
    );

    cleanup_table_escrow(&pool, table_id).await;
    cleanup_user(&pool, username).await;
}

#[tokio::test]
async fn test_transaction_idempotency() {
    // Test: Same transaction key should not be processed twice
    let (wallet_mgr, auth_mgr, pool) = setup_managers().await;
    let username = "test_idempotency";
    let table_id = 1003;
    cleanup_user(&pool, username).await;
    cleanup_table_escrow(&pool, table_id).await;

    // Register user
    let user = auth_mgr
        .register(RegisterRequest {
            username: username.to_string(),
            password: "SecurePass123!".to_string(),
            display_name: username.to_string(),
            email: None,
        })
        .await
        .expect("Registration should succeed");

    wallet_mgr
        .claim_faucet(user.id)
        .await
        .expect("Faucet claim should succeed");

    // Create table escrow
    sqlx::query("INSERT INTO table_escrows (table_id, balance) VALUES ($1, 0) ON CONFLICT (table_id) DO UPDATE SET balance = 0")
        .bind(table_id)
        .execute(pool.as_ref())
        .await
        .expect("Should create table escrow");

    let tx_key = unique_key("idempotent");

    // First transfer
    wallet_mgr
        .transfer_to_escrow(user.id, table_id, 100, tx_key.clone())
        .await
        .expect("First transfer should succeed");

    let wallet_first = wallet_mgr
        .get_wallet(user.id)
        .await
        .expect("Should get wallet");
    let balance_after_first = wallet_first.balance;

    // Second transfer with same key (simulating retry)
    wallet_mgr
        .transfer_to_escrow(user.id, table_id, 100, tx_key)
        .await
        .ok(); // May succeed or fail depending on implementation

    let wallet_second = wallet_mgr
        .get_wallet(user.id)
        .await
        .expect("Should get wallet");
    let balance_after_second = wallet_second.balance;

    // Balance should be the same (idempotent)
    assert_eq!(
        balance_after_first, balance_after_second,
        "Balance should not change on duplicate transaction"
    );

    cleanup_table_escrow(&pool, table_id).await;
    cleanup_user(&pool, username).await;
}
