//! Admin REST endpoints for operational management (Phase 7 Week 4)
//!
//! Provides 3 critical operational endpoints:
//! 1. GET /admin/health - Comprehensive health check
//! 2. POST /admin/collections/{id}/dlq/retry - DLQ retry (clear)
//! 3. POST /admin/circuit-breaker/reset - Circuit breaker reset

use akidb_core::CollectionId;
use akidb_service::CollectionService;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use std::str::FromStr;
use std::sync::Arc;

// ============================================================================
// Health Check
// ============================================================================

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub components: HealthComponents,
}

#[derive(Debug, Serialize)]
pub struct HealthComponents {
    pub database: ComponentHealth,
    pub storage: ComponentHealth,
    pub memory: ComponentHealth,
}

#[derive(Debug, Serialize)]
pub struct ComponentHealth {
    pub status: String,
    pub message: Option<String>,
    pub details: Option<serde_json::Value>,
}

impl ComponentHealth {
    fn healthy() -> Self {
        Self {
            status: "healthy".to_string(),
            message: None,
            details: None,
        }
    }

    fn unhealthy(message: String) -> Self {
        Self {
            status: "unhealthy".to_string(),
            message: Some(message),
            details: None,
        }
    }

    fn degraded(message: String) -> Self {
        Self {
            status: "degraded".to_string(),
            message: Some(message),
            details: None,
        }
    }

    fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

/// GET /admin/health
///
/// Comprehensive health check for Kubernetes liveness/readiness probes
pub async fn health_check(
    State(service): State<Arc<CollectionService>>,
) -> Result<Json<HealthResponse>, (StatusCode, String)> {
    // Check database connectivity
    let database_health = match service.list_collections().await {
        Ok(_) => ComponentHealth::healthy(),
        Err(e) => ComponentHealth::unhealthy(format!("Database error: {}", e)),
    };

    // Check storage backend
    let storage_health = match service.get_storage_metrics().await {
        Ok(metrics) => {
            let circuit_breaker_state = metrics.circuit_breaker_state;

            // 0 = Closed, 1 = HalfOpen, 2 = Open
            match circuit_breaker_state {
                0 => ComponentHealth::healthy().with_details(serde_json::json!({
                    "circuit_breaker": "closed",
                    "s3_uploads": metrics.s3_uploads,
                    "s3_permanent_failures": metrics.s3_permanent_failures,
                })),
                1 => {
                    ComponentHealth::degraded("Circuit breaker half-open (recovering)".to_string())
                        .with_details(serde_json::json!({
                            "circuit_breaker": "half_open",
                        }))
                }
                _ => ComponentHealth::unhealthy("Circuit breaker open (S3 failing)".to_string())
                    .with_details(serde_json::json!({
                        "circuit_breaker": "open",
                    })),
            }
        }
        Err(e) => ComponentHealth::unhealthy(format!("Storage metrics error: {}", e)),
    };

    // Check memory usage
    let memory_health = match service.get_cache_stats().await {
        Ok(stats) => {
            let usage_percent = if stats.capacity > 0 {
                (stats.size as f64 / stats.capacity as f64) * 100.0
            } else {
                0.0
            };

            if usage_percent >= 90.0 {
                ComponentHealth::unhealthy(format!("Memory usage critical: {:.1}%", usage_percent))
                    .with_details(serde_json::json!({
                        "usage_percent": usage_percent,
                        "size_bytes": stats.size,
                        "capacity_bytes": stats.capacity,
                    }))
            } else if usage_percent >= 75.0 {
                ComponentHealth::degraded(format!("Memory usage high: {:.1}%", usage_percent))
                    .with_details(serde_json::json!({
                        "usage_percent": usage_percent,
                        "size_bytes": stats.size,
                        "capacity_bytes": stats.capacity,
                    }))
            } else {
                ComponentHealth::healthy().with_details(serde_json::json!({
                    "usage_percent": usage_percent,
                    "size_bytes": stats.size,
                    "capacity_bytes": stats.capacity,
                    "hit_rate_percent": stats.hit_rate * 100.0,
                }))
            }
        }
        Err(e) => ComponentHealth::unhealthy(format!("Memory stats error: {}", e)),
    };

    // Overall status
    let overall_status = if database_health.status == "unhealthy"
        || storage_health.status == "unhealthy"
        || memory_health.status == "unhealthy"
    {
        "unhealthy"
    } else if database_health.status == "degraded"
        || storage_health.status == "degraded"
        || memory_health.status == "degraded"
    {
        "degraded"
    } else {
        "healthy"
    };

    let uptime = service.uptime_seconds();

    let response = HealthResponse {
        status: overall_status.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: uptime,
        components: HealthComponents {
            database: database_health,
            storage: storage_health,
            memory: memory_health,
        },
    };

    // Return 503 Service Unavailable if unhealthy
    if overall_status == "unhealthy" {
        Err((
            StatusCode::SERVICE_UNAVAILABLE,
            serde_json::to_string(&response).unwrap(),
        ))
    } else {
        Ok(Json(response))
    }
}

// ============================================================================
// DLQ Retry
// ============================================================================

#[derive(Debug, Serialize)]
pub struct DLQRetryResponse {
    pub collection_id: String,
    pub retried_count: usize,
    pub success_count: usize,
    pub failed_count: usize,
}

/// POST /admin/collections/{id}/dlq/retry
///
/// Retry all DLQ entries for a collection (currently clears the DLQ)
pub async fn retry_dlq(
    State(service): State<Arc<CollectionService>>,
    Path(collection_id): Path<String>,
) -> Result<Json<DLQRetryResponse>, (StatusCode, String)> {
    let collection_id = CollectionId::from_str(&collection_id).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid collection ID: {}", e),
        )
    })?;

    match service.retry_dlq_entries(collection_id).await {
        Ok(result) => Ok(Json(DLQRetryResponse {
            collection_id: collection_id.to_string(),
            retried_count: result.total,
            success_count: result.succeeded,
            failed_count: result.failed,
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("DLQ retry failed: {}", e),
        )),
    }
}

// ============================================================================
// Circuit Breaker Reset
// ============================================================================

#[derive(Debug, Serialize)]
pub struct CircuitBreakerResetResponse {
    pub status: String,
    pub message: String,
    pub previous_state: String,
    pub new_state: String,
}

/// POST /admin/circuit-breaker/reset
///
/// Reset circuit breaker to Closed state (emergency recovery)
pub async fn reset_circuit_breaker(
    State(service): State<Arc<CollectionService>>,
) -> Result<Json<CircuitBreakerResetResponse>, (StatusCode, String)> {
    match service.reset_circuit_breaker().await {
        Ok(previous_state) => Ok(Json(CircuitBreakerResetResponse {
            status: "success".to_string(),
            message: "Circuit breaker reset successfully".to_string(),
            previous_state: format!("{:?}", previous_state),
            new_state: "Closed".to_string(),
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Circuit breaker reset failed: {}", e),
        )),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_response_structure() {
        let response = HealthResponse {
            status: "healthy".to_string(),
            version: "2.0.0".to_string(),
            uptime_seconds: 3600,
            components: HealthComponents {
                database: ComponentHealth::healthy(),
                storage: ComponentHealth::healthy(),
                memory: ComponentHealth::healthy(),
            },
        };

        assert_eq!(response.status, "healthy");
        assert_eq!(response.version, "2.0.0");
        assert_eq!(response.uptime_seconds, 3600);
    }

    #[test]
    fn test_component_health_states() {
        let healthy = ComponentHealth::healthy();
        assert_eq!(healthy.status, "healthy");
        assert!(healthy.message.is_none());

        let unhealthy = ComponentHealth::unhealthy("Database down".to_string());
        assert_eq!(unhealthy.status, "unhealthy");
        assert_eq!(unhealthy.message, Some("Database down".to_string()));

        let degraded = ComponentHealth::degraded("High latency".to_string());
        assert_eq!(degraded.status, "degraded");
        assert_eq!(degraded.message, Some("High latency".to_string()));
    }

    #[test]
    fn test_dlq_retry_response_structure() {
        let response = DLQRetryResponse {
            collection_id: "test-id".to_string(),
            retried_count: 10,
            success_count: 8,
            failed_count: 2,
        };

        assert_eq!(response.retried_count, 10);
        assert_eq!(response.success_count, 8);
        assert_eq!(response.failed_count, 2);
    }

    #[test]
    fn test_circuit_breaker_reset_response_structure() {
        let response = CircuitBreakerResetResponse {
            status: "success".to_string(),
            message: "Reset".to_string(),
            previous_state: "Open".to_string(),
            new_state: "Closed".to_string(),
        };

        assert_eq!(response.previous_state, "Open");
        assert_eq!(response.new_state, "Closed");
    }
}
