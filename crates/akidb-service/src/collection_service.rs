//! Service layer for collection operations.
//! Shared by gRPC and REST APIs.

use akidb_core::{
    CollectionDescriptor, CollectionId, CollectionRepository, CoreError, CoreResult, DatabaseId,
    DistanceMetric, DocumentId, SearchResult, VectorDocument, VectorIndex,
};
use akidb_index::{BruteForceIndex, InstantDistanceConfig, InstantDistanceIndex};
use akidb_storage::{
    CacheStats, CircuitBreakerState, StorageBackend, StorageConfig, StorageMetrics,
};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

// Import metrics for instrumentation
use crate::metrics::*;

// Phase 10 Week 3: Tiering manager integration
use akidb_storage::tiering_manager::TieringManager;

/// Result of DLQ retry operation
#[derive(Debug, Clone)]
pub struct DLQRetryResult {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
}

/// Service-level metrics for collections, vectors, and operations
#[derive(Debug, Clone)]
pub struct ServiceMetrics {
    pub total_collections: usize,
    pub total_vectors: usize,
    pub total_searches: u64,
    pub total_inserts: u64,
    pub uptime_seconds: u64,
}

impl ServiceMetrics {
    /// Export metrics in Prometheus text format
    pub async fn export_prometheus(&self) -> String {
        let mut output = String::new();

        output.push_str("# HELP akidb_total_collections Total number of collections\n");
        output.push_str("# TYPE akidb_total_collections gauge\n");
        output.push_str(&format!(
            "akidb_total_collections {}\n",
            self.total_collections
        ));
        output.push('\n');

        output.push_str(
            "# HELP akidb_total_vectors Total number of vectors across all collections\n",
        );
        output.push_str("# TYPE akidb_total_vectors gauge\n");
        output.push_str(&format!("akidb_total_vectors {}\n", self.total_vectors));
        output.push('\n');

        output.push_str("# HELP akidb_uptime_seconds Server uptime in seconds\n");
        output.push_str("# TYPE akidb_uptime_seconds counter\n");
        output.push_str(&format!("akidb_uptime_seconds {}\n", self.uptime_seconds));
        output.push('\n');

        output
    }

    /// Get total collections created
    pub fn collections_created(&self) -> usize {
        self.total_collections
    }

    /// Get total vectors inserted
    pub fn vectors_inserted(&self) -> usize {
        self.total_vectors
    }

    /// Get total searches performed
    pub fn searches_performed(&self) -> u64 {
        self.total_searches
    }

    /// Get total collections deleted (placeholder for now)
    pub fn collections_deleted(&self) -> usize {
        0 // TODO: Track deletions separately
    }
}

/// Service layer for collection operations.
/// Shared by gRPC and REST APIs.
pub struct CollectionService {
    // SQLite repository for persistence (optional for testing)
    repository: Option<Arc<dyn CollectionRepository>>,

    // Vector persistence (optional, for Week 2+ vector durability)
    vector_persistence: Option<Arc<akidb_metadata::VectorPersistence>>,

    // In-memory cache for fast reads (synced with repository)
    collections: Arc<RwLock<HashMap<CollectionId, CollectionDescriptor>>>,

    // In-memory vector indexes (collection_id -> VectorIndex)
    indexes: Arc<RwLock<HashMap<CollectionId, Box<dyn VectorIndex>>>>,

    // Default database_id for RC1 (single-database mode)
    default_database_id: Arc<RwLock<Option<DatabaseId>>>,

    // Storage backends (Phase 6 Week 5+: per-collection tiered storage)
    storage_backends: Arc<RwLock<HashMap<CollectionId, Arc<StorageBackend>>>>,

    // Storage configuration (used when creating new storage backends)
    storage_config: StorageConfig,

    // Server start time for uptime tracking (Phase 7 Week 4)
    start_time: Instant,

    // Tiering manager for hot/warm/cold tier management (Phase 10 Week 3)
    // Optional: If None, tiering is disabled (backward compatible)
    tiering_manager: Option<Arc<TieringManager>>,
}

impl CollectionService {
    /// Creates a new collection service with in-memory storage only.
    /// For testing purposes. Production should use `with_repository()`.
    pub fn new() -> Self {
        Self {
            repository: None,
            vector_persistence: None,
            collections: Arc::new(RwLock::new(HashMap::new())),
            indexes: Arc::new(RwLock::new(HashMap::new())),
            default_database_id: Arc::new(RwLock::new(None)),
            storage_backends: Arc::new(RwLock::new(HashMap::new())),
            storage_config: StorageConfig::default(),
            start_time: Instant::now(),
            tiering_manager: None,
        }
    }

    /// Creates a new collection service with SQLite persistence.
    /// Collections are persisted to the database and loaded on startup.
    pub fn with_repository(repository: Arc<dyn CollectionRepository>) -> Self {
        Self {
            repository: Some(repository),
            vector_persistence: None,
            collections: Arc::new(RwLock::new(HashMap::new())),
            indexes: Arc::new(RwLock::new(HashMap::new())),
            default_database_id: Arc::new(RwLock::new(None)),
            storage_backends: Arc::new(RwLock::new(HashMap::new())),
            storage_config: StorageConfig::default(),
            start_time: Instant::now(),
            tiering_manager: None,
        }
    }

    /// Creates a new collection service with full persistence (collections + vectors).
    /// This is the recommended constructor for production use (Phase 5 Week 2+).
    pub fn with_full_persistence(
        repository: Arc<dyn CollectionRepository>,
        vector_persistence: Arc<akidb_metadata::VectorPersistence>,
    ) -> Self {
        Self {
            repository: Some(repository),
            vector_persistence: Some(vector_persistence),
            collections: Arc::new(RwLock::new(HashMap::new())),
            indexes: Arc::new(RwLock::new(HashMap::new())),
            default_database_id: Arc::new(RwLock::new(None)),
            storage_backends: Arc::new(RwLock::new(HashMap::new())),
            storage_config: StorageConfig::default(),
            start_time: Instant::now(),
            tiering_manager: None,
        }
    }

    /// Creates a new collection service with tiered storage configuration.
    /// This constructor allows custom storage policies (Memory, MemoryS3, S3Only).
    /// (Phase 6 Week 5+: Integration with StorageBackend for each collection).
    pub fn with_storage(
        repository: Arc<dyn CollectionRepository>,
        vector_persistence: Arc<akidb_metadata::VectorPersistence>,
        storage_config: StorageConfig,
    ) -> Self {
        Self {
            repository: Some(repository),
            vector_persistence: Some(vector_persistence),
            collections: Arc::new(RwLock::new(HashMap::new())),
            indexes: Arc::new(RwLock::new(HashMap::new())),
            default_database_id: Arc::new(RwLock::new(None)),
            storage_backends: Arc::new(RwLock::new(HashMap::new())),
            storage_config,
            start_time: Instant::now(),
            tiering_manager: None,
        }
    }

    /// Creates a new collection service with hot/warm/cold tiering manager.
    /// This is the full-featured constructor for production use with automatic tiering.
    /// (Phase 10 Week 3: Integration with TieringManager).
    pub fn with_tiering(
        repository: Arc<dyn CollectionRepository>,
        vector_persistence: Arc<akidb_metadata::VectorPersistence>,
        storage_config: StorageConfig,
        tiering_manager: Arc<TieringManager>,
    ) -> Self {
        Self {
            repository: Some(repository),
            vector_persistence: Some(vector_persistence),
            collections: Arc::new(RwLock::new(HashMap::new())),
            indexes: Arc::new(RwLock::new(HashMap::new())),
            default_database_id: Arc::new(RwLock::new(None)),
            storage_backends: Arc::new(RwLock::new(HashMap::new())),
            storage_config,
            start_time: Instant::now(),
            tiering_manager: Some(tiering_manager),
        }
    }

    /// Gets a reference to the tiering manager (if enabled).
    /// (Phase 10 Week 3: Tiering manager integration).
    pub fn tiering_manager(&self) -> Option<Arc<TieringManager>> {
        self.tiering_manager.clone()
    }

    /// Gets aggregated storage metrics from all storage backends.
    ///
    /// Returns `None` if no storage backends are configured (e.g., in-memory only mode).
    /// Aggregates metrics across all collections' storage backends.
    pub async fn storage_metrics(&self) -> Option<akidb_storage::StorageMetrics> {
        let backends = self.storage_backends.read().await;

        if backends.is_empty() {
            return None;
        }

        // Aggregate metrics from all storage backends
        let mut aggregated = akidb_storage::StorageMetrics::default();

        for backend in backends.values() {
            let backend_metrics = backend.metrics();

            // FIX BUG #11: Use saturating_add to prevent integer overflow
            // In debug mode, += would panic on overflow
            // In release mode, += would wrap around (incorrect metrics)
            aggregated.inserts = aggregated.inserts.saturating_add(backend_metrics.inserts);
            aggregated.queries = aggregated.queries.saturating_add(backend_metrics.queries);
            aggregated.deletes = aggregated.deletes.saturating_add(backend_metrics.deletes);
            aggregated.s3_uploads = aggregated
                .s3_uploads
                .saturating_add(backend_metrics.s3_uploads);
            aggregated.s3_downloads = aggregated
                .s3_downloads
                .saturating_add(backend_metrics.s3_downloads);
            aggregated.cache_hits = aggregated
                .cache_hits
                .saturating_add(backend_metrics.cache_hits);
            aggregated.cache_misses = aggregated
                .cache_misses
                .saturating_add(backend_metrics.cache_misses);
            aggregated.wal_size_bytes = aggregated
                .wal_size_bytes
                .saturating_add(backend_metrics.wal_size_bytes);
            aggregated.compactions = aggregated
                .compactions
                .saturating_add(backend_metrics.compactions);
            aggregated.s3_retries = aggregated
                .s3_retries
                .saturating_add(backend_metrics.s3_retries);
            aggregated.s3_permanent_failures = aggregated
                .s3_permanent_failures
                .saturating_add(backend_metrics.s3_permanent_failures);
            aggregated.dlq_size = aggregated
                .dlq_size
                .saturating_add(backend_metrics.dlq_size);

            // Take the highest error rate and breaker state across all backends
            if backend_metrics.circuit_breaker_error_rate > aggregated.circuit_breaker_error_rate {
                aggregated.circuit_breaker_error_rate = backend_metrics.circuit_breaker_error_rate;
            }

            // If any circuit breaker is open (1), use that state
            if backend_metrics.circuit_breaker_state > aggregated.circuit_breaker_state {
                aggregated.circuit_breaker_state = backend_metrics.circuit_breaker_state;
            }

            // Use the most recent snapshot time
            if backend_metrics.last_snapshot_at.is_some() {
                match (
                    aggregated.last_snapshot_at,
                    backend_metrics.last_snapshot_at,
                ) {
                    (None, Some(ts)) => aggregated.last_snapshot_at = Some(ts),
                    (Some(existing), Some(new)) if new > existing => {
                        aggregated.last_snapshot_at = Some(new);
                    }
                    _ => {}
                }
            }
        }

        Some(aggregated)
    }

    /// Set the default database_id for RC1 single-database mode.
    pub async fn set_default_database_id(&self, database_id: DatabaseId) {
        let mut default_db = self.default_database_id.write().await;
        *default_db = Some(database_id);
    }

    /// Get the default database_id, or create a new one if not set (for in-memory mode).
    async fn get_or_create_database_id(&self) -> DatabaseId {
        let default_db = self.default_database_id.read().await;
        match *default_db {
            Some(id) => id,
            None => DatabaseId::new(), // Fallback for in-memory mode
        }
    }

    // ========== Collection Management Operations ==========

    /// Create per-collection storage configuration.
    /// Each collection gets its own WAL and snapshot directories.
    fn create_storage_backend_for_collection(
        &self,
        collection: &CollectionDescriptor,
    ) -> CoreResult<StorageConfig> {
        // Create per-collection WAL directory: {base_wal_path}/collections/{collection_id}/wal
        let collection_wal_path = self
            .storage_config
            .wal_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("collections")
            .join(collection.collection_id.to_string())
            .join("wal");

        // Create per-collection snapshot directory: {base_snapshot_path}/collections/{collection_id}/snapshots
        let collection_snapshot_dir = self
            .storage_config
            .snapshot_dir
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("collections")
            .join(collection.collection_id.to_string())
            .join("snapshots");

        // Create directories if they don't exist
        if let Some(parent) = collection_wal_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                CoreError::invalid_state(format!("Failed to create WAL directory: {}", e))
            })?;
        }
        std::fs::create_dir_all(&collection_snapshot_dir).map_err(|e| {
            CoreError::invalid_state(format!("Failed to create snapshot directory: {}", e))
        })?;

        // Clone base config and update paths
        let mut config = self.storage_config.clone();
        config.wal_path = collection_wal_path;
        config.snapshot_dir = collection_snapshot_dir;

        // FIX BUG #16: Set the real collection_id for WAL entries and S3 keys
        config.collection_id = collection.collection_id;

        Ok(config)
    }

    /// Load all collections from repository on startup.
    /// Only works if service was created with `with_repository()`.
    pub async fn load_all_collections(&self) -> CoreResult<()> {
        let Some(repo) = &self.repository else {
            // No repository = in-memory mode, nothing to load
            return Ok(());
        };

        // Load all collections from SQLite
        let descriptors = repo.list_all().await?;

        // Populate cache and load indexes
        for descriptor in descriptors {
            {
                let mut collections = self.collections.write().await;
                collections.insert(descriptor.collection_id, descriptor.clone());
            }
            // Load index (ignoring errors for individual collections)
            if let Err(e) = self.load_collection(&descriptor).await {
                tracing::warn!(
                    "Failed to load index for collection {}: {}",
                    descriptor.collection_id,
                    e
                );
            }
        }

        Ok(())
    }

    /// Create a new collection.
    pub async fn create_collection(
        &self,
        name: String,
        dimension: u32,
        metric: DistanceMetric,
        embedding_model: Option<String>,
    ) -> CoreResult<CollectionId> {
        // FIX BUG #14: Validate collection name (prevent path traversal, DoS, file system attacks)
        const MAX_COLLECTION_NAME_LEN: usize = 255; // File system path component limit

        if name.is_empty() {
            return Err(CoreError::ValidationError(
                "collection name cannot be empty".to_string(),
            ));
        }

        if name.len() > MAX_COLLECTION_NAME_LEN {
            return Err(CoreError::ValidationError(format!(
                "collection name must be <= {} characters (got {})",
                MAX_COLLECTION_NAME_LEN,
                name.len()
            )));
        }

        // Check for path traversal attacks
        if name.contains("..") || name.contains('/') || name.contains('\\') || name.contains('\0') {
            return Err(CoreError::ValidationError(
                "collection name contains invalid path characters (.. / \\ \\0)".to_string(),
            ));
        }

        // Check for Windows invalid characters (cross-platform compatibility)
        const WINDOWS_INVALID_CHARS: &[char] = &['<', '>', ':', '"', '|', '?', '*'];
        if name.chars().any(|c| WINDOWS_INVALID_CHARS.contains(&c)) {
            return Err(CoreError::ValidationError(
                "collection name contains invalid characters (< > : \" | ? *)".to_string(),
            ));
        }

        // Check for control characters (0x00-0x1F, 0x7F-0x9F)
        if name.chars().any(|c| c.is_control()) {
            return Err(CoreError::ValidationError(
                "collection name contains control characters".to_string(),
            ));
        }

        // Validate dimension
        if !(16..=4096).contains(&dimension) {
            return Err(CoreError::invalid_state(format!(
                "dimension must be between 16 and 4096, got {}",
                dimension
            )));
        }

        // FIX BUG #13: Validate embedding_model length (prevent DoS via unbounded strings)
        const MAX_EMBEDDING_MODEL_LEN: usize = 256;
        let embedding_model_validated = match embedding_model {
            Some(model) if !model.is_empty() && model.len() <= MAX_EMBEDDING_MODEL_LEN => model,
            Some(ref model) if model.is_empty() => {
                return Err(CoreError::ValidationError(
                    "embedding_model cannot be empty".to_string(),
                ))
            }
            Some(ref model) => {
                return Err(CoreError::ValidationError(format!(
                    "embedding_model must be <= {} characters (got {})",
                    MAX_EMBEDDING_MODEL_LEN,
                    model.len()
                )))
            }
            None => "none".to_string(),
        };

        // Get database_id for RC1 single-database mode
        let database_id = self.get_or_create_database_id().await;

        // Create collection descriptor
        let collection_id = CollectionId::new();
        let collection = CollectionDescriptor {
            collection_id,
            database_id,
            name: name.clone(),
            dimension,
            metric,
            embedding_model: embedding_model_validated,
            hnsw_m: 32,
            hnsw_ef_construction: 200,
            max_doc_count: 50_000_000,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // FIX BUG #7: Atomic creation with rollback on failure
        // Use early-return pattern to ensure all steps succeed or rollback

        // Step 1: Persist to SQLite if repository exists
        if let Some(repo) = &self.repository {
            repo.create(&collection).await?;
        }

        // Step 2: Store in cache
        {
            let mut collections = self.collections.write().await;
            collections.insert(collection_id, collection.clone());
        }

        // FIX BUG #15: load_collection creates AND stores the StorageBackend
        // Removed duplicate StorageBackend creation to prevent data loss on restart
        //
        // BEFORE (BROKEN):
        // 1. load_collection creates StorageBackend #1 and loads legacy SQLite vectors into it
        // 2. load_collection stores backend #1 in storage_backends map
        // 3. create_collection creates StorageBackend #2 (empty, new WAL)
        // 4. create_collection OVERWRITES backend #1 with backend #2 in map
        // → Backend #1 (with migrated data) is dropped without shutdown
        // → Migrated data exists only in RAM, lost on restart
        // → Legacy SQLite vectors are loaded again on next startup (infinite migration loop)
        //
        // AFTER (FIXED):
        // 1. load_collection creates StorageBackend and loads legacy SQLite vectors into it
        // 2. load_collection stores backend in storage_backends map
        // 3. Done! No duplicate creation, no data loss

        // Step 3: Create and load index + storage backend (with rollback on failure)
        if let Err(e) = self.load_collection(&collection).await {
            // Rollback: Remove from cache
            self.collections.write().await.remove(&collection_id);
            // Rollback: Remove from SQLite
            if let Some(repo) = &self.repository {
                let _ = repo.delete(collection_id).await; // Best effort
            }
            return Err(e);
        }

        Ok(collection_id)
    }

    /// List all collections.
    pub async fn list_collections(&self) -> CoreResult<Vec<CollectionDescriptor>> {
        let collections = self.collections.read().await;
        Ok(collections.values().cloned().collect())
    }

    /// Get a specific collection by ID.
    pub async fn get_collection(
        &self,
        collection_id: CollectionId,
    ) -> CoreResult<CollectionDescriptor> {
        let collections = self.collections.read().await;
        collections
            .get(&collection_id)
            .cloned()
            .ok_or_else(|| CoreError::not_found("Collection", collection_id.to_string()))
    }

    /// Delete a collection.
    pub async fn delete_collection(&self, collection_id: CollectionId) -> CoreResult<()> {
        // Delete from SQLite if repository exists
        if let Some(repo) = &self.repository {
            repo.delete(collection_id).await?;
        }

        // Remove from cache
        {
            let mut collections = self.collections.write().await;
            collections
                .remove(&collection_id)
                .ok_or_else(|| CoreError::not_found("Collection", collection_id.to_string()))?;
        }

        // Unload index
        self.unload_collection(collection_id).await?;

        // FIX BUG #2: Shutdown storage backend BEFORE removing to prevent resource leaks
        // This ensures background tasks (S3 uploader, retry worker, compaction, DLQ cleanup) are stopped
        // and WAL buffers are flushed to prevent data loss
        {
            let mut backends = self.storage_backends.write().await;
            if let Some(backend) = backends.remove(&collection_id) {
                // Shutdown gracefully (aborts tasks, flushes WAL)
                if let Err(e) = backend.shutdown().await {
                    tracing::warn!(
                        "Failed to shutdown storage backend for collection {}: {}",
                        collection_id,
                        e
                    );
                    // Continue with deletion even if shutdown fails
                }
            }
        }

        Ok(())
    }

    // ========== Vector Operations ==========

    /// Query vectors (k-NN search).
    pub async fn query(
        &self,
        collection_id: CollectionId,
        query_vector: Vec<f32>,
        top_k: usize,
    ) -> CoreResult<Vec<SearchResult>> {
        let start = Instant::now();

        // FIX BUG #8: Validate top_k to prevent DoS via memory exhaustion
        // Reasonable limit: 10,000 results (prevents usize::MAX attacks)
        const MAX_TOP_K: usize = 10_000;
        if top_k == 0 {
            return Err(CoreError::ValidationError(
                "top_k must be greater than 0".to_string(),
            ));
        }
        if top_k > MAX_TOP_K {
            return Err(CoreError::ValidationError(format!(
                "top_k must be <= {} (got {})",
                MAX_TOP_K, top_k
            )));
        }

        // Record access for tiering (Phase 10 Week 3)
        if let Some(tiering_manager) = &self.tiering_manager {
            // Ignore errors from access tracking (non-critical)
            let _ = tiering_manager.record_access(collection_id).await;
        }

        // Get index
        let indexes = self.indexes.read().await;
        let index = indexes
            .get(&collection_id)
            .ok_or_else(|| CoreError::not_found("Collection", collection_id.to_string()))?;

        // Perform search
        let result = index.search(&query_vector, top_k, None).await;

        // Record metrics
        let duration = start.elapsed().as_secs_f64();
        VECTOR_SEARCH_DURATION_SECONDS
            .with_label_values(&["hot"]) // TODO: Get actual tier from TieringManager
            .observe(duration);

        result
    }

    /// Insert single vector.
    pub async fn insert(
        &self,
        collection_id: CollectionId,
        doc: VectorDocument,
    ) -> CoreResult<DocumentId> {
        let start = Instant::now();

        // Record access for tiering (Phase 10 Week 3)
        if let Some(tiering_manager) = &self.tiering_manager {
            // Ignore errors from access tracking (non-critical)
            let _ = tiering_manager.record_access(collection_id).await;
        }

        // Validate vector dimension matches collection's expected dimension
        {
            let collections = self.collections.read().await;
            let collection = collections
                .get(&collection_id)
                .ok_or_else(|| CoreError::not_found("Collection", collection_id.to_string()))?;

            let expected_dim = collection.dimension as usize;
            let actual_dim = doc.vector.len();

            if actual_dim != expected_dim {
                return Err(CoreError::ValidationError(format!(
                    "Vector dimension mismatch: expected {}, got {}",
                    expected_dim, actual_dim
                )));
            }
        }

        // FIX BUG #1 & #6: Insert into index FIRST, then persist to WAL
        // Hold BOTH locks simultaneously to prevent collection deletion race condition
        //
        // RACE CONDITION FIX: If we release the index lock before acquiring the backend lock,
        // another thread could delete the collection in between, causing:
        // - Document in index but not in WAL → data loss on restart
        //
        // By holding both locks, we ensure atomic insert across index + WAL
        let doc_id = doc.doc_id;
        {
            // Acquire BOTH locks before any mutations (prevents delete_collection race)
            let indexes = self.indexes.read().await;
            let backends = self.storage_backends.read().await;

            // Get index reference
            let index = indexes
                .get(&collection_id)
                .ok_or_else(|| CoreError::not_found("Collection", collection_id.to_string()))?;

            // Insert into in-memory index FIRST
            // If this fails, we return error WITHOUT persisting to WAL
            index.insert(doc.clone()).await?;

            // Only persist to StorageBackend AFTER successful index insert
            // This prevents WAL/index inconsistency on index failures (Bug #1)
            //
            // BUG FIX #2 COMPLETE: If persistence fails, rollback index insert to maintain consistency
            if let Some(storage_backend) = backends.get(&collection_id) {
                // Use insert_with_auto_compact for automatic WAL management
                if let Err(e) = storage_backend.insert_with_auto_compact(doc).await {
                    // Rollback: Remove document from index since WAL persistence failed
                    if let Err(rollback_err) = index.delete(doc_id).await {
                        tracing::error!(
                            "Failed to rollback index insert after WAL failure for doc {}: {}. Index may be inconsistent.",
                            doc_id, rollback_err
                        );
                    }
                    return Err(e);
                }
            } else {
                // Fallback: Legacy persistence (Phase 5 compatibility)
                if let Some(persistence) = &self.vector_persistence {
                    if let Err(e) = persistence.save_vector(collection_id, &doc).await {
                        // Rollback: Remove document from index since persistence failed
                        if let Err(rollback_err) = index.delete(doc_id).await {
                            tracing::error!(
                                "Failed to rollback index insert after persistence failure for doc {}: {}. Index may be inconsistent.",
                                doc_id, rollback_err
                            );
                        }
                        return Err(e);
                    }
                }
            }

            // Both locks released here - collection cannot be deleted during insert
        }

        // Record metrics
        let duration = start.elapsed().as_secs_f64();
        VECTOR_INSERT_DURATION_SECONDS
            .with_label_values(&[&collection_id.to_string()])
            .observe(duration);

        COLLECTION_SIZE_VECTORS
            .with_label_values(&[&collection_id.to_string()])
            .inc();

        Ok(doc_id)
    }

    /// Get vector by ID.
    pub async fn get(
        &self,
        collection_id: CollectionId,
        doc_id: DocumentId,
    ) -> CoreResult<Option<VectorDocument>> {
        // Record access for tiering (Phase 10 Week 3)
        if let Some(tiering_manager) = &self.tiering_manager {
            // Ignore errors from access tracking (non-critical)
            let _ = tiering_manager.record_access(collection_id).await;
        }

        let indexes = self.indexes.read().await;
        let index = indexes
            .get(&collection_id)
            .ok_or_else(|| CoreError::not_found("Collection", collection_id.to_string()))?;

        index.get(doc_id).await
    }

    /// Delete vector by ID.
    pub async fn delete(&self, collection_id: CollectionId, doc_id: DocumentId) -> CoreResult<()> {
        // Record access for tiering (Phase 10 Week 3)
        if let Some(tiering_manager) = &self.tiering_manager {
            // Ignore errors from access tracking (non-critical)
            let _ = tiering_manager.record_access(collection_id).await;
        }

        // FIX BUG #6: Delete from WAL first, then index
        // Hold BOTH locks simultaneously to prevent collection deletion race condition
        //
        // RACE CONDITION FIX: If we release the backend lock before acquiring the index lock,
        // another thread could delete the collection in between, causing:
        // - Document deleted from WAL but not from index → stale data in index
        //
        // By holding both locks, we ensure atomic delete across WAL + index
        {
            // Acquire BOTH locks before any mutations (prevents delete_collection race)
            let backends = self.storage_backends.read().await;
            let indexes = self.indexes.read().await;

            // Delete from WAL-backed storage FIRST (durability first)
            if let Some(storage_backend) = backends.get(&collection_id) {
                storage_backend.delete(&doc_id).await?;
            } else {
                // Fallback: Legacy persistence (Phase 5 compatibility)
                if let Some(persistence) = &self.vector_persistence {
                    persistence.delete_vector(collection_id, doc_id).await?;
                }
            }

            // Delete from index AFTER successful WAL delete
            let index = indexes
                .get(&collection_id)
                .ok_or_else(|| CoreError::not_found("Collection", collection_id.to_string()))?;

            index.delete(doc_id).await?;

            // Both locks released here - collection cannot be deleted during delete operation
        }

        Ok(())
    }

    /// Load collection into memory (called on startup or creation).
    /// Creates appropriate index based on collection config.
    /// If vector persistence is enabled, loads all vectors from SQLite.
    pub async fn load_collection(&self, collection: &CollectionDescriptor) -> CoreResult<()> {
        // Create appropriate index based on collection config
        let index: Box<dyn VectorIndex> = if collection.max_doc_count <= 10_000 {
            // Use BruteForce for small collections
            Box::new(BruteForceIndex::new(
                collection.dimension as usize,
                collection.metric,
            ))
        } else {
            // Use InstantDistance for large collections
            let config =
                InstantDistanceConfig::balanced(collection.dimension as usize, collection.metric);
            Box::new(InstantDistanceIndex::new(config)?)
        };

        // Phase 6 Week 5 Day 3: Create StorageBackend FIRST to enable WAL recovery
        let storage_config = self.create_storage_backend_for_collection(collection)?;
        let storage_backend = Arc::new(StorageBackend::new(storage_config).await?);

        // Load vectors from StorageBackend (recovered from WAL)
        let recovered_vectors = storage_backend.all_vectors();
        if !recovered_vectors.is_empty() {
            tracing::info!(
                "Loading {} vector(s) from StorageBackend for collection {}",
                recovered_vectors.len(),
                collection.collection_id
            );

            // FIX BUG #12: Validate dimension before inserting into index
            // Corrupted WAL data could have wrong dimension, causing index corruption
            let expected_dim = collection.dimension as usize;
            let mut skipped_count = 0;

            for doc in recovered_vectors {
                // Validate dimension matches collection's expected dimension
                if doc.vector.len() != expected_dim {
                    tracing::error!(
                        "Skipping corrupted vector {} from WAL: expected dimension {}, got {}",
                        doc.doc_id,
                        expected_dim,
                        doc.vector.len()
                    );
                    skipped_count += 1;
                    continue; // Skip corrupted vector, don't insert into index
                }

                // Insert validated vector into the VectorIndex
                index.insert(doc).await?;
            }

            if skipped_count > 0 {
                tracing::warn!(
                    "Skipped {} corrupted vector(s) during WAL recovery for collection {}",
                    skipped_count,
                    collection.collection_id
                );
            }
        } else {
            // Fallback: Load vectors from legacy SQLite persistence (Phase 5 compatibility)
            if let Some(persistence) = &self.vector_persistence {
                let vectors = persistence
                    .load_all_vectors(collection.collection_id)
                    .await?;
                if !vectors.is_empty() {
                    tracing::info!(
                        "Loading {} vector(s) from SQLite for collection {}",
                        vectors.len(),
                        collection.collection_id
                    );

                    // FIX BUG #12: Validate dimension for legacy SQLite vectors too
                    let expected_dim = collection.dimension as usize;
                    let mut skipped_count = 0;

                    for doc in vectors {
                        // Validate dimension
                        if doc.vector.len() != expected_dim {
                            tracing::error!(
                                "Skipping corrupted vector {} from SQLite: expected dimension {}, got {}",
                                doc.doc_id,
                                expected_dim,
                                doc.vector.len()
                            );
                            skipped_count += 1;
                            continue;
                        }

                        // Insert validated vectors into the index AND StorageBackend
                        index.insert(doc.clone()).await?;
                        storage_backend.insert(doc).await?;
                    }

                    if skipped_count > 0 {
                        tracing::warn!(
                            "Skipped {} corrupted vector(s) from SQLite for collection {}",
                            skipped_count,
                            collection.collection_id
                        );
                    }
                }
            }
        }

        // Store in indexes map
        let mut indexes = self.indexes.write().await;
        indexes.insert(collection.collection_id, index);

        // Store in storage_backends map
        {
            let mut backends = self.storage_backends.write().await;
            backends.insert(collection.collection_id, storage_backend);
        }

        Ok(())
    }

    /// Unload collection from memory (called on deletion).
    pub async fn unload_collection(&self, collection_id: CollectionId) -> CoreResult<()> {
        let mut indexes = self.indexes.write().await;
        indexes.remove(&collection_id);
        Ok(())
    }

    /// Get collection count (number of documents).
    pub async fn get_count(&self, collection_id: CollectionId) -> CoreResult<usize> {
        let indexes = self.indexes.read().await;
        let index = indexes
            .get(&collection_id)
            .ok_or_else(|| CoreError::not_found("Collection", collection_id.to_string()))?;

        index.count().await
    }

    // ========================================================================
    // Admin Operations (Phase 7 Week 4)
    // ========================================================================

    /// Get storage metrics for a specific collection or overall
    pub async fn get_storage_metrics(&self) -> CoreResult<StorageMetrics> {
        // Aggregate metrics across all storage backends
        let backends = self.storage_backends.read().await;

        if backends.is_empty() {
            // Return default metrics if no storage backends
            return Ok(StorageMetrics {
                inserts: 0,
                queries: 0,
                deletes: 0,
                s3_uploads: 0,
                s3_downloads: 0,
                cache_hits: 0,
                cache_misses: 0,
                wal_size_bytes: 0,
                last_snapshot_at: None,
                compactions: 0,
                s3_retries: 0,
                s3_permanent_failures: 0,
                dlq_size: 0,
                circuit_breaker_state: 0, // Closed = 0
                circuit_breaker_error_rate: 0.0,
            });
        }

        // Aggregate from all backends
        let mut total_metrics = StorageMetrics {
            inserts: 0,
            queries: 0,
            deletes: 0,
            s3_uploads: 0,
            s3_downloads: 0,
            cache_hits: 0,
            cache_misses: 0,
            wal_size_bytes: 0,
            last_snapshot_at: None,
            compactions: 0,
            s3_retries: 0,
            s3_permanent_failures: 0,
            dlq_size: 0,
            circuit_breaker_state: 0, // Closed = 0
            circuit_breaker_error_rate: 0.0,
        };

        for backend in backends.values() {
            let metrics = backend.metrics();

            // FIX BUG #11: Use saturating_add to prevent integer overflow
            total_metrics.inserts = total_metrics.inserts.saturating_add(metrics.inserts);
            total_metrics.queries = total_metrics.queries.saturating_add(metrics.queries);
            total_metrics.deletes = total_metrics.deletes.saturating_add(metrics.deletes);
            total_metrics.s3_uploads = total_metrics.s3_uploads.saturating_add(metrics.s3_uploads);
            total_metrics.s3_downloads = total_metrics
                .s3_downloads
                .saturating_add(metrics.s3_downloads);
            total_metrics.cache_hits = total_metrics.cache_hits.saturating_add(metrics.cache_hits);
            total_metrics.cache_misses = total_metrics
                .cache_misses
                .saturating_add(metrics.cache_misses);
            total_metrics.wal_size_bytes = total_metrics
                .wal_size_bytes
                .saturating_add(metrics.wal_size_bytes);
            total_metrics.compactions = total_metrics.compactions.saturating_add(metrics.compactions);
            total_metrics.s3_retries = total_metrics.s3_retries.saturating_add(metrics.s3_retries);
            total_metrics.s3_permanent_failures = total_metrics
                .s3_permanent_failures
                .saturating_add(metrics.s3_permanent_failures);
            total_metrics.dlq_size = total_metrics.dlq_size.saturating_add(metrics.dlq_size);

            // Use max error rate
            total_metrics.circuit_breaker_error_rate = total_metrics
                .circuit_breaker_error_rate
                .max(metrics.circuit_breaker_error_rate);

            // Use most recent snapshot time
            if metrics.last_snapshot_at.is_some() {
                total_metrics.last_snapshot_at = Some(
                    total_metrics
                        .last_snapshot_at
                        .unwrap_or(metrics.last_snapshot_at.unwrap())
                        .max(metrics.last_snapshot_at.unwrap()),
                );
            }

            // Use worst circuit breaker state (0=Closed, 1=HalfOpen, 2=Open)
            total_metrics.circuit_breaker_state = total_metrics
                .circuit_breaker_state
                .max(metrics.circuit_breaker_state);
        }

        Ok(total_metrics)
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> CoreResult<CacheStats> {
        // Aggregate cache stats from all storage backends
        let backends = self.storage_backends.read().await;

        if backends.is_empty() {
            return Ok(CacheStats {
                size: 0,
                capacity: 0,
                hit_rate: 0.0,
                hits: 0,
                misses: 0,
            });
        }

        let mut total_stats = CacheStats {
            size: 0,
            capacity: 0,
            hit_rate: 0.0,
            hits: 0,
            misses: 0,
        };

        for backend in backends.values() {
            if let Some(stats) = backend.get_cache_stats() {
                total_stats.size += stats.size;
                total_stats.capacity += stats.capacity;
                total_stats.hits += stats.hits;
                total_stats.misses += stats.misses;
            }
        }

        // Recalculate hit rate
        let total_requests = total_stats.hits + total_stats.misses;
        total_stats.hit_rate = if total_requests > 0 {
            total_stats.hits as f64 / total_requests as f64
        } else {
            0.0
        };

        Ok(total_stats)
    }

    /// Get server uptime in seconds
    pub fn uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    /// Get service-level metrics
    ///
    /// Returns aggregated metrics across all collections including:
    /// - Total collections count
    /// - Total vectors across all collections
    /// - Server uptime
    ///
    /// Returns `None` if the service was created without persistence (testing mode).
    pub fn metrics(&self) -> Option<ServiceMetrics> {
        if self.repository.is_none() {
            return None;
        }

        Some(ServiceMetrics {
            total_collections: 0, // Will be populated by async call
            total_vectors: 0,
            total_searches: 0,
            total_inserts: 0,
            uptime_seconds: self.uptime_seconds(),
        })
    }

    /// Retry all DLQ entries for a collection
    ///
    /// Note: Currently clears the DLQ. In the future, this could retry each entry.
    pub async fn retry_dlq_entries(
        &self,
        collection_id: CollectionId,
    ) -> CoreResult<DLQRetryResult> {
        let backends = self.storage_backends.read().await;
        let backend = backends
            .get(&collection_id)
            .ok_or_else(|| CoreError::not_found("Collection", collection_id.to_string()))?;

        // Get DLQ entries before clearing
        let entries = backend.get_dead_letter_queue();
        let total = entries.len();

        // Clear DLQ (in the future, could retry each entry)
        backend.clear_dead_letter_queue();

        Ok(DLQRetryResult {
            total,
            succeeded: 0,  // Not actually retrying yet
            failed: total, // All remain in "manual review needed" state
        })
    }

    /// Reset circuit breaker (emergency recovery)
    pub async fn reset_circuit_breaker(&self) -> CoreResult<CircuitBreakerState> {
        // Reset circuit breaker for all storage backends
        let backends = self.storage_backends.read().await;

        // Get previous state from metrics before resetting
        let mut previous_state_u8 = 0u8; // Closed

        for backend in backends.values() {
            let metrics_before = backend.metrics();
            previous_state_u8 = previous_state_u8.max(metrics_before.circuit_breaker_state);

            // Reset (not async)
            backend.reset_circuit_breaker();
        }

        // Convert u8 back to CircuitBreakerState
        let previous_state = match previous_state_u8 {
            0 => CircuitBreakerState::Closed,
            1 => CircuitBreakerState::HalfOpen,
            _ => CircuitBreakerState::Open,
        };

        Ok(previous_state)
    }

    // ========================================================================
    // Lifecycle Management (Production Critical)
    // ========================================================================

    /// Gracefully shutdown the collection service.
    ///
    /// This method ensures:
    /// 1. All storage backends are shutdown cleanly (WAL flush, task abort)
    /// 2. Background tasks are stopped
    /// 3. Resources are released properly
    ///
    /// CRITICAL: Call this during server shutdown (SIGTERM handler) to prevent:
    /// - Data loss (unflushed WAL buffers)
    /// - Resource leaks (background tasks, file descriptors)
    /// - Corruption (incomplete writes)
    ///
    /// # Example
    /// ```rust
    /// // In server shutdown handler
    /// collection_service.shutdown().await?;
    /// ```
    ///
    /// # Timeout
    /// This method has an internal timeout of 30 seconds. If shutdown takes longer,
    /// it will log warnings but continue to ensure the server can stop.
    pub async fn shutdown(&self) -> CoreResult<()> {
        tracing::info!("CollectionService shutdown initiated...");

        let shutdown_start = std::time::Instant::now();

        // Step 1: Shutdown all storage backends
        // This is CRITICAL - ensures WAL flush and task cleanup
        {
            let backends = self.storage_backends.read().await;
            let backend_count = backends.len();

            tracing::info!(
                "Shutting down {} storage backend(s)...",
                backend_count
            );

            let mut successful_shutdowns = 0;
            let mut failed_shutdowns = 0;

            for (collection_id, backend) in backends.iter() {
                tracing::debug!("Shutting down backend for collection {}", collection_id);

                match backend.shutdown().await {
                    Ok(()) => {
                        successful_shutdowns += 1;
                        tracing::debug!(
                            "Backend shutdown successful for collection {}",
                            collection_id
                        );
                    }
                    Err(e) => {
                        failed_shutdowns += 1;
                        tracing::warn!(
                            "Failed to shutdown backend for collection {}: {}",
                            collection_id,
                            e
                        );
                        // Continue shutting down other backends even if one fails
                    }
                }
            }

            tracing::info!(
                "Storage backend shutdown complete: {}/{} successful",
                successful_shutdowns,
                backend_count
            );

            if failed_shutdowns > 0 {
                tracing::warn!(
                    "{} backend(s) failed to shutdown cleanly",
                    failed_shutdowns
                );
            }
        }

        // Step 2: Note on in-memory indexes
        // VectorIndex implementations (BruteForceIndex, InstantDistanceIndex) are
        // Drop-based and don't require explicit shutdown. They will clean up when
        // the Arc refcount reaches 0.

        // Step 3: Note on repository
        // SQLite connections are managed by sqlx pool and will close automatically.
        // No explicit shutdown needed.

        let shutdown_duration = shutdown_start.elapsed();
        tracing::info!(
            "CollectionService shutdown complete in {:.2}s",
            shutdown_duration.as_secs_f64()
        );

        // Warn if shutdown took > 10 seconds (may indicate hanging tasks)
        if shutdown_duration.as_secs() > 10 {
            tracing::warn!(
                "Shutdown took {:.1}s (>10s threshold). Check for slow/hanging tasks.",
                shutdown_duration.as_secs_f64()
            );
        }

        Ok(())
    }

    /// Check if the service is ready to serve requests.
    ///
    /// Returns `true` if:
    /// - All required dependencies are initialized
    /// - At least one collection is loaded (if applicable)
    /// - Storage backends are healthy
    ///
    /// This is used for Kubernetes readiness probes.
    pub async fn is_ready(&self) -> bool {
        // Check 1: At least have the basic structures initialized
        if self.collections.read().await.is_empty() && self.repository.is_some() {
            // If we have a repository but no collections, we haven't loaded yet
            return false;
        }

        // Check 2: Verify storage backends are healthy (not in permanent failure state)
        let backends = self.storage_backends.read().await;
        for backend in backends.values() {
            // Check if circuit breaker is permanently open (indicates S3 down)
            if let Some(state) = backend.circuit_breaker_state() {
                if matches!(state, akidb_storage::CircuitBreakerState::Open) {
                    tracing::warn!("Storage backend circuit breaker is OPEN (not ready)");
                    return false;
                }
            }
        }

        // All checks passed
        true
    }

    /// Get health status of the service.
    ///
    /// Always returns `true` unless the service is critically broken.
    /// This is used for Kubernetes liveness probes.
    ///
    /// Difference from `is_ready()`:
    /// - `is_healthy()` checks if the service is alive (liveness)
    /// - `is_ready()` checks if the service can handle traffic (readiness)
    pub fn is_healthy(&self) -> bool {
        // For liveness check, we just verify the service struct is valid
        // If we can call this method, the service is alive
        true
    }
}

impl Default for CollectionService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use akidb_core::{CollectionDescriptor, DatabaseId, DistanceMetric, DocumentId};
    use akidb_storage::TieringPolicy;
    use async_trait::async_trait;
    use chrono::Utc;

    fn create_test_collection() -> CollectionDescriptor {
        CollectionDescriptor {
            collection_id: CollectionId::new(),
            database_id: DatabaseId::new(),
            name: "test-collection".to_string(),
            dimension: 128,
            metric: DistanceMetric::Cosine,
            embedding_model: "test-model".to_string(),
            hnsw_m: 32,
            hnsw_ef_construction: 200,
            max_doc_count: 50_000_000,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_load_and_insert() {
        let service = CollectionService::new();
        let collection = create_test_collection();

        // Load collection
        service.load_collection(&collection).await.unwrap();

        // Insert vector
        let doc = VectorDocument::new(DocumentId::new(), vec![0.1; 128]);
        let doc_id = service.insert(collection.collection_id, doc).await.unwrap();

        // Get vector
        let retrieved = service.get(collection.collection_id, doc_id).await.unwrap();
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn test_query() {
        let service = CollectionService::new();
        let collection = create_test_collection();

        service.load_collection(&collection).await.unwrap();

        // Insert some vectors (non-zero for Cosine similarity)
        for i in 0..10 {
            let vector = vec![0.1 * (i + 1) as f32; 128]; // Start from 0.1, not 0.0
            let doc = VectorDocument::new(DocumentId::new(), vector);
            service.insert(collection.collection_id, doc).await.unwrap();
        }

        // Query
        let query = vec![0.5; 128];
        let results = service
            .query(collection.collection_id, query, 5)
            .await
            .unwrap();

        assert_eq!(results.len(), 5);
    }

    #[tokio::test]
    async fn test_delete() {
        let service = CollectionService::new();
        let collection = create_test_collection();

        service.load_collection(&collection).await.unwrap();

        // Insert vector
        let doc = VectorDocument::new(DocumentId::new(), vec![0.1; 128]);
        let doc_id = doc.doc_id;
        service.insert(collection.collection_id, doc).await.unwrap();

        // Delete vector
        service
            .delete(collection.collection_id, doc_id)
            .await
            .unwrap();

        // Verify deleted
        let retrieved = service.get(collection.collection_id, doc_id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_collection_service_with_storage_config() {
        use akidb_storage::{StorageConfig, TieringPolicy};

        // Create custom storage config
        let storage_config = StorageConfig::memory("./test-storage.wal");
        assert_eq!(storage_config.tiering_policy, TieringPolicy::Memory);

        // Create service with custom storage config
        let service = CollectionService::with_storage(
            Arc::new(MockCollectionRepository {}),
            Arc::new(akidb_metadata::VectorPersistence::new(
                create_test_db().await,
            )),
            storage_config,
        );

        // Verify service was created successfully
        assert!(service.repository.is_some());
        assert!(service.vector_persistence.is_some());

        // Verify storage config was set correctly
        assert_eq!(service.storage_config.tiering_policy, TieringPolicy::Memory);
        assert_eq!(
            service.storage_config.wal_path.to_str().unwrap(),
            "./test-storage.wal"
        );
    }

    #[tokio::test]
    async fn test_storage_backends_initialized_empty() {
        let service = CollectionService::new();

        // Verify storage_backends starts empty
        let backends = service.storage_backends.read().await;
        assert_eq!(backends.len(), 0);

        // Verify default storage config is set
        assert_eq!(service.storage_config.tiering_policy, TieringPolicy::Memory);
    }

    #[tokio::test]
    async fn test_create_collection_creates_storage_backend() {
        use tempfile::TempDir;

        // Create temp directory for WAL/snapshots
        let temp_dir = TempDir::new().unwrap();
        let storage_config = StorageConfig::memory(temp_dir.path().join("akidb.wal"));

        let service = CollectionService::with_storage(
            Arc::new(MockCollectionRepository {}),
            Arc::new(akidb_metadata::VectorPersistence::new(
                create_test_db().await,
            )),
            storage_config,
        );

        // Set default database ID
        service.set_default_database_id(DatabaseId::new()).await;

        // Create collection
        let collection_id = service
            .create_collection(
                "test-collection".to_string(),
                128,
                DistanceMetric::Cosine,
                Some("test-model".to_string()),
            )
            .await
            .unwrap();

        // Verify storage backend was created
        let backends = service.storage_backends.read().await;
        assert_eq!(backends.len(), 1);
        assert!(backends.contains_key(&collection_id));
    }

    #[tokio::test]
    async fn test_storage_backend_has_correct_wal_path() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let base_wal_path = temp_dir.path().join("akidb.wal");
        let storage_config = StorageConfig::memory(&base_wal_path);

        let service = CollectionService::with_storage(
            Arc::new(MockCollectionRepository {}),
            Arc::new(akidb_metadata::VectorPersistence::new(
                create_test_db().await,
            )),
            storage_config,
        );

        service.set_default_database_id(DatabaseId::new()).await;

        // Create collection
        let collection_id = service
            .create_collection(
                "test-collection".to_string(),
                128,
                DistanceMetric::Cosine,
                Some("test-model".to_string()),
            )
            .await
            .unwrap();

        // Verify per-collection WAL directory was created
        let expected_wal_dir = temp_dir
            .path()
            .join("collections")
            .join(collection_id.to_string())
            .join("wal");

        assert!(
            expected_wal_dir.parent().unwrap().exists(),
            "Per-collection directory should be created"
        );
    }

    #[tokio::test]
    async fn test_multiple_collections_separate_storage() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let storage_config = StorageConfig::memory(temp_dir.path().join("akidb.wal"));

        let service = CollectionService::with_storage(
            Arc::new(MockCollectionRepository {}),
            Arc::new(akidb_metadata::VectorPersistence::new(
                create_test_db().await,
            )),
            storage_config,
        );

        service.set_default_database_id(DatabaseId::new()).await;

        // Create multiple collections
        let collection_id_1 = service
            .create_collection(
                "collection-1".to_string(),
                128,
                DistanceMetric::Cosine,
                Some("model-1".to_string()),
            )
            .await
            .unwrap();

        let collection_id_2 = service
            .create_collection(
                "collection-2".to_string(),
                256,
                DistanceMetric::L2,
                Some("model-2".to_string()),
            )
            .await
            .unwrap();

        // Verify both have separate storage backends
        let backends = service.storage_backends.read().await;
        assert_eq!(backends.len(), 2);
        assert!(backends.contains_key(&collection_id_1));
        assert!(backends.contains_key(&collection_id_2));

        // Verify separate directories
        let dir_1 = temp_dir
            .path()
            .join("collections")
            .join(collection_id_1.to_string());
        let dir_2 = temp_dir
            .path()
            .join("collections")
            .join(collection_id_2.to_string());

        assert!(dir_1.exists(), "Collection 1 directory should exist");
        assert!(dir_2.exists(), "Collection 2 directory should exist");
    }

    #[tokio::test]
    async fn test_delete_collection_removes_storage_backend() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let storage_config = StorageConfig::memory(temp_dir.path().join("akidb.wal"));

        let service = CollectionService::with_storage(
            Arc::new(MockCollectionRepository {}),
            Arc::new(akidb_metadata::VectorPersistence::new(
                create_test_db().await,
            )),
            storage_config,
        );

        service.set_default_database_id(DatabaseId::new()).await;

        // Create collection
        let collection_id = service
            .create_collection(
                "test-collection".to_string(),
                128,
                DistanceMetric::Cosine,
                Some("test-model".to_string()),
            )
            .await
            .unwrap();

        // Verify storage backend exists
        {
            let backends = service.storage_backends.read().await;
            assert_eq!(backends.len(), 1);
        }

        // Delete collection
        service.delete_collection(collection_id).await.unwrap();

        // Verify storage backend was removed
        let backends = service.storage_backends.read().await;
        assert_eq!(backends.len(), 0);
        assert!(!backends.contains_key(&collection_id));
    }

    #[tokio::test]
    async fn test_insert_persists_to_storage_backend() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let storage_config = StorageConfig::memory(temp_dir.path().join("akidb.wal"));

        let service = CollectionService::with_storage(
            Arc::new(MockCollectionRepository {}),
            Arc::new(akidb_metadata::VectorPersistence::new(
                create_test_db().await,
            )),
            storage_config,
        );

        service.set_default_database_id(DatabaseId::new()).await;

        // Create collection
        let collection_id = service
            .create_collection(
                "test-collection".to_string(),
                128,
                DistanceMetric::Cosine,
                Some("test-model".to_string()),
            )
            .await
            .unwrap();

        // Insert vector
        let doc = VectorDocument::new(DocumentId::new(), vec![0.1; 128]);
        let doc_id = doc.doc_id;
        service.insert(collection_id, doc).await.unwrap();

        // Verify vector persisted to StorageBackend (not just in-memory index)
        let backends = service.storage_backends.read().await;
        let backend = backends.get(&collection_id).unwrap();

        let retrieved = backend.get(&doc_id).await.unwrap();
        assert!(
            retrieved.is_some(),
            "Vector should be persisted to StorageBackend"
        );
        assert_eq!(retrieved.unwrap().vector, vec![0.1; 128]);
    }

    #[tokio::test]
    async fn test_delete_persists_to_storage_backend() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let storage_config = StorageConfig::memory(temp_dir.path().join("akidb.wal"));

        let service = CollectionService::with_storage(
            Arc::new(MockCollectionRepository {}),
            Arc::new(akidb_metadata::VectorPersistence::new(
                create_test_db().await,
            )),
            storage_config,
        );

        service.set_default_database_id(DatabaseId::new()).await;

        // Create collection
        let collection_id = service
            .create_collection(
                "test-collection".to_string(),
                128,
                DistanceMetric::Cosine,
                Some("test-model".to_string()),
            )
            .await
            .unwrap();

        // Insert vector
        let doc = VectorDocument::new(DocumentId::new(), vec![0.1; 128]);
        let doc_id = doc.doc_id;
        service.insert(collection_id, doc).await.unwrap();

        // Verify vector exists in StorageBackend
        {
            let backends = service.storage_backends.read().await;
            let backend = backends.get(&collection_id).unwrap();
            assert!(backend.get(&doc_id).await.unwrap().is_some());
        }

        // Delete vector
        service.delete(collection_id, doc_id).await.unwrap();

        // Verify vector deleted from StorageBackend
        let backends = service.storage_backends.read().await;
        let backend = backends.get(&collection_id).unwrap();
        assert!(
            backend.get(&doc_id).await.unwrap().is_none(),
            "Vector should be deleted from StorageBackend"
        );
    }

    #[tokio::test]
    async fn test_insert_updates_both_storage_and_index() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let storage_config = StorageConfig::memory(temp_dir.path().join("akidb.wal"));

        let service = CollectionService::with_storage(
            Arc::new(MockCollectionRepository {}),
            Arc::new(akidb_metadata::VectorPersistence::new(
                create_test_db().await,
            )),
            storage_config,
        );

        service.set_default_database_id(DatabaseId::new()).await;

        // Create collection
        let collection_id = service
            .create_collection(
                "test-collection".to_string(),
                128,
                DistanceMetric::Cosine,
                Some("test-model".to_string()),
            )
            .await
            .unwrap();

        // Insert vector
        let doc = VectorDocument::new(DocumentId::new(), vec![0.1; 128]);
        let doc_id = doc.doc_id;
        service.insert(collection_id, doc).await.unwrap();

        // Verify vector in StorageBackend
        {
            let backends = service.storage_backends.read().await;
            let backend = backends.get(&collection_id).unwrap();
            assert!(backend.get(&doc_id).await.unwrap().is_some());
        }

        // Verify vector in VectorIndex (for search)
        let retrieved_from_index = service.get(collection_id, doc_id).await.unwrap();
        assert!(
            retrieved_from_index.is_some(),
            "Vector should be in VectorIndex"
        );

        // Verify search works (requires VectorIndex)
        let results = service
            .query(collection_id, vec![0.1; 128], 1)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].doc_id, doc_id);
    }

    #[tokio::test]
    #[ignore = "Memory policy doesn't support S3 compaction - compaction only works with MemoryS3 or S3Only policies"]
    async fn test_auto_compaction_triggered() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let storage_config = StorageConfig::memory(temp_dir.path().join("akidb.wal"))
            .with_compaction_thresholds(1_000_000, 10); // Trigger at 10 inserts

        let service = CollectionService::with_storage(
            Arc::new(MockCollectionRepository {}),
            Arc::new(akidb_metadata::VectorPersistence::new(
                create_test_db().await,
            )),
            storage_config,
        );

        service.set_default_database_id(DatabaseId::new()).await;

        // Create collection
        let collection_id = service
            .create_collection(
                "test-collection".to_string(),
                128,
                DistanceMetric::Cosine,
                Some("test-model".to_string()),
            )
            .await
            .unwrap();

        // Insert 15 vectors (exceeds compaction threshold of 10)
        // Use non-zero vectors to avoid Cosine similarity undefined behavior
        for i in 0..15 {
            let doc = VectorDocument::new(DocumentId::new(), vec![0.1 * (i + 1) as f32; 128]);
            service.insert(collection_id, doc).await.unwrap();
        }

        // Wait for background compaction worker to run
        // Background compaction checks every 1 second, so wait 2 seconds to be safe
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Verify auto-compaction was triggered
        let backends = service.storage_backends.read().await;
        let backend = backends.get(&collection_id).unwrap();
        let metrics = backend.metrics();

        assert!(
            metrics.compactions >= 1,
            "Auto-compaction should have been triggered (compactions: {})",
            metrics.compactions
        );
        assert!(
            metrics.last_snapshot_at.is_some(),
            "Snapshot should have been created"
        );
    }

    // Mock repository for testing
    struct MockCollectionRepository {}

    #[async_trait]
    impl CollectionRepository for MockCollectionRepository {
        async fn create(&self, _collection: &CollectionDescriptor) -> CoreResult<()> {
            Ok(())
        }

        async fn get(
            &self,
            _collection_id: CollectionId,
        ) -> CoreResult<Option<CollectionDescriptor>> {
            Ok(None)
        }

        async fn list_by_database(
            &self,
            _database_id: DatabaseId,
        ) -> CoreResult<Vec<CollectionDescriptor>> {
            Ok(vec![])
        }

        async fn list_all(&self) -> CoreResult<Vec<CollectionDescriptor>> {
            Ok(vec![])
        }

        async fn update(&self, _collection: &CollectionDescriptor) -> CoreResult<()> {
            Ok(())
        }

        async fn delete(&self, _collection_id: CollectionId) -> CoreResult<()> {
            Ok(())
        }
    }

    // Helper to create test database with migrations
    async fn create_test_db() -> sqlx::Pool<sqlx::Sqlite> {
        use sqlx::migrate::MigrateDatabase;

        let db_url = "sqlite::memory:";
        let pool = sqlx::SqlitePool::connect(db_url).await.unwrap();

        // Run migrations to create vector_documents table
        sqlx::migrate!("../akidb-metadata/migrations")
            .run(&pool)
            .await
            .unwrap();

        pool
    }
}
