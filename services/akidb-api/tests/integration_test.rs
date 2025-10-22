//! End-to-end integration tests for AkiDB API

use akidb_api::{build_router, AppState};
use akidb_index::NativeIndexProvider;
use akidb_query::{BasicQueryPlanner, SimpleExecutionEngine};
use akidb_storage::MemoryStorageBackend;
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
    let planner = Arc::new(BasicQueryPlanner::new());
    let engine = Arc::new(SimpleExecutionEngine::new(index_provider.clone()));

    AppState::new(storage, index_provider, planner, engine)
}

#[tokio::test]
async fn test_health_check() {
    init_tracing();
    let state = create_test_state();
    let app = build_router(state);

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
}

#[tokio::test]
async fn test_logging_middleware() {
    init_tracing();
    let state = create_test_state();
    let app = build_router(state);

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
    let app = build_router(state);

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
    let app = build_router(state);

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
    let app = build_router(state);

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
    let app = build_router(state);

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
    let app = build_router(state);

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
    let app = build_router(state);

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
    let app = build_router(state);

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
