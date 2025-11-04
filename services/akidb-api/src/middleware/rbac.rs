use akidb_core::{Permission, Role, User};
use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, warn};

use super::tenant::TenantContext;

/// User context extracted from JWT or API key
#[derive(Debug, Clone)]
pub struct UserContext {
    pub user_id: String,
    pub username: String,
    pub tenant_id: String,
    pub roles: Vec<String>,
    pub permissions: Vec<Permission>,
}

impl UserContext {
    /// Check if user has a specific permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        // Admin permission grants all
        if self.permissions.contains(&Permission::Admin) {
            return true;
        }

        self.permissions.contains(permission)
    }

    /// Check if user has all permissions
    pub fn has_all_permissions(&self, permissions: &[Permission]) -> bool {
        // Admin permission grants all
        if self.permissions.contains(&Permission::Admin) {
            return true;
        }

        permissions.iter().all(|p| self.permissions.contains(p))
    }

    /// Check if user has any of the permissions
    pub fn has_any_permission(&self, permissions: &[Permission]) -> bool {
        // Admin permission grants all
        if self.permissions.contains(&Permission::Admin) {
            return true;
        }

        permissions.iter().any(|p| self.permissions.contains(p))
    }
}

/// JWT claims structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JwtClaims {
    /// Subject (user_id)
    sub: String,
    /// Username
    username: String,
    /// Tenant ID
    tenant_id: String,
    /// User roles
    roles: Vec<String>,
    /// Expiration time (Unix timestamp)
    exp: usize,
    /// Issued at (Unix timestamp)
    iat: usize,
}

/// RBAC enforcement state
#[derive(Clone)]
pub struct RbacEnforcementState {
    /// User store for looking up users and roles
    /// In production, this would be a proper database
    users: Arc<std::sync::RwLock<std::collections::HashMap<String, User>>>,
    roles: Arc<std::sync::RwLock<std::collections::HashMap<String, Role>>>,
    /// JWT secret key for token validation
    jwt_secret: Arc<String>,
}

impl RbacEnforcementState {
    pub fn new() -> Self {
        // Load JWT secret from environment or use default for development
        let jwt_secret = std::env::var("AKIDB_JWT_SECRET").unwrap_or_else(|_| {
            warn!("AKIDB_JWT_SECRET not set, using default (INSECURE for production)");
            "development_secret_change_in_production".to_string()
        });

        Self {
            users: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            roles: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            jwt_secret: Arc::new(jwt_secret),
        }
    }

    /// Create RBAC state with custom JWT secret
    pub fn with_jwt_secret(jwt_secret: String) -> Self {
        Self {
            users: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            roles: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            jwt_secret: Arc::new(jwt_secret),
        }
    }

    /// Add a user (for testing/demo)
    pub fn add_user(&self, user: User) {
        self.users
            .write()
            .expect("RBAC user lock poisoned")
            .insert(user.user_id.clone(), user);
    }

    /// Add a role (for testing/demo)
    pub fn add_role(&self, role: Role) {
        self.roles
            .write()
            .expect("RBAC role lock poisoned")
            .insert(role.role_id.clone(), role);
    }

    /// Get user by ID
    pub fn get_user(&self, user_id: &str) -> Option<User> {
        self.users
            .read()
            .expect("RBAC user lock poisoned")
            .get(user_id)
            .cloned()
    }

    /// Get role by ID
    pub fn get_role(&self, role_id: &str) -> Option<Role> {
        self.roles
            .read()
            .expect("RBAC role lock poisoned")
            .get(role_id)
            .cloned()
    }

    /// Get user permissions by resolving all roles
    pub fn get_user_permissions(&self, user: &User) -> Vec<Permission> {
        let mut permissions = std::collections::HashSet::new();

        for role_id in &user.roles {
            if let Some(role) = self.get_role(role_id) {
                permissions.extend(role.permissions.iter().copied());
            }
        }

        permissions.into_iter().collect()
    }

    /// Validate JWT token and extract claims
    ///
    /// CRITICAL SECURITY FIX (Bug #51): Replaces unsafe X-User-ID header trust
    /// with cryptographically validated JWT tokens.
    fn validate_jwt(&self, token: &str) -> Result<JwtClaims, String> {
        let decoding_key = DecodingKey::from_secret(self.jwt_secret.as_bytes());
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true; // Ensure token hasn't expired

        decode::<JwtClaims>(token, &decoding_key, &validation)
            .map(|token_data| token_data.claims)
            .map_err(|e| format!("Invalid JWT: {}", e))
    }
}

impl Default for RbacEnforcementState {
    fn default() -> Self {
        Self::new()
    }
}

/// RBAC middleware
///
/// Extracts user context from JWT token and enforces role-based access control.
/// Must be used after tenant_context_middleware to ensure tenant ID is available.
///
/// # JWT Token Format
///
/// Expected JWT claims:
/// ```json
/// {
///   "sub": "user_id",
///   "username": "john_doe",
///   "tenant_id": "tenant_123",
///   "roles": ["role_1", "role_2"]
/// }
/// ```
pub async fn rbac_middleware(
    request: Request,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    // Extract tenant context (should be injected by tenant_context_middleware)
    let tenant_context = request
        .extensions()
        .get::<TenantContext>()
        .cloned()
        .ok_or((
            StatusCode::UNAUTHORIZED,
            "Tenant context not found".to_string(),
        ))?;

    // Get RBAC state from request extensions
    let rbac_state = request
        .extensions()
        .get::<RbacEnforcementState>()
        .cloned()
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "RBAC state not configured".to_string(),
        ))?;

    // CRITICAL SECURITY FIX (Bug #51): Validate JWT token instead of trusting unsigned headers
    //
    // VULNERABILITY: Previous code blindly trusted X-User-ID header, allowing anyone with
    // a valid API key to impersonate any user by setting X-User-ID to their target user's ID.
    //
    // FIX: Extract and cryptographically validate JWT Bearer token from Authorization header.
    // Only trust user_id from validated, signed JWT tokens.

    // Extract JWT from Authorization Bearer header
    let jwt_token = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|auth| auth.strip_prefix("Bearer "))
        .ok_or((
            StatusCode::UNAUTHORIZED,
            "Missing Authorization: Bearer <jwt> header".to_string(),
        ))?;

    // Validate JWT and extract claims
    let claims = rbac_state.validate_jwt(jwt_token).map_err(|e| {
        warn!("JWT validation failed: {}", e);
        (
            StatusCode::UNAUTHORIZED,
            format!("Invalid JWT token: {}", e),
        )
    })?;

    let user_id = &claims.sub;

    // Get user from store
    let user = rbac_state.get_user(user_id).ok_or((
        StatusCode::UNAUTHORIZED,
        format!("User not found: {}", user_id),
    ))?;

    // Verify JWT claims match user record
    if user.username != claims.username {
        warn!(
            "JWT username mismatch: token={}, user={}",
            claims.username, user.username
        );
        return Err((
            StatusCode::UNAUTHORIZED,
            "JWT claims do not match user record".to_string(),
        ));
    }

    // Verify JWT tenant matches tenant context
    if claims.tenant_id != tenant_context.tenant_id {
        warn!(
            "JWT tenant mismatch: token={}, context={}",
            claims.tenant_id, tenant_context.tenant_id
        );
        return Err((
            StatusCode::FORBIDDEN,
            "JWT tenant does not match request tenant context".to_string(),
        ));
    }

    // Check if user is active
    if !user.is_active() {
        warn!("Inactive user attempted access: {}", user.user_id);
        return Err((
            StatusCode::FORBIDDEN,
            format!("User is not active: {}", user.status),
        ));
    }

    // Check if user belongs to the same tenant
    if user.tenant_id != tenant_context.tenant_id {
        warn!(
            "User {} attempted cross-tenant access: {} -> {}",
            user.user_id, user.tenant_id, tenant_context.tenant_id
        );
        return Err((
            StatusCode::FORBIDDEN,
            "Cross-tenant access denied".to_string(),
        ));
    }

    // Get user permissions
    let permissions = rbac_state.get_user_permissions(&user);

    // Create user context
    let user_context = UserContext {
        user_id: user.user_id.clone(),
        username: user.username.clone(),
        tenant_id: user.tenant_id.clone(),
        roles: user.roles.clone(),
        permissions,
    };

    debug!(
        "User {} authenticated with {} permissions",
        user_context.username,
        user_context.permissions.len()
    );

    // Inject user context into request
    let mut request = request;
    request.extensions_mut().insert(user_context);

    Ok(next.run(request).await)
}

/// Helper middleware to inject RBAC state into requests
pub async fn inject_rbac_state(
    rbac_state: RbacEnforcementState,
    mut request: Request,
    next: Next,
) -> Response {
    request.extensions_mut().insert(rbac_state);
    next.run(request).await
}

/// Permission check helper for use in handlers
///
/// Returns Err if user doesn't have required permission.
pub fn require_permission(
    user_context: &UserContext,
    permission: Permission,
) -> Result<(), (StatusCode, String)> {
    if !user_context.has_permission(&permission) {
        warn!(
            "User {} denied access: missing permission {:?}",
            user_context.username, permission
        );
        return Err((
            StatusCode::FORBIDDEN,
            format!("Permission denied: {:?}", permission),
        ));
    }

    Ok(())
}

/// Permission check helper for multiple permissions (all required)
pub fn require_all_permissions(
    user_context: &UserContext,
    permissions: &[Permission],
) -> Result<(), (StatusCode, String)> {
    if !user_context.has_all_permissions(permissions) {
        warn!(
            "User {} denied access: missing permissions {:?}",
            user_context.username, permissions
        );
        return Err((
            StatusCode::FORBIDDEN,
            format!("Permissions denied: {:?}", permissions),
        ));
    }

    Ok(())
}

/// Permission check helper for multiple permissions (any required)
pub fn require_any_permission(
    user_context: &UserContext,
    permissions: &[Permission],
) -> Result<(), (StatusCode, String)> {
    if !user_context.has_any_permission(permissions) {
        warn!(
            "User {} denied access: missing any of permissions {:?}",
            user_context.username, permissions
        );
        return Err((
            StatusCode::FORBIDDEN,
            format!("Permissions denied: {:?}", permissions),
        ));
    }

    Ok(())
}

/// Extract user context from request extensions
pub fn get_user_context(request: &Request) -> Option<UserContext> {
    request.extensions().get::<UserContext>().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use akidb_core::{Permission, Role, User};

    #[test]
    fn test_user_context_permissions() {
        let mut user_context = UserContext {
            user_id: "user_1".to_string(),
            username: "test_user".to_string(),
            tenant_id: "tenant_1".to_string(),
            roles: vec!["role_1".to_string()],
            permissions: vec![Permission::CollectionRead, Permission::VectorSearch],
        };

        assert!(user_context.has_permission(&Permission::CollectionRead));
        assert!(!user_context.has_permission(&Permission::CollectionCreate));

        assert!(user_context
            .has_all_permissions(&[Permission::CollectionRead, Permission::VectorSearch]));

        assert!(!user_context
            .has_all_permissions(&[Permission::CollectionRead, Permission::CollectionCreate]));

        assert!(user_context
            .has_any_permission(&[Permission::CollectionRead, Permission::CollectionCreate]));

        // Test admin permission
        user_context.permissions.push(Permission::Admin);
        assert!(user_context.has_permission(&Permission::CollectionCreate));
        assert!(user_context
            .has_all_permissions(&[Permission::CollectionCreate, Permission::CollectionDelete]));
    }

    #[test]
    fn test_rbac_state_basic() {
        let state = RbacEnforcementState::new();

        let user = User::new(
            "testuser".to_string(),
            "test@example.com".to_string(),
            "password_hash".to_string(),
            "tenant_1".to_string(),
        );

        state.add_user(user.clone());

        let retrieved = state.get_user(&user.user_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().username, "testuser");
    }

    #[test]
    fn test_rbac_state_role_permissions() {
        let state = RbacEnforcementState::new();

        let mut role = Role::new(
            "developer".to_string(),
            "Developer role".to_string(),
            "tenant_1".to_string(),
        );
        role.add_permission(Permission::CollectionRead);
        role.add_permission(Permission::VectorSearch);

        state.add_role(role.clone());

        let mut user = User::new(
            "testuser".to_string(),
            "test@example.com".to_string(),
            "password_hash".to_string(),
            "tenant_1".to_string(),
        );
        user.add_role(role.role_id.clone());

        state.add_user(user.clone());

        let permissions = state.get_user_permissions(&user);
        assert_eq!(permissions.len(), 2);
        assert!(permissions.contains(&Permission::CollectionRead));
        assert!(permissions.contains(&Permission::VectorSearch));
    }

    #[test]
    fn test_require_permission() {
        let user_context = UserContext {
            user_id: "user_1".to_string(),
            username: "test_user".to_string(),
            tenant_id: "tenant_1".to_string(),
            roles: vec![],
            permissions: vec![Permission::CollectionRead],
        };

        assert!(require_permission(&user_context, Permission::CollectionRead).is_ok());
        assert!(require_permission(&user_context, Permission::CollectionCreate).is_err());
    }

    #[test]
    fn test_require_all_permissions() {
        let user_context = UserContext {
            user_id: "user_1".to_string(),
            username: "test_user".to_string(),
            tenant_id: "tenant_1".to_string(),
            roles: vec![],
            permissions: vec![Permission::CollectionRead, Permission::VectorSearch],
        };

        assert!(require_all_permissions(
            &user_context,
            &[Permission::CollectionRead, Permission::VectorSearch]
        )
        .is_ok());

        assert!(require_all_permissions(
            &user_context,
            &[Permission::CollectionRead, Permission::CollectionCreate]
        )
        .is_err());
    }

    #[test]
    fn test_require_any_permission() {
        let user_context = UserContext {
            user_id: "user_1".to_string(),
            username: "test_user".to_string(),
            tenant_id: "tenant_1".to_string(),
            roles: vec![],
            permissions: vec![Permission::CollectionRead],
        };

        assert!(require_any_permission(
            &user_context,
            &[Permission::CollectionRead, Permission::CollectionCreate]
        )
        .is_ok());

        assert!(require_any_permission(
            &user_context,
            &[Permission::CollectionCreate, Permission::CollectionDelete]
        )
        .is_err());
    }
}
