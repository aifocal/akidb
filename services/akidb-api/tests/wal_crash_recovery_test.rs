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
use akidb_storage::{MetadataStore, 
    MemoryMetadataStore, MemoryStorageBackend, S3WalBackend, WalAppender, WalRecord, WalReplayer,
    WalStreamId,
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
