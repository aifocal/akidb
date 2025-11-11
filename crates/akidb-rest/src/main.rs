use akidb_metadata::{SqliteCollectionRepository, VectorPersistence};
use akidb_rest::handlers;
use akidb_service::{CollectionService, Config, EmbeddingManager};
use axum::{
    routing::{delete, get, post},
    Router,
};
use sqlx::SqlitePool;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = Config::load().unwrap_or_else(|e| {
        eprintln!("Warning: Failed to load config: {}. Using defaults.", e);
        Config::default()
    });

    // Validate configuration
    config.validate()?;

    // Initialize distributed tracing with OpenTelemetry (optional)
    // Set ENABLE_TRACING=true to enable Jaeger tracing
    if std::env::var("ENABLE_TRACING").unwrap_or_else(|_| "false".to_string()) == "true" {
        tracing::info!("🔍 Initializing distributed tracing with Jaeger...");
        if let Err(e) = akidb_rest::tracing_init::init_from_env() {
            tracing::warn!(
                "⚠️  Failed to initialize tracing: {}. Falling back to basic logging.",
                e
            );
            // Fall back to basic logging
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
        } else {
            tracing::info!("✅ Distributed tracing initialized");
        }
    } else {
        // Use basic logging (no distributed tracing)
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

    // Initialize EmbeddingManager for MLX embeddings
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
            tracing::warn!("⚠️  Failed to initialize EmbeddingManager: {}. /embed endpoint will not be available.", e);
            None
        }
    };

    // Build embedding state if manager is available
    let embedding_state = embedding_manager.map(|manager| {
        Arc::new(handlers::EmbeddingAppState {
            embedding_manager: manager,
        })
    });

    // Build main router with collection endpoints
    let app = Router::new()
        // Kubernetes health and readiness probes
        .route("/health", get(handlers::health_handler))
        .route("/ready", get(handlers::ready_handler))
        .route("/metrics", get(handlers::metrics))
        // Collection management endpoints
        .route("/api/v1/collections", post(handlers::create_collection))
        .route("/api/v1/collections", get(handlers::list_collections))
        .route("/api/v1/collections/:id", get(handlers::get_collection))
        .route(
            "/api/v1/collections/:id",
            delete(handlers::delete_collection),
        )
        // Vector operation endpoints
        .route(
            "/api/v1/collections/:id/query",
            post(handlers::query_vectors),
        )
        .route(
            "/api/v1/collections/:id/insert",
            post(handlers::insert_vector),
        )
        .route(
            "/api/v1/collections/:id/docs/:doc_id",
            get(handlers::get_vector),
        )
        .route(
            "/api/v1/collections/:id/docs/:doc_id",
            delete(handlers::delete_vector),
        )
        // Admin/Operations endpoints (Phase 7 Week 4)
        .route("/admin/health", get(handlers::health_check))
        .route(
            "/admin/collections/:id/dlq/retry",
            post(handlers::retry_dlq),
        )
        .route(
            "/admin/circuit-breaker/reset",
            post(handlers::reset_circuit_breaker),
        )
        // Tier management endpoints (Phase 10 Week 3)
        .route(
            "/api/v1/collections/:id/tier",
            get(handlers::get_collection_tier),
        )
        .route(
            "/api/v1/collections/:id/tier",
            post(handlers::update_collection_tier),
        )
        .route("/api/v1/metrics/tiers", get(handlers::get_tier_metrics))
        .with_state(Arc::clone(&service));

    // Clone service for shutdown handler before moving it into router state
    let service_for_shutdown = Arc::clone(&service);

    // Add embedding endpoint if manager is available (nested router)
    let app = if let Some(state) = embedding_state {
        tracing::info!("🔌 Adding /api/v1/embed endpoint");
        let embedding_router = Router::new()
            .route("/api/v1/embed", post(handlers::embed_handler))
            .with_state(state);

        app.merge(embedding_router)
    } else {
        app
    };

    let addr = format!("{}:{}", config.server.host, config.server.rest_port).parse()?;

    tracing::info!("🌐 REST server listening on {}", addr);

    // Setup graceful shutdown
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal(service_for_shutdown))
        .await?;

    tracing::info!("✅ Server shutdown complete");

    // Shutdown tracing provider (flushes pending spans)
    if std::env::var("ENABLE_TRACING").unwrap_or_else(|_| "false".to_string()) == "true" {
        tracing::info!("🔍 Shutting down tracing...");
        akidb_rest::tracing_init::shutdown();
    }

    Ok(())
}

/// Wait for SIGTERM or SIGINT signal for graceful shutdown.
///
/// When a signal is received, this function:
/// 1. Logs the signal type
/// 2. Calls CollectionService::shutdown() to flush WAL, stop background tasks, etc.
/// 3. Returns to allow Axum to complete in-flight requests
async fn shutdown_signal(service: Arc<CollectionService>) {
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

    // Shutdown collection service (flush WAL, stop background tasks)
    if let Err(e) = service.shutdown().await {
        tracing::error!("❌ Error during collection service shutdown: {}", e);
    } else {
        tracing::info!("✅ Collection service shutdown complete");
    }
}
