use akidb_core::{TenantDescriptor, TenantError, TenantId};
use async_trait::async_trait;
use bytes::Bytes;
use object_store::{path::Path as ObjectPath, ObjectStore};
use serde_json;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Tenant storage operations
#[async_trait]
pub trait TenantStore: Send + Sync {
    /// Create a new tenant
    async fn create_tenant(&self, tenant: &TenantDescriptor) -> Result<(), TenantError>;

    /// Get tenant by ID
    async fn get_tenant(&self, tenant_id: &TenantId) -> Result<TenantDescriptor, TenantError>;

    /// Update tenant
    async fn update_tenant(&self, tenant: &TenantDescriptor) -> Result<(), TenantError>;

    /// Delete tenant (soft delete)
    async fn delete_tenant(&self, tenant_id: &TenantId) -> Result<(), TenantError>;

    /// List all tenants (with pagination)
    async fn list_tenants(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<TenantDescriptor>, TenantError>;

    /// Check if tenant exists
    async fn tenant_exists(&self, tenant_id: &TenantId) -> Result<bool, TenantError>;
}

/// S3-backed tenant storage
pub struct S3TenantStore {
    object_store: Arc<dyn ObjectStore>,
    bucket: String,
}

impl S3TenantStore {
    /// Create a new S3 tenant store
    pub fn new(object_store: Arc<dyn ObjectStore>, bucket: String) -> Self {
        Self {
            object_store,
            bucket,
        }
    }

    /// Get tenant manifest path
    fn tenant_path(&self, tenant_id: &TenantId) -> ObjectPath {
        ObjectPath::from(format!("tenants/{}/manifest.json", tenant_id))
    }

    /// Get tenants list path (for pagination)
    fn tenants_list_path(&self) -> ObjectPath {
        ObjectPath::from("tenants/")
    }
}

#[async_trait]
impl TenantStore for S3TenantStore {
    async fn create_tenant(&self, tenant: &TenantDescriptor) -> Result<(), TenantError> {
        debug!("Creating tenant: {}", tenant.tenant_id);

        // Validate tenant before creation
        tenant.validate()?;

        // Check if tenant already exists
        if self.tenant_exists(&tenant.tenant_id).await? {
            return Err(TenantError::AlreadyExists(tenant.tenant_id.clone()));
        }

        // Serialize tenant manifest
        let manifest_json = serde_json::to_vec_pretty(tenant).map_err(|e| {
            TenantError::StorageError(format!("Failed to serialize tenant manifest: {}", e))
        })?;

        // Write to S3
        let path = self.tenant_path(&tenant.tenant_id);
        self.object_store
            .put(&path, Bytes::from(manifest_json).into())
            .await
            .map_err(|e| {
                TenantError::StorageError(format!("Failed to write tenant manifest: {}", e))
            })?;

        info!("Created tenant: {} ({})", tenant.name, tenant.tenant_id);
        Ok(())
    }

    async fn get_tenant(&self, tenant_id: &TenantId) -> Result<TenantDescriptor, TenantError> {
        debug!("Getting tenant: {}", tenant_id);

        let path = self.tenant_path(tenant_id);

        // Read from S3
        let result = self.object_store.get(&path).await.map_err(|e| {
            if e.to_string().contains("404") || e.to_string().contains("NotFound") {
                TenantError::NotFound(tenant_id.clone())
            } else {
                TenantError::StorageError(format!("Failed to read tenant manifest: {}", e))
            }
        })?;

        let bytes = result.bytes().await.map_err(|e| {
            TenantError::StorageError(format!("Failed to read tenant bytes: {}", e))
        })?;

        // Deserialize manifest
        let tenant: TenantDescriptor = serde_json::from_slice(&bytes).map_err(|e| {
            TenantError::StorageError(format!("Failed to parse tenant manifest: {}", e))
        })?;

        debug!("Retrieved tenant: {} ({})", tenant.name, tenant.tenant_id);
        Ok(tenant)
    }

    async fn update_tenant(&self, tenant: &TenantDescriptor) -> Result<(), TenantError> {
        debug!("Updating tenant: {}", tenant.tenant_id);

        // Validate tenant
        tenant.validate()?;

        // Check if tenant exists
        if !self.tenant_exists(&tenant.tenant_id).await? {
            return Err(TenantError::NotFound(tenant.tenant_id.clone()));
        }

        // Serialize tenant manifest
        let manifest_json = serde_json::to_vec_pretty(tenant).map_err(|e| {
            TenantError::StorageError(format!("Failed to serialize tenant manifest: {}", e))
        })?;

        // Write to S3 (overwrite)
        let path = self.tenant_path(&tenant.tenant_id);
        self.object_store
            .put(&path, Bytes::from(manifest_json).into())
            .await
            .map_err(|e| {
                TenantError::StorageError(format!("Failed to update tenant manifest: {}", e))
            })?;

        info!("Updated tenant: {} ({})", tenant.name, tenant.tenant_id);
        Ok(())
    }

    async fn delete_tenant(&self, tenant_id: &TenantId) -> Result<(), TenantError> {
        debug!("Deleting tenant: {}", tenant_id);

        // Get existing tenant
        let mut tenant = self.get_tenant(tenant_id).await?;

        // Soft delete
        tenant.soft_delete();

        // Update tenant with deleted status
        self.update_tenant(&tenant).await?;

        warn!("Soft deleted tenant: {}", tenant_id);
        Ok(())
    }

    async fn list_tenants(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<TenantDescriptor>, TenantError> {
        debug!("Listing tenants: offset={}, limit={}", offset, limit);

        let prefix = self.tenants_list_path();

        // List all tenant manifest files
        let list_result = self
            .object_store
            .list(Some(&prefix))
            .map_err(|e| TenantError::StorageError(format!("Failed to list tenants: {}", e)))?;

        let mut tenants = Vec::new();

        // Read each tenant manifest
        for meta in list_result {
            let path = meta.location;

            // Skip non-manifest files
            if !path.as_ref().ends_with("manifest.json") {
                continue;
            }

            // Read tenant manifest
            match self.object_store.get(&path).await {
                Ok(result) => {
                    if let Ok(bytes) = result.bytes().await {
                        if let Ok(tenant) = serde_json::from_slice::<TenantDescriptor>(&bytes) {
                            tenants.push(tenant);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to read tenant manifest at {}: {}", path, e);
                }
            }
        }

        // Sort by created_at (newest first)
        tenants.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        // Apply pagination
        let paginated: Vec<TenantDescriptor> =
            tenants.into_iter().skip(offset).take(limit).collect();

        debug!("Retrieved {} tenants", paginated.len());
        Ok(paginated)
    }

    async fn tenant_exists(&self, tenant_id: &TenantId) -> Result<bool, TenantError> {
        let path = self.tenant_path(tenant_id);

        match self.object_store.head(&path).await {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.to_string().contains("404") || e.to_string().contains("NotFound") {
                    Ok(false)
                } else {
                    Err(TenantError::StorageError(format!(
                        "Failed to check tenant existence: {}",
                        e
                    )))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use akidb_core::TenantQuota;
    use object_store::memory::InMemory;

    fn create_test_store() -> S3TenantStore {
        let object_store = Arc::new(InMemory::new());
        S3TenantStore::new(object_store, "test-bucket".to_string())
    }

    #[tokio::test]
    async fn test_create_and_get_tenant() {
        let store = create_test_store();
        let tenant = TenantDescriptor::new("Test Corp".to_string(), None);

        // Create tenant
        store.create_tenant(&tenant).await.unwrap();

        // Get tenant
        let retrieved = store.get_tenant(&tenant.tenant_id).await.unwrap();
        assert_eq!(retrieved.tenant_id, tenant.tenant_id);
        assert_eq!(retrieved.name, "Test Corp");
    }

    #[tokio::test]
    async fn test_create_duplicate_tenant() {
        let store = create_test_store();
        let tenant = TenantDescriptor::new("Test Corp".to_string(), None);

        // Create tenant
        store.create_tenant(&tenant).await.unwrap();

        // Try to create again - should fail
        let result = store.create_tenant(&tenant).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TenantError::AlreadyExists(_)));
    }

    #[tokio::test]
    async fn test_get_nonexistent_tenant() {
        let store = create_test_store();
        let result = store.get_tenant(&"nonexistent".to_string()).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TenantError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_update_tenant() {
        let store = create_test_store();
        let mut tenant = TenantDescriptor::new("Test Corp".to_string(), None);

        // Create tenant
        store.create_tenant(&tenant).await.unwrap();

        // Update tenant
        tenant.name = "Updated Corp".to_string();
        store.update_tenant(&tenant).await.unwrap();

        // Verify update
        let retrieved = store.get_tenant(&tenant.tenant_id).await.unwrap();
        assert_eq!(retrieved.name, "Updated Corp");
    }

    #[tokio::test]
    async fn test_soft_delete_tenant() {
        let store = create_test_store();
        let tenant = TenantDescriptor::new("Test Corp".to_string(), None);

        // Create tenant
        store.create_tenant(&tenant).await.unwrap();

        // Delete tenant
        store.delete_tenant(&tenant.tenant_id).await.unwrap();

        // Verify tenant is marked as deleted
        let retrieved = store.get_tenant(&tenant.tenant_id).await.unwrap();
        assert!(retrieved.is_deleted());
        assert!(retrieved.deleted_at.is_some());
    }

    #[tokio::test]
    async fn test_list_tenants() {
        let store = create_test_store();

        // Create multiple tenants
        for i in 0..5 {
            let tenant = TenantDescriptor::new(format!("Corp {}", i), None);
            store.create_tenant(&tenant).await.unwrap();
        }

        // List all tenants
        let tenants = store.list_tenants(0, 10).await.unwrap();
        assert_eq!(tenants.len(), 5);

        // Test pagination
        let page1 = store.list_tenants(0, 2).await.unwrap();
        assert_eq!(page1.len(), 2);

        let page2 = store.list_tenants(2, 2).await.unwrap();
        assert_eq!(page2.len(), 2);
    }

    #[tokio::test]
    async fn test_tenant_exists() {
        let store = create_test_store();
        let tenant = TenantDescriptor::new("Test Corp".to_string(), None);

        // Check non-existent tenant
        assert!(!store.tenant_exists(&tenant.tenant_id).await.unwrap());

        // Create tenant
        store.create_tenant(&tenant).await.unwrap();

        // Check existing tenant
        assert!(store.tenant_exists(&tenant.tenant_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_tenant_validation() {
        let store = create_test_store();
        let mut tenant = TenantDescriptor::new("".to_string(), None); // Invalid empty name

        // Should fail validation
        let result = store.create_tenant(&tenant).await;
        assert!(result.is_err());
    }
}
