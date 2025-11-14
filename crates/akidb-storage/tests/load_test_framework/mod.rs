//! Load test framework for AkiDB 2.0
//!
//! Provides comprehensive load testing infrastructure with:
//! - Multiple load profiles (constant, ramp, spike, random)
//! - Configurable workload mixes
//! - Detailed metrics collection
//! - Pass/fail assessment
//! - Report generation

pub mod orchestrator;
pub mod metrics;
pub mod reporter;
pub mod profiles;
pub mod client;

pub use orchestrator::{LoadTestOrchestrator, ScenarioConfig};
pub use reporter::ReportFormat;
pub use profiles::{LoadProfile, WorkloadMix};

/// Success criteria for load test scenarios
#[derive(Debug, Clone)]
pub struct SuccessCriteria {
    /// Maximum P95 latency in milliseconds
    pub max_p95_latency_ms: f64,

    /// Maximum error rate (0.0-1.0)
    pub max_error_rate: f64,

    /// Maximum memory growth in MB per minute
    pub max_memory_growth_mb_per_min: f64,

    /// Maximum CPU utilization (0.0-1.0)
    pub max_cpu_utilization: f64,

    /// Minimum throughput (requests per second)
    pub min_throughput_qps: Option<f64>,

    /// Maximum P99 latency in milliseconds (optional)
    pub max_p99_latency_ms: Option<f64>,
}

impl Default for SuccessCriteria {
    fn default() -> Self {
        Self {
            max_p95_latency_ms: 25.0,
            max_error_rate: 0.001, // 0.1%
            max_memory_growth_mb_per_min: 10.0,
            max_cpu_utilization: 0.80,
            min_throughput_qps: Some(50.0),
            max_p99_latency_ms: Some(100.0),
        }
    }
}

impl SuccessCriteria {
    /// Production-ready criteria (strict)
    pub fn production() -> Self {
        Self {
            max_p95_latency_ms: 25.0,
            max_error_rate: 0.001,
            max_memory_growth_mb_per_min: 5.0,
            max_cpu_utilization: 0.75,
            min_throughput_qps: Some(100.0),
            max_p99_latency_ms: Some(50.0),
        }
    }

    /// Development criteria (relaxed)
    pub fn development() -> Self {
        Self {
            max_p95_latency_ms: 50.0,
            max_error_rate: 0.01,
            max_memory_growth_mb_per_min: 20.0,
            max_cpu_utilization: 0.90,
            min_throughput_qps: Some(10.0),
            max_p99_latency_ms: Some(200.0),
        }
    }
}
