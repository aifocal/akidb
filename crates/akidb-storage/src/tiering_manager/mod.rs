//! Hot/Warm/Cold Tiering Manager
//!
//! This module implements automatic tiering of vector collections based on access patterns:
//! - **Hot Tier** (RAM): Frequently accessed collections, <1ms latency
//! - **Warm Tier** (Local Disk): Occasionally accessed collections, 1-10ms latency
//! - **Cold Tier** (S3/MinIO): Rarely accessed collections, 100-500ms latency
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      CollectionService                          │
//! │                                                                 │
//! │  search() → AccessTracker.record_access(collection_id)         │
//! │                        ↓                                        │
//! │                  TieringManager                                 │
//! │                        ↓                                        │
//! │     ┌──────────────────┼──────────────────┐                   │
//! │     │                  │                  │                   │
//! │  Hot Tier          Warm Tier          Cold Tier               │
//! │  (RAM)             (SSD)              (S3/MinIO)              │
//! │  - VectorIndex     - Parquet files    - Parquet snapshots     │
//! │  - In-memory       - Local disk       - Object store          │
//! │  - <1ms latency    - 1-10ms latency   - 100-500ms latency     │
//! │     │                  │                  │                   │
//! │     └──────────────────┴──────────────────┘                   │
//! │                        ↑                                        │
//! │               Background Worker                                 │
//! │               (runs every 5 minutes)                            │
//! │               - Check access patterns                           │
//! │               - Demote hot → warm                               │
//! │               - Demote warm → cold                              │
//! │               - Promote warm → hot                              │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Example
//!
//! ```no_run
//! use akidb_storage::tiering_manager::{TieringManager, TieringPolicyConfig};
//! use akidb_metadata::TierStateRepository;
//! use std::sync::Arc;
//!
//! # async fn example() -> akidb_core::CoreResult<()> {
//! let pool = sqlx::SqlitePool::connect("sqlite::memory:").await?;
//! let repo = Arc::new(TierStateRepository::new(pool));
//! let policy = TieringPolicyConfig::default();
//! let mut manager = TieringManager::new(policy, repo)?;
//!
//! // Start background worker
//! manager.start_worker();
//!
//! // Record access
//! let collection_id = akidb_core::CollectionId::new();
//! manager.record_access(collection_id).await?;
//!
//! // Get tier state
//! let state = manager.get_tier_state(collection_id).await?;
//! # Ok(())
//! # }
//! ```

mod manager;
mod policy;
mod state;
mod tracker;

pub use manager::TieringManager;
pub use policy::TieringPolicyConfig;
pub use state::{Tier, TierState};
pub use tracker::{AccessStats, AccessTracker};
