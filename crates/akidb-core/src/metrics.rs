//! Central metrics registry and metric definitions
//!
//! This module provides Prometheus metrics for all AkiDB components.
//! Metrics are registered lazily on first access using once_cell::Lazy.

use once_cell::sync::Lazy;
use prometheus::{
    register_histogram_vec, register_int_counter_vec, register_int_gauge, register_int_gauge_vec,
    HistogramVec, IntCounterVec, IntGauge, IntGaugeVec,
};

// ===== API Request Metrics =====

/// Total number of API requests by method, endpoint, and status code
pub static API_REQUEST_COUNT: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "akidb_api_requests_total",
        "Total number of API requests",
        &["method", "endpoint", "status"]
    )
    .expect("Failed to register API request counter")
});

/// API request duration histogram
pub static API_REQUEST_DURATION: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "akidb_api_request_duration_seconds",
        "API request duration in seconds",
        &["method", "endpoint"],
        // Buckets: 1ms, 5ms, 10ms, 25ms, 50ms, 100ms, 250ms, 500ms, 1s, 2.5s, 5s, 10s
        vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
    )
    .expect("Failed to register API request duration histogram")
});

// ===== Storage Metrics =====

/// Total number of storage operations (get/put/delete) by status
pub static STORAGE_OPERATIONS: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "akidb_storage_operations_total",
        "Total number of storage operations",
        &["operation", "status"]
    )
    .expect("Failed to register storage operations counter")
});

/// Storage operation latency histogram
pub static STORAGE_LATENCY: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "akidb_storage_latency_seconds",
        "Storage operation latency in seconds",
        &["operation"],
        vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]
    )
    .expect("Failed to register storage latency histogram")
});

/// S3 circuit breaker state gauge (0=closed, 1=open, 2=half-open)
pub static CIRCUIT_BREAKER_STATE: Lazy<IntGaugeVec> = Lazy::new(|| {
    register_int_gauge_vec!(
        "akidb_circuit_breaker_state",
        "Circuit breaker state (0=closed, 1=open, 2=half-open)",
        &["backend"]
    )
    .expect("Failed to register circuit breaker state gauge")
});

// ===== Index Search Metrics =====

/// Index search duration histogram
pub static INDEX_SEARCH_DURATION: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "akidb_index_search_duration_seconds",
        "Index search duration in seconds",
        &["index_type", "distance_metric"],
        vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]
    )
    .expect("Failed to register index search duration histogram")
});

/// Total number of vectors in all indexes
pub static INDEX_VECTOR_COUNT: Lazy<IntGaugeVec> = Lazy::new(|| {
    register_int_gauge_vec!(
        "akidb_index_vectors_total",
        "Total number of vectors in index",
        &["collection", "index_type"]
    )
    .expect("Failed to register index vector count gauge")
});

// ===== WAL Metrics =====

/// Total number of WAL operations by type and status
pub static WAL_OPERATIONS: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "akidb_wal_operations_total",
        "Total number of WAL operations",
        &["operation", "status"]
    )
    .expect("Failed to register WAL operations counter")
});

/// Current WAL size in bytes per collection
pub static WAL_SIZE_BYTES: Lazy<IntGaugeVec> = Lazy::new(|| {
    register_int_gauge_vec!(
        "akidb_wal_size_bytes",
        "Current WAL size in bytes",
        &["collection"]
    )
    .expect("Failed to register WAL size gauge")
});

// ===== Query Profiling Metrics =====

/// Total number of slow queries detected
pub static SLOW_QUERIES: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "akidb_slow_queries_total",
        "Total number of slow queries detected",
        &["collection", "threshold_ms"]
    )
    .expect("Failed to register slow queries counter")
});

/// Query execution stage duration
pub static QUERY_STAGE_DURATION: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "akidb_query_stage_duration_seconds",
        "Query execution stage duration in seconds",
        &["stage"],
        vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]
    )
    .expect("Failed to register query stage duration histogram")
});

// ===== System Metrics =====

/// Number of active connections to the API server
pub static ACTIVE_CONNECTIONS: Lazy<IntGauge> = Lazy::new(|| {
    register_int_gauge!(
        "akidb_active_connections",
        "Number of active connections to the API server"
    )
    .expect("Failed to register active connections gauge")
});

/// Number of collections currently loaded
pub static COLLECTIONS_LOADED: Lazy<IntGauge> = Lazy::new(|| {
    register_int_gauge!(
        "akidb_collections_loaded",
        "Number of collections currently loaded in memory"
    )
    .expect("Failed to register collections loaded gauge")
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_initialization() {
        // Access each metric to ensure they can be initialized without panicking
        let _ = &*API_REQUEST_COUNT;
        let _ = &*API_REQUEST_DURATION;
        let _ = &*STORAGE_OPERATIONS;
        let _ = &*STORAGE_LATENCY;
        let _ = &*CIRCUIT_BREAKER_STATE;
        let _ = &*INDEX_SEARCH_DURATION;
        let _ = &*INDEX_VECTOR_COUNT;
        let _ = &*WAL_OPERATIONS;
        let _ = &*WAL_SIZE_BYTES;
        let _ = &*SLOW_QUERIES;
        let _ = &*QUERY_STAGE_DURATION;
        let _ = &*ACTIVE_CONNECTIONS;
        let _ = &*COLLECTIONS_LOADED;
    }

    #[test]
    fn test_api_metrics_increment() {
        API_REQUEST_COUNT
            .with_label_values(&["GET", "/collections", "200"])
            .inc();

        let metrics = prometheus::gather();
        let api_metrics: Vec<_> = metrics
            .iter()
            .filter(|m| m.get_name() == "akidb_api_requests_total")
            .collect();

        assert!(!api_metrics.is_empty());
    }

    #[test]
    fn test_storage_metrics_timing() {
        let _timer = STORAGE_LATENCY
            .with_label_values(&["get_object"])
            .start_timer();

        // Timer is automatically recorded when dropped
        drop(_timer);

        let metrics = prometheus::gather();
        let storage_metrics: Vec<_> = metrics
            .iter()
            .filter(|m| m.get_name() == "akidb_storage_latency_seconds")
            .collect();

        assert!(!storage_metrics.is_empty());
    }
}
