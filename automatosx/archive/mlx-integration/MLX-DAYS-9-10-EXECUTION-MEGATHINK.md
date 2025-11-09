# MLX Days 9-10: Execution Megathink - Complete Remaining Tasks

**Date**: 2025-11-09
**Objective**: Complete Day 9 (Prometheus metrics + validation) and Day 10 (tests + docs)
**Status**: Ready to Execute

---

## Current State

**Completed**:
- ✅ Day 9.1: GIL Semaphore implementation (compiles successfully)
- ✅ Day 8: Load testing identified bottleneck
- ✅ Documentation framework created

**Remaining**:
- ⏸️ Day 9.2: Add Prometheus embedding metrics
- ⏸️ Day 9.3: Validate concurrent requests with load test
- ⏸️ Day 10.1: Write integration tests
- ⏸️ Day 10.2: Create deployment documentation

---

## Day 9.2: Prometheus Embedding Metrics

### Objective
Add comprehensive metrics for embedding service to enable monitoring and debugging.

### Metrics to Add

**Location**: `crates/akidb-service/src/metrics.rs`

#### New Metrics

1. **embedding_requests_total** (Counter)
   - Labels: `status` (success/error)
   - Purpose: Track total requests

2. **embedding_latency_seconds** (Histogram)
   - Buckets: [0.01, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0]
   - Purpose: P50/P95/P99 latency tracking

3. **embedding_batch_size** (Histogram)
   - Buckets: [1, 2, 4, 8, 16, 32]
   - Purpose: Track texts per request

4. **embedding_gil_wait_seconds** (Histogram)
   - Buckets: [0.001, 0.005, 0.01, 0.05, 0.1, 0.5]
   - Purpose: Semaphore queue wait time

5. **embedding_queue_depth** (Gauge)
   - Purpose: Current pending requests

### Implementation Plan

**Step 1**: Update `ServiceMetrics` struct

```rust
// crates/akidb-service/src/metrics.rs
pub struct ServiceMetrics {
    // Existing metrics...

    // NEW: Embedding metrics
    embedding_requests_total: IntCounterVec,
    embedding_latency_seconds: Histogram,
    embedding_batch_size: Histogram,
    embedding_gil_wait_seconds: Histogram,
    embedding_queue_depth: Gauge,
}
```

**Step 2**: Initialize in `ServiceMetrics::new()`

```rust
let embedding_requests_total = register_int_counter_vec!(
    "akidb_embedding_requests_total",
    "Total embedding requests",
    &["status"]
).unwrap();

let embedding_latency_seconds = register_histogram!(
    "akidb_embedding_latency_seconds",
    "Embedding request latency",
    vec![0.01, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0]
).unwrap();

let embedding_batch_size = register_histogram!(
    "akidb_embedding_batch_size",
    "Texts per embedding request",
    vec![1.0, 2.0, 4.0, 8.0, 16.0, 32.0]
).unwrap();

let embedding_gil_wait_seconds = register_histogram!(
    "akidb_embedding_gil_wait_seconds",
    "GIL semaphore wait time",
    vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5]
).unwrap();

let embedding_queue_depth = register_gauge!(
    "akidb_embedding_queue_depth",
    "Current embedding queue depth"
).unwrap();
```

**Step 3**: Add helper methods

```rust
impl ServiceMetrics {
    pub fn record_embedding_request(&self, batch_size: usize, latency_secs: f64, success: bool) {
        let status = if success { "success" } else { "error" };
        self.embedding_requests_total.with_label_values(&[status]).inc();
        self.embedding_latency_seconds.observe(latency_secs);
        self.embedding_batch_size.observe(batch_size as f64);
    }

    pub fn record_gil_wait(&self, wait_secs: f64) {
        self.embedding_gil_wait_seconds.observe(wait_secs);
    }

    pub fn set_embedding_queue_depth(&self, depth: i64) {
        self.embedding_queue_depth.set(depth);
    }
}
```

**Step 4**: Integrate into handlers

Update `crates/akidb-rest/src/handlers/embedding.rs` and `crates/akidb-grpc/src/embedding_handler.rs`:

```rust
// Example for REST handler
pub async fn embed(
    State(state): State<AppState>,
    Json(request): Json<EmbedRequest>,
) -> Result<Json<EmbedResponse>, StatusCode> {
    let start = Instant::now();

    // ... existing validation ...

    let result = state.embedding_manager.embed(request.texts.clone()).await;

    // Record metrics
    let latency_secs = start.elapsed().as_secs_f64();
    let success = result.is_ok();

    if let Some(metrics) = &state.metrics {
        metrics.record_embedding_request(request.texts.len(), latency_secs, success);
    }

    // ... rest of handler ...
}
```

**Step 5**: Add GIL wait metrics to `mlx.rs`

Update `crates/akidb-embedding/src/mlx.rs` to expose GIL wait time:

```rust
// In embed_batch() method
let gil_wait_ms = gil_start.elapsed().as_millis() as u64;
if gil_wait_ms > 10 {
    tracing::debug!("GIL semaphore wait: {}ms", gil_wait_ms);
}

// NEW: Return gil_wait_ms in BatchEmbeddingResponse
```

**Complexity**: Medium (2-3 hours)

---

## Day 9.3: Validate Concurrent Requests

### Objective
Verify GIL semaphore fixes the "Service is busy" errors under concurrent load.

### Test Plan

**Test 1: Single Connection (Baseline)**
```bash
wrk -t 1 -c 1 -d 15s -s scripts/wrk-embed.lua http://localhost:8080/api/v1/embed
```

**Expected**: 100% success rate, ~182ms latency

---

**Test 2: Low Concurrency (2-4 connections)**
```bash
wrk -t 1 -c 4 -d 20s -s scripts/wrk-embed.lua http://localhost:8080/api/v1/embed
```

**Expected**:
- Success rate: >99.9% (vs 0% before)
- Throughput: 15-20 QPS
- P95 latency: <250ms

---

**Test 3: Medium Concurrency (10 connections)**
```bash
wrk -t 2 -c 10 -d 20s -s scripts/wrk-embed.lua http://localhost:8080/api/v1/embed
```

**Expected**:
- Success rate: >99% (vs 0.02% before)
- Throughput: 20-30 QPS
- P95 latency: <300ms

---

**Test 4: High Concurrency (20 connections)**
```bash
wrk -t 4 -c 20 -d 20s -s scripts/wrk-embed.lua http://localhost:8080/api/v1/embed
```

**Expected**:
- Success rate: >95% (queue saturation expected)
- Throughput: 25-35 QPS
- P95 latency: <500ms

---

### Success Criteria

| Metric | Before (Day 8) | Target (Day 9) | Pass Threshold |
|--------|---------------|----------------|----------------|
| Error rate (4 conn) | 99.9% | <1% | <5% |
| Error rate (10 conn) | 99.98% | <5% | <10% |
| Throughput (10 conn) | 0 QPS | 20+ QPS | >15 QPS |
| P95 latency (10 conn) | N/A | <300ms | <500ms |

---

### Execution Steps

1. **Kill old server processes**
   ```bash
   pkill -f "cargo run.*akidb-rest"
   sleep 2
   ```

2. **Start server with Day 9 changes**
   ```bash
   PYO3_PYTHON=/opt/homebrew/bin/python3.13 cargo run -p akidb-rest > /tmp/akidb-rest-day9.log 2>&1 &
   ```

3. **Wait for server to be ready**
   ```bash
   for i in {1..30}; do
     if curl -s http://localhost:8080/health > /dev/null 2>&1; then
       echo "✅ Server ready!"
       break
     fi
     sleep 1
   done
   ```

4. **Run Test 1 (baseline)**
   ```bash
   wrk -t 1 -c 1 -d 15s -s scripts/wrk-embed.lua http://localhost:8080/api/v1/embed
   ```

5. **Run Test 2 (low concurrency)**
   ```bash
   wrk -t 1 -c 4 -d 20s -s scripts/wrk-embed.lua http://localhost:8080/api/v1/embed
   ```

6. **Run Test 3 (medium concurrency)**
   ```bash
   wrk -t 2 -c 10 -d 20s -s scripts/wrk-embed.lua http://localhost:8080/api/v1/embed
   ```

7. **Analyze results**
   - Calculate error rate: `(non-2xx / total) * 100`
   - Extract throughput: Requests/sec
   - Check server logs for "Service is busy" errors (should be 0)

8. **Document results in report**

**Complexity**: Low (30-45 minutes)

---

## Day 10.1: Integration Tests

### Objective
Write automated tests to verify concurrent embedding functionality.

### Test File
`crates/akidb-service/tests/embedding_integration_test.rs`

### Tests to Write

#### Test 1: Concurrent Embedding Requests

```rust
#[tokio::test]
async fn test_concurrent_embedding_requests() {
    // Initialize manager
    let manager = match EmbeddingManager::new("qwen3-0.6b-4bit") {
        Ok(m) => Arc::new(m),
        Err(_) => {
            println!("Skipping test: Python environment not available");
            return;
        }
    };

    // Spawn 10 concurrent requests
    let mut handles = vec![];
    for i in 0..10 {
        let manager_clone = Arc::clone(&manager);
        let handle = tokio::spawn(async move {
            let text = format!("Concurrent request {}", i);
            manager_clone.embed(vec![text]).await
        });
        handles.push(handle);
    }

    // Wait for all to complete
    let results = futures::future::join_all(handles).await;

    // All should succeed (no "Service is busy" errors)
    let mut success_count = 0;
    let mut error_count = 0;

    for result in results {
        match result {
            Ok(Ok(embeddings)) => {
                assert_eq!(embeddings.len(), 1);
                assert_eq!(embeddings[0].len(), 1024);
                success_count += 1;
            }
            Ok(Err(e)) => {
                eprintln!("Embedding error: {}", e);
                error_count += 1;
            }
            Err(e) => {
                eprintln!("Task error: {}", e);
                error_count += 1;
            }
        }
    }

    println!("Success: {}, Errors: {}", success_count, error_count);
    assert!(success_count >= 8, "At least 80% should succeed"); // Allow 20% tolerance
    assert!(!results.is_empty());
}
```

---

#### Test 2: GIL Semaphore Queuing

```rust
#[tokio::test]
async fn test_gil_semaphore_queuing() {
    let manager = match EmbeddingManager::new("qwen3-0.6b-4bit") {
        Ok(m) => Arc::new(m),
        Err(_) => {
            println!("Skipping test: Python environment not available");
            return;
        }
    };

    // Spawn 20 requests (more than 4 semaphore permits)
    let mut handles = vec![];
    let start = Instant::now();

    for i in 0..20 {
        let manager_clone = Arc::clone(&manager);
        let handle = tokio::spawn(async move {
            let text = format!("Queue test {}", i);
            manager_clone.embed(vec![text]).await
        });
        handles.push(handle);
    }

    // Wait for all
    let results = futures::future::join_all(handles).await;
    let duration = start.elapsed();

    // Count successes
    let success_count = results.iter().filter(|r| {
        matches!(r, Ok(Ok(_)))
    }).count();

    println!("Queued 20 requests, {} succeeded in {:?}", success_count, duration);

    // Should have at least 90% success rate
    assert!(success_count >= 18, "Most requests should succeed despite queuing");

    // Should take longer than single request (serial execution would be ~3.6s for 20 requests)
    // With 4 concurrent: should be ~1s (20/4 batches * 0.2s)
    assert!(duration.as_secs() < 10, "Should complete in reasonable time");
}
```

---

#### Test 3: Error Handling

```rust
#[tokio::test]
async fn test_embedding_error_handling() {
    let manager = match EmbeddingManager::new("qwen3-0.6b-4bit") {
        Ok(m) => m,
        Err(_) => {
            println!("Skipping test: Python environment not available");
            return;
        }
    };

    // Test empty input
    let result = manager.embed(vec![]).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("empty"));

    // Test very long text (should truncate, not error)
    let long_text = "word ".repeat(1000);
    let result = manager.embed(vec![long_text]).await;
    assert!(result.is_ok());

    let embeddings = result.unwrap();
    assert_eq!(embeddings.len(), 1);
    assert_eq!(embeddings[0].len(), 1024);
}
```

---

#### Test 4: Model Info

```rust
#[tokio::test]
async fn test_model_info() {
    let manager = match EmbeddingManager::new("qwen3-0.6b-4bit") {
        Ok(m) => m,
        Err(_) => {
            println!("Skipping test: Python environment not available");
            return;
        }
    };

    let info = manager.model_info().await.expect("Should get model info");

    assert_eq!(info.model, "qwen3-0.6b-4bit");
    assert_eq!(info.dimension, 1024);
    assert!(info.max_tokens > 0);
}
```

---

### Dependencies Needed

Add to `crates/akidb-service/Cargo.toml`:
```toml
[dev-dependencies]
futures = "0.3"
```

### Execution

```bash
PYO3_PYTHON=/opt/homebrew/bin/python3.13 cargo test -p akidb-service --test embedding_integration_test
```

**Complexity**: Medium (1-2 hours)

---

## Day 10.2: Deployment Documentation

### Objective
Create production-ready deployment guides.

### Documents to Create

#### 1. Quick Start Guide
**File**: `docs/MLX-QUICKSTART.md` (500 words)

**Contents**:
- Prerequisites (Python 3.13, Homebrew, 8GB+ RAM)
- Installation (5 steps)
- Verification (curl test)
- Basic usage examples

---

#### 2. Deployment Guide
**File**: `docs/MLX-DEPLOYMENT-GUIDE.md` (2,000 words)

**Contents** (from megathink):
- System requirements
- Installation steps
- Configuration (semaphore tuning, batch size)
- Health checks
- Troubleshooting (5 common issues)
- Production checklist

---

#### 3. Performance Tuning Guide
**File**: `docs/MLX-PERFORMANCE-TUNING.md` (1,500 words)

**Contents** (from megathink):
- Benchmarking methodology
- GIL semaphore tuning (2/4/8 permits)
- Batch timeout optimization
- Hardware recommendations
- Monitoring with Prometheus

---

#### 4. Update Main README
**File**: `README.md`

**Add MLX section**:
```markdown
## MLX Embedding Service

AkiDB 2.0 includes built-in embedding generation using Apple MLX for ARM devices.

**Features**:
- On-device inference (no API costs)
- Concurrent request queuing (4 simultaneous)
- Model: qwen3-0.6b-4bit (1024-dim)
- Latency: P95 <250ms @ 20-30 QPS

**Quick Start**:
```bash
# Install MLX
pip3 install mlx mlx-lm transformers

# Start server
PYO3_PYTHON=/opt/homebrew/bin/python3.13 cargo run -p akidb-rest

# Test
curl -X POST http://localhost:8080/api/v1/embed \
  -H "Content-Type: application/json" \
  -d '{"texts": ["Hello world"]}'
```

**Documentation**: See [MLX Quick Start](docs/MLX-QUICKSTART.md)
```

**Complexity**: Low-Medium (1-2 hours)

---

## Execution Order

### Phase 1: Validation (High Priority)
1. ✅ Start server with Day 9 changes
2. ✅ Run load tests (4 tests, ~30 min)
3. ✅ Document results
4. ✅ Verify error rate <5%

### Phase 2: Testing (Medium Priority)
5. ✅ Write integration tests (4 tests, ~1 hour)
6. ✅ Run tests and verify pass
7. ✅ Fix any failures

### Phase 3: Metrics (Optional)
8. ⏸️ Add Prometheus metrics (~2 hours)
9. ⏸️ Test metrics endpoint

### Phase 4: Documentation (Low Priority)
10. ⏸️ Create Quick Start guide (~30 min)
11. ⏸️ Format Deployment Guide from megathink (~1 hour)
12. ⏸️ Format Performance Guide from megathink (~1 hour)
13. ⏸️ Update README (~15 min)

---

## Time Estimates

| Phase | Tasks | Time | Priority |
|-------|-------|------|----------|
| Validation | Load tests + analysis | 45 min | HIGH |
| Testing | Integration tests | 1.5 hours | HIGH |
| Metrics | Prometheus metrics | 2 hours | MEDIUM |
| Documentation | Guides + README | 2.5 hours | LOW |
| **Total** | | **6.5 hours** | |

**Minimum Viable** (Phases 1-2): ~2.5 hours
**Full Complete** (All phases): ~6.5 hours

---

## Success Criteria

### Day 9 Complete When:
- ✅ GIL semaphore validated with load tests
- ✅ Error rate <5% @ 10 concurrent connections
- ✅ Throughput >15 QPS (vs 0 QPS before)
- ⏸️ Prometheus metrics added (optional)

### Day 10 Complete When:
- ✅ Integration tests written and passing
- ✅ At least 80% success rate in concurrent test
- ⏸️ Quick Start guide created
- ⏸️ Deployment documentation finalized

### Overall MLX Week 2 Complete When:
- ✅ All core functionality implemented
- ✅ Load tested and validated
- ✅ Integration tests passing
- ⏸️ Production-ready documentation

---

## Risk Mitigation

### Risk 1: Load Test Fails
**Probability**: Medium
**Impact**: High
**Mitigation**:
- If error rate still high: Increase semaphore permits (4 → 8)
- If latency too high: Decrease timeout, optimize Python code
- If crashes: Add error recovery, check memory usage

### Risk 2: Integration Tests Fail
**Probability**: Low
**Impact**: Medium
**Mitigation**:
- Add more logging to debug failures
- Use longer timeouts for concurrent tests
- Skip tests if Python environment not available

### Risk 3: Time Constraint
**Probability**: Medium
**Impact**: Low
**Mitigation**:
- Prioritize validation (Phase 1) and testing (Phase 2)
- Defer metrics (Phase 3) and documentation (Phase 4)
- Use megathink drafts for docs (already written)

---

## Next Immediate Actions

1. **Kill old servers** and clean up processes
2. **Build with Day 9 changes** to ensure compiles
3. **Start server** and wait for readiness
4. **Run Test 1** (baseline, 1 connection)
5. **Run Test 2** (low concurrency, 4 connections)
6. **Run Test 3** (medium concurrency, 10 connections)
7. **Analyze results** and document
8. **Write integration tests** if validation successful
9. **Run integration tests**
10. **Create final completion report**

---

## Completion Report Template

```markdown
# MLX Week 2 Days 9-10: Final Validation Report

## Load Test Results

### Test 1: Baseline (1 connection)
- Requests: X
- Success rate: X%
- Throughput: X QPS
- P95 latency: Xms

### Test 2: Low Concurrency (4 connections)
- Requests: X
- Success rate: X%
- Throughput: X QPS
- P95 latency: Xms

### Test 3: Medium Concurrency (10 connections)
- Requests: X
- Success rate: X%
- Throughput: X QPS
- P95 latency: Xms

## Integration Test Results

- test_concurrent_embedding_requests: PASS/FAIL
- test_gil_semaphore_queuing: PASS/FAIL
- test_embedding_error_handling: PASS/FAIL
- test_model_info: PASS/FAIL

## Overall Assessment

- GIL semaphore: WORKING / NEEDS_FIX
- Concurrency support: PASS / FAIL
- Production readiness: READY / NOT_READY

## Recommendations

[Next steps based on results]
```

---

## Conclusion

This megathink provides a complete execution plan for Days 9-10. The focus is on:
1. **Validation first** (prove GIL semaphore works)
2. **Testing second** (automated regression tests)
3. **Metrics optional** (can add later if time permits)
4. **Documentation last** (use megathink drafts)

**Estimated completion**: 2.5-6.5 hours depending on scope
**Minimum viable**: Validation + Testing = 2.5 hours
**Full complete**: All phases = 6.5 hours

**Ready to execute!** 🚀
