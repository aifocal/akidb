# Phase 10 Week 3: Integration Testing + RC2 Release - Product Requirements Document

**Version**: 1.0
**Date**: 2025-11-09
**Status**: ✅ APPROVED
**Owner**: Phase 10 Team
**Timeline**: 5 days (Week 3)

---

## Executive Summary

Validate Week 1-2 deliverables through comprehensive integration testing and prepare **v2.0.0-rc2 release** with production-grade documentation and performance benchmarks.

**Business Value**:
- Production-ready release candidate with S3/MinIO tiering
- Validated performance targets (all benchmarks met)
- Enterprise-grade documentation (S3 setup, tuning, migration)
- Confidence in crash recovery and data integrity
- Clear upgrade path for v1.x users

**Technical Value**:
- 36 new tests (20 E2E + 10 benchmarks + 5 crash + 1 smoke)
- Full workflow validation (insert → tier → snapshot → restore)
- Crash recovery guarantees (zero data loss)
- Performance baselines documented
- Docker images published

---

## Goals and Non-Goals

### Goals
- ✅ 20+ E2E integration tests covering full workflows
- ✅ 10 performance benchmarks meeting all targets
- ✅ 5 crash recovery tests with zero data loss
- ✅ Comprehensive documentation (4 guides)
- ✅ RC2 release published (Docker + GitHub)
- ✅ 100% test pass rate
- ✅ All benchmarks documented with results

### Non-Goals
- ❌ Load testing (1000+ QPS) - deferred to Week 4
- ❌ Distributed deployment - deferred to Week 6
- ❌ Production monitoring (Prometheus) - deferred to Week 5
- ❌ GA release - RC2 is pre-release for feedback

---

## User Stories

### Story 1: DevOps Engineer
**As a** DevOps engineer
**I want** comprehensive documentation for S3 setup
**So that** I can deploy AkiDB 2.0 with tiering in production

**Acceptance Criteria**:
- S3 configuration guide covers AWS, MinIO, Oracle Cloud
- Step-by-step setup with runnable examples
- Security best practices included
- Troubleshooting section for common errors

### Story 2: Database Administrator
**As a** database administrator
**I want** performance benchmarks and tuning guidance
**So that** I can optimize tiering policies for my workload

**Acceptance Criteria**:
- Tiering tuning guide with workload profiles
- Benchmark results documented (snapshot, tiering, search)
- Clear recommendations for different use cases
- Examples of cost vs performance tradeoffs

### Story 3: Application Developer
**As an** application developer
**I want** to upgrade from v1.x to v2.0 without data loss
**So that** I can benefit from new tiering features

**Acceptance Criteria**:
- Migration guide with step-by-step instructions
- Backward compatibility verified
- Rollback procedure documented
- FAQ addresses common concerns

### Story 4: QA Engineer
**As a** QA engineer
**I want** comprehensive test coverage for crash scenarios
**So that** I can trust the system won't lose data

**Acceptance Criteria**:
- Crash recovery tests cover node crashes and network failures
- Zero data loss verified in all scenarios
- Recovery procedures documented
- All tests automated and repeatable

---

## Technical Specification

### Test Strategy

**Test Pyramid** (Week 3 additions):
```
              ┌──────────┐
              │ 1 Smoke  │  RC2 release validation
              └──────────┘
         ┌─────────────────┐
         │ 5 Crash         │  Crash recovery scenarios
         │ Recovery        │
         └─────────────────┘
    ┌────────────────────────┐
    │ 20 E2E                 │  Full workflow integration
    │ Integration            │
    └────────────────────────┘
┌──────────────────────────────┐
│ 10 Performance               │  Benchmarks
│ Benchmarks                   │
└──────────────────────────────┘
```

**Total Week 3 Tests**: 36 new tests
**Combined Total**: 83 tests (47 from Week 1-2 + 36 new)

### E2E Integration Tests (20 tests)

**Category 1: Full Workflow (8 tests)**
1. `test_insert_tier_snapshot_restore` - End-to-end workflow
2. `test_hot_to_cold_full_cycle` - Complete tier lifecycle
3. `test_concurrent_tier_transitions` - Parallel operations
4. `test_large_dataset_tiering` - 100k vectors
5. `test_pinned_collection_never_demoted` - Pin functionality
6. `test_manual_tier_control_workflow` - Force promote/demote
7. `test_multiple_snapshots_per_collection` - Multi-snapshot
8. `test_tier_state_persistence_across_restart` - Server restart

**Category 2: S3 Integration (6 tests)**
9. `test_s3_upload_large_snapshot` - 100k vectors to S3
10. `test_s3_download_and_restore` - Download and verify
11. `test_s3_list_snapshots` - List operations
12. `test_s3_delete_snapshot` - Delete operations
13. `test_s3_retry_on_transient_error` - Retry logic
14. `test_s3_fail_on_permanent_error` - Error handling

**Category 3: Tiering + Snapshot (6 tests)**
15. `test_warm_to_cold_creates_snapshot` - Auto-snapshot
16. `test_cold_to_warm_restores_snapshot` - Auto-restore
17. `test_access_tracking_resets_after_promotion` - Counter reset
18. `test_background_worker_demotes_multiple_collections` - Bulk demotion
19. `test_background_worker_promotes_high_access_collections` - Bulk promotion
20. `test_tiering_disabled_collections_always_hot` - Disable option

### Performance Benchmarks (10 tests)

**Snapshot Performance** (3 benchmarks):
- Create: <2s for 10k vectors (512-dim)
- Restore: <3s for 10k vectors
- Compression: >2x vs JSON

**Tiering Performance** (4 benchmarks):
- Hot → Warm: <2s for 10k vectors
- Warm → Hot: <3s for 10k vectors
- Warm → Cold: <5s for 10k vectors
- Cold → Warm: <10s for 100k vectors

**Search Performance** (3 benchmarks):
- Hot tier P95: <5ms @ 100k vectors
- Warm tier P95: <25ms @ 100k vectors
- Cold tier first access: <10s

### Crash Recovery Tests (5 tests)

**Node Crashes** (3 tests):
1. `test_crash_during_snapshot_upload` - Mid-upload crash
2. `test_crash_during_tier_demotion` - Mid-demotion crash
3. `test_crash_during_background_worker_cycle` - Worker crash

**Network Failures** (2 tests):
4. `test_s3_connection_lost_during_upload` - Upload interruption
5. `test_s3_connection_lost_during_download` - Download interruption

### Smoke Test (1 test)

`test_rc2_release_smoke` - Full deployment test:
- Pull Docker image
- Start server with S3 config
- Create collection + insert vectors
- Trigger tier demotion
- Search (restore from S3)
- Verify results
- Check metrics

---

## Implementation Phases

### Day 1: E2E Integration Tests (Part 1) (4-5 hours)
- Setup test infrastructure (MockS3, helpers)
- Implement full workflow tests (8 tests)
- Implement S3 integration tests (6 tests)
- **Deliverable**: 14 E2E tests passing

### Day 2: E2E Integration Tests (Part 2) (4-5 hours)
- Implement tiering + snapshot tests (6 tests)
- Implement crash recovery tests (5 tests)
- Add fault injection framework
- **Deliverable**: 20 E2E + 5 crash tests passing (25 total)

### Day 3: Performance Benchmarks (4-5 hours)
- Setup Criterion.rs framework
- Implement snapshot benchmarks (3 tests)
- Implement tiering benchmarks (4 tests)
- Implement search benchmarks (3 tests)
- Run and document results
- **Deliverable**: 10 benchmarks with baselines

### Day 4: Documentation (4-5 hours)
- Write S3 Configuration Guide (AWS, MinIO, Oracle)
- Write Tiering Tuning Guide (workload profiles)
- Update Migration Guide (v1.x → v2.0)
- Update Deployment Guide (Docker, S3 setup)
- **Deliverable**: 4 comprehensive guides

### Day 5: RC2 Release (4-5 hours)
- Update version numbers (2.0.0-rc2)
- Write changelog
- Build Docker images
- Run smoke test
- Create GitHub release
- **Deliverable**: v2.0.0-rc2 released!

---

## Testing Strategy

### Test Infrastructure

**MockS3ObjectStore**:
```rust
pub struct MockS3ObjectStore {
    storage: Arc<RwLock<HashMap<String, Bytes>>>,
    error_mode: Arc<RwLock<Option<S3Error>>>,
}

impl MockS3ObjectStore {
    pub fn inject_error(&self, error: S3Error);
    pub fn clear_errors(&self);
}
```

**FaultInjector**:
```rust
pub struct FaultInjector {
    crash_points: Arc<RwLock<HashMap<String, bool>>>,
}

impl FaultInjector {
    pub fn enable_crash_at(&self, point: &str);
    pub fn check_crash_point(&self, point: &str) -> bool;
}
```

**Test Helpers**:
```rust
async fn create_test_collection_with_vectors(
    count: usize,
    dimension: usize,
) -> (CollectionId, Vec<VectorDocument>);

async fn simulate_time_passage(duration: Duration);

async fn assert_tier_state(
    collection_id: CollectionId,
    expected_tier: Tier,
);

async fn assert_vectors_match(
    actual: &[VectorDocument],
    expected: &[VectorDocument],
    epsilon: f32,
);
```

### Performance Benchmark Framework

**Criterion.rs Setup**:
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn snapshot_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("snapshot");

    group.bench_function("create_10k", |b| {
        b.to_async(&rt).iter(|| async {
            snapshotter.create_snapshot(collection_id, vectors.clone()).await
        });
    });

    group.finish();
}

criterion_group!(benches, snapshot_benchmarks, tiering_benchmarks);
criterion_main!(benches);
```

---

## Documentation Requirements

### 1. S3 Configuration Guide

**File**: `docs/S3-CONFIGURATION-GUIDE.md`

**Sections**:
- Overview: Why S3/MinIO for cold tier
- AWS S3 Setup (bucket, IAM, credentials)
- MinIO Setup (local testing)
- Oracle Cloud Object Storage Setup
- Security Best Practices (encryption, IAM roles)
- Troubleshooting (common errors, solutions)

**Key Content**:
```markdown
## AWS S3 Setup

### Create Bucket
bash
aws s3api create-bucket --bucket akidb-cold-tier --region us-west-2


### Configure AkiDB
toml
[storage.s3]
endpoint = "https://s3.us-west-2.amazonaws.com"
bucket = "akidb-cold-tier"
region = "us-west-2"
access_key_id = "YOUR_KEY"
secret_access_key = "YOUR_SECRET"

```

### 2. Tiering Tuning Guide

**File**: `docs/TIERING-TUNING-GUIDE.md`

**Sections**:
- Overview: How policies affect cost/performance
- Default Policies Explained
- Tuning for Workloads (RAG, Analytics, E-commerce)
- Monitoring Tier Distribution
- Case Studies

**Key Content**:
```markdown
## Workload Profiles

### High Read Frequency (RAG Chatbot)
toml
[tiering.policy]
hot_tier_ttl_hours = 24
warm_tier_ttl_days = 30
hot_promotion_threshold = 5


Expected: 80-90% hot, 10-15% warm, 0-5% cold

### Low Read Frequency (Batch Analytics)
toml
[tiering.policy]
hot_tier_ttl_hours = 1
warm_tier_ttl_days = 1
hot_promotion_threshold = 20


Expected: 5-10% hot, 10-20% warm, 70-85% cold
```

### 3. Migration Guide (v1.x → v2.0)

**File**: `docs/MIGRATION-V1-TO-V2.md` (update)

**Sections**:
- Breaking Changes (tiering optional, S3 config)
- Migration Steps (backup, install, migrate, verify)
- Rollback Procedure
- FAQ (downtime, duration, tiering requirement)

### 4. Deployment Guide

**File**: `docs/DEPLOYMENT-GUIDE.md` (update)

**New Sections**:
- S3/MinIO setup for production
- Tiering configuration examples
- Docker Compose with MinIO
- Kubernetes with S3 ConfigMap
- Monitoring tier distribution

---

## Performance Requirements

| Category | Metric | Target | Measurement |
|----------|--------|--------|-------------|
| **Snapshot** | Create (10k, 512-dim) | <2s | Benchmark |
| | Restore (10k, 512-dim) | <3s | Benchmark |
| | Compression ratio | >2x | File size comparison |
| **Tiering** | Hot → Warm (10k) | <2s | Benchmark |
| | Warm → Hot (10k) | <3s | Benchmark |
| | Warm → Cold (10k) | <5s | Benchmark |
| | Cold → Warm (100k) | <10s | Benchmark |
| **Search** | Hot tier P95 (100k) | <5ms | Benchmark |
| | Warm tier P95 (100k) | <25ms | Benchmark |
| | Cold tier first access | <10s | Benchmark |
| **Tests** | Execution time | <30min | CI/CD |

---

## Error Handling

### Test Failure Handling

**E2E Test Failure**:
- Log full error trace
- Preserve test artifacts (snapshots, logs)
- Retry flaky tests (max 3 retries)
- Block RC2 release if any test fails

**Benchmark Failure**:
- Document actual vs target performance
- If <10% miss, document and proceed
- If >10% miss, investigate and optimize
- Update targets if infeasible

**Crash Recovery Failure**:
- Critical bug - must fix before RC2
- Add regression test
- Re-run all crash tests
- Document fix in changelog

---

## Dependencies

### External Crates
- `criterion = "0.5"` (NEW - benchmarking)
- All Week 1-2 dependencies ✅

### Internal Modules
- Week 1: ParquetSnapshotter
- Week 2: TieringManager
- Existing: StorageBackend, CollectionService

### Infrastructure
- **Required**: MinIO for local S3 testing
- **Optional**: AWS S3 account for cloud testing
- **Optional**: Oracle Cloud account

---

## Release Preparation

### Version Bump

**Files to Update**:
- `Cargo.toml` (all crates): `version = "2.0.0-rc2"`
- `CHANGELOG.md`: Add RC2 section
- `README.md`: Update version badges

### Changelog

```markdown
## [2.0.0-rc2] - 2025-11-XX

### Added
- Parquet-based vector snapshots (2-3x compression)
- Hot/warm/cold tiering policies
- S3/MinIO cold tier storage
- Background worker for automatic tier transitions
- Manual tier control API
- 36 new tests (20 E2E + 10 benchmarks + 5 crash + 1 smoke)
- S3 Configuration Guide
- Tiering Tuning Guide
- Updated Migration Guide

### Performance
- Snapshot create: <2s for 10k vectors ✅
- Snapshot restore: <3s for 10k vectors ✅
- Search P95: <25ms @ 100k vectors ✅
- Compression: 2-3x vs JSON ✅

### Documentation
- Comprehensive S3 setup guide
- Tiering tuning for different workloads
- Docker deployment with MinIO
- Crash recovery procedures
```

### Docker Image

**Build**:
```bash
docker build -t akidb/akidb:2.0.0-rc2 .
docker tag akidb/akidb:2.0.0-rc2 akidb/akidb:latest
```

**Publish**:
```bash
docker push akidb/akidb:2.0.0-rc2
docker push akidb/akidb:latest
```

### GitHub Release

**Tag**:
```bash
git tag -a v2.0.0-rc2 -m "Release Candidate 2: S3/MinIO Tiering"
git push origin v2.0.0-rc2
```

**Release Notes**:
```markdown
# AkiDB 2.0 RC2: S3/MinIO Tiering

## Highlights

- Automatic hot/warm/cold tiering with 60-80% cost savings
- Parquet snapshots with 2-3x compression
- Production-ready S3/MinIO integration
- 83 tests passing (47 existing + 36 new)
- Comprehensive documentation

## Performance

All targets met:
- Snapshot: <2s create, <3s restore (10k vectors)
- Tiering: <10s cold → warm (100k vectors)
- Search: P95 <25ms @ 100k vectors

## Documentation

- S3 Configuration Guide
- Tiering Tuning Guide
- Migration Guide (v1.x → v2.0)

## Install

bash
docker pull akidb/akidb:2.0.0-rc2


See [Migration Guide](docs/MIGRATION-V1-TO-V2.md) for upgrade instructions.
```

---

## Smoke Test

**Script**: `scripts/smoke-test-rc2.sh`

```bash
#!/bin/bash
set -e

echo "🚀 Starting RC2 Smoke Test"

# Start MinIO
docker run -d --name minio -p 9000:9000 \
  -e "MINIO_ROOT_USER=minioadmin" \
  -e "MINIO_ROOT_PASSWORD=minioadmin" \
  minio/minio server /data

# Start AkiDB
docker run -d --name akidb-rc2 -p 8080:8080 \
  -e AKIDB_S3_ENDPOINT=http://minio:9000 \
  akidb/akidb:2.0.0-rc2

sleep 5

# Create collection
curl -X POST http://localhost:8080/collections \
  -d '{"name":"test","dimension":512,"metric":"cosine"}'

# Insert vectors
for i in {1..100}; do
  curl -X POST http://localhost:8080/collections/test/vectors \
    -d "{\"vector\":[...]}"
done

# Search
curl -X POST http://localhost:8080/collections/test/search \
  -d "{\"vector\":[...],\"k\":10}"

# Cleanup
docker stop akidb-rc2 minio
docker rm akidb-rc2 minio

echo "✅ RC2 Smoke Test Passed!"
```

---

## Success Criteria

### Functional
- ✅ 20 E2E integration tests passing
- ✅ 10 performance benchmarks passing
- ✅ 5 crash recovery tests passing
- ✅ 1 smoke test passing
- ✅ Zero data loss in all scenarios

### Performance
- ✅ All 10 benchmarks meet targets
- ✅ Test execution time <30 minutes
- ✅ Compression >2x vs JSON

### Quality
- ✅ Code coverage >80% (combined)
- ✅ 4 comprehensive documentation guides
- ✅ RC2 release published (Docker + GitHub)
- ✅ Smoke test passes on RC2 image
- ✅ No known critical bugs

### Release
- ✅ Version bumped to 2.0.0-rc2
- ✅ Changelog complete
- ✅ Docker images published
- ✅ GitHub release created
- ✅ Release notes written

---

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Benchmarks fail to meet targets | Medium | High | Start early, tune if needed, document actual |
| Crash recovery uncovers data loss | Medium | Critical | Test early, fix immediately, add regression tests |
| Documentation incomplete | Low | Medium | Use templates, test examples, peer review |
| Docker build fails | Low | Medium | Test early, multi-stage build, smoke test |
| E2E tests too slow (>30min) | Low | Low | Reduce dataset sizes, parallelize |

---

## Timeline

**Total Duration**: 5 days (20-25 hours)

**Daily Milestones**:
- **Day 1** (EOD): 14 E2E tests passing
- **Day 2** (EOD): 25 tests passing (20 E2E + 5 crash)
- **Day 3** (EOD): 10 benchmarks documented
- **Day 4** (EOD): 4 documentation guides complete
- **Day 5** (EOD): v2.0.0-rc2 released! 🎉

---

## Approval

**Product**: ✅ Approved
**Engineering**: ✅ Approved
**Architecture**: ✅ Approved
**QA**: ✅ Approved

**Go-Live Date**: Day 5 (Week 3 completion)

---

## References

- **Main PRD**: `automatosx/PRD/PHASE-10-PRODUCTION-READY-V2-PRD.md`
- **Week 1 PRD**: `automatosx/PRD/PHASE-10-WEEK-1-PARQUET-SNAPSHOTTER-PRD.md`
- **Week 2 PRD**: `automatosx/PRD/PHASE-10-WEEK-2-TIERING-POLICIES-PRD.md`
- **Megathink**: `automatosx/tmp/PHASE-10-WEEK-3-COMPREHENSIVE-MEGATHINK.md`
- **Action Plan**: `automatosx/tmp/PHASE-10-ACTION-PLAN.md`

---

**Status**: ✅ APPROVED - READY FOR IMPLEMENTATION
