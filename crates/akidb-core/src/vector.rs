//! Vector domain types for AkiDB 2.0 vector engine.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::ids::DocumentId;
use crate::DistanceMetric;

/// A vector document stored in the index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorDocument {
    /// Unique identifier within the collection
    pub doc_id: DocumentId,

    /// External identifier (user-provided, optional)
    pub external_id: Option<String>,

    /// Dense vector embedding
    pub vector: Vec<f32>,

    /// JSON metadata payload (user-defined)
    pub metadata: Option<JsonValue>,

    /// Timestamp when document was inserted
    pub inserted_at: DateTime<Utc>,
}

impl VectorDocument {
    /// Creates a new vector document with the given ID and vector.
    #[must_use]
    pub fn new(doc_id: DocumentId, vector: Vec<f32>) -> Self {
        Self {
            doc_id,
            external_id: None,
            vector,
            metadata: None,
            inserted_at: Utc::now(),
        }
    }

    /// Sets the external identifier (builder pattern).
    #[must_use]
    pub fn with_external_id(mut self, external_id: String) -> Self {
        self.external_id = Some(external_id);
        self
    }

    /// Sets the metadata (builder pattern).
    #[must_use]
    pub fn with_metadata(mut self, metadata: JsonValue) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Returns the dimension of the vector.
    #[must_use]
    pub fn dimension(&self) -> usize {
        self.vector.len()
    }
}

/// Result of a vector search operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Document identifier
    pub doc_id: DocumentId,

    /// External identifier (if set)
    pub external_id: Option<String>,

    /// Distance/similarity score (metric-dependent)
    pub score: f32,

    /// Document metadata (if requested)
    pub metadata: Option<JsonValue>,
}

impl SearchResult {
    /// Creates a new search result with the given document ID and score.
    #[must_use]
    pub fn new(doc_id: DocumentId, score: f32) -> Self {
        Self {
            doc_id,
            external_id: None,
            score,
            metadata: None,
        }
    }

    /// Sets the external identifier (builder pattern).
    #[must_use]
    pub fn with_external_id(mut self, external_id: String) -> Self {
        self.external_id = Some(external_id);
        self
    }

    /// Sets the metadata (builder pattern).
    #[must_use]
    pub fn with_metadata(mut self, metadata: JsonValue) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Computes the cosine similarity between two vectors.
///
/// Returns a value in [-1, 1] where 1 means identical direction.
/// Higher is more similar.
///
/// # Panics
///
/// Panics if the vectors have different dimensions.
#[must_use]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "Vector dimensions must match");

    let dot = dot_product(a, b);
    let norm_a = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

/// Computes the Euclidean distance (L2 norm) between two vectors.
///
/// Returns a value in [0, ∞) where 0 means identical vectors.
/// Lower is more similar.
///
/// # Panics
///
/// Panics if the vectors have different dimensions.
#[must_use]
pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "Vector dimensions must match");

    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

/// Computes the dot product between two vectors.
///
/// Returns a value in (-∞, ∞). Higher is more similar (for normalized vectors).
///
/// # Panics
///
/// Panics if the vectors have different dimensions.
#[must_use]
pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "Vector dimensions must match");

    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

impl DistanceMetric {
    /// Computes the distance/similarity score between two vectors.
    ///
    /// # Panics
    ///
    /// Panics if the vectors have different dimensions.
    #[must_use]
    pub fn compute(&self, a: &[f32], b: &[f32]) -> f32 {
        match self {
            Self::Cosine => cosine_similarity(a, b),
            Self::L2 => euclidean_distance(a, b),
            Self::Dot => dot_product(a, b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6, "Identical vectors should have cosine similarity of 1.0");
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 0.0).abs() < 1e-6, "Orthogonal vectors should have cosine similarity of 0.0");
    }

    #[test]
    fn test_euclidean_distance_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let dist = euclidean_distance(&a, &b);
        assert!((dist - 0.0).abs() < 1e-6, "Identical vectors should have Euclidean distance of 0.0");
    }

    #[test]
    fn test_euclidean_distance_unit() {
        let a = vec![0.0, 0.0];
        let b = vec![1.0, 0.0];
        let dist = euclidean_distance(&a, &b);
        assert!((dist - 1.0).abs() < 1e-6, "Unit distance should be 1.0");
    }

    #[test]
    fn test_dot_product_positive() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![2.0, 3.0, 4.0];
        let dot = dot_product(&a, &b);
        // 1*2 + 2*3 + 3*4 = 2 + 6 + 12 = 20
        assert!((dot - 20.0).abs() < 1e-6);
    }

    #[test]
    fn test_dot_product_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let dot = dot_product(&a, &b);
        assert!((dot - 0.0).abs() < 1e-6, "Orthogonal vectors should have dot product of 0.0");
    }

    #[test]
    fn test_distance_metric_compute_cosine() {
        let metric = DistanceMetric::Cosine;
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let score = metric.compute(&a, &b);
        assert!((score - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_distance_metric_compute_l2() {
        let metric = DistanceMetric::L2;
        let a = vec![0.0, 0.0];
        let b = vec![3.0, 4.0];
        let score = metric.compute(&a, &b);
        assert!((score - 5.0).abs() < 1e-6); // 3-4-5 triangle
    }

    #[test]
    fn test_distance_metric_compute_dot() {
        let metric = DistanceMetric::Dot;
        let a = vec![1.0, 2.0];
        let b = vec![3.0, 4.0];
        let score = metric.compute(&a, &b);
        assert!((score - 11.0).abs() < 1e-6); // 1*3 + 2*4 = 11
    }

    #[test]
    fn test_vector_document_builder() {
        let doc_id = DocumentId::new();
        let vector = vec![1.0, 2.0, 3.0];
        let metadata = serde_json::json!({"title": "test"});

        let doc = VectorDocument::new(doc_id, vector.clone())
            .with_external_id("doc-123".to_string())
            .with_metadata(metadata.clone());

        assert_eq!(doc.doc_id, doc_id);
        assert_eq!(doc.external_id, Some("doc-123".to_string()));
        assert_eq!(doc.vector, vector);
        assert_eq!(doc.metadata, Some(metadata));
        assert_eq!(doc.dimension(), 3);
    }

    #[test]
    fn test_search_result_builder() {
        let doc_id = DocumentId::new();
        let metadata = serde_json::json!({"title": "result"});

        let result = SearchResult::new(doc_id, 0.95)
            .with_external_id("doc-456".to_string())
            .with_metadata(metadata.clone());

        assert_eq!(result.doc_id, doc_id);
        assert_eq!(result.external_id, Some("doc-456".to_string()));
        assert_eq!(result.score, 0.95);
        assert_eq!(result.metadata, Some(metadata));
    }
}
