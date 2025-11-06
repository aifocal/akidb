//! HNSW (Hierarchical Navigable Small World) index implementation.
//!
//! This implements the HNSW algorithm from Malkov & Yashunin (2018):
//! "Efficient and robust approximate nearest neighbor search using Hierarchical Navigable Small World graphs"
//! https://arxiv.org/abs/1603.09320
//!
//! HNSW provides approximate nearest neighbor (ANN) search with:
//! - Time complexity: O(log(n) · d) with high probability
//! - Space complexity: O(n · M · log(n))
//! - Recall: >0.95 @ k=10 with proper parameter tuning
//!
//! Note: This is a research implementation (Phase 4C) for educational purposes.
//! For production use, see InstantDistanceIndex (Phase 4B) which achieves >95% recall.

#![allow(dead_code)]

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;
use rand::Rng;

use akidb_core::{
    CoreError, CoreResult, DistanceMetric, DocumentId, SearchResult, VectorDocument, VectorIndex,
};

/// HNSW index configuration parameters.
#[derive(Debug, Clone)]
pub struct HnswConfig {
    /// Dimension of vectors
    pub dim: usize,

    /// Distance metric
    pub metric: DistanceMetric,

    /// Maximum number of connections per node (M)
    /// Range: [8, 128], Default: 32
    pub m: usize,

    /// Maximum connections for layer 0 (M0 = M * 2)
    pub m0: usize,

    /// Construction-time EF parameter (size of dynamic candidate list)
    /// Range: [100, 1000], Default: 200
    pub ef_construction: usize,

    /// Search-time EF parameter (default)
    /// Range: [10, 500], Default: 128
    pub ef_search: usize,

    /// Level generation parameter (ml = 1/ln(M))
    pub ml: f64,
}

impl HnswConfig {
    /// Creates a new HNSW configuration with balanced parameters.
    ///
    /// Suitable for 5M-30M vectors with P95 <25ms target.
    pub fn balanced(dim: usize, metric: DistanceMetric) -> Self {
        let m = 32;
        Self {
            dim,
            metric,
            m,
            m0: m * 2,
            ef_construction: 200,
            ef_search: 128,
            ml: 1.0 / (m as f64).ln(),
        }
    }

    /// Creates configuration optimized for edge cache (small datasets).
    ///
    /// Suitable for ≤5M vectors with P95 <15ms target.
    pub fn edge_cache(dim: usize, metric: DistanceMetric) -> Self {
        let m = 16;
        Self {
            dim,
            metric,
            m,
            m0: m * 2,
            ef_construction: 80,
            ef_search: 64,
            ml: 1.0 / (m as f64).ln(),
        }
    }

    /// Creates configuration optimized for high recall (large datasets).
    ///
    /// Suitable for 30M-100M vectors with P95 <40ms target.
    pub fn high_recall(dim: usize, metric: DistanceMetric) -> Self {
        let m = 48;
        Self {
            dim,
            metric,
            m,
            m0: m * 2,
            ef_construction: 320,
            ef_search: 256,
            ml: 1.0 / (m as f64).ln(),
        }
    }
}

/// A node in the HNSW graph.
#[derive(Debug, Clone)]
struct Node {
    /// Document ID
    doc_id: DocumentId,

    /// External identifier
    external_id: Option<String>,

    /// Vector data
    vector: Vec<f32>,

    /// Metadata
    metadata: Option<serde_json::Value>,

    /// Maximum layer this node appears in
    max_layer: usize,

    /// Tombstone flag for soft deletes
    deleted: bool,
}

/// HNSW index with hierarchical graph structure.
///
/// # Example
///
/// ```
/// use akidb_core::{DistanceMetric, DocumentId, VectorDocument, VectorIndex};
/// use akidb_index::{HnswConfig, HnswIndex};
///
/// # #[tokio::main]
/// # async fn main() -> akidb_core::CoreResult<()> {
/// let config = HnswConfig::balanced(512, DistanceMetric::Cosine);
/// let index = HnswIndex::new(config);
///
/// // Insert documents
/// for i in 0..1000 {
///     let doc = VectorDocument::new(DocumentId::new(), vec![0.1; 512]);
///     index.insert(doc).await?;
/// }
///
/// // Search with custom ef_search
/// let query = vec![0.1; 512];
/// let results = index.search(&query, 10, Some(256)).await?;
/// # Ok(())
/// # }
/// ```
pub struct HnswIndex {
    /// Configuration
    config: HnswConfig,

    /// Graph state (wrapped for interior mutability)
    state: Arc<RwLock<HnswState>>,
}

/// Internal mutable state of HNSW index.
struct HnswState {
    /// Node storage by document ID
    nodes: HashMap<DocumentId, Node>,

    /// Adjacency lists per layer
    /// layers[l][node_id] = Vec<neighbor_id>
    layers: Vec<HashMap<DocumentId, Vec<DocumentId>>>,

    /// Entry point (node with highest layer)
    entry_point: Option<DocumentId>,

    /// Current maximum layer
    max_layer: usize,
}

impl HnswIndex {
    /// Creates a new HNSW index with the given configuration.
    pub fn new(config: HnswConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(HnswState {
                nodes: HashMap::new(),
                layers: vec![HashMap::new()], // Start with layer 0
                entry_point: None,
                max_layer: 0,
            })),
        }
    }

    /// Assigns a random layer to a new node using exponential distribution.
    fn assign_layer(&self) -> usize {
        let mut rng = rand::thread_rng();
        let uniform: f64 = rng.gen_range(0.0..1.0);
        let layer = (-uniform.ln() * self.config.ml).floor() as usize;
        layer
    }

    /// Computes distance between query and a node's vector.
    fn compute_distance(&self, query: &[f32], node: &Node) -> f32 {
        self.config.metric.compute(query, &node.vector)
    }

    /// Searches layer for nearest neighbors (greedy search).
    fn search_layer(
        &self,
        state: &HnswState,
        query: &[f32],
        entry_points: &[DocumentId],
        ef: usize,
        layer: usize,
    ) -> Vec<(f32, DocumentId)> {
        let mut visited = HashSet::new();

        // Candidates: min-heap for exploring nearest neighbors first
        // For L2: store raw distance (lower is better, min-heap correct)
        // For Cosine/Dot: negate score (higher is better, so -score makes min-heap correct)
        let mut candidates: BinaryHeap<std::cmp::Reverse<OrderedDist>> = BinaryHeap::new();

        // Working set: track best ef results (uses original distances/scores)
        let mut working_set: Vec<OrderedDist> = Vec::new();

        // Initialize with entry points
        for &ep in entry_points {
            if let Some(node) = state.nodes.get(&ep) {
                if node.deleted {
                    continue;
                }
                let dist = self.compute_distance(query, node);
                let od = OrderedDist(dist, ep);

                // For candidates heap: negate for Cosine/Dot so min-heap explores best first
                let heap_dist = match self.config.metric {
                    DistanceMetric::L2 => dist,
                    DistanceMetric::Cosine | DistanceMetric::Dot => -dist,
                };
                candidates.push(std::cmp::Reverse(OrderedDist(heap_dist, ep)));
                working_set.push(od);
                visited.insert(ep);
            }
        }

        // Greedy search
        while let Some(std::cmp::Reverse(curr_heap)) = candidates.pop() {
            let curr_id = curr_heap.1;

            // Get the actual node and recompute distance (candidates heap may have negated values)
            let curr_node = match state.nodes.get(&curr_id) {
                Some(node) if !node.deleted => node,
                _ => continue,
            };
            let curr_dist = self.compute_distance(query, curr_node);
            let curr = OrderedDist(curr_dist, curr_id);

            // Find worst in working set (least useful element)
            // For L2: worst = highest distance
            // For Cosine/Dot: worst = lowest score
            // Use max_by: returns element that compares as Greater = worst
            let worst = working_set.iter()
                .max_by(|a, b| {
                    if a.is_better_than(b, self.config.metric) {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    }
                })
                .copied();

            // If current is worse than worst in working set AND working set is full, we're done
            if working_set.len() >= ef {
                if let Some(worst_od) = worst {
                    if !curr.is_better_than(&worst_od, self.config.metric) {
                        break;
                    }
                }
            }

            // Expand neighbors
            if let Some(neighbors) = state.layers.get(layer).and_then(|l| l.get(&curr.1)) {
                for &neighbor_id in neighbors {
                    if visited.insert(neighbor_id) {
                        if let Some(neighbor_node) = state.nodes.get(&neighbor_id) {
                            if neighbor_node.deleted {
                                continue;
                            }
                            let neighbor_dist = self.compute_distance(query, neighbor_node);
                            let neighbor_od = OrderedDist(neighbor_dist, neighbor_id);

                            // Check if this neighbor should be added
                            let should_add = if working_set.len() < ef {
                                true
                            } else if let Some(worst_od) = worst {
                                neighbor_od.is_better_than(&worst_od, self.config.metric)
                            } else {
                                false
                            };

                            if should_add {
                                // For candidates heap: negate for Cosine/Dot
                                let heap_dist = match self.config.metric {
                                    DistanceMetric::L2 => neighbor_dist,
                                    DistanceMetric::Cosine | DistanceMetric::Dot => -neighbor_dist,
                                };
                                candidates.push(std::cmp::Reverse(OrderedDist(heap_dist, neighbor_id)));
                                working_set.push(neighbor_od);

                                // Remove worst if we exceed ef
                                if working_set.len() > ef {
                                    if let Some(worst_idx) = working_set.iter()
                                        .enumerate()
                                        .max_by(|(_, a), (_, b)| {
                                            // max_by returns element that compares as Greater = worst
                                            if a.is_better_than(b, self.config.metric) {
                                                Ordering::Less
                                            } else {
                                                Ordering::Greater
                                            }
                                        })
                                        .map(|(idx, _)| idx)
                                    {
                                        working_set.swap_remove(worst_idx);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Sort working set (best first)
        working_set.sort_by(|a, b| {
            if a.is_better_than(b, self.config.metric) {
                Ordering::Less
            } else if b.is_better_than(a, self.config.metric) {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        });

        working_set.into_iter().map(|OrderedDist(dist, id)| (dist, id)).collect()
    }

    /// Compares two distances according to the metric convention.
    fn compare_distances(&self, a: f32, b: f32) -> Ordering {
        match self.config.metric {
            DistanceMetric::L2 => {
                // Lower is better
                a.partial_cmp(&b).unwrap_or(Ordering::Equal)
            }
            DistanceMetric::Cosine | DistanceMetric::Dot => {
                // Higher is better, so reverse
                b.partial_cmp(&a).unwrap_or(Ordering::Equal)
            }
        }
    }

    /// Selects M nearest neighbors using heuristic (Algorithm 4 from paper).
    ///
    /// This heuristic maintains graph connectivity by selecting diverse neighbors
    /// rather than just the M nearest, preventing hub formation.
    fn select_neighbors_heuristic(
        &self,
        state: &HnswState,
        _query_vector: &[f32],
        candidates: Vec<(f32, DocumentId)>,
        m: usize,
        keep_pruned: bool,
    ) -> Vec<DocumentId> {
        if candidates.len() <= m {
            return candidates.into_iter().map(|(_, id)| id).collect();
        }

        // Algorithm 4 from HNSW paper
        let mut result = Vec::new();
        let mut working = candidates.clone();
        let mut discarded = Vec::new();

        // Sort candidates by distance to query (best first)
        working.sort_by(|a, b| self.compare_distances(a.0, b.0));

        while !working.is_empty() && result.len() < m {
            // Take the nearest remaining candidate
            let (cand_dist, cand_id) = working.remove(0);

            // Check if this candidate is closer to query than to any element in result
            let mut should_add = true;

            if !result.is_empty() {
                // Get candidate node
                if let Some(cand_node) = state.nodes.get(&cand_id) {
                    // Check distance to all elements in result
                    for &result_id in &result {
                        if let Some(result_node) = state.nodes.get(&result_id) {
                            let dist_to_result = self.compute_distance(&cand_node.vector, result_node);

                            // If candidate is closer to an existing result element than to query,
                            // it would create a hub, so discard it
                            match self.config.metric {
                                DistanceMetric::L2 => {
                                    // For L2: lower is better
                                    if dist_to_result < cand_dist {
                                        should_add = false;
                                        break;
                                    }
                                }
                                DistanceMetric::Cosine | DistanceMetric::Dot => {
                                    // For Cosine/Dot: higher is better
                                    if dist_to_result > cand_dist {
                                        should_add = false;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if should_add {
                result.push(cand_id);
            } else if keep_pruned {
                discarded.push((cand_dist, cand_id));
            }
        }

        // If we still need more neighbors and keepPruned is true, add from discarded
        if keep_pruned {
            for (_, id) in discarded {
                if result.len() >= m {
                    break;
                }
                result.push(id);
            }
        }

        result
    }

    /// Wrapper for select_neighbors_heuristic with default parameters.
    fn select_neighbors(
        &self,
        state: &HnswState,
        query_vector: &[f32],
        candidates: Vec<(f32, DocumentId)>,
        m: usize,
    ) -> Vec<DocumentId> {
        self.select_neighbors_heuristic(state, query_vector, candidates, m, true)
    }

    /// Adds a bidirectional edge between two nodes at a specific layer.
    fn add_edge(
        state: &mut HnswState,
        from: DocumentId,
        to: DocumentId,
        layer: usize,
    ) {
        // Ensure layer exists
        while state.layers.len() <= layer {
            state.layers.push(HashMap::new());
        }

        state
            .layers[layer]
            .entry(from)
            .or_insert_with(Vec::new)
            .push(to);
    }

    /// Prunes connections if a node exceeds M neighbors.
    ///
    /// Uses Algorithm 4 heuristic to maintain graph connectivity.
    fn prune_connections(
        &self,
        state: &mut HnswState,
        node_id: DocumentId,
        m: usize,
        layer: usize,
    ) {
        // Check if pruning is needed
        let needs_pruning = state
            .layers
            .get(layer)
            .and_then(|l| l.get(&node_id))
            .map(|neighbors| neighbors.len() > m)
            .unwrap_or(false);

        if !needs_pruning {
            return;
        }

        // Get node vector and neighbor list (immutable borrows)
        let node_vector = state.nodes.get(&node_id).unwrap().vector.clone();
        let neighbor_ids: Vec<DocumentId> = state.layers[layer][&node_id].clone();

        // Compute distances to all neighbors
        let neighbor_dists: Vec<_> = neighbor_ids
            .iter()
            .filter_map(|&neighbor_id| {
                state.nodes.get(&neighbor_id).map(|neighbor_node| {
                    let dist = self.compute_distance(&node_vector, neighbor_node);
                    (dist, neighbor_id)
                })
            })
            .collect();

        // Use Algorithm 4 heuristic to select best M neighbors (immutable borrow)
        let selected = self.select_neighbors(state, &node_vector, neighbor_dists, m);

        // Update neighbors with mutable borrow
        if let Some(neighbors) = state.layers.get_mut(layer).and_then(|l| l.get_mut(&node_id)) {
            *neighbors = selected;
        }
    }
}

#[async_trait]
impl VectorIndex for HnswIndex {
    async fn insert(&self, doc: VectorDocument) -> CoreResult<()> {
        if doc.vector.len() != self.config.dim {
            return Err(CoreError::invalid_state(format!(
                "Vector dimension mismatch: expected {}, got {}",
                self.config.dim,
                doc.vector.len()
            )));
        }

        let mut state = self.state.write();

        // Assign random layer
        let target_layer = self.assign_layer();

        // Create node
        let node = Node {
            doc_id: doc.doc_id,
            external_id: doc.external_id.clone(),
            vector: doc.vector.clone(),
            metadata: doc.metadata.clone(),
            max_layer: target_layer,
            deleted: false,
        };

        // Find entry points
        let mut entry_points = if let Some(ep) = state.entry_point {
            vec![ep]
        } else {
            // First insertion
            state.nodes.insert(doc.doc_id, node);
            state.entry_point = Some(doc.doc_id);
            state.max_layer = target_layer;
            return Ok(());
        };

        // Search from top layer down to target layer
        for layer in ((target_layer + 1)..=state.max_layer).rev() {
            let nearest = self.search_layer(&state, &doc.vector, &entry_points, 1, layer);
            if !nearest.is_empty() {
                entry_points = vec![nearest[0].1];
            }
        }

        // Insert into layers 0..=target_layer
        for layer in (0..=target_layer).rev() {
            let m = if layer == 0 {
                self.config.m0
            } else {
                self.config.m
            };

            // Find candidates
            let candidates =
                self.search_layer(&state, &doc.vector, &entry_points, self.config.ef_construction, layer);

            // Select M neighbors using Algorithm 4 heuristic
            let neighbors = self.select_neighbors(&state, &doc.vector, candidates.clone(), m);

            // Add bidirectional edges
            for &neighbor_id in &neighbors {
                Self::add_edge(&mut state, doc.doc_id, neighbor_id, layer);
                Self::add_edge(&mut state, neighbor_id, doc.doc_id, layer);

                // Prune neighbor's connections if needed
                self.prune_connections(&mut state, neighbor_id, m, layer);
            }

            // Update entry points for next layer (keep current if no neighbors found)
            if !neighbors.is_empty() {
                entry_points = neighbors;
            }
            // else: keep current entry_points for searching the next layer
        }

        // Store node
        state.nodes.insert(doc.doc_id, node);

        // Update entry point if this node is on a higher layer
        if target_layer > state.max_layer {
            state.entry_point = Some(doc.doc_id);
            state.max_layer = target_layer;
        }

        Ok(())
    }

    async fn search(
        &self,
        query: &[f32],
        k: usize,
        ef_search: Option<usize>,
    ) -> CoreResult<Vec<SearchResult>> {
        if query.len() != self.config.dim {
            return Err(CoreError::invalid_state(format!(
                "Query dimension mismatch: expected {}, got {}",
                self.config.dim,
                query.len()
            )));
        }

        let state = self.state.read();

        let ef = ef_search.unwrap_or(self.config.ef_search).max(k);

        // Handle empty index
        let entry_point = match state.entry_point {
            Some(ep) => ep,
            None => return Ok(Vec::new()),
        };

        let mut entry_points = vec![entry_point];

        // Search from top layer down to layer 1
        for layer in (1..=state.max_layer).rev() {
            let nearest = self.search_layer(&state, query, &entry_points, 1, layer);
            if !nearest.is_empty() {
                entry_points = vec![nearest[0].1];
            }
        }

        // Search layer 0 with ef parameter
        let candidates = self.search_layer(&state, query, &entry_points, ef, 0);

        // Convert to SearchResult and return top-k
        let results: Vec<SearchResult> = candidates
            .into_iter()
            .take(k)
            .filter_map(|(score, doc_id)| {
                state.nodes.get(&doc_id).map(|node| {
                    let mut result = SearchResult::new(doc_id, score);
                    if let Some(ref ext_id) = node.external_id {
                        result = result.with_external_id(ext_id.clone());
                    }
                    if let Some(ref meta) = node.metadata {
                        result = result.with_metadata(meta.clone());
                    }
                    result
                })
            })
            .collect();

        Ok(results)
    }

    async fn delete(&self, doc_id: DocumentId) -> CoreResult<()> {
        let mut state = self.state.write();

        // Soft delete with tombstone
        if let Some(node) = state.nodes.get_mut(&doc_id) {
            node.deleted = true;
        }

        Ok(())
    }

    async fn get(&self, doc_id: DocumentId) -> CoreResult<Option<VectorDocument>> {
        let state = self.state.read();

        Ok(state.nodes.get(&doc_id).and_then(|node| {
            if node.deleted {
                None
            } else {
                Some(VectorDocument {
                    doc_id: node.doc_id,
                    external_id: node.external_id.clone(),
                    vector: node.vector.clone(),
                    metadata: node.metadata.clone(),
                    inserted_at: chrono::Utc::now(), // Note: We don't store inserted_at in HNSW node
                })
            }
        }))
    }

    async fn count(&self) -> CoreResult<usize> {
        let state = self.state.read();
        let count = state.nodes.values().filter(|n| !n.deleted).count();
        Ok(count)
    }

    async fn clear(&self) -> CoreResult<()> {
        let mut state = self.state.write();
        state.nodes.clear();
        state.layers.clear();
        state.layers.push(HashMap::new()); // Reset to layer 0
        state.entry_point = None;
        state.max_layer = 0;
        Ok(())
    }
}

/// Wrapper for f32 to implement Ord for use in BinaryHeap.
/// Orders by f32 value (lower first by default).
#[derive(Debug, Clone, Copy)]
struct OrderedDist(f32, DocumentId);

impl OrderedDist {
    /// Returns ordering for "better" comparison (metric-aware).
    /// For L2: lower is better
    /// For Cosine/Dot: higher is better
    fn is_better_than(&self, other: &Self, metric: DistanceMetric) -> bool {
        match metric {
            DistanceMetric::L2 => self.0 < other.0,
            DistanceMetric::Cosine | DistanceMetric::Dot => self.0 > other.0,
        }
    }
}

impl PartialEq for OrderedDist {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for OrderedDist {}

impl PartialOrd for OrderedDist {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Ord for OrderedDist {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hnsw_insert_and_get() {
        let config = HnswConfig::balanced(3, DistanceMetric::Cosine);
        let index = HnswIndex::new(config);

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
    async fn test_hnsw_search_small() {
        let config = HnswConfig::balanced(3, DistanceMetric::Cosine);
        let index = HnswIndex::new(config);

        // Insert 3 documents
        let doc1 = VectorDocument::new(DocumentId::new(), vec![1.0, 0.0, 0.0]);
        let doc2 = VectorDocument::new(DocumentId::new(), vec![0.0, 1.0, 0.0]);
        let doc3 = VectorDocument::new(DocumentId::new(), vec![1.0, 0.0, 0.0]); // Same as doc1

        index.insert(doc1.clone()).await.unwrap();
        index.insert(doc2).await.unwrap();
        index.insert(doc3.clone()).await.unwrap();

        // Query with [1, 0, 0] - should match doc1 and doc3
        let results = index.search(&[1.0, 0.0, 0.0], 2, None).await.unwrap();

        assert_eq!(results.len(), 2);
        // Both results should have high cosine similarity
        assert!(results[0].score > 0.99);
        assert!(results[1].score > 0.99);
    }

    #[tokio::test]
    async fn test_hnsw_delete() {
        let config = HnswConfig::balanced(2, DistanceMetric::Cosine);
        let index = HnswIndex::new(config);

        let doc_id = DocumentId::new();
        let doc = VectorDocument::new(doc_id, vec![1.0, 2.0]);

        index.insert(doc).await.unwrap();
        assert_eq!(index.count().await.unwrap(), 1);

        index.delete(doc_id).await.unwrap();
        assert_eq!(index.count().await.unwrap(), 0);
        assert!(index.get(doc_id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_hnsw_insert_many() {
        let config = HnswConfig::edge_cache(2, DistanceMetric::L2);
        let index = HnswIndex::new(config);

        // Insert 100 documents
        for i in 0..100 {
            let doc = VectorDocument::new(DocumentId::new(), vec![i as f32, (i * 2) as f32]);
            index.insert(doc).await.unwrap();
        }

        assert_eq!(index.count().await.unwrap(), 100);

        // Search for [0, 0] - should find nearby points
        let results = index.search(&[0.0, 0.0], 5, None).await.unwrap();
        assert_eq!(results.len(), 5);
        // First result should have smallest distance
        assert!(results[0].score < results[1].score);
    }

    #[tokio::test]
    async fn test_hnsw_dimension_mismatch() {
        let config = HnswConfig::balanced(3, DistanceMetric::Cosine);
        let index = HnswIndex::new(config);

        // Try to insert wrong dimension
        let doc = VectorDocument::new(DocumentId::new(), vec![1.0, 2.0]);
        let result = index.insert(doc).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("dimension mismatch"));
    }

    #[tokio::test]
    async fn test_hnsw_clear() {
        let config = HnswConfig::balanced(2, DistanceMetric::Cosine);
        let index = HnswIndex::new(config);

        for i in 0..10 {
            let doc = VectorDocument::new(DocumentId::new(), vec![i as f32, 0.0]);
            index.insert(doc).await.unwrap();
        }

        assert_eq!(index.count().await.unwrap(), 10);

        index.clear().await.unwrap();
        assert_eq!(index.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_hnsw_configs() {
        let balanced = HnswConfig::balanced(512, DistanceMetric::Cosine);
        assert_eq!(balanced.m, 32);
        assert_eq!(balanced.m0, 64);
        assert_eq!(balanced.ef_construction, 200);

        let edge = HnswConfig::edge_cache(512, DistanceMetric::L2);
        assert_eq!(edge.m, 16);
        assert_eq!(edge.ef_search, 64);

        let high = HnswConfig::high_recall(512, DistanceMetric::Dot);
        assert_eq!(high.m, 48);
        assert_eq!(high.ef_construction, 320);
    }
}
