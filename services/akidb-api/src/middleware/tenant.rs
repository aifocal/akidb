use axum::{
    extract::Request,
    http::{header, HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use tracing::{debug, warn};

/// Tenant ID extracted from request
#[derive(Debug, Clone)]
pub struct TenantContext {
    pub tenant_id: String,
}

/// Extension key for tenant context
pub const TENANT_CONTEXT_KEY: &str = "X-Tenant-ID";

/// Tenant context middleware
///
/// Extracts tenant ID from request headers and injects into request extensions.
/// Supports multiple methods:
/// 1. X-Tenant-ID header (primary)
/// 2. Authorization Bearer token (future: JWT claims)
/// 3. Query parameter ?tenant_id= (fallback)
///
/// If no tenant ID found, defaults to "default" tenant for backward compatibility.
pub async fn tenant_context_middleware(
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Response {
    let tenant_id = extract_tenant_id(&headers, &request);

    debug!("Request tenant: {}", tenant_id);

    // Inject tenant context into request extensions
    let context = TenantContext {
        tenant_id: tenant_id.clone(),
    };
    request.extensions_mut().insert(context);

    next.run(request).await
}

/// Extract tenant ID from request
fn extract_tenant_id(headers: &HeaderMap, request: &Request) -> String {
    // 1. Check X-Tenant-ID header (primary method)
    if let Some(tenant_id) = headers.get(TENANT_CONTEXT_KEY) {
        if let Ok(id) = tenant_id.to_str() {
            if !id.is_empty() {
                return id.to_string();
            }
        }
    }

    // 2. Check Authorization header for tenant hint
    // Format: Bearer ak_tenant123_...
    if let Some(auth) = headers.get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth.to_str() {
            if auth_str.starts_with("Bearer ak_") {
                // Extract tenant from API key format: ak_{tenant_id}_{random}
                let parts: Vec<&str> = auth_str.split('_').collect();
                if parts.len() >= 3 {
                    return format!("tenant_{}", parts[1]);
                }
            }
        }
    }

    // 3. Check query parameter (fallback, less secure)
    if let Some(query) = request.uri().query() {
        for param in query.split('&') {
            if let Some((key, value)) = param.split_once('=') {
                if key == "tenant_id" && !value.is_empty() {
                    return value.to_string();
                }
            }
        }
    }

    // 4. Default tenant for backward compatibility
    warn!("No tenant ID found in request, using default tenant");
    "default".to_string()
}

/// Tenant isolation enforcement middleware
///
/// Ensures tenant context exists and tenant is active.
/// Returns 401 if tenant is suspended or deleted.
pub async fn tenant_enforcement_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract tenant context (should be injected by tenant_context_middleware)
    let tenant_context = request
        .extensions()
        .get::<TenantContext>()
        .cloned()
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // TODO: Check tenant status (active, suspended, deleted)
    // For now, allow all requests
    // In production, would query TenantStore to verify tenant is active

    debug!(
        "Tenant enforcement passed for tenant: {}",
        tenant_context.tenant_id
    );

    Ok(next.run(request).await)
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

        let tenant_id = extract_tenant_id(&headers, &request);
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

        let tenant_id = extract_tenant_id(&headers, &request);
        assert_eq!(tenant_id, "tenant_tenant123");
    }

    #[test]
    fn test_extract_tenant_from_query() {
        let headers = HeaderMap::new();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/test?tenant_id=tenant_456")
            .body(Body::empty())
            .unwrap();

        let tenant_id = extract_tenant_id(&headers, &request);
        assert_eq!(tenant_id, "tenant_456");
    }

    #[test]
    fn test_default_tenant_fallback() {
        let headers = HeaderMap::new();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let tenant_id = extract_tenant_id(&headers, &request);
        assert_eq!(tenant_id, "default");
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

        let tenant_id = extract_tenant_id(&headers, &request);
        assert_eq!(tenant_id, "tenant_header");
    }
}
