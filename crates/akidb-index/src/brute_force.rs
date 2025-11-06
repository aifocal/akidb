//! Brute-force linear scan index implementation.
//!
//! This is a simple, correct baseline implementation that exhaustively
//! compares the query vector against all indexed vectors. It serves as:
//! - A correctness reference for HNSW implementation
//! - A viable option for small collections (< 10k vectors)
//! - A testing baseline for recall validation

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;

use akidb_core::{
    CoreError, CoreResult, DistanceMetric, DocumentId, SearchResult, VectorDocument, VectorIndex,
};

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

        let mut docs = self.documents.write();
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

        let docs = self.documents.read();

        // Compute distances for all documents
        let mut results: Vec<_> = docs
            .values()
            .map(|doc| {
                let score = self.metric.compute(query, &doc.vector);
                SearchResult::new(doc.doc_id, score)
                    .with_external_id(doc.external_id.clone().unwrap_or_default())
                    .with_metadata(doc.metadata.clone().unwrap_or_default())
            })
            .collect();

        // Sort by score according to distance metric convention
        match self.metric {
            DistanceMetric::L2 => {
                // Lower is more similar (distance)
                results.sort_by(|a, b| {
                    a.score
                        .partial_cmp(&b.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            DistanceMetric::Cosine | DistanceMetric::Dot => {
                // Higher is more similar (similarity)
                results.sort_by(|a, b| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
        }

        // Return top-k
        results.truncate(k);
        Ok(results)
    }

    async fn delete(&self, doc_id: DocumentId) -> CoreResult<()> {
        let mut docs = self.documents.write();
        docs.remove(&doc_id);
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

        // Insert 10 documents
        for i in 0..10 {
            let doc = VectorDocument::new(DocumentId::new(), vec![i as f32, 0.0]);
            index.insert(doc).await.unwrap();
        }

        // Request only top 3
        let results = index.search(&[5.0, 0.0], 3, None).await.unwrap();
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

        for i in 0..5 {
            let doc = VectorDocument::new(DocumentId::new(), vec![i as f32, 0.0]);
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
}
