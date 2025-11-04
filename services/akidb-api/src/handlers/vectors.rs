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

/// Maximum number of vectors allowed in a single insert request.
/// This prevents DoS attacks via oversized payloads.
/// Limit: 10,000 vectors × ~2KB each = ~20MB payload (reasonable for single request)
const MAX_INSERT_BATCH_SIZE: usize = 10_000;

/// Maximum size of a single payload in bytes (100KB).
/// This prevents DoS attacks via individual oversized JSON payloads.
/// Rationale: Most metadata payloads should be < 10KB. 100KB provides generous headroom.
const MAX_PAYLOAD_SIZE_BYTES: usize = 100 * 1024; // 100KB

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

    // Prevent DoS via oversized batch
    if req.vectors.len() > MAX_INSERT_BATCH_SIZE {
        return Err(ApiError::Validation(format!(
            "Batch size {} exceeds maximum allowed {} vectors per request. Please split into smaller batches.",
            req.vectors.len(),
            MAX_INSERT_BATCH_SIZE
        )));
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

    // Validate payload sizes to prevent DoS via oversized JSON payloads
    // IMPORTANT: Even if batch size is within limits, individual payloads could be enormous
    // (e.g., 10K vectors with 10MB payloads each = 100GB request). This check prevents
    // memory exhaustion attacks.
    for (idx, vec_input) in req.vectors.iter().enumerate() {
        let payload_str = serde_json::to_string(&vec_input.payload).map_err(|e| {
            ApiError::Validation(format!(
                "Vector at index {}: Failed to serialize payload: {}",
                idx, e
            ))
        })?;
        let payload_size = payload_str.len();

        if payload_size > MAX_PAYLOAD_SIZE_BYTES {
            return Err(ApiError::Validation(format!(
                "Vector at index {}: Payload size {} bytes exceeds maximum allowed {} bytes. \
                 Please reduce payload size or split across multiple documents.",
                idx, payload_size, MAX_PAYLOAD_SIZE_BYTES
            )));
        }
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
    //
    // IMPORTANT: WAL Idempotency and Partial Failure Recovery
    //
    // This function performs multiple state-modifying operations after WAL sync:
    // 1. WAL sync (durable - point of no return)
    // 2. Add vectors to index
    // 3. Index metadata
    // 4. Bump epoch
    // 5. Persist to S3
    //
    // If any operation fails after WAL sync, we have two recovery mechanisms:
    //
    // 1. Immediate: Client receives error and can retry with same vector IDs
    //    - Retry should be idempotent (replace existing vectors with same IDs)
    //    - Current implementation: add_batch may add duplicates (depends on index provider)
    //    - TODO: Implement idempotent insert (upsert semantics)
    //
    // 2. Crash recovery: WAL replay will re-execute all operations
    //    - WAL replay should be idempotent
    //    - Current implementation: May create duplicate vectors on replay
    //    - TODO: Track completed WAL records to skip replayed duplicates
    //
    // Partial failure scenarios:
    // - add_batch fails: WAL has vectors, index doesn't → WAL replay will retry
    // - index_metadata fails: Vectors searchable but not filterable → Client retry needed
    // - bump_epoch fails: Vectors searchable, cache not invalidated → Degraded but functional
    // - persist to S3 fails: Vectors searchable, not durable in S3 → WAL replay will persist
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

    // Sync WAL to storage to ensure durability before proceeding
    // CRITICAL: Without sync(), records stay in memory and are lost on crash
    state.wal.sync(metadata.wal_stream_id).await.map_err(|e| {
        ApiError::Internal(akidb_core::Error::Storage(format!(
            "WAL sync failed: {}",
            e
        )))
    })?;

    debug!(
        "WAL write complete and synced for {} vectors in collection '{}'",
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

        // Update state (returns existing handle if set by concurrent thread)
        // CRITICAL: This prevents TOCTOU race where multiple threads build duplicate indices.
        // If another thread already set the handle, we use theirs and discard ours.
        let actual_handle = state
            .update_index_handle(&collection_name, handle.clone())
            .await
            .map_err(ApiError::Internal)?;

        if actual_handle.index_id != handle.index_id {
            debug!(
                "Concurrent index build detected for collection '{}'. Using existing index {} instead of newly built {}",
                collection_name, actual_handle.index_id, handle.index_id
            );
        } else {
            debug!("Created new index with ID: {}", handle.index_id);
        }

        actual_handle
    };

    // CRITICAL FIX (Bug #46): Index metadata BEFORE adding vectors to prevent race condition
    //
    // ISSUE: Previous code added vectors to index first (line 281), making them searchable via ANN,
    // then indexed metadata (line 293). During this window:
    // - Concurrent filtered search finds vector via ANN (searchable)
    // - Tries to apply filter using metadata (not indexed yet)
    // - Drops vector due to missing metadata (visible correctness bug)
    //
    // FIX: Reverse order - index metadata first, THEN add to ANN index.
    // This ensures vectors are only searchable after metadata is ready for filtering.
    // Worst case during race window: vector is filterable but not in ANN yet (miss, not incorrect result).

    // 1. Index metadata FIRST for filter queries using globally reserved doc_id range
    // IMPORTANT: Metadata indexing failures must propagate as errors to ensure data consistency.
    for (idx, vec_input) in req.vectors.iter().enumerate() {
        // Safety: idx < req.vectors.len() which was validated as <= u32::MAX at line 86
        let offset = u32::try_from(idx).expect("idx within u32 range due to validation at line 86");
        let doc_id = start_doc_id + offset;
        state
            .metadata_store
            .index_metadata(&collection_name, doc_id, &vec_input.payload)
            .await
            .map_err(|e| {
                ApiError::Internal(akidb_core::Error::Storage(format!(
                    "Failed to index metadata for doc_id {}: {}. Aborting insert to maintain consistency.",
                    doc_id, e
                )))
            })?;
    }

    // 2. Add vectors to ANN index AFTER metadata is indexed (vectors become searchable)
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

    // Bump collection epoch AFTER add_batch and metadata indexing to minimize inconsistency window
    //
    // DESIGN DECISION: Epoch bump placement trade-offs
    //
    // Option A - Bump BEFORE add_batch (previous implementation):
    //   Pro: Prevents cache poisoning race (cache invalidated before vectors searchable)
    //   Con: If add_batch fails, epoch already bumped → unnecessary cache invalidation overhead
    //   Con: If metadata indexing fails, epoch bumped but vectors not fully indexed
    //
    // Option B - Bump AFTER add_batch and metadata indexing (current implementation):
    //   Pro: If add_batch fails, no state change (epoch not bumped, no cache overhead)
    //   Pro: If metadata indexing fails, error propagates before epoch bump
    //   Pro: Only bump epoch if vectors are fully indexed and searchable
    //   Con: Small cache poisoning window (concurrent searches could cache stale results)
    //   Mitigation: Cache TTL will eventually expire stale entries; correctness not affected
    //
    // Rationale: Option B chosen because:
    // 1. Minimizes unnecessary cache invalidation (performance)
    // 2. Ensures epoch only bumped on successful insert (consistency)
    // 3. Cache poisoning is temporary and doesn't affect correctness (acceptable trade-off)
    // 4. If bump_epoch fails, vectors are still searchable (degraded but functional)
    state
        .bump_collection_epoch(&collection_name)
        .await
        .map_err(ApiError::Internal)?;

    // Persist vectors to S3 directly from request payload
    // IMPORTANT: We persist from req.vectors instead of extracting from index to avoid
    // concurrent insert issues. The index's internal vector positions are non-deterministic
    // under concurrent writes, so using skip(start_doc_id) would fail when a later-reserved
    // batch completes add_batch() first. By persisting the original request data, we ensure
    // correctness regardless of concurrent execution order.
    if !req.vectors.is_empty() {
        debug!(
            "Persisting {} vectors to S3 for segment {}",
            req.vectors.len(),
            segment_id
        );

        // Extract vectors and payloads directly from request
        let new_vectors: Vec<Vec<f32>> = req.vectors.iter().map(|v| v.vector.clone()).collect();
        let new_payloads: Vec<serde_json::Value> =
            req.vectors.iter().map(|v| v.payload.clone()).collect();

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
