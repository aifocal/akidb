# Phase 10 Week 4: Performance Optimization + Advanced E2E Testing

**Status:** Draft
**Author:** AkiDB Team
**Created:** 2025-11-09
**Phase:** Phase 10 (S3/MinIO Tiered Storage) - Week 4
**Part:** Part B - Production Hardening

---

## Executive Summary

Week 4 marks the transition from Part A (Storage Infrastructure) to Part B (Production Hardening) of Phase 10. The goal is to optimize S3/MinIO tiered storage performance by **3x** (from 200 ops/sec to 600+ ops/sec) and validate the system with comprehensive edge-case testing.

**Business Value:**
- **3x performance improvement** in storage operations (batch + parallel uploads)
- **100 QPS sustained load** validation (10-minute load tests)
- **15 advanced E2E tests** covering concurrency, quotas, and failure modes
- **Production-ready** system with performance profiling and optimization

**Key Deliverables:**
1. Batch S3 uploads (2.5x improvement: >500 ops/sec)
2. Parallel S3 uploads (3x improvement: >600 ops/sec)
3. Mock S3 service for fast, deterministic testing
4. Load testing framework (100 QPS sustained)
5. 15 advanced E2E test scenarios
6. CPU/memory profiling infrastructure

**Dependencies:** Weeks 1-3 (Parquet Snapshotter, Tiering Policies, Integration Tests)

**Timeline:** 5 days

---

## Goals and Non-Goals

### Goals

1. **Performance Optimization**
   - Achieve 3x improvement in S3 upload throughput
   - Implement batch uploads with multipart upload API
   - Implement parallel uploads with connection pooling
   - Reduce S3 upload latency with HTTP keep-alive

2. **Load Testing**
   - Build framework for 100 QPS sustained load
   - Mixed workload: 70% search, 20% insert, 10% tier control
   - Validate P95 latency <25ms under load
   - Verify error rate <0.1%

3. **Advanced E2E Testing**
   - 5 concurrency/race condition tests
   - 4 quota/limit enforcement tests
   - 6 failure mode recovery tests
   - Validate data consistency under stress

4. **Profiling Infrastructure**
   - CPU profiling with flamegraph
   - Memory profiling with heaptrack
   - Performance regression detection
   - Bottleneck identification

### Non-Goals

1. **Out of Scope for Week 4**
   - Distributed deployment (Phase 9+)
   - Cross-region replication (Phase 9+)
   - Advanced caching strategies (Phase 11+)
   - ML-based tiering policies (Phase 11+)

2. **Not Changing**
   - Core storage architecture (stable from Week 1-3)
   - Parquet format (stable)
   - Tiering policy rules (stable from Week 2)

---

## User Stories

### DevOps Engineer (Primary Persona)

**Story 1: High-Throughput Uploads**
> As a DevOps engineer, I want S3 uploads to handle **600+ ops/sec** so that bulk data migration doesn't bottleneck during peak hours.

**Acceptance Criteria:**
- Batch uploads achieve >500 ops/sec (2.5x baseline)
- Parallel uploads achieve >600 ops/sec (3x baseline)
- Connection pooling reduces latency by 2.5x
- Metrics track upload throughput

**Story 2: Load Testing Validation**
> As a DevOps engineer, I want to validate the system under **100 QPS sustained load** for 10 minutes so that I can confidently deploy to production.

**Acceptance Criteria:**
- Load test framework supports mixed workload (search/insert/tier)
- P95 latency <25ms under 100 QPS load
- Error rate <0.1%
- Memory and CPU usage stable (no leaks)

### QA Engineer (Secondary Persona)

**Story 3: Concurrency Testing**
> As a QA engineer, I want comprehensive tests for **concurrent operations** so that race conditions are caught before production.

**Acceptance Criteria:**
- 5 tests for concurrent demotions, promotions, snapshots
- Race condition detection on tier state updates
- Validation of background worker concurrent with API calls

**Story 4: Failure Mode Testing**
> As a QA engineer, I want to test **S3 failure scenarios** (rate limits, timeouts, corrupted files) so that the system handles errors gracefully.

**Acceptance Criteria:**
- MockS3ObjectStore supports error injection
- 6 tests for S3 rate limits, timeouts, partial uploads, corruption
- Automatic retry and DLQ handling
- Zero data loss in all failure scenarios

### SRE (Secondary Persona)

**Story 5: Performance Profiling**
> As an SRE, I want **CPU and memory profiling** tools so that I can identify bottlenecks and optimize hot paths.

**Acceptance Criteria:**
- Flamegraph generation for CPU profiling
- Heaptrack integration for memory profiling
- Performance regression tests in CI
- Documented optimization strategies

---

## Technical Specification

### 1. Batch S3 Uploads

**Goal:** Reduce S3 upload overhead by grouping multiple snapshots into a single multipart upload.

**Architecture:**
```rust
pub struct BatchUploader {
    object_store: Arc<dyn ObjectStore>,
    batch_size: usize,           // Default: 10 snapshots
    flush_interval: Duration,     // Default: 5 seconds
    pending: RwLock<Vec<PendingUpload>>,
}

impl BatchUploader {
    pub async fn enqueue(&self, key: String, data: Bytes) -> CoreResult<()>;
    pub async fn flush(&self) -> CoreResult<()>;
    async fn upload_batch(&self, prefix: String, uploads: Vec<PendingUpload>) -> CoreResult<()>;
}
```

**S3 Multipart Upload API:**
- Use `CreateMultipartUpload` to initiate
- Upload parts concurrently (max 10,000 parts)
- Complete with `CompleteMultipartUpload`
- Abort on error with `AbortMultipartUpload`

**Performance Target:** >500 ops/sec (2.5x baseline of 200 ops/sec)

**Configuration:**
```toml
[storage.batch_upload]
enabled = true
batch_size = 10          # snapshots per batch
flush_interval_secs = 5  # max time before flush
```

### 2. Parallel S3 Uploads

**Goal:** Maximize S3 throughput by uploading multiple snapshots concurrently.

**Architecture:**
```rust
pub struct ParallelUploader {
    object_store: Arc<dyn ObjectStore>,
    concurrency: usize,      // Default: 10 concurrent uploads
    semaphore: Semaphore,    // Concurrency control
}

impl ParallelUploader {
    pub async fn upload_many(&self, uploads: Vec<Upload>) -> CoreResult<Vec<CoreResult<()>>>;
    async fn upload_one(&self, upload: Upload) -> CoreResult<()>;
}
```

**Concurrency Control:**
- Use `tokio::sync::Semaphore` to limit concurrent uploads
- Configurable concurrency (default: 10)
- Graceful handling of S3 rate limits (429/503 errors)
- Exponential backoff on errors

**Performance Target:** >600 ops/sec (3x baseline)

**Configuration:**
```toml
[storage.parallel_upload]
enabled = true
concurrency = 10
max_retries = 3
backoff_base_ms = 100
```

### 3. Connection Pooling

**Goal:** Reduce HTTP handshake overhead by reusing connections.

**Architecture:**
```rust
pub struct HttpClientPool {
    client: reqwest::Client,
}

impl HttpClientPool {
    pub fn new(config: PoolConfig) -> Self {
        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(config.pool_size)
            .pool_idle_timeout(Duration::from_secs(config.idle_timeout_secs))
            .build()
            .unwrap();
        Self { client }
    }
}
```

**Configuration:**
```toml
[storage.connection_pool]
pool_size = 20           # connections per host
idle_timeout_secs = 90   # keep-alive timeout
```

**Performance Impact:** 2.5x faster requests (avoid TLS handshake)

### 4. Mock S3 Service

**Goal:** Enable fast, deterministic testing without real S3 infrastructure.

**Architecture:**
```rust
pub struct MockS3ObjectStore {
    storage: Arc<RwLock<HashMap<String, Bytes>>>,
    latency: Duration,                    // Simulated latency
    error_mode: Arc<RwLock<Option<MockS3Error>>>,
    metrics: Arc<RwLock<MockS3Metrics>>,
}

pub enum MockS3Error {
    RateLimit,       // 503 SlowDown
    Timeout,         // Connection timeout
    PartialUpload,   // Upload incomplete
    Corruption,      // Data corruption
}

impl ObjectStore for MockS3ObjectStore {
    async fn put(&self, key: &str, value: Bytes) -> CoreResult<()>;
    async fn get(&self, key: &str) -> CoreResult<Option<Bytes>>;
    async fn delete(&self, key: &str) -> CoreResult<()>;
    async fn list(&self, prefix: &str) -> CoreResult<Vec<String>>;
}
```

**Features:**
- In-memory storage (no I/O)
- Configurable latency simulation
- Error injection for testing
- Metrics tracking (upload/download counts, bytes)
- Thread-safe (Arc + RwLock)

**Benefits:**
- Zero cost for CI/CD
- Deterministic behavior
- Fast tests (<1s)
- Easy error injection

### 5. Load Testing Framework

**Goal:** Validate system behavior under 100 QPS sustained load for 10 minutes.

**Architecture:**
```rust
pub struct LoadTest {
    service: Arc<CollectionService>,
    workload: WorkloadConfig,
    metrics: Arc<LoadTestMetrics>,
}

pub struct WorkloadConfig {
    pub duration: Duration,        // 10 minutes
    pub qps: usize,                // 100 QPS
    pub search_pct: f32,           // 70%
    pub insert_pct: f32,           // 20%
    pub tier_control_pct: f32,     // 10%
}

pub struct LoadTestMetrics {
    pub latency_p50: Duration,
    pub latency_p95: Duration,
    pub latency_p99: Duration,
    pub error_rate: f32,
    pub memory_usage_mb: f64,
    pub cpu_usage_pct: f32,
}
```

**Dataset:**
- 100k vectors across 100 collections
- 512-dimensional vectors
- Mixed access patterns (hot/warm/cold)

**Success Criteria:**
- P50 latency <10ms
- P95 latency <25ms
- P99 latency <50ms
- Error rate <0.1%
- Memory stable (no leaks)
- CPU <80% average

---

## Advanced E2E Test Scenarios

### Category 1: Concurrency & Race Conditions (5 tests)

**Test 1: Concurrent Demotions**
```rust
#[tokio::test]
async fn test_concurrent_demotions_same_collection() {
    // Setup: 10 workers trying to demote same collection
    // Expected: Only one succeeds, others skip gracefully
    // Validation: Tier state consistent, no data loss
}
```

**Test 2: Concurrent Promotions + Searches**
```rust
#[tokio::test]
async fn test_concurrent_promotion_and_search() {
    // Setup: Promote from cold while searching
    // Expected: Search waits or fails gracefully
    // Validation: No corrupted index, search results correct
}
```

**Test 3: Concurrent Snapshot + Restore**
```rust
#[tokio::test]
async fn test_concurrent_snapshot_and_restore() {
    // Setup: Snapshot while restore in progress
    // Expected: Operations serialize or fail cleanly
    // Validation: No data corruption
}
```

**Test 4: Race Condition on Tier State**
```rust
#[tokio::test]
async fn test_race_condition_tier_state_update() {
    // Setup: Multiple workers updating tier state
    // Expected: Last write wins, no lost updates
    // Validation: Tier state matches actual tier
}
```

**Test 5: Background Worker + API Concurrent Access**
```rust
#[tokio::test]
async fn test_background_worker_concurrent_with_api() {
    // Setup: Background worker running, API calls in flight
    // Expected: No deadlocks, operations complete
    // Validation: All operations succeed
}
```

### Category 2: Quota & Limits (4 tests)

**Test 6: Memory Quota Enforcement**
```rust
#[tokio::test]
async fn test_memory_quota_enforcement() {
    // Setup: Insert vectors until memory quota exceeded
    // Expected: Error with clear message, automatic demotion
    // Validation: Memory usage stays within quota
}
```

**Test 7: Storage Quota Enforcement**
```rust
#[tokio::test]
async fn test_storage_quota_enforcement() {
    // Setup: Create snapshots until storage quota exceeded
    // Expected: Error, oldest snapshots cleaned
    // Validation: Storage usage within quota
}
```

**Test 8: Collection Count Limit**
```rust
#[tokio::test]
async fn test_collection_count_limit() {
    // Setup: Create 1000 collections (max limit)
    // Expected: 1001st collection fails
    // Validation: Limit enforced, error clear
}
```

**Test 9: Warm Tier Disk Full**
```rust
#[tokio::test]
async fn test_warm_tier_disk_full_handling() {
    // Setup: Fill warm tier disk to capacity
    // Expected: Automatic demotion to cold tier
    // Validation: System continues operating
}
```

### Category 3: Failure Modes (6 tests)

**Test 10: S3 Rate Limit Handling**
```rust
#[tokio::test]
async fn test_s3_rate_limit_503_throttle() {
    // Setup: MockS3ObjectStore returns 503 SlowDown
    // Expected: Exponential backoff, retry success
    // Validation: Upload eventually succeeds
}
```

**Test 11: S3 Connection Timeout**
```rust
#[tokio::test]
async fn test_s3_connection_timeout() {
    // Setup: MockS3ObjectStore simulates timeout
    // Expected: Retry with backoff, eventual success or DLQ
    // Validation: No data loss
}
```

**Test 12: Partial Snapshot Upload Recovery**
```rust
#[tokio::test]
async fn test_partial_snapshot_upload_recovery() {
    // Setup: Upload fails mid-transfer
    // Expected: Cleanup partial upload, retry
    // Validation: No orphaned data in S3
}
```

**Test 13: Corrupted Parquet File Recovery**
```rust
#[tokio::test]
async fn test_corrupted_parquet_file_recovery() {
    // Setup: Download corrupted Parquet file
    // Expected: Error detection, fallback to previous snapshot
    // Validation: Collection still accessible
}
```

**Test 14: Background Worker Restart Mid-Cycle**
```rust
#[tokio::test]
async fn test_background_worker_restart_mid_cycle() {
    // Setup: Kill background worker during tier cycle
    // Expected: Restart, resume from checkpoint
    // Validation: No duplicate demotions
}
```

**Test 15: Tier Demotion Rollback on S3 Failure**
```rust
#[tokio::test]
async fn test_tier_demotion_rollback_on_s3_failure() {
    // Setup: S3 upload fails during demotion
    // Expected: Rollback demotion, collection stays in original tier
    // Validation: Tier state consistent, no data loss
}
```

---

## Performance Requirements

| Metric | Baseline (Week 3) | Target (Week 4) | Improvement |
|--------|------------------|----------------|-------------|
| **S3 Upload Throughput** | 200 ops/sec | 600+ ops/sec | 3x |
| Batch Uploads | N/A | 500 ops/sec | 2.5x |
| Parallel Uploads | N/A | 600 ops/sec | 3x |
| **Load Test (100 QPS)** | N/A | Pass | - |
| Search P50 | <5ms | <10ms | - |
| Search P95 | <10ms | <25ms | - |
| Search P99 | <25ms | <50ms | - |
| Error Rate | N/A | <0.1% | - |
| Memory Stability | N/A | No leaks | - |
| CPU Usage | N/A | <80% avg | - |

---

## Test Strategy

### Code Metrics
- BatchUploader: ~300 lines
- ParallelUploader: ~300 lines
- MockS3ObjectStore: ~400 lines
- LoadTest framework: ~500 lines
- Advanced E2E tests: ~1,000 lines
- **Total**: ~2,500 lines

### Test Breakdown
1. **Unit Tests:** 10 tests
   - 3 BatchUploader tests
   - 3 ParallelUploader tests
   - 4 MockS3ObjectStore tests

2. **Integration Tests:** 5 tests
   - Batch upload with real S3Config
   - Parallel upload with connection pooling
   - Mock S3 end-to-end workflow
   - Load test infrastructure validation
   - Profiling tool integration

3. **Advanced E2E Tests:** 15 tests
   - 5 concurrency/race condition tests
   - 4 quota/limit enforcement tests
   - 6 failure mode recovery tests

4. **Load Tests:** 1 test
   - 100 QPS sustained for 10 minutes

5. **Profiling:** 2 tests
   - CPU profiling (flamegraph)
   - Memory profiling (heaptrack)

**Total Tests:** 33 new tests

### Test Infrastructure

**MockS3ObjectStore:**
- In-memory storage
- Error injection
- Latency simulation
- Metrics tracking

**LoadTest Framework:**
- Multi-worker load generation
- Mixed workload (search/insert/tier)
- Metrics collection (P50/P95/P99)
- Memory and CPU monitoring

**Profiling Tools:**
- `cargo-flamegraph` for CPU profiling
- `heaptrack` for memory profiling
- Criterion.rs for benchmarking
- Performance regression tests

---

## Dependencies

### Internal Dependencies (Phase 10)
- ✅ Week 1: ParquetSnapshotter (required for batch uploads)
- ✅ Week 2: TieringManager (required for load testing)
- ✅ Week 3: Integration tests (baseline for E2E tests)

### External Dependencies
- **Rust Crates:**
  - `tokio` (async runtime, semaphore)
  - `futures` (stream processing)
  - `reqwest` (HTTP client with connection pooling)
  - `criterion` (benchmarking)
  - `parking_lot` (RwLock for MockS3ObjectStore)

- **Profiling Tools:**
  - `cargo-flamegraph` (CPU profiling)
  - `heaptrack` (memory profiling)
  - `perf` (Linux performance analysis)

- **Infrastructure:**
  - MinIO (local S3 testing)
  - Docker Compose (MinIO deployment)

---

## Success Criteria

### Performance Targets
- ✅ Batch uploads achieve >500 ops/sec
- ✅ Parallel uploads achieve >600 ops/sec
- ✅ Combined improvement: 3x baseline (200 → 600 ops/sec)
- ✅ Connection pooling reduces latency by 2.5x

### Load Testing
- ✅ 100 QPS sustained for 10 minutes
- ✅ P95 latency <25ms under load
- ✅ Error rate <0.1%
- ✅ Memory stable (no leaks)
- ✅ CPU <80% average

### Test Coverage
- ✅ 33 new tests (10 unit + 5 integration + 15 E2E + 1 load + 2 profiling)
- ✅ 100% pass rate
- ✅ All advanced E2E scenarios validated
- ✅ Zero data loss in all failure modes

### Profiling
- ✅ CPU profiling infrastructure in place
- ✅ Memory profiling infrastructure in place
- ✅ Bottlenecks identified and documented
- ✅ Optimization strategies documented

---

## Risks and Mitigations

### Risk 1: S3 Rate Limiting in Production
**Impact:** High
**Probability:** Medium
**Mitigation:**
- Implement exponential backoff
- Add circuit breaker for S3 errors
- Monitor S3 API metrics in production
- Use DLQ for failed uploads

### Risk 2: Load Test Infrastructure Overhead
**Impact:** Medium
**Probability:** Low
**Mitigation:**
- Use MockS3ObjectStore for fast tests
- Run load tests in dedicated environment
- Profile load test framework itself
- Document resource requirements

### Risk 3: Memory Leaks Under Load
**Impact:** High
**Probability:** Low
**Mitigation:**
- Run heaptrack profiling
- Add memory leak detection to CI
- Monitor memory usage over time
- Implement automatic cleanup

### Risk 4: Flaky Concurrency Tests
**Impact:** Medium
**Probability:** Medium
**Mitigation:**
- Use deterministic timing where possible
- Add retries for timing-sensitive tests
- Use Loom for concurrency testing
- Document known race conditions

### Risk 5: Performance Regression
**Impact:** High
**Probability:** Low
**Mitigation:**
- Add performance regression tests to CI
- Baseline metrics from Week 3
- Automated alerts on regression
- Regular benchmarking

---

## Timeline

**Duration:** 5 days

### Day 1: Batch S3 Uploads
- Implement BatchUploader struct
- Add multipart upload support
- 3 unit tests
- Benchmark ops/sec improvement
- **Deliverable:** BatchUploader with >500 ops/sec

### Day 2: Parallel S3 Uploads
- Implement ParallelUploader struct
- Add connection pooling
- 3 unit tests
- Benchmark ops/sec improvement
- **Deliverable:** ParallelUploader with >600 ops/sec

### Day 3: Mock S3 Service
- Implement MockS3ObjectStore
- Error injection framework
- Metrics tracking
- 4 unit tests + 1 integration test
- **Deliverable:** MockS3ObjectStore ready for E2E tests

### Day 4: Load Testing Framework
- Implement LoadTest struct
- Multi-worker load generation
- Metrics collection
- Run 10-minute load test
- **Deliverable:** Load test passing 100 QPS target

### Day 5: Advanced E2E Tests + Profiling
- 15 advanced E2E scenarios
- CPU/memory profiling
- Bottleneck identification
- Week 4 completion report
- **Deliverable:** All tests passing, profiling complete

---

## Approval

**Stakeholders:**
- Engineering Lead: ___________________
- Product Manager: ___________________
- QA Lead: ___________________
- SRE/DevOps: ___________________

**Approval Date:** ___________________

---

## Appendix

### A. Configuration Reference

```toml
[storage.batch_upload]
enabled = true
batch_size = 10
flush_interval_secs = 5

[storage.parallel_upload]
enabled = true
concurrency = 10
max_retries = 3
backoff_base_ms = 100

[storage.connection_pool]
pool_size = 20
idle_timeout_secs = 90

[load_test]
duration_secs = 600  # 10 minutes
qps = 100
search_pct = 0.7
insert_pct = 0.2
tier_control_pct = 0.1
```

### B. Profiling Commands

**CPU Profiling:**
```bash
cargo flamegraph --bin akidb-rest
```

**Memory Profiling:**
```bash
heaptrack cargo run -p akidb-rest
heaptrack_gui heaptrack.akidb-rest.*.gz
```

**Benchmarking:**
```bash
cargo bench --bench upload_bench
cargo bench --bench load_test_bench
```

### C. Load Test Dataset

**Dataset Characteristics:**
- 100k vectors total
- 100 collections (1k vectors each)
- 512-dimensional vectors
- Mixed access patterns:
  - Hot tier: 20 collections (frequently accessed)
  - Warm tier: 50 collections (occasionally accessed)
  - Cold tier: 30 collections (rarely accessed)

**Workload Distribution:**
- 70% search queries (mixed hot/warm/cold)
- 20% insert operations (new vectors)
- 10% tier control (promote/demote/pin)

### D. Error Injection Scenarios

**MockS3Error Types:**
1. **RateLimit:** 503 SlowDown (S3 throttling)
2. **Timeout:** Connection timeout after 30s
3. **PartialUpload:** Upload incomplete (network interruption)
4. **Corruption:** Data corruption during transfer

**Injection API:**
```rust
let mock_s3 = MockS3ObjectStore::new();
mock_s3.inject_error(MockS3Error::RateLimit).await;
```

---

**End of Document**
