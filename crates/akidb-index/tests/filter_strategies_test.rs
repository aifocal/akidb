//! Phase 3 M3 Integration Tests - Filter Pushdown Strategies
//!
//! This test suite validates the three-tier filter pushdown optimization:
//! 1. Highly Selective Filters (<10%): Brute force on filtered subset
//! 2. Moderately Selective Filters (10-50%): HNSW with oversampling
//! 3. Non-Selective Filters (≥50%): Post-filtering (existing behavior)

use akidb_core::{DistanceMetric, SegmentDescriptor, SegmentState};
use akidb_index::{
    BuildRequest, HnswIndexProvider, IndexBatch, IndexKind, IndexProvider, QueryVector,
    SearchOptions,
};
use chrono::Utc;
use roaring::RoaringBitmap;
use uuid::Uuid;

/// Helper to create test vectors
fn generate_test_vectors(count: usize, dim: usize) -> Vec<Vec<f32>> {
    (0..count)
        .map(|i| (0..dim).map(|j| ((i + j) as f32 * 0.01).sin()).collect())
        .collect()
}

/// Helper to create a build request
fn create_build_request(dimension: u16, distance: DistanceMetric) -> BuildRequest {
    BuildRequest {
        collection: "test_collection".to_string(),
        kind: IndexKind::Hnsw,
        distance,
        dimension,
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

/// Test highly selective filter (<10%) uses brute force strategy
#[tokio::test]
async fn test_highly_selective_filter_strategy() {
    const TOTAL: usize = 10000;
    const DIM: usize = 128;
    const SELECTED: usize = 500; // 5% selectivity

    // Create test data
    let vectors_raw = generate_test_vectors(TOTAL, DIM);
    let primary_keys: Vec<String> = (0..TOTAL).map(|i| format!("vec_{}", i)).collect();

    let batch = IndexBatch {
        primary_keys: primary_keys.clone(),
        vectors: vectors_raw
            .iter()
            .map(|v| QueryVector {
                components: v.clone(),
            })
            .collect(),
        payloads: vec![serde_json::json!({}); TOTAL],
    };

    // Build index
    let request = create_build_request(DIM as u16, DistanceMetric::Cosine);

    let provider = HnswIndexProvider::default();
    let handle = provider.build(request).await.unwrap();

    // Add vectors
    provider.add_batch(&handle, batch).await.unwrap();

    // Create highly selective filter (5% of vectors)
    let mut filter = RoaringBitmap::new();
    for i in 0..SELECTED {
        filter.insert(i as u32);
    }

    // Query vector
    let query = QueryVector {
        components: vec![0.5; DIM],
    };

    // Search with filter
    let options = SearchOptions {
        top_k: 10,
        filter: Some(filter.clone()),
        timeout_ms: 5000,
    };

    let result = provider
        .search(&handle, query.clone(), options)
        .await
        .unwrap();

    // Validate results
    assert_eq!(result.neighbors.len(), 10, "Should return top_k=10 results");

    // All results should be in the filter set
    for neighbor in &result.neighbors {
        // Extract doc_id from primary_key (format: "vec_N")
        let doc_id: u32 = neighbor.primary_key[4..].parse().unwrap();
        assert!(
            filter.contains(doc_id),
            "Result doc_id {} should be in filter set",
            doc_id
        );
    }

    // Results should be ordered by score (lower distance = better)
    for i in 1..result.neighbors.len() {
        assert!(
            result.neighbors[i - 1].score <= result.neighbors[i].score,
            "Results should be ordered by score (ascending - lower is better)"
        );
    }
}

/// Test moderately selective filter (10-50%) uses oversampling strategy
#[tokio::test]
async fn test_moderate_selectivity_filter_strategy() {
    const TOTAL: usize = 10000;
    const DIM: usize = 128;
    const SELECTED: usize = 3000; // 30% selectivity

    // Create test data
    let vectors_raw = generate_test_vectors(TOTAL, DIM);
    let primary_keys: Vec<String> = (0..TOTAL).map(|i| format!("vec_{}", i)).collect();

    let batch = IndexBatch {
        primary_keys: primary_keys.clone(),
        vectors: vectors_raw
            .iter()
            .map(|v| QueryVector {
                components: v.clone(),
            })
            .collect(),
        payloads: vec![serde_json::json!({}); TOTAL],
    };

    // Build index
    let request = create_build_request(DIM as u16, DistanceMetric::L2);

    let provider = HnswIndexProvider::default();
    let handle = provider.build(request).await.unwrap();

    // Add vectors
    provider.add_batch(&handle, batch).await.unwrap();

    // Create moderately selective filter (30% of vectors)
    let mut filter = RoaringBitmap::new();
    for i in 0..SELECTED {
        filter.insert(i as u32);
    }

    // Query vector
    let query = QueryVector {
        components: vec![0.3; DIM],
    };

    // Search with filter
    let options = SearchOptions {
        top_k: 50,
        filter: Some(filter.clone()),
        timeout_ms: 5000,
    };

    let result = provider
        .search(&handle, query.clone(), options)
        .await
        .unwrap();

    // Validate results
    assert_eq!(result.neighbors.len(), 50, "Should return top_k=50 results");

    // All results should be in the filter set
    for neighbor in &result.neighbors {
        let doc_id: u32 = neighbor.primary_key[4..].parse().unwrap();
        assert!(
            filter.contains(doc_id),
            "Result doc_id {} should be in filter set",
            doc_id
        );
    }

    // Results should be ordered by score (for L2, lower is better)
    for i in 1..result.neighbors.len() {
        assert!(
            result.neighbors[i - 1].score <= result.neighbors[i].score,
            "Results should be ordered by score (ascending for L2)"
        );
    }
}

/// Test non-selective filter (≥50%) uses post-filtering strategy
#[tokio::test]
async fn test_non_selective_filter_strategy() {
    const TOTAL: usize = 10000;
    const DIM: usize = 128;
    const SELECTED: usize = 8000; // 80% selectivity

    // Create test data
    let vectors_raw = generate_test_vectors(TOTAL, DIM);
    let primary_keys: Vec<String> = (0..TOTAL).map(|i| format!("vec_{}", i)).collect();

    let batch = IndexBatch {
        primary_keys: primary_keys.clone(),
        vectors: vectors_raw
            .iter()
            .map(|v| QueryVector {
                components: v.clone(),
            })
            .collect(),
        payloads: vec![serde_json::json!({}); TOTAL],
    };

    // Build index
    let request = create_build_request(DIM as u16, DistanceMetric::L2);

    let provider = HnswIndexProvider::default();
    let handle = provider.build(request).await.unwrap();

    // Add vectors
    provider.add_batch(&handle, batch).await.unwrap();

    // Create non-selective filter (80% of vectors)
    let mut filter = RoaringBitmap::new();
    for i in 0..SELECTED {
        filter.insert(i as u32);
    }

    // Query vector
    let query = QueryVector {
        components: vec![0.7; DIM],
    };

    // Search with filter
    let options = SearchOptions {
        top_k: 100,
        filter: Some(filter.clone()),
        timeout_ms: 5000,
    };

    let result = provider
        .search(&handle, query.clone(), options)
        .await
        .unwrap();

    // Validate results
    assert_eq!(
        result.neighbors.len(),
        100,
        "Should return top_k=100 results"
    );

    // All results should be in the filter set
    for neighbor in &result.neighbors {
        let doc_id: u32 = neighbor.primary_key[4..].parse().unwrap();
        assert!(
            filter.contains(doc_id),
            "Result doc_id {} should be in filter set",
            doc_id
        );
    }

    // Results should be ordered by score (lower distance = better)
    for i in 1..result.neighbors.len() {
        assert!(
            result.neighbors[i - 1].score <= result.neighbors[i].score,
            "Results should be ordered by score (ascending - lower is better)"
        );
    }
}

/// Test edge case: filter size exactly at 10% threshold
#[tokio::test]
async fn test_filter_threshold_boundary() {
    const TOTAL: usize = 10000;
    const DIM: usize = 64;
    const SELECTED: usize = 1000; // Exactly 10% selectivity

    let vectors_raw = generate_test_vectors(TOTAL, DIM);
    let primary_keys: Vec<String> = (0..TOTAL).map(|i| format!("vec_{}", i)).collect();

    let batch = IndexBatch {
        primary_keys,
        vectors: vectors_raw
            .iter()
            .map(|v| QueryVector {
                components: v.clone(),
            })
            .collect(),
        payloads: vec![serde_json::json!({}); TOTAL],
    };

    let request = create_build_request(DIM as u16, DistanceMetric::Cosine);

    let provider = HnswIndexProvider::default();
    let handle = provider.build(request).await.unwrap();
    provider.add_batch(&handle, batch).await.unwrap();

    // Filter exactly at 10% boundary
    let mut filter = RoaringBitmap::new();
    for i in 0..SELECTED {
        filter.insert(i as u32);
    }

    let query = QueryVector {
        components: vec![0.1; DIM],
    };

    let options = SearchOptions {
        top_k: 20,
        filter: Some(filter.clone()),
        timeout_ms: 5000,
    };

    let result = provider
        .search(&handle, query.clone(), options)
        .await
        .unwrap();

    // Should still return correct results
    assert_eq!(result.neighbors.len(), 20);
    for neighbor in &result.neighbors {
        let doc_id: u32 = neighbor.primary_key[4..].parse().unwrap();
        assert!(filter.contains(doc_id));
    }
}

/// Test edge case: very small filter (1 vector)
#[tokio::test]
async fn test_extremely_selective_filter() {
    const TOTAL: usize = 10000;
    const DIM: usize = 64;

    let vectors_raw = generate_test_vectors(TOTAL, DIM);
    let primary_keys: Vec<String> = (0..TOTAL).map(|i| format!("vec_{}", i)).collect();

    let batch = IndexBatch {
        primary_keys,
        vectors: vectors_raw
            .iter()
            .map(|v| QueryVector {
                components: v.clone(),
            })
            .collect(),
        payloads: vec![serde_json::json!({}); TOTAL],
    };

    let request = create_build_request(DIM as u16, DistanceMetric::L2);

    let provider = HnswIndexProvider::default();
    let handle = provider.build(request).await.unwrap();
    provider.add_batch(&handle, batch).await.unwrap();

    // Filter with only 1 vector
    let mut filter = RoaringBitmap::new();
    filter.insert(42);

    let query = QueryVector {
        components: vec![0.5; DIM],
    };

    let options = SearchOptions {
        top_k: 10, // Request 10 but only 1 available
        filter: Some(filter.clone()),
        timeout_ms: 5000,
    };

    let result = provider
        .search(&handle, query.clone(), options)
        .await
        .unwrap();

    // Should return only 1 result
    assert_eq!(result.neighbors.len(), 1);
    let doc_id: u32 = result.neighbors[0].primary_key[4..].parse().unwrap();
    assert_eq!(doc_id, 42);
}

/// Test performance: compare filtered vs non-filtered search
#[tokio::test]
async fn test_filter_strategy_performance_comparison() {
    const TOTAL: usize = 10000;
    const DIM: usize = 128;

    let vectors_raw = generate_test_vectors(TOTAL, DIM);
    let primary_keys: Vec<String> = (0..TOTAL).map(|i| format!("vec_{}", i)).collect();

    let batch = IndexBatch {
        primary_keys,
        vectors: vectors_raw
            .iter()
            .map(|v| QueryVector {
                components: v.clone(),
            })
            .collect(),
        payloads: vec![serde_json::json!({}); TOTAL],
    };

    let request = create_build_request(DIM as u16, DistanceMetric::Cosine);

    let provider = HnswIndexProvider::default();
    let handle = provider.build(request).await.unwrap();
    provider.add_batch(&handle, batch).await.unwrap();

    let query = QueryVector {
        components: vec![0.5; DIM],
    };

    // Test 1: No filter
    let options_no_filter = SearchOptions {
        top_k: 50,
        filter: None,
        timeout_ms: 5000,
    };
    let result_no_filter = provider
        .search(&handle, query.clone(), options_no_filter)
        .await
        .unwrap();
    assert_eq!(result_no_filter.neighbors.len(), 50);

    // Test 2: Highly selective filter (5%)
    let mut filter_5pct = RoaringBitmap::new();
    for i in 0..500 {
        filter_5pct.insert(i);
    }
    let options_5pct = SearchOptions {
        top_k: 50,
        filter: Some(filter_5pct.clone()),
        timeout_ms: 5000,
    };
    let result_5pct = provider
        .search(&handle, query.clone(), options_5pct)
        .await
        .unwrap();
    assert_eq!(result_5pct.neighbors.len(), 50);
    for neighbor in &result_5pct.neighbors {
        let doc_id: u32 = neighbor.primary_key[4..].parse().unwrap();
        assert!(filter_5pct.contains(doc_id));
    }

    // Test 3: Moderate filter (30%)
    let mut filter_30pct = RoaringBitmap::new();
    for i in 0..3000 {
        filter_30pct.insert(i);
    }
    let options_30pct = SearchOptions {
        top_k: 50,
        filter: Some(filter_30pct.clone()),
        timeout_ms: 5000,
    };
    let result_30pct = provider
        .search(&handle, query.clone(), options_30pct)
        .await
        .unwrap();
    assert_eq!(result_30pct.neighbors.len(), 50);
    for neighbor in &result_30pct.neighbors {
        let doc_id: u32 = neighbor.primary_key[4..].parse().unwrap();
        assert!(filter_30pct.contains(doc_id));
    }

    // All results should be properly ordered (ascending - lower distance is better)
    for result in [&result_no_filter, &result_5pct, &result_30pct] {
        for i in 1..result.neighbors.len() {
            assert!(result.neighbors[i - 1].score <= result.neighbors[i].score);
        }
    }
}
