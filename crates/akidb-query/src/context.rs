use std::sync::Arc;
use std::time::Duration;

use akidb_core::collection::CollectionDescriptor;
use akidb_index::types::{QueryVector, ScoredPoint, SearchResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::Span;

/// Query payload provided by clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequest {
    pub collection: String,
    pub vector: QueryVector,
    pub top_k: u16,
    pub filter: Option<Value>,
    pub timeout_ms: u64,
}

/// Execution context propagated across planner and executor stages.
#[derive(Debug, Clone)]
pub struct QueryContext {
    pub descriptor: Arc<CollectionDescriptor>,
    pub timeout: Duration,
    pub span: Span,
}

/// Structured response returned to API layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResponse {
    pub collection: String,
    pub top_k: u16,
    pub results: SearchResult,
}

/// Convenience alias exposing scored neighbor details.
pub type SearchNeighbor = ScoredPoint;

/// Batch query request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchQueryRequest {
    pub collection: String,
    pub queries: Vec<SingleQuery>,
    pub timeout_ms: u64,
}

/// Single query payload inside a batch request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleQuery {
    pub id: String,
    pub vector: Vec<f32>,
    pub top_k: u16,
    #[serde(default)]
    pub filter: Option<Value>,
}

/// Batch query response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchQueryResponse {
    pub collection: String,
    pub results: Vec<SingleQueryResult>,
}

/// Result produced for an individual batched query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleQueryResult {
    pub id: String,
    pub neighbors: Vec<SearchNeighbor>,
    pub latency_ms: f64,
}
