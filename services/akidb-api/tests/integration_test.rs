//! End-to-end integration tests for AkiDB API

use akidb_api::{build_router_with_auth, AppState, AuthConfig};
use akidb_index::NativeIndexProvider;
use akidb_query::{
    BasicQueryPlanner, BatchExecutionEngine, ExecutionEngine, QueryPlanner, SimpleExecutionEngine,
};
use akidb_storage::{MemoryMetadataStore, MemoryStorageBackend, MetadataStore, S3WalBackend};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt; // for `oneshot`

/// Initialize tracing for tests (call once)
fn init_tracing() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter("akidb_api=debug")
            .with_test_writer()
            .try_init()
            .ok();
    });
}

/// Helper to create AppState for testing
fn create_test_state() -> AppState {
    let storage = Arc::new(MemoryStorageBackend::new());
    let index_provider = Arc::new(NativeIndexProvider::new());
    let planner: Arc<dyn QueryPlanner> = Arc::new(BasicQueryPlanner::new());
    let engine: Arc<dyn ExecutionEngine> =
        Arc::new(SimpleExecutionEngine::new(index_provider.clone()));
    let metadata_store: Arc<dyn MetadataStore> = Arc::new(MemoryMetadataStore::new());
    let batch_engine = Arc::new(BatchExecutionEngine::new(
        Arc::clone(&engine),
        Arc::clone(&metadata_store),
    ));
    let wal = Arc::new(S3WalBackend::new_unchecked(storage.clone()));
    let query_cache = Arc::new(akidb_api::query_cache::QueryCache::default());

    AppState::new(
        storage,
        index_provider,
        planner,
        engine,
        batch_engine,
        metadata_store,
        wal,
        query_cache,
    )
}

#[tokio::test]
async fn test_health_check_detailed() {
    init_tracing();
    let state = create_test_state();
    let app = build_router_with_auth(state, AuthConfig::disabled());

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

    // Verify JSON response
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let health: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Verify response structure
    assert!(health.get("status").is_some(), "Should have status field");
    assert!(health.get("version").is_some(), "Should have version field");
    assert!(
        health.get("uptime_seconds").is_some(),
        "Should have uptime field"
    );
    assert!(
        health.get("components").is_some(),
        "Should have components field"
    );

    // Verify components
    let components = health.get("components").unwrap();
    assert!(
        components.get("storage").is_some(),
        "Should have storage component"
    );
    assert!(components.get("wal").is_some(), "Should have wal component");
    assert!(
        components.get("index").is_some(),
        "Should have index component"
    );

    println!("✅ Detailed health check test passed");
    println!(
        "   Response: {}",
        serde_json::to_string_pretty(&health).unwrap()
    );
}

#[tokio::test]
async fn test_health_liveness() {
    init_tracing();
    let state = create_test_state();
    let app = build_router_with_auth(state, AuthConfig::disabled());

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
        "Liveness probe should always return 200 OK"
    );

    println!("✅ Liveness probe test passed");
}

#[tokio::test]
async fn test_health_readiness() {
    init_tracing();
    let state = create_test_state();
    let app = build_router_with_auth(state, AuthConfig::disabled());

    let response = app
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
        StatusCode::OK,
        "Readiness probe should return 200 when storage is healthy"
    );

    println!("✅ Readiness probe test passed");
}

#[tokio::test]
async fn test_logging_middleware() {
    init_tracing();
    let state = create_test_state();
    let app = build_router_with_auth(state, AuthConfig::disabled());

    // Make a request to trigger logging
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
    // Logging is verified by visual inspection of test output with --nocapture
    // The middleware should log request_id, method, uri, status, and latency
}

#[tokio::test]
async fn test_e2e_create_insert_search() {
    let state = create_test_state();
    let app = build_router_with_auth(state, AuthConfig::disabled());

    // 1. Create collection
    let create_req = json!({
        "name": "test_collection",
        "vector_dim": 3,
        "distance": "Cosine"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&create_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // 2. Insert vectors
    let insert_req = json!({
        "vectors": [
            {
                "id": "vec1",
                "vector": [1.0, 0.0, 0.0],
                "payload": {"label": "first"}
            },
            {
                "id": "vec2",
                "vector": [0.0, 1.0, 0.0],
                "payload": {"label": "second"}
            },
            {
                "id": "vec3",
                "vector": [0.0, 0.0, 1.0],
                "payload": {"label": "third"}
            }
        ]
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/test_collection/vectors")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&insert_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // 3. Search for similar vectors
    let search_req = json!({
        "vector": [1.0, 0.1, 0.0],
        "top_k": 2
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/test_collection/search")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&search_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Check if search succeeded
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    if status != StatusCode::OK {
        let error_text = String::from_utf8_lossy(&body);
        panic!("Search failed with status {}: {}", status, error_text);
    }

    let search_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Verify results
    assert_eq!(search_response["collection"], "test_collection");
    assert_eq!(search_response["count"], 2);
    assert_eq!(search_response["results"][0]["id"], "vec1"); // Most similar
}

#[tokio::test]
async fn test_get_collection() {
    let state = create_test_state();
    let app = build_router_with_auth(state, AuthConfig::disabled());

    // Create collection first
    let create_req = json!({
        "name": "test_collection",
        "vector_dim": 128,
        "distance": "L2"
    });

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&create_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Get collection info
    let response = app
        .oneshot(
            Request::builder()
                .uri("/collections/test_collection")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let collection_info: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(collection_info["name"], "test_collection");
    assert_eq!(collection_info["vector_dim"], 128);
    assert_eq!(collection_info["distance"], "L2");
}

#[tokio::test]
async fn test_collection_not_found() {
    let state = create_test_state();
    let app = build_router_with_auth(state, AuthConfig::disabled());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/collections/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Verify error message
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let error_response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(error_response["error"]
        .as_str()
        .unwrap()
        .contains("not found"));
}

#[tokio::test]
async fn test_search_before_insert() {
    let state = create_test_state();
    let app = build_router_with_auth(state, AuthConfig::disabled());

    // Create collection
    let create_req = json!({
        "name": "empty_collection",
        "vector_dim": 3,
        "distance": "Cosine"
    });

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&create_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Try to search without inserting vectors
    let search_req = json!({
        "vector": [1.0, 0.0, 0.0],
        "top_k": 10
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/empty_collection/search")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&search_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND); // No index exists yet
}

#[tokio::test]
async fn test_list_collections() {
    let state = create_test_state();
    let app = build_router_with_auth(state, AuthConfig::disabled());

    // Initially should be empty
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/collections")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let collections: Vec<String> = serde_json::from_slice(&body).unwrap();
    assert_eq!(collections.len(), 0);

    // Create two collections
    for name in ["collection1", "collection2"] {
        let create_req = json!({
            "name": name,
            "vector_dim": 3,
            "distance": "Cosine"
        });

        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/collections")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&create_req).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // List should now contain both
    let response = app
        .oneshot(
            Request::builder()
                .uri("/collections")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let collections: Vec<String> = serde_json::from_slice(&body).unwrap();
    assert_eq!(collections.len(), 2);
    assert!(collections.contains(&"collection1".to_string()));
    assert!(collections.contains(&"collection2".to_string()));
}

#[tokio::test]
async fn test_delete_collection() {
    let state = create_test_state();
    let app = build_router_with_auth(state, AuthConfig::disabled());

    // Create a collection
    let create_req = json!({
        "name": "to_delete",
        "vector_dim": 3,
        "distance": "Cosine"
    });

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&create_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Delete it
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/collections/to_delete")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify it's gone
    let response = app
        .oneshot(
            Request::builder()
                .uri("/collections/to_delete")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_nonexistent_collection() {
    let state = create_test_state();
    let app = build_router_with_auth(state, AuthConfig::disabled());

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/collections/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_concurrent_storage_operations() {
    // Test concurrent storage operations via direct storage access
    // to verify optimistic locking in write_segment_with_data
    use akidb_core::{CollectionDescriptor, PayloadSchema, SegmentDescriptor, SegmentState};
    use akidb_storage::StorageBackend;
    use chrono::Utc;
    use uuid::Uuid;

    let storage = Arc::new(MemoryStorageBackend::new());

    // 1. Create collection
    let collection = CollectionDescriptor {
        name: "concurrent_storage_test".to_string(),
        vector_dim: 32,
        distance: akidb_core::DistanceMetric::L2,
        replication: 1,
        shard_count: 1,
        payload_schema: PayloadSchema { fields: vec![] },
        wal_stream_id: None,
    };

    storage.create_collection(&collection).await.unwrap();

    // 2. Prepare multiple segment descriptors
    let segment_ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();

    let descriptors: Vec<SegmentDescriptor> = segment_ids
        .iter()
        .enumerate()
        .map(|(i, &segment_id)| SegmentDescriptor {
            segment_id,
            collection: "concurrent_storage_test".to_string(),
            record_count: 20,
            vector_dim: 32,
            lsn_range: (i * 20) as u64..=((i + 1) * 20 - 1) as u64,
            compression_level: 0,
            created_at: Utc::now(),
            state: SegmentState::Active,
        })
        .collect();

    // 3. Generate test vectors for each segment
    let vectors_sets: Vec<Vec<Vec<f32>>> = (0..3)
        .map(|seg_idx| {
            (0..20)
                .map(|i| {
                    (0..32)
                        .map(|j| ((seg_idx * 20 + i) * 32 + j) as f32 * 0.01)
                        .collect()
                })
                .collect()
        })
        .collect();

    // 4. Drive concurrent writes using tokio::join!
    let storage_1 = storage.clone();
    let storage_2 = storage.clone();
    let storage_3 = storage.clone();

    let (result_1, result_2, result_3) = tokio::join!(
        storage_1.write_segment_with_data(&descriptors[0], vectors_sets[0].clone(), None),
        storage_2.write_segment_with_data(&descriptors[1], vectors_sets[1].clone(), None),
        storage_3.write_segment_with_data(&descriptors[2], vectors_sets[2].clone(), None)
    );

    // 5. Assert all operations succeeded
    assert!(
        result_1.is_ok(),
        "First write should succeed: {:?}",
        result_1
    );
    assert!(
        result_2.is_ok(),
        "Second write should succeed: {:?}",
        result_2
    );
    assert!(
        result_3.is_ok(),
        "Third write should succeed: {:?}",
        result_3
    );

    // 6. Verify manifest contains all segments
    let manifest = storage
        .load_manifest("concurrent_storage_test")
        .await
        .unwrap();

    assert_eq!(
        manifest.segments.len(),
        3,
        "Manifest should contain all 3 segments"
    );

    // Verify all segment IDs are present
    let manifest_segment_ids: Vec<Uuid> = manifest.segments.iter().map(|s| s.segment_id).collect();

    for &expected_id in &segment_ids {
        assert!(
            manifest_segment_ids.contains(&expected_id),
            "Manifest should contain segment {:?}",
            expected_id
        );
    }

    // Verify manifest version incremented due to concurrent updates
    assert!(
        manifest.latest_version >= 3,
        "Manifest version should be at least 3, got {}",
        manifest.latest_version
    );

    // Verify total vector count
    assert_eq!(
        manifest.total_vectors, 60,
        "Manifest should track 60 total vectors (3 × 20)"
    );

    println!(
        "✅ Concurrent storage operations test passed (manifest version: {}, {} segments)",
        manifest.latest_version,
        manifest.segments.len()
    );
}

#[tokio::test]
async fn test_multi_batch_ingestion_with_filters() {
    init_tracing();
    let state = create_test_state();
    let app = build_router_with_auth(state, AuthConfig::disabled());

    // 1. Create collection
    let create_req = json!({
        "name": "multi_batch_test",
        "vector_dim": 3,
        "distance": "Cosine"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&create_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // 2. Insert batch 1 (100 vectors with category=A)
    let mut batch1_vectors = Vec::new();
    for i in 0..100 {
        batch1_vectors.push(json!({
            "id": format!("vec_a_{}", i),
            "vector": [1.0, 0.0, 0.0],
            "payload": {"category": "A", "batch": 1}
        }));
    }

    let insert_req1 = json!({
        "vectors": batch1_vectors
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/multi_batch_test/vectors")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&insert_req1).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "First batch insert should succeed"
    );

    // 3. Insert batch 2 (100 vectors with category=B)
    let mut batch2_vectors = Vec::new();
    for i in 0..100 {
        batch2_vectors.push(json!({
            "id": format!("vec_b_{}", i),
            "vector": [0.0, 1.0, 0.0],
            "payload": {"category": "B", "batch": 2}
        }));
    }

    let insert_req2 = json!({
        "vectors": batch2_vectors
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/multi_batch_test/vectors")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&insert_req2).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // This is the critical assertion for P0-2 fix
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Second batch insert should succeed (P0-2 fix verification)"
    );

    // 4. Insert batch 3 (100 vectors with category=C)
    let mut batch3_vectors = Vec::new();
    for i in 0..100 {
        batch3_vectors.push(json!({
            "id": format!("vec_c_{}", i),
            "vector": [0.0, 0.0, 1.0],
            "payload": {"category": "C", "batch": 3}
        }));
    }

    let insert_req3 = json!({
        "vectors": batch3_vectors
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/multi_batch_test/vectors")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&insert_req3).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Third batch insert should succeed"
    );

    // 5. Search to verify all batches are accessible
    let search_req = json!({
        "vector": [1.0, 0.0, 0.0],
        "top_k": 300
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/multi_batch_test/search")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&search_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    if status != StatusCode::OK {
        let error_text = String::from_utf8_lossy(&body);
        panic!("Search failed with status {}: {}", status, error_text);
    }

    let search_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Verify all 300 vectors are accessible
    assert_eq!(
        search_response["count"], 300,
        "Should be able to access all 300 vectors across 3 batches"
    );

    // 6. Test filter queries across all batches (P0-1 fix verification)
    // Filter for category=A (batch 1)
    let search_req_a = json!({
        "vector": [1.0, 0.0, 0.0],
        "top_k": 200,
        "filter": {
            "must": [
                {"field": "category", "match": "A"}
            ]
        }
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/multi_batch_test/search")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&search_req_a).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let filter_response_a: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        filter_response_a["count"], 100,
        "Filter for category=A should return 100 vectors (P0-1 fix verification)"
    );

    // Filter for category=B (batch 2)
    let search_req_b = json!({
        "vector": [0.0, 1.0, 0.0],
        "top_k": 200,
        "filter": {
            "must": [
                {"field": "category", "match": "B"}
            ]
        }
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/multi_batch_test/search")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&search_req_b).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let filter_response_b: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        filter_response_b["count"], 100,
        "Filter for category=B should return 100 vectors (P0-1 fix verification)"
    );

    // Filter for category=C (batch 3)
    let search_req_c = json!({
        "vector": [0.0, 0.0, 1.0],
        "top_k": 200,
        "filter": {
            "must": [
                {"field": "category", "match": "C"}
            ]
        }
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/multi_batch_test/search")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&search_req_c).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let filter_response_c: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        filter_response_c["count"], 100,
        "Filter for category=C should return 100 vectors (P0-1 fix verification)"
    );

    println!("✅ Multi-batch ingestion test passed:");
    println!("   - 3 batches inserted successfully (300 total vectors)");
    println!("   - Second insert did not return HTTP 500 (P0-2 fixed)");
    println!("   - Filters work correctly across all batches (P0-1 fixed)");
    println!("   - No metadata overwrites detected");
}

#[tokio::test]
async fn test_metrics_endpoint() {
    init_tracing();
    let state = create_test_state();
    let app = build_router_with_auth(state.clone(), AuthConfig::disabled());

    // 1. Make some API requests to generate metrics
    // Create a collection
    let create_req = json!({
        "name": "metrics_test",
        "vector_dim": 3,
        "distance": "Cosine"
    });

    let _response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&create_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // 2. Fetch /metrics endpoint
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

    assert_eq!(response.status(), StatusCode::OK);

    // 3. Verify content type is Prometheus text format
    let content_type = response.headers().get("content-type").unwrap();
    assert!(
        content_type
            .to_str()
            .unwrap()
            .contains("text/plain; version=0.0.4"),
        "Metrics endpoint should return Prometheus text format"
    );

    // 4. Verify metrics content
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();

    // Verify some expected metrics are present
    assert!(
        text.contains("akidb_api_requests_total"),
        "Metrics should include API request counter"
    );
    assert!(
        text.contains("akidb_api_request_duration_seconds"),
        "Metrics should include API request duration"
    );
    assert!(
        text.contains("akidb_active_connections"),
        "Metrics should include active connections gauge"
    );
    assert!(text.contains("# HELP"), "Metrics should include help text");
    assert!(
        text.contains("# TYPE"),
        "Metrics should include type information"
    );

    println!("✅ Metrics endpoint test passed:");
    println!("   - /metrics endpoint is accessible");
    println!("   - Returns Prometheus text format");
    println!("   - Contains expected metrics (API requests, duration, connections)");
    println!("   - Includes HELP and TYPE annotations");
}
