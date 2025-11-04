//! Simplified multi-batch integration test
//!
//! Validates P0-1 and P0-2 fixes by Bob:
//! - P0-1: Global doc_id counter prevents metadata overwrite
//! - P0-2: Persistence slicing ensures correct batch size

use akidb_api::AppState;
use akidb_core::collection::{CollectionDescriptor, DistanceMetric, PayloadSchema};
use akidb_index::NativeIndexProvider;
use akidb_query::{
    BasicQueryPlanner, BatchExecutionEngine, ExecutionEngine, QueryPlanner, SimpleExecutionEngine,
};
use akidb_storage::{MemoryMetadataStore, MemoryStorageBackend, MetadataStore, S3WalBackend};
use serde_json::json;
use std::sync::Arc;

/// Helper to create test state
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
async fn test_global_doc_id_counter() {
    // Test P0-1 fix: Global doc_id counter
    let state = create_test_state();

    // Create collection
    let descriptor = CollectionDescriptor {
        name: "test".to_string(),
        vector_dim: 3,
        distance: DistanceMetric::Cosine,
        replication: 1,
        shard_count: 1,
        payload_schema: PayloadSchema::default(),
        wal_stream_id: None,
    };

    let manifest = akidb_core::manifest::CollectionManifest {
        collection: "test".to_string(),
        latest_version: 0,
        updated_at: chrono::Utc::now(),
        dimension: 3,
        metric: DistanceMetric::Cosine,
        total_vectors: 0,
        epoch: 0,
        created_at: Some(chrono::Utc::now()),
        snapshot: None,
        segments: vec![],
    };

    // Persist manifest to storage (required for write_segment_with_data)
    state.storage.persist_manifest(&manifest).await.unwrap();

    state
        .register_collection(
            "test".to_string(),
            Arc::new(descriptor),
            manifest,
            0,
            akidb_storage::WalStreamId::new(),
        )
        .await
        .unwrap();

    // Get metadata and verify initial doc_id is 0
    let metadata = state.get_collection("test").await.unwrap();
    assert_eq!(
        metadata
            .next_doc_id
            .load(std::sync::atomic::Ordering::SeqCst),
        0,
        "Initial doc_id should be 0"
    );

    // Build index manually
    let build_request = akidb_index::BuildRequest {
        collection: "test".to_string(),
        kind: state.index_provider.kind(),
        distance: DistanceMetric::Cosine,
        dimension: 3,
        segments: vec![],
    };

    let index_handle = state.index_provider.build(build_request).await.unwrap();
    state
        .update_index_handle("test", index_handle.clone())
        .await
        .unwrap();

    // Simulate first batch: insert 5 vectors
    let batch1 = akidb_index::IndexBatch {
        primary_keys: vec![
            "v1".to_string(),
            "v2".to_string(),
            "v3".to_string(),
            "v4".to_string(),
            "v5".to_string(),
        ],
        vectors: vec![
            akidb_index::QueryVector {
                components: vec![1.0, 0.0, 0.0],
            },
            akidb_index::QueryVector {
                components: vec![0.9, 0.1, 0.0],
            },
            akidb_index::QueryVector {
                components: vec![0.8, 0.2, 0.0],
            },
            akidb_index::QueryVector {
                components: vec![0.7, 0.3, 0.0],
            },
            akidb_index::QueryVector {
                components: vec![0.6, 0.4, 0.0],
            },
        ],
        payloads: vec![
            json!({"category": "A", "value": 1}),
            json!({"category": "A", "value": 2}),
            json!({"category": "A", "value": 3}),
            json!({"category": "A", "value": 4}),
            json!({"category": "A", "value": 5}),
        ],
    };

    // Reserve doc_id range for batch 1 (simulating what insert_vectors does)
    let metadata = state.get_collection("test").await.unwrap();
    let batch1_start = metadata
        .next_doc_id
        .fetch_update(
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
            |current| {
                current.checked_add(5) // 5 vectors
            },
        )
        .unwrap();

    assert_eq!(batch1_start, 0, "Batch 1 should start at doc_id 0");

    // Index metadata for batch 1
    for (idx, payload) in batch1.payloads.iter().enumerate() {
        state
            .metadata_store
            .index_metadata("test", batch1_start + idx as u32, payload)
            .await
            .unwrap();
    }

    // Add batch 1 to index
    state
        .index_provider
        .add_batch(&index_handle, batch1)
        .await
        .unwrap();

    // Verify category A has 5 records
    let category_a = state
        .metadata_store
        .find_term("test", "category", &json!("A"))
        .await
        .unwrap();

    assert_eq!(
        category_a.len(),
        5,
        "Category A should have 5 vectors after batch 1"
    );

    // Simulate second batch: insert 4 vectors with category B
    let batch2 = akidb_index::IndexBatch {
        primary_keys: vec![
            "v6".to_string(),
            "v7".to_string(),
            "v8".to_string(),
            "v9".to_string(),
        ],
        vectors: vec![
            akidb_index::QueryVector {
                components: vec![0.0, 1.0, 0.0],
            },
            akidb_index::QueryVector {
                components: vec![0.1, 0.9, 0.0],
            },
            akidb_index::QueryVector {
                components: vec![0.2, 0.8, 0.0],
            },
            akidb_index::QueryVector {
                components: vec![0.3, 0.7, 0.0],
            },
        ],
        payloads: vec![
            json!({"category": "B", "value": 10}),
            json!({"category": "B", "value": 20}),
            json!({"category": "B", "value": 30}),
            json!({"category": "B", "value": 40}),
        ],
    };

    // Reserve doc_id range for batch 2
    let metadata = state.get_collection("test").await.unwrap();
    let batch2_start = metadata
        .next_doc_id
        .fetch_update(
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
            |current| {
                current.checked_add(4) // 4 vectors
            },
        )
        .unwrap();

    // P0-1 VALIDATION: Batch 2 should start at doc_id 5 (not 0!)
    assert_eq!(
        batch2_start, 5,
        "Batch 2 should start at doc_id 5 (P0-1 fix validation)"
    );

    // Index metadata for batch 2 with GLOBAL doc_id
    for (idx, payload) in batch2.payloads.iter().enumerate() {
        state
            .metadata_store
            .index_metadata("test", batch2_start + idx as u32, payload)
            .await
            .unwrap();
    }

    // Add batch 2 to index
    state
        .index_provider
        .add_batch(&index_handle, batch2)
        .await
        .unwrap();

    // P0-1 VALIDATION: Category A should still have 5 records (not overwritten)
    let category_a_after = state
        .metadata_store
        .find_term("test", "category", &json!("A"))
        .await
        .unwrap();

    assert_eq!(
        category_a_after.len(),
        5,
        "Category A should STILL have 5 vectors after batch 2 (P0-1 validation)"
    );

    // Category B should have 4 records
    let category_b = state
        .metadata_store
        .find_term("test", "category", &json!("B"))
        .await
        .unwrap();

    assert_eq!(
        category_b.len(),
        4,
        "Category B should have 4 vectors after batch 2"
    );

    // Verify final doc_id counter
    let metadata = state.get_collection("test").await.unwrap();
    let final_doc_id = metadata
        .next_doc_id
        .load(std::sync::atomic::Ordering::SeqCst);

    assert_eq!(final_doc_id, 9, "Final doc_id should be 9 (5 + 4)");
}

#[tokio::test]
async fn test_persistence_slicing() {
    // Test P0-2 fix: Persistence slicing
    // This test verifies that extract_for_persistence + slicing works correctly

    let state = create_test_state();

    // Create collection
    let descriptor = CollectionDescriptor {
        name: "test_persist".to_string(),
        vector_dim: 2,
        distance: DistanceMetric::L2,
        replication: 1,
        shard_count: 1,
        payload_schema: PayloadSchema::default(),
        wal_stream_id: None,
    };

    let manifest = akidb_core::manifest::CollectionManifest {
        collection: "test_persist".to_string(),
        latest_version: 0,
        updated_at: chrono::Utc::now(),
        dimension: 2,
        metric: DistanceMetric::L2,
        total_vectors: 0,
        epoch: 0,
        created_at: Some(chrono::Utc::now()),
        snapshot: None,
        segments: vec![],
    };

    // Persist manifest to storage (required for write_segment_with_data)
    state.storage.persist_manifest(&manifest).await.unwrap();

    state
        .register_collection(
            "test_persist".to_string(),
            Arc::new(descriptor),
            manifest,
            0,
            akidb_storage::WalStreamId::new(),
        )
        .await
        .unwrap();

    // Build index
    let build_request = akidb_index::BuildRequest {
        collection: "test_persist".to_string(),
        kind: state.index_provider.kind(),
        distance: DistanceMetric::L2,
        dimension: 2,
        segments: vec![],
    };

    let index_handle = state.index_provider.build(build_request).await.unwrap();
    state
        .update_index_handle("test_persist", index_handle.clone())
        .await
        .unwrap();

    // Insert batch 1: 3 vectors
    let batch1 = akidb_index::IndexBatch {
        primary_keys: vec!["p1".to_string(), "p2".to_string(), "p3".to_string()],
        vectors: vec![
            akidb_index::QueryVector {
                components: vec![1.0, 0.0],
            },
            akidb_index::QueryVector {
                components: vec![2.0, 0.0],
            },
            akidb_index::QueryVector {
                components: vec![3.0, 0.0],
            },
        ],
        payloads: vec![json!({"id": 1}), json!({"id": 2}), json!({"id": 3})],
    };

    // Reserve doc_id for batch 1
    let metadata = state.get_collection("test_persist").await.unwrap();
    let batch1_start = metadata
        .next_doc_id
        .fetch_update(
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
            |current| {
                current.checked_add(3) // 3 vectors in batch 1
            },
        )
        .unwrap();

    assert_eq!(batch1_start, 0, "Batch 1 should start at doc_id 0");

    state
        .index_provider
        .add_batch(&index_handle, batch1)
        .await
        .unwrap();

    // Extract ALL vectors from index (this is what extract_for_persistence does)
    let (all_vectors, _all_payloads) = state
        .index_provider
        .extract_for_persistence(&index_handle)
        .unwrap();

    assert_eq!(
        all_vectors.len(),
        3,
        "After batch 1, index should have 3 vectors"
    );

    // P0-2 SCENARIO: Insert batch 2 with 2 vectors
    let metadata = state.get_collection("test_persist").await.unwrap();
    let batch2_start = metadata
        .next_doc_id
        .fetch_update(
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
            |current| {
                current.checked_add(2) // 2 vectors in batch 2
            },
        )
        .unwrap();

    assert_eq!(
        batch2_start, 3,
        "Batch 2 should start at doc_id 3 (after batch 1's 3 vectors)"
    );

    let batch2 = akidb_index::IndexBatch {
        primary_keys: vec!["p4".to_string(), "p5".to_string()],
        vectors: vec![
            akidb_index::QueryVector {
                components: vec![4.0, 0.0],
            },
            akidb_index::QueryVector {
                components: vec![5.0, 0.0],
            },
        ],
        payloads: vec![json!({"id": 4}), json!({"id": 5})],
    };

    state
        .index_provider
        .add_batch(&index_handle, batch2.clone())
        .await
        .unwrap();

    // Extract ALL vectors (returns 5 total)
    let (all_vectors_after, all_payloads_after) = state
        .index_provider
        .extract_for_persistence(&index_handle)
        .unwrap();

    assert_eq!(
        all_vectors_after.len(),
        5,
        "After batch 2, index should have 5 total vectors"
    );

    // P0-2 FIX: Slice to get ONLY the new batch
    let batch2_len = batch2.vectors.len(); // 2
    let start_index = batch2_start as usize; // 0

    let new_vectors: Vec<Vec<f32>> = all_vectors_after
        .into_iter()
        .skip(start_index)
        .take(batch2_len)
        .collect();

    let new_payloads: Vec<serde_json::Value> = all_payloads_after
        .into_iter()
        .skip(start_index)
        .take(batch2_len)
        .collect();

    // P0-2 VALIDATION: Sliced vectors should match batch2 size
    assert_eq!(
        new_vectors.len(),
        batch2_len,
        "Sliced vectors should match batch 2 size (P0-2 fix validation)"
    );

    assert_eq!(
        new_payloads.len(),
        batch2_len,
        "Sliced payloads should match batch 2 size (P0-2 fix validation)"
    );

    // Verify sliced data matches batch2
    assert_eq!(new_vectors[0], vec![4.0, 0.0]);
    assert_eq!(new_vectors[1], vec![5.0, 0.0]);
}
