//! Contract tests for IndexProvider trait
//!
//! These tests ensure that all implementations of IndexProvider conform to
//! the expected behavior defined by the trait contract.

#![allow(clippy::expect_fun_call)]

use akidb_core::{DistanceMetric, SegmentDescriptor, SegmentState};
use akidb_index::{
    HnswIndexProvider, IndexProvider, NativeIndexProvider, QueryVector, SearchOptions,
};
use akidb_index::{BuildRequest, IndexBatch, IndexKind};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

/// Helper to create test providers
fn create_providers() -> Vec<(&'static str, Box<dyn IndexProvider>)> {
    vec![
        ("Native", Box::new(NativeIndexProvider::new())),
        (
            "HNSW",
            Box::new(HnswIndexProvider::new(Default::default())),
        ),
    ]
}

/// Helper to create a build request with specified dimension
fn create_build_request(dimension: u16, distance: DistanceMetric) -> BuildRequest {
    BuildRequest {
        collection: "test_collection".to_string(),
        kind: IndexKind::Native, // Kind doesn't matter for these tests
        distance,
        segments: vec![SegmentDescriptor {
            segment_id: Uuid::new_v4(),
            collection: "test_collection".to_string(),
            vector_dim: dimension,
            record_count: 0,
            state: SegmentState::Active,
            lsn_range: 0..=0,
            compression_level: 0,
            created_at: Utc::now(),
        }],
    }
}

/// Contract Test 1: Both providers should reject dimension=0
#[tokio::test]
async fn contract_reject_zero_dimension() {
    for (name, provider) in create_providers() {
        let request = BuildRequest {
            collection: "test".to_string(),
            kind: provider.kind(),
            distance: DistanceMetric::Cosine,
            segments: vec![],
        };

        let result = provider.build(request).await;
        assert!(
            result.is_err(),
            "{} provider should reject dimension=0",
            name
        );
    }
}

/// Contract Test 2: Both providers should handle empty index search gracefully
#[tokio::test]
async fn contract_empty_index_search() {
    for (name, provider) in create_providers() {
        let request = create_build_request(3, DistanceMetric::Cosine);
        let handle = provider
            .build(request)
            .await
            .expect(&format!("{} should build index", name));

        // Search empty index
        let query = QueryVector {
            components: vec![1.0, 0.0, 0.0],
        };

        let options = SearchOptions {
            top_k: 10,
            filter: None,
            timeout_ms: 1000,
        };

        let result = provider
            .search(&handle, query, options)
            .await
            .expect(&format!("{} should handle empty search", name));

        assert_eq!(
            result.neighbors.len(),
            0,
            "{} should return empty results for empty index",
            name
        );
    }
}

/// Contract Test 3: Both providers should serialize/deserialize correctly
#[tokio::test]
async fn contract_roundtrip_serialization() {
    for (name, provider) in create_providers() {
        let request = create_build_request(2, DistanceMetric::L2);
        let handle = provider
            .build(request)
            .await
            .expect(&format!("{} should build index", name));

        // Add some vectors
        let batch = IndexBatch {
            primary_keys: vec!["key1".to_string(), "key2".to_string()],
            vectors: vec![
                QueryVector {
                    components: vec![1.0, 0.0],
                },
                QueryVector {
                    components: vec![0.0, 1.0],
                },
            ],
            payloads: vec![json!({"id": 1}), json!({"id": 2})],
        };

        provider
            .add_batch(&handle, batch)
            .await
            .expect(&format!("{} should add batch", name));

        // Serialize
        let serialized = provider
            .serialize(&handle)
            .expect(&format!("{} should serialize", name));

        assert!(
            !serialized.is_empty(),
            "{} serialized data should not be empty",
            name
        );

        // Deserialize
        let new_handle = provider
            .deserialize(&serialized)
            .expect(&format!("{} should deserialize", name));

        assert_eq!(
            new_handle.dimension, handle.dimension,
            "{} should preserve dimension",
            name
        );
        assert_eq!(
            new_handle.kind, handle.kind,
            "{} should preserve kind",
            name
        );

        // Verify search works on deserialized index
        let query = QueryVector {
            components: vec![1.0, 0.0],
        };

        let options = SearchOptions {
            top_k: 1,
            filter: None,
            timeout_ms: 1000,
        };

        let result = provider
            .search(&new_handle, query, options)
            .await
            .expect(&format!(
                "{} should search on deserialized index",
                name
            ));

        assert_eq!(
            result.neighbors.len(),
            1,
            "{} should return results after deserialization",
            name
        );
    }
}

/// Contract Test 4: Both providers should extract data for persistence
#[tokio::test]
async fn contract_extract_for_persistence() {
    for (name, provider) in create_providers() {
        let request = create_build_request(3, DistanceMetric::Cosine);
        let handle = provider
            .build(request)
            .await
            .expect(&format!("{} should build index", name));

        // Add vectors
        let batch = IndexBatch {
            primary_keys: vec![
                "key1".to_string(),
                "key2".to_string(),
                "key3".to_string(),
            ],
            vectors: vec![
                QueryVector {
                    components: vec![1.0, 0.0, 0.0],
                },
                QueryVector {
                    components: vec![0.0, 1.0, 0.0],
                },
                QueryVector {
                    components: vec![0.0, 0.0, 1.0],
                },
            ],
            payloads: vec![
                json!({"id": 1}),
                json!({"id": 2}),
                json!({"id": 3}),
            ],
        };

        provider
            .add_batch(&handle, batch)
            .await
            .expect(&format!("{} should add batch", name));

        // Extract for persistence
        let (vectors, payloads) = provider
            .extract_for_persistence(&handle)
            .expect(&format!("{} should extract data", name));

        assert_eq!(
            vectors.len(),
            3,
            "{} should extract correct number of vectors",
            name
        );
        assert_eq!(
            payloads.len(),
            3,
            "{} should extract correct number of payloads",
            name
        );

        // Verify vector dimensions
        for (i, vec) in vectors.iter().enumerate() {
            assert_eq!(
                vec.len(),
                3,
                "{} vector {} should have correct dimension",
                name,
                i
            );
        }

        // Verify payloads are JSON objects
        for (i, payload) in payloads.iter().enumerate() {
            assert!(
                payload.is_object(),
                "{} payload {} should be JSON object",
                name,
                i
            );
        }
    }
}

/// Contract Test 5: Both providers should reject mismatched dimensions
#[tokio::test]
async fn contract_dimension_validation() {
    for (name, provider) in create_providers() {
        let request = create_build_request(3, DistanceMetric::Cosine);
        let handle = provider
            .build(request)
            .await
            .expect(&format!("{} should build index", name));

        // Try to add vectors with wrong dimension
        let wrong_batch = IndexBatch {
            primary_keys: vec!["key1".to_string()],
            vectors: vec![QueryVector {
                components: vec![1.0, 0.0], // Wrong: 2 dimensions instead of 3
            }],
            payloads: vec![json!({"id": 1})],
        };

        let result = provider.add_batch(&handle, wrong_batch).await;
        assert!(
            result.is_err(),
            "{} should reject mismatched dimension on add_batch",
            name
        );

        // Try to search with wrong dimension
        let wrong_query = QueryVector {
            components: vec![1.0, 0.0, 0.0, 0.0], // Wrong: 4 dimensions instead of 3
        };

        let options = SearchOptions {
            top_k: 10,
            filter: None,
            timeout_ms: 1000,
        };

        let result = provider.search(&handle, wrong_query, options).await;
        assert!(
            result.is_err(),
            "{} should reject mismatched dimension on search",
            name
        );
    }
}

/// Contract Test 6: Both providers should reject duplicate keys
#[tokio::test]
async fn contract_reject_duplicate_keys() {
    for (name, provider) in create_providers() {
        let request = create_build_request(2, DistanceMetric::L2);
        let handle = provider
            .build(request)
            .await
            .expect(&format!("{} should build index", name));

        // Add initial batch
        let batch1 = IndexBatch {
            primary_keys: vec!["key1".to_string()],
            vectors: vec![QueryVector {
                components: vec![1.0, 0.0],
            }],
            payloads: vec![json!({"id": 1})],
        };

        provider
            .add_batch(&handle, batch1)
            .await
            .expect(&format!("{} should add first batch", name));

        // Try to add duplicate key
        let batch2 = IndexBatch {
            primary_keys: vec!["key1".to_string()], // Duplicate!
            vectors: vec![QueryVector {
                components: vec![0.0, 1.0],
            }],
            payloads: vec![json!({"id": 2})],
        };

        let result = provider.add_batch(&handle, batch2).await;
        assert!(
            result.is_err(),
            "{} should reject duplicate primary keys",
            name
        );
    }
}

/// Contract Test 7: Both providers should validate batch consistency
#[tokio::test]
async fn contract_batch_consistency() {
    for (name, provider) in create_providers() {
        let request = create_build_request(2, DistanceMetric::Cosine);
        let handle = provider
            .build(request)
            .await
            .expect(&format!("{} should build index", name));

        // Try batch with mismatched array lengths
        let inconsistent_batch = IndexBatch {
            primary_keys: vec!["key1".to_string(), "key2".to_string()], // 2 keys
            vectors: vec![QueryVector {
                components: vec![1.0, 0.0],
            }], // Only 1 vector!
            payloads: vec![json!({"id": 1})], // Only 1 payload!
        };

        let result = provider.add_batch(&handle, inconsistent_batch).await;
        assert!(
            result.is_err(),
            "{} should reject batch with inconsistent array lengths",
            name
        );
    }
}

/// Contract Test 8: Both providers should return scored results in correct order
#[tokio::test]
async fn contract_search_result_ordering() {
    for (name, provider) in create_providers() {
        let request = create_build_request(3, DistanceMetric::L2);
        let handle = provider
            .build(request)
            .await
            .expect(&format!("{} should build index", name));

        // Add vectors with known distances from query [1, 0, 0]
        let batch = IndexBatch {
            primary_keys: vec![
                "closest".to_string(),    // [1, 0, 0] - distance 0
                "mid".to_string(),        // [0.5, 0.5, 0] - distance ~0.707
                "farthest".to_string(),   // [0, 1, 0] - distance ~1.414
            ],
            vectors: vec![
                QueryVector {
                    components: vec![1.0, 0.0, 0.0],
                },
                QueryVector {
                    components: vec![0.5, 0.5, 0.0],
                },
                QueryVector {
                    components: vec![0.0, 1.0, 0.0],
                },
            ],
            payloads: vec![json!({"id": 1}), json!({"id": 2}), json!({"id": 3})],
        };

        provider
            .add_batch(&handle, batch)
            .await
            .expect(&format!("{} should add batch", name));

        // Search with query [1, 0, 0]
        let query = QueryVector {
            components: vec![1.0, 0.0, 0.0],
        };

        let options = SearchOptions {
            top_k: 3,
            filter: None,
            timeout_ms: 1000,
        };

        let result = provider
            .search(&handle, query, options)
            .await
            .expect(&format!("{} should search", name));

        assert_eq!(
            result.neighbors.len(),
            3,
            "{} should return all 3 results",
            name
        );

        // For L2 distance, results should be ordered by increasing distance
        assert_eq!(
            result.neighbors[0].primary_key, "closest",
            "{} should return closest vector first",
            name
        );
        assert_eq!(
            result.neighbors[2].primary_key, "farthest",
            "{} should return farthest vector last",
            name
        );

        // Verify scores are in ascending order (for L2)
        assert!(
            result.neighbors[0].score <= result.neighbors[1].score,
            "{} results should be sorted by ascending score",
            name
        );
        assert!(
            result.neighbors[1].score <= result.neighbors[2].score,
            "{} results should be sorted by ascending score",
            name
        );
    }
}
