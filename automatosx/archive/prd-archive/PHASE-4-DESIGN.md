# AkiDB 2.0 - Phase 4: Vector Engine Design

**Date:** 2025-11-06
**Phase:** 4 (Vector Engine)
**Status:** Design Complete
**Prerequisites:** Phase 1 (Metadata), Phase 2 (Embeddings), Phase 3 (RBAC)

---

## Executive Summary

Phase 4 delivers the core vector search engine for AkiDB 2.0, providing RAM-first vector indexing with sub-25ms P95 latency. This phase takes an **incremental approach**: start with a simple, correct brute-force baseline, then optimize with HNSW graph-based indexing and ARM NEON SIMD acceleration.

**Key Deliverables:**
1. Domain model: `VectorDocument` with metadata
2. Trait interface: `VectorIndex` with insert/search/delete operations
3. Baseline implementation: `BruteForceIndex` for correctness validation
4. Optimized implementation: `HnswIndex` with configurable parameters
5. Distance metrics: Cosine, Euclidean (L2), Dot Product with SIMD
6. Integration tests: 15+ tests covering all CRUD operations
7. Performance benchmarks: Validate P95 < 25ms @ 50 QPS

**Performance Targets (Balanced Default Profile):**
- Latency: P95 < 25ms, P99 < 40ms @ 50 QPS
- Scale: 5M-30M vectors, ≤100GB RAM
- Recall: >0.95 @ k=10
- HNSW params: M=32, efConstruction=200, efSearch=128

---

## 1. Domain Model

### 1.1 VectorDocument

Primary entity representing a vector with metadata.

```rust
/// A vector document stored in the index.
#[derive(Debug, Clone)]
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
    pub fn new(doc_id: DocumentId, vector: Vec<f32>) -> Self {
        Self {
            doc_id,
            external_id: None,
            vector,
            metadata: None,
            inserted_at: Utc::now(),
        }
    }

    pub fn with_external_id(mut self, external_id: String) -> Self {
        self.external_id = Some(external_id);
        self
    }

    pub fn with_metadata(mut self, metadata: JsonValue) -> Self {
        self.metadata = Some(metadata);
        self
    }
}
```

**Design Rationale:**
- `DocumentId` (UUID v7) ensures time-ordered insertion
- `external_id` allows user-defined identifiers (e.g., "doc-123")
- `metadata` stores arbitrary JSON for filtering/enrichment
- `vector` uses `Vec<f32>` for flexibility (converts to SIMD types internally)

### 1.2 SearchResult

Result of a vector search query.

```rust
/// Result of a vector search operation.
#[derive(Debug, Clone)]
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
    pub fn new(doc_id: DocumentId, score: f32) -> Self {
        Self {
            doc_id,
            external_id: None,
            score,
            metadata: None,
        }
    }
}
```

### 1.3 DistanceMetric

Supported distance/similarity functions.

```rust
/// Distance metric for vector similarity computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DistanceMetric {
    /// Cosine similarity (normalized dot product)
    /// Range: [-1, 1], higher is more similar
    Cosine,

    /// Euclidean distance (L2 norm)
    /// Range: [0, ∞), lower is more similar
    Euclidean,

    /// Dot product (unnormalized)
    /// Range: (-∞, ∞), higher is more similar
    DotProduct,
}

impl DistanceMetric {
    pub fn compute(&self, a: &[f32], b: &[f32]) -> f32 {
        match self {
            Self::Cosine => cosine_similarity(a, b),
            Self::Euclidean => euclidean_distance(a, b),
            Self::DotProduct => dot_product(a, b),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cosine => "cosine",
            Self::Euclidean => "euclidean",
            Self::DotProduct => "dotproduct",
        }
    }
}
```

**Distance Function Implementations:**

```rust
/// Cosine similarity: dot(a, b) / (||a|| * ||b||)
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

/// Euclidean distance: sqrt(sum((a_i - b_i)^2))
pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "Vector dimensions must match");

    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

/// Dot product: sum(a_i * b_i)
pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "Vector dimensions must match");

    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}
```

**SIMD Optimization (Future):**
- ARM NEON intrinsics for 4x speedup (`vfmaq_laneq_f32`)
- Compile with `#[target_feature(enable = "neon")]`
- Fallback to scalar for non-ARM platforms

---

## 2. VectorIndex Trait

Core abstraction for vector index implementations.

```rust
/// Vector index trait for insert, search, and delete operations.
#[async_trait]
pub trait VectorIndex: Send + Sync {
    /// Insert a vector document into the index.
    async fn insert(&self, doc: VectorDocument) -> CoreResult<()>;

    /// Insert multiple documents in a batch.
    async fn insert_batch(&self, docs: Vec<VectorDocument>) -> CoreResult<()> {
        for doc in docs {
            self.insert(doc).await?;
        }
        Ok(())
    }

    /// Search for k nearest neighbors.
    ///
    /// Returns results sorted by score (ascending for distance, descending for similarity).
    async fn search(
        &self,
        query: &[f32],
        k: usize,
        ef_search: Option<usize>,
    ) -> CoreResult<Vec<SearchResult>>;

    /// Delete a document by ID.
    async fn delete(&self, doc_id: DocumentId) -> CoreResult<()>;

    /// Get a document by ID (for verification).
    async fn get(&self, doc_id: DocumentId) -> CoreResult<Option<VectorDocument>>;

    /// Count total documents in the index.
    async fn count(&self) -> CoreResult<usize>;

    /// Clear the entire index (for testing).
    async fn clear(&self) -> CoreResult<()>;
}
```

**Design Notes:**
- `async_trait` for future WAL integration and disk I/O
- `ef_search` parameter allows runtime tuning (HNSW only)
- `insert_batch` enables optimized bulk loading
- `search` returns results sorted by distance metric convention

---

## 3. Implementation Strategy

### 3.1 Phase 4A: Brute-Force Baseline (Week 1)

**Goal:** Establish correctness with a simple, exhaustive search implementation.

```rust
/// Brute-force linear scan index (baseline for correctness).
pub struct BruteForceIndex {
    /// Vector dimension
    dim: usize,

    /// Distance metric
    metric: DistanceMetric,

    /// In-memory document storage
    documents: Arc<RwLock<HashMap<DocumentId, VectorDocument>>>,
}

impl BruteForceIndex {
    pub fn new(dim: usize, metric: DistanceMetric) -> Self {
        Self {
            dim,
            metric,
            documents: Arc::new(RwLock::new(HashMap::new())),
        }
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
                SearchResult {
                    doc_id: doc.doc_id,
                    external_id: doc.external_id.clone(),
                    score,
                    metadata: doc.metadata.clone(),
                }
            })
            .collect();

        // Sort by score (ascending for distance, descending for similarity)
        match self.metric {
            DistanceMetric::Euclidean => results.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap()),
            DistanceMetric::Cosine | DistanceMetric::DotProduct => {
                results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap())
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
```

**Characteristics:**
- Time complexity: O(n·d) per search (n = docs, d = dimension)
- Space complexity: O(n·d)
- Expected performance: ~5ms for 10k vectors (512-dim, ARM M3)
- Use case: Testing, small collections (< 10k vectors)

### 3.2 Phase 4B: HNSW Implementation (Week 2-3)

**Goal:** Production-grade approximate nearest neighbor (ANN) search.

#### HNSW Parameters

```rust
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

    /// Construction-time EF parameter
    /// Range: [100, 1000], Default: 200
    pub ef_construction: usize,

    /// Search-time EF parameter (default)
    /// Range: [10, 500], Default: 128
    pub ef_search: usize,

    /// Maximum layer (ml = 1/ln(2) ≈ 1.44)
    pub ml: f32,
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            dim: 512,
            metric: DistanceMetric::Cosine,
            m: 32,
            ef_construction: 200,
            ef_search: 128,
            ml: 1.0 / 2.0_f32.ln(),
        }
    }
}
```

**Tuning Guide (from PRD Section 3.1):**

| Workload Profile | Data Scale | Latency Target | (M, efConstruction, efSearch) |
|------------------|------------|----------------|-------------------------------|
| Edge Cache       | ≤5M        | P95 ≤15ms      | (16, 80, 64)                  |
| Balanced Default | 5M-30M     | P95 ≤25ms      | (32, 200, 128)                |
| High Recall      | 30M-100M   | P95 ≤40ms      | (48, 320, 256)                |

#### HNSW Structure

```rust
/// HNSW index with hierarchical graph structure.
pub struct HnswIndex {
    /// Configuration
    config: HnswConfig,

    /// Graph layers (layer 0 is the base layer with all nodes)
    layers: Vec<Layer>,

    /// Node storage (doc_id -> node metadata)
    nodes: HashMap<DocumentId, Node>,

    /// Vector storage (contiguous for cache efficiency)
    vectors: Vec<f32>,

    /// Entry point (top-level node)
    entry_point: Option<DocumentId>,
}

struct Layer {
    /// Adjacency list: node_id -> neighbor_ids
    adjacency: HashMap<DocumentId, Vec<DocumentId>>,
}

struct Node {
    /// Document ID
    doc_id: DocumentId,

    /// External ID
    external_id: Option<String>,

    /// Metadata
    metadata: Option<JsonValue>,

    /// Vector offset in vectors array
    vector_offset: usize,

    /// Maximum layer this node appears in
    max_layer: usize,

    /// Tombstone flag (soft delete)
    deleted: bool,
}
```

**HNSW Algorithm Pseudocode:**

**Insert:**
```rust
fn insert(&mut self, doc: VectorDocument) {
    // 1. Assign layer: l ~ floor(-ln(rand()) * ml)
    let layer = self.assign_layer();

    // 2. Find entry points at each layer (greedy search from top)
    let mut entry_points = vec![self.entry_point];
    for l in (layer + 1..self.layers.len()).rev() {
        entry_points = self.search_layer(&doc.vector, entry_points, 1, l);
    }

    // 3. Insert into layers 0..=layer
    for l in (0..=layer).rev() {
        // Find M nearest neighbors
        let candidates = self.search_layer(&doc.vector, entry_points, self.config.ef_construction, l);
        let neighbors = self.select_neighbors_heuristic(doc.doc_id, candidates, self.config.m);

        // Add bidirectional links
        for neighbor in &neighbors {
            self.add_edge(doc.doc_id, *neighbor, l);
            self.add_edge(*neighbor, doc.doc_id, l);

            // Prune neighbor's connections if exceeds M
            self.prune_connections(*neighbor, self.config.m, l);
        }

        entry_points = neighbors;
    }

    // 4. Update entry point if this node is on a higher layer
    if layer > self.entry_point_layer() {
        self.entry_point = Some(doc.doc_id);
    }
}
```

**Search:**
```rust
fn search(&self, query: &[f32], k: usize, ef_search: usize) -> Vec<SearchResult> {
    // 1. Start from entry point, traverse down to layer 0
    let mut entry_points = vec![self.entry_point];
    for l in (1..self.layers.len()).rev() {
        entry_points = self.search_layer(query, entry_points, 1, l);
    }

    // 2. Search layer 0 with ef_search candidates
    let candidates = self.search_layer(query, entry_points, ef_search, 0);

    // 3. Return top-k results
    candidates.into_iter().take(k).collect()
}

fn search_layer(&self, query: &[f32], entry_points: Vec<DocumentId>, ef: usize, layer: usize) -> Vec<DocumentId> {
    let mut visited = HashSet::new();
    let mut candidates = BinaryHeap::new(); // Max-heap
    let mut results = BinaryHeap::new();    // Min-heap of top ef

    // Initialize with entry points
    for ep in entry_points {
        let dist = self.compute_distance(query, ep);
        candidates.push(Reverse((OrderedFloat(dist), ep)));
        results.push((OrderedFloat(dist), ep));
        visited.insert(ep);
    }

    // Greedy search
    while let Some(Reverse((dist, node))) = candidates.pop() {
        if dist > results.peek().unwrap().0 {
            break; // All remaining candidates are farther
        }

        // Expand neighbors
        for neighbor in self.get_neighbors(node, layer) {
            if visited.insert(neighbor) {
                let neighbor_dist = self.compute_distance(query, neighbor);

                if neighbor_dist < results.peek().unwrap().0 || results.len() < ef {
                    candidates.push(Reverse((OrderedFloat(neighbor_dist), neighbor)));
                    results.push((OrderedFloat(neighbor_dist), neighbor));

                    if results.len() > ef {
                        results.pop();
                    }
                }
            }
        }
    }

    results.into_sorted_vec().into_iter().map(|(_, id)| id).collect()
}
```

**Complexity:**
- Insert: O(M · log(n) · d) amortized
- Search: O(log(n) · d) with high probability
- Space: O(n · M · log(n))

### 3.3 Concurrency Model

**Read-Write Lock Strategy:**
- Reads: Multiple concurrent searches with `Arc<RwLock<_>>` read locks
- Writes: Exclusive lock for inserts/deletes
- Future: Lock-free reads via `ArcSwap` snapshots (Phase 5)

**Thread Safety:**
- All index implementations are `Send + Sync`
- Internal state protected by `RwLock` or `Mutex`
- Search operations are read-only after index construction

---

## 4. Distance Metrics & SIMD

### 4.1 Scalar Implementations

Provided in Section 1.3 above (cosine, euclidean, dot product).

### 4.2 ARM NEON SIMD (Future Optimization)

**Dot Product with NEON:**

```rust
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn dot_product_neon(a: &[f32], b: &[f32]) -> f32 {
    use std::arch::aarch64::*;

    let len = a.len();
    let chunks = len / 4;
    let remainder = len % 4;

    let mut sum = vdupq_n_f32(0.0);

    for i in 0..chunks {
        let a_vec = vld1q_f32(a.as_ptr().add(i * 4));
        let b_vec = vld1q_f32(b.as_ptr().add(i * 4));
        sum = vfmaq_f32(sum, a_vec, b_vec);
    }

    // Horizontal sum
    let mut result = vaddvq_f32(sum);

    // Handle remainder
    for i in (chunks * 4)..len {
        result += a[i] * b[i];
    }

    result
}
```

**Expected Speedup:**
- NEON: 3-4x faster than scalar on ARM (M1/M2/M3, Jetson)
- Critical for sub-25ms latency at scale

---

## 5. Crate Structure

### 5.1 New Crate: `akidb-index`

```
crates/akidb-index/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public exports
│   ├── brute_force.rs      # BruteForceIndex
│   ├── hnsw.rs             # HnswIndex
│   ├── distance.rs         # Distance metrics
│   ├── simd.rs             # SIMD optimizations
│   └── types.rs            # VectorDocument, SearchResult
├── benches/
│   └── index_bench.rs      # Criterion benchmarks
└── tests/
    └── integration_test.rs # Integration tests
```

**Cargo.toml:**

```toml
[package]
name = "akidb-index"
version.workspace = true
edition.workspace = true

[dependencies]
akidb-core = { path = "../akidb-core" }
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
uuid.workspace = true
thiserror.workspace = true
async-trait = "0.1"
parking_lot = "0.12"        # RwLock
ordered-float = "4.0"       # OrderedFloat for BinaryHeap

[dev-dependencies]
criterion.workspace = true
tokio = { workspace = true, features = ["test-util"] }
rand = "0.8"

[[bench]]
name = "index_bench"
harness = false
```

### 5.2 Updates to `akidb-core`

**Add to `crates/akidb-core/src/lib.rs`:**

```rust
mod vector;
pub use vector::{VectorDocument, SearchResult, DistanceMetric};

mod traits;
pub use traits::VectorIndex;
```

**Create `crates/akidb-core/src/vector.rs`:**
- VectorDocument struct (see Section 1.1)
- SearchResult struct (see Section 1.2)
- DistanceMetric enum (see Section 1.3)

**Add to `crates/akidb-core/src/traits.rs`:**
- VectorIndex trait (see Section 2)

---

## 6. Testing Strategy

### 6.1 Unit Tests (12 tests)

**Distance Metrics (3 tests):**
- `test_cosine_similarity_orthogonal`
- `test_euclidean_distance_zero`
- `test_dot_product_parallel`

**BruteForceIndex (9 tests):**
- `test_insert_and_get`
- `test_insert_dimension_mismatch`
- `test_search_cosine_similarity`
- `test_search_euclidean_distance`
- `test_search_returns_top_k`
- `test_delete_removes_document`
- `test_batch_insert`
- `test_clear_empties_index`
- `test_count_returns_document_count`

### 6.2 Integration Tests (6 tests)

**BruteForce + HNSW Comparison:**
- `test_brute_force_vs_hnsw_recall` - Validate HNSW recall ≥ 0.95
- `test_hnsw_insert_1000_documents` - Bulk insert performance
- `test_hnsw_search_latency` - P95 < 25ms @ 10k vectors
- `test_hnsw_incremental_insert` - Insert after search
- `test_hnsw_soft_delete_tombstone` - Soft delete handling
- `test_hnsw_entry_point_update` - Entry point layer update

### 6.3 Performance Benchmarks

**Criterion Benchmarks:**

```rust
fn bench_brute_force_search(c: &mut Criterion) {
    let index = BruteForceIndex::new(512, DistanceMetric::Cosine);

    // Insert 10k vectors
    for _ in 0..10_000 {
        let vec = generate_random_vector(512);
        let doc = VectorDocument::new(DocumentId::new_v7(), vec);
        tokio_block_on(index.insert(doc)).unwrap();
    }

    let query = generate_random_vector(512);

    c.bench_function("brute_force_search_10k", |b| {
        b.iter(|| {
            tokio_block_on(index.search(&query, 10, None)).unwrap()
        });
    });
}

fn bench_hnsw_search(c: &mut Criterion) {
    let config = HnswConfig::default();
    let index = HnswIndex::new(config);

    // Insert 100k vectors
    for _ in 0..100_000 {
        let vec = generate_random_vector(512);
        let doc = VectorDocument::new(DocumentId::new_v7(), vec);
        tokio_block_on(index.insert(doc)).unwrap();
    }

    let query = generate_random_vector(512);

    c.bench_function("hnsw_search_100k", |b| {
        b.iter(|| {
            tokio_block_on(index.search(&query, 10, None)).unwrap()
        });
    });
}
```

**Target Results:**
- BruteForce @ 10k: ~5ms (baseline)
- HNSW @ 100k: <25ms P95 (10x improvement over brute force at 10x scale)

---

## 7. Acceptance Criteria

### 7.1 Functional Requirements

- ✅ VectorDocument domain model with metadata support
- ✅ VectorIndex trait with insert/search/delete operations
- ✅ BruteForceIndex implementation (100% recall baseline)
- ✅ HnswIndex implementation with configurable parameters
- ✅ Three distance metrics: Cosine, Euclidean, Dot Product
- ✅ Batch insert support for bulk loading
- ✅ Soft delete with tombstone marking

### 7.2 Performance Requirements

- ✅ BruteForce: 10k vectors in <10ms (512-dim, ARM M3)
- ✅ HNSW: 100k vectors in <25ms P95 (512-dim, M=32, ef=128)
- ✅ HNSW recall: ≥0.95 @ k=10 vs brute-force baseline
- ✅ Memory: <100GB for 30M vectors (512-dim)

### 7.3 Quality Requirements

- ✅ Zero compiler warnings
- ✅ All tests passing (12 unit + 6 integration)
- ✅ Benchmark results documented
- ✅ CLAUDE.md updated with Phase 4 status
- ✅ Phase 4 completion report created

---

## 8. Implementation Plan

### Week 1: Baseline Implementation

**Day 1-2:**
- Add VectorDocument and related types to akidb-core
- Implement distance metric functions (cosine, euclidean, dot product)
- Write 3 distance metric unit tests

**Day 3-4:**
- Implement BruteForceIndex with VectorIndex trait
- Write 9 BruteForceIndex unit tests
- Validate correctness with integration tests

**Day 5:**
- Create akidb-index crate structure
- Move BruteForceIndex to akidb-index crate
- Setup benchmarks with Criterion

**Deliverable:** Working brute-force index with 100% recall

### Week 2: HNSW Implementation

**Day 1-2:**
- Implement HNSW graph structure (layers, nodes, adjacency)
- Implement layer assignment (`floor(-ln(rand()) * ml)`)
- Implement insert algorithm (greedy search + neighbor selection)

**Day 3-4:**
- Implement search algorithm (hierarchical traversal)
- Implement delete with soft tombstone marking
- Add HNSW configuration validation

**Day 5:**
- Write 6 HNSW integration tests
- Compare recall against brute-force baseline
- Optimize with profiling (flamegraphs)

**Deliverable:** Working HNSW index with ≥0.95 recall

### Week 3: Optimization & Documentation

**Day 1-2:**
- Add ARM NEON SIMD optimizations for distance functions
- Benchmark scalar vs SIMD performance
- Validate P95 < 25ms target

**Day 3:**
- Run full benchmark suite (10k, 100k, 1M vectors)
- Generate performance report with graphs
- Validate memory usage

**Day 4:**
- Update CLAUDE.md with Phase 4 architecture
- Write Phase 4 completion report
- Document HNSW tuning guide

**Day 5:**
- Code review and cleanup
- Fix any remaining issues
- Tag Phase 4 release

**Deliverable:** Production-ready vector engine with benchmarks

---

## 9. Risk Analysis

### 9.1 Technical Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| HNSW complexity causes delays | Medium | High | Start with brute-force baseline first |
| SIMD optimization breaks correctness | Low | Medium | Keep scalar fallback, validate with tests |
| Memory usage exceeds 100GB | Low | High | Profile early, optimize node storage |
| Recall <0.95 on production data | Low | High | Tune ef_construction, validate with real embeddings |

### 9.2 Schedule Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| HNSW takes >2 weeks | Medium | Medium | Week 1 delivers working baseline (brute-force) |
| Benchmarking uncovers perf issues | Low | Medium | Profile early, optimize hot paths |

---

## 10. Future Work (Phase 5+)

### 10.1 WAL Integration

- Persist vector inserts/deletes to SQLite WAL
- Replay WAL on startup for crash recovery
- Enable hot-standby replicas

### 10.2 Memory-Mapped Storage

- Store vectors in mmap files for cold-start speedup
- Incremental snapshots (every 512MB WAL)
- zstd compression for S3 uploads

### 10.3 Advanced Indexing

- IVF (Inverted File) index for >100M vectors
- Product Quantization for memory reduction
- GPU-accelerated search on Jetson (TensorRT)

### 10.4 Query Filtering

- Metadata filtering before/after vector search
- Hybrid search (vector + keyword)
- Range queries (find all vectors within distance threshold)

---

## 11. References

**HNSW Paper:**
- Malkov, Y., & Yashunin, D. (2018). Efficient and robust approximate nearest neighbor search using Hierarchical Navigable Small World graphs. *IEEE Transactions on Pattern Analysis and Machine Intelligence*.
- https://arxiv.org/abs/1603.09320

**SIMD Resources:**
- ARM NEON Programmer's Guide: https://developer.arm.com/architectures/instruction-sets/simd-isas/neon
- Rust `std::arch` docs: https://doc.rust-lang.org/std/arch/

**Benchmark Harnesses:**
- Criterion.rs: https://github.com/bheisler/criterion.rs
- ann-benchmarks: https://github.com/erikbern/ann-benchmarks

---

**Phase 4 Design Status:** ✅ Complete
**Ready for Implementation:** Yes
**Estimated Effort:** 3 weeks (1 engineer)
**Next Step:** Implement VectorDocument domain model in akidb-core
