# AutomatosX Round 2 - Bug Discovery & MEGATHINK Analysis

**Date:** 2025-11-09
**Discovery Method:** AutomatosX Backend Agent (Bob) - Round 2
**Status:** 🔴 **7 CRITICAL BUGS DISCOVERED**

---

## Executive Summary

After fixing 14 bugs across 4 rounds, the AutomatosX agent performed a second deep analysis and discovered **7 additional critical bugs** that cause:
- Data loss on server restart
- Impossible S3 backup/recovery
- WAL replay failures
- Deleted vectors appearing in search results
- Broken monitoring
- Continuous unnecessary compaction

**These are CRITICAL production-breaking bugs that MUST be fixed immediately.**

---

## All 7 Bugs Discovered

| # | Severity | Bug | Impact |
|---|----------|-----|--------|
| 15 | 🔴 CRITICAL | Double StorageBackend creation | Data loss on restart |
| 16 | 🔴 CRITICAL | Random CollectionIds in WAL/S3 | S3 backups unusable |
| 17 | 🔴 CRITICAL | WAL rotation LSN off-by-one | Replay data loss |
| 18 | 🟡 HIGH | Compaction threshold broken | Performance degradation |
| 19 | 🟡 HIGH | Queries counter never incremented | Monitoring broken |
| 20 | 🟡 HIGH | Zero vector search not validated | NaN/unstable results |
| 21 | 🔴 CRITICAL | Deleted vectors in search results | Data integrity violation |

---

## Detailed Bug Analysis

### 🔴 Bug #15: Double StorageBackend Creation (CRITICAL - Data Loss)

**Location:** `crates/akidb-service/src/collection_service.rs:501-547` + `870-922`

**Problem:**
```rust
// In create_collection():

// Step 3: Load collection (creates FIRST StorageBackend)
if let Err(e) = self.load_collection(&collection).await {
    // ... rollback ...
}

// In load_collection():
// Line 748: Creates StorageBackend #1
let storage_backend = Arc::new(StorageBackend::new(storage_config).await?);

// Lines 770-901: Loads legacy vectors from SQLite
if let Some(persistence) = &self.vector_persistence {
    let vectors = persistence.load_all_vectors(...).await?;
    for doc in vectors {
        // Persists to StorageBackend #1
        storage_backend.insert(doc).await?;
    }
}

// Lines 790-793: Stores StorageBackend #1 in map
let mut backends = self.storage_backends.write().await;
backends.insert(collection.collection_id, storage_backend);

// THEN back in create_collection():

// Step 4: Creates SECOND StorageBackend (lines 473-490)
let storage_config = match self.create_storage_backend_for_collection(&collection) {
    Ok(config) => config,
    // ... error handling ...
};

let storage_backend = match StorageBackend::new(storage_config).await {
    Ok(backend) => Arc::new(backend),
    // ... error handling ...
};

// Step 5: OVERWRITES the first backend! (lines 504-507)
{
    let mut backends = self.storage_backends.write().await;
    backends.insert(collection_id, storage_backend);  // OVERWRITES!
}
```

**Impact:**
- StorageBackend #1 is created and receives legacy SQLite vectors
- StorageBackend #2 is created fresh (empty WAL)
- StorageBackend #1 is dropped without shutdown
- **Migrated data exists only in RAM**
- **On restart: All migrated data is lost!**
- **Legacy SQLite vectors are loaded again on next startup → infinite migration loop**

**Fix:** Reuse the StorageBackend created in `load_collection` instead of creating a second one.

---

### 🔴 Bug #16: Random CollectionIds Everywhere (CRITICAL - S3 Unusable)

**Locations:**
- `crates/akidb-storage/src/storage_backend.rs:1181-1210` (insert WAL entry)
- `crates/akidb-storage/src/storage_backend.rs:1330-1349` (delete WAL entry)
- `crates/akidb-storage/src/storage_backend.rs:912-919` (S3 upload task)
- `crates/akidb-storage/src/storage_backend.rs:1034-1048` (retry task)
- `crates/akidb-storage/src/storage_backend.rs:1147-1149` (compaction snapshot)

**Problem:**
```rust
// Line 1195 in insert():
let entry = LogEntry::Upsert {
    collection_id: CollectionId::new(),  // RANDOM ID!
    doc_id: doc.doc_id,
    vector: doc.vector.clone(),
    // ...
};

// Line 916 in background S3 uploader:
let key = format!("vectors/{}/{}", CollectionId::new(), doc_id);  // RANDOM!

// Line 1148 in perform_compaction:
let snapshot_key = format!("snapshots/{}/snapshot-{}.parquet",
    CollectionId::new(),  // RANDOM!
    Utc::now().timestamp()
);
```

**Impact:**
- Every WAL entry has a random collection_id
- S3 uploads go to `vectors/<random-uuid>/<doc-id>`
- Snapshots go to `snapshots/<random-uuid>/...`
- **Cannot correlate S3 files with actual collections**
- **S3 backup/restore completely broken**
- **DLQ retries cannot find original collection**
- **Incremental replication impossible**

**Fix:** Thread the real CollectionId through StorageBackend constructor and use it everywhere.

---

### 🔴 Bug #17: WAL Rotation LSN Off-by-One (CRITICAL - Replay Data Loss)

**Location:** `crates/akidb-storage/src/wal/file_wal.rs:241-264`, `374-389`

**Problem:**
```rust
// Line 380-389 in rotate():
pub async fn rotate(&self) -> CoreResult<()> {
    let current_lsn = self.current_lsn().await?;

    // Creates new file with CURRENT lsn (last entry written)
    let new_file_path = self.base_path.with_extension(
        format!("wal.{}", current_lsn.value())  // WRONG!
    );
    // ...
}

// Line 241-264 in get_wal_files():
pub fn get_wal_files(&self, from_lsn: LogSequenceNumber) -> Vec<PathBuf> {
    // ...
    files.into_iter()
        .filter(|(_, lsn)| *lsn >= from_lsn)  // Filters by filename LSN
        .map(|(path, _)| path)
        .collect()
}
```

**Impact:**
- File rotated at LSN 1000 is named `wal.1000`
- Next entry (LSN 1001) goes into `wal.1000`
- `replay(from_lsn = 1001)` filters out `wal.1000` (1000 < 1001)
- **LSN 1001+ entries are skipped during replay**
- **Data loss on crash recovery**
- **Incremental replication broken**

**Fix:** Name rotated files with `current_lsn.next()` (first LSN they will contain).

---

### 🟡 Bug #18: Compaction Threshold Broken (HIGH - Performance)

**Location:** `crates/akidb-storage/src/storage_backend.rs:94`, `1433-1435`, `1580-1608`, `1105-1129`

**Problem:**
```rust
// Line 94 - StorageMetrics has wal_size_bytes field
pub struct StorageMetrics {
    pub wal_size_bytes: u64,  // NEVER UPDATED ANYWHERE!
    // ...
}

// Line 1433-1435 - should_compact checks:
fn should_compact(&self) -> bool {
    let metrics = self.metrics.read();

    // Byte threshold: DEAD CODE (wal_size_bytes always 0)
    if metrics.wal_size_bytes >= self.config.compaction_threshold_bytes {
        return true;
    }

    // Op threshold: NEVER RESET!
    if metrics.inserts >= self.config.compaction_threshold_ops {
        return true;  // Always true after first trigger!
    }

    false
}

// Line 1580-1608 - perform_compaction:
pub async fn perform_compaction(&self) -> CoreResult<()> {
    // ... compaction logic ...

    // DOES NOT RESET metrics.inserts!

    Ok(())
}
```

**Impact:**
- `wal_size_bytes` is never updated → byte threshold never triggers
- `metrics.inserts` is a lifetime counter, never reset
- Once `inserts >= threshold`, `should_compact()` is always true
- **Background worker compacts on every iteration (every 1s)**
- **Continuous unnecessary compaction**
- **CPU/disk waste, performance degradation**

**Fix:**
1. Update `wal_size_bytes` on every WAL append
2. Reset `inserts` counter after compaction (or track "since last compaction")

---

### 🟡 Bug #19: Queries Counter Never Incremented (HIGH - Monitoring)

**Location:** `crates/akidb-storage/src/storage_backend.rs:82`, `1250-1318`

**Problem:**
```rust
// Line 82 - StorageMetrics has queries field
pub struct StorageMetrics {
    pub queries: u64,  // NEVER INCREMENTED ANYWHERE!
    // ...
}

// Lines 1250-1318 - get() method:
pub async fn get(&self, doc_id: &DocumentId) -> CoreResult<Option<VectorDocument>> {
    match self.config.tiering_policy {
        TieringPolicy::Memory | TieringPolicy::MemoryS3 => {
            let store = self.vector_store.read();
            Ok(store.get(doc_id).cloned())
            // NO metrics.queries increment!
        }
        TieringPolicy::S3Only => {
            // Check cache first
            if let Some(cache) = &self.vector_cache {
                if let Some(doc) = cache.write().get(doc_id) {
                    return Ok(Some(doc.clone()));
                    // NO metrics.queries increment!
                }
            }

            // Fallback to S3
            // NO metrics.queries increment!
            // ...
        }
    }
}
```

**Impact:**
- Prometheus/Grafana dashboards show 0 queries forever
- **SLO/SLA monitoring broken**
- **Alerting broken** (cannot detect traffic spikes/drops)
- **Capacity planning impossible**
- **Cannot measure QPS**

**Fix:** Increment `metrics.queries` on every successful `get()` call.

---

### 🟡 Bug #20: Zero Vector Search Not Validated (HIGH - NaN Results)

**Location:** `crates/akidb-index/src/instant_hnsw.rs:180-189`, `360-417`

**Problem:**
```rust
// Lines 180-189 - insert rejects zero vectors for Cosine:
pub async fn insert(&self, doc: VectorDocument) -> CoreResult<()> {
    if self.config.metric == DistanceMetric::Cosine {
        let norm: f32 = doc.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm == 0.0 {
            return Err(CoreError::ValidationError(
                "Cannot insert zero vector with Cosine metric".to_string(),
            ));
        }
    }
    // ...
}

// Lines 360-417 - search DOES NOT validate:
pub async fn search(
    &self,
    query: &[f32],
    k: usize,
    _ef: Option<usize>,
) -> CoreResult<Vec<SearchResult>> {
    // Normalize for Cosine
    let query_normalized = if self.config.metric == DistanceMetric::Cosine {
        normalize_vector(query)  // Returns ZERO vector unchanged if norm=0!
    } else {
        query.to_vec()
    };

    // Uses zero vector for search → undefined cosine similarity
    // Produces NaN scores and unstable ranking
    // ...
}

// normalize_vector implementation:
fn normalize_vector(v: &[f32]) -> Vec<f32> {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm == 0.0 {
        return v.to_vec();  // RETURNS ZERO VECTOR UNCHANGED!
    }
    v.iter().map(|x| x / norm).collect()
}
```

**Impact:**
- Zero query vector → `normalize_vector` returns it unchanged
- Cosine similarity undefined (0/0 = NaN)
- **Search results have NaN scores**
- **Ranking unstable/undefined**
- **Client applications crash on NaN**

**Fix:** Validate query vector in `search()` for Cosine metric (mirror insert validation).

---

### 🔴 Bug #21: Deleted Vectors Still in Search Results (CRITICAL)

**Location:** `crates/akidb-index/src/hnsw.rs:640-676`, `689-700`

**Problem:**
```rust
// Lines 640-676 - delete() sets deleted flag:
pub async fn delete(&self, doc_id: DocumentId) -> CoreResult<()> {
    let mut nodes = self.nodes.write().await;

    if let Some(node) = nodes.get_mut(&doc_id) {
        node.deleted = true;  // SOFT DELETE
    } else {
        return Err(CoreError::NotFound(format!("Document {} not found", doc_id)));
    }

    Ok(())
}

// Lines 689-700 - search() does NOT check deleted flag:
pub async fn search(
    &self,
    query: &[f32],
    k: usize,
    ef: Option<usize>,
) -> CoreResult<Vec<SearchResult>> {
    // ... HNSW traversal ...

    // Build results WITHOUT checking deleted flag
    let results: Vec<SearchResult> = candidates.into_iter()
        .take(k)
        .map(|(dist, doc_id)| SearchResult {
            doc_id,
            score: 1.0 - dist,  // INCLUDES DELETED NODES!
        })
        .collect();

    Ok(results)
}
```

**Impact:**
- `delete()` only sets `node.deleted = true`
- `search()` never checks the flag
- **Deleted vectors continue to appear in search results**
- **Data integrity violation**
- **GDPR/compliance violation** (deleted user data still returned)
- **Cannot implement "soft delete with background cleanup"**

**Fix:** Filter out deleted nodes before building search results.

---

## Summary

**Total Bugs:** 21 bugs (all rounds combined)
- Previous rounds: 14 bugs
- **AutomatosX Round 2: 7 bugs**

**Severity Breakdown (Round 2):**
- 4 CRITICAL bugs
- 3 HIGH priority bugs

**Impact:**
- Data loss on restart (Bug #15)
- S3 backup/restore broken (Bug #16)
- WAL replay data loss (Bug #17)
- Data integrity violation (Bug #21)
- Performance degradation (Bug #18)
- Monitoring broken (Bug #19)
- Unstable search results (Bug #20)

**All bugs MUST be fixed before GA release.**

---

**Analysis Complete**
**Next Step:** MEGATHINK to design comprehensive fixes
