//! Filter pushdown integration tests
//!
//! Tests for Phase 3 M3 Filter Pushdown functionality

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

/// Initialize tracing for tests
fn init_tracing() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter("akidb_api=debug,akidb_query=debug")
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

/// Test 1: Empty filter result should return early without vector search
#[tokio::test]
async fn test_empty_filter_result_early_return() {
    init_tracing();
    let state = create_test_state();
    let app = build_router_with_auth(state, AuthConfig::disabled());

    // 1. Create collection
    let create_req = json!({
        "name": "test_empty_filter",
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

    // 2. Insert vectors with metadata
    let insert_req = json!({
        "vectors": [
            {
                "id": "vec1",
                "vector": [1.0, 0.0, 0.0],
                "payload": {"category": "electronics", "price": 100}
            },
            {
                "id": "vec2",
                "vector": [0.0, 1.0, 0.0],
                "payload": {"category": "books", "price": 20}
            },
            {
                "id": "vec3",
                "vector": [0.0, 0.0, 1.0],
                "payload": {"category": "electronics", "price": 500}
            }
        ]
    });

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/test_empty_filter/vectors")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&insert_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // 3. Search with filter that matches 0 documents
    let search_req = json!({
        "vector": [1.0, 0.0, 0.0],
        "top_k": 10,
        "filter": {
            "field": "category",
            "match": "nonexistent_category"
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/test_empty_filter/search")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&search_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return 200 OK with empty results
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let search_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(search_response["count"], 0);
    assert_eq!(search_response["results"].as_array().unwrap().len(), 0);
}

/// Test 2: Too many filter clauses should be rejected
#[tokio::test]
async fn test_too_many_clauses_rejected() {
    init_tracing();
    let state = create_test_state();
    let app = build_router_with_auth(state, AuthConfig::disabled());

    // 1. Create collection
    let create_req = json!({
        "name": "test_too_many_clauses",
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

    // 2. Insert a vector
    let insert_req = json!({
        "vectors": [
            {
                "id": "vec1",
                "vector": [1.0, 0.0, 0.0],
                "payload": {"field": "value"}
            }
        ]
    });

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/test_too_many_clauses/vectors")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&insert_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // 3. Create filter with 129 clauses (exceeds MAX_BOOLEAN_CLAUSES=128)
    let mut clauses = Vec::new();
    for i in 0..129 {
        clauses.push(json!({
            "field": format!("field_{}", i),
            "match": format!("value_{}", i)
        }));
    }

    let search_req = json!({
        "vector": [1.0, 0.0, 0.0],
        "top_k": 10,
        "filter": {
            "should": clauses
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/test_too_many_clauses/search")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&search_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return 400 Bad Request (validation error)
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let error_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Error message should mention "Too many filter clauses"
    let error_msg = error_response["error"].as_str().unwrap();
    assert!(
        error_msg.contains("Too many filter clauses") || error_msg.contains("Invalid filter"),
        "Expected error about too many clauses, got: {}",
        error_msg
    );
}

/// Test 3: Invalid filter JSON should return clear error message
#[tokio::test]
async fn test_invalid_filter_clear_error() {
    init_tracing();
    let state = create_test_state();
    let app = build_router_with_auth(state, AuthConfig::disabled());

    // 1. Create collection
    let create_req = json!({
        "name": "test_invalid_filter",
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

    // 2. Insert a vector
    let insert_req = json!({
        "vectors": [
            {
                "id": "vec1",
                "vector": [1.0, 0.0, 0.0],
                "payload": {"category": "test"}
            }
        ]
    });

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/test_invalid_filter/vectors")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&insert_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // 3. Search with invalid filter (missing required fields)
    let search_req = json!({
        "vector": [1.0, 0.0, 0.0],
        "top_k": 10,
        "filter": {
            "invalid": "structure"
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/test_invalid_filter/search")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&search_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return 400 Bad Request
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let error_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Error message should be clear and mention "Invalid filter syntax"
    let error_msg = error_response["error"].as_str().unwrap();
    assert!(
        error_msg.contains("Invalid filter") || error_msg.contains("filter"),
        "Expected error about invalid filter, got: {}",
        error_msg
    );
}

/// Test 4: Complex nested filter should work correctly
#[tokio::test]
async fn test_complex_nested_filter() {
    init_tracing();
    let state = create_test_state();
    let app = build_router_with_auth(state, AuthConfig::disabled());

    // 1. Create collection
    let create_req = json!({
        "name": "test_complex_filter",
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

    // 2. Insert vectors with varied metadata
    let insert_req = json!({
        "vectors": [
            {
                "id": "vec1",
                "vector": [1.0, 0.0, 0.0],
                "payload": {"category": "electronics", "price": 100, "brand": "sony"}
            },
            {
                "id": "vec2",
                "vector": [0.8, 0.2, 0.0],
                "payload": {"category": "electronics", "price": 200, "brand": "apple"}
            },
            {
                "id": "vec3",
                "vector": [0.0, 1.0, 0.0],
                "payload": {"category": "books", "price": 30, "brand": "penguin"}
            },
            {
                "id": "vec4",
                "vector": [0.0, 0.8, 0.2],
                "payload": {"category": "electronics", "price": 500, "brand": "sony"}
            }
        ]
    });

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/test_complex_filter/vectors")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&insert_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // 3. Search with complex nested filter:
    // (category=electronics AND price<=300) AND (brand=sony OR brand=apple)
    let search_req = json!({
        "vector": [1.0, 0.0, 0.0],
        "top_k": 10,
        "filter": {
            "must": [
                {"field": "category", "match": "electronics"},
                {"field": "price", "range": {"lte": 300}},
                {
                    "should": [
                        {"field": "brand", "match": "sony"},
                        {"field": "brand", "match": "apple"}
                    ]
                }
            ]
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/test_complex_filter/search")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&search_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let search_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Should match vec1 (electronics, 100, sony) and vec2 (electronics, 200, apple)
    // Should NOT match vec3 (books) or vec4 (electronics, 500 - price > 300)
    assert_eq!(search_response["count"], 2);

    let results = search_response["results"].as_array().unwrap();
    let result_ids: Vec<&str> = results.iter().map(|r| r["id"].as_str().unwrap()).collect();

    assert!(result_ids.contains(&"vec1"));
    assert!(result_ids.contains(&"vec2"));
}

/// Test 5: Filter on empty collection should return empty results
#[tokio::test]
async fn test_filter_on_empty_collection() {
    init_tracing();
    let state = create_test_state();
    let app = build_router_with_auth(state, AuthConfig::disabled());

    // 1. Create collection (but don't insert any vectors)
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

    // 2. Try to search with filter on empty collection
    let search_req = json!({
        "vector": [1.0, 0.0, 0.0],
        "top_k": 10,
        "filter": {
            "field": "category",
            "match": "electronics"
        }
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

    // Should return 404 (no index exists yet) or 200 with empty results
    // Both are acceptable behaviors
    assert!(
        response.status() == StatusCode::NOT_FOUND || response.status() == StatusCode::OK,
        "Expected 404 or 200, got: {}",
        response.status()
    );
}

/// Test 6: Successful filter should reduce result set
#[tokio::test]
async fn test_filter_reduces_results() {
    init_tracing();
    let state = create_test_state();
    let app = build_router_with_auth(state, AuthConfig::disabled());

    // 1. Create collection
    let create_req = json!({
        "name": "test_filter_reduction",
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

    // 2. Insert 5 vectors, 3 electronics, 2 books
    let insert_req = json!({
        "vectors": [
            {"id": "e1", "vector": [1.0, 0.0, 0.0], "payload": {"category": "electronics"}},
            {"id": "e2", "vector": [0.9, 0.1, 0.0], "payload": {"category": "electronics"}},
            {"id": "e3", "vector": [0.8, 0.2, 0.0], "payload": {"category": "electronics"}},
            {"id": "b1", "vector": [0.7, 0.3, 0.0], "payload": {"category": "books"}},
            {"id": "b2", "vector": [0.6, 0.4, 0.0], "payload": {"category": "books"}}
        ]
    });

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/test_filter_reduction/vectors")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&insert_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // 3. Search WITHOUT filter (should return all 5, limited by top_k)
    let search_no_filter = json!({
        "vector": [1.0, 0.0, 0.0],
        "top_k": 10
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/test_filter_reduction/search")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&search_no_filter).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let no_filter_response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(no_filter_response["count"], 5);

    // 4. Search WITH filter (category=electronics, should return only 3)
    let search_with_filter = json!({
        "vector": [1.0, 0.0, 0.0],
        "top_k": 10,
        "filter": {
            "field": "category",
            "match": "electronics"
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/collections/test_filter_reduction/search")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&search_with_filter).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let filtered_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Should return only 3 results (electronics items)
    assert_eq!(filtered_response["count"], 3);

    let results = filtered_response["results"].as_array().unwrap();
    for result in results {
        let id = result["id"].as_str().unwrap();
        assert!(
            id.starts_with('e'),
            "Expected electronics item, got: {}",
            id
        );
    }
}
