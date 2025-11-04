use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

/// Tenant identifier (UUIDv4)
pub type TenantId = String;

/// Tenant management errors
#[derive(Debug, Error)]
pub enum TenantError {
    #[error("Tenant not found: {0}")]
    NotFound(TenantId),

    #[error("Tenant already exists: {0}")]
    AlreadyExists(TenantId),

    #[error("Quota exceeded: {quota_type}")]
    QuotaExceeded { quota_type: String },

    #[error("Invalid tenant ID: {0}")]
    InvalidTenantId(String),

    #[error("Tenant validation failed: {0}")]
    ValidationFailed(String),

    #[error("Storage error: {0}")]
    StorageError(String),
}

/// Tenant resource quotas
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TenantQuota {
    /// Maximum storage in bytes (0 = unlimited)
    pub max_storage_bytes: u64,

    /// Maximum number of collections (0 = unlimited)
    pub max_collections: u32,

    /// Maximum vectors per collection (0 = unlimited)
    pub max_vectors_per_collection: u64,

    /// API requests per second (0 = unlimited)
    pub api_rate_limit_per_second: u32,

    /// Maximum concurrent searches (0 = unlimited)
    pub max_concurrent_searches: u32,
}

impl Default for TenantQuota {
    fn default() -> Self {
        Self {
            max_storage_bytes: 107_374_182_400, // 100 GB
            max_collections: 100,
            max_vectors_per_collection: 10_000_000, // 10M vectors
            api_rate_limit_per_second: 1000,
            max_concurrent_searches: 100,
        }
    }
}

impl TenantQuota {
    /// Create unlimited quota
    pub fn unlimited() -> Self {
        Self {
            max_storage_bytes: 0,
            max_collections: 0,
            max_vectors_per_collection: 0,
            api_rate_limit_per_second: 0,
            max_concurrent_searches: 0,
        }
    }

    /// Check if a resource is unlimited
    pub fn is_unlimited(&self, resource: &str) -> bool {
        match resource {
            "storage" => self.max_storage_bytes == 0,
            "collections" => self.max_collections == 0,
            "vectors" => self.max_vectors_per_collection == 0,
            "rate" => self.api_rate_limit_per_second == 0,
            "searches" => self.max_concurrent_searches == 0,
            _ => false,
        }
    }
}

/// Tenant metadata (custom key-value pairs)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TenantMetadata {
    /// Contact email for tenant admin
    pub contact_email: Option<String>,

    /// Billing plan: free, starter, professional, enterprise
    pub billing_plan: Option<String>,

    /// Company/organization name
    pub organization: Option<String>,

    /// Custom key-value metadata
    pub custom: HashMap<String, serde_json::Value>,
}

impl Default for TenantMetadata {
    fn default() -> Self {
        Self {
            contact_email: None,
            billing_plan: Some("free".to_string()),
            organization: None,
            custom: HashMap::new(),
        }
    }
}

/// Tenant status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TenantStatus {
    /// Active and operational
    Active,
    /// Suspended (quota exceeded, payment issue)
    Suspended,
    /// Soft deleted (can be restored)
    Deleted,
}

impl Default for TenantStatus {
    fn default() -> Self {
        Self::Active
    }
}

/// Tenant descriptor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantDescriptor {
    /// Unique tenant identifier (UUIDv4)
    pub tenant_id: TenantId,

    /// Human-readable tenant name
    pub name: String,

    /// Tenant status
    pub status: TenantStatus,

    /// Resource quotas
    pub quotas: TenantQuota,

    /// Metadata
    pub metadata: TenantMetadata,

    /// API key for authentication (hashed)
    pub api_key_hash: Option<String>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,

    /// Deleted timestamp (for soft deletes)
    pub deleted_at: Option<DateTime<Utc>>,

    /// Current resource usage
    pub usage: TenantUsage,
}

/// Current tenant resource usage
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TenantUsage {
    /// Current storage usage in bytes
    pub storage_bytes: u64,

    /// Current number of collections
    pub collection_count: u32,

    /// Total vectors across all collections
    pub total_vectors: u64,

    /// API requests in last minute (rolling window)
    pub api_requests_last_minute: u64,

    /// Last usage update timestamp
    pub last_updated: Option<DateTime<Utc>>,
}

impl TenantDescriptor {
    /// Create a new tenant
    pub fn new(name: String, quotas: Option<TenantQuota>) -> Self {
        let tenant_id = format!("tenant_{}", Uuid::new_v4().to_string().replace('-', ""));
        let now = Utc::now();

        Self {
            tenant_id,
            name,
            status: TenantStatus::Active,
            quotas: quotas.unwrap_or_default(),
            metadata: TenantMetadata::default(),
            api_key_hash: None,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            usage: TenantUsage::default(),
        }
    }

    /// Validate tenant data
    pub fn validate(&self) -> Result<(), TenantError> {
        // Validate tenant ID format
        if !self.tenant_id.starts_with("tenant_") {
            return Err(TenantError::InvalidTenantId(self.tenant_id.clone()));
        }

        // Validate name is not empty
        if self.name.trim().is_empty() {
            return Err(TenantError::ValidationFailed(
                "Tenant name cannot be empty".to_string(),
            ));
        }

        // Validate name length
        if self.name.len() > 255 {
            return Err(TenantError::ValidationFailed(
                "Tenant name too long (max 255 characters)".to_string(),
            ));
        }

        Ok(())
    }

    /// Check if tenant is active
    pub fn is_active(&self) -> bool {
        self.status == TenantStatus::Active
    }

    /// Check if tenant is deleted
    pub fn is_deleted(&self) -> bool {
        self.status == TenantStatus::Deleted
    }

    /// Soft delete tenant
    pub fn soft_delete(&mut self) {
        self.status = TenantStatus::Deleted;
        self.deleted_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    /// Restore deleted tenant
    pub fn restore(&mut self) {
        if self.is_deleted() {
            self.status = TenantStatus::Active;
            self.deleted_at = None;
            self.updated_at = Utc::now();
        }
    }

    /// Suspend tenant
    pub fn suspend(&mut self, reason: &str) {
        self.status = TenantStatus::Suspended;
        self.metadata
            .custom
            .insert("suspension_reason".to_string(), serde_json::json!(reason));
        self.updated_at = Utc::now();
    }

    /// Check if storage quota is exceeded
    pub fn is_storage_quota_exceeded(&self) -> bool {
        if self.quotas.max_storage_bytes == 0 {
            return false; // Unlimited
        }
        self.usage.storage_bytes >= self.quotas.max_storage_bytes
    }

    /// Check if collection quota is exceeded
    pub fn is_collection_quota_exceeded(&self) -> bool {
        if self.quotas.max_collections == 0 {
            return false; // Unlimited
        }
        self.usage.collection_count >= self.quotas.max_collections
    }

    /// Check if vector quota is exceeded for a collection
    pub fn is_vector_quota_exceeded(&self, collection_vectors: u64) -> bool {
        if self.quotas.max_vectors_per_collection == 0 {
            return false; // Unlimited
        }
        collection_vectors >= self.quotas.max_vectors_per_collection
    }

    /// Update resource usage
    pub fn update_usage(&mut self, storage_bytes: u64, collection_count: u32, total_vectors: u64) {
        self.usage.storage_bytes = storage_bytes;
        self.usage.collection_count = collection_count;
        self.usage.total_vectors = total_vectors;
        self.usage.last_updated = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    /// Increment API request counter
    pub fn increment_api_requests(&mut self) {
        self.usage.api_requests_last_minute += 1;
        self.usage.last_updated = Some(Utc::now());
    }
}

/// Tenant API key generation
pub mod api_key {
    use rand::Rng;
    use sha2::{Digest, Sha256};

    /// Generate a new API key
    ///
    /// Format: ak_{tenant_id}_{random}
    pub fn generate(tenant_id: &str) -> String {
        let random: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();

        format!("ak_{}_{}", tenant_id, random)
    }

    /// Hash an API key for storage
    pub fn hash(api_key: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(api_key.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Verify an API key against a hash
    pub fn verify(api_key: &str, api_key_hash: &str) -> bool {
        Self::hash(api_key) == api_key_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tenant_creation() {
        let tenant = TenantDescriptor::new("Test Corp".to_string(), None);
        assert!(tenant.tenant_id.starts_with("tenant_"));
        assert_eq!(tenant.name, "Test Corp");
        assert_eq!(tenant.status, TenantStatus::Active);
        assert!(tenant.is_active());
    }

    #[test]
    fn test_tenant_validation() {
        let tenant = TenantDescriptor::new("Valid Name".to_string(), None);
        assert!(tenant.validate().is_ok());

        let mut invalid_tenant = TenantDescriptor::new("".to_string(), None);
        assert!(invalid_tenant.validate().is_err());

        invalid_tenant.name = "a".repeat(300);
        assert!(invalid_tenant.validate().is_err());
    }

    #[test]
    fn test_soft_delete() {
        let mut tenant = TenantDescriptor::new("Test".to_string(), None);
        assert!(!tenant.is_deleted());

        tenant.soft_delete();
        assert!(tenant.is_deleted());
        assert!(tenant.deleted_at.is_some());
    }

    #[test]
    fn test_restore_tenant() {
        let mut tenant = TenantDescriptor::new("Test".to_string(), None);
        tenant.soft_delete();
        assert!(tenant.is_deleted());

        tenant.restore();
        assert!(tenant.is_active());
        assert!(tenant.deleted_at.is_none());
    }

    #[test]
    fn test_quota_checks() {
        let mut tenant = TenantDescriptor::new("Test".to_string(), Some(TenantQuota::default()));

        // Storage quota
        tenant.usage.storage_bytes = tenant.quotas.max_storage_bytes + 1;
        assert!(tenant.is_storage_quota_exceeded());

        // Collection quota
        tenant.usage.collection_count = tenant.quotas.max_collections + 1;
        assert!(tenant.is_collection_quota_exceeded());

        // Vector quota
        assert!(tenant.is_vector_quota_exceeded(tenant.quotas.max_vectors_per_collection + 1));
    }

    #[test]
    fn test_unlimited_quota() {
        let quota = TenantQuota::unlimited();
        assert_eq!(quota.max_storage_bytes, 0);
        assert!(quota.is_unlimited("storage"));
        assert!(quota.is_unlimited("collections"));
    }

    #[test]
    fn test_api_key_generation() {
        let api_key = api_key::generate("tenant_123");
        assert!(api_key.starts_with("ak_tenant_123_"));
        assert!(api_key.len() > 20);
    }

    #[test]
    fn test_api_key_hashing() {
        let api_key = "ak_test_12345";
        let hash = api_key::hash(api_key);
        assert_eq!(hash.len(), 64); // SHA-256 hex length

        assert!(api_key::verify(api_key, &hash));
        assert!(!api_key::verify("wrong_key", &hash));
    }

    #[test]
    fn test_tenant_suspension() {
        let mut tenant = TenantDescriptor::new("Test".to_string(), None);
        tenant.suspend("Payment overdue");

        assert_eq!(tenant.status, TenantStatus::Suspended);
        assert!(tenant.metadata.custom.contains_key("suspension_reason"));
    }

    #[test]
    fn test_usage_tracking() {
        let mut tenant = TenantDescriptor::new("Test".to_string(), None);
        tenant.update_usage(1000, 5, 50000);

        assert_eq!(tenant.usage.storage_bytes, 1000);
        assert_eq!(tenant.usage.collection_count, 5);
        assert_eq!(tenant.usage.total_vectors, 50000);
        assert!(tenant.usage.last_updated.is_some());
    }

    #[test]
    fn test_default_metadata() {
        let metadata = TenantMetadata::default();
        assert_eq!(metadata.billing_plan, Some("free".to_string()));
        assert!(metadata.contact_email.is_none());
    }
}
