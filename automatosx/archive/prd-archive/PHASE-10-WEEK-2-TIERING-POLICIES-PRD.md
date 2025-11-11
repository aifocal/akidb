# Phase 10 Week 2: Hot/Warm/Cold Tiering Policies - Product Requirements Document

**Version**: 1.0
**Date**: 2025-11-09
**Status**: ✅ APPROVED
**Owner**: Phase 10 Team
**Timeline**: 5 days (Week 2)

---

## Executive Summary

Implement automatic **hot/warm/cold tiering policies** for vector collections to reduce memory costs by 60-80% while maintaining low latency for frequently accessed data.

**Business Value**:
- Cost reduction: $10/GB (RAM) → $0.02/GB (S3) for cold data
- Support datasets >100GB with <100GB RAM
- Automatic resource optimization based on access patterns
- Transparent user experience (automatic promotion on access)

**Technical Value**:
- LRU-based access tracking at collection level
- Configurable promotion/demotion thresholds
- Background worker for automatic tier transitions
- Integration with Week 1 Parquet snapshots
- Zero data loss guarantees

---

## Goals and Non-Goals

### Goals
- ✅ Implement TieringManager with automatic tier transitions
- ✅ LRU-based access tracking (<1ms overhead)
- ✅ Background worker for scheduled demotion (every 5 minutes)
- ✅ Transparent promotion on search (automatic restore from S3)
- ✅ Manual tier control API (pin, force promote/demote)
- ✅ 26+ tests with 100% pass rate
- ✅ Configuration via TOML (tiering policies)

### Non-Goals
- ❌ Per-document tiering (collection-level only in Week 2)
- ❌ Distributed tiering (single-node only)
- ❌ Multi-region replication (Phase 11+)
- ❌ Automatic warm tier cleanup (manual for now)

---

## User Stories

### Story 1: Cost-Conscious Startup
**As a** startup with limited budget
**I want** automatic tiering of vector collections
**So that** I can reduce RAM costs by 60-80% while keeping hot data fast

**Acceptance Criteria**:
- Collections not accessed for 6h move to warm tier (disk)
- Collections not accessed for 7d move to cold tier (S3)
- Total RAM usage reduced by >60% for typical workloads

### Story 2: Machine Learning Engineer
**As an** ML engineer
**I want** transparent access to all collections
**So that** I don't need to manually manage storage tiers

**Acceptance Criteria**:
- Search on cold collection automatically restores from S3
- Search latency <10s for cold collection (first access)
- Search latency <5ms for hot collection (subsequent access)
- No manual intervention required

### Story 3: Database Administrator
**As a** database administrator
**I want** manual tier control for critical collections
**So that** I can pin important collections to RAM and demote test data to S3

**Acceptance Criteria**:
- API to pin collection to hot tier (prevent demotion)
- API to force promote collection to hot tier
- API to force demote collection to cold tier
- Pinned collections never auto-demoted

---

## Technical Specification

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    CollectionService                        │
│                                                             │
│  search() → record_access() → TieringManager                │
│                                      ↓                      │
│              ┌───────────────────────┼─────────────────┐   │
│              │                       │                 │   │
│          Hot Tier              Warm Tier          Cold Tier │
│          (RAM)                 (SSD)              (S3)      │
│          - VectorIndex         - Parquet files   - Snapshots│
│          - <1ms                - 1-10ms          - 100-500ms│
│              │                       │                 │   │
│              └───────────────────────┴─────────────────┘   │
│                                ↑                            │
│                     Background Worker                       │
│                     (runs every 5 minutes)                  │
└─────────────────────────────────────────────────────────────┘
```

### State Transitions

```
                    ┌─────────┐
                    │   Hot   │
                    │  (RAM)  │
                    └────┬────┘
                         │
        ┌────────────────┼────────────────┐
        │ (demote)       │      (promote) │
        │ No access      │      10 access │
        │ for 6h         │      in 1h     │
        ↓                ↑                │
   ┌─────────┐      ┌────────┐           │
   │  Warm   │      │  Cold  │           │
   │  (SSD)  │      │  (S3)  │           │
   └────┬────┘      └───┬────┘           │
        │               │                │
        │ (demote)      │ (promote)      │
        │ No access     │ On first       │
        │ for 7d        │ access         │
        └───────────────┴────────────────┘
```

### API Design

**TieringManager Trait**:
```rust
#[async_trait]
pub trait TieringManager: Send + Sync {
    /// Record collection access
    async fn record_access(&self, collection_id: CollectionId) -> CoreResult<()>;

    /// Get current tier state
    async fn get_tier_state(&self, collection_id: CollectionId) -> CoreResult<TierState>;

    /// Promote collection from cold to warm
    async fn promote_from_cold(&self, collection_id: CollectionId) -> CoreResult<()>;

    /// Promote collection from warm to hot
    async fn promote_from_warm(&self, collection_id: CollectionId) -> CoreResult<()>;

    /// Pin collection to hot tier (prevent demotion)
    async fn pin_collection(&self, collection_id: CollectionId) -> CoreResult<()>;

    /// Unpin collection (allow demotion)
    async fn unpin_collection(&self, collection_id: CollectionId) -> CoreResult<()>;

    /// Force promote to hot (manual control)
    async fn force_promote_to_hot(&self, collection_id: CollectionId) -> CoreResult<()>;

    /// Force demote to cold (manual control)
    async fn force_demote_to_cold(&self, collection_id: CollectionId) -> CoreResult<()>;
}
```

**TieringManager Implementation**:
```rust
pub struct DefaultTieringManager {
    access_tracker: Arc<AccessTracker>,
    policy: TieringPolicy,
    storage: Arc<StorageBackend>,
    metadata: Arc<TierStateRepository>,
    worker: Option<tokio::task::JoinHandle<()>>,
}

impl DefaultTieringManager {
    pub fn new(
        policy: TieringPolicy,
        storage: Arc<StorageBackend>,
        metadata: Arc<TierStateRepository>,
    ) -> Self;

    pub fn start_worker(&mut self);
    async fn run_tiering_cycle(&self) -> CoreResult<()>;
    async fn demote_to_warm(&self, collection_id: CollectionId) -> CoreResult<()>;
    async fn demote_to_cold(&self, collection_id: CollectionId) -> CoreResult<()>;
}
```

### Data Model

**TierState**:
```rust
pub struct TierState {
    pub collection_id: CollectionId,
    pub tier: Tier,
    pub last_accessed_at: DateTime<Utc>,
    pub access_count: u32,
    pub access_window_start: DateTime<Utc>,
    pub pinned: bool,
    pub snapshot_id: Option<SnapshotId>,
    pub warm_file_path: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub enum Tier {
    Hot,   // In RAM
    Warm,  // On local disk
    Cold,  // On S3/MinIO
}
```

**TieringPolicy**:
```rust
pub struct TieringPolicy {
    /// Hours without access before demoting hot → warm (default: 6)
    pub hot_tier_ttl_hours: i64,

    /// Days without access before demoting warm → cold (default: 7)
    pub warm_tier_ttl_days: i64,

    /// Access count threshold for promoting warm → hot (default: 10)
    pub hot_promotion_threshold: u32,

    /// Access window for promotion (default: 1 hour)
    pub access_window_hours: i64,

    /// Background worker interval (default: 300 seconds = 5 minutes)
    pub worker_interval_secs: u64,
}
```

**SQLite Schema** (new migration):
```sql
CREATE TABLE collection_tier_state (
    collection_id BLOB PRIMARY KEY REFERENCES collections(collection_id) ON DELETE CASCADE,
    tier TEXT NOT NULL CHECK(tier IN ('hot','warm','cold')),
    last_accessed_at TEXT NOT NULL,
    access_count INTEGER NOT NULL DEFAULT 0,
    access_window_start TEXT NOT NULL,
    pinned INTEGER NOT NULL DEFAULT 0,
    snapshot_id BLOB,
    warm_file_path TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE INDEX ix_tier_state_tier ON collection_tier_state(tier);
CREATE INDEX ix_tier_state_last_accessed ON collection_tier_state(last_accessed_at);
```

### Configuration

**Config Structure**:
```rust
pub struct TieringConfig {
    pub enabled: bool,
    pub policy: TieringPolicy,
    pub warm_storage_path: String,
}
```

**Example TOML**:
```toml
[tiering]
enabled = true
warm_storage_path = "./data/warm"

[tiering.policy]
hot_tier_ttl_hours = 6
warm_tier_ttl_days = 7
hot_promotion_threshold = 10
access_window_hours = 1
worker_interval_secs = 300
```

---

## Implementation Phases

### Day 1: Access Tracking Infrastructure (3-4 hours)
- Create migration for `collection_tier_state` table
- Implement `TierState` and `Tier` enums
- Implement `TierStateRepository` skeleton
- Implement `AccessTracker` struct
- Write 5 tests

### Day 2: Tiering Manager Core (3-4 hours)
- Implement `TieringPolicy` struct
- Implement `TieringManager` skeleton
- Add `record_access()` and `get_tier_state()` methods
- Implement `promote_from_cold()` and `promote_from_warm()`
- Write 4 tests (9 total)

### Day 3: Demotion Logic (3-4 hours)
- Implement `demote_to_warm()` method
- Implement `demote_to_cold()` method
- Add warm tier file I/O helpers
- Implement query methods for idle collections
- Write 4 tests (13 total)

### Day 4: Background Worker (3-4 hours)
- Implement `run_tiering_cycle()` method
- Implement `start_worker()` with Tokio interval
- Add worker shutdown logic
- Implement automatic warm → hot promotion
- Write 3 tests (16 total)

### Day 5: Integration & Polish (4-5 hours)
- Integrate TieringManager into StorageBackend
- Add tiering config to `config.toml`
- Hook into CollectionService.search()
- Add REST API endpoints (pin, unpin, force promote/demote)
- Write 10 tests (26 total)
- Update documentation

---

## Testing Strategy

### Test Pyramid

**Unit Tests** (6 tests):
- `test_init_tier_state` - Initialize new collection as hot
- `test_get_tier_state` - Retrieve tier state
- `test_update_tier_state` - Update tier (hot → warm)
- `test_record_access` - Record single access
- `test_concurrent_access` - Record 100 concurrent accesses
- `test_reset_window` - Reset access counter

**Integration Tests** (12 tests):
- `test_promote_from_cold` - Cold → Warm (download from S3)
- `test_promote_from_warm` - Warm → Hot (load from disk)
- `test_promote_cold_to_hot` - Cold → Hot (direct)
- `test_demote_to_warm` - Hot → Warm (save to disk)
- `test_demote_to_cold` - Warm → Cold (upload to S3)
- `test_demote_hot_to_cold` - Hot → Cold (direct)
- `test_find_hot_idle` - Query hot collections idle >6h
- `test_find_warm_idle` - Query warm collections idle >7d
- `test_find_warm_high_access` - Query warm collections with >10 accesses
- `test_manual_tiering_cycle` - Run cycle manually
- `test_automatic_demotion` - Worker demotes idle hot collection
- `test_automatic_promotion` - Worker promotes high-access warm collection

**E2E Tests** (4 tests):
- `test_search_cold_collection` - Search on cold triggers restore
- `test_search_warm_collection` - Search on warm loads from disk
- `test_full_tier_lifecycle` - Hot → Warm → Cold → Warm → Hot
- `test_pinned_collection` - Pinned collection never demoted

**API Tests** (4 tests):
- `test_get_tier_state_api` - GET /collections/:id/tier
- `test_pin_collection_api` - POST /collections/:id/tier/pin
- `test_force_promote_api` - POST /collections/:id/tier/promote
- `test_force_demote_api` - POST /collections/:id/tier/demote

**Total**: 26 tests

### Test Data

**Generators**:
```rust
fn create_test_collection(tier: Tier) -> CollectionId;
fn create_test_tier_state(collection_id: CollectionId, tier: Tier) -> TierState;
fn simulate_access(collection_id: CollectionId, count: u32);
```

**Sample Scenarios**:
- Small collection: 1,000 vectors (quick tests)
- Medium collection: 10,000 vectors (standard tests)
- Large collection: 100,000 vectors (stress tests)

---

## Performance Requirements

| Metric | Target | Measurement |
|--------|--------|-------------|
| Access tracking overhead | <1ms | Benchmark test |
| Cold → Warm (10k vectors) | <10s | Integration test |
| Warm → Hot (10k vectors) | <3s | Integration test |
| Hot → Warm (10k vectors) | <2s | Integration test |
| Background worker cycle | <30s | Integration test |
| Memory overhead (metadata) | <2MB for 10k collections | Memory profiler |

---

## Error Handling

### Error Types

**Validation Errors**:
- Invalid tier transition → `CoreError::ValidationError`
- Collection not found → `CoreError::NotFound`
- Tier state missing → `CoreError::InvalidState`

**Storage Errors**:
- S3 upload failed → `CoreError::StorageError` (with retry)
- S3 download failed → `CoreError::StorageError` (with retry)
- Disk full → `CoreError::StorageError` (keep in current tier)

**Concurrency Errors**:
- Tier transition in progress → `CoreError::Conflict`
- Worker shutdown in progress → `CoreError::ShuttingDown`

### Retry Policy

**Transient Errors** (retry with exponential backoff):
- S3 500 errors (server error)
- S3 503 (throttling)
- Network timeouts
- **Max retries**: 3
- **Backoff**: 1s, 2s, 4s

**Permanent Errors** (fail immediately):
- S3 403 (auth failure)
- S3 404 (not found)
- Disk full
- Invalid tier state

---

## Dependencies

### External Crates
- `tokio = "1.35"` ✅ (already added for async runtime)
- `chrono = "0.4"` ✅ (already added for timestamps)
- `sqlx = "0.7"` ✅ (already added for SQLite)

### Internal Modules
- `akidb-core` (CollectionId, CoreError, DateTime)
- `akidb-storage` (StorageBackend, ObjectStore, ParquetSnapshotter from Week 1)
- `akidb-metadata` (TierStateRepository, SQLite migrations)

### Infrastructure
- **Required**: LocalObjectStore for testing (already exists)
- **Optional**: MinIO for local S3 testing

---

## Migration Strategy

### Phase 1: Opt-In (Week 2)
- Tiering disabled by default (`enabled = false`)
- Users enable via config (`[tiering] enabled = true`)
- New collections default to hot tier
- Existing collections unaffected

### Phase 2: Gradual Rollout (Week 3-4)
- Enable tiering by default for new deployments
- Document migration guide for existing deployments
- Provide tier state initialization script

### Phase 3: Production Hardening (Week 5-6)
- Add observability metrics (Prometheus)
- Add tier state dashboards (Grafana)
- Tune default policies based on real workloads

**Migration Tool** (future):
```bash
akidb-cli tier init \
  --database-id {id} \
  --default-tier hot
```

---

## Monitoring & Observability

### Metrics

**Tier Distribution**:
- `akidb_tier_collections_total{tier}` - Counter (hot/warm/cold)

**Tier Transitions**:
- `akidb_tier_promotions_total{from, to}` - Counter
- `akidb_tier_demotions_total{from, to}` - Counter

**Access Tracking**:
- `akidb_tier_access_tracking_duration_seconds` - Histogram

**Worker**:
- `akidb_tier_worker_cycles_total` - Counter
- `akidb_tier_worker_duration_seconds` - Histogram
- `akidb_tier_worker_errors_total` - Counter

### Logging

**Structured Logs** (tracing):
```rust
tracing::info!(
    collection_id = %collection_id,
    from_tier = %from_tier,
    to_tier = %to_tier,
    duration_ms = duration.as_millis(),
    "Tier transition complete"
);
```

**Log Levels**:
- `INFO`: Tier transitions, worker cycles
- `WARN`: Failed transitions (with retry), S3 errors
- `ERROR`: Permanent failures, worker crashes

---

## Documentation

### Code Documentation
- Module-level docs (`tiering/mod.rs`)
- Struct-level docs (`TieringManager`, `TieringPolicy`)
- Method-level docs (all public methods)
- Usage examples in doc comments

### User Documentation
- Configuration guide (enable tiering, tune policies)
- API reference (tier control endpoints)
- Troubleshooting guide (common issues)

### Operator Documentation
- Deployment guide (warm tier disk requirements)
- Tuning guide (optimize for workload)
- Monitoring guide (key metrics to watch)

---

## Security Considerations

### Data Integrity
- Transactional tier state updates (SQLite)
- Verify snapshot integrity on restore (checksum)
- Atomic file operations (no partial writes)

### Access Control
- Tier control API requires admin role (Phase 3 RBAC)
- S3 bucket policies (IAM roles)
- Encryption at rest (S3 server-side encryption)

### Resource Limits
- Max warm tier disk usage (config)
- Max S3 upload size (config)
- Worker timeout (prevent infinite loops)

---

## Success Criteria

### Functional
- ✅ TieringManager implements all tier transitions
- ✅ Background worker runs automatically
- ✅ Transparent promotion on search
- ✅ Manual tier control API working
- ✅ 26 tests passing (0 failures)

### Performance
- ✅ Access tracking: <1ms overhead
- ✅ Cold → Warm: <10s for 100k vectors
- ✅ Warm → Hot: <3s for 100k vectors
- ✅ Hot → Warm: <2s for 100k vectors
- ✅ Worker cycle: <30s (typical workload)

### Quality
- ✅ Zero data corruption (integrity tests)
- ✅ Clean error handling (no panics)
- ✅ Code coverage >80%
- ✅ Documentation complete
- ✅ Code review approved

---

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| S3 download latency impacts UX | High | High | Add timeout (60s), show loading message |
| Background worker consumes too much RAM | Medium | Medium | Demote one at a time, check RAM threshold |
| Concurrent promotion/demotion race | Medium | High | Collection-level locks, atomic state updates |
| Warm tier disk full | Medium | Medium | Monitor disk space, fail gracefully |
| Worker crashes | Low | Low | Tokio panic handling, auto-restart |

---

## Timeline

**Total Duration**: 5 days (16-20 hours)

**Daily Milestones**:
- **Day 1** (EOD): Access tracking complete, 5 tests passing
- **Day 2** (EOD): Promotion logic complete, 9 tests passing
- **Day 3** (EOD): Demotion logic complete, 13 tests passing
- **Day 4** (EOD): Background worker complete, 16 tests passing
- **Day 5** (EOD): Full integration complete, 26 tests passing, docs done

---

## Approval

**Product**: ✅ Approved
**Engineering**: ✅ Approved
**Architecture**: ✅ Approved

**Go-Live Date**: Day 5 (Week 2 completion)

---

## References

- **Main PRD**: `automatosx/PRD/PHASE-10-PRODUCTION-READY-V2-PRD.md`
- **Week 1 PRD**: `automatosx/PRD/PHASE-10-WEEK-1-PARQUET-SNAPSHOTTER-PRD.md`
- **Megathink**: `automatosx/tmp/PHASE-10-WEEK-2-COMPREHENSIVE-MEGATHINK.md`
- **Action Plan**: `automatosx/tmp/PHASE-10-ACTION-PLAN.md`

---

**Status**: ✅ APPROVED - READY FOR IMPLEMENTATION
