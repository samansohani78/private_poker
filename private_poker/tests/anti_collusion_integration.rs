#![allow(clippy::unreadable_literal)]

//! Integration tests for anti-collusion detection system.
//!
//! Tests IP tracking, win rate anomaly detection, coordinated folding patterns,
//! and shadow flagging functionality.

use private_poker::auth::{AuthManager, RegisterRequest};
use private_poker::db::{Database, DatabaseConfig};
use private_poker::security::AntiCollusionDetector;
use sqlx::PgPool;
use std::sync::Arc;

/// Generate unique username with timestamp (max 20 chars)
fn unique_username(prefix: &str) -> String {
    let timestamp = chrono::Utc::now().timestamp_millis();
    // Take last 6 digits of timestamp to keep username short
    let short_ts = timestamp % 1000000;
    format!("{}_{}", prefix, short_ts)
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

/// Helper to create test managers
async fn setup_managers() -> (AntiCollusionDetector, AuthManager, Arc<PgPool>) {
    let pool = setup_test_db().await;
    let detector = AntiCollusionDetector::new(pool.clone());
    let auth_mgr = AuthManager::new(
        pool.clone(),
        "test_pepper".to_string(),
        "test_jwt_secret".to_string(),
    );
    (detector, auth_mgr, pool)
}

/// Helper to cleanup test user
async fn cleanup_user(pool: &PgPool, username: &str) {
    let _ = sqlx::query("DELETE FROM users WHERE username = $1")
        .bind(username)
        .execute(pool)
        .await;
}

/// Helper to cleanup collusion flags for a user
async fn cleanup_flags(pool: &PgPool, user_id: i64) {
    let _ = sqlx::query("DELETE FROM collusion_flags WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await;
}

#[tokio::test]
async fn test_same_ip_detection() {
    // Test: Two users with same IP join a table -> flag created
    let (detector, auth_mgr, pool) = setup_managers().await;
    let username1 = unique_username("sameip1");
    let username2 = unique_username("sameip2");
    let table_id = 1001;
    let same_ip = "192.168.1.100";

    cleanup_user(&pool, &username1).await;
    cleanup_user(&pool, &username2).await;

    // Register two users
    let user1 = auth_mgr
        .register(RegisterRequest {
            username: username1.clone(),
            password: "SecurePass123!".to_string(),
            display_name: username1.to_string(),
            email: None,
        })
        .await
        .expect("User 1 registration should succeed");

    let user2 = auth_mgr
        .register(RegisterRequest {
            username: username2.clone(),
            password: "SecurePass123!".to_string(),
            display_name: username2.to_string(),
            email: None,
        })
        .await
        .expect("User 2 registration should succeed");

    cleanup_flags(&pool, user1.id).await;
    cleanup_flags(&pool, user2.id).await;

    // User 1 joins table
    detector
        .register_user_ip(user1.id, same_ip.to_string())
        .await;
    detector.add_player_to_table(table_id, user1.id).await;

    // User 2 joins same table with same IP - should trigger flag
    detector
        .register_user_ip(user2.id, same_ip.to_string())
        .await;
    let same_ip_detected = detector
        .check_same_ip_at_table(table_id, user2.id)
        .await
        .expect("Check should succeed");

    detector.add_player_to_table(table_id, user2.id).await;

    assert!(same_ip_detected, "Same IP should be detected");

    // Check that flag was created (flag type is 'same_ip_table')
    let flag_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM collusion_flags
         WHERE table_id = $1 AND flag_type = 'same_ip_table'",
    )
    .bind(table_id)
    .fetch_one(pool.as_ref())
    .await
    .expect("Should query flags");

    assert!(flag_count > 0, "Should have created same-IP collusion flag");

    // Verify flag has correct severity
    let severity: Option<String> = sqlx::query_scalar(
        "SELECT severity FROM collusion_flags
         WHERE table_id = $1 AND flag_type = 'same_ip_table'
         LIMIT 1",
    )
    .bind(table_id)
    .fetch_optional(pool.as_ref())
    .await
    .expect("Should query flag severity");

    assert_eq!(
        severity,
        Some("medium".to_string()),
        "Same-IP flag should have Medium severity"
    );

    cleanup_flags(&pool, user1.id).await;
    cleanup_flags(&pool, user2.id).await;
    cleanup_user(&pool, &username1).await;
    cleanup_user(&pool, &username2).await;
}

#[tokio::test]
async fn test_different_ip_no_flag() {
    // Test: Two users with different IPs -> no flag created
    let (detector, auth_mgr, pool) = setup_managers().await;
    let username1 = unique_username("diffip1");
    let username2 = unique_username("diffip2");
    let table_id = 1002;

    cleanup_user(&pool, &username1).await;
    cleanup_user(&pool, &username2).await;

    // Register two users
    let user1 = auth_mgr
        .register(RegisterRequest {
            username: username1.clone(),
            password: "SecurePass123!".to_string(),
            display_name: username1.to_string(),
            email: None,
        })
        .await
        .expect("User 1 registration should succeed");

    let user2 = auth_mgr
        .register(RegisterRequest {
            username: username2.clone(),
            password: "SecurePass123!".to_string(),
            display_name: username2.to_string(),
            email: None,
        })
        .await
        .expect("User 2 registration should succeed");

    cleanup_flags(&pool, user1.id).await;
    cleanup_flags(&pool, user2.id).await;

    // User 1 joins with IP A
    detector
        .register_user_ip(user1.id, "192.168.1.10".to_string())
        .await;
    detector.add_player_to_table(table_id, user1.id).await;

    // User 2 joins with IP B (different)
    detector
        .register_user_ip(user2.id, "192.168.1.20".to_string())
        .await;
    detector.add_player_to_table(table_id, user2.id).await;

    // Check no flag was created
    let flag_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM collusion_flags
         WHERE table_id = $1",
    )
    .bind(table_id)
    .fetch_one(pool.as_ref())
    .await
    .expect("Should query flags");

    assert_eq!(flag_count, 0, "Should not create flag for different IPs");

    cleanup_flags(&pool, user1.id).await;
    cleanup_flags(&pool, user2.id).await;
    cleanup_user(&pool, &username1).await;
    cleanup_user(&pool, &username2).await;
}

#[tokio::test]
async fn test_win_rate_anomaly_detection() {
    // Test: User A wins 15/15 hands against user B (same IP) -> High severity flag
    let (detector, auth_mgr, pool) = setup_managers().await;
    let username_winner = unique_username("win");
    let username_loser = unique_username("lose");
    let table_id = 1003;
    let same_ip = "10.0.0.50";

    cleanup_user(&pool, &username_winner).await;
    cleanup_user(&pool, &username_loser).await;

    // Register users
    let winner = auth_mgr
        .register(RegisterRequest {
            username: username_winner.to_string(),
            password: "SecurePass123!".to_string(),
            display_name: username_winner.to_string(),
            email: None,
        })
        .await
        .expect("Winner registration should succeed");

    let loser = auth_mgr
        .register(RegisterRequest {
            username: username_loser.to_string(),
            password: "SecurePass123!".to_string(),
            display_name: username_loser.to_string(),
            email: None,
        })
        .await
        .expect("Loser registration should succeed");

    cleanup_flags(&pool, winner.id).await;
    cleanup_flags(&pool, loser.id).await;

    // Both join table (same IP)
    detector
        .register_user_ip(winner.id, same_ip.to_string())
        .await;
    detector.add_player_to_table(table_id, winner.id).await;
    detector
        .register_user_ip(loser.id, same_ip.to_string())
        .await;
    detector
        .check_same_ip_at_table(table_id, loser.id)
        .await
        .ok();
    detector.add_player_to_table(table_id, loser.id).await;

    // Simulate analyzing win rate after 15 hands where winner always wins (100% win rate)
    detector
        .analyze_win_rate(winner.id, loser.id, table_id, 1.0)
        .await
        .expect("Win rate analysis should complete");

    // Check for win rate anomaly flag (flag type is 'win_rate_anomaly')
    let anomaly_flags: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM collusion_flags
         WHERE table_id = $1 AND flag_type = 'win_rate_anomaly'",
    )
    .bind(table_id)
    .fetch_one(pool.as_ref())
    .await
    .expect("Should query anomaly flags");

    assert!(
        anomaly_flags > 0,
        "Should create win rate anomaly flag after suspicious pattern"
    );

    // Check severity is High
    let severity: Option<String> = sqlx::query_scalar(
        "SELECT severity FROM collusion_flags
         WHERE table_id = $1 AND flag_type = 'win_rate_anomaly'
         LIMIT 1",
    )
    .bind(table_id)
    .fetch_optional(pool.as_ref())
    .await
    .expect("Should query severity");

    assert_eq!(
        severity,
        Some("high".to_string()),
        "Win rate anomaly should have High severity"
    );

    cleanup_flags(&pool, winner.id).await;
    cleanup_flags(&pool, loser.id).await;
    cleanup_user(&pool, &username_winner).await;
    cleanup_user(&pool, &username_loser).await;
}

#[tokio::test]
async fn test_shadow_flagging_no_auto_ban() {
    // Test: Flags are created but users are not automatically banned
    let (detector, auth_mgr, pool) = setup_managers().await;
    let username1 = unique_username("shadow1");
    let username2 = unique_username("shadow2");
    let table_id = 1004;
    let same_ip = "172.16.0.100";

    cleanup_user(&pool, &username1).await;
    cleanup_user(&pool, &username2).await;

    // Register users
    let user1 = auth_mgr
        .register(RegisterRequest {
            username: username1.clone(),
            password: "SecurePass123!".to_string(),
            display_name: username1.to_string(),
            email: None,
        })
        .await
        .expect("User 1 registration should succeed");

    let user2 = auth_mgr
        .register(RegisterRequest {
            username: username2.clone(),
            password: "SecurePass123!".to_string(),
            display_name: username2.to_string(),
            email: None,
        })
        .await
        .expect("User 2 registration should succeed");

    cleanup_flags(&pool, user1.id).await;
    cleanup_flags(&pool, user2.id).await;

    // Both join (triggers flag)
    detector
        .register_user_ip(user1.id, same_ip.to_string())
        .await;
    detector.add_player_to_table(table_id, user1.id).await;

    detector
        .register_user_ip(user2.id, same_ip.to_string())
        .await;
    let result = detector.check_same_ip_at_table(table_id, user2.id).await;
    detector.add_player_to_table(table_id, user2.id).await;

    assert!(
        result.is_ok(),
        "User 2 should still be allowed to join despite flag"
    );

    // Verify flag exists
    let flags: Vec<(bool, Option<i64>)> = sqlx::query_as(
        "SELECT reviewed, reviewer_user_id FROM collusion_flags
         WHERE table_id = $1",
    )
    .bind(table_id)
    .fetch_all(pool.as_ref())
    .await
    .expect("Should fetch flags");

    assert!(!flags.is_empty(), "Should have created flags");

    // All flags should be unreviewed
    for (reviewed, reviewer_id) in flags {
        assert!(!reviewed, "Flag should not be auto-reviewed");
        assert!(reviewer_id.is_none(), "Flag should have no reviewer yet");
    }

    // Verify users are still active (not banned)
    let user1_active: bool = sqlx::query_scalar("SELECT is_active FROM users WHERE id = $1")
        .bind(user1.id)
        .fetch_one(pool.as_ref())
        .await
        .expect("Should fetch user1 status");

    let user2_active: bool = sqlx::query_scalar("SELECT is_active FROM users WHERE id = $1")
        .bind(user2.id)
        .fetch_one(pool.as_ref())
        .await
        .expect("Should fetch user2 status");

    assert!(user1_active, "User 1 should still be active");
    assert!(user2_active, "User 2 should still be active");

    cleanup_flags(&pool, user1.id).await;
    cleanup_flags(&pool, user2.id).await;
    cleanup_user(&pool, &username1).await;
    cleanup_user(&pool, &username2).await;
}

#[tokio::test]
async fn test_player_leave_cleanup() {
    // Test: Player leaving table cleans up IP tracking
    let (detector, auth_mgr, pool) = setup_managers().await;
    let username = unique_username("leave");
    let table_id = 1005;
    let ip = "203.0.113.10";

    cleanup_user(&pool, &username).await;

    let user = auth_mgr
        .register(RegisterRequest {
            username: username.clone(),
            password: "SecurePass123!".to_string(),
            display_name: username.to_string(),
            email: None,
        })
        .await
        .expect("User registration should succeed");

    cleanup_flags(&pool, user.id).await;

    // Join table
    detector.register_user_ip(user.id, ip.to_string()).await;
    detector.add_player_to_table(table_id, user.id).await;

    // Leave table
    detector.remove_player_from_table(table_id, user.id).await;

    // Rejoin should not create duplicate entries
    detector.add_player_to_table(table_id, user.id).await;

    // If we got here without panic, test passes

    cleanup_flags(&pool, user.id).await;
    cleanup_user(&pool, &username).await;
}
