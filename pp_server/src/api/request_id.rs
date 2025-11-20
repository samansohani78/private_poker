//! Request ID middleware for tracing and debugging.
//!
//! This module provides request ID generation and propagation for better
//! debugging, log correlation, and distributed tracing support.

use axum::{
    extract::Request,
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

/// Header name for request ID
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Generate or extract request ID from headers
fn get_or_generate_request_id(headers: &axum::http::HeaderMap) -> String {
    headers
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

/// Middleware to add request ID to all requests and responses
///
/// This middleware:
/// 1. Extracts existing request ID from header or generates a new one
/// 2. Adds request ID to response headers
/// 3. Makes request ID available for logging
///
/// # Example
///
/// ```no_run
/// use axum::{Router, routing::get, middleware};
/// use pp_server::api::request_id::request_id_middleware;
///
/// # async fn example() {
/// let app: Router = Router::new()
///     .route("/", get(|| async { "Hello" }))
///     .layer(middleware::from_fn(request_id_middleware));
/// # }
/// ```
pub async fn request_id_middleware(
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Get or generate request ID
    let request_id = get_or_generate_request_id(request.headers());

    // Store request ID in request extensions for access by handlers
    request.extensions_mut().insert(RequestId(request_id.clone()));

    // Log request start with ID
    tracing::info!(
        request_id = %request_id,
        method = %request.method(),
        uri = %request.uri(),
        "Request started"
    );

    // Process request
    let response = next.run(request).await;

    // Add request ID to response headers
    let (mut parts, body) = response.into_parts();
    if let Ok(header_value) = HeaderValue::from_str(&request_id) {
        parts.headers.insert(REQUEST_ID_HEADER, header_value);
    }

    // Log request completion
    tracing::info!(
        request_id = %request_id,
        status = %parts.status,
        "Request completed"
    );

    Ok(Response::from_parts(parts, body))
}

/// Request ID wrapper for extracting from request extensions
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct RequestId(pub String);

impl RequestId {
    /// Get the request ID as a string slice
    #[allow(dead_code)]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Get the request ID as an owned string
    #[allow(dead_code)]
    pub fn into_string(self) -> String {
        self.0
    }
}

/// Axum extractor for request ID
impl<S> axum::extract::FromRequestParts<S> for RequestId
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<RequestId>()
            .cloned()
            .ok_or((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Request ID not found in extensions",
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, header::HeaderMap},
    };

    #[test]
    fn test_get_or_generate_request_id_with_existing() {
        let mut headers = HeaderMap::new();
        headers.insert(REQUEST_ID_HEADER, HeaderValue::from_static("test-id-123"));

        let request_id = get_or_generate_request_id(&headers);
        assert_eq!(request_id, "test-id-123");
    }

    #[test]
    fn test_get_or_generate_request_id_generates_new() {
        let headers = HeaderMap::new();
        let request_id = get_or_generate_request_id(&headers);

        // Should be a valid UUID
        assert!(Uuid::parse_str(&request_id).is_ok());
    }

    #[test]
    fn test_request_id_as_str() {
        let request_id = RequestId("test-123".to_string());
        assert_eq!(request_id.as_str(), "test-123");
    }

    #[test]
    fn test_request_id_into_string() {
        let request_id = RequestId("test-123".to_string());
        assert_eq!(request_id.into_string(), "test-123");
    }
}
