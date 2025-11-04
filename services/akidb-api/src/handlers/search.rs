//! Vector search handlers

use crate::{handlers::collections::ApiError, state::AppState, validation};
use akidb_core::Error as CoreError;
use akidb_index::{QueryVector, SearchOptions};
use akidb_query::{
    plan::AnnSearchNode, BatchQueryRequest, BatchQueryResponse, FilterParser, PhysicalPlan,
    PlanNode, QueryContext,
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Search request
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchRequest {
    pub vector: Vec<f32>,
    #[serde(default = "default_top_k")]
    pub top_k: u16,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u32,
    pub filter: Option<serde_json::Value>,
}

fn default_top_k() -> u16 {
    10
}

fn default_timeout_ms() -> u32 {
    1000
}

/// Maximum number of queries allowed in a batch request.
const MAX_BATCH_SIZE: usize = 100;

/// Search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub score: f32,
    pub payload: serde_json::Value,
}

/// Search response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub collection: String,
    pub results: Vec<SearchResult>,
    pub count: usize,
}

/// Search for similar vectors
pub async fn search_vectors(
    State(state): State<AppState>,
    Path(collection_name): Path<String>,
    Json(req): Json<SearchRequest>,
) -> std::result::Result<Json<SearchResponse>, ApiError> {
    info!(
        "Searching in collection '{}' with top_k={}",
        collection_name, req.top_k
    );

    // Get collection metadata
    let metadata = state
        .get_collection(&collection_name)
        .await
        .map_err(|e| match e {
            CoreError::NotFound(_) => {
                ApiError::NotFound(format!("Collection '{}' not found", collection_name))
            }
            _ => ApiError::Internal(e),
        })?;

    // Validate request
    validation::validate_vector(&req.vector, metadata.descriptor.vector_dim as usize)?;
    validation::validate_top_k(req.top_k)?;

    // Get current epoch for cache key
    let current_epoch = metadata.epoch.load(std::sync::atomic::Ordering::SeqCst);

    // Check cache before executing query
    if let Some(cached_response) = state
        .query_cache
        .get(
            &collection_name,
            &req.vector,
            req.top_k,
            req.filter.as_ref(),
            current_epoch,
        )
        .await
    {
        debug!(
            "Cache hit for collection '{}', epoch={}",
            collection_name, current_epoch
        );
        return Ok(Json((*cached_response).clone()));
    }

    debug!(
        "Cache miss for collection '{}', epoch={}",
        collection_name, current_epoch
    );

    // Get index handle
    let index_handle = metadata.index_handle.as_ref().ok_or_else(|| {
        ApiError::NotFound(format!(
            "No index found for collection '{}'. Please insert vectors first.",
            collection_name
        ))
    })?;

    // Create physical plan directly (bypassing QueryPlanner)
    let query_vector = QueryVector {
        components: req.vector.clone(),
    };

    // Parse filter from JSON to RoaringBitmap if provided
    let filter_bitmap = if let Some(ref filter_json) = req.filter {
        debug!("Parsing filter: {:?}", filter_json);
        let parser = FilterParser::new(state.metadata_store.clone());
        // Parse and evaluate filter in one step
        match parser
            .parse_with_collection(filter_json, &collection_name)
            .await
        {
            Ok(bitmap) => {
                // Early return if filter matched 0 documents (no need to search)
                if bitmap.is_empty() {
                    info!(
                        "Filter matched 0 documents in collection '{}', returning empty result",
                        collection_name
                    );
                    return Ok(Json(SearchResponse {
                        collection: collection_name,
                        results: vec![],
                        count: 0,
                    }));
                }

                info!(
                    "Filter parsed successfully, matched {} documents",
                    bitmap.len()
                );
                Some(bitmap)
            }
            Err(e) => {
                warn!(
                    "Filter parse error for collection '{}': {}",
                    collection_name, e
                );
                return Err(ApiError::Validation(format!(
                    "Invalid filter syntax: {}. See documentation for supported filter format.",
                    e
                )));
            }
        }
    } else {
        None
    };

    let search_options = SearchOptions {
        top_k: req.top_k,
        filter: filter_bitmap,
        timeout_ms: req.timeout_ms as u64,
    };

    let search_node = PlanNode::AnnSearch(AnnSearchNode {
        index_handle: index_handle.index_id,
        query: query_vector,
        options: search_options,
    });

    let mut nodes = HashMap::new();
    nodes.insert(0, search_node);

    let plan = PhysicalPlan { root: 0, nodes };

    debug!("Created physical plan with {} nodes", plan.nodes.len());

    // Create query context
    let ctx = QueryContext {
        descriptor: metadata.descriptor.clone(),
        timeout: Duration::from_millis(req.timeout_ms as u64),
        span: tracing::Span::current(),
    };

    // Execute query
    let response = state
        .engine
        .execute(plan, ctx)
        .await
        .map_err(ApiError::Internal)?;

    // Convert to API response
    let results: Vec<SearchResult> = response
        .results
        .neighbors
        .into_iter()
        .map(|neighbor| SearchResult {
            id: neighbor.primary_key,
            score: neighbor.score,
            payload: neighbor.payload.unwrap_or(serde_json::Value::Null),
        })
        .collect();

    let count = results.len();

    info!(
        "Search completed in collection '{}', returned {} results",
        collection_name, count
    );

    let response = SearchResponse {
        collection: collection_name.clone(),
        results,
        count,
    };

    // Store in cache for future queries
    state
        .query_cache
        .put(
            &collection_name,
            &req.vector,
            req.top_k,
            req.filter.as_ref(),
            current_epoch,
            Arc::new(response.clone()),
        )
        .await;

    Ok(Json(response))
}

/// Execute a batch of vector searches using the shared batch engine.
pub async fn batch_search_vectors(
    State(state): State<AppState>,
    Path(collection_name): Path<String>,
    Json(mut req): Json<BatchQueryRequest>,
) -> std::result::Result<Json<BatchQueryResponse>, ApiError> {
    info!(
        "Batch search in collection '{}' with {} queries",
        collection_name,
        req.queries.len()
    );

    // Do not allow oversized batches from the API layer.
    if req.queries.len() > MAX_BATCH_SIZE {
        return Err(ApiError::Validation(format!(
            "Batch size {} exceeds maximum allowed size of {}",
            req.queries.len(),
            MAX_BATCH_SIZE
        )));
    }

    // Align collection name between path and payload for consistency.
    if req.collection.is_empty() {
        req.collection = collection_name.clone();
    } else if req.collection != collection_name {
        return Err(ApiError::Validation(format!(
            "Collection name mismatch between path '{}' and payload '{}'",
            collection_name, req.collection
        )));
    }

    // Ensure timeout is reasonable (non-zero) to avoid immediate cancellation.
    if req.timeout_ms == 0 {
        return Err(ApiError::Validation(
            "timeout_ms must be greater than 0 for batch queries".to_string(),
        ));
    }

    // Load collection metadata first so we can validate requests.
    let metadata = state
        .get_collection(&collection_name)
        .await
        .map_err(|e| match e {
            CoreError::NotFound(_) => {
                ApiError::NotFound(format!("Collection '{}' not found", collection_name))
            }
            other => ApiError::Internal(other),
        })?;

    // Batch request must contain at least one query.
    if req.queries.is_empty() {
        return Err(ApiError::Validation(
            "Batch search request must contain at least one query".to_string(),
        ));
    }

    // Validate each query vector payload prior to execution.
    // CRITICAL: Track query IDs to detect duplicates within the batch.
    // Duplicate IDs cause ambiguity in results - client cannot distinguish
    // which result corresponds to which query, leading to data corruption
    // if IDs are used as unique keys.
    let mut seen_ids = std::collections::HashSet::with_capacity(req.queries.len());

    for (idx, query) in req.queries.iter().enumerate() {
        if query.id.is_empty() {
            return Err(ApiError::Validation(format!(
                "Query {} must provide a non-empty id",
                idx
            )));
        }

        // Check for duplicate IDs within this batch
        if !seen_ids.insert(&query.id) {
            return Err(ApiError::Validation(format!(
                "Duplicate query id '{}' found at index {}. All query ids in a batch must be unique.",
                query.id, idx
            )));
        }

        validation::validate_vector(&query.vector, metadata.descriptor.vector_dim as usize)?;
        validation::validate_top_k(query.top_k)?;
    }

    // Ensure we have an index to search against.
    let index_handle = metadata.index_handle.as_ref().ok_or_else(|| {
        ApiError::NotFound(format!(
            "No index found for collection '{}'. Please insert vectors first.",
            collection_name
        ))
    })?;

    let ctx = QueryContext {
        descriptor: metadata.descriptor.clone(),
        timeout: Duration::from_millis(req.timeout_ms),
        span: tracing::Span::current(),
    };

    let response = state
        .batch_engine
        .execute_batch(req, ctx, index_handle.index_id)
        .await
        .map_err(|err| match err {
            akidb_core::Error::Validation(msg) => ApiError::Validation(msg),
            akidb_core::Error::NotFound(msg) => ApiError::NotFound(msg),
            other => ApiError::Internal(other),
        })?;

    Ok(Json(response))
}
