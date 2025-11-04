//! Collection bootstrap and recovery from storage
//!
//! This module handles loading collections from S3 on server startup,
//! enabling restart recovery.

use crate::state::{AppState, CollectionMetadata};
use akidb_core::{collection::CollectionDescriptor, Error, Result};
use akidb_index::{BuildRequest, IndexBatch, IndexProvider, QueryVector};
use akidb_storage::{StorageBackend, WalStreamId};
use serde_json::Value;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
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
/// 4. Replays uncommitted WAL records
/// 5. Returns CollectionMetadata ready for registration
async fn load_collection(
    name: &str,
    storage: &dyn StorageBackend,
    index_provider: &dyn IndexProvider,
    metadata_store: &dyn akidb_storage::MetadataStore,
    wal: &dyn akidb_storage::WalReplayer,
) -> Result<CollectionMetadata> {
    info!("Loading collection: {}", name);

    // 1. Load manifest
    let manifest = storage.load_manifest(name).await?;
    debug!(
        "Loaded manifest for '{}': {} vectors, {} segments, dimension: {}",
        name,
        manifest.total_vectors,
        manifest.segments.len(),
        manifest.dimension
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
        dimension: descriptor.vector_dim,
        segments: manifest.segments.clone(),
    };

    let index_handle = index_provider.build(build_request).await?;
    debug!(
        "Created index {} for collection '{}'",
        index_handle.index_id, name
    );

    // 4. Load segments and populate index
    let mut total_vectors_loaded = 0;
    let mut global_vector_index: usize = 0; // Track global index across all segments

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

                // Extract primary keys from payloads using GLOBAL index
                // This ensures unique fallback keys across all segments
                let primary_keys: Vec<String> = payloads
                    .iter()
                    .enumerate()
                    .map(|(i, payload)| extract_primary_key(payload, global_vector_index + i))
                    .collect();

                // Save doc_id start for metadata indexing (before incrementing global_vector_index)
                let doc_id_start = global_vector_index as u32;
                global_vector_index += vector_count;

                // Convert vectors to QueryVector format
                let query_vectors: Vec<QueryVector> = vectors
                    .into_iter()
                    .map(|components| QueryVector { components })
                    .collect();

                // Add batch to index
                let batch = IndexBatch {
                    primary_keys,
                    vectors: query_vectors,
                    payloads: payloads.clone(),
                };

                index_provider.add_batch(&index_handle, batch).await?;

                // Rehydrate metadata store for filter queries
                // IMPORTANT: Without this, filters are blind to persisted segments after restart.
                // The in-memory metadata store only contains WAL-replayed data by default.
                for (idx, payload) in payloads.iter().enumerate() {
                    let doc_id = doc_id_start + idx as u32;
                    metadata_store.index_metadata(name, doc_id, payload).await?;
                }

                total_vectors_loaded += vector_count;

                debug!(
                    "Loaded {} vectors from segment {} (metadata indexed for filter queries)",
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

    // Load persisted WAL stream ID or create new one for backward compatibility
    let wal_stream_id = descriptor
        .wal_stream_id
        .map(WalStreamId::from_uuid)
        .unwrap_or_default();

    debug!("Using WAL stream ID for '{}': {}", name, wal_stream_id.0);

    // 5. WAL Replay - Replay any uncommitted WAL records after loading persisted segments
    debug!("Starting WAL replay for collection '{}'", name);

    let replay_stats = wal.replay(wal_stream_id, None).await?;

    if replay_stats.records > 0 {
        info!(
            "Found {} uncommitted WAL records for '{}' ({} bytes)",
            replay_stats.records, name, replay_stats.bytes
        );

        // Fetch WAL entries batch by batch in a loop
        let max_batch_bytes = 10 * 1024 * 1024; // 10MB per batch
        let mut last_lsn: Option<akidb_storage::LogSequence> = None;
        let mut total_replayed = 0;

        // Maintain primary_key → doc_id mapping for Delete/Upsert operations
        let mut key_to_doc_id: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();

        loop {
            // Fetch next batch starting from last processed LSN
            let wal_entries_bytes = wal
                .next_batch(wal_stream_id, max_batch_bytes, last_lsn)
                .await?;

            if wal_entries_bytes.is_empty() {
                debug!("No more WAL entries to replay for '{}'", name);
                break;
            }

            // Parse and convert WAL entries to IndexBatch
            let mut batch_vectors = Vec::new();
            let mut batch_payloads = Vec::new();
            let mut batch_keys = Vec::new();

            for entry_bytes in wal_entries_bytes {
                // Parse WAL entry with error handling
                let entry: akidb_storage::WalEntry = match serde_json::from_slice(&entry_bytes) {
                    Ok(e) => e,
                    Err(e) => {
                        // Log warning and skip corrupted entry instead of failing
                        warn!("Skipping corrupted WAL entry for '{}': {}", name, e);
                        continue;
                    }
                };

                // Track last processed LSN
                last_lsn = Some(entry.lsn);

                match entry.record {
                    akidb_storage::WalRecord::Insert {
                        primary_key,
                        vector,
                        payload,
                        ..
                    } => {
                        // Validate vector dimension
                        if vector.len() != descriptor.vector_dim as usize {
                            warn!(
                                "Skipping WAL entry with dimension mismatch: expected {}, got {} (key: {})",
                                descriptor.vector_dim,
                                vector.len(),
                                primary_key
                            );
                            continue;
                        }

                        batch_keys.push(primary_key);
                        batch_vectors.push(QueryVector { components: vector });
                        batch_payloads.push(payload);
                    }
                    akidb_storage::WalRecord::Delete { primary_key, .. } => {
                        // IMPORTANT: Flush pending inserts before processing delete to maintain
                        // correct ordering. If we buffer deletes and inserts together, a sequence
                        // like [Insert(A), Delete(A), Insert(A)] could replay incorrectly.
                        if !batch_vectors.is_empty() {
                            let batch_count = batch_vectors.len();
                            let doc_id_start = global_vector_index as u32;

                            let batch = IndexBatch {
                                primary_keys: batch_keys.clone(),
                                vectors: batch_vectors.clone(),
                                payloads: batch_payloads.clone(),
                            };

                            // Add to index
                            index_provider.add_batch(&index_handle, batch).await?;

                            // Index metadata and track key → doc_id mapping
                            for (idx, (key, payload)) in
                                batch_keys.iter().zip(batch_payloads.iter()).enumerate()
                            {
                                let doc_id = doc_id_start + idx as u32;
                                metadata_store.index_metadata(name, doc_id, payload).await?;
                                key_to_doc_id.insert(key.clone(), doc_id);
                            }

                            global_vector_index += batch_count;
                            total_replayed += batch_count;

                            // Clear batch buffers
                            batch_keys.clear();
                            batch_vectors.clear();
                            batch_payloads.clear();

                            debug!("Flushed {} vectors before Delete operation", batch_count);
                        }

                        // Process delete operation
                        if let Some(&doc_id) = key_to_doc_id.get(&primary_key) {
                            // Remove from index
                            index_provider
                                .remove(&index_handle, &[primary_key.clone()])
                                .await?;

                            // Remove from metadata store
                            metadata_store.remove_metadata(name, doc_id).await?;

                            // Remove from mapping
                            key_to_doc_id.remove(&primary_key);

                            debug!(
                                "WAL replay: Deleted vector with key '{}' (doc_id: {})",
                                primary_key, doc_id
                            );
                        } else {
                            warn!(
                                "WAL replay: Cannot delete key '{}' - not found in collection '{}'",
                                primary_key, name
                            );
                        }
                    }
                    akidb_storage::WalRecord::UpsertPayload { primary_key, payload, .. } => {
                        // IMPORTANT: Flush pending inserts before upsert (same reasoning as Delete)
                        if !batch_vectors.is_empty() {
                            let batch_count = batch_vectors.len();
                            let doc_id_start = global_vector_index as u32;

                            let batch = IndexBatch {
                                primary_keys: batch_keys.clone(),
                                vectors: batch_vectors.clone(),
                                payloads: batch_payloads.clone(),
                            };

                            // Add to index
                            index_provider.add_batch(&index_handle, batch).await?;

                            // Index metadata and track key → doc_id mapping
                            for (idx, (key, pl)) in batch_keys.iter().zip(batch_payloads.iter()).enumerate() {
                                let doc_id = doc_id_start + idx as u32;
                                metadata_store.index_metadata(name, doc_id, pl).await?;
                                key_to_doc_id.insert(key.clone(), doc_id);
                            }

                            global_vector_index += batch_count;
                            total_replayed += batch_count;

                            // Clear batch buffers
                            batch_keys.clear();
                            batch_vectors.clear();
                            batch_payloads.clear();

                            debug!("Flushed {} vectors before UpsertPayload operation", batch_count);
                        }

                        // Process upsert operation (update metadata only, vector stays same)
                        if let Some(&doc_id) = key_to_doc_id.get(&primary_key) {
                            // Update metadata store with new payload
                            metadata_store.insert_metadata(name, doc_id, &payload).await?;

                            debug!(
                                "WAL replay: Updated payload for key '{}' (doc_id: {})",
                                primary_key, doc_id
                            );
                        } else {
                            warn!(
                                "WAL replay: Cannot upsert key '{}' - not found in collection '{}'. \
                                 This may indicate the vector was deleted or never inserted.",
                                primary_key, name
                            );
                        }
                    }
                }
            }

            // Add batch to index and metadata store if we have remaining vectors
            if !batch_vectors.is_empty() {
                let batch_count = batch_vectors.len();
                let doc_id_start = global_vector_index as u32;

                let batch = IndexBatch {
                    primary_keys: batch_keys.clone(),
                    vectors: batch_vectors,
                    payloads: batch_payloads.clone(),
                };

                // Add to index
                index_provider.add_batch(&index_handle, batch).await?;

                // Index metadata and track key → doc_id mapping
                for (idx, (key, payload)) in batch_keys.iter().zip(batch_payloads.iter()).enumerate() {
                    let doc_id = doc_id_start + idx as u32;
                    metadata_store.index_metadata(name, doc_id, payload).await?;
                    key_to_doc_id.insert(key.clone(), doc_id);
                }

                global_vector_index += batch_count;
                total_replayed += batch_count;

                debug!(
                    "Replayed batch of {} WAL records for '{}' (total: {})",
                    batch_count, name, total_replayed
                );
            }
        }

        info!(
            "Successfully replayed {} WAL records into index for collection '{}'",
            total_replayed, name
        );
    } else {
        debug!("No uncommitted WAL records for '{}'", name);
    }

    // Calculate final next_doc_id accounting for both segments and WAL records
    let manifest_total = usize::try_from(manifest.total_vectors).unwrap_or(usize::MAX);
    let doc_count = std::cmp::max(global_vector_index, manifest_total);
    let next_doc_id = u32::try_from(doc_count).map_err(|_| {
        Error::Validation(format!(
            "Collection '{}' exceeds supported document capacity (u32::MAX)",
            name
        ))
    })?;

    Ok(CollectionMetadata {
        descriptor: Arc::new(descriptor),
        manifest,
        index_handle: Some(index_handle),
        next_doc_id: Arc::new(AtomicU32::new(next_doc_id)),
        wal_stream_id,
        epoch: Arc::new(AtomicU64::new(0)),
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

        match load_collection(
            &name,
            state.storage.as_ref(),
            state.index_provider.as_ref(),
            state.metadata_store.as_ref(),
            state.wal.as_ref(),
        )
        .await
        {
            Ok(metadata) => {
                // Register collection in state
                if let Err(e) = state
                    .register_collection(
                        name.clone(),
                        metadata.descriptor.clone(),
                        metadata.manifest.clone(),
                        metadata.next_doc_id.load(Ordering::SeqCst),
                        metadata.wal_stream_id,
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
    use akidb_query::{
        BasicQueryPlanner, BatchExecutionEngine, ExecutionEngine, QueryPlanner,
        SimpleExecutionEngine,
    };
    use akidb_storage::{MemoryMetadataStore, MemoryStorageBackend, S3WalBackend};
    use std::sync::Arc;

    fn create_test_state() -> AppState {
        let storage = Arc::new(MemoryStorageBackend::new());
        let index_provider = Arc::new(NativeIndexProvider::new());
        let planner: Arc<dyn QueryPlanner> = Arc::new(BasicQueryPlanner::new());
        let engine: Arc<dyn ExecutionEngine> =
            Arc::new(SimpleExecutionEngine::new(index_provider.clone()));
        let metadata_store: Arc<dyn akidb_storage::MetadataStore> =
            Arc::new(MemoryMetadataStore::new());
        let batch_engine = Arc::new(BatchExecutionEngine::new(
            Arc::clone(&engine),
            Arc::clone(&metadata_store),
        ));
        let wal = Arc::new(S3WalBackend::new_unchecked(storage.clone()));
        let query_cache = Arc::new(crate::query_cache::QueryCache::default());

        AppState::new(
            storage,
            index_provider,
            planner,
            engine,
            batch_engine,
            metadata_store,
            wal,
            query_cache,
        )
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
