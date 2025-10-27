//! Integration tests for batch search API

use akidb_api::{build_router_with_auth, AppState, AuthConfig};
use akidb_index::NativeIndexProvider;
use akidb_query::{
    BasicQueryPlanner, BatchExecutionEngine, ExecutionEngine, QueryPlanner, SimpleExecutionEngine,
};
use akidb_storage::{MetadataStore, MemoryMetadataStore, MemoryStorageBackend, S3WalBackend};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;

/// Helper to create test AppState
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

/// Helper to create a collection via API
async fn create_collection_via_api(
    app: &axum::Router,
    name: &str,
    vector_dim: u16,
) -> axum::response::Response {
    let create_req = json!({
        "name": name,
        "vector_dim": vector_dim,
        "distance": "Cosine"
    });

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&create_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap()
}

/// Helper to insert vectors via API
async fn insert_vectors_via_api(
    app: &axum::Router,
    collection_name: &str,
    vectors: serde_json::Value,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/collections/{}/vectors", collection_name))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&vectors).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn test_batch_search_success() {
    let state = create_test_state();
    let app = build_router_with_auth(state, AuthConfig::disabled());

    // Create collection
    let response = create_collection_via_api(&app, "test_collection", 128).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Insert test vectors
    let vectors = json!({
        "vectors": [
            {
                "id": "vec1",
                "vector": vec![1.0; 128],
                "payload": {"tag": "a"}
            },
            {
                "id": "vec2",
                "vector": vec![0.5; 128],
                "payload": {"tag": "b"}
            },
            {
                "id": "vec3",
                "vector": vec![0.0; 128],
                "payload": {"tag": "c"}
            }
        ]
    });
    let response = insert_vectors_via_api(&app, "test_collection", vectors).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Batch search with 3 queries
    let batch_request = json!({
        "collection": "test_collection",
        "timeout_ms": 1000,
        "queries": [
            {
                "id": "q1",
                "vector": vec![1.0; 128],
                "top_k": 2
            },
            {
                "id": "q2",
                "vector": vec![0.5; 128],
                "top_k": 2
            },
            {
                "id": "q3",
                "vector": vec![0.0; 128],
                "top_k": 1
            }
        ]
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/test_collection/batch-search")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&batch_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let batch_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(batch_response["collection"], "test_collection");

    let results = batch_response["results"].as_array().unwrap();
    assert_eq!(results.len(), 3);

    assert_eq!(results[0]["id"], "q1");
    assert_eq!(results[0]["neighbors"].as_array().unwrap().len(), 2);
    assert!(results[0]["latency_ms"].as_f64().unwrap() >= 0.0);

    assert_eq!(results[1]["id"], "q2");
    assert_eq!(results[1]["neighbors"].as_array().unwrap().len(), 2);

    assert_eq!(results[2]["id"], "q3");
    assert_eq!(results[2]["neighbors"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_batch_search_empty_request() {
    let state = create_test_state();
    let app = build_router_with_auth(state, AuthConfig::disabled());

    // Create collection
    let response = create_collection_via_api(&app, "test_collection", 128).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Empty batch request
    let batch_request = json!({
        "collection": "test_collection",
        "timeout_ms": 1000,
        "queries": []
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/test_collection/batch-search")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&batch_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_batch_search_exceeds_max_size() {
    let state = create_test_state();
    let app = build_router_with_auth(state, AuthConfig::disabled());

    // Create collection
    let response = create_collection_via_api(&app, "test_collection", 128).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Create 101 queries (exceeds MAX_BATCH_SIZE of 100)
    let queries: Vec<serde_json::Value> = (0..101)
        .map(|i| {
            json!({
                "id": format!("q{}", i),
                "vector": vec![1.0; 128],
                "top_k": 1
            })
        })
        .collect();

    let batch_request = json!({
        "collection": "test_collection",
        "timeout_ms": 1000,
        "queries": queries
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/test_collection/batch-search")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&batch_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_batch_search_basic_functionality() {
    let state = create_test_state();
    let app = build_router_with_auth(state, AuthConfig::disabled());

    // Create collection
    let response = create_collection_via_api(&app, "test_collection", 128).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Insert test vectors
    let vectors = json!({
        "vectors": [
            {
                "id": "vec1",
                "vector": vec![1.0; 128],
                "payload": {"tag": "a"}
            },
            {
                "id": "vec2",
                "vector": vec![0.5; 128],
                "payload": {"tag": "b"}
            },
            {
                "id": "vec3",
                "vector": vec![0.0; 128],
                "payload": {"tag": "c"}
            }
        ]
    });
    let response = insert_vectors_via_api(&app, "test_collection", vectors).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Batch request without filters
    let batch_request = json!({
        "collection": "test_collection",
        "timeout_ms": 1000,
        "queries": [
            {
                "id": "q1",
                "vector": vec![1.0; 128],
                "top_k": 2
            },
            {
                "id": "q2",
                "vector": vec![0.5; 128],
                "top_k": 2
            }
        ]
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/test_collection/batch-search")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&batch_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let batch_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let results = batch_response["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["id"], "q1");
    assert!(!results[0]["neighbors"].as_array().unwrap().is_empty());
    assert_eq!(results[1]["id"], "q2");
    assert!(!results[1]["neighbors"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_batch_search_parallel_execution() {
    let state = create_test_state();
    let app = build_router_with_auth(state, AuthConfig::disabled());

    // Create collection
    let response = create_collection_via_api(&app, "test_collection", 128).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Insert test vectors
    let vectors = json!({
        "vectors": [
            {
                "id": "vec1",
                "vector": vec![1.0; 128],
                "payload": {"tag": "test"}
            },
            {
                "id": "vec2",
                "vector": vec![0.5; 128],
                "payload": {"tag": "test"}
            },
            {
                "id": "vec3",
                "vector": vec![0.0; 128],
                "payload": {"tag": "test"}
            }
        ]
    });
    let response = insert_vectors_via_api(&app, "test_collection", vectors).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Create 10 queries to test parallel execution
    let queries: Vec<serde_json::Value> = (0..10)
        .map(|i| {
            json!({
                "id": format!("q{}", i),
                "vector": vec![i as f32 / 10.0; 128],
                "top_k": 2
            })
        })
        .collect();

    let batch_request = json!({
        "collection": "test_collection",
        "timeout_ms": 1000,
        "queries": queries
    });

    let start = std::time::Instant::now();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/test_collection/batch-search")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&batch_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let elapsed = start.elapsed();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let batch_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(batch_response["results"].as_array().unwrap().len(), 10);

    // Parallel execution should be reasonably fast (< 200ms for 10 queries)
    assert!(
        elapsed.as_millis() < 200,
        "Batch search took too long: {:?}",
        elapsed
    );
}

#[tokio::test]
async fn test_batch_search_collection_not_found() {
    let state = create_test_state();
    let app = build_router_with_auth(state, AuthConfig::disabled());

    let batch_request = json!({
        "collection": "",
        "timeout_ms": 1000,
        "queries": [
            {
                "id": "q1",
                "vector": vec![1.0; 128],
                "top_k": 1
            }
        ]
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/nonexistent/batch-search")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&batch_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // All queries should fail because collection doesn't exist
    // API correctly returns 404 (Not Found) for missing collections
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
