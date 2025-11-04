use akidb_core::{QuotaTracker, TenantError};
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use tracing::{debug, warn};

use super::tenant::TenantContext;

/// Quota enforcement middleware state
#[derive(Clone)]
pub struct QuotaEnforcementState {
    pub tracker: Arc<QuotaTracker>,
}

impl QuotaEnforcementState {
    pub fn new(tracker: Arc<QuotaTracker>) -> Self {
        Self { tracker }
    }
}

/// Quota enforcement middleware
///
/// Checks resource quotas and rate limits before allowing requests to proceed.
/// Must be used after tenant_context_middleware to ensure tenant ID is available.
pub async fn quota_enforcement_middleware(
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

    // Get quota tracker from request extensions
    let quota_state = request
        .extensions()
        .get::<QuotaEnforcementState>()
        .cloned()
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Quota tracker not configured".to_string(),
        ))?;

    // Check rate limit before processing request
    match quota_state
        .tracker
        .check_rate_limit(&tenant_context.tenant_id)
    {
        Ok(_) => {
            debug!(
                "Rate limit check passed for tenant: {}",
                tenant_context.tenant_id
            );
        }
        Err(TenantError::QuotaExceeded { quota_type }) => {
            warn!(
                "Rate limit exceeded for tenant {}: {}",
                tenant_context.tenant_id, quota_type
            );
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                format!("Rate limit exceeded: {}", quota_type),
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Quota check failed: {}", e),
            ));
        }
    }

    // Increment API request counter
    quota_state
        .tracker
        .increment_api_requests(tenant_context.tenant_id.clone());

    // Allow request to proceed
    let response = next.run(request).await;

    Ok(response)
}

/// Helper middleware to inject quota state into requests
pub async fn inject_quota_state(
    quota_state: QuotaEnforcementState,
    mut request: Request,
    next: Next,
) -> Response {
    request.extensions_mut().insert(quota_state);
    next.run(request).await
}

/// Storage quota check helper
///
/// Use this in handlers that write data to check storage quotas.
pub async fn check_storage_quota(
    tracker: &QuotaTracker,
    tenant_id: &str,
    additional_bytes: u64,
) -> Result<(), (StatusCode, String)> {
    match tracker.check_storage_quota(tenant_id, additional_bytes) {
        Ok(_) => Ok(()),
        Err(TenantError::QuotaExceeded { quota_type }) => Err((
            StatusCode::INSUFFICIENT_STORAGE,
            format!("Storage quota exceeded: {}", quota_type),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Quota check failed: {}", e),
        )),
    }
}

/// Collection quota check helper
///
/// Use this in handlers that create collections.
pub async fn check_collection_quota(
    tracker: &QuotaTracker,
    tenant_id: &str,
) -> Result<(), (StatusCode, String)> {
    match tracker.check_collection_quota(tenant_id) {
        Ok(_) => Ok(()),
        Err(TenantError::QuotaExceeded { quota_type }) => Err((
            StatusCode::FORBIDDEN,
            format!("Collection quota exceeded: {}", quota_type),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Quota check failed: {}", e),
        )),
    }
}

/// Vector quota check helper
///
/// Use this in handlers that insert vectors.
pub async fn check_vector_quota(
    tracker: &QuotaTracker,
    tenant_id: &str,
    collection_vectors: u64,
    additional_vectors: u64,
) -> Result<(), (StatusCode, String)> {
    match tracker.check_vector_quota(tenant_id, collection_vectors, additional_vectors) {
        Ok(_) => Ok(()),
        Err(TenantError::QuotaExceeded { quota_type }) => Err((
            StatusCode::FORBIDDEN,
            format!("Vector quota exceeded: {}", quota_type),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Quota check failed: {}", e),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use akidb_core::{TenantQuota, TenantUsage};
    use axum::{
        body::Body,
        http::{Method, Request},
    };

    #[test]
    fn test_quota_state_creation() {
        let tracker = Arc::new(QuotaTracker::new());
        let state = QuotaEnforcementState::new(tracker.clone());
        assert!(Arc::ptr_eq(&state.tracker, &tracker));
    }

    #[tokio::test]
    async fn test_check_storage_quota_ok() {
        let tracker = QuotaTracker::new();
        let tenant_id = "tenant_test";

        let mut quota = TenantQuota::default();
        quota.max_storage_bytes = 10000;
        tracker.set_quota(tenant_id.to_string(), quota);

        let result = check_storage_quota(&tracker, tenant_id, 5000).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_check_storage_quota_exceeded() {
        let tracker = QuotaTracker::new();
        let tenant_id = "tenant_test";

        let mut quota = TenantQuota::default();
        quota.max_storage_bytes = 1000;
        tracker.set_quota(tenant_id.to_string(), quota);
        tracker.update_storage(tenant_id.to_string(), 800);

        let result = check_storage_quota(&tracker, tenant_id, 300).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().0, StatusCode::INSUFFICIENT_STORAGE);
    }

    #[tokio::test]
    async fn test_check_collection_quota_ok() {
        let tracker = QuotaTracker::new();
        let tenant_id = "tenant_test";

        let mut quota = TenantQuota::default();
        quota.max_collections = 10;
        tracker.set_quota(tenant_id.to_string(), quota);
        tracker.update_collections(tenant_id.to_string(), 5);

        let result = check_collection_quota(&tracker, tenant_id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_check_collection_quota_exceeded() {
        let tracker = QuotaTracker::new();
        let tenant_id = "tenant_test";

        let mut quota = TenantQuota::default();
        quota.max_collections = 5;
        tracker.set_quota(tenant_id.to_string(), quota);
        tracker.update_collections(tenant_id.to_string(), 5);

        let result = check_collection_quota(&tracker, tenant_id).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().0, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_check_vector_quota_ok() {
        let tracker = QuotaTracker::new();
        let tenant_id = "tenant_test";

        let mut quota = TenantQuota::default();
        quota.max_vectors_per_collection = 10000;
        tracker.set_quota(tenant_id.to_string(), quota);

        let result = check_vector_quota(&tracker, tenant_id, 5000, 3000).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_check_vector_quota_exceeded() {
        let tracker = QuotaTracker::new();
        let tenant_id = "tenant_test";

        let mut quota = TenantQuota::default();
        quota.max_vectors_per_collection = 10000;
        tracker.set_quota(tenant_id.to_string(), quota);

        let result = check_vector_quota(&tracker, tenant_id, 9000, 2000).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().0, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_unlimited_quota() {
        let tracker = QuotaTracker::new();
        let tenant_id = "tenant_test";

        // Set unlimited quota (all zeros)
        tracker.set_quota(tenant_id.to_string(), TenantQuota::unlimited());

        // All checks should pass
        assert!(check_storage_quota(&tracker, tenant_id, u64::MAX / 2)
            .await
            .is_ok());
        assert!(check_collection_quota(&tracker, tenant_id).await.is_ok());
        assert!(
            check_vector_quota(&tracker, tenant_id, u64::MAX / 2, u64::MAX / 4)
                .await
                .is_ok()
        );
    }
}
