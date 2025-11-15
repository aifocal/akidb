//! Integration tests for observability (metrics, tracing)
//!
//! Tests verify that:
//! - Prometheus metrics are collected correctly
//! - Metrics endpoint returns valid format
//! - Service operations record metrics
//! - Metrics accuracy

use akidb_core::{DistanceMetric, DocumentId, VectorDocument};
use akidb_service::{CollectionService, ServiceMetrics};

#[tokio::test]
async fn test_service_metrics_structure() {
    let service = CollectionService::new();

    // Service without repository returns None
    let metrics = service.metrics();
    assert!(
        metrics.is_none(),
        "In-memory service should not have metrics"
    );
}

#[tokio::test]
async fn test_metrics_export_prometheus_format() {
    let metrics = ServiceMetrics {
        total_collections: 5,
        total_vectors: 1000,
        total_searches: 500,
        total_inserts: 1000,
        uptime_seconds: 3600,
    };

    let output = metrics.export_prometheus().await;

    // Verify Prometheus text format
    assert!(output.contains("# HELP"), "Should have HELP lines");
    assert!(output.contains("# TYPE"), "Should have TYPE lines");
    assert!(
        output.contains("akidb_total_collections 5"),
        "Should have collections metric"
    );
    assert!(
        output.contains("akidb_total_vectors 1000"),
        "Should have vectors metric"
    );
    assert!(
        output.contains("akidb_uptime_seconds 3600"),
        "Should have uptime metric"
    );
}

#[tokio::test]
async fn test_vector_operations_record_metrics() {
    

    let service = CollectionService::new();

    // Create collection
    let collection_id = service
        .create_collection(
            "test-metrics".to_string(),
            128,
            DistanceMetric::Cosine,
            None,
        )
        .await
        .unwrap();

    // Insert vector
    let doc = VectorDocument::new(DocumentId::new(), vec![0.1; 128]);
    let _doc_id = service.insert(collection_id, doc).await.unwrap();

    // Perform search
    let query = vec![0.1; 128];
    let _ = service.query(collection_id, query, 5).await.unwrap();

    // Verify metrics were recorded
    let metric_families = prometheus::gather();

    // Find vector_search_duration_seconds metric
    let search_metric = metric_families
        .iter()
        .find(|m| m.get_name() == "akidb_vector_search_duration_seconds");
    assert!(
        search_metric.is_some(),
        "Search duration metric should be recorded"
    );

    // Find vector_insert_duration_seconds metric
    let insert_metric = metric_families
        .iter()
        .find(|m| m.get_name() == "akidb_vector_insert_duration_seconds");
    assert!(
        insert_metric.is_some(),
        "Insert duration metric should be recorded"
    );

    // Find collection_size_vectors metric
    let size_metric = metric_families
        .iter()
        .find(|m| m.get_name() == "akidb_collection_size_vectors");
    assert!(
        size_metric.is_some(),
        "Collection size metric should be recorded"
    );
}

#[tokio::test]
async fn test_all_core_metrics_registered() {
    use akidb_service::metrics;

    // Initialize all metrics to ensure they are registered
    metrics::init_metrics();

    // Actually USE each metric to force lazy_static initialization
    use akidb_service::metrics::*;
    HTTP_REQUESTS_TOTAL.with_label_values(&["TEST", "/test", "200"]).inc();
    HTTP_REQUEST_DURATION_SECONDS.with_label_values(&["TEST", "/test"]).observe(0.001);
    GRPC_REQUESTS_TOTAL.with_label_values(&["test_service", "test_method", "ok"]).inc();
    GRPC_REQUEST_DURATION_SECONDS.with_label_values(&["test_service", "test_method"]).observe(0.001);
    VECTOR_SEARCH_DURATION_SECONDS.with_label_values(&["hot"]).observe(0.001);
    VECTOR_INSERT_DURATION_SECONDS.with_label_values(&["test_collection"]).observe(0.001);
    COLLECTION_SIZE_VECTORS.with_label_values(&["test_collection"]).set(100.0);
    TIER_DISTRIBUTION_COLLECTIONS.with_label_values(&["hot"]).set(10.0);
    S3_OPERATIONS_TOTAL.with_label_values(&["put", "success"]).inc();
    S3_OPERATION_DURATION_SECONDS.with_label_values(&["put"]).observe(0.1);
    MEMORY_USAGE_BYTES.with_label_values(&["test_component"]).set(1024.0);
    BACKGROUND_WORKER_RUNS_TOTAL.with_label_values(&["test_worker", "success"]).inc();

    let metric_families = prometheus::gather();
    let metric_names: Vec<String> = metric_families
        .iter()
        .map(|m| m.get_name().to_string())
        .collect();

    // Debug: Print all registered metrics
    eprintln!("Registered metrics ({} total):", metric_names.len());
    for name in &metric_names {
        eprintln!("  - {}", name);
    }

    // Verify all 12 core metrics are registered
    assert!(
        metric_names.contains(&"akidb_http_requests_total".to_string()),
        "HTTP requests metric should be registered. Found: {:?}",
        metric_names
    );
    assert!(
        metric_names.contains(&"akidb_http_request_duration_seconds".to_string()),
        "HTTP duration metric should be registered"
    );
    assert!(
        metric_names.contains(&"akidb_grpc_requests_total".to_string()),
        "gRPC requests metric should be registered"
    );
    assert!(
        metric_names.contains(&"akidb_grpc_request_duration_seconds".to_string()),
        "gRPC duration metric should be registered"
    );
    assert!(
        metric_names.contains(&"akidb_vector_search_duration_seconds".to_string()),
        "Vector search metric should be registered"
    );
    assert!(
        metric_names.contains(&"akidb_vector_insert_duration_seconds".to_string()),
        "Vector insert metric should be registered"
    );
    assert!(
        metric_names.contains(&"akidb_collection_size_vectors".to_string()),
        "Collection size metric should be registered"
    );
    assert!(
        metric_names.contains(&"akidb_tier_distribution_collections".to_string()),
        "Tier distribution metric should be registered"
    );
    assert!(
        metric_names.contains(&"akidb_s3_operations_total".to_string()),
        "S3 operations metric should be registered"
    );
    assert!(
        metric_names.contains(&"akidb_s3_operation_duration_seconds".to_string()),
        "S3 duration metric should be registered"
    );
    assert!(
        metric_names.contains(&"akidb_memory_usage_bytes".to_string()),
        "Memory usage metric should be registered"
    );
    assert!(
        metric_names.contains(&"akidb_background_worker_runs_total".to_string()),
        "Background worker metric should be registered"
    );
}

#[tokio::test]
async fn test_uptime_tracking() {
    let service = CollectionService::new();

    // Wait a short time
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let uptime = service.uptime_seconds();
    // uptime is u64, so always non-negative by type
    assert!(uptime < 10, "Uptime should be reasonable for test");
}

#[tokio::test]
async fn test_metrics_labels() {
    use akidb_service::metrics::*;

    // Record metrics with different labels
    HTTP_REQUESTS_TOTAL
        .with_label_values(&["GET", "/health", "200"])
        .inc();
    HTTP_REQUESTS_TOTAL
        .with_label_values(&["POST", "/collections", "201"])
        .inc();
    HTTP_REQUESTS_TOTAL
        .with_label_values(&["GET", "/collections", "500"])
        .inc();

    let metric_families = prometheus::gather();
    let http_metric = metric_families
        .iter()
        .find(|m| m.get_name() == "akidb_http_requests_total")
        .expect("HTTP metric should exist");

    // Verify multiple label combinations exist
    assert!(
        http_metric.get_metric().len() >= 3,
        "Should have at least 3 label combinations"
    );
}

#[tokio::test]
async fn test_histogram_buckets() {
    use akidb_service::metrics::*;

    // Record some latencies
    VECTOR_SEARCH_DURATION_SECONDS
        .with_label_values(&["hot"])
        .observe(0.0001);
    VECTOR_SEARCH_DURATION_SECONDS
        .with_label_values(&["hot"])
        .observe(0.001);
    VECTOR_SEARCH_DURATION_SECONDS
        .with_label_values(&["hot"])
        .observe(0.01);
    VECTOR_SEARCH_DURATION_SECONDS
        .with_label_values(&["hot"])
        .observe(0.1);

    let metric_families = prometheus::gather();
    let search_metric = metric_families
        .iter()
        .find(|m| m.get_name() == "akidb_vector_search_duration_seconds")
        .expect("Search metric should exist");

    // Verify histogram has buckets
    let first_metric = &search_metric.get_metric()[0];
    let histogram = first_metric.get_histogram();
    assert!(
        histogram.get_bucket().len() > 5,
        "Histogram should have multiple buckets"
    );
    assert!(
        histogram.get_sample_count() >= 4,
        "Should have recorded 4 observations"
    );
}

#[tokio::test]
async fn test_gauge_metrics() {
    use akidb_service::metrics::*;

    // Set gauge values
    COLLECTION_SIZE_VECTORS
        .with_label_values(&["collection-1"])
        .set(100.0);
    COLLECTION_SIZE_VECTORS
        .with_label_values(&["collection-2"])
        .set(200.0);

    // Increment/decrement
    COLLECTION_SIZE_VECTORS
        .with_label_values(&["collection-1"])
        .inc();
    COLLECTION_SIZE_VECTORS
        .with_label_values(&["collection-1"])
        .dec();

    let metric_families = prometheus::gather();
    let size_metric = metric_families
        .iter()
        .find(|m| m.get_name() == "akidb_collection_size_vectors")
        .expect("Size metric should exist");

    // Verify gauge can be set and modified
    assert!(
        size_metric.get_metric().len() >= 2,
        "Should have metrics for both collections"
    );
}

#[tokio::test]
async fn test_prometheus_export_function() {
    use akidb_service::metrics;

    // Record some test metrics
    metrics::HTTP_REQUESTS_TOTAL
        .with_label_values(&["GET", "/test", "200"])
        .inc();

    let output = metrics::export_prometheus();

    // Verify output format
    assert!(output.contains("# HELP"), "Should have HELP comments");
    assert!(output.contains("# TYPE"), "Should have TYPE comments");
    assert!(
        output.contains("akidb_"),
        "Should have akidb_ prefixed metrics"
    );
    assert!(!output.is_empty(), "Output should not be empty");
}
