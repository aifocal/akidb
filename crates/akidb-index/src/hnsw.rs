//! HNSW (Hierarchical Navigable Small World) index provider.
//!
//! This implementation uses the `instant-distance` library to provide
//! approximate nearest neighbor search with HNSW algorithm.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use uuid::Uuid;

use akidb_core::{DistanceMetric, Error, Result};

use crate::provider::IndexProvider;
use crate::types::*;

/// Configuration for HNSW index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswConfig {
    /// M: Max connections per layer (default: 16)
    /// Higher M = better recall, slower build, more memory
    pub m: usize,

    /// efConstruction: Search width during build (default: 200)
    /// Higher efConstruction = better quality, slower build
    pub ef_construction: usize,

    /// efSearch: Search width during query (default: 100)
    /// Higher efSearch = better recall, slower search
    pub ef_search: usize,
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            m: 16,
            ef_construction: 200,
            ef_search: 100,
        }
    }
}

/// In-memory vector store for HNSW index
#[derive(Debug, Clone, Serialize, Deserialize)]
struct VectorStore {
    /// Collection name
    collection: String,
    /// Primary keys (index → key mapping)
    primary_keys: Vec<String>,
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
    /// Helper to find index by primary key
    fn find_index(&self, key: &str) -> Option<usize> {
        self.primary_keys.iter().position(|k| k == key)
    }
}

impl VectorStore {
    fn new(collection: String, dimension: usize, distance: DistanceMetric) -> Self {
        Self {
            collection,
            primary_keys: Vec::new(),
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
            if self.find_index(key).is_some() {
                return Err(Error::Conflict(format!("Duplicate key: {}", key)));
            }

            self.primary_keys.push(key.clone());
            self.vectors.extend_from_slice(&vec.components);
            self.payloads.push(batch.payloads[i].clone());
        }

        Ok(())
    }

    /// Search for nearest neighbors using brute force
    /// TODO: Integrate instant-distance HNSW search
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

        // Compute distances for all vectors (brute force fallback)
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
            .map(|(idx, score)| ScoredPoint {
                primary_key: self.primary_keys[idx].clone(),
                score,
                payload: Some(self.payloads[idx].clone()),
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

/// HNSW index provider implementation
pub struct HnswIndexProvider {
    config: HnswConfig,
    /// In-memory index storage
    indices: Arc<RwLock<HashMap<Uuid, VectorStore>>>,
}

impl HnswIndexProvider {
    pub fn new(config: HnswConfig) -> Self {
        Self {
            config,
            indices: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Extract vectors and payloads for persistence to storage
    pub fn extract_segment_data(
        &self,
        handle: &IndexHandle,
    ) -> Result<(Vec<Vec<f32>>, Vec<serde_json::Value>)> {
        let indices = self.indices.read();
        let store = indices
            .get(&handle.index_id)
            .ok_or_else(|| Error::NotFound(format!("Index {} not found", handle.index_id)))?;

        // Convert flat vector array to Vec<Vec<f32>>
        let vectors: Vec<Vec<f32>> = store
            .vectors
            .chunks(store.dimension)
            .map(|chunk| chunk.to_vec())
            .collect();

        debug!(
            "Extracted {} vectors and {} payloads from HNSW index {}",
            vectors.len(),
            store.payloads.len(),
            handle.index_id
        );

        Ok((vectors, store.payloads.clone()))
    }
}

impl Default for HnswIndexProvider {
    fn default() -> Self {
        Self::new(HnswConfig::default())
    }
}

#[async_trait]
impl IndexProvider for HnswIndexProvider {
    fn kind(&self) -> IndexKind {
        IndexKind::Hnsw
    }

    async fn build(&self, request: BuildRequest) -> Result<IndexHandle> {
        info!(
            collection = %request.collection,
            m = self.config.m,
            ef_construction = self.config.ef_construction,
            "Building HNSW index"
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
            kind: IndexKind::Hnsw,
            dimension: dimension as u16,
            collection: request.collection,
        };

        info!(
            index_id = %index_id,
            dimension = dimension,
            "Created HNSW index"
        );

        Ok(handle)
    }

    async fn add_batch(&self, handle: &IndexHandle, batch: IndexBatch) -> Result<()> {
        debug!(
            batch_size = batch.primary_keys.len(),
            index_id = %handle.index_id,
            "Adding batch to HNSW index"
        );

        let mut indices = self.indices.write();
        let store = indices
            .get_mut(&handle.index_id)
            .ok_or_else(|| Error::NotFound(format!("Index {} not found", handle.index_id)))?;

        store.add_batch(&batch)?;

        debug!(
            index_id = %handle.index_id,
            total_vectors = store.count(),
            "HNSW index updated"
        );

        Ok(())
    }

    /// Remove vectors from the index by primary keys.
    ///
    /// # HNSW Limitation
    ///
    /// HNSW indices do not support efficient deletion due to their graph-based structure.
    /// Removing nodes from the navigable small world graph would require expensive graph
    /// reconstruction to maintain connectivity and search quality.
    ///
    /// This method returns `NotImplemented` error. To remove vectors from an HNSW index,
    /// you must rebuild the index from scratch with the filtered vector set.
    ///
    /// # Workaround
    ///
    /// ```rust,ignore
    /// // Filter out unwanted vectors
    /// let filtered_vectors: Vec<Vec<f32>> = original_vectors
    ///     .into_iter()
    ///     .filter(|(key, _)| !keys_to_remove.contains(key))
    ///     .map(|(_, vec)| vec)
    ///     .collect();
    ///
    /// // Rebuild index with filtered vectors
    /// let new_handle = provider.build(BuildRequest {
    ///     collection: "my_collection".to_string(),
    ///     kind: IndexKind::Hnsw,
    ///     distance: DistanceMetric::Cosine,
    ///     segments: filtered_segments,
    /// }).await?;
    /// ```
    ///
    /// # Returns
    ///
    /// Always returns `Error::NotImplemented`.
    ///
    /// # See Also
    ///
    /// - [`NativeIndexProvider::remove`](crates/akidb-index/src/native.rs:365) - Supports efficient deletion
    /// - [`IndexProvider::build`](crates/akidb-index/src/provider.rs:10) - Rebuild index with filtered data
    async fn remove(&self, _handle: &IndexHandle, _keys: &[String]) -> Result<()> {
        Err(Error::NotImplemented(
            "HNSW does not support deletion - rebuild index instead".to_string(),
        ))
    }

    async fn search(
        &self,
        handle: &IndexHandle,
        query: QueryVector,
        options: SearchOptions,
    ) -> Result<SearchResult> {
        debug!(
            index_id = %handle.index_id,
            top_k = options.top_k,
            "Searching HNSW index"
        );

        let indices = self.indices.read();
        let store = indices
            .get(&handle.index_id)
            .ok_or_else(|| Error::NotFound(format!("Index {} not found", handle.index_id)))?;

        let result = store.search(&query, &options)?;

        debug!(
            results = result.neighbors.len(),
            "HNSW search complete"
        );

        Ok(result)
    }

    fn serialize(&self, handle: &IndexHandle) -> Result<Vec<u8>> {
        debug!(index_id = %handle.index_id, "Serializing HNSW index");

        let indices = self.indices.read();
        let store = indices
            .get(&handle.index_id)
            .ok_or_else(|| Error::NotFound(format!("Index {} not found", handle.index_id)))?;

        let data = serde_json::to_vec(store)
            .map_err(|e| Error::Serialization(format!("Failed to serialize HNSW index: {}", e)))?;

        debug!(
            index_id = %handle.index_id,
            size_bytes = data.len(),
            "HNSW index serialized"
        );

        Ok(data)
    }

    fn deserialize(&self, bytes: &[u8]) -> Result<IndexHandle> {
        debug!(size_bytes = bytes.len(), "Deserializing HNSW index");

        let store: VectorStore = serde_json::from_slice(bytes)
            .map_err(|e| Error::Serialization(format!("Failed to deserialize HNSW index: {}", e)))?;

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
            kind: IndexKind::Hnsw,
            dimension,
            collection,
        };

        debug!(index_id = %index_id, "HNSW index deserialized");

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

    #[test]
    fn test_hnsw_config_default() {
        let config = HnswConfig::default();
        assert_eq!(config.m, 16);
        assert_eq!(config.ef_construction, 200);
        assert_eq!(config.ef_search, 100);
    }

    #[test]
    fn test_compute_distance_l2() {
        let store = VectorStore::new("test".to_string(), 3, DistanceMetric::L2);
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let distance = store.compute_distance(&a, &b);
        // Expected: sqrt((4-1)^2 + (5-2)^2 + (6-3)^2) = sqrt(9 + 9 + 9) = sqrt(27) ≈ 5.196
        assert!((distance - 5.196).abs() < 0.01);
    }

    #[test]
    fn test_compute_distance_cosine() {
        let store = VectorStore::new("test".to_string(), 3, DistanceMetric::Cosine);
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let distance = store.compute_distance(&a, &b);
        // Expected: 1.0 (orthogonal vectors)
        assert!((distance - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_compute_distance_dot() {
        let store = VectorStore::new("test".to_string(), 3, DistanceMetric::Dot);
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let distance = store.compute_distance(&a, &b);
        // Expected: 1*4 + 2*5 + 3*6 = 4 + 10 + 18 = 32
        assert!((distance - 32.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_hnsw_index_build() {
        let provider = HnswIndexProvider::default();

        let request = BuildRequest {
            collection: "test".to_string(),
            kind: IndexKind::Hnsw,
            distance: DistanceMetric::Cosine,
            segments: vec![],
        };

        // Should fail with dimension 0
        assert!(provider.build(request).await.is_err());
    }

    #[tokio::test]
    async fn test_hnsw_index_add_and_search() {
        let provider = HnswIndexProvider::default();

        // Build index with dimension 3
        let request = BuildRequest {
            collection: "test".to_string(),
            kind: IndexKind::Hnsw,
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
    async fn test_hnsw_index_serialize() {
        let provider = HnswIndexProvider::default();

        let request = BuildRequest {
            collection: "test".to_string(),
            kind: IndexKind::Hnsw,
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
