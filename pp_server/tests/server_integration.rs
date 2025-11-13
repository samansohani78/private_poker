//! Integration tests for HTTP/WebSocket server functionality.
//!
//! Tests timeout handling, connection management, and rate limiting.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use private_poker::auth::{AuthManager, RegisterRequest};
use private_poker::db::{Database, DatabaseConfig};
use private_poker::table::TableManager;
use private_poker::wallet::WalletManager;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tower::ServiceExt; // For `oneshot` method

/// Helper to create test database pool
async fn setup_test_db() -> Arc<sqlx::PgPool> {
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

/// Helper to create test server with managers
async fn create_test_server() -> (axum::Router, Arc<AuthManager>, Arc<TableManager>) {
    let pool = setup_test_db().await;

    let wallet_manager = Arc::new(WalletManager::new(pool.clone()));
    let table_manager = Arc::new(TableManager::new(pool.clone(), wallet_manager.clone()));

    let pepper = "test_pepper_for_testing_only";
    let jwt_secret = "test_secret_key_for_testing_only";
    let auth_manager = Arc::new(AuthManager::new(
        pool.clone(),
        pepper.to_string(),
        jwt_secret.to_string(),
    ));

    let state = pp_server::api::AppState {
        auth_manager: auth_manager.clone(),
        table_manager: table_manager.clone(),
        wallet_manager,
    };

    let app = pp_server::api::create_router(state);

    (app, auth_manager, table_manager)
}

/// Generate unique username for tests
fn unique_username(prefix: &str) -> String {
    let rand_id: u32 = rand::random();
    format!("{}_{}", prefix, rand_id % 100000)
}

// ============================================================================
// Health Check Tests
// ============================================================================

#[tokio::test]
async fn test_health_check_endpoint() {
    let (app, _, _) = create_test_server().await;

    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"OK");
}

// ============================================================================
// Timeout Handling Tests
// ============================================================================

#[tokio::test]
async fn test_request_timeout_handling() {
    let (app, _, _) = create_test_server().await;

    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    // Test that normal requests complete within timeout
    let result = timeout(Duration::from_secs(5), app.oneshot(request)).await;

    assert!(result.is_ok(), "Request should complete within timeout");
    assert_eq!(result.unwrap().unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn test_database_connection_timeout() {
    // Create database config with very short timeout
    let config = DatabaseConfig {
        database_url: "postgres://invalid_user:invalid_pass@localhost:9999/invalid_db".to_string(),
        max_connections: 1,
        min_connections: 1,
        connection_timeout_secs: 1, // Very short timeout
        idle_timeout_secs: 300,
        max_lifetime_secs: 1800,
    };

    // Attempt to connect should fail quickly due to timeout
    let start = std::time::Instant::now();
    let result = Database::new(&config).await;
    let elapsed = start.elapsed();

    assert!(
        result.is_err(),
        "Connection to invalid database should fail"
    );
    assert!(
        elapsed < Duration::from_secs(3),
        "Should timeout within configured time"
    );
}

// ============================================================================
// Authentication Endpoint Tests
// ============================================================================

#[tokio::test]
async fn test_register_endpoint() {
    let (app, _, _) = create_test_server().await;

    let username = unique_username("reg");
    let register_data = serde_json::json!({
        "username": username,
        "password": "TestPass123!",
        "display_name": "Test User",
        "email": format!("{}@test.com", username)
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&register_data).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_login_endpoint() {
    let (app, auth_manager, _) = create_test_server().await;

    // Create user first
    let username = unique_username("login");
    let register_req = RegisterRequest {
        username: username.clone(),
        password: "TestPass123!".to_string(),
        display_name: "Test User".to_string(),
        email: Some(format!("{}@test.com", username)),
    };
    auth_manager.register(register_req).await.unwrap();

    // Test login
    let login_data = serde_json::json!({
        "username": username,
        "password": "TestPass123!",
        "totp_code": null
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&login_data).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_invalid_login_returns_error() {
    let (app, _, _) = create_test_server().await;

    let login_data = serde_json::json!({
        "username": "nonexistent_user",
        "password": "WrongPassword123!",
        "totp_code": null
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&login_data).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert!(
        response.status() == StatusCode::UNAUTHORIZED
            || response.status() == StatusCode::BAD_REQUEST
            || response.status() == StatusCode::INTERNAL_SERVER_ERROR,
        "Invalid login should return error status"
    );
}

// ============================================================================
// Table Endpoint Tests
// ============================================================================

#[tokio::test]
async fn test_list_tables_endpoint() {
    let (app, _, _) = create_test_server().await;

    let request = Request::builder()
        .uri("/api/tables")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[tokio::test]
async fn test_404_for_invalid_endpoint() {
    let (app, _, _) = create_test_server().await;

    let request = Request::builder()
        .uri("/api/invalid/endpoint")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_malformed_json_request() {
    let (app, _, _) = create_test_server().await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from("{ invalid json }"))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert!(
        response.status() == StatusCode::BAD_REQUEST
            || response.status() == StatusCode::UNPROCESSABLE_ENTITY,
        "Malformed JSON should return 400 or 422"
    );
}

// ============================================================================
// CORS Tests
// ============================================================================

#[tokio::test]
async fn test_cors_headers_present() {
    let (app, _, _) = create_test_server().await;

    let request = Request::builder()
        .uri("/health")
        .header("Origin", "http://example.com")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // CORS should allow the request
    assert_eq!(response.status(), StatusCode::OK);

    // Check for CORS headers
    let headers = response.headers();
    assert!(
        headers.contains_key("access-control-allow-origin")
            || headers.contains_key("Access-Control-Allow-Origin"),
        "CORS headers should be present"
    );
}

// ============================================================================
// Connection Drop Tests
// ============================================================================

#[tokio::test]
async fn test_graceful_shutdown_doesnt_crash() {
    // This test verifies that the shutdown signal handler is properly set up
    // We can't actually test the full shutdown without killing the process,
    // but we can verify the app is constructed correctly
    let (_, _, _) = create_test_server().await;

    // If we get here without panicking, the server setup is correct
    // (no assertion needed - panic = test failure)
}

#[tokio::test]
async fn test_multiple_requests_same_connection() {
    let (app, _, _) = create_test_server().await;

    // Make multiple requests to simulate connection reuse
    for _ in 0..5 {
        let app_clone = app.clone();
        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app_clone.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}

// ============================================================================
// Concurrent Request Tests
// ============================================================================

#[tokio::test]
async fn test_concurrent_health_checks() {
    let (app, _, _) = create_test_server().await;

    let mut handles = Vec::new();

    for _ in 0..10 {
        let app_clone = app.clone();
        let handle = tokio::spawn(async move {
            let request = Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap();
            app_clone.oneshot(request).await
        });
        handles.push(handle);
    }

    let mut success_count = 0;
    for handle in handles {
        let response = handle.await.expect("Task should complete").unwrap();
        if response.status() == StatusCode::OK {
            success_count += 1;
        }
    }

    assert_eq!(success_count, 10, "All concurrent requests should succeed");
}

#[tokio::test]
async fn test_concurrent_registration() {
    let (app, _, _) = create_test_server().await;

    let mut handles = Vec::new();

    for i in 0..5 {
        let app_clone = app.clone();
        let username = unique_username(&format!("conc{}", i));
        let handle = tokio::spawn(async move {
            let email = format!("{}@test.com", username);
            let register_data = serde_json::json!({
                "username": username,
                "password": "TestPass123!",
                "display_name": format!("User {}", username),
                "email": email
            });

            let request = Request::builder()
                .method("POST")
                .uri("/api/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&register_data).unwrap()))
                .unwrap();

            app_clone.oneshot(request).await
        });
        handles.push(handle);
    }

    let mut success_count = 0;
    for handle in handles {
        let response = handle.await.expect("Task should complete").unwrap();
        if response.status() == StatusCode::OK {
            success_count += 1;
        }
    }

    assert!(
        success_count >= 3,
        "Most concurrent registrations should succeed, got {}",
        success_count
    );
}

// ============================================================================
// Rate Limiter Edge Case Tests
// ============================================================================

#[tokio::test]
async fn test_rapid_requests_dont_crash_server() {
    let (app, _, _) = create_test_server().await;

    // Make many rapid requests to test server stability
    let mut handles = Vec::new();

    for _ in 0..20 {
        let app_clone = app.clone();
        let handle = tokio::spawn(async move {
            let request = Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap();
            app_clone.oneshot(request).await
        });
        handles.push(handle);
    }

    let mut completed_count = 0;
    for handle in handles {
        if handle.await.is_ok() {
            completed_count += 1;
        }
    }

    // All requests should complete (even if some might be rate limited)
    assert!(
        completed_count >= 15,
        "Most rapid requests should complete without crashing"
    );
}

#[tokio::test]
async fn test_empty_request_body_handled_gracefully() {
    let (app, _, _) = create_test_server().await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return an error status, not crash
    assert!(
        response.status().is_client_error() || response.status().is_server_error(),
        "Empty body should be handled gracefully"
    );
}
