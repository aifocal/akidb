# Phase 1 Implementation Plan: Foundation

**Status:** 🔄 IN PROGRESS
**Start Date:** 2025-11-06
**Target Completion:** 2025-12-20 (M1 Milestone)
**Duration:** 4 weeks
**Budget:** $127,587 (25% of $510,345)

---

## Executive Summary

Phase 1 (Foundation) implements the core metadata layer for AkiDB 2.0, migrating from v1.x's in-memory tenant management to a SQLite-backed persistence layer with the new `DatabaseDescriptor` entity. This phase delivers the M1 milestone which unblocks all subsequent phases.

**Key Deliverables:**
- ✅ akidb-core crate with domain models and traits
- ✅ akidb-metadata crate with SQLite persistence
- ✅ Migration tool (v1.x → v2.0)
- ✅ Integration test suite
- ✅ M1 milestone validation

---

## Implementation Checklist

### 1. Workspace Setup ✅

**Status:** COMPLETE
**Files Created:**
- [x] `Cargo.toml` - Workspace configuration
- [x] `crates/` directory structure
- [x] `.gitignore` - Rust/SQLite exclusions

**Validation:**
```bash
cargo --version  # Verify Rust 1.75+
tree crates/     # Verify structure
```

---

### 2. akidb-core Crate (Domain Layer)

**Status:** IN PROGRESS (Backend Agent)
**Location:** `crates/akidb-core/`

#### 2.1 Domain Models

**ID Types** (crates/akidb-core/src/ids.rs):
- [x] `TenantId` - UUID v7 wrapper with serde, display
- [x] `DatabaseId` - UUID v7 wrapper
- [x] `CollectionId` - UUID v7 wrapper
- [x] `UserId` - UUID v7 wrapper

**Tenant Domain** (crates/akidb-core/src/tenant.rs):
- [x] `TenantStatus` enum:
  - `Provisioning` - Initial state
  - `Active` - Ready for operations
  - `Suspended` - Temporarily disabled
  - `Decommissioned` - Soft deleted
- [x] `TenantQuota` struct:
  - `memory_quota_bytes: u64` (default: 32 GiB)
  - `storage_quota_bytes: u64` (default: 1 TiB)
  - `qps_quota: u32` (default: 200)
- [x] `TenantDescriptor` struct:
  - `tenant_id: TenantId`
  - `external_id: Option<String>`
  - `name: String`
  - `slug: String`
  - `status: TenantStatus`
  - `quotas: TenantQuota`
  - `metadata: Option<serde_json::Value>`
  - `created_at: chrono::DateTime<Utc>`
  - `updated_at: chrono::DateTime<Utc>`

**Database Domain** (crates/akidb-core/src/database.rs):
- [ ] `DatabaseState` enum:
  - `Provisioning` - Creating database
  - `Ready` - Available for collections
  - `Migrating` - Schema migration in progress
  - `Deleting` - Cleanup in progress
- [ ] `DatabaseDescriptor` struct (NEW in v2.0):
  - `database_id: DatabaseId`
  - `tenant_id: TenantId`
  - `name: String`
  - `description: Option<String>`
  - `state: DatabaseState`
  - `schema_version: i32` (default: 1)
  - `created_at: chrono::DateTime<Utc>`
  - `updated_at: chrono::DateTime<Utc>`

#### 2.2 Repository Traits

**Tenant Repository** (crates/akidb-core/src/traits.rs):
- [ ] `TenantCatalog` trait:
```rust
#[async_trait]
pub trait TenantCatalog: Send + Sync {
    async fn create(&self, tenant: TenantDescriptor) -> Result<TenantDescriptor, CoreError>;
    async fn get(&self, tenant_id: &TenantId) -> Result<Option<TenantDescriptor>, CoreError>;
    async fn get_by_slug(&self, slug: &str) -> Result<Option<TenantDescriptor>, CoreError>;
    async fn list(&self, limit: usize, offset: usize) -> Result<Vec<TenantDescriptor>, CoreError>;
    async fn update(&self, tenant: TenantDescriptor) -> Result<TenantDescriptor, CoreError>;
    async fn delete(&self, tenant_id: &TenantId) -> Result<(), CoreError>;
}
```

**Database Repository** (crates/akidb-core/src/traits.rs):
- [ ] `DatabaseRepository` trait:
```rust
#[async_trait]
pub trait DatabaseRepository: Send + Sync {
    async fn create(&self, database: DatabaseDescriptor) -> Result<DatabaseDescriptor, CoreError>;
    async fn get(&self, database_id: &DatabaseId) -> Result<Option<DatabaseDescriptor>, CoreError>;
    async fn list_by_tenant(&self, tenant_id: &TenantId) -> Result<Vec<DatabaseDescriptor>, CoreError>;
    async fn update(&self, database: DatabaseDescriptor) -> Result<DatabaseDescriptor, CoreError>;
    async fn delete(&self, database_id: &DatabaseId) -> Result<(), CoreError>;
}
```

#### 2.3 Error Types

**Core Errors** (crates/akidb-core/src/error.rs):
- [ ] `CoreError` enum:
```rust
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("Entity not found: {entity_type} with ID {id}")]
    NotFound { entity_type: String, id: String },

    #[error("Entity already exists: {entity_type} with {field}={value}")]
    AlreadyExists { entity_type: String, field: String, value: String },

    #[error("Quota exceeded: {quota_type} limit {limit} exceeded")]
    QuotaExceeded { quota_type: String, limit: u64 },

    #[error("Invalid state transition: {entity} cannot go from {from} to {to}")]
    InvalidState { entity: String, from: String, to: String },

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}
```

#### 2.4 Cargo.toml

**Dependencies:**
```toml
[package]
name = "akidb-core"
version.workspace = true
edition.workspace = true

[dependencies]
# Async
tokio = { workspace = true }
async-trait = "0.1"

# Serialization
serde = { workspace = true }
serde_json = { workspace = true }

# UUIDs
uuid = { workspace = true }

# Time
chrono = { workspace = true }

# Error handling
thiserror = { workspace = true }
anyhow = { workspace = true }
```

**Validation:**
```bash
cd crates/akidb-core
cargo build
cargo test
cargo doc --no-deps --open
```

---

### 3. akidb-metadata Crate (Persistence Layer)

**Status:** IN PROGRESS (Backend Agent)
**Location:** `crates/akidb-metadata/`

#### 3.1 SQL Migrations

**Migration 001** (crates/akidb-metadata/migrations/001_initial_schema.sql):
- [ ] Create `tenants` table:
```sql
CREATE TABLE IF NOT EXISTS tenants (
    tenant_id BLOB PRIMARY KEY,
    external_id TEXT UNIQUE NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL CHECK(status IN ('provisioning','active','suspended','decommissioned')),
    memory_quota_bytes INTEGER NOT NULL DEFAULT 34359738368,  -- 32 GiB
    storage_quota_bytes INTEGER NOT NULL DEFAULT 1099511627776,  -- 1 TiB
    qps_quota INTEGER NOT NULL DEFAULT 200,
    metadata TEXT,  -- JSON blob
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;
```

- [ ] Create `users` table:
```sql
CREATE TABLE IF NOT EXISTS users (
    user_id BLOB PRIMARY KEY,
    tenant_id BLOB NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    password_hash BLOB NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending','active','locked','revoked')),
    last_login_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE UNIQUE INDEX ux_users_tenant_email ON users(tenant_id, email);
```

- [ ] Create `databases` table:
```sql
CREATE TABLE IF NOT EXISTS databases (
    database_id BLOB PRIMARY KEY,
    tenant_id BLOB NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,
    state TEXT NOT NULL CHECK(state IN ('provisioning','ready','migrating','deleting')),
    schema_version INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE UNIQUE INDEX ux_databases_tenant_name ON databases(tenant_id, name);
```

- [ ] Create UPDATE triggers:
```sql
-- Trigger for tenants
CREATE TRIGGER IF NOT EXISTS update_tenants_timestamp
AFTER UPDATE ON tenants
BEGIN
    UPDATE tenants SET updated_at = datetime('now') WHERE tenant_id = NEW.tenant_id;
END;

-- Trigger for users
CREATE TRIGGER IF NOT EXISTS update_users_timestamp
AFTER UPDATE ON users
BEGIN
    UPDATE users SET updated_at = datetime('now') WHERE user_id = NEW.user_id;
END;

-- Trigger for databases
CREATE TRIGGER IF NOT EXISTS update_databases_timestamp
AFTER UPDATE ON databases
BEGIN
    UPDATE databases SET updated_at = datetime('now') WHERE database_id = NEW.database_id;
END;
```

**Migration 002** (crates/akidb-metadata/migrations/002_collections_table.sql):
- [ ] Create `collections` table (prepared for Phase 2):
```sql
CREATE TABLE IF NOT EXISTS collections (
    collection_id BLOB PRIMARY KEY,
    database_id BLOB NOT NULL REFERENCES databases(database_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    dimension INTEGER NOT NULL CHECK(dimension BETWEEN 16 AND 4096),
    metric TEXT NOT NULL CHECK(metric IN ('cosine','dot','l2')),
    tiering_policy TEXT NOT NULL CHECK(tiering_policy IN ('memory','memory_s3','s3_only')),
    replica_factor INTEGER NOT NULL DEFAULT 1 CHECK(replica_factor BETWEEN 1 AND 3),
    hnsw_m INTEGER NOT NULL DEFAULT 32,
    hnsw_ef_construction INTEGER NOT NULL DEFAULT 200,
    wal_retention_seconds INTEGER NOT NULL DEFAULT 604800,  -- 7 days
    max_doc_count INTEGER NOT NULL DEFAULT 50000000,
    embedding_model TEXT NOT NULL,
    sync_policy TEXT NOT NULL CHECK(sync_policy IN ('continuous','scheduled','manual')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE UNIQUE INDEX ux_collections_db_name ON collections(database_id, name);
```

#### 3.2 Repository Implementations

**Tenant Catalog** (crates/akidb-metadata/src/tenant_catalog.rs):
- [ ] `SqliteTenantCatalog` struct
- [ ] Implement `TenantCatalog` trait
- [ ] UUID v7 ↔ BLOB conversion helpers
- [ ] Error mapping (sqlx → CoreError)

**Database Repository** (crates/akidb-metadata/src/database_repository.rs):
- [ ] `SqliteDatabaseRepository` struct
- [ ] Implement `DatabaseRepository` trait
- [ ] Foreign key validation (tenant_id exists)
- [ ] Cascade delete verification

#### 3.3 Connection Pool

**Pool Manager** (crates/akidb-metadata/src/pool.rs):
- [ ] Create `MetadataPool` wrapper around `sqlx::Pool<Sqlite>`
- [ ] WAL mode pragma: `PRAGMA journal_mode=WAL`
- [ ] Foreign keys enabled: `PRAGMA foreign_keys=ON`
- [ ] Connection pooling (min: 2, max: 10)

#### 3.4 Cargo.toml

**Dependencies:**
```toml
[package]
name = "akidb-metadata"
version.workspace = true
edition.workspace = true

[dependencies]
akidb-core = { path = "../akidb-core" }

# Database
sqlx = { workspace = true }

# Async
tokio = { workspace = true }
async-trait = "0.1"

# Serialization
serde = { workspace = true }
serde_json = { workspace = true }

# UUIDs
uuid = { workspace = true }

# Time
chrono = { workspace = true }

# Error handling
thiserror = { workspace = true }
anyhow = { workspace = true }
```

**Validation:**
```bash
cd crates/akidb-metadata
cargo sqlx database create
cargo sqlx migrate run
cargo build
cargo test
```

---

### 4. Migration Tool (v1.x → v2.0)

**Status:** PENDING
**Location:** `crates/akidb-cli/src/commands/migrate.rs`

#### 4.1 Migration Command

**CLI Interface:**
```bash
akidb migrate v1-to-v2 \
  --v1-data-dir /Users/akiralam/code/akidb/data \
  --v2-database /path/to/metadata.db \
  --dry-run  # Optional: preview changes without commit
```

**Implementation Tasks:**
- [ ] Parse v1.x tenant JSON files
- [ ] Map v1.x `TenantDescriptor` → v2.0 schema
- [ ] Insert tenants into SQLite with transaction
- [ ] Create default database for each tenant
- [ ] Validate foreign key integrity
- [ ] Rollback on any error

#### 4.2 Data Mapping

**v1.x → v2.0 Field Mapping:**

| v1.x Field | v2.0 Table.Column | Notes |
|------------|-------------------|-------|
| `tenant_id` | `tenants.tenant_id` | UUID v7 (binary) |
| `name` | `tenants.name` | Direct copy |
| `slug` | `tenants.slug` | Validate uniqueness |
| `status` | `tenants.status` | Enum string |
| `quotas.max_storage_bytes` | `tenants.storage_quota_bytes` | Default: 100 GB |
| `quotas.max_collections` | Ignored | Moved to database-level in v2.0 |
| `quotas.max_vectors_per_collection` | `collections.max_doc_count` | Phase 2 |
| `quotas.api_rate_limit_per_second` | `tenants.qps_quota` | Direct copy |
| `metadata` | `tenants.metadata` | JSON string |
| `created_at` | `tenants.created_at` | ISO-8601 string |
| `api_key_hash` | `users.password_hash` | Create admin user |

**New v2.0 Fields (Generated):**
- `tenants.external_id` → NULL (not present in v1.x)
- `tenants.memory_quota_bytes` → 32 GiB (default)
- `tenants.updated_at` → Same as `created_at` initially

**Default Database Creation:**
- For each migrated tenant, create a default database:
  - `database_id` → New UUID v7
  - `tenant_id` → From migrated tenant
  - `name` → "default"
  - `description` → "Auto-created during v1.x migration"
  - `state` → "ready"
  - `schema_version` → 1

#### 4.3 Rollback Capability

**Rollback Strategy:**
- [ ] Use SQLite transaction for all writes
- [ ] On error: `ROLLBACK TRANSACTION`
- [ ] On success: `COMMIT TRANSACTION`
- [ ] Log all changes to `migration.log`

**Validation Checks:**
- [ ] Tenant count matches (v1.x == v2.0)
- [ ] All tenant IDs preserved
- [ ] All slugs unique
- [ ] Default database created for each tenant
- [ ] Foreign keys valid

---

### 5. Integration Tests

**Status:** PENDING
**Location:** `crates/akidb-metadata/tests/integration_test.rs`

#### 5.1 Test Scenarios

**Tenant Lifecycle Tests:**
- [ ] `test_create_tenant_success()` - Happy path
- [ ] `test_create_tenant_duplicate_slug()` - Unique constraint violation
- [ ] `test_get_tenant_by_id()` - Retrieve existing tenant
- [ ] `test_get_tenant_not_found()` - Return None
- [ ] `test_get_tenant_by_slug()` - Query by slug
- [ ] `test_list_tenants_pagination()` - Limit/offset
- [ ] `test_update_tenant_status()` - State transitions
- [ ] `test_delete_tenant()` - Soft delete

**Database Lifecycle Tests:**
- [ ] `test_create_database_success()` - Happy path
- [ ] `test_create_database_foreign_key_violation()` - Invalid tenant_id
- [ ] `test_create_database_duplicate_name()` - Unique constraint (tenant + name)
- [ ] `test_list_databases_by_tenant()` - Filter by tenant
- [ ] `test_update_database_state()` - State transitions
- [ ] `test_delete_database()` - DELETE operation

**Cascade Delete Tests:**
- [ ] `test_delete_tenant_cascades_to_databases()` - FK ON DELETE CASCADE
- [ ] `test_delete_database_cascades_to_collections()` - Phase 2 prep

**Quota Validation Tests:**
- [ ] `test_enforce_memory_quota()` - Validate against quota
- [ ] `test_enforce_storage_quota()` - Validate against quota
- [ ] `test_enforce_qps_quota()` - Rate limiting (Phase 2)

**Timestamp Tests:**
- [ ] `test_created_at_immutable()` - Never changes
- [ ] `test_updated_at_auto_update()` - Trigger fires on UPDATE

#### 5.2 Test Fixtures

**Helper Functions:**
```rust
async fn create_test_pool() -> sqlx::Pool<Sqlite> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(":memory:")
        .await
        .unwrap();

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .unwrap();

    pool
}

fn create_sample_tenant() -> TenantDescriptor {
    TenantDescriptor {
        tenant_id: TenantId::new_v7(),
        external_id: None,
        name: "Test Tenant".to_string(),
        slug: "test-tenant".to_string(),
        status: TenantStatus::Active,
        quotas: TenantQuota {
            memory_quota_bytes: 10 * 1024 * 1024 * 1024,  // 10 GB
            storage_quota_bytes: 100 * 1024 * 1024 * 1024,  // 100 GB
            qps_quota: 100,
        },
        metadata: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn create_sample_database(tenant_id: TenantId) -> DatabaseDescriptor {
    DatabaseDescriptor {
        database_id: DatabaseId::new_v7(),
        tenant_id,
        name: "test-db".to_string(),
        description: Some("Test database".to_string()),
        state: DatabaseState::Ready,
        schema_version: 1,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}
```

#### 5.3 Test Execution

**Commands:**
```bash
# Run all integration tests
cargo test --workspace --test integration_test

# Run specific test
cargo test test_create_tenant_success

# Run with output
cargo test -- --nocapture

# Run with coverage (requires cargo-tarpaulin)
cargo tarpaulin --out Html --output-dir coverage/
```

**Coverage Target:** 80% minimum for Phase 1

---

### 6. M1 Milestone Validation

**Status:** PENDING
**Validation Date:** TBD (after all components complete)

#### 6.1 Exit Criteria

**Functional Requirements:**
- [ ] Metadata database operational on macOS ARM
- [ ] v1.x tenants migrated to SQLite successfully (zero data loss)
- [ ] Integration tests passing (100% pass rate)
- [ ] No critical blockers or regressions
- [ ] Rollback script validated (can revert to v1.x)

**Quality Requirements:**
- [ ] Code coverage ≥ 80%
- [ ] Zero compiler warnings (`cargo clippy --all-targets`)
- [ ] Documentation complete (`cargo doc --no-deps`)
- [ ] Performance: Tenant CRUD operations < 5ms P95

#### 6.2 Validation Tests

**Build Validation:**
```bash
cargo build --workspace --release
cargo test --workspace
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt -- --check
```

**Migration Validation:**
```bash
# Migrate sample v1.x data
akidb migrate v1-to-v2 --v1-data-dir test-data/ --v2-database test.db

# Verify tenant count
sqlite3 test.db "SELECT COUNT(*) FROM tenants;"

# Verify databases created
sqlite3 test.db "SELECT COUNT(*) FROM databases WHERE name='default';"

# Verify foreign keys
sqlite3 test.db "PRAGMA foreign_key_check;"
```

**Performance Validation:**
```bash
# Benchmark tenant CRUD
cargo bench --bench tenant_crud

# Expected results:
# - Create tenant: P95 < 3ms
# - Get tenant by ID: P95 < 1ms
# - List tenants (100 results): P95 < 5ms
```

#### 6.3 Go/No-Go Decision

**Decision Makers:** Engineering Director, CTO, Product Lead
**Decision Date:** TBD
**Decision Criteria:**

| Criteria | Weight | Pass Threshold | Status |
|----------|--------|----------------|--------|
| All tests passing | 30% | 100% | TBD |
| Code coverage | 20% | ≥ 80% | TBD |
| Migration success | 25% | Zero data loss | TBD |
| Performance | 15% | P95 < 5ms | TBD |
| Documentation | 10% | Complete | TBD |

**Outcomes:**
- **GO:** Proceed to Phase 2 (Embeddings)
- **NO-GO:** Fix critical issues, re-validate

---

## Phase 1 Timeline

### Week 1 (Nov 25 - Dec 1)
- **Day 1-2:** akidb-core domain models complete
- **Day 3-4:** akidb-core traits and errors complete
- **Day 5:** Code review and adjustments

### Week 2 (Dec 2 - Dec 8)
- **Day 1-2:** SQL migrations written and tested
- **Day 3-4:** SqliteTenantCatalog implementation
- **Day 5:** SqliteDatabaseRepository implementation

### Week 3 (Dec 9 - Dec 15)
- **Day 1-2:** Migration tool implementation
- **Day 3-4:** Integration tests (tenant lifecycle)
- **Day 5:** Integration tests (database lifecycle)

### Week 4 (Dec 16 - Dec 20)
- **Day 1-2:** Integration tests (cascade, quotas, timestamps)
- **Day 3:** M1 validation testing
- **Day 4:** Performance benchmarking
- **Day 5:** M1 milestone review and Go/No-Go decision

---

## Risk Register

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| SQLite performance issues on ARM | Low | High | Benchmark early (Week 1), fallback to RocksDB |
| UUID v7 library incompatibility | Low | Medium | Test with sample data, fallback to UUID v4 |
| v1.x data migration edge cases | Medium | High | Comprehensive test suite, manual validation |
| Integration test flakiness | Medium | Low | Use deterministic test data, retry failed tests |
| Team PTO during December holidays | High | Medium | Front-load critical work to Weeks 1-2 |

---

## Deliverables Summary

**Code Artifacts:**
- `crates/akidb-core/` - Domain models and traits (100% complete)
- `crates/akidb-metadata/` - SQLite persistence layer (100% complete)
- `crates/akidb-cli/` - Migration tool (100% complete)
- `crates/akidb-metadata/tests/` - Integration test suite (100% complete)

**Documentation:**
- API documentation (`cargo doc`)
- Migration guide (v1.x → v2.0)
- M1 validation report

**Database:**
- SQLite schema v1 (STRICT tables, WAL mode)
- Migration scripts (001, 002)
- Test fixtures

---

## Success Metrics

**Quantitative:**
- Code coverage: ≥ 80%
- Test pass rate: 100%
- Migration success rate: 100% (zero data loss)
- Performance: Tenant CRUD P95 < 5ms
- Build time: < 2 minutes (release mode)

**Qualitative:**
- Code review approval (all PRs)
- Architecture compliance (ADR-001 SQLite)
- Developer ergonomics (trait-based design)
- Backward compatibility (v1.x migration works)

---

## Next Phase Preview

**Phase 2: Embeddings (Weeks 5-8, Dec 23 - Jan 17)**
- MLX embedding service integration
- Qwen3-Embedding-8B model loading
- Batch embedding API
- Collection-level embedding configuration

**Blocked Until M1 Complete:**
- Cannot implement collection CRUD without database entity
- Cannot test embedding service without tenant/database hierarchy
- Cannot implement RBAC without user authentication (Phase 3)

---

**Document Version:** 1.0
**Last Updated:** 2025-11-06
**Next Review:** 2025-11-15 (M1 Go/No-Go meeting)
**Owner:** Engineering Director
**Prepared by:** Claude Code (AI Assistant) + Backend Agent

---

**END OF PHASE 1 IMPLEMENTATION PLAN**
