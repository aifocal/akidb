use akidb_grpc::{CollectionHandler, CollectionManagementHandler};
use akidb_service::CollectionService;
use akidb_metadata::SqliteCollectionRepository;
use akidb_proto::collection_service_server::CollectionServiceServer;
use akidb_proto::collection_management_service_server::CollectionManagementServiceServer;
use tonic::transport::Server;
use sqlx::SqlitePool;
use std::sync::Arc;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Configuration from environment
    let host = env::var("AKIDB_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = env::var("AKIDB_PORT").unwrap_or_else(|_| "9000".to_string());
    let db_path = env::var("AKIDB_DB_PATH").unwrap_or_else(|_| "sqlite://akidb.db".to_string());

    // Initialize SQLite database
    tracing::info!("📦 Connecting to database: {}", db_path);
    let pool = SqlitePool::connect(&db_path).await?;

    // Run migrations
    tracing::info!("🔄 Running database migrations...");
    sqlx::migrate!("../akidb-metadata/migrations")
        .run(&pool)
        .await?;

    // Create repository and service with persistence
    let repository = Arc::new(SqliteCollectionRepository::new(pool.clone()));
    let service = Arc::new(CollectionService::with_repository(repository));

    // Initialize default database_id for RC1 (single-database mode)
    tracing::info!("🔍 Initializing default tenant and database...");

    // Check if default tenant exists
    let tenant_row: Option<(Vec<u8>,)> = sqlx::query_as("SELECT tenant_id FROM tenants WHERE slug = 'default' LIMIT 1")
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
    let database_row: Option<(Vec<u8>,)> = sqlx::query_as("SELECT database_id FROM databases WHERE name = 'default' LIMIT 1")
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

    // Create gRPC handlers
    let collection_handler = CollectionHandler::new(Arc::clone(&service));
    let management_handler = CollectionManagementHandler::new(Arc::clone(&service));

    // Start gRPC server
    let addr = format!("{}:{}", host, port).parse()?;
    tracing::info!("🚀 gRPC server listening on {}", addr);

    Server::builder()
        .add_service(CollectionServiceServer::new(collection_handler))
        .add_service(CollectionManagementServiceServer::new(management_handler))
        .serve(addr)
        .await?;

    Ok(())
}
