# Phase 10 Week 2: Daily Action Plan - Hot/Warm/Cold Tiering

**Timeline**: 5 days (Week 2 of Phase 10)
**Goal**: Implement automatic tiering policies with hot/warm/cold tiers
**Target**: 26 tests passing, <1ms access tracking overhead

---

## Day 1: Access Tracking Infrastructure

**Goal**: Build foundation for access tracking and tier state persistence
**Time**: 3-4 hours
**Tests**: 5 tests passing by EOD

### Morning Session (2 hours)

#### Task 1.1: Create SQLite Migration (30 min)

**File**: `crates/akidb-metadata/migrations/006_collection_tier_state.sql`

```sql
-- Migration: Collection Tier State
-- Created: 2025-11-09

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

-- Trigger to auto-update updated_at
CREATE TRIGGER update_tier_state_timestamp
AFTER UPDATE ON collection_tier_state
FOR EACH ROW
BEGIN
    UPDATE collection_tier_state
    SET updated_at = CURRENT_TIMESTAMP
    WHERE collection_id = NEW.collection_id;
END;
```

**Run Migration**:
```bash
cd crates/akidb-metadata
cargo sqlx migrate run
cargo sqlx prepare --workspace
```

#### Task 1.2: Implement Core Data Structures (45 min)

**File**: `crates/akidb-storage/src/tiering/state.rs`

```rust
use chrono::{DateTime, Utc};
use akidb_core::{CollectionId, SnapshotId, CoreError, CoreResult};
use std::str::FromStr;

/// Tier level for a collection
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum Tier {
    /// Hot tier: in RAM, <1ms latency
    Hot,
    /// Warm tier: on local disk, 1-10ms latency
    Warm,
    /// Cold tier: on S3/MinIO, 100-500ms latency
    Cold,
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

impl FromStr for Tier {
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

/// Complete tier state for a collection
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

impl TierState {
    /// Create new tier state (default: Hot)
    pub fn new(collection_id: CollectionId) -> Self {
        let now = Utc::now();
        Self {
            collection_id,
            tier: Tier::Hot,
            last_accessed_at: now,
            access_count: 0,
            access_window_start: now,
            pinned: false,
            snapshot_id: None,
            warm_file_path: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Check if collection is hot
    pub fn is_hot(&self) -> bool {
        self.tier == Tier::Hot
    }

    /// Check if collection is warm
    pub fn is_warm(&self) -> bool {
        self.tier == Tier::Warm
    }

    /// Check if collection is cold
    pub fn is_cold(&self) -> bool {
        self.tier == Tier::Cold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_as_str() {
        assert_eq!(Tier::Hot.as_str(), "hot");
        assert_eq!(Tier::Warm.as_str(), "warm");
        assert_eq!(Tier::Cold.as_str(), "cold");
    }

    #[test]
    fn test_tier_from_str() {
        assert_eq!("hot".parse::<Tier>().unwrap(), Tier::Hot);
        assert_eq!("warm".parse::<Tier>().unwrap(), Tier::Warm);
        assert_eq!("cold".parse::<Tier>().unwrap(), Tier::Cold);
        assert!("invalid".parse::<Tier>().is_err());
    }

    #[test]
    fn test_tier_state_new() {
        let collection_id = CollectionId::new();
        let state = TierState::new(collection_id);

        assert_eq!(state.collection_id, collection_id);
        assert_eq!(state.tier, Tier::Hot);
        assert_eq!(state.access_count, 0);
        assert!(!state.pinned);
        assert!(state.snapshot_id.is_none());
        assert!(state.warm_file_path.is_none());
    }
}
```

#### Task 1.3: Implement TierStateRepository (45 min)

**File**: `crates/akidb-metadata/src/tier_state_repository.rs`

```rust
use akidb_core::{CollectionId, SnapshotId, CoreError, CoreResult};
use crate::tiering::{TierState, Tier};
use chrono::{DateTime, Utc};
use sqlx::{SqlitePool, Row};

/// Repository for tier state persistence
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
            snapshot_id: row.snapshot_id
                .as_ref()
                .map(|bytes| SnapshotId::from_bytes(bytes))
                .transpose()?,
            warm_file_path: row.warm_file_path,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    /// Update access time and increment counter
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
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_db() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn test_init_tier_state() {
        let pool = setup_db().await;
        let repo = TierStateRepository::new(pool);
        let collection_id = CollectionId::new();

        repo.init_tier_state(collection_id).await.unwrap();

        let state = repo.get_tier_state(collection_id).await.unwrap();
        assert_eq!(state.tier, Tier::Hot);
        assert_eq!(state.access_count, 0);
        assert!(!state.pinned);
    }

    #[tokio::test]
    async fn test_update_access_time() {
        let pool = setup_db().await;
        let repo = TierStateRepository::new(pool);
        let collection_id = CollectionId::new();

        repo.init_tier_state(collection_id).await.unwrap();

        let before = repo.get_tier_state(collection_id).await.unwrap();
        assert_eq!(before.access_count, 0);

        repo.update_access_time(collection_id, Utc::now()).await.unwrap();

        let after = repo.get_tier_state(collection_id).await.unwrap();
        assert_eq!(after.access_count, 1);
    }
}
```

**Checkpoint**: Run tests
```bash
cargo test -p akidb-metadata tier_state
```

### Afternoon Session (1-2 hours)

#### Task 1.4: Implement AccessTracker (1 hour)

**File**: `crates/akidb-storage/src/tiering/tracker.rs`

```rust
use akidb_core::{CollectionId, CoreResult};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Access statistics for a collection
#[derive(Debug, Clone)]
pub struct AccessStats {
    pub last_accessed_at: DateTime<Utc>,
    pub access_count: u32,
    pub window_start: DateTime<Utc>,
}

/// In-memory access tracker (LRU cache)
pub struct AccessTracker {
    cache: RwLock<HashMap<CollectionId, AccessStats>>,
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

    /// Clear all stats (for testing)
    #[cfg(test)]
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_record_access() {
        let tracker = AccessTracker::new();
        let collection_id = CollectionId::new();

        tracker.record(collection_id).await.unwrap();

        let stats = tracker.get_stats(collection_id).await.unwrap();
        assert_eq!(stats.access_count, 1);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let tracker = Arc::new(AccessTracker::new());
        let collection_id = CollectionId::new();

        let mut handles = vec![];
        for _ in 0..100 {
            let tracker = Arc::clone(&tracker);
            let handle = tokio::spawn(async move {
                tracker.record(collection_id).await.unwrap();
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap();
        }

        let stats = tracker.get_stats(collection_id).await.unwrap();
        assert_eq!(stats.access_count, 100);
    }
}
```

#### Task 1.5: Create Module Structure (30 min)

**File**: `crates/akidb-storage/src/tiering/mod.rs`

```rust
//! Hot/Warm/Cold Tiering Policies
//!
//! This module implements automatic tiering of vector collections based on access patterns:
//! - **Hot Tier** (RAM): Frequently accessed collections, <1ms latency
//! - **Warm Tier** (Local Disk): Occasionally accessed collections, 1-10ms latency
//! - **Cold Tier** (S3/MinIO): Rarely accessed collections, 100-500ms latency
//!
//! ## Example
//!
//! ```rust
//! use akidb_storage::tiering::{TieringManager, TieringPolicy};
//!
//! let policy = TieringPolicy::default();
//! let manager = TieringManager::new(policy, storage, metadata);
//! manager.start_worker();
//! ```

mod state;
mod tracker;
mod policy;
mod repository;
mod manager;

pub use state::{Tier, TierState};
pub use tracker::{AccessTracker, AccessStats};
pub use policy::TieringPolicy;
pub use repository::TierStateRepository;
pub use manager::TieringManager;
```

**Update**: `crates/akidb-storage/src/lib.rs`
```rust
pub mod tiering;
```

### End of Day 1 Checkpoint

**Tests Passing**: 5 tests
- `test_tier_as_str`
- `test_tier_from_str`
- `test_tier_state_new`
- `test_init_tier_state`
- `test_update_access_time`
- `test_record_access`
- `test_concurrent_access`

**Code Metrics**: ~300 lines
**Status**: ✅ Access tracking infrastructure complete

---

## Day 2: Tiering Manager Core

**Goal**: Implement TieringManager with promotion logic
**Time**: 3-4 hours
**Tests**: 9 tests passing by EOD (5 + 4 new)

### Morning Session (2 hours)

#### Task 2.1: Implement TieringPolicy (30 min)

**File**: `crates/akidb-storage/src/tiering/policy.rs`

```rust
use serde::{Deserialize, Serialize};

/// Tiering policy configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TieringPolicy {
    /// Hours without access before demoting hot → warm (default: 6)
    pub hot_tier_ttl_hours: i64,

    /// Days without access before demoting warm → cold (default: 7)
    pub warm_tier_ttl_days: i64,

    /// Access count threshold for promoting warm → hot (default: 10)
    pub hot_promotion_threshold: u32,

    /// Access window for promotion (default: 1 hour)
    pub access_window_hours: i64,

    /// Background worker interval in seconds (default: 300 = 5 minutes)
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

impl TieringPolicy {
    /// Validate policy configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.hot_tier_ttl_hours < 1 {
            return Err("hot_tier_ttl_hours must be >= 1".into());
        }
        if self.warm_tier_ttl_days < 1 {
            return Err("warm_tier_ttl_days must be >= 1".into());
        }
        if self.hot_promotion_threshold < 1 {
            return Err("hot_promotion_threshold must be >= 1".into());
        }
        if self.worker_interval_secs < 60 {
            return Err("worker_interval_secs must be >= 60".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy() {
        let policy = TieringPolicy::default();
        assert_eq!(policy.hot_tier_ttl_hours, 6);
        assert_eq!(policy.warm_tier_ttl_days, 7);
        assert_eq!(policy.hot_promotion_threshold, 10);
        assert!(policy.validate().is_ok());
    }

    #[test]
    fn test_invalid_policy() {
        let mut policy = TieringPolicy::default();
        policy.hot_tier_ttl_hours = 0;
        assert!(policy.validate().is_err());
    }
}
```

#### Task 2.2: Implement TieringManager Skeleton (1 hour)

**File**: `crates/akidb-storage/src/tiering/manager.rs`

```rust
use super::{TieringPolicy, AccessTracker, TierState, Tier, TierStateRepository};
use akidb_core::{CollectionId, SnapshotId, CoreResult, CoreError};
use std::sync::Arc;
use chrono::{DateTime, Utc, Duration};

/// Tiering manager for hot/warm/cold tier transitions
pub struct TieringManager {
    access_tracker: Arc<AccessTracker>,
    policy: TieringPolicy,
    metadata: Arc<TierStateRepository>,
    worker: Option<tokio::task::JoinHandle<()>>,
}

impl TieringManager {
    pub fn new(
        policy: TieringPolicy,
        metadata: Arc<TierStateRepository>,
    ) -> CoreResult<Self> {
        policy.validate()
            .map_err(|e| CoreError::ValidationError(e))?;

        Ok(Self {
            access_tracker: Arc::new(AccessTracker::new()),
            policy,
            metadata,
            worker: None,
        })
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

        tracing::info!(
            collection_id = %collection_id,
            snapshot_id = %snapshot_id,
            "Promoting from cold to warm"
        );

        // TODO: Download from S3 and save to warm tier
        // This will be implemented when we integrate with StorageBackend

        let warm_path = format!("warm/{}.parquet", collection_id);
        self.metadata.update_tier_state(
            collection_id,
            Tier::Warm,
            Some(warm_path),
            None
        ).await
    }

    /// Promote collection from warm to hot
    pub async fn promote_from_warm(&self, collection_id: CollectionId) -> CoreResult<()> {
        let state = self.metadata.get_tier_state(collection_id).await?;
        if state.tier != Tier::Warm {
            return Ok(());
        }

        tracing::info!(
            collection_id = %collection_id,
            "Promoting from warm to hot"
        );

        // TODO: Load from warm tier into RAM
        // This will be implemented when we integrate with StorageBackend

        self.metadata.update_tier_state(
            collection_id,
            Tier::Hot,
            None,
            None
        ).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> (TieringManager, SqlitePool) {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("../akidb-metadata/migrations").run(&pool).await.unwrap();

        let repo = Arc::new(TierStateRepository::new(pool.clone()));
        let policy = TieringPolicy::default();
        let manager = TieringManager::new(policy, repo).unwrap();

        (manager, pool)
    }

    #[tokio::test]
    async fn test_record_access() {
        let (manager, _pool) = setup().await;
        let collection_id = CollectionId::new();

        manager.metadata.init_tier_state(collection_id).await.unwrap();
        manager.record_access(collection_id).await.unwrap();

        let state = manager.get_tier_state(collection_id).await.unwrap();
        assert_eq!(state.access_count, 1);
    }

    #[tokio::test]
    async fn test_get_tier_state() {
        let (manager, _pool) = setup().await;
        let collection_id = CollectionId::new();

        manager.metadata.init_tier_state(collection_id).await.unwrap();

        let state = manager.get_tier_state(collection_id).await.unwrap();
        assert_eq!(state.tier, Tier::Hot);
    }
}
```

### Afternoon Session (1-2 hours)

#### Task 2.3: Add Promotion Tests (1 hour)

**File**: `crates/akidb-storage/tests/tiering_tests.rs`

```rust
use akidb_storage::tiering::{TieringManager, TieringPolicy, Tier, TierStateRepository};
use akidb_core::{CollectionId, SnapshotId};
use sqlx::SqlitePool;
use std::sync::Arc;

async fn setup_manager() -> (TieringManager, SqlitePool) {
    let pool = SqlitePool::connect(":memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let repo = Arc::new(TierStateRepository::new(pool.clone()));
    let policy = TieringPolicy::default();
    let manager = TieringManager::new(policy, repo).unwrap();

    (manager, pool)
}

#[tokio::test]
async fn test_promote_from_warm() {
    let (manager, _pool) = setup_manager().await;
    let collection_id = CollectionId::new();

    // Initialize as warm tier
    manager.metadata.init_tier_state(collection_id).await.unwrap();
    let warm_path = format!("warm/{}.parquet", collection_id);
    manager.metadata.update_tier_state(
        collection_id,
        Tier::Warm,
        Some(warm_path.clone()),
        None
    ).await.unwrap();

    // Promote to hot
    manager.promote_from_warm(collection_id).await.unwrap();

    let state = manager.get_tier_state(collection_id).await.unwrap();
    assert_eq!(state.tier, Tier::Hot);
    assert!(state.warm_file_path.is_none());
}

#[tokio::test]
async fn test_promote_from_cold() {
    let (manager, _pool) = setup_manager().await;
    let collection_id = CollectionId::new();
    let snapshot_id = SnapshotId::new();

    // Initialize as cold tier
    manager.metadata.init_tier_state(collection_id).await.unwrap();
    manager.metadata.update_tier_state(
        collection_id,
        Tier::Cold,
        None,
        Some(snapshot_id)
    ).await.unwrap();

    // Promote to warm
    manager.promote_from_cold(collection_id).await.unwrap();

    let state = manager.get_tier_state(collection_id).await.unwrap();
    assert_eq!(state.tier, Tier::Warm);
    assert!(state.warm_file_path.is_some());
}
```

### End of Day 2 Checkpoint

**Tests Passing**: 9 tests
- Day 1: 5 tests
- Day 2: 4 new tests (record_access, get_tier_state, promote_from_warm, promote_from_cold)

**Code Metrics**: ~600 lines total
**Status**: ✅ Tiering manager core + promotion logic complete

---

## Day 3: Demotion Logic

**Goal**: Implement demotion methods and query helpers
**Time**: 3-4 hours
**Tests**: 13 tests passing by EOD (9 + 4 new)

### Morning Session (2 hours)

#### Task 3.1: Implement Demotion Methods (1.5 hours)

**File**: `crates/akidb-storage/src/tiering/manager.rs` (add to existing)

```rust
impl TieringManager {
    // ... existing methods

    /// Demote collection from hot to warm
    async fn demote_to_warm(&self, collection_id: CollectionId) -> CoreResult<()> {
        let state = self.metadata.get_tier_state(collection_id).await?;
        if state.tier != Tier::Hot {
            return Ok(());
        }

        if state.pinned {
            tracing::debug!(
                collection_id = %collection_id,
                "Skipping demotion: collection is pinned"
            );
            return Ok(());
        }

        tracing::info!(
            collection_id = %collection_id,
            "Demoting from hot to warm"
        );

        // TODO: Serialize collection to Parquet and save to warm tier
        // This requires integration with StorageBackend

        let warm_path = format!("warm/{}.parquet", collection_id);
        self.metadata.update_tier_state(
            collection_id,
            Tier::Warm,
            Some(warm_path),
            None
        ).await
    }

    /// Demote collection from warm to cold
    async fn demote_to_cold(&self, collection_id: CollectionId) -> CoreResult<()> {
        let state = self.metadata.get_tier_state(collection_id).await?;
        if state.tier != Tier::Warm {
            return Ok(());
        }

        if state.pinned {
            tracing::debug!(
                collection_id = %collection_id,
                "Skipping demotion: collection is pinned"
            );
            return Ok(());
        }

        let warm_path = state.warm_file_path
            .ok_or(CoreError::InvalidState("Warm collection missing file path".into()))?;

        tracing::info!(
            collection_id = %collection_id,
            warm_path = %warm_path,
            "Demoting from warm to cold"
        );

        // TODO: Create snapshot and upload to S3
        // This requires integration with ParquetSnapshotter from Week 1

        let snapshot_id = SnapshotId::new();  // Placeholder
        self.metadata.update_tier_state(
            collection_id,
            Tier::Cold,
            None,
            Some(snapshot_id)
        ).await
    }
}
```

#### Task 3.2: Implement Query Methods (30 min)

**File**: `crates/akidb-metadata/src/tier_state_repository.rs` (add to existing)

```rust
impl TierStateRepository {
    // ... existing methods

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

### Afternoon Session (1-2 hours)

#### Task 3.3: Add Demotion Tests (1 hour)

**File**: `crates/akidb-storage/tests/tiering_tests.rs` (add to existing)

```rust
#[tokio::test]
async fn test_demote_to_warm() {
    let (manager, _pool) = setup_manager().await;
    let collection_id = CollectionId::new();

    // Initialize as hot tier
    manager.metadata.init_tier_state(collection_id).await.unwrap();

    // Demote to warm
    manager.demote_to_warm(collection_id).await.unwrap();

    let state = manager.get_tier_state(collection_id).await.unwrap();
    assert_eq!(state.tier, Tier::Warm);
    assert!(state.warm_file_path.is_some());
}

#[tokio::test]
async fn test_demote_to_cold() {
    let (manager, _pool) = setup_manager().await;
    let collection_id = CollectionId::new();

    // Initialize as warm tier
    manager.metadata.init_tier_state(collection_id).await.unwrap();
    let warm_path = format!("warm/{}.parquet", collection_id);
    manager.metadata.update_tier_state(
        collection_id,
        Tier::Warm,
        Some(warm_path),
        None
    ).await.unwrap();

    // Demote to cold
    manager.demote_to_cold(collection_id).await.unwrap();

    let state = manager.get_tier_state(collection_id).await.unwrap();
    assert_eq!(state.tier, Tier::Cold);
    assert!(state.snapshot_id.is_some());
}

#[tokio::test]
async fn test_find_hot_idle() {
    let (manager, _pool) = setup_manager().await;
    let collection_id = CollectionId::new();

    manager.metadata.init_tier_state(collection_id).await.unwrap();

    // Simulate old access time
    let old_time = Utc::now() - Duration::hours(12);
    manager.metadata.update_access_time(collection_id, old_time).await.unwrap();

    let cutoff = Utc::now() - Duration::hours(6);
    let idle = manager.metadata.find_hot_collections_idle_since(cutoff).await.unwrap();

    assert!(idle.contains(&collection_id));
}

#[tokio::test]
async fn test_find_warm_high_access() {
    let (manager, _pool) = setup_manager().await;
    let collection_id = CollectionId::new();

    // Initialize as warm
    manager.metadata.init_tier_state(collection_id).await.unwrap();
    manager.metadata.update_tier_state(
        collection_id,
        Tier::Warm,
        Some("warm/test.parquet".into()),
        None
    ).await.unwrap();

    // Simulate 15 accesses
    for _ in 0..15 {
        manager.record_access(collection_id).await.unwrap();
    }

    let window_start = Utc::now() - Duration::hours(1);
    let candidates = manager.metadata
        .find_warm_collections_with_high_access(window_start, 10)
        .await
        .unwrap();

    assert!(candidates.contains(&collection_id));
}
```

### End of Day 3 Checkpoint

**Tests Passing**: 13 tests
- Day 1-2: 9 tests
- Day 3: 4 new tests (demote_to_warm, demote_to_cold, find_hot_idle, find_warm_high_access)

**Code Metrics**: ~900 lines total
**Status**: ✅ Demotion logic + query methods complete

---

## Day 4: Background Worker

**Goal**: Implement background worker for automatic tier transitions
**Time**: 3-4 hours
**Tests**: 16 tests passing by EOD (13 + 3 new)

### Morning Session (2 hours)

#### Task 4.1: Implement Tiering Cycle (1.5 hours)

**File**: `crates/akidb-storage/src/tiering/manager.rs` (add to existing)

```rust
use tokio::time::{interval, Duration as TokioDuration};

impl TieringManager {
    // ... existing methods

    /// Start background worker
    pub fn start_worker(&mut self) {
        let manager = Arc::new(self.clone());
        let interval_secs = self.policy.worker_interval_secs;

        let handle = tokio::spawn(async move {
            let mut ticker = interval(TokioDuration::from_secs(interval_secs));

            loop {
                ticker.tick().await;

                if let Err(e) = manager.run_tiering_cycle().await {
                    tracing::error!(error = %e, "Tiering cycle failed");
                }
            }
        });

        self.worker = Some(handle);
    }

    /// Run one tiering cycle (demotions and promotions)
    async fn run_tiering_cycle(&self) -> CoreResult<()> {
        tracing::info!("Starting tiering cycle");
        let start = std::time::Instant::now();

        // Demote hot → warm (no access for hot_tier_ttl_hours)
        let hot_cutoff = Utc::now() - Duration::hours(self.policy.hot_tier_ttl_hours);
        let hot_candidates = self.metadata
            .find_hot_collections_idle_since(hot_cutoff)
            .await?;

        for collection_id in hot_candidates {
            tracing::info!(collection_id = %collection_id, "Demoting hot → warm");
            if let Err(e) = self.demote_to_warm(collection_id).await {
                tracing::error!(
                    collection_id = %collection_id,
                    error = %e,
                    "Failed to demote hot → warm"
                );
            }
        }

        // Demote warm → cold (no access for warm_tier_ttl_days)
        let warm_cutoff = Utc::now() - Duration::days(self.policy.warm_tier_ttl_days);
        let warm_candidates = self.metadata
            .find_warm_collections_idle_since(warm_cutoff)
            .await?;

        for collection_id in warm_candidates {
            tracing::info!(collection_id = %collection_id, "Demoting warm → cold");
            if let Err(e) = self.demote_to_cold(collection_id).await {
                tracing::error!(
                    collection_id = %collection_id,
                    error = %e,
                    "Failed to demote warm → cold"
                );
            }
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
            if let Err(e) = self.promote_from_warm(collection_id).await {
                tracing::error!(
                    collection_id = %collection_id,
                    error = %e,
                    "Failed to promote warm → hot"
                );
            }
            // Reset access window after promotion
            self.access_tracker.reset_window(collection_id).await?;
        }

        let duration = start.elapsed();
        tracing::info!(duration_ms = duration.as_millis(), "Tiering cycle complete");

        Ok(())
    }

    /// Shutdown background worker
    pub async fn shutdown(&mut self) -> CoreResult<()> {
        if let Some(handle) = self.worker.take() {
            handle.abort();
            tracing::info!("Background worker shut down");
        }
        Ok(())
    }
}

// Make TieringManager cloneable for Arc sharing
impl Clone for TieringManager {
    fn clone(&self) -> Self {
        Self {
            access_tracker: Arc::clone(&self.access_tracker),
            policy: self.policy.clone(),
            metadata: Arc::clone(&self.metadata),
            worker: None,  // Don't clone worker handle
        }
    }
}
```

### Afternoon Session (1-2 hours)

#### Task 4.2: Add Worker Tests (1 hour)

**File**: `crates/akidb-storage/tests/tiering_tests.rs` (add to existing)

```rust
#[tokio::test]
async fn test_manual_tiering_cycle() {
    let (mut manager, _pool) = setup_manager().await;
    let collection_id = CollectionId::new();

    // Initialize as hot with old access time
    manager.metadata.init_tier_state(collection_id).await.unwrap();
    let old_time = Utc::now() - Duration::hours(12);
    manager.metadata.update_access_time(collection_id, old_time).await.unwrap();

    // Run manual cycle
    manager.run_tiering_cycle().await.unwrap();

    // Should be demoted to warm
    let state = manager.get_tier_state(collection_id).await.unwrap();
    assert_eq!(state.tier, Tier::Warm);
}

#[tokio::test]
async fn test_automatic_demotion() {
    let (mut manager, _pool) = setup_manager().await;

    // Set short TTL for testing
    manager.policy.hot_tier_ttl_hours = 0;  // Demote immediately
    manager.policy.worker_interval_secs = 1;  // Run every 1 second

    let collection_id = CollectionId::new();
    manager.metadata.init_tier_state(collection_id).await.unwrap();

    // Start worker
    manager.start_worker();

    // Wait for worker to run
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Should be demoted
    let state = manager.get_tier_state(collection_id).await.unwrap();
    assert_eq!(state.tier, Tier::Warm);

    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_automatic_promotion() {
    let (mut manager, _pool) = setup_manager().await;
    let collection_id = CollectionId::new();

    // Initialize as warm with high access count
    manager.metadata.init_tier_state(collection_id).await.unwrap();
    manager.metadata.update_tier_state(
        collection_id,
        Tier::Warm,
        Some("warm/test.parquet".into()),
        None
    ).await.unwrap();

    // Simulate 15 accesses
    for _ in 0..15 {
        manager.record_access(collection_id).await.unwrap();
    }

    // Run cycle
    manager.run_tiering_cycle().await.unwrap();

    // Should be promoted to hot
    let state = manager.get_tier_state(collection_id).await.unwrap();
    assert_eq!(state.tier, Tier::Hot);
}
```

### End of Day 4 Checkpoint

**Tests Passing**: 16 tests
- Day 1-3: 13 tests
- Day 4: 3 new tests (manual_cycle, automatic_demotion, automatic_promotion)

**Code Metrics**: ~1,200 lines total
**Status**: ✅ Background worker complete

---

## Day 5: Integration & Polish

**Goal**: Full integration with StorageBackend and REST API
**Time**: 4-5 hours
**Tests**: 26 tests passing by EOD (16 + 10 new)

### Morning Session (2-3 hours)

#### Task 5.1: StorageBackend Integration (1.5 hours)

**File**: `crates/akidb-storage/src/storage_backend.rs` (modify existing)

```rust
use crate::tiering::{TieringManager, TieringPolicy};

pub struct StorageBackend {
    // ... existing fields
    tiering_manager: Option<Arc<TieringManager>>,
}

impl StorageBackend {
    pub fn new(config: StorageBackendConfig) -> CoreResult<Self> {
        // ... existing initialization

        let tiering_manager = if let Some(tiering_config) = config.tiering {
            if tiering_config.enabled {
                let repo = Arc::new(TierStateRepository::new(pool.clone()));
                let mut manager = TieringManager::new(tiering_config.policy, repo)?;
                manager.start_worker();
                Some(Arc::new(manager))
            } else {
                None
            }
        } else {
            None
        };

        Ok(Self {
            // ... existing fields
            tiering_manager,
        })
    }

    /// Record collection access (hook for tiering)
    pub async fn record_access(&self, collection_id: CollectionId) -> CoreResult<()> {
        if let Some(tm) = &self.tiering_manager {
            tm.record_access(collection_id).await?;
        }
        Ok(())
    }

    /// Get tier state
    pub async fn get_tier_state(&self, collection_id: CollectionId) -> CoreResult<TierState> {
        self.tiering_manager
            .as_ref()
            .ok_or(CoreError::ConfigError("Tiering not enabled".into()))?
            .get_tier_state(collection_id)
            .await
    }

    /// Pin collection to hot tier
    pub async fn pin_collection(&self, collection_id: CollectionId) -> CoreResult<()> {
        self.tiering_manager
            .as_ref()
            .ok_or(CoreError::ConfigError("Tiering not enabled".into()))?
            .pin_collection(collection_id)
            .await
    }

    // ... more tier control methods
}
```

#### Task 5.2: CollectionService Integration (30 min)

**File**: `crates/akidb-service/src/collection_service.rs` (modify existing)

```rust
impl CollectionService {
    pub async fn search(
        &self,
        collection_id: CollectionId,
        query_vector: Vec<f32>,
        k: usize,
    ) -> CoreResult<Vec<SearchResult>> {
        // Record access for tiering
        self.storage_backend.record_access(collection_id).await?;

        // Check tier state and promote if needed
        if let Ok(tier_state) = self.storage_backend.get_tier_state(collection_id).await {
            if tier_state.tier == Tier::Cold {
                tracing::info!(
                    collection_id = %collection_id,
                    "Promoting cold collection on first access"
                );
                self.storage_backend.promote_from_cold(collection_id).await?;
            }
        }

        // Normal search logic
        let collection = self.storage_backend.load_collection(collection_id).await?;
        collection.index.search(&query_vector, k)
    }
}
```

#### Task 5.3: Configuration (30 min)

**File**: `crates/akidb-service/src/config.rs` (modify existing)

```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Config {
    // ... existing fields
    pub tiering: Option<TieringConfig>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TieringConfig {
    pub enabled: bool,
    pub warm_storage_path: String,
    pub policy: TieringPolicy,
}

impl Default for TieringConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            warm_storage_path: "./data/warm".to_string(),
            policy: TieringPolicy::default(),
        }
    }
}
```

**File**: `config.example.toml` (add section)

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

### Afternoon Session (2 hours)

#### Task 5.4: REST API Endpoints (1 hour)

**File**: `crates/akidb-rest/src/handlers/tier_handler.rs` (new file)

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use akidb_core::CollectionId;
use akidb_service::CollectionService;
use akidb_storage::tiering::TierState;
use std::sync::Arc;

use crate::error::AppError;

/// GET /collections/:id/tier
pub async fn get_tier_state(
    State(service): State<Arc<CollectionService>>,
    Path(collection_id): Path<String>,
) -> Result<Json<TierState>, AppError> {
    let collection_id = CollectionId::parse(&collection_id)?;
    let state = service.storage_backend.get_tier_state(collection_id).await?;
    Ok(Json(state))
}

/// POST /collections/:id/tier/pin
pub async fn pin_collection(
    State(service): State<Arc<CollectionService>>,
    Path(collection_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let collection_id = CollectionId::parse(&collection_id)?;
    service.storage_backend.pin_collection(collection_id).await?;
    Ok(StatusCode::OK)
}

/// POST /collections/:id/tier/unpin
pub async fn unpin_collection(
    State(service): State<Arc<CollectionService>>,
    Path(collection_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let collection_id = CollectionId::parse(&collection_id)?;
    service.storage_backend.unpin_collection(collection_id).await?;
    Ok(StatusCode::OK)
}

/// POST /collections/:id/tier/promote
pub async fn force_promote(
    State(service): State<Arc<CollectionService>>,
    Path(collection_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let collection_id = CollectionId::parse(&collection_id)?;
    service.storage_backend.force_promote_to_hot(collection_id).await?;
    Ok(StatusCode::OK)
}

/// POST /collections/:id/tier/demote
pub async fn force_demote(
    State(service): State<Arc<CollectionService>>,
    Path(collection_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let collection_id = CollectionId::parse(&collection_id)?;
    service.storage_backend.force_demote_to_cold(collection_id).await?;
    Ok(StatusCode::OK)
}
```

**File**: `crates/akidb-rest/src/main.rs` (add routes)

```rust
use crate::handlers::tier_handler;

let app = Router::new()
    // ... existing routes
    .route("/collections/:id/tier", get(tier_handler::get_tier_state))
    .route("/collections/:id/tier/pin", post(tier_handler::pin_collection))
    .route("/collections/:id/tier/unpin", post(tier_handler::unpin_collection))
    .route("/collections/:id/tier/promote", post(tier_handler::force_promote))
    .route("/collections/:id/tier/demote", post(tier_handler::force_demote));
```

#### Task 5.5: E2E and API Tests (1 hour)

**File**: `crates/akidb-rest/tests/tier_api_tests.rs` (new file)

```rust
use akidb_rest::create_app;
use axum_test::TestServer;

#[tokio::test]
async fn test_get_tier_state_api() {
    let app = create_app().await;
    let server = TestServer::new(app).unwrap();

    let response = server
        .get(&format!("/collections/{}/tier", collection_id))
        .await;

    assert_eq!(response.status_code(), 200);
    let state: TierState = response.json();
    assert_eq!(state.tier, Tier::Hot);
}

#[tokio::test]
async fn test_pin_collection_api() {
    let app = create_app().await;
    let server = TestServer::new(app).unwrap();

    let response = server
        .post(&format!("/collections/{}/tier/pin", collection_id))
        .await;

    assert_eq!(response.status_code(), 200);
}

// ... 2 more API tests (force_promote, force_demote)
```

**File**: `crates/akidb-storage/tests/tiering_e2e_tests.rs` (new file)

```rust
#[tokio::test]
async fn test_search_cold_collection() {
    // Initialize collection as cold
    // Search → should trigger automatic restore from S3
    // Verify collection is now warm/hot
}

#[tokio::test]
async fn test_full_tier_lifecycle() {
    // Hot → (no access 6h) → Warm
    // Warm → (no access 7d) → Cold
    // Cold → (search) → Warm
    // Warm → (10 accesses in 1h) → Hot
}

// ... 2 more E2E tests
```

#### Task 5.6: Documentation (30 min)

**File**: `docs/TIERING-GUIDE.md` (new file)

```markdown
# Hot/Warm/Cold Tiering Guide

## Overview

AkiDB 2.0 supports automatic tiering of vector collections based on access patterns...

## Configuration

... (full guide with examples)

## API Reference

... (tier control endpoints)

## Troubleshooting

... (common issues)
```

**Update**: `README.md`, `CLAUDE.md` with tiering status

### End of Day 5 Checkpoint

**Tests Passing**: 26 tests
- Day 1-4: 16 tests
- Day 5: 10 new tests (4 API + 4 E2E + 2 integration)

**Code Metrics**: ~1,800 lines total (including tests, docs)
**Status**: ✅ Full integration complete, ready for Week 3

---

## Week 2 Summary

**Deliverables**:
- ✅ TieringManager with automatic tier transitions
- ✅ Access tracking infrastructure (<1ms overhead)
- ✅ Background worker (runs every 5 minutes)
- ✅ Manual tier control API (pin, promote, demote)
- ✅ 26 tests passing (100% pass rate)
- ✅ Configuration via TOML
- ✅ Documentation complete

**Code Metrics**:
- ~1,200 lines production code
- ~600 lines test code
- ~200 lines documentation
- **Total**: ~2,000 lines

**Performance Targets Met**:
- ✅ Access tracking: <1ms overhead
- ✅ Background worker: <30s per cycle
- ✅ Zero data corruption

**Next**: Week 3 - Integration Testing + RC2 Release

---

## Appendix: Quick Commands

**Run All Tests**:
```bash
cargo test --workspace
```

**Run Tiering Tests Only**:
```bash
cargo test -p akidb-storage tiering
cargo test -p akidb-metadata tier_state
cargo test -p akidb-rest tier_api
```

**Start Server with Tiering**:
```bash
# Edit config.toml to enable tiering
cargo run -p akidb-rest
```

**Test Tier Control API**:
```bash
# Get tier state
curl http://localhost:8080/collections/{id}/tier

# Pin collection
curl -X POST http://localhost:8080/collections/{id}/tier/pin

# Force promote
curl -X POST http://localhost:8080/collections/{id}/tier/promote
```

---

**Status**: ✅ WEEK 2 ACTION PLAN COMPLETE - READY FOR EXECUTION
