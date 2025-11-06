# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

---

## Project Overview

AkiDB 2.0 is a RAM-first vector database optimized for ARM edge devices (Mac ARM, NVIDIA Jetson, Oracle ARM Cloud) with built-in embedding services, S3/MinIO tiered storage, and enterprise-grade multi-tenancy with RBAC.

**Target Constraints:**
- Storage: ≤100GB in-memory datasets
- Latency: P95 ≤25ms vector search @ 50 QPS
- Platform: ARM-first (Apple Silicon, Jetson Orin, Oracle ARM Cloud)

---

## Workspace Structure

This is a Cargo workspace with five crates:

```
crates/
├── akidb-core/         # Domain models, traits, error types (no I/O)
├── akidb-metadata/     # SQLite persistence layer for control plane
├── akidb-embedding/    # Embedding service provider traits and implementations
├── akidb-index/        # Vector indexing (brute-force, HNSW)
└── akidb-cli/          # CLI tools (migration, admin commands)
```

**Architecture Pattern:** Trait-based repository pattern with async SQLite persistence.

- **akidb-core**: Pure domain layer. Contains domain models (`TenantDescriptor`, `DatabaseDescriptor`, `CollectionDescriptor`, `VectorDocument`) and traits (`TenantCatalog`, `DatabaseRepository`, `VectorIndex`). Zero database dependencies.
- **akidb-metadata**: Implements core traits using SQLx + SQLite. Manages tenant/database/collection lifecycle, migrations, and metadata catalog.
- **akidb-embedding**: Embedding service infrastructure. Defines `EmbeddingProvider` trait with mock implementation for testing. Future: MLX/ONNX backends.
- **akidb-index**: Vector indexing implementations. Provides `BruteForceIndex` (baseline) and future `HnswIndex` for approximate nearest neighbor search.
- **akidb-cli**: Migration tools for v1.x → v2.0 upgrades and admin operations.

---

## Common Commands

### Build & Test

```bash
# Build entire workspace (release mode recommended for ARM)
cargo build --workspace --release

# Run all tests
cargo test --workspace

# Run specific crate tests
cargo test -p akidb-core
cargo test -p akidb-metadata

# Run a single test
cargo test test_name -- --nocapture

# Check without building
cargo check --workspace
```

### Linting & Formatting

```bash
# Run clippy (fail on warnings)
cargo clippy --all-targets --all-features -- -D warnings

# Format code
cargo fmt --all

# Check formatting without modifying
cargo fmt --all -- --check
```

### Documentation

```bash
# Generate and open docs
cargo doc --workspace --no-deps --open

# Build docs only
cargo doc --workspace --no-deps
```

### Database Migrations

```bash
# Create SQLite database and run migrations
cd crates/akidb-metadata
cargo sqlx database create
cargo sqlx migrate run

# Check migration status
cargo sqlx migrate info

# Revert last migration
cargo sqlx migrate revert
```

### Migration Tool (v1.x → v2.0)

```bash
# Dry-run migration (preview changes)
cargo run -p akidb-cli -- migrate v1-to-v2 \
  --v1-data-dir /path/to/v1/data \
  --v2-database /path/to/metadata.db \
  --dry-run

# Execute migration
cargo run -p akidb-cli -- migrate v1-to-v2 \
  --v1-data-dir /path/to/v1/data \
  --v2-database /path/to/metadata.db
```

---

## Key Architectural Decisions

### ADR-001: SQLite for Metadata Storage
- **Decision**: Use SQLite (STRICT mode, WAL journal) for control plane metadata.
- **Rationale**: Serverless, ACID guarantees, <1ms read latency, excellent ARM support.
- **Trade-offs**: Single-writer constraint (acceptable for control plane write volume).

### ADR-002: Cedar Policy Engine (Phase 3+)
- **Decision**: Use AWS Cedar for RBAC policy evaluation.
- **Location**: See `automatosx/PRD/ADR-002-cedar-policy-engine.md`

### ADR-003: Dual API Strategy
- **Decision**: Support both gRPC (primary) and REST (compatibility) APIs.
- **Location**: See `automatosx/PRD/ADR-003-dual-api-strategy.md`

---

## Domain Model Hierarchy

```
Tenant (Multi-tenancy root)
  ├── TenantId (UUID v7)
  ├── TenantStatus: Provisioning | Active | Suspended | Decommissioned
  ├── TenantQuota: memory_quota_bytes, storage_quota_bytes, qps_quota
  ├── users: Vec<UserDescriptor>
  │   └── User (Authentication and authorization)
  │       ├── UserId (UUID v7)
  │       ├── email: String (unique per tenant)
  │       ├── password_hash: String (Argon2id)
  │       ├── role: Admin | Developer | Viewer | Auditor
  │       ├── status: Active | Suspended | Deactivated
  │       └── last_login_at: Option<DateTime<Utc>>
  │
  ├── audit_logs: Vec<AuditLogEntry>
  │   └── AuditLog (Compliance and security monitoring)
  │       ├── AuditLogId (UUID v7)
  │       ├── action: Action (17 types: user::create, collection::read, etc.)
  │       ├── result: Allowed | Denied
  │       ├── metadata: JSON (request details)
  │       └── ip_address, user_agent: String
  │
  └── databases: Vec<DatabaseDescriptor>
      └── Database (Logical namespace for collections)
          ├── DatabaseId (UUID v7)
          ├── DatabaseState: Provisioning | Ready | Migrating | Deleting
          └── collections: Vec<CollectionDescriptor>
              └── Collection (Vector collection with embedding model)
                  ├── CollectionId (UUID v7)
                  ├── dimension: u32 (16-4096)
                  ├── metric: Cosine | Dot | L2
                  ├── embedding_model: String
                  ├── HNSW params: m, ef_construction
                  └── documents: Vec<VectorDocument>
                      └── VectorDocument (Phase 4)
                          ├── DocumentId (UUID v7)
                          ├── external_id: Option<String> (user-provided ID)
                          ├── vector: Vec<f32> (dense embedding)
                          ├── metadata: Option<JsonValue>
                          └── inserted_at: DateTime<Utc>
```

**Critical Constraints:**
- UUIDs use v7 (time-ordered) for natural SQLite index ordering.
- All timestamps are `chrono::DateTime<Utc>` (ISO-8601 in SQLite).
- Foreign keys cascade: DELETE tenant → DELETE databases → DELETE collections.

---

## SQLite Schema (Phase 1 + Phase 2 + Phase 3)

**Tables:**
- `tenants`: Tenant metadata with quotas (memory, storage, QPS)
- `databases`: Logical database entities (NEW in v2.0, not in v1.x)
- `collections`: Vector collections with embedding model config (Phase 2)
- `users`: Tenant-scoped users with email/password/role (Phase 3)
- `audit_logs`: Compliance audit trail with action tracking (Phase 3)

**Indexes:**
- `ux_databases_tenant_name`: Enforce unique database names per tenant
- `ux_collections_database_name`: Enforce unique collection names per database
- `ux_users_tenant_email`: Enforce unique email per tenant
- `ix_audit_logs_tenant_created`: Time-series queries by tenant
- `ix_audit_logs_user_created`: Time-series queries by user
- `ix_audit_logs_denied`: Security monitoring (denied actions)

**Triggers:**
- Auto-update `updated_at` on all UPDATE operations

**Pragmas (Required):**
```sql
PRAGMA journal_mode=WAL;       -- Enable write-ahead logging
PRAGMA foreign_keys=ON;        -- Enforce referential integrity
```

**Collections Schema (Phase 2):**
```sql
CREATE TABLE collections (
    collection_id BLOB PRIMARY KEY,
    database_id BLOB NOT NULL REFERENCES databases(database_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    dimension INTEGER NOT NULL CHECK(dimension BETWEEN 16 AND 4096),
    metric TEXT NOT NULL CHECK(metric IN ('cosine','dot','l2')),
    embedding_model TEXT NOT NULL,
    hnsw_m INTEGER NOT NULL DEFAULT 32,
    hnsw_ef_construction INTEGER NOT NULL DEFAULT 200,
    max_doc_count INTEGER NOT NULL DEFAULT 50000000,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(database_id, name)
) STRICT;
```

**Users Schema (Phase 3):**
```sql
CREATE TABLE users (
    user_id BLOB PRIMARY KEY,
    tenant_id BLOB NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    password_hash TEXT NOT NULL,  -- Argon2id hash
    role TEXT NOT NULL CHECK(role IN ('admin','developer','viewer','auditor')),
    status TEXT NOT NULL CHECK(status IN ('active','suspended','deactivated')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_login_at TEXT,
    UNIQUE(tenant_id, email)
) STRICT;
```

**Audit Logs Schema (Phase 3):**
```sql
CREATE TABLE audit_logs (
    audit_log_id BLOB PRIMARY KEY,
    tenant_id BLOB NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    user_id BLOB REFERENCES users(user_id) ON DELETE SET NULL,
    action TEXT NOT NULL,  -- e.g., 'user::create', 'collection::delete'
    resource_type TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    result TEXT NOT NULL CHECK(result IN ('allowed','denied')),
    reason TEXT,
    metadata TEXT,  -- JSON
    ip_address TEXT,
    user_agent TEXT,
    created_at TEXT NOT NULL
) STRICT;
```

---

## Development Workflow

### Adding a New Domain Model

1. Define pure domain types in `akidb-core/src/`:
   - Add struct with serde derives
   - Add state enums with `as_str()` and `FromStr`
   - Add constructor with defaults (e.g., `::new()`)

2. Add trait methods to `akidb-core/src/traits.rs`:
   - Use `#[async_trait]` for async methods
   - Return `CoreResult<T>` for all fallible ops

3. Implement persistence in `akidb-metadata/src/`:
   - Create SQL migration in `migrations/`
   - Implement repository struct (e.g., `SqliteXxxRepository`)
   - Map SQLite errors to `CoreError`

4. Add integration tests in `akidb-metadata/tests/`:
   - Use in-memory SQLite: `connect(":memory:")`
   - Test happy path + edge cases (FK violations, unique constraints)

### Common Pitfalls

**UUID Conversion:**
```rust
// SQLite stores UUIDs as BLOB (16 bytes)
// Correct conversion:
let uuid_bytes = tenant_id.as_bytes();
let tenant_id = TenantId::from_bytes(row.try_get("tenant_id")?);
```

**DateTime Handling:**
```rust
// SQLite stores timestamps as TEXT (ISO-8601)
// Use sqlx::types::chrono::DateTime<Utc> directly
created_at: row.try_get("created_at")?,
```

**SQLx Macro Gotcha:**
- `sqlx::query!()` validates SQL at compile time against DATABASE_URL
- Set `DATABASE_URL=sqlite:///tmp/test.db` in `.env` for macro validation
- If schema changes, run `cargo sqlx prepare --workspace` to regenerate metadata

---

## Testing Strategy

### Unit Tests (akidb-core)
- Test pure domain logic (state transitions, validation)
- No async or I/O required

### Integration Tests (akidb-metadata)
- Test repository implementations against real SQLite
- Use `:memory:` database for isolation
- Cover: CRUD operations, FK constraints, unique violations, cascade deletes

### Performance Benchmarks (Phase 1+)
- Located in `benches/` (uses Criterion)
- Target: Tenant CRUD P95 < 5ms

**Run Benchmarks:**
```bash
cargo bench --bench tenant_crud
```

---

## Development Status

### Phase 1: ✅ COMPLETED (M1 - Foundation/Metadata Layer)

**Deliverables:**
- ✅ Workspace setup (4 crates: core, metadata, embedding, cli)
- ✅ akidb-core domain models (TenantDescriptor, DatabaseDescriptor, IDs)
- ✅ akidb-core traits (TenantCatalog, DatabaseRepository)
- ✅ akidb-metadata SQLite migrations (001_initial_schema.sql)
- ✅ akidb-metadata repository implementations (all working)
- ✅ Integration tests (10 tests passing: tenant CRUD, database CRUD, FK cascades, unique constraints)
- ✅ Zero compiler warnings, all clippy checks passing

**Completion Report:** `automatosx/PRD/PHASE-1-M1-COMPLETION-REPORT.md`

### Phase 2: ✅ COMPLETED (Embedding Service Infrastructure)

**Deliverables:**
- ✅ CollectionDescriptor domain model in akidb-core
- ✅ CollectionRepository trait with full CRUD operations
- ✅ SQLite migration (002_collections_table.sql) with dimension validation (16-4096)
- ✅ SqliteCollectionRepository implementation (runtime query validation)
- ✅ akidb-embedding crate with EmbeddingProvider trait
- ✅ MockEmbeddingProvider with deterministic embeddings for testing
- ✅ Integration tests (7 collection tests + 5 embedding tests = 12 new tests)
- ✅ All 22 tests passing (10 Phase 1 + 12 Phase 2)

**Key Design Decisions:**
- Trait-based architecture for embedding providers (supports future MLX/ONNX backends)
- Mock implementation using deterministic hash-based embeddings (no ML dependencies)
- Runtime SQLx validation (avoids DATABASE_URL compile-time dependency)
- Distance metrics: Cosine (default), Dot, L2

**Design Document:** `automatosx/PRD/PHASE-2-DESIGN.md`

### Phase 3: ✅ COMPLETED (User Management & RBAC)

**Deliverables:**
- ✅ UserDescriptor domain model with secure password hashing (Argon2id)
- ✅ Role-based permissions (Admin, Developer, Viewer, Auditor)
- ✅ AuditLogEntry domain model for compliance
- ✅ UserRepository and AuditLogRepository traits
- ✅ SQLite migrations (003_users_table.sql, 004_audit_logs_table.sql)
- ✅ SqliteUserRepository and SqliteAuditLogRepository implementations
- ✅ Password hashing utilities with Argon2id (OWASP recommended)
- ✅ Integration tests (8 user + 4 RBAC + 3 audit = 15 new tests)
- ✅ All 40 tests passing (10 Phase 1 + 7 Phase 2 collections + 5 Phase 2 embedding + 3 password + 15 Phase 3)

**Key Design Decisions:**
- Enum-based RBAC for Phase 3 (pragmatic, production-ready)
- Argon2id password hashing (memory-hard, resistant to GPU/ASIC attacks)
- Action-based permissions (17 action types: user::create, collection::read, etc.)
- Audit logging with IP tracking and metadata for compliance (SOC 2, HIPAA ready)
- Multi-tenant isolation (users scoped to tenants, cascade deletes)

**Security Features:**
- Password hashing: Argon2id with unique salts (OWASP recommended)
- RBAC: Deny by default, least privilege model
- Audit trail: Every authorization decision logged (allow + deny)
- Status-based access control: Suspended users have zero permissions

**Design Document:** `automatosx/PRD/PHASE-3-DESIGN.md`
**Completion Report:** `automatosx/PRD/PHASE-3-COMPLETION-REPORT.md`

### Phase 4A: ✅ COMPLETED (Vector Engine - BruteForce Baseline)

**Deliverables:**
- ✅ VectorDocument and SearchResult domain models in akidb-core
- ✅ VectorIndex trait with insert/search/delete/batch operations
- ✅ Distance metric implementations (cosine similarity, Euclidean L2, dot product)
- ✅ akidb-index crate with BruteForceIndex implementation
- ✅ Unit tests (11 vector core + 10 brute-force = 21 new tests)
- ✅ Doctests and examples for public API
- ✅ Benchmarking infrastructure with Criterion
- ✅ All 58 tests passing (32 Phase 1-3 integration + 5 embedding + 11 vector + 10 brute-force)

**Key Design Decisions:**
- Incremental approach: Start with correct brute-force baseline before HNSW optimization
- Trait-based VectorIndex interface for multiple implementations (brute-force, HNSW, IVF)
- Distance metrics use existing DistanceMetric enum (Cosine, Dot, L2)
- 100% Rust safe code with parking_lot::RwLock for concurrency
- Builder pattern for VectorDocument and SearchResult

**Performance Characteristics:**
- BruteForceIndex: O(n·d) search, suitable for <10k vectors
- Expected: ~5ms @ 10k vectors (512-dim, ARM M3)
- Memory: O(n·d) storage with HashMap backing
- 100% recall (exhaustive search, perfect accuracy)

**Design Document:** `automatosx/PRD/PHASE-4-DESIGN.md`
**Completion Report:** `automatosx/PRD/PHASE-4-COMPLETION-REPORT.md`
**Final Summary:** `automatosx/PRD/PHASE-4-FINAL-SUMMARY.md`

### Phase 4B: ⚠️ PARTIALLY COMPLETE (HNSW Approximate Nearest Neighbor)

**Status:** ⚠️ 85% Complete - Functional but recall below production targets

**Test Results:**
- ✅ 7/7 unit tests passing (insert, search, delete, get, clear, configs, dimension validation)
- ⚠️ 0/5 recall integration tests passing (60-70% recall vs target >80-90%)

**What Works:**
- ✅ HNSW data structures (Node, HnswState, hierarchical layers)
- ✅ Config presets (balanced, edge_cache, high_recall) with proper M and ef parameters
- ✅ Insert algorithm with exponential layer assignment and bidirectional edge creation
- ✅ Delete algorithm with soft-delete tombstone marking
- ✅ Search algorithm with greedy traversal (functionally correct)
- ✅ Metric-aware heap ordering (L2 vs Cosine/Dot)
- ✅ Entry point preservation during layer traversal

**Critical Fixes Applied:**
1. Fixed worst-element finding logic in working set (min_by vs max_by)
2. Fixed candidates heap ordering for Cosine similarity (negation for min-heap)
3. Fixed entry points preservation when no neighbors found at sparse layers
4. Fixed search termination condition to allow working set to fill

**Current Limitations:**
- ⚠️ Recall: 60-70% @ 100 vectors, 2-6% @ 1000 vectors (target: >80-90%)
- ⚠️ Performance not validated (no benchmarks run yet)
- ⚠️ Neighbor selection uses simple heuristic (not Algorithm 4 from paper)
- ⚠️ Graph connectivity may degrade with incremental inserts

**Root Cause Analysis:**
The HNSW implementation is algorithmically sound and passes all functional tests, but graph connectivity quality is suboptimal. Likely issues:
1. Neighbor selection heuristic too simplistic (takes first M, doesn't optimize for diversity)
2. Pruning strategy may disconnect parts of the graph
3. Entry point selection during insert may not find optimal paths

**Estimated Effort to Fix:** 3-5 days of algorithmic refinement + extensive testing

**Recommendation:** Use external library for production

**Alternatives (In Priority Order):**
1. **RECOMMENDED:** Integrate battle-tested external library (instant-distance or hnswlib-rs)
   - Effort: 1 day
   - Benefit: Production-ready >95% recall, well-optimized
   - Trade-off: External dependency, less control

2. Refine custom HNSW with Algorithm 4 heuristic + better pruning
   - Effort: 3-5 days
   - Benefit: Full control, learning opportunity
   - Trade-off: Time investment, may still need tuning

3. Implement simpler IVF (Inverted File) index first
   - Effort: 2-3 days
   - Benefit: Good for 100k-1M vectors, easier to get right
   - Trade-off: Lower recall than HNSW at same latency

**For MVP:** Use Phase 4A BruteForceIndex (production-ready for <10k vectors) until Phase 4B is production-quality or external library is integrated.

**Next Steps:**
- Phase 4C: Integrate instant-distance library (RECOMMENDED) OR refine custom HNSW
- Phase 4D: ARM NEON SIMD optimizations for distance functions
- Target: P95 < 25ms @ 100k vectors with >95% recall

### Phase 5+: Pending

- ⏸️ Phase 5: S3/MinIO tiered storage integration
- ⏸️ Phase 6: Cedar policy engine migration (optional ABAC upgrade)
- ⏸️ Phase 7: Production hardening (WAL, crash recovery, distributed deployment)

---

## Known Issues & Workarounds

**Issue:** SQLx macro borrow checker errors with `&database.field`
- **Workaround:** Extract values to local variables before `query!()` macro:
  ```rust
  let name = &database.name;
  let description = &database.description;
  sqlx::query!("... VALUES (?1, ?2)", name, description)
  ```

**Issue:** Database not found during `cargo sqlx prepare`
- **Workaround:** Create SQLite database first:
  ```bash
  cd crates/akidb-metadata
  cargo sqlx database create
  cargo sqlx migrate run
  cargo sqlx prepare --workspace
  ```

---

## Performance Targets

**Phase 1 (Metadata Layer):**
- Tenant CRUD: P95 < 5ms
- Database CRUD: P95 < 5ms
- Foreign key checks: < 1ms overhead

**Phase 2+ (Vector Engine):**
- Vector search: P95 < 25ms @ 50 QPS
- Embedding generation: < 50ms/batch (512-dim)
- Memory footprint: ≤100GB dataset size

---

## AutomatosX Integration

This project uses [AutomatosX](https://github.com/defai-digital/automatosx) - an AI agent orchestration platform with persistent memory and multi-agent collaboration.

**File Conventions:**
- `automatosx/PRD/` - Product Requirements Documents, design specs, ADRs, and planning documents
- `automatosx/tmp/` - Temporary files, scratch work, and intermediate outputs (auto-cleaned)

**Common Agent Commands:**
```bash
# List available agents
ax list agents

# Work with specific agents
ax run backend "implement database repository"
ax run architecture "review ADR-001"
ax run quality "write integration tests for TenantCatalog"

# Search past decisions
ax memory search "migration strategy"
```

**See:** Root `CLAUDE.md` (parent directory) for full AutomatosX documentation.

---

## References

**Project Documentation:**
- Main PRD: `automatosx/PRD/akidb-2.0-improved-prd.md`
- Phase 1 Plan: `automatosx/PRD/PHASE-1-IMPLEMENTATION-PLAN.md`
- Technical Architecture: `automatosx/PRD/akidb-2.0-technical-architecture.md`
- Migration Strategy: `automatosx/PRD/akidb-2.0-migration-strategy.md`

**External Dependencies:**
- SQLx documentation: https://docs.rs/sqlx/latest/sqlx/
- Tokio async runtime: https://docs.rs/tokio/latest/tokio/
- UUID v7 spec: https://www.ietf.org/archive/id/draft-peabody-dispatch-new-uuid-format-04.html

---

## Rust Version Requirements

- **Minimum:** Rust 1.75 (MSRV set in workspace Cargo.toml)
- **Recommended:** Latest stable for Apple Silicon optimizations
- **Check Version:** `cargo --version`

**Installation:**
```bash
# Update to latest stable
rustup update stable

# Target ARM builds
rustup target add aarch64-apple-darwin
```
