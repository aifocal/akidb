pub mod admin;
pub mod collections;
pub mod embedding;
pub mod health; // Kubernetes health and readiness probes
pub mod management;
pub mod tier; // Phase 10 Week 3: Tier control endpoints

pub use admin::{health_check, reset_circuit_breaker, retry_dlq};
pub use collections::{delete_vector, get_vector, insert_vector, query_vectors};
pub use embedding::{embed_handler, AppState as EmbeddingAppState};
pub use health::{health_handler, ready_handler};
pub use management::{
    create_collection, delete_collection, get_collection, list_collections, metrics,
};
pub use tier::{get_collection_tier, get_tier_metrics, update_collection_tier};
