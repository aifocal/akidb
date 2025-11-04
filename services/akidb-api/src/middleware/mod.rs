pub mod auth;
pub mod metrics;
pub mod quota;
pub mod rbac;
pub mod tenant;

pub use auth::{auth_middleware, AuthConfig};
pub use metrics::track_metrics;
pub use quota::{
    check_and_increment_collection, check_and_increment_storage, check_and_increment_vectors,
    inject_quota_state, quota_enforcement_middleware, QuotaEnforcementState,
};
pub use rbac::{
    get_user_context, inject_rbac_state, rbac_middleware, require_all_permissions,
    require_any_permission, require_permission, RbacEnforcementState, UserContext,
};
pub use tenant::{
    get_tenant_context, inject_tenant_enforcement_state, tenant_context_middleware,
    tenant_enforcement_middleware, TenantContext, TenantEnforcementState, TENANT_CONTEXT_KEY,
};

use tower::layer::Layer;

/// Placeholder API layer providing a hook for shared middleware.
#[derive(Debug, Clone, Default)]
pub struct ApiLayer;

impl<S> Layer<S> for ApiLayer {
    type Service = S;

    fn layer(&self, service: S) -> Self::Service {
        service
    }
}
