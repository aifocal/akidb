//! SQLite metadata adapters for the AkiDB 2.0 control plane.

mod api_key_repository;
mod audit_repository;
mod collection_repository;
pub mod password;
mod repository;
mod tenant_catalog;
mod tier_state_repository;
mod user_repository;
mod util;
mod vector_persistence;

pub use api_key_repository::SqliteApiKeyRepository;
pub use audit_repository::SqliteAuditLogRepository;
pub use collection_repository::SqliteCollectionRepository;
pub use repository::SqliteDatabaseRepository;
pub use tenant_catalog::SqliteTenantCatalog;
pub use tier_state_repository::{Tier, TierState, TierStateRepository};
pub use user_repository::SqliteUserRepository;
pub use util::{create_sqlite_pool, run_migrations};
pub use vector_persistence::VectorPersistence;

/// Embedded SQL migrations for the metadata database.
pub const MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
