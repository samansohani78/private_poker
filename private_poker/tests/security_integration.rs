//! Integration tests for security features.
//!
//! Tests rate limiting, anti-collusion detection, and seat randomization.

use private_poker::db::{Database, DatabaseConfig};
use private_poker::security::{AntiCollusionDetector, RateLimiter, SeatRandomizer};
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

/// Generate unique identifier for tests
fn unique_id(prefix: &str) -> String {
    format!("{}_{}", prefix, chrono::Utc::now().timestamp_nanos_opt().unwrap())
}

// === Rate Limiter Tests ===

#[tokio::test]
async fn test_rate_limit_login_success() {
    let pool = setup_test_db().await;
    let limiter = RateLimiter::new(pool);

    let identifier = unique_id("test_login_ip");

    // First 5 attempts should be allowed (login limit)
    for i in 0..5 {
        let result = limiter.check_rate_limit("login", &identifier).await;
        assert!(result.is_ok(), "Check #{} should succeed", i + 1);
        assert!(
            result.unwrap().is_allowed(),
            "Attempt #{} should be allowed",
            i + 1
        );

        limiter
            .record_attempt("login", &identifier)
            .await
            .expect("Recording should succeed");
    }

    // Clean up
    limiter
        .reset("login", &identifier)
        .await
        .expect("Reset should succeed");
}

#[tokio::test]
async fn test_rate_limit_exceeded() {
    let pool = setup_test_db().await;
    let limiter = RateLimiter::new(pool);

    let identifier = unique_id("test_exceeded_ip");

    // Exceed the login limit (5 attempts)
    for _i in 0..5 {
        limiter
            .check_rate_limit("login", &identifier)
            .await
            .expect("Check should succeed");
        limiter
            .record_attempt("login", &identifier)
            .await
            .expect("Recording should succeed");
    }

    // 6th attempt should be locked
    let result = limiter.check_rate_limit("login", &identifier).await;
    assert!(result.is_ok(), "Check should succeed");
    assert!(!result.unwrap().is_allowed(), "Attempt should be locked");

    // Clean up
    limiter
        .reset("login", &identifier)
        .await
        .expect("Reset should succeed");
}

#[tokio::test]
async fn test_rate_limit_different_endpoints() {
    let pool = setup_test_db().await;
    let limiter = RateLimiter::new(pool);

    let identifier = unique_id("test_multi_endpoint");

    // Login attempts (limit 5)
    for _ in 0..5 {
        limiter.record_attempt("login", &identifier).await.expect("Should succeed");
    }

    // Register attempts should have separate counter
    let result = limiter.check_rate_limit("register", &identifier).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_allowed(), "Register should still be allowed");

    // Clean up
    limiter.reset("login", &identifier).await.ok();
    limiter.reset("register", &identifier).await.ok();
}

#[tokio::test]
async fn test_rate_limit_reset() {
    let pool = setup_test_db().await;
    let limiter = RateLimiter::new(pool);

    let identifier = unique_id("test_reset");

    // Exceed limit
    for _ in 0..6 {
        limiter.record_attempt("login", &identifier).await.ok();
    }

    // Should be locked
    let result = limiter.check_rate_limit("login", &identifier).await.unwrap();
    assert!(!result.is_allowed());

    // Reset
    limiter.reset("login", &identifier).await.expect("Reset should succeed");

    // Should be allowed again
    let result = limiter.check_rate_limit("login", &identifier).await.unwrap();
    assert!(result.is_allowed(), "Should be allowed after reset");

    limiter.reset("login", &identifier).await.ok();
}

// === Anti-Collusion Tests ===

#[tokio::test]
async fn test_register_user_ip() {
    let pool = setup_test_db().await;
    let detector = AntiCollusionDetector::new(pool.clone());

    let user_id = 9000 + (chrono::Utc::now().timestamp_nanos_opt().unwrap() % 1000);
    let ip = format!("192.168.1.{}", user_id % 255);

    detector.register_user_ip(user_id, ip).await;

    // Cleanup
    let _ = sqlx::query("DELETE FROM user_ips WHERE user_id = $1")
        .bind(user_id)
        .execute(pool.as_ref())
        .await;
}

#[tokio::test]
async fn test_check_same_ip_at_table() {
    let pool = setup_test_db().await;
    let detector = AntiCollusionDetector::new(pool.clone());

    let timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap();
    let username1 = format!("uip{}", timestamp % 100000);
    let username2 = format!("uip{}", timestamp % 100000 + 1);
    let table_id = 500 + (timestamp % 100);
    let shared_ip = format!("10.0.0.{}", timestamp % 255);

    // Create test users
    let user1 = sqlx::query_scalar::<_, i64>(
        "INSERT INTO users (username, password_hash, display_name) VALUES ($1, 'hash', $1) RETURNING id"
    )
    .bind(&username1)
    .fetch_one(pool.as_ref())
    .await
    .expect("Failed to create user1");

    let user2 = sqlx::query_scalar::<_, i64>(
        "INSERT INTO users (username, password_hash, display_name) VALUES ($1, 'hash', $1) RETURNING id"
    )
    .bind(&username2)
    .fetch_one(pool.as_ref())
    .await
    .expect("Failed to create user2");

    // Register both users with same IP
    detector.register_user_ip(user1, shared_ip.clone()).await;
    detector.register_user_ip(user2, shared_ip).await;

    // Add both to same table
    detector.add_player_to_table(table_id, user1).await;
    detector.add_player_to_table(table_id, user2).await;

    // Check for same IP
    let result = detector.check_same_ip_at_table(table_id, user1).await;
    assert!(result.is_ok(), "check_same_ip_at_table failed: {:?}", result.err());
    assert!(result.unwrap(), "Should detect same IP at table");

    // Cleanup
    detector.remove_player_from_table(table_id, user1).await;
    detector.remove_player_from_table(table_id, user2).await;
    let _ = sqlx::query("DELETE FROM collusion_flags WHERE user_id IN ($1, $2)")
        .bind(user1)
        .bind(user2)
        .execute(pool.as_ref())
        .await;
    let _ = sqlx::query("DELETE FROM users WHERE id IN ($1, $2)")
        .bind(user1)
        .bind(user2)
        .execute(pool.as_ref())
        .await;
}

#[tokio::test]
async fn test_no_same_ip_detection() {
    let pool = setup_test_db().await;
    let detector = AntiCollusionDetector::new(pool.clone());

    let timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap();
    let user1 = 11000 + (timestamp % 1000);
    let user2 = 11000 + (timestamp % 1000) + 1;
    let table_id = 600 + (timestamp % 100);

    // Register users with different IPs
    detector.register_user_ip(user1, format!("10.1.1.{}", timestamp % 255)).await;
    detector.register_user_ip(user2, format!("10.1.2.{}", timestamp % 255)).await;

    // Add both to same table
    detector.add_player_to_table(table_id, user1).await;
    detector.add_player_to_table(table_id, user2).await;

    // Check for same IP
    let result = detector.check_same_ip_at_table(table_id, user1).await;
    assert!(result.is_ok());
    assert!(!result.unwrap(), "Should not detect same IP with different IPs");

    // Cleanup
    detector.remove_player_from_table(table_id, user1).await;
    detector.remove_player_from_table(table_id, user2).await;
    let _ = sqlx::query("DELETE FROM user_ips WHERE user_id IN ($1, $2)")
        .bind(user1)
        .bind(user2)
        .execute(pool.as_ref())
        .await;
}

#[tokio::test]
async fn test_add_remove_player_from_table() {
    let pool = setup_test_db().await;
    let detector = AntiCollusionDetector::new(pool.clone());

    let timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap();
    let user_id = 12000 + (timestamp % 1000);
    let table_id = 700 + (timestamp % 100);

    // Add player
    detector.add_player_to_table(table_id, user_id).await;

    // Verify player is at table
    let result: Result<bool, _> = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM table_players WHERE table_id = $1 AND user_id = $2)"
    )
    .bind(table_id)
    .bind(user_id)
    .fetch_one(pool.as_ref())
    .await;

    if let Ok(exists) = result {
        assert!(exists, "Player should be at table");
    }

    // Remove player
    detector.remove_player_from_table(table_id, user_id).await;

    // Verify player is removed
    let result: Result<bool, _> = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM table_players WHERE table_id = $1 AND user_id = $2)"
    )
    .bind(table_id)
    .bind(user_id)
    .fetch_one(pool.as_ref())
    .await;

    if let Ok(exists) = result {
        assert!(!exists, "Player should be removed from table");
    }
}

// === Seat Randomizer Tests ===

#[tokio::test]
async fn test_assign_seats_randomization() {
    let mut randomizer = SeatRandomizer::new();

    let users = vec![1, 2, 3, 4, 5];
    let seats1 = randomizer.assign_seats(&users, 9);
    let seats2 = randomizer.assign_seats(&users, 9);

    // All users should get seats
    assert_eq!(seats1.len(), users.len());
    assert_eq!(seats2.len(), users.len());

    // Seats should be within range [0, 9)
    for &seat in seats1.values() {
        assert!(seat < 9);
    }

    // Each seat assignment might differ (randomized)
    // We can't guarantee they differ in every run, but structure is correct
}

#[tokio::test]
async fn test_find_random_seat_available() {
    let mut randomizer = SeatRandomizer::new();

    let occupied = vec![0, 2, 4, 6, 8];
    let max_seats = 9;

    let seat = randomizer.find_random_seat(&occupied, max_seats);
    assert!(seat.is_some(), "Should find an available seat");

    let seat_num = seat.unwrap();
    assert!(!occupied.contains(&seat_num), "Should not return occupied seat");
    assert!(seat_num < max_seats, "Seat should be within range");
}

#[tokio::test]
async fn test_find_random_seat_all_occupied() {
    let mut randomizer = SeatRandomizer::new();

    let occupied = vec![0, 1, 2, 3, 4, 5, 6, 7, 8];
    let max_seats = 9;

    let seat = randomizer.find_random_seat(&occupied, max_seats);
    assert!(seat.is_none(), "Should return None when all seats occupied");
}

#[tokio::test]
async fn test_shuffle_seats() {
    let mut randomizer = SeatRandomizer::new();

    let current_seats = vec![
        (1, 0),
        (2, 2),
        (3, 4),
        (4, 6),
        (5, 8),
    ].into_iter().collect();

    let shuffled = randomizer.shuffle_seats(&current_seats);

    // All users should still have seats
    assert_eq!(shuffled.len(), current_seats.len());

    // Each user should have a valid seat
    for (user_id, seat) in &shuffled {
        assert!(current_seats.contains_key(user_id), "User should be in original set");
        assert!(*seat < 9, "Seat should be within range");
    }

    // No duplicate seats
    let seats: Vec<_> = shuffled.values().copied().collect();
    let unique_seats: std::collections::HashSet<_> = seats.iter().copied().collect();
    assert_eq!(seats.len(), unique_seats.len(), "All seats should be unique");
}

#[tokio::test]
async fn test_rate_limit_cleanup() {
    let pool = setup_test_db().await;
    let limiter = RateLimiter::new(pool);

    // Cleanup should succeed (removes expired entries)
    let result = limiter.cleanup_expired().await;
    assert!(result.is_ok(), "Cleanup should succeed");
}
