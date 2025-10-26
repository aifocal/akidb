//! Batch execution engine orchestrating parallel ANN queries.
//!
//! Provides a concurrency-aware wrapper over the core execution engine to
//! evaluate multiple search requests in parallel while enforcing batch limits.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use akidb_core::{Error, Result};
use akidb_index::types::{QueryVector, SearchOptions};
use akidb_storage::MetadataStore;
use futures::future::join_all;
use uuid::Uuid;

use crate::{
    context::{BatchQueryRequest, BatchQueryResponse, SingleQuery, SingleQueryResult},
    engine::ExecutionEngine,
    filter_parser::FilterParser,
    plan::{AnnSearchNode, PhysicalPlan, PlanNode, PlanNodeId},
    QueryContext,
};

const DEFAULT_MAX_BATCH_SIZE: usize = 100;

/// Execution engine capable of running multiple ANN queries in parallel.
pub struct BatchExecutionEngine {
    engine: Arc<dyn ExecutionEngine>,
    metadata_store: Arc<dyn MetadataStore>,
    max_batch_size: usize,
}

impl BatchExecutionEngine {
    /// Construct a new batch execution engine with default configuration.
    pub fn new(engine: Arc<dyn ExecutionEngine>, metadata_store: Arc<dyn MetadataStore>) -> Self {
        Self {
            engine,
            metadata_store,
            max_batch_size: DEFAULT_MAX_BATCH_SIZE,
        }
    }

    /// Construct a batch execution engine with a custom batch size limit.
    pub fn with_max_batch_size(
        engine: Arc<dyn ExecutionEngine>,
        metadata_store: Arc<dyn MetadataStore>,
        max_batch_size: usize,
    ) -> Self {
        Self {
            engine,
            metadata_store,
            max_batch_size,
        }
    }

    /// Execute a batch of ANN queries using the provided context and index handle.
    pub async fn execute_batch(
        &self,
        request: BatchQueryRequest,
        ctx: QueryContext,
        index_id: Uuid,
    ) -> Result<BatchQueryResponse> {
        if request.queries.is_empty() {
            return Err(Error::Validation(
                "batch query request must contain at least one query".to_string(),
            ));
        }

        if request.queries.len() > self.max_batch_size {
            return Err(Error::Validation(format!(
                "batch size {} exceeds configured maximum {}",
                request.queries.len(),
                self.max_batch_size
            )));
        }

        if request.collection != ctx.descriptor.name {
            return Err(Error::Validation(format!(
                "batch request collection '{}' does not match context '{}'",
                request.collection, ctx.descriptor.name
            )));
        }

        let BatchQueryRequest {
            collection,
            queries,
            timeout_ms,
        } = request;

        let futures = queries.into_iter().map(|query| {
            let ctx_clone = ctx.clone();
            let collection_clone = collection.clone();
            async move {
                self.execute_single(query, ctx_clone, timeout_ms, index_id, collection_clone)
                    .await
            }
        });

        let results = join_all(futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>>>()?;

        Ok(BatchQueryResponse {
            collection,
            results,
        })
    }

    /// Execute a single query within the batch and return its result.
    async fn execute_single(
        &self,
        query: SingleQuery,
        ctx: QueryContext,
        timeout_ms: u64,
        index_id: Uuid,
        collection: String,
    ) -> Result<SingleQueryResult> {
        let start = Instant::now();

        let SingleQuery {
            id,
            vector,
            top_k,
            filter,
        } = query;

        let filter_bitmap = if let Some(filter_json) = filter {
            let parser = FilterParser::with_collection(self.metadata_store.clone(), collection);
            let bitmap = parser.parse(&filter_json).await?;

            if bitmap.is_empty() {
                return Ok(SingleQueryResult {
                    id,
                    neighbors: Vec::new(),
                    latency_ms: start.elapsed().as_secs_f64() * 1000.0,
                });
            }

            Some(bitmap)
        } else {
            None
        };

        let options = SearchOptions {
            top_k,
            filter: filter_bitmap,
            timeout_ms,
        };

        let ann_node = PlanNode::AnnSearch(AnnSearchNode {
            index_handle: index_id,
            query: QueryVector { components: vector },
            options,
        });

        let mut nodes = HashMap::new();
        nodes.insert(PlanNodeId::default(), ann_node);

        let plan = PhysicalPlan {
            root: PlanNodeId::default(),
            nodes,
        };

        let response = self.engine.execute(plan, ctx).await?;

        Ok(SingleQueryResult {
            id,
            neighbors: response.results.neighbors,
            latency_ms: start.elapsed().as_secs_f64() * 1000.0,
        })
    }
}
