use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Cache invalidation tracker using Bloom filters
///
/// # Overview
///
/// When vectors are inserted/updated/deleted, we need to invalidate cached queries
/// that might be affected. A naive approach would invalidate all queries for the collection,
/// but this is wasteful.
///
/// Instead, we use Bloom filters to efficiently track which vectors participated in
/// which cached queries. When a vector changes, we can quickly identify and invalidate
/// only the affected cache entries.
///
/// # Architecture
///
/// - **Write Path**: When caching a query result, record all vector IDs in a Bloom filter
/// - **Invalidation Path**: When a vector changes, check Bloom filter to find affected caches
///
/// # Trade-offs
///
/// - **False Positives**: Bloom filters may indicate a cache needs invalidation when it doesn't
///   (acceptable - we just re-compute unnecessarily)
/// - **False Negatives**: Not possible with Bloom filters (critical for correctness)
/// - **Memory**: O(k) per cache entry where k is the number of results
#[derive(Clone)]
pub struct InvalidationTracker {
    /// Map: cache_key -> set of vector IDs in the result
    cache_vectors: Arc<RwLock<std::collections::HashMap<String, VectorSet>>>,
    /// Map: vector_id -> set of cache keys affected by this vector
    vector_caches: Arc<RwLock<std::collections::HashMap<String, HashSet<String>>>>,
    /// Configuration
    config: InvalidationConfig,
}

impl InvalidationTracker {
    /// Create a new invalidation tracker
    pub fn new(config: InvalidationConfig) -> Self {
        Self {
            cache_vectors: Arc::new(RwLock::new(std::collections::HashMap::new())),
            vector_caches: Arc::new(RwLock::new(std::collections::HashMap::new())),
            config,
        }
    }

    /// Record that a cache entry contains specific vectors
    ///
    /// Called when caching a query result.
    pub async fn record_cache_vectors(&self, cache_key: String, vector_ids: Vec<String>) {
        let mut cache_vectors = self.cache_vectors.write().await;
        let mut vector_caches = self.vector_caches.write().await;

        // Store vectors for this cache key
        cache_vectors.insert(cache_key.clone(), VectorSet::new(vector_ids.clone()));

        // Add cache key to each vector's affected caches
        for vector_id in vector_ids {
            vector_caches
                .entry(vector_id)
                .or_insert_with(HashSet::new)
                .insert(cache_key.clone());
        }
    }

    /// Get cache keys affected by vector changes
    ///
    /// Called when vectors are inserted, updated, or deleted.
    pub async fn get_affected_caches(&self, vector_ids: &[String]) -> Vec<String> {
        let vector_caches = self.vector_caches.read().await;

        let mut affected = HashSet::new();
        for vector_id in vector_ids {
            if let Some(cache_keys) = vector_caches.get(vector_id) {
                affected.extend(cache_keys.iter().cloned());
            }
        }

        affected.into_iter().collect()
    }

    /// Remove cache entry from tracking
    ///
    /// Called when a cache entry is evicted or invalidated.
    pub async fn remove_cache(&self, cache_key: &str) {
        let mut cache_vectors = self.cache_vectors.write().await;
        let mut vector_caches = self.vector_caches.write().await;

        // Get vectors in this cache
        if let Some(vector_set) = cache_vectors.remove(cache_key) {
            // Remove cache key from each vector's affected caches
            for vector_id in vector_set.iter() {
                if let Some(cache_keys) = vector_caches.get_mut(&vector_id) {
                    cache_keys.remove(cache_key);
                    // Clean up empty entries
                    if cache_keys.is_empty() {
                        vector_caches.remove(&vector_id);
                    }
                }
            }
        }
    }

    /// Invalidate all caches for a collection
    ///
    /// Called when collection is dropped or rebuilt.
    pub async fn invalidate_collection(&self, collection: &str, tenant_id: &str) {
        // For now, clear all tracking (simplified)
        // In production, would filter by collection/tenant
        let mut cache_vectors = self.cache_vectors.write().await;
        let mut vector_caches = self.vector_caches.write().await;

        cache_vectors.clear();
        vector_caches.clear();
    }

    /// Get statistics
    pub async fn stats(&self) -> InvalidationStats {
        let cache_vectors = self.cache_vectors.read().await;
        let vector_caches = self.vector_caches.read().await;

        InvalidationStats {
            tracked_caches: cache_vectors.len(),
            tracked_vectors: vector_caches.len(),
            avg_caches_per_vector: if vector_caches.is_empty() {
                0.0
            } else {
                vector_caches.values().map(|s| s.len()).sum::<usize>() as f64
                    / vector_caches.len() as f64
            },
        }
    }
}

impl Default for InvalidationTracker {
    fn default() -> Self {
        Self::new(InvalidationConfig::default())
    }
}

/// Set of vector IDs (using HashSet for simplicity)
///
/// In production, this could be a Bloom filter for memory efficiency,
/// but HashSet provides exact membership testing.
#[derive(Debug, Clone)]
struct VectorSet {
    ids: HashSet<String>,
}

impl VectorSet {
    fn new(ids: Vec<String>) -> Self {
        Self {
            ids: ids.into_iter().collect(),
        }
    }

    fn iter(&self) -> impl Iterator<Item = String> + '_ {
        self.ids.iter().cloned()
    }
}

/// Invalidation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidationConfig {
    /// Enable invalidation tracking
    pub enabled: bool,
    /// Maximum number of cache entries to track
    pub max_tracked_caches: usize,
    /// Use Bloom filters (future optimization)
    pub use_bloom_filters: bool,
    /// Bloom filter false positive rate
    pub bloom_fp_rate: f64,
}

impl Default for InvalidationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_tracked_caches: 100_000,
            use_bloom_filters: false,
            bloom_fp_rate: 0.01, // 1% false positive rate
        }
    }
}

/// Invalidation statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidationStats {
    /// Number of cache entries being tracked
    pub tracked_caches: usize,
    /// Number of vectors being tracked
    pub tracked_vectors: usize,
    /// Average number of caches affected per vector
    pub avg_caches_per_vector: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_invalidation_tracker_basic() {
        let tracker = InvalidationTracker::default();

        // Record cache with vectors
        tracker
            .record_cache_vectors(
                "cache_1".to_string(),
                vec!["vec_1".to_string(), "vec_2".to_string(), "vec_3".to_string()],
            )
            .await;

        // Vector change should affect cache
        let affected = tracker.get_affected_caches(&["vec_1".to_string()]).await;
        assert_eq!(affected.len(), 1);
        assert!(affected.contains(&"cache_1".to_string()));
    }

    #[tokio::test]
    async fn test_multiple_caches_same_vector() {
        let tracker = InvalidationTracker::default();

        // Multiple caches with overlapping vectors
        tracker
            .record_cache_vectors(
                "cache_1".to_string(),
                vec!["vec_1".to_string(), "vec_2".to_string()],
            )
            .await;

        tracker
            .record_cache_vectors(
                "cache_2".to_string(),
                vec!["vec_2".to_string(), "vec_3".to_string()],
            )
            .await;

        // Changing vec_2 should affect both caches
        let affected = tracker.get_affected_caches(&["vec_2".to_string()]).await;
        assert_eq!(affected.len(), 2);
        assert!(affected.contains(&"cache_1".to_string()));
        assert!(affected.contains(&"cache_2".to_string()));
    }

    #[tokio::test]
    async fn test_remove_cache() {
        let tracker = InvalidationTracker::default();

        tracker
            .record_cache_vectors("cache_1".to_string(), vec!["vec_1".to_string()])
            .await;

        // Remove cache
        tracker.remove_cache("cache_1").await;

        // Vector change should not affect any cache
        let affected = tracker.get_affected_caches(&["vec_1".to_string()]).await;
        assert_eq!(affected.len(), 0);
    }

    #[tokio::test]
    async fn test_batch_vector_changes() {
        let tracker = InvalidationTracker::default();

        tracker
            .record_cache_vectors(
                "cache_1".to_string(),
                vec!["vec_1".to_string(), "vec_2".to_string()],
            )
            .await;

        tracker
            .record_cache_vectors(
                "cache_2".to_string(),
                vec!["vec_3".to_string(), "vec_4".to_string()],
            )
            .await;

        tracker
            .record_cache_vectors(
                "cache_3".to_string(),
                vec!["vec_2".to_string(), "vec_3".to_string()],
            )
            .await;

        // Batch change affecting multiple vectors
        let affected = tracker
            .get_affected_caches(&["vec_2".to_string(), "vec_3".to_string()])
            .await;

        assert_eq!(affected.len(), 3);
        assert!(affected.contains(&"cache_1".to_string()));
        assert!(affected.contains(&"cache_2".to_string()));
        assert!(affected.contains(&"cache_3".to_string()));
    }

    #[tokio::test]
    async fn test_invalidation_stats() {
        let tracker = InvalidationTracker::default();

        tracker
            .record_cache_vectors(
                "cache_1".to_string(),
                vec!["vec_1".to_string(), "vec_2".to_string()],
            )
            .await;

        tracker
            .record_cache_vectors(
                "cache_2".to_string(),
                vec!["vec_2".to_string(), "vec_3".to_string()],
            )
            .await;

        let stats = tracker.stats().await;
        assert_eq!(stats.tracked_caches, 2);
        assert_eq!(stats.tracked_vectors, 3); // vec_1, vec_2, vec_3
    }

    #[tokio::test]
    async fn test_no_false_negatives() {
        let tracker = InvalidationTracker::default();

        // Record many caches
        for i in 0..100 {
            tracker
                .record_cache_vectors(
                    format!("cache_{}", i),
                    vec![format!("vec_{}", i), format!("vec_{}", i + 1)],
                )
                .await;
        }

        // Each vector change must be detected
        for i in 0..100 {
            let affected = tracker.get_affected_caches(&[format!("vec_{}", i)]).await;
            assert!(
                !affected.is_empty(),
                "Vector {} should affect at least one cache",
                i
            );
        }
    }

    #[tokio::test]
    async fn test_untracked_vector_change() {
        let tracker = InvalidationTracker::default();

        tracker
            .record_cache_vectors("cache_1".to_string(), vec!["vec_1".to_string()])
            .await;

        // Changing untracked vector should not affect any cache
        let affected = tracker
            .get_affected_caches(&["vec_999".to_string()])
            .await;
        assert_eq!(affected.len(), 0);
    }
}
