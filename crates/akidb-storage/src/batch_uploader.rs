//! Batch uploader for S3 with automatic flushing

use crate::batch_config::S3BatchConfig;
use crate::object_store::ObjectStore;
use crate::parquet_encoder::ParquetEncoder;
use akidb_core::error::CoreResult;
use akidb_core::ids::CollectionId;
use akidb_core::vector::VectorDocument;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant};

/// Batch uploader with automatic flushing
pub struct BatchUploader {
    /// Object store backend
    store: Arc<dyn ObjectStore>,
    /// Parquet encoder
    encoder: ParquetEncoder,
    /// Configuration
    config: S3BatchConfig,
    /// Pending batches per collection
    pending: Arc<Mutex<HashMap<CollectionId, BatchState>>>,
}

/// State for a single collection's batch
struct BatchState {
    /// Buffered documents
    documents: Vec<VectorDocument>,
    /// Vector dimension
    dimension: u32,
    /// Time when first document was added
    first_added: Instant,
}

impl BatchUploader {
    /// Create new batch uploader
    pub fn new(store: Arc<dyn ObjectStore>, config: S3BatchConfig) -> CoreResult<Self> {
        config.validate().map_err(|e| {
            akidb_core::error::CoreError::ValidationError(format!("Invalid batch config: {}", e))
        })?;

        Ok(Self {
            store,
            encoder: ParquetEncoder::default(),
            config,
            pending: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Add document to batch (may trigger flush)
    ///
    /// # Returns
    /// - `Ok(true)` if batch was flushed
    /// - `Ok(false)` if document was buffered
    pub async fn add_document(
        &self,
        collection_id: CollectionId,
        dimension: u32,
        document: VectorDocument,
    ) -> CoreResult<bool> {
        let mut pending = self.pending.lock().await;

        let state = pending.entry(collection_id).or_insert_with(|| BatchState {
            documents: Vec::new(),
            dimension,
            first_added: Instant::now(),
        });

        // Validate dimension matches
        if state.dimension != dimension {
            return Err(akidb_core::error::CoreError::ValidationError(format!(
                "Dimension mismatch: expected {}, got {}",
                state.dimension, dimension
            )));
        }

        state.documents.push(document);

        // Check if we should flush
        let should_flush = state.documents.len() >= self.config.batch_size
            || state.first_added.elapsed() > Duration::from_millis(self.config.max_wait_ms);

        if should_flush {
            self.flush_collection_locked(collection_id, &mut pending)
                .await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Flush all pending batches
    pub async fn flush_all(&self) -> CoreResult<usize> {
        let mut pending = self.pending.lock().await;
        let collection_ids: Vec<CollectionId> = pending.keys().copied().collect();

        let mut flushed_count = 0;
        for collection_id in collection_ids {
            let count = self
                .flush_collection_locked(collection_id, &mut pending)
                .await?;
            flushed_count += count;
        }

        Ok(flushed_count)
    }

    /// Flush a specific collection (requires lock held)
    async fn flush_collection_locked(
        &self,
        collection_id: CollectionId,
        pending: &mut HashMap<CollectionId, BatchState>,
    ) -> CoreResult<usize> {
        if let Some(state) = pending.remove(&collection_id) {
            if state.documents.is_empty() {
                return Ok(0);
            }

            let doc_count = state.documents.len();

            // Encode to Parquet
            let parquet_bytes = self
                .encoder
                .encode_batch(&state.documents, state.dimension)?;

            // Generate S3 key
            let batch_id = uuid::Uuid::new_v4();
            let key = format!("collections/{}/batches/{}.parquet", collection_id, batch_id);

            // Upload to S3 (note: ObjectStore trait only takes key and data)
            self.store.put(&key, parquet_bytes).await?;

            tracing::info!(
                collection_id = %collection_id,
                doc_count = doc_count,
                batch_id = %batch_id,
                "Flushed batch to S3"
            );

            Ok(doc_count)
        } else {
            Ok(0)
        }
    }

    /// Get pending document count for a collection
    pub async fn pending_count(&self, collection_id: CollectionId) -> usize {
        self.pending
            .lock()
            .await
            .get(&collection_id)
            .map(|s| s.documents.len())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object_store::{MockS3Config, MockS3ObjectStore};
    use akidb_core::ids::DocumentId;
    use chrono::Utc;
    use std::time::Duration;

    fn create_test_doc(vector: Vec<f32>) -> VectorDocument {
        VectorDocument {
            doc_id: DocumentId::new(),
            external_id: None,
            vector,
            metadata: None,
            inserted_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_batch_uploader_auto_flush() {
        let mock_config = MockS3Config {
            latency: Duration::from_millis(1),
            track_history: true,
        };
        let store = Arc::new(MockS3ObjectStore::new_with_config(mock_config));

        let batch_config = S3BatchConfig {
            batch_size: 3,
            max_wait_ms: 5000,
            enable_compression: true,
        };

        let uploader = BatchUploader::new(store.clone(), batch_config).unwrap();

        let collection_id = CollectionId::new();

        // Add 2 documents (should buffer)
        let flushed = uploader
            .add_document(collection_id, 3, create_test_doc(vec![1.0, 2.0, 3.0]))
            .await
            .unwrap();
        assert!(!flushed);

        let flushed = uploader
            .add_document(collection_id, 3, create_test_doc(vec![4.0, 5.0, 6.0]))
            .await
            .unwrap();
        assert!(!flushed);

        // Add 3rd document (should trigger flush)
        let flushed = uploader
            .add_document(collection_id, 3, create_test_doc(vec![7.0, 8.0, 9.0]))
            .await
            .unwrap();
        assert!(flushed);

        // Verify S3 upload
        assert_eq!(store.storage_size(), 1);

        let objects = store.list("").await.unwrap();
        assert_eq!(objects.len(), 1);
        assert!(objects[0].key.contains("batches"));
        assert!(objects[0].key.ends_with(".parquet"));
    }

    #[tokio::test]
    async fn test_batch_uploader_manual_flush() {
        let store = Arc::new(MockS3ObjectStore::default());
        let config = S3BatchConfig::default();
        let uploader = BatchUploader::new(store.clone(), config).unwrap();

        let collection_id = CollectionId::new();

        // Add documents
        for i in 0..5 {
            uploader
                .add_document(
                    collection_id,
                    3,
                    create_test_doc(vec![i as f32, i as f32 + 1.0, i as f32 + 2.0]),
                )
                .await
                .unwrap();
        }

        // Manual flush
        let flushed = uploader.flush_all().await.unwrap();
        assert_eq!(flushed, 5);

        // Verify upload
        assert_eq!(store.storage_size(), 1);
    }

    #[tokio::test]
    async fn test_batch_uploader_dimension_mismatch() {
        let store = Arc::new(MockS3ObjectStore::default());
        let config = S3BatchConfig::default();
        let uploader = BatchUploader::new(store.clone(), config).unwrap();

        let collection_id = CollectionId::new();

        // Add document with dimension 3
        uploader
            .add_document(collection_id, 3, create_test_doc(vec![1.0, 2.0, 3.0]))
            .await
            .unwrap();

        // Try to add document with dimension 5 (should fail)
        let result = uploader
            .add_document(
                collection_id,
                5,
                create_test_doc(vec![1.0, 2.0, 3.0, 4.0, 5.0]),
            )
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Dimension mismatch"));
    }
}
