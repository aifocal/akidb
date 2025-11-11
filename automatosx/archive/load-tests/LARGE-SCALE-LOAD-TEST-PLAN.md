# AkiDB 2.0 - Large-Scale Load Test Plan

**Date:** 2025-11-10
**Purpose:** Stress testing to find system limits and discover unknown bugs
**Status:** 🔜 READY TO EXECUTE

---

## Executive Summary

This plan extends beyond the successful "quick" load tests (414k+ requests, 0 errors) to push the system to its limits and uncover edge cases, race conditions, and unknown bugs.

**Goals:**
1. Find the **maximum sustainable QPS** before degradation
2. Discover **memory limits** with large datasets
3. Test **endurance** over extended periods (hours, not minutes)
4. Trigger **race conditions** with extreme concurrency
5. Validate **error handling** under failure scenarios
6. Find **breaking points** for capacity planning

---

## Test Environment

### Hardware Target
- **Platform:** Apple Silicon (ARM M1/M2)
- **Memory:** Assume 16-32GB available
- **CPU:** 8-10 cores
- **Storage:** NVMe SSD

### Configuration
```toml
# config.toml for stress testing
[server]
host = "0.0.0.0"
rest_port = 8080

[database]
path = "sqlite:///tmp/akidb-stress.db"

[vector_persistence]
enabled = true
base_path = "collections-stress"

[metrics]
enabled = true
```

---

## Test Suite Design

### Part A: Throughput Stress Tests (Finding QPS Limits)

**Goal:** Find maximum QPS before P95 exceeds 25ms or errors occur

#### Test A1: Linear QPS Ramp (30 minutes)
```rust
Duration: 30 minutes
Load Profile:
  [0-5min]   100 QPS  (warmup)
  [5-10min]  500 QPS
  [10-15min] 1000 QPS
  [15-20min] 2000 QPS
  [20-25min] 3000 QPS
  [25-30min] 5000 QPS (expected to fail)

Dataset: 50,000 vectors @ 512 dimensions
Concurrency: Auto-scale (1 client per 10 QPS)

Success Criteria:
- Identify QPS where P95 > 25ms
- Identify QPS where error rate > 0.1%
- Measure degradation curve

Expected Result:
- Graceful degradation around 1000-2000 QPS
- System should not crash, even at 5000 QPS
```

#### Test A2: Sustained Peak Load (60 minutes)
```rust
Duration: 60 minutes
Load Profile:
  Use 80% of max QPS from Test A1
  Example: If max = 1500 QPS, use 1200 QPS

Dataset: 100,000 vectors @ 512 dimensions
Concurrency: 120 clients

Success Criteria:
- P95 < 30ms throughout (slight degradation allowed)
- Error rate < 0.01%
- No memory leaks (stable RSS)
- No performance degradation over time

Monitoring:
- Sample P95 every 30 seconds
- Track memory usage every minute
- Log GC pauses (if any)
```

#### Test A3: Burst Storm (15 minutes)
```rust
Duration: 15 minutes
Load Profile:
  [0-3min]   100 QPS   (baseline)
  [3-4min]   10000 QPS (extreme burst, 100x spike)
  [4-7min]   100 QPS   (recovery)
  [7-8min]   10000 QPS (second burst)
  [8-11min]  100 QPS   (recovery)
  [11-12min] 10000 QPS (third burst)
  [12-15min] 100 QPS   (final recovery)

Dataset: 50,000 vectors @ 512 dimensions
Concurrency: Spike to 1000 clients during burst

Success Criteria:
- System survives bursts without crashing
- Error rate < 5% during burst (backpressure expected)
- Recovery to baseline P95 within 60 seconds
- No permanent performance degradation

Expected Result:
- Graceful degradation during burst
- Request queue/backpressure handling
- Clean recovery after burst ends
```

---

### Part B: Dataset Size Stress Tests (Finding Memory Limits)

**Goal:** Find maximum dataset size before OOM or severe degradation

#### Test B1: Large Dataset Ladder (45 minutes)
```rust
Duration: 45 minutes total (5 phases × 9 minutes each)

Phase 1: 100k vectors  (9 min)
Phase 2: 500k vectors  (9 min)
Phase 3: 1M vectors    (9 min)
Phase 4: 2M vectors    (9 min) - expected limit
Phase 5: 5M vectors    (9 min) - expected failure

Load Profile per phase:
  [0-3min]   Insert all vectors (variable QPS)
  [3-9min]   100 QPS search queries

Dataset: 512 dimensions
Concurrency: 10 clients for search

Success Criteria:
- Identify dataset size where memory > 80% of available
- P95 should scale logarithmically (HNSW property)
- No OOM crashes

Monitoring:
- Memory RSS after each insert phase
- P95 latency for each dataset size
- Index build time for each phase

Expected Results:
- 100k:  ~2GB memory,   P95 ~3ms
- 500k:  ~10GB memory,  P95 ~5ms
- 1M:    ~20GB memory,  P95 ~7ms
- 2M:    ~40GB memory,  P95 ~10ms (may hit limit)
- 5M:    ~100GB memory, P95 ~15ms (likely OOM)
```

#### Test B2: High-Dimensional Vectors (30 minutes)
```rust
Duration: 30 minutes

Test Cases:
1. 50k vectors @ 2048 dimensions (4x normal)
2. 50k vectors @ 4096 dimensions (8x normal, max supported)
3. 10k vectors @ 4096 dimensions (reduced count)

Load Profile per test:
  [0-5min]  Insert all vectors
  [5-10min] 100 QPS search queries

Success Criteria:
- System handles max dimension (4096)
- P95 scales linearly with dimension
- Memory usage scales linearly with dimension

Expected Results:
- 2048d: P95 ~6ms  (2x dimensions → 2x latency)
- 4096d: P95 ~12ms (4x dimensions → 4x latency)
```

---

### Part C: Endurance Tests (Finding Time-Based Issues)

**Goal:** Discover memory leaks, resource leaks, and degradation over time

#### Test C1: 24-Hour Soak Test
```rust
Duration: 24 hours

Load Profile:
  Constant 200 QPS (realistic production load)

Dataset: 100,000 vectors @ 512 dimensions

Operations Mix:
  70% search queries
  20% inserts (new vectors)
  10% gets (by ID)

Concurrency: 20 clients

Success Criteria:
- P95 < 30ms throughout
- Error rate < 0.01%
- Memory growth < 10% over 24 hours
- No connection leaks
- WAL compaction triggers correctly

Monitoring:
- P95 every 5 minutes
- Memory RSS every 10 minutes
- Open file descriptors every hour
- WAL file count every hour

Expected Issues to Discover:
- Memory leaks in index
- Connection pool exhaustion
- File descriptor leaks
- WAL unbounded growth
- Background task accumulation
```

#### Test C2: Weekend Simulation (72 hours)
```rust
Duration: 72 hours

Load Profile (simulates real usage):
  [00:00-06:00] 10 QPS   (night)
  [06:00-09:00] 50 QPS   (morning ramp)
  [09:00-17:00] 200 QPS  (business hours)
  [17:00-20:00] 100 QPS  (evening)
  [20:00-00:00] 30 QPS   (late night)

Dataset: 200,000 vectors @ 512 dimensions

Operations Mix:
  60% search
  25% insert
  10% get
  5% delete (new for this test)

Success Criteria:
- Survives 72 hours without restart
- P95 remains stable across day/night cycles
- Compaction happens automatically
- Deleted vectors do not accumulate in memory

Expected Issues:
- Soft delete tombstone accumulation
- Compaction not triggering during low load
- Memory fragmentation over time
```

---

### Part D: Concurrency Stress Tests (Finding Race Conditions)

**Goal:** Trigger race conditions, deadlocks, and concurrency bugs

#### Test D1: Extreme Concurrency (20 minutes)
```rust
Duration: 20 minutes

Load Profile:
  100 QPS total across 1000 concurrent clients
  (each client sends 1 request every 10 seconds)

Dataset: 10,000 vectors @ 512 dimensions

Success Criteria:
- No deadlocks
- No "collection not found" errors (race in map access)
- No duplicate document IDs
- No panics from concurrent access

Expected Issues:
- RwLock contention
- Collection deletion during query race
- Index update race conditions
```

#### Test D2: Chaos Concurrency (30 minutes)
```rust
Duration: 30 minutes

Operations (all running concurrently):
  - 50 clients: Continuous search (50 QPS)
  - 10 clients: Continuous insert (10 QPS)
  - 5 clients:  Continuous delete (5 QPS)
  - 5 clients:  Create/delete collections (1 per minute)
  - 3 clients:  Load/unload collections (1 per 2 minutes)

Dataset: Start with 50k vectors, grows/shrinks over test

Success Criteria:
- No data corruption
- No deadlocks or panics
- Collection lifecycle operations succeed
- Search results remain consistent

Expected Issues:
- Delete collection during query race (Bug #6 validation)
- Insert during unload race
- Concurrent WAL writes
```

#### Test D3: Multi-Collection Stress (30 minutes)
```rust
Duration: 30 minutes

Setup:
  100 collections (extreme multi-tenancy)
  10,000 vectors per collection (1M total vectors)

Load Profile:
  500 QPS total across all collections
  (randomly distributed)

Operations Mix:
  80% search
  15% insert
  5% get

Concurrency: 50 clients

Success Criteria:
- P95 < 50ms (higher acceptable due to collection switching)
- No cross-collection data leakage
- All collections remain isolated
- Memory shared efficiently (not 100x single collection)

Expected Issues:
- Collection map RwLock contention
- Incorrect collection_id in WAL (Bug #16 validation)
- Cross-collection result contamination
```

---

### Part E: Failure Mode Tests (Finding Error Handling Gaps)

**Goal:** Validate error handling and recovery

#### Test E1: Disk Full Simulation
```rust
Duration: 15 minutes

Setup:
  Limit WAL directory to 100MB

Load Profile:
  Insert-heavy: 200 inserts/sec until disk full

Expected Behavior:
  - Graceful error when disk full
  - No data corruption
  - System continues after disk space freed

Success Criteria:
- Proper error messages ("disk full")
- No WAL corruption
- Recovery after space added
- No data loss for committed operations
```

#### Test E2: Network Partition Simulation (S3)
```rust
Duration: 20 minutes

Setup:
  Mock S3 with 50% failure rate

Load Profile:
  100 QPS with S3 tiering enabled

Expected Behavior:
  - Retry logic engages (Bug validation)
  - DLQ captures failed uploads
  - Circuit breaker trips after threshold
  - System continues without S3

Success Criteria:
- No data loss (WAL persists)
- DLQ contains failed uploads
- Circuit breaker prevents cascade failure
- Automatic recovery when S3 returns
```

#### Test E3: Corrupted WAL Recovery
```rust
Duration: 10 minutes per scenario

Scenarios:
1. Truncated WAL file (incomplete write)
2. Corrupted LSN sequence (LSN 100 → LSN 105)
3. Invalid operation type (unknown enum variant)
4. Mismatched collection_id in entry

Expected Behavior:
- Detect corruption during replay
- Log error with specific LSN
- Skip corrupted entry (or stop replay)
- Do not crash

Success Criteria:
- No panics during replay
- Clear error messages
- Partial recovery possible
```

---

### Part F: Memory Pressure Tests (Finding Memory Issues)

**Goal:** Test behavior under memory constraints

#### Test F1: Gradual Memory Exhaustion (60 minutes)
```rust
Duration: 60 minutes

Setup:
  No memory limit (let OS handle)

Load Profile:
  Insert 100k vectors every 5 minutes
  No compaction, no deletions

Target: Grow to ~50GB memory usage

Success Criteria:
- Graceful degradation as memory fills
- No OOM crashes
- Clear error when memory exhausted
- System recovers after memory freed

Monitoring:
- Memory RSS every minute
- Swap usage
- OOM killer logs
```

#### Test F2: Memory Churn Test (30 minutes)
```rust
Duration: 30 minutes

Load Profile:
  Continuous insert + delete at same rate
  100 inserts/sec + 100 deletes/sec

Dataset: Maintains ~50k vectors steady state

Success Criteria:
- Memory usage remains stable (no leak)
- No memory fragmentation issues
- GC pressure manageable

Expected Issues:
- Soft delete tombstone accumulation
- Memory not freed after delete
```

---

### Part G: Advanced Scenarios (Real-World Edge Cases)

#### Test G1: Cold Start Performance
```rust
Duration: 20 minutes

Scenario:
  1. Load collection with 100k vectors
  2. Shutdown server
  3. Restart server (cold start)
  4. Immediately hit with 500 QPS

Success Criteria:
- Collection loads in < 10 seconds
- First queries succeed (not "collection loading")
- P95 reaches steady state within 30 seconds

Expected Issues:
- Collection not ready during startup
- Warmup needed for optimal performance
```

#### Test G2: Update Storm
```rust
Duration: 15 minutes

Scenario:
  Continuously update same 1000 vectors
  100 updates/sec per vector (very high churn)

Success Criteria:
- No stale reads (eventually consistent)
- Version conflicts handled gracefully
- Memory usage stable (old versions GC'd)

Expected Issues:
- HNSW index thrashing
- WAL bloat from updates
```

#### Test G3: Pathological Query Patterns
```rust
Duration: 20 minutes

Scenarios:
1. Search for same vector 10,000 times (cache test)
2. Search for vectors that don't exist (NaN queries)
3. Search with very small top_k (k=1)
4. Search with very large top_k (k=10,000)
5. Random query vectors (no cache hits)

Success Criteria:
- Cache provides speedup for repeated queries
- Invalid queries return proper errors
- Edge cases (k=1, k=10000) work correctly

Expected Issues:
- Cache misses cause slowdown
- top_k=10000 DoS validation (Bug #8)
```

---

## Test Execution Plan

### Phase 1: Throughput Tests (1.5 hours)
```bash
# Day 1 Morning
cargo test --release --test comprehensive_load_test -- \
  test_a1_linear_qps_ramp --nocapture --ignored

cargo test --release --test comprehensive_load_test -- \
  test_a2_sustained_peak --nocapture --ignored

cargo test --release --test comprehensive_load_test -- \
  test_a3_burst_storm --nocapture --ignored
```

### Phase 2: Dataset Size Tests (1.5 hours)
```bash
# Day 1 Afternoon
cargo test --release --test comprehensive_load_test -- \
  test_b1_large_dataset_ladder --nocapture --ignored

cargo test --release --test comprehensive_load_test -- \
  test_b2_high_dimensional --nocapture --ignored
```

### Phase 3: Concurrency Tests (1.5 hours)
```bash
# Day 2 Morning
cargo test --release --test comprehensive_load_test -- \
  test_d1_extreme_concurrency --nocapture --ignored

cargo test --release --test comprehensive_load_test -- \
  test_d2_chaos_concurrency --nocapture --ignored

cargo test --release --test comprehensive_load_test -- \
  test_d3_multi_collection_stress --nocapture --ignored
```

### Phase 4: Failure Mode Tests (1 hour)
```bash
# Day 2 Afternoon
cargo test --release --test comprehensive_load_test -- \
  test_e1_disk_full --nocapture --ignored

cargo test --release --test comprehensive_load_test -- \
  test_e2_network_partition --nocapture --ignored

cargo test --release --test comprehensive_load_test -- \
  test_e3_corrupted_wal --nocapture --ignored
```

### Phase 5: Long-Running Tests (72+ hours)
```bash
# Day 3-6 (Background)
# Start 24-hour test
nohup cargo test --release --test comprehensive_load_test -- \
  test_c1_24hour_soak --nocapture --ignored > \
  logs/24hour-test.log 2>&1 &

# Start 72-hour test (weekend)
nohup cargo test --release --test comprehensive_load_test -- \
  test_c2_weekend_simulation --nocapture --ignored > \
  logs/72hour-test.log 2>&1 &
```

### Phase 6: Advanced Scenarios (1.5 hours)
```bash
# Day 7 (After soak tests)
cargo test --release --test comprehensive_load_test -- \
  test_g1_cold_start --nocapture --ignored

cargo test --release --test comprehensive_load_test -- \
  test_g2_update_storm --nocapture --ignored

cargo test --release --test comprehensive_load_test -- \
  test_g3_pathological_queries --nocapture --ignored
```

---

## Expected Bugs to Discover

Based on stress testing experience, likely bugs:

### High Priority (Expected)

1. **Memory Leak in HNSW Index**
   - Symptom: Memory grows over 24-hour test
   - Location: `akidb-index/src/instant_hnsw.rs`
   - Fix: Review node deletion logic

2. **WAL Unbounded Growth**
   - Symptom: WAL directory grows to 10GB+ during soak test
   - Location: `akidb-storage/src/wal/file_wal.rs`
   - Fix: Implement WAL rotation/cleanup

3. **Connection Pool Exhaustion**
   - Symptom: "Too many open files" after 12 hours
   - Location: Server initialization
   - Fix: Implement connection limits

4. **Soft Delete Tombstone Accumulation**
   - Symptom: Memory grows with deletes, not freed
   - Location: `akidb-index/src/hnsw.rs`
   - Fix: Implement tombstone compaction

5. **Race Condition: Collection Deletion During Query**
   - Symptom: Panic in query after delete_collection
   - Location: `akidb-service/src/collection_service.rs`
   - Fix: Extend dual lock acquisition (Bug #6 extension)

### Medium Priority (Possible)

6. **P95 Degradation Under Memory Pressure**
   - Symptom: P95 climbs from 3ms → 50ms at 80% memory
   - Location: OS paging, HNSW search
   - Fix: Implement memory-aware backpressure

7. **HNSW Index Corruption Under High Concurrency**
   - Symptom: Search returns wrong results after 1000 QPS
   - Location: `instant_hnsw.rs` concurrent updates
   - Fix: Review Mutex vs RwLock usage

8. **DLQ Infinite Growth**
   - Symptom: DLQ grows unbounded during S3 failures
   - Location: `akidb-storage/src/storage_backend.rs`
   - Fix: Implement DLQ max size

### Low Priority (Edge Cases)

9. **NaN Results with Very High Dimensions**
   - Symptom: Cosine distance returns NaN at 4096d
   - Location: Distance calculation overflow
   - Fix: Use f64 for intermediate calculations

10. **Collection Map Deadlock**
    - Symptom: System hangs during multi-collection stress
    - Location: `collection_service.rs` nested locks
    - Fix: Acquire locks in consistent order

---

## Success Metrics

### Performance Benchmarks

| Metric | Target | Stretch Goal |
|--------|--------|--------------|
| Max Sustainable QPS | >500 QPS | >1000 QPS |
| Max Dataset Size | >1M vectors | >5M vectors |
| 24-Hour Uptime | 100% | 100% |
| P95 Under Load | <50ms @ 500 QPS | <30ms @ 500 QPS |
| Memory Efficiency | <100GB for 1M vectors | <50GB for 1M vectors |

### Bug Discovery Goals

| Category | Expected Bugs | Severity |
|----------|---------------|----------|
| Memory Leaks | 2-3 bugs | HIGH |
| Race Conditions | 1-2 bugs | CRITICAL |
| Resource Leaks | 2-4 bugs | MEDIUM |
| Performance Regressions | 1-2 bugs | LOW |
| **Total** | **6-11 bugs** | **MIXED** |

---

## Monitoring & Instrumentation

### Metrics to Collect

```rust
// Real-time metrics (every 10 seconds)
- P50, P95, P99 latency
- QPS (queries per second)
- Error rate
- Active connections

// Resource metrics (every 1 minute)
- Memory RSS
- Memory virtual
- CPU usage (per core)
- Open file descriptors
- Thread count

// Storage metrics (every 5 minutes)
- WAL file count
- WAL total size
- Index size in memory
- Compaction events

// Long-term metrics (every 1 hour)
- Total requests processed
- Total errors
- Uptime
- Restart count
```

### Logging Strategy

```rust
// Error logs (always)
- All errors with stack traces
- Panics with full context
- Deadlock warnings

// Warning logs (threshold-based)
- P95 > 25ms
- Error rate > 0.1%
- Memory > 80% of limit
- Open files > 10,000

// Info logs (periodic)
- Test phase transitions
- Collection lifecycle events
- Compaction events
```

---

## Deliverables

### Test Reports

1. **Executive Summary Report**
   - Overall pass/fail status
   - Maximum sustainable QPS discovered
   - Maximum dataset size validated
   - List of bugs discovered

2. **Detailed Test Results**
   - Per-scenario results
   - Performance graphs (P95 over time)
   - Resource usage graphs
   - Error logs and stack traces

3. **Bug Reports**
   - Each bug with:
     - Description
     - Reproduction steps
     - Severity assessment
     - Proposed fix

4. **Capacity Planning Guide**
   - QPS vs P95 curve
   - Dataset size vs memory curve
   - Recommended production limits
   - Scaling guidelines

---

## Implementation Tasks

### Immediate (Before Running Tests)

1. **Implement missing test scenarios** in `tests/comprehensive_load_test.rs`
2. **Add monitoring infrastructure** (Prometheus metrics)
3. **Create test data generators** (large datasets)
4. **Setup logging aggregation** (file + stdout)

### Week 1: Core Stress Tests

- [ ] Implement Test A1-A3 (Throughput tests)
- [ ] Implement Test B1-B2 (Dataset size tests)
- [ ] Implement Test D1-D3 (Concurrency tests)
- [ ] Run all tests, collect results
- [ ] Fix any critical bugs discovered

### Week 2: Endurance & Failure Tests

- [ ] Implement Test C1-C2 (Endurance tests)
- [ ] Implement Test E1-E3 (Failure mode tests)
- [ ] Start 24-hour soak test
- [ ] Start 72-hour weekend test
- [ ] Monitor and log results

### Week 3: Analysis & Optimization

- [ ] Implement Test F1-F2 (Memory pressure)
- [ ] Implement Test G1-G3 (Advanced scenarios)
- [ ] Analyze all results
- [ ] Create bug reports
- [ ] Prepare capacity planning guide

---

## Risk Assessment

### High Risk Areas

1. **Memory exhaustion** during large dataset tests
   - Mitigation: Run on machine with 32GB+ RAM
   - Fallback: Reduce dataset size in increments

2. **System instability** during 72-hour test
   - Mitigation: Run in isolated environment
   - Fallback: Reduce to 24-hour test only

3. **Data corruption** during concurrency tests
   - Mitigation: Backup test data before each test
   - Fallback: Reset database between tests

### Medium Risk Areas

4. **Test suite too long** (>1 week total)
   - Mitigation: Parallelize where possible
   - Fallback: Prioritize critical tests only

5. **False positives** in bug detection
   - Mitigation: Reproduce bugs 3 times before reporting
   - Fallback: Mark as "suspected" bugs

---

## Conclusion

This large-scale load testing plan will:

✅ Stress the system beyond normal operating conditions
✅ Discover unknown bugs and edge cases
✅ Validate system limits and breaking points
✅ Provide data for capacity planning
✅ Confirm production readiness for high-scale deployments

**Estimated Timeline:** 2-3 weeks (including long-running tests)
**Estimated Bugs to Discover:** 6-11 bugs (mostly medium severity)
**Deliverables:** 4 comprehensive reports + capacity planning guide

**Next Step:** Implement test scenarios in `tests/comprehensive_load_test.rs` and begin execution.

---

**Created:** 2025-11-10
**Status:** 🔜 READY FOR IMPLEMENTATION
**Priority:** HIGH (required for GA release confidence)

