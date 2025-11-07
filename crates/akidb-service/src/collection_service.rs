//! Service layer for collection operations.
//! Shared by gRPC and REST APIs.

use akidb_core::{
    CollectionDescriptor, CollectionId, CollectionRepository, CoreError, CoreResult, DatabaseId,
    DistanceMetric, DocumentId, SearchResult, VectorDocument, VectorIndex,
};
use akidb_index::{BruteForceIndex, InstantDistanceConfig, InstantDistanceIndex};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Service layer for collection operations.
/// Shared by gRPC and REST APIs.
pub struct CollectionService {
    // SQLite repository for persistence (optional for testing)
    repository: Option<Arc<dyn CollectionRepository>>,

    // In-memory cache for fast reads (synced with repository)
    collections: Arc<RwLock<HashMap<CollectionId, CollectionDescriptor>>>,

    // In-memory vector indexes (collection_id -> VectorIndex)
    indexes: Arc<RwLock<HashMap<CollectionId, Box<dyn VectorIndex>>>>,

    // Default database_id for RC1 (single-database mode)
    default_database_id: Arc<RwLock<Option<DatabaseId>>>,
}

impl CollectionService {
    /// Creates a new collection service with in-memory storage only.
    /// For testing purposes. Production should use `with_repository()`.
    pub fn new() -> Self {
        Self {
            repository: None,
            collections: Arc::new(RwLock::new(HashMap::new())),
            indexes: Arc::new(RwLock::new(HashMap::new())),
            default_database_id: Arc::new(RwLock::new(None)),
        }
    }

    /// Creates a new collection service with SQLite persistence.
    /// Collections are persisted to the database and loaded on startup.
    pub fn with_repository(repository: Arc<dyn CollectionRepository>) -> Self {
        Self {
            repository: Some(repository),
            collections: Arc::new(RwLock::new(HashMap::new())),
            indexes: Arc::new(RwLock::new(HashMap::new())),
            default_database_id: Arc::new(RwLock::new(None)),
        }
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
        // Validate dimension
        if !(16..=4096).contains(&dimension) {
            return Err(CoreError::invalid_state(
                format!("dimension must be between 16 and 4096, got {}", dimension),
            ));
        }

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
            embedding_model: embedding_model.unwrap_or_else(|| "none".to_string()),
            hnsw_m: 32,
            hnsw_ef_construction: 200,
            max_doc_count: 50_000_000,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Persist to SQLite if repository exists
        if let Some(repo) = &self.repository {
            repo.create(&collection).await?;
        }

        // Store in cache
        {
            let mut collections = self.collections.write().await;
            collections.insert(collection_id, collection.clone());
        }

        // Create and load index
        self.load_collection(&collection).await?;

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
        // Get index
        let indexes = self.indexes.read().await;
        let index = indexes
            .get(&collection_id)
            .ok_or_else(|| CoreError::not_found("Collection", collection_id.to_string()))?;

        // Perform search
        index.search(&query_vector, top_k, None).await
    }

    /// Insert single vector.
    pub async fn insert(
        &self,
        collection_id: CollectionId,
        doc: VectorDocument,
    ) -> CoreResult<DocumentId> {
        // Get index
        let indexes = self.indexes.read().await;
        let index = indexes
            .get(&collection_id)
            .ok_or_else(|| CoreError::not_found("Collection", collection_id.to_string()))?;

        // Insert document
        let doc_id = doc.doc_id;
        index.insert(doc).await?;
        Ok(doc_id)
    }

    /// Get vector by ID.
    pub async fn get(
        &self,
        collection_id: CollectionId,
        doc_id: DocumentId,
    ) -> CoreResult<Option<VectorDocument>> {
        let indexes = self.indexes.read().await;
        let index = indexes
            .get(&collection_id)
            .ok_or_else(|| CoreError::not_found("Collection", collection_id.to_string()))?;

        index.get(doc_id).await
    }

    /// Delete vector by ID.
    pub async fn delete(
        &self,
        collection_id: CollectionId,
        doc_id: DocumentId,
    ) -> CoreResult<()> {
        let indexes = self.indexes.read().await;
        let index = indexes
            .get(&collection_id)
            .ok_or_else(|| CoreError::not_found("Collection", collection_id.to_string()))?;

        index.delete(doc_id).await
    }

    /// Load collection into memory (called on startup or creation).
    /// Creates appropriate index based on collection config.
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

        // Store in indexes map
        let mut indexes = self.indexes.write().await;
        indexes.insert(collection.collection_id, index);

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
        let doc_id = service
            .insert(collection.collection_id, doc)
            .await
            .unwrap();

        // Get vector
        let retrieved = service
            .get(collection.collection_id, doc_id)
            .await
            .unwrap();
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
        let results = service.query(collection.collection_id, query, 5).await.unwrap();

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
        service.delete(collection.collection_id, doc_id).await.unwrap();

        // Verify deleted
        let retrieved = service.get(collection.collection_id, doc_id).await.unwrap();
        assert!(retrieved.is_none());
    }
}
