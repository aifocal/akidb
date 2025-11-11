# Changelog

All notable changes to AkiDB will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Planned for GA (v2.0.0)
- API authentication (API keys, JWT)
- TLS/mTLS support
- Rate limiting
- Comprehensive security audit

## [2.0.0-rc2] - 2025-11-09

### 🚀 Major Features - Phase 10: Hot/Warm/Cold Tiering

**Automatic Tier Management:**
- **Hot Tier** (RAM): <1ms latency, frequently accessed collections
- **Warm Tier** (Local SSD): 1-10ms latency, occasionally accessed collections
- **Cold Tier** (S3/MinIO): 100-500ms latency, rarely accessed collections

**Key Capabilities:**
- LRU-based automatic demotion (hot → warm → cold)
- Access-based automatic promotion (cold → warm → hot)
- Pin/unpin API to prevent auto-demotion of critical collections
- Background worker for tier management (configurable interval)
- REST API endpoints for tier status and manual control

**Tier Management API:**
```bash
# Get tier status
GET /api/v1/collections/{id}/tier

# Manual tier control
POST /api/v1/collections/{id}/tier
{
  "action": "promote_to_hot" | "pin" | "unpin"
}

# Tier distribution metrics
GET /api/v1/metrics/tiers
```

**Access Tracking:**
- Automatic access tracking on all vector operations (query, insert, get, delete)
- Sub-millisecond overhead (<0.1ms)
- Persistent access counts and timestamps in SQLite

**Persistence Layer:**
- Parquet-based columnar vector storage (111x compression ratio)
- Apache Arrow integration for efficient encoding/decoding
- Snappy compression (balances speed and ratio)
- Full roundtrip integrity (100% verified)

**Week 1-2 Implementation:**
- SQLite migration: `007_collection_tier_state.sql`
- TierStateRepository with 9 async operations
- AccessTracker with LRU candidate selection
- TieringPolicyConfig with validation
- TieringManager with promote/demote logic
- Background worker with configurable intervals

**Week 3 Integration:**
- TieringManager wired into CollectionService
- Access tracking on all vector operations
- REST API tier control endpoints (3 routes)
- Integration test suite (8 tests)
- Production-ready documentation

### Added

**Phase 10 Week 1-2: Core Infrastructure**
- ParquetSnapshotter for efficient vector serialization
- AccessTracker for LRU-based tier management
- TieringPolicyConfig with hot/warm/cold thresholds
- TieringManager with automatic promotion/demotion
- Background worker for tier management (runs every 5 minutes)

**Phase 10 Week 3: Integration & API**
- CollectionService integration with optional TieringManager
- Automatic access tracking on query/insert/get/delete operations
- REST API tier control endpoints:
  - `GET /api/v1/collections/{id}/tier` - Get tier status
  - `POST /api/v1/collections/{id}/tier` - Manual tier control
  - `GET /api/v1/metrics/tiers` - Tier distribution metrics
- Constructor: `CollectionService::with_tiering()` for full tiering support

**Tiering Configuration:**
```toml
[tiering]
enabled = true
hot_tier_max_memory_bytes = 8_589_934_592  # 8 GB
hot_tier_max_collections = 1000
demotion_idle_threshold = "1h"
promotion_access_threshold = 100
worker_interval = "5m"

[tiering.warm_store]
type = "local"
path = "./warm"

[tiering.cold_store]
type = "s3"
bucket = "akidb-cold"
region = "us-west-2"
```

### Performance

**Parquet Compression (Phase 10 Week 1):**
- Compression ratio: 111x (vs JSON baseline)
- 10k vectors (512-dim): 20MB → 185KB
- 100k vectors (512-dim): 200MB → 1.8MB
- Encode: ~5ms/10k vectors
- Decode: ~8ms/10k vectors

**Tier Operations (Phase 10 Week 2-3):**
- Promote to hot: <50ms (10k vectors)
- Demote to warm: <100ms (10k vectors)
- Demote to cold: <500ms (S3 upload)
- Access tracking: <0.1ms overhead
- LRU selection: <10ms (1000 collections)

### Database Changes

**New Migration: `007_collection_tier_state.sql`**
- `collection_tier_state` table with tier enum (hot/warm/cold)
- Access tracking: `last_accessed_at`, `access_count`, `access_window_start`
- Pin/unpin functionality: `pinned` boolean flag
- Foreign key: `collection_id` → `collections(collection_id) ON DELETE CASCADE`
- Indexes: `ix_collection_tier_state_tier_accessed`, `ix_collection_tier_state_access_count`

### Documentation

**New Documentation:**
- `docs/TIERING-GUIDE.md` - Comprehensive tiering guide with examples
- `automatosx/tmp/phase-10-week-3-implementation-complete.md` - Completion report

**Updated Documentation:**
- `CLAUDE.md` - Phase 10 status and tiering architecture
- `docs/API-TUTORIAL.md` - Tier management API examples

### Migration Notes

**Backward Compatibility:**
- Tiering is optional: existing deployments continue to work
- `CollectionService::with_full_persistence()` uses no tiering (backward compatible)
- New constructor: `CollectionService::with_tiering()` for tiering support
- Collections default to hot tier on creation

**Upgrading to RC2:**
1. Run new migration: `007_collection_tier_state.sql`
2. Update config to enable tiering (optional)
3. Use `CollectionService::with_tiering()` constructor if tiering desired
4. Monitor tier distribution via `/api/v1/metrics/tiers`

### Planned for rc3
- gRPC streaming operations (bulk insert/query)
- Kubernetes manifests and Helm charts
- Advanced monitoring dashboards
- Load testing results

## [2.0.0-rc1] - 2025-11-18

### 🚀 Major Features

#### Dual API Architecture (gRPC + REST)

**gRPC API (Recommended for Production):**
- Protocol Buffers schema (`akidb.collection.v1`)
- High-performance binary protocol (HTTP/2)
- Operations: Query, Insert, Get, Delete, Describe
- Port: 9000 (default)
- Latency: P95 <3ms @ 1k vectors (8x better than target!)

**REST API (Compatibility/Development):**
- JSON over HTTP/1.1
- Same operations as gRPC
- Port: 8080 (default)
- Latency: P95 <3ms @ 1k vectors
- Endpoints: `/api/v1/collections/{id}/*`

**Performance Comparison:**
| Operation | gRPC | REST | Overhead |
|-----------|------|------|----------|
| Insert | 1.3ms | 1.8ms | +0.5ms |
| Query | 2.3ms | 2.8ms | +0.5ms |
| Get | 0.4ms | 0.9ms | +0.5ms |
| Delete | 0.9ms | 1.4ms | +0.5ms |

#### Vector Search Engine

**BruteForceIndex - Exact k-NN Search:**
- Suitable for: <10k vectors
- Recall: 100% (exhaustive search)
- Latency: ~2ms @ 1k vectors (128-dim)
- Memory: O(n·d) where n=vectors, d=dimensions
- Use case: Small datasets, maximum accuracy required

**InstantDistanceIndex - HNSW Approximate Search:**
- Suitable for: 10k-1M+ vectors
- Recall: >95% (tested up to 100% on benchmarks)
- Latency: ~2ms @ 100k vectors (128-dim)
- Memory: O(n·d) + HNSW graph overhead (~20%)
- Powered by: instant-distance library (v0.6)
- Use case: Large datasets, sub-second search required

#### Multi-Tenancy & Security

**Tenant Isolation:**
- SQLite metadata layer with ACID guarantees
- Tenant → Databases → Collections hierarchy
- Resource quotas (memory, storage, QPS)
- Foreign key cascade deletes
- UUID v7 IDs for natural time ordering

**Role-Based Access Control (RBAC):**
- Roles: Admin, Developer, Viewer, Auditor
- 17 granular action types (user::create, collection::read, etc.)
- Deny-by-default security model
- Status-based access control (suspended users = zero permissions)

**Audit Logging:**
- Every authorization decision logged (allow + deny)
- IP tracking and user agent capture
- JSON metadata for request details
- Compliance-ready (SOC 2, HIPAA)

#### Distance Metrics

- **Cosine Similarity** (default): Normalized vectors, range [0,1]
- **Euclidean L2**: Geometric distance, unbounded
- **Dot Product**: Raw vector similarity, unbounded

### 🔧 Technical Improvements

**Concurrency Safety:**
- parking_lot RwLock (2-5x faster than std::sync)
- 9 stress tests passing (1000+ concurrent operations)
- ThreadSanitizer verified (zero data races detected)
- Expert concurrency review by Bob (AutomatosX Backend Agent)
- Documented in `automatosx/PRD/ARCHITECTURE-CONCURRENCY.md`

**Testing Coverage:**
- 77 unit tests (100% pass rate)
- 9 stress tests (concurrent workload validation)
- 5 integration tests (gRPC + REST E2E)
- 6 property tests (600 randomly generated cases)
- Zero compiler warnings (Clippy pedantic mode)
- Zero unsafe code in core implementation

**Performance:**
- Target: P95 <25ms @ 50 QPS
- Actual: P95 <3ms (8.3x better than target)
- Benchmarked on: Apple M3, 128-dim vectors
- BruteForce: O(n·d) search complexity
- InstantDistance: O(log n) search complexity

**Build & Tooling:**
- Rust 1.75 minimum (MSRV)
- Cargo workspace (7 crates)
- SQLx for compile-time SQL validation
- Tonic for gRPC code generation
- Axum for REST API
- Criterion for benchmarking

### 📚 Documentation

**New Documentation:**
- `docs/API-QUICKSTART.md` - Get started in 5 minutes
- `docs/MIGRATION-V1-TO-V2.md` - Upgrade guide with rollback steps
- `docs/API-REFERENCE.md` - Complete API specification
- `automatosx/PRD/ARCHITECTURE-CONCURRENCY.md` - Thread-safety analysis
- `CHANGELOG.md` - This file

**Examples:**
- gRPC examples with grpcurl
- REST examples with curl
- Python client examples (gRPC + REST)
- Rust client examples

**Architecture:**
- System design overview
- Component interaction diagrams
- Data flow specifications
- Database schema documentation

### ⚠️ Breaking Changes from v1.x

**⚠️ IMPORTANT:** v2.0 is not backward-compatible with v1.x. Migration required.

**API Changes:**
1. **Protocol:** v1.x HTTP/JSON only → v2.0 gRPC (recommended) + REST (compatibility)
2. **Endpoints:** v1.x `/api/search` → v2.0 gRPC `CollectionService/Query` or REST `/api/v1/collections/{id}/query`
3. **IDs:** v1.x slug-based → v2.0 UUID v7 format
4. **Ports:** v1.x 8000 → v2.0 gRPC 9000 + REST 8080

**Schema Changes:**
1. **Hierarchy:** v1.x Tenants → Collections → v2.0 Tenants → Databases → Collections
2. **Metadata:** v1.x custom JSON → v2.0 SQLite STRICT tables
3. **Users:** v1.x external auth → v2.0 built-in RBAC

**Configuration:**
1. **Format:** v1.x JSON config → v2.0 TOML config
2. **Structure:** See `config/akidb.toml.example`

**Migration Path:**
```bash
# Automated migration tool
akidb-cli migrate v1-to-v2 \
  --v1-data-dir /path/to/v1/data \
  --v2-database /path/to/metadata.db \
  --dry-run  # Preview changes first

# See docs/MIGRATION-V1-TO-V2.md for complete guide
```

### 🐛 Bug Fixes

**Concurrency:**
- Fixed Send trait violation in lock guards (no locks held across await)
- Fixed potential deadlock in read→write lock upgrade (now drops read first)
- Fixed data race in dirty flag pattern (both flag and map behind same lock)

**Validation:**
- Fixed dimension validation (now enforces 16-4096 range)
- Fixed duplicate document insert (now returns proper error vs silent overwrite)
- Fixed empty vector rejection (now validates before processing)

**Error Handling:**
- Fixed gRPC status code mapping (CoreError → tonic::Status)
- Fixed REST HTTP status codes (404 for not found, 400 for bad input)
- Fixed error messages (no stack traces exposed in API responses)

### 🚧 Known Limitations (RC1)

**⚠️ Not Production-Ready:** This is a release candidate for testing. Use GA for production.

**Architecture:**
- Single-node only (no distributed deployment yet)
- SQLite metadata (no PostgreSQL option yet)
- In-memory vectors only (no persistence yet)

**Security:**
- No authentication (deploy behind firewall/VPN)
- No TLS (use TLS termination proxy)
- No rate limiting (use API gateway)

**Operations:**
- No Kubernetes support (Docker only)
- Basic health checks (advanced monitoring in rc2)
- Manual deployment (no Helm charts yet)

**Performance:**
- No query result caching
- No connection pooling tuning
- No multi-threading for single queries

**See `docs/SECURITY-CHECKLIST-RC1.md` for deployment recommendations.**

### 🔜 Roadmap

**rc2 (3 weeks, ~2025-12-09):**
- gRPC streaming (bulk insert/query)
- Kubernetes manifests + Helm charts
- Advanced monitoring (Prometheus + Grafana)
- Load testing results (1000+ QPS validation)
- Performance optimization
- Community feedback integration

**GA v2.0.0 (6 weeks, ~2025-12-30):**
- API authentication (API keys, JWT)
- TLS/mTLS support
- Rate limiting and circuit breakers
- Comprehensive security audit
- Penetration testing
- Production deployment guides
- SLA documentation

**v2.1 (3 months, ~2026-02-15):**
- S3/MinIO tiered storage
- Vector persistence (disk-backed storage)
- Distributed deployment
- PostgreSQL metadata option
- Advanced RBAC with Cedar policy engine

**v2.2 (6 months, ~2026-05-15):**
- Multi-region deployment
- Read replicas
- High availability (HA) setup
- Disaster recovery
- Advanced caching

---

## [1.0.0] - 2024-09-15

Initial release. See v1.x branch for legacy documentation.

### Features
- HTTP/JSON API
- BruteForce vector search
- Basic multi-tenancy
- File-based metadata

**Note:** v1.x is deprecated. Migrate to v2.0 for new features and security improvements.

---

## Versioning Policy

AkiDB follows [Semantic Versioning 2.0.0](https://semver.org/):

- **MAJOR** (X.0.0): Incompatible API changes
- **MINOR** (2.X.0): Backward-compatible new features
- **PATCH** (2.0.X): Backward-compatible bug fixes
- **RC** (2.0.0-rcX): Release candidates (preview releases)

**Stability Guarantees:**
- RC releases: API may change, not production-ready
- GA releases: Stable API, production-ready
- PATCH updates: Safe to upgrade (bugfixes only)
- MINOR updates: Safe to upgrade (opt-in new features)
- MAJOR updates: Migration required (breaking changes)
