//! Service layer for AkiDB 2.0.
//! Shared business logic for gRPC and REST APIs.

mod collection_service;
mod config;
mod embedding_manager;
pub mod metrics;

pub use collection_service::{CollectionService, DLQRetryResult, ServiceMetrics};
pub use config::{
    Config, ConfigError, DatabaseConfig, FeaturesConfig, HnswConfig, LoggingConfig, ServerConfig,
};
pub use embedding_manager::EmbeddingManager;

// Re-export ModelInfo from akidb_embedding
pub use akidb_embedding::ModelInfo;

// TODO: Add TenantService, DatabaseService in rc2
