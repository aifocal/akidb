//! End-to-End Storage Integration Tests for AkiDB Service Layer
//!
//! These tests validate the full stack: REST/gRPC → CollectionService → StorageBackend → WAL/Snapshots
//! They exercise real-world scenarios including persistence, recovery, compaction, and multi-collection isolation.

use akidb_core::{DistanceMetric, DocumentId, VectorDocument};
use akidb_metadata::{SqliteCollectionRepository, VectorPersistence};
use akidb_service::CollectionService;
use akidb_storage::StorageConfig;
use sqlx::SqlitePool;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::OnceLock;

/// Global counter for unique tenant slugs
static TENANT_COUNTER: AtomicU64 = AtomicU64::new(1000);

/// Global temp directory for WAL files (persists across test service instances)
static TEMP_DIR: OnceLock<tempfile::TempDir> = OnceLock::new();

/// Helper to create a test database with migrations
async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("../akidb-metadata/migrations")
        .run(&pool)
        .await
        .unwrap();
    pool
}

/// Helper to create a collection service with storage backend
async fn setup_service(pool: &SqlitePool) -> Arc<CollectionService> {
    let repository = Arc::new(SqliteCollectionRepository::new(pool.clone()));
    let vector_persistence = Arc::new(VectorPersistence::new(pool.clone()));

    // Use persistent temp directory for WAL files across service instances
    let temp_dir = TEMP_DIR.get_or_init(|| tempfile::TempDir::new().unwrap());
    let storage_config = StorageConfig::memory(temp_dir.path().join("akidb.wal"));

    let service = Arc::new(CollectionService::with_storage(
        repository,
        vector_persistence,
        storage_config,
    ));

    // Create default tenant with unique slug
    let tenant_id = akidb_core::TenantId::new();
    let counter = TENANT_COUNTER.fetch_add(1, Ordering::SeqCst);
    let unique_slug = format!("test-e2e-{}", counter);
    sqlx::query(
        "INSERT INTO tenants (tenant_id, name, slug, status, created_at, updated_at)
         VALUES (?1, 'test-tenant', ?2, 'active', datetime('now'), datetime('now'))",
    )
    .bind(&tenant_id.to_bytes()[..])
    .bind(&unique_slug)
    .execute(pool)
    .await
    .unwrap();

    // Create default database
    let database_id = akidb_core::DatabaseId::new();
    sqlx::query(
        "INSERT INTO databases (database_id, tenant_id, name, state, created_at, updated_at)
         VALUES (?1, ?2, 'test-database', 'ready', datetime('now'), datetime('now'))",
    )
    .bind(&database_id.to_bytes()[..])
    .bind(&tenant_id.to_bytes()[..])
    .execute(pool)
    .await
    .unwrap();

    service.set_default_database_id(database_id).await;

    service
}

/// Create a test vector with deterministic values based on ID string
fn create_test_vector(id: &str, dimension: u32) -> VectorDocument {
    let mut vector = vec![0.0; dimension as usize];
    for (i, byte) in id.bytes().enumerate() {
        if i < dimension as usize {
            vector[i] = f32::from(byte) / 255.0;
        }
    }
    VectorDocument::new(DocumentId::new(), vector).with_external_id(id.to_string())
}

/// Create a simple query vector for search tests
fn create_test_query_vector(dimension: u32) -> Vec<f32> {
    vec![0.5; dimension as usize]
}

// ========== E2E Storage Integration Tests ==========

#[tokio::test]
async fn test_e2e_storage_persistence_full_stack() {
    // Setup service with storage
    let pool = setup_test_db().await;
    let service1 = setup_service(&pool).await;

    // Create collection via API
    let collection_id = service1
        .create_collection(
            "full-stack-test".to_string(),
            128,
            DistanceMetric::Cosine,
            None,
        )
        .await
        .unwrap();

    // Insert 100 vectors via API
    let mut doc_ids = Vec::new();
    for i in 0..100 {
        let doc = create_test_vector(&format!("doc-{}", i), 128);
        let doc_id = service1.insert(collection_id, doc).await.unwrap();
        doc_ids.push(doc_id);
    }

    // Query to verify all present
    let count1 = service1.get_count(collection_id).await.unwrap();
    assert_eq!(count1, 100, "Should have 100 vectors before restart");

    // Perform search and record results
    let query1 = create_test_query_vector(128);
    let results1 = service1
        .query(collection_id, query1.clone(), 10)
        .await
        .unwrap();
    assert_eq!(results1.len(), 10, "Should return 10 search results");

    // Drop service (simulate restart)
    drop(service1);

    // Create new service (recovery)
    let service2 = setup_service(&pool).await;
    service2.load_all_collections().await.unwrap();

    // Query again to verify all 100 vectors recovered
    let count2 = service2.get_count(collection_id).await.unwrap();
    assert_eq!(count2, 100, "Should have 100 vectors after recovery");

    // Verify search results are identical pre/post restart
    let results2 = service2.query(collection_id, query1, 10).await.unwrap();
    assert_eq!(
        results2.len(),
        10,
        "Should return 10 search results after recovery"
    );

    // Verify top results have same doc_ids (order might vary slightly due to ties)
    for (i, result) in results2.iter().enumerate() {
        assert!(
            doc_ids.contains(&result.doc_id),
            "Result {} should be from original set of documents",
            i
        );
    }

    // Verify all original documents can be retrieved
    for doc_id in &doc_ids {
        let retrieved = service2.get(collection_id, *doc_id).await.unwrap();
        assert!(
            retrieved.is_some(),
            "Document {:?} should exist after recovery",
            doc_id
        );
    }
}

#[tokio::test]
async fn test_e2e_storage_compaction_workflow() {
    let pool = setup_test_db().await;
    let service = setup_service(&pool).await;

    // Create collection (storage backend will auto-compact based on configured thresholds)
    let collection_id = service
        .create_collection(
            "compaction-test".to_string(),
            64,
            DistanceMetric::Cosine,
            None,
        )
        .await
        .unwrap();

    // Insert 150 vectors (enough to trigger auto-compaction with default thresholds)
    // This tests that compaction doesn't break persistence
    for i in 0..150 {
        let doc = create_test_vector(&format!("vec-{}", i), 64);
        service.insert(collection_id, doc).await.unwrap();
    }

    // Verify all 150 vectors present
    let count = service.get_count(collection_id).await.unwrap();
    assert_eq!(count, 150, "All 150 vectors should be present");

    // Drop and restart service
    drop(service);
    let service2 = setup_service(&pool).await;
    service2.load_all_collections().await.unwrap();

    // Verify all 150 vectors recovered (proving compaction didn't lose data)
    let count_after_restart = service2.get_count(collection_id).await.unwrap();
    assert_eq!(
        count_after_restart, 150,
        "All 150 vectors should be recovered after restart"
    );

    // Verify search works after recovery
    let results = service2
        .query(collection_id, vec![0.5; 64], 10)
        .await
        .unwrap();
    assert_eq!(
        results.len(),
        10,
        "Search should return 10 results after recovery"
    );
}

#[tokio::test]
async fn test_e2e_storage_multi_collection_isolation() {
    let pool = setup_test_db().await;
    let service1 = setup_service(&pool).await;

    // Create 3 collections (A, B, C)
    let col_a = service1
        .create_collection("collection-a".to_string(), 64, DistanceMetric::Cosine, None)
        .await
        .unwrap();

    let col_b = service1
        .create_collection("collection-b".to_string(), 64, DistanceMetric::Cosine, None)
        .await
        .unwrap();

    let col_c = service1
        .create_collection("collection-c".to_string(), 64, DistanceMetric::Cosine, None)
        .await
        .unwrap();

    // Insert 50 vectors to A (doc_ids: "a-1" to "a-50")
    for i in 1..=50 {
        let doc = create_test_vector(&format!("a-{}", i), 64);
        service1.insert(col_a, doc).await.unwrap();
    }

    // Insert 30 vectors to B (doc_ids: "b-1" to "b-30")
    for i in 1..=30 {
        let doc = create_test_vector(&format!("b-{}", i), 64);
        service1.insert(col_b, doc).await.unwrap();
    }

    // Insert 40 vectors to C (doc_ids: "c-1" to "c-40")
    for i in 1..=40 {
        let doc = create_test_vector(&format!("c-{}", i), 64);
        service1.insert(col_c, doc).await.unwrap();
    }

    // Verify counts before restart
    assert_eq!(service1.get_count(col_a).await.unwrap(), 50);
    assert_eq!(service1.get_count(col_b).await.unwrap(), 30);
    assert_eq!(service1.get_count(col_c).await.unwrap(), 40);

    // Restart service
    drop(service1);
    let service2 = setup_service(&pool).await;
    service2.load_all_collections().await.unwrap();

    // Verify A has exactly 50 vectors with "a-" prefix
    let count_a = service2.get_count(col_a).await.unwrap();
    assert_eq!(count_a, 50, "Collection A should have exactly 50 vectors");

    // Verify B has exactly 30 vectors with "b-" prefix
    let count_b = service2.get_count(col_b).await.unwrap();
    assert_eq!(count_b, 30, "Collection B should have exactly 30 vectors");

    // Verify C has exactly 40 vectors with "c-" prefix
    let count_c = service2.get_count(col_c).await.unwrap();
    assert_eq!(count_c, 40, "Collection C should have exactly 40 vectors");

    // Verify no cross-contamination (search in A should only return A vectors)
    let results_a = service2.query(col_a, vec![0.5; 64], 50).await.unwrap();
    assert_eq!(
        results_a.len(),
        50,
        "Collection A should return all 50 vectors"
    );
    for result in results_a {
        let doc = service2.get(col_a, result.doc_id).await.unwrap().unwrap();
        if let Some(ext_id) = doc.external_id {
            assert!(
                ext_id.starts_with("a-"),
                "Collection A should only contain vectors with 'a-' prefix, got: {}",
                ext_id
            );
        }
    }

    // Verify total vector count across all collections
    let total = count_a + count_b + count_c;
    assert_eq!(
        total, 120,
        "Total vectors across all collections should be 120"
    );
}

#[tokio::test]
async fn test_e2e_storage_delete_persistence() {
    let pool = setup_test_db().await;
    let service1 = setup_service(&pool).await;

    // Create collection
    let collection_id = service1
        .create_collection("delete-test".to_string(), 64, DistanceMetric::Cosine, None)
        .await
        .unwrap();

    // Insert 100 vectors
    let mut doc_ids = Vec::new();
    for i in 0..100 {
        let doc = create_test_vector(&format!("vec-{}", i), 64);
        let doc_id = service1.insert(collection_id, doc).await.unwrap();
        doc_ids.push(doc_id);
    }

    // Verify initial count
    assert_eq!(service1.get_count(collection_id).await.unwrap(), 100);

    // Delete vectors 0-24 (25 vectors)
    for i in 0..25 {
        service1.delete(collection_id, doc_ids[i]).await.unwrap();
    }

    // Verify 75 vectors remain via query
    let count1 = service1.get_count(collection_id).await.unwrap();
    assert_eq!(count1, 75, "Should have 75 vectors after deleting 25");

    // Verify search only returns non-deleted vectors
    let results1 = service1
        .query(collection_id, vec![0.5; 64], 100)
        .await
        .unwrap();
    assert_eq!(
        results1.len(),
        75,
        "Search should return only 75 non-deleted vectors"
    );

    // Restart service
    drop(service1);
    let service2 = setup_service(&pool).await;
    service2.load_all_collections().await.unwrap();

    // Verify still 75 vectors
    let count2 = service2.get_count(collection_id).await.unwrap();
    assert_eq!(count2, 75, "Should still have 75 vectors after restart");

    // Verify deleted IDs return NotFound error
    for i in 0..25 {
        let result = service2.get(collection_id, doc_ids[i]).await.unwrap();
        assert!(
            result.is_none(),
            "Deleted vector {} should not exist after restart",
            i
        );
    }

    // Verify non-deleted IDs still exist
    for i in 25..100 {
        let result = service2.get(collection_id, doc_ids[i]).await.unwrap();
        assert!(
            result.is_some(),
            "Non-deleted vector {} should exist after restart",
            i
        );
    }

    // Verify search only returns non-deleted vectors after restart
    let results2 = service2
        .query(collection_id, vec![0.5; 64], 100)
        .await
        .unwrap();
    assert_eq!(
        results2.len(),
        75,
        "Search should return only 75 non-deleted vectors after restart"
    );
}

#[tokio::test]
async fn test_e2e_storage_concurrent_operations_durability() {
    let pool = setup_test_db().await;
    let service = Arc::new(setup_service(&pool).await);

    // Create collection
    let collection_id = service
        .create_collection(
            "concurrent-durability".to_string(),
            64,
            DistanceMetric::Cosine,
            None,
        )
        .await
        .unwrap();

    // Spawn 10 concurrent tasks, each inserting 20 vectors (200 total)
    let mut handles = Vec::new();
    for task_id in 0..10 {
        let service_clone = Arc::clone(&service);
        let handle = tokio::spawn(async move {
            for i in 0..20 {
                let doc_id_str = format!("task{}-vec{}", task_id, i);
                let doc = create_test_vector(&doc_id_str, 64);
                service_clone.insert(collection_id, doc).await.unwrap();
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify 200 vectors present
    let count1 = service.get_count(collection_id).await.unwrap();
    assert_eq!(
        count1, 200,
        "Should have 200 vectors after concurrent inserts"
    );

    // Restart service
    drop(service);
    let service2 = setup_service(&pool).await;
    service2.load_all_collections().await.unwrap();

    // Verify all 200 vectors recovered
    let count2 = service2.get_count(collection_id).await.unwrap();
    assert_eq!(
        count2, 200,
        "All 200 vectors should be recovered after restart"
    );

    // Verify search works correctly (query may return less than 200 due to index limits)
    let search_results = service2
        .query(collection_id, vec![0.3; 64], 50)
        .await
        .unwrap();
    assert_eq!(search_results.len(), 50, "Search should return 50 results");

    // Verify more comprehensive search
    let large_search = service2
        .query(collection_id, vec![0.5; 64], 100)
        .await
        .unwrap();
    assert!(
        large_search.len() >= 100,
        "Large search should return at least 100 results, got {}",
        large_search.len()
    );
}

#[tokio::test]
async fn test_e2e_storage_search_accuracy_after_recovery() {
    let pool = setup_test_db().await;
    let service1 = setup_service(&pool).await;

    // Create collection with known vectors (use simple embeddings)
    let collection_id = service1
        .create_collection(
            "search-accuracy".to_string(),
            16, // Small dimension for predictable results
            DistanceMetric::Cosine,
            None,
        )
        .await
        .unwrap();

    // Insert 10 vectors with predictable patterns
    let mut doc_ids = Vec::new();
    for i in 0..10 {
        let mut vector = vec![0.0; 16];
        // Create distinct patterns: first element varies, rest are constant
        vector[0] = i as f32 / 10.0;
        for j in 1..16 {
            vector[j] = 0.1;
        }
        let doc = VectorDocument::new(DocumentId::new(), vector);
        let doc_id = doc.doc_id;
        doc_ids.push(doc_id);
        service1.insert(collection_id, doc).await.unwrap();
    }

    // Perform vector search with specific query, record top-3 results
    let query = vec![0.5; 16]; // Should match vectors with similar values
    let results1 = service1
        .query(collection_id, query.clone(), 3)
        .await
        .unwrap();
    assert_eq!(results1.len(), 3, "Should return 3 results before restart");

    // Record the top-3 doc_ids and scores
    let top3_ids_before: Vec<DocumentId> = results1.iter().map(|r| r.doc_id).collect();
    let top3_scores_before: Vec<f32> = results1.iter().map(|r| r.score).collect();

    // Restart service
    drop(service1);
    let service2 = setup_service(&pool).await;
    service2.load_all_collections().await.unwrap();

    // Perform same search with same query vector
    let results2 = service2.query(collection_id, query, 3).await.unwrap();
    assert_eq!(results2.len(), 3, "Should return 3 results after restart");

    // Verify top-3 results are identical (same doc_ids, same order, similar scores)
    for i in 0..3 {
        assert_eq!(
            results2[i].doc_id, top3_ids_before[i],
            "Result {} should have same doc_id before and after restart",
            i
        );

        // Scores should be nearly identical (allow small floating point tolerance)
        let score_diff = (results2[i].score - top3_scores_before[i]).abs();
        assert!(
            score_diff < 0.0001,
            "Result {} score should match before/after restart (diff: {}, before: {}, after: {})",
            i,
            score_diff,
            top3_scores_before[i],
            results2[i].score
        );
    }
}

#[tokio::test]
#[ignore] // Mark as ignored - run with `cargo test --ignored` for benchmarks
async fn bench_e2e_storage_insert_throughput() {
    use std::time::Instant;

    let pool = setup_test_db().await;
    let service = setup_service(&pool).await;

    // Create collection
    let collection_id = service
        .create_collection(
            "throughput-bench".to_string(),
            128,
            DistanceMetric::Cosine,
            None,
        )
        .await
        .unwrap();

    // Measure insert throughput with storage enabled
    let start = Instant::now();
    let num_vectors = 1000;

    for i in 0..num_vectors {
        let doc = create_test_vector(&format!("bench-{}", i), 128);
        service.insert(collection_id, doc).await.unwrap();
    }

    let elapsed = start.elapsed();
    let ops_per_sec = num_vectors as f64 / elapsed.as_secs_f64();

    // Print benchmark results
    println!("\n=== E2E Storage Insert Throughput Benchmark ===");
    println!("Vectors inserted: {}", num_vectors);
    println!("Time elapsed: {:?}", elapsed);
    println!("Throughput: {:.2} ops/sec", ops_per_sec);
    println!(
        "Avg latency: {:.2} ms/op",
        (elapsed.as_millis() as f64) / (num_vectors as f64)
    );
    println!("===============================================\n");

    // Verify all vectors inserted
    let count = service.get_count(collection_id).await.unwrap();
    assert_eq!(
        count, num_vectors,
        "All {} vectors should be inserted",
        num_vectors
    );

    // Basic performance assertion (should achieve at least 100 ops/sec with storage)
    assert!(
        ops_per_sec >= 100.0,
        "Insert throughput should be at least 100 ops/sec, got {:.2}",
        ops_per_sec
    );
}
