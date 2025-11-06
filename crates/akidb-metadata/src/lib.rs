//! SQLite metadata adapters for the AkiDB 2.0 control plane.

mod audit_repository;
mod collection_repository;
pub mod password;
mod repository;
mod tenant_catalog;
mod user_repository;
mod util;

pub use audit_repository::SqliteAuditLogRepository;
pub use collection_repository::SqliteCollectionRepository;
pub use repository::SqliteDatabaseRepository;
pub use tenant_catalog::SqliteTenantCatalog;
pub use user_repository::SqliteUserRepository;
pub use util::{create_sqlite_pool, run_migrations};

/// Embedded SQL migrations for the metadata database.
pub const MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
