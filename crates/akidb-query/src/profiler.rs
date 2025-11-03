//! Query profiling and slow query detection
//!
//! This module provides infrastructure for profiling query execution
//! and detecting slow queries that exceed configured thresholds.

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// Query profile that tracks execution stages and detects slow queries
#[derive(Debug, Clone, Serialize)]
pub struct QueryProfile {
    /// Unique identifier for this query
    pub query_id: String,

    /// Collection being queried
    pub collection: String,

    /// Query start timestamp (not serialized)
    #[serde(skip)]
    pub start_time: Instant,

    /// Individual execution stages with timing
    pub stages: Vec<ProfileStage>,

    /// Total query duration (set when finished)
    pub total_duration_ms: Option<u64>,

    /// Whether this query was flagged as slow
    pub is_slow: bool,
}

/// A single profiled stage of query execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileStage {
    /// Stage name (e.g., "parse", "plan", "execute", "serialize")
    pub name: String,

    /// Stage duration in milliseconds
    pub duration_ms: u64,

    /// Additional metadata about this stage
    pub metadata: serde_json::Value,
}

impl QueryProfile {
    /// Create a new query profile
    pub fn new(query_id: impl Into<String>, collection: impl Into<String>) -> Self {
        Self {
            query_id: query_id.into(),
            collection: collection.into(),
            start_time: Instant::now(),
            stages: Vec::new(),
            total_duration_ms: None,
            is_slow: false,
        }
    }

    /// Record a completed execution stage
    pub fn record_stage(
        &mut self,
        name: impl Into<String>,
        duration: Duration,
        metadata: serde_json::Value,
    ) {
        let stage = ProfileStage {
            name: name.into(),
            duration_ms: duration.as_millis() as u64,
            metadata,
        };

        // Record metric for this stage
        akidb_core::metrics::QUERY_STAGE_DURATION
            .with_label_values(&[&stage.name])
            .observe(duration.as_secs_f64());

        self.stages.push(stage);
    }

    /// Finish profiling and check for slow query
    pub fn finish(mut self, slow_query_threshold: Duration) -> Self {
        let total = self.start_time.elapsed();
        let total_ms = total.as_millis() as u64;
        self.total_duration_ms = Some(total_ms);
        self.is_slow = total > slow_query_threshold;

        if self.is_slow {
            // Log slow query with details
            warn!(
                query_id = %self.query_id,
                collection = %self.collection,
                duration_ms = total_ms,
                threshold_ms = slow_query_threshold.as_millis(),
                stages = ?self.stages,
                bottleneck = ?self.bottleneck().map(|s| s.name.as_str()),
                "Slow query detected"
            );

            // Record slow query metric
            akidb_core::metrics::SLOW_QUERIES
                .with_label_values(&[
                    &self.collection,
                    &slow_query_threshold.as_millis().to_string(),
                ])
                .inc();
        } else {
            info!(
                query_id = %self.query_id,
                collection = %self.collection,
                duration_ms = total_ms,
                "Query completed"
            );
        }

        self
    }

    /// Identify the bottleneck stage (longest duration)
    pub fn bottleneck(&self) -> Option<&ProfileStage> {
        self.stages.iter().max_by_key(|s| s.duration_ms)
    }

    /// Get the total duration (if finished)
    pub fn total_duration(&self) -> Option<Duration> {
        self.total_duration_ms.map(Duration::from_millis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_query_profile_creation() {
        let profile = QueryProfile::new("test-query-1", "test_collection");
        assert_eq!(profile.query_id, "test-query-1");
        assert_eq!(profile.collection, "test_collection");
        assert_eq!(profile.stages.len(), 0);
        assert_eq!(profile.total_duration_ms, None);
        assert!(!profile.is_slow);
    }

    #[test]
    fn test_record_stages() {
        let mut profile = QueryProfile::new("test-query-2", "test_collection");

        profile.record_stage("parse", Duration::from_millis(5), json!({"lines": 10}));
        profile.record_stage(
            "execute",
            Duration::from_millis(100),
            json!({"vectors": 1000}),
        );
        profile.record_stage(
            "serialize",
            Duration::from_millis(3),
            json!({"results": 10}),
        );

        assert_eq!(profile.stages.len(), 3);
        assert_eq!(profile.stages[0].name, "parse");
        assert_eq!(profile.stages[0].duration_ms, 5);
        assert_eq!(profile.stages[1].duration_ms, 100);
    }

    #[test]
    fn test_bottleneck_detection() {
        let mut profile = QueryProfile::new("test-query-3", "test_collection");

        profile.record_stage("parse", Duration::from_millis(5), json!({}));
        profile.record_stage("execute", Duration::from_millis(100), json!({}));
        profile.record_stage("serialize", Duration::from_millis(3), json!({}));

        let bottleneck = profile.bottleneck().unwrap();
        assert_eq!(bottleneck.name, "execute");
        assert_eq!(bottleneck.duration_ms, 100);
    }

    #[test]
    fn test_slow_query_detection() {
        let profile = QueryProfile::new("slow-query-1", "test_collection");

        // Simulate slow execution
        std::thread::sleep(Duration::from_millis(150));

        let profile = profile.finish(Duration::from_millis(100));

        assert!(profile.is_slow);
        assert!(profile.total_duration_ms.unwrap() >= 150);
    }

    #[test]
    fn test_fast_query() {
        let profile = QueryProfile::new("fast-query-1", "test_collection");

        // Immediate finish
        let profile = profile.finish(Duration::from_secs(1));

        assert!(!profile.is_slow);
        assert!(profile.total_duration_ms.unwrap() < 100);
    }

    #[test]
    fn test_total_duration() {
        let mut profile = QueryProfile::new("duration-test", "test_collection");

        std::thread::sleep(Duration::from_millis(50));
        profile.record_stage("stage1", Duration::from_millis(20), json!({}));

        let profile = profile.finish(Duration::from_secs(1));

        let duration = profile.total_duration().unwrap();
        assert!(duration >= Duration::from_millis(50));
    }
}
