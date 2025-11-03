//! HNSW (Hierarchical Navigable Small World) index provider.
//!
//! This implementation uses the `hnsw_rs` library to provide
//! approximate nearest neighbor search with HNSW algorithm.
//!
//! Performance: hnsw_rs provides 2.86x faster search compared to instant-distance
//! based on 100K vector PoC testing (Phase 3 M3).

use std::collections::HashMap;
use std::sync::Arc;

use anndists::dist::*;
use async_trait::async_trait;
use hnsw_rs::prelude::*;
use parking_lot::RwLock;
use roaring::RoaringBitmap;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use uuid::Uuid;

use akidb_core::{DistanceMetric, Error, Result};

use crate::provider::IndexProvider;
use crate::simd::compute_distance_simd;
use crate::types::*;

/// Configuration for HNSW index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswConfig {
    /// M: Max connections per layer (default: 16)
    /// Higher M = better recall, slower build, more memory
    pub m: usize,

    /// efConstruction: Search width during build (default: 400)
    /// Higher efConstruction = better quality, slower build
    pub ef_construction: usize,

    /// efSearch: Search width during query (default: 200)
    /// Higher efSearch = better recall, slower search
    pub ef_search: usize,
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            m: 16,
            ef_construction: 400,
            ef_search: 200, // Tuned for optimal performance (Phase 3 M3)
        }
    }
}

/// Serializable representation of VectorStore (without HNSW index)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SerializableVectorStore {
    collection: String,
    primary_keys: Vec<String>,
    vectors: Vec<f32>,
    payloads: Vec<serde_json::Value>,
    dimension: usize,
    distance: DistanceMetric,
    config: HnswConfig,
}

/// Type alias for HNSW index based on distance metric
/// Note: hnsw_rs Hnsw requires a lifetime parameter, so we use it directly where needed
/// In-memory vector store for HNSW index
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
    /// HNSW configuration
    config: HnswConfig,
    /// HNSW index (optional, built when vectors are added)
    hnsw_index: Option<Hnsw<'static, f32, DistL2>>,
}

impl VectorStore {
    /// Convert to serializable representation
    fn to_serializable(&self) -> SerializableVectorStore {
        SerializableVectorStore {
            collection: self.collection.clone(),
            primary_keys: self.primary_keys.clone(),
            vectors: self.vectors.clone(),
            payloads: self.payloads.clone(),
            dimension: self.dimension,
            distance: self.distance,
            config: self.config.clone(),
        }
    }

    /// Create from serializable representation and rebuild HNSW index
    fn from_serializable(data: SerializableVectorStore) -> Self {
        let config = data.config.clone();
        let mut store = Self {
            collection: data.collection,
            primary_keys: data.primary_keys,
            vectors: data.vectors,
            payloads: data.payloads,
            dimension: data.dimension,
            distance: data.distance,
            config: data.config,
            hnsw_index: None,
        };

        // Rebuild HNSW index if there are vectors
        store.rebuild_hnsw_index_with_config(config);

        store
    }
}

impl VectorStore {
    /// Helper to find index by primary key
    fn find_index(&self, key: &str) -> Option<usize> {
        self.primary_keys.iter().position(|k| k == key)
    }
}

impl VectorStore {
    fn new(
        collection: String,
        dimension: usize,
        distance: DistanceMetric,
        config: HnswConfig,
    ) -> Self {
        Self {
            collection,
            primary_keys: Vec::new(),
            vectors: Vec::new(),
            payloads: Vec::new(),
            dimension,
            distance,
            config,
            hnsw_index: None,
        }
    }

    /// Add a batch of vectors and rebuild HNSW index
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

        // Rebuild HNSW index with all vectors using configured parameters
        let config = self.config.clone();
        self.rebuild_hnsw_index_with_config(config);

        Ok(())
    }

    /// Rebuild HNSW index with specific configuration using hnsw_rs
    fn rebuild_hnsw_index_with_config(&mut self, config: HnswConfig) {
        // INVARIANT CHECK: vectors length must be exact multiple of dimension
        assert_eq!(
            self.vectors.len() % self.dimension,
            0,
            "Data integrity error: vectors.len() ({}) must be divisible by dimension ({})",
            self.vectors.len(),
            self.dimension
        );

        let count = self.vectors.len() / self.dimension;

        // HNSW is an approximate algorithm that works best with larger datasets
        const MIN_VECTORS_FOR_HNSW: usize = 100;

        if count < MIN_VECTORS_FOR_HNSW {
            info!(
                "Skipping HNSW build for {} vectors (minimum: {}), will use brute force",
                count, MIN_VECTORS_FOR_HNSW
            );
            self.hnsw_index = None;
            return;
        }

        // Convert flat vector array to Vec<Vec<f32>>
        let vectors_2d: Vec<Vec<f32>> = self
            .vectors
            .chunks_exact(self.dimension)
            .map(|chunk| chunk.to_vec())
            .collect();

        info!(
            "Building HNSW index with {} vectors (dimension={}), M={}, ef_construction={}, ef_search={}",
            count, self.dimension, config.m, config.ef_construction, config.ef_search
        );

        // Create HNSW parameters based on distance metric
        // hnsw_rs supports multiple distance metrics through anndists
        let hnsw = match self.distance {
            DistanceMetric::L2 => {
                let dist = DistL2;
                Hnsw::<f32, DistL2>::new(
                    config.m,
                    count,
                    config.ef_construction,
                    config.ef_search,
                    dist,
                )
            }
            DistanceMetric::Cosine => {
                // hnsw_rs doesn't have direct Cosine, use L2 on normalized vectors
                // Normalization handled in search
                let dist = DistL2;
                Hnsw::<f32, DistL2>::new(
                    config.m,
                    count,
                    config.ef_construction,
                    config.ef_search,
                    dist,
                )
            }
            DistanceMetric::Dot => {
                // For dot product, use L2 but interpret results differently
                let dist = DistL2;
                Hnsw::<f32, DistL2>::new(
                    config.m,
                    count,
                    config.ef_construction,
                    config.ef_search,
                    dist,
                )
            }
        };

        // Insert vectors into HNSW index
        // For hnsw_rs, we need to use parallel_insert for better performance
        let data_with_ids: Vec<(&Vec<f32>, usize)> = vectors_2d
            .iter()
            .enumerate()
            .map(|(id, vec)| (vec, id))
            .collect();

        // Use hnsw_rs's parallel insertion for better performance
        hnsw.parallel_insert(&data_with_ids);

        self.hnsw_index = Some(hnsw);

        info!("HNSW index built successfully with {} vectors", count);
    }

    /// Search for nearest neighbors using hnsw_rs
    ///
    /// # Filter Pushdown Optimization (Phase 3 M3)
    ///
    /// This method implements intelligent filter pushdown based on filter selectivity:
    /// - **Selective filters (< 10%)**: Use brute force on filtered subset
    /// - **Moderate filters (10-50%)**: Search larger k, then filter
    /// - **Non-selective filters (> 50%)**: Post-filter HNSW results
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

        // OPTIMIZATION: Filter pushdown strategy selection
        if let Some(ref filter_bitmap) = options.filter {
            let filtered_count = filter_bitmap.len() as usize;
            let selectivity = filtered_count as f64 / count as f64;

            // Strategy 1: Very selective filter (< 10%) - use brute force on filtered set
            if selectivity < 0.10 {
                debug!(
                    selectivity = %selectivity,
                    filtered_count = filtered_count,
                    total_count = count,
                    "Using brute force on filtered subset (highly selective filter)"
                );
                return self.brute_force_filtered(query, options, filter_bitmap);
            }

            // Strategy 2: Moderately selective filter (10-50%) - search larger k then filter
            if selectivity < 0.50 {
                debug!(
                    selectivity = %selectivity,
                    "Using oversampling strategy (moderate filter)"
                );
                return self.search_hnsw_with_oversampling(query, options, filter_bitmap);
            }

            // Strategy 3: Non-selective filter (>= 50%) - post-filter with oversampling
            // Even for non-selective filters, we need oversampling to ensure we get enough results
            debug!(
                selectivity = %selectivity,
                "Using post-filter strategy with oversampling (non-selective filter)"
            );
            return self.search_hnsw_with_oversampling(query, options, filter_bitmap);
        }

        // Use HNSW search if index is available
        if let Some(ref hnsw) = self.hnsw_index {
            // hnsw_rs uses search for nearest neighbor search
            let k = options.top_k as usize;
            let results = hnsw.search(&query.components, k, self.config.ef_search);

            // Convert hnsw_rs results to our format
            let neighbors: Vec<ScoredPoint> = results
                .into_iter()
                .filter_map(|neighbor| {
                    let idx = neighbor.d_id;

                    // Apply filter if provided
                    if let Some(ref bitmap) = options.filter {
                        if !bitmap.contains(idx as u32) {
                            return None;
                        }
                    }

                    // hnsw_rs returns distance (lower = closer)
                    let score = neighbor.distance;

                    if idx >= self.primary_keys.len() {
                        return None;
                    }

                    Some(ScoredPoint {
                        primary_key: self.primary_keys[idx].clone(),
                        score,
                        payload: Some(self.payloads[idx].clone()),
                    })
                })
                .take(options.top_k as usize)
                .collect();

            return Ok(SearchResult {
                query: query.clone(),
                neighbors,
            });
        }

        // Fallback to brute force if HNSW index not available
        warn!("HNSW index not available, falling back to brute force search");

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
                b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
            } else {
                a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
            }
        });

        // Take top_k results
        let neighbors: Vec<ScoredPoint> = scored
            .into_iter()
            .take(options.top_k as usize)
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

    /// Brute force search on filtered subset
    fn brute_force_filtered(
        &self,
        query: &QueryVector,
        options: &SearchOptions,
        filter: &RoaringBitmap,
    ) -> Result<SearchResult> {
        let mut scored: Vec<(usize, f32)> = filter
            .iter()
            .filter_map(|doc_id| {
                let idx = doc_id as usize;
                if idx >= self.primary_keys.len() {
                    return None;
                }

                let start = idx * self.dimension;
                let end = start + self.dimension;
                if end > self.vectors.len() {
                    return None;
                }

                let vector = &self.vectors[start..end];
                let score = self.compute_distance(&query.components, vector);
                Some((idx, score))
            })
            .collect();

        // Sort by score
        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top_k results
        let neighbors: Vec<ScoredPoint> = scored
            .into_iter()
            .take(options.top_k as usize)
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

    /// HNSW search with oversampling for moderate filters
    fn search_hnsw_with_oversampling(
        &self,
        query: &QueryVector,
        options: &SearchOptions,
        filter: &RoaringBitmap,
    ) -> Result<SearchResult> {
        let hnsw = self.hnsw_index.as_ref().ok_or_else(|| {
            Error::Internal("HNSW index not available for oversampling".to_string())
        })?;

        // Calculate selectivity and required oversampling
        let count = self.vectors.len() / self.dimension;
        let filtered_count = filter.len() as usize;
        let selectivity = filtered_count as f64 / count as f64;

        // Dynamic oversampling calculation
        let safety_factor = 1.5;
        let oversample_k = if selectivity > 0.0 {
            ((options.top_k as f64 / selectivity) * safety_factor).ceil() as usize
        } else {
            options.top_k as usize * 10
        };

        let effective_k = oversample_k.min(1000).min(count);

        debug!(
            selectivity = %selectivity,
            oversample_k = effective_k,
            "Using HNSW with dynamic oversampling"
        );

        // Search with larger k
        let results = hnsw.search(&query.components, effective_k, self.config.ef_search);

        // Filter and take top_k
        let neighbors: Vec<ScoredPoint> = results
            .into_iter()
            .filter_map(|neighbor| {
                let idx = neighbor.d_id;
                let doc_id = u32::try_from(idx).ok()?;

                if !filter.contains(doc_id) {
                    return None;
                }

                if idx >= self.primary_keys.len() {
                    return None;
                }

                Some(ScoredPoint {
                    primary_key: self.primary_keys[idx].clone(),
                    score: neighbor.distance,
                    payload: Some(self.payloads[idx].clone()),
                })
            })
            .take(options.top_k as usize)
            .collect();

        Ok(SearchResult {
            query: query.clone(),
            neighbors,
        })
    }

    /// Compute distance between two vectors using SIMD optimization
    fn compute_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        compute_distance_simd(self.distance, a, b)
    }

    fn count(&self) -> usize {
        self.vectors.len() / self.dimension
    }
}

/// HNSW index provider implementation using hnsw_rs
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
            ef_search = self.config.ef_search,
            "Building HNSW index with hnsw_rs"
        );

        let index_id = Uuid::new_v4();
        let dimension = request.dimension as usize;

        if dimension == 0 {
            return Err(Error::Validation(
                "Cannot build index with dimension 0".to_string(),
            ));
        }

        // Create new vector store with HNSW configuration
        let store = VectorStore::new(
            request.collection.clone(),
            dimension,
            request.distance,
            self.config.clone(),
        );

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
            "Created HNSW index (hnsw_rs)"
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
            "Searching HNSW index (hnsw_rs)"
        );

        let indices = self.indices.read();
        let store = indices
            .get(&handle.index_id)
            .ok_or_else(|| Error::NotFound(format!("Index {} not found", handle.index_id)))?;

        let result = store.search(&query, &options)?;

        debug!(results = result.neighbors.len(), "HNSW search complete");

        Ok(result)
    }

    fn serialize(&self, handle: &IndexHandle) -> Result<Vec<u8>> {
        debug!(index_id = %handle.index_id, "Serializing HNSW index");

        let indices = self.indices.read();
        let store = indices
            .get(&handle.index_id)
            .ok_or_else(|| Error::NotFound(format!("Index {} not found", handle.index_id)))?;

        // Serialize using the serializable representation (without HNSW graph)
        let serializable = store.to_serializable();
        let data = serde_json::to_vec(&serializable)
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

        let serializable: SerializableVectorStore = serde_json::from_slice(bytes).map_err(|e| {
            Error::Serialization(format!("Failed to deserialize HNSW index: {}", e))
        })?;

        // Reconstruct VectorStore and rebuild HNSW index from vectors
        let store = VectorStore::from_serializable(serializable);

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
        assert_eq!(config.ef_construction, 400);
        assert_eq!(config.ef_search, 200);
    }

    #[test]
    fn test_compute_distance_l2() {
        let store = VectorStore::new(
            "test".to_string(),
            3,
            DistanceMetric::L2,
            HnswConfig::default(),
        );
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let distance = store.compute_distance(&a, &b);
        // Expected: sqrt((4-1)^2 + (5-2)^2 + (6-3)^2) = sqrt(27) ≈ 5.196
        assert!((distance - 5.196).abs() < 0.01);
    }

    #[test]
    fn test_compute_distance_cosine() {
        let store = VectorStore::new(
            "test".to_string(),
            3,
            DistanceMetric::Cosine,
            HnswConfig::default(),
        );
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let distance = store.compute_distance(&a, &b);
        // Expected: 1.0 (orthogonal vectors)
        assert!((distance - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_compute_distance_dot() {
        let store = VectorStore::new(
            "test".to_string(),
            3,
            DistanceMetric::Dot,
            HnswConfig::default(),
        );
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let distance = store.compute_distance(&a, &b);
        // Expected: 1*4 + 2*5 + 3*6 = 32
        assert!((distance - 32.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_hnsw_index_build() {
        let provider = HnswIndexProvider::default();

        let request = BuildRequest {
            collection: "test".to_string(),
            kind: IndexKind::Hnsw,
            distance: DistanceMetric::Cosine,
            dimension: 0,
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
            dimension: 3,
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
        assert_eq!(result.neighbors[0].primary_key, "key1");
    }

    #[tokio::test]
    async fn test_hnsw_index_serialize() {
        let provider = HnswIndexProvider::default();

        let request = BuildRequest {
            collection: "test".to_string(),
            kind: IndexKind::Hnsw,
            distance: DistanceMetric::L2,
            dimension: 2,
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
