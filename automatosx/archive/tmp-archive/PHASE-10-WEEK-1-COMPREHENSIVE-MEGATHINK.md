# Phase 10 Week 1: Parquet Snapshotter - Comprehensive Megathink

**Date**: 2025-11-09
**Status**: 🎯 READY TO EXECUTE
**Timeline**: 5 days (Week 1 of Phase 10)
**Goal**: Implement production-ready Parquet-based vector snapshot system

---

## Executive Summary

Week 1 builds the **Parquet Snapshotter** - a high-performance columnar storage layer for vector snapshots. This replaces the existing JSON snapshotter with a **2-3x compression** improvement and **90% reduction in S3 API calls**.

**Why Parquet**:
- **Columnar storage**: Optimized for vector data (all floats together)
- **Compression**: 2-3x better than JSON (Snappy: fast, Zstd: max compression)
- **Batch-friendly**: 100+ documents per S3 upload vs. 1 per upload (JSON)
- **Analytics-ready**: Compatible with Athena, BigQuery, Spark for future analysis

**Current State**:
- ✅ WAL infrastructure exists (Phase 6 Week 1)
- ✅ S3/ObjectStore exists (Phase 6 Week 2)
- ✅ ParquetEncoder exists (`parquet_encoder.rs`, 300+ lines)
- ✅ JSON Snapshotter exists (`snapshotter/mod.rs`, baseline implementation)
- ⏸️ **Missing**: ParquetSnapshotter integration

**Week 1 Objective**: Create `ParquetSnapshotter` that integrates ParquetEncoder with ObjectStore for production-ready S3 snapshots.

---

## Technical Architecture Analysis

### Existing Components (Foundation)

**1. ParquetEncoder** (`parquet_encoder.rs`)
```rust
pub struct ParquetEncoder {
    config: ParquetConfig,  // Compression, row group size
}

impl ParquetEncoder {
    // Encode batch of vectors to Parquet bytes
    pub fn encode_batch(&self, documents: &[VectorDocument], dimension: u32)
        -> CoreResult<Bytes>

    // Decode Parquet bytes back to vectors
    pub fn decode_batch(&self, parquet_bytes: &[u8])
        -> CoreResult<Vec<VectorDocument>>
}
```

**Schema**:
```
document_id: Binary (UUID as bytes)
external_id: Utf8 (optional)
dimension: UInt32
vector: FixedSizeList<Float32> (dimension floats)
metadata_json: Utf8 (JSON)
inserted_at: Timestamp (milliseconds)
```

**2. ObjectStore Trait** (`object_store/mod.rs`)
```rust
#[async_trait]
pub trait ObjectStore: Send + Sync {
    async fn put(&self, key: &str, value: Bytes) -> CoreResult<()>;
    async fn get(&self, key: &str) -> CoreResult<Bytes>;
    async fn delete(&self, key: &str) -> CoreResult<()>;
    async fn list(&self, prefix: &str) -> CoreResult<Vec<String>>;
    // ... 4 more methods
}
```

**Implementations**:
- `S3ObjectStore` - AWS S3 / MinIO compatible
- `LocalObjectStore` - Local filesystem for testing

**3. JSON Snapshotter** (`snapshotter/mod.rs`)
```rust
pub struct JsonSnapshotter {
    store: Arc<dyn ObjectStore>,
    compression: CompressionCodec,  // None, Gzip, Snappy
}

impl Snapshotter for JsonSnapshotter {
    async fn create_snapshot(&self, collection_id, vectors) -> SnapshotId;
    async fn restore_snapshot(&self, snapshot_id) -> Vec<VectorDocument>;
    async fn list_snapshots(&self, collection_id) -> Vec<SnapshotMetadata>;
    async fn delete_snapshot(&self, snapshot_id) -> CoreResult<()>;
}
```

**Key Format**:
- JSON: `snapshots/{collection_id}/{snapshot_id}.json[.gz]`
- Metadata: `snapshots/{collection_id}/{snapshot_id}.metadata.json`

### New Component: ParquetSnapshotter

**Design Goals**:
1. **API-compatible** with JsonSnapshotter (same Snapshotter trait)
2. **Batch-optimized** for large datasets (100k+ vectors)
3. **S3-efficient** - reduce API calls by 90%+
4. **Compression** - 2-3x smaller files than JSON
5. **Production-ready** - error handling, metrics, tests

**Architecture**:

```
┌────────────────────────────────────────────────────────────┐
│                   ParquetSnapshotter                       │
│                                                            │
│  ┌──────────────┐    ┌────────────────┐    ┌───────────┐ │
│  │ VectorDoc[]  │ -> │ ParquetEncoder │ -> │ ObjectStore│ │
│  └──────────────┘    └────────────────┘    └───────────┘ │
│         │                    │                     │       │
│         │                    │                     │       │
│    create()              encode()               put()     │
│                                                            │
│  ┌──────────────┐    ┌────────────────┐    ┌───────────┐ │
│  │ VectorDoc[]  │ <- │ ParquetEncoder │ <- │ ObjectStore│ │
│  └──────────────┘    └────────────────┘    └───────────┘ │
│                                                            │
│    restore()             decode()               get()     │
└────────────────────────────────────────────────────────────┘
```

**Key Difference from JSON**:
- **JSON**: One file per snapshot, entire collection serialized
- **Parquet**: **Batched** - can split large collections into multiple Parquet files

**Proposed Approach** (for Week 1):
- Start simple: **Single Parquet file per snapshot** (like JSON)
- **Future enhancement** (Week 2/3): Multi-file snapshots for >100k vectors

**File Structure**:
```
s3://bucket/snapshots/{collection_id}/
  ├── {snapshot_id}.parquet           # Vector data (columnar)
  └── {snapshot_id}.metadata.json     # SnapshotMetadata
```

**Metadata**:
```rust
pub struct SnapshotMetadata {
    snapshot_id: SnapshotId,
    collection_id: CollectionId,
    vector_count: u64,
    dimension: u32,
    created_at: DateTime<Utc>,
    size_bytes: u64,           // Compressed Parquet file size
    compression: Compression,   // Snappy, Zstd, None
    format: SnapshotFormat,     // NEW: Parquet vs JSON
}

pub enum SnapshotFormat {
    Json,
    Parquet,
}
```

---

## Implementation Plan

### Day 1: Core ParquetSnapshotter Structure

**Objective**: Scaffold `ParquetSnapshotter` struct and `Snapshotter` trait implementation

**Files to Create**:
- `crates/akidb-storage/src/snapshotter/parquet.rs` (~200 lines)

**Tasks**:
1. **Define struct**:
   ```rust
   pub struct ParquetSnapshotter {
       store: Arc<dyn ObjectStore>,
       encoder: ParquetEncoder,
       config: ParquetSnapshotConfig,
   }

   pub struct ParquetSnapshotConfig {
       compression: Compression,  // Snappy recommended
       row_group_size: usize,     // 10,000 default
       enable_dictionary: bool,   // true for metadata
   }
   ```

2. **Implement `new()` constructor**:
   ```rust
   impl ParquetSnapshotter {
       pub fn new(store: Arc<dyn ObjectStore>, config: ParquetSnapshotConfig) -> Self {
           let encoder = ParquetEncoder::new(ParquetConfig {
               compression: config.compression,
               row_group_size: config.row_group_size,
               enable_dictionary: config.enable_dictionary,
           });

           Self { store, encoder, config }
       }
   }
   ```

3. **Scaffold trait methods** (empty stubs):
   ```rust
   #[async_trait]
   impl Snapshotter for ParquetSnapshotter {
       async fn create_snapshot(
           &self,
           collection_id: CollectionId,
           vectors: Vec<VectorDocument>,
       ) -> CoreResult<SnapshotId> {
           todo!("Day 2: Implement create_snapshot")
       }

       async fn restore_snapshot(
           &self,
           snapshot_id: SnapshotId,
       ) -> CoreResult<Vec<VectorDocument>> {
           todo!("Day 3: Implement restore_snapshot")
       }

       async fn list_snapshots(
           &self,
           collection_id: CollectionId,
       ) -> CoreResult<Vec<SnapshotMetadata>> {
           todo!("Day 4: Implement list_snapshots")
       }

       async fn delete_snapshot(&self, snapshot_id: SnapshotId) -> CoreResult<()> {
           todo!("Day 4: Implement delete_snapshot")
       }
   }
   ```

4. **Update `snapshotter/mod.rs`**:
   - Export `ParquetSnapshotter`
   - Add `SnapshotFormat` enum to `SnapshotMetadata`

5. **First unit test**:
   ```rust
   #[tokio::test]
   async fn test_parquet_snapshotter_creation() {
       let store = Arc::new(LocalObjectStore::new("./test-snapshots").await.unwrap());
       let config = ParquetSnapshotConfig::default();
       let snapshotter = ParquetSnapshotter::new(store, config);
       // Just verify construction works
   }
   ```

**Deliverable**: Compiling `ParquetSnapshotter` scaffold

**Time Estimate**: 2-3 hours

---

### Day 2: Implement `create_snapshot()`

**Objective**: Create Parquet snapshots from vector collections

**Tasks**:

1. **Implement snapshot creation**:
   ```rust
   async fn create_snapshot(
       &self,
       collection_id: CollectionId,
       vectors: Vec<VectorDocument>,
   ) -> CoreResult<SnapshotId> {
       // Step 1: Validate input
       if vectors.is_empty() {
           return Err(CoreError::ValidationError("Empty snapshot".into()));
       }

       let dimension = vectors[0].vector.len() as u32;

       // Verify all vectors have same dimension
       for doc in &vectors {
           if doc.vector.len() as u32 != dimension {
               return Err(CoreError::ValidationError(
                   format!("Dimension mismatch: expected {}, got {}",
                           dimension, doc.vector.len())
               ));
           }
       }

       // Step 2: Generate snapshot ID
       let snapshot_id = SnapshotId::new();

       // Step 3: Encode to Parquet
       let parquet_bytes = self.encoder.encode_batch(&vectors, dimension)?;

       // Step 4: Upload to S3
       let parquet_key = format!("snapshots/{}/{}.parquet",
                                 collection_id, snapshot_id);
       self.store.put(&parquet_key, parquet_bytes.clone()).await?;

       // Step 5: Create and upload metadata
       let metadata = SnapshotMetadata {
           snapshot_id,
           collection_id,
           vector_count: vectors.len() as u64,
           dimension,
           created_at: Utc::now(),
           size_bytes: parquet_bytes.len() as u64,
           compression: self.config.compression,
           format: SnapshotFormat::Parquet,
       };

       let metadata_json = serde_json::to_vec(&metadata)?;
       let metadata_key = format!("snapshots/{}/{}.metadata.json",
                                  collection_id, snapshot_id);
       self.store.put(&metadata_key, Bytes::from(metadata_json)).await?;

       Ok(snapshot_id)
   }
   ```

2. **Error Handling**:
   - Empty vector list
   - Dimension mismatch
   - Encoding failures
   - S3 upload failures

3. **Testing** (3 tests):
   ```rust
   #[tokio::test]
   async fn test_create_snapshot_10k_vectors() {
       // Generate 10k test vectors (512-dim)
       let vectors = generate_test_vectors(10_000, 512);
       let snapshot_id = snapshotter.create_snapshot(collection_id, vectors).await.unwrap();
       assert!(snapshot_id.as_uuid().version() == Version::Random);
   }

   #[tokio::test]
   async fn test_create_snapshot_empty_vectors() {
       let result = snapshotter.create_snapshot(collection_id, vec![]).await;
       assert!(matches!(result, Err(CoreError::ValidationError(_))));
   }

   #[tokio::test]
   async fn test_create_snapshot_dimension_mismatch() {
       let mut vectors = generate_test_vectors(100, 512);
       vectors.push(generate_test_vector(256));  // Wrong dimension
       let result = snapshotter.create_snapshot(collection_id, vectors).await;
       assert!(matches!(result, Err(CoreError::ValidationError(_))));
   }
   ```

**Deliverable**: Working `create_snapshot()` with 3 tests

**Time Estimate**: 3-4 hours

---

### Day 3: Implement `restore_snapshot()`

**Objective**: Restore vectors from Parquet snapshots

**Tasks**:

1. **Implement restoration**:
   ```rust
   async fn restore_snapshot(
       &self,
       snapshot_id: SnapshotId,
   ) -> CoreResult<Vec<VectorDocument>> {
       // Step 1: Load metadata to get collection_id
       // (Alternative: accept collection_id as parameter)
       let metadata = self.load_metadata(snapshot_id).await?;

       // Step 2: Download Parquet file from S3
       let parquet_key = format!("snapshots/{}/{}.parquet",
                                 metadata.collection_id, snapshot_id);
       let parquet_bytes = self.store.get(&parquet_key).await?;

       // Step 3: Decode Parquet to vectors
       let vectors = self.encoder.decode_batch(&parquet_bytes)?;

       // Step 4: Verify integrity
       if vectors.len() != metadata.vector_count as usize {
           return Err(CoreError::DataCorruption(
               format!("Expected {} vectors, got {}",
                       metadata.vector_count, vectors.len())
           ));
       }

       Ok(vectors)
   }

   async fn load_metadata(&self, snapshot_id: SnapshotId)
       -> CoreResult<SnapshotMetadata>
   {
       // Try to find metadata file by scanning all collections
       // (This is why we might want to accept collection_id)
       // For now, assume we know the collection_id somehow

       // Better approach: Change trait signature to:
       // restore_snapshot(collection_id, snapshot_id)
       todo!("Metadata lookup strategy")
   }
   ```

2. **Trait Signature Update** (IMPORTANT):
   - Current `Snapshotter` trait doesn't pass `collection_id` to `restore_snapshot()`
   - **Decision**: Update trait to accept `collection_id`:
     ```rust
     async fn restore_snapshot(
         &self,
         collection_id: CollectionId,
         snapshot_id: SnapshotId,
     ) -> CoreResult<Vec<VectorDocument>>;
     ```

3. **Testing** (4 tests):
   ```rust
   #[tokio::test]
   async fn test_roundtrip_10k_vectors() {
       let original = generate_test_vectors(10_000, 512);
       let snapshot_id = snapshotter.create_snapshot(coll_id, original.clone()).await.unwrap();
       let restored = snapshotter.restore_snapshot(coll_id, snapshot_id).await.unwrap();

       assert_eq!(original.len(), restored.len());
       for (orig, rest) in original.iter().zip(restored.iter()) {
           assert_eq!(orig.id, rest.id);
           assert_vectors_equal(&orig.vector, &rest.vector, 1e-6);
       }
   }

   #[tokio::test]
   async fn test_restore_nonexistent_snapshot() {
       let result = snapshotter.restore_snapshot(coll_id, SnapshotId::new()).await;
       assert!(matches!(result, Err(CoreError::NotFound(_))));
   }

   #[tokio::test]
   async fn test_restore_corrupted_parquet() {
       // Upload invalid Parquet data
       let key = format!("snapshots/{}/corrupted.parquet", coll_id);
       snapshotter.store.put(&key, Bytes::from("not parquet")).await.unwrap();

       let result = snapshotter.restore_snapshot(coll_id, snapshot_id).await;
       assert!(result.is_err());
   }

   #[tokio::test]
   async fn test_restore_large_snapshot_100k() {
       let vectors = generate_test_vectors(100_000, 512);
       let snapshot_id = snapshotter.create_snapshot(coll_id, vectors.clone()).await.unwrap();
       let restored = snapshotter.restore_snapshot(coll_id, snapshot_id).await.unwrap();
       assert_eq!(vectors.len(), restored.len());
   }
   ```

**Deliverable**: Working `restore_snapshot()` with 4 tests

**Time Estimate**: 3-4 hours

---

### Day 4: Implement `list_snapshots()` and `delete_snapshot()`

**Objective**: Complete snapshot management operations

**Tasks**:

1. **Implement listing**:
   ```rust
   async fn list_snapshots(
       &self,
       collection_id: CollectionId,
   ) -> CoreResult<Vec<SnapshotMetadata>> {
       // List all metadata files for this collection
       let prefix = format!("snapshots/{}/", collection_id);
       let keys = self.store.list(&prefix).await?;

       let mut metadata_list = Vec::new();

       for key in keys {
           if key.ends_with(".metadata.json") {
               let bytes = self.store.get(&key).await?;
               let metadata: SnapshotMetadata = serde_json::from_slice(&bytes)?;
               metadata_list.push(metadata);
           }
       }

       // Sort by creation time (newest first)
       metadata_list.sort_by(|a, b| b.created_at.cmp(&a.created_at));

       Ok(metadata_list)
   }
   ```

2. **Implement deletion**:
   ```rust
   async fn delete_snapshot(&self, snapshot_id: SnapshotId) -> CoreResult<()> {
       // Need collection_id to construct keys
       // Option 1: Accept collection_id as parameter
       // Option 2: Store mapping in metadata index
       // For now: Update trait signature

       let parquet_key = format!("snapshots/{}/{}.parquet", collection_id, snapshot_id);
       let metadata_key = format!("snapshots/{}/{}.metadata.json", collection_id, snapshot_id);

       // Delete both files (best-effort)
       self.store.delete(&parquet_key).await?;
       self.store.delete(&metadata_key).await?;

       Ok(())
   }
   ```

3. **Retention Policy Helper**:
   ```rust
   /// Delete snapshots older than retention period
   pub async fn cleanup_old_snapshots(
       &self,
       collection_id: CollectionId,
       retention: chrono::Duration,
   ) -> CoreResult<usize> {
       let snapshots = self.list_snapshots(collection_id).await?;
       let cutoff = Utc::now() - retention;

       let mut deleted = 0;
       for snapshot in snapshots {
           if snapshot.created_at < cutoff {
               self.delete_snapshot(collection_id, snapshot.snapshot_id).await?;
               deleted += 1;
           }
       }

       Ok(deleted)
   }
   ```

4. **Testing** (4 tests):
   ```rust
   #[tokio::test]
   async fn test_list_empty_snapshots() {
       let snapshots = snapshotter.list_snapshots(coll_id).await.unwrap();
       assert!(snapshots.is_empty());
   }

   #[tokio::test]
   async fn test_list_multiple_snapshots() {
       // Create 5 snapshots
       let mut ids = Vec::new();
       for i in 0..5 {
           let vectors = generate_test_vectors(100, 512);
           let id = snapshotter.create_snapshot(coll_id, vectors).await.unwrap();
           ids.push(id);
       }

       let snapshots = snapshotter.list_snapshots(coll_id).await.unwrap();
       assert_eq!(snapshots.len(), 5);

       // Verify sorted by creation time (newest first)
       for i in 1..snapshots.len() {
           assert!(snapshots[i-1].created_at >= snapshots[i].created_at);
       }
   }

   #[tokio::test]
   async fn test_delete_snapshot() {
       let vectors = generate_test_vectors(1000, 512);
       let snapshot_id = snapshotter.create_snapshot(coll_id, vectors).await.unwrap();

       snapshotter.delete_snapshot(coll_id, snapshot_id).await.unwrap();

       let result = snapshotter.restore_snapshot(coll_id, snapshot_id).await;
       assert!(matches!(result, Err(CoreError::NotFound(_))));
   }

   #[tokio::test]
   async fn test_cleanup_old_snapshots() {
       // Create snapshots with different ages
       // (Simulate by setting created_at in metadata)
       // ...
       let deleted = snapshotter.cleanup_old_snapshots(
           coll_id,
           chrono::Duration::days(7)
       ).await.unwrap();
       assert_eq!(deleted, 3);  // Expect 3 old ones deleted
   }
   ```

**Deliverable**: Complete snapshot management with 4 tests

**Time Estimate**: 3-4 hours

---

### Day 5: Integration, Benchmarking, Documentation

**Objective**: Integrate with StorageBackend, benchmark performance, complete documentation

**Tasks**:

1. **StorageBackend Integration**:
   - Update `storage_backend.rs` to accept ParquetSnapshotter
   - Add config option to choose JSON vs Parquet
   - Test end-to-end with S3

   ```rust
   pub enum SnapshotterType {
       Json,
       Parquet,
   }

   pub struct StorageBackendConfig {
       // ... existing fields
       pub snapshotter_type: SnapshotterType,
   }

   impl StorageBackend {
       pub async fn new(config: StorageBackendConfig) -> CoreResult<Self> {
           let object_store = ...;

           let snapshotter: Arc<dyn Snapshotter> = match config.snapshotter_type {
               SnapshotterType::Json => {
                   Arc::new(JsonSnapshotter::new(object_store.clone(), ...))
               }
               SnapshotterType::Parquet => {
                   Arc::new(ParquetSnapshotter::new(object_store.clone(), ...))
               }
           };

           // ...
       }
   }
   ```

2. **Performance Benchmarking**:
   ```rust
   #[tokio::test]
   async fn bench_create_snapshot_10k() {
       let vectors = generate_test_vectors(10_000, 512);
       let start = Instant::now();
       let snapshot_id = snapshotter.create_snapshot(coll_id, vectors).await.unwrap();
       let duration = start.elapsed();

       println!("✅ 10k vectors snapshot created in {:?}", duration);
       assert!(duration < Duration::from_secs(2), "Target: <2s, actual: {:?}", duration);
   }

   #[tokio::test]
   async fn bench_restore_snapshot_10k() {
       // Create snapshot first
       let vectors = generate_test_vectors(10_000, 512);
       let snapshot_id = snapshotter.create_snapshot(coll_id, vectors).await.unwrap();

       // Benchmark restore
       let start = Instant::now();
       let restored = snapshotter.restore_snapshot(coll_id, snapshot_id).await.unwrap();
       let duration = start.elapsed();

       println!("✅ 10k vectors restored in {:?}", duration);
       assert!(duration < Duration::from_secs(3), "Target: <3s, actual: {:?}", duration);
   }

   #[tokio::test]
   async fn bench_file_size_comparison() {
       let vectors = generate_test_vectors(10_000, 512);

       // Parquet size
       let parquet_id = parquet_snapshotter.create_snapshot(coll_id, vectors.clone()).await.unwrap();
       let parquet_metadata = parquet_snapshotter.list_snapshots(coll_id).await.unwrap()[0];
       let parquet_size = parquet_metadata.size_bytes;

       // JSON size (for comparison)
       let json_id = json_snapshotter.create_snapshot(coll_id, vectors).await.unwrap();
       let json_metadata = json_snapshotter.list_snapshots(coll_id).await.unwrap()[0];
       let json_size = json_metadata.size_bytes;

       println!("Parquet: {} bytes, JSON: {} bytes", parquet_size, json_size);
       println!("Compression ratio: {:.2}x", json_size as f64 / parquet_size as f64);

       assert!(parquet_size < json_size / 2, "Expect >2x compression");
   }
   ```

3. **Documentation**:
   - Update `snapshotter/mod.rs` module docs
   - Add usage examples to `ParquetSnapshotter` docs
   - Create migration guide (JSON → Parquet)

4. **Final Integration Test**:
   ```rust
   #[tokio::test]
   async fn test_storage_backend_with_parquet() {
       let config = StorageBackendConfig {
           snapshotter_type: SnapshotterType::Parquet,
           // ... other config
       };

       let backend = StorageBackend::new(config).await.unwrap();

       // Insert vectors
       let vectors = generate_test_vectors(10_000, 512);
       for vec in vectors {
           backend.insert(collection_id, vec).await.unwrap();
       }

       // Trigger snapshot (via flush or explicit call)
       backend.flush(collection_id).await.unwrap();

       // Verify snapshot was created
       let snapshots = backend.list_snapshots(collection_id).await.unwrap();
       assert_eq!(snapshots.len(), 1);
       assert_eq!(snapshots[0].format, SnapshotFormat::Parquet);
   }
   ```

**Deliverable**:
- Integrated ParquetSnapshotter in StorageBackend
- Benchmark results documented
- Migration guide written

**Time Estimate**: 4-5 hours

---

## Testing Strategy

### Test Pyramid

**Unit Tests** (5-7 tests):
- Constructor validation
- Empty snapshot error
- Dimension mismatch error
- Compression config
- Metadata serialization

**Integration Tests** (10-12 tests):
- Create snapshot (10k vectors)
- Restore snapshot (roundtrip)
- List snapshots (empty, multiple)
- Delete snapshot
- Large dataset (100k vectors)
- S3 upload/download
- LocalObjectStore fallback
- Concurrent snapshots
- Retention policy cleanup
- Error recovery (corrupted Parquet)

**Benchmark Tests** (3 tests):
- Create performance (<2s for 10k)
- Restore performance (<3s for 10k)
- File size comparison (>2x compression)

**Total**: ~20 tests for Week 1

---

## Risk Analysis

### Risk 1: Parquet Encoding Performance
**Likelihood**: Medium
**Impact**: Medium
**Mitigation**:
- Benchmark early (Day 2)
- Tune compression (Snappy vs Zstd)
- Consider chunking for >100k vectors

### Risk 2: S3 API Rate Limits
**Likelihood**: Low (Week 1 - single file uploads)
**Impact**: Low
**Mitigation**:
- Use LocalObjectStore for testing
- Batch uploads come in Week 4 (performance optimization)

### Risk 3: Memory Pressure (Large Snapshots)
**Likelihood**: Medium
**Impact**: Medium
**Mitigation**:
- Stream encoding (future enhancement)
- For now: Document limits (100k vectors recommended max)

### Risk 4: Trait Signature Changes
**Likelihood**: High (need to add `collection_id` to methods)
**Impact**: Medium (affects JsonSnapshotter too)
**Mitigation**:
- Update trait early (Day 1)
- Update both implementations together
- Add deprecation warnings if needed

---

## Success Criteria

### Functional
- ✅ ParquetSnapshotter implements Snapshotter trait
- ✅ Create/restore/list/delete all working
- ✅ Integration with StorageBackend complete
- ✅ 20+ tests passing (0 failures)

### Performance
- ✅ Create snapshot: <2s for 10k vectors (512-dim)
- ✅ Restore snapshot: <3s for 10k vectors
- ✅ File size: >2x compression vs JSON
- ✅ Memory footprint: <1GB for 100k vectors

### Quality
- ✅ Zero data corruption (roundtrip integrity)
- ✅ Clean error handling (no panics)
- ✅ Documentation complete
- ✅ Code review ready

---

## Dependencies

### External Crates (Already Added)
- `parquet = "53.2"`
- `arrow = "53.2"`
- `bytes = "1.5"`
- `serde_json = "1.0"`

### Internal Dependencies
- `akidb-core` (VectorDocument, CoreError)
- `object_store` module (S3ObjectStore, LocalObjectStore)
- `parquet_encoder` module (ParquetEncoder)

### Infrastructure
- MinIO (local S3 testing) - optional for Week 1
- LocalObjectStore sufficient for testing

---

## Open Questions

### Q1: Should `restore_snapshot()` accept `collection_id`?
**Current**: Takes only `snapshot_id`
**Proposed**: Add `collection_id` parameter
**Rationale**: Need to construct S3 key `snapshots/{collection_id}/{snapshot_id}.parquet`

**Decision**: YES - Update trait signature on Day 1

### Q2: Single file vs. multi-file snapshots?
**Current Plan**: Single Parquet file per snapshot (simple)
**Future**: Multi-file for >100k vectors (Week 2-3)

**Decision**: Start with single file, add multi-file in Week 2 if needed

### Q3: Compression codec default?
**Options**:
- Snappy: Fast, ~2x compression
- Zstd: Best compression (~3x), slower
- None: No compression, fastest

**Decision**: Snappy for Week 1 (balanced), make configurable

### Q4: Snapshot retention policy?
**Scope**: Day 4 - implement cleanup_old_snapshots() helper
**Integration**: Week 2 tiering policies will use this

**Decision**: Implement helper, integrate in Week 2

---

## Next Steps After Week 1

### Week 2: Hot/Warm/Cold Tiering
- Use ParquetSnapshotter for warm tier (SSD cache)
- Use ParquetSnapshotter for cold tier (S3)
- Automatic promotion/demotion

### Week 3: Integration Testing + RC2
- E2E tests with real S3
- Multi-collection scenarios
- Crash recovery tests
- RC2 release with Parquet support

---

## Code Organization

```
crates/akidb-storage/src/
├── snapshotter/
│   ├── mod.rs              # Trait definition, SnapshotMetadata
│   ├── json.rs             # JsonSnapshotter (existing)
│   └── parquet.rs          # ParquetSnapshotter (NEW - ~300 lines)
├── parquet_encoder.rs      # Existing encoder (used by ParquetSnapshotter)
├── object_store/
│   ├── mod.rs              # ObjectStore trait
│   ├── s3.rs               # S3ObjectStore
│   └── local.rs            # LocalObjectStore
└── storage_backend.rs      # Integration point (update ~50 lines)

crates/akidb-storage/tests/
├── parquet_snapshotter_tests.rs  # NEW - 20+ tests
└── storage_backend_tests.rs      # UPDATE - add Parquet config test
```

---

## Completion Checklist

**Day 1**:
- [  ] ParquetSnapshotter struct defined
- [  ] Trait scaffolded (todo!() stubs)
- [  ] First unit test passing
- [  ] Module exports updated

**Day 2**:
- [  ] `create_snapshot()` implemented
- [  ] Dimension validation working
- [  ] S3 upload integration working
- [  ] 3 tests passing (create, empty, mismatch)

**Day 3**:
- [  ] `restore_snapshot()` implemented
- [  ] Roundtrip test passing (10k vectors)
- [  ] Error handling for corrupted data
- [  ] 4 tests passing (roundtrip, nonexistent, corrupted, large)

**Day 4**:
- [  ] `list_snapshots()` implemented
- [  ] `delete_snapshot()` implemented
- [  ] Retention policy helper added
- [  ] 4 tests passing (list empty, list multiple, delete, cleanup)

**Day 5**:
- [  ] StorageBackend integration complete
- [  ] Benchmarks run and documented
- [  ] Migration guide written
- [  ] All 20+ tests passing
- [  ] Code review ready

---

## Estimated Effort

**Total**: ~18-22 hours over 5 days

**Breakdown**:
- Day 1: 2-3 hours (scaffold)
- Day 2: 3-4 hours (create)
- Day 3: 3-4 hours (restore)
- Day 4: 3-4 hours (list/delete)
- Day 5: 4-5 hours (integration)
- Buffer: 2-3 hours (debugging, refinement)

**Pace**: ~4 hours/day (focused work)

---

**Status**: 🎯 READY FOR EXECUTION

**Next Action**: Begin Day 1 implementation!
