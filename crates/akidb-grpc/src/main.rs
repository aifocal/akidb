use akidb_grpc::{CollectionHandler, CollectionManagementHandler, EmbeddingHandler};
use akidb_metadata::{SqliteCollectionRepository, VectorPersistence};
use akidb_proto::collection_management_service_server::CollectionManagementServiceServer;
use akidb_proto::collection_service_server::CollectionServiceServer;
use akidb_proto::embedding::embedding_service_server::EmbeddingServiceServer;
use akidb_service::{CollectionService, Config, EmbeddingManager};
use sqlx::SqlitePool;
use std::sync::Arc;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = Config::load().unwrap_or_else(|e| {
        eprintln!("Warning: Failed to load config: {}. Using defaults.", e);
        Config::default()
    });

    // Validate configuration
    config.validate()?;

    // Initialize logging based on config
    let subscriber =
        tracing_subscriber::fmt().with_max_level(match config.logging.level.as_str() {
            "trace" => tracing::Level::TRACE,
            "debug" => tracing::Level::DEBUG,
            "info" => tracing::Level::INFO,
            "warn" => tracing::Level::WARN,
            "error" => tracing::Level::ERROR,
            _ => tracing::Level::INFO,
        });

    if config.logging.format == "json" {
        subscriber.json().init();
    } else {
        subscriber.init();
    }

    // Initialize SQLite database
    tracing::info!("📦 Connecting to database: {}", config.database.path);
    let pool = SqlitePool::connect(&config.database.path).await?;

    // Run migrations
    tracing::info!("🔄 Running database migrations...");
    sqlx::migrate!("../akidb-metadata/migrations")
        .run(&pool)
        .await?;

    // Create repository and service with full persistence (collections + vectors + metrics)
    let repository = Arc::new(SqliteCollectionRepository::new(pool.clone()));
    let vector_persistence = Arc::new(VectorPersistence::new(pool.clone()));
    let service = Arc::new(CollectionService::with_full_persistence(
        repository,
        vector_persistence,
    ));

    // Initialize default database_id for RC1 (single-database mode)
    tracing::info!("🔍 Initializing default tenant and database...");

    // Check if default tenant exists
    let tenant_row: Option<(Vec<u8>,)> =
        sqlx::query_as("SELECT tenant_id FROM tenants WHERE slug = 'default' LIMIT 1")
            .fetch_optional(&pool)
            .await?;

    let tenant_id = if let Some((tenant_id_bytes,)) = tenant_row {
        akidb_core::TenantId::from_bytes(&tenant_id_bytes)
            .map_err(|e| anyhow::anyhow!("Invalid tenant_id: {}", e))?
    } else {
        // Create default tenant
        tracing::info!("📝 Creating default tenant...");
        let tenant_id = akidb_core::TenantId::new();
        let tenant_id_bytes = tenant_id.to_bytes();
        sqlx::query("INSERT INTO tenants (tenant_id, name, slug, status, created_at, updated_at) VALUES (?1, 'default', 'default', 'active', datetime('now'), datetime('now'))")
            .bind(&tenant_id_bytes[..])
            .execute(&pool)
            .await?;
        tracing::info!("✅ Created default tenant: {}", tenant_id);
        tenant_id
    };

    // Check if default database exists
    let database_row: Option<(Vec<u8>,)> =
        sqlx::query_as("SELECT database_id FROM databases WHERE name = 'default' LIMIT 1")
            .fetch_optional(&pool)
            .await?;

    let database_id = if let Some((db_id_bytes,)) = database_row {
        akidb_core::DatabaseId::from_bytes(&db_id_bytes)
            .map_err(|e| anyhow::anyhow!("Invalid database_id: {}", e))?
    } else {
        // Create default database
        tracing::info!("📝 Creating default database...");
        let database_id = akidb_core::DatabaseId::new();
        let database_id_bytes = database_id.to_bytes();
        let tenant_id_bytes = tenant_id.to_bytes();
        sqlx::query("INSERT INTO databases (database_id, tenant_id, name, state, created_at, updated_at) VALUES (?1, ?2, 'default', 'ready', datetime('now'), datetime('now'))")
            .bind(&database_id_bytes[..])
            .bind(&tenant_id_bytes[..])
            .execute(&pool)
            .await?;
        tracing::info!("✅ Created default database: {}", database_id);
        database_id
    };

    service.set_default_database_id(database_id).await;
    tracing::info!("✅ Using default database_id: {}", database_id);

    // Load existing collections from database
    tracing::info!("🔄 Loading collections from database...");
    service.load_all_collections().await?;
    let collection_count = service.list_collections().await?.len();
    tracing::info!("✅ Loaded {} collection(s)", collection_count);

    // Initialize EmbeddingManager for MLX embeddings (optional)
    tracing::info!("🤖 Initializing MLX EmbeddingManager...");
    let embedding_manager = match EmbeddingManager::new("qwen3-0.6b-4bit").await {
        Ok(manager) => {
            tracing::info!(
                "✅ MLX EmbeddingManager initialized (model: qwen3-0.6b-4bit, dimension: {})",
                manager.dimension()
            );
            Some(Arc::new(manager))
        }
        Err(e) => {
            tracing::warn!("⚠️  Failed to initialize EmbeddingManager: {}. Embedding service will not be available.", e);
            None
        }
    };

    // Create gRPC handlers
    let collection_handler = CollectionHandler::new(Arc::clone(&service));
    let management_handler = CollectionManagementHandler::new(Arc::clone(&service));

    // Start gRPC server
    let addr = format!("{}:{}", config.server.host, config.server.grpc_port).parse()?;
    tracing::info!("🚀 gRPC server listening on {}", addr);

    let mut server_builder = Server::builder()
        .add_service(CollectionServiceServer::new(collection_handler))
        .add_service(CollectionManagementServiceServer::new(management_handler));

    // Conditionally add embedding service if manager is available
    if let Some(manager) = embedding_manager {
        tracing::info!("🔌 Adding EmbeddingService to gRPC server");
        let embedding_handler = EmbeddingHandler::new(manager);
        server_builder = server_builder.add_service(EmbeddingServiceServer::new(embedding_handler));
    }

    server_builder
        .serve_with_shutdown(addr, shutdown_signal())
        .await?;

    tracing::info!("✅ Server shutdown complete");

    Ok(())
}

/// Wait for SIGTERM or SIGINT signal for graceful shutdown.
async fn shutdown_signal() {
    use tokio::signal;

    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("🛑 Received SIGINT (Ctrl+C), initiating graceful shutdown...");
        },
        _ = terminate => {
            tracing::info!("🛑 Received SIGTERM, initiating graceful shutdown...");
        },
    }
}
