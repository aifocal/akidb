use akidb_core::{QuotaTracker, TenantError};
use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
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

/// Storage quota check and increment helper (atomic operation)
///
/// Use this in handlers that write data to atomically check and increment storage usage.
/// This prevents race conditions where multiple threads could bypass quota limits.
pub async fn check_and_increment_storage(
    tracker: &QuotaTracker,
    tenant_id: &str,
    additional_bytes: u64,
) -> Result<(), (StatusCode, String)> {
    match tracker.check_and_increment_storage(tenant_id.to_string(), additional_bytes) {
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

/// Collection quota check and increment helper (atomic operation)
///
/// Use this in handlers that create collections to atomically check and increment collection count.
/// This prevents race conditions where multiple threads could bypass quota limits.
pub async fn check_and_increment_collection(
    tracker: &QuotaTracker,
    tenant_id: &str,
) -> Result<(), (StatusCode, String)> {
    match tracker.check_and_increment_collection(tenant_id.to_string()) {
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

/// Vector quota check and increment helper (atomic operation)
///
/// Use this in handlers that insert vectors to atomically check and increment vector count.
/// This prevents race conditions where multiple threads could bypass quota limits.
pub async fn check_and_increment_vectors(
    tracker: &QuotaTracker,
    tenant_id: &str,
    collection_vectors: u64,
    additional_vectors: u64,
) -> Result<(), (StatusCode, String)> {
    match tracker.check_and_increment_vectors(tenant_id.to_string(), collection_vectors, additional_vectors) {
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
    use akidb_core::TenantQuota;

    #[test]
    fn test_quota_state_creation() {
        let tracker = Arc::new(QuotaTracker::new());
        let state = QuotaEnforcementState::new(tracker.clone());
        assert!(Arc::ptr_eq(&state.tracker, &tracker));
    }

    #[tokio::test]
    async fn test_check_and_increment_storage_ok() {
        let tracker = QuotaTracker::new();
        let tenant_id = "tenant_test";

        let quota = TenantQuota {
            max_storage_bytes: 10000,
            ..Default::default()
        };
        tracker.set_quota(tenant_id.to_string(), quota);

        let result = check_and_increment_storage(&tracker, tenant_id, 5000).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_check_and_increment_storage_exceeded() {
        let tracker = QuotaTracker::new();
        let tenant_id = "tenant_test";

        let quota = TenantQuota {
            max_storage_bytes: 1000,
            ..Default::default()
        };
        tracker.set_quota(tenant_id.to_string(), quota);

        // First increment succeeds
        let result = check_and_increment_storage(&tracker, tenant_id, 800).await;
        assert!(result.is_ok());

        // Second increment exceeds quota
        let result = check_and_increment_storage(&tracker, tenant_id, 300).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().0, StatusCode::INSUFFICIENT_STORAGE);
    }

    #[tokio::test]
    async fn test_check_and_increment_collection_ok() {
        let tracker = QuotaTracker::new();
        let tenant_id = "tenant_test";

        let quota = TenantQuota {
            max_collections: 10,
            ..Default::default()
        };
        tracker.set_quota(tenant_id.to_string(), quota);

        // Add 5 collections atomically
        for _ in 0..5 {
            let result = check_and_increment_collection(&tracker, tenant_id).await;
            assert!(result.is_ok());
        }

        // 6th collection should still succeed (5 < 10)
        let result = check_and_increment_collection(&tracker, tenant_id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_check_and_increment_collection_exceeded() {
        let tracker = QuotaTracker::new();
        let tenant_id = "tenant_test";

        let quota = TenantQuota {
            max_collections: 5,
            ..Default::default()
        };
        tracker.set_quota(tenant_id.to_string(), quota);

        // Add 5 collections atomically
        for _ in 0..5 {
            let result = check_and_increment_collection(&tracker, tenant_id).await;
            assert!(result.is_ok());
        }

        // 6th collection should fail (5 == 5, at limit)
        let result = check_and_increment_collection(&tracker, tenant_id).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().0, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_check_and_increment_vectors_ok() {
        let tracker = QuotaTracker::new();
        let tenant_id = "tenant_test";

        let quota = TenantQuota {
            max_vectors_per_collection: 10000,
            ..Default::default()
        };
        tracker.set_quota(tenant_id.to_string(), quota);

        // First increment of 5000 vectors (current=0, adding 5000)
        let result = check_and_increment_vectors(&tracker, tenant_id, 0, 5000).await;
        assert!(result.is_ok());

        // Second increment of 3000 vectors (current=5000, adding 3000, total=8000, under limit)
        let result = check_and_increment_vectors(&tracker, tenant_id, 5000, 3000).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_check_and_increment_vectors_exceeded() {
        let tracker = QuotaTracker::new();
        let tenant_id = "tenant_test";

        let quota = TenantQuota {
            max_vectors_per_collection: 10000,
            ..Default::default()
        };
        tracker.set_quota(tenant_id.to_string(), quota);

        // First increment of 9000 vectors (current=0, adding 9000)
        let result = check_and_increment_vectors(&tracker, tenant_id, 0, 9000).await;
        assert!(result.is_ok());

        // Second increment of 2000 vectors would exceed (current=9000, adding 2000 = 11000 > 10000)
        let result = check_and_increment_vectors(&tracker, tenant_id, 9000, 2000).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().0, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_unlimited_quota() {
        let tracker = QuotaTracker::new();
        let tenant_id = "tenant_test";

        // Set unlimited quota (all zeros)
        tracker.set_quota(tenant_id.to_string(), TenantQuota::unlimited());

        // All atomic operations should pass with unlimited quota
        assert!(check_and_increment_storage(&tracker, tenant_id, u64::MAX / 2)
            .await
            .is_ok());
        assert!(check_and_increment_collection(&tracker, tenant_id)
            .await
            .is_ok());
        assert!(
            check_and_increment_vectors(&tracker, tenant_id, 0, u64::MAX / 4)
                .await
                .is_ok()
        );
    }
}
