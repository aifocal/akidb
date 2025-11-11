//! Report generation for load test results

use super::metrics::LoadTestMetrics;
use super::SuccessCriteria;
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Report format options
#[derive(Debug, Clone, Copy)]
pub enum ReportFormat {
    Markdown,
    Json,
}

/// Result writer for generating load test reports
pub struct ResultWriter {
    pub metrics: LoadTestMetrics,
    success_criteria: SuccessCriteria,
    scenario_name: String,
}

impl ResultWriter {
    /// Create new result writer
    pub fn new(
        scenario_name: String,
        metrics: LoadTestMetrics,
        success_criteria: SuccessCriteria,
    ) -> Self {
        Self {
            metrics,
            success_criteria,
            scenario_name,
        }
    }

    /// Check if test passed success criteria
    pub fn passes(&self) -> bool {
        self.check_criteria().is_empty()
    }

    /// Get failure summary
    pub fn failure_summary(&self) -> String {
        let failures = self.check_criteria();
        if failures.is_empty() {
            return "All criteria passed".to_string();
        }

        failures.join("\n")
    }

    /// Check success criteria and return failures
    fn check_criteria(&self) -> Vec<String> {
        let mut failures = Vec::new();

        // Check P95 latency
        let p95_ms = self.metrics.p95_latency().as_secs_f64() * 1000.0;
        if p95_ms > self.success_criteria.max_p95_latency_ms {
            failures.push(format!(
                "P95 latency {:.2}ms exceeds target {:.2}ms",
                p95_ms, self.success_criteria.max_p95_latency_ms
            ));
        }

        // Check error rate
        if self.metrics.error_rate() > self.success_criteria.max_error_rate {
            failures.push(format!(
                "Error rate {:.4}% exceeds target {:.4}%",
                self.metrics.error_rate() * 100.0,
                self.success_criteria.max_error_rate * 100.0
            ));
        }

        // Check memory growth
        let memory_growth = self.metrics.memory_growth_mb_per_min();
        if memory_growth > self.success_criteria.max_memory_growth_mb_per_min {
            failures.push(format!(
                "Memory growth {:.2} MB/min exceeds target {:.2} MB/min",
                memory_growth, self.success_criteria.max_memory_growth_mb_per_min
            ));
        }

        // Check CPU utilization
        let cpu_avg = self.metrics.avg_cpu_utilization();
        if cpu_avg > self.success_criteria.max_cpu_utilization {
            failures.push(format!(
                "CPU utilization {:.1}% exceeds target {:.1}%",
                cpu_avg * 100.0,
                self.success_criteria.max_cpu_utilization * 100.0
            ));
        }

        // Check minimum throughput
        if let Some(min_qps) = self.success_criteria.min_throughput_qps {
            if self.metrics.throughput_qps() < min_qps {
                failures.push(format!(
                    "Throughput {:.1} QPS below target {:.1} QPS",
                    self.metrics.throughput_qps(),
                    min_qps
                ));
            }
        }

        // Check P99 latency (optional)
        if let Some(max_p99) = self.success_criteria.max_p99_latency_ms {
            let p99_ms = self.metrics.p99_latency().as_secs_f64() * 1000.0;
            if p99_ms > max_p99 {
                failures.push(format!(
                    "P99 latency {:.2}ms exceeds target {:.2}ms",
                    p99_ms, max_p99
                ));
            }
        }

        failures
    }

    /// Write report to file
    pub fn write_report(&self, path: impl AsRef<Path>, format: ReportFormat) -> std::io::Result<()> {
        let content = match format {
            ReportFormat::Markdown => self.generate_markdown(),
            ReportFormat::Json => self.generate_json(),
        };

        let mut file = File::create(path)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }

    /// Generate Markdown report
    fn generate_markdown(&self) -> String {
        let pass_emoji = if self.passes() { "✅" } else { "❌" };
        let p95_ms = self.metrics.p95_latency().as_secs_f64() * 1000.0;
        let p99_ms = self.metrics.p99_latency().as_secs_f64() * 1000.0;
        let max_ms = self.metrics.max_latency().as_secs_f64() * 1000.0;

        format!(
            r#"# Load Test Report: {}

**Status**: {} {}

---

## Summary

- **Duration**: {:.1} seconds
- **Total Requests**: {}
- **Successful**: {}
- **Failed**: {}
- **Error Rate**: {:.4}%

---

## Latency

| Percentile | Latency | Target | Status |
|------------|---------|--------|--------|
| P50 | {:.2}ms | - | - |
| P95 | {:.2}ms | <{:.2}ms | {} |
| P99 | {:.2}ms | {} | {} |
| Max | {:.2}ms | - | - |

---

## Throughput

- **Average**: {:.1} QPS
- **Target**: >{:.0} QPS
- **Status**: {}

---

## Resource Utilization

### Memory

- **Average**: {:.1} MB
- **Peak**: {:.1} MB
- **Growth**: {:.2} MB/min
- **Target**: <{:.2} MB/min
- **Status**: {}

### CPU

- **Average**: {:.1}%
- **Peak**: {:.1}%
- **Target**: <{:.1}%
- **Status**: {}

---

## Success Criteria

{}

---

## Errors

{}

---

**Report Generated**: {}
"#,
            self.scenario_name,
            pass_emoji,
            if self.passes() { "PASSED" } else { "FAILED" },
            self.metrics.duration().as_secs_f64(),
            self.metrics.total_requests,
            self.metrics.successful_requests,
            self.metrics.failed_requests,
            self.metrics.error_rate() * 100.0,
            self.metrics.p50_latency().as_secs_f64() * 1000.0,
            p95_ms,
            self.success_criteria.max_p95_latency_ms,
            if p95_ms <= self.success_criteria.max_p95_latency_ms {
                "✅"
            } else {
                "❌"
            },
            p99_ms,
            self.success_criteria
                .max_p99_latency_ms
                .map(|m| format!("<{:.2}ms", m))
                .unwrap_or_else(|| "-".to_string()),
            if let Some(max_p99) = self.success_criteria.max_p99_latency_ms {
                if p99_ms <= max_p99 {
                    "✅"
                } else {
                    "❌"
                }
            } else {
                "-"
            },
            max_ms,
            self.metrics.throughput_qps(),
            self.success_criteria.min_throughput_qps.unwrap_or(0.0),
            if let Some(min_qps) = self.success_criteria.min_throughput_qps {
                if self.metrics.throughput_qps() >= min_qps {
                    "✅"
                } else {
                    "❌"
                }
            } else {
                "✅"
            },
            self.metrics.avg_memory_mb(),
            self.metrics.peak_memory_mb(),
            self.metrics.memory_growth_mb_per_min(),
            self.success_criteria.max_memory_growth_mb_per_min,
            if self.metrics.memory_growth_mb_per_min()
                <= self.success_criteria.max_memory_growth_mb_per_min
            {
                "✅"
            } else {
                "❌"
            },
            self.metrics.avg_cpu_utilization() * 100.0,
            self.metrics.peak_cpu_utilization() * 100.0,
            self.success_criteria.max_cpu_utilization * 100.0,
            if self.metrics.avg_cpu_utilization() <= self.success_criteria.max_cpu_utilization {
                "✅"
            } else {
                "❌"
            },
            if self.passes() {
                "✅ **All criteria passed**".to_string()
            } else {
                format!("❌ **Failed criteria**:\n\n{}", self.failure_summary())
            },
            if self.metrics.errors.is_empty() {
                "No errors recorded".to_string()
            } else {
                format!(
                    "{} errors:\n\n```\n{}\n```",
                    self.metrics.errors.len(),
                    self.metrics.errors.join("\n")
                )
            },
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        )
    }

    /// Generate JSON report
    fn generate_json(&self) -> String {
        serde_json::json!({
            "scenario": self.scenario_name,
            "status": if self.passes() { "passed" } else { "failed" },
            "duration_seconds": self.metrics.duration().as_secs_f64(),
            "total_requests": self.metrics.total_requests,
            "successful_requests": self.metrics.successful_requests,
            "failed_requests": self.metrics.failed_requests,
            "error_rate": self.metrics.error_rate(),
            "latency_ms": {
                "p50": self.metrics.p50_latency().as_secs_f64() * 1000.0,
                "p90": self.metrics.p90_latency().as_secs_f64() * 1000.0,
                "p95": self.metrics.p95_latency().as_secs_f64() * 1000.0,
                "p99": self.metrics.p99_latency().as_secs_f64() * 1000.0,
                "max": self.metrics.max_latency().as_secs_f64() * 1000.0,
            },
            "throughput_qps": self.metrics.throughput_qps(),
            "memory_mb": {
                "average": self.metrics.avg_memory_mb(),
                "peak": self.metrics.peak_memory_mb(),
                "growth_per_min": self.metrics.memory_growth_mb_per_min(),
            },
            "cpu": {
                "average": self.metrics.avg_cpu_utilization(),
                "peak": self.metrics.peak_cpu_utilization(),
            },
            "success_criteria": {
                "passed": self.passes(),
                "failures": self.check_criteria(),
            },
            "errors": self.metrics.errors,
        })
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_passes_when_criteria_met() {
        let start = Instant::now();
        let metrics = LoadTestMetrics {
            start_time: start,
            end_time: start + std::time::Duration::from_secs(10),
            total_requests: 1000,
            successful_requests: 999,
            failed_requests: 1,
            latencies_us: vec![10000; 1000], // 10ms
            ..Default::default()
        };

        let criteria = SuccessCriteria::default();
        let writer = ResultWriter::new("Test".to_string(), metrics, criteria);

        assert!(writer.passes());
    }

    #[test]
    fn test_fails_when_criteria_not_met() {
        let start = Instant::now();
        let metrics = LoadTestMetrics {
            start_time: start,
            end_time: start + std::time::Duration::from_secs(10),
            total_requests: 100,
            successful_requests: 50,
            failed_requests: 50,
            latencies_us: vec![50000; 100], // 50ms (exceeds 25ms P95 target)
            ..Default::default()
        };

        let criteria = SuccessCriteria::default();
        let writer = ResultWriter::new("Test".to_string(), metrics, criteria);

        assert!(!writer.passes());
        assert!(writer.failure_summary().contains("Error rate"));
        assert!(writer.failure_summary().contains("P95 latency"));
    }
}
