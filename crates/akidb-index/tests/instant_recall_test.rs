//! Recall tests for InstantDistanceIndex (production HNSW).
//!
//! These tests validate that instant-distance achieves >95% recall.

use akidb_core::{DistanceMetric, DocumentId, VectorDocument, VectorIndex};
use akidb_index::{BruteForceIndex, InstantDistanceConfig, InstantDistanceIndex};
use rand::Rng;

/// Generates a random vector with values in [0, 1].
fn random_vector(dim: usize) -> Vec<f32> {
    let mut rng = rand::thread_rng();
    (0..dim).map(|_| rng.gen::<f32>()).collect()
}

/// Computes recall@k: fraction of true nearest neighbors found.
fn compute_recall(ground_truth: &[DocumentId], results: &[DocumentId], k: usize) -> f64 {
    let gt_set: std::collections::HashSet<_> = ground_truth.iter().take(k).collect();
    let found = results
        .iter()
        .take(k)
        .filter(|id| gt_set.contains(id))
        .count();
    found as f64 / k as f64
}

#[tokio::test]
async fn test_instant_distance_recall_100_vectors() {
    let dim = 128;
    let n_vectors = 100;
    let k = 10;

    // Create indices
    let bf_index = BruteForceIndex::new(dim, DistanceMetric::Cosine);
    let instant_config = InstantDistanceConfig::balanced(dim, DistanceMetric::Cosine);
    let instant_index = InstantDistanceIndex::new(instant_config).unwrap();

    // Insert same vectors into both indices
    for _ in 0..n_vectors {
        let doc_id = DocumentId::new();
        let vector = random_vector(dim);
        let doc = VectorDocument::new(doc_id, vector);

        bf_index.insert(doc.clone()).await.unwrap();
        instant_index.insert(doc).await.unwrap();
    }

    // Rebuild instant-distance index before searching
    instant_index.force_rebuild().await.unwrap();

    // Query with 10 random vectors
    let mut total_recall = 0.0;
    let n_queries = 10;

    for _ in 0..n_queries {
        let query = random_vector(dim);

        // Get ground truth from brute-force
        let bf_results = bf_index.search(&query, k, None).await.unwrap();
        let bf_ids: Vec<_> = bf_results.iter().map(|r| r.doc_id).collect();

        // Get instant-distance results
        let instant_results = instant_index.search(&query, k, None).await.unwrap();
        let instant_ids: Vec<_> = instant_results.iter().map(|r| r.doc_id).collect();

        // Compute recall
        let recall = compute_recall(&bf_ids, &instant_ids, k);
        total_recall += recall;
    }

    let avg_recall = total_recall / n_queries as f64;
    println!(
        "InstantDistance recall@{} for {} vectors: {:.3}",
        k, n_vectors, avg_recall
    );

    // instant-distance should achieve >95% recall
    assert!(
        avg_recall > 0.95,
        "Recall too low: {:.3} < 0.95",
        avg_recall
    );
}

#[tokio::test]
async fn test_instant_distance_recall_1000_vectors() {
    let dim = 128;
    let n_vectors = 1000;
    let k = 10;

    // Create indices
    let bf_index = BruteForceIndex::new(dim, DistanceMetric::Cosine);
    let instant_config = InstantDistanceConfig::balanced(dim, DistanceMetric::Cosine);
    let instant_index = InstantDistanceIndex::new(instant_config).unwrap();

    // Insert same vectors into both indices
    for _ in 0..n_vectors {
        let doc_id = DocumentId::new();
        let vector = random_vector(dim);
        let doc = VectorDocument::new(doc_id, vector);

        bf_index.insert(doc.clone()).await.unwrap();
        instant_index.insert(doc).await.unwrap();
    }

    // Rebuild instant-distance index before searching
    instant_index.force_rebuild().await.unwrap();

    // Query with 5 random vectors (fewer to keep test fast)
    let mut total_recall = 0.0;
    let n_queries = 5;

    for _ in 0..n_queries {
        let query = random_vector(dim);

        // Get ground truth from brute-force
        let bf_results = bf_index.search(&query, k, None).await.unwrap();
        let bf_ids: Vec<_> = bf_results.iter().map(|r| r.doc_id).collect();

        // Get instant-distance results
        let instant_results = instant_index.search(&query, k, None).await.unwrap();
        let instant_ids: Vec<_> = instant_results.iter().map(|r| r.doc_id).collect();

        // Compute recall
        let recall = compute_recall(&bf_ids, &instant_ids, k);
        total_recall += recall;
    }

    let avg_recall = total_recall / n_queries as f64;
    println!(
        "InstantDistance recall@{} for {} vectors: {:.3}",
        k, n_vectors, avg_recall
    );

    // instant-distance should achieve >95% recall even at 1000 vectors
    assert!(
        avg_recall > 0.95,
        "Recall too low: {:.3} < 0.95",
        avg_recall
    );
}

#[tokio::test]
async fn test_instant_distance_l2_metric_recall() {
    let dim = 128;
    let n_vectors = 500;
    let k = 5;

    // Create indices with L2 metric
    let bf_index = BruteForceIndex::new(dim, DistanceMetric::L2);
    let instant_config = InstantDistanceConfig::balanced(dim, DistanceMetric::L2);
    let instant_index = InstantDistanceIndex::new(instant_config).unwrap();

    // Insert vectors
    for _ in 0..n_vectors {
        let doc_id = DocumentId::new();
        let vector = random_vector(dim);
        let doc = VectorDocument::new(doc_id, vector);

        bf_index.insert(doc.clone()).await.unwrap();
        instant_index.insert(doc).await.unwrap();
    }

    // Rebuild instant-distance index before searching
    instant_index.force_rebuild().await.unwrap();

    // Query
    let query = random_vector(dim);

    // Get ground truth
    let bf_results = bf_index.search(&query, k, None).await.unwrap();
    let bf_ids: Vec<_> = bf_results.iter().map(|r| r.doc_id).collect();

    // Get instant-distance results
    let instant_results = instant_index.search(&query, k, None).await.unwrap();
    let instant_ids: Vec<_> = instant_results.iter().map(|r| r.doc_id).collect();

    // Compute recall
    let recall = compute_recall(&bf_ids, &instant_ids, k);
    println!("InstantDistance L2 recall@{}: {:.3}", k, recall);

    // Should achieve >90% recall for L2
    assert!(recall > 0.90, "L2 recall too low: {:.3}", recall);
}

#[tokio::test]
async fn test_instant_distance_high_recall_config() {
    let dim = 128;
    let n_vectors = 500;
    let k = 10;

    // Create indices - use high_recall config
    let bf_index = BruteForceIndex::new(dim, DistanceMetric::Cosine);
    let instant_config = InstantDistanceConfig::high_recall(dim, DistanceMetric::Cosine);
    let instant_index = InstantDistanceIndex::new(instant_config).unwrap();

    // Insert vectors
    for _ in 0..n_vectors {
        let doc_id = DocumentId::new();
        let vector = random_vector(dim);
        let doc = VectorDocument::new(doc_id, vector);

        bf_index.insert(doc.clone()).await.unwrap();
        instant_index.insert(doc).await.unwrap();
    }

    // Rebuild instant-distance index before searching
    instant_index.force_rebuild().await.unwrap();

    // Query
    let query = random_vector(dim);

    // Get ground truth
    let bf_results = bf_index.search(&query, k, None).await.unwrap();
    let bf_ids: Vec<_> = bf_results.iter().map(|r| r.doc_id).collect();

    // Get instant-distance results
    let instant_results = instant_index.search(&query, k, None).await.unwrap();
    let instant_ids: Vec<_> = instant_results.iter().map(|r| r.doc_id).collect();

    // Compute recall
    let recall = compute_recall(&bf_ids, &instant_ids, k);
    println!(
        "InstantDistance high_recall config recall@{}: {:.3}",
        k, recall
    );

    // High recall config should achieve >97% recall
    assert!(recall > 0.97, "High recall config recall: {:.3}", recall);
}
