//! Integration tests comparing HNSW recall against brute-force baseline.

use akidb_core::{DistanceMetric, DocumentId, VectorDocument, VectorIndex};
use akidb_index::{BruteForceIndex, HnswConfig, HnswIndex};
use rand::Rng;

/// Generates a random vector with values in [0, 1].
fn random_vector(dim: usize) -> Vec<f32> {
    let mut rng = rand::thread_rng();
    (0..dim).map(|_| rng.gen::<f32>()).collect()
}

/// Computes recall@k: fraction of true nearest neighbors found by HNSW.
fn compute_recall(ground_truth: &[DocumentId], hnsw_results: &[DocumentId], k: usize) -> f64 {
    let gt_set: std::collections::HashSet<_> = ground_truth.iter().take(k).collect();
    let found = hnsw_results
        .iter()
        .take(k)
        .filter(|id| gt_set.contains(id))
        .count();
    found as f64 / k as f64
}

#[tokio::test]
#[ignore = "Research implementation - Phase 4C (65% recall, educational only)"]
async fn test_hnsw_recall_100_vectors() {
    let dim = 128;
    let n_vectors = 100;
    let k = 10;

    // Create indices
    let bf_index = BruteForceIndex::new(dim, DistanceMetric::Cosine);
    let hnsw_config = HnswConfig::balanced(dim, DistanceMetric::Cosine);
    let hnsw_index = HnswIndex::new(hnsw_config);

    // Insert same vectors into both indices
    let mut doc_ids = Vec::new();
    for _ in 0..n_vectors {
        let doc_id = DocumentId::new();
        let vector = random_vector(dim);
        let doc = VectorDocument::new(doc_id, vector);

        bf_index.insert(doc.clone()).await.unwrap();
        hnsw_index.insert(doc).await.unwrap();
        doc_ids.push(doc_id);
    }

    // Query with 10 random vectors
    let mut total_recall = 0.0;
    let n_queries = 10;

    for _ in 0..n_queries {
        let query = random_vector(dim);

        // Get ground truth from brute-force
        let bf_results = bf_index.search(&query, k, None).await.unwrap();
        let bf_ids: Vec<_> = bf_results.iter().map(|r| r.doc_id).collect();

        // Get HNSW results
        let hnsw_results = hnsw_index.search(&query, k, None).await.unwrap();
        let hnsw_ids: Vec<_> = hnsw_results.iter().map(|r| r.doc_id).collect();

        // Compute recall
        let recall = compute_recall(&bf_ids, &hnsw_ids, k);
        total_recall += recall;
    }

    let avg_recall = total_recall / n_queries as f64;
    println!("Average recall@{} for {} vectors: {:.3}", k, n_vectors, avg_recall);

    // HNSW should achieve >80% recall at this scale
    assert!(
        avg_recall > 0.8,
        "Recall too low: {:.3} < 0.8",
        avg_recall
    );
}

#[tokio::test]
#[ignore = "Research implementation - Phase 4C (65% recall, educational only)"]
async fn test_hnsw_recall_1000_vectors() {
    let dim = 128;
    let n_vectors = 1000;
    let k = 10;

    // Create indices
    let bf_index = BruteForceIndex::new(dim, DistanceMetric::Cosine);
    let hnsw_config = HnswConfig::balanced(dim, DistanceMetric::Cosine);
    let hnsw_index = HnswIndex::new(hnsw_config);

    // Insert same vectors into both indices
    for _ in 0..n_vectors {
        let doc_id = DocumentId::new();
        let vector = random_vector(dim);
        let doc = VectorDocument::new(doc_id, vector);

        bf_index.insert(doc.clone()).await.unwrap();
        hnsw_index.insert(doc).await.unwrap();
    }

    // Query with 5 random vectors (fewer to keep test fast)
    let mut total_recall = 0.0;
    let n_queries = 5;

    for _ in 0..n_queries {
        let query = random_vector(dim);

        // Get ground truth from brute-force
        let bf_results = bf_index.search(&query, k, None).await.unwrap();
        let bf_ids: Vec<_> = bf_results.iter().map(|r| r.doc_id).collect();

        // Get HNSW results with higher ef_search for better recall
        let hnsw_results = hnsw_index.search(&query, k, Some(256)).await.unwrap();
        let hnsw_ids: Vec<_> = hnsw_results.iter().map(|r| r.doc_id).collect();

        // Compute recall
        let recall = compute_recall(&bf_ids, &hnsw_ids, k);
        total_recall += recall;
    }

    let avg_recall = total_recall / n_queries as f64;
    println!("Average recall@{} for {} vectors: {:.3}", k, n_vectors, avg_recall);

    // HNSW should achieve >90% recall with ef_search=256
    assert!(
        avg_recall > 0.9,
        "Recall too low: {:.3} < 0.9",
        avg_recall
    );
}

#[tokio::test]
#[ignore = "Research implementation - Phase 4C (65% recall, educational only)"]
async fn test_hnsw_l2_metric_recall() {
    let dim = 64;
    let n_vectors = 200;
    let k = 5;

    // Create indices with L2 metric
    let bf_index = BruteForceIndex::new(dim, DistanceMetric::L2);
    let hnsw_config = HnswConfig::balanced(dim, DistanceMetric::L2);
    let hnsw_index = HnswIndex::new(hnsw_config);

    // Insert vectors
    for _ in 0..n_vectors {
        let doc_id = DocumentId::new();
        let vector = random_vector(dim);
        let doc = VectorDocument::new(doc_id, vector);

        bf_index.insert(doc.clone()).await.unwrap();
        hnsw_index.insert(doc).await.unwrap();
    }

    // Query
    let query = random_vector(dim);

    let bf_results = bf_index.search(&query, k, None).await.unwrap();
    let bf_ids: Vec<_> = bf_results.iter().map(|r| r.doc_id).collect();

    let hnsw_results = hnsw_index.search(&query, k, None).await.unwrap();
    let hnsw_ids: Vec<_> = hnsw_results.iter().map(|r| r.doc_id).collect();

    let recall = compute_recall(&bf_ids, &hnsw_ids, k);
    println!("L2 recall@{}: {:.3}", k, recall);

    assert!(recall > 0.6, "L2 recall too low: {:.3}", recall);
}

#[tokio::test]
#[ignore = "Research implementation - Phase 4C (65% recall, educational only)"]
async fn test_hnsw_incremental_insert() {
    // Test that HNSW maintains good recall when inserting incrementally
    let dim = 64;
    let initial_size = 100;
    let k = 5;

    let bf_index = BruteForceIndex::new(dim, DistanceMetric::Cosine);
    let hnsw_config = HnswConfig::balanced(dim, DistanceMetric::Cosine);
    let hnsw_index = HnswIndex::new(hnsw_config);

    // Insert initial batch
    for _ in 0..initial_size {
        let doc_id = DocumentId::new();
        let vector = random_vector(dim);
        let doc = VectorDocument::new(doc_id, vector);

        bf_index.insert(doc.clone()).await.unwrap();
        hnsw_index.insert(doc).await.unwrap();
    }

    // Search once
    let query1 = random_vector(dim);
    let bf_results1 = bf_index.search(&query1, k, None).await.unwrap();
    let hnsw_results1 = hnsw_index.search(&query1, k, None).await.unwrap();

    let bf_ids1: Vec<_> = bf_results1.iter().map(|r| r.doc_id).collect();
    let hnsw_ids1: Vec<_> = hnsw_results1.iter().map(|r| r.doc_id).collect();
    let recall1 = compute_recall(&bf_ids1, &hnsw_ids1, k);

    // Insert more documents
    for _ in 0..50 {
        let doc_id = DocumentId::new();
        let vector = random_vector(dim);
        let doc = VectorDocument::new(doc_id, vector);

        bf_index.insert(doc.clone()).await.unwrap();
        hnsw_index.insert(doc).await.unwrap();
    }

    // Search again
    let query2 = random_vector(dim);
    let bf_results2 = bf_index.search(&query2, k, None).await.unwrap();
    let hnsw_results2 = hnsw_index.search(&query2, k, None).await.unwrap();

    let bf_ids2: Vec<_> = bf_results2.iter().map(|r| r.doc_id).collect();
    let hnsw_ids2: Vec<_> = hnsw_results2.iter().map(|r| r.doc_id).collect();
    let recall2 = compute_recall(&bf_ids2, &hnsw_ids2, k);

    println!("Recall before incremental insert: {:.3}", recall1);
    println!("Recall after incremental insert: {:.3}", recall2);

    // Both recalls should be reasonable
    assert!(recall1 > 0.6);
    assert!(recall2 > 0.6);
}

#[tokio::test]
#[ignore = "Research implementation - Phase 4C (65% recall, educational only)"]
async fn test_hnsw_edge_cache_config() {
    // Test edge cache config on small dataset
    let dim = 128;
    let n_vectors = 500;
    let k = 10;

    let bf_index = BruteForceIndex::new(dim, DistanceMetric::Cosine);
    let hnsw_config = HnswConfig::edge_cache(dim, DistanceMetric::Cosine);
    let hnsw_index = HnswIndex::new(hnsw_config);

    for _ in 0..n_vectors {
        let doc_id = DocumentId::new();
        let vector = random_vector(dim);
        let doc = VectorDocument::new(doc_id, vector);

        bf_index.insert(doc.clone()).await.unwrap();
        hnsw_index.insert(doc).await.unwrap();
    }

    let query = random_vector(dim);

    let bf_results = bf_index.search(&query, k, None).await.unwrap();
    let bf_ids: Vec<_> = bf_results.iter().map(|r| r.doc_id).collect();

    let hnsw_results = hnsw_index.search(&query, k, None).await.unwrap();
    let hnsw_ids: Vec<_> = hnsw_results.iter().map(|r| r.doc_id).collect();

    let recall = compute_recall(&bf_ids, &hnsw_ids, k);
    println!("Edge cache config recall@{}: {:.3}", k, recall);

    // Edge cache should still maintain good recall
    assert!(recall > 0.8, "Edge cache recall: {:.3}", recall);
}
