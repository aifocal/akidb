//! Test WAL crash recovery (simulated restart)
//!
//! This test verifies that WAL replay works correctly during bootstrap,
//! ensuring uncommitted writes are recovered after a simulated crash.

use akidb_api::state::AppState;
use akidb_core::{collection::CollectionDescriptor, manifest::CollectionManifest, DistanceMetric};
use akidb_index::NativeIndexProvider;
use akidb_query::{
    BasicQueryPlanner, BatchExecutionEngine, ExecutionEngine, QueryPlanner, SimpleExecutionEngine,
};
use akidb_storage::{
    MemoryMetadataStore, MemoryStorageBackend, MetadataStore, S3WalBackend, WalAppender, WalRecord,
    WalReplayer, WalStreamId,
};
use serde_json::json;
use std::sync::Arc;

/// Helper to create a test AppState
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
async fn test_wal_replay_after_simulated_crash() {
    // Step 1: Create AppState and collection
    let state = create_test_state();

    // Create collection manually
    let wal_stream_id = WalStreamId::new();
    let descriptor = Arc::new(CollectionDescriptor {
        name: "test_crash_recovery".to_string(),
        vector_dim: 128,
        distance: DistanceMetric::Cosine,
        replication: 1,
        shard_count: 1,
        payload_schema: Default::default(),
        wal_stream_id: Some(wal_stream_id.0),
    });

    // Create manifest
    let now = chrono::Utc::now();
    let manifest = CollectionManifest {
        collection: "test_crash_recovery".to_string(),
        latest_version: 0,
        updated_at: now,
        dimension: 128,
        metric: DistanceMetric::Cosine,
        total_vectors: 0,
        epoch: 0,
        created_at: Some(now),
        snapshot: None,
        segments: Vec::new(),
    };

    // Create in storage
    state.storage.create_collection(&descriptor).await.unwrap();

    // Register in state
    state
        .register_collection(
            "test_crash_recovery".to_string(),
            descriptor.clone(),
            manifest.clone(),
            0,
            wal_stream_id,
        )
        .await
        .unwrap();

    // Step 2: Insert vectors and sync to WAL (but not to segments)
    let wal = state.wal.clone();

    for i in 0..5 {
        let record = WalRecord::Insert {
            collection: "test_crash_recovery".to_string(),
            primary_key: format!("key_{}", i),
            vector: vec![i as f32; 128],
            payload: json!({"id": i}),
        };
        wal.append(wal_stream_id, record).await.unwrap();
    }

    // Sync WAL to S3 (but do NOT persist segments - simulating incomplete flush)
    wal.sync(wal_stream_id).await.unwrap();

    // Verify WAL entries were written
    let replay_stats = wal.replay(wal_stream_id, None).await.unwrap();
    assert_eq!(
        replay_stats.records, 5,
        "Should have 5 records in WAL before crash"
    );

    // Persist descriptor and manifest (normally done by create_collection)
    let desc_key = format!("collections/{}/descriptor.json", "test_crash_recovery");
    let desc_data = serde_json::to_vec(descriptor.as_ref()).unwrap();
    state
        .storage
        .put_object(&desc_key, desc_data.into())
        .await
        .unwrap();

    // IMPORTANT: Also persist the manifest so bootstrap can find the collection
    state.storage.persist_manifest(&manifest).await.unwrap();

    // Step 3: Simulate crash - create new AppState (fresh memory, same S3)
    // In a real scenario, this would be a server restart
    let state2 = create_test_state();

    // IMPORTANT: We need to share the same storage and WAL backend
    // to simulate persistent S3 state across restarts
    // For this test, we'll need to copy the data

    // Copy storage data from state to state2
    // This simulates persistent S3 storage
    // (In reality, both would point to the same S3 bucket)

    // For MemoryStorageBackend, we need to manually copy the data
    // Since we're using different MemoryStorageBackend instances,
    // let's use the same shared backend instead

    // Actually, let me re-architect this test to use shared storage
    drop(state2);

    // Create state2 with SAME storage and WAL backends
    let storage2 = state.storage.clone();
    let wal2 = state.wal.clone();
    let index_provider2 = Arc::new(NativeIndexProvider::new());
    let planner2: Arc<dyn QueryPlanner> = Arc::new(BasicQueryPlanner::new());
    let engine2: Arc<dyn ExecutionEngine> =
        Arc::new(SimpleExecutionEngine::new(index_provider2.clone()));
    let metadata_store2: Arc<dyn MetadataStore> = Arc::new(MemoryMetadataStore::new());
    let batch_engine2 = Arc::new(BatchExecutionEngine::new(
        Arc::clone(&engine2),
        Arc::clone(&metadata_store2),
    ));
    let query_cache2 = Arc::new(akidb_api::query_cache::QueryCache::default());

    let state2 = AppState::new(
        storage2,
        index_provider2,
        planner2,
        engine2,
        batch_engine2,
        metadata_store2,
        wal2,
        query_cache2,
    );

    // Step 4: Bootstrap (should trigger WAL replay)
    akidb_api::bootstrap::bootstrap_collections(&state2)
        .await
        .unwrap();

    // Step 5: Verify that WAL records were replayed
    let collection = state2.get_collection("test_crash_recovery").await.unwrap();

    let next_doc_id = collection
        .next_doc_id
        .load(std::sync::atomic::Ordering::SeqCst);

    // Should have 5 documents from WAL replay
    assert_eq!(
        next_doc_id, 5,
        "WAL replay should restore 5 documents from uncommitted WAL"
    );

    println!("✅ WAL replay test passed: 5 documents recovered after simulated crash");
}

#[tokio::test]
async fn test_wal_replay_with_persisted_segments() {
    // Test that WAL replay works alongside persisted segments
    // Scenario:
    // 1. Insert 10 vectors → seal segment
    // 2. Insert 5 more vectors (only in WAL, not in segment)
    // 3. Crash
    // 4. Bootstrap should load 10 from segment + 5 from WAL = 15 total

    // This test is more complex and requires segment sealing logic
    // For now, we'll skip it as it requires more infrastructure
    // TODO: Implement this test when segment sealing is ready

    println!("⏭️  Skipping test_wal_replay_with_persisted_segments (requires segment sealing)");
}

#[tokio::test]
async fn test_no_wal_records_bootstrap() {
    // Test that bootstrap works correctly when there are no WAL records
    let state = create_test_state();

    // Create collection manually
    let _wal_stream_id = WalStreamId::new();

    // Persist descriptor
    let descriptor = CollectionDescriptor {
        name: "test_no_wal".to_string(),
        vector_dim: 128,
        distance: DistanceMetric::L2,
        replication: 1,
        shard_count: 1,
        payload_schema: Default::default(),
        wal_stream_id: Some(uuid::Uuid::new_v4()),
    };

    // Persist descriptor
    let desc_key = format!("collections/{}/descriptor.json", "test_no_wal");
    let desc_data = serde_json::to_vec(&descriptor).unwrap();
    state
        .storage
        .put_object(&desc_key, desc_data.into())
        .await
        .unwrap();

    // Persist empty manifest
    let now = chrono::Utc::now();
    let manifest = CollectionManifest {
        collection: "test_no_wal".to_string(),
        latest_version: 0,
        updated_at: now,
        dimension: 128,
        metric: DistanceMetric::L2,
        total_vectors: 0,
        epoch: 0,
        created_at: Some(now),
        snapshot: None,
        segments: Vec::new(),
    };
    state.storage.persist_manifest(&manifest).await.unwrap();

    // Create new state and bootstrap
    let storage2 = state.storage.clone();
    let wal2 = state.wal.clone();
    let index_provider2 = Arc::new(NativeIndexProvider::new());
    let planner2: Arc<dyn QueryPlanner> = Arc::new(BasicQueryPlanner::new());
    let engine2: Arc<dyn ExecutionEngine> =
        Arc::new(SimpleExecutionEngine::new(index_provider2.clone()));
    let metadata_store2: Arc<dyn MetadataStore> = Arc::new(MemoryMetadataStore::new());
    let batch_engine2 = Arc::new(BatchExecutionEngine::new(
        Arc::clone(&engine2),
        Arc::clone(&metadata_store2),
    ));

    let query_cache2 = Arc::new(akidb_api::query_cache::QueryCache::default());

    let state2 = AppState::new(
        storage2,
        index_provider2,
        planner2,
        engine2,
        batch_engine2,
        metadata_store2,
        wal2,
        query_cache2,
    );

    // Bootstrap should succeed without any WAL records
    akidb_api::bootstrap::bootstrap_collections(&state2)
        .await
        .unwrap();

    let collection = state2.get_collection("test_no_wal").await.unwrap();
    let next_doc_id = collection
        .next_doc_id
        .load(std::sync::atomic::Ordering::SeqCst);

    assert_eq!(
        next_doc_id, 0,
        "Should have 0 documents when no WAL records"
    );

    println!("✅ No WAL records test passed: bootstrap works with empty WAL");
}

#[tokio::test]
async fn test_lsn_recovery_with_builder_pattern() {
    //! Test that builder pattern correctly recovers LSN counters from S3
    //!
    //! This test validates the fix for P1 Bug #5 (LSN counter initialization).
    //! Without the builder pattern, LSN counters would start at 0 on restart,
    //! causing data loss by overwriting existing WAL entries.
    //!
    //! Test scenario:
    //! 1. Create WAL backend with builder (auto-recovery)
    //! 2. Append 5 entries (LSN 1-5) and sync to S3
    //! 3. Create NEW WAL backend with builder (simulating restart)
    //! 4. Verify LSN counter was recovered (should be 5)
    //! 5. Append new entry and verify it gets LSN 6 (not LSN 1!)

    // Shared storage for both WAL backends
    let storage = Arc::new(MemoryStorageBackend::new());
    let stream_id = WalStreamId::new();

    // Phase 1: Create first WAL backend using builder
    let wal1 = Arc::new(
        S3WalBackend::builder(storage.clone())
            .build()
            .await
            .unwrap(),
    );

    // Append 5 WAL entries
    let mut last_lsn = None;
    for i in 0..5 {
        let record = WalRecord::Insert {
            collection: "test_lsn_recovery".to_string(),
            primary_key: format!("key_{}", i),
            vector: vec![i as f32; 128],
            payload: json!({"id": i}),
        };
        let lsn = wal1.append(stream_id, record).await.unwrap();
        last_lsn = Some(lsn);

        // Verify LSNs are sequential
        assert_eq!(
            lsn.value(),
            (i + 1) as u64,
            "LSN should be sequential starting from 1"
        );
    }

    assert_eq!(
        last_lsn.unwrap().value(),
        5,
        "Last LSN should be 5 after 5 appends"
    );

    // Sync to S3
    wal1.sync(stream_id).await.unwrap();

    // Verify WAL entries are persisted
    let replay_stats = wal1.replay(stream_id, None).await.unwrap();
    assert_eq!(replay_stats.records, 5, "Should have 5 records in S3");

    // Phase 2: Drop first WAL backend and create NEW one with builder
    // This simulates a server restart
    drop(wal1);

    // Create new WAL backend using builder (should auto-recover LSN counter)
    let wal2 = Arc::new(
        S3WalBackend::builder(storage.clone())
            .build()
            .await
            .unwrap(),
    );

    // Phase 3: Append new entry to verify LSN counter was recovered
    let record_new = WalRecord::Insert {
        collection: "test_lsn_recovery".to_string(),
        primary_key: "key_5".to_string(),
        vector: vec![5.0; 128],
        payload: json!({"id": 5}),
    };

    let new_lsn = wal2.append(stream_id, record_new).await.unwrap();

    // CRITICAL ASSERTION: LSN should continue from 6, NOT restart from 1
    assert_eq!(
        new_lsn.value(),
        6,
        "LSN should continue from 6 after recovery (P1 Bug #5 validation)"
    );

    // Sync and verify total entries
    wal2.sync(stream_id).await.unwrap();

    let final_stats = wal2.replay(stream_id, None).await.unwrap();
    assert_eq!(
        final_stats.records, 6,
        "Should have 6 total records (5 before restart + 1 after)"
    );

    println!("✅ LSN recovery test passed: LSN counter correctly recovered from S3 (6 after restart, not 1)");
}

#[tokio::test]
async fn test_wal_replay_delete_operations() {
    //! Test that WAL replay correctly handles Delete operations
    //!
    //! Scenario:
    //! 1. Insert 5 vectors (WAL entries 1-5)
    //! 2. Delete 2 vectors (WAL entries 6-7)
    //! 3. Crash (simulated)
    //! 4. Bootstrap should replay all operations: 5 inserts, then 2 deletes
    //! 5. Final state should have only 3 vectors

    let state = create_test_state();
    let wal_stream_id = WalStreamId::new();

    // Create collection
    let descriptor = Arc::new(CollectionDescriptor {
        name: "test_delete_replay".to_string(),
        vector_dim: 128,
        distance: DistanceMetric::Cosine,
        replication: 1,
        shard_count: 1,
        payload_schema: Default::default(),
        wal_stream_id: Some(wal_stream_id.0),
    });

    state.storage.create_collection(&descriptor).await.unwrap();

    let now = chrono::Utc::now();
    let manifest = CollectionManifest {
        collection: "test_delete_replay".to_string(),
        latest_version: 0,
        updated_at: now,
        dimension: 128,
        metric: DistanceMetric::Cosine,
        total_vectors: 0,
        epoch: 0,
        created_at: Some(now),
        snapshot: None,
        segments: Vec::new(),
    };

    state.storage.persist_manifest(&manifest).await.unwrap();

    // Persist descriptor
    let desc_key = format!("collections/{}/descriptor.json", "test_delete_replay");
    let desc_data = serde_json::to_vec(descriptor.as_ref()).unwrap();
    state
        .storage
        .put_object(&desc_key, desc_data.into())
        .await
        .unwrap();

    let wal = state.wal.clone();

    // Phase 1: Insert 5 vectors
    for i in 0..5 {
        let record = WalRecord::Insert {
            collection: "test_delete_replay".to_string(),
            primary_key: format!("key_{}", i),
            vector: vec![i as f32; 128],
            payload: json!({"id": i, "category": "test"}),
        };
        wal.append(wal_stream_id, record).await.unwrap();
    }

    // Phase 2: Delete 2 vectors (key_1 and key_3)
    for key_id in [1, 3] {
        let record = WalRecord::Delete {
            collection: "test_delete_replay".to_string(),
            primary_key: format!("key_{}", key_id),
        };
        wal.append(wal_stream_id, record).await.unwrap();
    }

    // Sync WAL to S3
    wal.sync(wal_stream_id).await.unwrap();

    // Verify WAL has 7 entries (5 inserts + 2 deletes)
    let replay_stats = wal.replay(wal_stream_id, None).await.unwrap();
    assert_eq!(
        replay_stats.records, 7,
        "Should have 7 WAL records before crash"
    );

    // Phase 3: Simulate crash - create new state
    let storage2 = state.storage.clone();
    let wal2 = state.wal.clone();
    let index_provider2 = Arc::new(NativeIndexProvider::new());
    let planner2: Arc<dyn QueryPlanner> = Arc::new(BasicQueryPlanner::new());
    let engine2: Arc<dyn ExecutionEngine> =
        Arc::new(SimpleExecutionEngine::new(index_provider2.clone()));
    let metadata_store2: Arc<dyn MetadataStore> = Arc::new(MemoryMetadataStore::new());
    let batch_engine2 = Arc::new(BatchExecutionEngine::new(
        Arc::clone(&engine2),
        Arc::clone(&metadata_store2),
    ));
    let query_cache2 = Arc::new(akidb_api::query_cache::QueryCache::default());

    let state2 = AppState::new(
        storage2,
        index_provider2,
        planner2,
        engine2,
        batch_engine2,
        metadata_store2,
        wal2,
        query_cache2,
    );

    // Phase 4: Bootstrap (should replay 5 inserts, then 2 deletes)
    akidb_api::bootstrap::bootstrap_collections(&state2)
        .await
        .unwrap();

    // Phase 5: Verify final state has 3 vectors (key_0, key_2, key_4)
    let collection = state2.get_collection("test_delete_replay").await.unwrap();

    // Check next_doc_id reflects inserts (5 vectors were inserted, even though 2 were deleted)
    // doc_id counter should be 5 (not decremented by deletes)
    let next_doc_id = collection
        .next_doc_id
        .load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        next_doc_id, 5,
        "next_doc_id should be 5 (delete doesn't decrement counter)"
    );

    // Verify the deleted vectors are actually gone from metadata store
    // Keys key_1 and key_3 should not exist in metadata
    let all_docs = state2
        .metadata_store
        .get_all_docs("test_delete_replay")
        .await
        .unwrap();

    // Should have 3 documents (0, 2, 4) since doc_ids match insertion order
    assert_eq!(
        all_docs.len(),
        3,
        "Should have 3 documents after deleting 2 out of 5"
    );

    // Verify specific doc_ids exist: 0, 2, 4 (corresponding to key_0, key_2, key_4)
    assert!(all_docs.contains(0), "doc_id 0 (key_0) should exist");
    assert!(!all_docs.contains(1), "doc_id 1 (key_1) should be deleted");
    assert!(all_docs.contains(2), "doc_id 2 (key_2) should exist");
    assert!(!all_docs.contains(3), "doc_id 3 (key_3) should be deleted");
    assert!(all_docs.contains(4), "doc_id 4 (key_4) should exist");

    println!("✅ WAL replay Delete test passed: 2 deletes correctly applied, 3 vectors remain");
}

#[tokio::test]
async fn test_wal_replay_upsert_operations() {
    //! Test that WAL replay correctly handles UpsertPayload operations
    //!
    //! Scenario:
    //! 1. Insert 3 vectors with payload {"version": 1}
    //! 2. Upsert 2 vectors with payload {"version": 2}
    //! 3. Crash (simulated)
    //! 4. Bootstrap should replay all operations
    //! 5. Final state should have 3 vectors, with 2 having version=2

    let state = create_test_state();
    let wal_stream_id = WalStreamId::new();

    // Create collection
    let descriptor = Arc::new(CollectionDescriptor {
        name: "test_upsert_replay".to_string(),
        vector_dim: 128,
        distance: DistanceMetric::L2,
        replication: 1,
        shard_count: 1,
        payload_schema: Default::default(),
        wal_stream_id: Some(wal_stream_id.0),
    });

    state.storage.create_collection(&descriptor).await.unwrap();

    let now = chrono::Utc::now();
    let manifest = CollectionManifest {
        collection: "test_upsert_replay".to_string(),
        latest_version: 0,
        updated_at: now,
        dimension: 128,
        metric: DistanceMetric::L2,
        total_vectors: 0,
        epoch: 0,
        created_at: Some(now),
        snapshot: None,
        segments: Vec::new(),
    };

    state.storage.persist_manifest(&manifest).await.unwrap();

    // Persist descriptor
    let desc_key = format!("collections/{}/descriptor.json", "test_upsert_replay");
    let desc_data = serde_json::to_vec(descriptor.as_ref()).unwrap();
    state
        .storage
        .put_object(&desc_key, desc_data.into())
        .await
        .unwrap();

    let wal = state.wal.clone();

    // Phase 1: Insert 3 vectors with version=1
    for i in 0..3 {
        let record = WalRecord::Insert {
            collection: "test_upsert_replay".to_string(),
            primary_key: format!("key_{}", i),
            vector: vec![i as f32; 128],
            payload: json!({"id": i, "version": 1, "data": format!("original_{}", i)}),
        };
        wal.append(wal_stream_id, record).await.unwrap();
    }

    // Phase 2: Upsert payload for key_0 and key_2 (change version to 2)
    for key_id in [0, 2] {
        let record = WalRecord::UpsertPayload {
            collection: "test_upsert_replay".to_string(),
            primary_key: format!("key_{}", key_id),
            payload: json!({"id": key_id, "version": 2, "data": format!("updated_{}", key_id)}),
        };
        wal.append(wal_stream_id, record).await.unwrap();
    }

    // Sync WAL to S3
    wal.sync(wal_stream_id).await.unwrap();

    // Verify WAL has 5 entries (3 inserts + 2 upserts)
    let replay_stats = wal.replay(wal_stream_id, None).await.unwrap();
    assert_eq!(
        replay_stats.records, 5,
        "Should have 5 WAL records before crash"
    );

    // Phase 3: Simulate crash - create new state
    let storage2 = state.storage.clone();
    let wal2 = state.wal.clone();
    let index_provider2 = Arc::new(NativeIndexProvider::new());
    let planner2: Arc<dyn QueryPlanner> = Arc::new(BasicQueryPlanner::new());
    let engine2: Arc<dyn ExecutionEngine> =
        Arc::new(SimpleExecutionEngine::new(index_provider2.clone()));
    let metadata_store2: Arc<dyn MetadataStore> = Arc::new(MemoryMetadataStore::new());
    let batch_engine2 = Arc::new(BatchExecutionEngine::new(
        Arc::clone(&engine2),
        Arc::clone(&metadata_store2),
    ));
    let query_cache2 = Arc::new(akidb_api::query_cache::QueryCache::default());

    let state2 = AppState::new(
        storage2,
        index_provider2,
        planner2,
        engine2,
        batch_engine2,
        metadata_store2,
        wal2,
        query_cache2,
    );

    // Phase 4: Bootstrap (should replay 3 inserts, then 2 upserts)
    akidb_api::bootstrap::bootstrap_collections(&state2)
        .await
        .unwrap();

    // Phase 5: Verify final state
    let collection = state2.get_collection("test_upsert_replay").await.unwrap();

    // Should have 3 vectors
    let next_doc_id = collection
        .next_doc_id
        .load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(next_doc_id, 3, "Should have 3 vectors");

    // Verify all 3 documents exist in metadata store
    let all_docs = state2
        .metadata_store
        .get_all_docs("test_upsert_replay")
        .await
        .unwrap();
    assert_eq!(all_docs.len(), 3, "Should have 3 documents");

    // Verify doc_ids 0, 1, 2 exist
    assert!(all_docs.contains(0), "doc_id 0 (key_0) should exist");
    assert!(all_docs.contains(1), "doc_id 1 (key_1) should exist");
    assert!(all_docs.contains(2), "doc_id 2 (key_2) should exist");

    // Verify version field was updated for doc_id 0 and 2
    // We can check this by querying metadata store with version=2
    let version2_docs = state2
        .metadata_store
        .find_term("test_upsert_replay", "version", &json!(2))
        .await
        .unwrap();

    // Should have 2 documents with version=2 (key_0 and key_2)
    assert_eq!(
        version2_docs.len(),
        2,
        "Should have 2 documents with version=2 after upsert"
    );
    assert!(version2_docs.contains(0), "doc_id 0 should have version=2");
    assert!(version2_docs.contains(2), "doc_id 2 should have version=2");

    // Verify key_1 still has version=1
    let version1_docs = state2
        .metadata_store
        .find_term("test_upsert_replay", "version", &json!(1))
        .await
        .unwrap();
    assert_eq!(
        version1_docs.len(),
        1,
        "Should have 1 document with version=1"
    );
    assert!(version1_docs.contains(1), "doc_id 1 should have version=1");

    println!("✅ WAL replay Upsert test passed: 2 payloads correctly updated, 1 unchanged");
}
