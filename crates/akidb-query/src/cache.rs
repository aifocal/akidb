use moka::future::Cache as MokaCache;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;

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
#[derive(Clone)]
pub struct QueryCache {
    /// L1 in-memory cache (moka)
    memory_cache: Arc<MokaCache<String, CachedQueryResult>>,
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
            config,
        }
    }

    /// Generate cache key for a query
    pub fn generate_key(&self, query: &QueryCacheKey) -> String {
        let mut hasher = Sha256::new();

        // Include all query parameters
        hasher.update(query.collection.as_bytes());
        hasher.update(&query.tenant_id.as_bytes());

        // Normalize and hash query vector
        for &val in &query.query_vector {
            hasher.update(&val.to_le_bytes());
        }

        hasher.update(&query.k.to_le_bytes());

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
    pub async fn set(&self, key: String, result: CachedQueryResult) {
        self.memory_cache.insert(key, result).await;
    }

    /// Invalidate cache entry
    pub async fn invalidate(&self, key: &str) {
        self.memory_cache.invalidate(key).await;
    }

    /// Invalidate all cache entries for a collection
    pub async fn invalidate_collection(&self, _collection: &str, _tenant_id: &str) {
        // moka doesn't support prefix-based invalidation, so we need to iterate
        // This is acceptable for L1 cache with bounded size (10k entries by default)
        // For L2 Redis cache, use SCAN with pattern matching

        // Iterate over all entries and invalidate matching ones
        for (key, _) in self.memory_cache.iter() {
            // Parse key to check if it matches this collection/tenant
            // Key format: "qc:{hash}" where hash includes tenant_id and collection
            // We need to invalidate conservatively since we can't decode the hash
            // Solution: Track keys by collection/tenant in a separate map for O(1) lookup
            // For now, invalidate all as a safe default until we implement key tracking
            self.memory_cache.invalidate(&key).await;
        }

        // TODO: Implement key tracking map: HashMap<(TenantId, Collection), HashSet<CacheKey>>
        // This would allow O(1) lookup and targeted invalidation without iteration
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            memory_entries: self.memory_cache.entry_count(),
            memory_hit_rate: self.memory_cache.hit_count() as f64
                / (self.memory_cache.hit_count() + self.memory_cache.miss_count()).max(1) as f64,
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

        let key = QueryCacheKey::new(
            "tenant_1".to_string(),
            "test_collection".to_string(),
            vec![1.0, 2.0, 3.0],
            10,
        );

        let cache_key = cache.generate_key(&key);

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

        cache.set(cache_key.clone(), result.clone()).await;

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

        let key = QueryCacheKey::new(
            "tenant_1".to_string(),
            "test_collection".to_string(),
            vec![1.0, 2.0, 3.0],
            10,
        );

        let cache_key = cache.generate_key(&key);

        let result = CachedQueryResult {
            results: vec![],
            cached_at: chrono::Utc::now().timestamp(),
            latency_ms: 10,
        };

        cache.set(cache_key.clone(), result).await;
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

        let key = QueryCacheKey::new(
            "tenant_1".to_string(),
            "test_collection".to_string(),
            vec![1.0, 2.0, 3.0],
            10,
        );

        let cache_key = cache.generate_key(&key);
        let result = CachedQueryResult {
            results: vec![],
            cached_at: chrono::Utc::now().timestamp(),
            latency_ms: 10,
        };

        cache.set(cache_key.clone(), result).await;

        let stats = cache.stats();
        assert_eq!(stats.memory_entries, 1);
    }
}
