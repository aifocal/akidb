// ! Query result caching with epoch-based invalidation
//!
//! This module provides an LRU cache for vector search results.
//! Cache keys are based on collection name, query vector, top_k, filter, and collection epoch.
//! When vectors are inserted, the collection epoch is bumped, invalidating all cached queries.

use crate::handlers::search::SearchResponse;
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;

/// Cache key components
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheKeyComponents {
    collection: String,
    vector: Vec<f32>,
    top_k: u16,
    filter: Option<serde_json::Value>,
    epoch: u64,
}

/// Query cache key (SHA256 hash of components)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey(String);

impl CacheKey {
    /// Create cache key from components
    pub fn from_components(
        collection: &str,
        vector: &[f32],
        top_k: u16,
        filter: Option<&serde_json::Value>,
        epoch: u64,
    ) -> Self {
        let components = CacheKeyComponents {
            collection: collection.to_string(),
            vector: vector.to_vec(),
            top_k,
            filter: filter.cloned(),
            epoch,
        };

        // Serialize to JSON for deterministic hashing
        let json =
            serde_json::to_string(&components).expect("CacheKeyComponents should always serialize");

        // Hash with SHA256
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        let hash = hasher.finalize();

        // Convert to hex string
        let hash_hex = format!("{:x}", hash);

        Self(hash_hex)
    }
}

/// Query result cache
#[derive(Clone)]
pub struct QueryCache {
    cache: Cache<CacheKey, Arc<SearchResponse>>,
}

impl QueryCache {
    /// Create a new query cache
    ///
    /// # Arguments
    /// * `max_capacity` - Maximum number of cached queries (default: 10,000)
    /// * `ttl` - Time-to-live for cached entries (default: 5 minutes)
    pub fn new(max_capacity: u64, ttl: Duration) -> Self {
        let cache = Cache::builder()
            .max_capacity(max_capacity)
            .time_to_live(ttl)
            .build();

        debug!(
            "QueryCache initialized with capacity={}, ttl={:?}",
            max_capacity, ttl
        );

        Self { cache }
    }

    /// Get cached result
    pub async fn get(
        &self,
        collection: &str,
        vector: &[f32],
        top_k: u16,
        filter: Option<&serde_json::Value>,
        epoch: u64,
    ) -> Option<Arc<SearchResponse>> {
        let key = CacheKey::from_components(collection, vector, top_k, filter, epoch);
        self.cache.get(&key).await
    }

    /// Put result into cache
    pub async fn put(
        &self,
        collection: &str,
        vector: &[f32],
        top_k: u16,
        filter: Option<&serde_json::Value>,
        epoch: u64,
        response: Arc<SearchResponse>,
    ) {
        let key = CacheKey::from_components(collection, vector, top_k, filter, epoch);
        self.cache.insert(key, response).await;
    }

    /// Invalidate all entries for a collection by bumping its epoch
    ///
    /// Note: This doesn't actively remove entries. Instead, the epoch-based key
    /// ensures new inserts will cause a cache miss (epoch mismatch).
    pub async fn invalidate_collection(&self, _collection: &str) {
        // Epoch-based invalidation: no action needed here
        // The caller should bump the epoch in CollectionMetadata
        // Old entries will naturally expire via TTL or LRU eviction
        debug!("Collection invalidated via epoch bump (passive invalidation)");
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        CacheStats {
            entry_count: self.cache.entry_count(),
            weighted_size: self.cache.weighted_size(),
        }
    }

    /// Clear all cached entries (for testing)
    #[cfg(test)]
    pub async fn clear(&self) {
        self.cache.invalidate_all();
        self.cache.run_pending_tasks().await;
    }
}

impl Default for QueryCache {
    fn default() -> Self {
        Self::new(10_000, Duration::from_secs(300))
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entry_count: u64,
    pub weighted_size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_key_generation() {
        let key1 = CacheKey::from_components("test_collection", &[1.0, 2.0, 3.0], 10, None, 1);

        let key2 = CacheKey::from_components("test_collection", &[1.0, 2.0, 3.0], 10, None, 1);

        // Same components should produce same key
        assert_eq!(key1, key2);

        let key3 = CacheKey::from_components(
            "test_collection",
            &[1.0, 2.0, 3.0],
            10,
            None,
            2, // Different epoch
        );

        // Different epoch should produce different key
        assert_ne!(key1, key3);
    }

    #[tokio::test]
    async fn test_cache_get_put() {
        let cache = QueryCache::new(100, Duration::from_secs(60));

        let response = Arc::new(SearchResponse {
            collection: "test".to_string(),
            results: vec![],
            count: 0,
        });

        // Initially empty
        let result = cache.get("test", &[1.0, 2.0], 10, None, 1).await;
        assert!(result.is_none());

        // Put and retrieve
        cache
            .put("test", &[1.0, 2.0], 10, None, 1, response.clone())
            .await;

        let result = cache.get("test", &[1.0, 2.0], 10, None, 1).await;
        assert!(result.is_some());
        assert_eq!(result.unwrap().collection, "test");
    }

    #[tokio::test]
    async fn test_cache_epoch_invalidation() {
        let cache = QueryCache::new(100, Duration::from_secs(60));

        let response = Arc::new(SearchResponse {
            collection: "test".to_string(),
            results: vec![],
            count: 0,
        });

        // Cache with epoch 1
        cache
            .put("test", &[1.0, 2.0], 10, None, 1, response.clone())
            .await;

        // Retrieve with epoch 1 - should hit
        let result = cache.get("test", &[1.0, 2.0], 10, None, 1).await;
        assert!(result.is_some());

        // Retrieve with epoch 2 - should miss (epoch changed)
        let result = cache.get("test", &[1.0, 2.0], 10, None, 2).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache = QueryCache::new(100, Duration::from_secs(60));

        let response = Arc::new(SearchResponse {
            collection: "test".to_string(),
            results: vec![],
            count: 0,
        });

        // Initially empty
        let stats = cache.stats().await;
        assert_eq!(stats.entry_count, 0);

        // Add entry
        cache
            .put("test", &[1.0, 2.0], 10, None, 1, response.clone())
            .await;

        // Run pending tasks to update stats
        cache.cache.run_pending_tasks().await;

        let stats = cache.stats().await;
        assert_eq!(stats.entry_count, 1);
    }
}
