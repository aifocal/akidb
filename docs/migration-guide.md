# AkiDB Storage API Migration Guide

**Version**: 0.2.0
**Date**: 2025-10-23

---

## Overview

This guide explains the migration from the deprecated `write_segment` API to the new `write_segment_with_data` API, which provides complete segment persistence with **SEGv1 binary format** support.

## Why Migrate?

### Previous Workflow (Deprecated)
```rust
// Old: Two-step process with manual serialization
let descriptor = SegmentDescriptor { ... };
storage.write_segment(&descriptor).await?;  // Only writes JSON metadata

// Manually serialize and upload vectors
let segment_data = SegmentData::new(dimension, vectors)?;
let serialized = writer.write(&segment_data)?;
storage.put_object(&key, serialized.into()).await?;
```

**Problems**:
- ❌ Requires manual vector serialization
- ❌ Two separate operations (not atomic)
- ❌ Easy to forget vector data upload
- ❌ Error-prone key management

### New Workflow (Recommended)
```rust
// New: Single atomic operation with SEGv1 format
storage.write_segment_with_data(
    &descriptor,
    vectors,
    Some(metadata)  // Optional metadata block
).await?;
```

**Benefits**:
- ✅ **Single atomic operation** with optimistic locking
- ✅ **Automatic SEGv1 serialization** with Zstd compression
- ✅ **Metadata support** (Arrow IPC format)
- ✅ **Manifest auto-update** (tracks all segments)
- ✅ **Built-in validation** (dimension checks, deduplication)

---

## Migration Steps

### 1. Update Storage Backend Calls

#### Before:
```rust
use akidb_storage::{StorageBackend, SegmentWriter, CompressionType, ChecksumType};

async fn write_vectors(
    storage: &impl StorageBackend,
    descriptor: &SegmentDescriptor,
    vectors: Vec<Vec<f32>>,
) -> Result<()> {
    // Write descriptor (JSON only)
    storage.write_segment(descriptor).await?;

    // Manually create segment data
    let segment_data = SegmentData::new(descriptor.vector_dim as u32, vectors)?;

    // Serialize with SEGv1 format
    let writer = SegmentWriter::new(CompressionType::Zstd, ChecksumType::XXH3);
    let serialized = writer.write(&segment_data)?;

    // Upload to S3 manually
    let key = format!(
        "collections/{}/segments/{}.seg",
        descriptor.collection,
        descriptor.segment_id
    );
    storage.put_object(&key, serialized.into()).await?;

    Ok(())
}
```

#### After:
```rust
use akidb_storage::StorageBackend;

async fn write_vectors(
    storage: &impl StorageBackend,
    descriptor: &SegmentDescriptor,
    vectors: Vec<Vec<f32>>,
    metadata: Option<MetadataBlock>,
) -> Result<()> {
    // Single call handles everything
    storage.write_segment_with_data(descriptor, vectors, metadata).await?;

    Ok(())
}
```

---

### 2. Update E2E Tests

#### Before:
```rust
#[tokio::test]
async fn test_segment_persistence() {
    let storage = MemoryStorageBackend::new();
    let descriptor = create_test_descriptor();
    let vectors = generate_test_vectors(100, 128);

    // Old API: two steps
    storage.write_segment(&descriptor).await.unwrap();

    // Manual serialization omitted in test (bug prone!)

    // Check existence
    let key = format!("collections/test/segments/{}.json", descriptor.segment_id);
    assert!(storage.object_exists(&key).await.unwrap());
}
```

#### After:
```rust
#[tokio::test]
async fn test_segment_persistence() {
    let storage = MemoryStorageBackend::new();
    let descriptor = create_test_descriptor();
    let vectors = generate_test_vectors(100, 128);

    // New API: single call with vectors
    storage.write_segment_with_data(&descriptor, vectors, None).await.unwrap();

    // Check existence (SEGv1 format)
    let key = format!("collections/test/segments/{}.seg", descriptor.segment_id);
    assert!(storage.object_exists(&key).await.unwrap());
}
```

---

### 3. Add Metadata Support (Optional)

If you need to persist vector metadata (payloads):

```rust
use akidb_storage::MetadataBlock;

// Create metadata from JSON payloads
let payloads = vec![
    json!({"label": "product_1", "price": 99.99}),
    json!({"label": "product_2", "price": 149.99}),
];

let metadata = MetadataBlock::from_json(payloads)?;

// Write segment with metadata
storage.write_segment_with_data(
    &descriptor,
    vectors,
    Some(metadata)  // Persisted alongside vectors
).await?;

// Load segment recovers both vectors and metadata
let segment_data = storage.load_segment(&collection_name, segment_id).await?;
```

---

## API Compatibility

### Deprecated Methods

| Method | Status | Replacement | Since |
|--------|--------|-------------|-------|
| `write_segment(&descriptor)` | ⚠️ **Deprecated** | `write_segment_with_data(&descriptor, vectors, metadata)` | 0.2.0 |

### Compiler Warnings

After upgrading to 0.2.0+, you will see:

```
warning: use of deprecated method `backend::StorageBackend::write_segment`:
  use `write_segment_with_data` for complete segment persistence with SEGv1 format
```

**Action**: Follow this migration guide to update your code.

---

## Migration Checklist

- [ ] **Update all `write_segment` calls** to `write_segment_with_data`
- [ ] **Pass vectors and optional metadata** to new API
- [ ] **Remove manual serialization code** (SEGv1 serialization handled internally)
- [ ] **Remove manual `put_object` calls** for segment data
- [ ] **Update E2E tests** to use new API
- [ ] **Verify all tests pass** after migration
- [ ] **Check for deprecation warnings** with `cargo build`

---

## Backward Compatibility

### Will my old code break?

**No** - the old API is **deprecated** but still functional in 0.2.0.

### When will `write_segment` be removed?

Target removal: **v0.3.0** (estimated Q2 2026)

**Recommendation**: Migrate now to avoid breaking changes in future versions.

---

## Performance Impact

### SEGv1 Format Benefits

| Metric | JSON (Old) | SEGv1 (New) | Improvement |
|--------|-----------|-------------|-------------|
| **Compression** | None | Zstd | ~70% size reduction |
| **Serialization Speed** | ~50 MB/s | **~200 MB/s** | **4x faster** |
| **Deserialization Speed** | ~40 MB/s | **~300 MB/s** | **7.5x faster** |
| **Checksum** | None | XXH3 | Data integrity |

### Optimistic Locking Overhead

- **Single-threaded**: < 1% overhead (negligible)
- **High-concurrency**: < 5% overhead (10 concurrent writers)
- **Benefit**: Eliminates data corruption from concurrent writes

See `tmp/PHASE3-M2-PERFORMANCE-VALIDATION.md` for detailed benchmarks.

---

## Troubleshooting

### Error: "Dimension mismatch"

**Cause**: Vectors don't match collection's `vector_dim`

**Solution**:
```rust
// Ensure all vectors have correct dimension
assert_eq!(vectors[0].len(), descriptor.vector_dim as usize);
```

### Error: "Segment already exists"

**Cause**: Attempting to write duplicate `segment_id`

**Solution**:
```rust
// Generate unique segment IDs
use uuid::Uuid;
let segment_id = Uuid::new_v4();
```

### Error: "Collection not found"

**Cause**: Writing segment before creating collection

**Solution**:
```rust
// Create collection first
storage.create_collection(&collection_descriptor).await?;

// Then write segments
storage.write_segment_with_data(&descriptor, vectors, None).await?;
```

---

## Support

- **Documentation**: `/docs/`
- **Examples**: `services/akidb-api/tests/e2e_test.rs`
- **Performance Guide**: `docs/performance-guide.md`
- **Issue Tracker**: [GitHub Issues](https://github.com/defai-digital/akidb/issues)

---

**Last Updated**: 2025-10-23 (Phase 3 M2)
**Related**: `tmp/PHASE3-M2-UPDATED-STRATEGY-2025-10-23.md`
