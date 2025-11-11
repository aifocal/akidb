//! Dead Letter Queue (DLQ) - Persistent queue for permanently failed operations
//!
//! The DLQ stores operations that have exhausted all retry attempts and need manual intervention.
//! It provides:
//! - Size limit enforcement with FIFO eviction
//! - TTL-based expiration
//! - Persistence to disk
//! - Background cleanup
//! - Comprehensive metrics

use akidb_core::{CollectionId, CoreResult, DocumentId};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

/// DLQ configuration
#[derive(Debug, Clone)]
pub struct DLQConfig {
    /// Maximum number of entries (default: 10,000)
    pub max_size: usize,
    /// Time-to-live in seconds (default: 604,800 = 7 days)
    pub ttl_seconds: i64,
    /// Persistence path for saving DLQ to disk (default: /tmp/akidb-dlq)
    pub persistence_path: PathBuf,
    /// Cleanup interval in seconds (default: 3,600 = 1 hour)
    pub cleanup_interval_seconds: u64,
}

impl Default for DLQConfig {
    fn default() -> Self {
        Self {
            max_size: 10_000,
            ttl_seconds: 604_800, // 7 days
            persistence_path: PathBuf::from("/tmp/akidb-dlq/dlq.json"),
            cleanup_interval_seconds: 3600, // 1 hour
        }
    }
}

/// DLQ entry with TTL and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DLQEntry {
    /// Unique entry ID
    pub id: Uuid,
    /// Document ID that failed
    pub document_id: DocumentId,
    /// Collection ID
    pub collection_id: CollectionId,
    /// Error message from last failure
    pub error_message: String,
    /// Number of retry attempts before DLQ
    pub retry_count: u32,
    /// When the entry was created
    pub created_at: DateTime<Utc>,
    /// When the entry expires
    pub expires_at: DateTime<Utc>,
    /// Raw data (serialized vector document)
    pub data: Vec<u8>,
}

impl DLQEntry {
    /// Create a new DLQ entry
    #[must_use]
    pub fn new(
        document_id: DocumentId,
        collection_id: CollectionId,
        error_message: String,
        data: Vec<u8>,
        ttl_seconds: i64,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            document_id,
            collection_id,
            error_message,
            retry_count: 0,
            created_at: now,
            expires_at: now + ChronoDuration::seconds(ttl_seconds),
            data,
        }
    }

    /// Check if entry has expired
    #[must_use]
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Get age of entry in seconds
    #[must_use]
    pub fn age(&self) -> ChronoDuration {
        Utc::now() - self.created_at
    }
}

/// DLQ metrics for monitoring
#[derive(Debug, Clone, Default)]
pub struct DLQMetrics {
    /// Current queue size
    pub size: usize,
    /// Age of oldest entry in seconds
    pub oldest_entry_age_seconds: i64,
    /// Total number of entries evicted due to size limit
    pub total_evictions: u64,
    /// Total number of expired entries removed
    pub total_expired: u64,
}

/// Dead Letter Queue for permanently failed operations
///
/// # Examples
///
/// ```rust,no_run
/// use akidb_storage::dlq::{DeadLetterQueue, DLQConfig, DLQEntry};
/// use akidb_core::{DocumentId, CollectionId};
///
/// #[tokio::main]
/// async fn main() {
///     let config = DLQConfig::default();
///     let dlq = DeadLetterQueue::new(config);
///
///     // Add failed operation
///     let entry = DLQEntry::new(
///         DocumentId::new(),
///         CollectionId::new(),
///         "S3 upload failed after 5 retries".to_string(),
///         vec![1, 2, 3],
///         604_800, // 7 days TTL
///     );
///
///     dlq.add_entry(entry).await.unwrap();
///
///     // Check metrics
///     let metrics = dlq.metrics();
///     println!("DLQ size: {}", metrics.size);
/// }
/// ```
pub struct DeadLetterQueue {
    entries: Arc<RwLock<VecDeque<DLQEntry>>>,
    config: DLQConfig,
    metrics: Arc<RwLock<DLQMetrics>>,
}

impl DeadLetterQueue {
    /// Create a new Dead Letter Queue
    #[must_use]
    pub fn new(config: DLQConfig) -> Self {
        Self {
            entries: Arc::new(RwLock::new(VecDeque::new())),
            config,
            metrics: Arc::new(RwLock::new(DLQMetrics::default())),
        }
    }

    /// Add entry with size limit enforcement (FIFO eviction)
    ///
    /// If the queue is at max capacity, the oldest entry will be evicted.
    ///
    /// # Errors
    ///
    /// Currently doesn't return errors, but signature kept for future persistence failures
    pub async fn add_entry(&self, entry: DLQEntry) -> CoreResult<()> {
        let mut entries = self.entries.write();

        // Enforce size limit (FIFO eviction)
        if entries.len() >= self.config.max_size {
            entries.pop_front(); // Evict oldest
            let mut metrics = self.metrics.write();
            metrics.total_evictions += 1;
            tracing::debug!("DLQ size limit reached, evicted oldest entry");
        }

        entries.push_back(entry);

        // Update metrics
        let mut metrics = self.metrics.write();
        metrics.size = entries.len();

        Ok(())
    }

    /// Get entry by ID
    #[must_use]
    pub fn get_entry(&self, id: &Uuid) -> Option<DLQEntry> {
        self.entries.read().iter().find(|e| &e.id == id).cloned()
    }

    /// Remove entry by ID
    pub fn remove_entry(&self, id: &Uuid) {
        let mut entries = self.entries.write();
        if let Some(pos) = entries.iter().position(|e| &e.id == id) {
            entries.remove(pos);
        }

        // Update metrics
        let mut metrics = self.metrics.write();
        metrics.size = entries.len();
    }

    /// Get current queue size
    #[must_use]
    pub fn size(&self) -> usize {
        self.entries.read().len()
    }

    /// Check if queue is full
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.size() >= self.config.max_size
    }

    /// Get age of oldest entry
    #[must_use]
    pub fn oldest_entry_age(&self) -> Option<ChronoDuration> {
        self.entries.read().front().map(DLQEntry::age)
    }

    /// Cleanup expired entries
    ///
    /// Returns the number of expired entries removed
    ///
    /// # Errors
    ///
    /// Currently doesn't return errors, but signature kept for future persistence failures
    pub async fn cleanup_expired(&self) -> CoreResult<usize> {
        let mut entries = self.entries.write();
        let initial_size = entries.len();

        entries.retain(|entry| !entry.is_expired());

        let expired_count = initial_size - entries.len();

        // Update metrics
        let mut metrics = self.metrics.write();
        metrics.size = entries.len();
        metrics.total_expired += expired_count as u64;

        if expired_count > 0 {
            tracing::info!("DLQ cleanup: removed {} expired entries", expired_count);
        }

        Ok(expired_count)
    }

    /// Persist DLQ to disk
    ///
    /// # Errors
    ///
    /// Returns error if file I/O fails or serialization fails
    pub async fn persist(&self) -> CoreResult<()> {
        // Clone entries to avoid holding lock across await
        let entries_clone = {
            let entries = self.entries.read();
            entries.clone()
        };

        // Create parent directory if needed
        if let Some(parent) = self.config.persistence_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                akidb_core::CoreError::StorageError(format!(
                    "Failed to create DLQ directory: {}",
                    e
                ))
            })?;
        }

        // Serialize entries to JSON
        let json = serde_json::to_string_pretty(&entries_clone).map_err(|e| {
            akidb_core::CoreError::StorageError(format!("Failed to serialize DLQ: {}", e))
        })?;

        // Write to file
        tokio::fs::write(&self.config.persistence_path, json)
            .await
            .map_err(|e| {
                akidb_core::CoreError::StorageError(format!("Failed to write DLQ file: {}", e))
            })?;

        tracing::debug!("DLQ persisted: {} entries", entries_clone.len());

        Ok(())
    }

    /// Load DLQ from disk
    ///
    /// # Errors
    ///
    /// Returns error if file I/O fails or deserialization fails
    pub async fn load_from_disk(&self) -> CoreResult<()> {
        if !self.config.persistence_path.exists() {
            tracing::debug!("DLQ persistence file not found, starting with empty queue");
            return Ok(());
        }

        // Read file
        let json = tokio::fs::read_to_string(&self.config.persistence_path)
            .await
            .map_err(|e| {
                akidb_core::CoreError::StorageError(format!("Failed to read DLQ file: {}", e))
            })?;

        // Deserialize
        let loaded_entries: VecDeque<DLQEntry> = serde_json::from_str(&json).map_err(|e| {
            akidb_core::CoreError::StorageError(format!("Failed to deserialize DLQ: {}", e))
        })?;

        // Filter out expired entries during load
        let valid_entries: VecDeque<DLQEntry> = loaded_entries
            .into_iter()
            .filter(|e| !e.is_expired())
            .collect();

        let loaded_count = valid_entries.len();

        // Update entries
        *self.entries.write() = valid_entries;

        // Update metrics
        let mut metrics = self.metrics.write();
        metrics.size = loaded_count;

        tracing::info!("DLQ loaded from disk: {} entries", loaded_count);

        Ok(())
    }

    /// Get current metrics
    #[must_use]
    pub fn metrics(&self) -> DLQMetrics {
        let entries = self.entries.read();
        let mut metrics = self.metrics.read().clone();

        metrics.size = entries.len();
        metrics.oldest_entry_age_seconds =
            entries.front().map(|e| e.age().num_seconds()).unwrap_or(0);

        metrics
    }

    /// Get all entries (for inspection/debugging)
    #[must_use]
    pub fn all_entries(&self) -> Vec<DLQEntry> {
        self.entries.read().iter().cloned().collect()
    }

    /// Clear all entries (admin operation)
    pub fn clear(&self) {
        self.entries.write().clear();
        let mut metrics = self.metrics.write();
        metrics.size = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dlq_add_and_get() {
        let config = DLQConfig::default();
        let dlq = DeadLetterQueue::new(config);

        let entry = DLQEntry::new(
            DocumentId::new(),
            CollectionId::new(),
            "test error".to_string(),
            vec![1, 2, 3],
            604_800,
        );

        let entry_id = entry.id;

        dlq.add_entry(entry).await.unwrap();

        assert_eq!(dlq.size(), 1);

        let retrieved = dlq.get_entry(&entry_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().error_message, "test error");
    }

    #[tokio::test]
    async fn test_dlq_size_limit_enforcement() {
        let config = DLQConfig {
            max_size: 5,
            ..Default::default()
        };
        let dlq = DeadLetterQueue::new(config);

        // Add 10 entries (exceeds max_size of 5)
        for i in 0..10 {
            let entry = DLQEntry::new(
                DocumentId::new(),
                CollectionId::new(),
                format!("error_{}", i),
                vec![i as u8],
                604_800,
            );
            dlq.add_entry(entry).await.unwrap();
        }

        // Should have exactly 5 entries (oldest 5 evicted)
        assert_eq!(dlq.size(), 5);

        // Verify metrics
        let metrics = dlq.metrics();
        assert_eq!(metrics.size, 5);
        assert_eq!(metrics.total_evictions, 5);
    }

    #[tokio::test]
    async fn test_dlq_ttl_expiration() {
        let config = DLQConfig {
            ttl_seconds: 1, // 1 second TTL
            ..Default::default()
        };
        let dlq = DeadLetterQueue::new(config);

        // Add entry with 1-second TTL
        let entry = DLQEntry::new(
            DocumentId::new(),
            CollectionId::new(),
            "test error".to_string(),
            vec![1, 2, 3],
            1, // 1 second TTL
        );
        dlq.add_entry(entry).await.unwrap();

        assert_eq!(dlq.size(), 1);

        // Wait for expiration
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Cleanup expired entries
        let expired_count = dlq.cleanup_expired().await.unwrap();
        assert_eq!(expired_count, 1);
        assert_eq!(dlq.size(), 0);

        // Verify metrics
        let metrics = dlq.metrics();
        assert_eq!(metrics.total_expired, 1);
    }

    #[tokio::test]
    async fn test_dlq_remove_entry() {
        let config = DLQConfig::default();
        let dlq = DeadLetterQueue::new(config);

        let entry = DLQEntry::new(
            DocumentId::new(),
            CollectionId::new(),
            "test error".to_string(),
            vec![1, 2, 3],
            604_800,
        );

        let entry_id = entry.id;

        dlq.add_entry(entry).await.unwrap();
        assert_eq!(dlq.size(), 1);

        dlq.remove_entry(&entry_id);
        assert_eq!(dlq.size(), 0);
    }
}
