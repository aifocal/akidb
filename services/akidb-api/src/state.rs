//! API state management
//!
//! This module defines the shared state for the API server, including
//! storage backends, index providers, and query components.

use akidb_core::{collection::CollectionDescriptor, manifest::CollectionManifest, Result};
use akidb_index::{IndexHandle, IndexProvider};
use akidb_query::{ExecutionEngine, QueryPlanner};
use akidb_storage::StorageBackend;
use std::collections::HashMap;
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
    /// Collection metadata (in-memory for now)
    collections: Arc<RwLock<HashMap<String, CollectionMetadata>>>,
}

/// Metadata for a collection
#[derive(Debug, Clone)]
pub struct CollectionMetadata {
    pub descriptor: Arc<CollectionDescriptor>,
    pub manifest: CollectionManifest,
    pub index_handle: Option<IndexHandle>,
}

impl AppState {
    /// Create a new application state
    pub fn new(
        storage: Arc<dyn StorageBackend>,
        index_provider: Arc<dyn IndexProvider>,
        planner: Arc<dyn QueryPlanner>,
        engine: Arc<dyn ExecutionEngine>,
    ) -> Self {
        Self {
            storage,
            index_provider,
            planner,
            engine,
            collections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new collection
    pub async fn register_collection(
        &self,
        name: String,
        descriptor: Arc<CollectionDescriptor>,
        manifest: CollectionManifest,
    ) -> Result<()> {
        info!("Registering collection: {}", name);

        let metadata = CollectionMetadata {
            descriptor,
            manifest,
            index_handle: None,
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
    pub async fn update_index_handle(&self, name: &str, handle: IndexHandle) -> Result<()> {
        let mut collections = self.collections.write().await;

        if let Some(metadata) = collections.get_mut(name) {
            metadata.index_handle = Some(handle);
            debug!("Updated index handle for collection: {}", name);
            Ok(())
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
}
