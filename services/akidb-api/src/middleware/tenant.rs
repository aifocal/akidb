use akidb_core::{TenantDescriptor, TenantStatus};
use axum::{
    extract::Request,
    http::{header, HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{debug, warn};

/// Tenant ID extracted from request
#[derive(Debug, Clone)]
pub struct TenantContext {
    pub tenant_id: String,
}

/// Tenant enforcement state
///
/// Holds tenant descriptors for status checking.
/// In production, this would query a database instead of in-memory map.
#[derive(Clone)]
pub struct TenantEnforcementState {
    tenants: Arc<RwLock<HashMap<String, TenantDescriptor>>>,
}

impl TenantEnforcementState {
    /// Create new tenant enforcement state
    pub fn new() -> Self {
        Self {
            tenants: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add or update a tenant (for testing/demo)
    pub fn upsert_tenant(&self, tenant: TenantDescriptor) {
        // BUGFIX (Bug #26): Handle poisoned lock gracefully instead of panicking.
        // If another thread panicked while holding this lock, we can still recover
        // the data by using into_inner().
        let mut guard = match self.tenants.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                warn!("Tenant lock was poisoned during upsert, recovering data...");
                poisoned.into_inner()
            }
        };
        guard.insert(tenant.tenant_id.clone(), tenant);
    }

    /// Get tenant by ID
    pub fn get_tenant(&self, tenant_id: &str) -> Option<TenantDescriptor> {
        // BUGFIX (Bug #26): Handle poisoned lock gracefully instead of panicking
        let guard = match self.tenants.read() {
            Ok(guard) => guard,
            Err(poisoned) => {
                warn!("Tenant lock was poisoned during get, recovering data...");
                poisoned.into_inner()
            }
        };
        guard.get(tenant_id).cloned()
    }

    /// Check if tenant is active
    pub fn is_tenant_active(&self, tenant_id: &str) -> bool {
        self.get_tenant(tenant_id)
            .map(|t| t.status == TenantStatus::Active)
            .unwrap_or(true) // If tenant not in store, allow (for backward compatibility)
    }

    /// Get tenant status
    pub fn get_tenant_status(&self, tenant_id: &str) -> Option<TenantStatus> {
        self.get_tenant(tenant_id).map(|t| t.status)
    }
}

impl Default for TenantEnforcementState {
    fn default() -> Self {
        Self::new()
    }
}

/// Extension key for tenant context
pub const TENANT_CONTEXT_KEY: &str = "X-Tenant-ID";

/// Tenant context middleware
///
/// Extracts tenant ID from request headers and injects into request extensions.
/// Supports multiple methods:
/// 1. X-Tenant-ID header (primary)
/// 2. Authorization Bearer token (JWT tenant claim)
/// 3. Query parameter ?tenant_id= (fallback, less secure)
///
/// CRITICAL SECURITY FIX (Bug #50): REQUIRES tenant ID in all requests.
/// No default fallback to prevent cross-tenant access attacks.
pub async fn tenant_context_middleware(
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    // Only bypass tenant requirement for liveness probe
    if request.uri().path() == "/health/live" {
        let context = TenantContext {
            tenant_id: "system".to_string(),
        };
        request.extensions_mut().insert(context);
        return Ok(next.run(request).await);
    }

    let tenant_id = extract_tenant_id(&headers, &request)?;

    debug!("Request tenant: {}", tenant_id);

    // Inject tenant context into request extensions
    let context = TenantContext {
        tenant_id: tenant_id.clone(),
    };
    request.extensions_mut().insert(context);

    Ok(next.run(request).await)
}

/// Extract tenant ID from request
///
/// CRITICAL SECURITY FIX (Bug #50): No longer falls back to "default" tenant.
/// Returns error if no tenant ID is provided, preventing cross-tenant access attacks.
fn extract_tenant_id(
    headers: &HeaderMap,
    request: &Request,
) -> Result<String, (StatusCode, String)> {
    // 1. Check X-Tenant-ID header (primary method)
    if let Some(tenant_id) = headers.get(TENANT_CONTEXT_KEY) {
        if let Ok(id) = tenant_id.to_str() {
            if !id.is_empty() {
                return Ok(id.to_string());
            }
        }
    }

    // 2. Check Authorization header for tenant hint
    // Format: Bearer ak_tenant123_...
    if let Some(auth) = headers.get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth.to_str() {
            if let Some(bearer_stripped) = auth_str.strip_prefix("Bearer ") {
                if let Some(api_key) = bearer_stripped.strip_prefix("ak_") {
                    // API key format: ak_{tenant_id}_{random}
                    // Find last underscore to separate tenant_id from random suffix
                    if let Some(last_underscore) = api_key.rfind('_') {
                        let tenant_id = &api_key[..last_underscore];
                        if !tenant_id.is_empty() {
                            return Ok(tenant_id.to_string());
                        }
                    }
                }
            }
        }
    }

    // 3. Check query parameter (fallback, less secure)
    // WARNING: Query parameters are logged in access logs and browser history
    // Only use this for development/testing, not production
    if let Some(query) = request.uri().query() {
        for param in query.split('&') {
            if let Some((key, value)) = param.split_once('=') {
                if key == "tenant_id" && !value.is_empty() {
                    warn!(
                        "Tenant ID extracted from query parameter (insecure): {}",
                        value
                    );
                    return Ok(value.to_string());
                }
            }
        }
    }

    // SECURITY: No default fallback - require explicit tenant ID
    warn!(
        "Missing tenant ID in request to {}. Provide via X-Tenant-ID header.",
        request.uri().path()
    );
    Err((
        StatusCode::BAD_REQUEST,
        "Missing required X-Tenant-ID header. Tenant ID must be explicitly provided.".to_string(),
    ))
}

/// Tenant isolation enforcement middleware
///
/// CRITICAL SECURITY FIX (Bug #4): Enforces tenant status checking.
///
/// Ensures tenant context exists and tenant is active.
/// Returns 403 FORBIDDEN if tenant is suspended or deleted.
///
/// # Tenant Status Enforcement
///
/// - **Active**: Request allowed to proceed
/// - **Suspended**: Returns 403 with "Tenant suspended" message
/// - **Deleted**: Returns 403 with "Tenant deleted" message
/// - **Not Found**: Allowed for backward compatibility (assumes active)
///
/// In production, this would query a database-backed TenantStore.
pub async fn tenant_enforcement_middleware(
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

    // Get tenant enforcement state from request extensions
    let tenant_state = request
        .extensions()
        .get::<TenantEnforcementState>()
        .cloned();

    // If enforcement state is configured, check tenant status
    if let Some(state) = tenant_state {
        match state.get_tenant_status(&tenant_context.tenant_id) {
            Some(TenantStatus::Active) => {
                debug!(
                    "Tenant enforcement passed: {} is active",
                    tenant_context.tenant_id
                );
            }
            Some(TenantStatus::Suspended) => {
                warn!(
                    "Access denied: Tenant {} is suspended",
                    tenant_context.tenant_id
                );
                return Err((
                    StatusCode::FORBIDDEN,
                    format!("Tenant '{}' is suspended. Please contact support to reactivate your account.", tenant_context.tenant_id),
                ));
            }
            Some(TenantStatus::Deleted) => {
                warn!(
                    "Access denied: Tenant {} is deleted",
                    tenant_context.tenant_id
                );
                return Err((
                    StatusCode::FORBIDDEN,
                    format!("Tenant '{}' has been deleted. Please contact support if you believe this is an error.", tenant_context.tenant_id),
                ));
            }
            None => {
                // Tenant not found in store - allow for backward compatibility
                // In production with database, this would be an error
                debug!(
                    "Tenant {} not in enforcement store, allowing (backward compatibility)",
                    tenant_context.tenant_id
                );
            }
        }
    } else {
        // No enforcement state configured - allow for backward compatibility
        debug!("Tenant enforcement state not configured, skipping status check");
    }

    Ok(next.run(request).await)
}

/// Helper middleware to inject tenant enforcement state into requests
pub async fn inject_tenant_enforcement_state(
    tenant_state: TenantEnforcementState,
    mut request: Request,
    next: Next,
) -> Response {
    request.extensions_mut().insert(tenant_state);
    next.run(request).await
}

/// Helper to extract tenant context from request extensions
pub fn get_tenant_context(request: &Request) -> Option<TenantContext> {
    request.extensions().get::<TenantContext>().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{header, Method, Request},
    };

    #[test]
    fn test_extract_tenant_from_header() {
        let mut headers = HeaderMap::new();
        headers.insert(TENANT_CONTEXT_KEY, "tenant_123".parse().unwrap());

        let request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let tenant_id = extract_tenant_id(&headers, &request).unwrap();
        assert_eq!(tenant_id, "tenant_123");
    }

    #[test]
    fn test_extract_tenant_from_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            "Bearer ak_tenant123_random".parse().unwrap(),
        );

        let request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let tenant_id = extract_tenant_id(&headers, &request).unwrap();
        assert_eq!(tenant_id, "tenant123");
    }

    #[test]
    fn test_extract_tenant_from_query() {
        let headers = HeaderMap::new();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/test?tenant_id=tenant_456")
            .body(Body::empty())
            .unwrap();

        let tenant_id = extract_tenant_id(&headers, &request).unwrap();
        assert_eq!(tenant_id, "tenant_456");
    }

    #[test]
    fn test_missing_tenant_id_returns_error() {
        // SECURITY FIX (Bug #50): No longer accepts requests without tenant ID
        let headers = HeaderMap::new();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let result = extract_tenant_id(&headers, &request);
        assert!(result.is_err());

        if let Err((status, message)) = result {
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert!(message.contains("X-Tenant-ID"));
        }
    }

    #[test]
    fn test_header_priority() {
        // Header should take priority over query param
        let mut headers = HeaderMap::new();
        headers.insert(TENANT_CONTEXT_KEY, "tenant_header".parse().unwrap());

        let request = Request::builder()
            .method(Method::GET)
            .uri("/test?tenant_id=tenant_query")
            .body(Body::empty())
            .unwrap();

        let tenant_id = extract_tenant_id(&headers, &request).unwrap();
        assert_eq!(tenant_id, "tenant_header");
    }
}
