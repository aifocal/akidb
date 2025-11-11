//! Authentication and authorization domain types for AkiDB 2.0.

use chrono::{DateTime, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{ApiKeyId, TenantId, UserId};

/// API key descriptor with permissions and metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiKeyDescriptor {
    /// Unique identifier for this API key.
    pub key_id: ApiKeyId,

    /// Tenant this API key belongs to.
    pub tenant_id: TenantId,

    /// Human-readable name for the API key.
    pub name: String,

    /// List of permissions granted to this API key.
    pub permissions: Vec<String>,

    /// When this API key was created.
    pub created_at: DateTime<Utc>,

    /// When this API key expires (None = never expires).
    pub expires_at: Option<DateTime<Utc>>,

    /// When this API key was last used (None = never used).
    pub last_used_at: Option<DateTime<Utc>>,

    /// User who created this API key (None = system-created).
    pub created_by: Option<UserId>,
}

impl ApiKeyDescriptor {
    /// Creates a new API key descriptor.
    #[must_use]
    pub fn new(
        tenant_id: TenantId,
        name: String,
        permissions: Vec<String>,
        expires_at: Option<DateTime<Utc>>,
        created_by: Option<UserId>,
    ) -> Self {
        Self {
            key_id: ApiKeyId::new(),
            tenant_id,
            name,
            permissions,
            created_at: Utc::now(),
            expires_at,
            last_used_at: None,
            created_by,
        }
    }

    /// Checks if this API key has expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }

    /// Checks if this API key has a specific permission.
    #[must_use]
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.iter().any(|p| p == permission)
    }
}

/// Request to create a new API key.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateApiKeyRequest {
    /// Human-readable name for the API key.
    pub name: String,

    /// List of permissions to grant.
    pub permissions: Vec<String>,

    /// Optional expiration time (None = never expires).
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}

/// Response containing the newly created API key.
/// The plaintext API key is only returned once during creation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateApiKeyResponse {
    /// API key descriptor.
    #[serde(flatten)]
    pub descriptor: ApiKeyDescriptor,

    /// Plaintext API key (only returned during creation).
    pub api_key: String,
}

/// List of API keys for a tenant.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ListApiKeysResponse {
    /// List of API key descriptors.
    pub keys: Vec<ApiKeyDescriptor>,

    /// Total number of keys for this tenant.
    pub total: usize,
}

/// Generates a new API key with 32 random bytes.
/// Returns the key in format: "ak_" + hex-encoded 32 bytes (64 hex characters).
#[must_use]
pub fn generate_api_key() -> String {
    let mut rng = rand::thread_rng();
    let mut key_bytes = [0u8; 32];
    rng.fill(&mut key_bytes);
    format!("ak_{}", hex::encode(key_bytes))
}

/// Hashes an API key using SHA-256.
/// Returns the hex-encoded hash suitable for storage.
#[must_use]
pub fn hash_api_key(api_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(api_key.as_bytes());
    hex::encode(hasher.finalize())
}

/// Validates an API key format.
/// Returns true if the key has the correct "ak_" prefix and length.
#[must_use]
pub fn is_valid_api_key_format(api_key: &str) -> bool {
    api_key.starts_with("ak_") && api_key.len() == 67 // "ak_" (3) + 64 hex chars
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_descriptor_new() {
        let tenant_id = TenantId::new();
        let user_id = UserId::new();
        let name = "test-key".to_string();
        let permissions = vec!["collection::read".to_string()];
        let expires_at = Some(Utc::now() + chrono::Duration::days(30));

        let descriptor = ApiKeyDescriptor::new(
            tenant_id,
            name.clone(),
            permissions.clone(),
            expires_at,
            Some(user_id),
        );

        assert_eq!(descriptor.tenant_id, tenant_id);
        assert_eq!(descriptor.name, name);
        assert_eq!(descriptor.permissions, permissions);
        assert_eq!(descriptor.created_by, Some(user_id));
        assert!(descriptor.expires_at.is_some());
        assert!(descriptor.last_used_at.is_none());
    }

    #[test]
    fn test_api_key_not_expired() {
        let descriptor = ApiKeyDescriptor::new(
            TenantId::new(),
            "test-key".to_string(),
            vec![],
            Some(Utc::now() + chrono::Duration::days(30)),
            None,
        );

        assert!(!descriptor.is_expired());
    }

    #[test]
    fn test_api_key_expired() {
        let descriptor = ApiKeyDescriptor::new(
            TenantId::new(),
            "test-key".to_string(),
            vec![],
            Some(Utc::now() - chrono::Duration::days(1)),
            None,
        );

        assert!(descriptor.is_expired());
    }

    #[test]
    fn test_api_key_never_expires() {
        let descriptor =
            ApiKeyDescriptor::new(TenantId::new(), "test-key".to_string(), vec![], None, None);

        assert!(!descriptor.is_expired());
    }

    #[test]
    fn test_has_permission() {
        let descriptor = ApiKeyDescriptor::new(
            TenantId::new(),
            "test-key".to_string(),
            vec![
                "collection::read".to_string(),
                "collection::write".to_string(),
            ],
            None,
            None,
        );

        assert!(descriptor.has_permission("collection::read"));
        assert!(descriptor.has_permission("collection::write"));
        assert!(!descriptor.has_permission("collection::delete"));
    }

    #[test]
    fn test_generate_api_key_format() {
        let key = generate_api_key();
        assert!(key.starts_with("ak_"));
        assert_eq!(key.len(), 67); // "ak_" (3) + 64 hex chars
        assert!(is_valid_api_key_format(&key));
    }

    #[test]
    fn test_generate_api_key_uniqueness() {
        let key1 = generate_api_key();
        let key2 = generate_api_key();
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_hash_api_key() {
        let key = "ak_1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        let hash = hash_api_key(key);

        // SHA-256 always produces 64 hex characters
        assert_eq!(hash.len(), 64);

        // Same key should produce same hash
        let hash2 = hash_api_key(key);
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_hash_api_key_different_keys() {
        let key1 = "ak_1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        let key2 = "ak_abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";

        let hash1 = hash_api_key(key1);
        let hash2 = hash_api_key(key2);

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_is_valid_api_key_format() {
        // Valid keys
        assert!(is_valid_api_key_format(
            "ak_1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
        ));

        // Invalid: wrong prefix
        assert!(!is_valid_api_key_format(
            "sk_1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
        ));

        // Invalid: too short
        assert!(!is_valid_api_key_format("ak_123"));

        // Invalid: no prefix
        assert!(!is_valid_api_key_format(
            "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
        ));
    }
}
