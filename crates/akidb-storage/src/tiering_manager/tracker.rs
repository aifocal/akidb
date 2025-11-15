use akidb_core::{CollectionId, CoreResult};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Access statistics for a collection
#[derive(Debug, Clone)]
pub struct AccessStats {
    /// Timestamp of most recent access
    pub last_accessed_at: DateTime<Utc>,
    /// Number of accesses in current time window
    pub access_count: u32,
    /// Start time of current measurement window
    pub window_start: DateTime<Utc>,
}

/// In-memory access tracker (LRU cache)
///
/// Tracks collection access patterns to inform tiering decisions.
/// - Records access timestamps
/// - Maintains access counts within time windows
/// - Provides <1ms overhead per access
pub struct AccessTracker {
    cache: Arc<RwLock<HashMap<CollectionId, AccessStats>>>,
}

impl AccessTracker {
    /// Create new access tracker
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Record a collection access
    ///
    /// This is called on every search/insert operation.
    /// Performance target: <1ms overhead
    pub async fn record(&self, collection_id: CollectionId) -> CoreResult<()> {
        let mut cache = self.cache.write().await;
        let now = Utc::now();

        cache
            .entry(collection_id)
            .and_modify(|stats| {
                stats.last_accessed_at = now;
                stats.access_count += 1;
            })
            .or_insert_with(|| AccessStats {
                last_accessed_at: now,
                access_count: 1,
                window_start: now,
            });

        Ok(())
    }

    /// Get access stats for a collection
    pub async fn get_stats(&self, collection_id: CollectionId) -> Option<AccessStats> {
        let cache = self.cache.read().await;
        cache.get(&collection_id).cloned()
    }

    /// Reset access window (called by background worker)
    ///
    /// After promoting a collection, reset its access count
    pub async fn reset_window(&self, collection_id: CollectionId) -> CoreResult<()> {
        let mut cache = self.cache.write().await;
        if let Some(stats) = cache.get_mut(&collection_id) {
            stats.access_count = 0;
            stats.window_start = Utc::now();
        }
        Ok(())
    }

    /// Clear all stats (for testing)
    #[cfg(test)]
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
}

impl Default for AccessTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_record_access() {
        let tracker = AccessTracker::new();
        let collection_id = CollectionId::new();

        tracker.record(collection_id).await.unwrap();

        let stats = tracker.get_stats(collection_id).await.unwrap();
        assert_eq!(stats.access_count, 1);
    }

    #[tokio::test]
    async fn test_multiple_accesses() {
        let tracker = AccessTracker::new();
        let collection_id = CollectionId::new();

        tracker.record(collection_id).await.unwrap();
        tracker.record(collection_id).await.unwrap();
        tracker.record(collection_id).await.unwrap();

        let stats = tracker.get_stats(collection_id).await.unwrap();
        assert_eq!(stats.access_count, 3);
    }

    #[tokio::test]
    async fn test_reset_window() {
        let tracker = AccessTracker::new();
        let collection_id = CollectionId::new();

        // Record 5 accesses
        for _ in 0..5 {
            tracker.record(collection_id).await.unwrap();
        }

        let stats_before = tracker.get_stats(collection_id).await.unwrap();
        assert_eq!(stats_before.access_count, 5);

        // Reset window
        tracker.reset_window(collection_id).await.unwrap();

        let stats_after = tracker.get_stats(collection_id).await.unwrap();
        assert_eq!(stats_after.access_count, 0);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let tracker = Arc::new(AccessTracker::new());
        let collection_id = CollectionId::new();

        let mut handles = vec![];
        for _ in 0..100 {
            let tracker = Arc::clone(&tracker);
            let handle = tokio::spawn(async move {
                tracker.record(collection_id).await.unwrap();
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap();
        }

        let stats = tracker.get_stats(collection_id).await.unwrap();
        assert_eq!(stats.access_count, 100);
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let tracker = AccessTracker::new();
        let collection_id = CollectionId::new();

        let stats = tracker.get_stats(collection_id).await;
        assert!(stats.is_none());
    }

    #[tokio::test]
    async fn test_clear() {
        let tracker = AccessTracker::new();
        let collection_id = CollectionId::new();

        tracker.record(collection_id).await.unwrap();
        assert!(tracker.get_stats(collection_id).await.is_some());

        tracker.clear().await;
        assert!(tracker.get_stats(collection_id).await.is_none());
    }
}
