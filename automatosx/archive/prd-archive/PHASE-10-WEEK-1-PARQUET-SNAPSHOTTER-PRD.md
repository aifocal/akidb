# Phase 10 Week 1: Parquet Snapshotter - Product Requirements Document

**Version**: 1.0
**Date**: 2025-11-09
**Status**: ✅ APPROVED
**Owner**: Phase 10 Team
**Timeline**: 5 days (Week 1)

---

## Executive Summary

Implement production-ready **Parquet Snapshotter** to replace JSON-based vector snapshots with efficient columnar storage, achieving **2-3x compression** and **90% reduction in S3 API calls**.

**Business Value**:
- Reduced S3 storage costs (50-70% savings)
- Faster backup/restore operations
- Better analytics integration (Parquet is standard for data warehouses)
- Foundation for Week 2 tiering policies

**Technical Value**:
- Columnar storage optimized for vector data
- Streaming-friendly format
- Industry-standard compression (Snappy, Zstd)
- Future-proof for big data analytics

---

## Goals and Non-Goals

### Goals
- ✅ Implement `ParquetSnapshotter` with Snapshotter trait
- ✅ Achieve >2x compression vs JSON snapshots
- ✅ Create snapshot in <2s for 10k vectors (512-dim)
- ✅ Restore snapshot in <3s for 10k vectors
- ✅ 100% data integrity (no corruption)
- ✅ Integration with existing StorageBackend
- ✅ 20+ tests with 100% pass rate

### Non-Goals
- ❌ Multi-file snapshots (deferred to Week 2)
- ❌ Incremental snapshots (future enhancement)
- ❌ Automatic tiering (Week 2 scope)
- ❌ Cross-region replication (Phase 11+)

---

## User Stories

### Story 1: Database Administrator
**As a** database administrator
**I want** efficient vector backups to S3
**So that** I can reduce storage costs and improve disaster recovery time

**Acceptance Criteria**:
- Snapshot creation completes in <2s for 10k vectors
- File size is 50-70% smaller than JSON
- Restore operation is data-lossless

### Story 2: MLOps Engineer
**As an** MLOps engineer
**I want** Parquet-format vector snapshots
**So that** I can analyze vector distributions with standard BI tools (Athena, BigQuery)

**Acceptance Criteria**:
- Snapshots are valid Parquet files
- Compatible with Apache Arrow ecosystem
- Metadata includes schema version

### Story 3: Application Developer
**As an** application developer
**I want** transparent snapshot format migration
**So that** my existing code continues to work without changes

**Acceptance Criteria**:
- Same Snapshotter trait interface
- Config option to choose JSON vs Parquet
- Backward-compatible metadata format

---

## Technical Specification

### Architecture

```
┌─────────────────────────────────────────────────────────┐
│              ParquetSnapshotter                         │
│                                                         │
│  1. Create Snapshot                                     │
│     VectorDocument[] → ParquetEncoder → Bytes          │
│                                  ↓                      │
│                         ObjectStore.put()               │
│                                  ↓                      │
│                    S3/MinIO: {snapshot_id}.parquet      │
│                                                         │
│  2. Restore Snapshot                                    │
│                    S3/MinIO: {snapshot_id}.parquet      │
│                                  ↓                      │
│                         ObjectStore.get()               │
│                                  ↓                      │
│     VectorDocument[] ← ParquetEncoder ← Bytes          │
└─────────────────────────────────────────────────────────┘
```

### API Design

**Snapshotter Trait** (existing, shared with JsonSnapshotter):
```rust
#[async_trait]
pub trait Snapshotter: Send + Sync {
    /// Create snapshot from vectors
    async fn create_snapshot(
        &self,
        collection_id: CollectionId,
        vectors: Vec<VectorDocument>,
    ) -> CoreResult<SnapshotId>;

    /// Restore vectors from snapshot
    async fn restore_snapshot(
        &self,
        collection_id: CollectionId,  // NEW: added parameter
        snapshot_id: SnapshotId,
    ) -> CoreResult<Vec<VectorDocument>>;

    /// List all snapshots for collection
    async fn list_snapshots(
        &self,
        collection_id: CollectionId,
    ) -> CoreResult<Vec<SnapshotMetadata>>;

    /// Delete snapshot
    async fn delete_snapshot(
        &self,
        collection_id: CollectionId,  // NEW: added parameter
        snapshot_id: SnapshotId,
    ) -> CoreResult<()>;
}
```

**ParquetSnapshotter Implementation**:
```rust
pub struct ParquetSnapshotter {
    store: Arc<dyn ObjectStore>,
    encoder: ParquetEncoder,
    config: ParquetSnapshotConfig,
}

pub struct ParquetSnapshotConfig {
    /// Compression algorithm (Snappy recommended)
    pub compression: Compression,
    /// Row group size for Parquet (default: 10,000)
    pub row_group_size: usize,
    /// Enable dictionary encoding (recommended)
    pub enable_dictionary: bool,
}
```

### Data Model

**Parquet Schema**:
```
message VectorSnapshot {
  required binary document_id;      // UUID as 16 bytes
  optional binary external_id (UTF8);
  required int32 dimension;
  required group vector (LIST) {    // Fixed-size list
    repeated float value;
  }
  optional binary metadata_json (UTF8);
  required int64 inserted_at (TIMESTAMP(MILLIS,true));
}
```

**SnapshotMetadata** (JSON file):
```rust
pub struct SnapshotMetadata {
    pub snapshot_id: SnapshotId,
    pub collection_id: CollectionId,
    pub vector_count: u64,
    pub dimension: u32,
    pub created_at: DateTime<Utc>,
    pub size_bytes: u64,
    pub compression: Compression,
    pub format: SnapshotFormat,  // NEW: Parquet vs JSON
}

pub enum SnapshotFormat {
    Json,
    Parquet,
}
```

**S3 Key Structure**:
```
s3://bucket/snapshots/{collection_id}/
  ├── {snapshot_id}.parquet           # Vector data (columnar)
  └── {snapshot_id}.metadata.json     # Snapshot metadata
```

### Configuration

**StorageBackend Config**:
```rust
pub struct StorageBackendConfig {
    // ... existing fields
    pub snapshotter_type: SnapshotterType,  // NEW
}

pub enum SnapshotterType {
    Json,
    Parquet,
}
```

**Example Config (TOML)**:
```toml
[storage]
snapshotter_type = "parquet"  # or "json"

[storage.parquet]
compression = "snappy"        # or "zstd", "none"
row_group_size = 10000
enable_dictionary = true
```

---

## Implementation Phases

### Day 1: Scaffold (2-3 hours)
- Create `parquet.rs` module
- Define ParquetSnapshotter struct
- Scaffold trait methods (todo!() stubs)
- First unit test

### Day 2: Create Snapshot (3-4 hours)
- Implement `create_snapshot()`
- Validation logic (dimension, empty check)
- S3 upload integration
- 3 tests (create, empty, mismatch)

### Day 3: Restore Snapshot (3-4 hours)
- Implement `restore_snapshot()`
- Roundtrip test (create → restore)
- Error handling (corrupted data)
- 4 tests (roundtrip, nonexistent, corrupted, large)

### Day 4: Management Operations (3-4 hours)
- Implement `list_snapshots()`
- Implement `delete_snapshot()`
- Retention policy helper
- 4 tests (list, delete, cleanup)

### Day 5: Integration & Polish (4-5 hours)
- StorageBackend integration
- Performance benchmarks
- Documentation
- Final E2E test

---

## Testing Strategy

### Test Pyramid

**Unit Tests** (6 tests):
- Constructor with default config
- Constructor with custom config
- Empty snapshot validation
- Dimension mismatch validation
- Metadata serialization
- Compression config

**Integration Tests** (12 tests):
- Create snapshot (10k vectors)
- Restore snapshot (roundtrip)
- List snapshots (empty)
- List snapshots (multiple)
- Delete snapshot
- Large dataset (100k vectors)
- S3 upload/download
- LocalObjectStore fallback
- Concurrent snapshots
- Retention policy cleanup
- Error recovery (corrupted Parquet)
- Cross-format compatibility (JSON → Parquet)

**Benchmark Tests** (3 tests):
- Create performance (<2s for 10k)
- Restore performance (<3s for 10k)
- File size comparison (>2x compression)

**Total**: 21 tests

### Test Data

**Generators**:
```rust
fn generate_test_vectors(count: usize, dimension: usize) -> Vec<VectorDocument>;
fn generate_test_vector(dimension: usize) -> VectorDocument;
fn assert_vectors_equal(a: &[f32], b: &[f32], epsilon: f64);
```

**Sample Sizes**:
- Small: 100 vectors (quick tests)
- Medium: 10,000 vectors (performance tests)
- Large: 100,000 vectors (stress tests)

---

## Performance Requirements

| Metric | Target | Measurement |
|--------|--------|-------------|
| Create (10k, 512-dim) | <2s | Benchmark test |
| Restore (10k, 512-dim) | <3s | Benchmark test |
| File size (vs JSON) | >2x compression | File size comparison |
| Memory footprint (100k) | <1GB | Memory profiler |
| Data integrity | 100% (zero corruption) | Roundtrip test |

---

## Error Handling

### Error Types

**Validation Errors**:
- Empty snapshot → `CoreError::ValidationError`
- Dimension mismatch → `CoreError::ValidationError`
- Invalid collection ID → `CoreError::ValidationError`

**Storage Errors**:
- S3 upload failed → `CoreError::StorageError` (with retry)
- S3 download failed → `CoreError::StorageError` (with retry)
- Snapshot not found → `CoreError::NotFound`

**Data Errors**:
- Corrupted Parquet → `CoreError::DataCorruption`
- Encoding failed → `CoreError::EncodingError`
- Decoding failed → `CoreError::DecodingError`

### Retry Policy

**Transient Errors** (retry with exponential backoff):
- S3 500 errors
- S3 503 (throttling)
- Network timeouts

**Permanent Errors** (fail immediately):
- S3 403 (auth failure)
- S3 404 (not found)
- Corrupted Parquet data

---

## Dependencies

### External Crates
- `parquet = "53.2"` ✅ (already added)
- `arrow = "53.2"` ✅ (already added)
- `bytes = "1.5"` ✅
- `serde_json = "1.0"` ✅

### Internal Modules
- `akidb-core` (VectorDocument, CoreError)
- `object_store` (S3ObjectStore, LocalObjectStore)
- `parquet_encoder` (ParquetEncoder)

### Infrastructure
- **Optional**: MinIO for local S3 testing
- **Required**: LocalObjectStore for unit tests

---

## Migration Strategy

### Phase 1: Parallel Operation (Week 1)
- Both JSON and Parquet snapshotters available
- Config option to choose format
- Default: JSON (backward compatible)

### Phase 2: Gradual Migration (Week 2-3)
- New snapshots created in Parquet format
- Existing JSON snapshots remain valid
- Cross-format restore supported

### Phase 3: Deprecation (Week 4+)
- Default changed to Parquet
- JSON support marked deprecated
- Migration tool for JSON → Parquet conversion

**Migration Tool** (future):
```bash
akidb-cli snapshot migrate \
  --from-format json \
  --to-format parquet \
  --collection-id {id}
```

---

## Monitoring & Observability

### Metrics

**Snapshot Operations**:
- `snapshot_creates_total{format}` - Counter
- `snapshot_restores_total{format}` - Counter
- `snapshot_create_duration_seconds{format}` - Histogram
- `snapshot_restore_duration_seconds{format}` - Histogram
- `snapshot_size_bytes{format}` - Histogram

**Storage**:
- `s3_upload_bytes_total` - Counter
- `s3_download_bytes_total` - Counter
- `snapshot_compression_ratio{format}` - Gauge

### Logging

**Structured Logs** (tracing):
```rust
tracing::info!(
    snapshot_id = %snapshot_id,
    collection_id = %collection_id,
    vector_count = vectors.len(),
    size_bytes = parquet_bytes.len(),
    duration_ms = duration.as_millis(),
    "Snapshot created successfully"
);
```

---

## Documentation

### Code Documentation
- Module-level docs (`snapshotter/mod.rs`)
- Struct-level docs (`ParquetSnapshotter`)
- Method-level docs (all public methods)
- Usage examples in doc comments

### User Documentation
- Configuration guide (Parquet vs JSON)
- Performance tuning guide
- Migration guide (JSON → Parquet)
- Troubleshooting guide

### API Documentation
- OpenAPI spec update (if applicable)
- gRPC proto update (if applicable)

---

## Security Considerations

### Data Integrity
- SHA256 checksums for Parquet files (future enhancement)
- Metadata validation on restore
- Dimension verification

### Access Control
- S3 bucket policies (IAM roles)
- Encryption at rest (S3 server-side encryption)
- Encryption in transit (HTTPS for S3)

### Sensitive Data
- Metadata field may contain PII
- Document external IDs may contain sensitive info
- Encryption recommended for production

---

## Success Criteria

### Functional
- ✅ ParquetSnapshotter implements Snapshotter trait
- ✅ All CRUD operations working (create, restore, list, delete)
- ✅ Integration with StorageBackend complete
- ✅ 21 tests passing (0 failures)

### Performance
- ✅ Create: <2s for 10k vectors
- ✅ Restore: <3s for 10k vectors
- ✅ Compression: >2x vs JSON
- ✅ Memory: <1GB for 100k vectors

### Quality
- ✅ Zero data corruption (roundtrip integrity test)
- ✅ Clean error handling (no panics)
- ✅ Code coverage >85%
- ✅ Documentation complete
- ✅ Code review approved

---

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Parquet encoding performance | Medium | Medium | Benchmark early, tune compression |
| Memory pressure (large snapshots) | Medium | Medium | Document limits, plan streaming for Week 2 |
| S3 rate limits | Low | Low | Use batching (Week 4) |
| Trait signature changes | High | Medium | Update both implementations together |
| Data corruption | Low | High | Comprehensive roundtrip tests |

---

## Timeline

**Total Duration**: 5 days (18-22 hours)

**Daily Milestones**:
- **Day 1** (EOD): Scaffold complete, 1 test passing
- **Day 2** (EOD): Create snapshot working, 4 tests passing
- **Day 3** (EOD): Restore snapshot working, 8 tests passing
- **Day 4** (EOD): List/delete working, 12 tests passing
- **Day 5** (EOD): Integration complete, 21 tests passing, docs done

---

## Approval

**Product**: ✅ Approved
**Engineering**: ✅ Approved
**Architecture**: ✅ Approved

**Go-Live Date**: Day 5 (Week 1 completion)

---

## References

- **Main PRD**: `automatosx/PRD/PHASE-10-PRODUCTION-READY-V2-PRD.md`
- **Megathink**: `automatosx/tmp/PHASE-10-WEEK-1-COMPREHENSIVE-MEGATHINK.md`
- **Parquet Format**: https://parquet.apache.org/docs/file-format/
- **Arrow Schema**: https://arrow.apache.org/docs/python/parquet.html

---

**Status**: ✅ APPROVED - READY FOR IMPLEMENTATION
