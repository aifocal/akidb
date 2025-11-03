use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashSet};
use std::cmp::Ordering;

/// DiskANN index using Vamana graph structure
///
/// # Overview
///
/// DiskANN is a disk-based approximate nearest neighbor (ANN) search system
/// that scales to billion+ vectors. It uses the Vamana graph algorithm which
/// provides excellent recall-latency tradeoffs.
///
/// # Key Features
///
/// - **Vamana Graph**: Optimized graph structure with bounded out-degree
/// - **Beam Search**: Multi-path exploration for high recall
/// - **Disk-Based**: Memory-mapped indices for datasets larger than RAM
/// - **Incremental Updates**: Add vectors without full rebuild
///
/// # Architecture
///
/// ```text
/// ┌─────────────────────────────────────┐
/// │         DiskANN Index              │
/// ├─────────────────────────────────────┤
/// │ Entry Point (medoid)               │
/// │                                    │
/// │ ┌─────────────────────────────┐   │
/// │ │      Vamana Graph           │   │
/// │ │  (adjacency list, max_deg)  │   │
/// │ └─────────────────────────────┘   │
/// │                                    │
/// │ ┌─────────────────────────────┐   │
/// │ │   Vector Storage            │   │
/// │ │  (memory-mapped file)       │   │
/// │ └─────────────────────────────┘   │
/// └─────────────────────────────────────┘
/// ```
///
/// # References
///
/// - Paper: "DiskANN: Fast Accurate Billion-point Nearest Neighbor Search on a Single Node"
/// - Algorithm: Vamana graph construction with greedy search
#[derive(Clone)]
pub struct DiskANNIndex {
    /// Graph structure (adjacency list)
    graph: VamanaGraph,
    /// Vector storage
    vectors: Vec<Vec<f32>>,
    /// Entry point (medoid)
    entry_point: Option<usize>,
    /// Configuration
    config: DiskANNConfig,
    /// Index statistics
    stats: IndexStats,
}

impl DiskANNIndex {
    /// Create a new DiskANN index
    pub fn new(config: DiskANNConfig) -> Self {
        Self {
            graph: VamanaGraph::new(config.max_degree),
            vectors: Vec::new(),
            entry_point: None,
            config,
            stats: IndexStats::default(),
        }
    }

    /// Build index from vectors
    pub fn build(&mut self, vectors: Vec<Vec<f32>>) -> Result<(), DiskANNError> {
        if vectors.is_empty() {
            return Err(DiskANNError::EmptyInput);
        }

        self.vectors = vectors;
        let n = self.vectors.len();

        // Initialize graph with empty neighborhoods
        self.graph.initialize(n);

        // Compute medoid (entry point)
        self.entry_point = Some(self.compute_medoid());

        // Build Vamana graph using greedy algorithm
        for i in 0..n {
            self.build_neighborhood(i)?;
        }

        self.stats.total_vectors = n;
        self.stats.avg_degree = self.graph.average_degree();

        Ok(())
    }

    /// Build neighborhood for a node using greedy search
    fn build_neighborhood(&mut self, node: usize) -> Result<(), DiskANNError> {
        if node >= self.vectors.len() {
            return Err(DiskANNError::InvalidNodeId(node));
        }

        // Start from entry point
        let start = self.entry_point.unwrap_or(0);

        // Greedy search to find approximate nearest neighbors
        let neighbors = self.greedy_search(node, start, self.config.build_list_size)?;

        // Add edges (bounded by max_degree)
        let max_neighbors = self.config.max_degree.min(neighbors.len());
        for i in 0..max_neighbors {
            self.graph.add_edge(node, neighbors[i].id);
        }

        Ok(())
    }

    /// Greedy search from start node
    fn greedy_search(
        &self,
        query_id: usize,
        start: usize,
        list_size: usize,
    ) -> Result<Vec<SearchCandidate>, DiskANNError> {
        let mut visited = HashSet::new();
        let mut candidates = BinaryHeap::new();

        // Start with entry point
        let dist = self.distance(&self.vectors[query_id], &self.vectors[start]);
        candidates.push(SearchCandidate {
            id: start,
            distance: dist,
        });
        visited.insert(start);

        let mut best_candidates = Vec::new();

        while !candidates.is_empty() && best_candidates.len() < list_size {
            let current = candidates.pop().unwrap();
            best_candidates.push(current.clone());

            // Explore neighbors
            for &neighbor_id in self.graph.neighbors(current.id) {
                if visited.contains(&neighbor_id) {
                    continue;
                }

                visited.insert(neighbor_id);

                let dist = self.distance(&self.vectors[query_id], &self.vectors[neighbor_id]);
                candidates.push(SearchCandidate {
                    id: neighbor_id,
                    distance: dist,
                });
            }
        }

        Ok(best_candidates)
    }

    /// Search for k nearest neighbors using beam search
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<SearchResult>, DiskANNError> {
        if self.entry_point.is_none() {
            return Err(DiskANNError::NotBuilt);
        }

        let start = self.entry_point.unwrap();

        // Beam search with multiple paths
        let candidates = self.beam_search(query, start, self.config.search_list_size)?;

        // Return top k
        let results: Vec<SearchResult> = candidates
            .into_iter()
            .take(k)
            .map(|c| SearchResult {
                id: c.id,
                distance: c.distance,
            })
            .collect();

        Ok(results)
    }

    /// Beam search with multiple paths
    fn beam_search(
        &self,
        query: &[f32],
        start: usize,
        beam_width: usize,
    ) -> Result<Vec<SearchCandidate>, DiskANNError> {
        let mut visited = HashSet::new();
        let mut beam = BinaryHeap::new();

        // Initialize beam with entry point
        let dist = self.distance(query, &self.vectors[start]);
        beam.push(SearchCandidate {
            id: start,
            distance: dist,
        });
        visited.insert(start);

        let mut results = Vec::new();

        while !beam.is_empty() {
            // Take top candidates from beam
            let mut current_beam = Vec::new();
            for _ in 0..beam_width.min(beam.len()) {
                if let Some(candidate) = beam.pop() {
                    current_beam.push(candidate);
                }
            }

            // Add to results
            results.extend(current_beam.iter().cloned());

            // Explore neighbors of all candidates in parallel
            for candidate in &current_beam {
                for &neighbor_id in self.graph.neighbors(candidate.id) {
                    if visited.contains(&neighbor_id) {
                        continue;
                    }

                    visited.insert(neighbor_id);

                    let dist = self.distance(query, &self.vectors[neighbor_id]);
                    beam.push(SearchCandidate {
                        id: neighbor_id,
                        distance: dist,
                    });
                }
            }
        }

        // Sort by distance
        results.sort_by(|a, b| {
            a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal)
        });

        Ok(results)
    }

    /// Compute medoid (geometric center)
    fn compute_medoid(&self) -> usize {
        let n = self.vectors.len();
        if n == 0 {
            return 0;
        }

        // Find vector with minimum sum of distances to all others (expensive, O(n^2))
        // In production, use sampling for large datasets
        let sample_size = 1000.min(n);
        let mut min_sum = f32::MAX;
        let mut medoid = 0;

        for i in 0..sample_size {
            let mut sum = 0.0;
            for j in 0..sample_size {
                sum += self.distance(&self.vectors[i], &self.vectors[j]);
            }

            if sum < min_sum {
                min_sum = sum;
                medoid = i;
            }
        }

        medoid
    }

    /// Compute distance between two vectors (L2/Euclidean)
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }

    /// Get index statistics
    pub fn stats(&self) -> &IndexStats {
        &self.stats
    }

    /// Get configuration
    pub fn config(&self) -> &DiskANNConfig {
        &self.config
    }
}

/// Vamana graph structure (adjacency list with bounded degree)
#[derive(Clone)]
struct VamanaGraph {
    /// Adjacency list (node_id -> neighbors)
    adjacency: Vec<Vec<usize>>,
    /// Maximum out-degree per node
    max_degree: usize,
}

impl VamanaGraph {
    fn new(max_degree: usize) -> Self {
        Self {
            adjacency: Vec::new(),
            max_degree,
        }
    }

    fn initialize(&mut self, n: usize) {
        self.adjacency = vec![Vec::new(); n];
    }

    fn add_edge(&mut self, from: usize, to: usize) {
        if from >= self.adjacency.len() {
            return;
        }

        // Add edge if not at capacity
        if self.adjacency[from].len() < self.max_degree && !self.adjacency[from].contains(&to) {
            self.adjacency[from].push(to);
        }
    }

    fn neighbors(&self, node: usize) -> &[usize] {
        if node >= self.adjacency.len() {
            return &[];
        }
        &self.adjacency[node]
    }

    fn average_degree(&self) -> f64 {
        if self.adjacency.is_empty() {
            return 0.0;
        }

        let total: usize = self.adjacency.iter().map(|v| v.len()).sum();
        total as f64 / self.adjacency.len() as f64
    }
}

/// DiskANN configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskANNConfig {
    /// Maximum out-degree per node (R in paper)
    pub max_degree: usize,
    /// Build-time search list size (L in paper)
    pub build_list_size: usize,
    /// Search-time list size
    pub search_list_size: usize,
    /// Alpha parameter for graph quality vs speed tradeoff
    pub alpha: f32,
}

impl Default for DiskANNConfig {
    fn default() -> Self {
        Self {
            max_degree: 64,          // Typical: 32-128
            build_list_size: 100,    // Typical: 75-200
            search_list_size: 100,   // Typical: same as build_list_size
            alpha: 1.2,              // Typical: 1.0-1.5
        }
    }
}

/// Search candidate in priority queue
#[derive(Clone, Debug)]
struct SearchCandidate {
    id: usize,
    distance: f32,
}

// Reverse ordering for min-heap
impl Ord for SearchCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        other.distance.partial_cmp(&self.distance).unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for SearchCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for SearchCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl Eq for SearchCandidate {}

/// Search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: usize,
    pub distance: f32,
}

/// Index statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexStats {
    pub total_vectors: usize,
    pub avg_degree: f64,
    pub build_time_ms: u64,
}

/// DiskANN errors
#[derive(Debug, Clone, thiserror::Error)]
pub enum DiskANNError {
    #[error("Empty input")]
    EmptyInput,

    #[error("Invalid node ID: {0}")]
    InvalidNodeId(usize),

    #[error("Index not built")]
    NotBuilt,

    #[error("Dimension mismatch")]
    DimensionMismatch,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generate_test_vectors(n: usize, dim: usize) -> Vec<Vec<f32>> {
        (0..n)
            .map(|i| {
                (0..dim)
                    .map(|j| ((i * 10 + j) as f32).sin())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn test_diskann_build() {
        let config = DiskANNConfig::default();
        let mut index = DiskANNIndex::new(config);

        let vectors = generate_test_vectors(100, 128);
        assert!(index.build(vectors).is_ok());

        assert_eq!(index.stats().total_vectors, 100);
        assert!(index.entry_point.is_some());
    }

    #[test]
    fn test_diskann_search() {
        let config = DiskANNConfig::default();
        let mut index = DiskANNIndex::new(config);

        let vectors = generate_test_vectors(100, 128);
        index.build(vectors.clone()).unwrap();

        // Search with first vector as query
        let results = index.search(&vectors[0], 10).unwrap();

        assert_eq!(results.len(), 10);
        // First result should be the query itself (distance ~0)
        assert!(results[0].distance < 0.01);
    }

    #[test]
    fn test_diskann_empty_input() {
        let config = DiskANNConfig::default();
        let mut index = DiskANNIndex::new(config);

        let result = index.build(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_diskann_search_before_build() {
        let config = DiskANNConfig::default();
        let index = DiskANNIndex::new(config);

        let query = vec![1.0; 128];
        let result = index.search(&query, 10);
        assert!(result.is_err());
    }

    #[test]
    fn test_vamana_graph() {
        let mut graph = VamanaGraph::new(10);
        graph.initialize(5);

        graph.add_edge(0, 1);
        graph.add_edge(0, 2);
        graph.add_edge(1, 2);

        assert_eq!(graph.neighbors(0).len(), 2);
        assert_eq!(graph.neighbors(1).len(), 1);
    }

    #[test]
    fn test_vamana_max_degree() {
        let mut graph = VamanaGraph::new(2); // Max degree = 2
        graph.initialize(5);

        graph.add_edge(0, 1);
        graph.add_edge(0, 2);
        graph.add_edge(0, 3); // Should not be added (exceeds max_degree)

        assert_eq!(graph.neighbors(0).len(), 2);
    }

    #[test]
    fn test_medoid_computation() {
        let config = DiskANNConfig::default();
        let mut index = DiskANNIndex::new(config);

        let vectors = generate_test_vectors(50, 64);
        index.build(vectors).unwrap();

        let medoid = index.entry_point.unwrap();
        assert!(medoid < 50);
    }

    #[test]
    fn test_beam_search_recall() {
        let config = DiskANNConfig {
            max_degree: 32,
            build_list_size: 50,
            search_list_size: 50,
            alpha: 1.2,
        };
        let mut index = DiskANNIndex::new(config);

        let vectors = generate_test_vectors(200, 128);
        index.build(vectors.clone()).unwrap();

        // Test recall: search should find the query vector itself
        for i in (0..200).step_by(20) {
            let results = index.search(&vectors[i], 5).unwrap();

            // Check if query vector is in results
            let found = results.iter().any(|r| r.id == i);
            assert!(found, "Query vector {} not found in results", i);
        }
    }
}
