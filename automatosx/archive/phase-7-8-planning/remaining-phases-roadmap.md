# AkiDB 2.0: Remaining Phases Roadmap

**Date:** 2025-11-07
**Current Status:** Phase 5 Complete (95%), RC1 Ready
**Document Purpose:** Complete overview of remaining work to reach GA and beyond

---

## Executive Summary

**Completed:** Phases 1-5 (Core foundation through RC1)
**Remaining:** Phases 6-8 + Post-GA enhancements
**Estimated Timeline:** 8-12 weeks to GA, 6+ months for v2.x series

---

## Current Position

### ✅ Completed Phases (Phases 1-5)

| Phase | Name | Status | Duration | Completion |
|-------|------|--------|----------|------------|
| **Phase 1** | Foundation/Metadata Layer | ✅ Complete | 4 weeks | 100% |
| **Phase 2** | Embedding Service Infrastructure | ✅ Complete | 3 weeks | 100% |
| **Phase 3** | User Management & RBAC | ✅ Complete | 3 weeks | 100% |
| **Phase 4** | Vector Engine (BruteForce + HNSW) | ✅ Complete | 4 weeks | 100% |
| **Phase 5** | RC1 Server Layer & Persistence | ✅ Complete | 4 weeks | 95% |

**Total Completed:** 18 weeks, 147 tests passing, RC1 production-ready

---

## Remaining Phases Overview

### Summary Table

| Phase | Name | Status | Estimated Duration | Target Release |
|-------|------|--------|-------------------|----------------|
| **Phase 6** | S3/MinIO Tiered Storage | ⏸️ Not Started | 5 weeks | RC2 (v2.0.0-rc2) |
| **Phase 7** | Production Hardening | ⏸️ Not Started | 2 weeks | RC3/GA (v2.0.0) |
| **Phase 8** | Optional: Cedar Policy Engine | ⏸️ Deferred | 2 weeks | v2.1.0+ |
| **Post-GA** | Enhancements & Scale | ⏸️ Future | Ongoing | v2.1-2.2+ |

**Remaining to GA:** 7 weeks minimum (5 weeks Phase 6 + 2 weeks Phase 7)

---

## Phase 6: S3/MinIO Tiered Storage (5 Weeks) ⏸️

### Overview

**Goal:** Enable vector storage persistence with S3/MinIO for datasets >100GB, supporting hot/warm/cold tiering.

**Why Critical:**
- RC1 is RAM-only (vectors lost on restart, now has SQLite persistence but not optimized for large scale)
- Need S3 for cost-effective large-scale storage
- Enables 10x dataset size increase (1B+ vectors)

### Week-by-Week Breakdown

#### Week 1: Write-Ahead Log (WAL) Implementation
**Duration:** 5 days (40 hours)

**Deliverables:**
- WAL trait and file-based implementation
- Crash recovery with replay
- Log rotation and compaction
- Unit + integration tests (15+ tests)

**Key Features:**
```rust
pub trait WriteAheadLog {
    async fn append(&self, entry: LogEntry) -> Result<LogSequenceNumber>;
    async fn replay(&self, from_lsn: LogSequenceNumber) -> Result<Vec<LogEntry>>;
    async fn checkpoint(&self) -> Result<()>;
    async fn rotate(&self) -> Result<()>;
}

// FileWAL implementation with:
// - Fsync durability
// - Crash recovery
// - Log compaction
```

**Estimated Lines:** 1,200 (600 implementation + 600 tests)

---

#### Week 2: S3/ObjectStore Integration
**Duration:** 5 days (40 hours)

**Deliverables:**
- ObjectStore trait (S3, MinIO, Local filesystem)
- S3 client integration (AWS SDK)
- MinIO compatibility testing
- Object lifecycle management
- Integration tests (20+ tests)

**Key Features:**
```rust
pub trait ObjectStore {
    async fn put(&self, key: &str, data: Vec<u8>) -> Result<()>;
    async fn get(&self, key: &str) -> Result<Vec<u8>>;
    async fn delete(&self, key: &str) -> Result<()>;
    async fn list(&self, prefix: &str) -> Result<Vec<String>>;
}

// Implementations:
// - S3ObjectStore (AWS S3)
// - MinioObjectStore (MinIO)
// - LocalObjectStore (filesystem fallback)
```

**Estimated Lines:** 1,500 (800 implementation + 700 tests)

---

#### Week 3: Parquet Snapshotter
**Duration:** 5 days (40 hours)

**Deliverables:**
- Parquet writer for vector snapshots
- Columnar format optimization
- Compression (Snappy, Zstd)
- Snapshot restoration
- Benchmark suite (20+ tests)

**Key Features:**
```rust
pub trait Snapshotter {
    async fn create_snapshot(&self, collection_id: CollectionId) -> Result<SnapshotId>;
    async fn restore_snapshot(&self, snapshot_id: SnapshotId) -> Result<Collection>;
    async fn list_snapshots(&self) -> Result<Vec<SnapshotMetadata>>;
    async fn delete_snapshot(&self, snapshot_id: SnapshotId) -> Result<()>;
}

// ParquetSnapshotter features:
// - 50-70% space savings via compression
// - Fast columnar loading
// - Incremental snapshots
```

**Estimated Lines:** 1,000 (500 implementation + 500 tests)

---

#### Week 4: Tiering Integration & Policies
**Duration:** 5 days (40 hours)

**Deliverables:**
- Tiering policy engine
- Hot/warm/cold tier management
- Automatic eviction policies
- Transparent data movement
- End-to-end tests (25+ tests)

**Key Features:**
```rust
pub enum TieringPolicy {
    Memory,          // All in RAM (current RC1 behavior)
    MemoryS3,        // Hot in RAM, cold in S3
    S3Only,          // All in S3 (smallest memory footprint)
}

pub struct StorageBackend {
    wal: Arc<dyn WriteAheadLog>,
    object_store: Arc<dyn ObjectStore>,
    snapshotter: Arc<dyn Snapshotter>,
    policy: TieringPolicy,
}

// Automatic tiering based on:
// - Access frequency (LRU)
// - Age (FIFO)
// - Collection size
// - Manual policies
```

**Estimated Lines:** 1,500 (900 implementation + 600 tests)

---

#### Week 5: Polish & Production Readiness
**Duration:** 5 days (40 hours)

**Deliverables:**
- E2E integration tests (full workflows)
- Performance benchmarks (S3 latency, throughput)
- Documentation (S3-PERSISTENCE-GUIDE.md)
- Example code (s3_storage_demo.rs)
- RC2 release preparation

**Key Activities:**
- Chaos testing (S3 failures, network partitions)
- Cross-platform validation (Mac ARM, Linux x86_64, ARM)
- Memory profiling
- Security audit (credentials, IAM)
- Migration guide (RC1 → RC2)

**Estimated Lines:** 2,000 (1,000 tests + 1,000 docs/examples)

---

### Phase 6 Summary

**Total Duration:** 5 weeks (200 hours)
**Total Code:** ~7,200 lines (3,800 implementation + 3,400 tests/docs)
**Total Tests:** 95+ (60 unit + 35 integration)
**Target Release:** RC2 (v2.0.0-rc2)

**Success Criteria:**
- ✅ WAL ensures durability (zero data loss on crash)
- ✅ S3/MinIO storage working (10x dataset capacity)
- ✅ Parquet snapshots 50-70% smaller
- ✅ Tiering policies reduce memory 80%+
- ✅ All 95+ tests passing
- ✅ Performance: +10-20ms latency for cold data (acceptable)

---

## Phase 7: Production Hardening (2 Weeks) ⏸️

### Overview

**Goal:** Production-ready GA release with monitoring, security, and operational excellence.

**Why Critical:** RC2 will have all features but needs hardening for production deployment.

### Week-by-Week Breakdown

#### Week 1: Security & Authentication
**Duration:** 5 days (40 hours)

**Deliverables:**
- API key authentication
- JWT token support
- TLS/mTLS configuration
- Rate limiting
- Security audit & penetration testing

**Key Features:**
```rust
// API Authentication
pub enum AuthMethod {
    ApiKey(String),
    JWT(JwtToken),
    mTLS(Certificate),
}

// Rate limiting
pub struct RateLimiter {
    qps_quota: u32,
    burst_size: u32,
}

// TLS configuration
pub struct TlsConfig {
    cert_path: PathBuf,
    key_path: PathBuf,
    client_ca_path: Option<PathBuf>,  // For mTLS
}
```

**Estimated Lines:** 1,200 (800 implementation + 400 tests)

---

#### Week 2: Monitoring & Operations
**Duration:** 5 days (40 hours)

**Deliverables:**
- Prometheus metrics expansion
- Grafana dashboard templates
- Distributed tracing (OpenTelemetry)
- Health checks (liveness, readiness)
- Deployment automation (Helm charts, K8s manifests)
- SLA documentation

**Key Features:**
```rust
// Comprehensive metrics
pub struct Metrics {
    // Existing RC1 metrics plus:
    s3_operation_latency: Histogram,
    s3_error_rate: Counter,
    wal_write_latency: Histogram,
    snapshot_size_bytes: Gauge,
    tiering_eviction_count: Counter,
    cold_data_access_latency: Histogram,
}

// Distributed tracing
#[instrument(skip(self))]
async fn search_vectors(&self, query: Vec<f32>) -> Result<Vec<SearchResult>> {
    // Automatic trace spans
}
```

**Estimated Lines:** 1,500 (800 implementation + 400 tests + 300 docs)

---

### Phase 7 Summary

**Total Duration:** 2 weeks (80 hours)
**Total Code:** ~2,700 lines
**Total Tests:** 30+ (security + monitoring)
**Target Release:** GA (v2.0.0)

**Success Criteria:**
- ✅ Zero critical/high security vulnerabilities
- ✅ API authentication working (API keys + JWT)
- ✅ TLS enabled by default
- ✅ Rate limiting prevents abuse
- ✅ Prometheus + Grafana monitoring
- ✅ Kubernetes deployment tested
- ✅ SLA documentation complete
- ✅ Penetration test passed

---

## Phase 8: Cedar Policy Engine (2 Weeks, Optional) ⏸️

### Overview

**Goal:** Migrate from enum-based RBAC to AWS Cedar policy engine for advanced ABAC (Attribute-Based Access Control).

**Status:** OPTIONAL - Current RBAC sufficient for 80% use cases

**Why Defer:**
- Phase 3 enum-based RBAC works well
- Cedar adds complexity
- Can upgrade later without breaking changes

### Implementation (If Needed)

**When to implement:**
- Customer requires fine-grained ABAC
- Need dynamic policy updates without code changes
- Compliance requires policy-as-code audit trail

**Estimated Duration:** 2 weeks
**Estimated Lines:** 1,500 lines
**Target Release:** v2.1.0 or later

---

## Post-GA Roadmap (v2.1 - v2.2+)

### v2.1.0: Advanced Features (3 months)
**Target:** 2026-02-15

**Features:**
- ✅ S3/MinIO tiered storage (from Phase 6)
- Multi-model embedding support (beyond current mock)
- Advanced query filters (metadata + vector hybrid)
- Batch operations API (bulk insert/delete)
- Client SDKs (Python, JavaScript, Go)

**Estimated Duration:** 12 weeks

---

### v2.2.0: High Availability & Scale (6 months)
**Target:** 2026-05-15

**Features:**
- Multi-region deployment
- Read replicas
- High availability setup
- Disaster recovery
- Distributed consensus (Raft)
- Advanced caching strategies
- GPU acceleration (Apple Silicon MLX)

**Estimated Duration:** 24 weeks

---

### v2.3.0+: Future Enhancements (Ongoing)

**Potential Features:**
- Hybrid search (dense + sparse vectors)
- Multi-vector search (multiple embeddings per document)
- Advanced recommender systems
- Real-time streaming ingestion
- Time-series vector support
- Federated search across clusters
- Edge deployment optimizations (even smaller footprint)

---

## Complete Timeline Summary

### To GA (v2.0.0)

| Milestone | Status | Duration | Target Date |
|-----------|--------|----------|-------------|
| Phase 5 Complete | ✅ Done | - | 2025-11-07 |
| **Phase 6 (S3/MinIO)** | ⏸️ Next | 5 weeks | 2025-12-12 |
| **Phase 7 (Hardening)** | ⏸️ Pending | 2 weeks | 2025-12-26 |
| **GA Release (v2.0.0)** | 🎯 Target | - | **2025-12-26** |

**Critical Path:** 7 weeks from today to GA

---

### Post-GA Releases

| Version | Features | Duration | Target Date |
|---------|----------|----------|-------------|
| v2.0.0 (GA) | Core vector DB + S3 storage | - | 2025-12-26 |
| v2.0.1-2.0.x | Bug fixes, patches | Ongoing | Q1 2026 |
| v2.1.0 | Advanced features, SDKs | 12 weeks | 2026-02-15 |
| v2.2.0 | HA, multi-region, GPU | 24 weeks | 2026-05-15 |
| v2.3.0+ | Future enhancements | TBD | 2026+ |

---

## Effort Estimation

### Remaining to GA

**Phase 6 (S3/MinIO):**
- Engineering: 200 hours (5 weeks × 40 hours)
- Code: ~7,200 lines
- Tests: 95+

**Phase 7 (Hardening):**
- Engineering: 80 hours (2 weeks × 40 hours)
- Code: ~2,700 lines
- Tests: 30+

**Total to GA:**
- **Engineering:** 280 hours (7 weeks)
- **Code:** ~10,000 lines
- **Tests:** 125+
- **Docs:** ~2,000 lines

---

### Post-GA (Optional)

**Phase 8 (Cedar, Optional):**
- Engineering: 80 hours (2 weeks)
- Code: ~1,500 lines
- Target: v2.1.0+

**v2.1.0 Features:**
- Engineering: 480 hours (12 weeks)
- Code: ~15,000 lines
- Target: Q1 2026

**v2.2.0 Features:**
- Engineering: 960 hours (24 weeks)
- Code: ~25,000 lines
- Target: Q2 2026

---

## Risk Assessment

### Phase 6 Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| S3 latency higher than expected | Medium | Low | Use caching, tiering policies |
| WAL overhead impacts performance | Medium | Medium | Async writes, batching |
| Parquet compatibility issues | Low | Low | Use standard Arrow format |
| S3 costs exceed budget | Medium | Medium | Document cost optimization |

**Overall Phase 6 Risk:** 🟡 Medium (manageable)

---

### Phase 7 Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Security vulnerabilities found | High | Medium | Penetration testing, audit |
| TLS configuration complexity | Low | Medium | Clear documentation |
| Monitoring overhead | Low | Low | Sampling, efficient metrics |
| K8s deployment issues | Medium | Medium | Extensive testing |

**Overall Phase 7 Risk:** 🟡 Medium (manageable)

---

## Decision Points

### Should We Implement Phase 8 (Cedar)?

**Defer to Post-GA (Recommended):**
- ✅ Current RBAC sufficient for 80% use cases
- ✅ Can add Cedar later without breaking changes
- ✅ Focus on core functionality first
- ✅ Lower complexity for GA

**Implement Now (If):**
- Customer requires fine-grained ABAC immediately
- Compliance mandates policy-as-code
- Budget/time available after Phase 7

**Recommendation:** ⏸️ Defer to v2.1.0

---

### Alternative: Fast-Track to GA

**Option 1: Standard Path (Recommended)**
- Phase 6 (5 weeks) → Phase 7 (2 weeks) → GA
- Full S3/MinIO support
- Complete hardening
- Target: 2025-12-26

**Option 2: Minimal S3 + Quick GA**
- Phase 6 Week 1-2 only (basic S3) → Phase 7 → GA
- Limited S3 support (no tiering, no Parquet)
- Faster to GA (3 weeks total)
- Target: 2025-11-28
- Risk: ⚠️ Feature incomplete

**Option 3: Skip S3, GA from RC1**
- RC1 → Phase 7 (hardening only) → GA
- No S3 support (RAM-only)
- Fastest to GA (2 weeks)
- Target: 2025-11-21
- Risk: ⚠️ Limited scale, not competitive

**Recommendation:** ✅ Option 1 (Standard Path)

---

## Conclusion

### What's Left?

**To RC2 (v2.0.0-rc2):**
- ⏸️ Phase 6: S3/MinIO Tiered Storage (5 weeks)
- Estimated: 200 hours, ~7,200 lines, 95+ tests

**To GA (v2.0.0):**
- ⏸️ Phase 6: S3/MinIO (5 weeks)
- ⏸️ Phase 7: Production Hardening (2 weeks)
- Estimated: 280 hours, ~10,000 lines, 125+ tests

**Post-GA (Optional):**
- ⏸️ Phase 8: Cedar Policy Engine (2 weeks, optional)
- ⏸️ v2.1.0: Advanced Features (12 weeks)
- ⏸️ v2.2.0: HA & Scale (24 weeks)

### Critical Path

**From Today (2025-11-07) to GA:**
1. Start Phase 6 immediately: 2025-11-08
2. Complete Phase 6: 2025-12-12 (5 weeks)
3. Complete Phase 7: 2025-12-26 (2 weeks)
4. **GA Release: 2025-12-26** 🎯

**Total Remaining:** 7 weeks (49 days)

### Recommendation

**✅ Proceed with standard path:**
1. Release RC1 now (v2.0.0-rc1) - Already complete
2. Implement Phase 6 (S3/MinIO) - 5 weeks
3. Implement Phase 7 (Hardening) - 2 weeks
4. Release GA (v2.0.0) - 2025-12-26
5. Defer Phase 8 (Cedar) to v2.1.0

**This balances feature completeness, time to market, and production readiness.**

---

**Document Version:** 1.0
**Date:** 2025-11-07
**Status:** Planning
**Next Review:** After Phase 6 kickoff
