# Phase 10 Week 3: Integration Testing + RC2 Release - Comprehensive Megathink

**Date**: 2025-11-09
**Phase**: Phase 10 Week 3
**Focus**: E2E Integration Testing, Performance Benchmarks, RC2 Release
**Status**: 🔍 ANALYSIS & PLANNING

---

## Executive Summary

**Objective**: Validate all Week 1-2 deliverables through comprehensive integration testing, performance benchmarking, and prepare for RC2 release.

**Business Value**:
- Production-ready v2.0.0-rc2 release with S3/MinIO tiering
- Validated performance targets (all benchmarks met)
- Comprehensive test coverage (20+ E2E scenarios)
- Enterprise-grade documentation (deployment, S3 setup, tuning)
- Confidence in crash recovery and data integrity

**Technical Approach**:
- Full workflow E2E tests (insert → tier → snapshot → restore)
- Crash recovery scenarios with S3 (node restart, network failures)
- Performance benchmarks (snapshot, tiering, search latency)
- Stress tests (large datasets, concurrent operations)
- Documentation updates (S3 config, tiering tuning, migration guide)
- Release preparation (changelog, version bump, Docker images)

**Timeline**: 5 days (Week 3 of Phase 10)

---

## Table of Contents

1. [Background & Context](#1-background--context)
2. [Problem Statement](#2-problem-statement)
3. [Integration Test Strategy](#3-integration-test-strategy)
4. [Performance Benchmark Design](#4-performance-benchmark-design)
5. [Crash Recovery Scenarios](#5-crash-recovery-scenarios)
6. [Documentation Requirements](#6-documentation-requirements)
7. [Release Preparation](#7-release-preparation)
8. [Implementation Plan](#8-implementation-plan)
9. [Success Criteria](#9-success-criteria)
10. [Risk Analysis](#10-risk-analysis)

---

## 1. Background & Context

### 1.1 What We've Built (Week 1-2)

**Week 1: Parquet Snapshotter**
- Efficient columnar storage for vector snapshots
- S3/MinIO upload/download integration
- 2-3x compression vs JSON
- 21 tests passing

**Week 2: Hot/Warm/Cold Tiering**
- Automatic tier transitions based on access patterns
- LRU-based access tracking
- Background worker for demotions/promotions
- Manual tier control API
- 26 tests passing

**Total Code**:
- Week 1: ~500 lines production code + 10 tests
- Week 2: ~1,200 lines production code + 26 tests
- **Combined**: ~1,700 lines code + 47 tests

### 1.2 What Week 3 Delivers

**Primary Goal**: Validate that Week 1 + Week 2 work together correctly in production scenarios.

**Key Questions to Answer**:
1. Does the full workflow work? (Insert → Tier → Snapshot → Restore)
2. Can we recover from crashes? (Node restart, S3 failures)
3. Do we meet performance targets? (<2s snapshot, <10s restore)
4. Is the system production-ready? (Documentation, monitoring, deployment)

**Deliverables**:
- 20+ E2E integration tests
- Performance benchmarks (with baseline metrics)
- Crash recovery test suite
- S3 configuration guide
- Tiering tuning guide
- Migration guide (v1.x → v2.0)
- Release candidate: v2.0.0-rc2

### 1.3 Why Week 3 is Critical

**Gap**: Individual components tested in isolation, but not together.

**Risk**: Integration bugs only discovered in production.

**Solution**: Comprehensive E2E testing before RC2 release.

**Example Scenario**:
- Week 1 test: "Snapshot creation works" ✅
- Week 2 test: "Tier demotion works" ✅
- Week 3 test: "Tier demotion → snapshot creation → S3 upload → node restart → automatic restore" ✅

---

## 2. Problem Statement

### 2.1 Core Requirements

**FR-1**: Full workflow integration tests
- Insert 100k vectors → demote to warm → demote to cold → search triggers restore → verify data integrity

**FR-2**: Crash recovery scenarios
- Node crash during snapshot upload → restart → verify recovery
- S3 connection lost during restore → retry → verify completion
- Background worker crash → restart → verify automatic resume

**FR-3**: Performance benchmarks
- Snapshot creation: <2s for 10k vectors (512-dim)
- Snapshot restore: <3s for 10k vectors
- Tier transitions: <5s for 10k vectors
- Search latency: P95 <25ms @ 100k vectors (with tiering)

**FR-4**: Documentation updates
- S3/MinIO configuration guide (AWS S3, MinIO, Oracle Cloud)
- Tiering tuning guide (optimize for workload)
- Migration guide (v1.x → v2.0 upgrade path)
- Deployment guide (Docker, Kubernetes, bare metal)

**FR-5**: Release preparation
- Version bump to v2.0.0-rc2
- Changelog generation (all features since RC1)
- Docker image build and publish
- Release notes with migration guide

### 2.2 Non-Functional Requirements

**NFR-1**: Test execution time <30 minutes total

**NFR-2**: All benchmarks reproducible (seed-based randomness)

**NFR-3**: Documentation includes runnable examples

**NFR-4**: RC2 release includes backward compatibility with RC1

**NFR-5**: Zero data loss in all crash recovery scenarios

### 2.3 Constraints

**C-1**: Tests must work with both LocalObjectStore and S3ObjectStore

**C-2**: Benchmarks must run on CI/CD (GitHub Actions)

**C-3**: Documentation must be beginner-friendly (no assumptions)

**C-4**: RC2 must be production-ready (no known critical bugs)

---

## 3. Integration Test Strategy

### 3.1 Test Pyramid for Week 3

```
                    ┌──────────────┐
                    │   1 RC2      │  Release smoke test
                    │   Release    │
                    └──────────────┘
                    ┌──────────────┐
                    │   5 Crash    │  Crash recovery scenarios
                    │   Recovery   │
                    └──────────────┘
               ┌─────────────────────┐
               │   20 E2E            │  Full workflow integration
               │   Integration       │
               └─────────────────────┘
          ┌──────────────────────────────┐
          │   10 Performance             │  Benchmarks
          │   Benchmarks                 │
          └──────────────────────────────┘
     ┌────────────────────────────────────────┐
     │   47 Unit + Component Tests            │  Week 1-2 tests
     │   (already passing)                    │
     └────────────────────────────────────────┘
```

**Total Week 3 Tests**: 36 new tests (20 E2E + 10 benchmarks + 5 crash + 1 smoke)

**Combined Total**: 83 tests (47 existing + 36 new)

### 3.2 E2E Integration Test Scenarios

**Category 1: Full Workflow Tests (8 tests)**

1. **test_insert_tier_snapshot_restore**
   - Insert 10k vectors → wait for demotion → verify snapshot on S3 → delete local data → search triggers restore → verify results match

2. **test_hot_to_cold_full_cycle**
   - Create collection (hot) → no access for 6h → demote to warm → no access for 7d → demote to cold → search → promote to warm → verify data

3. **test_concurrent_tier_transitions**
   - 10 collections → concurrent demotions/promotions → verify no race conditions → all data intact

4. **test_large_dataset_tiering**
   - Insert 100k vectors → tier to cold → restore → verify P95 latency <25ms

5. **test_pinned_collection_never_demoted**
   - Create collection → pin to hot → wait 7d → verify still hot → unpin → wait 6h → verify demoted to warm

6. **test_manual_tier_control_workflow**
   - Force demote to cold → verify snapshot created → force promote to hot → verify data restored

7. **test_multiple_snapshots_per_collection**
   - Create snapshot manually → demote to cold (auto-snapshot) → list snapshots → verify both exist

8. **test_tier_state_persistence_across_restart**
   - Create 5 collections in different tiers → shutdown server → restart → verify tier states preserved

**Category 2: S3 Integration Tests (6 tests)**

9. **test_s3_upload_large_snapshot**
   - Create 100k vector snapshot → upload to S3 → verify file size ~20MB (compressed)

10. **test_s3_download_and_restore**
    - Snapshot on S3 → download → deserialize → verify all vectors match original

11. **test_s3_list_snapshots**
    - Upload 5 snapshots → list via S3 → verify all returned with correct metadata

12. **test_s3_delete_snapshot**
    - Create snapshot → upload to S3 → delete via API → verify removed from S3

13. **test_s3_retry_on_transient_error**
    - Mock S3 500 error → attempt upload → verify retry → eventually succeed

14. **test_s3_fail_on_permanent_error**
    - Mock S3 403 (auth failure) → attempt upload → verify immediate failure (no retry)

**Category 3: Tiering + Snapshot Integration (6 tests)**

15. **test_warm_to_cold_creates_snapshot**
    - Collection in warm tier → demote to cold → verify ParquetSnapshotter called → snapshot on S3

16. **test_cold_to_warm_restores_snapshot**
    - Collection in cold tier → search → verify snapshot downloaded → data in warm tier → search succeeds

17. **test_access_tracking_resets_after_promotion**
    - Collection in warm → 15 accesses → promote to hot → access count reset → verify

18. **test_background_worker_demotes_multiple_collections**
    - 10 collections idle >6h → worker cycle → verify all demoted to warm

19. **test_background_worker_promotes_high_access_collections**
    - 5 warm collections with >10 accesses → worker cycle → verify promoted to hot

20. **test_tiering_disabled_collections_always_hot**
    - Disable tiering in config → create collection → wait 7d → verify still hot

**Total**: 20 E2E integration tests

### 3.3 Test Infrastructure

**Mock S3 Service** (for testing):
```rust
pub struct MockS3ObjectStore {
    storage: Arc<RwLock<HashMap<String, Bytes>>>,
    error_mode: Arc<RwLock<Option<S3Error>>>,  // Inject errors for testing
}

impl MockS3ObjectStore {
    pub fn inject_error(&self, error: S3Error) {
        *self.error_mode.write() = Some(error);
    }

    pub fn clear_errors(&self) {
        *self.error_mode.write() = None;
    }
}
```

**Test Helpers**:
```rust
async fn create_test_collection_with_vectors(
    service: &CollectionService,
    count: usize,
    dimension: usize,
) -> (CollectionId, Vec<VectorDocument>);

async fn simulate_time_passage(manager: &TieringManager, duration: Duration);

async fn assert_tier_state(
    service: &CollectionService,
    collection_id: CollectionId,
    expected_tier: Tier,
);

async fn assert_vectors_match(
    actual: &[VectorDocument],
    expected: &[VectorDocument],
    epsilon: f32,
);
```

---

## 4. Performance Benchmark Design

### 4.1 Benchmark Categories

**Category 1: Snapshot Performance (3 benchmarks)**

1. **bench_snapshot_create_10k**
   - Target: <2s for 10k vectors (512-dim)
   - Measure: Encoding + S3 upload time
   - Baseline: Compare to JSON snapshotter (should be 2-3x faster)

2. **bench_snapshot_restore_10k**
   - Target: <3s for 10k vectors
   - Measure: S3 download + decoding time
   - Baseline: Compare to JSON snapshotter

3. **bench_snapshot_compression_ratio**
   - Target: >2x compression vs JSON
   - Measure: Parquet file size / JSON file size
   - Dataset: 10k vectors with metadata

**Category 2: Tiering Performance (4 benchmarks)**

4. **bench_tier_hot_to_warm_10k**
   - Target: <2s for 10k vectors
   - Measure: Serialization + disk write time

5. **bench_tier_warm_to_hot_10k**
   - Target: <3s for 10k vectors
   - Measure: Disk read + deserialization time

6. **bench_tier_warm_to_cold_10k**
   - Target: <5s for 10k vectors (includes S3 upload)
   - Measure: Snapshot creation + S3 upload

7. **bench_tier_cold_to_warm_10k**
   - Target: <10s for 100k vectors
   - Measure: S3 download + warm tier save

**Category 3: Search Performance with Tiering (3 benchmarks)**

8. **bench_search_hot_tier_100k**
   - Target: P95 <5ms
   - Measure: Search latency with 100k vectors in RAM

9. **bench_search_warm_tier_100k**
   - Target: P95 <25ms
   - Measure: Search latency with 100k vectors on disk (first access)

10. **bench_search_cold_tier_first_access**
    - Target: <10s (includes S3 download)
    - Measure: Search latency on first access (cold tier)

**Total**: 10 performance benchmarks

### 4.2 Benchmark Infrastructure

**Criterion.rs Integration**:
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

fn snapshot_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("snapshot");

    group.bench_function("create_10k", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (snapshotter, vectors) = rt.block_on(setup_snapshotter(10_000, 512));

        b.to_async(&rt).iter(|| async {
            black_box(snapshotter.create_snapshot(collection_id, vectors.clone()).await.unwrap())
        });
    });

    group.finish();
}

criterion_group!(benches, snapshot_benchmarks, tiering_benchmarks, search_benchmarks);
criterion_main!(benches);
```

**Baseline Comparison**:
```bash
# Run benchmarks and save baseline
cargo bench --bench integration_bench -- --save-baseline week3

# Compare with RC1 baseline
cargo bench --bench integration_bench -- --baseline rc1

# Expected output:
# snapshot_create_10k    time:   [1.8s 1.9s 2.0s]  change: [-15% -12% -10%]  (improvement)
```

---

## 5. Crash Recovery Scenarios

### 5.1 Recovery Test Categories

**Category 1: Node Crashes (3 tests)**

1. **test_crash_during_snapshot_upload**
   - Scenario: Node crashes mid-upload to S3
   - Recovery: On restart, detect incomplete upload → retry from beginning
   - Verification: Snapshot eventually uploaded, no corruption

2. **test_crash_during_tier_demotion**
   - Scenario: Node crashes while demoting hot → warm (disk write in progress)
   - Recovery: On restart, detect incomplete demotion → rollback to hot tier
   - Verification: Collection still in hot tier, no data loss

3. **test_crash_during_background_worker_cycle**
   - Scenario: Background worker crashes mid-cycle
   - Recovery: On restart, worker resumes from beginning of cycle
   - Verification: All idle collections eventually demoted

**Category 2: Network Failures (2 tests)**

4. **test_s3_connection_lost_during_upload**
   - Scenario: S3 connection drops mid-upload
   - Recovery: Exponential backoff retry → eventually succeed
   - Verification: Snapshot uploaded, checksum matches

5. **test_s3_connection_lost_during_download**
   - Scenario: S3 connection drops mid-download
   - Recovery: Retry download from beginning (S3 supports range requests)
   - Verification: Snapshot fully downloaded, no corruption

**Total**: 5 crash recovery tests

### 5.2 Crash Injection Framework

**Fault Injection Utilities**:
```rust
pub struct FaultInjector {
    crash_points: Arc<RwLock<HashMap<String, bool>>>,
}

impl FaultInjector {
    pub fn enable_crash_at(&self, point: &str) {
        self.crash_points.write().insert(point.to_string(), true);
    }

    pub fn check_crash_point(&self, point: &str) -> bool {
        self.crash_points.read().get(point).copied().unwrap_or(false)
    }
}

// Usage in code:
async fn upload_to_s3(&self, data: Bytes) -> CoreResult<()> {
    if self.fault_injector.check_crash_point("before_s3_upload") {
        panic!("Simulated crash before S3 upload");
    }

    self.object_store.put("key", data).await?;

    if self.fault_injector.check_crash_point("after_s3_upload") {
        panic!("Simulated crash after S3 upload");
    }

    Ok(())
}
```

**Recovery Verification**:
```rust
#[tokio::test]
async fn test_crash_during_snapshot_upload() {
    // Setup
    let service = create_test_service().await;
    let injector = service.fault_injector();

    // Enable crash point
    injector.enable_crash_at("after_s3_upload");

    // Attempt snapshot (will crash)
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        tokio_test::block_on(async {
            service.snapshot_collection(collection_id).await
        })
    }));
    assert!(result.is_err(), "Expected panic");

    // Simulate restart
    drop(service);
    let service = create_test_service().await;

    // Verify recovery
    let snapshots = service.list_snapshots(collection_id).await.unwrap();
    assert_eq!(snapshots.len(), 1, "Snapshot should be uploaded after recovery");
}
```

---

## 6. Documentation Requirements

### 6.1 S3 Configuration Guide

**File**: `docs/S3-CONFIGURATION-GUIDE.md`

**Sections**:
1. **Overview**: Why S3/MinIO for cold tier storage
2. **AWS S3 Setup**:
   - Create S3 bucket
   - Configure IAM credentials
   - Set bucket policies
   - Example config.toml
3. **MinIO Setup**:
   - Install MinIO locally
   - Create bucket
   - Configure access keys
   - Example config.toml
4. **Oracle Cloud Object Storage**:
   - Create bucket
   - Generate API keys
   - Configure endpoint
   - Example config.toml
5. **Security Best Practices**:
   - Encryption at rest (S3 SSE)
   - Encryption in transit (HTTPS)
   - IAM role recommendations
   - Credential rotation
6. **Troubleshooting**:
   - Connection errors
   - Authentication failures
   - Permission denied
   - Upload timeouts

**Example Content**:
```markdown
## AWS S3 Setup

### Step 1: Create S3 Bucket

bash
aws s3api create-bucket \
  --bucket akidb-cold-tier \
  --region us-west-2 \
  --create-bucket-configuration LocationConstraint=us-west-2


### Step 2: Create IAM User with S3 Access

bash
aws iam create-user --user-name akidb-s3-user
aws iam attach-user-policy \
  --user-name akidb-s3-user \
  --policy-arn arn:aws:iam::aws:policy/AmazonS3FullAccess


### Step 3: Generate Access Keys

bash
aws iam create-access-key --user-name akidb-s3-user


### Step 4: Configure AkiDB

toml
[storage.s3]
endpoint = "https://s3.us-west-2.amazonaws.com"
bucket = "akidb-cold-tier"
region = "us-west-2"
access_key_id = "AKIAIOSFODNN7EXAMPLE"
secret_access_key = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"

```

### 6.2 Tiering Tuning Guide

**File**: `docs/TIERING-TUNING-GUIDE.md`

**Sections**:
1. **Overview**: How tiering policies affect performance and cost
2. **Default Policies Explained**:
   - Hot tier TTL: 6 hours (why this default?)
   - Warm tier TTL: 7 days (why this default?)
   - Promotion threshold: 10 accesses in 1 hour
3. **Tuning for Different Workloads**:
   - **High Read Frequency**: Increase hot tier TTL to 24h
   - **Low Read Frequency**: Decrease hot tier TTL to 1h
   - **Cost-Optimized**: Aggressive demotion (1h hot, 1d warm)
   - **Performance-Optimized**: Conservative demotion (24h hot, 30d warm)
4. **Monitoring Tier Distribution**:
   - How to view tier metrics
   - Expected tier distribution for different workloads
   - When to adjust policies
5. **Case Studies**:
   - E-commerce (seasonal traffic)
   - Analytics (daily batch jobs)
   - RAG (chatbot, frequent access)

**Example Content**:
```markdown
## Workload Profiles

### High Read Frequency (RAG Chatbot)

Characteristics:
- 1000+ searches/day per collection
- Consistent access pattern
- Low latency required

Recommended Policy:
toml
[tiering.policy]
hot_tier_ttl_hours = 24       # Keep hot for 1 day
warm_tier_ttl_days = 30       # Keep warm for 30 days
hot_promotion_threshold = 5   # Promote on 5 accesses/hour


Expected Distribution:
- Hot: 80-90% of collections
- Warm: 10-15% of collections
- Cold: 0-5% of collections

### Low Read Frequency (Batch Analytics)

Characteristics:
- 10 searches/day per collection
- Scheduled jobs (nightly)
- Cost optimization priority

Recommended Policy:
toml
[tiering.policy]
hot_tier_ttl_hours = 1        # Demote after 1 hour
warm_tier_ttl_days = 1        # Demote after 1 day
hot_promotion_threshold = 20  # Rarely promote


Expected Distribution:
- Hot: 5-10% of collections
- Warm: 10-20% of collections
- Cold: 70-85% of collections
```

### 6.3 Migration Guide (v1.x → v2.0)

**File**: `docs/MIGRATION-V1-TO-V2.md`

**Sections**:
1. **Breaking Changes**:
   - New tiering system (optional, opt-in)
   - S3 configuration required for tiering
   - Database schema changes (new migrations)
2. **Migration Steps**:
   - Backup v1.x data
   - Install v2.0
   - Run migrations
   - Configure S3 (if using tiering)
   - Verify data integrity
3. **Rollback Procedure**:
   - Export v2.0 collections to JSON
   - Restore v1.x from backup
   - Import collections
4. **FAQ**:
   - Do I need to use tiering?
   - Can I upgrade without downtime?
   - How long does migration take?

### 6.4 Deployment Guide

**File**: `docs/DEPLOYMENT-GUIDE.md` (update existing)

**New Sections**:
- S3/MinIO setup for production
- Tiering configuration examples
- Monitoring tier distribution
- Backup and disaster recovery with S3

---

## 7. Release Preparation

### 7.1 RC2 Release Checklist

**Version Bump**:
- [ ] Update version in all `Cargo.toml` files: `2.0.0-rc2`
- [ ] Update version in `CHANGELOG.md`
- [ ] Update version in Docker files

**Changelog Generation**:
```markdown
# Changelog

## [2.0.0-rc2] - 2025-11-XX

### Added
- Parquet-based vector snapshots (2-3x compression vs JSON)
- Hot/warm/cold tiering policies for automatic cost optimization
- S3/MinIO integration for cold tier storage
- Background worker for automatic tier transitions
- Manual tier control API (pin, promote, demote)
- Access tracking with LRU eviction
- Crash recovery for snapshot operations
- 36 new integration tests (20 E2E + 10 benchmarks + 5 crash + 1 smoke)

### Changed
- StorageBackend now supports tiering (optional, disabled by default)
- CollectionService tracks access for tiering decisions
- Configuration adds [tiering] section

### Fixed
- (List any bug fixes from Week 1-2 testing)

### Performance
- Snapshot creation: <2s for 10k vectors (512-dim)
- Snapshot restore: <3s for 10k vectors
- Search P95 latency: <25ms @ 100k vectors (with tiering)
- Compression: 2-3x vs JSON snapshots

### Documentation
- S3 Configuration Guide
- Tiering Tuning Guide
- Migration Guide (v1.x → v2.0)
- Updated Deployment Guide
```

**Docker Image Build**:
```dockerfile
# Update Dockerfile
FROM rust:1.75-slim as builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/akidb-rest /usr/local/bin/
COPY --from=builder /app/target/release/akidb-grpc /usr/local/bin/
COPY config.example.toml /etc/akidb/config.toml

EXPOSE 8080 9090
CMD ["akidb-rest"]
```

**Build and Push**:
```bash
# Build Docker image
docker build -t akidb/akidb:2.0.0-rc2 .
docker tag akidb/akidb:2.0.0-rc2 akidb/akidb:latest

# Push to Docker Hub
docker push akidb/akidb:2.0.0-rc2
docker push akidb/akidb:latest
```

**GitHub Release**:
```bash
# Tag release
git tag -a v2.0.0-rc2 -m "Release Candidate 2: S3/MinIO Tiering"
git push origin v2.0.0-rc2

# Create GitHub release with notes
gh release create v2.0.0-rc2 \
  --title "AkiDB 2.0 RC2: S3/MinIO Tiering" \
  --notes-file RELEASE-NOTES.md \
  --prerelease
```

### 7.2 Release Smoke Test

**Test Scenario**:
1. Pull Docker image: `docker pull akidb/akidb:2.0.0-rc2`
2. Start server with S3 config
3. Create collection with 10k vectors
4. Wait for demotion to cold tier
5. Search (trigger restore from S3)
6. Verify results correct
7. Check metrics endpoint
8. Shutdown gracefully

**Automated Smoke Test Script**:
```bash
#!/bin/bash
# smoke-test-rc2.sh

set -e

echo "🚀 Starting AkiDB 2.0 RC2 Smoke Test"

# Start MinIO (local S3)
docker run -d --name minio \
  -p 9000:9000 \
  -e "MINIO_ROOT_USER=minioadmin" \
  -e "MINIO_ROOT_PASSWORD=minioadmin" \
  minio/minio server /data

# Start AkiDB
docker run -d --name akidb-rc2 \
  -p 8080:8080 \
  -e AKIDB_S3_ENDPOINT=http://minio:9000 \
  akidb/akidb:2.0.0-rc2

# Wait for startup
sleep 5

# Create collection
curl -X POST http://localhost:8080/collections \
  -H "Content-Type: application/json" \
  -d '{"name":"test","dimension":512,"metric":"cosine"}'

# Insert vectors
for i in {1..100}; do
  curl -X POST http://localhost:8080/collections/test/vectors \
    -H "Content-Type: application/json" \
    -d "{\"vector\":[$(seq -s, 1 512 | sed 's/[0-9]\+/0.1/g')]}"
done

# Search
curl -X POST http://localhost:8080/collections/test/search \
  -H "Content-Type: application/json" \
  -d "{\"vector\":[$(seq -s, 1 512 | sed 's/[0-9]\+/0.1/g')],\"k\":10}"

# Check metrics
curl http://localhost:8080/metrics | grep akidb_tier

# Cleanup
docker stop akidb-rc2 minio
docker rm akidb-rc2 minio

echo "✅ RC2 Smoke Test Passed!"
```

---

## 8. Implementation Plan

### Day 1: E2E Integration Tests (Part 1)

**Goal**: Implement 10 E2E tests (full workflow scenarios)
**Time**: 4-5 hours

**Morning Session** (2-3 hours):
1. Create test infrastructure (MockS3ObjectStore, test helpers)
2. Implement tests 1-5 (full workflow scenarios)

**Afternoon Session** (2 hours):
3. Implement tests 6-10 (S3 integration scenarios)

**Deliverable**: 10 E2E tests passing

---

### Day 2: E2E Integration Tests (Part 2)

**Goal**: Complete remaining 10 E2E tests + crash recovery tests
**Time**: 4-5 hours

**Morning Session** (2-3 hours):
1. Implement tests 11-20 (tiering + snapshot integration)

**Afternoon Session** (2 hours):
2. Implement 5 crash recovery tests
3. Add fault injection framework

**Deliverable**: 20 E2E tests + 5 crash recovery tests passing (25 total)

---

### Day 3: Performance Benchmarks

**Goal**: Implement 10 performance benchmarks with baselines
**Time**: 4-5 hours

**Morning Session** (2-3 hours):
1. Setup Criterion.rs benchmark framework
2. Implement snapshot benchmarks (3 tests)
3. Implement tiering benchmarks (4 tests)

**Afternoon Session** (2 hours):
4. Implement search benchmarks (3 tests)
5. Run all benchmarks and save baselines
6. Generate performance report

**Deliverable**: 10 benchmarks with documented results

---

### Day 4: Documentation

**Goal**: Complete all documentation guides
**Time**: 4-5 hours

**Morning Session** (2-3 hours):
1. Write S3 Configuration Guide (AWS, MinIO, Oracle)
2. Write Tiering Tuning Guide (workload profiles)

**Afternoon Session** (2 hours):
3. Update Migration Guide (v1.x → v2.0)
4. Update Deployment Guide (S3 setup, Docker)
5. Review and polish all docs

**Deliverable**: 4 comprehensive guides ready for users

---

### Day 5: Release Preparation

**Goal**: Prepare and publish RC2 release
**Time**: 4-5 hours

**Morning Session** (2-3 hours):
1. Update version numbers (Cargo.toml, changelog)
2. Write changelog with all Week 1-3 features
3. Build Docker images
4. Run smoke test script

**Afternoon Session** (2 hours):
5. Create GitHub release (tag, notes)
6. Publish Docker images
7. Write release announcement
8. Create Week 3 completion report

**Deliverable**: v2.0.0-rc2 released!

---

## 9. Success Criteria

### 9.1 Functional Requirements

- ✅ 20 E2E integration tests passing
- ✅ 5 crash recovery tests passing
- ✅ 1 smoke test passing
- ✅ All tests complete in <30 minutes
- ✅ Zero data loss in all scenarios

### 9.2 Performance Requirements

- ✅ Snapshot creation: <2s for 10k vectors (512-dim)
- ✅ Snapshot restore: <3s for 10k vectors
- ✅ Tier transitions: <5s for 10k vectors
- ✅ Search P95 latency: <25ms @ 100k vectors
- ✅ Compression: >2x vs JSON

### 9.3 Quality Requirements

- ✅ Code coverage >80% (combined Week 1-3)
- ✅ All benchmarks documented with results
- ✅ 4 comprehensive documentation guides
- ✅ RC2 release published (Docker + GitHub)
- ✅ Smoke test passes on RC2 image

### 9.4 Release Criteria

- ✅ Version bumped to 2.0.0-rc2
- ✅ Changelog complete
- ✅ Docker images built and published
- ✅ GitHub release created with notes
- ✅ No known critical bugs

---

## 10. Risk Analysis

### 10.1 High-Risk Areas

**Risk 1: Performance Benchmarks Fail to Meet Targets**
- **Likelihood**: Medium
- **Impact**: High (blocks RC2 release)
- **Mitigation**:
  - Start benchmarking early (Day 3)
  - If targets missed, tune compression settings
  - If still failing, revise targets based on data
  - Document actual vs target performance

**Risk 2: Crash Recovery Tests Uncover Data Loss Bug**
- **Likelihood**: Medium
- **Impact**: Critical (must fix before RC2)
- **Mitigation**:
  - Test early (Day 2)
  - If bug found, implement fix immediately
  - Add additional tests to prevent regression
  - May slip RC2 release by 1-2 days if needed

**Risk 3: Documentation Incomplete or Inaccurate**
- **Likelihood**: Low
- **Impact**: Medium (poor user experience)
- **Mitigation**:
  - Use templates for consistency
  - Include runnable examples
  - Test all code snippets manually
  - Peer review before release

**Risk 4: Docker Image Build Fails**
- **Likelihood**: Low
- **Impact**: Medium (delays release)
- **Mitigation**:
  - Test Docker build early (Day 4)
  - Use multi-stage build for efficiency
  - Include smoke test in CI/CD

### 10.2 Medium-Risk Areas

**Risk 5: E2E Tests Take Too Long (>30min)**
- **Mitigation**: Reduce dataset sizes, parallelize tests

**Risk 6: S3 Mock Doesn't Accurately Simulate Real S3**
- **Mitigation**: Run tests against real MinIO instance

**Risk 7: Changelog Missing Important Changes**
- **Mitigation**: Review all Week 1-2 PRs, use git log

---

## Appendix A: Test File Structure

```
crates/akidb-storage/tests/
├── integration_e2e_tests.rs         # 20 E2E tests
├── crash_recovery_tests.rs          # 5 crash recovery tests
└── smoke_test.rs                    # 1 smoke test

crates/akidb-storage/benches/
└── integration_bench.rs             # 10 performance benchmarks

scripts/
├── smoke-test-rc2.sh                # Automated smoke test
└── release-rc2.sh                   # Release automation script

docs/
├── S3-CONFIGURATION-GUIDE.md
├── TIERING-TUNING-GUIDE.md
├── MIGRATION-V1-TO-V2.md (update)
└── DEPLOYMENT-GUIDE.md (update)
```

---

## Appendix B: Benchmark Results Template

**File**: `docs/PERFORMANCE-BENCHMARKS-RC2.md`

```markdown
# AkiDB 2.0 RC2 Performance Benchmarks

**Date**: 2025-11-XX
**Hardware**: Apple M1 Max, 64GB RAM, 1TB SSD
**Software**: Rust 1.75, macOS 14.0

## Snapshot Performance

| Metric | Target | Actual | Status |
|--------|--------|--------|--------|
| Create (10k, 512-dim) | <2s | 1.85s ± 0.12s | ✅ PASS |
| Restore (10k, 512-dim) | <3s | 2.73s ± 0.18s | ✅ PASS |
| Compression Ratio | >2x | 2.8x | ✅ PASS |

## Tiering Performance

| Metric | Target | Actual | Status |
|--------|--------|--------|--------|
| Hot → Warm (10k) | <2s | 1.92s ± 0.08s | ✅ PASS |
| Warm → Hot (10k) | <3s | 2.41s ± 0.15s | ✅ PASS |
| Warm → Cold (10k) | <5s | 4.67s ± 0.22s | ✅ PASS |
| Cold → Warm (100k) | <10s | 9.23s ± 0.51s | ✅ PASS |

## Search Performance

| Metric | Target | Actual | Status |
|--------|--------|--------|--------|
| Hot Tier P95 (100k) | <5ms | 4.2ms | ✅ PASS |
| Warm Tier P95 (100k) | <25ms | 18.7ms | ✅ PASS |
| Cold Tier (first access) | <10s | 9.8s | ✅ PASS |

**Conclusion**: All performance targets met ✅
```

---

**Status**: ✅ MEGATHINK COMPLETE - READY FOR PRD CREATION

**Next**: Create detailed PRD document for Phase 10 Week 3
