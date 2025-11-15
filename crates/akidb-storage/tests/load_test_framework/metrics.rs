//! Metrics collection and analysis

use std::time::{Duration, Instant};

/// Collected metrics from a load test run
#[derive(Debug, Clone)]
pub struct LoadTestMetrics {
    /// Test start time
    pub start_time: Instant,

    /// Test end time
    pub end_time: Instant,

    /// Total requests attempted
    pub total_requests: usize,

    /// Successful requests
    pub successful_requests: usize,

    /// Failed requests
    pub failed_requests: usize,

    /// Latencies in microseconds
    pub latencies_us: Vec<u64>,

    /// Memory samples (RSS in bytes)
    pub memory_samples: Vec<MemorySample>,

    /// CPU samples (utilization 0.0-1.0)
    pub cpu_samples: Vec<CpuSample>,

    /// Error messages
    pub errors: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MemorySample {
    pub timestamp: Instant,
    pub rss_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct CpuSample {
    #[allow(dead_code)] // Reserved for time-series analysis
    pub timestamp: Instant,
    pub utilization: f64, // 0.0-1.0
}

impl Default for LoadTestMetrics {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            start_time: now,
            end_time: now,
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            latencies_us: Vec::new(),
            memory_samples: Vec::new(),
            cpu_samples: Vec::new(),
            errors: Vec::new(),
        }
    }
}

impl LoadTestMetrics {
    /// Calculate error rate (0.0-1.0)
    pub fn error_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        self.failed_requests as f64 / self.total_requests as f64
    }

    /// Calculate throughput (requests per second)
    pub fn throughput_qps(&self) -> f64 {
        let duration = self.duration();
        if duration.as_secs_f64() == 0.0 {
            return 0.0;
        }
        self.successful_requests as f64 / duration.as_secs_f64()
    }

    /// Get test duration
    pub fn duration(&self) -> Duration {
        self.end_time.duration_since(self.start_time)
    }

    /// Get P50 latency
    pub fn p50_latency(&self) -> Duration {
        self.percentile(0.50)
    }

    /// Get P90 latency
    pub fn p90_latency(&self) -> Duration {
        self.percentile(0.90)
    }

    /// Get P95 latency
    pub fn p95_latency(&self) -> Duration {
        self.percentile(0.95)
    }

    /// Get P99 latency
    pub fn p99_latency(&self) -> Duration {
        self.percentile(0.99)
    }

    /// Get max latency
    pub fn max_latency(&self) -> Duration {
        if self.latencies_us.is_empty() {
            return Duration::from_micros(0);
        }
        Duration::from_micros(*self.latencies_us.iter().max().unwrap())
    }

    /// Calculate latency percentile
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

    /// Get average memory usage (MB)
    pub fn avg_memory_mb(&self) -> f64 {
        if self.memory_samples.is_empty() {
            return 0.0;
        }

        let sum: u64 = self.memory_samples.iter().map(|s| s.rss_bytes).sum();
        (sum as f64 / self.memory_samples.len() as f64) / 1_048_576.0
    }

    /// Get peak memory usage (MB)
    pub fn peak_memory_mb(&self) -> f64 {
        self.memory_samples
            .iter()
            .map(|s| s.rss_bytes)
            .max()
            .unwrap_or(0) as f64
            / 1_048_576.0
    }

    /// Calculate memory growth rate (MB per minute)
    pub fn memory_growth_mb_per_min(&self) -> f64 {
        if self.memory_samples.len() < 2 {
            return 0.0;
        }

        let first = &self.memory_samples[0];
        let last = &self.memory_samples[self.memory_samples.len() - 1];

        let duration_mins = last.timestamp.duration_since(first.timestamp).as_secs_f64() / 60.0;
        if duration_mins == 0.0 {
            return 0.0;
        }

        let growth_bytes = last.rss_bytes as f64 - first.rss_bytes as f64;
        (growth_bytes / 1_048_576.0) / duration_mins
    }

    /// Get average CPU utilization (0.0-1.0)
    pub fn avg_cpu_utilization(&self) -> f64 {
        if self.cpu_samples.is_empty() {
            return 0.0;
        }

        let sum: f64 = self.cpu_samples.iter().map(|s| s.utilization).sum();
        sum / self.cpu_samples.len() as f64
    }

    /// Get peak CPU utilization (0.0-1.0)
    pub fn peak_cpu_utilization(&self) -> f64 {
        self.cpu_samples
            .iter()
            .map(|s| s.utilization)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0)
    }
}

/// Metrics collector for real-time metrics gathering
pub struct MetricsCollector {
    metrics: LoadTestMetrics,
}

impl MetricsCollector {
    /// Create new metrics collector
    pub fn new() -> Self {
        Self {
            metrics: LoadTestMetrics {
                start_time: Instant::now(),
                ..Default::default()
            },
        }
    }

    /// Record a successful request
    pub fn record_success(&mut self, latency: Duration) {
        self.metrics.total_requests += 1;
        self.metrics.successful_requests += 1;
        self.metrics.latencies_us.push(latency.as_micros() as u64);
    }

    /// Record a failed request
    pub fn record_failure(&mut self, latency: Duration, error: String) {
        self.metrics.total_requests += 1;
        self.metrics.failed_requests += 1;
        self.metrics.latencies_us.push(latency.as_micros() as u64);
        self.metrics.errors.push(error);
    }

    /// Record memory sample
    pub fn record_memory(&mut self, rss_bytes: u64) {
        self.metrics.memory_samples.push(MemorySample {
            timestamp: Instant::now(),
            rss_bytes,
        });
    }

    /// Record CPU sample
    pub fn record_cpu(&mut self, utilization: f64) {
        self.metrics.cpu_samples.push(CpuSample {
            timestamp: Instant::now(),
            utilization,
        });
    }

    /// Get current metrics snapshot
    pub fn snapshot(&self) -> LoadTestMetrics {
        let mut metrics = self.metrics.clone();
        metrics.end_time = Instant::now();
        metrics
    }

    /// Finalize metrics collection
    pub fn finalize(mut self) -> LoadTestMetrics {
        self.metrics.end_time = Instant::now();
        self.metrics
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_error_rate() {
        let metrics = LoadTestMetrics {
            total_requests: 100,
            successful_requests: 98,
            failed_requests: 2,
            ..Default::default()
        };

        assert_eq!(metrics.error_rate(), 0.02); // 2%
    }

    #[test]
    fn test_metrics_throughput() {
        let start = Instant::now();
        let end = start + Duration::from_secs(10);

        let metrics = LoadTestMetrics {
            start_time: start,
            end_time: end,
            successful_requests: 1000,
            ..Default::default()
        };

        assert_eq!(metrics.throughput_qps(), 100.0); // 1000 / 10 = 100 QPS
    }

    #[test]
    fn test_metrics_percentiles() {
        let mut metrics = LoadTestMetrics::default();

        // Add 100 samples: 0us, 100us, 200us, ..., 9900us
        metrics.latencies_us = (0..100).map(|i| i * 100).collect();

        assert_eq!(metrics.p50_latency(), Duration::from_micros(5000)); // 50th
        assert_eq!(metrics.p95_latency(), Duration::from_micros(9500)); // 95th
        assert_eq!(metrics.p99_latency(), Duration::from_micros(9900)); // 99th
    }

    #[test]
    fn test_memory_growth() {
        let start = Instant::now();
        let metrics = LoadTestMetrics {
            memory_samples: vec![
                MemorySample {
                    timestamp: start,
                    rss_bytes: 100 * 1_048_576, // 100 MB
                },
                MemorySample {
                    timestamp: start + Duration::from_secs(60),
                    rss_bytes: 110 * 1_048_576, // 110 MB
                },
            ],
            ..Default::default()
        };

        assert_eq!(metrics.memory_growth_mb_per_min(), 10.0); // 10 MB/min
    }

    #[test]
    fn test_collector() {
        let mut collector = MetricsCollector::new();

        collector.record_success(Duration::from_millis(10));
        collector.record_success(Duration::from_millis(20));
        collector.record_failure(Duration::from_millis(100), "timeout".to_string());

        let metrics = collector.finalize();

        assert_eq!(metrics.total_requests, 3);
        assert_eq!(metrics.successful_requests, 2);
        assert_eq!(metrics.failed_requests, 1);
        assert_eq!(metrics.error_rate(), 1.0 / 3.0);
        assert_eq!(metrics.errors.len(), 1);
    }
}
