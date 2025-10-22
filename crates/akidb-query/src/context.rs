use std::sync::Arc;
use std::time::Duration;

use akidb_core::collection::CollectionDescriptor;
use akidb_index::types::{QueryVector, SearchResult};
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
