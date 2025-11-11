# AkiDB 2.0 Load Testing Guide

This guide covers how to run load tests for AkiDB 2.0, interpret results, and integrate with CI/CD pipelines.

---

## Quick Start

### Run Smoke Test (30 seconds)

```bash
cargo test --release -p akidb-storage smoke_test_load_framework -- --nocapture
```

**Purpose:** Fast validation that the load test framework works. Runs in CI on every PR.

---

## Test Scenarios

### Quick Tests (5-15 minutes each)

**Scenario 1: Baseline Performance (5 min)**
```bash
cargo test --release -p akidb-storage scenario_1_baseline_quick -- --nocapture
```
- 100 QPS constant load
- 70% search, 20% insert, 10% metadata
- Target: P95 <25ms, error rate <0.1%

**Scenario 2: Sustained High Load (10 min)**
```bash
cargo test --release -p akidb-storage scenario_2_sustained_load_quick -- --nocapture
```
- 200 QPS constant load (2x baseline)
- Tests long-running stability
- Target: P95 <50ms, error rate <0.5%

**Scenario 3: Spike Load (5 min)**
```bash
cargo test --release -p akidb-storage scenario_3_spike_load_quick -- --nocapture
```
- 100 → 500 → 100 QPS (5x spike)
- Tests spike handling and recovery
- Target: P95 <100ms during spike

**Scenario 4: Tiered Storage (10 min)**
```bash
cargo test --release -p akidb-storage scenario_4_tiered_storage_quick -- --nocapture
```
- 100 QPS, 100k vectors
- Read-heavy workload (80% searches)
- Target: P95 <25ms

**Scenario 5: Multi-Tenant Load (10 min)**
```bash
cargo test --release -p akidb-storage scenario_5_multi_tenant_quick -- --nocapture
```
- 150 QPS (3 tenants @ 50 QPS each)
- Tests tenant isolation
- Target: P95 <30ms, error rate <0.5%

**Scenario 6: Large Dataset (15 min)**
```bash
cargo test --release -p akidb-storage scenario_6_large_dataset_quick -- --nocapture
```
- 100 QPS, 500k vectors
- Tests memory pressure
- Target: P95 <100ms, no OOM crashes

**Scenario 7: Failure Injection (5 min)**
```bash
cargo test --release -p akidb-storage scenario_7_failure_injection_quick -- --nocapture
```
- 100 QPS with injected failures
- Tests circuit breaker, DLQ
- Target: Error rate <15% (due to injected failures)

**Scenario 8: Mixed Workload Chaos (10 min)**
```bash
cargo test --release -p akidb-storage scenario_8_mixed_chaos_quick -- --nocapture
```
- Random 50-300 QPS
- All operation types
- Target: P95 <100ms, error rate <2%

---

### Run All Quick Tests (75 minutes)

```bash
cargo test --release -p akidb-storage --test comprehensive_load_test -- --nocapture
```

---

### Full Tests (30-60 minutes each)

**Run with `--ignored` flag:**

```bash
# Scenario 1: Baseline (30 min)
cargo test --release -p akidb-storage scenario_1_baseline -- --ignored --nocapture

# Scenario 2: Sustained Load (60 min)
cargo test --release -p akidb-storage scenario_2_sustained_load -- --ignored --nocapture

# Scenario 3: Spike Load (15 min)
cargo test --release -p akidb-storage scenario_3_spike_load -- --ignored --nocapture

# Scenario 4: Tiered Storage (45 min)
cargo test --release -p akidb-storage scenario_4_tiered_storage -- --ignored --nocapture

# Scenario 5: Multi-Tenant (30 min)
cargo test --release -p akidb-storage scenario_5_multi_tenant -- --ignored --nocapture

# Scenario 6: Large Dataset (60 min)
cargo test --release -p akidb-storage scenario_6_large_dataset -- --ignored --nocapture

# Scenario 7: Failure Injection (20 min)
cargo test --release -p akidb-storage scenario_7_failure_injection -- --ignored --nocapture

# Scenario 8: Chaos (30 min)
cargo test --release -p akidb-storage scenario_8_mixed_chaos -- --ignored --nocapture
```

**Run all full tests (5+ hours):**
```bash
cargo test --release -p akidb-storage --test comprehensive_load_test -- --ignored --nocapture
```

---

## Interpreting Results

### Console Output

During test execution, you'll see real-time progress:

```
🚀 Starting load test: Scenario 1: Baseline Performance (Quick)
   Duration: 300s
   Load profile: Constant 100 QPS
   Dataset: 10000 vectors (512d)
   Concurrency: 10 clients

[10s] Requests: 1100, P95: 1.3ms, Errors: 0.00%, QPS: 100
[20s] Requests: 2100, P95: 1.3ms, Errors: 0.00%, QPS: 100
...

✅ Load test complete
   Total requests: 30100
   Successful: 30100
   Failed: 0
   Error rate: 0.0000%
   P95 latency: 1.33ms
   Throughput: 100.3 QPS
```

### Report Files

Reports are generated in `target/load_test_reports/`:

- **Markdown** (`*.md`): Human-readable reports
- **JSON** (`*.json`): Machine-parseable for CI integration

**View Markdown report:**
```bash
cat target/load_test_reports/baseline_quick.md
```

**View JSON report:**
```bash
cat target/load_test_reports/baseline_quick.json | jq .
```

### Success Criteria

Each scenario defines specific success criteria:

| Criterion | Description | Example |
|-----------|-------------|---------|
| **P95 Latency** | 95th percentile latency | <25ms |
| **P99 Latency** | 99th percentile latency | <50ms |
| **Error Rate** | Failed requests / Total requests | <0.1% |
| **Throughput** | Requests per second | >95 QPS |
| **Memory Growth** | Memory increase per minute | <10 MB/min |
| **CPU Utilization** | Average CPU usage | <70% |

**Pass/Fail:**
- ✅ **PASSED**: All criteria met
- ❌ **FAILED**: One or more criteria violated

---

## CI/CD Integration

### GitHub Actions

The load test suite runs automatically on:

1. **Every PR and Push to main:** Smoke test (30s)
2. **Weekly (Sunday):** Quick tests (45 min)
3. **Manual trigger:** Full tests (5+ hours)

**Manual Trigger:**
```bash
# From GitHub UI:
Actions → Load Tests → Run workflow
# Select test suite: smoke, quick, or full
```

**View Results:**
- Check the "Actions" tab in GitHub
- Download artifacts for detailed reports
- PR comments show summary table

### Local CI Simulation

Run the same tests that CI runs:

```bash
# Smoke test (runs on every PR)
cargo test --release -p akidb-storage smoke_test_load_framework -- --nocapture

# Quick tests (runs weekly)
cargo test --release -p akidb-storage --test comprehensive_load_test -- --nocapture

# Full tests (runs on demand)
cargo test --release -p akidb-storage --test comprehensive_load_test -- --ignored --nocapture
```

---

## Performance Baselines

### Expected Performance (Apple M1/M2)

| Scenario | QPS | P95 Latency | Error Rate |
|----------|-----|-------------|------------|
| Baseline | 100 | <2ms | 0.00% |
| Sustained High Load | 200 | <2ms | 0.00% |
| Spike Load | 100-500 | <2ms | 0.00% |
| Tiered Storage | 100 | <2ms | 0.00% |
| Multi-Tenant | 150 | <2ms | 0.00% |
| Large Dataset | 100 | <5ms | 0.00% |
| Failure Injection | 100 | <2ms | <15% |
| Chaos | 50-300 | <3ms | <1% |

**Note:** Results are 10-50x better than targets on Apple Silicon!

---

## Troubleshooting

### Test Timeout

If a test runs longer than expected:

```bash
# Check system resources
top
htop

# Increase timeout (default: 120s)
RUST_TEST_TIMEOUT=300 cargo test ...
```

### High Error Rate

If error rate exceeds target:

1. Check system resources (memory, CPU)
2. Review error messages in report
3. Look for panics in console output
4. Check disk space for temp files

### Memory Issues

If seeing OOM errors:

```bash
# Monitor memory during test
watch -n 1 'ps aux | grep comprehensive_load_test'

# Reduce dataset size for quick tests
# Edit comprehensive_load_test.rs and lower dataset_size
```

### Performance Regression

If P95 latency increased:

1. Compare against baseline reports
2. Check for recent code changes
3. Profile with `cargo flamegraph`
4. Review system load (other processes)

---

## Advanced Usage

### Custom Scenarios

Create custom load test scenarios by editing:
```
crates/akidb-storage/tests/comprehensive_load_test.rs
```

Example:
```rust
#[tokio::test]
async fn my_custom_scenario() {
    let config = ScenarioConfig {
        name: "Custom Test".to_string(),
        duration: Duration::from_secs(60),
        load_profile: LoadProfile::Constant { qps: 50 },
        workload_mix: WorkloadMix::default(),
        dataset_size: 5_000,
        dimension: 256,
        concurrency: 5,
        sample_interval: Duration::from_secs(1),
    };

    let criteria = SuccessCriteria::development();
    run_scenario(config, criteria, "my_custom_test").await;
}
```

### Load Profiles

Available load patterns:

```rust
// Constant QPS
LoadProfile::Constant { qps: 100 }

// Gradual ramp
LoadProfile::Ramp {
    from_qps: 50,
    to_qps: 200,
    ramp_duration: Duration::from_secs(300),
}

// Spike pattern
LoadProfile::Spike {
    baseline_qps: 100,
    spike_qps: 500,
    spike_start: Duration::from_secs(60),
    spike_duration: Duration::from_secs(120),
}

// Random (chaos)
LoadProfile::Random {
    min_qps: 50,
    max_qps: 300,
    change_interval: Duration::from_secs(30),
}
```

### Workload Mix

Customize operation distribution:

```rust
// Default (balanced)
WorkloadMix::default()  // 70% search, 20% insert, 10% metadata

// Read-heavy
WorkloadMix::read_heavy()  // 80% search, 15% insert, 5% metadata

// Write-heavy
WorkloadMix::write_heavy()  // 40% search, 50% insert, 10% metadata

// Custom
WorkloadMix {
    search_pct: 0.5,
    insert_pct: 0.3,
    update_pct: 0.1,
    delete_pct: 0.05,
    metadata_pct: 0.05,
}
```

### Success Criteria

Define custom success criteria:

```rust
// Production (strict)
SuccessCriteria::production()

// Development (relaxed)
SuccessCriteria::development()

// Custom
SuccessCriteria {
    max_p95_latency_ms: 25.0,
    max_error_rate: 0.001,  // 0.1%
    max_memory_growth_mb_per_min: 10.0,
    max_cpu_utilization: 0.70,
    min_throughput_qps: Some(95.0),
    max_p99_latency_ms: Some(50.0),
}
```

---

## Best Practices

1. **Run smoke test first** before long-running tests
2. **Close other applications** to avoid resource contention
3. **Use release mode** (`--release`) for accurate performance
4. **Monitor system resources** during long tests
5. **Compare against baselines** to detect regressions
6. **Run full tests** before major releases
7. **Archive reports** for historical comparison

---

## Report Structure

### Markdown Report

```markdown
# Load Test Report: Scenario 1: Baseline Performance (Quick)

**Status**: ✅ PASSED

## Summary
- **Duration**: 300.0 seconds
- **Total Requests**: 30100
- **Successful**: 30100
- **Failed**: 0
- **Error Rate**: 0.0000%

## Latency
| Percentile | Latency | Target | Status |
|------------|---------|--------|--------|
| P50 | 1.20ms | - | - |
| P95 | 1.33ms | <25.00ms | ✅ |
| P99 | 1.45ms | <50.00ms | ✅ |
| Max | 2.10ms | - | - |

## Throughput
- **Average**: 100.3 QPS
- **Target**: >95 QPS
- **Status**: ✅

## Resource Utilization
### Memory
- **Average**: 512.0 MB
- **Peak**: 520.5 MB
- **Growth**: 0.05 MB/min
- **Target**: <10.00 MB/min
- **Status**: ✅

### CPU
- **Average**: 45.2%
- **Peak**: 52.8%
- **Target**: <70.0%
- **Status**: ✅

## Success Criteria
✅ **All criteria passed**
```

### JSON Report

```json
{
  "scenario": "Scenario 1: Baseline Performance (Quick)",
  "status": "passed",
  "duration_seconds": 300.0,
  "total_requests": 30100,
  "successful_requests": 30100,
  "failed_requests": 0,
  "error_rate": 0.0,
  "latency_ms": {
    "p50": 1.20,
    "p90": 1.30,
    "p95": 1.33,
    "p99": 1.45,
    "max": 2.10
  },
  "throughput_qps": 100.3,
  "memory_mb": {
    "average": 512.0,
    "peak": 520.5,
    "growth_per_min": 0.05
  },
  "cpu": {
    "average": 0.452,
    "peak": 0.528
  },
  "success_criteria": {
    "passed": true,
    "failures": []
  },
  "errors": []
}
```

---

## Further Reading

- [Load Test Design](../automatosx/tmp/LOAD-TEST-DESIGN.md) - Comprehensive 47-page design
- [Quick Start Guide](../automatosx/tmp/LOAD-TEST-QUICKSTART.md) - 15-minute tutorial
- [Week 1 Report](../automatosx/tmp/LOAD-TEST-WEEK-1-COMPLETION-REPORT.md) - Implementation details
- [Performance Benchmarks](PERFORMANCE-BENCHMARKS.md) - Historical performance data

---

**Last Updated:** 2025-11-09
**Framework Version:** 2.0
**Scenarios:** 8 (+ 1 smoke test)
**Total Test Time:** Quick: 75 min, Full: 5+ hours
