//! Simple execution engine implementation
//!
//! This engine executes physical plans by coordinating with index providers
//! and storage backends to perform vector searches.

use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, info, span, warn, Level};

use akidb_core::{Error, Result};
use akidb_index::IndexProvider;

use crate::{
    context::{QueryContext, QueryResponse},
    engine::ExecutionEngine,
    plan::{PhysicalPlan, PlanNode},
};

/// Simple execution engine that executes plans sequentially
pub struct SimpleExecutionEngine {
    /// Index provider for ANN search
    index_provider: Arc<dyn IndexProvider>,
}

impl SimpleExecutionEngine {
    /// Create a new simple execution engine
    pub fn new(index_provider: Arc<dyn IndexProvider>) -> Self {
        Self { index_provider }
    }

    /// Execute a single plan node
    async fn execute_node(
        &self,
        node: &PlanNode,
        _ctx: &QueryContext,
    ) -> Result<akidb_index::SearchResult> {
        match node {
            PlanNode::AnnSearch(ann_node) => {
                debug!(
                    "Executing ANN search on index {} with top_k={}",
                    ann_node.index_handle, ann_node.options.top_k
                );

                // Create index handle
                let handle = akidb_index::IndexHandle {
                    index_id: ann_node.index_handle,
                    kind: self.index_provider.kind(),
                    dimension: ann_node.query.components.len() as u16,
                    collection: "unknown".to_string(), // TODO: Get from context
                };

                // Perform search
                let result = self
                    .index_provider
                    .search(&handle, ann_node.query.clone(), ann_node.options.clone())
                    .await?;

                debug!("ANN search returned {} results", result.neighbors.len());

                Ok(result)
            }
            PlanNode::Filter(_) => {
                // TODO: Implement filter execution
                warn!("Filter node execution not yet implemented");
                Err(Error::NotImplemented(
                    "Filter node execution".to_string(),
                ))
            }
            PlanNode::Merge(_) => {
                // TODO: Implement merge execution
                warn!("Merge node execution not yet implemented");
                Err(Error::NotImplemented("Merge node execution".to_string()))
            }
        }
    }
}

#[async_trait]
impl ExecutionEngine for SimpleExecutionEngine {
    async fn execute(&self, plan: PhysicalPlan, ctx: QueryContext) -> Result<QueryResponse> {
        let _span = span!(Level::INFO, "execute_query", collection = %ctx.descriptor.name);

        info!("Executing plan with {} nodes", plan.nodes.len());

        // Get root node
        let root_node = plan
            .nodes
            .get(&plan.root)
            .ok_or_else(|| Error::Internal("Root node not found in plan".to_string()))?;

        // Execute root node (for now, we only support single-node plans)
        let search_result = self.execute_node(root_node, &ctx).await?;

        // Convert to QueryResponse
        let response = QueryResponse {
            collection: ctx.descriptor.name.clone(),
            top_k: search_result.neighbors.len() as u16,
            results: search_result,
        };

        info!(
            "Query execution completed, returned {} results",
            response.top_k
        );

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use akidb_core::collection::{CollectionDescriptor, DistanceMetric, PayloadSchema};
    use akidb_index::{BuildRequest, IndexBatch, IndexKind, NativeIndexProvider, QueryVector};
    use serde_json::json;
    use std::time::Duration;

    #[tokio::test]
    async fn test_simple_engine_execution() {
        // Create native index provider
        let provider = Arc::new(NativeIndexProvider::new());

        // Build an index
        let build_request = BuildRequest {
            collection: "test".to_string(),
            kind: IndexKind::Native,
            distance: DistanceMetric::Cosine,
            segments: vec![akidb_core::segment::SegmentDescriptor {
                segment_id: uuid::Uuid::new_v4(),
                collection: "test".to_string(),
                vector_dim: 3,
                record_count: 0,
                state: akidb_core::segment::SegmentState::Active,
                lsn_range: 0..=0,
                compression_level: 0,
                created_at: chrono::Utc::now(),
            }],
        };

        let handle = provider.build(build_request).await.unwrap();

        // Add some test data
        let batch = IndexBatch {
            primary_keys: vec!["key1".to_string(), "key2".to_string(), "key3".to_string()],
            vectors: vec![
                QueryVector {
                    components: vec![1.0, 0.0, 0.0],
                },
                QueryVector {
                    components: vec![0.0, 1.0, 0.0],
                },
                QueryVector {
                    components: vec![0.0, 0.0, 1.0],
                },
            ],
            payloads: vec![json!({"id": 1}), json!({"id": 2}), json!({"id": 3})],
        };

        provider.add_batch(&handle, batch).await.unwrap();

        // Create execution engine
        let engine = SimpleExecutionEngine::new(provider);

        // Create a simple plan
        use crate::plan::{AnnSearchNode, PhysicalPlan, PlanNode};
        use akidb_index::SearchOptions;
        use std::collections::HashMap;

        let query = QueryVector {
            components: vec![1.0, 0.1, 0.0],
        };

        let search_node = PlanNode::AnnSearch(AnnSearchNode {
            index_handle: handle.index_id,
            query,
            options: SearchOptions {
                top_k: 2,
                filter: None,
                timeout_ms: 1000,
            },
        });

        let mut nodes = HashMap::new();
        nodes.insert(0, search_node);

        let plan = PhysicalPlan { root: 0, nodes };

        // Create query context
        let descriptor = Arc::new(CollectionDescriptor {
            name: "test".to_string(),
            vector_dim: 3,
            distance: DistanceMetric::Cosine,
            replication: 1,
            shard_count: 1,
            payload_schema: PayloadSchema::default(),
        });

        let ctx = QueryContext {
            descriptor,
            timeout: Duration::from_secs(1),
            span: tracing::Span::none(),
        };

        // Execute query
        let response = engine.execute(plan, ctx).await.unwrap();

        // Verify results
        assert_eq!(response.collection, "test");
        assert_eq!(response.top_k, 2);
        assert_eq!(response.results.neighbors.len(), 2);
        assert_eq!(response.results.neighbors[0].primary_key, "key1"); // Closest to [1,0.1,0]
    }
}
