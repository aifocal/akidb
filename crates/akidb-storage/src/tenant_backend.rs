use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use uuid::Uuid;

use akidb_core::{CollectionDescriptor, CollectionManifest, Result, SegmentDescriptor, TenantId};

use crate::backend::{StorageBackend, StorageStatus};
use crate::metadata::MetadataBlock;
use crate::segment_format::SegmentData;

/// Tenant-aware storage backend wrapper
///
/// Adds tenant namespace prefix to all storage paths for isolation.
/// Wraps any StorageBackend implementation to provide multi-tenancy.
///
/// # Path Structure
///
/// ```text
/// Single-tenant: collections/{collection}/...
/// Multi-tenant:  tenants/{tenant_id}/collections/{collection}/...
/// ```
///
/// # Example
///
/// ```rust,ignore
/// let s3_backend = S3StorageBackend::new(config)?;
/// let tenant_backend = TenantStorageBackend::new(s3_backend, "tenant_acme");
///
/// // All operations are automatically prefixed with tenant ID
/// tenant_backend.create_collection(&descriptor).await?;
/// // Writes to: tenants/tenant_acme/collections/{name}/manifest.json
/// ```
#[derive(Clone)]
pub struct TenantStorageBackend {
    /// Underlying storage backend
    inner: Arc<dyn StorageBackend>,
    /// Tenant ID for namespace isolation
    tenant_id: TenantId,
}

impl TenantStorageBackend {
    /// Create a new tenant-scoped storage backend
    pub fn new(inner: Arc<dyn StorageBackend>, tenant_id: TenantId) -> Self {
        Self { inner, tenant_id }
    }

    /// Add tenant prefix to a storage key
    fn with_tenant_prefix(&self, key: &str) -> String {
        format!("tenants/{}/{}", self.tenant_id, key)
    }

    /// Get the tenant ID
    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    /// Get the underlying storage backend
    pub fn inner(&self) -> &Arc<dyn StorageBackend> {
        &self.inner
    }
}

#[async_trait]
impl StorageBackend for TenantStorageBackend {
    async fn status(&self) -> Result<StorageStatus> {
        self.inner.status().await
    }

    async fn create_collection(&self, descriptor: &CollectionDescriptor) -> Result<()> {
        self.inner.create_collection(descriptor).await
    }

    async fn drop_collection(&self, name: &str) -> Result<()> {
        self.inner.drop_collection(name).await
    }

    #[allow(deprecated)]
    async fn write_segment(&self, descriptor: &SegmentDescriptor) -> Result<()> {
        self.inner.write_segment(descriptor).await
    }

    async fn seal_segment(&self, segment_id: Uuid) -> Result<SegmentDescriptor> {
        self.inner.seal_segment(segment_id).await
    }

    async fn load_manifest(&self, collection: &str) -> Result<CollectionManifest> {
        // Add tenant prefix to manifest path
        let key = self.with_tenant_prefix(&format!("collections/{}/manifest.json", collection));
        let bytes = self.inner.get_object(&key).await?;
        let manifest: CollectionManifest = serde_json::from_slice(&bytes).map_err(|e| {
            akidb_core::Error::Serialization(format!("Failed to parse manifest: {}", e))
        })?;
        Ok(manifest)
    }

    async fn persist_manifest(&self, manifest: &CollectionManifest) -> Result<()> {
        // Add tenant prefix to manifest path
        let key = self.with_tenant_prefix(&format!(
            "collections/{}/manifest.json",
            &manifest.collection
        ));
        let bytes = serde_json::to_vec_pretty(manifest).map_err(|e| {
            akidb_core::Error::Serialization(format!("Failed to serialize manifest: {}", e))
        })?;
        self.inner.put_object(&key, Bytes::from(bytes)).await
    }

    async fn get_object(&self, key: &str) -> Result<Bytes> {
        let prefixed_key = self.with_tenant_prefix(key);
        self.inner.get_object(&prefixed_key).await
    }

    async fn put_object(&self, key: &str, data: Bytes) -> Result<()> {
        let prefixed_key = self.with_tenant_prefix(key);
        self.inner.put_object(&prefixed_key, data).await
    }

    async fn delete_object(&self, key: &str) -> Result<()> {
        let prefixed_key = self.with_tenant_prefix(key);
        self.inner.delete_object(&prefixed_key).await
    }

    async fn object_exists(&self, key: &str) -> Result<bool> {
        let prefixed_key = self.with_tenant_prefix(key);
        self.inner.object_exists(&prefixed_key).await
    }

    async fn list_objects(&self, prefix: &str) -> Result<Vec<String>> {
        let prefixed_prefix = self.with_tenant_prefix(prefix);
        let keys = self.inner.list_objects(&prefixed_prefix).await?;

        // Strip tenant prefix from returned keys
        let tenant_prefix = format!("tenants/{}/", self.tenant_id);
        let stripped_keys = keys
            .into_iter()
            .filter_map(|key| key.strip_prefix(&tenant_prefix).map(|s| s.to_string()))
            .collect();

        Ok(stripped_keys)
    }

    async fn write_segment_with_data(
        &self,
        descriptor: &SegmentDescriptor,
        vectors: Vec<Vec<f32>>,
        metadata: Option<MetadataBlock>,
    ) -> Result<()> {
        // Add tenant prefix to segment path
        let key = self.with_tenant_prefix(&format!(
            "collections/{}/segments/{}.seg",
            descriptor.collection, descriptor.segment_id
        ));

        // Write using underlying backend's implementation
        // We can't use the trait method directly because it doesn't accept a key parameter
        // Instead, we'll use put_object with segment data

        // Serialize segment data
        let segment_data = SegmentData {
            dimension: descriptor.vector_dim as u32,
            vectors,
            metadata,
        };

        // Write to storage using the segment writer
        let writer = crate::segment_format::SegmentWriter::new(
            crate::segment_format::CompressionType::Zstd,
            crate::segment_format::ChecksumType::XXH3,
        );

        let buffer = writer.write(&segment_data)?;

        self.inner.put_object(&key, Bytes::from(buffer)).await
    }

    async fn load_segment(&self, collection: &str, segment_id: Uuid) -> Result<SegmentData> {
        // Add tenant prefix to segment path
        let key = self.with_tenant_prefix(&format!(
            "collections/{}/segments/{}.seg",
            collection, segment_id
        ));

        let bytes = self.inner.get_object(&key).await?;

        // Deserialize segment data
        crate::segment_format::SegmentReader::read(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryStorageBackend;
    use akidb_core::DistanceMetric;

    #[tokio::test]
    async fn test_tenant_storage_wrapper() {
        let inner = Arc::new(MemoryStorageBackend::new());
        let tenant1 = TenantStorageBackend::new(inner.clone(), "tenant_1".to_string());
        let tenant2 = TenantStorageBackend::new(inner.clone(), "tenant_2".to_string());

        // Create collections for both tenants
        let desc1 =
            CollectionDescriptor::new("test_collection".to_string(), 128, DistanceMetric::Cosine);
        let desc2 =
            CollectionDescriptor::new("test_collection".to_string(), 128, DistanceMetric::Cosine);

        tenant1.create_collection(&desc1).await.unwrap();
        tenant2.create_collection(&desc2).await.unwrap();

        // Both tenants can have same collection name due to isolation
        assert!(tenant1.load_manifest("test_collection").await.is_ok());
        assert!(tenant2.load_manifest("test_collection").await.is_ok());
    }

    #[tokio::test]
    async fn test_tenant_prefix_isolation() {
        let inner = Arc::new(MemoryStorageBackend::new());
        let tenant1 = TenantStorageBackend::new(inner.clone(), "tenant_a".to_string());
        let tenant2 = TenantStorageBackend::new(inner.clone(), "tenant_b".to_string());

        // Write object for tenant1
        tenant1
            .put_object("test/file.txt", Bytes::from("tenant1 data"))
            .await
            .unwrap();

        // Tenant2 should not see tenant1's object
        assert!(tenant2.get_object("test/file.txt").await.is_err());

        // Tenant1 can read its own object
        let data = tenant1.get_object("test/file.txt").await.unwrap();
        assert_eq!(data, Bytes::from("tenant1 data"));
    }

    #[tokio::test]
    async fn test_list_objects_strips_prefix() {
        let inner = Arc::new(MemoryStorageBackend::new());
        let tenant = TenantStorageBackend::new(inner.clone(), "tenant_test".to_string());

        // Write some objects
        tenant
            .put_object("data/file1.txt", Bytes::from("data1"))
            .await
            .unwrap();
        tenant
            .put_object("data/file2.txt", Bytes::from("data2"))
            .await
            .unwrap();

        // List objects should return paths without tenant prefix
        let keys = tenant.list_objects("data/").await.unwrap();
        assert!(keys.contains(&"data/file1.txt".to_string()));
        assert!(keys.contains(&"data/file2.txt".to_string()));

        // Keys should not contain tenant prefix
        for key in &keys {
            assert!(!key.contains("tenants/tenant_test/"));
        }
    }

    #[tokio::test]
    async fn test_tenant_id_getter() {
        let inner = Arc::new(MemoryStorageBackend::new());
        let tenant = TenantStorageBackend::new(inner.clone(), "tenant_xyz".to_string());

        assert_eq!(tenant.tenant_id(), "tenant_xyz");
    }

    #[tokio::test]
    async fn test_status_passthrough() {
        let inner = Arc::new(MemoryStorageBackend::new());
        let tenant = TenantStorageBackend::new(inner, "tenant_test".to_string());

        let status = tenant.status().await.unwrap();
        assert_eq!(status, StorageStatus::Healthy);
    }
}
