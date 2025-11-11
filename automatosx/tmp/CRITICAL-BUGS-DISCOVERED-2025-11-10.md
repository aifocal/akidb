# AkiDB 2.0 - Critical Bugs Discovered (2025-11-10)

**Date:** 2025-11-10
**Discovered By:** AutomatosX Backend Agent (2 analysis runs)
**Status:** 🔴 12 CRITICAL BUGS REQUIRE IMMEDIATE ATTENTION
**Priority:** PRODUCTION BLOCKING

---

## Executive Summary

Following the implementation of 4 critical operational fixes (graceful shutdown, config validation, health checks, signal handlers), two independent AutomatosX agent analyses discovered **12 additional critical bugs** that will cause data loss, resource leaks, and incorrect behavior in production.

**Impact Categories:**
- 🔴 **Data Loss:** 5 bugs (Bugs #2, #3, #4, #5, #6)
- 🔴 **Resource Leaks:** 1 bug (Bug #1)
- 🟡 **Incorrect Behavior:** 4 bugs (Bugs #7, #8, #9, #12)
- 🟢 **Build/Test Issues:** 2 bugs (Bugs #10, #11)

---

## Critical Bugs (Data Loss & Resource Leaks)

### Bug #1: Collection Deletion Leaks Background Tasks 🔴

**Severity:** CRITICAL (Resource Leak)
**Location:** `crates/akidb-service/src/collection_service.rs:453-474` + `crates/akidb-storage/src/storage_backend.rs:1665-1693`

**Issue:**
When a collection is deleted, the code drops the `Arc<StorageBackend>` without calling `StorageBackend::shutdown()`. Background tasks (S3 uploader, retry worker, compaction worker, DLQ cleanup) keep running detached, WAL buffers are not flushed, and task handles never abort.

**Impact:**
- Leaked tasks consume CPU/memory indefinitely
- WAL corruption risk (unflushed buffers)
- S3 uploads may continue for deleted collections

**Fix Required:**
```rust
// In delete_collection():
if let Some(backend) = self.storage_backends.write().await.remove(&collection_id) {
    // Shutdown before dropping
    backend.shutdown().await?;
}

// Also add Drop guard in CollectionService
impl Drop for CollectionService {
    fn drop(&mut self) {
        // Iterate all backends and shutdown
    }
}
```

---

### Bug #2: Insert-Before-Index Causes Ghost Records After Crash 🔴

**Severity:** CRITICAL (Data Loss/Corruption)
**Location:** `crates/akidb-service/src/collection_service.rs:546-571`

**Issue:**
Inserts write to `StorageBackend` (WAL + S3) **before** touching the in-memory index. If `index.insert()` fails (dimension mismatch, HNSW error, cancellation), the method returns an error but the durable layer already stored the document. After restart, replaying WAL reintroduces the vector even though the index never accepted it.

**Impact:**
- "Ghost" records in storage but not in index
- Future inserts of same `doc_id` fail (duplicate key error)
- Data integrity violation

**Fix Required:**
```rust
// Insert into index FIRST
index.insert(doc.clone()).await?;

// THEN persist (rollback on error)
if let Err(e) = backend.persist(...).await {
    index.delete(doc_id).await?; // Rollback
    return Err(e);
}
```

---

### Bug #3: Double StorageBackend Construction Loses Data 🔴

**Severity:** CRITICAL (Data Loss)
**Location:** `crates/akidb-service/src/collection_service.rs:501-547` + `870-922`

**Issue:**
`create_collection()` calls `load_collection()`, which already instantiates a `StorageBackend`, loads legacy vectors from SQLite (870-901), persists them via that backend, and stores it in `storage_backends`. Step 4 immediately re-derives a config, builds a **new** backend, and overwrites the map entry without copying recovered WAL data or shutting down the first backend.

**Impact:**
- Migrated legacy vectors only live in RAM
- Restart silently discards recovered data
- First backend's WAL writes lost

**Fix Required:**
- Reuse backend created in `load_collection()` instead of double-constructing
- OR defer loading until after single backend created

---

### Bug #4: Random CollectionId Breaks S3 Backups/DLQ 🔴

**Severity:** CRITICAL (Data Loss in Backups)
**Location:** `crates/akidb-storage/src/storage_backend.rs:1181-1210`, `1330-1349`, `912-919`, `1034-1048`, `1147-1149`

**Issue:**
Every WAL entry, S3 upload task, retry key, and compaction snapshot uses `CollectionId::new()` (random UUID) instead of the real collection ID. Inserts/deletes logged under random IDs, uploaded to `vectors/<random>/<doc>`, so S3 backups and DLQ retries cannot be correlated with real collections. Snapshots also land under random prefixes, making restores impossible.

**Impact:**
- S3 backups unusable (random prefixes)
- DLQ retries fail (wrong collection ID)
- Disaster recovery impossible

**Fix Required:**
- Thread real `collection_id` through backend config/API
- Stop fabricating random IDs

---

### Bug #5: WAL Rotation LSN Bug Loses Data 🔴

**Severity:** CRITICAL (Data Loss in Replication)
**Location:** `crates/akidb-storage/src/wal/file_wal.rs:241-264` & `374-389`

**Issue:**
`get_wal_files()` filters files based on LSN embedded in filename, but `rotate()` names new file with **current LSN** (last entry written) instead of **next LSN** the file will contain. After rotating, `replay(from_lsn > current_lsn)` skips the file that actually holds desired entries.

**Impact:**
- Incremental replication loses data
- Checkpoint replay skips entries
- Silent data loss on failover

**Fix Required:**
- Rename rotated files with `current_lsn.next()` (first LSN stored in file)
- OR inspect file contents instead of using filenames for filtering

---

### Bug #6: Compaction Thresholds Never Trigger Correctly 🔴

**Severity:** CRITICAL (WAL Unbounded Growth)
**Location:** `crates/akidb-storage/src/storage_backend.rs:94`, `1433-1435`, `1580-1608`, `1105-1129`

**Issue:**
Compaction thresholds broken. Byte threshold is dead code (`StorageMetrics::wal_size_bytes` never updated). Op threshold compares `metrics.inserts` (lifetime counter) against `compaction_threshold_ops` and never resets after `perform_compaction()`, so once triggered, `should_compact()` stays true forever and background worker compacts continuously even with empty WAL.

**Impact:**
- WAL grows unbounded (byte threshold broken)
- Continuous compaction (op threshold broken)
- CPU waste, disk thrashing

**Fix Required:**
- Track actual WAL sizes
- Maintain "since last compaction" counter (or reset `metrics.inserts`)

---

## High Priority Bugs (Incorrect Behavior)

### Bug #7: Queries Counter Never Incremented 🟡

**Severity:** HIGH (Observability)
**Location:** `crates/akidb-storage/src/storage_backend.rs:82` & `1250-1318`

**Issue:**
The `queries` counter in `StorageMetrics` is never incremented on any read path (Memory, MemoryS3, S3Only). Observability dashboards always show zero queries, making SLOs and alerting meaningless.

**Fix Required:**
- Increment `metrics.queries` whenever `get()` succeeds

---

### Bug #8: Zero Vector Search Causes NaN with Cosine 🟡

**Severity:** HIGH (Incorrect Results)
**Location:** `crates/akidb-index/src/instant_hnsw.rs:180-189`, `360-417`

**Issue:**
Inserts reject zero vectors for Cosine, but search does not. `normalize_vector()` silently returns zero query unchanged when norm is zero, and `search()` proceeds, producing undefined cosine similarities (NaN scores) and unstable ranking.

**Fix Required:**
- Mirror insert-time validation: reject cosine searches where query norm is zero

---

### Bug #9: Soft-Deleted Vectors Appear in Search Results 🟡

**Severity:** HIGH (Incorrect Results)
**Location:** `crates/akidb-index/src/hnsw.rs:640-676` vs. `689-700`

**Issue:**
Deletes only set `node.deleted` flag, yet `search()` converts candidates straight into `SearchResult` without checking the flag. Soft-deleted vectors continue to appear in results indefinitely.

**Fix Required:**
- Filter out `deleted` nodes before building result list

---

### Bug #12: EmbeddingManager Panics Without Runtime 🟡

**Severity:** HIGH (Startup Crash)
**Location:** `crates/akidb-service/src/embedding_manager.rs:23-45`

**Issue:**
`EmbeddingManager::new()` is synchronous but calls `tokio::task::block_in_place(|| Handle::current().block_on(...))`. Outside of an active Tokio runtime (non-async unit tests, CLI/worker startup) this panics with "There is no reactor running…".

**Fix Required:**
- Make constructor `async` so callers `await` it
- OR create dedicated runtime (`Runtime::new()?.block_on()`)

---

## Medium Priority Bugs (Build/Test)

### Bug #10: Parallel Upload Benchmark Outdated API 🟢

**Severity:** MEDIUM (Build Failure)
**Location:** `crates/akidb-storage/benches/parallel_upload_bench.rs:29-148`

**Issue:**
Benchmark targets pre-RC1 APIs:
- `LocalObjectStore::new(...)` without `.await`
- `VectorDocument::new` without required `DocumentId`
- Non-existent `S3BatchConfig::enabled` field
- Calls `flush_all()` instead of `flush_all_parallel()`

**Impact:**
- `cargo clippy --all-targets` fails
- Benchmark suite broken

**Fix Required:**
- Update bench to RC1 APIs

---

### Bug #11: Python 3.13 Dependency Breaks Tests 🟢

**Severity:** MEDIUM (Test Failure)
**Location:** `crates/akidb-embedding/Cargo.toml` + embedding tests

**Issue:**
Embedding crate links against `libpython3.13.dylib`. On machines without Python 3.13, test binary aborts during load (`dyld: Library not loaded`). Even with `abi3-py310` feature, still links to 3.13.

**Impact:**
- `cargo test --workspace` fails
- CI/CD breaks on different machines

**Fix Required:**
- Switch to `abi3` feature with lower version (e.g., `abi3-py38`)
- OR gate MLX provider/tests behind cargo feature

---

## Impact Analysis

### Production Deployment Risk: 🔴 VERY HIGH

If deployed with these bugs:

1. **Data Loss Scenarios:**
   - Crash during insert → ghost records (Bug #2)
   - Collection migration → silent data loss (Bug #3)
   - S3 backups unusable → disaster recovery fails (Bug #4)
   - WAL replay → missing entries (Bug #5)
   - WAL unbounded growth → disk full (Bug #6)

2. **Resource Exhaustion:**
   - Deleted collections leak tasks → memory exhaustion (Bug #1)
   - Continuous compaction → CPU exhaustion (Bug #6)

3. **Incorrect Behavior:**
   - Zero vector queries → NaN results, crashes (Bug #8)
   - Deleted vectors still returned → data leakage (Bug #9)
   - Metrics always zero → blind observability (Bug #7)

---

## Recommended Action Plan

### Phase 1: Immediate Fixes (Day 1 - CRITICAL)

**Priority Order:**
1. Bug #2 (Insert ordering) - DATA CORRUPTION RISK
2. Bug #4 (Random CollectionId) - BACKUPS BROKEN
3. Bug #1 (Resource leaks) - MEMORY EXHAUSTION
4. Bug #5 (WAL LSN) - REPLICATION DATA LOSS

**Estimated Time:** 6-8 hours

---

### Phase 2: High Priority (Day 2)

5. Bug #3 (Double backend) - DATA LOSS ON MIGRATION
6. Bug #6 (Compaction) - WAL UNBOUNDED GROWTH
7. Bug #8 (Zero vector) - NaN RESULTS
8. Bug #9 (Soft delete) - INCORRECT RESULTS

**Estimated Time:** 4-6 hours

---

### Phase 3: Medium Priority (Day 3)

9. Bug #7 (Metrics) - OBSERVABILITY
10. Bug #12 (EmbeddingManager) - STARTUP CRASH
11. Bug #10 (Benchmark) - BUILD FAILURE
12. Bug #11 (Python) - TEST FAILURE

**Estimated Time:** 3-4 hours

---

## Testing Strategy

After each fix:
1. Unit test for the specific bug
2. Integration test with failure injection
3. Regression test for related functionality
4. Load test to ensure no performance degradation

**Total Testing Time:** ~2-3 hours per bug

---

## Conclusion

**DO NOT DEPLOY TO PRODUCTION** until at least Bugs #1-6 are fixed. These bugs will cause:
- Silent data loss
- Resource exhaustion
- Unrecoverable backups
- Data corruption

**Estimated Total Fix Time:** 15-20 hours (implementation + testing)

**Recommended Approach:** Fix bugs #1-6 this week, deploy to staging for 1 week validation, then proceed to production.

---

**Created:** 2025-11-10
**Discovered By:** AutomatosX Backend Agent
**Status:** PRODUCTION BLOCKING
**Next Action:** Begin Phase 1 fixes immediately

