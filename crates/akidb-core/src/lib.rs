//! Core domain types and traits for AkiDB 2.0 metadata services.

pub mod audit;
pub mod collection;
pub mod database;
pub mod error;
pub mod ids;
pub mod tenant;
pub mod traits;
pub mod user;
pub mod vector;

pub use audit::{AuditLogEntry, AuditResult};
pub use collection::{CollectionDescriptor, DistanceMetric};
pub use database::{DatabaseDescriptor, DatabaseState};
pub use error::{CoreError, CoreResult};
pub use ids::{AuditLogId, CollectionId, DatabaseId, DocumentId, TenantId, UserId};
pub use tenant::{TenantDescriptor, TenantQuota, TenantStatus};
pub use traits::{AuditLogRepository, CollectionRepository, DatabaseRepository, TenantCatalog, UserRepository, VectorIndex};
pub use user::{Action, Role, UserDescriptor, UserStatus};
pub use vector::{SearchResult, VectorDocument};
