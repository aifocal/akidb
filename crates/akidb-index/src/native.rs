//! Native (pure Rust) index provider using brute force linear search.
//!
//! This implementation provides a simple, reliable baseline for ANN search without
//! external dependencies. While not optimized for large-scale production use,
//! it serves as:
//! - A reference implementation
//! - A testing baseline
//! - A fallback option for small datasets

use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

use akidb_core::{collection::DistanceMetric, Error, Result};

use crate::provider::IndexProvider;
use crate::types::*;

/// In-memory vector store for the native index
#[derive(Debug, Clone, Serialize, Deserialize)]
struct VectorStore {
    /// Collection name
    collection: String,
    /// Primary key → vector index mapping
    key_to_idx: HashMap<String, usize>,
    /// Vector data (row-major, dimension × count)
    vectors: Vec<f32>,
    /// Metadata for each vector
    payloads: Vec<serde_json::Value>,
    /// Vector dimension
    dimension: usize,
    /// Distance metric
    distance: DistanceMetric,
}

impl VectorStore {
    fn new(collection: String, dimension: usize, distance: DistanceMetric) -> Self {
        Self {
            collection,
            key_to_idx: HashMap::new(),
            vectors: Vec::new(),
            payloads: Vec::new(),
            dimension,
            distance,
        }
    }

    /// Add a batch of vectors
    fn add_batch(&mut self, batch: &IndexBatch) -> Result<()> {
        if batch.primary_keys.len() != batch.vectors.len()
            || batch.vectors.len() != batch.payloads.len()
        {
            return Err(Error::Validation(
                "Batch arrays must have equal length".to_string(),
            ));
        }

        for (i, (key, vec)) in batch.primary_keys.iter().zip(&batch.vectors).enumerate() {
            if vec.components.len() != self.dimension {
                return Err(Error::Validation(format!(
                    "Vector dimension mismatch: expected {}, got {}",
                    self.dimension,
                    vec.components.len()
                )));
            }

            // Check for duplicates
            if self.key_to_idx.contains_key(key) {
                return Err(Error::Conflict(format!("Duplicate key: {}", key)));
            }

            let idx = self.vectors.len() / self.dimension;
            self.key_to_idx.insert(key.clone(), idx);
            self.vectors.extend_from_slice(&vec.components);
            self.payloads.push(batch.payloads[i].clone());
        }

        Ok(())
    }

    /// Remove vectors by primary keys
    fn remove(&mut self, keys: &[String]) -> Result<()> {
        let mut removed_indices = Vec::new();

        for key in keys {
            if let Some(&idx) = self.key_to_idx.get(key) {
                removed_indices.push(idx);
            }
        }

        if removed_indices.is_empty() {
            return Ok(());
        }

        // Sort in reverse order to avoid index shifts
        removed_indices.sort_unstable_by(|a, b| b.cmp(a));

        for idx in removed_indices {
            // Remove from key map
            self.key_to_idx.retain(|_, &mut v| v != idx);

            // Shift indices
            for (_, v) in self.key_to_idx.iter_mut() {
                if *v > idx {
                    *v -= 1;
                }
            }

            // Remove vector data
            let start = idx * self.dimension;
            let end = start + self.dimension;
            self.vectors.drain(start..end);

            // Remove payload
            self.payloads.remove(idx);
        }

        Ok(())
    }

    /// Search for nearest neighbors using brute force
    fn search(&self, query: &QueryVector, options: &SearchOptions) -> Result<SearchResult> {
        if query.components.len() != self.dimension {
            return Err(Error::Validation(format!(
                "Query dimension mismatch: expected {}, got {}",
                self.dimension,
                query.components.len()
            )));
        }

        let count = self.vectors.len() / self.dimension;
        if count == 0 {
            return Ok(SearchResult {
                query: query.clone(),
                neighbors: Vec::new(),
            });
        }

        // Compute distances for all vectors
        let mut scored: Vec<(usize, f32)> = (0..count)
            .filter_map(|idx| {
                // Apply filter if provided
                if let Some(ref bitmap) = options.filter {
                    if !bitmap.contains(idx as u32) {
                        return None;
                    }
                }

                let start = idx * self.dimension;
                let end = start + self.dimension;
                let vector = &self.vectors[start..end];

                let score = self.compute_distance(&query.components, vector);
                Some((idx, score))
            })
            .collect();

        // Sort by score (ascending for L2/Cosine, descending for Dot)
        scored.sort_by(|a, b| {
            if matches!(self.distance, DistanceMetric::Dot) {
                b.1.partial_cmp(&a.1).unwrap()
            } else {
                a.1.partial_cmp(&b.1).unwrap()
            }
        });

        // Take top_k results
        let top_k = options.top_k as usize;
        let neighbors: Vec<ScoredPoint> = scored
            .into_iter()
            .take(top_k)
            .filter_map(|(idx, score)| {
                // Find primary key
                let key = self
                    .key_to_idx
                    .iter()
                    .find(|(_, &v)| v == idx)
                    .map(|(k, _)| k.clone())?;

                Some(ScoredPoint {
                    primary_key: key,
                    score,
                    payload: Some(self.payloads[idx].clone()),
                })
            })
            .collect();

        Ok(SearchResult {
            query: query.clone(),
            neighbors,
        })
    }

    /// Compute distance between two vectors
    fn compute_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        match self.distance {
            DistanceMetric::L2 => {
                let mut sum = 0.0;
                for i in 0..self.dimension {
                    let diff = a[i] - b[i];
                    sum += diff * diff;
                }
                sum.sqrt()
            }
            DistanceMetric::Cosine => {
                let mut dot = 0.0;
                let mut norm_a = 0.0;
                let mut norm_b = 0.0;

                for i in 0..self.dimension {
                    dot += a[i] * b[i];
                    norm_a += a[i] * a[i];
                    norm_b += b[i] * b[i];
                }

                if norm_a == 0.0 || norm_b == 0.0 {
                    return 1.0; // Maximum distance
                }

                1.0 - (dot / (norm_a.sqrt() * norm_b.sqrt()))
            }
            DistanceMetric::Dot => {
                let mut dot = 0.0;
                for i in 0..self.dimension {
                    dot += a[i] * b[i];
                }
                dot
            }
        }
    }

    fn count(&self) -> usize {
        self.vectors.len() / self.dimension
    }
}

/// Native index provider implementation
pub struct NativeIndexProvider {
    /// In-memory index storage
    indices: Arc<RwLock<HashMap<Uuid, VectorStore>>>,
}

impl NativeIndexProvider {
    pub fn new() -> Self {
        Self {
            indices: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Extract vectors and payloads for persistence to storage
    ///
    /// This method retrieves all vectors and their associated payloads from the index
    /// so they can be persisted to S3 storage.
    pub fn extract_segment_data(
        &self,
        handle: &IndexHandle,
    ) -> Result<(Vec<Vec<f32>>, Vec<serde_json::Value>)> {
        let indices = self.indices.read();
        let store = indices
            .get(&handle.index_id)
            .ok_or_else(|| Error::NotFound(format!("Index {} not found", handle.index_id)))?;

        // Convert flat vector array to Vec<Vec<f32>>
        // Use chunks_exact to enforce dimension integrity
        debug_assert_eq!(
            store.vectors.len() % store.dimension,
            0,
            "vectors.len() must be divisible by dimension"
        );
        let vectors: Vec<Vec<f32>> = store
            .vectors
            .chunks_exact(store.dimension)
            .map(|chunk| chunk.to_vec())
            .collect();

        debug!(
            "Extracted {} vectors and {} payloads from index {}",
            vectors.len(),
            store.payloads.len(),
            handle.index_id
        );

        Ok((vectors, store.payloads.clone()))
    }
}

impl Default for NativeIndexProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IndexProvider for NativeIndexProvider {
    fn kind(&self) -> IndexKind {
        IndexKind::Native
    }

    async fn build(&self, request: BuildRequest) -> Result<IndexHandle> {
        info!(
            "Building native index for collection: {}",
            request.collection
        );

        let index_id = Uuid::new_v4();

        // Determine dimension from segments (if available)
        let dimension = request
            .segments
            .first()
            .map(|seg| seg.vector_dim)
            .unwrap_or(0) as usize;

        if dimension == 0 {
            return Err(Error::Validation(
                "Cannot build index with dimension 0".to_string(),
            ));
        }

        // Create new vector store
        let store = VectorStore::new(request.collection.clone(), dimension, request.distance);

        // Store index
        {
            let mut indices = self.indices.write();
            indices.insert(index_id, store);
        }

        let handle = IndexHandle {
            index_id,
            kind: IndexKind::Native,
            dimension: dimension as u16,
            collection: request.collection,
        };

        info!(
            "Created native index {} with dimension {}",
            index_id, dimension
        );

        Ok(handle)
    }

    async fn add_batch(&self, handle: &IndexHandle, batch: IndexBatch) -> Result<()> {
        debug!(
            "Adding batch of {} vectors to index {}",
            batch.primary_keys.len(),
            handle.index_id
        );

        let mut indices = self.indices.write();
        let store = indices
            .get_mut(&handle.index_id)
            .ok_or_else(|| Error::NotFound(format!("Index {} not found", handle.index_id)))?;

        store.add_batch(&batch)?;

        debug!(
            "Index {} now contains {} vectors",
            handle.index_id,
            store.count()
        );

        Ok(())
    }

    async fn remove(&self, handle: &IndexHandle, keys: &[String]) -> Result<()> {
        debug!(
            "Removing {} keys from index {}",
            keys.len(),
            handle.index_id
        );

        let mut indices = self.indices.write();
        let store = indices
            .get_mut(&handle.index_id)
            .ok_or_else(|| Error::NotFound(format!("Index {} not found", handle.index_id)))?;

        store.remove(keys)?;

        debug!(
            "Index {} now contains {} vectors",
            handle.index_id,
            store.count()
        );

        Ok(())
    }

    async fn search(
        &self,
        handle: &IndexHandle,
        query: QueryVector,
        options: SearchOptions,
    ) -> Result<SearchResult> {
        debug!(
            "Searching index {} for top_k={}",
            handle.index_id, options.top_k
        );

        let indices = self.indices.read();
        let store = indices
            .get(&handle.index_id)
            .ok_or_else(|| Error::NotFound(format!("Index {} not found", handle.index_id)))?;

        let result = store.search(&query, &options)?;

        debug!("Search returned {} results", result.neighbors.len());

        Ok(result)
    }

    fn serialize(&self, handle: &IndexHandle) -> Result<Vec<u8>> {
        debug!("Serializing index {}", handle.index_id);

        let indices = self.indices.read();
        let store = indices
            .get(&handle.index_id)
            .ok_or_else(|| Error::NotFound(format!("Index {} not found", handle.index_id)))?;

        let data = serde_json::to_vec(store)
            .map_err(|e| Error::Storage(format!("Failed to serialize index: {}", e)))?;

        debug!(
            "Serialized index {} ({} bytes)",
            handle.index_id,
            data.len()
        );

        Ok(data)
    }

    fn deserialize(&self, bytes: &[u8]) -> Result<IndexHandle> {
        debug!("Deserializing index ({} bytes)", bytes.len());

        let store: VectorStore = serde_json::from_slice(bytes)
            .map_err(|e| Error::Storage(format!("Failed to deserialize index: {}", e)))?;

        let index_id = Uuid::new_v4();
        let dimension = store.dimension as u16;
        let collection = store.collection.clone();

        // Store index
        {
            let mut indices = self.indices.write();
            indices.insert(index_id, store);
        }

        let handle = IndexHandle {
            index_id,
            kind: IndexKind::Native,
            dimension,
            collection,
        };

        debug!("Deserialized index {}", index_id);

        Ok(handle)
    }

    fn extract_for_persistence(
        &self,
        handle: &IndexHandle,
    ) -> Result<(Vec<Vec<f32>>, Vec<serde_json::Value>)> {
        // Delegate to the existing extract_segment_data method
        self.extract_segment_data(handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_native_index_build() {
        let provider = NativeIndexProvider::new();

        let request = BuildRequest {
            collection: "test".to_string(),
            kind: IndexKind::Native,
            distance: DistanceMetric::Cosine,
            segments: vec![],
        };

        // Should fail with dimension 0
        assert!(provider.build(request).await.is_err());
    }

    #[tokio::test]
    async fn test_native_index_add_and_search() {
        let provider = NativeIndexProvider::new();

        // Build index with dimension 3
        let request = BuildRequest {
            collection: "test".to_string(),
            kind: IndexKind::Native,
            distance: DistanceMetric::Cosine,
            segments: vec![akidb_core::segment::SegmentDescriptor {
                segment_id: Uuid::new_v4(),
                collection: "test".to_string(),
                vector_dim: 3,
                record_count: 0,
                state: akidb_core::segment::SegmentState::Active,
                lsn_range: 0..=0,
                compression_level: 0,
                created_at: chrono::Utc::now(),
            }],
        };

        let handle = provider.build(request).await.unwrap();

        // Add some vectors
        let batch = IndexBatch {
            primary_keys: vec!["key1".to_string(), "key2".to_string(), "key3".to_string()],
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
            payloads: vec![json!({"id": 1}), json!({"id": 2}), json!({"id": 3})],
        };

        provider.add_batch(&handle, batch).await.unwrap();

        // Search
        let query = QueryVector {
            components: vec![1.0, 0.1, 0.0],
        };

        let options = SearchOptions {
            top_k: 2,
            filter: None,
            timeout_ms: 1000,
        };

        let result = provider.search(&handle, query, options).await.unwrap();

        assert_eq!(result.neighbors.len(), 2);
        assert_eq!(result.neighbors[0].primary_key, "key1"); // Closest to [1,0,0]
    }

    #[tokio::test]
    async fn test_native_index_serialize() {
        let provider = NativeIndexProvider::new();

        let request = BuildRequest {
            collection: "test".to_string(),
            kind: IndexKind::Native,
            distance: DistanceMetric::L2,
            segments: vec![akidb_core::segment::SegmentDescriptor {
                segment_id: Uuid::new_v4(),
                collection: "test".to_string(),
                vector_dim: 2,
                record_count: 0,
                state: akidb_core::segment::SegmentState::Active,
                lsn_range: 0..=0,
                compression_level: 0,
                created_at: chrono::Utc::now(),
            }],
        };

        let handle = provider.build(request).await.unwrap();

        // Serialize
        let data = provider.serialize(&handle).unwrap();
        assert!(!data.is_empty());

        // Deserialize
        let new_handle = provider.deserialize(&data).unwrap();
        assert_eq!(new_handle.dimension, handle.dimension);
        assert_eq!(new_handle.kind, handle.kind);
    }
}
