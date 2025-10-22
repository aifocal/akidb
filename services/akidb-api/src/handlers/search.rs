//! Vector search handlers

use crate::{handlers::collections::ApiError, state::AppState, validation};
use akidb_core::Error as CoreError;
use akidb_index::{QueryVector, SearchOptions};
use akidb_query::{plan::AnnSearchNode, PhysicalPlan, PlanNode, QueryContext};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, info};

/// Search request
#[derive(Debug, Deserialize)]
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

/// Search result
#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub id: String,
    pub score: f32,
    pub payload: serde_json::Value,
}

/// Search response
#[derive(Debug, Serialize)]
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

    // Get index handle
    let index_handle = metadata.index_handle.as_ref().ok_or_else(|| {
        ApiError::NotFound(format!(
            "No index found for collection '{}'. Please insert vectors first.",
            collection_name
        ))
    })?;

    // Create physical plan directly (bypassing QueryPlanner)
    let query_vector = QueryVector {
        components: req.vector,
    };

    // TODO: Parse filter from JSON to RoaringBitmap
    let search_options = SearchOptions {
        top_k: req.top_k,
        filter: None, // Not yet implemented
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

    Ok(Json(SearchResponse {
        collection: collection_name,
        results,
        count,
    }))
}
