//! Write-Ahead Log (WAL) implementation for durability
//!
//! The WAL ensures that all operations are persisted to disk before being acknowledged,
//! guaranteeing zero data loss on crashes. Each entry is assigned a monotonically
//! increasing Log Sequence Number (LSN) for ordering and replay.

mod file_wal;

pub use file_wal::{FileWAL, FileWALConfig};

use akidb_core::{CollectionId, CoreResult, DocumentId};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Log Sequence Number - monotonically increasing identifier for WAL entries
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LogSequenceNumber(u64);

impl LogSequenceNumber {
    /// Create a new LSN
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    /// Get the next LSN
    ///
    /// # Panics
    ///
    /// Panics if LSN has reached u64::MAX (overflow protection).
    /// In practice, this would require 18 quintillion operations,
    /// which is unrealistic for any production workload.
    ///
    /// # Bug Fix (Bug #9 - ULTRATHINK)
    ///
    /// Changed from `wrapping_add` to `checked_add` to prevent LSN wraparound.
    /// Wraparound would violate WAL ordering guarantees and cause data loss.
    pub fn next(&self) -> Self {
        Self(
            self.0
                .checked_add(1)
                .expect("LSN overflow: exceeded u64::MAX operations"),
        )
    }

    /// Get the raw value
    pub fn value(&self) -> u64 {
        self.0
    }

    /// Zero LSN (start of log)
    pub const ZERO: Self = Self(0);
}

impl fmt::Display for LogSequenceNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LSN({})", self.0)
    }
}

impl From<u64> for LogSequenceNumber {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<LogSequenceNumber> for u64 {
    fn from(lsn: LogSequenceNumber) -> Self {
        lsn.0
    }
}

/// WAL entry types - all operations that modify state
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LogEntry {
    /// Insert or update a vector document
    Upsert {
        collection_id: CollectionId,
        doc_id: DocumentId,
        vector: Vec<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        external_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<serde_json::Value>,
        timestamp: DateTime<Utc>,
    },

    /// Delete a vector document
    Delete {
        collection_id: CollectionId,
        doc_id: DocumentId,
        timestamp: DateTime<Utc>,
    },

    /// Create a new collection
    CreateCollection {
        collection_id: CollectionId,
        dimension: u32,
        timestamp: DateTime<Utc>,
    },

    /// Delete a collection (soft delete, vectors may remain in S3)
    DeleteCollection {
        collection_id: CollectionId,
        timestamp: DateTime<Utc>,
    },

    /// Checkpoint marker - all entries before this LSN are safe to discard
    Checkpoint {
        lsn: LogSequenceNumber,
        timestamp: DateTime<Utc>,
    },
}

impl LogEntry {
    /// Get the timestamp of this entry
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            LogEntry::Upsert { timestamp, .. }
            | LogEntry::Delete { timestamp, .. }
            | LogEntry::CreateCollection { timestamp, .. }
            | LogEntry::DeleteCollection { timestamp, .. }
            | LogEntry::Checkpoint { timestamp, .. } => *timestamp,
        }
    }

    /// Get the collection ID if applicable
    pub fn collection_id(&self) -> Option<CollectionId> {
        match self {
            LogEntry::Upsert { collection_id, .. }
            | LogEntry::Delete { collection_id, .. }
            | LogEntry::CreateCollection { collection_id, .. }
            | LogEntry::DeleteCollection { collection_id, .. } => Some(*collection_id),
            LogEntry::Checkpoint { .. } => None,
        }
    }

    /// Check if this is a checkpoint entry
    pub fn is_checkpoint(&self) -> bool {
        matches!(self, LogEntry::Checkpoint { .. })
    }
}

/// Write-Ahead Log trait - ensures durability before acknowledging operations
///
/// Implementations must guarantee:
/// 1. Durability: Data is fsync'd to disk before `append()` returns
/// 2. Ordering: LSNs are strictly monotonically increasing
/// 3. Atomicity: Batch operations are all-or-nothing
/// 4. Recoverability: `replay()` can reconstruct state from any LSN
#[async_trait]
pub trait WriteAheadLog: Send + Sync {
    /// Append a single entry to the WAL
    ///
    /// Returns the assigned LSN. Guarantees durability (fsync) before return.
    ///
    /// # Errors
    /// - `CoreError::IoError` if disk write fails
    /// - `CoreError::StorageError` if WAL is full or corrupted
    async fn append(&self, entry: LogEntry) -> CoreResult<LogSequenceNumber>;

    /// Append multiple entries atomically
    ///
    /// All entries are assigned consecutive LSNs and written atomically.
    /// If any write fails, the entire batch is rolled back.
    ///
    /// # Errors
    /// - `CoreError::IoError` if disk write fails
    /// - `CoreError::StorageError` if WAL is full or corrupted
    async fn append_batch(&self, entries: Vec<LogEntry>) -> CoreResult<Vec<LogSequenceNumber>>;

    /// Replay entries from a given LSN for crash recovery
    ///
    /// Returns all entries with LSN >= `from_lsn` in order.
    /// Used to reconstruct in-memory state after a crash.
    ///
    /// # Errors
    /// - `CoreError::IoError` if log files cannot be read
    /// - `CoreError::DeserializationError` if entries are corrupted
    async fn replay(
        &self,
        from_lsn: LogSequenceNumber,
    ) -> CoreResult<Vec<(LogSequenceNumber, LogEntry)>>;

    /// Mark all entries before this LSN as safe to discard
    ///
    /// This is called after creating a snapshot to S3, indicating that
    /// entries before this point are durable elsewhere and can be removed.
    ///
    /// # Errors
    /// - `CoreError::IoError` if checkpoint marker cannot be written
    async fn checkpoint(&self, lsn: LogSequenceNumber) -> CoreResult<()>;

    /// Rotate the current log file
    ///
    /// Creates a new log file and archives the current one.
    /// Typically called when the current file exceeds a size threshold.
    ///
    /// # Errors
    /// - `CoreError::IoError` if file operations fail
    async fn rotate(&self) -> CoreResult<()>;

    /// Get the current (highest assigned) LSN
    ///
    /// # Errors
    /// - `CoreError::IoError` if WAL state cannot be read
    async fn current_lsn(&self) -> CoreResult<LogSequenceNumber>;

    /// Force flush all pending writes to disk
    ///
    /// Guarantees all buffered data is fsync'd.
    ///
    /// # Errors
    /// - `CoreError::IoError` if fsync fails
    async fn flush(&self) -> CoreResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lsn_ordering() {
        let lsn1 = LogSequenceNumber::new(1);
        let lsn2 = LogSequenceNumber::new(2);
        let lsn3 = lsn1.next();

        assert!(lsn1 < lsn2);
        assert_eq!(lsn2, lsn3);
        assert_eq!(lsn1.value(), 1);
    }

    #[test]
    fn test_lsn_zero() {
        let zero = LogSequenceNumber::ZERO;
        assert_eq!(zero.value(), 0);
        assert_eq!(zero.next().value(), 1);
    }

    #[test]
    fn test_log_entry_timestamp() {
        let now = Utc::now();
        let entry = LogEntry::Upsert {
            collection_id: CollectionId::new(),
            doc_id: DocumentId::new(),
            vector: vec![1.0, 2.0, 3.0],
            external_id: None,
            metadata: None,
            timestamp: now,
        };

        assert_eq!(entry.timestamp(), now);
        assert!(entry.collection_id().is_some());
        assert!(!entry.is_checkpoint());
    }

    #[test]
    fn test_log_entry_serialization() {
        let entry = LogEntry::CreateCollection {
            collection_id: CollectionId::new(),
            dimension: 128,
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: LogEntry = serde_json::from_str(&json).unwrap();

        assert!(matches!(deserialized, LogEntry::CreateCollection { .. }));
    }

    #[test]
    fn test_checkpoint_entry() {
        let checkpoint = LogEntry::Checkpoint {
            lsn: LogSequenceNumber::new(1000),
            timestamp: Utc::now(),
        };

        assert!(checkpoint.is_checkpoint());
        assert!(checkpoint.collection_id().is_none());
    }
}
