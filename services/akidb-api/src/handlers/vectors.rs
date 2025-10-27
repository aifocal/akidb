//! Vector insertion handlers

use crate::{handlers::collections::ApiError, state::AppState, validation};
use akidb_core::segment::{SegmentDescriptor, SegmentState};
use akidb_index::{BuildRequest, IndexBatch, QueryVector};
use akidb_storage::{WalAppender, WalRecord};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use tracing::{debug, info};
use uuid::Uuid;

/// Request to insert vectors
#[derive(Debug, Deserialize)]
pub struct InsertVectorsRequest {
    pub vectors: Vec<VectorInput>,
}

/// A single vector with metadata
#[derive(Debug, Deserialize)]
pub struct VectorInput {
    pub id: String,
    pub vector: Vec<f32>,
    #[serde(default)]
    pub payload: serde_json::Value,
}

/// Response for vector insertion
#[derive(Debug, Serialize)]
pub struct InsertVectorsResponse {
    pub inserted: usize,
    pub segment_id: Uuid,
}

/// Insert vectors into a collection
pub async fn insert_vectors(
    State(state): State<AppState>,
    Path(collection_name): Path<String>,
    Json(req): Json<InsertVectorsRequest>,
) -> std::result::Result<Json<InsertVectorsResponse>, ApiError> {
    info!(
        "Inserting {} vectors into collection: {}",
        req.vectors.len(),
        collection_name
    );

    // Validate request
    if req.vectors.is_empty() {
        return Err(ApiError::Validation(
            "Cannot insert empty vector list".to_string(),
        ));
    }

    // Get collection metadata
    let metadata = state
        .get_collection(&collection_name)
        .await
        .map_err(|e| match e {
            akidb_core::Error::NotFound(_) => {
                ApiError::NotFound(format!("Collection '{}' not found", collection_name))
            }
            _ => ApiError::Internal(e),
        })?;

    // Validate vectors
    for (idx, vec_input) in req.vectors.iter().enumerate() {
        validation::validate_vector(&vec_input.vector, metadata.descriptor.vector_dim as usize)
            .map_err(|e| match e {
                ApiError::Validation(msg) => {
                    ApiError::Validation(format!("Vector at index {}: {}", idx, msg))
                }
                other => other,
            })?;
    }

    let batch_len = req.vectors.len();

    // Create segment for tracking
    let segment_id = Uuid::new_v4();

    // Validate vector count fits in u32 (segment size limit)
    let record_count = u32::try_from(batch_len).map_err(|_| {
        ApiError::Validation(format!(
            "Vector count {} exceeds maximum segment size of {} (u32::MAX)",
            batch_len,
            u32::MAX
        ))
    })?;

    // Reserve doc_id range for this batch and detect overflow
    let start_doc_id = metadata
        .next_doc_id
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
            current.checked_add(record_count)
        })
        .map_err(|_| {
            ApiError::Validation(
                "Collection has reached maximum document ID capacity (u32::MAX)".to_string(),
            )
        })?;

    let segment_descriptor = SegmentDescriptor {
        segment_id,
        collection: collection_name.clone(),
        vector_dim: metadata.descriptor.vector_dim,
        record_count,
        state: SegmentState::Active,
        lsn_range: 0..=0,
        compression_level: 0,
        created_at: chrono::Utc::now(),
    };

    // Write to WAL before modifying any state (durability guarantee)
    info!(
        "Writing {} vectors to WAL for collection '{}'",
        batch_len, collection_name
    );

    for vec_input in &req.vectors {
        let wal_record = WalRecord::Insert {
            collection: collection_name.clone(),
            primary_key: vec_input.id.clone(),
            vector: vec_input.vector.clone(),
            payload: vec_input.payload.clone(),
        };

        state
            .wal
            .append(metadata.wal_stream_id, wal_record)
            .await
            .map_err(|e| {
                ApiError::Internal(akidb_core::Error::Storage(format!(
                    "WAL append failed: {}",
                    e
                )))
            })?;
    }

    debug!(
        "WAL write complete for {} vectors in collection '{}'",
        batch_len, collection_name
    );

    // Build or update index
    let index_handle = if let Some(handle) = &metadata.index_handle {
        // Use existing index
        handle.clone()
    } else {
        // Build new index
        info!("Building new index for collection: {}", collection_name);

        let build_request = BuildRequest {
            collection: collection_name.clone(),
            kind: state.index_provider.kind(),
            distance: metadata.descriptor.distance,
            dimension: metadata.descriptor.vector_dim,
            segments: vec![segment_descriptor.clone()],
        };

        let handle = state
            .index_provider
            .build(build_request)
            .await
            .map_err(ApiError::Internal)?;

        // Update state
        state
            .update_index_handle(&collection_name, handle.clone())
            .await
            .map_err(ApiError::Internal)?;

        debug!("Created new index with ID: {}", handle.index_id);
        handle
    };

    // Bump collection epoch BEFORE adding vectors to index to prevent cache poisoning
    // Race condition: If we bump AFTER add_batch(), concurrent searches could:
    // 1. Read old epoch N
    // 2. Find newly added vectors (from add_batch)
    // 3. Cache results with old epoch N
    // Result: Cache contains new data with old epoch key (cache poisoning)
    state
        .bump_collection_epoch(&collection_name)
        .await
        .map_err(ApiError::Internal)?;

    // Add vectors to index (vectors become searchable)
    let batch = IndexBatch {
        primary_keys: req.vectors.iter().map(|v| v.id.clone()).collect(),
        vectors: req
            .vectors
            .iter()
            .map(|v| QueryVector {
                components: v.vector.clone(),
            })
            .collect(),
        payloads: req.vectors.iter().map(|v| v.payload.clone()).collect(),
    };

    state
        .index_provider
        .add_batch(&index_handle, batch)
        .await
        .map_err(ApiError::Internal)?;

    // Index metadata for filter queries using globally reserved doc_id range
    for (idx, vec_input) in req.vectors.iter().enumerate() {
        // Safety: idx < req.vectors.len() which was validated as <= u32::MAX at line 86
        let offset = u32::try_from(idx).expect("idx within u32 range due to validation at line 86");
        let doc_id = start_doc_id + offset;
        if let Err(e) = state
            .metadata_store
            .index_metadata(&collection_name, doc_id, &vec_input.payload)
            .await
        {
            debug!(
                "Failed to index metadata for doc_id {}: {} (continuing)",
                doc_id, e
            );
        }
    }

    // Extract vectors and payloads for persistence to S3
    let (vectors, payloads) = state
        .index_provider
        .extract_for_persistence(&index_handle)
        .map_err(ApiError::Internal)?;

    // Only persist if we have data
    if !vectors.is_empty() {
        debug!(
            "Extracted {} vectors and {} payloads for persistence",
            vectors.len(),
            payloads.len()
        );

        let start_index = usize::try_from(start_doc_id).map_err(|_| {
            ApiError::Internal(akidb_core::Error::Storage(
                "Failed to convert doc_id to index for persistence".to_string(),
            ))
        })?;
        let expected_len = batch_len;
        let new_vectors: Vec<Vec<f32>> = vectors
            .into_iter()
            .skip(start_index)
            .take(expected_len)
            .collect();
        let new_payloads: Vec<serde_json::Value> = payloads
            .into_iter()
            .skip(start_index)
            .take(expected_len)
            .collect();

        if new_vectors.len() != expected_len || new_payloads.len() != expected_len {
            return Err(ApiError::Internal(akidb_core::Error::Storage(
                "Persisted batch size mismatch - index returned inconsistent vector count"
                    .to_string(),
            )));
        }

        // Create metadata block from payloads
        let metadata = akidb_storage::MetadataBlock::from_json(new_payloads).map_err(|e| {
            ApiError::Internal(akidb_core::Error::Storage(format!(
                "Failed to create metadata: {}",
                e
            )))
        })?;

        // Persist to S3
        state
            .storage
            .write_segment_with_data(&segment_descriptor, new_vectors, Some(metadata))
            .await
            .map_err(ApiError::Internal)?;

        info!(
            "Persisted {} vectors with metadata to S3, segment {}",
            batch_len, segment_id
        );
    }

    // Note: Epoch was already bumped before add_batch() to prevent cache poisoning
    info!(
        "Successfully inserted {} vectors into collection '{}', segment {}",
        batch_len, collection_name, segment_id
    );

    Ok(Json(InsertVectorsResponse {
        inserted: batch_len,
        segment_id,
    }))
}
