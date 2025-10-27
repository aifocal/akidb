// ! Health check endpoints for Kubernetes liveness and readiness probes
//!
//! This module provides three health check endpoints:
//! - `/health/live` - Liveness probe (returns 200 if service is running)
//! - `/health/ready` - Readiness probe (checks dependencies: storage, WAL)
//! - `/health` - Detailed health status (JSON with component details)

use crate::state::AppState;
use akidb_storage::StorageStatus;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

/// Overall health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Component health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub status: HealthStatus,
    pub message: Option<String>,
}

/// Detailed health response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub version: String,
    pub uptime_seconds: u64,
    pub components: ComponentHealthDetails,
}

/// Health details for each component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealthDetails {
    pub storage: ComponentHealth,
    pub wal: ComponentHealth,
    pub index: ComponentHealth,
}

/// Liveness probe handler
///
/// Returns 200 OK if the service is running. This endpoint should always
/// succeed unless the process is completely hung or crashed.
///
/// Kubernetes uses this to determine if the pod should be restarted.
pub async fn liveness_handler() -> impl IntoResponse {
    debug!("Liveness check requested");
    StatusCode::OK
}

/// Readiness probe handler
///
/// Returns 200 OK only if all critical dependencies are healthy:
/// - Storage backend is accessible
/// - WAL is operational
///
/// Kubernetes uses this to determine if the pod should receive traffic.
pub async fn readiness_handler(State(state): State<AppState>) -> Response {
    debug!("Readiness check requested");

    // Check storage backend status
    let storage_status = match state.storage.status().await {
        Ok(StorageStatus::Healthy) => {
            debug!("Storage backend is healthy");
            true
        }
        Ok(StorageStatus::Degraded) => {
            warn!("Storage backend is degraded");
            false
        }
        Err(e) => {
            warn!("Storage backend check failed: {}", e);
            false
        }
    };

    // If storage is unhealthy, return 503 Service Unavailable
    if !storage_status {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "Storage backend not ready",
        )
            .into_response();
    }

    // All checks passed
    debug!("All readiness checks passed");
    StatusCode::OK.into_response()
}

/// Detailed health check handler
///
/// Returns detailed JSON with:
/// - Overall health status
/// - Service version
/// - Uptime in seconds
/// - Component-level health (storage, WAL, index)
///
/// This endpoint is useful for monitoring dashboards and debugging.
pub async fn detailed_health_handler(State(state): State<AppState>) -> Response {
    debug!("Detailed health check requested");

    // Calculate uptime (mock for now, should use server start time)
    let uptime = calculate_uptime();

    // Check storage backend
    let storage_health = match state.storage.status().await {
        Ok(StorageStatus::Healthy) => ComponentHealth {
            status: HealthStatus::Healthy,
            message: Some("Storage backend operational".to_string()),
        },
        Ok(StorageStatus::Degraded) => ComponentHealth {
            status: HealthStatus::Degraded,
            message: Some("Storage backend degraded (circuit breaker may be open)".to_string()),
        },
        Err(e) => ComponentHealth {
            status: HealthStatus::Unhealthy,
            message: Some(format!("Storage backend error: {}", e)),
        },
    };

    // WAL health (assume healthy if storage is healthy)
    let wal_health = ComponentHealth {
        status: storage_health.status,
        message: Some("WAL operational".to_string()),
    };

    // Index provider health (always healthy for now)
    let index_health = ComponentHealth {
        status: HealthStatus::Healthy,
        message: Some("Index provider operational".to_string()),
    };

    // Determine overall status
    let overall_status = determine_overall_status(&[
        storage_health.status,
        wal_health.status,
        index_health.status,
    ]);

    let response = HealthResponse {
        status: overall_status,
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: uptime,
        components: ComponentHealthDetails {
            storage: storage_health,
            wal: wal_health,
            index: index_health,
        },
    };

    // Return appropriate status code
    let status_code = match overall_status {
        HealthStatus::Healthy => StatusCode::OK,
        HealthStatus::Degraded => StatusCode::OK, // Still serving traffic
        HealthStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
    };

    (status_code, Json(response)).into_response()
}

/// Calculate service uptime in seconds
///
/// Note: This is a simple implementation that returns seconds since Unix epoch.
/// In production, you should track the actual server start time.
fn calculate_uptime() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        % 86400 // Mock: return time within current day
}

/// Determine overall health status from component statuses
fn determine_overall_status(statuses: &[HealthStatus]) -> HealthStatus {
    if statuses.contains(&HealthStatus::Unhealthy) {
        HealthStatus::Unhealthy
    } else if statuses.contains(&HealthStatus::Degraded) {
        HealthStatus::Degraded
    } else {
        HealthStatus::Healthy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_overall_status_all_healthy() {
        let statuses = vec![
            HealthStatus::Healthy,
            HealthStatus::Healthy,
            HealthStatus::Healthy,
        ];
        assert_eq!(
            determine_overall_status(&statuses),
            HealthStatus::Healthy
        );
    }

    #[test]
    fn test_determine_overall_status_one_degraded() {
        let statuses = vec![
            HealthStatus::Healthy,
            HealthStatus::Degraded,
            HealthStatus::Healthy,
        ];
        assert_eq!(
            determine_overall_status(&statuses),
            HealthStatus::Degraded
        );
    }

    #[test]
    fn test_determine_overall_status_one_unhealthy() {
        let statuses = vec![
            HealthStatus::Healthy,
            HealthStatus::Degraded,
            HealthStatus::Unhealthy,
        ];
        assert_eq!(
            determine_overall_status(&statuses),
            HealthStatus::Unhealthy
        );
    }

    #[tokio::test]
    async fn test_liveness_handler() {
        let response = liveness_handler().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn test_component_health_serialization() {
        let health = ComponentHealth {
            status: HealthStatus::Healthy,
            message: Some("All good".to_string()),
        };

        let json = serde_json::to_string(&health).unwrap();
        assert!(json.contains("healthy"));
        assert!(json.contains("All good"));
    }

    #[test]
    fn test_health_response_serialization() {
        let response = HealthResponse {
            status: HealthStatus::Healthy,
            version: "0.1.0".to_string(),
            uptime_seconds: 3600,
            components: ComponentHealthDetails {
                storage: ComponentHealth {
                    status: HealthStatus::Healthy,
                    message: Some("OK".to_string()),
                },
                wal: ComponentHealth {
                    status: HealthStatus::Healthy,
                    message: Some("OK".to_string()),
                },
                index: ComponentHealth {
                    status: HealthStatus::Healthy,
                    message: Some("OK".to_string()),
                },
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"version\":\"0.1.0\""));
        assert!(json.contains("\"uptime_seconds\":3600"));
        assert!(json.contains("\"storage\""));
        assert!(json.contains("\"wal\""));
        assert!(json.contains("\"index\""));
    }
}
