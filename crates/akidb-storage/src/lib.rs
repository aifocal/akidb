//! AkiDB Storage Layer - Tiered storage with S3/MinIO support
//!
//! This crate provides durable storage for vector data with multiple tiers:
//! - Write-Ahead Log (WAL) for crash recovery
//! - Object Store (S3/MinIO/Local) for cold data
//! - Parquet snapshots for efficient serialization
//! - Tiering policies for automatic hot/warm/cold management
//!
//! # Architecture
//!
//! ```text
//! ┌────────────────────────────────────┐
//! │   StorageBackend (Orchestrator)    │
//! └────────────────────────────────────┘
//!     ↓        ↓          ↓        ↓
//! ┌─────┐ ┌──────┐ ┌──────────┐ ┌────────┐
//! │ WAL │ │Index │ │Snapshotter│ │S3/MinIO│
//! └─────┘ └──────┘ └──────────┘ └────────┘
//! ```
//!
//! # Usage Example
//!
//! ```rust,no_run
//! use akidb_storage::wal::{FileWAL, FileWALConfig, WriteAheadLog};
//! use akidb_core::{CollectionId, DocumentId};
//!
//! #[tokio::main]
//! async fn main() -> akidb_core::CoreResult<()> {
//!     // Create WAL for durability
//!     let wal = FileWAL::new("./wal", FileWALConfig::default()).await?;
//!
//!     // Append an operation
//!     let entry = akidb_storage::wal::LogEntry::CreateCollection {
//!         collection_id: CollectionId::new(),
//!         dimension: 128,
//!         timestamp: chrono::Utc::now(),
//!     };
//!
//!     let lsn = wal.append(entry).await?;
//!     println!("Logged with LSN: {}", lsn);
//!
//!     Ok(())
//! }
//! ```
//!
//! # Features
//!
//! - **Phase 6 Week 1: WAL** ✅ Complete
//!   - File-based Write-Ahead Log with fsync
//!   - Crash recovery and replay
//!   - Automatic log rotation
//!   - Checkpoint and cleanup
//!
//! - **Phase 6 Week 2: ObjectStore** (Pending)
//!   - S3 implementation
//!   - MinIO compatibility
//!   - Local filesystem fallback
//!
//! - **Phase 6 Week 3: Snapshotter** (Pending)
//!   - Parquet serialization
//!   - Compression (Snappy, Zstd)
//!   - Efficient columnar format
//!
//! - **Phase 6 Week 4: Tiering** (Pending)
//!   - Hot/warm/cold tier management
//!   - Eviction policies (LRU, LFU, FIFO)
//!   - Automatic data movement
//!
//! - **Phase 6 Week 5: Production** (Pending)
//!   - End-to-end testing
//!   - Performance optimization
//!   - Documentation and examples

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod batch_config;
pub mod batch_uploader;
pub mod circuit_breaker;
pub mod compression;
pub mod dlq;
pub mod object_store;
pub mod parallel_uploader;
pub mod parquet_encoder;
pub mod snapshotter;
pub mod storage_backend;
pub mod tiering;
pub mod tiering_manager; // Phase 10 Week 2: Hot/Warm/Cold tiering
pub mod wal;

// Re-export commonly used types
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerState};
pub use dlq::{DLQConfig, DLQEntry, DLQMetrics, DeadLetterQueue};
pub use object_store::{
    CallHistoryEntry, MockFailure, MockS3Config, MockS3ObjectStore, ObjectStore,
};
pub use storage_backend::{CacheStats, RetryConfig, StorageBackend, StorageMetrics};
pub use tiering::{CompactionConfig, CompressionType, StorageConfig, TieringPolicy};
pub use wal::{FileWAL, FileWALConfig, LogEntry, LogSequenceNumber, WriteAheadLog};

/// Storage module version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Check if storage backend is initialized
///
/// This can be used to verify the storage layer is ready before
/// starting the main application.
pub fn is_initialized() -> bool {
    // For Phase 6 Week 1, just check if the module compiles
    // Future: Check if WAL directory exists, S3 credentials valid, etc.
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_is_initialized() {
        assert!(is_initialized());
    }
}
