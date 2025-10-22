//! Collection bootstrap and recovery from storage
//!
//! This module handles loading collections from S3 on server startup,
//! enabling restart recovery.

use crate::state::{AppState, CollectionMetadata};
use akidb_core::{collection::CollectionDescriptor, Error, Result};
use akidb_index::{BuildRequest, IndexBatch, IndexProvider, QueryVector};
use akidb_storage::StorageBackend;
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Discover all collections in storage
///
/// Scans S3 for all collection manifest files and returns collection names.
async fn discover_collections(storage: &dyn StorageBackend) -> Result<Vec<String>> {
    debug!("Discovering collections in storage...");

    // List all objects in collections/ prefix
    let objects = storage.list_objects("collections/").await?;

    // Filter for manifest.json files and extract collection names
    let mut collections = Vec::new();
    for key in objects {
        // Pattern: collections/{name}/manifest.json
        if key.ends_with("/manifest.json") {
            let parts: Vec<&str> = key.split('/').collect();
            if parts.len() >= 2 {
                let collection_name = parts[1].to_string();
                if !collection_name.is_empty() {
                    collections.push(collection_name);
                }
            }
        }
    }

    // Remove duplicates (shouldn't happen, but be safe)
    collections.sort();
    collections.dedup();

    debug!("Found {} collections", collections.len());
    Ok(collections)
}

/// Extract primary key from payload or generate UUID
fn extract_primary_key(payload: &Value, index: usize) -> String {
    // Try to get "id" field from payload
    if let Some(id) = payload.get("id") {
        match id {
            Value::String(s) => return s.clone(),
            Value::Number(n) => return n.to_string(),
            Value::Bool(b) => return b.to_string(),
            _ => {}
        }
    }

    // Fallback: use index as key (not ideal, but backward compatible)
    format!("vector_{}", index)
}

/// Load a single collection from storage
///
/// This function:
/// 1. Loads the manifest and descriptor
/// 2. Loads all segments with their vectors and payloads
/// 3. Rebuilds the index from scratch
/// 4. Returns CollectionMetadata ready for registration
async fn load_collection(
    name: &str,
    storage: &dyn StorageBackend,
    index_provider: &dyn IndexProvider,
) -> Result<CollectionMetadata> {
    info!("Loading collection: {}", name);

    // 1. Load manifest
    let manifest = storage.load_manifest(name).await?;
    debug!(
        "Loaded manifest for '{}': {} vectors, {} segments",
        name,
        manifest.total_vectors,
        manifest.segments.len()
    );

    // 2. Load descriptor
    let descriptor_key = format!("collections/{}/descriptor.json", name);
    let descriptor_data = storage.get_object(&descriptor_key).await?;
    let descriptor: CollectionDescriptor = serde_json::from_slice(&descriptor_data)
        .map_err(|e| Error::Storage(format!("Failed to deserialize descriptor: {}", e)))?;

    debug!(
        "Loaded descriptor for '{}': dim={}, distance={:?}",
        name, descriptor.vector_dim, descriptor.distance
    );

    // 3. Build index structure (initially empty)
    let build_request = BuildRequest {
        collection: name.to_string(),
        kind: index_provider.kind(),
        distance: descriptor.distance,
        segments: manifest.segments.clone(),
    };

    let index_handle = index_provider.build(build_request).await?;
    debug!(
        "Created index {} for collection '{}'",
        index_handle.index_id, name
    );

    // 4. Load segments and populate index
    let mut total_vectors_loaded = 0;

    for segment_desc in &manifest.segments {
        debug!(
            "Loading segment {} ({} records expected)",
            segment_desc.segment_id, segment_desc.record_count
        );

        // Load segment data using StorageBackend's load_segment()
        // Note: This might not be available on all backends
        // For now, we'll handle the case where segments exist but can't be loaded
        match storage.load_segment(name, segment_desc.segment_id).await {
            Ok(segment_data) => {
                let vectors = segment_data.vectors;
                let vector_count = vectors.len();

                // Extract payloads from metadata if available
                let payloads = if let Some(metadata) = segment_data.metadata {
                    match metadata.to_json() {
                        Ok(p) => p,
                        Err(e) => {
                            warn!(
                                "Failed to extract payloads from segment {}: {}",
                                segment_desc.segment_id, e
                            );
                            vec![Value::Null; vector_count]
                        }
                    }
                } else {
                    // Backward compatibility: segments without metadata
                    debug!(
                        "Segment {} has no metadata (old format)",
                        segment_desc.segment_id
                    );
                    vec![Value::Null; vector_count]
                };

                // Extract primary keys from payloads
                let primary_keys: Vec<String> = payloads
                    .iter()
                    .enumerate()
                    .map(|(i, payload)| extract_primary_key(payload, i))
                    .collect();

                // Convert vectors to QueryVector format
                let query_vectors: Vec<QueryVector> = vectors
                    .into_iter()
                    .map(|components| QueryVector { components })
                    .collect();

                // Add batch to index
                let batch = IndexBatch {
                    primary_keys,
                    vectors: query_vectors,
                    payloads,
                };

                index_provider.add_batch(&index_handle, batch).await?;
                total_vectors_loaded += vector_count;

                debug!(
                    "Loaded {} vectors from segment {}",
                    vector_count, segment_desc.segment_id
                );
            }
            Err(e) => {
                // If load_segment not implemented (e.g., MemoryStorageBackend),
                // skip this segment
                warn!(
                    "Failed to load segment {}: {} (continuing)",
                    segment_desc.segment_id, e
                );
            }
        }
    }

    info!(
        "Loaded collection '{}': {} vectors across {} segments",
        name,
        total_vectors_loaded,
        manifest.segments.len()
    );

    Ok(CollectionMetadata {
        descriptor: Arc::new(descriptor),
        manifest,
        index_handle: Some(index_handle),
    })
}

/// Bootstrap all collections from storage
///
/// This function is called on server startup to restore all collections
/// from S3 into the AppState.
pub async fn bootstrap_collections(state: &AppState) -> Result<()> {
    info!("🔄 Bootstrapping collections from storage...");

    // Discover collections
    let collection_names = match discover_collections(state.storage.as_ref()).await {
        Ok(names) => names,
        Err(e) => {
            warn!("Failed to discover collections: {}", e);
            return Ok(()); // Don't fail startup if discovery fails
        }
    };

    if collection_names.is_empty() {
        info!("No collections found in storage");
        return Ok(());
    }

    info!("Found {} collections to load", collection_names.len());

    // Load each collection
    let mut loaded_count = 0;
    let mut failed_count = 0;

    for name in collection_names {
        info!("Loading collection: {}", name);

        match load_collection(&name, state.storage.as_ref(), state.index_provider.as_ref()).await {
            Ok(metadata) => {
                // Register collection in state
                if let Err(e) = state
                    .register_collection(
                        name.clone(),
                        metadata.descriptor.clone(),
                        metadata.manifest.clone(),
                    )
                    .await
                {
                    error!("❌ Failed to register collection '{}': {}", name, e);
                    failed_count += 1;
                    continue;
                }

                // Update index handle
                if let Some(handle) = metadata.index_handle {
                    if let Err(e) = state.update_index_handle(&name, handle).await {
                        error!("❌ Failed to update index handle for '{}': {}", name, e);
                        failed_count += 1;
                        continue;
                    }
                }

                info!("✅ Loaded collection: {}", name);
                loaded_count += 1;
            }
            Err(e) => {
                error!("❌ Failed to load collection '{}': {}", name, e);
                failed_count += 1;
                // Continue loading other collections (fault-tolerant)
            }
        }
    }

    if failed_count > 0 {
        warn!(
            "Bootstrap complete: {} loaded, {} failed",
            loaded_count, failed_count
        );
    } else {
        info!("✅ Bootstrap complete: {} collections loaded", loaded_count);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use akidb_index::NativeIndexProvider;
    use akidb_query::{BasicQueryPlanner, SimpleExecutionEngine};
    use akidb_storage::MemoryStorageBackend;
    use std::sync::Arc;

    fn create_test_state() -> AppState {
        let storage = Arc::new(MemoryStorageBackend::new());
        let index_provider = Arc::new(NativeIndexProvider::new());
        let planner = Arc::new(BasicQueryPlanner::new());
        let engine = Arc::new(SimpleExecutionEngine::new(index_provider.clone()));

        AppState::new(storage, index_provider, planner, engine)
    }

    #[tokio::test]
    async fn test_discover_collections_empty() {
        let storage = MemoryStorageBackend::new();
        let collections = discover_collections(&storage).await.unwrap();
        assert_eq!(collections.len(), 0);
    }

    #[tokio::test]
    async fn test_extract_primary_key() {
        // Test with string ID
        let payload = serde_json::json!({"id": "test-123", "data": "foo"});
        assert_eq!(extract_primary_key(&payload, 0), "test-123");

        // Test with number ID
        let payload = serde_json::json!({"id": 42, "data": "bar"});
        assert_eq!(extract_primary_key(&payload, 0), "42");

        // Test without ID (fallback)
        let payload = serde_json::json!({"data": "baz"});
        assert_eq!(extract_primary_key(&payload, 5), "vector_5");

        // Test with null payload
        let payload = Value::Null;
        assert_eq!(extract_primary_key(&payload, 10), "vector_10");
    }

    #[tokio::test]
    async fn test_bootstrap_collections_empty_storage() {
        let state = create_test_state();
        // Should not fail even if storage is empty
        bootstrap_collections(&state).await.unwrap();
    }
}
