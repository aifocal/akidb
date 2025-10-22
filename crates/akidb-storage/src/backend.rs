use async_trait::async_trait;
use bytes::Bytes;
use uuid::Uuid;

use akidb_core::{CollectionDescriptor, CollectionManifest, SegmentDescriptor};

use crate::error::Result;

/// High level status reported by the storage subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageStatus {
    Healthy,
    Degraded,
}

/// Abstraction over persistence layer implementations (local FS, S3, etc.).
#[async_trait]
pub trait StorageBackend: Send + Sync {
    // === Collection & Segment Operations ===
    async fn status(&self) -> Result<StorageStatus>;
    async fn create_collection(&self, descriptor: &CollectionDescriptor) -> Result<()>;
    async fn drop_collection(&self, name: &str) -> Result<()>;
    async fn write_segment(&self, descriptor: &SegmentDescriptor) -> Result<()>;
    async fn seal_segment(&self, segment_id: Uuid) -> Result<SegmentDescriptor>;
    async fn load_manifest(&self, collection: &str) -> Result<CollectionManifest>;
    async fn persist_manifest(&self, manifest: &CollectionManifest) -> Result<()>;

    // === Generic Object Operations (for WAL, snapshots, etc.) ===

    /// Get an object by key. Returns error if object not found.
    async fn get_object(&self, key: &str) -> Result<Bytes>;

    /// Put an object with the given key and data.
    async fn put_object(&self, key: &str, data: Bytes) -> Result<()>;

    /// Delete an object by key. Succeeds even if object doesn't exist.
    async fn delete_object(&self, key: &str) -> Result<()>;

    /// Check if an object exists without reading its contents.
    async fn object_exists(&self, key: &str) -> Result<bool>;

    /// List all objects with the given prefix.
    async fn list_objects(&self, prefix: &str) -> Result<Vec<String>>;
}
