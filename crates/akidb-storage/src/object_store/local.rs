//! Local filesystem implementation of ObjectStore
//!
//! Provides a local directory-based object store for testing and development.
//! Objects are stored as files with the key as the relative path.

use super::{ObjectMetadata, ObjectStore};
use akidb_core::{CoreError, CoreResult};
use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Local filesystem object store
///
/// Stores objects as files in a base directory. Keys are treated as relative paths
/// within the base directory.
///
/// # Example
///
/// ```rust,no_run
/// use akidb_storage::object_store::{LocalObjectStore, ObjectStore};
/// use bytes::Bytes;
///
/// #[tokio::main]
/// async fn main() -> akidb_core::CoreResult<()> {
///     // Create store in ./test-data directory
///     let store = LocalObjectStore::new("./test-data").await?;
///
///     // Put nested object (creates directories automatically)
///     store.put("snapshots/2024/data.bin", Bytes::from("test")).await?;
///
///     // List all objects in snapshots/ directory
///     let objects = store.list("snapshots/").await?;
///     println!("Found {} objects", objects.len());
///
///     Ok(())
/// }
/// ```
pub struct LocalObjectStore {
    base_dir: PathBuf,
}

impl LocalObjectStore {
    /// Create a new local object store
    ///
    /// Creates the base directory if it doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns `CoreError::IoError` if directory creation fails
    pub async fn new(base_dir: impl AsRef<Path>) -> CoreResult<Self> {
        let base_dir = base_dir.as_ref().to_path_buf();
        tokio::fs::create_dir_all(&base_dir).await?;
        Ok(Self { base_dir })
    }

    /// Convert key to full filesystem path
    fn full_path(&self, key: &str) -> PathBuf {
        self.base_dir.join(key)
    }

    /// Strip base directory from path to get key
    fn path_to_key(&self, path: &Path) -> Option<String> {
        path.strip_prefix(&self.base_dir)
            .ok()
            .and_then(|p| p.to_str())
            .map(|s| s.to_string())
    }

    /// Recursively list all files under a directory
    fn list_recursive<'a>(
        &'a self,
        dir: &'a Path,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = CoreResult<Vec<PathBuf>>> + Send + 'a>>
    {
        Box::pin(async move {
            let mut results = Vec::new();

            let mut read_dir = tokio::fs::read_dir(dir).await?;
            while let Some(entry) = read_dir.next_entry().await? {
                let path = entry.path();
                let metadata = entry.metadata().await?;

                if metadata.is_file() {
                    results.push(path);
                } else if metadata.is_dir() {
                    // Recurse into subdirectory
                    let mut sub_results = self.list_recursive(&path).await?;
                    results.append(&mut sub_results);
                }
            }

            Ok(results)
        })
    }
}

#[async_trait]
impl ObjectStore for LocalObjectStore {
    async fn put(&self, key: &str, data: Bytes) -> CoreResult<()> {
        if key.is_empty() {
            return Err(CoreError::ValidationError(
                "Key cannot be empty".to_string(),
            ));
        }

        let path = self.full_path(key);

        // Create parent directories
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Write data
        tokio::fs::write(&path, &data).await?;

        Ok(())
    }

    async fn get(&self, key: &str) -> CoreResult<Bytes> {
        let path = self.full_path(key);

        let data = tokio::fs::read(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                CoreError::not_found("object", key)
            } else {
                CoreError::from(e)
            }
        })?;

        Ok(Bytes::from(data))
    }

    async fn exists(&self, key: &str) -> CoreResult<bool> {
        let path = self.full_path(key);
        Ok(tokio::fs::try_exists(&path).await.unwrap_or(false))
    }

    async fn delete(&self, key: &str) -> CoreResult<()> {
        let path = self.full_path(key);

        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            tokio::fs::remove_file(&path).await?;
        }

        // Idempotent - no error if file doesn't exist
        Ok(())
    }

    async fn list(&self, prefix: &str) -> CoreResult<Vec<ObjectMetadata>> {
        let prefix_path = self.full_path(prefix);

        // Check if prefix path exists
        if !tokio::fs::try_exists(&prefix_path).await.unwrap_or(false) {
            return Ok(Vec::new());
        }

        let metadata_check = tokio::fs::metadata(&prefix_path).await?;

        let files = if metadata_check.is_file() {
            // If prefix is a file, return just that file
            vec![prefix_path]
        } else if metadata_check.is_dir() {
            // If prefix is a directory, list recursively
            self.list_recursive(&prefix_path).await?
        } else {
            vec![]
        };

        let mut results = Vec::new();
        for path in files {
            if let Ok(metadata) = tokio::fs::metadata(&path).await {
                if let Some(key) = self.path_to_key(&path) {
                    results.push(ObjectMetadata {
                        key,
                        size_bytes: metadata.len(),
                        last_modified: metadata
                            .modified()
                            .ok()
                            .and_then(|t| {
                                t.duration_since(UNIX_EPOCH)
                                    .ok()
                                    .and_then(|d| DateTime::from_timestamp(d.as_secs() as i64, 0))
                            })
                            .unwrap_or_else(Utc::now),
                        etag: None,
                    });
                }
            }
        }

        Ok(results)
    }

    async fn head(&self, key: &str) -> CoreResult<ObjectMetadata> {
        let path = self.full_path(key);

        let metadata = tokio::fs::metadata(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                CoreError::not_found("object", key)
            } else {
                CoreError::from(e)
            }
        })?;

        Ok(ObjectMetadata {
            key: key.to_string(),
            size_bytes: metadata.len(),
            last_modified: metadata
                .modified()
                .ok()
                .and_then(|t| {
                    t.duration_since(UNIX_EPOCH)
                        .ok()
                        .and_then(|d| DateTime::from_timestamp(d.as_secs() as i64, 0))
                })
                .unwrap_or_else(Utc::now),
            etag: None,
        })
    }

    async fn copy(&self, from_key: &str, to_key: &str) -> CoreResult<()> {
        let from_path = self.full_path(from_key);
        let to_path = self.full_path(to_key);

        // Check if source exists
        if !tokio::fs::try_exists(&from_path).await.unwrap_or(false) {
            return Err(CoreError::not_found("object", from_key));
        }

        // Create parent directories for destination
        if let Some(parent) = to_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Copy file
        tokio::fs::copy(&from_path, &to_path).await?;

        Ok(())
    }

    async fn put_multipart(&self, key: &str, parts: Vec<Bytes>) -> CoreResult<()> {
        // For local storage, just concatenate parts and use single put
        let total_size: usize = parts.iter().map(|p| p.len()).sum();
        let mut combined = Vec::with_capacity(total_size);

        for part in parts {
            combined.extend_from_slice(&part);
        }

        self.put(key, Bytes::from(combined)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_local_store_creation() {
        let temp_dir = TempDir::new().unwrap();
        let _store = LocalObjectStore::new(temp_dir.path()).await.unwrap();
        assert!(temp_dir.path().exists());
    }

    #[tokio::test]
    async fn test_local_store_put_get() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalObjectStore::new(temp_dir.path()).await.unwrap();

        let data = Bytes::from("Hello, World!");
        store.put("test.txt", data.clone()).await.unwrap();

        let retrieved = store.get("test.txt").await.unwrap();
        assert_eq!(retrieved, data);
    }

    #[tokio::test]
    async fn test_local_store_nested_path() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalObjectStore::new(temp_dir.path()).await.unwrap();

        let data = Bytes::from("nested data");
        store.put("path/to/file.bin", data.clone()).await.unwrap();

        let retrieved = store.get("path/to/file.bin").await.unwrap();
        assert_eq!(retrieved, data);
    }

    #[tokio::test]
    async fn test_local_store_exists() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalObjectStore::new(temp_dir.path()).await.unwrap();

        assert!(!store.exists("nonexistent.txt").await.unwrap());

        store.put("exists.txt", Bytes::from("data")).await.unwrap();
        assert!(store.exists("exists.txt").await.unwrap());
    }

    #[tokio::test]
    async fn test_local_store_delete() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalObjectStore::new(temp_dir.path()).await.unwrap();

        store.put("delete.txt", Bytes::from("data")).await.unwrap();
        assert!(store.exists("delete.txt").await.unwrap());

        store.delete("delete.txt").await.unwrap();
        assert!(!store.exists("delete.txt").await.unwrap());

        // Idempotent delete
        store.delete("delete.txt").await.unwrap();
    }

    #[tokio::test]
    async fn test_local_store_list() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalObjectStore::new(temp_dir.path()).await.unwrap();

        store.put("file1.txt", Bytes::from("data1")).await.unwrap();
        store.put("file2.txt", Bytes::from("data2")).await.unwrap();
        store
            .put("subdir/file3.txt", Bytes::from("data3"))
            .await
            .unwrap();

        let all_objects = store.list("").await.unwrap();
        assert_eq!(all_objects.len(), 3);

        let subdir_objects = store.list("subdir").await.unwrap();
        assert!(subdir_objects.len() >= 1);
    }

    #[tokio::test]
    async fn test_local_store_head() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalObjectStore::new(temp_dir.path()).await.unwrap();

        let data = Bytes::from("test data");
        store.put("head.txt", data.clone()).await.unwrap();

        let metadata = store.head("head.txt").await.unwrap();
        assert_eq!(metadata.key, "head.txt");
        assert_eq!(metadata.size_bytes, data.len() as u64);
        assert!(metadata.etag.is_none());
    }

    #[tokio::test]
    async fn test_local_store_copy() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalObjectStore::new(temp_dir.path()).await.unwrap();

        let data = Bytes::from("copy me");
        store.put("source.txt", data.clone()).await.unwrap();

        store.copy("source.txt", "dest.txt").await.unwrap();

        let dest_data = store.get("dest.txt").await.unwrap();
        assert_eq!(dest_data, data);
    }

    #[tokio::test]
    async fn test_local_store_multipart() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalObjectStore::new(temp_dir.path()).await.unwrap();

        let parts = vec![
            Bytes::from("part1"),
            Bytes::from("part2"),
            Bytes::from("part3"),
        ];

        store.put_multipart("multi.bin", parts).await.unwrap();

        let data = store.get("multi.bin").await.unwrap();
        assert_eq!(data, Bytes::from("part1part2part3"));
    }

    #[tokio::test]
    async fn test_local_store_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalObjectStore::new(temp_dir.path()).await.unwrap();

        let result = store.get("nonexistent.txt").await;
        assert!(matches!(result, Err(CoreError::NotFound { .. })));

        let result = store.head("nonexistent.txt").await;
        assert!(matches!(result, Err(CoreError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_local_store_empty_key() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalObjectStore::new(temp_dir.path()).await.unwrap();

        let result = store.put("", Bytes::from("data")).await;
        assert!(matches!(result, Err(CoreError::ValidationError(_))));
    }
}
