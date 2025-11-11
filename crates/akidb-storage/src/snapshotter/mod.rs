//! Vector snapshot serialization for durable storage
//!
//! Provides efficient snapshot creation and restoration for vector collections.
//! Supports both JSON and Parquet formats with optional compression.
//!
//! # Architecture
//!
//! ```text
//! VectorDocument[] → JSON/Parquet → [Compression] → ObjectStore
//!                     ↓
//!              SnapshotMetadata
//! ```
//!
//! # Usage
//!
//! ```rust,no_run
//! use akidb_storage::snapshotter::{JsonSnapshotter, ParquetSnapshotter, Snapshotter, CompressionCodec, ParquetSnapshotConfig};
//! use akidb_storage::object_store::{LocalObjectStore, ObjectStore};
//! use akidb_core::{CollectionId, VectorDocument};
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> akidb_core::CoreResult<()> {
//!     let store = Arc::new(LocalObjectStore::new("./snapshots").await?);
//!
//!     // JSON snapshotter (legacy)
//!     let json_snapshotter = JsonSnapshotter::new(store.clone(), CompressionCodec::None);
//!
//!     // Parquet snapshotter (recommended)
//!     let parquet_snapshotter = ParquetSnapshotter::new(store.clone(), ParquetSnapshotConfig::default());
//!
//!     // Create snapshot
//!     let vectors = vec![/* VectorDocuments */];
//!     let collection_id = CollectionId::new();
//!     let snapshot_id = parquet_snapshotter.create_snapshot(collection_id, vectors).await?;
//!
//!     // Restore snapshot
//!     let restored = parquet_snapshotter.restore_snapshot(snapshot_id).await?;
//!
//!     Ok(())
//! }
//! ```

pub mod parquet;

use super::object_store::ObjectStore;
use akidb_core::{CollectionId, CoreError, CoreResult, VectorDocument};
use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
pub use parquet::{ParquetSnapshotConfig, ParquetSnapshotter};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// Snapshot identifier (UUID v4)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SnapshotId(Uuid);

impl SnapshotId {
    /// Create a new snapshot ID
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Get the inner UUID
    #[must_use]
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for SnapshotId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SnapshotId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Snapshot format (JSON vs Parquet)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapshotFormat {
    /// JSON format with optional compression
    Json,
    /// Parquet columnar format
    Parquet,
}

impl std::fmt::Display for SnapshotFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SnapshotFormat::Json => write!(f, "json"),
            SnapshotFormat::Parquet => write!(f, "parquet"),
        }
    }
}

/// Snapshot metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// Unique snapshot identifier
    pub snapshot_id: SnapshotId,
    /// Collection this snapshot belongs to
    pub collection_id: CollectionId,
    /// Number of vectors in snapshot
    pub vector_count: u64,
    /// Vector dimension
    pub dimension: u32,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Size in bytes (compressed)
    pub size_bytes: u64,
    /// Compression codec used
    pub compression: CompressionCodec,
    /// Snapshot format (JSON or Parquet)
    pub format: SnapshotFormat,
}

/// Compression codec for snapshot storage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionCodec {
    /// No compression
    None,
    /// Snappy compression (fast, moderate ratio)
    Snappy,
    /// Zstd compression (slower, better ratio)
    Zstd,
    /// LZ4 compression (fastest, lower ratio)
    Lz4,
}

impl std::fmt::Display for CompressionCodec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompressionCodec::None => write!(f, "none"),
            CompressionCodec::Snappy => write!(f, "snappy"),
            CompressionCodec::Zstd => write!(f, "zstd"),
            CompressionCodec::Lz4 => write!(f, "lz4"),
        }
    }
}

/// Snapshotter trait - serialize vectors to storage-efficient format
///
/// Implementations must guarantee:
/// 1. Atomicity: Snapshot creation is all-or-nothing
/// 2. Consistency: Restored vectors match original vectors
/// 3. Durability: Snapshots survive crashes
#[async_trait]
pub trait Snapshotter: Send + Sync {
    /// Create snapshot from in-memory vectors
    ///
    /// # Errors
    ///
    /// - `CoreError::ValidationError` if vectors is empty
    /// - `CoreError::StorageError` if upload fails
    async fn create_snapshot(
        &self,
        collection_id: CollectionId,
        vectors: Vec<VectorDocument>,
    ) -> CoreResult<SnapshotId>;

    /// Restore vectors from snapshot
    ///
    /// # Errors
    ///
    /// - `CoreError::NotFound` if snapshot doesn't exist
    /// - `CoreError::DeserializationError` if snapshot is corrupted
    async fn restore_snapshot(&self, snapshot_id: SnapshotId) -> CoreResult<Vec<VectorDocument>>;

    /// List all snapshots for a collection
    ///
    /// # Errors
    ///
    /// - `CoreError::StorageError` if listing fails
    async fn list_snapshots(
        &self,
        collection_id: CollectionId,
    ) -> CoreResult<Vec<SnapshotMetadata>>;

    /// Get snapshot metadata
    ///
    /// # Errors
    ///
    /// - `CoreError::NotFound` if snapshot doesn't exist
    async fn get_metadata(&self, snapshot_id: SnapshotId) -> CoreResult<SnapshotMetadata>;

    /// Delete snapshot
    ///
    /// Idempotent - no error if snapshot doesn't exist.
    ///
    /// # Errors
    ///
    /// - `CoreError::StorageError` if deletion fails
    async fn delete_snapshot(&self, snapshot_id: SnapshotId) -> CoreResult<()>;

    /// Verify snapshot integrity
    ///
    /// Returns true if both snapshot data and metadata exist.
    ///
    /// # Errors
    ///
    /// - `CoreError::StorageError` if check fails
    async fn verify_snapshot(&self, snapshot_id: SnapshotId) -> CoreResult<bool>;
}

/// JSON-based snapshotter with optional compression
///
/// Stores vectors as JSON with metadata sidecar file.
/// Suitable for datasets up to 100GB (target scale for AkiDB 2.0).
///
/// # File Format
///
/// - Snapshot: `snapshots/{snapshot_id}.json[.gz|.zst|.lz4]`
/// - Metadata: `snapshots/{snapshot_id}.meta.json`
///
/// # Future: Parquet Enhancement
///
/// Can be enhanced to use Apache Parquet for >100GB datasets with:
/// - Better compression ratio (columnar format)
/// - Faster deserialization (predicate pushdown)
/// - Schema evolution support
pub struct JsonSnapshotter {
    object_store: Arc<dyn ObjectStore>,
    compression: CompressionCodec,
}

impl JsonSnapshotter {
    /// Create a new JSON snapshotter
    pub fn new(object_store: Arc<dyn ObjectStore>, compression: CompressionCodec) -> Self {
        Self {
            object_store,
            compression,
        }
    }

    /// Get snapshot key for object store
    fn snapshot_key(&self, snapshot_id: SnapshotId) -> String {
        let extension = match self.compression {
            CompressionCodec::None => "json",
            CompressionCodec::Snappy => "json.snappy",
            CompressionCodec::Zstd => "json.zst",
            CompressionCodec::Lz4 => "json.lz4",
        };
        format!("snapshots/{}.{}", snapshot_id, extension)
    }

    /// Get metadata key
    fn metadata_key(&self, snapshot_id: SnapshotId) -> String {
        format!("snapshots/{}.meta.json", snapshot_id)
    }

    /// Compress data according to compression codec
    fn compress(&self, data: Vec<u8>) -> CoreResult<Vec<u8>> {
        match self.compression {
            CompressionCodec::None => Ok(data),
            // Note: For Week 3, we implement None compression
            // Snappy/Zstd/Lz4 can be added in Week 5 polish phase
            _ => Err(CoreError::ValidationError(format!(
                "Compression codec {:?} not yet implemented",
                self.compression
            ))),
        }
    }

    /// Decompress data according to compression codec
    fn decompress(&self, data: Vec<u8>) -> CoreResult<Vec<u8>> {
        match self.compression {
            CompressionCodec::None => Ok(data),
            // Note: For Week 3, we implement None compression
            // Snappy/Zstd/Lz4 can be added in Week 5 polish phase
            _ => Err(CoreError::ValidationError(format!(
                "Compression codec {:?} not yet implemented",
                self.compression
            ))),
        }
    }
}

#[async_trait]
impl Snapshotter for JsonSnapshotter {
    async fn create_snapshot(
        &self,
        collection_id: CollectionId,
        vectors: Vec<VectorDocument>,
    ) -> CoreResult<SnapshotId> {
        if vectors.is_empty() {
            return Err(CoreError::ValidationError(
                "Cannot snapshot empty collection".to_string(),
            ));
        }

        let snapshot_id = SnapshotId::new();
        let dimension = vectors[0].vector.len() as u32;

        // Serialize to JSON
        let json_data = serde_json::to_vec(&vectors)?;

        // Compress
        let compressed_data = self.compress(json_data)?;
        let size_bytes = compressed_data.len() as u64;

        // Upload to object store
        let snapshot_key = self.snapshot_key(snapshot_id);
        self.object_store
            .put(&snapshot_key, Bytes::from(compressed_data))
            .await?;

        // Save metadata
        let metadata = SnapshotMetadata {
            snapshot_id,
            collection_id,
            vector_count: vectors.len() as u64,
            dimension,
            created_at: Utc::now(),
            size_bytes,
            compression: self.compression,
            format: SnapshotFormat::Json,
        };

        let metadata_json = serde_json::to_vec(&metadata)?;
        let metadata_key = self.metadata_key(snapshot_id);
        self.object_store
            .put(&metadata_key, Bytes::from(metadata_json))
            .await?;

        Ok(snapshot_id)
    }

    async fn restore_snapshot(&self, snapshot_id: SnapshotId) -> CoreResult<Vec<VectorDocument>> {
        // Download from object store
        let snapshot_key = self.snapshot_key(snapshot_id);
        let compressed_data = self.object_store.get(&snapshot_key).await?;

        // Decompress
        let json_data = self.decompress(compressed_data.to_vec())?;

        // Deserialize
        let vectors: Vec<VectorDocument> = serde_json::from_slice(&json_data)?;

        Ok(vectors)
    }

    async fn list_snapshots(
        &self,
        collection_id: CollectionId,
    ) -> CoreResult<Vec<SnapshotMetadata>> {
        let prefix = "snapshots/";
        let objects = self.object_store.list(prefix).await?;

        let mut snapshots = Vec::new();
        for obj in objects {
            if obj.key.ends_with(".meta.json") {
                let data = self.object_store.get(&obj.key).await?;
                if let Ok(metadata) = serde_json::from_slice::<SnapshotMetadata>(&data) {
                    if metadata.collection_id == collection_id {
                        snapshots.push(metadata);
                    }
                }
            }
        }

        // Sort by creation time (newest first)
        snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(snapshots)
    }

    async fn get_metadata(&self, snapshot_id: SnapshotId) -> CoreResult<SnapshotMetadata> {
        let metadata_key = self.metadata_key(snapshot_id);
        let data = self.object_store.get(&metadata_key).await?;
        let metadata = serde_json::from_slice(&data)?;
        Ok(metadata)
    }

    async fn delete_snapshot(&self, snapshot_id: SnapshotId) -> CoreResult<()> {
        let snapshot_key = self.snapshot_key(snapshot_id);
        let metadata_key = self.metadata_key(snapshot_id);

        // Delete both files (idempotent)
        self.object_store.delete(&snapshot_key).await?;
        self.object_store.delete(&metadata_key).await?;

        Ok(())
    }

    async fn verify_snapshot(&self, snapshot_id: SnapshotId) -> CoreResult<bool> {
        let snapshot_key = self.snapshot_key(snapshot_id);
        let metadata_key = self.metadata_key(snapshot_id);

        let snapshot_exists = self.object_store.exists(&snapshot_key).await?;
        let metadata_exists = self.object_store.exists(&metadata_key).await?;

        Ok(snapshot_exists && metadata_exists)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object_store::LocalObjectStore;
    use akidb_core::DocumentId;
    use tempfile::TempDir;

    fn create_test_vectors(count: usize, dimension: usize) -> Vec<VectorDocument> {
        (0..count)
            .map(|i| VectorDocument {
                doc_id: DocumentId::new(),
                external_id: Some(format!("doc-{}", i)),
                vector: vec![i as f32; dimension],
                metadata: Some(serde_json::json!({"index": i})),
                inserted_at: Utc::now(),
            })
            .collect()
    }

    #[tokio::test]
    async fn test_snapshot_id_creation() {
        let id1 = SnapshotId::new();
        let id2 = SnapshotId::new();
        assert_ne!(id1, id2);
    }

    #[tokio::test]
    async fn test_compression_codec_display() {
        assert_eq!(CompressionCodec::None.to_string(), "none");
        assert_eq!(CompressionCodec::Snappy.to_string(), "snappy");
        assert_eq!(CompressionCodec::Zstd.to_string(), "zstd");
        assert_eq!(CompressionCodec::Lz4.to_string(), "lz4");
    }

    #[tokio::test]
    async fn test_create_and_restore_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(LocalObjectStore::new(temp_dir.path()).await.unwrap());
        let snapshotter = JsonSnapshotter::new(store, CompressionCodec::None);

        let vectors = create_test_vectors(10, 128);
        let collection_id = CollectionId::new();

        // Create snapshot
        let snapshot_id = snapshotter
            .create_snapshot(collection_id, vectors.clone())
            .await
            .unwrap();

        // Restore snapshot
        let restored = snapshotter.restore_snapshot(snapshot_id).await.unwrap();

        assert_eq!(restored.len(), vectors.len());
        for (original, restored) in vectors.iter().zip(restored.iter()) {
            assert_eq!(original.doc_id, restored.doc_id);
            assert_eq!(original.external_id, restored.external_id);
            assert_eq!(original.vector, restored.vector);
        }
    }

    #[tokio::test]
    async fn test_snapshot_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(LocalObjectStore::new(temp_dir.path()).await.unwrap());
        let snapshotter = JsonSnapshotter::new(store, CompressionCodec::None);

        let vectors = create_test_vectors(5, 64);
        let collection_id = CollectionId::new();

        let snapshot_id = snapshotter
            .create_snapshot(collection_id, vectors.clone())
            .await
            .unwrap();

        let metadata = snapshotter.get_metadata(snapshot_id).await.unwrap();

        assert_eq!(metadata.snapshot_id, snapshot_id);
        assert_eq!(metadata.collection_id, collection_id);
        assert_eq!(metadata.vector_count, 5);
        assert_eq!(metadata.dimension, 64);
        assert!(metadata.size_bytes > 0);
    }

    #[tokio::test]
    async fn test_list_snapshots() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(LocalObjectStore::new(temp_dir.path()).await.unwrap());
        let snapshotter = JsonSnapshotter::new(store, CompressionCodec::None);

        let collection1 = CollectionId::new();
        let collection2 = CollectionId::new();

        // Create snapshots for two collections
        let vectors = create_test_vectors(3, 32);
        snapshotter
            .create_snapshot(collection1, vectors.clone())
            .await
            .unwrap();
        snapshotter
            .create_snapshot(collection1, vectors.clone())
            .await
            .unwrap();
        snapshotter
            .create_snapshot(collection2, vectors.clone())
            .await
            .unwrap();

        // List snapshots for collection1
        let snapshots = snapshotter.list_snapshots(collection1).await.unwrap();
        assert_eq!(snapshots.len(), 2);

        // List snapshots for collection2
        let snapshots = snapshotter.list_snapshots(collection2).await.unwrap();
        assert_eq!(snapshots.len(), 1);
    }

    #[tokio::test]
    async fn test_delete_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(LocalObjectStore::new(temp_dir.path()).await.unwrap());
        let snapshotter = JsonSnapshotter::new(store, CompressionCodec::None);

        let vectors = create_test_vectors(2, 16);
        let collection_id = CollectionId::new();

        let snapshot_id = snapshotter
            .create_snapshot(collection_id, vectors)
            .await
            .unwrap();

        // Verify exists
        assert!(snapshotter.verify_snapshot(snapshot_id).await.unwrap());

        // Delete
        snapshotter.delete_snapshot(snapshot_id).await.unwrap();

        // Verify deleted
        assert!(!snapshotter.verify_snapshot(snapshot_id).await.unwrap());

        // Idempotent delete
        snapshotter.delete_snapshot(snapshot_id).await.unwrap();
    }

    #[tokio::test]
    async fn test_verify_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(LocalObjectStore::new(temp_dir.path()).await.unwrap());
        let snapshotter = JsonSnapshotter::new(store, CompressionCodec::None);

        let vectors = create_test_vectors(1, 8);
        let collection_id = CollectionId::new();

        let snapshot_id = snapshotter
            .create_snapshot(collection_id, vectors)
            .await
            .unwrap();

        // Should exist
        assert!(snapshotter.verify_snapshot(snapshot_id).await.unwrap());

        // Non-existent snapshot
        let fake_id = SnapshotId::new();
        assert!(!snapshotter.verify_snapshot(fake_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_empty_vectors_error() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(LocalObjectStore::new(temp_dir.path()).await.unwrap());
        let snapshotter = JsonSnapshotter::new(store, CompressionCodec::None);

        let collection_id = CollectionId::new();
        let result = snapshotter.create_snapshot(collection_id, vec![]).await;

        assert!(matches!(result, Err(CoreError::ValidationError(_))));
    }

    #[tokio::test]
    async fn test_restore_nonexistent_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(LocalObjectStore::new(temp_dir.path()).await.unwrap());
        let snapshotter = JsonSnapshotter::new(store, CompressionCodec::None);

        let fake_id = SnapshotId::new();
        let result = snapshotter.restore_snapshot(fake_id).await;

        assert!(matches!(result, Err(CoreError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_large_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(LocalObjectStore::new(temp_dir.path()).await.unwrap());
        let snapshotter = JsonSnapshotter::new(store, CompressionCodec::None);

        // Create larger snapshot (1000 vectors × 256 dimensions)
        let vectors = create_test_vectors(1000, 256);
        let collection_id = CollectionId::new();

        let snapshot_id = snapshotter
            .create_snapshot(collection_id, vectors.clone())
            .await
            .unwrap();

        let metadata = snapshotter.get_metadata(snapshot_id).await.unwrap();
        assert_eq!(metadata.vector_count, 1000);
        assert_eq!(metadata.dimension, 256);

        // Restore should work
        let restored = snapshotter.restore_snapshot(snapshot_id).await.unwrap();
        assert_eq!(restored.len(), 1000);
    }
}
