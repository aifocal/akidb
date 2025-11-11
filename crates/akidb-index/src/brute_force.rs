//! Brute-force linear scan index implementation.
//!
//! This is a simple, correct baseline implementation that exhaustively
//! compares the query vector against all indexed vectors. It serves as:
//! - A correctness reference for HNSW implementation
//! - A viable option for small collections (< 10k vectors)
//! - A testing baseline for recall validation

use std::collections::HashMap;

use async_trait::async_trait;

use akidb_core::{
    CoreError, CoreResult, DistanceMetric, DocumentId, SearchResult, VectorDocument, VectorIndex,
};

// Use crate-level sync module for conditional compilation (Loom vs production)
use crate::{Arc, RwLock};

/// Brute-force linear scan index (baseline for correctness).
///
/// Time complexity: O(n·d) per search where n = number of documents, d = dimension
/// Space complexity: O(n·d)
///
/// # Example
///
/// ```
/// use akidb_core::{DistanceMetric, DocumentId, VectorDocument, VectorIndex};
/// use akidb_index::BruteForceIndex;
///
/// # #[tokio::main]
/// # async fn main() -> akidb_core::CoreResult<()> {
/// let index = BruteForceIndex::new(512, DistanceMetric::Cosine);
///
/// // Insert a document
/// let doc = VectorDocument::new(DocumentId::new(), vec![0.1; 512]);
/// index.insert(doc).await?;
///
/// // Search for nearest neighbors
/// let query = vec![0.1; 512];
/// let results = index.search(&query, 10, None).await?;
/// assert_eq!(results.len(), 1);
/// # Ok(())
/// # }
/// ```
pub struct BruteForceIndex {
    /// Vector dimension
    dim: usize,

    /// Distance metric
    metric: DistanceMetric,

    /// In-memory document storage
    documents: Arc<RwLock<HashMap<DocumentId, VectorDocument>>>,
}

impl BruteForceIndex {
    /// Creates a new brute-force index with the specified dimension and metric.
    ///
    /// # Example
    ///
    /// ```
    /// use akidb_core::DistanceMetric;
    /// use akidb_index::BruteForceIndex;
    ///
    /// let index = BruteForceIndex::new(512, DistanceMetric::Cosine);
    /// ```
    #[must_use]
    pub fn new(dim: usize, metric: DistanceMetric) -> Self {
        Self {
            dim,
            metric,
            documents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Returns the vector dimension.
    #[must_use]
    pub fn dimension(&self) -> usize {
        self.dim
    }

    /// Returns the distance metric.
    #[must_use]
    pub fn metric(&self) -> DistanceMetric {
        self.metric
    }
}

#[async_trait]
impl VectorIndex for BruteForceIndex {
    async fn insert(&self, doc: VectorDocument) -> CoreResult<()> {
        if doc.vector.len() != self.dim {
            return Err(CoreError::invalid_state(format!(
                "Vector dimension mismatch: expected {}, got {}",
                self.dim,
                doc.vector.len()
            )));
        }

        // BUG-5 FIX: Validate no NaN or Inf values
        for (i, &val) in doc.vector.iter().enumerate() {
            if !val.is_finite() {
                return Err(CoreError::invalid_state(format!(
                    "Vector contains invalid value at index {}: {}. \
                     Only finite numbers are allowed (no NaN or Infinity)",
                    i, val
                )));
            }
        }

        // BUG-9 FIX: For Cosine metric, reject zero vectors (undefined similarity)
        if matches!(self.metric, DistanceMetric::Cosine) {
            let norm_squared: f32 = doc.vector.iter().map(|x| x * x).sum();
            if norm_squared == 0.0 {
                return Err(CoreError::invalid_state(
                    "Cannot insert zero vector with Cosine similarity metric. \
                     Cosine similarity is mathematically undefined for zero vectors. \
                     Consider using L2 distance metric instead."
                        .to_string(),
                ));
            }
        }

        let mut docs = self.documents.write();

        // BUG-1 FIX: Reject duplicate inserts (consistent with InstantDistanceIndex)
        if docs.contains_key(&doc.doc_id) {
            return Err(CoreError::invalid_state(format!(
                "Document {} already exists",
                doc.doc_id
            )));
        }

        docs.insert(doc.doc_id, doc);
        Ok(())
    }

    async fn search(
        &self,
        query: &[f32],
        k: usize,
        _ef_search: Option<usize>,
    ) -> CoreResult<Vec<SearchResult>> {
        if query.len() != self.dim {
            return Err(CoreError::invalid_state(format!(
                "Query dimension mismatch: expected {}, got {}",
                self.dim,
                query.len()
            )));
        }

        // BUG-8 FIX: Validate query contains no NaN/Inf
        for (i, &val) in query.iter().enumerate() {
            if !val.is_finite() {
                return Err(CoreError::invalid_state(format!(
                    "Query vector contains invalid value at index {}: {}. \
                     Only finite numbers are allowed (no NaN or Infinity)",
                    i, val
                )));
            }
        }

        let docs = self.documents.read();

        // Compute distances for all documents
        let mut results: Vec<_> = docs
            .values()
            .map(|doc| {
                let score = self.metric.compute(query, &doc.vector);
                let mut result = SearchResult::new(doc.doc_id, score);

                // BUG-2 FIX: Only set external_id/metadata if they exist (don't fabricate empty values)
                if let Some(ref ext_id) = doc.external_id {
                    result = result.with_external_id(ext_id.clone());
                }
                if let Some(ref meta) = doc.metadata {
                    result = result.with_metadata(meta.clone());
                }

                result
            })
            .collect();

        // BUG-6 FIX: Use total_cmp for deterministic NaN handling
        // Sort by score according to distance metric convention
        match self.metric {
            DistanceMetric::L2 => {
                // Lower is more similar (distance)
                results.sort_by(|a, b| a.score.total_cmp(&b.score));
            }
            DistanceMetric::Cosine | DistanceMetric::Dot => {
                // Higher is more similar (similarity)
                results.sort_by(|a, b| b.score.total_cmp(&a.score));
            }
        }

        // Return top-k
        results.truncate(k);
        Ok(results)
    }

    async fn delete(&self, doc_id: DocumentId) -> CoreResult<()> {
        let mut docs = self.documents.write();

        // BUG-3 FIX: Return error if document doesn't exist (consistent with InstantDistanceIndex)
        docs.remove(&doc_id)
            .ok_or_else(|| CoreError::not_found("Document", doc_id.to_string()))?;

        Ok(())
    }

    async fn get(&self, doc_id: DocumentId) -> CoreResult<Option<VectorDocument>> {
        let docs = self.documents.read();
        Ok(docs.get(&doc_id).cloned())
    }

    async fn count(&self) -> CoreResult<usize> {
        let docs = self.documents.read();
        Ok(docs.len())
    }

    async fn clear(&self) -> CoreResult<()> {
        let mut docs = self.documents.write();
        docs.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_insert_and_get() {
        let index = BruteForceIndex::new(3, DistanceMetric::Cosine);
        let doc_id = DocumentId::new();
        let doc = VectorDocument::new(doc_id, vec![1.0, 2.0, 3.0]);

        index.insert(doc.clone()).await.expect("insert failed");

        let retrieved = index.get(doc_id).await.expect("get failed");
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.doc_id, doc_id);
        assert_eq!(retrieved.vector, vec![1.0, 2.0, 3.0]);
    }

    #[tokio::test]
    async fn test_insert_dimension_mismatch() {
        let index = BruteForceIndex::new(3, DistanceMetric::Cosine);
        let doc = VectorDocument::new(DocumentId::new(), vec![1.0, 2.0]); // Wrong dimension

        let result = index.insert(doc).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("dimension mismatch"));
    }

    #[tokio::test]
    async fn test_search_cosine_similarity() {
        let index = BruteForceIndex::new(3, DistanceMetric::Cosine);

        // Insert three documents
        let doc1 = VectorDocument::new(DocumentId::new(), vec![1.0, 0.0, 0.0]);
        let doc2 = VectorDocument::new(DocumentId::new(), vec![0.0, 1.0, 0.0]);
        let doc3 = VectorDocument::new(DocumentId::new(), vec![1.0, 0.0, 0.0]); // Same as doc1

        index.insert(doc1.clone()).await.unwrap();
        index.insert(doc2).await.unwrap();
        index.insert(doc3.clone()).await.unwrap();

        // Query with [1, 0, 0] - should match doc1 and doc3 perfectly
        let results = index.search(&[1.0, 0.0, 0.0], 2, None).await.unwrap();

        assert_eq!(results.len(), 2);
        assert!((results[0].score - 1.0).abs() < 1e-6); // Perfect match
        assert!((results[1].score - 1.0).abs() < 1e-6); // Perfect match
    }

    #[tokio::test]
    async fn test_search_l2_distance() {
        let index = BruteForceIndex::new(2, DistanceMetric::L2);

        // Insert three documents
        let doc1 = VectorDocument::new(DocumentId::new(), vec![0.0, 0.0]);
        let doc2 = VectorDocument::new(DocumentId::new(), vec![3.0, 4.0]); // Distance 5
        let doc3 = VectorDocument::new(DocumentId::new(), vec![1.0, 0.0]); // Distance 1

        index.insert(doc1.clone()).await.unwrap();
        index.insert(doc2).await.unwrap();
        index.insert(doc3.clone()).await.unwrap();

        // Query with [0, 0] - should return doc1 (dist 0), doc3 (dist 1), doc2 (dist 5)
        let results = index.search(&[0.0, 0.0], 3, None).await.unwrap();

        assert_eq!(results.len(), 3);
        assert!((results[0].score - 0.0).abs() < 1e-6); // doc1
        assert!((results[1].score - 1.0).abs() < 1e-6); // doc3
        assert!((results[2].score - 5.0).abs() < 1e-6); // doc2
    }

    #[tokio::test]
    async fn test_search_returns_top_k() {
        let index = BruteForceIndex::new(2, DistanceMetric::Cosine);

        // Insert 10 documents (start from 1 to avoid zero vector with Cosine)
        for i in 1..=10 {
            let doc = VectorDocument::new(DocumentId::new(), vec![i as f32, 1.0]);
            index.insert(doc).await.unwrap();
        }

        // Request only top 3
        let results = index.search(&[5.0, 1.0], 3, None).await.unwrap();
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn test_delete_removes_document() {
        let index = BruteForceIndex::new(2, DistanceMetric::Cosine);
        let doc_id = DocumentId::new();
        let doc = VectorDocument::new(doc_id, vec![1.0, 2.0]);

        index.insert(doc).await.unwrap();
        assert_eq!(index.count().await.unwrap(), 1);

        index.delete(doc_id).await.unwrap();
        assert_eq!(index.count().await.unwrap(), 0);
        assert!(index.get(doc_id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_batch_insert() {
        let index = BruteForceIndex::new(2, DistanceMetric::Cosine);

        let docs = vec![
            VectorDocument::new(DocumentId::new(), vec![1.0, 0.0]),
            VectorDocument::new(DocumentId::new(), vec![0.0, 1.0]),
            VectorDocument::new(DocumentId::new(), vec![1.0, 1.0]),
        ];

        index.insert_batch(docs).await.unwrap();
        assert_eq!(index.count().await.unwrap(), 3);
    }

    #[tokio::test]
    async fn test_clear_empties_index() {
        let index = BruteForceIndex::new(2, DistanceMetric::Cosine);

        // Start from 1 to avoid zero vector with Cosine
        for i in 1..=5 {
            let doc = VectorDocument::new(DocumentId::new(), vec![i as f32, 1.0]);
            index.insert(doc).await.unwrap();
        }

        assert_eq!(index.count().await.unwrap(), 5);

        index.clear().await.unwrap();
        assert_eq!(index.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_search_dimension_mismatch() {
        let index = BruteForceIndex::new(3, DistanceMetric::Cosine);
        let doc = VectorDocument::new(DocumentId::new(), vec![1.0, 2.0, 3.0]);
        index.insert(doc).await.unwrap();

        // Query with wrong dimension
        let result = index.search(&[1.0, 2.0], 1, None).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("dimension mismatch"));
    }

    #[tokio::test]
    async fn test_count_returns_document_count() {
        let index = BruteForceIndex::new(2, DistanceMetric::Cosine);

        assert_eq!(index.count().await.unwrap(), 0);

        index
            .insert(VectorDocument::new(DocumentId::new(), vec![1.0, 0.0]))
            .await
            .unwrap();
        assert_eq!(index.count().await.unwrap(), 1);

        index
            .insert(VectorDocument::new(DocumentId::new(), vec![0.0, 1.0]))
            .await
            .unwrap();
        assert_eq!(index.count().await.unwrap(), 2);
    }

    // BUG-8 Tests: Query NaN/Inf validation
    #[tokio::test]
    async fn test_search_rejects_nan_query() {
        let index = BruteForceIndex::new(3, DistanceMetric::Cosine);

        // Insert valid document
        let doc = VectorDocument::new(DocumentId::new(), vec![1.0, 2.0, 3.0]);
        index.insert(doc).await.unwrap();

        // Try query with NaN
        let query_nan = vec![1.0, f32::NAN, 3.0];
        let result = index.search(&query_nan, 1, None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid value"));
    }

    #[tokio::test]
    async fn test_search_rejects_infinity_query() {
        let index = BruteForceIndex::new(3, DistanceMetric::L2);

        // Insert valid document
        let doc = VectorDocument::new(DocumentId::new(), vec![1.0, 2.0, 3.0]);
        index.insert(doc).await.unwrap();

        // Try query with Infinity
        let query_inf = vec![1.0, f32::INFINITY, 3.0];
        let result = index.search(&query_inf, 1, None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid value"));

        // Try query with negative Infinity
        let query_neg_inf = vec![f32::NEG_INFINITY, 2.0, 3.0];
        let result = index.search(&query_neg_inf, 1, None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid value"));
    }

    // BUG-9 Tests: Zero vector validation for Cosine
    #[tokio::test]
    async fn test_cosine_rejects_zero_vector() {
        let index = BruteForceIndex::new(3, DistanceMetric::Cosine);

        // Try to insert zero vector
        let zero_vec = VectorDocument::new(DocumentId::new(), vec![0.0, 0.0, 0.0]);
        let result = index.insert(zero_vec).await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("zero vector"));
        assert!(err_msg.contains("Cosine"));
    }

    #[tokio::test]
    async fn test_l2_accepts_zero_vector() {
        // L2 distance is well-defined for zero vectors (distance to self = 0)
        let index = BruteForceIndex::new(3, DistanceMetric::L2);

        let zero_vec = VectorDocument::new(DocumentId::new(), vec![0.0, 0.0, 0.0]);
        let result = index.insert(zero_vec).await;

        assert!(result.is_ok()); // Should succeed for L2 metric
    }

    #[tokio::test]
    async fn test_dot_accepts_zero_vector() {
        // Dot product is well-defined for zero vectors (dot(0, v) = 0)
        let index = BruteForceIndex::new(3, DistanceMetric::Dot);

        let zero_vec = VectorDocument::new(DocumentId::new(), vec![0.0, 0.0, 0.0]);
        let result = index.insert(zero_vec).await;

        assert!(result.is_ok()); // Should succeed for Dot metric
    }
}
