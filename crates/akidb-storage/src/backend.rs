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

    /// Write a segment with vector data and metadata (for payload persistence)
    ///
    /// This method allows writing segments directly from vector data and metadata,
    /// enabling end-to-end payload persistence. Default implementation returns
    /// an error - backends that support this feature should override.
    async fn write_segment_with_data(
        &self,
        _descriptor: &SegmentDescriptor,
        _vectors: Vec<Vec<f32>>,
        _metadata: Option<crate::metadata::MetadataBlock>,
    ) -> Result<()> {
        Err(akidb_core::Error::NotImplemented(
            "write_segment_with_data not implemented for this backend".to_string(),
        ))
    }

    /// Load a segment with vectors and metadata from storage
    ///
    /// This method loads a complete segment including vectors and optional metadata,
    /// enabling collection recovery on restart. Default implementation returns
    /// an error - backends that support this feature should override.
    async fn load_segment(
        &self,
        _collection: &str,
        _segment_id: Uuid,
    ) -> Result<crate::segment_format::SegmentData> {
        Err(akidb_core::Error::NotImplemented(
            "load_segment not implemented for this backend".to_string(),
        ))
    }
}
