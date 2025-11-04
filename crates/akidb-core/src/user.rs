use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

use crate::tenant::TenantId;

/// User ID (UUID format)
pub type UserId = String;

/// User descriptor with authentication and authorization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct User {
    /// Unique user identifier
    pub user_id: UserId,
    /// Username for login
    pub username: String,
    /// Email address
    pub email: String,
    /// Password hash (bcrypt or argon2)
    pub password_hash: String,
    /// Tenant this user belongs to
    pub tenant_id: TenantId,
    /// Assigned roles
    pub roles: Vec<RoleId>,
    /// User status
    pub status: UserStatus,
    /// User metadata
    pub metadata: UserMetadata,
    /// Account created timestamp
    pub created_at: DateTime<Utc>,
    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,
    /// Last login timestamp
    pub last_login: Option<DateTime<Utc>>,
}

impl User {
    /// Create a new user
    pub fn new(
        username: String,
        email: String,
        password_hash: String,
        tenant_id: TenantId,
    ) -> Self {
        let now = Utc::now();
        Self {
            user_id: uuid::Uuid::new_v4().to_string(),
            username,
            email,
            password_hash,
            tenant_id,
            roles: vec![],
            status: UserStatus::Active,
            metadata: UserMetadata::default(),
            created_at: now,
            updated_at: now,
            last_login: None,
        }
    }

    /// Add a role to the user
    pub fn add_role(&mut self, role_id: RoleId) {
        if !self.roles.contains(&role_id) {
            self.roles.push(role_id);
            self.updated_at = Utc::now();
        }
    }

    /// Remove a role from the user
    pub fn remove_role(&mut self, role_id: &RoleId) {
        if let Some(pos) = self.roles.iter().position(|r| r == role_id) {
            self.roles.remove(pos);
            self.updated_at = Utc::now();
        }
    }

    /// Check if user has a specific role
    pub fn has_role(&self, role_id: &RoleId) -> bool {
        self.roles.contains(role_id)
    }

    /// Update last login timestamp
    pub fn update_last_login(&mut self) {
        self.last_login = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    /// Check if user is active
    pub fn is_active(&self) -> bool {
        self.status == UserStatus::Active
    }

    /// Validate user data
    pub fn validate(&self) -> Result<(), UserError> {
        if self.username.is_empty() {
            return Err(UserError::InvalidUsername(
                "Username cannot be empty".to_string(),
            ));
        }

        // CRITICAL FIX (Bug #7): Improved email validation beyond simple '@' check
        //
        // ISSUE: Previous validation only checked for '@' symbol, accepting invalid formats like:
        // - "@@example.com" (multiple @ symbols)
        // - "@" (just @ symbol)
        // - "user@" (missing domain)
        // - "user@domain" (missing TLD)
        // - "user name@example.com" (spaces)
        //
        // FIX: RFC 5322-inspired validation (basic subset):
        // 1. Exactly one '@' symbol
        // 2. Local part (before @) must be non-empty and <= 64 chars
        // 3. Domain part (after @) must be non-empty and contain at least one '.'
        // 4. No whitespace allowed
        // 5. Basic character validation
        if self.email.is_empty() {
            return Err(UserError::InvalidEmail(
                "Email address cannot be empty".to_string(),
            ));
        }

        if self.email.contains(char::is_whitespace) {
            return Err(UserError::InvalidEmail(
                "Email address cannot contain whitespace".to_string(),
            ));
        }

        let parts: Vec<&str> = self.email.split('@').collect();
        if parts.len() != 2 {
            return Err(UserError::InvalidEmail(
                "Email must contain exactly one '@' symbol".to_string(),
            ));
        }

        let local_part = parts[0];
        let domain_part = parts[1];

        // Validate local part (before @)
        if local_part.is_empty() {
            return Err(UserError::InvalidEmail(
                "Email local part (before @) cannot be empty".to_string(),
            ));
        }

        if local_part.len() > 64 {
            return Err(UserError::InvalidEmail(
                "Email local part (before @) exceeds 64 characters".to_string(),
            ));
        }

        // Validate domain part (after @)
        if domain_part.is_empty() {
            return Err(UserError::InvalidEmail(
                "Email domain (after @) cannot be empty".to_string(),
            ));
        }

        if !domain_part.contains('.') {
            return Err(UserError::InvalidEmail(
                "Email domain must contain at least one '.' for TLD".to_string(),
            ));
        }

        // Validate domain has content before and after last dot (prevents "user@example." or "user@.com")
        if domain_part.starts_with('.') || domain_part.ends_with('.') {
            return Err(UserError::InvalidEmail(
                "Email domain cannot start or end with '.'".to_string(),
            ));
        }

        // Check for consecutive dots in domain
        if domain_part.contains("..") {
            return Err(UserError::InvalidEmail(
                "Email domain cannot contain consecutive dots".to_string(),
            ));
        }

        if self.password_hash.is_empty() {
            return Err(UserError::InvalidPassword(
                "Password hash cannot be empty".to_string(),
            ));
        }

        Ok(())
    }
}

/// User status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserStatus {
    /// Active user, can authenticate
    Active,
    /// Suspended user, cannot authenticate
    Suspended,
    /// Deleted user (soft delete)
    Deleted,
}

impl fmt::Display for UserStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserStatus::Active => write!(f, "active"),
            UserStatus::Suspended => write!(f, "suspended"),
            UserStatus::Deleted => write!(f, "deleted"),
        }
    }
}

/// User metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UserMetadata {
    /// Display name
    pub display_name: Option<String>,
    /// User's preferred language
    pub language: Option<String>,
    /// User's timezone
    pub timezone: Option<String>,
    /// Custom key-value pairs
    #[serde(flatten)]
    pub custom: std::collections::HashMap<String, String>,
}

/// Role ID
pub type RoleId = String;

/// Role descriptor with permissions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Role {
    /// Unique role identifier
    pub role_id: RoleId,
    /// Role name (e.g., "admin", "developer", "viewer")
    pub name: String,
    /// Role description
    pub description: String,
    /// Tenant this role belongs to
    pub tenant_id: TenantId,
    /// Permissions granted by this role
    pub permissions: HashSet<Permission>,
    /// Role metadata
    pub metadata: RoleMetadata,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,
}

impl Role {
    /// Create a new role
    pub fn new(name: String, description: String, tenant_id: TenantId) -> Self {
        let now = Utc::now();
        Self {
            role_id: uuid::Uuid::new_v4().to_string(),
            name,
            description,
            tenant_id,
            permissions: HashSet::new(),
            metadata: RoleMetadata::default(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Add a permission to the role
    pub fn add_permission(&mut self, permission: Permission) {
        self.permissions.insert(permission);
        self.updated_at = Utc::now();
    }

    /// Remove a permission from the role
    pub fn remove_permission(&mut self, permission: &Permission) {
        self.permissions.remove(permission);
        self.updated_at = Utc::now();
    }

    /// Check if role has a specific permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission)
    }

    /// Check if role has all permissions
    pub fn has_all_permissions(&self, permissions: &[Permission]) -> bool {
        permissions.iter().all(|p| self.permissions.contains(p))
    }

    /// Check if role has any of the permissions
    pub fn has_any_permission(&self, permissions: &[Permission]) -> bool {
        permissions.iter().any(|p| self.permissions.contains(p))
    }

    /// Validate role data
    pub fn validate(&self) -> Result<(), UserError> {
        if self.name.is_empty() {
            return Err(UserError::InvalidRole(
                "Role name cannot be empty".to_string(),
            ));
        }

        Ok(())
    }
}

/// Role metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RoleMetadata {
    /// Is this a system role (cannot be deleted)
    pub is_system: bool,
    /// Custom key-value pairs
    #[serde(flatten)]
    pub custom: std::collections::HashMap<String, String>,
}

/// Permission enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    // Collection permissions
    CollectionCreate,
    CollectionRead,
    CollectionUpdate,
    CollectionDelete,
    CollectionList,

    // Vector permissions
    VectorInsert,
    VectorSearch,
    VectorUpdate,
    VectorDelete,

    // Index permissions
    IndexCreate,
    IndexRebuild,
    IndexDelete,

    // Tenant permissions
    TenantRead,
    TenantUpdate,
    TenantDelete,

    // User permissions
    UserCreate,
    UserRead,
    UserUpdate,
    UserDelete,
    UserList,

    // Role permissions
    RoleCreate,
    RoleRead,
    RoleUpdate,
    RoleDelete,
    RoleList,

    // System permissions
    SystemMetrics,
    SystemHealth,
    SystemConfig,

    // Admin permission (grants all)
    Admin,
}

impl Permission {
    /// Get all permissions
    pub fn all() -> Vec<Permission> {
        vec![
            Permission::CollectionCreate,
            Permission::CollectionRead,
            Permission::CollectionUpdate,
            Permission::CollectionDelete,
            Permission::CollectionList,
            Permission::VectorInsert,
            Permission::VectorSearch,
            Permission::VectorUpdate,
            Permission::VectorDelete,
            Permission::IndexCreate,
            Permission::IndexRebuild,
            Permission::IndexDelete,
            Permission::TenantRead,
            Permission::TenantUpdate,
            Permission::TenantDelete,
            Permission::UserCreate,
            Permission::UserRead,
            Permission::UserUpdate,
            Permission::UserDelete,
            Permission::UserList,
            Permission::RoleCreate,
            Permission::RoleRead,
            Permission::RoleUpdate,
            Permission::RoleDelete,
            Permission::RoleList,
            Permission::SystemMetrics,
            Permission::SystemHealth,
            Permission::SystemConfig,
            Permission::Admin,
        ]
    }

    /// Get read-only permissions
    pub fn read_only() -> Vec<Permission> {
        vec![
            Permission::CollectionRead,
            Permission::CollectionList,
            Permission::VectorSearch,
            Permission::SystemMetrics,
            Permission::SystemHealth,
        ]
    }

    /// Get developer permissions (read + write data)
    pub fn developer() -> Vec<Permission> {
        vec![
            Permission::CollectionCreate,
            Permission::CollectionRead,
            Permission::CollectionList,
            Permission::VectorInsert,
            Permission::VectorSearch,
            Permission::VectorUpdate,
            Permission::IndexCreate,
            Permission::IndexRebuild,
            Permission::SystemMetrics,
            Permission::SystemHealth,
        ]
    }

    /// Get admin permissions (all)
    pub fn admin() -> Vec<Permission> {
        vec![Permission::Admin]
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Permission::CollectionCreate => "collection:create",
            Permission::CollectionRead => "collection:read",
            Permission::CollectionUpdate => "collection:update",
            Permission::CollectionDelete => "collection:delete",
            Permission::CollectionList => "collection:list",
            Permission::VectorInsert => "vector:insert",
            Permission::VectorSearch => "vector:search",
            Permission::VectorUpdate => "vector:update",
            Permission::VectorDelete => "vector:delete",
            Permission::IndexCreate => "index:create",
            Permission::IndexRebuild => "index:rebuild",
            Permission::IndexDelete => "index:delete",
            Permission::TenantRead => "tenant:read",
            Permission::TenantUpdate => "tenant:update",
            Permission::TenantDelete => "tenant:delete",
            Permission::UserCreate => "user:create",
            Permission::UserRead => "user:read",
            Permission::UserUpdate => "user:update",
            Permission::UserDelete => "user:delete",
            Permission::UserList => "user:list",
            Permission::RoleCreate => "role:create",
            Permission::RoleRead => "role:read",
            Permission::RoleUpdate => "role:update",
            Permission::RoleDelete => "role:delete",
            Permission::RoleList => "role:list",
            Permission::SystemMetrics => "system:metrics",
            Permission::SystemHealth => "system:health",
            Permission::SystemConfig => "system:config",
            Permission::Admin => "admin",
        };
        write!(f, "{}", s)
    }
}

/// User-related errors
#[derive(Debug, Clone, thiserror::Error)]
pub enum UserError {
    #[error("User not found: {0}")]
    NotFound(String),

    #[error("User already exists: {0}")]
    AlreadyExists(String),

    #[error("Invalid username: {0}")]
    InvalidUsername(String),

    #[error("Invalid email: {0}")]
    InvalidEmail(String),

    #[error("Invalid password: {0}")]
    InvalidPassword(String),

    #[error("Invalid role: {0}")]
    InvalidRole(String),

    #[error("Role not found: {0}")]
    RoleNotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("User suspended: {0}")]
    UserSuspended(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Helper to hash passwords securely using Argon2
pub mod password {
    use argon2::{
        password_hash::{
            rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
        },
        Argon2,
    };

    /// Hash a password using Argon2id
    ///
    /// Returns the PHC string format hash that includes the algorithm, parameters, salt, and hash.
    ///
    /// # Errors
    ///
    /// Returns an error if the password cannot be hashed (e.g., invalid UTF-8).
    pub fn hash(password: &str) -> Result<String, String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        argon2
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|e| format!("Password hashing failed: {}", e))
    }

    /// Verify a password against an Argon2 hash
    ///
    /// Uses constant-time comparison to prevent timing attacks.
    ///
    /// # Errors
    ///
    /// Returns false if the password doesn't match or if the hash format is invalid.
    pub fn verify(password: &str, password_hash: &str) -> bool {
        let parsed_hash = match PasswordHash::new(password_hash) {
            Ok(hash) => hash,
            Err(_) => return false,
        };

        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok()
    }
}

/// Pre-defined roles
impl Role {
    /// Create admin role with all permissions
    pub fn admin(tenant_id: TenantId) -> Self {
        let mut role = Role::new(
            "admin".to_string(),
            "Administrator with full access".to_string(),
            tenant_id,
        );
        for perm in Permission::admin() {
            role.add_permission(perm);
        }
        role.metadata.is_system = true;
        role
    }

    /// Create developer role with data access permissions
    pub fn developer(tenant_id: TenantId) -> Self {
        let mut role = Role::new(
            "developer".to_string(),
            "Developer with read/write access to collections and vectors".to_string(),
            tenant_id,
        );
        for perm in Permission::developer() {
            role.add_permission(perm);
        }
        role.metadata.is_system = true;
        role
    }

    /// Create viewer role with read-only permissions
    pub fn viewer(tenant_id: TenantId) -> Self {
        let mut role = Role::new(
            "viewer".to_string(),
            "Viewer with read-only access".to_string(),
            tenant_id,
        );
        for perm in Permission::read_only() {
            role.add_permission(perm);
        }
        role.metadata.is_system = true;
        role
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_creation() {
        let user = User::new(
            "testuser".to_string(),
            "test@example.com".to_string(),
            "hashed_password".to_string(),
            "tenant_test".to_string(),
        );

        assert_eq!(user.username, "testuser");
        assert_eq!(user.email, "test@example.com");
        assert_eq!(user.tenant_id, "tenant_test");
        assert_eq!(user.status, UserStatus::Active);
        assert!(user.roles.is_empty());
    }

    #[test]
    fn test_user_role_management() {
        let mut user = User::new(
            "testuser".to_string(),
            "test@example.com".to_string(),
            "hashed_password".to_string(),
            "tenant_test".to_string(),
        );

        let role_id = "role_1".to_string();
        user.add_role(role_id.clone());

        assert!(user.has_role(&role_id));
        assert_eq!(user.roles.len(), 1);

        user.remove_role(&role_id);
        assert!(!user.has_role(&role_id));
        assert_eq!(user.roles.len(), 0);
    }

    #[test]
    fn test_user_validation() {
        let mut user = User::new(
            "testuser".to_string(),
            "test@example.com".to_string(),
            "hashed_password".to_string(),
            "tenant_test".to_string(),
        );

        assert!(user.validate().is_ok());

        user.username = String::new();
        assert!(user.validate().is_err());
    }

    #[test]
    fn test_role_creation() {
        let role = Role::new(
            "developer".to_string(),
            "Developer role".to_string(),
            "tenant_test".to_string(),
        );

        assert_eq!(role.name, "developer");
        assert_eq!(role.tenant_id, "tenant_test");
        assert!(role.permissions.is_empty());
    }

    #[test]
    fn test_role_permission_management() {
        let mut role = Role::new(
            "developer".to_string(),
            "Developer role".to_string(),
            "tenant_test".to_string(),
        );

        role.add_permission(Permission::CollectionRead);
        role.add_permission(Permission::VectorSearch);

        assert!(role.has_permission(&Permission::CollectionRead));
        assert!(role.has_permission(&Permission::VectorSearch));
        assert!(!role.has_permission(&Permission::CollectionDelete));

        role.remove_permission(&Permission::CollectionRead);
        assert!(!role.has_permission(&Permission::CollectionRead));
    }

    #[test]
    fn test_role_permission_queries() {
        let mut role = Role::new(
            "developer".to_string(),
            "Developer role".to_string(),
            "tenant_test".to_string(),
        );

        role.add_permission(Permission::CollectionRead);
        role.add_permission(Permission::VectorSearch);

        assert!(role.has_all_permissions(&[Permission::CollectionRead, Permission::VectorSearch]));

        assert!(
            !role.has_all_permissions(&[Permission::CollectionRead, Permission::CollectionDelete])
        );

        assert!(
            role.has_any_permission(&[Permission::CollectionRead, Permission::CollectionDelete])
        );
    }

    #[test]
    fn test_predefined_admin_role() {
        let role = Role::admin("tenant_test".to_string());

        assert_eq!(role.name, "admin");
        assert!(role.metadata.is_system);
        assert!(role.has_permission(&Permission::Admin));
    }

    #[test]
    fn test_predefined_developer_role() {
        let role = Role::developer("tenant_test".to_string());

        assert_eq!(role.name, "developer");
        assert!(role.metadata.is_system);
        assert!(role.has_permission(&Permission::CollectionCreate));
        assert!(role.has_permission(&Permission::VectorInsert));
        assert!(!role.has_permission(&Permission::Admin));
    }

    #[test]
    fn test_predefined_viewer_role() {
        let role = Role::viewer("tenant_test".to_string());

        assert_eq!(role.name, "viewer");
        assert!(role.metadata.is_system);
        assert!(role.has_permission(&Permission::CollectionRead));
        assert!(role.has_permission(&Permission::VectorSearch));
        assert!(!role.has_permission(&Permission::CollectionCreate));
    }

    #[test]
    fn test_password_hashing() {
        let password = "test_password_123";
        let hash = password::hash(password).expect("Failed to hash password");

        // Hash should be in PHC string format (starts with $)
        assert!(hash.starts_with("$argon2"));
        assert_ne!(password, hash);

        // Verify correct password
        assert!(password::verify(password, &hash));

        // Verify wrong password
        assert!(!password::verify("wrong_password", &hash));

        // Verify invalid hash format
        assert!(!password::verify(password, "invalid_hash"));
    }

    #[test]
    fn test_permission_display() {
        assert_eq!(
            Permission::CollectionCreate.to_string(),
            "collection:create"
        );
        assert_eq!(Permission::VectorSearch.to_string(), "vector:search");
        assert_eq!(Permission::Admin.to_string(), "admin");
    }

    #[test]
    fn test_permission_categories() {
        let read_only = Permission::read_only();
        assert!(read_only.contains(&Permission::CollectionRead));
        assert!(!read_only.contains(&Permission::CollectionCreate));

        let developer = Permission::developer();
        assert!(developer.contains(&Permission::CollectionCreate));
        assert!(developer.contains(&Permission::VectorInsert));

        let admin = Permission::admin();
        assert_eq!(admin.len(), 1);
        assert_eq!(admin[0], Permission::Admin);
    }
}
