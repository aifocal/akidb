//! HTTP handlers for REST API endpoints

pub mod collections;
pub mod search;
pub mod vectors;

// Phase 4 M1: Production Monitoring
pub mod health;
pub mod metrics;

// Phase 7 M1: Multi-Tenancy
pub mod tenants;

pub use akidb_query::{BatchQueryRequest, BatchQueryResponse};
pub use collections::{
    create_collection, delete_collection, get_collection, list_collections, ApiError,
    CollectionResponse, CreateCollectionRequest, ErrorResponse,
};
pub use health::{
    detailed_health_handler, liveness_handler, readiness_handler, ComponentHealth,
    ComponentHealthDetails, HealthResponse, HealthStatus,
};
pub use metrics::metrics_handler;
pub use search::{
    batch_search_vectors, search_vectors, SearchRequest, SearchResponse, SearchResult,
};
pub use tenants::{
    create_tenant, delete_tenant, get_tenant, list_tenants, update_tenant, CreateTenantRequest,
    CreateTenantResponse, ListTenantsQuery, ListTenantsResponse, UpdateTenantRequest,
};
pub use vectors::{insert_vectors, InsertVectorsRequest, InsertVectorsResponse, VectorInput};
