//! Health check endpoints for Kubernetes liveness and readiness probes.
//!
//! # Endpoints
//!
//! - `GET /health` - Liveness probe (is the service alive?)
//! - `GET /ready` - Readiness probe (is the service ready to serve traffic?)

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde_json::json;
use std::sync::Arc;

use akidb_service::CollectionService;

/// Health check handler (Kubernetes liveness probe).
///
/// Returns 200 OK if the service is alive and healthy.
/// Returns 503 Service Unavailable if the service is unhealthy.
///
/// This endpoint should be used for Kubernetes liveness probes.
/// If this endpoint fails, Kubernetes will restart the pod.
///
/// # Kubernetes Configuration
///
/// ```yaml
/// livenessProbe:
///   httpGet:
///     path: /health
///     port: 8080
///   initialDelaySeconds: 10
///   periodSeconds: 10
///   timeoutSeconds: 5
///   failureThreshold: 3
/// ```
pub async fn health_handler(
    State(service): State<Arc<CollectionService>>,
) -> impl IntoResponse {
    if service.is_healthy() {
        (
            StatusCode::OK,
            Json(json!({
                "status": "healthy",
                "message": "Service is alive and operational"
            })),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "unhealthy",
                "message": "Service is not operational"
            })),
        )
    }
}

/// Readiness check handler (Kubernetes readiness probe).
///
/// Returns 200 OK if the service is ready to serve traffic.
/// Returns 503 Service Unavailable if the service is not ready.
///
/// This endpoint should be used for Kubernetes readiness probes.
/// If this endpoint fails, Kubernetes will stop sending traffic to this pod.
///
/// # Readiness Criteria
///
/// - All collections have been loaded from database
/// - All storage backends are operational (circuit breakers not open)
///
/// # Kubernetes Configuration
///
/// ```yaml
/// readinessProbe:
///   httpGet:
///     path: /ready
///     port: 8080
///   initialDelaySeconds: 5
///   periodSeconds: 5
///   timeoutSeconds: 3
///   failureThreshold: 3
/// ```
pub async fn ready_handler(
    State(service): State<Arc<CollectionService>>,
) -> impl IntoResponse {
    if service.is_ready().await {
        (
            StatusCode::OK,
            Json(json!({
                "status": "ready",
                "message": "Service is ready to accept traffic"
            })),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "not_ready",
                "message": "Service is still loading collections or storage backends are unavailable"
            })),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_health_handler_healthy() {
        let service = Arc::new(CollectionService::new());

        let response = health_handler(State(service)).await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_ready_handler() {
        let service = Arc::new(CollectionService::new());

        // Without repository, should always be ready
        let response = ready_handler(State(service)).await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
