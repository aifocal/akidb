// Property-based tests for vector indexes
//
// Uses proptest to generate 100+ random test cases per property,
// validating mathematical invariants across both BruteForceIndex and InstantDistanceIndex.
//
// Properties tested:
// 1. Insert idempotency: inserting same document twice doesn't change count
// 2. Search result ordering: results are sorted by score
// 3. Count consistency: insert N vectors → count() = N
// 4. Delete correctness: delete decreases count by 1
// 5. Search quality: all scores are finite (no NaN/infinity)
// 6. Edge cases: empty index, single vector, duplicate vectors

use akidb_core::{DistanceMetric, DocumentId, VectorDocument, VectorIndex};
use akidb_index::{BruteForceIndex, InstantDistanceConfig, InstantDistanceIndex};
use proptest::prelude::*;

// Helper to create test vectors with random values
fn make_test_vector(dimension: usize, seed: u64) -> Vec<f32> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    (0..dimension)
        .map(|i| {
            let mut hasher = DefaultHasher::new();
            seed.hash(&mut hasher);
            i.hash(&mut hasher);
            let hash = hasher.finish();
            // Map hash to [-1.0, 1.0] range
            ((hash % 1000) as f32 / 500.0) - 1.0
        })
        .collect()
}

// Helper to create VectorDocument with external_id
fn make_test_document(dimension: usize, seed: u64) -> (VectorDocument, DocumentId) {
    let doc_id = DocumentId::new();
    let vector = make_test_vector(dimension, seed);
    let doc = VectorDocument::new(doc_id, vector).with_external_id(format!("test-{}", seed));
    (doc, doc_id)
}

// ============================================================================
// Property 1: Insert Duplicate Returns Error (Unique Insert Semantics)
// ============================================================================

proptest! {
    #[test]
    fn prop_brute_force_insert_duplicate_errors(
        dimension in 16usize..=256,
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result: Result<(), TestCaseError> = runtime.block_on(async {
            let index = BruteForceIndex::new(dimension, DistanceMetric::L2);

            // Insert a document
            let (doc, _doc_id) = make_test_document(dimension, 42);
            index.insert(doc.clone()).await.unwrap();

            let count_before = index.count().await.unwrap();
            prop_assert_eq!(count_before, 1);

            // Re-insert same document should error
            let result = index.insert(doc).await;
            prop_assert!(result.is_err(), "Duplicate insert should return error");

            // Count should remain the same
            let count_after = index.count().await.unwrap();
            prop_assert_eq!(count_after, count_before);
            Ok(())
        });
        result?;
    }

    #[test]
    fn prop_instant_insert_duplicate_errors(
        dimension in 16usize..=256,
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result: Result<(), TestCaseError> = runtime.block_on(async {
            let config = InstantDistanceConfig::balanced(dimension, DistanceMetric::L2);
            let index = InstantDistanceIndex::new(config).unwrap();

            // Insert a document
            let (doc, _doc_id) = make_test_document(dimension, 42);
            index.insert(doc.clone()).await.unwrap();

            let count_before = index.count().await.unwrap();
            prop_assert_eq!(count_before, 1);

            // Re-insert same document should error
            let result = index.insert(doc).await;
            prop_assert!(result.is_err(), "Duplicate insert should return error");

            // Count should remain the same
            let count_after = index.count().await.unwrap();
            prop_assert_eq!(count_after, count_before);
            Ok(())
        });
        result?;
    }
}

// ============================================================================
// Property 2: Search Result Ordering (L2 metric: ascending)
// ============================================================================

proptest! {
    #[test]
    fn prop_brute_force_search_ordering(
        dimension in 16usize..=256,
        num_vectors in 5usize..=50,
        k in 2usize..=10,
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result: Result<(), TestCaseError> = runtime.block_on(async {
            let index = BruteForceIndex::new(dimension, DistanceMetric::L2);

            // Insert vectors
            for i in 0..num_vectors {
                let (doc, _) = make_test_document(dimension, i as u64);
                index.insert(doc).await.unwrap();
            }

            // Search with random query
            let query = make_test_vector(dimension, 999);
            let results = index.search(&query, k, None).await.unwrap();

            // Verify results are sorted (L2: lower score = more similar)
            for i in 0..results.len().saturating_sub(1) {
                prop_assert!(
                    results[i].score <= results[i + 1].score,
                    "Results not sorted: results[{}].score = {}, results[{}].score = {}",
                    i, results[i].score, i + 1, results[i + 1].score
                );
            }
            Ok(())
        });
        result?;
    }

    #[test]
    fn prop_instant_search_ordering(
        dimension in 16usize..=256,
        num_vectors in 5usize..=50,
        k in 2usize..=10,
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result: Result<(), TestCaseError> = runtime.block_on(async {
            let config = InstantDistanceConfig::balanced(dimension, DistanceMetric::L2);
            let index = InstantDistanceIndex::new(config).unwrap();

            // Insert vectors
            for i in 0..num_vectors {
                let (doc, _) = make_test_document(dimension, i as u64);
                index.insert(doc).await.unwrap();
            }

            // Search with random query
            let query = make_test_vector(dimension, 999);
            let results = index.search(&query, k, None).await.unwrap();

            // Verify results are sorted (L2: lower score = more similar)
            for i in 0..results.len().saturating_sub(1) {
                prop_assert!(
                    results[i].score <= results[i + 1].score,
                    "Results not sorted: results[{}].score = {}, results[{}].score = {}",
                    i, results[i].score, i + 1, results[i + 1].score
                );
            }
            Ok(())
        });
        result?;
    }
}

// ============================================================================
// Property 3: Count Consistency
// ============================================================================

proptest! {
    #[test]
    fn prop_brute_force_count_consistency(
        dimension in 16usize..=256,
        num_inserts in 1usize..=100,
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result: Result<(), TestCaseError> = runtime.block_on(async {
            let index = BruteForceIndex::new(dimension, DistanceMetric::L2);

            // Insert N vectors
            for i in 0..num_inserts {
                let (doc, _) = make_test_document(dimension, i as u64);
                index.insert(doc).await.unwrap();
            }

            // Count should equal N
            let count = index.count().await.unwrap();
            prop_assert_eq!(count, num_inserts, "Count mismatch after {} inserts", num_inserts);
            Ok(())
        });
        result?;
    }

    #[test]
    fn prop_instant_count_consistency(
        dimension in 16usize..=256,
        num_inserts in 1usize..=100,
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result: Result<(), TestCaseError> = runtime.block_on(async {
            let config = InstantDistanceConfig::balanced(dimension, DistanceMetric::L2);
            let index = InstantDistanceIndex::new(config).unwrap();

            // Insert N vectors
            for i in 0..num_inserts {
                let (doc, _) = make_test_document(dimension, i as u64);
                index.insert(doc).await.unwrap();
            }

            // Count should equal N
            let count = index.count().await.unwrap();
            prop_assert_eq!(count, num_inserts, "Count mismatch after {} inserts", num_inserts);
            Ok(())
        });
        result?;
    }
}

// ============================================================================
// Property 4: Delete Correctness
// ============================================================================

proptest! {
    #[test]
    fn prop_brute_force_delete_decreases_count(
        dimension in 16usize..=256,
        num_vectors in 2usize..=50,
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result: Result<(), TestCaseError> = runtime.block_on(async {
            let index = BruteForceIndex::new(dimension, DistanceMetric::L2);

            // Insert vectors and remember IDs
            let mut doc_ids = Vec::new();
            for i in 0..num_vectors {
                let (doc, doc_id) = make_test_document(dimension, i as u64);
                doc_ids.push(doc_id);
                index.insert(doc).await.unwrap();
            }

            let count_before = index.count().await.unwrap();
            prop_assert_eq!(count_before, num_vectors);

            // Delete first vector
            let deleted_id = doc_ids[0];
            index.delete(deleted_id).await.unwrap();

            // Count should decrease by 1
            let count_after = index.count().await.unwrap();
            prop_assert_eq!(count_after, count_before - 1);

            // Deleted vector should not be retrievable
            let get_result = index.get(deleted_id).await.unwrap();
            prop_assert!(get_result.is_none(), "Deleted vector still retrievable");
            Ok(())
        });
        result?;
    }

    #[test]
    fn prop_instant_delete_decreases_count(
        dimension in 16usize..=256,
        num_vectors in 2usize..=50,
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result: Result<(), TestCaseError> = runtime.block_on(async {
            let config = InstantDistanceConfig::balanced(dimension, DistanceMetric::L2);
            let index = InstantDistanceIndex::new(config).unwrap();

            // Insert vectors and remember IDs
            let mut doc_ids = Vec::new();
            for i in 0..num_vectors {
                let (doc, doc_id) = make_test_document(dimension, i as u64);
                doc_ids.push(doc_id);
                index.insert(doc).await.unwrap();
            }

            let count_before = index.count().await.unwrap();
            prop_assert_eq!(count_before, num_vectors);

            // Delete first vector
            let deleted_id = doc_ids[0];
            index.delete(deleted_id).await.unwrap();

            // Count should decrease by 1
            let count_after = index.count().await.unwrap();
            prop_assert_eq!(count_after, count_before - 1);

            // Deleted vector should not be retrievable
            let get_result = index.get(deleted_id).await.unwrap();
            prop_assert!(get_result.is_none(), "Deleted vector still retrievable");
            Ok(())
        });
        result?;
    }
}

// ============================================================================
// Property 5: Search Quality (Finite Scores)
// ============================================================================

proptest! {
    #[test]
    fn prop_brute_force_search_finite_scores(
        dimension in 16usize..=256,
        num_vectors in 1usize..=50,
        k in 1usize..=10,
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result: Result<(), TestCaseError> = runtime.block_on(async {
            let index = BruteForceIndex::new(dimension, DistanceMetric::L2);

            // Insert vectors
            for i in 0..num_vectors {
                let (doc, _) = make_test_document(dimension, i as u64);
                index.insert(doc).await.unwrap();
            }

            // Search
            let query = make_test_vector(dimension, 999);
            let results = index.search(&query, k, None).await.unwrap();

            // All scores should be finite (no NaN, no infinity)
            for (i, result) in results.iter().enumerate() {
                prop_assert!(
                    result.score.is_finite(),
                    "Result[{}] has non-finite score: {}",
                    i, result.score
                );
            }
            Ok(())
        });
        result?;
    }

    #[test]
    fn prop_instant_search_finite_scores(
        dimension in 16usize..=256,
        num_vectors in 1usize..=50,
        k in 1usize..=10,
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result: Result<(), TestCaseError> = runtime.block_on(async {
            let config = InstantDistanceConfig::balanced(dimension, DistanceMetric::L2);
            let index = InstantDistanceIndex::new(config).unwrap();

            // Insert vectors
            for i in 0..num_vectors {
                let (doc, _) = make_test_document(dimension, i as u64);
                index.insert(doc).await.unwrap();
            }

            // Search
            let query = make_test_vector(dimension, 999);
            let results = index.search(&query, k, None).await.unwrap();

            // All scores should be finite (no NaN, no infinity)
            for (i, result) in results.iter().enumerate() {
                prop_assert!(
                    result.score.is_finite(),
                    "Result[{}] has non-finite score: {}",
                    i, result.score
                );
            }
            Ok(())
        });
        result?;
    }
}

// ============================================================================
// Property 6: Edge Cases
// ============================================================================

proptest! {
    #[test]
    fn prop_brute_force_empty_index(
        dimension in 16usize..=256,
        k in 1usize..=10,
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result: Result<(), TestCaseError> = runtime.block_on(async {
            let index = BruteForceIndex::new(dimension, DistanceMetric::L2);

            // Search on empty index should return empty results
            let query = make_test_vector(dimension, 999);
            let results = index.search(&query, k, None).await.unwrap();
            prop_assert!(results.is_empty(), "Empty index returned results");

            // Count should be 0
            let count = index.count().await.unwrap();
            prop_assert_eq!(count, 0, "Empty index has non-zero count");
            Ok(())
        });
        result?;
    }

    #[test]
    fn prop_instant_empty_index(
        dimension in 16usize..=256,
        k in 1usize..=10,
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result: Result<(), TestCaseError> = runtime.block_on(async {
            let config = InstantDistanceConfig::balanced(dimension, DistanceMetric::L2);
            let index = InstantDistanceIndex::new(config).unwrap();

            // Search on empty index should return empty results
            let query = make_test_vector(dimension, 999);
            let results = index.search(&query, k, None).await.unwrap();
            prop_assert!(results.is_empty(), "Empty index returned results");

            // Count should be 0
            let count = index.count().await.unwrap();
            prop_assert_eq!(count, 0, "Empty index has non-zero count");
            Ok(())
        });
        result?;
    }

    #[test]
    fn prop_brute_force_single_vector(
        dimension in 16usize..=256,
        k in 1usize..=5,
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result: Result<(), TestCaseError> = runtime.block_on(async {
            let index = BruteForceIndex::new(dimension, DistanceMetric::L2);

            // Insert single vector
            let (doc, doc_id) = make_test_document(dimension, 42);
            index.insert(doc).await.unwrap();

            // Search should return that vector
            let query = make_test_vector(dimension, 999);
            let results = index.search(&query, k, None).await.unwrap();
            prop_assert_eq!(results.len(), 1, "Single vector index returned {} results", results.len());
            prop_assert_eq!(results[0].doc_id, doc_id, "Single vector search returned wrong document");
            Ok(())
        });
        result?;
    }

    #[test]
    fn prop_instant_single_vector(
        dimension in 16usize..=256,
        k in 1usize..=5,
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result: Result<(), TestCaseError> = runtime.block_on(async {
            let config = InstantDistanceConfig::balanced(dimension, DistanceMetric::L2);
            let index = InstantDistanceIndex::new(config).unwrap();

            // Insert single vector
            let (doc, doc_id) = make_test_document(dimension, 42);
            index.insert(doc).await.unwrap();

            // Search should return that vector
            let query = make_test_vector(dimension, 999);
            let results = index.search(&query, k, None).await.unwrap();
            prop_assert_eq!(results.len(), 1, "Single vector index returned {} results", results.len());
            prop_assert_eq!(results[0].doc_id, doc_id, "Single vector search returned wrong document");
            Ok(())
        });
        result?;
    }
}

// ============================================================================
// Property 7: Clear Operation
// ============================================================================

proptest! {
    #[test]
    fn prop_brute_force_clear_empties_index(
        dimension in 16usize..=256,
        num_vectors in 1usize..=50,
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result: Result<(), TestCaseError> = runtime.block_on(async {
            let index = BruteForceIndex::new(dimension, DistanceMetric::L2);

            // Insert vectors
            for i in 0..num_vectors {
                let (doc, _) = make_test_document(dimension, i as u64);
                index.insert(doc).await.unwrap();
            }

            prop_assert_eq!(index.count().await.unwrap(), num_vectors);

            // Clear index
            index.clear().await.unwrap();

            // Count should be 0
            let count = index.count().await.unwrap();
            prop_assert_eq!(count, 0, "Clear did not empty index");

            // Search should return empty
            let query = make_test_vector(dimension, 999);
            let results = index.search(&query, 10, None).await.unwrap();
            prop_assert!(results.is_empty(), "Cleared index returned search results");
            Ok(())
        });
        result?;
    }

    #[test]
    fn prop_instant_clear_empties_index(
        dimension in 16usize..=256,
        num_vectors in 1usize..=50,
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result: Result<(), TestCaseError> = runtime.block_on(async {
            let config = InstantDistanceConfig::balanced(dimension, DistanceMetric::L2);
            let index = InstantDistanceIndex::new(config).unwrap();

            // Insert vectors
            for i in 0..num_vectors {
                let (doc, _) = make_test_document(dimension, i as u64);
                index.insert(doc).await.unwrap();
            }

            prop_assert_eq!(index.count().await.unwrap(), num_vectors);

            // Clear index
            index.clear().await.unwrap();

            // Count should be 0
            let count = index.count().await.unwrap();
            prop_assert_eq!(count, 0, "Clear did not empty index");

            // Search should return empty
            let query = make_test_vector(dimension, 999);
            let results = index.search(&query, 10, None).await.unwrap();
            prop_assert!(results.is_empty(), "Cleared index returned search results");
            Ok(())
        });
        result?;
    }
}
