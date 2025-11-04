// Append-only WAL implementation with O(1) append performance
// This module provides a segmented WAL that avoids O(n²) performance issues.

use async_trait::async_trait;
use bytes::Bytes;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use akidb_core::{Error, Result};

use crate::backend::StorageBackend;
use crate::wal::{
    LogSequence, RecoveryStats, ReplayStats, WalAppender, WalEntry, WalRecord, WalRecovery,
    WalReplayer, WalStreamId,
};

/// Maximum number of entries per WAL segment before sealing
const DEFAULT_SEGMENT_SIZE: usize = 10_000;

/// Metadata about a sealed WAL segment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentMetadata {
    pub id: u64,
    pub path: String,
    pub start_lsn: u64,
    pub end_lsn: u64,
    pub entry_count: usize,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub sealed: bool,
}

/// Manifest tracking all WAL segments for a stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalManifest {
    pub version: u32,
    pub segments: Vec<SegmentMetadata>,
    pub next_lsn: u64,
}

impl Default for WalManifest {
    fn default() -> Self {
        Self {
            version: 1,
            segments: Vec::new(),
            next_lsn: 0,
        }
    }
}

/// In-memory representation of an active WAL segment
struct ActiveSegment {
    id: u64,
    path: String,
    start_lsn: u64,
    entries: Vec<WalEntry>,
}

impl ActiveSegment {
    fn new(id: u64, path: String, start_lsn: u64) -> Self {
        Self {
            id,
            path,
            start_lsn,
            entries: Vec::new(),
        }
    }

    fn is_full(&self, max_size: usize) -> bool {
        self.entries.len() >= max_size
    }

    fn seal(&self) -> SegmentMetadata {
        let end_lsn = if self.entries.is_empty() {
            self.start_lsn
        } else {
            self.start_lsn + self.entries.len() as u64 - 1
        };

        SegmentMetadata {
            id: self.id,
            path: self.path.clone(),
            start_lsn: self.start_lsn,
            end_lsn,
            entry_count: self.entries.len(),
            created_at: chrono::Utc::now(),
            sealed: true,
        }
    }
}

/// State for a single WAL stream
struct StreamState {
    manifest: WalManifest,
    active_segment: ActiveSegment,
}

/// Append-only WAL backend with O(1) append performance
///
/// This implementation uses a segmented approach where:
/// - WAL is split into fixed-size segments (default: 10K entries)
/// - Each segment is immutable once sealed
/// - Only the active segment is modified on append
/// - Sync operations only upload the active segment (O(1) instead of O(n))
///
/// Storage layout:
/// ```text
/// wal/{stream_id}/
///   manifest.json          - Manifest tracking all segments
///   segments/
///     0000000000.wal       - Segment 0 (sealed)
///     0000010000.wal       - Segment 1 (sealed)
///     0000020000.wal       - Segment 2 (active)
/// ```
pub struct AppendOnlyWalBackend {
    storage: Arc<dyn StorageBackend>,
    /// Per-stream state (manifest + active segment)
    streams: Arc<RwLock<HashMap<WalStreamId, StreamState>>>,
    /// Maximum entries per segment
    segment_size: usize,
}

impl AppendOnlyWalBackend {
    /// Create a new append-only WAL backend
    ///
    /// # Parameters
    /// - `storage`: Storage backend for persisting WAL data
    /// - `segment_size`: Maximum entries per segment (default: 10,000)
    pub fn new(storage: Arc<dyn StorageBackend>, segment_size: Option<usize>) -> Self {
        Self {
            storage,
            streams: Arc::new(RwLock::new(HashMap::new())),
            segment_size: segment_size.unwrap_or(DEFAULT_SEGMENT_SIZE),
        }
    }

    /// Generate S3 key for WAL manifest
    fn manifest_key(&self, stream: WalStreamId) -> String {
        format!("wal/{}/manifest.json", stream.0)
    }

    /// Generate S3 key for WAL segment
    fn segment_key(&self, stream: WalStreamId, start_lsn: u64) -> String {
        format!("wal/{}/segments/{:010}.wal", stream.0, start_lsn)
    }

    /// Load manifest from storage
    async fn load_manifest(&self, stream: WalStreamId) -> Result<WalManifest> {
        let key = self.manifest_key(stream);

        match self.storage.get_object(&key).await {
            Ok(data) => {
                let manifest: WalManifest = serde_json::from_slice(&data).map_err(|e| {
                    Error::Storage(format!("Failed to deserialize WAL manifest: {}", e))
                })?;

                debug!(
                    "Loaded WAL manifest for stream {} with {} segments",
                    stream.0,
                    manifest.segments.len()
                );
                Ok(manifest)
            }
            Err(Error::NotFound(_)) => {
                debug!("No existing WAL manifest for stream {}, creating new", stream.0);
                Ok(WalManifest::default())
            }
            Err(e) => Err(e),
        }
    }

    /// Persist manifest to storage
    async fn persist_manifest(&self, stream: WalStreamId, manifest: &WalManifest) -> Result<()> {
        let key = self.manifest_key(stream);
        let data = serde_json::to_vec_pretty(manifest)
            .map_err(|e| Error::Storage(format!("Failed to serialize WAL manifest: {}", e)))?;

        self.storage.put_object(&key, Bytes::from(data)).await?;

        debug!(
            "Persisted WAL manifest for stream {} ({} segments)",
            stream.0,
            manifest.segments.len()
        );
        Ok(())
    }

    /// Load segment entries from storage
    async fn load_segment(&self, stream: WalStreamId, segment_meta: &SegmentMetadata) -> Result<Vec<WalEntry>> {
        match self.storage.get_object(&segment_meta.path).await {
            Ok(data) => {
                let entries: Vec<WalEntry> = serde_json::from_slice(&data).map_err(|e| {
                    Error::Storage(format!(
                        "Failed to deserialize WAL segment {}: {}",
                        segment_meta.path, e
                    ))
                })?;

                debug!(
                    "Loaded WAL segment {} with {} entries",
                    segment_meta.path,
                    entries.len()
                );
                Ok(entries)
            }
            Err(e) => {
                warn!(
                    "Failed to load WAL segment {} for stream {}: {}",
                    segment_meta.path, stream.0, e
                );
                Err(e)
            }
        }
    }

    /// Persist segment entries to storage
    async fn persist_segment(&self, segment: &ActiveSegment) -> Result<()> {
        if segment.entries.is_empty() {
            return Ok(());
        }

        let data = serde_json::to_vec(&segment.entries)
            .map_err(|e| Error::Storage(format!("Failed to serialize WAL segment: {}", e)))?;

        self.storage.put_object(&segment.path, Bytes::from(data)).await?;

        debug!(
            "Persisted WAL segment {} with {} entries",
            segment.path,
            segment.entries.len()
        );
        Ok(())
    }

    /// Seal the active segment and create a new one
    async fn seal_and_rotate(&self, stream: WalStreamId) -> Result<()> {
        // IMPORTANT: Clone data before async operations to avoid holding lock across await
        let (sealed_segment_to_persist, sealed_meta, new_segment_id, manifest_to_update) = {
            let mut streams = self.streams.write();
            let state = streams.get_mut(&stream).ok_or_else(|| {
                Error::NotFound(format!("WAL stream {} not initialized", stream.0))
            })?;

            // Seal current segment
            let sealed_meta = state.active_segment.seal();

            // Clone active segment for persisting (to avoid holding lock during I/O)
            let sealed_segment_clone = ActiveSegment {
                id: state.active_segment.id,
                path: state.active_segment.path.clone(),
                start_lsn: state.active_segment.start_lsn,
                entries: state.active_segment.entries.clone(),
            };

            // Add sealed metadata to manifest
            state.manifest.segments.push(sealed_meta.clone());

            // Create new active segment
            let new_segment_id = state.active_segment.id + 1;
            let new_start_lsn = state.manifest.next_lsn;
            let new_path = self.segment_key(stream, new_start_lsn);
            state.active_segment = ActiveSegment::new(new_segment_id, new_path, new_start_lsn);

            let manifest_clone = state.manifest.clone();

            (sealed_segment_clone, sealed_meta, new_segment_id, manifest_clone)
        }; // Lock is dropped here

        // Now perform async operations without holding the lock

        // Persist sealed segment
        self.persist_segment(&sealed_segment_to_persist).await?;

        // Persist manifest
        self.persist_manifest(stream, &manifest_to_update).await?;

        info!(
            "Sealed WAL segment {} and created new segment {} for stream {}",
            sealed_meta.id, new_segment_id, stream.0
        );

        Ok(())
    }

    /// Initialize stream state (load or create manifest)
    async fn ensure_stream_initialized(&self, stream: WalStreamId) -> Result<()> {
        // Fast path: check if already initialized
        {
            let streams = self.streams.read();
            if streams.contains_key(&stream) {
                return Ok(());
            }
        }

        // Slow path: initialize stream
        let manifest = self.load_manifest(stream).await?;
        let next_lsn = manifest.next_lsn;

        // Create initial active segment
        let segment_id = manifest.segments.len() as u64;
        let segment_path = self.segment_key(stream, next_lsn);
        let active_segment = ActiveSegment::new(segment_id, segment_path, next_lsn);

        let state = StreamState {
            manifest,
            active_segment,
        };

        // Insert into streams map
        {
            let mut streams = self.streams.write();
            streams.insert(stream, state);
        }

        debug!("Initialized WAL stream {}", stream.0);
        Ok(())
    }
}

#[async_trait]
impl WalAppender for AppendOnlyWalBackend {
    /// Append a record to the WAL (O(1) operation)
    ///
    /// IMPORTANT: This method only appends to in-memory buffer.
    /// Call sync() to persist to storage.
    async fn append(&self, stream: WalStreamId, record: WalRecord) -> Result<LogSequence> {
        // Ensure stream is initialized
        self.ensure_stream_initialized(stream).await?;

        // Append to active segment
        let lsn = {
            let mut streams = self.streams.write();
            let state = streams.get_mut(&stream).ok_or_else(|| {
                Error::NotFound(format!("WAL stream {} not found", stream.0))
            })?;

            // Assign LSN
            let lsn = LogSequence::new(state.manifest.next_lsn);
            state.manifest.next_lsn += 1;

            // Create entry
            let entry = WalEntry {
                lsn,
                timestamp: chrono::Utc::now(),
                record,
            };

            // Add to active segment
            state.active_segment.entries.push(entry);

            lsn
        };

        // Check if segment is full and needs rotation
        // IMPORTANT: Check if full before calling seal_and_rotate to avoid holding lock across await
        let needs_rotation = {
            let streams = self.streams.read();
            streams
                .get(&stream)
                .map(|state| state.active_segment.is_full(self.segment_size))
                .unwrap_or(false)
        }; // Lock is dropped here

        if needs_rotation {
            self.seal_and_rotate(stream).await?;
        }

        debug!("Appended WAL record with LSN {} to stream {}", lsn.value(), stream.0);
        Ok(lsn)
    }

    /// Sync the WAL to storage (O(1) operation)
    ///
    /// IMPORTANT: This only uploads the active segment, not the entire WAL.
    /// This is the key optimization that makes sync O(1) instead of O(n).
    async fn sync(&self, stream: WalStreamId) -> Result<()> {
        info!("Syncing WAL stream {}", stream.0);

        // Get active segment
        let (active_segment_clone, manifest_clone) = {
            let streams = self.streams.read();
            let state = streams.get(&stream).ok_or_else(|| {
                Error::NotFound(format!("WAL stream {} not found", stream.0))
            })?;

            // Clone to avoid holding lock during I/O
            let active_clone = ActiveSegment {
                id: state.active_segment.id,
                path: state.active_segment.path.clone(),
                start_lsn: state.active_segment.start_lsn,
                entries: state.active_segment.entries.clone(),
            };
            let manifest_clone = state.manifest.clone();

            (active_clone, manifest_clone)
        };

        if active_segment_clone.entries.is_empty() {
            debug!("No entries to sync for stream {}", stream.0);
            return Ok(());
        }

        // Persist active segment (only uploads current segment, not entire WAL)
        self.persist_segment(&active_segment_clone).await?;

        // Persist manifest
        self.persist_manifest(stream, &manifest_clone).await?;

        info!(
            "Synced {} WAL entries for stream {} (segment {})",
            active_segment_clone.entries.len(),
            stream.0,
            active_segment_clone.id
        );
        Ok(())
    }
}

#[async_trait]
impl WalReplayer for AppendOnlyWalBackend {
    async fn replay(&self, stream: WalStreamId, since: Option<LogSequence>) -> Result<ReplayStats> {
        info!("Replaying WAL stream {} since {:?}", stream.0, since);

        // Load manifest
        let manifest = self.load_manifest(stream).await?;

        let mut total_records = 0u64;
        let mut total_bytes = 0u64;

        // Load all sealed segments
        for segment_meta in &manifest.segments {
            if !segment_meta.sealed {
                continue;
            }

            // Skip segments that are entirely before 'since'
            if let Some(since_lsn) = since {
                if segment_meta.end_lsn < since_lsn.value() {
                    continue;
                }
            }

            // Load segment
            let entries = self.load_segment(stream, segment_meta).await?;

            // Count entries after 'since'
            for entry in entries {
                if let Some(since_lsn) = since {
                    if entry.lsn <= since_lsn {
                        continue;
                    }
                }

                total_records += 1;
                let entry_bytes = serde_json::to_vec(&entry)
                    .map(|v| v.len() as u64)
                    .unwrap_or(0);
                total_bytes += entry_bytes;
            }
        }

        // Load active segment
        {
            let streams = self.streams.read();
            if let Some(state) = streams.get(&stream) {
                for entry in &state.active_segment.entries {
                    if let Some(since_lsn) = since {
                        if entry.lsn <= since_lsn {
                            continue;
                        }
                    }

                    total_records += 1;
                    let entry_bytes = serde_json::to_vec(entry)
                        .map(|v| v.len() as u64)
                        .unwrap_or(0);
                    total_bytes += entry_bytes;
                }
            }
        }

        debug!(
            "Replayed {} records ({} bytes) from stream {}",
            total_records, total_bytes, stream.0
        );

        Ok(ReplayStats {
            records: total_records,
            bytes: total_bytes,
        })
    }

    async fn next_batch(
        &self,
        stream: WalStreamId,
        max_bytes: usize,
        since_lsn: Option<LogSequence>,
    ) -> Result<Vec<Bytes>> {
        debug!(
            "Fetching next batch (max {} bytes, since LSN {:?}) from stream {}",
            max_bytes, since_lsn, stream.0
        );

        // Load manifest
        let manifest = self.load_manifest(stream).await?;

        let mut batch = Vec::new();
        let mut current_bytes = 0;

        // Load from sealed segments
        for segment_meta in &manifest.segments {
            if !segment_meta.sealed {
                continue;
            }

            // Skip segments entirely before since_lsn
            if let Some(since) = since_lsn {
                if segment_meta.end_lsn < since.value() {
                    continue;
                }
            }

            // Load segment
            let entries = self.load_segment(stream, segment_meta).await?;

            for entry in entries {
                // Skip entries with LSN <= since_lsn
                if let Some(since) = since_lsn {
                    if entry.lsn <= since {
                        continue;
                    }
                }

                let data = serde_json::to_vec(&entry)
                    .map_err(|e| Error::Storage(format!("Failed to serialize entry: {}", e)))?;

                let entry_size = data.len();
                if current_bytes + entry_size > max_bytes && !batch.is_empty() {
                    // Batch is full
                    return Ok(batch);
                }

                current_bytes += entry_size;
                batch.push(Bytes::from(data));
            }
        }

        // Load from active segment
        {
            let streams = self.streams.read();
            if let Some(state) = streams.get(&stream) {
                for entry in &state.active_segment.entries {
                    // Skip entries with LSN <= since_lsn
                    if let Some(since) = since_lsn {
                        if entry.lsn <= since {
                            continue;
                        }
                    }

                    let data = serde_json::to_vec(entry)
                        .map_err(|e| Error::Storage(format!("Failed to serialize entry: {}", e)))?;

                    let entry_size = data.len();
                    if current_bytes + entry_size > max_bytes && !batch.is_empty() {
                        // Batch is full
                        return Ok(batch);
                    }

                    current_bytes += entry_size;
                    batch.push(Bytes::from(data));
                }
            }
        }

        debug!(
            "Returning batch of {} entries ({} bytes) from stream {}",
            batch.len(),
            current_bytes,
            stream.0
        );
        Ok(batch)
    }
}

#[async_trait]
impl WalRecovery for AppendOnlyWalBackend {
    async fn recover(&self) -> Result<RecoveryStats> {
        info!("Starting WAL crash recovery (append-only backend)");

        // List all manifests in S3 (format: "wal/{uuid}/manifest.json")
        let wal_prefix = "wal/";
        let keys = self.storage.list_objects(wal_prefix).await?;

        let mut stats = RecoveryStats::default();

        for key in keys {
            // Extract stream ID from key (format: "wal/{uuid}/manifest.json")
            if key.ends_with("/manifest.json") {
                if let Some(uuid_str) = key
                    .strip_prefix(wal_prefix)
                    .and_then(|s| s.strip_suffix("/manifest.json"))
                {
                    if let Ok(uuid) = Uuid::parse_str(uuid_str) {
                        let stream = WalStreamId::from_uuid(uuid);

                        // Recover this stream
                        if let Some(last_lsn) = self.recover_stream(stream).await? {
                            stats.streams_recovered += 1;
                            stats.last_lsn_per_stream.insert(stream, last_lsn);
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

        // Load manifest
        let manifest = match self.load_manifest(stream).await {
            Ok(m) => m,
            Err(Error::NotFound(_)) => {
                debug!("No manifest found for stream {}", stream.0);
                return Ok(None);
            }
            Err(e) => return Err(e),
        };

        if manifest.next_lsn == 0 {
            debug!("No entries found for stream {}", stream.0);
            return Ok(None);
        }

        let last_lsn = LogSequence::new(manifest.next_lsn - 1);

        // Initialize stream state
        let segment_id = manifest.segments.len() as u64;
        let segment_path = self.segment_key(stream, manifest.next_lsn);
        let active_segment = ActiveSegment::new(segment_id, segment_path, manifest.next_lsn);

        let state = StreamState {
            manifest,
            active_segment,
        };

        {
            let mut streams = self.streams.write();
            streams.insert(stream, state);
        }

        debug!(
            "Recovered stream {} with last LSN: {}",
            stream.0,
            last_lsn.value()
        );

        Ok(Some(last_lsn))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segment_metadata_serialization() {
        let meta = SegmentMetadata {
            id: 0,
            path: "wal/test/segments/0000000000.wal".to_string(),
            start_lsn: 0,
            end_lsn: 9999,
            entry_count: 10000,
            created_at: chrono::Utc::now(),
            sealed: true,
        };

        let json = serde_json::to_string(&meta).unwrap();
        let deserialized: SegmentMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, 0);
        assert_eq!(deserialized.entry_count, 10000);
        assert_eq!(deserialized.sealed, true);
    }

    #[test]
    fn test_wal_manifest_serialization() {
        let manifest = WalManifest {
            version: 1,
            segments: vec![
                SegmentMetadata {
                    id: 0,
                    path: "segment0".to_string(),
                    start_lsn: 0,
                    end_lsn: 9999,
                    entry_count: 10000,
                    created_at: chrono::Utc::now(),
                    sealed: true,
                },
            ],
            next_lsn: 10000,
        };

        let json = serde_json::to_string(&manifest).unwrap();
        let deserialized: WalManifest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.version, 1);
        assert_eq!(deserialized.segments.len(), 1);
        assert_eq!(deserialized.next_lsn, 10000);
    }

    #[test]
    fn test_active_segment_is_full() {
        let segment = ActiveSegment::new(0, "test".to_string(), 0);
        assert!(!segment.is_full(10000));

        let mut segment = ActiveSegment::new(0, "test".to_string(), 0);
        for i in 0..10000 {
            segment.entries.push(WalEntry {
                lsn: LogSequence::new(i),
                timestamp: chrono::Utc::now(),
                record: WalRecord::Delete {
                    collection: "test".to_string(),
                    primary_key: "key".to_string(),
                },
            });
        }
        assert!(segment.is_full(10000));
    }

    #[test]
    fn test_active_segment_seal() {
        let mut segment = ActiveSegment::new(0, "test_path".to_string(), 100);

        // Add 5 entries
        for i in 0..5 {
            segment.entries.push(WalEntry {
                lsn: LogSequence::new(100 + i),
                timestamp: chrono::Utc::now(),
                record: WalRecord::Delete {
                    collection: "test".to_string(),
                    primary_key: format!("key{}", i),
                },
            });
        }

        let sealed = segment.seal();
        assert_eq!(sealed.id, 0);
        assert_eq!(sealed.start_lsn, 100);
        assert_eq!(sealed.end_lsn, 104); // 100 + 5 - 1
        assert_eq!(sealed.entry_count, 5);
        assert_eq!(sealed.sealed, true);
    }
}
