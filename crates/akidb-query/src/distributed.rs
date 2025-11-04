use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Distributed query coordinator for sharded collections
///
/// # Overview
///
/// For billion-scale datasets, a single machine cannot hold all data in memory.
/// The distributed query system partitions (shards) collections across multiple
/// nodes and coordinates queries across shards.
///
/// # Architecture
///
/// ```text
/// ┌─────────────────────────────────────────────┐
/// │         Query Coordinator                   │
/// ├─────────────────────────────────────────────┤
/// │  1. Receive query from client              │
/// │  2. Determine relevant shards               │
/// │  3. Send sub-queries to shard nodes         │
/// │  4. Aggregate results                       │
/// │  5. Return top-k to client                  │
/// └─────────────────────────────────────────────┘
///                      │
///       ┌──────────────┼──────────────┐
///       ▼              ▼              ▼
///  ┌─────────┐   ┌─────────┐   ┌─────────┐
///  │ Shard 0 │   │ Shard 1 │   │ Shard 2 │
///  │ (node1) │   │ (node2) │   │ (node3) │
///  └─────────┘   └─────────┘   └─────────┘
/// ```
///
/// # Sharding Strategies
///
/// - **Hash**: Deterministic assignment based on vector ID hash
/// - **Range**: Vectors partitioned by ID ranges
/// - **Random**: Random assignment for load balancing
#[derive(Clone)]
pub struct QueryCoordinator {
    /// Shard registry
    shards: Arc<RwLock<HashMap<ShardId, ShardInfo>>>,
    /// Sharding strategy
    strategy: ShardingStrategy,
    /// Configuration
    config: CoordinatorConfig,
}

impl QueryCoordinator {
    /// Create a new query coordinator
    pub fn new(strategy: ShardingStrategy, config: CoordinatorConfig) -> Self {
        Self {
            shards: Arc::new(RwLock::new(HashMap::new())),
            strategy,
            config,
        }
    }

    /// Register a shard
    pub async fn register_shard(&self, shard: ShardInfo) -> Result<(), DistributedError> {
        let mut shards = self.shards.write().await;

        if shards.contains_key(&shard.shard_id) {
            return Err(DistributedError::ShardAlreadyExists(shard.shard_id));
        }

        shards.insert(shard.shard_id, shard);
        Ok(())
    }

    /// Execute distributed query
    pub async fn query(
        &self,
        request: DistributedQueryRequest,
    ) -> Result<DistributedQueryResponse, DistributedError> {
        // Get relevant shards
        let shard_ids = self.get_relevant_shards(&request).await?;

        if shard_ids.is_empty() {
            return Err(DistributedError::NoShards);
        }

        // CRITICAL FIX (Bug #40, #41, #42): Multiple distributed coordinator bugs fixed:
        //
        // Bug #40 (Compilation Error): shard_ids was consumed in the loop, then accessed
        // via shard_ids.len(). Fixed by using &shard_ids to borrow instead of consume.
        //
        // Bug #41 (Offline Shards Queried): Queries were sent to all shards regardless of
        // ShardStatus. Fixed by filtering for Active shards only.
        //
        // Bug #42 (Sequential Execution): Despite max_concurrent_queries config, queries
        // ran sequentially. Fixed by using tokio::spawn with concurrent execution.

        // Filter for active shards only
        let active_shard_ids = {
            let shards = self.shards.read().await;
            shard_ids
                .iter()
                .filter(|id| {
                    shards
                        .get(id)
                        .map(|s| s.status == ShardStatus::Active)
                        .unwrap_or(false)
                })
                .copied()
                .collect::<Vec<_>>()
        };

        if active_shard_ids.is_empty() {
            return Err(DistributedError::NoShards);
        }

        // Send sub-queries to active shards in parallel (respecting concurrency limit)
        // Note: Use semaphore to honor max_concurrent_queries config
        let semaphore = Arc::new(tokio::sync::Semaphore::new(
            self.config.max_concurrent_queries,
        ));

        let mut handles = Vec::new();
        for shard_id in &active_shard_ids {
            let shard_id = *shard_id;
            let request = request.clone();
            let coordinator = self.clone();
            let permit = semaphore.clone();

            let handle = tokio::spawn(async move {
                let _permit = permit.acquire().await.ok()?;
                coordinator.query_shard(shard_id, &request).await.ok()
            });

            handles.push(handle);
        }

        // Collect results from all shards
        let mut results = Vec::new();
        for handle in handles {
            if let Ok(Some(shard_result)) = handle.await {
                results.extend(shard_result.results);
            }
        }

        // Aggregate results: merge-sort by distance and take top-k
        results.sort_by(|a, b| {
            a.distance
                .partial_cmp(&b.distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let top_k = results.into_iter().take(request.k).collect();

        Ok(DistributedQueryResponse {
            results: top_k,
            shards_queried: active_shard_ids.len(),
        })
    }

    /// Get relevant shards for a query
    async fn get_relevant_shards(
        &self,
        _request: &DistributedQueryRequest,
    ) -> Result<Vec<ShardId>, DistributedError> {
        let shards = self.shards.read().await;

        // For now, query all shards (full fan-out)
        // In production, use partition pruning based on filters
        let shard_ids: Vec<ShardId> = shards.keys().cloned().collect();

        Ok(shard_ids)
    }

    /// Query a single shard
    async fn query_shard(
        &self,
        shard_id: ShardId,
        _request: &DistributedQueryRequest,
    ) -> Result<ShardQueryResponse, DistributedError> {
        let shards = self.shards.read().await;

        let _shard = shards
            .get(&shard_id)
            .ok_or(DistributedError::ShardNotFound(shard_id))?;

        // In production, this would make an HTTP/gRPC call to the shard node
        // For now, return mock results
        Ok(ShardQueryResponse {
            shard_id,
            results: vec![],
        })
    }

    /// Get shard assignment for a vector ID
    pub async fn get_shard_for_vector(&self, vector_id: &str) -> Result<ShardId, DistributedError> {
        let shards = self.shards.read().await;
        let shard_count = shards.len();

        if shard_count == 0 {
            return Err(DistributedError::NoShards);
        }

        match &self.strategy {
            ShardingStrategy::Hash => {
                // Hash-based sharding
                let hash = Self::hash_string(vector_id);
                let shard_idx = (hash % shard_count as u64) as usize;

                // IMPORTANT: Sort shard IDs to ensure stable, deterministic assignment
                // HashMap iteration order is arbitrary and can change between calls,
                // which would cause vectors to be reassigned to different shards.
                let mut shard_ids: Vec<ShardId> = shards.keys().copied().collect();
                shard_ids.sort_unstable();

                // Safe indexing - shard_idx is guaranteed to be < shard_count
                let shard_id = shard_ids[shard_idx];
                Ok(shard_id)
            }
            ShardingStrategy::Range { ranges } => {
                // Range-based sharding
                for (shard_id, range) in ranges {
                    if vector_id >= range.start.as_str() && vector_id < range.end.as_str() {
                        return Ok(*shard_id);
                    }
                }
                Err(DistributedError::NoMatchingShard)
            }
            ShardingStrategy::Random => {
                // Random sharding (load balancing)
                let shard_idx = rand::random::<usize>() % shard_count;

                // Map random index to actual shard ID (same as Hash strategy)
                let mut shard_ids: Vec<ShardId> = shards.keys().copied().collect();
                shard_ids.sort_unstable();

                let shard_id = shard_ids[shard_idx];
                Ok(shard_id)
            }
        }
    }

    /// Simple string hash
    fn hash_string(s: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    }

    /// Get statistics
    pub async fn stats(&self) -> CoordinatorStats {
        let shards = self.shards.read().await;

        CoordinatorStats {
            total_shards: shards.len(),
            active_shards: shards
                .values()
                .filter(|s| s.status == ShardStatus::Active)
                .count(),
        }
    }
}

/// Shard ID (numeric identifier)
pub type ShardId = usize;

/// Shard information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardInfo {
    /// Shard ID
    pub shard_id: ShardId,
    /// Node address (e.g., "http://node1:8080")
    pub node_address: String,
    /// Shard status
    pub status: ShardStatus,
    /// Number of vectors in this shard
    pub vector_count: u64,
    /// Shard metadata
    pub metadata: HashMap<String, String>,
}

/// Shard status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShardStatus {
    /// Shard is active and can serve queries
    Active,
    /// Shard is being initialized
    Initializing,
    /// Shard is temporarily offline
    Offline,
}

/// Sharding strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShardingStrategy {
    /// Hash-based sharding (consistent hashing)
    Hash,
    /// Range-based sharding (by vector ID)
    Range {
        ranges: HashMap<ShardId, VectorRange>,
    },
    /// Random sharding (for load balancing)
    Random,
}

/// Vector ID range for range-based sharding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorRange {
    pub start: String,
    pub end: String,
}

/// Coordinator configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinatorConfig {
    /// Maximum concurrent shard queries
    pub max_concurrent_queries: usize,
    /// Query timeout (milliseconds)
    pub query_timeout_ms: u64,
    /// Enable query result caching
    pub enable_caching: bool,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_queries: 100,
            query_timeout_ms: 5000,
            enable_caching: true,
        }
    }
}

/// Distributed query request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedQueryRequest {
    pub collection: String,
    pub tenant_id: String,
    pub query_vector: Vec<f32>,
    pub k: usize,
    pub filters: HashMap<String, String>,
}

/// Distributed query response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedQueryResponse {
    pub results: Vec<DistributedSearchResult>,
    pub shards_queried: usize,
}

/// Search result from distributed query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedSearchResult {
    pub vector_id: String,
    pub distance: f32,
    pub shard_id: ShardId,
    pub metadata: Option<serde_json::Value>,
}

/// Shard query response
#[derive(Debug, Clone)]
struct ShardQueryResponse {
    #[allow(dead_code)]
    shard_id: ShardId,
    results: Vec<DistributedSearchResult>,
}

/// Coordinator statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinatorStats {
    pub total_shards: usize,
    pub active_shards: usize,
}

/// Distributed system errors
#[derive(Debug, Clone, thiserror::Error)]
pub enum DistributedError {
    #[error("Shard already exists: {0}")]
    ShardAlreadyExists(ShardId),

    #[error("Shard not found: {0}")]
    ShardNotFound(ShardId),

    #[error("No shards available")]
    NoShards,

    #[error("No matching shard for vector")]
    NoMatchingShard,

    #[error("Query timeout")]
    Timeout,

    #[error("Network error: {0}")]
    NetworkError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_coordinator_creation() {
        let config = CoordinatorConfig::default();
        let coordinator = QueryCoordinator::new(ShardingStrategy::Hash, config);

        let stats = coordinator.stats().await;
        assert_eq!(stats.total_shards, 0);
    }

    #[tokio::test]
    async fn test_shard_registration() {
        let config = CoordinatorConfig::default();
        let coordinator = QueryCoordinator::new(ShardingStrategy::Hash, config);

        let shard = ShardInfo {
            shard_id: 0,
            node_address: "http://node1:8080".to_string(),
            status: ShardStatus::Active,
            vector_count: 1000,
            metadata: HashMap::new(),
        };

        coordinator.register_shard(shard).await.unwrap();

        let stats = coordinator.stats().await;
        assert_eq!(stats.total_shards, 1);
        assert_eq!(stats.active_shards, 1);
    }

    #[tokio::test]
    async fn test_hash_sharding() {
        let config = CoordinatorConfig::default();
        let coordinator = QueryCoordinator::new(ShardingStrategy::Hash, config);

        // Register 3 shards
        for i in 0..3 {
            let shard = ShardInfo {
                shard_id: i,
                node_address: format!("http://node{}:8080", i),
                status: ShardStatus::Active,
                vector_count: 1000,
                metadata: HashMap::new(),
            };
            coordinator.register_shard(shard).await.unwrap();
        }

        // Test hash-based assignment
        let shard1 = coordinator.get_shard_for_vector("vec_123").await.unwrap();
        let shard2 = coordinator.get_shard_for_vector("vec_123").await.unwrap();

        // Same vector ID should always map to same shard
        assert_eq!(shard1, shard2);

        // Verify shard is valid
        assert!(shard1 < 3);
    }

    #[tokio::test]
    async fn test_range_sharding() {
        let mut ranges = HashMap::new();
        ranges.insert(
            0,
            VectorRange {
                start: "vec_0".to_string(),
                end: "vec_500".to_string(),
            },
        );
        ranges.insert(
            1,
            VectorRange {
                start: "vec_500".to_string(),
                end: "vec_999".to_string(),
            },
        );

        let config = CoordinatorConfig::default();
        let coordinator = QueryCoordinator::new(ShardingStrategy::Range { ranges }, config);

        // Register shards
        for i in 0..2 {
            let shard = ShardInfo {
                shard_id: i,
                node_address: format!("http://node{}:8080", i),
                status: ShardStatus::Active,
                vector_count: 500,
                metadata: HashMap::new(),
            };
            coordinator.register_shard(shard).await.unwrap();
        }

        // Test range-based assignment
        let shard1 = coordinator.get_shard_for_vector("vec_100").await.unwrap();
        let shard2 = coordinator.get_shard_for_vector("vec_600").await.unwrap();

        assert_eq!(shard1, 0);
        assert_eq!(shard2, 1);
    }

    #[tokio::test]
    async fn test_distributed_query() {
        let config = CoordinatorConfig::default();
        let coordinator = QueryCoordinator::new(ShardingStrategy::Hash, config);

        // Register shards
        for i in 0..3 {
            let shard = ShardInfo {
                shard_id: i,
                node_address: format!("http://node{}:8080", i),
                status: ShardStatus::Active,
                vector_count: 1000,
                metadata: HashMap::new(),
            };
            coordinator.register_shard(shard).await.unwrap();
        }

        let request = DistributedQueryRequest {
            collection: "test_collection".to_string(),
            tenant_id: "tenant_1".to_string(),
            query_vector: vec![1.0; 128],
            k: 10,
            filters: HashMap::new(),
        };

        let response = coordinator.query(request).await.unwrap();

        assert_eq!(response.shards_queried, 3); // All shards queried
    }

    #[tokio::test]
    async fn test_duplicate_shard_registration() {
        let config = CoordinatorConfig::default();
        let coordinator = QueryCoordinator::new(ShardingStrategy::Hash, config);

        let shard = ShardInfo {
            shard_id: 0,
            node_address: "http://node1:8080".to_string(),
            status: ShardStatus::Active,
            vector_count: 1000,
            metadata: HashMap::new(),
        };

        coordinator.register_shard(shard.clone()).await.unwrap();

        // Second registration should fail
        let result = coordinator.register_shard(shard).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_shard_status_tracking() {
        let config = CoordinatorConfig::default();
        let coordinator = QueryCoordinator::new(ShardingStrategy::Hash, config);

        let active_shard = ShardInfo {
            shard_id: 0,
            node_address: "http://node1:8080".to_string(),
            status: ShardStatus::Active,
            vector_count: 1000,
            metadata: HashMap::new(),
        };

        let offline_shard = ShardInfo {
            shard_id: 1,
            node_address: "http://node2:8080".to_string(),
            status: ShardStatus::Offline,
            vector_count: 1000,
            metadata: HashMap::new(),
        };

        coordinator.register_shard(active_shard).await.unwrap();
        coordinator.register_shard(offline_shard).await.unwrap();

        let stats = coordinator.stats().await;
        assert_eq!(stats.total_shards, 2);
        assert_eq!(stats.active_shards, 1);
    }
}
