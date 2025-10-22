pub mod bootstrap;
pub mod grpc;
pub mod handlers;
pub mod middleware;
pub mod rest;
pub mod state;
pub mod validation;

pub use grpc::build_grpc_server;
pub use middleware::ApiLayer;
pub use rest::build_router;
pub use state::AppState;

use akidb_core::{Error, Result};
use akidb_index::NativeIndexProvider;
use akidb_query::{BasicQueryPlanner, SimpleExecutionEngine};
use akidb_storage::MemoryStorageBackend;
use std::sync::Arc;
use tracing::info;

/// Boots the AkiDB API stack (REST + gRPC).
pub async fn run_server() -> Result<()> {
    // Create components
    let storage = Arc::new(MemoryStorageBackend::new());
    let index_provider = Arc::new(NativeIndexProvider::new());
    let planner = Arc::new(BasicQueryPlanner::new());
    let engine = Arc::new(SimpleExecutionEngine::new(index_provider.clone()));

    // Create app state
    let state = AppState::new(storage, index_provider, planner, engine);

    // Bootstrap collections from storage (restart recovery)
    bootstrap::bootstrap_collections(&state).await?;

    // Build router
    let router = rest::build_router(state);
    let grpc_builder = grpc::build_grpc_server();
    info!("akidb-api placeholder started");

    // Placeholder waiting for full wiring. For now just await shutdown signal.
    tokio::signal::ctrl_c()
        .await
        .map_err(|err| Error::Internal(format!("failed to wait for shutdown signal: {err}")))?;

    // Drop handles to make lints happy until full impl lands.
    drop((router, grpc_builder));
    info!("akidb-api shutdown complete");
    Ok(())
}
