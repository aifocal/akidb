//! Load test orchestrator for running scenarios

use super::client::LoadTestClient;
use super::metrics::{LoadTestMetrics, MetricsCollector};
use super::profiles::{LoadProfile, WorkloadMix};
use super::reporter::ResultWriter;
use super::SuccessCriteria;
use akidb_core::ids::CollectionId;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;

/// Configuration for a load test scenario
#[derive(Debug, Clone)]
pub struct ScenarioConfig {
    /// Scenario name
    pub name: String,

    /// Test duration
    pub duration: Duration,

    /// Load profile (how QPS changes over time)
    pub load_profile: LoadProfile,

    /// Workload mix (operation type percentages)
    pub workload_mix: WorkloadMix,

    /// Dataset size (number of vectors)
    pub dataset_size: usize,

    /// Vector dimension
    pub dimension: usize,

    /// Number of concurrent clients
    pub concurrency: usize,

    /// Sample interval for system metrics
    pub sample_interval: Duration,
}

impl Default for ScenarioConfig {
    fn default() -> Self {
        Self {
            name: "Default Scenario".to_string(),
            duration: Duration::from_secs(60),
            load_profile: LoadProfile::Constant { qps: 100 },
            workload_mix: WorkloadMix::default(),
            dataset_size: 10_000,
            dimension: 512,
            concurrency: 10,
            sample_interval: Duration::from_secs(1),
        }
    }
}

/// Load test orchestrator
pub struct LoadTestOrchestrator {
    config: ScenarioConfig,
    collector: Arc<RwLock<MetricsCollector>>,
}

impl LoadTestOrchestrator {
    /// Create new orchestrator
    pub fn new(config: ScenarioConfig) -> Self {
        Self {
            config,
            collector: Arc::new(RwLock::new(MetricsCollector::new())),
        }
    }

    /// Run the load test
    pub async fn run(&self) -> Result<LoadTestMetrics, String> {
        println!("🚀 Starting load test: {}", self.config.name);
        println!("   Duration: {:?}", self.config.duration);
        println!("   Load profile: {}", self.config.load_profile.description());
        println!("   Dataset: {} vectors ({}d)", self.config.dataset_size, self.config.dimension);
        println!("   Concurrency: {} clients", self.config.concurrency);
        println!();

        // Validate workload mix
        self.config
            .workload_mix
            .validate()
            .map_err(|e| format!("Invalid workload mix: {}", e))?;

        // Create collection for testing
        let collection_id = CollectionId::new();
        let client = Arc::new(LoadTestClient::new(collection_id, self.config.dimension));

        // Spawn system metrics collector
        let collector_clone = Arc::clone(&self.collector);
        let sample_interval = self.config.sample_interval;
        let duration = self.config.duration;

        let metrics_handle = tokio::spawn(async move {
            Self::collect_system_metrics(collector_clone, sample_interval, duration).await;
        });

        // Run load generation
        self.run_load_generation(client).await?;

        // Wait for metrics collection to finish
        metrics_handle.await.map_err(|e| format!("Metrics collection failed: {}", e))?;

        // Finalize and return metrics
        let metrics = {
            let collector = self.collector.read().await;
            collector.snapshot()
        };

        println!("\n✅ Load test complete");
        println!("   Total requests: {}", metrics.total_requests);
        println!("   Successful: {}", metrics.successful_requests);
        println!("   Failed: {}", metrics.failed_requests);
        println!("   Error rate: {:.4}%", metrics.error_rate() * 100.0);
        println!("   P95 latency: {:.2}ms", metrics.p95_latency().as_secs_f64() * 1000.0);
        println!("   Throughput: {:.1} QPS", metrics.throughput_qps());
        println!();

        Ok(metrics)
    }

    /// Run load generation loop
    async fn run_load_generation(&self, client: Arc<LoadTestClient>) -> Result<(), String> {
        let start_time = tokio::time::Instant::now();
        let end_time = start_time + self.config.duration;

        let tick_duration = Duration::from_secs(1);
        let mut ticker = interval(tick_duration);

        let report_interval = Duration::from_secs(10);
        let mut last_report_time = std::time::Duration::ZERO;

        while tokio::time::Instant::now() < end_time {
            ticker.tick().await;

            let elapsed = start_time.elapsed();

            // Get current QPS based on load profile
            let current_qps = self.config.load_profile.qps_at(Duration::from_secs(elapsed.as_secs()));

            // Spawn workers for this tick
            let mut handles = Vec::new();

            for _ in 0..current_qps {
                let client_clone = Arc::clone(&client);
                let collector_clone = Arc::clone(&self.collector);
                let workload_mix = self.config.workload_mix.clone();

                let handle = tokio::spawn(async move {
                    Self::execute_operation(client_clone, collector_clone, workload_mix).await;
                });

                handles.push(handle);
            }

            // Wait for all operations in this tick
            for handle in handles {
                let _ = handle.await;
            }

            // Periodic progress report
            if elapsed - last_report_time >= report_interval {
                last_report_time = elapsed;
                let snapshot = self.collector.read().await.snapshot();
                println!(
                    "[{:.0}s] Requests: {}, P95: {:.1}ms, Errors: {:.2}%, QPS: {}",
                    elapsed.as_secs(),
                    snapshot.total_requests,
                    snapshot.p95_latency().as_secs_f64() * 1000.0,
                    snapshot.error_rate() * 100.0,
                    current_qps
                );
            }
        }

        Ok(())
    }

    /// Execute a single operation
    async fn execute_operation(
        client: Arc<LoadTestClient>,
        collector: Arc<RwLock<MetricsCollector>>,
        workload_mix: WorkloadMix,
    ) {
        // Choose operation type based on workload mix
        let op_type = LoadTestClient::choose_operation(
            workload_mix.search_pct,
            workload_mix.insert_pct,
            workload_mix.update_pct,
            workload_mix.delete_pct,
            workload_mix.metadata_pct,
        );

        // Execute operation
        match client.execute(op_type).await {
            Ok(duration) => {
                collector.write().await.record_success(duration);
            }
            Err(error) => {
                collector
                    .write()
                    .await
                    .record_failure(Duration::from_secs(0), error);
            }
        }
    }

    /// Collect system metrics (memory, CPU)
    async fn collect_system_metrics(
        collector: Arc<RwLock<MetricsCollector>>,
        sample_interval: Duration,
        duration: Duration,
    ) {
        let start = tokio::time::Instant::now();
        let mut ticker = interval(sample_interval);

        while start.elapsed() < duration {
            ticker.tick().await;

            // Sample memory (simplified - would use sysinfo in production)
            let memory_bytes = Self::get_current_memory();
            collector.write().await.record_memory(memory_bytes);

            // Sample CPU (simplified - would use sysinfo in production)
            let cpu_utilization = Self::get_cpu_utilization();
            collector.write().await.record_cpu(cpu_utilization);
        }
    }

    /// Get current process memory (simplified for now)
    fn get_current_memory() -> u64 {
        // In production, would use: sysinfo::System::new_all().process(pid).memory()
        // For now, return a dummy value
        512 * 1_048_576 // 512 MB
    }

    /// Get CPU utilization (simplified for now)
    fn get_cpu_utilization() -> f64 {
        // In production, would use: sysinfo::System::new_all().global_cpu_info().cpu_usage()
        // For now, return a dummy value
        0.65 // 65%
    }

    /// Create result writer with collected metrics
    pub async fn create_result_writer(
        &self,
        metrics: LoadTestMetrics,
        criteria: SuccessCriteria,
    ) -> ResultWriter {
        ResultWriter::new(self.config.name.clone(), metrics, criteria)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_orchestrator_constant_load() {
        let config = ScenarioConfig {
            name: "Test Constant".to_string(),
            duration: Duration::from_secs(5),
            load_profile: LoadProfile::Constant { qps: 10 },
            workload_mix: WorkloadMix::default(),
            concurrency: 5,
            ..Default::default()
        };

        let orchestrator = LoadTestOrchestrator::new(config);
        let metrics = orchestrator.run().await.unwrap();

        // Should have ~50 requests (10 QPS * 5 seconds)
        assert!(metrics.total_requests >= 45);
        assert!(metrics.total_requests <= 55);
    }

    #[tokio::test]
    async fn test_orchestrator_with_criteria() {
        let config = ScenarioConfig {
            name: "Test with Criteria".to_string(),
            duration: Duration::from_secs(3),
            load_profile: LoadProfile::Constant { qps: 10 },
            ..Default::default()
        };

        let criteria = SuccessCriteria::development(); // Relaxed criteria

        let orchestrator = LoadTestOrchestrator::new(config);
        let metrics = orchestrator.run().await.unwrap();

        let writer = orchestrator.create_result_writer(metrics, criteria).await;

        // Should pass with development criteria
        assert!(writer.passes());
    }
}
