# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

---

## Quick Reference (Most Common Commands)

```bash
# Run all tests
cargo test --workspace

# Run specific crate tests
cargo test -p akidb-metadata

# Run server (REST API on :8080)
cargo run -p akidb-rest

# Run server (gRPC API on :9090)
cargo run -p akidb-grpc

# Run smoke tests (requires servers running)
bash scripts/smoke-test.sh

# Check code without building
cargo check --workspace

# Format and lint
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings
```

---

## Project Overview

AkiDB is a RAM-first vector database optimized for ARM edge devices (Mac ARM, NVIDIA Jetson, Oracle ARM Cloud) with built-in embedding services, S3/MinIO tiered storage, and enterprise-grade multi-tenancy with RBAC.

**Target Constraints:**
- Storage: ≤100GB in-memory datasets
- Latency: P95 ≤25ms vector search @ 50 QPS
- Platform: ARM-first (Apple Silicon, Jetson Orin, Oracle ARM Cloud)

---

## Workspace Structure

This is a Cargo workspace with 10 crates:

```
crates/
├── akidb-core/         # Domain models, traits, error types (no I/O)
├── akidb-metadata/     # SQLite persistence layer for control plane
├── akidb-embedding/    # Embedding service provider traits and implementations
├── akidb-index/        # Vector indexing (brute-force, HNSW)
├── akidb-storage/      # Tiered storage (WAL, S3/MinIO, Parquet) [NEW - Phase 6]
├── akidb-service/      # Core business logic and collection management
├── akidb-proto/        # gRPC protocol definitions
├── akidb-grpc/         # gRPC API server
├── akidb-rest/         # REST API server
└── akidb-cli/          # CLI tools (migration, admin commands)
```

**Architecture Pattern:** Trait-based repository pattern with async SQLite persistence.

- **akidb-core**: Pure domain layer. Contains domain models (`TenantDescriptor`, `DatabaseDescriptor`, `CollectionDescriptor`, `VectorDocument`) and traits (`TenantCatalog`, `DatabaseRepository`, `VectorIndex`). Zero database dependencies.
- **akidb-metadata**: Implements core traits using SQLx + SQLite. Manages tenant/database/collection lifecycle, migrations, and metadata catalog.
- **akidb-embedding**: Embedding service infrastructure. Defines `EmbeddingProvider` trait with implementations:
  - `PythonBridgeProvider`: Python subprocess with ONNX Runtime + CoreML EP (production, default)
  - `OnnxEmbeddingProvider`: Pure Rust ONNX Runtime (CPU-only, optional)
  - `MlxEmbeddingProvider`: Python MLX backend (deprecated, Apple Silicon only)
  - `MockEmbeddingProvider`: Test implementation returning random embeddings
- **akidb-index**: Vector indexing implementations. Provides `BruteForceIndex` (baseline) and `InstantDistanceIndex` (production HNSW) for approximate nearest neighbor search.
- **akidb-storage**: [NEW - Phase 6] Tiered storage layer with Write-Ahead Log (WAL) for durability, S3/MinIO object store integration, Parquet snapshots, and hot/warm/cold tiering policies.
- **akidb-service**: Core business logic layer providing collection management and vector operations.
- **akidb-proto**: Protocol buffer definitions for gRPC API.
- **akidb-grpc**: gRPC API server implementation.
- **akidb-rest**: REST API server implementation (Axum).
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

# Run tests matching a pattern
cargo test --workspace -- tenant

# Run tests for a specific module
cargo test -p akidb-index hnsw

# Run ignored tests (marked with #[ignore])
cargo test --workspace -- --ignored

# Run all tests including ignored
cargo test --workspace -- --include-ignored

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

### Benchmarks

```bash
# Run all benchmarks
cargo bench --workspace

# Run specific benchmark
cargo bench --bench index_bench

# Run benchmark with baseline comparison
cargo bench --bench index_bench -- --save-baseline main

# Compare against baseline
cargo bench --bench index_bench -- --baseline main
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

### Running Servers

```bash
# Run REST API server (port 8080)
cargo run -p akidb-rest

# Run gRPC API server (port 9090)
cargo run -p akidb-grpc

# Run both servers in separate terminals
# Terminal 1:
cargo run -p akidb-rest
# Terminal 2:
cargo run -p akidb-grpc

# Run with custom config file
AKIDB_CONFIG=custom.toml cargo run -p akidb-rest

# Run with environment overrides
AKIDB_REST_PORT=3000 AKIDB_LOG_LEVEL=debug cargo run -p akidb-rest
```

**Configuration:**
- Default config: `config.toml` (copy from `config.example.toml`)
- Environment variables override config file settings
- Servers auto-create database and run migrations on startup
- Auto-initialization creates default tenant/database (can be disabled)

**Health Checks:**
```bash
# REST API
curl http://localhost:8080/health

# List collections
curl http://localhost:8080/collections

# Metrics endpoint
curl http://localhost:8080/metrics
```

### Docker & Docker Compose

```bash
# Start all services (REST, gRPC, observability stack)
docker compose up -d

# View logs
docker compose logs -f

# Stop all services
docker compose down

# Rebuild and restart
docker compose up -d --build

# Run smoke tests against Docker deployment
GRPC_HOST=localhost:9000 REST_HOST=http://localhost:8080 bash scripts/smoke-test.sh
```

### Advanced Testing

```bash
# Concurrency tests with Loom (model checker)
LOOM_MAX_PREEMPTIONS=1 cargo test -p akidb-index loom_concurrency

# Property-based tests
cargo test -p akidb-index property_tests

# Stress tests (1,000+ concurrent operations)
cargo test -p akidb-index stress_tests -- --nocapture

# Run with longer timeouts for slow tests
timeout 120 cargo test --workspace

# Thread sanitizer (requires nightly)
RUSTFLAGS="-Z sanitizer=thread" cargo +nightly test

# Memory leak detection with Miri (requires nightly)
MIRIFLAGS="-Zmiri-disable-isolation" cargo +nightly miri test
```

### Environment Configuration

**Config File Priority:**
1. Environment variables (highest priority)
2. `config.toml` (if present)
3. Built-in defaults (lowest priority)

**Key Environment Variables:**
```bash
# Server settings
AKIDB_HOST=0.0.0.0
AKIDB_REST_PORT=8080
AKIDB_GRPC_PORT=9090

# Database
AKIDB_DB_PATH=sqlite://akidb.db
DATABASE_URL=sqlite:///tmp/test.db  # For SQLx compile-time checks

# Logging
AKIDB_LOG_LEVEL=info  # trace|debug|info|warn|error
AKIDB_LOG_FORMAT=pretty  # pretty|json

# Features
AKIDB_METRICS_ENABLED=true
AKIDB_VECTOR_PERSISTENCE_ENABLED=true
```

**Quick Start:**
```bash
# 1. Copy example config
cp config.example.toml config.toml

# 2. (Optional) Edit config.toml with your settings

# 3. (Optional) Set up Python environment for embeddings
python3 -m venv .venv-onnx
.venv-onnx/bin/pip install onnxruntime-silicon transformers tokenizers

# 4. Start server
cargo run -p akidb-rest
```

---

## Embedding Providers

AkiDB supports multiple embedding providers via the `EmbeddingProvider` trait:

### Available Providers

1. **Python Bridge + ONNX Runtime (Production - RECOMMENDED)**
   ```bash
   # Build with Python bridge (default)
   cargo build

   # Install Python dependencies (Python 3.13 recommended)
   .venv-onnx/bin/pip install onnxruntime-silicon transformers tokenizers

   # Run tests with ONNX provider
   cargo test -p akidb-embedding
   ```
   - **Performance**: Excellent on Apple Silicon with CoreML EP
   - **Latency**: P95 <50ms (with GPU acceleration)
   - **Platform**: Apple Silicon (macOS ARM), NVIDIA Jetson (CUDA/TensorRT)
   - **Architecture**: Python subprocess with ONNX Runtime + CoreML EP for GPU acceleration
   - **Status**: Production-ready, default provider in v2.0.0

2. **ONNX Runtime (Pure Rust - Optional)**
   ```bash
   # Build with ONNX feature
   cargo build --features onnx

   # Run tests
   cargo test --features onnx
   ```
   - **Performance**: CPU-only unless custom-built with CoreML/TensorRT
   - **Platform**: Cross-platform (ARM + x86_64, all OS)
   - **Status**: Available but CPU-only, python-bridge preferred for GPU

3. **MLX (Legacy - Deprecated)**
   ```bash
   # Build with MLX support (Python 3.13 required)
   PYO3_PYTHON=/opt/homebrew/bin/python3.13 cargo build --features mlx

   # Run tests with MLX
   PYO3_PYTHON=/opt/homebrew/bin/python3.13 cargo test --features mlx
   ```
   - **Performance**: 5.5 QPS (GIL bottleneck)
   - **Latency**: P95 182ms
   - **Platform**: macOS ARM only (Apple Silicon)
   - **Status**: Deprecated, use python-bridge instead

4. **Mock (Testing Only)**
   ```bash
   # Mock is always available, no feature flag needed
   cargo test -p akidb-embedding
   ```
   - Returns random embeddings for testing
   - Zero latency, no model loading

### Migration History

**Candle → ONNX Runtime Migration (November 2025)**

The project migrated from Candle to ONNX Runtime with CoreML Execution Provider for better GPU acceleration on Apple Silicon:

- **Why**: CoreML EP provides native Apple GPU acceleration vs Candle's CPU-only Metal backend
- **Architecture**: Python subprocess bridge for reliability and isolation
- **Models**: Uses Hugging Face ONNX models with built-in tokenization
- **Deprecated**: Candle code moved to `crates/akidb-embedding/src/candle.rs.deprecated`

**See:** `automatosx/archive/candle-deprecated/` for historical Candle migration documentation

### Feature Flags

```toml
# In Cargo.toml (akidb-embedding)
[features]
default = ["python-bridge"]  # Python bridge with ONNX+CoreML (most reliable)
mlx = ["pyo3"]               # MLX embedding provider (deprecated)
onnx = ["ort", "ndarray", "tokenizers", "hf-hub"]  # Pure Rust ONNX
python-bridge = []           # Python subprocess bridge (recommended)
```

**Environment Variables:**
```bash
# For Python-based providers (MLX, python-bridge)
PYO3_PYTHON=/opt/homebrew/bin/python3.13  # or .venv-onnx/bin/python

# Embedding provider selection
AKIDB_EMBEDDING_PROVIDER=python-bridge  # default
AKIDB_EMBEDDING_MODEL=sentence-transformers/all-MiniLM-L6-v2

# Python path for subprocess bridge
AKIDB_EMBEDDING_PYTHON_PATH=/opt/homebrew/bin/python3.13

# Disable tokenizers parallelism (prevents warnings)
export TOKENIZERS_PARALLELISM=false
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

**Config File Loading:**
```rust
// Config loads from config.toml by default, falls back to defaults
let config = Config::load().unwrap_or_default();

// Always validate after loading
config.validate()?;

// Handle errors explicitly if needed
let config = Config::load().unwrap_or_else(|e| {
    eprintln!("Warning: Failed to load config: {}. Using defaults.", e);
    Config::default()
});
```

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

**Current Phase:** ✅ **DEVELOPMENT COMPLETE** - v2.0.0 GA Released
**Status Date:** November 10, 2025

| Phase | Status | Key Deliverables | Documentation |
|-------|--------|------------------|---------------|
| Phase 1 | ✅ Complete | Metadata layer, tenant/database management | [Report](automatosx/PRD/PHASE-1-M1-COMPLETION-REPORT.md) |
| Phase 2 | ✅ Complete | Collections, embedding infrastructure | [Design](automatosx/PRD/PHASE-2-DESIGN.md) |
| Phase 3 | ✅ Complete | User management, RBAC, audit logs | [Report](automatosx/PRD/PHASE-3-COMPLETION-REPORT.md) |
| Phase 4 | ✅ Complete | Vector indexing (BruteForce + HNSW) | [Summary](automatosx/PRD/PHASE-4-FINAL-SUMMARY.md) |
| Phase 5 | ✅ Complete | REST/gRPC servers, persistence (RC1) | [Benchmarks](docs/PERFORMANCE-BENCHMARKS.md) |
| MLX | ✅ Complete | Apple Silicon embeddings | [Report](automatosx/archive/mlx-integration/MLX-INTEGRATION-COMPLETE.md) |
| Phase 10 | ✅ Complete | S3/MinIO + Observability + K8s (GA) | [PRD](automatosx/PRD/PHASE-10-PRODUCTION-READY-V2-PRD.md) |

**Test Coverage:** 200+ tests passing (60+ unit + 50+ integration + 25+ E2E + 10+ observability + 6 chaos + 15+ benchmarks)

**Performance Targets Met:**
- ✅ Search P95 <25ms @ 100k vectors (HNSW, 512-dim, ARM)
- ✅ Insert throughput: 5,000+ ops/sec (HNSW)
- ✅ >95% recall guarantee

---

### Phase 1-5: Completed (Foundation through RC1)

**Phase 1-3:** Core infrastructure including metadata layer (SQLite), collections, embedding services, user management with Argon2id password hashing, and RBAC with audit logging.

**Phase 4:** Vector indexing with two implementations:
- **BruteForceIndex**: Baseline implementation for <10k vectors, 100% recall
- **InstantDistanceIndex**: Production HNSW via instant-distance library, >95% recall, suitable for 10k-1M+ vectors (RECOMMENDED)
- Custom HNSW: Research implementation (not recommended for production)

**Phase 5 (RC1):** Full server layer with REST/gRPC APIs, collection persistence, auto-initialization, and comprehensive testing:
- 147 tests passing (zero data corruption)
- E2E integration tests + stress tests (1,000+ concurrent operations)
- Performance benchmarks documented
- Docker Compose deployment ready

**Key Features Available:**
- Zero-configuration deployment with auto-initialization
- Collection persistence (survives server restarts)
- Dual API support (REST + gRPC)
- Search P95 <5ms @ 10k, <25ms @ 100k vectors
- >95% recall guarantee with HNSW

**See detailed completion reports in `automatosx/PRD/` and `automatosx/tmp/` for phase-specific information.**

### Phase 10: ✅ COMPLETE - Production-Ready v2.0 GA Release

**Status:** Complete (100%)
**Completion Date:** November 10, 2025
**Release:** v2.0.0 GA

**Delivered:**
- ✅ S3/MinIO tiered storage with Parquet snapshots
- ✅ Automatic hot/warm/cold tiering policies
- ✅ Prometheus + Grafana + OpenTelemetry observability
- ✅ Kubernetes Helm charts and deployment automation
- ✅ Chaos engineering tests (6 scenarios)
- ✅ Docker Compose production deployment
- ✅ Performance optimization (P95 <25ms @ 100 QPS)
- ✅ 200+ tests passing (comprehensive coverage)
- ✅ Full documentation suite

**Key Achievements:**
- Search P95 <25ms @ 100k vectors (HNSW, 512-dim, ARM)
- Insert throughput: 5,000+ ops/sec
- >95% recall guarantee
- S3 uploads >500 ops/sec
- Zero data corruption in stress tests
- Production-ready observability stack
- One-command Kubernetes deployment

**See:** [PHASE-10-PRODUCTION-READY-V2-PRD.md](automatosx/PRD/PHASE-10-PRODUCTION-READY-V2-PRD.md) for full details

---

### Post-Phase 10: Future Roadmap

After GA release, consider:
- **Phase 11**: Cedar policy engine (ABAC upgrade)
- **Phase 12**: Multi-region deployment
- **Phase 13**: Distributed vector search (sharding)
- **Phase 14**: Advanced ML features
- **Phase 15**: Enterprise features (SSO, enhanced audit)

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

#

# AutomatosX Integration

This project uses [AutomatosX](https://github.com/defai-digital/automatosx) for AI agent orchestration with persistent memory.

## Quick Commands

```bash
# List available agents
ax list agents

# Run an agent
ax run backend "implement database repository"
ax run quality "write tests for TenantCatalog"
ax run architecture "review ADR-001"

# Search past decisions
ax memory search "migration strategy"
```

## Workspace Conventions

- **`automatosx/PRD/`** - Product Requirements Documents, design specs, planning documents
- **`automatosx/tmp/`** - Temporary files, scratch work, intermediate outputs

## Key Features

- **Persistent Memory**: Agents remember all previous conversations
- **Multi-Agent Collaboration**: Agents can delegate to each other automatically
- **Natural Language**: Use natural language in Claude Code to work with agents

**Full Documentation**: See root `CLAUDE.md` for complete AutomatosX guide

---

## References

**Project Documentation:**
- Main PRD: `automatosx/PRD/akidb-2.0-improved-prd.md`
- Phase 1 Plan: `automatosx/PRD/PHASE-1-IMPLEMENTATION-PLAN.md`
- Technical Architecture: `automatosx/PRD/akidb-2.0-technical-architecture.md`
- Migration Strategy: `automatosx/PRD/akidb-2.0-migration-strategy.md`
- Performance Benchmarks: `docs/PERFORMANCE-BENCHMARKS.md` (NEW - Week 3)

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
