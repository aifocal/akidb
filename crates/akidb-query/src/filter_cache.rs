//! Filter cache for caching parsed filter ASTs
//!
//! This module provides caching for parsed filter expressions (FilterTree AST),
//! avoiding the cost of re-parsing the same filter JSON multiple times.

use moka::future::Cache;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;

use crate::filter_parser::FilterTree;

/// Cache key for filter expressions
/// Uses SHA-256 hash of the filter JSON for stable, collision-resistant keys
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FilterCacheKey {
    /// SHA-256 hash of the canonical JSON representation
    filter_hash: [u8; 32],
}

impl FilterCacheKey {
    /// Create a new cache key from a filter JSON value
    fn new(filter: &Value) -> Self {
        // Serialize to canonical JSON string (sorted keys)
        let json_str = serde_json::to_string(filter).unwrap_or_default();

        // Compute SHA-256 hash
        let mut hasher = Sha256::new();
        hasher.update(json_str.as_bytes());
        let hash_result = hasher.finalize();

        let mut filter_hash = [0u8; 32];
        filter_hash.copy_from_slice(&hash_result);

        Self { filter_hash }
    }
}

/// Filter cache for storing parsed FilterTree ASTs
///
/// This cache stores the parsed AST representation of filter expressions,
/// avoiding the cost of re-parsing JSON filters. The cache uses SHA-256
/// hashes of the filter JSON as keys for collision-resistant lookups.
///
/// # Cache Strategy
///
/// - **LRU eviction**: Least recently used entries are evicted when cache is full
/// - **TTL**: Entries expire after 5 minutes of creation (not access)
/// - **Size limit**: Maximum 10,000 entries
///
/// # Why Cache AST Instead of Bitmap?
///
/// 1. **Data independence**: AST is independent of actual data, only depends on filter structure
/// 2. **Smaller size**: AST is much smaller than RoaringBitmap results
/// 3. **Longer validity**: AST never becomes stale, unlike bitmaps which change with data
///
/// The actual bitmap evaluation still happens on every query, using the cached AST.
pub struct FilterCache {
    cache: Cache<FilterCacheKey, Arc<FilterTree>>,
}

impl FilterCache {
    /// Create a new filter cache with default configuration
    ///
    /// Default configuration:
    /// - Max 10,000 entries
    /// - 5 minute TTL (time to idle, not time to live)
    pub fn new() -> Self {
        Self {
            cache: Cache::builder()
                .max_capacity(10_000)
                .time_to_idle(Duration::from_secs(300)) // 5 minutes
                .build(),
        }
    }

    /// Get a cached FilterTree AST for the given filter
    ///
    /// Returns None if the filter is not in the cache.
    pub async fn get(&self, filter: &Value) -> Option<Arc<FilterTree>> {
        let key = FilterCacheKey::new(filter);
        self.cache.get(&key).await
    }

    /// Store a FilterTree AST in the cache
    ///
    /// The AST will be evicted after 5 minutes of no access or when
    /// the cache reaches its maximum capacity.
    pub async fn put(&self, filter: &Value, tree: Arc<FilterTree>) {
        let key = FilterCacheKey::new(filter);
        self.cache.insert(key, tree).await;
    }

    /// Get cache statistics
    ///
    /// Returns (entry_count, estimated_size)
    pub fn stats(&self) -> (u64, u64) {
        (self.cache.entry_count(), self.cache.weighted_size())
    }

    /// Clear all entries from the cache
    pub async fn clear(&self) {
        self.cache.invalidate_all();
        // Wait for invalidation to complete
        self.cache.run_pending_tasks().await;
    }
}

impl Default for FilterCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_cache_hit() {
        let cache = FilterCache::new();
        let filter = json!({"field": "category", "match": "electronics"});

        // First get should miss
        assert!(cache.get(&filter).await.is_none());

        // Put a dummy tree
        let tree = Arc::new(FilterTree::Term {
            field: "category".to_string(),
            value: json!("electronics"),
        });
        cache.put(&filter, tree.clone()).await;

        // Second get should hit
        let cached = cache.get(&filter).await;
        assert!(cached.is_some());

        // Should be the same Arc
        assert!(Arc::ptr_eq(&cached.unwrap(), &tree));
    }

    #[tokio::test]
    async fn test_different_filters_different_keys() {
        let cache = FilterCache::new();

        let filter1 = json!({"field": "category", "match": "electronics"});
        let filter2 = json!({"field": "category", "match": "books"});

        let tree1 = Arc::new(FilterTree::Term {
            field: "category".to_string(),
            value: json!("electronics"),
        });
        let tree2 = Arc::new(FilterTree::Term {
            field: "category".to_string(),
            value: json!("books"),
        });

        cache.put(&filter1, tree1.clone()).await;
        cache.put(&filter2, tree2.clone()).await;

        // Both should be cached independently
        let cached1 = cache.get(&filter1).await.unwrap();
        let cached2 = cache.get(&filter2).await.unwrap();

        assert!(Arc::ptr_eq(&cached1, &tree1));
        assert!(Arc::ptr_eq(&cached2, &tree2));
    }

    #[tokio::test]
    async fn test_same_filter_same_key() {
        let cache = FilterCache::new();

        // Two identical filters should produce the same cache key
        let filter1 = json!({"field": "category", "match": "electronics"});
        let filter2 = json!({"field": "category", "match": "electronics"});

        let tree = Arc::new(FilterTree::Term {
            field: "category".to_string(),
            value: json!("electronics"),
        });

        cache.put(&filter1, tree.clone()).await;

        // Should be able to retrieve with filter2
        let cached = cache.get(&filter2).await;
        assert!(cached.is_some());
        assert!(Arc::ptr_eq(&cached.unwrap(), &tree));
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache = FilterCache::new();

        let (count, _size) = cache.stats();
        assert_eq!(count, 0);

        // Add one entry
        let filter = json!({"field": "category", "match": "electronics"});
        let tree = Arc::new(FilterTree::Term {
            field: "category".to_string(),
            value: json!("electronics"),
        });
        cache.put(&filter, tree).await;

        // Run pending tasks to update stats
        cache.cache.run_pending_tasks().await;

        let (count, _size) = cache.stats();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let cache = FilterCache::new();

        let filter = json!({"field": "category", "match": "electronics"});
        let tree = Arc::new(FilterTree::Term {
            field: "category".to_string(),
            value: json!("electronics"),
        });
        cache.put(&filter, tree).await;

        // Should be cached
        assert!(cache.get(&filter).await.is_some());

        // Clear cache
        cache.clear().await;

        // Should be gone
        assert!(cache.get(&filter).await.is_none());
    }
}
