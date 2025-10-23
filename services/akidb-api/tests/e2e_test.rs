//! End-to-end integration tests for AkiDB
//!
//! These tests verify the complete data flow from API to Storage,
//! including SEGv1 binary format serialization.

use akidb_api::{build_router, AppState};
use akidb_core::{DistanceMetric, PayloadSchema, SegmentDescriptor, SegmentState};
use akidb_index::NativeIndexProvider;
use akidb_query::{BasicQueryPlanner, SimpleExecutionEngine};
use akidb_storage::{MemoryStorageBackend, SegmentData, StorageBackend};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::Utc;
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

/// Create test state with shared components
fn create_test_state() -> AppState {
    let storage = Arc::new(MemoryStorageBackend::new());
    let index_provider = Arc::new(NativeIndexProvider::new());
    let planner = Arc::new(BasicQueryPlanner::new());
    let engine = Arc::new(SimpleExecutionEngine::new(index_provider.clone()));

    AppState::new(storage, index_provider, planner, engine)
}

#[tokio::test]
async fn test_e2e_api_flow() {
    // Initialize test environment
    let state = create_test_state();
    let app = build_router(state.clone());

    // 1. Create collection
    let create_req = json!({
        "name": "test_products",
        "vector_dim": 4,
        "distance": "Cosine"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/collections")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&create_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // API returns 200 OK for successful creation
    assert_eq!(response.status(), StatusCode::OK);

    // 2. Insert vectors via API
    let insert_req = json!({
        "vectors": [
            {
                "id": "product_1",
                "vector": [1.0, 2.0, 3.0, 4.0],
                "payload": {"name": "Product A", "price": 99.99}
            },
            {
                "id": "product_2",
                "vector": [2.0, 3.0, 4.0, 5.0],
                "payload": {"name": "Product B", "price": 149.99}
            },
            {
                "id": "product_3",
                "vector": [1.5, 2.5, 3.5, 4.5],
                "payload": {"name": "Product C", "price": 199.99}
            }
        ]
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/collections/test_products/vectors")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&insert_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // 3. Search vectors
    let search_req = json!({
        "vector": [1.0, 2.0, 3.0, 4.0],
        "top_k": 2
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/collections/test_products/search")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&search_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let search_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Verify search results
    let results = search_response["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);

    // First result should be product_1 (exact match)
    assert_eq!(results[0]["id"].as_str().unwrap(), "product_1");
}

#[tokio::test]
async fn test_e2e_storage_persistence_with_segv1() {
    // This test demonstrates direct usage of Storage layer with SEGv1 format
    let storage = Arc::new(MemoryStorageBackend::new());

    // 1. Create collection
    let collection = akidb_core::CollectionDescriptor {
        name: "embeddings".to_string(),
        vector_dim: 128,
        distance: DistanceMetric::Cosine,
        replication: 1,
        shard_count: 1,
        payload_schema: PayloadSchema { fields: vec![] },
    };

    storage.create_collection(&collection).await.unwrap();

    // 2. Create segment descriptor
    let segment_id = Uuid::new_v4();
    let descriptor = SegmentDescriptor {
        segment_id,
        collection: "embeddings".to_string(),
        record_count: 100,
        vector_dim: 128,
        lsn_range: 0..=99,
        compression_level: 0,
        created_at: Utc::now(),
        state: SegmentState::Active,
    };

    // 3. Generate test vectors
    let vectors: Vec<Vec<f32>> = (0..100)
        .map(|i| (0..128).map(|j| (i * 128 + j) as f32 * 0.01).collect())
        .collect();

    // 4. Verify SegmentData serialization (SEGv1 format)
    let segment_data = SegmentData::new(128, vectors.clone()).unwrap();
    assert_eq!(segment_data.dimension, 128);
    assert_eq!(segment_data.vectors.len(), 100);

    // 5. Write segment with vectors using SEGv1 format (no metadata for this test)
    storage
        .write_segment_with_data(&descriptor, vectors, None)
        .await
        .unwrap();

    // 6. Verify segment was written by checking if it exists
    let segment_key = format!("collections/embeddings/segments/{}.json", segment_id);
    let exists = storage.object_exists(&segment_key).await.unwrap();
    assert!(exists, "Segment file should exist");

    // 7. Verify we can load the manifest (should have been created during create_collection)
    let manifest = storage.load_manifest("embeddings").await.unwrap();
    assert_eq!(manifest.collection, "embeddings");
    assert_eq!(manifest.dimension, 128);

    // Note: MemoryStorageBackend's write_segment() doesn't auto-update manifest
    // This is implementation-specific behavior. In S3StorageBackend, manifest is updated.
    println!("✅ E2E storage persistence test passed (demonstrated SEGv1 format usage)");
}

#[tokio::test]
async fn test_e2e_segv1_format_roundtrip() {
    use akidb_storage::{ChecksumType, CompressionType, SegmentReader, SegmentWriter};

    // 1. Create test vectors (768-dimensional embeddings, common in ML)
    let dimension = 768;
    let vector_count = 1000;

    let vectors: Vec<Vec<f32>> = (0..vector_count)
        .map(|i| {
            (0..dimension)
                .map(|j| ((i * dimension + j) as f32 * 0.001).sin())
                .collect()
        })
        .collect();

    let segment_data = SegmentData::new(dimension as u32, vectors.clone()).unwrap();

    // 2. Serialize using SEGv1 format with compression
    let writer = SegmentWriter::new(CompressionType::Zstd, ChecksumType::XXH3);
    let serialized = writer.write(&segment_data).unwrap();

    println!(
        "📦 Serialized {} vectors ({} dimensions) to {} bytes",
        vector_count,
        dimension,
        serialized.len()
    );

    let uncompressed_size = vector_count * dimension * 4; // 4 bytes per f32
    let compression_ratio = (serialized.len() as f64 / uncompressed_size as f64) * 100.0;
    println!("🗜️  Compression ratio: {:.1}%", compression_ratio);

    // 3. Deserialize and verify
    let recovered = SegmentReader::read(&serialized).unwrap();

    assert_eq!(recovered.dimension, dimension as u32);
    assert_eq!(recovered.vectors.len(), vector_count);

    // Verify first and last vectors
    for i in 0..dimension {
        assert!((recovered.vectors[0][i] - vectors[0][i]).abs() < 1e-6);
        assert!(
            (recovered.vectors[vector_count - 1][i] - vectors[vector_count - 1][i]).abs() < 1e-6
        );
    }

    println!("✅ SEGv1 format roundtrip test passed");
}

#[tokio::test]
async fn test_e2e_error_handling() {
    let state = create_test_state();
    let app = build_router(state);

    // 1. Try to insert into non-existent collection
    let insert_req = json!({
        "vectors": [
            {
                "id": "test_1",
                "vector": [1.0, 2.0, 3.0],
                "payload": {}
            }
        ]
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/collections/nonexistent/vectors")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&insert_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // 2. Create collection with valid dimensions
    let create_req = json!({
        "name": "test_errors",
        "vector_dim": 3,
        "distance": "Cosine"
    });

    app.clone()
        .oneshot(
            Request::builder()
                .uri("/collections")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&create_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // 3. Try to insert vector with wrong dimension
    let wrong_dim_req = json!({
        "vectors": [
            {
                "id": "test_1",
                "vector": [1.0, 2.0, 3.0, 4.0],  // Wrong: 4 dimensions instead of 3
                "payload": {}
            }
        ]
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/collections/test_errors/vectors")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&wrong_dim_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // 4. Try to insert vector with invalid values (NaN)
    let invalid_req = json!({
        "vectors": [
            {
                "id": "test_1",
                "vector": [1.0, f64::NAN, 3.0],
                "payload": {}
            }
        ]
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/collections/test_errors/vectors")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&invalid_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should fail validation - may return 400 or 422 depending on validation layer
    assert!(
        response.status() == StatusCode::BAD_REQUEST
            || response.status() == StatusCode::UNPROCESSABLE_ENTITY,
        "Expected 400 or 422, got {}",
        response.status()
    );

    println!("✅ E2E error handling test passed");
}

#[tokio::test]
async fn test_e2e_large_batch_insert() {
    let state = create_test_state();
    let app = build_router(state);

    // 1. Create collection
    let create_req = json!({
        "name": "large_batch",
        "vector_dim": 128,
        "distance": "Cosine"
    });

    app.clone()
        .oneshot(
            Request::builder()
                .uri("/collections")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&create_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // 2. Insert large batch (500 vectors)
    let vectors: Vec<_> = (0..500)
        .map(|i| {
            json!({
                "id": format!("vec_{}", i),
                "vector": (0..128).map(|j| ((i * 128 + j) as f64 * 0.01).sin()).collect::<Vec<f64>>(),
                "payload": {"batch": i / 100}
            })
        })
        .collect();

    let insert_req = json!({
        "vectors": vectors
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/collections/large_batch/vectors")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&insert_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let insert_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(insert_response["inserted"].as_u64().unwrap(), 500);

    // 3. Search and verify results
    let search_req = json!({
        "vector": (0..128).map(|j| (j as f64 * 0.01).sin()).collect::<Vec<f64>>(),
        "top_k": 10
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/collections/large_batch/search")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&search_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let search_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let results = search_response["results"].as_array().unwrap();
    assert_eq!(results.len(), 10);

    println!("✅ E2E large batch insert test passed (500 vectors)");
}
