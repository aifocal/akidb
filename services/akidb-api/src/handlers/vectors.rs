//! Vector insertion handlers

use crate::{handlers::collections::ApiError, state::AppState, validation};
use akidb_core::segment::{SegmentDescriptor, SegmentState};
use akidb_index::{BuildRequest, IndexBatch, QueryVector};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
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

    // Create segment for tracking
    let segment_id = Uuid::new_v4();

    // Validate vector count fits in u32 (segment size limit)
    let record_count = u32::try_from(req.vectors.len())
        .map_err(|_| ApiError::Validation(format!(
            "Vector count {} exceeds maximum segment size of {} (u32::MAX)",
            req.vectors.len(),
            u32::MAX
        )))?;

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

    // TODO: Write to WAL
    // For now, we'll skip WAL and write directly to index
    debug!("Skipping WAL write for now, writing directly to index");

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

    // Add vectors to index
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

        // Create metadata block from payloads
        let metadata = akidb_storage::MetadataBlock::from_json(payloads).map_err(|e| {
            ApiError::Internal(akidb_core::Error::Storage(format!(
                "Failed to create metadata: {}",
                e
            )))
        })?;

        // Persist to S3
        state
            .storage
            .write_segment_with_data(&segment_descriptor, vectors, Some(metadata))
            .await
            .map_err(ApiError::Internal)?;

        info!(
            "Persisted {} vectors with metadata to S3, segment {}",
            req.vectors.len(),
            segment_id
        );
    }

    info!(
        "Successfully inserted {} vectors into collection '{}', segment {}",
        req.vectors.len(),
        collection_name,
        segment_id
    );

    Ok(Json(InsertVectorsResponse {
        inserted: req.vectors.len(),
        segment_id,
    }))
}
