//! API Key authentication middleware

use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use tracing::{debug, warn};

/// API key configuration
#[derive(Clone)]
pub struct AuthConfig {
    /// Valid API keys (in production, use env var or secrets manager)
    pub api_keys: Arc<Vec<String>>,
    /// Whether auth is enabled (for testing)
    pub enabled: bool,
}

impl Default for AuthConfig {
    fn default() -> Self {
        // By default, require authentication
        Self {
            api_keys: Arc::new(vec![]),
            enabled: true,
        }
    }
}

impl AuthConfig {
    /// Create config from environment variable
    pub fn from_env() -> Self {
        let enabled = std::env::var("AKIDB_AUTH_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .parse()
            .unwrap_or(true);

        let api_keys: Vec<String> = std::env::var("AKIDB_API_KEYS")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_string())
            .collect();

        debug!(
            "Auth configured: enabled={}, key_count={}",
            enabled,
            api_keys.len()
        );

        Self {
            api_keys: Arc::new(api_keys),
            enabled,
        }
    }

    /// Create config for testing (no keys required)
    pub fn disabled() -> Self {
        Self {
            api_keys: Arc::new(vec![]),
            enabled: false,
        }
    }

    /// Create config with specific keys
    pub fn with_keys(keys: Vec<String>) -> Self {
        Self {
            api_keys: Arc::new(keys),
            enabled: true,
        }
    }
}

/// Extract API key from request headers
fn extract_api_key(headers: &HeaderMap) -> Option<String> {
    // Try Authorization header first (Bearer token)
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(key) = auth_str.strip_prefix("Bearer ") {
                return Some(key.to_string());
            }
        }
    }

    // Try X-API-Key header
    if let Some(api_key_header) = headers.get("x-api-key") {
        if let Ok(key) = api_key_header.to_str() {
            return Some(key.to_string());
        }
    }

    None
}

/// Authentication middleware
pub async fn auth_middleware(config: Arc<AuthConfig>, request: Request, next: Next) -> Response {
    // Skip auth if disabled (for testing)
    if !config.enabled {
        debug!("Authentication disabled, allowing request");
        return next.run(request).await;
    }

    // Skip auth for health check endpoint
    if request.uri().path() == "/health" {
        return next.run(request).await;
    }

    // Extract API key
    let api_key = match extract_api_key(request.headers()) {
        Some(key) => key,
        None => {
            warn!("Missing API key in request to {}", request.uri().path());
            return (
                StatusCode::UNAUTHORIZED,
                "Missing API key. Provide via Authorization: Bearer <key> or X-API-Key header",
            )
                .into_response();
        }
    };

    // Validate API key
    if !config.api_keys.contains(&api_key) {
        warn!("Invalid API key for request to {}", request.uri().path());
        return (StatusCode::UNAUTHORIZED, "Invalid API key").into_response();
    }

    // Key is valid, proceed
    debug!("API key validated for {}", request.uri().path());
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        middleware,
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    async fn test_handler() -> &'static str {
        "ok"
    }

    #[tokio::test]
    async fn test_auth_disabled() {
        let config = Arc::new(AuthConfig::disabled());
        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(middleware::from_fn(move |req, next| {
                let config = config.clone();
                auth_middleware(config, req, next)
            }));

        let response = app
            .oneshot(Request::builder().uri("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_auth_missing_key() {
        let config = Arc::new(AuthConfig::with_keys(vec!["test-key".to_string()]));
        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(middleware::from_fn(move |req, next| {
                let config = config.clone();
                auth_middleware(config, req, next)
            }));

        let response = app
            .oneshot(Request::builder().uri("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_invalid_key() {
        let config = Arc::new(AuthConfig::with_keys(vec!["valid-key".to_string()]));
        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(middleware::from_fn(move |req, next| {
                let config = config.clone();
                auth_middleware(config, req, next)
            }));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/test")
                    .header("Authorization", "Bearer invalid-key")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_valid_bearer_token() {
        let config = Arc::new(AuthConfig::with_keys(vec!["test-key".to_string()]));
        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(middleware::from_fn(move |req, next| {
                let config = config.clone();
                auth_middleware(config, req, next)
            }));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/test")
                    .header("Authorization", "Bearer test-key")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_auth_valid_api_key_header() {
        let config = Arc::new(AuthConfig::with_keys(vec!["test-key".to_string()]));
        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(middleware::from_fn(move |req, next| {
                let config = config.clone();
                auth_middleware(config, req, next)
            }));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/test")
                    .header("X-API-Key", "test-key")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_health_check_bypasses_auth() {
        let config = Arc::new(AuthConfig::with_keys(vec!["test-key".to_string()]));
        let app = Router::new()
            .route("/health", get(test_handler))
            .layer(middleware::from_fn(move |req, next| {
                let config = config.clone();
                auth_middleware(config, req, next)
            }));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
