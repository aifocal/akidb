//! API Key authentication middleware

use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use subtle::ConstantTimeEq;
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
    ///
    /// CRITICAL FIX (Bug #48): If auth is enabled but no API keys are configured,
    /// fail fast at startup instead of silently rejecting all requests.
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

        // CRITICAL SECURITY FIX (Bug #48): Fail fast if auth is enabled but no keys configured
        if enabled && api_keys.is_empty() {
            panic!(
                "FATAL: Authentication is enabled (AKIDB_AUTH_ENABLED=true) but no API keys are configured.\n\
                 \n\
                 To fix this issue, choose ONE of the following options:\n\
                 \n\
                 Option 1 (RECOMMENDED for production):\n\
                   Set AKIDB_API_KEYS environment variable with comma-separated API keys:\n\
                   export AKIDB_API_KEYS=\"key1,key2,key3\"\n\
                 \n\
                 Option 2 (INSECURE - development/testing only):\n\
                   Disable authentication:\n\
                   export AKIDB_AUTH_ENABLED=false\n\
                 \n\
                 Refusing to start with enabled auth and no API keys to prevent \n\
                 accidentally running an inaccessible API server."
            );
        }

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
    // IMPORTANT: Try X-API-Key header FIRST to allow coexistence with RBAC JWT middleware
    // If X-API-Key is present, use it for API key authentication
    // This allows Authorization: Bearer to be used for JWT tokens by RBAC middleware
    if let Some(api_key_header) = headers.get("x-api-key") {
        if let Ok(key) = api_key_header.to_str() {
            return Some(key.to_string());
        }
    }

    // Fallback to Authorization header (Bearer token) only if X-API-Key not present
    // This supports legacy clients that send API keys via Authorization header
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(key) = auth_str.strip_prefix("Bearer ") {
                // Only treat as API key if it starts with "ak_" prefix
                // This prevents consuming JWT tokens meant for RBAC middleware
                if key.starts_with("ak_") {
                    return Some(key.to_string());
                }
            }
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

    // DEFENSE IN DEPTH: Bypass auth for Kubernetes liveness probe even if middleware is applied
    // NOTE (Bug #49 fix): Health and metrics endpoints SHOULD be in separate public routes,
    // but we add this bypass as defense-in-depth in case routes are misconfigured.
    if request.uri().path() == "/health/live" {
        debug!("Bypassing auth for Kubernetes liveness probe: /health/live");
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

    // CRITICAL SECURITY FIX (Bug #33): Use constant-time comparison to prevent timing attacks.
    //
    // VULNERABILITY: Vec::contains() uses optimized string comparison that short-circuits
    // on first mismatch, creating measurable timing differences:
    //
    // Attack scenario:
    // - Valid key: "AtG7xK2p..."
    // - Try "aaaa..." → fails at byte 0 → ~5μs response
    // - Try "Aaaa..." → fails at byte 1 → ~6μs response (LEAKED first byte!)
    // - Try "Ataa..." → fails at byte 2 → ~7μs response (LEAKED second byte!)
    //
    // Attacker can extract full API key using ~256 * key_length timing measurements.
    //
    // FIX: Use constant-time comparison from `subtle` crate which processes
    // all bytes regardless of mismatches, eliminating timing leak.
    let api_key_valid = config.api_keys.iter().any(|valid_key| {
        // Ensure both strings have same length before comparison
        if api_key.len() != valid_key.len() {
            return false;
        }

        // Constant-time comparison (always compares all bytes)
        api_key.as_bytes().ct_eq(valid_key.as_bytes()).into()
    });

    if !api_key_valid {
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
        let config = Arc::new(AuthConfig::with_keys(vec!["ak_test-key".to_string()]));
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
                    .header("Authorization", "Bearer ak_test-key")
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
    async fn test_health_check_requires_auth() {
        let config = Arc::new(AuthConfig::with_keys(vec!["ak_test-key".to_string()]));
        let app = Router::new()
            .route("/health", get(test_handler))
            .layer(middleware::from_fn(move |req, next| {
                let config = config.clone();
                auth_middleware(config, req, next)
            }));

        // Without auth - should fail
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "/health should require authentication to prevent information disclosure"
        );

        // With valid auth - should succeed
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .header("Authorization", "Bearer ak_test-key")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_health_live_bypasses_auth() {
        let config = Arc::new(AuthConfig::with_keys(vec!["test-key".to_string()]));
        let app = Router::new()
            .route("/health/live", get(test_handler))
            .layer(middleware::from_fn(move |req, next| {
                let config = config.clone();
                auth_middleware(config, req, next)
            }));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health/live")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Liveness probe should bypass auth for Kubernetes"
        );
    }

    #[tokio::test]
    async fn test_health_ready_requires_auth() {
        let config = Arc::new(AuthConfig::with_keys(vec!["ak_test-key".to_string()]));
        let app = Router::new()
            .route("/health/ready", get(test_handler))
            .layer(middleware::from_fn(move |req, next| {
                let config = config.clone();
                auth_middleware(config, req, next)
            }));

        // Without auth - should fail
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health/ready")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "/health/ready exposes cluster readiness state and must require auth"
        );

        // With valid auth - should succeed
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health/ready")
                    .header("Authorization", "Bearer ak_test-key")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_metrics_requires_auth() {
        let config = Arc::new(AuthConfig::with_keys(vec!["ak_test-key".to_string()]));
        let app = Router::new()
            .route("/metrics", get(test_handler))
            .layer(middleware::from_fn(move |req, next| {
                let config = config.clone();
                auth_middleware(config, req, next)
            }));

        // Without auth - should fail
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "/metrics exposes sensitive cluster information (WAL sizes, vector counts, etc.) and must require auth"
        );

        // With valid auth - should succeed
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .header("Authorization", "Bearer ak_test-key")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
