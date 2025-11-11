//! AWS S3 implementation of ObjectStore
//!
//! Provides production-ready S3 integration with MinIO compatibility.
//! Supports standard AWS S3 and S3-compatible endpoints (MinIO, Wasabi, etc.).

use super::{ObjectMetadata, ObjectStore};
use akidb_core::{CoreError, CoreResult};
use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_s3::{config::Credentials, primitives::ByteStream, Client, Config};
use bytes::Bytes;
use chrono::Utc;
use serde::{Deserialize, Serialize};

/// S3 configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Config {
    /// S3 bucket name
    pub bucket: String,
    /// AWS region (e.g., "us-west-2")
    pub region: String,
    /// Optional custom endpoint URL (for MinIO compatibility)
    ///
    /// Examples:
    /// - MinIO: "http://localhost:9000"
    /// - Wasabi: "https://s3.wasabisys.com"
    pub endpoint: Option<String>,
    /// Optional access key (for custom S3 endpoints)
    pub access_key: Option<String>,
    /// Optional secret key (for custom S3 endpoints)
    pub secret_key: Option<String>,
    /// Optional key prefix (all keys will be prefixed with this)
    ///
    /// Example: "akidb/" will store object "data.bin" as "akidb/data.bin"
    pub prefix: Option<String>,
}

impl S3Config {
    /// Create config for standard AWS S3 (uses IAM credentials)
    pub fn aws(bucket: impl Into<String>, region: impl Into<String>) -> Self {
        Self {
            bucket: bucket.into(),
            region: region.into(),
            endpoint: None,
            access_key: None,
            secret_key: None,
            prefix: None,
        }
    }

    /// Create config for MinIO or custom S3-compatible endpoint
    pub fn custom(
        bucket: impl Into<String>,
        region: impl Into<String>,
        endpoint: impl Into<String>,
        access_key: impl Into<String>,
        secret_key: impl Into<String>,
    ) -> Self {
        Self {
            bucket: bucket.into(),
            region: region.into(),
            endpoint: Some(endpoint.into()),
            access_key: Some(access_key.into()),
            secret_key: Some(secret_key.into()),
            prefix: None,
        }
    }

    /// Set optional key prefix
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }
}

/// AWS S3 object store
///
/// Supports both standard AWS S3 and S3-compatible endpoints (MinIO, Wasabi, etc.).
///
/// # Examples
///
/// ## Standard AWS S3 (uses IAM credentials)
///
/// ```rust,no_run
/// use akidb_storage::object_store::{S3Config, S3ObjectStore, ObjectStore};
/// use bytes::Bytes;
///
/// #[tokio::main]
/// async fn main() -> akidb_core::CoreResult<()> {
///     let config = S3Config::aws("my-bucket", "us-west-2");
///     let store = S3ObjectStore::new(config).await?;
///
///     store.put("test.bin", Bytes::from("data")).await?;
///     Ok(())
/// }
/// ```
///
/// ## MinIO (custom S3-compatible endpoint)
///
/// ```rust,no_run
/// use akidb_storage::object_store::{S3Config, S3ObjectStore, ObjectStore};
/// use bytes::Bytes;
///
/// #[tokio::main]
/// async fn main() -> akidb_core::CoreResult<()> {
///     let config = S3Config::custom(
///         "test-bucket",
///         "us-east-1",
///         "http://localhost:9000",
///         "minioadmin",
///         "minioadmin",
///     );
///     let store = S3ObjectStore::new(config).await?;
///
///     store.put("snapshot.parquet", Bytes::from("data")).await?;
///     Ok(())
/// }
/// ```
pub struct S3ObjectStore {
    client: Client,
    bucket: String,
    prefix: Option<String>,
}

impl S3ObjectStore {
    /// Create a new S3 object store
    ///
    /// # Errors
    ///
    /// Returns `CoreError::StorageError` if AWS SDK initialization fails
    pub async fn new(config: S3Config) -> CoreResult<Self> {
        let aws_config = if let (Some(endpoint), Some(access), Some(secret)) =
            (&config.endpoint, &config.access_key, &config.secret_key)
        {
            // MinIO or custom S3-compatible endpoint
            let creds = Credentials::new(access, secret, None, None, "akidb-static");

            let s3_config = Config::builder()
                .endpoint_url(endpoint)
                .credentials_provider(creds)
                .region(aws_sdk_s3::config::Region::new(config.region.clone()))
                .force_path_style(true) // Required for MinIO
                .behavior_version(BehaviorVersion::latest())
                .build();

            Client::from_conf(s3_config)
        } else {
            // Standard AWS S3 - use environment credentials
            let aws_config = aws_config::defaults(BehaviorVersion::latest())
                .region(aws_config::Region::new(config.region.clone()))
                .load()
                .await;

            Client::new(&aws_config)
        };

        Ok(Self {
            client: aws_config,
            bucket: config.bucket,
            prefix: config.prefix,
        })
    }

    /// Apply prefix to key if configured
    fn full_key(&self, key: &str) -> String {
        if let Some(prefix) = &self.prefix {
            format!("{}/{}", prefix.trim_end_matches('/'), key)
        } else {
            key.to_string()
        }
    }

    /// Strip prefix from key if configured
    fn strip_prefix(&self, key: &str) -> String {
        if let Some(prefix) = &self.prefix {
            let prefix_with_slash = format!("{}/", prefix.trim_end_matches('/'));
            key.strip_prefix(&prefix_with_slash)
                .unwrap_or(key)
                .to_string()
        } else {
            key.to_string()
        }
    }
}

#[async_trait]
impl ObjectStore for S3ObjectStore {
    async fn put(&self, key: &str, data: Bytes) -> CoreResult<()> {
        if key.is_empty() {
            return Err(CoreError::ValidationError(
                "Key cannot be empty".to_string(),
            ));
        }

        let full_key = self.full_key(key);

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&full_key)
            .body(ByteStream::from(data))
            .send()
            .await
            .map_err(|e| CoreError::StorageError(format!("S3 put failed: {}", e)))?;

        Ok(())
    }

    async fn get(&self, key: &str) -> CoreResult<Bytes> {
        let full_key = self.full_key(key);

        let resp = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&full_key)
            .send()
            .await
            .map_err(|e| {
                let err_str = e.to_string();
                if err_str.contains("NoSuchKey") || err_str.contains("404") {
                    CoreError::not_found("object", key)
                } else {
                    CoreError::StorageError(format!("S3 get failed: {}", e))
                }
            })?;

        let data = resp
            .body
            .collect()
            .await
            .map_err(|e| CoreError::StorageError(format!("S3 read failed: {}", e)))?
            .into_bytes();

        Ok(data)
    }

    async fn exists(&self, key: &str) -> CoreResult<bool> {
        match self.head(key).await {
            Ok(_) => Ok(true),
            Err(CoreError::NotFound { .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }

    async fn delete(&self, key: &str) -> CoreResult<()> {
        let full_key = self.full_key(key);

        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(&full_key)
            .send()
            .await
            .map_err(|e| CoreError::StorageError(format!("S3 delete failed: {}", e)))?;

        // S3 delete is idempotent (no error if object doesn't exist)
        Ok(())
    }

    async fn list(&self, prefix: &str) -> CoreResult<Vec<ObjectMetadata>> {
        let full_prefix = self.full_key(prefix);

        let resp = self
            .client
            .list_objects_v2()
            .bucket(&self.bucket)
            .prefix(&full_prefix)
            .send()
            .await
            .map_err(|e| CoreError::StorageError(format!("S3 list failed: {}", e)))?;

        let objects = resp
            .contents()
            .iter()
            .filter_map(|obj| {
                let key = obj.key()?;
                let stripped_key = self.strip_prefix(key);
                let size = obj.size().unwrap_or(0);
                let modified = obj.last_modified()?;
                let etag = obj.e_tag().map(|s| s.to_string());

                // Convert AWS DateTime to chrono DateTime
                let last_modified =
                    chrono::DateTime::from_timestamp(modified.secs(), modified.subsec_nanos())
                        .unwrap_or_else(Utc::now);

                Some(ObjectMetadata {
                    key: stripped_key,
                    size_bytes: size as u64,
                    last_modified,
                    etag,
                })
            })
            .collect();

        Ok(objects)
    }

    async fn head(&self, key: &str) -> CoreResult<ObjectMetadata> {
        let full_key = self.full_key(key);

        let resp = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(&full_key)
            .send()
            .await
            .map_err(|e| {
                let err_str = e.to_string();
                if err_str.contains("404") || err_str.contains("NotFound") {
                    CoreError::not_found("object", key)
                } else {
                    CoreError::StorageError(format!("S3 head failed: {}", e))
                }
            })?;

        Ok(ObjectMetadata {
            key: key.to_string(),
            size_bytes: resp.content_length().unwrap_or(0) as u64,
            last_modified: resp
                .last_modified()
                .and_then(|dt| chrono::DateTime::from_timestamp(dt.secs(), dt.subsec_nanos()))
                .unwrap_or_else(Utc::now),
            etag: resp.e_tag().map(|s| s.to_string()),
        })
    }

    async fn copy(&self, from_key: &str, to_key: &str) -> CoreResult<()> {
        let from_full = self.full_key(from_key);
        let to_full = self.full_key(to_key);
        let copy_source = format!("{}/{}", self.bucket, from_full);

        self.client
            .copy_object()
            .bucket(&self.bucket)
            .copy_source(&copy_source)
            .key(&to_full)
            .send()
            .await
            .map_err(|e| {
                let err_str = e.to_string();
                if err_str.contains("NoSuchKey") || err_str.contains("404") {
                    CoreError::not_found("object", from_key)
                } else {
                    CoreError::StorageError(format!("S3 copy failed: {}", e))
                }
            })?;

        Ok(())
    }

    async fn put_multipart(&self, key: &str, parts: Vec<Bytes>) -> CoreResult<()> {
        // For simplicity, concatenate and use single put if < 5GB
        let total_size: usize = parts.iter().map(|p| p.len()).sum();

        if total_size < 5 * 1024 * 1024 * 1024 {
            // < 5GB, use single put
            let mut combined = Vec::with_capacity(total_size);
            for part in parts {
                combined.extend_from_slice(&part);
            }
            self.put(key, Bytes::from(combined)).await
        } else {
            // TODO: Implement true multipart upload for >5GB
            Err(CoreError::ValidationError(
                "Multipart upload for >5GB not yet implemented".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s3_config_aws() {
        let config = S3Config::aws("my-bucket", "us-west-2");
        assert_eq!(config.bucket, "my-bucket");
        assert_eq!(config.region, "us-west-2");
        assert!(config.endpoint.is_none());
        assert!(config.access_key.is_none());
    }

    #[test]
    fn test_s3_config_custom() {
        let config = S3Config::custom(
            "test-bucket",
            "us-east-1",
            "http://localhost:9000",
            "access",
            "secret",
        );
        assert_eq!(config.bucket, "test-bucket");
        assert_eq!(config.endpoint.unwrap(), "http://localhost:9000");
        assert_eq!(config.access_key.unwrap(), "access");
    }

    #[test]
    fn test_s3_config_with_prefix() {
        let config = S3Config::aws("bucket", "region").with_prefix("data/");
        assert_eq!(config.prefix.unwrap(), "data/");
    }

    #[test]
    fn test_s3_full_key() {
        let config = S3Config::aws("bucket", "region").with_prefix("prefix");
        let store = S3ObjectStore {
            client: Client::from_conf(
                Config::builder()
                    .region(aws_sdk_s3::config::Region::new("us-east-1"))
                    .behavior_version(BehaviorVersion::latest())
                    .build(),
            ),
            bucket: "bucket".to_string(),
            prefix: config.prefix,
        };

        assert_eq!(store.full_key("test.bin"), "prefix/test.bin");
    }

    #[test]
    fn test_s3_full_key_no_prefix() {
        let store = S3ObjectStore {
            client: Client::from_conf(
                Config::builder()
                    .region(aws_sdk_s3::config::Region::new("us-east-1"))
                    .behavior_version(BehaviorVersion::latest())
                    .build(),
            ),
            bucket: "bucket".to_string(),
            prefix: None,
        };

        assert_eq!(store.full_key("test.bin"), "test.bin");
    }

    #[test]
    fn test_s3_strip_prefix() {
        let config = S3Config::aws("bucket", "region").with_prefix("data");
        let store = S3ObjectStore {
            client: Client::from_conf(
                Config::builder()
                    .region(aws_sdk_s3::config::Region::new("us-east-1"))
                    .behavior_version(BehaviorVersion::latest())
                    .build(),
            ),
            bucket: "bucket".to_string(),
            prefix: config.prefix,
        };

        assert_eq!(store.strip_prefix("data/test.bin"), "test.bin");
        assert_eq!(store.strip_prefix("other/file.bin"), "other/file.bin");
    }
}
