//! End-to-End Integration Tests for AkiDB Service Layer
//!
//! These tests validate complete workflows from collection creation through
//! vector operations, persistence, and cleanup.

use akidb_core::{CollectionId, DistanceMetric, DocumentId, VectorDocument};
use akidb_metadata::{SqliteCollectionRepository, VectorPersistence};
use akidb_service::CollectionService;
use akidb_storage::StorageConfig;
use sqlx::SqlitePool;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::OnceLock;

/// Global counter for unique tenant slugs
static TENANT_COUNTER: AtomicU64 = AtomicU64::new(0);

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

/// Helper to create a collection service with full persistence
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
    let unique_slug = format!("test-{}", counter);
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

#[tokio::test]
async fn test_e2e_full_workflow() {
    // Setup
    let pool = setup_test_db().await;
    let service = setup_service(&pool).await;

    // Create collection
    let collection_id = service
        .create_collection(
            "test-collection".to_string(),
            128,
            DistanceMetric::Cosine,
            None,
        )
        .await
        .unwrap();

    // Insert vectors
    let mut doc_ids = Vec::new();
    for i in 0..10 {
        let vector = vec![0.1 * (i + 1) as f32; 128];
        let doc = VectorDocument::new(DocumentId::new(), vector);
        let doc_id = service.insert(collection_id, doc).await.unwrap();
        doc_ids.push(doc_id);
    }

    // Verify count
    let count = service.get_count(collection_id).await.unwrap();
    assert_eq!(count, 10);

    // Search vectors
    let query = vec![0.5; 128];
    let results = service.query(collection_id, query, 5).await.unwrap();
    assert_eq!(results.len(), 5);

    // Get specific vector
    let retrieved = service.get(collection_id, doc_ids[0]).await.unwrap();
    assert!(retrieved.is_some());

    // Delete vector
    service.delete(collection_id, doc_ids[0]).await.unwrap();
    let count = service.get_count(collection_id).await.unwrap();
    assert_eq!(count, 9);

    // Delete collection
    service.delete_collection(collection_id).await.unwrap();
    let collections = service.list_collections().await.unwrap();
    assert_eq!(collections.len(), 0);
}

#[tokio::test]
async fn test_e2e_persistence_across_restart() {
    // Create first service instance
    let pool = setup_test_db().await;
    let service1 = setup_service(&pool).await;

    // Create collection and insert vectors
    let collection_id = service1
        .create_collection(
            "persistent-collection".to_string(),
            64,
            DistanceMetric::Cosine,
            None,
        )
        .await
        .unwrap();

    let mut doc_ids = Vec::new();
    for i in 0..5 {
        let vector = vec![0.1 * (i + 1) as f32; 64];
        let doc = VectorDocument::new(DocumentId::new(), vector);
        let doc_id = service1.insert(collection_id, doc).await.unwrap();
        doc_ids.push(doc_id);
    }

    // Verify initial count
    let count1 = service1.get_count(collection_id).await.unwrap();
    assert_eq!(count1, 5);

    // Simulate restart: create new service instance with same pool
    let service2 = setup_service(&pool).await;
    service2.load_all_collections().await.unwrap();

    // Verify collections persisted
    let collections = service2.list_collections().await.unwrap();
    assert_eq!(collections.len(), 1);
    assert_eq!(collections[0].name, "persistent-collection");

    // Verify vectors persisted
    let count2 = service2.get_count(collection_id).await.unwrap();
    assert_eq!(count2, 5);

    // Verify can retrieve specific vector
    let retrieved = service2.get(collection_id, doc_ids[0]).await.unwrap();
    assert!(retrieved.is_some());

    // Verify can search
    let query = vec![0.3; 64];
    let results = service2.query(collection_id, query, 3).await.unwrap();
    assert_eq!(results.len(), 3);
}

#[tokio::test]
async fn test_e2e_multiple_collections() {
    let pool = setup_test_db().await;
    let service = setup_service(&pool).await;

    // Create multiple collections with different configs
    let col1 = service
        .create_collection("col1".to_string(), 64, DistanceMetric::Cosine, None)
        .await
        .unwrap();

    let col2 = service
        .create_collection("col2".to_string(), 128, DistanceMetric::L2, None)
        .await
        .unwrap();

    let col3 = service
        .create_collection("col3".to_string(), 256, DistanceMetric::L2, None)
        .await
        .unwrap();

    // Insert different vectors to each
    for i in 1..=5 {
        let vec1 = vec![0.1 * i as f32; 64];
        service
            .insert(col1, VectorDocument::new(DocumentId::new(), vec1))
            .await
            .unwrap();

        let vec2 = vec![0.2 * i as f32; 128];
        service
            .insert(col2, VectorDocument::new(DocumentId::new(), vec2))
            .await
            .unwrap();

        let vec3 = vec![0.3 * i as f32; 256];
        service
            .insert(col3, VectorDocument::new(DocumentId::new(), vec3))
            .await
            .unwrap();
    }

    // Verify counts
    assert_eq!(service.get_count(col1).await.unwrap(), 5);
    assert_eq!(service.get_count(col2).await.unwrap(), 5);
    assert_eq!(service.get_count(col3).await.unwrap(), 5);

    // Verify list collections
    let collections = service.list_collections().await.unwrap();
    assert_eq!(collections.len(), 3);

    // Verify search works on each
    let results1 = service.query(col1, vec![0.1; 64], 3).await.unwrap();
    assert_eq!(results1.len(), 3);

    let results2 = service.query(col2, vec![0.2; 128], 3).await.unwrap();
    assert_eq!(results2.len(), 3);

    let results3 = service.query(col3, vec![0.3; 256], 3).await.unwrap();
    assert_eq!(results3.len(), 3);
}

#[tokio::test]
async fn test_e2e_error_invalid_collection_id() {
    let pool = setup_test_db().await;
    let service = setup_service(&pool).await;

    let fake_id = CollectionId::new();

    // Query non-existent collection
    let result = service.query(fake_id, vec![0.1; 128], 10).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));

    // Insert to non-existent collection
    let doc = VectorDocument::new(DocumentId::new(), vec![0.1; 128]);
    let result = service.insert(fake_id, doc).await;
    assert!(result.is_err());

    // Get from non-existent collection
    let result = service.get(fake_id, DocumentId::new()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_e2e_error_invalid_dimension() {
    let pool = setup_test_db().await;
    let service = setup_service(&pool).await;

    // Too small
    let result = service
        .create_collection("invalid".to_string(), 15, DistanceMetric::Cosine, None)
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("dimension"));

    // Too large
    let result = service
        .create_collection("invalid".to_string(), 5000, DistanceMetric::Cosine, None)
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("dimension"));
}

#[tokio::test]
async fn test_e2e_concurrent_inserts() {
    let pool = setup_test_db().await;
    let service = setup_service(&pool).await;

    let collection_id = service
        .create_collection("concurrent".to_string(), 64, DistanceMetric::Cosine, None)
        .await
        .unwrap();

    // Spawn 10 concurrent insert tasks
    let mut handles = vec![];
    for i in 0..10 {
        let service = Arc::clone(&service);
        let handle = tokio::spawn(async move {
            for j in 0..10 {
                let vector = vec![0.1 * (i * 10 + j + 1) as f32; 64];
                let doc = VectorDocument::new(DocumentId::new(), vector);
                service.insert(collection_id, doc).await.unwrap();
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all vectors inserted
    let count = service.get_count(collection_id).await.unwrap();
    assert_eq!(count, 100);
}

#[tokio::test]
async fn test_e2e_concurrent_searches() {
    let pool = setup_test_db().await;
    let service = setup_service(&pool).await;

    let collection_id = service
        .create_collection("search-test".to_string(), 64, DistanceMetric::Cosine, None)
        .await
        .unwrap();

    // Insert 50 vectors
    for i in 0..50 {
        let vector = vec![0.1 * (i + 1) as f32; 64];
        let doc = VectorDocument::new(DocumentId::new(), vector);
        service.insert(collection_id, doc).await.unwrap();
    }

    // Spawn 20 concurrent search tasks
    let mut handles = vec![];
    for i in 0..20 {
        let service = Arc::clone(&service);
        let handle = tokio::spawn(async move {
            let query = vec![0.1 * (i + 1) as f32; 64];
            service.query(collection_id, query, 10).await.unwrap()
        });
        handles.push(handle);
    }

    // Wait for all searches and verify results
    for handle in handles {
        let results = handle.await.unwrap();
        assert_eq!(results.len(), 10);
    }
}

#[tokio::test]
async fn test_e2e_delete_and_recreate_collection() {
    let pool = setup_test_db().await;
    let service = setup_service(&pool).await;

    // Create collection
    let collection_id = service
        .create_collection("temp".to_string(), 64, DistanceMetric::Cosine, None)
        .await
        .unwrap();

    // Insert vectors
    for i in 1..=10 {
        let vector = vec![0.1 * i as f32; 64];
        service
            .insert(
                collection_id,
                VectorDocument::new(DocumentId::new(), vector),
            )
            .await
            .unwrap();
    }

    // Delete collection
    service.delete_collection(collection_id).await.unwrap();

    // Create new collection with same name
    let new_id = service
        .create_collection("temp".to_string(), 64, DistanceMetric::Cosine, None)
        .await
        .unwrap();

    // Verify it's empty
    let count = service.get_count(new_id).await.unwrap();
    assert_eq!(count, 0);

    // Verify different ID
    assert_ne!(collection_id, new_id);
}

#[tokio::test]
async fn test_e2e_empty_collection_operations() {
    let pool = setup_test_db().await;
    let service = setup_service(&pool).await;

    let collection_id = service
        .create_collection("empty".to_string(), 64, DistanceMetric::Cosine, None)
        .await
        .unwrap();

    // Count should be 0
    let count = service.get_count(collection_id).await.unwrap();
    assert_eq!(count, 0);

    // Search should return empty
    let results = service
        .query(collection_id, vec![0.1; 64], 10)
        .await
        .unwrap();
    assert_eq!(results.len(), 0);

    // Get non-existent doc should return None
    let result = service.get(collection_id, DocumentId::new()).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_e2e_large_batch_insert() {
    let pool = setup_test_db().await;
    let service = setup_service(&pool).await;

    let collection_id = service
        .create_collection("large".to_string(), 128, DistanceMetric::Cosine, None)
        .await
        .unwrap();

    // Insert 1000 vectors
    for i in 1..=1000 {
        let vector = vec![0.001 * i as f32; 128];
        let doc = VectorDocument::new(DocumentId::new(), vector);
        service.insert(collection_id, doc).await.unwrap();
    }

    // Verify count
    let count = service.get_count(collection_id).await.unwrap();
    assert_eq!(count, 1000);

    // Verify search works
    let results = service
        .query(collection_id, vec![0.5; 128], 10)
        .await
        .unwrap();
    assert_eq!(results.len(), 10);
}

// TODO: Implement ServiceMetrics counter tracking in CollectionService
// Currently metrics() returns hardcoded zeros - need to add AtomicU64 counters
// and increment them in create_collection(), insert(), query(), delete_collection()
#[tokio::test]
#[ignore = "ServiceMetrics counter tracking not yet implemented"]
async fn test_e2e_metrics_collection() {
    let pool = setup_test_db().await;
    let service = setup_service(&pool).await;

    // Get metrics reference
    let metrics = service.metrics().expect("Metrics should be enabled");

    // Initial state
    let initial_collections = metrics.collections_created();
    let initial_vectors = metrics.vectors_inserted();
    let initial_searches = metrics.searches_performed();

    // Create collection
    let collection_id = service
        .create_collection("metrics-test".to_string(), 64, DistanceMetric::Cosine, None)
        .await
        .unwrap();

    // Verify collection metric incremented
    assert_eq!(metrics.collections_created(), initial_collections + 1);

    // Insert 10 vectors
    for i in 1..=10 {
        let vector = vec![0.1 * i as f32; 64];
        service
            .insert(
                collection_id,
                VectorDocument::new(DocumentId::new(), vector),
            )
            .await
            .unwrap();
    }

    // Verify insert metrics
    assert_eq!(metrics.vectors_inserted(), initial_vectors + 10);

    // Perform 5 searches
    for i in 1..=5 {
        let query = vec![0.1 * i as f32; 64];
        service.query(collection_id, query, 5).await.unwrap();
    }

    // Verify search metrics
    assert_eq!(metrics.searches_performed(), initial_searches + 5);

    // Delete collection
    service.delete_collection(collection_id).await.unwrap();

    // Verify deletion metric
    assert_eq!(metrics.collections_deleted(), 1);
}

#[tokio::test]
async fn test_e2e_vector_retrieval_accuracy() {
    let pool = setup_test_db().await;
    let service = setup_service(&pool).await;

    let collection_id = service
        .create_collection("accuracy".to_string(), 128, DistanceMetric::Cosine, None)
        .await
        .unwrap();

    // Insert vectors with known values
    let mut expected_docs = Vec::new();
    for i in 1..=10 {
        let vector = vec![i as f32 / 10.0; 128];
        let doc = VectorDocument::new(DocumentId::new(), vector.clone());
        let doc_id = doc.doc_id;
        service.insert(collection_id, doc).await.unwrap();
        expected_docs.push((doc_id, vector));
    }

    // Retrieve each vector and verify accuracy
    for (doc_id, expected_vector) in expected_docs {
        let retrieved = service
            .get(collection_id, doc_id)
            .await
            .unwrap()
            .expect("Vector should exist");

        assert_eq!(retrieved.doc_id, doc_id);
        assert_eq!(retrieved.vector.len(), 128);

        // Verify vector values match
        for (i, &val) in retrieved.vector.iter().enumerate() {
            assert!((val - expected_vector[i]).abs() < 0.0001);
        }
    }
}

#[tokio::test]
async fn test_e2e_search_result_ordering() {
    let pool = setup_test_db().await;
    let service = setup_service(&pool).await;

    let collection_id = service
        .create_collection("ordering".to_string(), 16, DistanceMetric::Cosine, None)
        .await
        .unwrap();

    // Insert vectors at known positions (pad to 16 dimensions)
    let mut vec1 = vec![1.0, 0.0, 0.0];
    vec1.resize(16, 0.0);
    let mut vec2 = vec![0.8, 0.2, 0.0];
    vec2.resize(16, 0.0);
    let mut vec3 = vec![0.6, 0.4, 0.0];
    vec3.resize(16, 0.0);
    let mut vec4 = vec![0.0, 1.0, 0.0];
    vec4.resize(16, 0.0);

    let vectors = vec![
        vec1, // Closest to [1,0,0,...]
        vec2, vec3, vec4, // Furthest from [1,0,0,...]
    ];

    for vector in vectors {
        let doc = VectorDocument::new(DocumentId::new(), vector);
        service.insert(collection_id, doc).await.unwrap();
    }

    // Search with [1,0,0,...] query
    let mut query = vec![1.0, 0.0, 0.0];
    query.resize(16, 0.0);
    let results = service.query(collection_id, query, 4).await.unwrap();

    // Verify results are ordered by score (higher score = closer for Cosine)
    assert_eq!(results.len(), 4);
    for i in 0..results.len() - 1 {
        assert!(results[i].score >= results[i + 1].score);
    }
}

#[tokio::test]
async fn test_e2e_mixed_operations() {
    let pool = setup_test_db().await;
    let service = setup_service(&pool).await;

    let collection_id = service
        .create_collection("mixed".to_string(), 64, DistanceMetric::Cosine, None)
        .await
        .unwrap();

    // Mix of operations
    let mut doc_ids = Vec::new();

    // Insert 5 vectors
    for i in 1..=5 {
        let vector = vec![0.1 * i as f32; 64];
        let doc_id = service
            .insert(
                collection_id,
                VectorDocument::new(DocumentId::new(), vector),
            )
            .await
            .unwrap();
        doc_ids.push(doc_id);
    }

    // Search
    service
        .query(collection_id, vec![0.2; 64], 3)
        .await
        .unwrap();

    // Delete 2 vectors
    service.delete(collection_id, doc_ids[0]).await.unwrap();
    service.delete(collection_id, doc_ids[1]).await.unwrap();

    // Insert 3 more
    for i in 6..=8 {
        let vector = vec![0.1 * i as f32; 64];
        service
            .insert(
                collection_id,
                VectorDocument::new(DocumentId::new(), vector),
            )
            .await
            .unwrap();
    }

    // Verify final count
    let count = service.get_count(collection_id).await.unwrap();
    assert_eq!(count, 6); // 5 - 2 + 3

    // Search again
    let results = service
        .query(collection_id, vec![0.5; 64], 5)
        .await
        .unwrap();
    assert_eq!(results.len(), 5);
}

#[tokio::test]
async fn test_e2e_collection_get_by_id() {
    let pool = setup_test_db().await;
    let service = setup_service(&pool).await;

    // Create collection
    let collection_id = service
        .create_collection(
            "get-test".to_string(),
            256,
            DistanceMetric::L2,
            Some("test-model".to_string()),
        )
        .await
        .unwrap();

    // Get collection by ID
    let collection = service.get_collection(collection_id).await.unwrap();

    assert_eq!(collection.collection_id, collection_id);
    assert_eq!(collection.name, "get-test");
    assert_eq!(collection.dimension, 256);
    assert_eq!(collection.metric, DistanceMetric::L2);
    assert_eq!(collection.embedding_model, "test-model");
}

#[tokio::test]
async fn test_e2e_delete_nonexistent_vector() {
    let pool = setup_test_db().await;
    let service = setup_service(&pool).await;

    let collection_id = service
        .create_collection("delete-test".to_string(), 64, DistanceMetric::Cosine, None)
        .await
        .unwrap();

    // Try to delete non-existent vector
    let fake_doc_id = DocumentId::new();
    let result = service.delete(collection_id, fake_doc_id).await;

    // Should fail with NotFound error (implementation returns error for non-existent document)
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));

    // Count should still be 0
    let count = service.get_count(collection_id).await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_e2e_persistence_after_many_operations() {
    let pool = setup_test_db().await;
    let service1 = setup_service(&pool).await;

    let collection_id = service1
        .create_collection("complex".to_string(), 64, DistanceMetric::Cosine, None)
        .await
        .unwrap();

    // Perform many operations
    let mut doc_ids = Vec::new();
    for i in 1..=20 {
        let vector = vec![0.05 * i as f32; 64];
        let doc_id = service1
            .insert(
                collection_id,
                VectorDocument::new(DocumentId::new(), vector),
            )
            .await
            .unwrap();
        doc_ids.push(doc_id);
    }

    // Delete some
    for i in (0..10).step_by(2) {
        service1.delete(collection_id, doc_ids[i]).await.unwrap();
    }

    // Final count should be 15 (20 - 5)
    let count1 = service1.get_count(collection_id).await.unwrap();
    assert_eq!(count1, 15);

    // Simulate restart
    let service2 = setup_service(&pool).await;
    service2.load_all_collections().await.unwrap();

    // Verify count persisted
    let count2 = service2.get_count(collection_id).await.unwrap();
    assert_eq!(count2, 15);

    // Verify deleted vectors are gone
    for i in (0..10).step_by(2) {
        let result = service2.get(collection_id, doc_ids[i]).await.unwrap();
        assert!(result.is_none());
    }

    // Verify remaining vectors exist
    for i in (1..10).step_by(2) {
        let result = service2.get(collection_id, doc_ids[i]).await.unwrap();
        assert!(result.is_some());
    }
}

// ========== Phase 6 Week 5 Day 4: Crash Recovery Tests ==========

#[tokio::test]
async fn test_crash_recovery_multiple_collections() {
    // Setup first service instance
    let pool = setup_test_db().await;
    let service1 = setup_service(&pool).await;

    // Create 2 collections
    let col1 = service1
        .create_collection(
            "recovery-col1".to_string(),
            64,
            DistanceMetric::Cosine,
            None,
        )
        .await
        .unwrap();

    let col2 = service1
        .create_collection("recovery-col2".to_string(), 128, DistanceMetric::L2, None)
        .await
        .unwrap();

    // Insert 50 vectors into each collection (100 total)
    let mut doc_ids_col1 = Vec::new();
    let mut doc_ids_col2 = Vec::new();

    for i in 0..50 {
        let vector1 = vec![0.01 * (i + 1) as f32; 64];
        let doc1 = VectorDocument::new(DocumentId::new(), vector1);
        let doc_id1 = service1.insert(col1, doc1).await.unwrap();
        doc_ids_col1.push(doc_id1);

        let vector2 = vec![0.02 * (i + 1) as f32; 128];
        let doc2 = VectorDocument::new(DocumentId::new(), vector2);
        let doc_id2 = service1.insert(col2, doc2).await.unwrap();
        doc_ids_col2.push(doc_id2);
    }

    // Verify initial counts
    assert_eq!(service1.get_count(col1).await.unwrap(), 50);
    assert_eq!(service1.get_count(col2).await.unwrap(), 50);

    // Drop service (simulate crash)
    drop(service1);

    // Create new service with same repository (simulate restart)
    let service2 = setup_service(&pool).await;
    service2.load_all_collections().await.unwrap();

    // Verify collections loaded
    let collections = service2.list_collections().await.unwrap();
    assert_eq!(collections.len(), 2);

    // Verify all 100 vectors recovered correctly
    assert_eq!(service2.get_count(col1).await.unwrap(), 50);
    assert_eq!(service2.get_count(col2).await.unwrap(), 50);

    // Verify vectors searchable in both collections
    let results1 = service2.query(col1, vec![0.25; 64], 10).await.unwrap();
    assert_eq!(results1.len(), 10);

    let results2 = service2.query(col2, vec![0.5; 128], 10).await.unwrap();
    assert_eq!(results2.len(), 10);

    // Verify specific vectors can be retrieved
    let retrieved1 = service2.get(col1, doc_ids_col1[0]).await.unwrap();
    assert!(retrieved1.is_some());

    let retrieved2 = service2.get(col2, doc_ids_col2[25]).await.unwrap();
    assert!(retrieved2.is_some());
}

#[tokio::test]
async fn test_crash_recovery_with_deletes() {
    let pool = setup_test_db().await;
    let service1 = setup_service(&pool).await;

    // Create 1 collection
    let collection_id = service1
        .create_collection(
            "delete-recovery".to_string(),
            64,
            DistanceMetric::Cosine,
            None,
        )
        .await
        .unwrap();

    // Insert 20 vectors
    let mut doc_ids = Vec::new();
    for i in 0..20 {
        let vector = vec![0.05 * (i + 1) as f32; 64];
        let doc = VectorDocument::new(DocumentId::new(), vector);
        let doc_id = service1.insert(collection_id, doc).await.unwrap();
        doc_ids.push(doc_id);
    }

    assert_eq!(service1.get_count(collection_id).await.unwrap(), 20);

    // Delete 5 vectors
    for i in 0..5 {
        service1.delete(collection_id, doc_ids[i]).await.unwrap();
    }

    assert_eq!(service1.get_count(collection_id).await.unwrap(), 15);

    // Drop service (simulate crash)
    drop(service1);

    // Recreate service (simulate restart)
    let service2 = setup_service(&pool).await;
    service2.load_all_collections().await.unwrap();

    // Verify 15 vectors present
    assert_eq!(service2.get_count(collection_id).await.unwrap(), 15);

    // Verify 5 deleted ones absent
    for i in 0..5 {
        let result = service2.get(collection_id, doc_ids[i]).await.unwrap();
        assert!(result.is_none(), "Deleted vector should not exist");
    }

    // Verify remaining 15 vectors present
    for i in 5..20 {
        let result = service2.get(collection_id, doc_ids[i]).await.unwrap();
        assert!(result.is_some(), "Non-deleted vector should exist");
    }

    // Verify search works correctly
    let results = service2
        .query(collection_id, vec![0.5; 64], 10)
        .await
        .unwrap();
    assert_eq!(results.len(), 10);
}

#[tokio::test]
async fn test_crash_recovery_after_compaction() {
    let pool = setup_test_db().await;
    let service1 = setup_service(&pool).await;

    // Create 1 collection
    let collection_id = service1
        .create_collection(
            "compaction-recovery".to_string(),
            64,
            DistanceMetric::Cosine,
            None,
        )
        .await
        .unwrap();

    // Insert 150 vectors (should trigger auto-compaction at 100 ops threshold)
    for i in 0..150 {
        let vector = vec![0.01 * (i + 1) as f32; 64];
        let doc = VectorDocument::new(DocumentId::new(), vector);
        service1.insert(collection_id, doc).await.unwrap();
    }

    assert_eq!(service1.get_count(collection_id).await.unwrap(), 150);

    // Note: We can't directly check if compaction occurred without accessing StorageBackend internals,
    // but the test verifies that recovery works correctly regardless of compaction state

    // Drop service (simulate crash)
    drop(service1);

    // Recreate service (simulate restart)
    let service2 = setup_service(&pool).await;
    service2.load_all_collections().await.unwrap();

    // Verify all 150 vectors recovered
    assert_eq!(service2.get_count(collection_id).await.unwrap(), 150);

    // Verify search works
    let results = service2
        .query(collection_id, vec![0.75; 64], 20)
        .await
        .unwrap();
    assert_eq!(results.len(), 20);
}

#[tokio::test]
async fn test_recovery_preserves_timestamps() {
    let pool = setup_test_db().await;
    let service1 = setup_service(&pool).await;

    // Create 1 collection
    let collection_id = service1
        .create_collection(
            "timestamp-recovery".to_string(),
            64,
            DistanceMetric::Cosine,
            None,
        )
        .await
        .unwrap();

    // Insert 10 vectors with known timestamps
    use chrono::Utc;
    let mut expected_timestamps = Vec::new();
    let mut doc_ids = Vec::new();

    for i in 0..10 {
        let vector = vec![0.1 * (i + 1) as f32; 64];
        let timestamp = Utc::now();
        let mut doc = VectorDocument::new(DocumentId::new(), vector);
        doc.inserted_at = timestamp;

        let doc_id = doc.doc_id;
        service1.insert(collection_id, doc).await.unwrap();

        doc_ids.push(doc_id);
        expected_timestamps.push(timestamp);

        // Sleep briefly to ensure timestamps are different
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // Drop service (simulate crash)
    drop(service1);

    // Recreate service (simulate restart)
    let service2 = setup_service(&pool).await;
    service2.load_all_collections().await.unwrap();

    // Verify recovered vectors have original timestamps (not current time)
    for (i, doc_id) in doc_ids.iter().enumerate() {
        let retrieved = service2.get(collection_id, *doc_id).await.unwrap();
        assert!(retrieved.is_some(), "Vector should exist after recovery");

        let recovered_doc = retrieved.unwrap();
        let expected_ts = expected_timestamps[i];

        // Timestamps should match exactly (within 1 second tolerance for serialization)
        let time_diff = (recovered_doc.inserted_at - expected_ts)
            .num_seconds()
            .abs();
        assert!(
            time_diff <= 1,
            "Timestamp should be preserved across crash recovery. Expected: {:?}, Got: {:?}",
            expected_ts,
            recovered_doc.inserted_at
        );
    }
}

#[tokio::test]
async fn test_concurrent_collection_recovery() {
    let pool = setup_test_db().await;
    let service1 = setup_service(&pool).await;

    // Create 5 collections
    let mut collection_ids = Vec::new();
    for i in 0..5 {
        let col_id = service1
            .create_collection(
                format!("concurrent-{}", i),
                64,
                DistanceMetric::Cosine,
                None,
            )
            .await
            .unwrap();
        collection_ids.push(col_id);
    }

    // Insert 100 vectors per collection (500 total)
    for col_id in &collection_ids {
        for j in 0..100 {
            let vector = vec![0.01 * (j + 1) as f32; 64];
            let doc = VectorDocument::new(DocumentId::new(), vector);
            service1.insert(*col_id, doc).await.unwrap();
        }
    }

    // Verify initial counts
    for col_id in &collection_ids {
        assert_eq!(service1.get_count(*col_id).await.unwrap(), 100);
    }

    // Drop service (simulate crash)
    drop(service1);

    // Recreate service (simulate restart)
    let service2 = Arc::new(setup_service(&pool).await);
    service2.load_all_collections().await.unwrap();

    // Load all 5 collections concurrently using tokio::spawn
    let mut handles = Vec::new();
    for col_id in collection_ids.clone() {
        let service = Arc::clone(&service2);
        let handle = tokio::spawn(async move {
            // Verify count recovered
            let count = service.get_count(col_id).await.unwrap();
            assert_eq!(
                count, 100,
                "Collection should have 100 vectors after recovery"
            );

            // Perform a search to verify index is functional
            let results = service.query(col_id, vec![0.5; 64], 10).await.unwrap();
            assert_eq!(results.len(), 10, "Search should return 10 results");

            col_id
        });
        handles.push(handle);
    }

    // Wait for all concurrent recovery verifications
    let mut recovered_ids = Vec::new();
    for handle in handles {
        let col_id = handle.await.unwrap();
        recovered_ids.push(col_id);
    }

    // Verify all 500 vectors recovered (100 per collection)
    let mut total_count = 0;
    for col_id in &collection_ids {
        total_count += service2.get_count(*col_id).await.unwrap();
    }
    assert_eq!(total_count, 500, "All 500 vectors should be recovered");

    // Verify no cross-collection contamination
    // Each collection should have exactly 100 vectors, no more, no less
    for col_id in &collection_ids {
        let count = service2.get_count(*col_id).await.unwrap();
        assert_eq!(
            count, 100,
            "Each collection should have exactly 100 vectors"
        );
    }

    // Verify all recovered collection IDs match original (set equality check)
    assert_eq!(
        recovered_ids.len(),
        collection_ids.len(),
        "All collections should be recovered"
    );
    for recovered_id in &recovered_ids {
        assert!(
            collection_ids.contains(recovered_id),
            "Recovered collection ID should be in original list"
        );
    }
}
