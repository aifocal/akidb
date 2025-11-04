pub mod bootstrap;
pub mod grpc;
pub mod handlers;
pub mod middleware;
pub mod query_cache;
pub mod rest;
pub mod state;
pub mod telemetry;
pub mod validation;

pub use grpc::build_grpc_server;
pub use middleware::{ApiLayer, AuthConfig};
pub use rest::{build_router, build_router_with_auth};
pub use state::AppState;

use akidb_core::{Error, Result};
use akidb_index::NativeIndexProvider;
use akidb_query::{
    BasicQueryPlanner, BatchExecutionEngine, ExecutionEngine, QueryPlanner, SimpleExecutionEngine,
};
use akidb_storage::{
    MemoryMetadataStore, MemoryStorageBackend, MetadataStore, S3Config, S3StorageBackend,
    S3WalBackend, StorageBackend,
};
use query_cache::QueryCache;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

/// Boots the AkiDB API stack (REST + gRPC).
pub async fn run_server() -> Result<()> {
    // Check if we should use in-memory storage (for tests or development)
    let use_memory = std::env::var("AKIDB_USE_MEMORY_BACKEND")
        .map(|v| v.to_lowercase() == "true" || v == "1")
        .unwrap_or(false);

    let storage: Arc<dyn StorageBackend> = if use_memory {
        info!("Using in-memory storage backend (AKIDB_USE_MEMORY_BACKEND=true)");
        Arc::new(MemoryStorageBackend::new())
    } else {
        // Load S3 configuration from environment variables
        let s3_config = S3Config {
            endpoint: std::env::var("AKIDB_S3_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:9000".to_string()),
            region: std::env::var("AKIDB_S3_REGION").unwrap_or_else(|_| "us-east-1".to_string()),
            access_key: std::env::var("AKIDB_S3_ACCESS_KEY").map_err(|_| {
                Error::Internal("AKIDB_S3_ACCESS_KEY environment variable not set".to_string())
            })?,
            secret_key: std::env::var("AKIDB_S3_SECRET_KEY").map_err(|_| {
                Error::Internal("AKIDB_S3_SECRET_KEY environment variable not set".to_string())
            })?,
            bucket: std::env::var("AKIDB_S3_BUCKET").unwrap_or_else(|_| "akidb".to_string()),
            ..Default::default()
        };

        info!(
            "Initializing S3 storage backend: endpoint={}, bucket={}, region={}",
            s3_config.endpoint, s3_config.bucket, s3_config.region
        );

        // Create S3 storage backend
        Arc::new(
            S3StorageBackend::new(s3_config)
                .map_err(|e| Error::Internal(format!("Failed to initialize S3 backend: {}", e)))?,
        )
    };

    let index_provider = Arc::new(NativeIndexProvider::new());
    let planner: Arc<dyn QueryPlanner> = Arc::new(BasicQueryPlanner::new());
    let engine: Arc<dyn ExecutionEngine> =
        Arc::new(SimpleExecutionEngine::new(index_provider.clone()));
    let metadata_store: Arc<dyn MetadataStore> = Arc::new(MemoryMetadataStore::new());
    let batch_engine = Arc::new(BatchExecutionEngine::new(
        Arc::clone(&engine),
        Arc::clone(&metadata_store),
    ));

    // Create WAL backend with automatic LSN recovery from S3
    let wal = Arc::new(S3WalBackend::builder(storage.clone()).build().await?);

    // Create query cache (10K capacity, 5 min TTL)
    let query_cache = Arc::new(QueryCache::default());

    // Create app state
    let state = AppState::new(
        storage,
        index_provider,
        planner,
        engine,
        batch_engine,
        metadata_store,
        wal,
        query_cache,
    );

    // Bootstrap collections from storage (restart recovery)
    bootstrap::bootstrap_collections(&state).await?;

    // Build router
    let app = rest::build_router(state);

    // Parse bind address (use environment variable or default)
    let bind_address =
        std::env::var("AKIDB_BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let addr: SocketAddr = bind_address.parse().map_err(|e| {
        Error::Validation(format!("Invalid bind address '{}': {}", bind_address, e))
    })?;

    info!("Starting AkiDB API server on {}", addr);

    // Bind TCP listener
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| Error::Internal(format!("Failed to bind to {}: {}", addr, e)))?;

    info!("Server successfully bound to {}", addr);

    // Serve with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| Error::Internal(format!("Server error: {}", e)))?;

    info!("AkiDB API server shutdown complete");
    Ok(())
}

/// Graceful shutdown signal handler
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C signal handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received CTRL+C signal, initiating graceful shutdown");
        }
        _ = terminate => {
            info!("Received SIGTERM signal, initiating graceful shutdown");
        }
    }
}
