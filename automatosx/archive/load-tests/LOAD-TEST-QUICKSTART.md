# AkiDB 2.0 - Load Test Quick Start Guide

**For**: Developers who want to run load tests immediately
**Time**: 15 minutes to first load test
**Prerequisites**: Running AkiDB instance, Rust toolchain

---

## TL;DR - Run Load Tests Now

```bash
# 1. Start AkiDB server
cargo run -p akidb-rest --release

# 2. Run 5-minute smoke test (in another terminal)
cargo test --release -p akidb-storage test_load_test_short_duration -- --nocapture

# 3. Run full 10-minute load test (optional)
cargo test --release -p akidb-storage test_load_test_full_10_min -- --ignored --nocapture
```

---

## What's Already Implemented

AkiDB already has a load testing framework in `crates/akidb-storage/tests/load_test.rs`:

### Current Features

✅ **Workload Mix**:
- 70% search operations
- 20% insert operations
- 10% tier control operations

✅ **Configurable Parameters**:
- Duration (5 sec to 10 min)
- QPS (10-1000+)
- Workload percentages

✅ **Metrics Collected**:
- Latency percentiles (P50, P95, P99)
- Error rate
- Total requests
- Success/failure counts

✅ **Test Variants**:
- `test_load_test_short_duration`: 5 seconds @ 10 QPS (CI-friendly)
- `test_load_test_full_10_min`: 10 minutes @ 100 QPS (full validation)

---

## Running Tests

### Quick Smoke Test (5 seconds)

```bash
# Fast validation - runs in CI
cargo test --release -p akidb-storage test_load_test_short_duration -- --nocapture

# Expected output:
# Starting load test: 10 QPS for 5s
# [5s] Requests: 50, P95: 15.3ms, Errors: 0.00%
# === Load Test Complete ===
# Total requests: 50
# Successful: 50
# Failed: 0
# Error rate: 0.00%
# P50 latency: 8.2ms
# P95 latency: 15.3ms
# P99 latency: 23.1ms
# test test_load_test_short_duration ... ok
```

### Full Load Test (10 minutes)

```bash
# Production validation - run manually
cargo test --release -p akidb-storage test_load_test_full_10_min -- --ignored --nocapture

# This will:
# - Run 100 QPS for 10 minutes
# - Generate ~60,000 requests
# - Report every 10 seconds
# - Validate P95 <25ms
# - Validate error rate <0.1%
```

### Custom Load Test

Create your own test configuration:

```rust
use akidb_storage::tests::load_test::{LoadTest, WorkloadConfig};
use std::time::Duration;

#[tokio::test]
async fn my_custom_load_test() {
    let config = WorkloadConfig {
        duration: Duration::from_secs(120), // 2 minutes
        qps: 50,
        search_pct: 0.8,  // 80% searches
        insert_pct: 0.15, // 15% inserts
        tier_control_pct: 0.05, // 5% tier ops
    };

    let load_test = LoadTest::new(config);
    let metrics = load_test.run().await;

    // Custom assertions
    assert!(metrics.p95() < Duration::from_millis(30));
    assert!(metrics.error_rate() < 0.005); // <0.5%
}
```

---

## Understanding Metrics

### Latency Percentiles

- **P50 (Median)**: Half of requests faster than this
- **P95**: 95% of requests faster than this (SLA target)
- **P99**: 99% of requests faster than this (worst case)

**Example**:
```
P50 latency: 8.2ms   ← Typical request
P95 latency: 21.3ms  ← 95th percentile (target: <25ms)
P99 latency: 45.2ms  ← Worst 1% of requests
```

### Error Rate

```
Error rate = (Failed requests / Total requests) * 100%

Target: <0.1% (1 in 1000 requests)
```

### Throughput

```
Throughput = Successful requests / Duration (seconds)

Example: 50 successful / 5 seconds = 10 QPS
```

---

## Troubleshooting

### "Connection refused" Error

**Problem**: AkiDB server not running

**Solution**:
```bash
# Start server in separate terminal
cargo run -p akidb-rest --release

# Wait for:
# "Server listening on 0.0.0.0:8080"
```

### High Latency (P95 >100ms)

**Possible Causes**:
1. Debug build (use `--release`)
2. CPU overload (reduce QPS)
3. Memory pressure (check available RAM)
4. Disk I/O bottleneck (use SSD)

**Quick Fix**:
```bash
# Use release build
cargo test --release ...

# Reduce QPS
let config = WorkloadConfig {
    qps: 10, // Lower QPS
    ...
};
```

### High Error Rate (>1%)

**Possible Causes**:
1. Server overloaded
2. Network issues
3. Database locked
4. S3/MinIO unavailable

**Debug Steps**:
```bash
# Check server logs
RUST_LOG=debug cargo run -p akidb-rest

# Check system resources
top -pid $(pgrep akidb-rest)
```

### Test Timeout

**Problem**: Test runs longer than expected

**Solution**:
```bash
# Increase timeout (default: 120s)
cargo test --release -- --test-threads=1 --nocapture

# Or reduce test duration
let config = WorkloadConfig {
    duration: Duration::from_secs(30), // Shorter
    ...
};
```

---

## Interpreting Results

### ✅ Good Results

```
Total requests: 60,000
Successful: 59,994
Failed: 6
Error rate: 0.01%    ← Excellent!
P50 latency: 8.2ms   ← Fast
P95 latency: 21.3ms  ← Below 25ms target
P99 latency: 45.2ms  ← Acceptable spikes
```

**Interpretation**: System performing well, ready for production

### ⚠️ Warning Signs

```
Error rate: 0.15%    ← Slightly high
P95 latency: 32.1ms  ← Above 25ms target
P99 latency: 150ms   ← Long tail latency
```

**Interpretation**: System stressed, investigate bottlenecks

### ❌ Problematic Results

```
Error rate: 5.2%     ← Too many failures!
P95 latency: 250ms   ← Way too slow
P99 latency: 5000ms  ← Completely broken
```

**Interpretation**: Critical issues, don't deploy

---

## Next Steps

### For CI/CD Integration

Add to `.github/workflows/test.yml`:

```yaml
- name: Run Load Test
  run: |
    # Start server in background
    cargo run -p akidb-rest --release &
    SERVER_PID=$!

    # Wait for server to start
    sleep 5

    # Run smoke test
    cargo test --release -p akidb-storage test_load_test_short_duration -- --nocapture

    # Cleanup
    kill $SERVER_PID
```

### For Local Development

Create a script `scripts/load-test.sh`:

```bash
#!/bin/bash
set -e

echo "🚀 Starting AkiDB load test..."

# Start server
echo "Starting server..."
cargo run -p akidb-rest --release &
SERVER_PID=$!
sleep 5

# Run load test
echo "Running load test..."
cargo test --release -p akidb-storage test_load_test_short_duration -- --nocapture

# Cleanup
echo "Cleaning up..."
kill $SERVER_PID

echo "✅ Load test complete!"
```

### For Production Validation

Before deploying to production:

1. **Run full 10-minute test**:
   ```bash
   cargo test --release test_load_test_full_10_min -- --ignored --nocapture
   ```

2. **Check metrics**:
   - P95 latency <25ms
   - Error rate <0.1%
   - No memory leaks

3. **Monitor during test**:
   ```bash
   # In separate terminal
   watch -n 1 'ps aux | grep akidb-rest'
   ```

4. **Review results**:
   - If all pass → ✅ Deploy
   - If any fail → ❌ Investigate first

---

## Advanced Usage

### Custom Workload Mix

```rust
let config = WorkloadConfig {
    search_pct: 0.9,   // 90% searches (read-heavy)
    insert_pct: 0.08,  // 8% inserts
    tier_control_pct: 0.02, // 2% tier ops
    ..Default::default()
};
```

### Spike Load Testing

```rust
// Baseline
let baseline = WorkloadConfig { qps: 50, ..Default::default() };
load_test.run(baseline).await;

// Spike
let spike = WorkloadConfig { qps: 500, ..Default::default() };
load_test.run(spike).await;

// Recovery
load_test.run(baseline).await;
```

### Long-Running Stability Test

```rust
let config = WorkloadConfig {
    duration: Duration::from_secs(3600), // 1 hour
    qps: 100,
    ..Default::default()
};

// Monitor memory growth
let start_mem = process_memory_mb();
load_test.run(config).await;
let end_mem = process_memory_mb();

assert!(end_mem - start_mem < 100, "Memory leak detected!");
```

---

## Performance Baselines

Based on current benchmarks:

| Metric | Value | Notes |
|--------|-------|-------|
| **P95 Latency** | ~21ms | @ 100 QPS, 10k vectors |
| **P99 Latency** | ~45ms | Acceptable spikes |
| **Throughput** | 100 QPS | Sustained over 10 min |
| **Error Rate** | <0.01% | Excellent reliability |
| **Memory** | Stable | No leaks observed |

**Hardware**: M1/M2 Mac (ARM64), 16GB RAM

---

## FAQs

**Q: How long should I run load tests?**
A: 5 seconds for CI, 10 minutes for validation, 1 hour for stability

**Q: What QPS should I target?**
A: Start with 10 QPS, increase to 100 QPS for production validation

**Q: What's an acceptable error rate?**
A: <0.1% is excellent, <1% is acceptable, >1% needs investigation

**Q: How do I test with real data?**
A: Modify `load_test.rs` to use real vectors instead of random ones

**Q: Can I test against production?**
A: Never! Always test against staging or local instances

**Q: How do I test S3 integration?**
A: Start MinIO with `docker-compose up -d minio` first

---

## Summary

✅ **Quick Start**: `cargo test --release test_load_test_short_duration`
✅ **Full Validation**: `cargo test --release test_load_test_full_10_min --ignored`
✅ **Custom Tests**: Modify `WorkloadConfig` in `load_test.rs`
✅ **CI Integration**: Add smoke test to GitHub Actions
✅ **Target Metrics**: P95 <25ms, Error rate <0.1%

**Happy Load Testing! 🚀**

---

**See Also**:
- Full design: `automatosx/tmp/LOAD-TEST-DESIGN.md`
- Implementation: `crates/akidb-storage/tests/load_test.rs`
- Benchmark results: `automatosx/tmp/TEST-BENCHMARK-REPORT.md`

