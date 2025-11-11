//! Production-grade Prometheus metrics for AkiDB 2.0
//!
//! Provides comprehensive metrics collection for monitoring, alerting, and observability.
//! All metrics follow Prometheus naming conventions and best practices.

use lazy_static::lazy_static;
use prometheus::{
    register_counter_vec, register_gauge_vec, register_histogram_vec, CounterVec, Encoder,
    GaugeVec, HistogramVec, TextEncoder,
};

lazy_static! {
    // ========== Request Metrics (4 metrics) ==========

    /// Total HTTP requests by method, path, and status code
    pub static ref HTTP_REQUESTS_TOTAL: CounterVec = register_counter_vec!(
        "akidb_http_requests_total",
        "Total HTTP requests",
        &["method", "path", "status_code"]
    )
    .unwrap();

    /// HTTP request latency distribution (seconds)
    pub static ref HTTP_REQUEST_DURATION_SECONDS: HistogramVec = register_histogram_vec!(
        "akidb_http_request_duration_seconds",
        "HTTP request latency in seconds",
        &["method", "path"],
        vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
    )
    .unwrap();

    /// Total gRPC requests by service, method, and status
    pub static ref GRPC_REQUESTS_TOTAL: CounterVec = register_counter_vec!(
        "akidb_grpc_requests_total",
        "Total gRPC requests",
        &["service", "method", "status"]
    )
    .unwrap();

    /// gRPC request latency distribution (seconds)
    pub static ref GRPC_REQUEST_DURATION_SECONDS: HistogramVec = register_histogram_vec!(
        "akidb_grpc_request_duration_seconds",
        "gRPC request latency in seconds",
        &["service", "method"],
        vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]
    )
    .unwrap();

    // ========== Vector Operation Metrics (3 metrics) ==========

    /// Vector search latency by tier (hot/warm/cold) in seconds
    pub static ref VECTOR_SEARCH_DURATION_SECONDS: HistogramVec = register_histogram_vec!(
        "akidb_vector_search_duration_seconds",
        "Vector search latency by tier in seconds",
        &["tier"],
        vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5]
    )
    .unwrap();

    /// Vector insert latency in seconds
    pub static ref VECTOR_INSERT_DURATION_SECONDS: HistogramVec = register_histogram_vec!(
        "akidb_vector_insert_duration_seconds",
        "Vector insert latency in seconds",
        &["collection_id"],
        vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5]
    )
    .unwrap();

    /// Number of vectors in collection
    pub static ref COLLECTION_SIZE_VECTORS: GaugeVec = register_gauge_vec!(
        "akidb_collection_size_vectors",
        "Number of vectors in collection",
        &["collection_id"]
    )
    .unwrap();

    // ========== Storage Metrics (3 metrics) ==========

    /// Number of collections per tier (hot/warm/cold)
    pub static ref TIER_DISTRIBUTION_COLLECTIONS: GaugeVec = register_gauge_vec!(
        "akidb_tier_distribution_collections",
        "Number of collections per tier",
        &["tier"]
    )
    .unwrap();

    /// Total S3 operations by operation type and status
    pub static ref S3_OPERATIONS_TOTAL: CounterVec = register_counter_vec!(
        "akidb_s3_operations_total",
        "Total S3 operations",
        &["operation", "status"]
    )
    .unwrap();

    /// S3 operation latency distribution (seconds)
    pub static ref S3_OPERATION_DURATION_SECONDS: HistogramVec = register_histogram_vec!(
        "akidb_s3_operation_duration_seconds",
        "S3 operation latency in seconds",
        &["operation"],
        vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
    )
    .unwrap();

    // ========== System Metrics (2 metrics) ==========

    /// Memory usage by component in bytes
    pub static ref MEMORY_USAGE_BYTES: GaugeVec = register_gauge_vec!(
        "akidb_memory_usage_bytes",
        "Memory usage by component in bytes",
        &["component"]
    )
    .unwrap();

    /// Background worker execution count by worker type and status
    pub static ref BACKGROUND_WORKER_RUNS_TOTAL: CounterVec = register_counter_vec!(
        "akidb_background_worker_runs_total",
        "Background worker execution count",
        &["worker_type", "status"]
    )
    .unwrap();
}

/// Exports all metrics in Prometheus text format
///
/// This function gathers all registered metrics and encodes them in the
/// Prometheus exposition format, suitable for scraping by Prometheus servers.
///
/// # Returns
///
/// A `String` containing all metrics in Prometheus text format
///
/// # Example
///
/// ```rust
/// use akidb_service::metrics;
///
/// let metrics_text = metrics::export_prometheus();
/// println!("{}", metrics_text);
/// ```
pub fn export_prometheus() -> String {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();

    let mut buffer = Vec::new();
    encoder
        .encode(&metric_families, &mut buffer)
        .unwrap_or_else(|e| {
            tracing::error!("Failed to encode metrics: {}", e);
        });

    String::from_utf8(buffer).unwrap_or_else(|e| {
        tracing::error!("Failed to convert metrics to UTF-8: {}", e);
        String::from("# Error encoding metrics\n")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_request_counter() {
        HTTP_REQUESTS_TOTAL
            .with_label_values(&["GET", "/health", "200"])
            .inc();

        let metrics = prometheus::gather();
        let http_metrics = metrics
            .iter()
            .find(|m| m.get_name() == "akidb_http_requests_total");

        assert!(http_metrics.is_some());
    }

    #[test]
    fn test_http_request_duration() {
        HTTP_REQUEST_DURATION_SECONDS
            .with_label_values(&["POST", "/search"])
            .observe(0.015);

        let metrics = prometheus::gather();
        let duration_metrics = metrics
            .iter()
            .find(|m| m.get_name() == "akidb_http_request_duration_seconds");

        assert!(duration_metrics.is_some());
    }

    #[test]
    fn test_vector_search_duration() {
        VECTOR_SEARCH_DURATION_SECONDS
            .with_label_values(&["hot"])
            .observe(0.002);

        VECTOR_SEARCH_DURATION_SECONDS
            .with_label_values(&["warm"])
            .observe(0.020);

        VECTOR_SEARCH_DURATION_SECONDS
            .with_label_values(&["cold"])
            .observe(5.0);

        let metrics = prometheus::gather();
        let search_metrics = metrics
            .iter()
            .find(|m| m.get_name() == "akidb_vector_search_duration_seconds");

        assert!(search_metrics.is_some());
    }

    #[test]
    fn test_collection_size_gauge() {
        COLLECTION_SIZE_VECTORS
            .with_label_values(&["collection-123"])
            .set(10000.0);

        let metrics = prometheus::gather();
        let size_metrics = metrics
            .iter()
            .find(|m| m.get_name() == "akidb_collection_size_vectors");

        assert!(size_metrics.is_some());
    }

    #[test]
    fn test_tier_distribution() {
        TIER_DISTRIBUTION_COLLECTIONS
            .with_label_values(&["hot"])
            .set(10.0);

        TIER_DISTRIBUTION_COLLECTIONS
            .with_label_values(&["warm"])
            .set(50.0);

        TIER_DISTRIBUTION_COLLECTIONS
            .with_label_values(&["cold"])
            .set(100.0);

        let metrics = prometheus::gather();
        let tier_metrics = metrics
            .iter()
            .find(|m| m.get_name() == "akidb_tier_distribution_collections");

        assert!(tier_metrics.is_some());
    }

    #[test]
    fn test_s3_operations() {
        S3_OPERATIONS_TOTAL
            .with_label_values(&["put", "success"])
            .inc();

        S3_OPERATIONS_TOTAL
            .with_label_values(&["get", "success"])
            .inc();

        S3_OPERATIONS_TOTAL
            .with_label_values(&["put", "error"])
            .inc();

        let metrics = prometheus::gather();
        let s3_metrics = metrics
            .iter()
            .find(|m| m.get_name() == "akidb_s3_operations_total");

        assert!(s3_metrics.is_some());
    }

    #[test]
    fn test_s3_operation_duration() {
        S3_OPERATION_DURATION_SECONDS
            .with_label_values(&["put"])
            .observe(0.5);

        S3_OPERATION_DURATION_SECONDS
            .with_label_values(&["get"])
            .observe(0.1);

        let metrics = prometheus::gather();
        let duration_metrics = metrics
            .iter()
            .find(|m| m.get_name() == "akidb_s3_operation_duration_seconds");

        assert!(duration_metrics.is_some());
    }

    #[test]
    fn test_memory_usage() {
        MEMORY_USAGE_BYTES
            .with_label_values(&["hot_tier"])
            .set(1024.0 * 1024.0 * 1024.0); // 1 GB

        let metrics = prometheus::gather();
        let memory_metrics = metrics
            .iter()
            .find(|m| m.get_name() == "akidb_memory_usage_bytes");

        assert!(memory_metrics.is_some());
    }

    #[test]
    fn test_background_worker_runs() {
        BACKGROUND_WORKER_RUNS_TOTAL
            .with_label_values(&["tiering", "success"])
            .inc();

        BACKGROUND_WORKER_RUNS_TOTAL
            .with_label_values(&["dlq_cleanup", "success"])
            .inc();

        let metrics = prometheus::gather();
        let worker_metrics = metrics
            .iter()
            .find(|m| m.get_name() == "akidb_background_worker_runs_total");

        assert!(worker_metrics.is_some());
    }

    #[test]
    fn test_export_prometheus() {
        HTTP_REQUESTS_TOTAL
            .with_label_values(&["GET", "/test", "200"])
            .inc();

        VECTOR_SEARCH_DURATION_SECONDS
            .with_label_values(&["hot"])
            .observe(0.005);

        let output = export_prometheus();

        assert!(output.contains("akidb_http_requests_total"));
        assert!(output.contains("akidb_vector_search_duration_seconds"));
        assert!(output.contains("# HELP"));
        assert!(output.contains("# TYPE"));
    }
}
