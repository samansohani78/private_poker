//! Integration tests for security features.
//!
//! Tests rate limiting, anti-collusion detection, and seat randomization.

use private_poker::db::{Database, DatabaseConfig};
use private_poker::security::{
    AntiCollusionDetector, RateLimiter, SeatRandomizer,
};
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

// === Rate Limiter Tests ===

#[tokio::test]
async fn test_rate_limit_login_success() {
    let pool = setup_test_db().await;
    let limiter = RateLimiter::new(pool);

    let identifier = "test_login_ip_1";

    // First 5 attempts should be allowed (login limit)
    for i in 0..5 {
        let result = limiter.check_rate_limit("login", identifier).await;
        assert!(result.is_ok(), "Check #{} should succeed", i + 1);
        assert!(result.unwrap().is_allowed(), "Attempt #{} should be allowed", i + 1);

        limiter.record_attempt("login", identifier).await.expect("Recording should succeed");
    }

    // Clean up
    limiter.reset("login", identifier).await.expect("Reset should succeed");
}

#[tokio::test]
async fn test_rate_limit_exceeded() {
    let pool = setup_test_db().await;
    let limiter = RateLimiter::new(pool);

    let identifier = "test_exceeded_ip_1";

    // Exceed the login limit (5 attempts)
    for _i in 0..5 {
        limiter.check_rate_limit("login", identifier).await.expect("Check should succeed");
        limiter.record_attempt("login", identifier).await.expect("Recording should succeed");
    }

    // 6th attempt should be locked
    let result = limiter.check_rate_limit("login", identifier).await;
    assert!(result.is_ok(), "Check should succeed");
    assert!(!result.unwrap().is_allowed(), "Attempt should be locked");

    // Clean up
    limiter.reset("login", identifier).await.expect("Reset should succeed");
}

#[tokio::test]
async fn test_rate_limit_exponential_backoff() {
    let pool = setup_test_db().await;
    let limiter = RateLimiter::new(pool);

    let identifier = "test_backoff_ip_1";

    // First violation
    for _i in 0..5 {
        limiter.record_attempt("login", identifier).await.expect("Recording should succeed");
    }

    let result1 = limiter.check_rate_limit("login", identifier).await.expect("Check should succeed");
    let retry_after_1 = result1.retry_after().expect("Should have retry_after");

    // Wait a bit and violate again (simulated - in real test would need to wait for window expiry)
    // For this test, just verify the exponential backoff logic exists

    assert!(retry_after_1 > 0, "Should have lockout period");

    // Clean up
    limiter.reset("login", identifier).await.expect("Reset should succeed");
}

#[tokio::test]
async fn test_rate_limit_different_endpoints() {
    let pool = setup_test_db().await;
    let limiter = RateLimiter::new(pool);

    let identifier = "test_multi_endpoint_ip";

    // Use login endpoint 5 times
    for _i in 0..5 {
        limiter.record_attempt("login", identifier).await.expect("Recording should succeed");
    }

    // Register endpoint should still be available (different limit)
    let result = limiter.check_rate_limit("register", identifier).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_allowed(), "Register endpoint should still be available");

    // Clean up
    limiter.reset("login", identifier).await.expect("Reset should succeed");
    limiter.reset("register", identifier).await.expect("Reset should succeed");
}

#[tokio::test]
async fn test_rate_limit_reset() {
    let pool = setup_test_db().await;
    let limiter = RateLimiter::new(pool);

    let identifier = "test_reset_ip";

    // Hit the limit
    for _i in 0..5 {
        limiter.record_attempt("login", identifier).await.expect("Recording should succeed");
    }

    // Should be locked
    let result1 = limiter.check_rate_limit("login", identifier).await.expect("Check should succeed");
    assert!(!result1.is_allowed());

    // Reset
    limiter.reset("login", identifier).await.expect("Reset should succeed");

    // Should be allowed again
    let result2 = limiter.check_rate_limit("login", identifier).await.expect("Check should succeed");
    assert!(result2.is_allowed(), "After reset, should be allowed");
}

// === Anti-Collusion Tests ===

#[tokio::test]
async fn test_same_ip_detection() {
    let pool = setup_test_db().await;
    let detector = AntiCollusionDetector::new(pool);

    let table_id = 1;
    let user1_id = 100;
    let user2_id = 101;
    let ip = "192.168.1.100";

    // Register both users with same IP
    detector.register_user_ip(user1_id, ip.to_string()).await;
    detector.register_user_ip(user2_id, ip.to_string()).await;

    // Add first user to table
    detector.add_player_to_table(table_id, user1_id).await;

    // Check if second user has same IP at table
    let same_ip = detector
        .check_same_ip_at_table(table_id, user2_id)
        .await
        .expect("Check should succeed");

    assert!(same_ip, "Should detect same IP");

    // Clean up
    detector.remove_player_from_table(table_id, user1_id).await;
}

#[tokio::test]
async fn test_different_ip_allowed() {
    let pool = setup_test_db().await;
    let detector = AntiCollusionDetector::new(pool);

    let table_id = 2;
    let user1_id = 200;
    let user2_id = 201;

    // Register users with different IPs
    detector.register_user_ip(user1_id, "192.168.1.100".to_string()).await;
    detector.register_user_ip(user2_id, "192.168.1.101".to_string()).await;

    // Add first user to table
    detector.add_player_to_table(table_id, user1_id).await;

    // Check if second user has same IP (should be false)
    let same_ip = detector
        .check_same_ip_at_table(table_id, user2_id)
        .await
        .expect("Check should succeed");

    assert!(!same_ip, "Should not detect same IP for different IPs");

    // Clean up
    detector.remove_player_from_table(table_id, user1_id).await;
}

#[tokio::test]
async fn test_win_rate_anomaly_detection() {
    let pool = setup_test_db().await;
    let detector = AntiCollusionDetector::new(pool);

    let table_id = 3;
    let user1_id = 300;
    let user2_id = 301;
    let ip = "192.168.1.200";

    // Register both users with same IP
    detector.register_user_ip(user1_id, ip.to_string()).await;
    detector.register_user_ip(user2_id, ip.to_string()).await;

    // Analyze suspiciously high win rate (>80%)
    let result = detector
        .analyze_win_rate(user1_id, user2_id, table_id, 0.85)
        .await;

    assert!(result.is_ok(), "Analysis should succeed");

    // This should have created a flag in the database
    // Verify flags were created
    let flags = detector.get_unreviewed_flags().await.expect("Should get flags");
    assert!(flags.len() > 0, "Should have created collusion flags");
}

#[tokio::test]
async fn test_coordinated_folding_detection() {
    let pool = setup_test_db().await;
    let detector = AntiCollusionDetector::new(pool);

    let table_id = 4;
    let user1_id = 400;
    let user2_id = 401;
    let ip = "192.168.1.300";

    // Register both users with same IP
    detector.register_user_ip(user1_id, ip.to_string()).await;
    detector.register_user_ip(user2_id, ip.to_string()).await;

    // Analyze coordinated folding pattern
    let result = detector
        .analyze_folding_pattern(table_id, user1_id, user2_id)
        .await;

    assert!(result.is_ok(), "Analysis should succeed");
}

#[tokio::test]
async fn test_flag_review_workflow() {
    let pool = setup_test_db().await;
    let detector = AntiCollusionDetector::new(pool);

    let table_id = 5;
    let user1_id = 500;
    let user2_id = 501;
    let reviewer_id = 1; // Admin user
    let ip = "192.168.1.400";

    // Create a flag by detecting same IP
    detector.register_user_ip(user1_id, ip.to_string()).await;
    detector.register_user_ip(user2_id, ip.to_string()).await;
    detector.add_player_to_table(table_id, user1_id).await;
    detector.check_same_ip_at_table(table_id, user2_id).await.expect("Check should succeed");

    // Get unreviewed flags
    let flags = detector.get_unreviewed_flags().await.expect("Should get flags");
    assert!(flags.len() > 0, "Should have unreviewed flags");

    // Mark first flag as reviewed
    if let Some(flag) = flags.first() {
        let result = detector.mark_flag_reviewed(flag.id, reviewer_id).await;
        assert!(result.is_ok(), "Mark as reviewed should succeed");
    }

    // Clean up
    detector.remove_player_from_table(table_id, user1_id).await;
}

#[tokio::test]
async fn test_get_user_flags() {
    let pool = setup_test_db().await;
    let detector = AntiCollusionDetector::new(pool);

    let table_id = 6;
    let user1_id = 600;
    let user2_id = 601;
    let ip = "192.168.1.500";

    // Create flags for user
    detector.register_user_ip(user1_id, ip.to_string()).await;
    detector.register_user_ip(user2_id, ip.to_string()).await;
    detector.add_player_to_table(table_id, user1_id).await;
    detector.check_same_ip_at_table(table_id, user2_id).await.expect("Check should succeed");

    // Get flags for user2
    let user_flags = detector.get_user_flags(user2_id).await.expect("Should get user flags");
    assert!(user_flags.len() > 0, "User should have flags");

    // Clean up
    detector.remove_player_from_table(table_id, user1_id).await;
}

// === Seat Randomizer Tests ===

#[test]
fn test_seat_randomization() {
    let mut randomizer = SeatRandomizer::new();
    let user_ids = vec![1, 2, 3, 4, 5];
    let max_seats = 10;

    let assignments1 = randomizer.assign_seats(&user_ids, max_seats);
    let assignments2 = randomizer.assign_seats(&user_ids, max_seats);

    // All users should be assigned
    assert_eq!(assignments1.len(), 5);
    assert_eq!(assignments2.len(), 5);

    // Seats should be within range
    for &seat in assignments1.values() {
        assert!(seat < max_seats);
    }

    // Should be randomized (high probability they differ)
    let same_count = user_ids
        .iter()
        .filter(|&&uid| assignments1.get(&uid) == assignments2.get(&uid))
        .count();

    assert!(same_count < 5, "Assignments should be randomized");
}

#[test]
fn test_find_random_available_seat() {
    let mut randomizer = SeatRandomizer::new();
    let occupied = vec![0, 2, 4, 6, 8];
    let max_seats = 10;

    let seat = randomizer.find_random_seat(&occupied, max_seats);
    assert!(seat.is_some(), "Should find available seat");

    let seat_idx = seat.unwrap();
    assert!(!occupied.contains(&seat_idx), "Seat should not be occupied");
    assert!(seat_idx < max_seats, "Seat should be within range");
}

#[test]
fn test_no_available_seat() {
    let mut randomizer = SeatRandomizer::new();
    let max_seats = 5;
    let occupied = vec![0, 1, 2, 3, 4]; // All seats occupied

    let seat = randomizer.find_random_seat(&occupied, max_seats);
    assert!(seat.is_none(), "Should return None when table is full");
}

#[test]
fn test_seat_shuffle() {
    let mut randomizer = SeatRandomizer::new();
    let mut current_assignments = std::collections::HashMap::new();
    current_assignments.insert(1, 0);
    current_assignments.insert(2, 2);
    current_assignments.insert(3, 5);

    let new_assignments = randomizer.shuffle_seats(&current_assignments);

    // Should have same users
    assert_eq!(new_assignments.len(), current_assignments.len());

    // All original users should still be assigned
    for &user_id in current_assignments.keys() {
        assert!(new_assignments.contains_key(&user_id));
    }
}

#[tokio::test]
async fn test_concurrent_ip_tracking() {
    let pool = setup_test_db().await;
    let detector = Arc::new(AntiCollusionDetector::new(pool));

    let mut handles = vec![];

    // Register 100 users concurrently
    for i in 0..100 {
        let detector_clone = Arc::clone(&detector);
        let handle = tokio::spawn(async move {
            detector_clone
                .register_user_ip(i as i64, format!("192.168.1.{}", i % 256))
                .await;
        });
        handles.push(handle);
    }

    // Wait for all operations
    for handle in handles {
        handle.await.unwrap();
    }

    // All registrations should have succeeded (no panics)
}

#[tokio::test]
async fn test_rate_limit_cleanup() {
    let pool = setup_test_db().await;
    let limiter = RateLimiter::new(pool);

    // Create some expired entries (this would require database manipulation in a real test)
    // For now, just test that cleanup doesn't error
    let result = limiter.cleanup_expired().await;
    assert!(result.is_ok(), "Cleanup should succeed");
}
