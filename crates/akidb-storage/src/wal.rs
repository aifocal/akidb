use async_trait::async_trait;
use bytes::Bytes;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

use akidb_core::{Error, Result};

/// Strongly typed log sequence number used by WAL operations.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LogSequence(pub u64);

impl LogSequence {
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn next(&self) -> Self {
        Self(self.0.saturating_add(1))
    }

    pub fn value(&self) -> u64 {
        self.0
    }
}

/// Identifier for a WAL stream, typically scoped per collection shard.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct WalStreamId(pub Uuid);

impl WalStreamId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for WalStreamId {
    fn default() -> Self {
        Self::new()
    }
}

/// Logical WAL record capturing mutation intent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalRecord {
    Insert {
        collection: String,
        primary_key: String,
        vector: Vec<f32>,
        payload: Value,
    },
    Delete {
        collection: String,
        primary_key: String,
    },
    UpsertPayload {
        collection: String,
        primary_key: String,
        payload: Value,
    },
}

/// WAL entry with LSN and timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WalEntry {
    lsn: LogSequence,
    timestamp: chrono::DateTime<chrono::Utc>,
    record: WalRecord,
}

/// Aggregated statistics returned by WAL replay routines.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayStats {
    pub records: u64,
    pub bytes: u64,
}

/// Interface for appending WAL records in a durable, ordered manner.
#[async_trait]
pub trait WalAppender: Send + Sync {
    async fn append(&self, stream: WalStreamId, record: WalRecord) -> Result<LogSequence>;
    async fn sync(&self, stream: WalStreamId) -> Result<()>;
}

/// Interface for replaying WAL records from a persisted log.
#[async_trait]
pub trait WalReplayer: Send + Sync {
    async fn replay(&self, stream: WalStreamId, since: Option<LogSequence>) -> Result<ReplayStats>;
    async fn next_batch(&self, stream: WalStreamId, max_bytes: usize) -> Result<Vec<Bytes>>;
}

/// Recovery statistics returned after crash recovery.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RecoveryStats {
    pub streams_recovered: usize,
    pub total_entries: u64,
    pub last_lsn_per_stream: HashMap<WalStreamId, LogSequence>,
}

/// Interface for WAL crash recovery operations.
#[async_trait]
pub trait WalRecovery: Send + Sync {
    /// Recover all WAL streams by scanning S3 and rebuilding LSN counters.
    async fn recover(&self) -> Result<RecoveryStats>;

    /// Recover a specific WAL stream and return its last LSN.
    async fn recover_stream(&self, stream: WalStreamId) -> Result<Option<LogSequence>>;
}

/// S3-backed WAL implementation using simple JSON format
pub struct S3WalBackend {
    storage: Arc<dyn crate::backend::StorageBackend>,
    // Track current LSN for each stream
    lsn_counters: Arc<RwLock<HashMap<WalStreamId, LogSequence>>>,
    // In-memory buffer before syncing to S3
    buffers: Arc<RwLock<HashMap<WalStreamId, Vec<WalEntry>>>>,
}

impl S3WalBackend {
    pub fn new(storage: Arc<dyn crate::backend::StorageBackend>) -> Self {
        Self {
            storage,
            lsn_counters: Arc::new(RwLock::new(HashMap::new())),
            buffers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generate S3 key for WAL stream
    fn wal_key(&self, stream: WalStreamId) -> String {
        format!("wal/{}.wal", stream.0)
    }

    /// Load existing WAL entries from S3
    async fn load_entries(&self, stream: WalStreamId) -> Result<Vec<WalEntry>> {
        let key = self.wal_key(stream);

        // Try to get the object, return empty vec if not found
        match self.storage.get_object(&key).await {
            Ok(data) => {
                let entries: Vec<WalEntry> = serde_json::from_slice(&data)
                    .map_err(|e| Error::Storage(format!("Failed to deserialize WAL entries: {}", e)))?;

                debug!("Loaded {} WAL entries for stream {}", entries.len(), stream.0);
                Ok(entries)
            }
            Err(Error::NotFound(_)) => {
                debug!("No existing WAL found for stream {}", stream.0);
                Ok(Vec::new())
            }
            Err(e) => Err(e),
        }
    }

    /// Persist WAL entries to S3
    async fn persist_entries(&self, stream: WalStreamId, entries: &[WalEntry]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        let key = self.wal_key(stream);

        // Load existing entries and append new ones
        let mut all_entries = self.load_entries(stream).await?;
        all_entries.extend_from_slice(entries);

        // Serialize and persist
        let data = serde_json::to_vec(&all_entries)
            .map_err(|e| Error::Storage(format!("Failed to serialize WAL entries: {}", e)))?;

        self.storage.put_object(&key, Bytes::from(data)).await?;

        debug!(
            "Persisted {} WAL entries for stream {} (total: {} entries)",
            entries.len(),
            stream.0,
            all_entries.len()
        );

        Ok(())
    }

    /// Get next LSN for a stream
    fn next_lsn(&self, stream: WalStreamId) -> LogSequence {
        let mut counters = self.lsn_counters.write();
        let lsn = counters.entry(stream).or_insert(LogSequence::new(0));
        let next = lsn.next();
        *lsn = next;
        next
    }
}

#[async_trait]
impl WalAppender for S3WalBackend {
    async fn append(&self, stream: WalStreamId, record: WalRecord) -> Result<LogSequence> {
        let lsn = self.next_lsn(stream);

        let entry = WalEntry {
            lsn,
            timestamp: chrono::Utc::now(),
            record,
        };

        // Add to in-memory buffer
        {
            let mut buffers = self.buffers.write();
            buffers.entry(stream).or_default().push(entry);
        }

        debug!("Appended WAL record with LSN {} to stream {}", lsn.0, stream.0);
        Ok(lsn)
    }

    async fn sync(&self, stream: WalStreamId) -> Result<()> {
        info!("Syncing WAL stream {}", stream.0);

        // Get buffered entries
        let entries = {
            let mut buffers = self.buffers.write();
            buffers.remove(&stream).unwrap_or_default()
        };

        if entries.is_empty() {
            debug!("No entries to sync for stream {}", stream.0);
            return Ok(());
        }

        // Persist to S3
        self.persist_entries(stream, &entries).await?;

        info!("Synced {} WAL entries for stream {}", entries.len(), stream.0);
        Ok(())
    }
}

#[async_trait]
impl WalReplayer for S3WalBackend {
    async fn replay(&self, stream: WalStreamId, since: Option<LogSequence>) -> Result<ReplayStats> {
        info!("Replaying WAL stream {} since {:?}", stream.0, since);

        // Load all entries from S3
        let entries = self.load_entries(stream).await?;

        // Filter entries based on LSN
        let filtered: Vec<_> = entries
            .into_iter()
            .filter(|entry| {
                if let Some(since_lsn) = since {
                    entry.lsn > since_lsn
                } else {
                    true
                }
            })
            .collect();

        let record_count = filtered.len() as u64;
        let total_bytes: u64 = filtered
            .iter()
            .map(|entry| {
                serde_json::to_vec(entry)
                    .map(|v| v.len() as u64)
                    .unwrap_or(0)
            })
            .sum();

        debug!(
            "Replayed {} records ({} bytes) from stream {}",
            record_count, total_bytes, stream.0
        );

        Ok(ReplayStats {
            records: record_count,
            bytes: total_bytes,
        })
    }

    async fn next_batch(&self, stream: WalStreamId, max_bytes: usize) -> Result<Vec<Bytes>> {
        debug!("Fetching next batch (max {} bytes) from stream {}", max_bytes, stream.0);

        // Load entries
        let entries = self.load_entries(stream).await?;

        let mut batch = Vec::new();
        let mut current_bytes = 0;

        for entry in entries {
            let data = serde_json::to_vec(&entry)
                .map_err(|e| Error::Storage(format!("Failed to serialize entry: {}", e)))?;

            let entry_size = data.len();
            if current_bytes + entry_size > max_bytes && !batch.is_empty() {
                break;
            }

            current_bytes += entry_size;
            batch.push(Bytes::from(data));
        }

        debug!("Returning batch of {} entries ({} bytes)", batch.len(), current_bytes);
        Ok(batch)
    }
}

#[async_trait]
impl WalRecovery for S3WalBackend {
    async fn recover(&self) -> Result<RecoveryStats> {
        info!("Starting WAL crash recovery");

        // List all WAL files in S3
        let wal_prefix = "wal/";
        let keys = self.storage.list_objects(wal_prefix).await?;

        let mut stats = RecoveryStats::default();

        for key in keys {
            // Extract stream ID from key (format: "wal/{uuid}.wal")
            if let Some(filename) = key.strip_prefix(wal_prefix) {
                if let Some(uuid_str) = filename.strip_suffix(".wal") {
                    if let Ok(uuid) = Uuid::parse_str(uuid_str) {
                        let stream = WalStreamId::from_uuid(uuid);

                        // Recover this stream
                        if let Some(last_lsn) = self.recover_stream(stream).await? {
                            stats.streams_recovered += 1;
                            stats.last_lsn_per_stream.insert(stream, last_lsn);

                            // Update LSN counter
                            let mut counters = self.lsn_counters.write();
                            counters.insert(stream, last_lsn);
                        }
                    }
                }
            }
        }

        // Calculate total entries
        stats.total_entries = stats
            .last_lsn_per_stream
            .values()
            .map(|lsn| lsn.value())
            .sum();

        info!(
            "WAL recovery completed: {} streams, {} total entries",
            stats.streams_recovered, stats.total_entries
        );

        Ok(stats)
    }

    async fn recover_stream(&self, stream: WalStreamId) -> Result<Option<LogSequence>> {
        debug!("Recovering WAL stream {}", stream.0);

        // Load all entries for this stream
        let entries = self.load_entries(stream).await?;

        if entries.is_empty() {
            debug!("No entries found for stream {}", stream.0);
            return Ok(None);
        }

        // Find the maximum LSN
        let max_lsn = entries
            .iter()
            .map(|entry| entry.lsn)
            .max()
            .unwrap_or(LogSequence::new(0));

        debug!(
            "Recovered stream {} with {} entries, last LSN: {}",
            stream.0,
            entries.len(),
            max_lsn.value()
        );

        Ok(Some(max_lsn))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_log_sequence_new() {
        let lsn = LogSequence::new(42);
        assert_eq!(lsn.value(), 42);
    }

    #[test]
    fn test_log_sequence_next() {
        let lsn = LogSequence::new(42);
        let next = lsn.next();
        assert_eq!(next.value(), 43);
    }

    #[test]
    fn test_log_sequence_ordering() {
        let lsn1 = LogSequence::new(1);
        let lsn2 = LogSequence::new(2);
        assert!(lsn1 < lsn2);
        assert!(lsn2 > lsn1);
    }

    #[test]
    fn test_wal_stream_id_new() {
        let stream1 = WalStreamId::new();
        let stream2 = WalStreamId::new();
        assert_ne!(stream1, stream2);
    }

    #[test]
    fn test_wal_stream_id_from_uuid() {
        let uuid = Uuid::new_v4();
        let stream = WalStreamId::from_uuid(uuid);
        assert_eq!(stream.0, uuid);
    }

    #[test]
    fn test_wal_record_serialization() {
        let record = WalRecord::Insert {
            collection: "test".to_string(),
            primary_key: "key1".to_string(),
            vector: vec![1.0, 2.0, 3.0],
            payload: json!({"field": "value"}),
        };

        let serialized = serde_json::to_string(&record).unwrap();
        let deserialized: WalRecord = serde_json::from_str(&serialized).unwrap();

        match deserialized {
            WalRecord::Insert { collection, primary_key, vector, .. } => {
                assert_eq!(collection, "test");
                assert_eq!(primary_key, "key1");
                assert_eq!(vector, vec![1.0, 2.0, 3.0]);
            }
            _ => panic!("Expected Insert record"),
        }
    }

    #[test]
    fn test_wal_entry_serialization() {
        let entry = WalEntry {
            lsn: LogSequence::new(1),
            timestamp: chrono::Utc::now(),
            record: WalRecord::Delete {
                collection: "test".to_string(),
                primary_key: "key1".to_string(),
            },
        };

        let serialized = serde_json::to_vec(&entry).unwrap();
        let deserialized: WalEntry = serde_json::from_slice(&serialized).unwrap();

        assert_eq!(deserialized.lsn, entry.lsn);
        match deserialized.record {
            WalRecord::Delete { collection, primary_key } => {
                assert_eq!(collection, "test");
                assert_eq!(primary_key, "key1");
            }
            _ => panic!("Expected Delete record"),
        }
    }

    #[test]
    fn test_replay_stats_default() {
        let stats = ReplayStats::default();
        assert_eq!(stats.records, 0);
        assert_eq!(stats.bytes, 0);
    }

    #[test]
    fn test_recovery_stats_default() {
        let stats = RecoveryStats::default();
        assert_eq!(stats.streams_recovered, 0);
        assert_eq!(stats.total_entries, 0);
        assert!(stats.last_lsn_per_stream.is_empty());
    }
}

// Integration tests (require actual S3/MinIO)
#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::s3::{S3Config, S3StorageBackend};
    use serde_json::json;

    // Helper to create a test S3 backend (requires MinIO running)
    fn create_test_storage() -> Option<Arc<S3StorageBackend>> {
        // This requires MinIO to be running on localhost:9000
        // Skip if not available
        let config = S3Config {
            endpoint: "http://localhost:9000".to_string(),
            region: "us-east-1".to_string(),
            access_key: "minioadmin".to_string(),
            secret_key: "minioadmin".to_string(),
            bucket: "akidb-test".to_string(),
            ..Default::default()
        };

        S3StorageBackend::new(config).ok().map(Arc::new)
    }

    #[tokio::test]
    #[ignore] // Requires MinIO to be running
    async fn test_wal_append_and_sync() {
        let storage = match create_test_storage() {
            Some(s) => s,
            None => return, // Skip test if MinIO not available
        };

        let wal = S3WalBackend::new(storage.clone());
        let stream = WalStreamId::new();

        // Append some records
        let record1 = WalRecord::Insert {
            collection: "test".to_string(),
            primary_key: "key1".to_string(),
            vector: vec![1.0, 2.0, 3.0],
            payload: json!({"field": "value1"}),
        };

        let record2 = WalRecord::Delete {
            collection: "test".to_string(),
            primary_key: "key2".to_string(),
        };

        let lsn1 = wal.append(stream, record1).await.unwrap();
        let lsn2 = wal.append(stream, record2).await.unwrap();

        assert_eq!(lsn1.value(), 1);
        assert_eq!(lsn2.value(), 2);

        // Sync to S3
        wal.sync(stream).await.unwrap();

        // Load entries and verify
        let entries = wal.load_entries(stream).await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].lsn, lsn1);
        assert_eq!(entries[1].lsn, lsn2);
    }

    #[tokio::test]
    #[ignore] // Requires MinIO to be running
    async fn test_wal_replay() {
        let storage = match create_test_storage() {
            Some(s) => s,
            None => return,
        };

        let wal = S3WalBackend::new(storage.clone());
        let stream = WalStreamId::new();

        // Add and sync some records
        for i in 1..=5 {
            let record = WalRecord::Insert {
                collection: "test".to_string(),
                primary_key: format!("key{}", i),
                vector: vec![i as f32],
                payload: json!({"index": i}),
            };
            wal.append(stream, record).await.unwrap();
        }
        wal.sync(stream).await.unwrap();

        // Replay from beginning
        let stats = wal.replay(stream, None).await.unwrap();
        assert_eq!(stats.records, 5);

        // Replay from LSN 3
        let stats = wal.replay(stream, Some(LogSequence::new(3))).await.unwrap();
        assert_eq!(stats.records, 2); // Should only replay LSN 4 and 5
    }

    #[tokio::test]
    #[ignore] // Requires MinIO to be running
    async fn test_wal_recovery() {
        let storage = match create_test_storage() {
            Some(s) => s,
            None => return,
        };

        // Create first WAL and add records
        let wal1 = S3WalBackend::new(storage.clone());
        let stream1 = WalStreamId::new();
        let stream2 = WalStreamId::new();

        // Add records to stream1
        for i in 1..=3 {
            let record = WalRecord::Insert {
                collection: "test1".to_string(),
                primary_key: format!("key{}", i),
                vector: vec![i as f32],
                payload: json!({}),
            };
            wal1.append(stream1, record).await.unwrap();
        }
        wal1.sync(stream1).await.unwrap();

        // Add records to stream2
        for i in 1..=5 {
            let record = WalRecord::Insert {
                collection: "test2".to_string(),
                primary_key: format!("key{}", i),
                vector: vec![i as f32],
                payload: json!({}),
            };
            wal1.append(stream2, record).await.unwrap();
        }
        wal1.sync(stream2).await.unwrap();

        // Create new WAL backend (simulating restart)
        let wal2 = S3WalBackend::new(storage.clone());

        // Recover
        let stats = wal2.recover().await.unwrap();
        assert_eq!(stats.streams_recovered, 2);
        assert_eq!(stats.last_lsn_per_stream.get(&stream1).map(|l| l.value()), Some(3));
        assert_eq!(stats.last_lsn_per_stream.get(&stream2).map(|l| l.value()), Some(5));

        // Verify LSN counters are restored
        // Next append should use LSN 4 for stream1 and LSN 6 for stream2
        let record = WalRecord::Delete {
            collection: "test".to_string(),
            primary_key: "test".to_string(),
        };
        let next_lsn1 = wal2.append(stream1, record.clone()).await.unwrap();
        let next_lsn2 = wal2.append(stream2, record).await.unwrap();

        assert_eq!(next_lsn1.value(), 4);
        assert_eq!(next_lsn2.value(), 6);
    }

    #[tokio::test]
    #[ignore] // Requires MinIO to be running
    async fn test_wal_next_batch() {
        let storage = match create_test_storage() {
            Some(s) => s,
            None => return,
        };

        let wal = S3WalBackend::new(storage.clone());
        let stream = WalStreamId::new();

        // Add many records
        for i in 1..=10 {
            let record = WalRecord::Insert {
                collection: "test".to_string(),
                primary_key: format!("key{}", i),
                vector: vec![i as f32; 100], // Larger vectors
                payload: json!({"index": i}),
            };
            wal.append(stream, record).await.unwrap();
        }
        wal.sync(stream).await.unwrap();

        // Fetch batch with size limit
        let batch = wal.next_batch(stream, 1024).await.unwrap();

        // Should get at least one entry, but not all
        assert!(!batch.is_empty());
        assert!(batch.len() < 10);
    }
}
