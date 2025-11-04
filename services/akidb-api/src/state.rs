//! API state management
//!
//! This module defines the shared state for the API server, including
//! storage backends, index providers, and query components.

use crate::query_cache::QueryCache;
use akidb_core::{collection::CollectionDescriptor, manifest::CollectionManifest, Result};
use akidb_index::{IndexHandle, IndexProvider};
use akidb_query::{BatchExecutionEngine, ExecutionEngine, FilterCache, QueryPlanner};
use akidb_storage::{MetadataStore, S3WalBackend, StorageBackend, WalStreamId};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Shared application state for the API server
#[derive(Clone)]
pub struct AppState {
    /// Storage backend for persistence
    pub storage: Arc<dyn StorageBackend>,
    /// Index provider for vector search
    pub index_provider: Arc<dyn IndexProvider>,
    /// Query planner
    pub planner: Arc<dyn QueryPlanner>,
    /// Execution engine
    pub engine: Arc<dyn ExecutionEngine>,
    /// Batch execution engine
    pub batch_engine: Arc<BatchExecutionEngine>,
    /// Metadata store for filter queries
    pub metadata_store: Arc<dyn MetadataStore>,
    /// Write-Ahead Log backend for durability
    pub wal: Arc<S3WalBackend>,
    /// Query result cache
    pub query_cache: Arc<QueryCache>,
    /// Filter AST cache for pre-compiled filters
    pub filter_cache: Arc<FilterCache>,
    /// Collection metadata (in-memory for now)
    collections: Arc<RwLock<HashMap<String, CollectionMetadata>>>,
}

/// Metadata for a collection
#[derive(Debug, Clone)]
pub struct CollectionMetadata {
    pub descriptor: Arc<CollectionDescriptor>,
    pub manifest: CollectionManifest,
    pub index_handle: Option<IndexHandle>,
    pub next_doc_id: Arc<AtomicU32>,
    /// WAL stream ID for this collection
    pub wal_stream_id: WalStreamId,
    /// Collection epoch for cache invalidation (increments on every insert)
    pub epoch: Arc<AtomicU64>,
}

impl AppState {
    /// Create a new application state
    #[allow(clippy::too_many_arguments)] // Constructor with required dependencies
    pub fn new(
        storage: Arc<dyn StorageBackend>,
        index_provider: Arc<dyn IndexProvider>,
        planner: Arc<dyn QueryPlanner>,
        engine: Arc<dyn ExecutionEngine>,
        batch_engine: Arc<BatchExecutionEngine>,
        metadata_store: Arc<dyn MetadataStore>,
        wal: Arc<S3WalBackend>,
        query_cache: Arc<QueryCache>,
    ) -> Self {
        Self {
            storage,
            index_provider,
            planner,
            engine,
            batch_engine,
            metadata_store,
            wal,
            query_cache,
            filter_cache: Arc::new(FilterCache::default()),
            collections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new collection
    pub async fn register_collection(
        &self,
        name: String,
        descriptor: Arc<CollectionDescriptor>,
        manifest: CollectionManifest,
        initial_doc_id: u32,
        wal_stream_id: WalStreamId,
    ) -> Result<()> {
        info!("Registering collection: {}", name);

        let metadata = CollectionMetadata {
            descriptor,
            manifest,
            index_handle: None,
            next_doc_id: Arc::new(AtomicU32::new(initial_doc_id)),
            wal_stream_id,
            epoch: Arc::new(AtomicU64::new(0)),
        };

        let mut collections = self.collections.write().await;
        collections.insert(name.clone(), metadata);

        debug!("Collection {} registered successfully", name);
        Ok(())
    }

    /// Get collection metadata
    pub async fn get_collection(&self, name: &str) -> Result<CollectionMetadata> {
        let collections = self.collections.read().await;
        collections
            .get(name)
            .cloned()
            .ok_or_else(|| akidb_core::Error::NotFound(format!("Collection '{}' not found", name)))
    }

    /// Update collection index handle
    ///
    /// Returns the actual handle that was set (either the new one or an existing one if set concurrently)
    pub async fn update_index_handle(
        &self,
        name: &str,
        handle: IndexHandle,
    ) -> Result<IndexHandle> {
        let mut collections = self.collections.write().await;

        if let Some(metadata) = collections.get_mut(name) {
            // CRITICAL: Check if index was already set by concurrent thread
            // This prevents TOCTOU race where multiple threads build duplicate indices
            if let Some(existing_handle) = &metadata.index_handle {
                debug!(
                    "Index handle already set for collection '{}' by concurrent thread, using existing handle {}",
                    name, existing_handle.index_id
                );
                return Ok(existing_handle.clone());
            }

            metadata.index_handle = Some(handle.clone());
            debug!("Updated index handle for collection: {}", name);
            Ok(handle)
        } else {
            Err(akidb_core::Error::NotFound(format!(
                "Collection '{}' not found",
                name
            )))
        }
    }

    /// Check if collection exists
    pub async fn collection_exists(&self, name: &str) -> bool {
        let collections = self.collections.read().await;
        collections.contains_key(name)
    }

    /// List all collection names
    pub async fn list_collections(&self) -> Vec<String> {
        let collections = self.collections.read().await;
        collections.keys().cloned().collect()
    }

    /// Delete a collection
    pub async fn delete_collection(&self, name: &str) -> Result<()> {
        info!("Deleting collection: {}", name);

        // Remove from app state
        let mut collections = self.collections.write().await;
        if collections.remove(name).is_none() {
            return Err(akidb_core::Error::NotFound(format!(
                "Collection '{}' not found",
                name
            )));
        }
        drop(collections);

        // Delete from storage
        self.storage.drop_collection(name).await?;

        debug!("Collection {} deleted successfully", name);
        Ok(())
    }

    /// Bump collection epoch for cache invalidation
    ///
    /// Call this whenever vectors are inserted to invalidate cached queries
    ///
    /// # Safety
    ///
    /// Uses saturating_add to prevent epoch overflow (after 2^64 inserts).
    /// While extremely unlikely in practice, overflow could cause cache poisoning
    /// if old cached results (with epoch 2^64 - 1) wrap around and become "valid" again.
    pub async fn bump_collection_epoch(&self, name: &str) -> Result<u64> {
        let collections = self.collections.read().await;

        if let Some(metadata) = collections.get(name) {
            // SAFETY: Use fetch_update with saturating_add to prevent overflow
            let new_epoch = metadata
                .epoch
                .fetch_update(
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                    |current| Some(current.saturating_add(1)),
                )
                .expect("fetch_update with Some(_) never fails")
                .saturating_add(1); // Add 1 to get the new value (fetch_update returns old value)

            debug!("Bumped epoch for collection '{}' to {}", name, new_epoch);
            Ok(new_epoch)
        } else {
            Err(akidb_core::Error::NotFound(format!(
                "Collection '{}' not found",
                name
            )))
        }
    }
}
