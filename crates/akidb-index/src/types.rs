use roaring::RoaringBitmap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use akidb_core::{collection::DistanceMetric, segment::SegmentDescriptor};

/// Describes the concrete index provider capability.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum IndexKind {
    Faiss,
    Hnsw,
    Native,
}

/// Handle referencing a persisted index artifact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexHandle {
    pub index_id: Uuid,
    pub kind: IndexKind,
    pub dimension: u16,
    pub collection: String,
}

/// Request to build a new ANN index from a set of segments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildRequest {
    pub collection: String,
    pub kind: IndexKind,
    pub distance: DistanceMetric,
    pub dimension: u16,
    pub segments: Vec<SegmentDescriptor>,
}

/// Batch of vectors to add to an index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexBatch {
    pub primary_keys: Vec<String>,
    pub vectors: Vec<QueryVector>,
    pub payloads: Vec<Value>,
}

/// Query vector wrapper enforcing typed semantics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryVector {
    pub components: Vec<f32>,
}

/// Options controlling ANN search execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchOptions {
    pub top_k: u16,
    pub filter: Option<RoaringBitmap>,
    pub timeout_ms: u64,
}

/// Scored query result set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub query: QueryVector,
    pub neighbors: Vec<ScoredPoint>,
}

/// Single scored neighbor returned by an index lookup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredPoint {
    pub primary_key: String,
    pub score: f32,
    pub payload: Option<Value>,
}
