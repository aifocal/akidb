pub mod basic_planner;
pub mod batch_engine;
pub mod context;
pub mod engine;
pub mod filter_cache;
pub mod filter_parser;
pub mod plan;
pub mod planner;
pub mod simple_engine;

// Phase 3 M4: Production Monitoring
pub mod profiler;

// Phase 7 M7: Query Result Caching
pub mod cache;

// Phase 7 M8: Materialized Views
pub mod materialized_view;

// Phase 7 M9: Cache Invalidation
pub mod invalidation;

// Phase 7 M13-M15: Distributed Query Coordination
pub mod distributed;

pub use basic_planner::BasicQueryPlanner;
pub use batch_engine::BatchExecutionEngine;
pub use cache::{
    CacheConfig, CacheStats, CachedQueryResult, CachedSearchResult, QueryCache, QueryCacheKey,
};
pub use context::{
    BatchQueryRequest, BatchQueryResponse, QueryContext, QueryRequest, QueryResponse,
    SearchNeighbor, SingleQuery, SingleQueryResult,
};
pub use distributed::{
    CoordinatorConfig, CoordinatorStats, DistributedError, DistributedQueryRequest,
    DistributedQueryResponse, DistributedSearchResult, QueryCoordinator, ShardId, ShardInfo,
    ShardStatus, ShardingStrategy, VectorRange,
};
pub use engine::ExecutionEngine;
pub use filter_cache::FilterCache;
pub use filter_parser::{FilterParser, FilterTree};
pub use invalidation::{InvalidationConfig, InvalidationStats, InvalidationTracker};
pub use materialized_view::{
    AggregationType, MaterializedResult, MaterializedView, MaterializedViewManager,
    MaterializedViewType, RefreshStrategy, ViewDefinition, ViewError, ViewStats, ViewStatus,
};
pub use plan::{PhysicalPlan, PlanNode, PlanNodeId};
pub use planner::QueryPlanner;
pub use profiler::{ProfileStage, QueryProfile};
pub use simple_engine::SimpleExecutionEngine;
