# Phase 10 Week 4: Performance Optimization + E2E Testing - Comprehensive Megathink

**Date**: 2025-11-09
**Phase**: Phase 10 Week 4 (Part B Start)
**Focus**: Performance Optimization, Load Testing, Advanced E2E Testing
**Status**: 🔍 ANALYSIS & PLANNING

---

## Executive Summary

**Objective**: Optimize S3 upload/download performance and validate system behavior under production load with comprehensive E2E testing.

**Business Value**:
- 3x performance improvement (>600 ops/sec from ~200 ops/sec)
- Validated production readiness (100 QPS sustained load)
- Comprehensive test coverage for edge cases
- Identified and fixed performance bottlenecks
- Foundation for Week 5 observability

**Technical Approach**:
- Batch S3 uploads (group multiple operations into single requests)
- Parallel S3 uploads (concurrent uploads with connection pooling)
- Mock S3 service for deterministic testing
- Load testing framework (100 QPS sustained, 10-minute runs)
- CPU/memory profiling and optimization
- 15 advanced E2E test scenarios

**Timeline**: 5 days (Week 4 of Phase 10)

---

## Table of Contents

1. [Background & Context](#1-background--context)
2. [Problem Statement](#2-problem-statement)
3. [Performance Optimization Strategy](#3-performance-optimization-strategy)
4. [Mock S3 Infrastructure](#4-mock-s3-infrastructure)
5. [Load Testing Framework](#5-load-testing-framework)
6. [Advanced E2E Test Scenarios](#6-advanced-e2e-test-scenarios)
7. [Profiling and Optimization](#7-profiling-and-optimization)
8. [Implementation Plan](#8-implementation-plan)
9. [Success Criteria](#9-success-criteria)
10. [Risk Analysis](#10-risk-analysis)

---

## 1. Background & Context

### 1.1 What We've Built (Weeks 1-3)

**Week 1: Parquet Snapshotter**
- Efficient columnar storage with 2-3x compression
- S3/MinIO upload/download integration
- Performance: ~200 ops/sec S3 throughput

**Week 2: Hot/Warm/Cold Tiering**
- Automatic tier transitions
- Background worker for demotions/promotions
- Access tracking with LRU

**Week 3: Integration Testing + RC2**
- 83 tests passing (47 baseline + 36 new)
- Performance benchmarks documented
- v2.0.0-rc2 released

**Remaining Gap**: Performance optimization for production workloads

### 1.2 Performance Bottlenecks Identified

**Current State** (from Week 3 benchmarks):
- S3 upload: ~200 ops/sec (single-threaded, one-at-a-time)
- S3 download: ~150 ops/sec (single-threaded)
- Tier demotion: ~50 collections/minute (bottlenecked by S3)
- Background worker cycle: ~5-10 minutes for 1000 collections

**Root Causes**:
1. **Sequential S3 operations**: Uploads happen one at a time
2. **No batching**: Each collection creates separate S3 request
3. **No connection pooling**: New HTTP connection per request
4. **Synchronous worker**: Background worker blocks on each upload

**Impact**:
- Slow cold tier migrations (10+ minutes for 100 collections)
- Limited throughput for batch operations
- Poor resource utilization (CPU idle while waiting for S3)

### 1.3 What Week 4 Delivers

**Primary Goal**: Achieve >600 ops/sec S3 throughput through optimization.

**Key Improvements**:
1. **Batch S3 uploads**: Group multiple snapshots into fewer requests
2. **Parallel S3 uploads**: Concurrent uploads with tokio::spawn
3. **Connection pooling**: Reuse HTTP connections for S3
4. **Mock S3 service**: Deterministic testing without real S3
5. **Load testing**: Validate 100 QPS sustained search load
6. **15 E2E tests**: Edge cases and production scenarios

---

## 2. Problem Statement

### 2.1 Core Requirements

**FR-1**: Batch S3 upload operations
- Group multiple snapshots into single multipart upload
- Target: >500 ops/sec (2.5x improvement)
- Maintain data integrity (no corruption)

**FR-2**: Parallel S3 upload operations
- Concurrent uploads with connection pooling
- Target: >600 ops/sec (3x improvement)
- Graceful handling of rate limits

**FR-3**: Mock S3 service for testing
- In-memory S3-compatible service
- Deterministic behavior (no network variance)
- Support for error injection
- Compatible with existing ObjectStore trait

**FR-4**: Load testing framework
- Sustained 100 QPS for 10 minutes
- Measure P95/P99 latency under load
- Identify memory leaks and bottlenecks
- Generate performance report

**FR-5**: Advanced E2E test scenarios
- 15 new test scenarios covering edge cases
- Race conditions (concurrent tier transitions)
- Quota enforcement (memory limits, storage limits)
- Failure modes (S3 rate limits, disk full)

### 2.2 Non-Functional Requirements

**NFR-1**: Zero data loss during parallel operations

**NFR-2**: Graceful degradation under S3 rate limits

**NFR-3**: Memory usage <1GB for 100k vectors @ 100 QPS

**NFR-4**: All E2E tests complete in <15 minutes

**NFR-5**: Backward compatible with Week 1-3 code

### 2.3 Constraints

**C-1**: Cannot change ObjectStore trait interface (backward compat)

**C-2**: Must work with both real S3 and mock S3

**C-3**: Batch operations must be optional (config flag)

**C-4**: Load tests must run in CI/CD (GitHub Actions)

---

## 3. Performance Optimization Strategy

### 3.1 Batch S3 Uploads

**Problem**: Currently uploading 1 snapshot = 1 S3 PUT request

**Solution**: Group multiple snapshots into single multipart upload

**Design**:
```rust
pub struct BatchUploader {
    object_store: Arc<dyn ObjectStore>,
    batch_size: usize,           // Default: 10 snapshots per batch
    flush_interval: Duration,     // Default: 5 seconds
    pending: RwLock<Vec<PendingUpload>>,
}

struct PendingUpload {
    key: String,
    data: Bytes,
    enqueued_at: Instant,
}

impl BatchUploader {
    /// Add snapshot to batch queue
    pub async fn enqueue(&self, key: String, data: Bytes) -> CoreResult<()> {
        let mut pending = self.pending.write().await;
        pending.push(PendingUpload {
            key,
            data,
            enqueued_at: Instant::now(),
        });

        // Auto-flush if batch size reached
        if pending.len() >= self.batch_size {
            drop(pending);
            self.flush().await?;
        }

        Ok(())
    }

    /// Flush all pending uploads
    pub async fn flush(&self) -> CoreResult<()> {
        let pending = {
            let mut p = self.pending.write().await;
            std::mem::take(&mut *p)
        };

        if pending.is_empty() {
            return Ok(());
        }

        // Group uploads by prefix (collection_id)
        let mut batches: HashMap<String, Vec<PendingUpload>> = HashMap::new();
        for upload in pending {
            let prefix = extract_prefix(&upload.key);
            batches.entry(prefix).or_default().push(upload);
        }

        // Upload each batch
        for (prefix, uploads) in batches {
            self.upload_batch(prefix, uploads).await?;
        }

        Ok(())
    }

    async fn upload_batch(&self, prefix: String, uploads: Vec<PendingUpload>) -> CoreResult<()> {
        // S3 multipart upload
        let upload_id = self.object_store.initiate_multipart_upload(&prefix).await?;

        let mut parts = Vec::new();
        for (i, upload) in uploads.iter().enumerate() {
            let part_num = (i + 1) as u32;
            let etag = self.object_store
                .upload_part(&prefix, upload_id, part_num, upload.data.clone())
                .await?;
            parts.push((part_num, etag));
        }

        self.object_store.complete_multipart_upload(&prefix, upload_id, parts).await?;

        Ok(())
    }
}
```

**Performance Impact**:
- Before: 200 ops/sec (1 request per snapshot)
- After: 500 ops/sec (10 snapshots per request)
- **Improvement**: 2.5x

**Tradeoffs**:
- **Latency**: Individual snapshots delayed by flush interval
- **Complexity**: Requires multipart upload support
- **Memory**: Buffers snapshots in RAM before upload

### 3.2 Parallel S3 Uploads

**Problem**: Single-threaded uploads bottleneck on I/O wait

**Solution**: Concurrent uploads with tokio::spawn

**Design**:
```rust
pub struct ParallelUploader {
    object_store: Arc<dyn ObjectStore>,
    concurrency: usize,          // Default: 10 concurrent uploads
    semaphore: Arc<Semaphore>,   // Limit concurrent operations
}

impl ParallelUploader {
    pub fn new(object_store: Arc<dyn ObjectStore>, concurrency: usize) -> Self {
        Self {
            object_store,
            concurrency,
            semaphore: Arc::new(Semaphore::new(concurrency)),
        }
    }

    /// Upload multiple snapshots in parallel
    pub async fn upload_many(
        &self,
        uploads: Vec<(String, Bytes)>,
    ) -> CoreResult<Vec<UploadResult>> {
        let mut tasks = Vec::new();

        for (key, data) in uploads {
            let store = Arc::clone(&self.object_store);
            let semaphore = Arc::clone(&self.semaphore);

            let task = tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();

                store.put(&key, data).await.map(|_| UploadResult {
                    key,
                    success: true,
                    error: None,
                })
            });

            tasks.push(task);
        }

        // Wait for all uploads
        let results = futures::future::join_all(tasks).await;

        // Collect results
        results.into_iter()
            .map(|r| r.unwrap_or_else(|e| UploadResult {
                key: "unknown".to_string(),
                success: false,
                error: Some(e.to_string()),
            }))
            .collect()
    }
}

struct UploadResult {
    key: String,
    success: bool,
    error: Option<String>,
}
```

**Performance Impact**:
- Before: 200 ops/sec (single-threaded)
- After: 600 ops/sec (10 concurrent workers)
- **Improvement**: 3x

**Tradeoffs**:
- **Memory**: Higher RAM usage for concurrent buffers
- **S3 rate limits**: May trigger 503 throttling
- **Error handling**: More complex with concurrent failures

### 3.3 Connection Pooling

**Problem**: New HTTP connection for each S3 request (handshake overhead)

**Solution**: HTTP client with connection pool

**Design**:
```rust
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;

pub struct S3ObjectStore {
    client: Client<HttpsConnector<HttpConnector>>,
    config: S3Config,
}

impl S3ObjectStore {
    pub fn new(config: S3Config) -> Self {
        // Connection pool with keep-alive
        let https = HttpsConnector::new();
        let client = Client::builder()
            .pool_max_idle_per_host(20)      // Reuse up to 20 connections
            .pool_idle_timeout(Duration::from_secs(90))
            .build(https);

        Self { client, config }
    }
}
```

**Performance Impact**:
- Before: ~50ms per request (with handshake)
- After: ~20ms per request (reusing connections)
- **Improvement**: 2.5x faster requests

### 3.4 Combined Optimization

**Stacked Optimizations**:
1. Connection pooling: 2.5x faster requests
2. Batch uploads: 2.5x fewer requests
3. Parallel uploads: 3x concurrent throughput

**Combined Impact**:
- Before: 200 ops/sec
- After: 200 × 2.5 × 2.5 × 3 = **3,750 ops/sec** (theoretical max)
- **Realistic target**: 600 ops/sec (accounting for overhead)

---

## 4. Mock S3 Infrastructure

### 4.1 Why Mock S3?

**Problems with Real S3**:
- Network latency variance (50-500ms)
- Rate limits (unpredictable)
- Cost (API calls charge)
- CI/CD complexity (credentials, setup)

**Benefits of Mock S3**:
- Deterministic latency (always <1ms)
- No rate limits (for testing)
- Free (in-memory)
- Easy CI/CD integration (no credentials)

### 4.2 Mock S3 Design

**Requirements**:
1. Implement ObjectStore trait (drop-in replacement)
2. In-memory storage (HashMap)
3. Configurable latency simulation
4. Error injection (for testing failures)
5. Metrics tracking (upload count, bytes, etc.)

**Implementation**:
```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use bytes::Bytes;

pub struct MockS3ObjectStore {
    /// In-memory storage
    storage: Arc<RwLock<HashMap<String, Bytes>>>,

    /// Simulated latency (for realistic testing)
    latency: Duration,

    /// Error injection mode
    error_mode: Arc<RwLock<Option<MockS3Error>>>,

    /// Metrics
    metrics: Arc<RwLock<MockS3Metrics>>,
}

#[derive(Debug, Clone)]
pub enum MockS3Error {
    Throttle,           // 503 throttling
    ServerError,        // 500 server error
    NotFound,           // 404 not found
    Forbidden,          // 403 forbidden
}

#[derive(Debug, Default)]
pub struct MockS3Metrics {
    pub put_count: u64,
    pub get_count: u64,
    pub delete_count: u64,
    pub list_count: u64,
    pub bytes_uploaded: u64,
    pub bytes_downloaded: u64,
}

impl MockS3ObjectStore {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(RwLock::new(HashMap::new())),
            latency: Duration::from_millis(0),
            error_mode: Arc::new(RwLock::new(None)),
            metrics: Arc::new(RwLock::new(MockS3Metrics::default())),
        }
    }

    /// Configure simulated latency
    pub fn with_latency(mut self, latency: Duration) -> Self {
        self.latency = latency;
        self
    }

    /// Inject error for next operation
    pub async fn inject_error(&self, error: MockS3Error) {
        *self.error_mode.write().await = Some(error);
    }

    /// Clear error injection
    pub async fn clear_errors(&self) {
        *self.error_mode.write().await = None;
    }

    /// Get current metrics
    pub async fn metrics(&self) -> MockS3Metrics {
        self.metrics.read().await.clone()
    }

    /// Reset metrics
    pub async fn reset_metrics(&self) {
        *self.metrics.write().await = MockS3Metrics::default();
    }
}

#[async_trait]
impl ObjectStore for MockS3ObjectStore {
    async fn put(&self, key: &str, data: Bytes) -> CoreResult<()> {
        // Simulate latency
        if self.latency > Duration::ZERO {
            tokio::time::sleep(self.latency).await;
        }

        // Check for error injection
        if let Some(error) = self.error_mode.read().await.as_ref() {
            return match error {
                MockS3Error::Throttle => Err(CoreError::StorageError("503 throttle".into())),
                MockS3Error::ServerError => Err(CoreError::StorageError("500 server error".into())),
                MockS3Error::Forbidden => Err(CoreError::StorageError("403 forbidden".into())),
                MockS3Error::NotFound => Err(CoreError::NotFound("404 not found".into())),
            };
        }

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.put_count += 1;
            metrics.bytes_uploaded += data.len() as u64;
        }

        // Store data
        self.storage.write().await.insert(key.to_string(), data);

        Ok(())
    }

    async fn get(&self, key: &str) -> CoreResult<Bytes> {
        // Simulate latency
        if self.latency > Duration::ZERO {
            tokio::time::sleep(self.latency).await;
        }

        // Check for error injection
        if let Some(error) = self.error_mode.read().await.as_ref() {
            return match error {
                MockS3Error::NotFound => Err(CoreError::NotFound("404 not found".into())),
                _ => Err(CoreError::StorageError("Error injected".into())),
            };
        }

        // Update metrics
        self.metrics.write().await.get_count += 1;

        // Get data
        let storage = self.storage.read().await;
        let data = storage.get(key)
            .ok_or_else(|| CoreError::NotFound(format!("Key not found: {}", key)))?
            .clone();

        // Update metrics
        self.metrics.write().await.bytes_downloaded += data.len() as u64;

        Ok(data)
    }

    async fn delete(&self, key: &str) -> CoreResult<()> {
        if self.latency > Duration::ZERO {
            tokio::time::sleep(self.latency).await;
        }

        self.metrics.write().await.delete_count += 1;
        self.storage.write().await.remove(key);

        Ok(())
    }

    async fn list(&self, prefix: &str) -> CoreResult<Vec<String>> {
        if self.latency > Duration::ZERO {
            tokio::time::sleep(self.latency).await;
        }

        self.metrics.write().await.list_count += 1;

        let storage = self.storage.read().await;
        let keys: Vec<String> = storage.keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();

        Ok(keys)
    }
}
```

**Testing with Mock S3**:
```rust
#[tokio::test]
async fn test_batch_upload_with_mock_s3() {
    let mock_s3 = Arc::new(MockS3ObjectStore::new().with_latency(Duration::from_millis(10)));
    let batch_uploader = BatchUploader::new(mock_s3.clone(), 10);

    // Enqueue 100 uploads
    for i in 0..100 {
        let key = format!("snapshot-{}.parquet", i);
        let data = Bytes::from(vec![0u8; 1024]);  // 1KB
        batch_uploader.enqueue(key, data).await.unwrap();
    }

    // Flush
    batch_uploader.flush().await.unwrap();

    // Verify metrics
    let metrics = mock_s3.metrics().await;
    assert_eq!(metrics.put_count, 10, "Expected 10 batch uploads");
    assert_eq!(metrics.bytes_uploaded, 100 * 1024, "Expected 100KB uploaded");
}
```

---

## 5. Load Testing Framework

### 5.1 Load Test Requirements

**Scenario**: Simulate production traffic for 10 minutes

**Load Profile**:
- Sustained 100 QPS (queries per second)
- Mix of operations: 70% search, 20% insert, 10% tier control
- Dataset: 100k vectors (512-dim) across 100 collections

**Metrics to Track**:
- P50/P95/P99 search latency
- Throughput (actual QPS achieved)
- Error rate (5xx errors, timeouts)
- Memory usage (RSS, heap)
- CPU usage (user + system)

**Success Criteria**:
- P95 search latency <25ms
- Error rate <0.1%
- Memory stable (no leaks)
- CPU <80% average

### 5.2 Load Test Implementation

**Framework**: Custom load test using tokio + tracing

```rust
use tokio::time::{interval, Duration, Instant};
use tracing::{info, warn};

pub struct LoadTest {
    service: Arc<CollectionService>,
    config: LoadTestConfig,
    metrics: Arc<RwLock<LoadTestMetrics>>,
}

pub struct LoadTestConfig {
    pub duration: Duration,       // 10 minutes
    pub target_qps: usize,         // 100 QPS
    pub search_pct: f32,           // 70% search
    pub insert_pct: f32,           // 20% insert
    pub tier_control_pct: f32,     // 10% tier control
    pub dataset_size: usize,       // 100k vectors
    pub collection_count: usize,   // 100 collections
}

#[derive(Debug, Default)]
pub struct LoadTestMetrics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub search_latencies: Vec<Duration>,
    pub insert_latencies: Vec<Duration>,
    pub tier_control_latencies: Vec<Duration>,
    pub error_counts: HashMap<String, u64>,
}

impl LoadTest {
    pub async fn run(&self) -> LoadTestReport {
        info!("Starting load test: {} QPS for {:?}", self.config.target_qps, self.config.duration);

        let start = Instant::now();
        let end = start + self.config.duration;

        // Spawn worker tasks
        let mut tasks = Vec::new();
        let workers = self.config.target_qps / 10;  // 10 workers for 100 QPS

        for i in 0..workers {
            let task = self.spawn_worker(i, start, end);
            tasks.push(task);
        }

        // Wait for all workers
        futures::future::join_all(tasks).await;

        // Generate report
        self.generate_report(start.elapsed()).await
    }

    async fn spawn_worker(&self, worker_id: usize, start: Instant, end: Instant) -> tokio::task::JoinHandle<()> {
        let service = Arc::clone(&self.service);
        let config = self.config.clone();
        let metrics = Arc::clone(&self.metrics);

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(100));  // 10 ops/sec per worker

            loop {
                interval.tick().await;

                if Instant::now() > end {
                    break;
                }

                // Select operation based on distribution
                let op_type = Self::select_operation(&config);

                let latency = match op_type {
                    OpType::Search => Self::execute_search(&service, &config).await,
                    OpType::Insert => Self::execute_insert(&service, &config).await,
                    OpType::TierControl => Self::execute_tier_control(&service, &config).await,
                };

                // Record metrics
                let mut m = metrics.write().await;
                m.total_requests += 1;

                match latency {
                    Ok(duration) => {
                        m.successful_requests += 1;
                        match op_type {
                            OpType::Search => m.search_latencies.push(duration),
                            OpType::Insert => m.insert_latencies.push(duration),
                            OpType::TierControl => m.tier_control_latencies.push(duration),
                        }
                    }
                    Err(e) => {
                        m.failed_requests += 1;
                        *m.error_counts.entry(e.to_string()).or_default() += 1;
                    }
                }
            }
        })
    }

    async fn generate_report(&self, duration: Duration) -> LoadTestReport {
        let metrics = self.metrics.read().await;

        let mut search_latencies = metrics.search_latencies.clone();
        search_latencies.sort();

        let p50 = percentile(&search_latencies, 0.50);
        let p95 = percentile(&search_latencies, 0.95);
        let p99 = percentile(&search_latencies, 0.99);

        let actual_qps = metrics.total_requests as f64 / duration.as_secs_f64();
        let error_rate = metrics.failed_requests as f64 / metrics.total_requests as f64;

        LoadTestReport {
            duration,
            total_requests: metrics.total_requests,
            successful_requests: metrics.successful_requests,
            failed_requests: metrics.failed_requests,
            actual_qps,
            target_qps: self.config.target_qps as f64,
            search_p50: p50,
            search_p95: p95,
            search_p99: p99,
            error_rate,
            errors: metrics.error_counts.clone(),
        }
    }
}

pub struct LoadTestReport {
    pub duration: Duration,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub actual_qps: f64,
    pub target_qps: f64,
    pub search_p50: Duration,
    pub search_p95: Duration,
    pub search_p99: Duration,
    pub error_rate: f64,
    pub errors: HashMap<String, u64>,
}

impl LoadTestReport {
    pub fn print(&self) {
        println!("\n=== Load Test Report ===");
        println!("Duration: {:?}", self.duration);
        println!("Total Requests: {}", self.total_requests);
        println!("Successful: {}", self.successful_requests);
        println!("Failed: {}", self.failed_requests);
        println!("Actual QPS: {:.2} (target: {:.2})", self.actual_qps, self.target_qps);
        println!("Search Latency:");
        println!("  P50: {:?}", self.search_p50);
        println!("  P95: {:?}", self.search_p95);
        println!("  P99: {:?}", self.search_p99);
        println!("Error Rate: {:.2}%", self.error_rate * 100.0);
        println!("Errors: {:?}", self.errors);
    }

    pub fn assert_success_criteria(&self) -> Result<(), String> {
        // P95 <25ms
        if self.search_p95 > Duration::from_millis(25) {
            return Err(format!("P95 latency {}ms exceeds 25ms", self.search_p95.as_millis()));
        }

        // Error rate <0.1%
        if self.error_rate > 0.001 {
            return Err(format!("Error rate {:.2}% exceeds 0.1%", self.error_rate * 100.0));
        }

        // Actual QPS within 10% of target
        let qps_diff = (self.actual_qps - self.target_qps).abs() / self.target_qps;
        if qps_diff > 0.1 {
            return Err(format!("QPS {:.2} deviates >10% from target {:.2}", self.actual_qps, self.target_qps));
        }

        Ok(())
    }
}
```

---

## 6. Advanced E2E Test Scenarios

### 6.1 Test Categories

**15 Advanced E2E Tests** covering edge cases not tested in Week 3:

**Category 1: Concurrency & Race Conditions (5 tests)**
1. `test_concurrent_demotions_same_collection` - Multiple workers demoting same collection
2. `test_concurrent_promotions_and_searches` - Promotion while search in progress
3. `test_concurrent_snapshot_and_restore` - Snapshot while restore in progress
4. `test_race_condition_tier_state_update` - Concurrent tier state updates
5. `test_background_worker_concurrent_with_manual_tier_control` - Worker cycle + API calls

**Category 2: Quota & Limits (4 tests)**
6. `test_memory_quota_enforcement` - Reject insert when memory quota exceeded
7. `test_storage_quota_enforcement` - Reject snapshot when storage quota exceeded
8. `test_collection_count_limit` - Enforce max collections per tenant
9. `test_warm_tier_disk_full` - Handle disk full during warm tier save

**Category 3: Failure Modes (6 tests)**
10. `test_s3_rate_limit_handling` - Graceful handling of 503 throttle
11. `test_s3_connection_timeout` - Retry on connection timeout
12. `test_partial_snapshot_upload` - Handle incomplete multipart upload
13. `test_corrupted_parquet_file_recovery` - Detect and recover from corruption
14. `test_background_worker_restart_mid_cycle` - Resume after worker crash
15. `test_tier_demotion_rollback_on_s3_failure` - Rollback if S3 upload fails

### 6.2 Example Test Implementation

```rust
#[tokio::test]
async fn test_concurrent_demotions_same_collection() {
    let service = Arc::new(setup_test_service().await);
    let (collection_id, _) = create_test_collection(&service, 1_000, 128).await;

    // Simulate 10 workers trying to demote same collection concurrently
    let mut tasks = Vec::new();
    for i in 0..10 {
        let service = Arc::clone(&service);
        let task = tokio::spawn(async move {
            service.storage_backend
                .force_demote_to_warm(collection_id)
                .await
        });
        tasks.push(task);
    }

    // Wait for all tasks
    let results = futures::future::join_all(tasks).await;

    // Exactly one should succeed, others should be no-op or return "already demoted"
    let successes = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(successes, 1, "Expected exactly 1 successful demotion");

    // Verify final state is warm
    assert_tier_state(&service, collection_id, Tier::Warm).await;

    // Verify no duplicate snapshots
    let snapshots = service.storage_backend.list_snapshots(collection_id).await.unwrap();
    assert_eq!(snapshots.len(), 0, "Warm tier should not create snapshots");
}

#[tokio::test]
async fn test_memory_quota_enforcement() {
    let mut config = Config::default();
    config.tenant_quota = Some(TenantQuota {
        memory_quota_bytes: 100 * 1024 * 1024,  // 100MB
        storage_quota_bytes: 1024 * 1024 * 1024,  // 1GB
        qps_quota: 1000,
    });

    let service = setup_test_service_with_config(config).await;

    // Insert vectors until quota exceeded
    let mut inserted = 0;
    let vector_size = 512 * 4;  // 512-dim * 4 bytes per float = 2KB

    loop {
        let result = service.insert_vector(
            collection_id,
            generate_test_vector(512),
        ).await;

        match result {
            Ok(_) => inserted += 1,
            Err(e) if e.to_string().contains("quota") => break,
            Err(e) => panic!("Unexpected error: {}", e),
        }

        // Safety limit
        if inserted > 1_000_000 {
            panic!("Quota not enforced after 1M vectors");
        }
    }

    // Verify quota was enforced around expected limit
    let expected_max = 100 * 1024 * 1024 / vector_size;
    assert!(
        inserted >= expected_max * 9 / 10 && inserted <= expected_max * 11 / 10,
        "Quota enforcement inaccurate: inserted {} vs expected {}",
        inserted,
        expected_max
    );
}

#[tokio::test]
async fn test_s3_rate_limit_handling() {
    let mock_s3 = Arc::new(MockS3ObjectStore::new());
    let service = setup_test_service_with_mock_s3(mock_s3.clone()).await;

    let (collection_id, _) = create_test_collection(&service, 1_000, 128).await;

    // Inject throttle error for first 3 attempts
    mock_s3.inject_error(MockS3Error::Throttle).await;

    tokio::spawn({
        let mock_s3 = Arc::clone(&mock_s3);
        async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            mock_s3.clear_errors().await;  // Clear after 2 seconds
        }
    });

    // Attempt snapshot (should retry and eventually succeed)
    let start = Instant::now();
    let snapshot_id = service.storage_backend
        .create_manual_snapshot(collection_id)
        .await
        .unwrap();

    let duration = start.elapsed();

    // Verify retry happened (took >2s due to throttle)
    assert!(duration > Duration::from_secs(2), "Expected retry delay");

    // Verify snapshot eventually created
    let snapshots = service.storage_backend.list_snapshots(collection_id).await.unwrap();
    assert_eq!(snapshots.len(), 1);
}
```

---

## 7. Profiling and Optimization

### 7.1 CPU Profiling

**Tool**: `cargo-flamegraph`

```bash
# Install
cargo install flamegraph

# Profile REST server under load
cargo flamegraph --bin akidb-rest -- --config config.toml

# Run load test in separate terminal
./scripts/load-test.sh

# Ctrl+C to stop, generates flamegraph.svg
```

**Analyze**:
- Look for hot paths (functions consuming >10% CPU)
- Identify unnecessary allocations
- Find synchronous operations blocking async runtime

**Common Optimizations**:
1. Replace `HashMap` with `DashMap` for concurrent access
2. Use `Arc::clone()` instead of data copying
3. Batch database queries (N+1 problem)
4. Cache frequently accessed data

### 7.2 Memory Profiling

**Tool**: `heaptrack` (Linux) or `Instruments` (macOS)

```bash
# macOS (Instruments)
instruments -t "Allocations" -D allocations.trace target/release/akidb-rest

# Linux (heaptrack)
heaptrack target/release/akidb-rest
```

**Analyze**:
- Check for memory leaks (RSS growing unbounded)
- Identify large allocations
- Look for excessive cloning

**Common Fixes**:
1. Use `Bytes::from_static()` for static data
2. Implement `Drop` for cleanup
3. Limit cache sizes (LRU eviction)
4. Use `Vec::with_capacity()` to avoid reallocations

### 7.3 Performance Regression Tests

**Continuous Profiling**: Run benchmarks on every commit

```bash
#!/bin/bash
# scripts/perf-regression-check.sh

# Run benchmarks and save baseline
cargo bench --bench integration_bench -- --save-baseline current

# Compare with main branch
git checkout main
cargo bench --bench integration_bench -- --save-baseline main

git checkout -
cargo bench --bench integration_bench -- --baseline main

# Fail if >10% regression
if grep -q "change.*+[1-9][0-9]\%" target/criterion/*/report/index.html; then
    echo "❌ Performance regression detected!"
    exit 1
else
    echo "✅ No performance regression"
fi
```

---

## 8. Implementation Plan

### Day 1: Batch S3 Uploads (4-5 hours)
- Implement `BatchUploader` struct
- Add multipart upload support to ObjectStore
- Write 3 tests (batch queue, flush, multipart)
- Benchmark: Measure ops/sec improvement

### Day 2: Parallel S3 Uploads (4-5 hours)
- Implement `ParallelUploader` struct
- Add connection pooling to S3ObjectStore
- Write 3 tests (parallel upload, semaphore, rate limiting)
- Benchmark: Measure ops/sec improvement

### Day 3: Mock S3 Service (4-5 hours)
- Implement `MockS3ObjectStore`
- Add error injection framework
- Add metrics tracking
- Write 5 tests (basic ops, errors, metrics)

### Day 4: Load Testing Framework (4-5 hours)
- Implement `LoadTest` struct
- Add multi-worker load generation
- Add metrics collection and reporting
- Run 10-minute load test @ 100 QPS

### Day 5: Advanced E2E Tests (4-5 hours)
- Implement 15 advanced E2E test scenarios
- Run CPU and memory profiling
- Identify and fix bottlenecks
- Generate Week 4 completion report

---

## 9. Success Criteria

### 9.1 Performance
- ✅ Batch S3 uploads: >500 ops/sec
- ✅ Parallel S3 uploads: >600 ops/sec
- ✅ Load test: 100 QPS sustained for 10 minutes
- ✅ Search P95 <25ms under load
- ✅ Error rate <0.1% under load

### 9.2 Testing
- ✅ 15 advanced E2E tests passing
- ✅ Mock S3 service fully functional
- ✅ Load test report generated
- ✅ Profiling data collected

### 9.3 Quality
- ✅ Zero data loss in concurrent scenarios
- ✅ Graceful handling of S3 rate limits
- ✅ Memory usage stable under load
- ✅ CPU usage <80% under load

---

## 10. Risk Analysis

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| S3 rate limits in production | High | Medium | Exponential backoff, circuit breaker |
| Memory leaks under sustained load | Medium | High | Memory profiling, automated leak detection |
| Race conditions in concurrent tier transitions | Medium | High | Integration tests, lock analysis |
| Multipart upload complexity | Low | Medium | Use battle-tested library (aws-sdk-s3) |
| Load test too slow for CI/CD | Low | Low | Reduce duration to 1 minute for CI |

---

**Status**: ✅ MEGATHINK COMPLETE - READY FOR PRD CREATION

**Next**: Create detailed PRD document for Phase 10 Week 4
