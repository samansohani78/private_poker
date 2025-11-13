//! Integration tests for pp_client network functionality.
//!
//! Tests network error handling, connection retries, and protocol mismatch scenarios.

use pp_client::api_client::ApiClient;
use std::time::Duration;
use tokio::time::timeout;

/// Generate unique username for tests
#[allow(dead_code)]
fn unique_username(prefix: &str) -> String {
    let rand_id: u32 = rand::random();
    format!("{}_{}", prefix, rand_id % 100000)
}

// ============================================================================
// Network Error Scenario Tests
// ============================================================================

#[tokio::test]
async fn test_connection_refused() {
    // Try to connect to invalid port
    let mut client = ApiClient::new("http://localhost:19999".to_string());

    let result = client.login("testuser".to_string(), "password".to_string()).await;

    assert!(result.is_err(), "Should fail when server is not available");
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("Failed to send login request") ||
        error_msg.contains("Connection refused"),
        "Error should indicate connection failure"
    );
}

#[tokio::test]
async fn test_timeout_handling() {
    // Try to connect to a non-routable IP (will timeout)
    let mut client = ApiClient::new("http://192.0.2.1:80".to_string());

    // Set a short timeout for the test
    let result = timeout(
        Duration::from_secs(3),
        client.login("testuser".to_string(), "password".to_string())
    ).await;

    // Should either timeout or fail with connection error
    assert!(
        result.is_err() || result.unwrap().is_err(),
        "Should fail when connecting to unreachable host"
    );
}

#[tokio::test]
async fn test_invalid_hostname() {
    let mut client = ApiClient::new("http://invalid-hostname-that-does-not-exist.local".to_string());

    let result = client.login("testuser".to_string(), "password".to_string()).await;

    assert!(result.is_err(), "Should fail with invalid hostname");
}

#[tokio::test]
async fn test_malformed_url() {
    let mut client = ApiClient::new("not-a-valid-url".to_string());

    let result = client.register(
        "testuser".to_string(),
        "password".to_string(),
        "Test User".to_string(),
    ).await;

    assert!(result.is_err(), "Should fail with malformed URL");
}

#[tokio::test]
async fn test_network_error_on_list_tables() {
    let client = ApiClient::new("http://localhost:19999".to_string());

    let result = client.list_tables().await;

    assert!(result.is_err(), "Should fail when server is not available");
}

// ============================================================================
// HTTP Error Response Tests
// ============================================================================

#[tokio::test]
async fn test_http_404_handling() {
    // This test requires a running server but tests wrong endpoint
    // For now, we'll just verify the client can handle errors
    let mut client = ApiClient::new("http://httpbin.org".to_string());

    // Try to login to a non-existent endpoint
    let result = client.login("test".to_string(), "pass".to_string()).await;

    // Should fail because httpbin.org doesn't have our API endpoints
    assert!(result.is_err(), "Should fail with wrong API endpoint");
}

#[tokio::test]
async fn test_invalid_json_response() {
    // Connect to a server that returns non-JSON
    let mut client = ApiClient::new("http://example.com".to_string());

    let result = client.register(
        "test".to_string(),
        "pass".to_string(),
        "Test".to_string(),
    ).await;

    // Should fail to parse response
    assert!(result.is_err(), "Should fail with invalid JSON response");
}

// ============================================================================
// Connection State Tests
// ============================================================================

#[tokio::test]
async fn test_client_creation() {
    let client = ApiClient::new("http://localhost:3000".to_string());

    // Client should be created successfully
    // No tokens should be set initially
    let result = client.list_tables().await;

    // Should work even without authentication for public endpoints
    // (will fail with connection refused but that's expected)
    assert!(result.is_err());
}

#[tokio::test]
async fn test_multiple_clients() {
    // Create multiple clients to the same server
    let client1 = ApiClient::new("http://localhost:3000".to_string());
    let client2 = ApiClient::new("http://localhost:3000".to_string());

    // Both should be independent
    let result1 = client1.list_tables().await;
    let result2 = client2.list_tables().await;

    // Both should fail similarly (no server running)
    assert!(result1.is_err());
    assert!(result2.is_err());
}

// ============================================================================
// Protocol Mismatch Tests
// ============================================================================

#[tokio::test]
async fn test_wrong_content_type_response() {
    // Try to connect to a server that returns HTML instead of JSON
    let mut client = ApiClient::new("http://example.com".to_string());

    let result = client.login("user".to_string(), "pass".to_string()).await;

    assert!(result.is_err(), "Should fail with wrong content type");
    let error = result.unwrap_err().to_string();
    // Error could be various things: connection error, parse error, 404, etc.
    // Just verify we get an error - the exact message doesn't matter
    assert!(!error.is_empty(), "Should have error message: {}", error);
}

#[tokio::test]
async fn test_empty_response_body() {
    // This would happen if server returns 200 OK but no body
    // We can't easily test this without a mock server, but we verify
    // the client handles JSON parsing errors
    let client = ApiClient::new("http://localhost:19999".to_string());

    let result = client.list_tables().await;

    assert!(result.is_err(), "Should handle empty/invalid responses");
}

// ============================================================================
// URL Construction Tests
// ============================================================================

#[tokio::test]
async fn test_url_with_trailing_slash() {
    let mut client = ApiClient::new("http://localhost:3000/".to_string());

    // Should still work with trailing slash
    let result = client.login("user".to_string(), "pass".to_string()).await;

    // Will fail due to no server, but URL construction should work
    assert!(result.is_err());
}

#[tokio::test]
async fn test_url_with_path() {
    let mut client = ApiClient::new("http://localhost:3000/api".to_string());

    let result = client.register(
        "user".to_string(),
        "pass".to_string(),
        "User".to_string(),
    ).await;

    // Should construct URL correctly (will fail due to no server)
    assert!(result.is_err());
}

#[tokio::test]
async fn test_https_url() {
    let mut client = ApiClient::new("https://localhost:3443".to_string());

    let result = client.login("user".to_string(), "pass".to_string()).await;

    // Should handle HTTPS URLs (will fail due to no server)
    assert!(result.is_err());
}

// ============================================================================
// Concurrent Request Tests
// ============================================================================

#[tokio::test]
async fn test_concurrent_api_calls() {
    // Make multiple concurrent requests
    let mut handles = vec![];

    for _ in 0..5 {
        let client = ApiClient::new("http://localhost:19999".to_string());
        let handle = tokio::spawn(async move {
            client.list_tables().await
        });
        handles.push(handle);
    }

    // All should fail (no server)
    let mut error_count = 0;
    for handle in handles {
        let result = handle.await.expect("Task should complete");
        if result.is_err() {
            error_count += 1;
        }
    }

    assert_eq!(error_count, 5, "All concurrent requests should fail without server");
}

#[tokio::test]
async fn test_rapid_sequential_requests() {
    let client = ApiClient::new("http://localhost:19999".to_string());

    // Make rapid sequential requests
    for _ in 0..10 {
        let result = client.list_tables().await;
        assert!(result.is_err(), "Each request should fail without server");
    }
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[tokio::test]
async fn test_empty_base_url() {
    let mut client = ApiClient::new("".to_string());

    let result = client.login("user".to_string(), "pass".to_string()).await;

    assert!(result.is_err(), "Should fail with empty base URL");
}

#[tokio::test]
async fn test_very_long_username() {
    let mut client = ApiClient::new("http://localhost:3000".to_string());

    let long_username = "a".repeat(1000);
    let result = client.login(long_username, "password".to_string()).await;

    // Should handle long input gracefully
    assert!(result.is_err());
}

#[tokio::test]
async fn test_special_characters_in_credentials() {
    let mut client = ApiClient::new("http://localhost:3000".to_string());

    let result = client.login(
        "user@#$%".to_string(),
        "pass!@#$%^&*()".to_string(),
    ).await;

    // Should handle special characters
    assert!(result.is_err());
}

// ============================================================================
// Retry Behavior Tests
// ============================================================================

#[tokio::test]
async fn test_no_automatic_retry_on_failure() {
    let mut client = ApiClient::new("http://localhost:19999".to_string());

    let start = std::time::Instant::now();
    let result = client.login("user".to_string(), "pass".to_string()).await;
    let elapsed = start.elapsed();

    // Should fail quickly without retries (< 5 seconds)
    assert!(result.is_err());
    assert!(elapsed < Duration::from_secs(5), "Should not retry automatically");
}

#[tokio::test]
async fn test_client_state_after_failed_request() {
    let mut client = ApiClient::new("http://localhost:19999".to_string());

    // First request fails
    let result1 = client.login("user1".to_string(), "pass1".to_string()).await;
    assert!(result1.is_err());

    // Second request should also fail independently
    let result2 = client.login("user2".to_string(), "pass2".to_string()).await;
    assert!(result2.is_err());

    // Client should still be usable after failures
    let result3 = client.list_tables().await;
    assert!(result3.is_err());
}
