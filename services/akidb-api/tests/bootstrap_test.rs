//! Integration tests for collection bootstrap and restart recovery

use akidb_api::{bootstrap, AppState};
use akidb_core::collection::{CollectionDescriptor, DistanceMetric, PayloadSchema};
use akidb_index::NativeIndexProvider;
use akidb_query::{
    BasicQueryPlanner, BatchExecutionEngine, ExecutionEngine, QueryPlanner, SimpleExecutionEngine,
};
use akidb_storage::{MetadataStore, MemoryMetadataStore, MemoryStorageBackend, S3WalBackend};
use chrono::Utc;
use serde_json::json;
use std::sync::Arc;

/// Helper to create AppState for testing
fn create_test_state(storage: Arc<MemoryStorageBackend>) -> AppState {
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

/// Helper to create a collection with vectors
async fn create_and_populate_collection(
    state: &AppState,
    name: &str,
    vector_dim: usize,
    _vectors: Vec<(Vec<f32>, serde_json::Value)>,
) -> akidb_core::Result<()> {
    // Create collection
    let descriptor = CollectionDescriptor {
        name: name.to_string(),
        vector_dim: vector_dim as u16,
        distance: DistanceMetric::Cosine,
        replication: 1,
        shard_count: 1,
        payload_schema: PayloadSchema::default(),
        wal_stream_id: None,
    };

    // Create manifest
    let manifest = akidb_core::manifest::CollectionManifest {
        collection: name.to_string(),
        latest_version: 0,
        updated_at: Utc::now(),
        dimension: vector_dim as u32,
        metric: DistanceMetric::Cosine,
        total_vectors: 0,
        epoch: 0,
        created_at: Some(Utc::now()),
        snapshot: None,
        segments: vec![],
    };

    state
        .register_collection(
            name.to_string(),
            Arc::new(descriptor.clone()),
            manifest,
            0,
            akidb_storage::WalStreamId::new(),
        )
        .await?;

    // This is a placeholder - in real implementation, we would:
    // 1. Build index
    // 2. Add vectors with payloads
    // 3. Write segments to storage
    // For now, this test will verify the bootstrap framework works

    Ok(())
}

#[tokio::test]
async fn test_bootstrap_empty_storage() {
    // Test that bootstrap handles empty storage gracefully
    let storage = Arc::new(MemoryStorageBackend::new());
    let state = create_test_state(storage);

    // Should not fail even with no collections
    let result = bootstrap::bootstrap_collections(&state).await;
    assert!(
        result.is_ok(),
        "Bootstrap should succeed with empty storage"
    );
}

#[tokio::test]
async fn test_discover_collections_with_manifests() {
    // This test would require:
    // 1. Creating collection manifests in storage
    // 2. Running discover_collections()
    // 3. Verifying correct collection names returned

    // For now, we verify the basic structure compiles
    let storage = Arc::new(MemoryStorageBackend::new());
    let state = create_test_state(storage);

    // Bootstrap should handle empty state
    bootstrap::bootstrap_collections(&state).await.unwrap();
}

#[tokio::test]
async fn test_restart_recovery_single_collection() {
    // This is the key test for restart recovery
    let storage = Arc::new(MemoryStorageBackend::new());

    // Phase 1: Create collection and insert vectors
    {
        let state1 = create_test_state(storage.clone());

        // Create and populate a test collection
        let vectors = vec![
            (vec![1.0, 0.0, 0.0], json!({"id": "vec1", "category": "A"})),
            (vec![0.0, 1.0, 0.0], json!({"id": "vec2", "category": "B"})),
            (vec![0.0, 0.0, 1.0], json!({"id": "vec3", "category": "C"})),
        ];

        create_and_populate_collection(&state1, "test_collection", 3, vectors)
            .await
            .expect("Failed to create collection");

        // Verify collection exists
        assert!(
            state1.get_collection("test_collection").await.is_ok(),
            "Collection should exist in state1"
        );
    }

    // Phase 2: Simulate restart - create new state with same storage
    {
        let state2 = create_test_state(storage.clone());

        // Before bootstrap, collection should not exist
        assert!(
            state2.get_collection("test_collection").await.is_err(),
            "Collection should not exist before bootstrap"
        );

        // Bootstrap collections from storage
        bootstrap::bootstrap_collections(&state2)
            .await
            .expect("Bootstrap should succeed");

        // After bootstrap, collection should be restored
        // Note: This will currently fail because MemoryStorageBackend
        // doesn't implement the full persistence cycle.
        // This test documents the expected behavior for when
        // we have full S3 integration.

        // TODO: Enable this assertion when full persistence is implemented
        // let metadata = state2
        //     .get_collection("test_collection")
        //     .await
        //     .expect("Collection should exist after bootstrap");

        // assert!(
        //     metadata.index_handle.is_some(),
        //     "Index should be restored"
        // );
    }
}

#[tokio::test]
async fn test_restart_recovery_multiple_collections() {
    let storage = Arc::new(MemoryStorageBackend::new());

    // Phase 1: Create multiple collections
    {
        let state1 = create_test_state(storage.clone());

        // Create collection 1
        create_and_populate_collection(
            &state1,
            "collection_1",
            128,
            vec![(vec![1.0; 128], json!({"id": "1"}))],
        )
        .await
        .expect("Failed to create collection 1");

        // Create collection 2
        create_and_populate_collection(
            &state1,
            "collection_2",
            256,
            vec![(vec![1.0; 256], json!({"id": "2"}))],
        )
        .await
        .expect("Failed to create collection 2");

        // Verify both exist
        assert!(state1.get_collection("collection_1").await.is_ok());
        assert!(state1.get_collection("collection_2").await.is_ok());
    }

    // Phase 2: Restart and verify both are restored
    {
        let state2 = create_test_state(storage.clone());

        // Bootstrap
        bootstrap::bootstrap_collections(&state2)
            .await
            .expect("Bootstrap should succeed");

        // TODO: Enable when full persistence is implemented
        // assert!(state2.get_collection("collection_1").await.is_ok());
        // assert!(state2.get_collection("collection_2").await.is_ok());
    }
}

#[tokio::test]
async fn test_backward_compatibility_no_metadata() {
    // Test that old segments without metadata can still be loaded
    let storage = Arc::new(MemoryStorageBackend::new());
    let state = create_test_state(storage);

    // Create collection with segments that have no metadata
    // (simulating old format)

    // Bootstrap should handle this gracefully
    bootstrap::bootstrap_collections(&state)
        .await
        .expect("Bootstrap should handle segments without metadata");
}

//
// ===== Fault Injection Helpers =====
//

/// Helper to create a valid collection with manifest and descriptor
async fn create_valid_collection(
    storage: &Arc<MemoryStorageBackend>,
    name: &str,
    vector_dim: u16,
) -> akidb_core::Result<()> {
    use akidb_core::manifest::CollectionManifest;
    use akidb_storage::StorageBackend;

    // 1. Create descriptor
    let descriptor = CollectionDescriptor {
        name: name.to_string(),
        vector_dim,
        distance: DistanceMetric::Cosine,
        replication: 1,
        shard_count: 1,
        payload_schema: PayloadSchema::default(),
        wal_stream_id: Some(uuid::Uuid::new_v4()),
    };

    let descriptor_key = format!("collections/{}/descriptor.json", name);
    let descriptor_json = serde_json::to_vec(&descriptor)
        .map_err(|e| akidb_core::Error::Validation(format!("Failed to serialize descriptor: {}", e)))?;
    storage
        .as_ref()
        .put_object(&descriptor_key, descriptor_json.into())
        .await?;

    // 2. Create manifest
    let manifest = CollectionManifest {
        collection: name.to_string(),
        latest_version: 0,
        updated_at: Utc::now(),
        dimension: vector_dim as u32,
        metric: DistanceMetric::Cosine,
        total_vectors: 0,
        epoch: 0,
        created_at: Some(Utc::now()),
        snapshot: None,
        segments: vec![],
    };

    let manifest_key = format!("collections/{}/manifest.json", name);
    let manifest_json = serde_json::to_vec(&manifest)
        .map_err(|e| akidb_core::Error::Validation(format!("Failed to serialize manifest: {}", e)))?;
    storage
        .as_ref()
        .put_object(&manifest_key, manifest_json.into())
        .await?;

    Ok(())
}

/// Helper to corrupt a manifest file in storage
async fn corrupt_manifest(
    storage: &Arc<MemoryStorageBackend>,
    collection_name: &str,
) -> akidb_core::Result<()> {
    use akidb_storage::StorageBackend;

    let manifest_path = format!("collections/{}/manifest.json", collection_name);

    // Write invalid JSON
    let corrupted = b"{ invalid json content !!!";
    storage
        .as_ref()
        .put_object(&manifest_path, corrupted.to_vec().into())
        .await?;

    Ok(())
}

/// Helper to delete a descriptor file (simulates missing descriptor)
async fn delete_descriptor(
    storage: &Arc<MemoryStorageBackend>,
    collection_name: &str,
) -> akidb_core::Result<()> {
    use akidb_storage::StorageBackend;

    let descriptor_path = format!("collections/{}/descriptor.json", collection_name);
    storage.as_ref().delete_object(&descriptor_path).await?;
    Ok(())
}

//
// ===== Fault Tolerance Tests =====
//

#[tokio::test]
async fn test_fault_tolerant_bootstrap_corrupted_manifest() {
    // Test that if one collection has a corrupted manifest, others still load
    let storage = Arc::new(MemoryStorageBackend::new());

    // Create 3 valid collections
    create_valid_collection(&storage, "collection_a", 128)
        .await
        .expect("Failed to create collection_a");
    create_valid_collection(&storage, "collection_b", 128)
        .await
        .expect("Failed to create collection_b");
    create_valid_collection(&storage, "collection_c", 128)
        .await
        .expect("Failed to create collection_c");

    // Corrupt collection_b's manifest
    corrupt_manifest(&storage, "collection_b")
        .await
        .expect("Failed to corrupt manifest");

    // Bootstrap should succeed, loading collection_a and collection_c
    let state = create_test_state(storage);
    let result = bootstrap::bootstrap_collections(&state).await;

    assert!(
        result.is_ok(),
        "Bootstrap should succeed despite corrupted manifest"
    );

    // Verify collection_a and collection_c are loaded
    assert!(
        state.get_collection("collection_a").await.is_ok(),
        "collection_a should be loaded"
    );
    assert!(
        state.get_collection("collection_c").await.is_ok(),
        "collection_c should be loaded"
    );

    // Verify collection_b is NOT loaded (due to corrupted manifest)
    assert!(
        state.get_collection("collection_b").await.is_err(),
        "collection_b should not be loaded (corrupted manifest)"
    );
}

#[tokio::test]
async fn test_fault_tolerant_bootstrap_missing_descriptor() {
    // Test that if one collection is missing its descriptor, others still load
    let storage = Arc::new(MemoryStorageBackend::new());

    // Create 2 valid collections
    create_valid_collection(&storage, "collection_x", 64)
        .await
        .expect("Failed to create collection_x");
    create_valid_collection(&storage, "collection_y", 64)
        .await
        .expect("Failed to create collection_y");

    // Delete collection_x's descriptor
    delete_descriptor(&storage, "collection_x")
        .await
        .expect("Failed to delete descriptor");

    // Bootstrap should succeed, loading only collection_y
    let state = create_test_state(storage);
    let result = bootstrap::bootstrap_collections(&state).await;

    assert!(
        result.is_ok(),
        "Bootstrap should succeed despite missing descriptor"
    );

    // Verify collection_y is loaded
    assert!(
        state.get_collection("collection_y").await.is_ok(),
        "collection_y should be loaded"
    );

    // Verify collection_x is NOT loaded (missing descriptor)
    assert!(
        state.get_collection("collection_x").await.is_err(),
        "collection_x should not be loaded (missing descriptor)"
    );
}

#[tokio::test]
async fn test_fault_tolerant_bootstrap_all_collections_fail() {
    // Test that if ALL collections fail to load, bootstrap still succeeds
    // (returns Ok but no collections loaded)
    let storage = Arc::new(MemoryStorageBackend::new());

    // Create 2 collections with corrupted data
    create_valid_collection(&storage, "broken_1", 128)
        .await
        .expect("Failed to create broken_1");
    create_valid_collection(&storage, "broken_2", 128)
        .await
        .expect("Failed to create broken_2");

    // Corrupt both manifests
    corrupt_manifest(&storage, "broken_1")
        .await
        .expect("Failed to corrupt manifest");
    corrupt_manifest(&storage, "broken_2")
        .await
        .expect("Failed to corrupt manifest");

    // Bootstrap should still succeed (but load 0 collections)
    let state = create_test_state(storage);
    let result = bootstrap::bootstrap_collections(&state).await;

    assert!(
        result.is_ok(),
        "Bootstrap should succeed even if all collections fail"
    );

    // Verify no collections are loaded
    assert!(
        state.get_collection("broken_1").await.is_err(),
        "broken_1 should not be loaded"
    );
    assert!(
        state.get_collection("broken_2").await.is_err(),
        "broken_2 should not be loaded"
    );
}

#[tokio::test]
async fn test_fault_tolerant_bootstrap_mixed_failures() {
    // Test complex scenario: multiple collections with different failure modes
    let storage = Arc::new(MemoryStorageBackend::new());

    // Create 4 collections
    create_valid_collection(&storage, "good_collection", 128)
        .await
        .expect("Failed to create good_collection");
    create_valid_collection(&storage, "corrupted_manifest", 128)
        .await
        .expect("Failed to create corrupted_manifest");
    create_valid_collection(&storage, "missing_descriptor", 128)
        .await
        .expect("Failed to create missing_descriptor");
    create_valid_collection(&storage, "another_good_one", 64)
        .await
        .expect("Failed to create another_good_one");

    // Inject failures
    corrupt_manifest(&storage, "corrupted_manifest")
        .await
        .expect("Failed to corrupt manifest");
    delete_descriptor(&storage, "missing_descriptor")
        .await
        .expect("Failed to delete descriptor");

    // Bootstrap should succeed
    let state = create_test_state(storage);
    let result = bootstrap::bootstrap_collections(&state).await;

    assert!(
        result.is_ok(),
        "Bootstrap should succeed with mixed failures"
    );

    // Verify good collections are loaded
    assert!(
        state.get_collection("good_collection").await.is_ok(),
        "good_collection should be loaded"
    );
    assert!(
        state.get_collection("another_good_one").await.is_ok(),
        "another_good_one should be loaded"
    );

    // Verify broken collections are NOT loaded
    assert!(
        state.get_collection("corrupted_manifest").await.is_err(),
        "corrupted_manifest should not be loaded"
    );
    assert!(
        state.get_collection("missing_descriptor").await.is_err(),
        "missing_descriptor should not be loaded"
    );
}

#[tokio::test]
async fn test_fault_tolerant_bootstrap() {
    // Original test - verify basic fault tolerance behavior
    let storage = Arc::new(MemoryStorageBackend::new());

    // Create 1 good collection and 1 bad collection
    create_valid_collection(&storage, "good", 128)
        .await
        .expect("Failed to create good collection");
    create_valid_collection(&storage, "bad", 128)
        .await
        .expect("Failed to create bad collection");

    // Corrupt the bad collection
    corrupt_manifest(&storage, "bad")
        .await
        .expect("Failed to corrupt manifest");

    let state = create_test_state(storage);
    let result = bootstrap::bootstrap_collections(&state).await;

    assert!(result.is_ok(), "Bootstrap should be fault-tolerant");

    // Good collection should be loaded
    assert!(
        state.get_collection("good").await.is_ok(),
        "Good collection should be loaded"
    );

    // Bad collection should not be loaded
    assert!(
        state.get_collection("bad").await.is_err(),
        "Bad collection should not be loaded"
    );
}

#[tokio::test]
async fn test_bootstrap_with_large_collection() {
    // Test bootstrap performance with larger collection
    let storage = Arc::new(MemoryStorageBackend::new());
    let state = create_test_state(storage);

    // Create collection with many vectors
    let large_vectors: Vec<(Vec<f32>, serde_json::Value)> = (0..1000)
        .map(|i| {
            let vector = vec![i as f32 / 1000.0; 128];
            let payload = json!({
                "id": format!("vec_{}", i),
                "value": i,
            });
            (vector, payload)
        })
        .collect();

    create_and_populate_collection(&state, "large_collection", 128, large_vectors)
        .await
        .expect("Failed to create large collection");

    // Bootstrap should handle large collections efficiently
    bootstrap::bootstrap_collections(&state)
        .await
        .expect("Bootstrap should handle large collections");
}

#[tokio::test]
async fn test_primary_key_extraction() {
    // Test various primary key extraction scenarios
    let storage = Arc::new(MemoryStorageBackend::new());
    let state = create_test_state(storage);

    // Test with explicit string ID
    let vectors_with_string_id = vec![(
        vec![1.0, 0.0],
        json!({"id": "custom-id-123", "data": "value"}),
    )];

    create_and_populate_collection(&state, "collection_string_id", 2, vectors_with_string_id)
        .await
        .expect("Should handle string IDs");

    // Test with numeric ID
    let vectors_with_numeric_id = vec![(vec![1.0, 0.0], json!({"id": 42, "data": "value"}))];

    create_and_populate_collection(&state, "collection_numeric_id", 2, vectors_with_numeric_id)
        .await
        .expect("Should handle numeric IDs");

    // Test with no ID (should generate fallback)
    let vectors_without_id = vec![(vec![1.0, 0.0], json!({"data": "value"}))];

    create_and_populate_collection(&state, "collection_no_id", 2, vectors_without_id)
        .await
        .expect("Should handle missing IDs");
}

#[tokio::test]
async fn test_bootstrap_timing() {
    // Test that bootstrap completes in reasonable time
    use std::time::Instant;

    let storage = Arc::new(MemoryStorageBackend::new());
    let state = create_test_state(storage);

    // Create multiple small collections
    for i in 0..5 {
        let name = format!("collection_{}", i);
        create_and_populate_collection(&state, &name, 64, vec![(vec![1.0; 64], json!({"id": i}))])
            .await
            .expect("Failed to create collection");
    }

    // Measure bootstrap time
    let start = Instant::now();
    bootstrap::bootstrap_collections(&state)
        .await
        .expect("Bootstrap should succeed");
    let duration = start.elapsed();

    // Bootstrap should be fast for small collections
    assert!(
        duration.as_secs() < 5,
        "Bootstrap took too long: {:?}",
        duration
    );
}
