pub mod collection;
pub mod config;
pub mod error;
pub mod manifest;
pub mod segment;

// Phase 3 M4: Production Monitoring
pub mod metrics;

// Phase 7 M1: Multi-Tenancy
pub mod tenant;

pub use collection::{CollectionDescriptor, DistanceMetric, PayloadField, PayloadSchema};
pub use config::{
    AkidbConfig, ApiConfig, CircuitBreakerConfig, HnswIndexConfig, IndexConfig, NativeIndexConfig,
    QueryConfig, RetryConfig, StorageConfig, ValidationConfig,
};
pub use error::{Error, Result};
pub use manifest::{CollectionManifest, ManifestEntry, ManifestSnapshot};
pub use segment::{SegmentDescriptor, SegmentState};
pub use tenant::{
    TenantDescriptor, TenantError, TenantId, TenantMetadata, TenantQuota, TenantStatus,
    TenantUsage,
};
