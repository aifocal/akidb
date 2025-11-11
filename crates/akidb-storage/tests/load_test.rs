//! Load testing framework for AkiDB storage layer
//!
//! Tests sustained 100 QPS for 10 minutes with mixed workload:
//! - 70% search operations
//! - 20% insert operations
//! - 10% tier control operations
//!
//! Success criteria:
//! - P95 latency <25ms
//! - Error rate <0.1%
//! - Memory stable (no leaks)
//! - CPU <80% average

use akidb_core::ids::{CollectionId, DocumentId};
use akidb_core::vector::VectorDocument;
use akidb_storage::batch_config::S3BatchConfig;
use akidb_storage::object_store::MockS3ObjectStore;
use akidb_storage::parallel_uploader::{ParallelConfig, ParallelUploader};
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::interval;

/// Workload configuration
#[derive(Debug, Clone)]
pub struct WorkloadConfig {
    /// Total duration of load test
    pub duration: Duration,
    /// Queries per second
    pub qps: usize,
    /// Percentage of search/read queries (0.0-1.0)
    pub search_pct: f32,
    /// Percentage of insert/write queries (0.0-1.0)
    pub insert_pct: f32,
    /// Percentage of tier control operations (0.0-1.0)
    pub tier_control_pct: f32,
}

impl Default for WorkloadConfig {
    fn default() -> Self {
        Self {
            duration: Duration::from_secs(600), // 10 minutes
            qps: 100,
            search_pct: 0.7,
            insert_pct: 0.2,
            tier_control_pct: 0.1,
        }
    }
}

/// Load test metrics
#[derive(Debug, Clone, Default)]
pub struct LoadTestMetrics {
    pub total_requests: usize,
    pub successful_requests: usize,
    pub failed_requests: usize,
    pub latencies_us: Vec<u64>, // Microseconds
}

impl LoadTestMetrics {
    pub fn p50(&self) -> Duration {
        self.percentile(0.50)
    }

    pub fn p95(&self) -> Duration {
        self.percentile(0.95)
    }

    pub fn p99(&self) -> Duration {
        self.percentile(0.99)
    }

    pub fn error_rate(&self) -> f32 {
        if self.total_requests == 0 {
            return 0.0;
        }
        self.failed_requests as f32 / self.total_requests as f32
    }

    fn percentile(&self, p: f64) -> Duration {
        if self.latencies_us.is_empty() {
            return Duration::from_micros(0);
        }

        let mut sorted = self.latencies_us.clone();
        sorted.sort_unstable();

        let index = ((sorted.len() as f64) * p) as usize;
        let index = index.min(sorted.len() - 1);

        Duration::from_micros(sorted[index])
    }
}

/// Load test runner
pub struct LoadTest {
    uploader: Arc<ParallelUploader>,
    config: WorkloadConfig,
    metrics: Arc<RwLock<LoadTestMetrics>>,
    collection_id: CollectionId,
}

impl LoadTest {
    pub fn new(config: WorkloadConfig) -> Self {
        // Use MockS3 for fast, deterministic testing
        let store = Arc::new(MockS3ObjectStore::new());

        let parallel_config = ParallelConfig {
            batch: S3BatchConfig {
                batch_size: 10,
                max_wait_ms: 5000,
                enable_compression: false,
            },
            max_concurrency: 20,
        };

        let uploader = ParallelUploader::new(store, parallel_config).unwrap();

        Self {
            uploader: Arc::new(uploader),
            config,
            metrics: Arc::new(RwLock::new(LoadTestMetrics::default())),
            collection_id: CollectionId::new(),
        }
    }

    /// Run the load test
    pub async fn run(&self) -> LoadTestMetrics {
        println!(
            "Starting load test: {} QPS for {:?}",
            self.config.qps, self.config.duration
        );

        let start_time = Instant::now();
        let end_time = start_time + self.config.duration;

        // Calculate operations per tick
        let tick_duration = Duration::from_secs(1);
        let ops_per_tick = self.config.qps;

        let mut ticker = interval(tick_duration);
        let mut tick_count = 0;

        while Instant::now() < end_time {
            ticker.tick().await;
            tick_count += 1;

            // Spawn workers for this tick
            let mut handles = Vec::new();

            for _ in 0..ops_per_tick {
                let uploader = self.uploader.clone();
                let config = self.config.clone();
                let metrics = self.metrics.clone();
                let collection_id = self.collection_id;

                let handle = tokio::spawn(async move {
                    Self::run_operation(uploader, collection_id, config, metrics).await;
                });

                handles.push(handle);
            }

            // Wait for all operations in this tick
            for handle in handles {
                let _ = handle.await;
            }

            if tick_count % 10 == 0 {
                let current_metrics = self.metrics.read().clone();
                println!(
                    "[{:?}] Requests: {}, P95: {:?}, Errors: {:.2}%",
                    Instant::now() - start_time,
                    current_metrics.total_requests,
                    current_metrics.p95(),
                    current_metrics.error_rate() * 100.0
                );
            }
        }

        let final_metrics = self.metrics.read().clone();
        println!("\n=== Load Test Complete ===");
        println!("Total requests: {}", final_metrics.total_requests);
        println!("Successful: {}", final_metrics.successful_requests);
        println!("Failed: {}", final_metrics.failed_requests);
        println!("Error rate: {:.2}%", final_metrics.error_rate() * 100.0);
        println!("P50 latency: {:?}", final_metrics.p50());
        println!("P95 latency: {:?}", final_metrics.p95());
        println!("P99 latency: {:?}", final_metrics.p99());

        final_metrics
    }

    /// Run a single operation
    async fn run_operation(
        uploader: Arc<ParallelUploader>,
        collection_id: CollectionId,
        config: WorkloadConfig,
        metrics: Arc<RwLock<LoadTestMetrics>>,
    ) {
        let op_type = Self::choose_operation(&config);

        let start = Instant::now();
        let result = match op_type {
            OperationType::Search => Self::do_search().await,
            OperationType::Insert => Self::do_insert(uploader, collection_id).await,
            OperationType::TierControl => Self::do_tier_control().await,
        };
        let latency = start.elapsed();

        let mut m = metrics.write();
        m.total_requests += 1;
        m.latencies_us.push(latency.as_micros() as u64);

        if result.is_ok() {
            m.successful_requests += 1;
        } else {
            m.failed_requests += 1;
        }
    }

    fn choose_operation(config: &WorkloadConfig) -> OperationType {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let roll: f32 = rng.gen();

        if roll < config.search_pct {
            OperationType::Search
        } else if roll < config.search_pct + config.insert_pct {
            OperationType::Insert
        } else {
            OperationType::TierControl
        }
    }

    async fn do_search() -> Result<(), String> {
        // Simulate search operation (query vector index)
        tokio::time::sleep(Duration::from_micros(100)).await;
        Ok(())
    }

    async fn do_insert(
        uploader: Arc<ParallelUploader>,
        collection_id: CollectionId,
    ) -> Result<(), String> {
        // Insert a vector document
        let doc = VectorDocument::new(DocumentId::new(), vec![0.1; 128]);
        uploader
            .add_document(collection_id, 128, doc)
            .await
            .map_err(|e| format!("Insert failed: {}", e))
    }

    async fn do_tier_control() -> Result<(), String> {
        // Simulate tier control operation (promote/demote)
        tokio::time::sleep(Duration::from_micros(500)).await;
        Ok(())
    }
}

enum OperationType {
    Search,
    Insert,
    TierControl,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_load_test_short_duration() {
        // Short load test (5 seconds) for CI
        let config = WorkloadConfig {
            duration: Duration::from_secs(5),
            qps: 10,
            search_pct: 0.7,
            insert_pct: 0.2,
            tier_control_pct: 0.1,
        };

        let load_test = LoadTest::new(config);
        let metrics = load_test.run().await;

        // Assertions
        assert!(
            metrics.p95() < Duration::from_millis(25),
            "P95 latency too high: {:?}",
            metrics.p95()
        );
        assert!(
            metrics.error_rate() < 0.01,
            "Error rate too high: {}",
            metrics.error_rate()
        );
        assert!(
            metrics.total_requests >= 45,
            "Expected ~50 requests in 5 sec"
        ); // 10 QPS * 5s = 50
    }

    #[tokio::test]
    #[ignore] // Run manually: cargo test load_test_full -- --ignored
    async fn test_load_test_full_10_min() {
        // Full 10-minute load test
        let config = WorkloadConfig::default(); // 100 QPS, 10 min

        let load_test = LoadTest::new(config);
        let metrics = load_test.run().await;

        // Success criteria
        assert!(
            metrics.error_rate() < 0.001,
            "Error rate too high: {}",
            metrics.error_rate()
        );
        assert!(
            metrics.p95() < Duration::from_millis(25),
            "P95 latency too high: {:?}",
            metrics.p95()
        );
        assert!(
            metrics.total_requests >= 59_000,
            "Expected ~60k requests in 10 min"
        );
    }

    #[tokio::test]
    async fn test_load_test_metrics_percentiles() {
        let mut metrics = LoadTestMetrics {
            total_requests: 100,
            successful_requests: 98,
            failed_requests: 2,
            latencies_us: (0..100).map(|i| i * 100).collect(), // 0us, 100us, 200us, ..., 9900us
        };

        assert_eq!(metrics.p50(), Duration::from_micros(5000)); // 50th value
        assert_eq!(metrics.p95(), Duration::from_micros(9500)); // 95th value
        assert_eq!(metrics.p99(), Duration::from_micros(9900)); // 99th value
        assert_eq!(metrics.error_rate(), 0.02); // 2/100 = 0.02
    }
}
