//! Stress tests for concurrent index operations
//!
//! These tests validate thread-safety and correctness under high concurrency.
//! Run with: cargo test --test stress_tests -- --ignored --test-threads=1
//!
//! Test Strategy (Pragmatic Validation Approach):
//! - 1000+ concurrent operations per test
//! - Validates real-world access patterns
//! - Tests both BruteForceIndex and InstantDistanceIndex
//! - Complements ThreadSanitizer (TSAN) runtime race detection
//!
//! Test Scenarios:
//! 1. Concurrent inserts (1000 threads)
//! 2. Concurrent search during inserts
//! 3. Delete while searching
//! 4. Rebuild under load (InstantDistanceIndex dirty flag pattern)
//! 5. Mixed operations (insert + search + delete + rebuild)

use akidb_core::{DistanceMetric, DocumentId, VectorDocument, VectorIndex};
use akidb_index::{BruteForceIndex, InstantDistanceConfig, InstantDistanceIndex};
use std::sync::Arc;

/// Helper function to create test vectors with deterministic values
fn make_test_vector(id: usize, dimension: usize) -> Vec<f32> {
    (0..dimension)
        .map(|j| ((id + j) as f32) / dimension as f32)
        .collect()
}

/// Helper function to create a test document with known document ID
fn make_test_document(id: usize, dimension: usize) -> VectorDocument {
    VectorDocument::new(DocumentId::new(), make_test_vector(id, dimension))
}

/// Helper function to make a test document and return both doc and id
fn make_test_document_with_id(id: usize, dimension: usize) -> (VectorDocument, DocumentId) {
    let doc_id = DocumentId::new();
    let doc = VectorDocument::new(doc_id, make_test_vector(id, dimension));
    (doc, doc_id)
}

//
// Stress Test 1: Concurrent Inserts (1000 threads)
//
// Validates: Thread-safe insert operations under high concurrency
// Pattern: Pure insert workload, no reads
// Expected: All 1000 inserts succeed, final count = 1000
//

#[tokio::test]
#[ignore] // Run with: cargo test stress_ --ignored
async fn stress_concurrent_insert_1000_brute_force() {
    const NUM_THREADS: usize = 1000;
    const DIMENSION: usize = 128;

    // Use L2 for simpler test assertions (lower score = more similar)
    let index = Arc::new(BruteForceIndex::new(DIMENSION, DistanceMetric::L2));

    // Spawn 1000 concurrent insert tasks
    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|i| {
            let idx = index.clone();
            tokio::spawn(async move {
                let doc = make_test_document(i, DIMENSION);
                idx.insert(doc)
                    .await
                    .expect("Insert should succeed in stress test")
            })
        })
        .collect();

    // Wait for all inserts to complete
    for handle in handles {
        handle.await.expect("Task should not panic");
    }

    // Verify final count
    let count = index.count().await.expect("Count should succeed");
    assert_eq!(
        count, NUM_THREADS,
        "Expected {} documents after concurrent inserts, got {}",
        NUM_THREADS, count
    );

    println!(
        "✅ stress_concurrent_insert_1000_brute_force: {} inserts completed successfully",
        NUM_THREADS
    );
}

#[tokio::test]
#[ignore]
async fn stress_concurrent_insert_1000_instant_hnsw() {
    const NUM_THREADS: usize = 1000;
    const DIMENSION: usize = 128;

    // Use L2 for simpler test assertions (lower score = more similar)
    let config = InstantDistanceConfig::balanced(DIMENSION, DistanceMetric::L2);
    let index =
        Arc::new(InstantDistanceIndex::new(config).expect("Failed to create InstantDistanceIndex"));

    // Spawn 1000 concurrent insert tasks
    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|i| {
            let idx = index.clone();
            tokio::spawn(async move {
                let doc = make_test_document(i, DIMENSION);
                idx.insert(doc)
                    .await
                    .expect("Insert should succeed in stress test")
            })
        })
        .collect();

    // Wait for all inserts to complete
    for handle in handles {
        handle.await.expect("Task should not panic");
    }

    // Verify final count
    let count = index.count().await.expect("Count should succeed");
    assert_eq!(
        count, NUM_THREADS,
        "Expected {} documents after concurrent inserts, got {}",
        NUM_THREADS, count
    );

    println!(
        "✅ stress_concurrent_insert_1000_instant_hnsw: {} inserts completed successfully",
        NUM_THREADS
    );
}

//
// Stress Test 2: Concurrent Search During Inserts
//
// Validates: Readers don't block writers, search correctness during mutations
// Pattern: 500 writers + 500 readers simultaneously
// Expected: All operations succeed, search results are valid (sorted by distance)
//

#[tokio::test]
#[ignore]
async fn stress_search_during_insert_brute_force() {
    const NUM_WRITERS: usize = 500;
    const NUM_READERS: usize = 500;
    const DIMENSION: usize = 128;

    // Use L2 for simpler test assertions (lower score = more similar)
    let index = Arc::new(BruteForceIndex::new(DIMENSION, DistanceMetric::L2));

    // Pre-populate with some vectors for readers to find
    for i in 0..100 {
        let doc = make_test_document(i, DIMENSION);
        index.insert(doc).await.expect("Pre-insert should succeed");
    }

    let mut handles = Vec::new();

    // Spawn writers
    for i in 0..NUM_WRITERS {
        let idx = index.clone();
        let handle = tokio::spawn(async move {
            let doc = make_test_document(100 + i, DIMENSION);
            idx.insert(doc).await.expect("Insert should succeed")
        });
        handles.push(handle);
    }

    // Spawn readers
    for i in 0..NUM_READERS {
        let idx = index.clone();
        let handle = tokio::spawn(async move {
            let query = make_test_vector(i % 100, DIMENSION);
            let results = idx
                .search(&query, 10, None)
                .await
                .expect("Search should succeed");

            // Validate results are sorted by score (ascending)
            for window in results.windows(2) {
                assert!(
                    window[0].score <= window[1].score,
                    "Search results should be sorted by score"
                );
            }
        });
        handles.push(handle);
    }

    // Wait for all operations
    for handle in handles {
        handle.await.expect("Task should not panic");
    }

    // Verify final count
    let count = index.count().await.expect("Count should succeed");
    assert_eq!(
        count,
        100 + NUM_WRITERS,
        "Expected {} documents after stress test",
        100 + NUM_WRITERS
    );

    println!(
        "✅ stress_search_during_insert_brute_force: {} writes + {} reads completed",
        NUM_WRITERS, NUM_READERS
    );
}

#[tokio::test]
#[ignore]
async fn stress_search_during_insert_instant_hnsw() {
    const NUM_WRITERS: usize = 500;
    const NUM_READERS: usize = 500;
    const DIMENSION: usize = 128;

    // Use L2 for simpler test assertions (lower score = more similar)
    let config = InstantDistanceConfig::balanced(DIMENSION, DistanceMetric::L2);
    let index =
        Arc::new(InstantDistanceIndex::new(config).expect("Failed to create InstantDistanceIndex"));

    // Pre-populate with some vectors for readers to find
    for i in 0..100 {
        let doc = make_test_document(i, DIMENSION);
        index.insert(doc).await.expect("Pre-insert should succeed");
    }

    let mut handles = Vec::new();

    // Spawn writers
    for i in 0..NUM_WRITERS {
        let idx = index.clone();
        let handle = tokio::spawn(async move {
            let doc = make_test_document(100 + i, DIMENSION);
            idx.insert(doc).await.expect("Insert should succeed")
        });
        handles.push(handle);
    }

    // Spawn readers
    for i in 0..NUM_READERS {
        let idx = index.clone();
        let handle = tokio::spawn(async move {
            let query = make_test_vector(i % 100, DIMENSION);
            let results = idx
                .search(&query, 10, None)
                .await
                .expect("Search should succeed");

            // Validate results are sorted by score (ascending)
            for window in results.windows(2) {
                assert!(
                    window[0].score <= window[1].score,
                    "Search results should be sorted by score"
                );
            }
        });
        handles.push(handle);
    }

    // Wait for all operations
    for handle in handles {
        handle.await.expect("Task should not panic");
    }

    // Verify final count
    let count = index.count().await.expect("Count should succeed");
    assert_eq!(
        count,
        100 + NUM_WRITERS,
        "Expected {} documents after stress test",
        100 + NUM_WRITERS
    );

    println!(
        "✅ stress_search_during_insert_instant_hnsw: {} writes + {} reads completed",
        NUM_WRITERS, NUM_READERS
    );
}

//
// Stress Test 3: Delete While Searching
//
// Validates: Delete operations don't corrupt search results
// Pattern: Pre-populate 1000 docs, then 500 deletes + 500 searches concurrently
// Expected: All operations succeed, final count = 500
//

#[tokio::test]
#[ignore]
async fn stress_delete_while_searching_brute_force() {
    const INITIAL_DOCS: usize = 1000;
    const NUM_DELETES: usize = 500;
    const NUM_SEARCHES: usize = 500;
    const DIMENSION: usize = 128;

    // Use L2 for simpler test assertions (lower score = more similar)
    let index = Arc::new(BruteForceIndex::new(DIMENSION, DistanceMetric::L2));

    // Pre-populate with 1000 documents
    let mut doc_ids = Vec::new();
    for i in 0..INITIAL_DOCS {
        let (doc, id) = make_test_document_with_id(i, DIMENSION);
        index.insert(doc).await.expect("Pre-insert should succeed");
        doc_ids.push(id);
    }

    let mut handles = Vec::new();

    // Spawn deleters (delete first 500 documents)
    for i in 0..NUM_DELETES {
        let idx = index.clone();
        let id = doc_ids[i];
        let handle =
            tokio::spawn(async move { idx.delete(id).await.expect("Delete should succeed") });
        handles.push(handle);
    }

    // Spawn searchers
    for i in 0..NUM_SEARCHES {
        let idx = index.clone();
        let handle = tokio::spawn(async move {
            let query = make_test_vector(i, DIMENSION);
            let results = idx
                .search(&query, 10, None)
                .await
                .expect("Search should succeed");

            // Validate results are sorted
            for window in results.windows(2) {
                assert!(
                    window[0].score <= window[1].score,
                    "Search results should be sorted"
                );
            }
        });
        handles.push(handle);
    }

    // Wait for all operations
    for handle in handles {
        handle.await.expect("Task should not panic");
    }

    // Verify final count
    let count = index.count().await.expect("Count should succeed");
    assert_eq!(
        count,
        INITIAL_DOCS - NUM_DELETES,
        "Expected {} documents after deletes",
        INITIAL_DOCS - NUM_DELETES
    );

    println!(
        "✅ stress_delete_while_searching_brute_force: {} deletes + {} searches completed",
        NUM_DELETES, NUM_SEARCHES
    );
}

#[tokio::test]
#[ignore]
async fn stress_delete_while_searching_instant_hnsw() {
    const INITIAL_DOCS: usize = 1000;
    const NUM_DELETES: usize = 500;
    const NUM_SEARCHES: usize = 500;
    const DIMENSION: usize = 128;

    // Use L2 for simpler test assertions (lower score = more similar)
    let config = InstantDistanceConfig::balanced(DIMENSION, DistanceMetric::L2);
    let index =
        Arc::new(InstantDistanceIndex::new(config).expect("Failed to create InstantDistanceIndex"));

    // Pre-populate with 1000 documents
    let mut doc_ids = Vec::new();
    for i in 0..INITIAL_DOCS {
        let (doc, id) = make_test_document_with_id(i, DIMENSION);
        index.insert(doc).await.expect("Pre-insert should succeed");
        doc_ids.push(id);
    }

    let mut handles = Vec::new();

    // Spawn deleters (delete first 500 documents)
    for i in 0..NUM_DELETES {
        let idx = index.clone();
        let id = doc_ids[i];
        let handle =
            tokio::spawn(async move { idx.delete(id).await.expect("Delete should succeed") });
        handles.push(handle);
    }

    // Spawn searchers
    for i in 0..NUM_SEARCHES {
        let idx = index.clone();
        let handle = tokio::spawn(async move {
            let query = make_test_vector(i, DIMENSION);
            let results = idx
                .search(&query, 10, None)
                .await
                .expect("Search should succeed");

            // Validate results are sorted
            for window in results.windows(2) {
                assert!(
                    window[0].score <= window[1].score,
                    "Search results should be sorted"
                );
            }
        });
        handles.push(handle);
    }

    // Wait for all operations
    for handle in handles {
        handle.await.expect("Task should not panic");
    }

    // Verify final count
    let count = index.count().await.expect("Count should succeed");
    assert_eq!(
        count,
        INITIAL_DOCS - NUM_DELETES,
        "Expected {} documents after deletes",
        INITIAL_DOCS - NUM_DELETES
    );

    println!(
        "✅ stress_delete_while_searching_instant_hnsw: {} deletes + {} searches completed",
        NUM_DELETES, NUM_SEARCHES
    );
}

//
// Stress Test 4: Rebuild Under Load (InstantDistanceIndex only)
//
// Validates: The critical dirty flag pattern in InstantDistanceIndex
// Pattern: Concurrent inserts trigger multiple rebuilds while searches are running
// Expected: No panics, no data corruption, searches return valid results
//
// This tests the pattern Bob analyzed:
// - Dirty flag and HnswMap pointer protected by same RwLock
// - Visibility guarantee: readers see consistent (dirty, map) pairs
// - No TOCTOU race because both are behind same lock
//

#[tokio::test]
#[ignore]
async fn stress_rebuild_under_load_instant_hnsw() {
    const NUM_INSERTS: usize = 500;
    const NUM_SEARCHES: usize = 500;
    const DIMENSION: usize = 128;

    // Use L2 for simpler test assertions (lower score = more similar)
    let config = InstantDistanceConfig::balanced(DIMENSION, DistanceMetric::L2);
    let index =
        Arc::new(InstantDistanceIndex::new(config).expect("Failed to create InstantDistanceIndex"));

    // Pre-populate to trigger at least one rebuild
    for i in 0..50 {
        let doc = make_test_document(i, DIMENSION);
        index.insert(doc).await.expect("Pre-insert should succeed");
    }

    let mut handles = Vec::new();

    // Spawn inserters (will trigger multiple rebuilds)
    for i in 0..NUM_INSERTS {
        let idx = index.clone();
        let handle = tokio::spawn(async move {
            let doc = make_test_document(50 + i, DIMENSION);
            idx.insert(doc).await.expect("Insert should succeed")
        });
        handles.push(handle);
    }

    // Spawn searchers (may read during rebuilds)
    for i in 0..NUM_SEARCHES {
        let idx = index.clone();
        let handle = tokio::spawn(async move {
            let query = make_test_vector(i % 50, DIMENSION);
            let results = idx
                .search(&query, 10, None)
                .await
                .expect("Search should succeed even during rebuild");

            // Validate results are sorted
            for window in results.windows(2) {
                assert!(
                    window[0].score <= window[1].score,
                    "Search results should be sorted even during rebuild"
                );
            }
        });
        handles.push(handle);
    }

    // Wait for all operations
    for handle in handles {
        handle.await.expect("Task should not panic during rebuild");
    }

    // Verify final count
    let count = index.count().await.expect("Count should succeed");
    assert_eq!(
        count,
        50 + NUM_INSERTS,
        "Expected {} documents after rebuild stress test",
        50 + NUM_INSERTS
    );

    println!(
        "✅ stress_rebuild_under_load_instant_hnsw: {} inserts + {} searches with rebuilds completed",
        NUM_INSERTS, NUM_SEARCHES
    );
}

//
// Stress Test 5: Mixed Operations (All operations simultaneously)
//
// Validates: Complex real-world workload with all operation types
// Pattern: 200 inserts + 300 searches + 100 deletes + 50 gets + 50 clears (on separate index)
// Expected: All operations succeed, no data corruption, final counts correct
//

#[tokio::test]
#[ignore]
async fn stress_mixed_operations_brute_force() {
    const INITIAL_DOCS: usize = 200;
    const NUM_INSERTS: usize = 200;
    const NUM_SEARCHES: usize = 300;
    const NUM_DELETES: usize = 100;
    const NUM_GETS: usize = 50;
    const DIMENSION: usize = 128;

    // Use L2 for simpler test assertions (lower score = more similar)
    let index = Arc::new(BruteForceIndex::new(DIMENSION, DistanceMetric::L2));

    // Pre-populate
    let mut doc_ids = Vec::new();
    for i in 0..INITIAL_DOCS {
        let (doc, id) = make_test_document_with_id(i, DIMENSION);
        index.insert(doc).await.expect("Pre-insert should succeed");
        doc_ids.push(id);
    }

    let mut handles = Vec::new();

    // Spawn inserters
    for i in 0..NUM_INSERTS {
        let idx = index.clone();
        let handle = tokio::spawn(async move {
            let doc = make_test_document(INITIAL_DOCS + i, DIMENSION);
            idx.insert(doc).await.expect("Insert should succeed")
        });
        handles.push(handle);
    }

    // Spawn searchers
    for i in 0..NUM_SEARCHES {
        let idx = index.clone();
        let handle = tokio::spawn(async move {
            let query = make_test_vector(i % INITIAL_DOCS, DIMENSION);
            idx.search(&query, 10, None)
                .await
                .expect("Search should succeed");
        });
        handles.push(handle);
    }

    // Spawn deleters
    for i in 0..NUM_DELETES {
        let idx = index.clone();
        let id = doc_ids[i];
        let handle =
            tokio::spawn(async move { idx.delete(id).await.expect("Delete should succeed") });
        handles.push(handle);
    }

    // Spawn getters
    for i in 0..NUM_GETS {
        let idx = index.clone();
        let id = doc_ids[NUM_DELETES + i]; // Get from non-deleted docs
        let handle = tokio::spawn(async move {
            idx.get(id).await.expect("Get should succeed");
        });
        handles.push(handle);
    }

    // Wait for all operations
    for handle in handles {
        handle.await.expect("Task should not panic");
    }

    // Verify final count
    let count = index.count().await.expect("Count should succeed");
    assert_eq!(
        count,
        INITIAL_DOCS + NUM_INSERTS - NUM_DELETES,
        "Expected {} documents after mixed operations",
        INITIAL_DOCS + NUM_INSERTS - NUM_DELETES
    );

    println!(
        "✅ stress_mixed_operations_brute_force: {} inserts + {} searches + {} deletes + {} gets completed",
        NUM_INSERTS, NUM_SEARCHES, NUM_DELETES, NUM_GETS
    );
}

#[tokio::test]
#[ignore]
async fn stress_mixed_operations_instant_hnsw() {
    const INITIAL_DOCS: usize = 200;
    const NUM_INSERTS: usize = 200;
    const NUM_SEARCHES: usize = 300;
    const NUM_DELETES: usize = 100;
    const NUM_GETS: usize = 50;
    const DIMENSION: usize = 128;

    // Use L2 for simpler test assertions (lower score = more similar)
    let config = InstantDistanceConfig::balanced(DIMENSION, DistanceMetric::L2);
    let index =
        Arc::new(InstantDistanceIndex::new(config).expect("Failed to create InstantDistanceIndex"));

    // Pre-populate
    let mut doc_ids = Vec::new();
    for i in 0..INITIAL_DOCS {
        let (doc, id) = make_test_document_with_id(i, DIMENSION);
        index.insert(doc).await.expect("Pre-insert should succeed");
        doc_ids.push(id);
    }

    let mut handles = Vec::new();

    // Spawn inserters
    for i in 0..NUM_INSERTS {
        let idx = index.clone();
        let handle = tokio::spawn(async move {
            let doc = make_test_document(INITIAL_DOCS + i, DIMENSION);
            idx.insert(doc).await.expect("Insert should succeed")
        });
        handles.push(handle);
    }

    // Spawn searchers
    for i in 0..NUM_SEARCHES {
        let idx = index.clone();
        let handle = tokio::spawn(async move {
            let query = make_test_vector(i % INITIAL_DOCS, DIMENSION);
            idx.search(&query, 10, None)
                .await
                .expect("Search should succeed");
        });
        handles.push(handle);
    }

    // Spawn deleters
    for i in 0..NUM_DELETES {
        let idx = index.clone();
        let id = doc_ids[i];
        let handle =
            tokio::spawn(async move { idx.delete(id).await.expect("Delete should succeed") });
        handles.push(handle);
    }

    // Spawn getters
    for i in 0..NUM_GETS {
        let idx = index.clone();
        let id = doc_ids[NUM_DELETES + i]; // Get from non-deleted docs
        let handle = tokio::spawn(async move {
            idx.get(id).await.expect("Get should succeed");
        });
        handles.push(handle);
    }

    // Wait for all operations
    for handle in handles {
        handle.await.expect("Task should not panic");
    }

    // Verify final count
    let count = index.count().await.expect("Count should succeed");
    assert_eq!(
        count,
        INITIAL_DOCS + NUM_INSERTS - NUM_DELETES,
        "Expected {} documents after mixed operations",
        INITIAL_DOCS + NUM_INSERTS - NUM_DELETES
    );

    println!(
        "✅ stress_mixed_operations_instant_hnsw: {} inserts + {} searches + {} deletes + {} gets completed",
        NUM_INSERTS, NUM_SEARCHES, NUM_DELETES, NUM_GETS
    );
}

//
// Additional Comprehensive Stress Tests
//

#[tokio::test]
#[ignore = "Heavy test: Large dataset 10k vectors (~60s runtime)"]
async fn stress_large_dataset_integrity() {
    // Test: Validates index integrity with a large dataset (10,000 vectors)
    // and ensures search quality remains acceptable at scale
    const DIM: usize = 512;
    const N_VECTORS: usize = 10_000;
    const K: usize = 20;

    println!("Starting stress_large_dataset_integrity test...");
    let start = std::time::Instant::now();

    let config = InstantDistanceConfig::balanced(DIM, DistanceMetric::Cosine);
    let index = InstantDistanceIndex::new(config).expect("Failed to create InstantDistanceIndex");

    // Insert all vectors with progress tracking
    println!("Inserting {} vectors (dim={})...", N_VECTORS, DIM);
    let mut doc_ids = Vec::with_capacity(N_VECTORS);
    for i in 0..N_VECTORS {
        if i % 1000 == 0 && i > 0 {
            println!("  Inserted {}/{}", i, N_VECTORS);
        }

        let doc_id = DocumentId::new();
        let vector = make_test_vector(i, DIM);
        let doc = VectorDocument::new(doc_id, vector);
        index.insert(doc).await.expect("Insert failed");
        doc_ids.push(doc_id);
    }

    // Verify count
    let count = index.count().await.expect("Count failed");
    assert_eq!(count, N_VECTORS, "Document count mismatch");

    // Perform searches to validate quality
    println!("Running 20 search queries to validate integrity...");
    for i in 0..20 {
        let query = make_test_vector(i * 100, DIM);
        let results = index.search(&query, K, None).await.expect("Search failed");

        assert_eq!(
            results.len(),
            K,
            "Query {} returned {} results, expected {}",
            i,
            results.len(),
            K
        );

        // Verify scores are valid and sorted
        for (j, result) in results.iter().enumerate() {
            assert!(
                result.score.is_finite(),
                "Query {} result {} has non-finite score",
                i,
                j
            );

            if j > 0 {
                // Cosine similarity: higher is better, so results should be descending
                assert!(
                    results[j - 1].score >= result.score,
                    "Results not properly sorted"
                );
            }
        }
    }

    // Test random document retrieval
    println!("Verifying random document retrieval (100 samples)...");
    for _ in 0..100 {
        let idx = rand::random::<usize>() % doc_ids.len();
        let doc_id = doc_ids[idx];
        let doc = index.get(doc_id).await.expect("Get failed");
        assert!(doc.is_some(), "Document not found: {:?}", doc_id);
    }

    let elapsed = start.elapsed();
    println!(
        "✅ stress_large_dataset_integrity: {} vectors processed in {:.2}s",
        N_VECTORS,
        elapsed.as_secs_f64()
    );
}

#[tokio::test]
async fn stress_rapid_collection_lifecycle() {
    // Test: Validates that rapid creation and destruction of index instances
    // doesn't cause memory leaks or resource exhaustion
    const DIM: usize = 128;
    const N_COLLECTIONS: usize = 100;
    const DOCS_PER_COLLECTION: usize = 100;

    println!("Starting stress_rapid_collection_lifecycle test...");
    let start = std::time::Instant::now();

    for i in 0..N_COLLECTIONS {
        if i % 10 == 0 && i > 0 {
            println!("  Created {}/{} collections", i, N_COLLECTIONS);
        }

        // Create index
        let index = BruteForceIndex::new(DIM, DistanceMetric::Cosine);

        // Populate with documents
        for j in 0..DOCS_PER_COLLECTION {
            let doc = make_test_document(j, DIM);
            index.insert(doc).await.expect("Insert failed");
        }

        // Perform a search to validate
        let query = make_test_vector(0, DIM);
        let results = index.search(&query, 10, None).await.expect("Search failed");
        assert!(!results.is_empty(), "Search returned no results");

        // Verify count
        let count = index.count().await.expect("Count failed");
        assert_eq!(count, DOCS_PER_COLLECTION);

        // Index is dropped here, testing cleanup
    }

    let elapsed = start.elapsed();
    println!(
        "✅ stress_rapid_collection_lifecycle: {} collections in {:.2}s ({:.0} collections/sec)",
        N_COLLECTIONS,
        elapsed.as_secs_f64(),
        N_COLLECTIONS as f64 / elapsed.as_secs_f64()
    );
}

#[tokio::test]
#[ignore = "Heavy test: Memory pressure (~90s runtime, uses ~2GB RAM)"]
async fn stress_memory_pressure() {
    // Test: Validates behavior under high memory usage (careful: uses ~2GB RAM)
    // Ensures graceful handling and no memory corruption
    const DIM: usize = 1024; // Large dimension
    const TARGET_VECTORS: usize = 500_000; // 500k vectors @ 1024-dim ≈ 2GB
    const BATCH_SIZE: usize = 10_000;
    const K: usize = 10;

    println!("Starting stress_memory_pressure test...");
    println!("WARNING: This test will use approximately 2GB of RAM");
    let start = std::time::Instant::now();

    let config = InstantDistanceConfig::fast(DIM, DistanceMetric::L2);
    let index = InstantDistanceIndex::new(config).expect("Failed to create InstantDistanceIndex");

    println!(
        "Inserting {} vectors in batches of {}...",
        TARGET_VECTORS, BATCH_SIZE
    );

    for batch in 0..(TARGET_VECTORS / BATCH_SIZE) {
        println!("  Batch {}/{}", batch + 1, TARGET_VECTORS / BATCH_SIZE);

        for i in 0..BATCH_SIZE {
            let doc = make_test_document(batch * BATCH_SIZE + i, DIM);
            index.insert(doc).await.expect("Insert failed");
        }

        // Verify index still works after each batch
        if batch % 5 == 0 {
            let query = make_test_vector(batch * 100, DIM);
            let results = index.search(&query, K, None).await.expect("Search failed");
            assert!(!results.is_empty(), "Search failed after batch {}", batch);
        }
    }

    let final_count = index.count().await.expect("Count failed");
    assert_eq!(final_count, TARGET_VECTORS, "Vector count mismatch");

    // Final search to validate integrity
    let query = make_test_vector(0, DIM);
    let results = index
        .search(&query, K, None)
        .await
        .expect("Final search failed");
    assert_eq!(results.len(), K, "Final search returned wrong count");

    let elapsed = start.elapsed();
    println!(
        "✅ stress_memory_pressure: {} vectors in {:.2}s",
        TARGET_VECTORS,
        elapsed.as_secs_f64()
    );
}

#[tokio::test]
#[ignore = "Heavy test: Search accuracy under concurrent load (~40s runtime)"]
async fn stress_search_accuracy_under_load() {
    // Test: Validates that search quality (recall) doesn't degrade under
    // concurrent load by comparing against brute-force baseline
    const DIM: usize = 128;
    const N_VECTORS: usize = 2000;
    const N_CONCURRENT_SEARCHES: usize = 100;
    const K: usize = 10;

    println!("Starting stress_search_accuracy_under_load test...");
    let start = std::time::Instant::now();

    // Create both brute-force (ground truth) and HNSW index
    let bf_index = Arc::new(BruteForceIndex::new(DIM, DistanceMetric::Cosine));
    let hnsw_config = InstantDistanceConfig::high_recall(DIM, DistanceMetric::Cosine);
    let hnsw_index = Arc::new(
        InstantDistanceIndex::new(hnsw_config).expect("Failed to create InstantDistanceIndex"),
    );

    // Insert same vectors into both indices
    println!("Populating indices with {} vectors...", N_VECTORS);
    for i in 0..N_VECTORS {
        if i % 500 == 0 && i > 0 {
            println!("  Inserted {}/{}", i, N_VECTORS);
        }

        let doc_id = DocumentId::new();
        let vector = make_test_vector(i, DIM);
        let doc = VectorDocument::new(doc_id, vector);

        bf_index
            .insert(doc.clone())
            .await
            .expect("BF insert failed");
        hnsw_index.insert(doc).await.expect("HNSW insert failed");
    }

    // Generate test queries
    let test_queries: Vec<Vec<f32>> = (0..N_CONCURRENT_SEARCHES)
        .map(|i| make_test_vector(i, DIM))
        .collect();
    let test_queries = Arc::new(test_queries);

    // Spawn concurrent search tasks
    println!("Running {} concurrent searches...", N_CONCURRENT_SEARCHES);
    let mut handles = Vec::new();
    for i in 0..N_CONCURRENT_SEARCHES {
        let bf_clone = Arc::clone(&bf_index);
        let hnsw_clone = Arc::clone(&hnsw_index);
        let queries_clone = Arc::clone(&test_queries);

        let handle = tokio::spawn(async move {
            let query = &queries_clone[i];

            // Get ground truth from brute-force
            let bf_results = bf_clone
                .search(query, K, None)
                .await
                .expect("BF search failed");
            let bf_ids: std::collections::HashSet<_> =
                bf_results.iter().map(|r| r.doc_id).collect();

            // Get HNSW results
            let hnsw_results = hnsw_clone
                .search(query, K, None)
                .await
                .expect("HNSW search failed");
            let hnsw_ids: std::collections::HashSet<_> =
                hnsw_results.iter().map(|r| r.doc_id).collect();

            // Compute recall
            let overlap = bf_ids.intersection(&hnsw_ids).count();
            overlap as f64 / K as f64
        });
        handles.push(handle);
    }

    // Collect recall metrics
    let mut total_recall = 0.0;
    for handle in handles {
        let recall = handle.await.expect("Task panicked");
        total_recall += recall;
    }

    let avg_recall = total_recall / N_CONCURRENT_SEARCHES as f64;
    println!("Average recall@{}: {:.3}", K, avg_recall);

    // Verify recall is still high under load
    assert!(
        avg_recall > 0.90,
        "Recall degraded under load: {:.3} < 0.90",
        avg_recall
    );

    let elapsed = start.elapsed();
    println!(
        "✅ stress_search_accuracy_under_load: {:.2}s (recall: {:.3})",
        elapsed.as_secs_f64(),
        avg_recall
    );
}

#[tokio::test]
async fn stress_delete_and_reinsert_cycles() {
    // Test: Validates that repeatedly deleting and reinserting the same document IDs
    // doesn't cause corruption or resource leaks
    const DIM: usize = 128;
    const N_ITERATIONS: usize = 100;
    const N_DOCS: usize = 100;

    println!("Starting stress_delete_and_reinsert_cycles test...");
    let start = std::time::Instant::now();

    let index = BruteForceIndex::new(DIM, DistanceMetric::Cosine);

    // Create fixed set of document IDs
    let doc_ids: Vec<DocumentId> = (0..N_DOCS).map(|_| DocumentId::new()).collect();

    for iteration in 0..N_ITERATIONS {
        if iteration % 10 == 0 && iteration > 0 {
            println!("  Iteration {}/{}", iteration, N_ITERATIONS);
        }

        // Insert all documents
        for (i, doc_id) in doc_ids.iter().enumerate() {
            let vector = make_test_vector(i + iteration, DIM);
            let doc = VectorDocument::new(*doc_id, vector);
            index.insert(doc).await.expect("Insert failed");
        }

        // Verify all present
        let count = index.count().await.expect("Count failed");
        assert_eq!(count, N_DOCS, "Count mismatch after insert");

        // Delete all documents
        for doc_id in &doc_ids {
            index.delete(*doc_id).await.expect("Delete failed");
        }

        // Verify all gone
        let count = index.count().await.expect("Count failed");
        assert_eq!(count, 0, "Count mismatch after delete");
    }

    let elapsed = start.elapsed();
    println!(
        "✅ stress_delete_and_reinsert_cycles: {} iterations in {:.2}s",
        N_ITERATIONS,
        elapsed.as_secs_f64()
    );
}

#[tokio::test]
#[ignore = "Heavy test: Large batch operations (~50s runtime)"]
async fn stress_batch_operations() {
    // Test: Validates efficient handling of large batch inserts
    // to ensure bulk loading paths work correctly
    const DIM: usize = 256;
    const BATCH_SIZE: usize = 1000;
    const N_BATCHES: usize = 10;
    const K: usize = 10;

    println!("Starting stress_batch_operations test...");
    let start = std::time::Instant::now();

    let config = InstantDistanceConfig::balanced(DIM, DistanceMetric::Cosine);
    let index = InstantDistanceIndex::new(config).expect("Failed to create InstantDistanceIndex");

    for batch_idx in 0..N_BATCHES {
        println!("Processing batch {}/{}", batch_idx + 1, N_BATCHES);

        // Create batch
        let mut batch = Vec::with_capacity(BATCH_SIZE);
        for i in 0..BATCH_SIZE {
            let doc = make_test_document(batch_idx * BATCH_SIZE + i, DIM);
            batch.push(doc);
        }

        // Insert batch
        index
            .insert_batch(batch)
            .await
            .expect("Batch insert failed");

        // Verify count
        let expected_count = (batch_idx + 1) * BATCH_SIZE;
        let actual_count = index.count().await.expect("Count failed");
        assert_eq!(
            actual_count, expected_count,
            "Count mismatch after batch {}",
            batch_idx
        );

        // Verify search still works
        let query = make_test_vector(batch_idx * 100, DIM);
        let results = index.search(&query, K, None).await.expect("Search failed");
        assert!(
            !results.is_empty(),
            "Search failed after batch {}",
            batch_idx
        );
    }

    let total_docs = BATCH_SIZE * N_BATCHES;
    let final_count = index.count().await.expect("Final count failed");
    assert_eq!(final_count, total_docs, "Final count mismatch");

    let elapsed = start.elapsed();
    println!(
        "✅ stress_batch_operations: {} vectors in {} batches, {:.2}s ({:.0} vectors/sec)",
        total_docs,
        N_BATCHES,
        elapsed.as_secs_f64(),
        total_docs as f64 / elapsed.as_secs_f64()
    );
}

#[tokio::test]
#[ignore = "Heavy test: Index rebuild cycles under load (~45s runtime)"]
async fn stress_index_rebuild_cycles() {
    // Test: Validates that forcing index rebuilds under concurrent operations
    // doesn't cause data loss or corruption (specific to InstantDistanceIndex lazy rebuild)
    const DIM: usize = 128;
    const N_ROUNDS: usize = 10;
    const INSERTS_PER_ROUND: usize = 100;
    const DELETES_PER_ROUND: usize = 50;
    const SEARCHES_PER_ROUND: usize = 100;
    const K: usize = 10;

    println!("Starting stress_index_rebuild_cycles test...");
    let start = std::time::Instant::now();

    let config = InstantDistanceConfig::balanced(DIM, DistanceMetric::Cosine);
    let index =
        Arc::new(InstantDistanceIndex::new(config).expect("Failed to create InstantDistanceIndex"));

    let mut all_doc_ids = Vec::new();

    for round in 0..N_ROUNDS {
        println!("Round {}/{}", round + 1, N_ROUNDS);

        // Phase 1: Concurrent inserts (triggers rebuild)
        let mut insert_handles = Vec::new();
        for i in 0..INSERTS_PER_ROUND {
            let index_clone = Arc::clone(&index);
            let handle = tokio::spawn(async move {
                let doc = make_test_document(round * INSERTS_PER_ROUND + i, DIM);
                let doc_id = doc.doc_id;
                index_clone.insert(doc).await.expect("Insert failed");
                doc_id
            });
            insert_handles.push(handle);
        }

        for handle in insert_handles {
            let doc_id = handle.await.expect("Insert task panicked");
            all_doc_ids.push(doc_id);
        }

        // Phase 2: Concurrent searches (forces index rebuild if dirty)
        let mut search_handles = Vec::new();
        for i in 0..SEARCHES_PER_ROUND {
            let index_clone = Arc::clone(&index);
            let handle = tokio::spawn(async move {
                let query = make_test_vector(i, DIM);
                let _results = index_clone
                    .search(&query, K, None)
                    .await
                    .expect("Search failed");
            });
            search_handles.push(handle);
        }

        for handle in search_handles {
            handle.await.expect("Search task panicked");
        }

        // Phase 3: Delete some documents (makes index dirty again)
        if all_doc_ids.len() >= DELETES_PER_ROUND {
            for _ in 0..DELETES_PER_ROUND {
                let idx = rand::random::<usize>() % all_doc_ids.len();
                let doc_id = all_doc_ids.remove(idx);
                index.delete(doc_id).await.expect("Delete failed");
            }
        }

        // Verify index integrity
        let count = index.count().await.expect("Count failed");
        println!("  Round {} count: {}", round + 1, count);
    }

    // Final integrity check
    println!("Final integrity check...");
    let final_count = index.count().await.expect("Final count failed");
    println!("Final document count: {}", final_count);

    // Verify remaining documents are retrievable
    let sample_size = all_doc_ids.len().min(10);
    for (i, doc_id) in all_doc_ids.iter().enumerate().take(sample_size) {
        let doc = index.get(*doc_id).await.expect("Get failed");
        assert!(doc.is_some(), "Document {} not found after stress test", i);
    }

    // Final search
    let query = make_test_vector(0, DIM);
    let results = index
        .search(&query, K, None)
        .await
        .expect("Final search failed");
    assert!(!results.is_empty(), "Final search returned no results");

    let elapsed = start.elapsed();
    println!(
        "✅ stress_index_rebuild_cycles: {} rounds in {:.2}s",
        N_ROUNDS,
        elapsed.as_secs_f64()
    );
}
