//! Prometheus metrics endpoint handler
//!
//! Provides /metrics endpoint for Prometheus scraping.

use axum::{
    body::Body,
    http::{Response, StatusCode},
    response::IntoResponse,
};
use prometheus::{Encoder, TextEncoder};

/// Handler for /metrics endpoint
///
/// Returns Prometheus metrics in text format for scraping.
/// This endpoint should be accessible without authentication for Prometheus,
/// but should be restricted by network policies in production.
pub async fn metrics_handler() -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();

    match encoder.encode(&metric_families, &mut buffer) {
        Ok(_) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/plain; version=0.0.4; charset=utf-8")
            .body(Body::from(buffer))
            .unwrap(),
        Err(e) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from(format!("Failed to encode metrics: {}", e)))
            .unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    #[tokio::test]
    async fn test_metrics_endpoint() {
        let response = metrics_handler().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);

        let content_type = response.headers().get("content-type").unwrap();
        assert!(content_type
            .to_str()
            .unwrap()
            .contains("text/plain; version=0.0.4"));
    }

    #[tokio::test]
    async fn test_metrics_content() {
        use axum::body::to_bytes;

        // Increment some metrics first
        akidb_core::metrics::API_REQUEST_COUNT
            .with_label_values(&["GET", "/test", "200"])
            .inc();

        let response = metrics_handler().await.into_response();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();

        // Verify Prometheus format
        assert!(text.contains("# HELP akidb_api_requests_total"));
        assert!(text.contains("# TYPE akidb_api_requests_total counter"));
        assert!(text.contains("akidb_api_requests_total"));
    }
}
