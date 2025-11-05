use akidb_core::{TenantDescriptor, TenantQuota};
use akidb_storage::TenantStore;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;

/// Shared tenant store state
pub type TenantStoreState = Arc<dyn TenantStore>;

/// Create tenant request
#[derive(Debug, Deserialize)]
pub struct CreateTenantRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quotas: Option<TenantQuota>,
}

/// Create tenant response
#[derive(Debug, Serialize)]
pub struct CreateTenantResponse {
    pub tenant_id: String,
    pub name: String,
    pub api_key: String,
}

/// Update tenant request
#[derive(Debug, Deserialize)]
pub struct UpdateTenantRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quotas: Option<TenantQuota>,
}

/// List tenants query parameters
#[derive(Debug, Deserialize)]
pub struct ListTenantsQuery {
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    20
}

/// List tenants response
#[derive(Debug, Serialize)]
pub struct ListTenantsResponse {
    pub tenants: Vec<TenantDescriptor>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}

/// Tenant error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}

/// Newtype wrapper to implement IntoResponse for TenantError (orphan rule workaround)
pub struct TenantErrorResponse(pub akidb_core::TenantError);

impl From<akidb_core::TenantError> for TenantErrorResponse {
    fn from(err: akidb_core::TenantError) -> Self {
        TenantErrorResponse(err)
    }
}

impl IntoResponse for TenantErrorResponse {
    fn into_response(self) -> Response {
        let (status, error_type) = match &self.0 {
            akidb_core::TenantError::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            akidb_core::TenantError::AlreadyExists(_) => (StatusCode::CONFLICT, "already_exists"),
            akidb_core::TenantError::QuotaExceeded { .. } => {
                (StatusCode::TOO_MANY_REQUESTS, "quota_exceeded")
            }
            akidb_core::TenantError::InvalidTenantId(_) => {
                (StatusCode::BAD_REQUEST, "invalid_tenant_id")
            }
            akidb_core::TenantError::ValidationFailed(_) => {
                (StatusCode::BAD_REQUEST, "validation_failed")
            }
            akidb_core::TenantError::StorageError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "storage_error")
            }
        };

        let error_response = ErrorResponse {
            error: error_type.to_string(),
            message: self.0.to_string(),
        };

        (status, Json(error_response)).into_response()
    }
}

/// POST /tenants - Create a new tenant
pub async fn create_tenant(
    State(store): State<TenantStoreState>,
    Json(request): Json<CreateTenantRequest>,
) -> Result<(StatusCode, Json<CreateTenantResponse>), akidb_core::TenantError> {
    debug!("Creating tenant: {}", request.name);

    // Create tenant descriptor
    let mut tenant = TenantDescriptor::new(request.name.clone(), request.quotas);

    // Generate API key
    let api_key = akidb_core::tenant::api_key::generate(&tenant.tenant_id);
    tenant.api_key_hash = Some(akidb_core::tenant::api_key::hash(&api_key));

    // Store tenant
    store.create_tenant(&tenant).await?;

    Ok((
        StatusCode::CREATED,
        Json(CreateTenantResponse {
            tenant_id: tenant.tenant_id.clone(),
            name: tenant.name.clone(),
            api_key,
        }),
    ))
}

/// GET /tenants/{id} - Get tenant by ID
pub async fn get_tenant(
    State(store): State<TenantStoreState>,
    Path(tenant_id): Path<String>,
) -> Result<Json<TenantDescriptor>, akidb_core::TenantError> {
    debug!("Getting tenant: {}", tenant_id);

    let tenant = store.get_tenant(&tenant_id).await?;
    Ok(Json(tenant))
}

/// PUT /tenants/{id} - Update tenant
pub async fn update_tenant(
    State(store): State<TenantStoreState>,
    Path(tenant_id): Path<String>,
    Json(request): Json<UpdateTenantRequest>,
) -> Result<Json<TenantDescriptor>, akidb_core::TenantError> {
    debug!("Updating tenant: {}", tenant_id);

    // Get existing tenant
    let mut tenant = store.get_tenant(&tenant_id).await?;

    // Apply updates
    if let Some(name) = request.name {
        tenant.name = name;
    }
    if let Some(quotas) = request.quotas {
        tenant.quotas = quotas;
    }

    tenant.updated_at = chrono::Utc::now();

    // Update storage
    store.update_tenant(&tenant).await?;

    Ok(Json(tenant))
}

/// DELETE /tenants/{id} - Soft delete tenant
pub async fn delete_tenant(
    State(store): State<TenantStoreState>,
    Path(tenant_id): Path<String>,
) -> Result<StatusCode, akidb_core::TenantError> {
    debug!("Deleting tenant: {}", tenant_id);

    store.delete_tenant(&tenant_id).await?;

    Ok(StatusCode::NO_CONTENT)
}

/// GET /tenants - List all tenants with pagination
pub async fn list_tenants(
    State(store): State<TenantStoreState>,
    Query(query): Query<ListTenantsQuery>,
) -> Result<Json<ListTenantsResponse>, akidb_core::TenantError> {
    debug!(
        "Listing tenants: offset={}, limit={}",
        query.offset, query.limit
    );

    // BUGFIX (Bug #23): Validate pagination parameters
    // limit=0 returns empty results but wastes DB query
    // limit > 1000 could cause performance issues or OOM
    if query.limit == 0 {
        return Err(akidb_core::TenantError::Validation(
            "limit must be greater than 0".to_string(),
        ));
    }

    if query.limit > 1000 {
        return Err(akidb_core::TenantError::Validation(format!(
            "limit too large (max 1000, got {})",
            query.limit
        )));
    }

    let tenants = store.list_tenants(query.offset, query.limit).await?;
    let total = tenants.len();

    Ok(Json(ListTenantsResponse {
        tenants,
        total,
        offset: query.offset,
        limit: query.limit,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use akidb_storage::S3TenantStore;
    use object_store::memory::InMemory;

    fn create_test_store() -> Arc<dyn TenantStore> {
        Arc::new(S3TenantStore::new(
            Arc::new(InMemory::new()),
            "test".to_string(),
        ))
    }

    #[tokio::test]
    async fn test_create_tenant_handler() {
        let store = create_test_store();

        let request = CreateTenantRequest {
            name: "Test Corp".to_string(),
            quotas: None,
        };

        let result = create_tenant(State(store.clone()), Json(request)).await;
        assert!(result.is_ok());

        let (status, response) = result.unwrap();
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(response.name, "Test Corp");
        assert!(response.api_key.starts_with("ak_"));
    }

    #[tokio::test]
    async fn test_get_tenant_handler() {
        let store = create_test_store();

        // Create tenant first
        let tenant = TenantDescriptor::new("Test Corp".to_string(), None);
        store.create_tenant(&tenant).await.unwrap();

        // Get tenant
        let result = get_tenant(State(store), Path(tenant.tenant_id.clone())).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.tenant_id, tenant.tenant_id);
    }

    #[tokio::test]
    async fn test_update_tenant_handler() {
        let store = create_test_store();

        // Create tenant first
        let tenant = TenantDescriptor::new("Test Corp".to_string(), None);
        store.create_tenant(&tenant).await.unwrap();

        // Update tenant
        let request = UpdateTenantRequest {
            name: Some("Updated Corp".to_string()),
            quotas: None,
        };

        let result =
            update_tenant(State(store), Path(tenant.tenant_id.clone()), Json(request)).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.name, "Updated Corp");
    }

    #[tokio::test]
    async fn test_delete_tenant_handler() {
        let store = create_test_store();

        // Create tenant first
        let tenant = TenantDescriptor::new("Test Corp".to_string(), None);
        store.create_tenant(&tenant).await.unwrap();

        // Delete tenant
        let result = delete_tenant(State(store.clone()), Path(tenant.tenant_id.clone())).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), StatusCode::NO_CONTENT);

        // Verify tenant is soft deleted
        let deleted_tenant = store.get_tenant(&tenant.tenant_id).await.unwrap();
        assert!(deleted_tenant.is_deleted());
    }

    #[tokio::test]
    async fn test_list_tenants_handler() {
        let store = create_test_store();

        // Create multiple tenants
        for i in 0..3 {
            let tenant = TenantDescriptor::new(format!("Corp {}", i), None);
            store.create_tenant(&tenant).await.unwrap();
        }

        // List tenants
        let query = ListTenantsQuery {
            offset: 0,
            limit: 10,
        };

        let result = list_tenants(State(store), Query(query)).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.tenants.len(), 3);
    }
}
