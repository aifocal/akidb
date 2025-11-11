//! Parallel S3 uploader with bounded concurrency

use crate::batch_config::S3BatchConfig;
use crate::object_store::ObjectStore;
use crate::parquet_encoder::ParquetEncoder;
use akidb_core::error::{CoreError, CoreResult};
use akidb_core::ids::CollectionId;
use akidb_core::vector::VectorDocument;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinSet;

/// Parallel upload configuration
#[derive(Debug, Clone)]
pub struct ParallelConfig {
    /// Batch configuration
    pub batch: S3BatchConfig,
    /// Maximum concurrent uploads (recommended: 10-20)
    pub max_concurrency: usize,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            batch: S3BatchConfig::default(),
            max_concurrency: 10, // 10 concurrent S3 uploads
        }
    }
}

impl ParallelConfig {
    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        self.batch.validate()?;

        if self.max_concurrency == 0 {
            return Err("max_concurrency must be > 0".to_string());
        }

        if self.max_concurrency > 50 {
            return Err("max_concurrency too high (max: 50, risk of S3 throttling)".to_string());
        }

        Ok(())
    }
}

/// Parallel batch uploader
pub struct ParallelUploader {
    /// Object store backend
    store: Arc<dyn ObjectStore>,
    /// Parquet encoder
    encoder: Arc<ParquetEncoder>,
    /// Configuration
    config: ParallelConfig,
    /// Semaphore for concurrency control
    semaphore: Arc<Semaphore>,
    /// Pending batches
    pending: Arc<Mutex<Vec<Batch>>>,
}

/// A batch ready for upload
struct Batch {
    collection_id: CollectionId,
    dimension: u32,
    documents: Vec<VectorDocument>,
}

impl ParallelUploader {
    /// Create new parallel uploader
    pub fn new(store: Arc<dyn ObjectStore>, config: ParallelConfig) -> CoreResult<Self> {
        config
            .validate()
            .map_err(|e| CoreError::ValidationError(format!("Invalid parallel config: {}", e)))?;

        let semaphore = Arc::new(Semaphore::new(config.max_concurrency));

        Ok(Self {
            store,
            encoder: Arc::new(ParquetEncoder::default()),
            config,
            semaphore,
            pending: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// Add document (buffers locally, does not upload immediately)
    pub async fn add_document(
        &self,
        collection_id: CollectionId,
        dimension: u32,
        document: VectorDocument,
    ) -> CoreResult<()> {
        let mut pending = self.pending.lock().await;

        // Find or create batch for this collection
        if let Some(batch) = pending
            .iter_mut()
            .find(|b| b.collection_id == collection_id)
        {
            if batch.dimension != dimension {
                return Err(CoreError::ValidationError(format!(
                    "Dimension mismatch: expected {}, got {}",
                    batch.dimension, dimension
                )));
            }

            batch.documents.push(document);
        } else {
            pending.push(Batch {
                collection_id,
                dimension,
                documents: vec![document],
            });
        }

        Ok(())
    }

    /// Flush all batches in parallel
    ///
    /// Returns number of documents uploaded
    pub async fn flush_all_parallel(&self) -> CoreResult<usize> {
        // Take all pending batches
        let batches = {
            let mut pending = self.pending.lock().await;
            std::mem::take(&mut *pending)
        };

        if batches.is_empty() {
            return Ok(0);
        }

        // Split batches into batch_size chunks
        let mut upload_tasks = Vec::new();
        for batch in batches {
            let chunks: Vec<_> = batch
                .documents
                .chunks(self.config.batch.batch_size)
                .map(|chunk| (batch.collection_id, batch.dimension, chunk.to_vec()))
                .collect();

            upload_tasks.extend(chunks);
        }

        let total_batches = upload_tasks.len();
        let _total_docs: usize = upload_tasks.iter().map(|(_, _, docs)| docs.len()).sum();

        // Upload all chunks in parallel
        let mut join_set = JoinSet::new();

        for (collection_id, dimension, documents) in upload_tasks {
            let store = self.store.clone();
            let encoder = self.encoder.clone();
            let semaphore = self.semaphore.clone();

            join_set.spawn(async move {
                // Acquire semaphore permit
                let _permit = semaphore.acquire().await.unwrap();

                // Encode to Parquet
                let parquet_bytes = encoder.encode_batch(&documents, dimension)?;

                // Generate S3 key
                let batch_id = uuid::Uuid::new_v4();
                let key = format!("collections/{}/batches/{}.parquet", collection_id, batch_id);

                // Upload to S3 (ObjectStore trait only takes key and data)
                store.put(&key, parquet_bytes).await?;

                tracing::debug!(
                    collection_id = %collection_id,
                    doc_count = documents.len(),
                    batch_id = %batch_id,
                    "Uploaded batch to S3"
                );

                CoreResult::Ok(documents.len())
            });
        }

        // Wait for all uploads to complete
        let mut uploaded_count = 0;
        while let Some(result) = join_set.join_next().await {
            let count = result
                .map_err(|e| CoreError::StorageError(format!("Upload task failed: {}", e)))??;
            uploaded_count += count;
        }

        tracing::info!(
            batches = total_batches,
            documents = uploaded_count,
            "Parallel upload complete"
        );

        Ok(uploaded_count)
    }

    /// Get pending document count
    pub async fn pending_count(&self) -> usize {
        self.pending
            .lock()
            .await
            .iter()
            .map(|b| b.documents.len())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object_store::MockS3ObjectStore;
    use akidb_core::ids::DocumentId;
    use chrono::Utc;

    fn create_doc(vector: Vec<f32>) -> VectorDocument {
        VectorDocument {
            doc_id: DocumentId::new(),
            external_id: None,
            vector,
            metadata: None,
            inserted_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_parallel_uploader_basic() {
        let store = Arc::new(MockS3ObjectStore::default());
        let config = ParallelConfig::default();
        let uploader = ParallelUploader::new(store.clone(), config).unwrap();

        let collection_id = CollectionId::new();

        // Add 100 documents
        for i in 0..100 {
            uploader
                .add_document(collection_id, 3, create_doc(vec![i as f32, 0.0, 0.0]))
                .await
                .unwrap();
        }

        // Flush in parallel
        let uploaded = uploader.flush_all_parallel().await.unwrap();
        assert_eq!(uploaded, 100);

        // Should have 1 batch (100 docs = batch_size)
        assert_eq!(store.storage_size(), 1);
    }

    #[tokio::test]
    async fn test_parallel_uploader_multiple_batches() {
        let store = Arc::new(MockS3ObjectStore::default());
        let config = ParallelConfig {
            batch: S3BatchConfig {
                batch_size: 50,
                max_wait_ms: 5000,
                enable_compression: true,
            },
            max_concurrency: 10,
        };
        let uploader = ParallelUploader::new(store.clone(), config).unwrap();

        let collection_id = CollectionId::new();

        // Add 250 documents (should create 5 batches: 50 each)
        for i in 0..250 {
            uploader
                .add_document(collection_id, 3, create_doc(vec![i as f32, 0.0, 0.0]))
                .await
                .unwrap();
        }

        let uploaded = uploader.flush_all_parallel().await.unwrap();
        assert_eq!(uploaded, 250);

        // Should have 5 batches
        assert_eq!(store.storage_size(), 5);
    }

    #[tokio::test]
    async fn test_parallel_uploader_throughput() {
        use std::time::Instant;

        let store = Arc::new(MockS3ObjectStore::default());
        let config = ParallelConfig {
            batch: S3BatchConfig {
                batch_size: 100,
                max_wait_ms: 5000,
                enable_compression: true,
            },
            max_concurrency: 10,
        };
        let uploader = ParallelUploader::new(store.clone(), config).unwrap();

        let collection_id = CollectionId::new();

        let start = Instant::now();

        // Add 1000 documents
        for i in 0..1000 {
            let vec = vec![i as f32; 512];
            uploader
                .add_document(collection_id, 512, create_doc(vec))
                .await
                .unwrap();
        }

        // Parallel flush
        uploader.flush_all_parallel().await.unwrap();

        let elapsed = start.elapsed();
        let ops_per_sec = 1000.0 / elapsed.as_secs_f64();

        println!(
            "Parallel upload: {} docs in {:.2}s = {:.0} ops/sec",
            1000,
            elapsed.as_secs_f64(),
            ops_per_sec
        );

        // Target: 600 ops/sec (should be ~1.67 seconds for 1000 docs)
        // Relaxed for CI: 550 ops/sec
        assert!(ops_per_sec >= 550.0, "Too slow: {} ops/sec", ops_per_sec);
    }

    #[tokio::test]
    async fn test_parallel_uploader_concurrency_limit() {
        let store = Arc::new(MockS3ObjectStore::default());

        // Very low concurrency limit
        let config = ParallelConfig {
            batch: S3BatchConfig {
                batch_size: 10,
                max_wait_ms: 5000,
                enable_compression: true,
            },
            max_concurrency: 2, // Only 2 concurrent uploads
        };

        let uploader = ParallelUploader::new(store.clone(), config).unwrap();
        let collection_id = CollectionId::new();

        // Add 100 documents (will create 10 batches)
        for i in 0..100 {
            uploader
                .add_document(collection_id, 3, create_doc(vec![i as f32, 0.0, 0.0]))
                .await
                .unwrap();
        }

        uploader.flush_all_parallel().await.unwrap();

        // Should still upload all 10 batches
        assert_eq!(store.storage_size(), 10);
    }
}
