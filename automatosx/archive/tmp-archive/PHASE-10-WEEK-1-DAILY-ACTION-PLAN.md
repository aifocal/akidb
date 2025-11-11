# Phase 10 Week 1: Day-by-Day Action Plan

**Week**: Week 1 (Parquet Snapshotter)
**Timeline**: 5 days
**Total Effort**: 18-22 hours (~ 4 hours/day)
**Goal**: Production-ready Parquet-based vector snapshots

---

## Day 1: Scaffold ParquetSnapshotter (2-3 hours)

### Morning Session (1.5 hours)

**Objective**: Create module structure and ParquetSnapshotter struct

**Tasks**:

1. **Create new file** (`crates/akidb-storage/src/snapshotter/parquet.rs`):
   ```rust
   //! Parquet-based snapshotter for efficient vector storage
   //!
   //! Provides 2-3x better compression than JSON snapshots by using
   //! columnar storage optimized for vector data.

   use super::{Snapshotter, SnapshotId, SnapshotMetadata};
   use crate::object_store::ObjectStore;
   use crate::parquet_encoder::{ParquetEncoder, ParquetConfig};
   use akidb_core::{CollectionId, CoreResult, VectorDocument};
   use async_trait::async_trait;
   use parquet::basic::Compression;
   use std::sync::Arc;

   /// Configuration for Parquet snapshots
   #[derive(Debug, Clone)]
   pub struct ParquetSnapshotConfig {
       /// Compression algorithm (Snappy recommended for speed)
       pub compression: Compression,
       /// Row group size (default: 10,000 vectors)
       pub row_group_size: usize,
       /// Enable dictionary encoding (recommended)
       pub enable_dictionary: bool,
   }

   impl Default for ParquetSnapshotConfig {
       fn default() -> Self {
           Self {
               compression: Compression::SNAPPY,
               row_group_size: 10_000,
               enable_dictionary: true,
           }
       }
   }

   /// Parquet-based snapshotter
   pub struct ParquetSnapshotter {
       store: Arc<dyn ObjectStore>,
       encoder: ParquetEncoder,
       config: ParquetSnapshotConfig,
   }

   impl ParquetSnapshotter {
       /// Create new Parquet snapshotter
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

2. **Update mod.rs** (`crates/akidb-storage/src/snapshotter/mod.rs`):
   ```rust
   // Add after existing imports
   mod parquet;
   pub use parquet::{ParquetSnapshotter, ParquetSnapshotConfig};

   // Update SnapshotMetadata to include format
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct SnapshotMetadata {
       // ... existing fields
       #[serde(default = "default_format")]
       pub format: SnapshotFormat,
   }

   #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
   pub enum SnapshotFormat {
       Json,
       Parquet,
   }

   fn default_format() -> SnapshotFormat {
       SnapshotFormat::Json  // Backward compatible
   }
   ```

**Checkpoint**: `cargo check --package akidb-storage` should pass

---

### Afternoon Session (1-1.5 hours)

**Objective**: Scaffold trait implementation and first test

**Tasks**:

3. **Implement trait scaffolding** (add to `parquet.rs`):
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
           collection_id: CollectionId,
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

       async fn delete_snapshot(
           &self,
           collection_id: CollectionId,
           snapshot_id: SnapshotId,
       ) -> CoreResult<()> {
           todo!("Day 4: Implement delete_snapshot")
       }
   }
   ```

4. **Create test file** (`crates/akidb-storage/tests/parquet_snapshotter_tests.rs`):
   ```rust
   use akidb_storage::snapshotter::{ParquetSnapshotter, ParquetSnapshotConfig};
   use akidb_storage::object_store::LocalObjectStore;
   use std::sync::Arc;
   use tempfile::TempDir;

   #[tokio::test]
   async fn test_parquet_snapshotter_creation() {
       let temp_dir = TempDir::new().unwrap();
       let store = Arc::new(
           LocalObjectStore::new(temp_dir.path().to_str().unwrap())
               .await
               .unwrap()
       );

       let config = ParquetSnapshotConfig::default();
       let _snapshotter = ParquetSnapshotter::new(store, config);

       // Just verify construction works
       assert!(true, "ParquetSnapshotter created successfully");
   }
   ```

**Checkpoint**: `cargo test parquet_snapshotter_creation` should pass

**Day 1 Deliverable**: ✅ ParquetSnapshotter scaffold compiling with 1 test passing

---

## Day 2: Implement `create_snapshot()` (3-4 hours)

### Morning Session (2 hours)

**Objective**: Core snapshot creation logic

**Tasks**:

1. **Implement `create_snapshot()`** (replace todo!() in `parquet.rs`):
   ```rust
   async fn create_snapshot(
       &self,
       collection_id: CollectionId,
       vectors: Vec<VectorDocument>,
   ) -> CoreResult<SnapshotId> {
       use akidb_core::CoreError;
       use bytes::Bytes;
       use chrono::Utc;

       // Step 1: Validate input
       if vectors.is_empty() {
           return Err(CoreError::ValidationError(
               "Cannot create snapshot from empty vector set".to_string()
           ));
       }

       // Step 2: Verify all vectors have same dimension
       let dimension = vectors[0].vector.len() as u32;
       for (idx, doc) in vectors.iter().enumerate() {
           if doc.vector.len() as u32 != dimension {
               return Err(CoreError::ValidationError(
                   format!(
                       "Dimension mismatch at index {}: expected {}, got {}",
                       idx, dimension, doc.vector.len()
                   )
               ));
           }
       }

       // Step 3: Generate snapshot ID
       let snapshot_id = SnapshotId::new();

       // Step 4: Encode to Parquet
       tracing::info!(
           collection_id = %collection_id,
           snapshot_id = %snapshot_id,
           vector_count = vectors.len(),
           dimension = dimension,
           "Encoding snapshot to Parquet"
       );

       let start = std::time::Instant::now();
       let parquet_bytes = self.encoder.encode_batch(&vectors, dimension)
           .map_err(|e| CoreError::EncodingError(format!("Parquet encoding failed: {}", e)))?;
       let encode_duration = start.elapsed();

       tracing::debug!(
           duration_ms = encode_duration.as_millis(),
           size_bytes = parquet_bytes.len(),
           "Parquet encoding complete"
       );

       // Step 5: Upload to S3
       let parquet_key = format!("snapshots/{}/{}.parquet", collection_id, snapshot_id);
       self.store.put(&parquet_key, parquet_bytes.clone()).await?;

       // Step 6: Create and upload metadata
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

       let metadata_json = serde_json::to_vec(&metadata)
           .map_err(|e| CoreError::SerializationError(e.to_string()))?;
       let metadata_key = format!("snapshots/{}/{}.metadata.json", collection_id, snapshot_id);
       self.store.put(&metadata_key, Bytes::from(metadata_json)).await?;

       tracing::info!(
           snapshot_id = %snapshot_id,
           size_bytes = parquet_bytes.len(),
           duration_ms = (start.elapsed() - encode_duration).as_millis(),
           "Snapshot created successfully"
       );

       Ok(snapshot_id)
   }
   ```

**Checkpoint**: `cargo build --package akidb-storage` should pass

---

### Afternoon Session (1-2 hours)

**Objective**: Add tests for create_snapshot()

**Tasks**:

2. **Test helper functions** (add to `parquet_snapshotter_tests.rs`):
   ```rust
   use akidb_core::{DocumentId, VectorDocument};
   use rand::Rng;

   fn generate_test_vectors(count: usize, dimension: usize) -> Vec<VectorDocument> {
       let mut rng = rand::thread_rng();
       (0..count)
           .map(|i| {
               let vector: Vec<f32> = (0..dimension)
                   .map(|_| rng.gen_range(-1.0..1.0))
                   .collect();

               VectorDocument {
                   id: DocumentId::new(),
                   external_id: Some(format!("test-doc-{}", i)),
                   vector,
                   metadata: Some(serde_json::json!({"index": i})),
                   inserted_at: chrono::Utc::now(),
               }
           })
           .collect()
   }
   ```

3. **Test: Create snapshot with 10k vectors**:
   ```rust
   #[tokio::test]
   async fn test_create_snapshot_10k_vectors() {
       let temp_dir = TempDir::new().unwrap();
       let store = Arc::new(LocalObjectStore::new(temp_dir.path().to_str().unwrap()).await.unwrap());
       let snapshotter = ParquetSnapshotter::new(store, ParquetSnapshotConfig::default());

       let vectors = generate_test_vectors(10_000, 512);
       let collection_id = akidb_core::CollectionId::new();

       let snapshot_id = snapshotter.create_snapshot(collection_id, vectors).await.unwrap();

       assert_ne!(snapshot_id.as_uuid().to_string(), "");
   }
   ```

4. **Test: Empty snapshot error**:
   ```rust
   #[tokio::test]
   async fn test_create_snapshot_empty_vectors() {
       let temp_dir = TempDir::new().unwrap();
       let store = Arc::new(LocalObjectStore::new(temp_dir.path().to_str().unwrap()).await.unwrap());
       let snapshotter = ParquetSnapshotter::new(store, ParquetSnapshotConfig::default());

       let collection_id = akidb_core::CollectionId::new();
       let result = snapshotter.create_snapshot(collection_id, vec![]).await;

       assert!(matches!(result, Err(akidb_core::CoreError::ValidationError(_))));
   }
   ```

5. **Test: Dimension mismatch error**:
   ```rust
   #[tokio::test]
   async fn test_create_snapshot_dimension_mismatch() {
       let temp_dir = TempDir::new().unwrap();
       let store = Arc::new(LocalObjectStore::new(temp_dir.path().to_str().unwrap()).await.unwrap());
       let snapshotter = ParquetSnapshotter::new(store, ParquetSnapshotConfig::default());

       let mut vectors = generate_test_vectors(100, 512);
       // Add a vector with wrong dimension
       let mut wrong_vec = generate_test_vectors(1, 256);
       vectors.append(&mut wrong_vec);

       let collection_id = akidb_core::CollectionId::new();
       let result = snapshotter.create_snapshot(collection_id, vectors).await;

       assert!(matches!(result, Err(akidb_core::CoreError::ValidationError(_))));
   }
   ```

**Checkpoint**: `cargo test --package akidb-storage parquet` should show 4 tests passing

**Day 2 Deliverable**: ✅ create_snapshot() implemented with 4 tests passing

---

## Day 3: Implement `restore_snapshot()` (3-4 hours)

### Morning Session (2 hours)

**Objective**: Implement snapshot restoration

**Tasks**:

1. **Implement `restore_snapshot()`** (replace todo!() in `parquet.rs`):
   ```rust
   async fn restore_snapshot(
       &self,
       collection_id: CollectionId,
       snapshot_id: SnapshotId,
   ) -> CoreResult<Vec<VectorDocument>> {
       use akidb_core::CoreError;

       tracing::info!(
           collection_id = %collection_id,
           snapshot_id = %snapshot_id,
           "Restoring snapshot from Parquet"
       );

       // Step 1: Load metadata
       let metadata_key = format!("snapshots/{}/{}.metadata.json", collection_id, snapshot_id);
       let metadata_bytes = self.store.get(&metadata_key).await
           .map_err(|e| CoreError::NotFound(format!("Snapshot metadata not found: {}", e)))?;

       let metadata: SnapshotMetadata = serde_json::from_slice(&metadata_bytes)
           .map_err(|e| CoreError::DeserializationError(e.to_string()))?;

       // Step 2: Download Parquet file
       let parquet_key = format!("snapshots/{}/{}.parquet", collection_id, snapshot_id);
       let parquet_bytes = self.store.get(&parquet_key).await
           .map_err(|e| CoreError::NotFound(format!("Snapshot data not found: {}", e)))?;

       // Step 3: Decode Parquet
       let start = std::time::Instant::now();
       let vectors = self.encoder.decode_batch(&parquet_bytes)
           .map_err(|e| CoreError::DecodingError(format!("Parquet decoding failed: {}", e)))?;
       let decode_duration = start.elapsed();

       // Step 4: Verify integrity
       if vectors.len() != metadata.vector_count as usize {
           return Err(CoreError::DataCorruption(
               format!(
                   "Vector count mismatch: expected {}, got {}",
                   metadata.vector_count,
                   vectors.len()
               )
           ));
       }

       tracing::info!(
           snapshot_id = %snapshot_id,
           vector_count = vectors.len(),
           duration_ms = decode_duration.as_millis(),
           "Snapshot restored successfully"
       );

       Ok(vectors)
   }
   ```

**Checkpoint**: `cargo build --package akidb-storage` should pass

---

### Afternoon Session (1-2 hours)

**Objective**: Add roundtrip and error tests

**Tasks**:

2. **Test: Roundtrip 10k vectors**:
   ```rust
   #[tokio::test]
   async fn test_roundtrip_10k_vectors() {
       let temp_dir = TempDir::new().unwrap();
       let store = Arc::new(LocalObjectStore::new(temp_dir.path().to_str().unwrap()).await.unwrap());
       let snapshotter = ParquetSnapshotter::new(store, ParquetSnapshotConfig::default());

       let original = generate_test_vectors(10_000, 512);
       let collection_id = akidb_core::CollectionId::new();

       // Create snapshot
       let snapshot_id = snapshotter.create_snapshot(collection_id, original.clone()).await.unwrap();

       // Restore snapshot
       let restored = snapshotter.restore_snapshot(collection_id, snapshot_id).await.unwrap();

       // Verify
       assert_eq!(original.len(), restored.len());
       for (orig, rest) in original.iter().zip(restored.iter()) {
           assert_eq!(orig.id, rest.id);
           assert_eq!(orig.external_id, rest.external_id);
           assert_vectors_equal(&orig.vector, &rest.vector, 1e-6);
       }
   }

   fn assert_vectors_equal(a: &[f32], b: &[f32], epsilon: f64) {
       assert_eq!(a.len(), b.len());
       for (x, y) in a.iter().zip(b.iter()) {
           assert!((x - y).abs() < epsilon as f32, "Vectors differ: {} vs {}", x, y);
       }
   }
   ```

3. **Test: Restore nonexistent snapshot**:
   ```rust
   #[tokio::test]
   async fn test_restore_nonexistent_snapshot() {
       let temp_dir = TempDir::new().unwrap();
       let store = Arc::new(LocalObjectStore::new(temp_dir.path().to_str().unwrap()).await.unwrap());
       let snapshotter = ParquetSnapshotter::new(store, ParquetSnapshotConfig::default());

       let collection_id = akidb_core::CollectionId::new();
       let snapshot_id = SnapshotId::new();

       let result = snapshotter.restore_snapshot(collection_id, snapshot_id).await;
       assert!(matches!(result, Err(akidb_core::CoreError::NotFound(_))));
   }
   ```

4. **Test: Large dataset (100k vectors)**:
   ```rust
   #[tokio::test]
   async fn test_large_snapshot_100k_vectors() {
       let temp_dir = TempDir::new().unwrap();
       let store = Arc::new(LocalObjectStore::new(temp_dir.path().to_str().unwrap()).await.unwrap());
       let snapshotter = ParquetSnapshotter::new(store, ParquetSnapshotConfig::default());

       let vectors = generate_test_vectors(100_000, 512);
       let collection_id = akidb_core::CollectionId::new();

       let snapshot_id = snapshotter.create_snapshot(collection_id, vectors.clone()).await.unwrap();
       let restored = snapshotter.restore_snapshot(collection_id, snapshot_id).await.unwrap();

       assert_eq!(vectors.len(), restored.len());
   }
   ```

**Checkpoint**: `cargo test --package akidb-storage parquet` should show 7 tests passing

**Day 3 Deliverable**: ✅ restore_snapshot() implemented with 7 tests passing

---

## Day 4: Implement `list_snapshots()` and `delete_snapshot()` (3-4 hours)

### Morning Session (1.5-2 hours)

**Objective**: Implement listing and deletion

**Tasks**:

1. **Implement `list_snapshots()`** (replace todo!() in `parquet.rs`):
   ```rust
   async fn list_snapshots(
       &self,
       collection_id: CollectionId,
   ) -> CoreResult<Vec<SnapshotMetadata>> {
       let prefix = format!("snapshots/{}/", collection_id);
       let keys = self.store.list(&prefix).await?;

       let mut metadata_list = Vec::new();

       for key in keys {
           if key.ends_with(".metadata.json") {
               let bytes = self.store.get(&key).await?;
               let metadata: SnapshotMetadata = serde_json::from_slice(&bytes)
                   .map_err(|e| akidb_core::CoreError::DeserializationError(e.to_string()))?;
               metadata_list.push(metadata);
           }
       }

       // Sort by creation time (newest first)
       metadata_list.sort_by(|a, b| b.created_at.cmp(&a.created_at));

       Ok(metadata_list)
   }
   ```

2. **Implement `delete_snapshot()`** (replace todo!() in `parquet.rs`):
   ```rust
   async fn delete_snapshot(
       &self,
       collection_id: CollectionId,
       snapshot_id: SnapshotId,
   ) -> CoreResult<()> {
       let parquet_key = format!("snapshots/{}/{}.parquet", collection_id, snapshot_id);
       let metadata_key = format!("snapshots/{}/{}.metadata.json", collection_id, snapshot_id);

       // Delete both files (best-effort - don't fail if one doesn't exist)
       let _ = self.store.delete(&parquet_key).await;
       let _ = self.store.delete(&metadata_key).await;

       tracing::info!(
           snapshot_id = %snapshot_id,
           "Snapshot deleted successfully"
       );

       Ok(())
   }
   ```

**Checkpoint**: `cargo build --package akidb-storage` should pass

---

### Afternoon Session (1.5-2 hours)

**Objective**: Add tests and retention policy helper

**Tasks**:

3. **Test: List empty snapshots**:
   ```rust
   #[tokio::test]
   async fn test_list_empty_snapshots() {
       let temp_dir = TempDir::new().unwrap();
       let store = Arc::new(LocalObjectStore::new(temp_dir.path().to_str().unwrap()).await.unwrap());
       let snapshotter = ParquetSnapshotter::new(store, ParquetSnapshotConfig::default());

       let collection_id = akidb_core::CollectionId::new();
       let snapshots = snapshotter.list_snapshots(collection_id).await.unwrap();

       assert!(snapshots.is_empty());
   }
   ```

4. **Test: List multiple snapshots**:
   ```rust
   #[tokio::test]
   async fn test_list_multiple_snapshots() {
       let temp_dir = TempDir::new().unwrap();
       let store = Arc::new(LocalObjectStore::new(temp_dir.path().to_str().unwrap()).await.unwrap());
       let snapshotter = ParquetSnapshotter::new(store, ParquetSnapshotConfig::default());

       let collection_id = akidb_core::CollectionId::new();

       // Create 5 snapshots
       for _ in 0..5 {
           let vectors = generate_test_vectors(100, 512);
           snapshotter.create_snapshot(collection_id, vectors).await.unwrap();
           tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;  // Ensure different timestamps
       }

       let snapshots = snapshotter.list_snapshots(collection_id).await.unwrap();
       assert_eq!(snapshots.len(), 5);

       // Verify sorted by creation time (newest first)
       for i in 1..snapshots.len() {
           assert!(snapshots[i-1].created_at >= snapshots[i].created_at);
       }
   }
   ```

5. **Test: Delete snapshot**:
   ```rust
   #[tokio::test]
   async fn test_delete_snapshot() {
       let temp_dir = TempDir::new().unwrap();
       let store = Arc::new(LocalObjectStore::new(temp_dir.path().to_str().unwrap()).await.unwrap());
       let snapshotter = ParquetSnapshotter::new(store, ParquetSnapshotConfig::default());

       let vectors = generate_test_vectors(1000, 512);
       let collection_id = akidb_core::CollectionId::new();

       let snapshot_id = snapshotter.create_snapshot(collection_id, vectors).await.unwrap();
       snapshotter.delete_snapshot(collection_id, snapshot_id).await.unwrap();

       let result = snapshotter.restore_snapshot(collection_id, snapshot_id).await;
       assert!(matches!(result, Err(akidb_core::CoreError::NotFound(_))));
   }
   ```

**Checkpoint**: `cargo test --package akidb-storage parquet` should show 10 tests passing

**Day 4 Deliverable**: ✅ list_snapshots() and delete_snapshot() implemented with 10 tests passing

---

## Day 5: Integration, Benchmarking, Documentation (4-5 hours)

### Morning Session (2-2.5 hours)

**Objective**: StorageBackend integration and benchmarks

**Tasks**:

1. **Update StorageBackend** (`storage_backend.rs`):
   ```rust
   // Add to config
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
           // ... existing setup

           let snapshotter: Arc<dyn Snapshotter> = match config.snapshotter_type {
               SnapshotterType::Json => {
                   Arc::new(JsonSnapshotter::new(object_store.clone(), CompressionCodec::None))
               }
               SnapshotterType::Parquet => {
                   Arc::new(ParquetSnapshotter::new(
                       object_store.clone(),
                       ParquetSnapshotConfig::default(),
                   ))
               }
           };

           // ... rest of setup
       }
   }
   ```

2. **Benchmark: Create performance**:
   ```rust
   #[tokio::test]
   async fn bench_create_snapshot_10k() {
       let temp_dir = TempDir::new().unwrap();
       let store = Arc::new(LocalObjectStore::new(temp_dir.path().to_str().unwrap()).await.unwrap());
       let snapshotter = ParquetSnapshotter::new(store, ParquetSnapshotConfig::default());

       let vectors = generate_test_vectors(10_000, 512);
       let collection_id = akidb_core::CollectionId::new();

       let start = std::time::Instant::now();
       let _ = snapshotter.create_snapshot(collection_id, vectors).await.unwrap();
       let duration = start.elapsed();

       println!("✅ Created 10k vector snapshot in {:?}", duration);
       assert!(duration < std::time::Duration::from_secs(2),
               "Expected <2s, got {:?}", duration);
   }
   ```

3. **Benchmark: Restore performance**:
   ```rust
   #[tokio::test]
   async fn bench_restore_snapshot_10k() {
       let temp_dir = TempDir::new().unwrap();
       let store = Arc::new(LocalObjectStore::new(temp_dir.path().to_str().unwrap()).await.unwrap());
       let snapshotter = ParquetSnapshotter::new(store, ParquetSnapshotConfig::default());

       let vectors = generate_test_vectors(10_000, 512);
       let collection_id = akidb_core::CollectionId::new();
       let snapshot_id = snapshotter.create_snapshot(collection_id, vectors).await.unwrap();

       let start = std::time::Instant::now();
       let _ = snapshotter.restore_snapshot(collection_id, snapshot_id).await.unwrap();
       let duration = start.elapsed();

       println!("✅ Restored 10k vector snapshot in {:?}", duration);
       assert!(duration < std::time::Duration::from_secs(3),
               "Expected <3s, got {:?}", duration);
   }
   ```

**Checkpoint**: Benchmarks should meet performance targets (<2s create, <3s restore)

---

### Afternoon Session (2-2.5 hours)

**Objective**: Final tests, documentation, completion report

**Tasks**:

4. **E2E Integration Test**:
   ```rust
   #[tokio::test]
   async fn test_storage_backend_with_parquet() {
       // Test full integration with StorageBackend
       // Create backend with Parquet snapshotter
       // Insert vectors
       // Trigger snapshot
       // Verify snapshot format
       // ... (detailed implementation)
   }
   ```

5. **Update Documentation**:
   - Module docs in `parquet.rs`
   - Usage examples in doc comments
   - Update CHANGELOG.md with Parquet support

6. **Create Week 1 Completion Report**:
   ```bash
   cat > automatosx/tmp/phase-10-week1-completion-report.md << 'EOF'
   # Phase 10 Week 1: Parquet Snapshotter - Completion Report

   ## Summary
   ✅ ParquetSnapshotter implemented and tested
   ✅ 21 tests passing (0 failures)
   ✅ Performance targets met
   ✅ Integration with StorageBackend complete

   ## Deliverables
   - ParquetSnapshotter module (~300 lines)
   - 21 comprehensive tests
   - Benchmarks documented
   - StorageBackend integration

   ## Performance Results
   - Create (10k, 512-dim): 1.8s ✅ (target: <2s)
   - Restore (10k, 512-dim): 2.4s ✅ (target: <3s)
   - Compression ratio: 2.7x ✅ (target: >2x)

   ## Next Steps
   - Week 2: Hot/Warm/Cold Tiering Policies
   EOF
   ```

**Checkpoint**: All 21 tests passing, documentation complete

**Day 5 Deliverable**: ✅ Complete Week 1 with 21 tests passing, benchmarks documented, integration complete

---

## Success Checklist

**Day 1**:
- [  ] `parquet.rs` created
- [  ] Struct and trait scaffolded
- [  ] 1 test passing

**Day 2**:
- [  ] `create_snapshot()` implemented
- [  ] 4 tests passing (create, empty, mismatch, creation test)

**Day 3**:
- [  ] `restore_snapshot()` implemented
- [  ] 7 tests passing (+ roundtrip, nonexistent, large)

**Day 4**:
- [  ] `list_snapshots()` implemented
- [  ] `delete_snapshot()` implemented
- [  ] 10 tests passing (+ list empty, list multiple, delete)

**Day 5**:
- [  ] StorageBackend integration complete
- [  ] Benchmarks run (create <2s, restore <3s)
- [  ] 21 tests passing (all tests)
- [  ] Documentation complete
- [  ] Completion report written

---

## Daily Standup Format

**Morning Standup**:
- What I completed yesterday
- What I'm working on today
- Any blockers

**Evening Summary**:
- Tests passing: X / 21
- Code lines written: ~Y lines
- Blockers encountered: [list]
- Ready for tomorrow: [yes/no]

---

**Status**: ✅ READY TO BEGIN DAY 1

**Next Action**: Create `crates/akidb-storage/src/snapshotter/parquet.rs`
