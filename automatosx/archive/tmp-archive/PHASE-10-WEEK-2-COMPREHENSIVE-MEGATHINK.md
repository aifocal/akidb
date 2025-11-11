# Phase 10 Week 2: Hot/Warm/Cold Tiering - Comprehensive Megathink

**Date**: 2025-11-09
**Phase**: Phase 10 Week 2
**Focus**: Hot/Warm/Cold Tiering Policies Implementation
**Status**: 🔍 ANALYSIS & PLANNING

---

## Executive Summary

**Objective**: Implement automatic tiered storage for vector collections with hot (RAM), warm (local disk), and cold (S3/MinIO) tiers based on access patterns.

**Business Value**:
- Reduce memory costs by 60-80% for large datasets
- Maintain low latency for frequently accessed vectors
- Automatically archive cold data to S3
- Support datasets >100GB with <100GB RAM

**Technical Approach**:
- LRU-based access tracking at collection level
- Configurable promotion/demotion thresholds
- Background worker for automatic tiering
- Integration with Week 1 Parquet snapshots
- Zero data loss guarantees

**Timeline**: 5 days (Week 2 of Phase 10)

---

## Table of Contents

1. [Background & Context](#1-background--context)
2. [Problem Statement](#2-problem-statement)
3. [Architecture Analysis](#3-architecture-analysis)
4. [Existing Infrastructure Review](#4-existing-infrastructure-review)
5. [Detailed Design](#5-detailed-design)
6. [Implementation Plan](#6-implementation-plan)
7. [Testing Strategy](#7-testing-strategy)
8. [Performance Considerations](#8-performance-considerations)
9. [Risk Analysis](#9-risk-analysis)
10. [Open Questions](#10-open-questions)
11. [Success Criteria](#11-success-criteria)

---

## 1. Background & Context

### 1.1 What is Tiered Storage?

Tiered storage is a data management strategy that automatically moves data between different storage types based on access patterns:

**Hot Tier (RAM)**:
- Fastest access: <1ms latency
- Highest cost: ~$10/GB/month
- Limited capacity: Typically 16-256GB on edge devices
- Use case: Frequently accessed vectors (daily/hourly)

**Warm Tier (Local SSD)**:
- Fast access: 1-10ms latency
- Medium cost: ~$0.20/GB/month
- Medium capacity: 500GB-2TB
- Use case: Occasionally accessed vectors (weekly)

**Cold Tier (S3/MinIO)**:
- Slow access: 100-500ms latency
- Low cost: ~$0.02/GB/month
- Unlimited capacity
- Use case: Rarely accessed vectors (monthly/never)

### 1.2 Why Tiering for AkiDB 2.0?

**Problem**: Target constraint is ≤100GB in-memory datasets, but users may have 500GB-1TB of total vector data.

**Solution**: Keep hot vectors in RAM, warm vectors on disk, cold vectors on S3.

**Example Scenario**:
- Total dataset: 500GB vectors
- Hot tier (RAM): 50GB (10% - accessed daily)
- Warm tier (SSD): 150GB (30% - accessed weekly)
- Cold tier (S3): 300GB (60% - accessed rarely)
- **Result**: System runs in 64GB RAM instead of 512GB

### 1.3 Access Pattern Assumptions

Based on typical vector search workloads:

- **80/20 Rule**: 80% of searches access 20% of collections
- **Temporal Locality**: Recently accessed collections likely to be accessed again
- **Collection-Level Granularity**: Entire collections move between tiers (not individual vectors)

### 1.4 Integration with Week 1

Week 1 delivered **ParquetSnapshotter** which enables:
- Efficient serialization of vector collections to Parquet format
- Upload/download from S3/MinIO
- 2-3x compression vs JSON

Week 2 builds on this by adding **automatic tiering logic**:
- When to snapshot a collection (demotion to cold tier)
- When to restore a collection (promotion to hot tier)
- How to track access patterns

---

## 2. Problem Statement

### 2.1 Core Requirements

**FR-1**: Automatically track collection access patterns (last_accessed_at, access_count)

**FR-2**: Define configurable tiering policies:
- Hot → Warm: No access for X hours (default: 6 hours)
- Warm → Cold: No access for Y days (default: 7 days)
- Cold → Warm: On first access (automatic restore)
- Warm → Hot: Z accesses within W hours (default: 10 accesses in 1 hour)

**FR-3**: Background worker runs periodically (default: every 5 minutes) to:
- Demote hot collections to warm
- Demote warm collections to cold (create snapshot + upload to S3)
- Cleanup local warm storage

**FR-4**: Transparent promotion on search:
- If collection is cold → restore from S3 (blocking, with timeout)
- If collection is warm → load into RAM (fast)
- Track access for future tiering decisions

**FR-5**: Manual tier control via API:
- Force demote collection to cold (for maintenance)
- Force promote collection to hot (for anticipated traffic)
- Pin collection to hot tier (prevent demotion)

### 2.2 Non-Functional Requirements

**NFR-1**: Access tracking overhead <1ms per search operation

**NFR-2**: Background worker does not block search operations

**NFR-3**: Promotion from cold tier completes in <10s for 100k vectors

**NFR-4**: Zero data loss during tier transitions

**NFR-5**: Graceful degradation: If S3 unavailable, keep data in warm tier

### 2.3 Constraints

**C-1**: Collection-level granularity only (no per-document tiering in Week 2)

**C-2**: Single-node deployment (no distributed consensus)

**C-3**: Must work with existing StorageBackend and CollectionService

**C-4**: Backward compatible with collections that don't use tiering

---

## 3. Architecture Analysis

### 3.1 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      CollectionService                          │
│                                                                 │
│  search() → AccessTracker.record_access(collection_id)         │
│                        ↓                                        │
│                  TieringManager                                 │
│                        ↓                                        │
│     ┌──────────────────┼──────────────────┐                   │
│     │                  │                  │                   │
│  Hot Tier          Warm Tier          Cold Tier               │
│  (RAM)             (SSD)              (S3/MinIO)              │
│  - VectorIndex     - Parquet files    - Parquet snapshots     │
│  - In-memory       - Local disk       - Object store          │
│  - <1ms latency    - 1-10ms latency   - 100-500ms latency     │
│     │                  │                  │                   │
│     └──────────────────┴──────────────────┘                   │
│                        ↑                                        │
│               Background Worker                                 │
│               (runs every 5 minutes)                            │
│               - Check access patterns                           │
│               - Demote hot → warm                               │
│               - Demote warm → cold                              │
│               - Cleanup expired data                            │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 Component Responsibilities

**TieringManager**:
- Owns tiering policy configuration
- Decides when to promote/demote collections
- Coordinates with AccessTracker and StorageBackend
- Exposes API for manual tier control

**AccessTracker**:
- Records collection access timestamps
- Maintains LRU metadata (last_accessed_at, access_count)
- Provides access statistics for tiering decisions
- Persists access metadata to SQLite

**TierState** (per collection):
- Current tier: Hot | Warm | Cold
- Last accessed timestamp
- Access count (in current window)
- Pinned flag (prevents demotion)
- Snapshot metadata (if in cold tier)

**Background Worker**:
- Tokio task running on interval
- Scans all collections for tier transitions
- Executes demotions (snapshot → upload → cleanup)
- Logs all tier changes for observability

### 3.3 State Transitions

```
                    ┌─────────┐
                    │   Hot   │
                    │  (RAM)  │
                    └────┬────┘
                         │
        ┌────────────────┼────────────────┐
        │ (demote)       │      (promote) │
        │ No access      │      10 access │
        │ for 6h         │      in 1h     │
        ↓                ↑                │
   ┌─────────┐      ┌────────┐           │
   │  Warm   │      │  Cold  │           │
   │  (SSD)  │      │  (S3)  │           │
   └────┬────┘      └───┬────┘           │
        │               │                │
        │ (demote)      │ (promote)      │
        │ No access     │ On first       │
        │ for 7d        │ access         │
        └───────────────┴────────────────┘
```

**State Transition Rules**:

1. **Hot → Warm** (demotion):
   - Condition: `now - last_accessed_at > hot_tier_ttl` (default 6h)
   - Action: Serialize VectorIndex to Parquet → Save to local disk → Drop from RAM
   - Reversible: Yes (promote back quickly from disk)

2. **Warm → Cold** (demotion):
   - Condition: `now - last_accessed_at > warm_tier_ttl` (default 7d)
   - Action: Create snapshot → Upload to S3 → Delete local file
   - Reversible: Yes (restore from S3, slower)

3. **Cold → Warm** (promotion):
   - Condition: Collection accessed while in cold tier
   - Action: Download snapshot from S3 → Save to local disk → Update state
   - Blocking: Yes (search waits for download)

4. **Warm → Hot** (promotion):
   - Condition: `access_count_in_window >= hot_promotion_threshold` (default 10 in 1h)
   - Action: Load Parquet from disk → Deserialize to VectorIndex → Load into RAM
   - Blocking: No (can search from warm tier while promoting)

5. **Pinned State** (special):
   - Condition: User pins collection via API
   - Action: Collection stays in hot tier, no demotion
   - Use case: Critical collections, anticipated traffic spike

### 3.4 Data Flow Examples

**Example 1: Search on Hot Collection**
```
1. User: search(collection_id, query_vector)
2. CollectionService: Load collection from hot tier (RAM)
3. AccessTracker: record_access(collection_id, timestamp)
4. VectorIndex: search(query_vector) → results
5. Return results (total time: <5ms)
```

**Example 2: Search on Cold Collection (First Access)**
```
1. User: search(collection_id, query_vector)
2. CollectionService: Check tier state → COLD
3. TieringManager: promote_from_cold(collection_id)
   a. Download snapshot from S3 (2-5s for 100k vectors)
   b. Deserialize Parquet to VectorIndex
   c. Load into RAM
   d. Update tier state → HOT
4. AccessTracker: record_access(collection_id, timestamp)
5. VectorIndex: search(query_vector) → results
6. Return results (total time: 2-5s for first search, <5ms after)
```

**Example 3: Background Demotion**
```
1. Background worker wakes up (every 5 minutes)
2. TieringManager: scan_for_demotions()
   a. Query all hot collections
   b. Filter: last_accessed_at > 6h ago
   c. For each collection:
      - Serialize VectorIndex to Parquet
      - Save to local disk (warm tier)
      - Drop from RAM
      - Update tier state → WARM
3. TieringManager: scan_for_cold_demotions()
   a. Query all warm collections
   b. Filter: last_accessed_at > 7d ago
   c. For each collection:
      - Create snapshot (reuse Parquet file)
      - Upload to S3 via ParquetSnapshotter
      - Delete local file
      - Update tier state → COLD
4. Log all transitions for observability
```

---

## 4. Existing Infrastructure Review

### 4.1 StorageBackend

**File**: `crates/akidb-storage/src/storage_backend.rs` (~2000 lines)

**Current Responsibilities**:
- Collection persistence (vector documents)
- WAL integration (durability)
- Snapshot management (via JsonSnapshotter)
- ObjectStore integration (S3/Local)

**Integration Point for Tiering**:
```rust
pub struct StorageBackend {
    wal: Arc<dyn WriteAheadLog>,
    object_store: Arc<dyn ObjectStore>,
    snapshotter: Arc<dyn Snapshotter>,  // Will use ParquetSnapshotter from Week 1
    // NEW: Add tiering manager
    tiering_manager: Option<Arc<TieringManager>>,
}

impl StorageBackend {
    // Existing methods
    pub async fn persist_collection(&self, ...) -> CoreResult<()>;
    pub async fn load_collection(&self, ...) -> CoreResult<Collection>;

    // NEW: Tiering hooks
    pub async fn record_access(&self, collection_id: CollectionId) -> CoreResult<()> {
        if let Some(tm) = &self.tiering_manager {
            tm.record_access(collection_id).await?;
        }
        Ok(())
    }

    pub async fn get_tier_state(&self, collection_id: CollectionId) -> CoreResult<TierState> {
        self.tiering_manager
            .as_ref()
            .ok_or(CoreError::ConfigError("Tiering not enabled".into()))?
            .get_tier_state(collection_id)
            .await
    }
}
```

**Changes Needed**:
1. Add `TieringManager` field (optional for backward compatibility)
2. Add `record_access()` hook in `load_collection()`
3. Add tier state checks in `load_collection()` (promote if needed)
4. Add config option for tiering policies

### 4.2 ParquetSnapshotter (Week 1)

**File**: `crates/akidb-storage/src/snapshotter/parquet.rs` (to be created in Week 1)

**API** (from Week 1 design):
```rust
#[async_trait]
pub trait Snapshotter: Send + Sync {
    async fn create_snapshot(
        &self,
        collection_id: CollectionId,
        vectors: Vec<VectorDocument>,
    ) -> CoreResult<SnapshotId>;

    async fn restore_snapshot(
        &self,
        collection_id: CollectionId,
        snapshot_id: SnapshotId,
    ) -> CoreResult<Vec<VectorDocument>>;
}
```

**Usage in Tiering**:
- **Warm → Cold**: Call `create_snapshot()` to upload to S3
- **Cold → Warm**: Call `restore_snapshot()` to download from S3

**Assumptions**:
- ParquetSnapshotter is fully functional by end of Week 1
- Snapshot creation takes <2s for 10k vectors (Week 1 target)
- Snapshot restore takes <3s for 10k vectors (Week 1 target)

### 4.3 ObjectStore

**File**: `crates/akidb-storage/src/object_store.rs`

**API**:
```rust
#[async_trait]
pub trait ObjectStore: Send + Sync {
    async fn put(&self, key: &str, data: Bytes) -> CoreResult<()>;
    async fn get(&self, key: &str) -> CoreResult<Bytes>;
    async fn delete(&self, key: &str) -> CoreResult<()>;
    async fn list(&self, prefix: &str) -> CoreResult<Vec<String>>;
}
```

**Tiering Usage**:
- Wrapped by ParquetSnapshotter, no direct interaction needed
- Already supports S3 and LocalObjectStore

### 4.4 SQLite Metadata

**File**: `crates/akidb-metadata/src/lib.rs`

**Existing Tables**:
- `tenants`
- `databases`
- `collections` (has `created_at`, `updated_at`)

**New Table Needed**: `collection_tier_state`

```sql
CREATE TABLE collection_tier_state (
    collection_id BLOB PRIMARY KEY REFERENCES collections(collection_id) ON DELETE CASCADE,
    tier TEXT NOT NULL CHECK(tier IN ('hot','warm','cold')),
    last_accessed_at TEXT NOT NULL,
    access_count INTEGER NOT NULL DEFAULT 0,
    access_window_start TEXT NOT NULL,
    pinned INTEGER NOT NULL DEFAULT 0,  -- 0 = false, 1 = true
    snapshot_id BLOB,  -- NULL if not in cold tier
    warm_file_path TEXT,  -- NULL if not in warm tier
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE INDEX ix_tier_state_tier ON collection_tier_state(tier);
CREATE INDEX ix_tier_state_last_accessed ON collection_tier_state(last_accessed_at);
```

**Why This Table?**:
- Persists tier state across restarts
- Enables background worker queries (e.g., "find all warm collections not accessed in 7 days")
- Stores access statistics (last_accessed_at, access_count)
- Tracks snapshot metadata for cold collections

### 4.5 CollectionService

**File**: `crates/akidb-service/src/collection_service.rs`

**Current Responsibilities**:
- Collection CRUD operations
- Vector search (delegates to VectorIndex)
- Integration with StorageBackend

**Integration Point**:
```rust
impl CollectionService {
    pub async fn search(
        &self,
        collection_id: CollectionId,
        query_vector: Vec<f32>,
        k: usize,
    ) -> CoreResult<Vec<SearchResult>> {
        // NEW: Record access
        self.storage_backend.record_access(collection_id).await?;

        // NEW: Check tier state, promote if needed
        let tier_state = self.storage_backend.get_tier_state(collection_id).await?;
        if tier_state.tier == Tier::Cold {
            self.storage_backend.promote_from_cold(collection_id).await?;
        }

        // Existing: Load collection and search
        let collection = self.storage_backend.load_collection(collection_id).await?;
        collection.index.search(&query_vector, k)
    }
}
```

**Changes Needed**:
1. Add `record_access()` call in `search()` method
2. Add tier state check and promotion logic
3. Handle cold tier promotion latency (may take 2-10s)

---

## 5. Detailed Design

### 5.1 Core Data Structures

#### TieringManager

```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc, Duration};

pub struct TieringManager {
    /// Access tracker for LRU
    access_tracker: Arc<AccessTracker>,

    /// Tiering policy configuration
    policy: TieringPolicy,

    /// Storage backend for snapshot operations
    storage: Arc<StorageBackend>,

    /// Metadata repository for tier state persistence
    metadata: Arc<TierStateRepository>,

    /// Background worker handle
    worker: Option<tokio::task::JoinHandle<()>>,
}

impl TieringManager {
    pub fn new(
        policy: TieringPolicy,
        storage: Arc<StorageBackend>,
        metadata: Arc<TierStateRepository>,
    ) -> Self {
        Self {
            access_tracker: Arc::new(AccessTracker::new()),
            policy,
            storage,
            metadata,
            worker: None,
        }
    }

    /// Start background worker
    pub fn start_worker(&mut self) {
        let manager = Arc::new(self.clone());
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                Duration::from_secs(manager.policy.worker_interval_secs)
            );
            loop {
                interval.tick().await;
                if let Err(e) = manager.run_tiering_cycle().await {
                    tracing::error!(error = %e, "Tiering cycle failed");
                }
            }
        });
        self.worker = Some(handle);
    }

    /// Record collection access
    pub async fn record_access(&self, collection_id: CollectionId) -> CoreResult<()> {
        self.access_tracker.record(collection_id).await?;
        self.metadata.update_access_time(collection_id, Utc::now()).await
    }

    /// Get current tier state
    pub async fn get_tier_state(&self, collection_id: CollectionId) -> CoreResult<TierState> {
        self.metadata.get_tier_state(collection_id).await
    }

    /// Promote collection from cold to warm
    pub async fn promote_from_cold(&self, collection_id: CollectionId) -> CoreResult<()> {
        let state = self.metadata.get_tier_state(collection_id).await?;
        if state.tier != Tier::Cold {
            return Ok(()); // Already promoted
        }

        let snapshot_id = state.snapshot_id
            .ok_or(CoreError::InvalidState("Cold collection missing snapshot ID".into()))?;

        // Download from S3 and restore
        let vectors = self.storage.snapshotter.restore_snapshot(collection_id, snapshot_id).await?;

        // Save to warm tier (local disk)
        let warm_path = format!("warm/{}.parquet", collection_id);
        self.storage.save_to_warm_tier(&warm_path, &vectors).await?;

        // Update tier state
        self.metadata.update_tier_state(collection_id, Tier::Warm, Some(warm_path), None).await
    }

    /// Promote collection from warm to hot
    pub async fn promote_from_warm(&self, collection_id: CollectionId) -> CoreResult<()> {
        let state = self.metadata.get_tier_state(collection_id).await?;
        if state.tier != Tier::Warm {
            return Ok(());
        }

        let warm_path = state.warm_file_path
            .ok_or(CoreError::InvalidState("Warm collection missing file path".into()))?;

        // Load from local disk
        let vectors = self.storage.load_from_warm_tier(&warm_path).await?;

        // Load into RAM (VectorIndex)
        self.storage.load_collection_into_memory(collection_id, vectors).await?;

        // Update tier state
        self.metadata.update_tier_state(collection_id, Tier::Hot, None, None).await
    }

    /// Demote collection from hot to warm
    async fn demote_to_warm(&self, collection_id: CollectionId) -> CoreResult<()> {
        // Serialize collection to Parquet
        let vectors = self.storage.get_collection_vectors(collection_id).await?;
        let warm_path = format!("warm/{}.parquet", collection_id);
        self.storage.save_to_warm_tier(&warm_path, &vectors).await?;

        // Drop from RAM
        self.storage.unload_collection(collection_id).await?;

        // Update tier state
        self.metadata.update_tier_state(collection_id, Tier::Warm, Some(warm_path), None).await
    }

    /// Demote collection from warm to cold
    async fn demote_to_cold(&self, collection_id: CollectionId) -> CoreResult<()> {
        let state = self.metadata.get_tier_state(collection_id).await?;
        let warm_path = state.warm_file_path
            .ok_or(CoreError::InvalidState("Warm collection missing file path".into()))?;

        // Load vectors from warm tier
        let vectors = self.storage.load_from_warm_tier(&warm_path).await?;

        // Create snapshot and upload to S3
        let snapshot_id = self.storage.snapshotter.create_snapshot(collection_id, vectors).await?;

        // Delete warm file
        self.storage.delete_warm_file(&warm_path).await?;

        // Update tier state
        self.metadata.update_tier_state(collection_id, Tier::Cold, None, Some(snapshot_id)).await
    }

    /// Background worker: scan and execute tier transitions
    async fn run_tiering_cycle(&self) -> CoreResult<()> {
        tracing::info!("Starting tiering cycle");

        // Demote hot → warm (no access for hot_tier_ttl)
        let hot_cutoff = Utc::now() - Duration::hours(self.policy.hot_tier_ttl_hours);
        let hot_candidates = self.metadata.find_hot_collections_idle_since(hot_cutoff).await?;
        for collection_id in hot_candidates {
            tracing::info!(collection_id = %collection_id, "Demoting hot → warm");
            self.demote_to_warm(collection_id).await?;
        }

        // Demote warm → cold (no access for warm_tier_ttl)
        let warm_cutoff = Utc::now() - Duration::days(self.policy.warm_tier_ttl_days);
        let warm_candidates = self.metadata.find_warm_collections_idle_since(warm_cutoff).await?;
        for collection_id in warm_candidates {
            tracing::info!(collection_id = %collection_id, "Demoting warm → cold");
            self.demote_to_cold(collection_id).await?;
        }

        // Promote warm → hot (high access frequency)
        let access_window_start = Utc::now() - Duration::hours(self.policy.access_window_hours);
        let warm_hot_candidates = self.metadata
            .find_warm_collections_with_high_access(
                access_window_start,
                self.policy.hot_promotion_threshold
            )
            .await?;
        for collection_id in warm_hot_candidates {
            tracing::info!(collection_id = %collection_id, "Promoting warm → hot");
            self.promote_from_warm(collection_id).await?;
        }

        tracing::info!("Tiering cycle complete");
        Ok(())
    }
}
```

#### TieringPolicy

```rust
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct TieringPolicy {
    /// Hours without access before demoting hot → warm (default: 6)
    pub hot_tier_ttl_hours: i64,

    /// Days without access before demoting warm → cold (default: 7)
    pub warm_tier_ttl_days: i64,

    /// Access count threshold for promoting warm → hot (default: 10)
    pub hot_promotion_threshold: u32,

    /// Access window for promotion (default: 1 hour)
    pub access_window_hours: i64,

    /// Background worker interval (default: 300 seconds = 5 minutes)
    pub worker_interval_secs: u64,
}

impl Default for TieringPolicy {
    fn default() -> Self {
        Self {
            hot_tier_ttl_hours: 6,
            warm_tier_ttl_days: 7,
            hot_promotion_threshold: 10,
            access_window_hours: 1,
            worker_interval_secs: 300,
        }
    }
}
```

#### TierState

```rust
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct TierState {
    pub collection_id: CollectionId,
    pub tier: Tier,
    pub last_accessed_at: DateTime<Utc>,
    pub access_count: u32,
    pub access_window_start: DateTime<Utc>,
    pub pinned: bool,
    pub snapshot_id: Option<SnapshotId>,
    pub warm_file_path: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum Tier {
    Hot,   // In RAM
    Warm,  // On local disk
    Cold,  // On S3/MinIO
}

impl Tier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Tier::Hot => "hot",
            Tier::Warm => "warm",
            Tier::Cold => "cold",
        }
    }
}

impl std::str::FromStr for Tier {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "hot" => Ok(Tier::Hot),
            "warm" => Ok(Tier::Warm),
            "cold" => Ok(Tier::Cold),
            _ => Err(CoreError::ValidationError(format!("Invalid tier: {}", s))),
        }
    }
}
```

#### AccessTracker

```rust
use std::collections::HashMap;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

pub struct AccessTracker {
    /// In-memory cache of access stats
    cache: RwLock<HashMap<CollectionId, AccessStats>>,
}

#[derive(Debug, Clone)]
struct AccessStats {
    last_accessed_at: DateTime<Utc>,
    access_count: u32,
    window_start: DateTime<Utc>,
}

impl AccessTracker {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Record a collection access
    pub async fn record(&self, collection_id: CollectionId) -> CoreResult<()> {
        let mut cache = self.cache.write().await;
        let now = Utc::now();

        cache.entry(collection_id)
            .and_modify(|stats| {
                stats.last_accessed_at = now;
                stats.access_count += 1;
            })
            .or_insert_with(|| AccessStats {
                last_accessed_at: now,
                access_count: 1,
                window_start: now,
            });

        Ok(())
    }

    /// Get access stats
    pub async fn get_stats(&self, collection_id: CollectionId) -> Option<AccessStats> {
        let cache = self.cache.read().await;
        cache.get(&collection_id).cloned()
    }

    /// Reset access window (called by background worker)
    pub async fn reset_window(&self, collection_id: CollectionId) -> CoreResult<()> {
        let mut cache = self.cache.write().await;
        if let Some(stats) = cache.get_mut(&collection_id) {
            stats.access_count = 0;
            stats.window_start = Utc::now();
        }
        Ok(())
    }
}
```

#### TierStateRepository

```rust
use sqlx::{SqlitePool, Row};

pub struct TierStateRepository {
    pool: SqlitePool,
}

impl TierStateRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Initialize tier state for new collection (default: Hot)
    pub async fn init_tier_state(&self, collection_id: CollectionId) -> CoreResult<()> {
        let now = Utc::now();
        let collection_id_bytes = collection_id.as_bytes();

        sqlx::query!(
            r#"
            INSERT INTO collection_tier_state (
                collection_id, tier, last_accessed_at, access_count,
                access_window_start, pinned, created_at, updated_at
            ) VALUES (?1, 'hot', ?2, 0, ?2, 0, ?2, ?2)
            "#,
            collection_id_bytes,
            now,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Get tier state
    pub async fn get_tier_state(&self, collection_id: CollectionId) -> CoreResult<TierState> {
        let collection_id_bytes = collection_id.as_bytes();

        let row = sqlx::query!(
            r#"
            SELECT
                collection_id, tier, last_accessed_at, access_count,
                access_window_start, pinned, snapshot_id, warm_file_path,
                created_at, updated_at
            FROM collection_tier_state
            WHERE collection_id = ?1
            "#,
            collection_id_bytes
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| CoreError::NotFound(format!("Tier state not found: {}", e)))?;

        Ok(TierState {
            collection_id: CollectionId::from_bytes(row.collection_id.as_slice())?,
            tier: row.tier.parse()?,
            last_accessed_at: row.last_accessed_at,
            access_count: row.access_count as u32,
            access_window_start: row.access_window_start,
            pinned: row.pinned != 0,
            snapshot_id: row.snapshot_id.map(|bytes| SnapshotId::from_bytes(&bytes)).transpose()?,
            warm_file_path: row.warm_file_path,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    /// Update access time
    pub async fn update_access_time(
        &self,
        collection_id: CollectionId,
        accessed_at: DateTime<Utc>,
    ) -> CoreResult<()> {
        let collection_id_bytes = collection_id.as_bytes();

        sqlx::query!(
            r#"
            UPDATE collection_tier_state
            SET last_accessed_at = ?2,
                access_count = access_count + 1,
                updated_at = ?2
            WHERE collection_id = ?1
            "#,
            collection_id_bytes,
            accessed_at,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Update tier state
    pub async fn update_tier_state(
        &self,
        collection_id: CollectionId,
        tier: Tier,
        warm_file_path: Option<String>,
        snapshot_id: Option<SnapshotId>,
    ) -> CoreResult<()> {
        let collection_id_bytes = collection_id.as_bytes();
        let tier_str = tier.as_str();
        let snapshot_id_bytes = snapshot_id.map(|id| id.as_bytes().to_vec());
        let now = Utc::now();

        sqlx::query!(
            r#"
            UPDATE collection_tier_state
            SET tier = ?2,
                warm_file_path = ?3,
                snapshot_id = ?4,
                updated_at = ?5
            WHERE collection_id = ?1
            "#,
            collection_id_bytes,
            tier_str,
            warm_file_path,
            snapshot_id_bytes,
            now,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Find hot collections idle since cutoff
    pub async fn find_hot_collections_idle_since(
        &self,
        cutoff: DateTime<Utc>,
    ) -> CoreResult<Vec<CollectionId>> {
        let rows = sqlx::query!(
            r#"
            SELECT collection_id
            FROM collection_tier_state
            WHERE tier = 'hot'
              AND last_accessed_at < ?1
              AND pinned = 0
            "#,
            cutoff
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::DatabaseError(e.to_string()))?;

        rows.into_iter()
            .map(|row| CollectionId::from_bytes(&row.collection_id))
            .collect()
    }

    /// Find warm collections idle since cutoff
    pub async fn find_warm_collections_idle_since(
        &self,
        cutoff: DateTime<Utc>,
    ) -> CoreResult<Vec<CollectionId>> {
        let rows = sqlx::query!(
            r#"
            SELECT collection_id
            FROM collection_tier_state
            WHERE tier = 'warm'
              AND last_accessed_at < ?1
              AND pinned = 0
            "#,
            cutoff
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::DatabaseError(e.to_string()))?;

        rows.into_iter()
            .map(|row| CollectionId::from_bytes(&row.collection_id))
            .collect()
    }

    /// Find warm collections with high access frequency
    pub async fn find_warm_collections_with_high_access(
        &self,
        window_start: DateTime<Utc>,
        threshold: u32,
    ) -> CoreResult<Vec<CollectionId>> {
        let rows = sqlx::query!(
            r#"
            SELECT collection_id
            FROM collection_tier_state
            WHERE tier = 'warm'
              AND access_window_start >= ?1
              AND access_count >= ?2
            "#,
            window_start,
            threshold
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::DatabaseError(e.to_string()))?;

        rows.into_iter()
            .map(|row| CollectionId::from_bytes(&row.collection_id))
            .collect()
    }
}
```

### 5.2 Configuration

Add to `akidb-service/src/config.rs`:

```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Config {
    // ... existing fields

    /// Tiering configuration (optional, disabled by default)
    pub tiering: Option<TieringConfig>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TieringConfig {
    /// Enable tiering
    pub enabled: bool,

    /// Tiering policy
    pub policy: TieringPolicy,

    /// Warm tier storage path (local disk)
    pub warm_storage_path: String,
}

impl Default for TieringConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            policy: TieringPolicy::default(),
            warm_storage_path: "./data/warm".to_string(),
        }
    }
}
```

Example TOML:
```toml
[tiering]
enabled = true
warm_storage_path = "./data/warm"

[tiering.policy]
hot_tier_ttl_hours = 6
warm_tier_ttl_days = 7
hot_promotion_threshold = 10
access_window_hours = 1
worker_interval_secs = 300
```

### 5.3 API Extensions

Add to `akidb-rest/src/handlers/collection_handler.rs`:

```rust
/// Get tier state for collection
pub async fn get_tier_state(
    State(service): State<Arc<CollectionService>>,
    Path(collection_id): Path<String>,
) -> Result<Json<TierState>, AppError> {
    let collection_id = CollectionId::parse(&collection_id)?;
    let state = service.storage_backend.get_tier_state(collection_id).await?;
    Ok(Json(state))
}

/// Pin collection to hot tier (prevent demotion)
pub async fn pin_collection(
    State(service): State<Arc<CollectionService>>,
    Path(collection_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let collection_id = CollectionId::parse(&collection_id)?;
    service.storage_backend.pin_collection(collection_id).await?;
    Ok(StatusCode::OK)
}

/// Unpin collection (allow demotion)
pub async fn unpin_collection(
    State(service): State<Arc<CollectionService>>,
    Path(collection_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let collection_id = CollectionId::parse(&collection_id)?;
    service.storage_backend.unpin_collection(collection_id).await?;
    Ok(StatusCode::OK)
}

/// Force promote collection to hot tier
pub async fn force_promote(
    State(service): State<Arc<CollectionService>>,
    Path(collection_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let collection_id = CollectionId::parse(&collection_id)?;
    service.storage_backend.force_promote_to_hot(collection_id).await?;
    Ok(StatusCode::OK)
}

/// Force demote collection to cold tier
pub async fn force_demote(
    State(service): State<Arc<CollectionService>>,
    Path(collection_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let collection_id = CollectionId::parse(&collection_id)?;
    service.storage_backend.force_demote_to_cold(collection_id).await?;
    Ok(StatusCode::OK)
}
```

Routes:
```rust
Router::new()
    .route("/collections/:id/tier", get(get_tier_state))
    .route("/collections/:id/tier/pin", post(pin_collection))
    .route("/collections/:id/tier/unpin", post(unpin_collection))
    .route("/collections/:id/tier/promote", post(force_promote))
    .route("/collections/:id/tier/demote", post(force_demote))
```

---

## 6. Implementation Plan

### Day 1: Access Tracking Infrastructure (3-4 hours)

**Morning Session** (2 hours):
1. Create migration for `collection_tier_state` table
2. Implement `TierState` and `Tier` enums
3. Implement `TierStateRepository` skeleton
4. Write 3 unit tests (init, get, update)

**Afternoon Session** (1-2 hours):
5. Implement `AccessTracker` struct
6. Add `record()` and `get_stats()` methods
7. Write 2 tests (record access, concurrent access)

**Deliverable**: Access tracking infrastructure complete, 5 tests passing

---

### Day 2: Tiering Manager Core (3-4 hours)

**Morning Session** (2 hours):
1. Implement `TieringPolicy` struct with defaults
2. Implement `TieringManager` skeleton
3. Add `record_access()` and `get_tier_state()` methods
4. Write 2 tests (record, get state)

**Afternoon Session** (1-2 hours):
5. Implement `promote_from_cold()` method
6. Implement `promote_from_warm()` method
7. Write 2 tests (promote from cold, promote from warm)

**Deliverable**: Promotion logic complete, 9 tests passing (5 + 4 new)

---

### Day 3: Demotion Logic (3-4 hours)

**Morning Session** (2 hours):
1. Implement `demote_to_warm()` method
2. Implement `demote_to_cold()` method
3. Add warm tier file I/O helpers
4. Write 2 tests (demote to warm, demote to cold)

**Afternoon Session** (1-2 hours):
5. Implement `find_hot_collections_idle_since()` query
6. Implement `find_warm_collections_idle_since()` query
7. Write 2 tests (query hot, query warm)

**Deliverable**: Demotion logic complete, 13 tests passing (9 + 4 new)

---

### Day 4: Background Worker (3-4 hours)

**Morning Session** (2 hours):
1. Implement `run_tiering_cycle()` method
2. Implement `start_worker()` with Tokio interval
3. Add worker shutdown logic
4. Write 1 test (manual cycle)

**Afternoon Session** (1-2 hours):
5. Implement `find_warm_collections_with_high_access()` query
6. Add automatic warm → hot promotion in cycle
7. Write 2 tests (automatic promotion, full cycle)

**Deliverable**: Background worker complete, 16 tests passing (13 + 3 new)

---

### Day 5: Integration & Polish (4-5 hours)

**Morning Session** (2-3 hours):
1. Integrate TieringManager into StorageBackend
2. Add tiering config to `config.toml`
3. Hook into CollectionService.search()
4. Write 3 E2E tests (search cold, search warm, automatic demotion)

**Afternoon Session** (2 hours):
5. Add REST API endpoints (pin, unpin, force promote/demote)
6. Write API tests (4 tests)
7. Update documentation (README, config guide)
8. Final smoke test

**Deliverable**: Full integration complete, 23 tests passing (16 + 7 new), docs updated

---

## 7. Testing Strategy

### 7.1 Unit Tests (6 tests)

**TierStateRepository**:
1. `test_init_tier_state` - Initialize new collection as hot
2. `test_get_tier_state` - Retrieve tier state
3. `test_update_tier_state` - Update tier (hot → warm)

**AccessTracker**:
4. `test_record_access` - Record single access
5. `test_concurrent_access` - Record 100 concurrent accesses
6. `test_reset_window` - Reset access counter

### 7.2 Integration Tests (12 tests)

**Promotion**:
1. `test_promote_from_cold` - Cold → Warm (download from S3)
2. `test_promote_from_warm` - Warm → Hot (load from disk)
3. `test_promote_cold_to_hot` - Cold → Hot (direct)

**Demotion**:
4. `test_demote_to_warm` - Hot → Warm (save to disk)
5. `test_demote_to_cold` - Warm → Cold (upload to S3)
6. `test_demote_hot_to_cold` - Hot → Cold (direct)

**Queries**:
7. `test_find_hot_idle` - Query hot collections idle >6h
8. `test_find_warm_idle` - Query warm collections idle >7d
9. `test_find_warm_high_access` - Query warm collections with >10 accesses

**Background Worker**:
10. `test_manual_tiering_cycle` - Run cycle manually
11. `test_automatic_demotion` - Worker demotes idle hot collection
12. `test_automatic_promotion` - Worker promotes high-access warm collection

### 7.3 E2E Tests (4 tests)

1. `test_search_cold_collection` - Search on cold collection triggers restore
2. `test_search_warm_collection` - Search on warm collection loads from disk
3. `test_full_tier_lifecycle` - Hot → Warm → Cold → Warm → Hot
4. `test_pinned_collection` - Pinned collection never demoted

### 7.4 API Tests (4 tests)

1. `test_get_tier_state_api` - GET /collections/:id/tier
2. `test_pin_collection_api` - POST /collections/:id/tier/pin
3. `test_force_promote_api` - POST /collections/:id/tier/promote
4. `test_force_demote_api` - POST /collections/:id/tier/demote

**Total Week 2 Tests**: 26 tests

---

## 8. Performance Considerations

### 8.1 Access Tracking Overhead

**Target**: <1ms per search operation

**Implementation**:
- AccessTracker uses in-memory HashMap (no I/O)
- RwLock for concurrent access (read-heavy workload)
- Async update to SQLite (non-blocking)

**Benchmark**:
```rust
#[bench]
fn bench_record_access(b: &mut Bencher) {
    let tracker = AccessTracker::new();
    let collection_id = CollectionId::new();

    b.iter(|| {
        tracker.record(collection_id).await
    });
}
// Target: <100µs (0.1ms)
```

### 8.2 Background Worker Impact

**Considerations**:
- Worker runs every 5 minutes (configurable)
- Should not block search operations
- S3 uploads can take 2-10s for large collections

**Design**:
- Worker runs in separate Tokio task
- No locks held during S3 operations
- Collections being demoted remain searchable

### 8.3 Promotion Latency

**Cold → Warm** (S3 download):
- 10k vectors (512-dim): 2-5s
- 100k vectors: 10-30s
- **Mitigation**: Show loading indicator, timeout after 60s

**Warm → Hot** (disk load):
- 10k vectors: 100-500ms
- 100k vectors: 1-3s
- **Mitigation**: Can search from warm tier while promoting (optional optimization)

### 8.4 Memory Usage

**Metadata Overhead**:
- TierState per collection: ~200 bytes
- 10,000 collections: ~2MB
- AccessTracker cache: ~100 bytes per active collection

**Warm Tier Disk Usage**:
- Parquet files: 2-3x compressed
- 100 collections × 100k vectors × 512-dim × 4 bytes = 20GB raw → 7-10GB compressed

---

## 9. Risk Analysis

### 9.1 High-Risk Areas

**Risk 1: S3 Download Latency Impacts UX**
- **Likelihood**: High
- **Impact**: High (users see 5-30s delay on first search)
- **Mitigation**:
  - Add timeout (60s)
  - Show clear loading message
  - Pre-warm critical collections via API
  - Consider async promotion (return partial results from cache)

**Risk 2: Background Worker Consumes Too Much RAM**
- **Likelihood**: Medium
- **Impact**: Medium (OOM on edge devices)
- **Mitigation**:
  - Demote collections one at a time (not in parallel)
  - Add memory threshold config (pause if RAM >80%)
  - Graceful degradation (skip cycle if system under load)

**Risk 3: Concurrent Promotion/Demotion Race Condition**
- **Likelihood**: Medium
- **Impact**: High (data corruption or inconsistent state)
- **Mitigation**:
  - Use collection-level locks during tier transitions
  - Atomic state updates in SQLite (transaction)
  - Idempotent operations (safe to retry)

**Risk 4: Warm Tier Disk Full**
- **Likelihood**: Medium
- **Impact**: Medium (demotions fail)
- **Mitigation**:
  - Monitor disk space (add metric)
  - Fail gracefully (keep in hot tier if warm unavailable)
  - Add config for max warm tier size

### 9.2 Medium-Risk Areas

**Risk 5: Background Worker Crashes**
- **Mitigation**: Tokio task panic handling, restart automatically

**Risk 6: SQLite Contention on Tier State Updates**
- **Mitigation**: Batch updates, use WAL mode, async writes

---

## 10. Open Questions

### Q1: Should we support per-document tiering?

**Answer**: Not in Week 2 (defer to future enhancement)

**Rationale**: Collection-level tiering is simpler and covers 80% of use cases. Per-document tiering adds significant complexity (need document-level access tracking, partial snapshots).

**Decision**: Week 2 = collection-level only

---

### Q2: What LRU implementation should we use?

**Options**:
1. **Custom HashMap + RwLock** (proposed)
   - Pros: Simple, no dependencies
   - Cons: Manual implementation

2. **lru crate** (https://crates.io/crates/lru)
   - Pros: Battle-tested, efficient
   - Cons: Requires Mutex (not RwLock), extra dependency

**Decision**: Start with custom HashMap (simpler), migrate to `lru` crate if performance issues

---

### Q3: Should warm → hot promotion be automatic or manual?

**Answer**: Automatic based on access frequency

**Rationale**: Manual promotion requires user intervention. Automatic promotion provides better UX.

**Default Rule**: If warm collection accessed ≥10 times in 1 hour → promote to hot

**Override**: User can pin collection to hot tier manually via API

---

### Q4: How to handle S3 failures during demotion?

**Options**:
1. **Fail and retry** (proposed)
   - Keep collection in current tier
   - Log error, retry on next cycle

2. **Keep local copy as backup**
   - Upload to S3 but don't delete warm file
   - Delete after successful upload confirmed

**Decision**: Option 2 (safer, prevents data loss)

**Implementation**: Add `cold_tier_backup_enabled` config (default: true)

---

### Q5: Should we support "hot only" collections?

**Answer**: Yes, via pinned flag

**Use Case**: Critical collections that must always be in RAM (e.g., authentication vectors)

**Implementation**: Set `pinned = true` in tier state → never demoted by background worker

---

## 11. Success Criteria

### 11.1 Functional Requirements

- ✅ Access tracking implemented and working (<1ms overhead)
- ✅ Tier promotion/demotion logic correct (0 data loss)
- ✅ Background worker runs automatically (every 5 minutes)
- ✅ REST API for manual tier control (pin, promote, demote)
- ✅ Configuration via TOML (tiering policies)

### 11.2 Performance Requirements

- ✅ Access tracking: <1ms per search
- ✅ Cold → Warm: <10s for 100k vectors (S3 download)
- ✅ Warm → Hot: <3s for 100k vectors (disk load)
- ✅ Hot → Warm: <2s for 100k vectors (disk save)
- ✅ Background worker: <30s per cycle (typical workload)

### 11.3 Quality Requirements

- ✅ 26 tests passing (6 unit + 12 integration + 4 E2E + 4 API)
- ✅ Zero data corruption (roundtrip integrity tests)
- ✅ Clean error handling (no panics)
- ✅ Code coverage >80%
- ✅ Documentation complete (config guide, API reference)

---

## 12. Next Steps

After Week 2 completion:

**Week 3: Integration Testing + RC2 Release**
- E2E tests for full tiering workflow
- Performance benchmarks (meet all targets)
- Crash recovery tests (restart with collections in different tiers)
- Documentation (S3 setup, tiering tuning guide)
- Tag `v2.0.0-rc2` release

**Week 4: Performance Optimization**
- Batch S3 uploads (>500 ops/sec)
- Parallel S3 uploads (>600 ops/sec)
- Mock S3 for testing
- 15 E2E tests

**Week 5-6: Observability + Kubernetes + GA**
- Prometheus metrics (12 metrics)
- Grafana dashboards (4 dashboards)
- Kubernetes Helm chart
- Chaos tests
- GA release `v2.0.0`

---

## Appendix A: File Structure

```
crates/akidb-storage/src/
├── tiering/
│   ├── mod.rs                  # Public API, re-exports
│   ├── manager.rs              # TieringManager implementation
│   ├── policy.rs               # TieringPolicy configuration
│   ├── state.rs                # TierState, Tier enum
│   ├── tracker.rs              # AccessTracker implementation
│   └── repository.rs           # TierStateRepository (SQLite)
├── snapshotter/
│   ├── mod.rs
│   ├── json.rs                 # Existing
│   └── parquet.rs              # Week 1 deliverable
└── storage_backend.rs          # Integration point

crates/akidb-storage/tests/
└── tiering_tests.rs            # 26 tests

crates/akidb-metadata/migrations/
└── 006_collection_tier_state.sql   # New migration

crates/akidb-rest/src/handlers/
└── tier_handler.rs             # New: Tier control API
```

---

## Appendix B: Configuration Example

**config.toml**:
```toml
[storage]
snapshotter_type = "parquet"  # From Week 1

[tiering]
enabled = true
warm_storage_path = "./data/warm"

[tiering.policy]
# Hot tier TTL: no access for 6 hours → demote to warm
hot_tier_ttl_hours = 6

# Warm tier TTL: no access for 7 days → demote to cold
warm_tier_ttl_days = 7

# Warm → Hot promotion: 10 accesses in 1 hour
hot_promotion_threshold = 10
access_window_hours = 1

# Background worker interval: 5 minutes
worker_interval_secs = 300

[storage.s3]
endpoint = "https://s3.amazonaws.com"
bucket = "akidb-cold-tier"
region = "us-west-2"
access_key_id = "AKIAIOSFODNN7EXAMPLE"
secret_access_key = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
```

---

## Appendix C: Metrics to Add (Week 5)

```
# Tier distribution
akidb_tier_collections_total{tier="hot"} 50
akidb_tier_collections_total{tier="warm"} 30
akidb_tier_collections_total{tier="cold"} 20

# Tier transitions
akidb_tier_promotions_total{from="cold",to="warm"} 12
akidb_tier_demotions_total{from="hot",to="warm"} 8

# Access tracking
akidb_tier_access_tracking_duration_seconds{quantile="0.95"} 0.0008

# Worker cycles
akidb_tier_worker_cycles_total 120
akidb_tier_worker_duration_seconds{quantile="0.95"} 15.3
```

---

**Status**: ✅ MEGATHINK COMPLETE - READY FOR PRD CREATION

**Next**: Create detailed PRD document for Phase 10 Week 2
