# AkiDB Index Providers Guide

**Version**: 0.2.0 (Phase 3 M2)
**Last Updated**: 2025-10-24

---

## Overview

AkiDB supports multiple **Index Provider** implementations for approximate nearest neighbor (ANN) search. Index providers abstract away the underlying search algorithm, allowing you to choose the best fit for your workload.

All providers implement the `IndexProvider` trait (`crates/akidb-index/src/provider.rs:10`), ensuring consistent behavior across different backends.

---

## Available Providers

### NativeIndexProvider (Brute Force)

**Implementation**: `crates/akidb-index/src/native.rs`

**Algorithm**: Brute-force linear search across all vectors.

**Use Cases**:
- **Small datasets** (< 10K vectors)
- **Testing and development** (reference implementation)
- **Baseline benchmarking** (ground truth for recall)
- **100% recall required** (exact nearest neighbors)

**Performance Characteristics**:
- **Search Time**: O(n) - linear scan
- **Memory Usage**: ~4 bytes per dimension per vector
- **Build Time**: O(1) - instant (no pre-processing)
- **Deletion Support**: ✅ **Yes** - efficient removal via key-to-index map

**Serialization**: JSON format (human-readable, debuggable)

**Example Performance** (10K vectors, 128-dim, k=10):
- P50 latency: 0.53-0.69ms
- Throughput: 1,450-1,890 QPS
- Memory: ~5 MB

---

### HnswIndexProvider (HNSW Graph)

**Implementation**: `crates/akidb-index/src/hnsw.rs`

**Algorithm**: Hierarchical Navigable Small World (HNSW) graph-based search.

**Current Status**: **Brute-force fallback** (Phase 3 M2+)
- Full HNSW graph implementation pending
- Currently uses same linear search as Native for correctness

**Use Cases** (when fully implemented):
- **Large datasets** (> 100K vectors)
- **Low-latency requirements** (sub-millisecond search)
- **High-throughput workloads** (10K+ QPS)
- **Approximate search acceptable** (95%+ recall)

**Performance Characteristics** (target):
- **Search Time**: O(log n) - logarithmic with graph navigation
- **Memory Usage**: ~(M × 4) bytes per vector (M = connections per layer)
- **Build Time**: O(n log n) - graph construction
- **Deletion Support**: ❌ **No** - requires index rebuild (see [HNSW Limitations](#hnsw-limitations))

**Serialization**: JSON format (matches Native for consistency)

**Example Performance** (target for 1M vectors, 128-dim, k=50):
- P95 latency: ≤150ms
- P99 latency: ≤250ms
- Throughput: +20% vs Phase 2 baseline
- Memory: ~500 MB

---

## Usage

### Basic Example

```rust
use akidb_index::{
    IndexProvider, NativeIndexProvider, HnswIndexProvider,
    BuildRequest, IndexBatch, IndexKind, QueryVector, SearchOptions
};
use akidb_core::{DistanceMetric, SegmentDescriptor};

#[tokio::main]
async fn main() -> Result<()> {
    // Choose provider
    let provider: Box<dyn IndexProvider> = Box::new(NativeIndexProvider::new());
    // or: Box::new(HnswIndexProvider::new(Default::default()))

    // Build index
    let handle = provider.build(BuildRequest {
        collection: "my_vectors".to_string(),
        kind: provider.kind(),
        distance: DistanceMetric::Cosine,
        segments: vec![/* segment descriptors */],
    }).await?;

    // Add vectors
    let batch = IndexBatch {
        primary_keys: vec!["vec1".to_string(), "vec2".to_string()],
        vectors: vec![
            QueryVector { components: vec![1.0, 0.0, 0.0] },
            QueryVector { components: vec![0.0, 1.0, 0.0] },
        ],
        payloads: vec![json!({"id": 1}), json!({"id": 2})],
    };
    provider.add_batch(&handle, batch).await?;

    // Search
    let query = QueryVector { components: vec![1.0, 0.1, 0.0] };
    let options = SearchOptions {
        top_k: 10,
        filter: None,
        timeout_ms: 1000,
    };
    let results = provider.search(&handle, query, options).await?;

    println!("Found {} neighbors", results.neighbors.len());
    Ok(())
}
```

### Serialization and Persistence

```rust
// Serialize index to bytes (for S3 storage)
let serialized = provider.serialize(&handle)?;

// Save to storage backend
storage.put_object(&index_key, serialized.into()).await?;

// Later: deserialize from storage
let bytes = storage.get_object(&index_key).await?;
let restored_handle = provider.deserialize(&bytes)?;

// Search works immediately
let results = provider.search(&restored_handle, query, options).await?;
```

### Extract Data for Persistence

```rust
// Extract vectors and payloads for S3 segment persistence
let (vectors, payloads) = provider.extract_for_persistence(&handle)?;

// Write to storage with SEGv1 format
storage.write_segment_with_data(&descriptor, vectors, Some(metadata)).await?;
```

---

## Choosing a Provider

### Decision Matrix

| Criteria | Native (Brute Force) | HNSW (Graph) |
|----------|---------------------|--------------|
| **Dataset Size** | < 10K vectors | > 100K vectors |
| **Recall Requirement** | 100% (exact) | 95%+ (approximate) |
| **Latency Target** | < 1ms (small datasets) | < 10ms (large datasets) |
| **Memory Budget** | Tight (minimal overhead) | Generous (graph storage) |
| **Deletion Frequency** | Frequent updates | Rare updates |
| **Build Time** | Instant | Minutes (large datasets) |
| **Use Case** | Testing, baselines | Production, high-scale |

### Recommendations

**Use Native when**:
- You have < 10K vectors
- You need exact nearest neighbors (100% recall)
- You're writing tests or benchmarking
- You need frequent deletion operations
- Memory is constrained

**Use HNSW when** (Phase 3 M2+):
- You have > 100K vectors
- Approximate search is acceptable (95%+ recall)
- You need sub-10ms latency at scale
- Deletions are rare (or you can rebuild periodically)
- You have memory for graph overhead

---

## HNSW Limitations

### Deletion Not Supported

HNSW indices do not support efficient deletion due to their graph-based structure. Removing nodes would require expensive graph reconstruction.

**Error**: Calling `remove()` on `HnswIndexProvider` returns `Error::NotImplemented`.

**Workaround**: Rebuild the index with filtered vectors:

```rust
// Filter out unwanted vectors
let filtered_vectors: Vec<Vec<f32>> = original_vectors
    .into_iter()
    .filter(|(key, _)| !keys_to_remove.contains(key))
    .map(|(_, vec)| vec)
    .collect();

// Rebuild index
let new_handle = provider.build(BuildRequest {
    collection: "my_collection".to_string(),
    kind: IndexKind::Hnsw,
    distance: DistanceMetric::Cosine,
    segments: filtered_segments,
}).await?;
```

See `crates/akidb-index/src/hnsw.rs:348` for detailed documentation.

---

## Contract Tests

All index providers must pass the same contract tests to ensure consistent behavior:

**Test Suite**: `crates/akidb-index/tests/contract_tests.rs`

**Covered Scenarios** (8/8 passing):
1. ✅ Reject dimension=0
2. ✅ Handle empty index search
3. ✅ Roundtrip serialization/deserialization
4. ✅ Extract data for persistence
5. ✅ Dimension validation (reject mismatches)
6. ✅ Reject duplicate primary keys
7. ✅ Batch array consistency validation
8. ✅ Search result ordering (by distance metric)

**Run Tests**:
```bash
cargo test -p akidb-index --test contract_tests
```

---

## Performance Validation

**Benchmark Suite**: `crates/akidb-benchmarks/benches/vector_search.rs`

**Baseline Metrics** (Phase 2, 10K vectors, 128-dim):
- **Cosine k=10**: P50=0.69ms, P95=0.82ms, 1,450 QPS
- **L2 k=10**: P50=0.53ms, P95=0.57ms, 1,890 QPS (23% faster)

**Run Benchmarks**:
```bash
cargo bench --package akidb-benchmarks --bench vector_search
```

See `docs/performance-guide.md` for detailed benchmarking instructions.

---

## Migration from Direct Index Usage

If you're currently using index providers directly, no migration is needed. The API is stable.

If you're implementing a custom index provider:
1. Implement `IndexProvider` trait
2. Ensure all 8 contract tests pass
3. Add performance benchmarks
4. Document any limitations (like HNSW deletion)

---

## Support

- **API Documentation**: `cargo doc --package akidb-index --open`
- **Source Code**: `crates/akidb-index/src/`
- **Contract Tests**: `crates/akidb-index/tests/contract_tests.rs`
- **Examples**: `crates/akidb-index/src/native.rs:489` (test examples)
- **Issue Tracker**: [GitHub Issues](https://github.com/defai-digital/akidb/issues)

---

**Related Documentation**:
- [Manifest V1 Migration](migrations/manifest_v1.md) - Atomic manifest operations
- [Performance Guide](performance-guide.md) - Benchmarking and tuning
- [CLAUDE.md](../CLAUDE.md) - Development guide

**Last Updated**: 2025-10-24 (Phase 3 M2)
