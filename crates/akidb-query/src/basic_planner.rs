//! Basic query planner implementation
//!
//! This planner generates simple physical plans for vector search queries.
//! It creates a single AnnSearch node for basic queries.

use std::collections::HashMap;
use tracing::{debug, info};
use uuid::Uuid;

use akidb_core::Result;
use akidb_index::types::SearchOptions;

use crate::{
    context::QueryRequest,
    plan::{AnnSearchNode, PhysicalPlan, PlanNode, PlanNodeId},
    planner::QueryPlanner,
};

/// Basic query planner that generates simple search plans
pub struct BasicQueryPlanner {
    /// Default index handle to use (if available)
    default_index: Option<Uuid>,
}

impl BasicQueryPlanner {
    /// Create a new basic query planner
    pub fn new() -> Self {
        Self {
            default_index: None,
        }
    }

    /// Create planner with a default index handle
    pub fn with_default_index(index_id: Uuid) -> Self {
        Self {
            default_index: Some(index_id),
        }
    }

    /// Set the default index handle
    pub fn set_default_index(&mut self, index_id: Uuid) {
        self.default_index = Some(index_id);
    }
}

impl Default for BasicQueryPlanner {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryPlanner for BasicQueryPlanner {
    fn plan(&self, request: &QueryRequest) -> Result<PhysicalPlan> {
        info!("Planning query for collection: {}", request.collection);

        // Get index handle (use default or generate one)
        let index_handle = self.default_index.unwrap_or_else(|| {
            // In a real implementation, this would look up the index from metadata
            debug!("No default index set, generating placeholder");
            Uuid::new_v4()
        });

        // Create search options
        let options = SearchOptions {
            top_k: request.top_k,
            filter: None, // TODO: Parse and convert filter from request
            timeout_ms: request.timeout_ms,
        };

        // Create ANN search node
        let search_node = PlanNode::AnnSearch(AnnSearchNode {
            index_handle,
            query: request.vector.clone(),
            options,
        });

        // Build plan with single node
        let root_id: PlanNodeId = 0;
        let mut nodes = HashMap::new();
        nodes.insert(root_id, search_node);

        let plan = PhysicalPlan {
            root: root_id,
            nodes,
        };

        debug!("Generated plan with {} nodes", plan.nodes.len());

        Ok(plan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use akidb_index::QueryVector;

    #[test]
    fn test_basic_planner_creation() {
        let planner = BasicQueryPlanner::new();
        assert!(planner.default_index.is_none());

        let index_id = Uuid::new_v4();
        let planner = BasicQueryPlanner::with_default_index(index_id);
        assert_eq!(planner.default_index, Some(index_id));
    }

    #[test]
    fn test_plan_generation() {
        let planner = BasicQueryPlanner::new();

        let request = QueryRequest {
            collection: "test".to_string(),
            vector: QueryVector {
                components: vec![1.0, 2.0, 3.0],
            },
            top_k: 10,
            filter: None,
            timeout_ms: 1000,
        };

        let plan = planner.plan(&request).unwrap();

        // Should have one node
        assert_eq!(plan.nodes.len(), 1);
        assert_eq!(plan.root, 0);

        // Check the node is an AnnSearch
        match plan.nodes.get(&0) {
            Some(PlanNode::AnnSearch(node)) => {
                assert_eq!(node.query.components, vec![1.0, 2.0, 3.0]);
                assert_eq!(node.options.top_k, 10);
                assert_eq!(node.options.timeout_ms, 1000);
            }
            _ => panic!("Expected AnnSearch node"),
        }
    }

    #[test]
    fn test_plan_with_default_index() {
        let index_id = Uuid::new_v4();
        let planner = BasicQueryPlanner::with_default_index(index_id);

        let request = QueryRequest {
            collection: "test".to_string(),
            vector: QueryVector {
                components: vec![1.0, 2.0, 3.0],
            },
            top_k: 5,
            filter: None,
            timeout_ms: 500,
        };

        let plan = planner.plan(&request).unwrap();

        // Check it uses the default index
        match plan.nodes.get(&0) {
            Some(PlanNode::AnnSearch(node)) => {
                assert_eq!(node.index_handle, index_id);
            }
            _ => panic!("Expected AnnSearch node"),
        }
    }
}
