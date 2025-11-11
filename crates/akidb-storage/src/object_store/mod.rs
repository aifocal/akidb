// ! Object Store abstraction for S3-compatible storage
//!
//! Provides a unified interface for object storage operations with multiple backends:
//! - AWS S3 (production)
//! - MinIO (S3-compatible)
//! - Local filesystem (testing)

mod local;
mod mock;
mod s3;

pub use local::LocalObjectStore;
pub use mock::{CallHistoryEntry, MockFailure, MockS3Config, MockS3ObjectStore};
pub use s3::{S3Config, S3ObjectStore};

use akidb_core::CoreResult;
use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Object metadata returned by list/head operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectMetadata {
    /// Object key (path)
    pub key: String,
    /// Size in bytes
    pub size_bytes: u64,
    /// Last modification timestamp
    pub last_modified: DateTime<Utc>,
    /// ETag (S3 entity tag, None for local storage)
    pub etag: Option<String>,
}

/// Object Store trait - S3-like interface for cloud/local storage
///
/// All implementations must be thread-safe (Send + Sync) and support
/// concurrent operations. Keys are UTF-8 strings treated as opaque identifiers.
///
/// # Error Handling
///
/// All methods return `CoreResult<T>` with the following semantics:
/// - `CoreError::NotFound` - Object does not exist (get, head, delete is idempotent)
/// - `CoreError::StorageError` - Backend-specific error (network, permissions, etc.)
/// - `CoreError::IoError` - I/O error (local filesystem only)
///
/// # Examples
///
/// ```rust,no_run
/// use akidb_storage::object_store::{LocalObjectStore, ObjectStore};
/// use bytes::Bytes;
///
/// #[tokio::main]
/// async fn main() -> akidb_core::CoreResult<()> {
///     let store = LocalObjectStore::new("./data").await?;
///
///     // Put object
///     let data = Bytes::from("Hello, World!");
///     store.put("test.txt", data).await?;
///
///     // Get object
///     let retrieved = store.get("test.txt").await?;
///     assert_eq!(retrieved, Bytes::from("Hello, World!"));
///
///     // List objects
///     let objects = store.list("").await?;
///     assert_eq!(objects.len(), 1);
///
///     Ok(())
/// }
/// ```
#[async_trait]
pub trait ObjectStore: Send + Sync {
    /// Put object (overwrites if exists)
    ///
    /// Stores the data at the given key. If an object already exists at that key,
    /// it will be overwritten.
    ///
    /// # Errors
    ///
    /// - `CoreError::StorageError` if the operation fails
    /// - `CoreError::ValidationError` if key is empty
    async fn put(&self, key: &str, data: Bytes) -> CoreResult<()>;

    /// Get object
    ///
    /// Retrieves the complete object data.
    ///
    /// # Errors
    ///
    /// - `CoreError::NotFound` if object does not exist
    /// - `CoreError::StorageError` if the operation fails
    async fn get(&self, key: &str) -> CoreResult<Bytes>;

    /// Check if object exists
    ///
    /// Returns true if the object exists, false otherwise.
    ///
    /// # Errors
    ///
    /// - `CoreError::StorageError` if the operation fails
    async fn exists(&self, key: &str) -> CoreResult<bool>;

    /// Delete object (idempotent)
    ///
    /// Deletes the object. If the object does not exist, this is a no-op.
    ///
    /// # Errors
    ///
    /// - `CoreError::StorageError` if the operation fails
    async fn delete(&self, key: &str) -> CoreResult<()>;

    /// List objects with prefix
    ///
    /// Returns metadata for all objects whose keys start with the given prefix.
    /// Use an empty string to list all objects.
    ///
    /// # Errors
    ///
    /// - `CoreError::StorageError` if the operation fails
    async fn list(&self, prefix: &str) -> CoreResult<Vec<ObjectMetadata>>;

    /// Get object metadata without downloading
    ///
    /// Returns metadata for a single object without transferring the data.
    ///
    /// # Errors
    ///
    /// - `CoreError::NotFound` if object does not exist
    /// - `CoreError::StorageError` if the operation fails
    async fn head(&self, key: &str) -> CoreResult<ObjectMetadata>;

    /// Copy object within same storage backend
    ///
    /// Efficiently copies an object from `from_key` to `to_key` without
    /// downloading and re-uploading the data.
    ///
    /// # Errors
    ///
    /// - `CoreError::NotFound` if source object does not exist
    /// - `CoreError::StorageError` if the operation fails
    async fn copy(&self, from_key: &str, to_key: &str) -> CoreResult<()>;

    /// Multipart upload (for large objects >5MB)
    ///
    /// Uploads data in multiple parts. For objects <5GB, implementations may
    /// concatenate parts and use a single upload.
    ///
    /// # Errors
    ///
    /// - `CoreError::StorageError` if the operation fails
    /// - `CoreError::ValidationError` if total size exceeds backend limits
    async fn put_multipart(&self, key: &str, parts: Vec<Bytes>) -> CoreResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_metadata_serialization() {
        let metadata = ObjectMetadata {
            key: "test/file.txt".to_string(),
            size_bytes: 1024,
            last_modified: Utc::now(),
            etag: Some("abc123".to_string()),
        };

        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: ObjectMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.key, metadata.key);
        assert_eq!(deserialized.size_bytes, metadata.size_bytes);
    }
}
