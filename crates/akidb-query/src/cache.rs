use moka::future::Cache as MokaCache;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Type alias for key tracking: maps (tenant_id, collection) to set of cache keys
type KeyTrackingMap = Arc<RwLock<HashMap<(String, String), HashSet<String>>>>;

/// Query result cache with multi-level support
///
/// # Architecture
///
/// - L1 Cache (Memory): moka in-process cache for hot queries (< 1ms latency)
/// - L2 Cache (Redis): Distributed cache for shared results across instances (< 10ms latency)
///
/// # Cache Key Generation
///
/// Cache keys are SHA-256 hashes of:
/// - Collection name
/// - Query vector (normalized)
/// - K (number of results)
/// - Filters (sorted by key)
/// - Tenant ID
///
/// This ensures identical queries produce identical keys across instances.
///
/// # Key Tracking
///
/// To enable O(1) invalidation by (tenant_id, collection), we maintain a reverse index
/// mapping from (tenant_id, collection) -> Set<cache_key>. This allows targeted invalidation
/// without iterating over all cache entries.
#[derive(Clone)]
pub struct QueryCache {
    /// L1 in-memory cache (moka)
    memory_cache: Arc<MokaCache<String, CachedQueryResult>>,
    /// Key tracking: (tenant_id, collection) -> Set<cache_key>
    /// This allows O(1) targeted invalidation by tenant and collection
    key_tracking: KeyTrackingMap,
    /// Configuration
    config: CacheConfig,
}

impl QueryCache {
    /// Create a new query cache
    pub fn new(config: CacheConfig) -> Self {
        let memory_cache = MokaCache::builder()
            .max_capacity(config.memory_max_entries)
            .time_to_live(Duration::from_secs(config.memory_ttl_seconds))
            .build();

        Self {
            memory_cache: Arc::new(memory_cache),
            key_tracking: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Generate cache key for a query
    pub fn generate_key(&self, query: &QueryCacheKey) -> String {
        let mut hasher = Sha256::new();

        // Include all query parameters
        hasher.update(query.collection.as_bytes());
        hasher.update(query.tenant_id.as_bytes());

        // Normalize and hash query vector
        for &val in &query.query_vector {
            hasher.update(val.to_le_bytes());
        }

        hasher.update(query.k.to_le_bytes());

        // Sort filters by key for consistent hashing
        let mut sorted_filters = query.filters.clone();
        sorted_filters.sort_by(|a, b| a.0.cmp(&b.0));
        for (key, value) in sorted_filters {
            hasher.update(key.as_bytes());
            hasher.update(value.as_bytes());
        }

        format!("qc:{:x}", hasher.finalize())
    }

    /// Get cached query result from L1 (memory)
    pub async fn get(&self, key: &str) -> Option<CachedQueryResult> {
        self.memory_cache.get(key).await
    }

    /// Store query result in L1 cache
    ///
    /// Also registers the key in the tracking index for targeted invalidation
    pub async fn set(&self, key: String, query: &QueryCacheKey, result: CachedQueryResult) {
        // Register key in tracking map
        let tracking_key = (query.tenant_id.clone(), query.collection.clone());
        let mut tracking = self.key_tracking.write().await;
        tracking
            .entry(tracking_key)
            .or_insert_with(HashSet::new)
            .insert(key.clone());
        drop(tracking);

        // Store in cache
        self.memory_cache.insert(key, result).await;
    }

    /// Invalidate cache entry
    pub async fn invalidate(&self, key: &str) {
        self.memory_cache.invalidate(key).await;
    }

    /// Invalidate all cache entries for a specific tenant's collection
    ///
    /// Uses the key tracking index for O(1) lookup - only invalidates entries
    /// belonging to the specified tenant and collection, preventing cross-tenant
    /// cache poisoning.
    pub async fn invalidate_collection(&self, collection: &str, tenant_id: &str) {
        let tracking_key = (tenant_id.to_string(), collection.to_string());

        // Get all cache keys for this (tenant_id, collection) pair
        let keys_to_invalidate = {
            let mut tracking = self.key_tracking.write().await;
            tracking.remove(&tracking_key).unwrap_or_default()
        };

        // Invalidate each cache entry
        for key in keys_to_invalidate {
            self.memory_cache.invalidate(&key).await;
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            memory_entries: self.memory_cache.entry_count(),
            // Note: moka Cache no longer provides hit_count/miss_count in newer versions
            // Set to 0.0 as placeholder until alternative statistics API is available
            memory_hit_rate: 0.0,
        }
    }

    /// Get cache configuration
    pub fn config(&self) -> &CacheConfig {
        &self.config
    }
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Enable query result caching
    pub enabled: bool,

    /// L1 (memory) cache settings
    pub memory_max_entries: u64,
    pub memory_ttl_seconds: u64,

    /// L2 (Redis) cache settings (future)
    pub redis_enabled: bool,
    pub redis_url: Option<String>,
    pub redis_ttl_seconds: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            memory_max_entries: 10_000,
            memory_ttl_seconds: 300, // 5 minutes
            redis_enabled: false,
            redis_url: None,
            redis_ttl_seconds: 3600, // 1 hour
        }
    }
}

/// Query cache key components
#[derive(Debug, Clone)]
pub struct QueryCacheKey {
    pub tenant_id: String,
    pub collection: String,
    pub query_vector: Vec<f32>,
    pub k: usize,
    pub filters: Vec<(String, String)>,
}

impl QueryCacheKey {
    pub fn new(tenant_id: String, collection: String, query_vector: Vec<f32>, k: usize) -> Self {
        Self {
            tenant_id,
            collection,
            query_vector,
            k,
            filters: Vec::new(),
        }
    }

    pub fn with_filters(mut self, filters: Vec<(String, String)>) -> Self {
        self.filters = filters;
        self
    }
}

/// Cached query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedQueryResult {
    /// Search results (vector IDs and distances)
    pub results: Vec<CachedSearchResult>,
    /// Timestamp when cached
    pub cached_at: i64,
    /// Query latency (milliseconds)
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedSearchResult {
    pub id: String,
    pub distance: f32,
    pub metadata: Option<serde_json::Value>,
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub memory_entries: u64,
    pub memory_hit_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_basic_operations() {
        let config = CacheConfig::default();
        let cache = QueryCache::new(config);

        let query = QueryCacheKey::new(
            "tenant_1".to_string(),
            "test_collection".to_string(),
            vec![1.0, 2.0, 3.0],
            10,
        );

        let cache_key = cache.generate_key(&query);

        // Cache miss initially
        assert!(cache.get(&cache_key).await.is_none());

        // Store result
        let result = CachedQueryResult {
            results: vec![CachedSearchResult {
                id: "vec_1".to_string(),
                distance: 0.5,
                metadata: None,
            }],
            cached_at: chrono::Utc::now().timestamp(),
            latency_ms: 10,
        };

        cache.set(cache_key.clone(), &query, result.clone()).await;

        // Cache hit
        let cached = cache.get(&cache_key).await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().results.len(), 1);
    }

    #[tokio::test]
    async fn test_cache_key_generation_consistency() {
        let config = CacheConfig::default();
        let cache = QueryCache::new(config);

        let key1 = QueryCacheKey::new(
            "tenant_1".to_string(),
            "test_collection".to_string(),
            vec![1.0, 2.0, 3.0],
            10,
        );

        let key2 = QueryCacheKey::new(
            "tenant_1".to_string(),
            "test_collection".to_string(),
            vec![1.0, 2.0, 3.0],
            10,
        );

        // Same query should produce same cache key
        assert_eq!(cache.generate_key(&key1), cache.generate_key(&key2));
    }

    #[tokio::test]
    async fn test_cache_key_generation_different_params() {
        let config = CacheConfig::default();
        let cache = QueryCache::new(config);

        let key1 = QueryCacheKey::new(
            "tenant_1".to_string(),
            "test_collection".to_string(),
            vec![1.0, 2.0, 3.0],
            10,
        );

        let key2 = QueryCacheKey::new(
            "tenant_1".to_string(),
            "test_collection".to_string(),
            vec![1.0, 2.0, 3.0],
            20, // Different k
        );

        // Different k should produce different cache key
        assert_ne!(cache.generate_key(&key1), cache.generate_key(&key2));
    }

    #[tokio::test]
    async fn test_cache_invalidation() {
        let config = CacheConfig::default();
        let cache = QueryCache::new(config);

        let query = QueryCacheKey::new(
            "tenant_1".to_string(),
            "test_collection".to_string(),
            vec![1.0, 2.0, 3.0],
            10,
        );

        let cache_key = cache.generate_key(&query);

        let result = CachedQueryResult {
            results: vec![],
            cached_at: chrono::Utc::now().timestamp(),
            latency_ms: 10,
        };

        cache.set(cache_key.clone(), &query, result).await;
        assert!(cache.get(&cache_key).await.is_some());

        // Invalidate
        cache.invalidate(&cache_key).await;
        assert!(cache.get(&cache_key).await.is_none());
    }

    #[tokio::test]
    async fn test_cache_with_filters() {
        let config = CacheConfig::default();
        let cache = QueryCache::new(config);

        let key1 = QueryCacheKey::new(
            "tenant_1".to_string(),
            "test_collection".to_string(),
            vec![1.0, 2.0, 3.0],
            10,
        )
        .with_filters(vec![
            ("category".to_string(), "tech".to_string()),
            ("author".to_string(), "alice".to_string()),
        ]);

        let key2 = QueryCacheKey::new(
            "tenant_1".to_string(),
            "test_collection".to_string(),
            vec![1.0, 2.0, 3.0],
            10,
        )
        .with_filters(vec![
            ("author".to_string(), "alice".to_string()),
            ("category".to_string(), "tech".to_string()), // Same filters, different order
        ]);

        // Same filters in different order should produce same cache key
        assert_eq!(cache.generate_key(&key1), cache.generate_key(&key2));
    }

    #[tokio::test]
    async fn test_cache_tenant_isolation() {
        let config = CacheConfig::default();
        let cache = QueryCache::new(config);

        let key1 = QueryCacheKey::new(
            "tenant_1".to_string(),
            "test_collection".to_string(),
            vec![1.0, 2.0, 3.0],
            10,
        );

        let key2 = QueryCacheKey::new(
            "tenant_2".to_string(),
            "test_collection".to_string(),
            vec![1.0, 2.0, 3.0],
            10,
        );

        // Different tenants should produce different cache keys
        assert_ne!(cache.generate_key(&key1), cache.generate_key(&key2));
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let config = CacheConfig::default();
        let cache = QueryCache::new(config);

        let query = QueryCacheKey::new(
            "tenant_1".to_string(),
            "test_collection".to_string(),
            vec![1.0, 2.0, 3.0],
            10,
        );

        let cache_key = cache.generate_key(&query);
        let result = CachedQueryResult {
            results: vec![],
            cached_at: chrono::Utc::now().timestamp(),
            latency_ms: 10,
        };

        cache.set(cache_key.clone(), &query, result).await;

        // Wait for moka's background tasks to process the insertion
        // Moka uses async background tasks for cache admission
        cache.memory_cache.run_pending_tasks().await;

        let stats = cache.stats();
        assert_eq!(stats.memory_entries, 1);
    }

    #[tokio::test]
    async fn test_cache_collection_invalidation_tenant_isolation() {
        let config = CacheConfig::default();
        let cache = QueryCache::new(config);

        // Create cache entries for two different tenants with same collection name
        let query1 = QueryCacheKey::new(
            "tenant_1".to_string(),
            "products".to_string(),
            vec![1.0, 2.0, 3.0],
            10,
        );
        let query2 = QueryCacheKey::new(
            "tenant_2".to_string(),
            "products".to_string(),
            vec![1.0, 2.0, 3.0],
            10,
        );
        let query3 = QueryCacheKey::new(
            "tenant_1".to_string(),
            "orders".to_string(),
            vec![4.0, 5.0, 6.0],
            10,
        );

        let key1 = cache.generate_key(&query1);
        let key2 = cache.generate_key(&query2);
        let key3 = cache.generate_key(&query3);

        let result = CachedQueryResult {
            results: vec![],
            cached_at: chrono::Utc::now().timestamp(),
            latency_ms: 10,
        };

        // Store all three entries
        cache.set(key1.clone(), &query1, result.clone()).await;
        cache.set(key2.clone(), &query2, result.clone()).await;
        cache.set(key3.clone(), &query3, result.clone()).await;

        cache.memory_cache.run_pending_tasks().await;

        // Verify all three are cached
        assert!(cache.get(&key1).await.is_some());
        assert!(cache.get(&key2).await.is_some());
        assert!(cache.get(&key3).await.is_some());

        // Invalidate tenant_1's "products" collection
        cache.invalidate_collection("products", "tenant_1").await;

        // tenant_1/products should be invalidated
        assert!(cache.get(&key1).await.is_none());

        // tenant_2/products should remain (different tenant)
        assert!(cache.get(&key2).await.is_some());

        // tenant_1/orders should remain (different collection)
        assert!(cache.get(&key3).await.is_some());
    }
}
