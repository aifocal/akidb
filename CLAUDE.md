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
- **akidb-embedding**: Embedding service infrastructure. Defines `EmbeddingProvider` trait with mock implementation for testing. Future: MLX/ONNX backends.
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

# 3. Start server
cargo run -p akidb-rest
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

**Current Phase:** Phase 10 - Production-Ready v2.0 GA Release (6-week sprint)

| Phase | Status | Key Deliverables | Documentation |
|-------|--------|------------------|---------------|
| Phase 1 | ✅ Complete | Metadata layer, tenant/database management | [Report](automatosx/PRD/PHASE-1-M1-COMPLETION-REPORT.md) |
| Phase 2 | ✅ Complete | Collections, embedding infrastructure | [Design](automatosx/PRD/PHASE-2-DESIGN.md) |
| Phase 3 | ✅ Complete | User management, RBAC, audit logs | [Report](automatosx/PRD/PHASE-3-COMPLETION-REPORT.md) |
| Phase 4 | ✅ Complete | Vector indexing (BruteForce + HNSW) | [Summary](automatosx/PRD/PHASE-4-FINAL-SUMMARY.md) |
| Phase 5 | ✅ Complete | REST/gRPC servers, persistence (RC1) | [Benchmarks](docs/PERFORMANCE-BENCHMARKS.md) |
| MLX | ✅ Complete | Apple Silicon embeddings | [Report](automatosx/archive/mlx-integration/MLX-INTEGRATION-COMPLETE.md) |
| Phase 10 | 🚧 IN PROGRESS | S3/MinIO + Observability + K8s (GA) | [PRD](automatosx/PRD/PHASE-10-PRODUCTION-READY-V2-PRD.md) |

**Test Coverage:** 147 tests passing (11 unit + 36 integration + 16 index + 4 recall + 17 E2E + 25 stress + 38 other)

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

### Phase 10: 🚧 IN PROGRESS - Production-Ready v2.0 GA Release (6 weeks)

**Status:** Ready to Start (0% complete)
**Timeline:** 6 weeks (30 days)
**Goal:** Complete S3/MinIO tiered storage + production hardening for GA release

**Phase 10 consolidates:**
- Phase 6 remaining work (Parquet, tiering, RC2)
- Phase 7 remaining work (performance, observability, K8s)

**Full PRD:** [PHASE-10-PRODUCTION-READY-V2-PRD.md](automatosx/PRD/PHASE-10-PRODUCTION-READY-V2-PRD.md)
**Action Plan:** [PHASE-10-ACTION-PLAN.md](automatosx/tmp/PHASE-10-ACTION-PLAN.md)

**6-Week Breakdown:**

**Part A: S3/MinIO Tiered Storage Completion (Weeks 1-3)**
- **Week 1**: Parquet Snapshotter (~500 lines, 10 tests)
- **Week 2**: Hot/Warm/Cold Tiering Policies (~400 lines, 12 tests)
- **Week 3**: Integration Testing + RC2 Release (20 tests, docs)

**Part B: Production Hardening Completion (Weeks 4-6)**
- **Week 4**: Performance Optimization + E2E Testing (15 tests, benchmarks)
- **Week 5**: Observability (12 metrics, 4 Grafana dashboards, tracing)
- **Week 6**: Kubernetes + Chaos Tests + GA Release (Helm chart, 5 chaos tests)

**Completed Foundation (from Phase 6-7):**
- ✅ WAL infrastructure (15 tests passing)
- ✅ S3/ObjectStore integration (19 tests passing)
- ✅ Circuit breaker + DLQ (reliability hardening)
- ✅ 142 tests baseline

**Remaining Deliverables:**
- Parquet snapshotter with S3 integration
- Automatic hot/warm/cold tiering
- Performance optimization (>500 ops/sec S3 uploads)
- Prometheus + Grafana + OpenTelemetry observability
- Kubernetes Helm charts
- Blue-green deployment automation
- Chaos engineering tests
- GA release (v2.0.0)

**Success Metrics:**
- ✅ 200+ tests passing (target)
- ✅ P95 <25ms @ 100 QPS
- ✅ S3 uploads >500 ops/sec
- ✅ K8s deployment (1 command)
- ✅ Full observability stack
- ✅ GA release published

**See PRD for detailed week-by-week action plan, dependencies, and deliverables.**

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

This project uses [AutomatosX](https://github.com/defai-digital/automatosx) - an AI agent orchestration platform with persistent memory and multi-agent collaboration.

## Quick Start

### Available Commands

```bash
# List all available agents
ax list agents

# Run an agent with a task
ax run <agent-name> "your task description"

# Example: Ask the backend agent to create an API
ax run backend "create a REST API for user management"

# Search memory for past conversations
ax memory search "keyword"

# View system status
ax status
```

### Using AutomatosX in Claude Code

You can interact with AutomatosX agents directly in Claude Code using natural language:

**Natural Language Examples**:
```
"Please work with ax agent backend to implement user authentication"
"Ask the ax security agent to audit this code for vulnerabilities"
"Have the ax quality agent write tests for this feature"
"Use ax agent product to design this new feature"
"Work with ax agent devops to set up the deployment pipeline"
```

Claude Code will understand your intent and invoke the appropriate AutomatosX agent for you. Just describe what you need in natural language - no special commands required!

### Available Agents

This project includes the following specialized agents:

- **backend** (Bob) - Backend development (Go/Rust systems)
- **frontend** (Frank) - Frontend development (React/Next.js/Swift)
- **architecture** (Avery) - System architecture and ADR management
- **fullstack** (Felix) - Full-stack development (Node.js/TypeScript)
- **mobile** (Maya) - Mobile development (iOS/Android, Swift/Kotlin/Flutter)
- **devops** (Oliver) - DevOps and infrastructure
- **security** (Steve) - Security auditing and threat modeling
- **data** (Daisy) - Data engineering and ETL
- **quality** (Queenie) - QA and testing
- **design** (Debbee) - UX/UI design
- **writer** (Wendy) - Technical writing
- **product** (Paris) - Product management
- **cto** (Tony) - Technical strategy
- **ceo** (Eric) - Business leadership
- **researcher** (Rodman) - Research and analysis
- **data-scientist** (Dana) - Machine learning and data science
- **aerospace-scientist** (Astrid) - Aerospace engineering and mission design
- **quantum-engineer** (Quinn) - Quantum computing and algorithms
- **creative-marketer** (Candy) - Creative marketing and content strategy
- **standard** (Stan) - Standards and best practices expert

For a complete list with capabilities, run: `ax list agents --format json`

## Key Features

### 1. Persistent Memory

AutomatosX agents remember all previous conversations and decisions:

```bash
# First task - design is saved to memory
ax run product "Design a calculator with add/subtract features"

# Later task - automatically retrieves the design from memory
ax run backend "Implement the calculator API"
```

### 2. Multi-Agent Collaboration

Agents can delegate tasks to each other automatically:

```bash
ax run product "Build a complete user authentication feature"
# → Product agent designs the system
# → Automatically delegates implementation to backend agent
# → Automatically delegates security audit to security agent
```

### 3. Cross-Provider Support

AutomatosX supports multiple AI providers with automatic fallback:
- **Claude** (Anthropic) - Primary provider for Claude Code users
- **Gemini** (Google) - Alternative provider
- **OpenAI** (GPT) - Alternative provider

Configuration is in `automatosx.config.json`.

## Configuration

### Project Configuration

Edit `automatosx.config.json` to customize:

```json
{
  "providers": {
    "claude-code": {
      "enabled": true,
      "priority": 1
    },
    "gemini-cli": {
      "enabled": true,
      "priority": 2
    }
  },
  "execution": {
    "defaultTimeout": 1500000,  // 25 minutes
    "maxRetries": 3
  },
  "memory": {
    "enabled": true,
    "maxEntries": 10000
  }
}
```

### Agent Customization

Create custom agents in `.automatosx/agents/`:

```bash
ax agent create my-agent --template developer --interactive
```

### Workspace Conventions

**IMPORTANT**: AutomatosX uses specific directories for organized file management. Please follow these conventions when working with agents:

- **`automatosx/PRD/`** - Product Requirements Documents, design specs, and planning documents
  - Use for: Architecture designs, feature specs, technical requirements
  - Example: `automatosx/PRD/auth-system-design.md`

- **`automatosx/tmp/`** - Temporary files, scratch work, and intermediate outputs
  - Use for: Draft code, test outputs, temporary analysis
  - Auto-cleaned periodically
  - Example: `automatosx/tmp/draft-api-endpoints.ts`

**Usage in Claude Code**:
```
"Please save the architecture design to automatosx/PRD/user-auth-design.md"
"Put the draft implementation in automatosx/tmp/auth-draft.ts for review"
"Work with ax agent backend to implement the spec in automatosx/PRD/api-spec.md"
```

These directories are automatically created by `ax setup` and included in `.gitignore` appropriately.

## Memory System

### Search Memory

```bash
# Search for past conversations
ax memory search "authentication"
ax memory search "API design"

# List recent memories
ax memory list --limit 10

# Export memory for backup
ax memory export > backup.json
```

### How Memory Works

- **Automatic**: All agent conversations are saved automatically
- **Fast**: SQLite FTS5 full-text search (< 1ms)
- **Local**: 100% private, data never leaves your machine
- **Cost**: $0 (no API calls for memory operations)

## Advanced Usage

### Parallel Execution (v5.6.0+)

Run multiple agents in parallel for faster workflows:

```bash
ax run product "Design authentication system" --parallel
```

### Resumable Runs (v5.3.0+)

For long-running tasks, enable checkpoints:

```bash
ax run backend "Refactor entire codebase" --resumable

# If interrupted, resume with:
ax resume <run-id>

# List all runs
ax runs list
```

### Streaming Output (v5.6.5+)

See real-time output from AI providers:

```bash
ax run backend "Explain this codebase" --streaming
```

### Spec-Driven Development (v5.8.0+)

For complex projects, use spec-driven workflows:

```bash
# Create spec from natural language
ax spec create "Build authentication with database, API, JWT, and tests"

# Or manually define in .specify/tasks.md
ax spec run --parallel

# Check progress
ax spec status
```

## Troubleshooting

### Common Issues

**"Agent not found"**
```bash
# List available agents
ax list agents

# Make sure agent name is correct
ax run backend "task"  # ✓ Correct
ax run Backend "task"  # ✗ Wrong (case-sensitive)
```

**"Provider not available"**
```bash
# Check system status
ax status

# View configuration
ax config show
```

**"Out of memory"**
```bash
# Clear old memories
ax memory clear --before "2024-01-01"

# View memory stats
ax cache stats
```

### Getting Help

```bash
# View command help
ax --help
ax run --help

# Enable debug mode
ax --debug run backend "task"

# Search memory for similar past tasks
ax memory search "similar task"
```

## Best Practices

1. **Use Natural Language in Claude Code**: Let Claude Code coordinate with agents for complex tasks
2. **Leverage Memory**: Reference past decisions and designs
3. **Start Simple**: Test with small tasks before complex workflows
4. **Review Configurations**: Check `automatosx.config.json` for timeouts and retries
5. **Keep Agents Specialized**: Use the right agent for each task type

## Documentation

- **AutomatosX Docs**: https://github.com/defai-digital/automatosx
- **Agent Directory**: `.automatosx/agents/`
- **Configuration**: `automatosx.config.json`
- **Memory Database**: `.automatosx/memory/memories.db`
- **Workspace**: `automatosx/PRD/` (planning docs) and `automatosx/tmp/` (temporary files)

## Support

- Issues: https://github.com/defai-digital/automatosx/issues
- NPM: https://www.npmjs.com/package/@defai.digital/automatosx


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
