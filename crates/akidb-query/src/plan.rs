use std::collections::HashMap;

use roaring::RoaringBitmap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use akidb_index::types::{QueryVector, SearchOptions};

pub type PlanNodeId = u32;

/// Logical + physical execution plan produced by planner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicalPlan {
    pub root: PlanNodeId,
    pub nodes: HashMap<PlanNodeId, PlanNode>,
}

/// Node kinds composing a query execution plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlanNode {
    AnnSearch(AnnSearchNode),
    Filter(FilterNode),
    Merge(MergeNode),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnSearchNode {
    pub index_handle: Uuid,
    pub query: QueryVector,
    pub options: SearchOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterNode {
    pub input: PlanNodeId,
    pub bitmap: RoaringBitmap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeNode {
    pub left: PlanNodeId,
    pub right: PlanNodeId,
}
