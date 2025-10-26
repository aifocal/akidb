//! Metrics middleware for tracking API requests
//!
//! This middleware automatically records Prometheus metrics for all HTTP requests,
//! including request count, duration, and active connections.

use axum::{
    body::Body,
    extract::MatchedPath,
    http::{Request, Response},
    middleware::Next,
};
use std::time::Instant;

/// Middleware to track API request metrics
///
/// This middleware:
/// - Increments active connections counter
/// - Records request duration histogram
/// - Counts requests by method, endpoint, and status code
/// - Decrements active connections when request completes
pub async fn track_metrics(req: Request<Body>, next: Next) -> Response<Body> {
    let start = Instant::now();
    let method = req.method().clone();

    // Extract matched path for consistent labeling
    // Fall back to "unknown" if path matching fails
    let path = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_owned())
        .unwrap_or_else(|| "unknown".to_string());

    // Increment active connections
    akidb_core::metrics::ACTIVE_CONNECTIONS.inc();

    // Execute the request
    let response = next.run(req).await;

    // Calculate duration
    let duration = start.elapsed();
    let status = response.status().as_u16().to_string();

    // Decrement active connections
    akidb_core::metrics::ACTIVE_CONNECTIONS.dec();

    // Record metrics
    akidb_core::metrics::API_REQUEST_COUNT
        .with_label_values(&[method.as_str(), &path, &status])
        .inc();

    akidb_core::metrics::API_REQUEST_DURATION
        .with_label_values(&[method.as_str(), &path])
        .observe(duration.as_secs_f64());

    response
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
        "OK"
    }

    #[tokio::test]
    async fn test_metrics_middleware() {
        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(middleware::from_fn(track_metrics));

        let response = app
            .oneshot(Request::builder().uri("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // TODO: Verify metrics were recorded
        // Requires prometheus dependency in akidb-api Cargo.toml
        // let metrics = prometheus::gather();
        // let api_metrics: Vec<_> = metrics
        //     .iter()
        //     .filter(|m| m.get_name().starts_with("akidb_api"))
        //     .collect();
        // assert!(!api_metrics.is_empty(), "API metrics should be recorded");
    }
}
