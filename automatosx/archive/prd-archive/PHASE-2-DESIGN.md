# Phase 2 Design Document: Embedding Service Infrastructure

**Status:** DESIGN
**Version:** 1.0
**Date:** 2025-11-06
**Owner:** Engineering Team

---

## Overview

Phase 2 implements the **collection management** and **embedding service infrastructure** for AkiDB 2.0. This phase focuses on creating the interfaces and control plane, with a mock embedding implementation. Actual MLX/model integration is deferred to Phase 2+ (production deployment).

---

## Design Principles

1. **Trait-Based Architecture**: Define clear interfaces (`EmbeddingProvider`) that can support multiple backends (MLX, ONNX, etc.)
2. **Testability First**: Provide mock implementations for integration testing without ML dependencies
3. **Incremental Deployment**: Infrastructure completed in Phase 2, model integration in Phase 2+
4. **ARM-Optimized**: Design assumes Apple Silicon/ARM architecture

---

## Component Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    API Layer (Future Phase 3)                │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                  akidb-core (Domain Layer)                   │
│  - CollectionDescriptor (NEW)                                │
│  - CollectionRepository trait (NEW)                          │
│  - EmbeddingConfig (NEW)                                     │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌──────────────────────────┐  ┌────────────────────────────┐
│  akidb-metadata          │  │  akidb-embedding (NEW)     │
│  - SqliteCollection      │  │  - EmbeddingProvider trait │
│    Repository            │  │  - MockEmbeddingProvider   │
│  - collections table     │  │  - BatchRequest/Response   │
└──────────────────────────┘  └────────────────────────────┘
```

---

## 1. Collection Entity (akidb-core)

### 1.1 CollectionDescriptor

```rust
pub struct CollectionDescriptor {
    pub collection_id: CollectionId,
    pub database_id: DatabaseId,
    pub name: String,
    pub dimension: u32,              // 16-4096
    pub metric: DistanceMetric,      // Cosine, Dot, L2
    pub embedding_model: String,     // e.g., "qwen3-embed-8b"
    pub hnsw_m: u32,                 // Graph degree (default: 32)
    pub hnsw_ef_construction: u32,   // Construction EF (default: 200)
    pub max_doc_count: u64,          // Guardrail (default: 50M)
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### 1.2 DistanceMetric Enum

```rust
pub enum DistanceMetric {
    Cosine,   // 1 - cosine similarity
    Dot,      // Negative dot product
    L2,       // Euclidean distance
}
```

### 1.3 CollectionRepository Trait

```rust
#[async_trait]
pub trait CollectionRepository: Send + Sync {
    async fn create(&self, collection: &CollectionDescriptor) -> CoreResult<()>;
    async fn get(&self, collection_id: CollectionId) -> CoreResult<Option<CollectionDescriptor>>;
    async fn list_by_database(&self, database_id: DatabaseId) -> CoreResult<Vec<CollectionDescriptor>>;
    async fn update(&self, collection: &CollectionDescriptor) -> CoreResult<()>;
    async fn delete(&self, collection_id: CollectionId) -> CoreResult<()>;
}
```

---

## 2. SQLite Persistence (akidb-metadata)

### 2.1 Migration: 002_collections_table.sql

```sql
CREATE TABLE IF NOT EXISTS collections (
    collection_id BLOB PRIMARY KEY,
    database_id BLOB NOT NULL REFERENCES databases(database_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    dimension INTEGER NOT NULL CHECK(dimension BETWEEN 16 AND 4096),
    metric TEXT NOT NULL CHECK(metric IN ('cosine','dot','l2')),
    embedding_model TEXT NOT NULL,
    hnsw_m INTEGER NOT NULL DEFAULT 32,
    hnsw_ef_construction INTEGER NOT NULL DEFAULT 200,
    max_doc_count INTEGER NOT NULL DEFAULT 50000000,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE(database_id, name)
) STRICT;

CREATE TRIGGER IF NOT EXISTS trg_collections_updated_at
AFTER UPDATE ON collections
FOR EACH ROW
WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE collections
       SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
     WHERE rowid = NEW.rowid;
END;
```

### 2.2 SqliteCollectionRepository

Implements `CollectionRepository` trait with:
- UUID v7 ↔ BLOB conversion
- DistanceMetric ↔ TEXT mapping
- Error handling (foreign keys, unique constraints)
- Transaction support via `executor` pattern

---

## 3. Embedding Service (akidb-embedding)

### 3.1 EmbeddingProvider Trait

```rust
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Generate embeddings for a batch of text inputs
    async fn embed_batch(&self, request: BatchEmbeddingRequest) -> EmbeddingResult<BatchEmbeddingResponse>;

    /// Get model information (dimension, max tokens)
    async fn model_info(&self) -> EmbeddingResult<ModelInfo>;

    /// Health check
    async fn health_check(&self) -> EmbeddingResult<()>;
}
```

### 3.2 BatchEmbeddingRequest

```rust
pub struct BatchEmbeddingRequest {
    pub model: String,          // "qwen3-embed-8b"
    pub inputs: Vec<String>,    // Text to embed
    pub normalize: bool,        // L2 normalize vectors
}
```

### 3.3 BatchEmbeddingResponse

```rust
pub struct BatchEmbeddingResponse {
    pub model: String,
    pub embeddings: Vec<Vec<f32>>,  // [batch_size][dimension]
    pub usage: Usage,
}

pub struct Usage {
    pub total_tokens: usize,
    pub duration_ms: u64,
}
```

### 3.4 MockEmbeddingProvider

For testing without ML dependencies:
- Returns deterministic embeddings based on input hash
- Configurable dimension (default: 512)
- Simulates realistic latency (10-50ms)
- Useful for integration tests

```rust
pub struct MockEmbeddingProvider {
    dimension: u32,
    latency_ms: u64,
}

impl MockEmbeddingProvider {
    pub fn new(dimension: u32) -> Self {
        Self { dimension, latency_ms: 20 }
    }

    // Generate deterministic embeddings for testing
    fn generate_embedding(&self, text: &str) -> Vec<f32> {
        // Hash input → deterministic vector
        // L2 normalize if requested
    }
}
```

---

## 4. Integration Testing Strategy

### 4.1 Collection CRUD Tests

Test scenarios:
- ✅ Create collection under database
- ✅ Get collection by ID
- ✅ List collections by database
- ✅ Update collection parameters (hnsw_m, embedding_model)
- ✅ Delete collection
- ✅ Cascade delete (database → collections)
- ✅ Unique constraint (database + name)
- ✅ Foreign key constraint (invalid database_id)
- ✅ Dimension validation (16-4096 range)

### 4.2 Embedding Service Tests

Test scenarios:
- ✅ Batch embedding generation (mock provider)
- ✅ Model info retrieval
- ✅ Health check
- ✅ Error handling (empty input, invalid model)
- ✅ Normalize flag behavior
- ✅ Deterministic output (same input → same embedding)

---

## 5. Future Work (Phase 2+)

### 5.1 MLX Integration

**Dependencies:**
```toml
[dependencies]
mlx-rs = "0.1"  # Rust bindings for MLX
```

**Implementation Steps:**
1. Download Qwen3-Embedding-8B model (Hugging Face)
2. Implement `MlxEmbeddingProvider` using mlx-rs
3. Add model caching and warm-up
4. Benchmark on Apple Silicon (M2 Max)
5. Implement blue/green model switching

**Reference:**
- MLX Framework: https://github.com/ml-explore/mlx
- mlx-rs Bindings: https://github.com/oxideai/mlx-rs

### 5.2 HNSW Vector Index

**Dependencies:**
```toml
[dependencies]
hnsw = "0.11"  # Or custom implementation
```

**Future Phases:**
- Phase 3: Document ingestion (vectors + metadata)
- Phase 4: HNSW index construction and search
- Phase 5: Memory-mapped storage for vectors

---

## 6. Non-Functional Requirements

### 6.1 Performance Targets

| Operation | Target | Notes |
|-----------|--------|-------|
| Collection create | P95 < 5ms | SQLite insert |
| Collection get | P95 < 1ms | Indexed query |
| Collection list | P95 < 5ms | 100 collections |
| Batch embed (mock) | P95 < 50ms | Simulated latency |
| Batch embed (MLX) | P95 < 200ms | 32 inputs, 512-dim |

### 6.2 Resource Constraints

- Memory: Collection metadata < 10 MB (10K collections)
- Storage: SQLite db < 100 MB (metadata only)
- Concurrency: Support 100 concurrent CRUD operations

---

## 7. Observability

### 7.1 Metrics (Future)

**Collection Metrics:**
- `akidb_collections_total{database_id}` - Total collection count
- `akidb_collection_operations_total{operation, status}` - CRUD counters
- `akidb_collection_operation_duration_seconds{operation}` - Latency histogram

**Embedding Metrics:**
- `akidb_embedding_requests_total{model, status}` - Request counter
- `akidb_embedding_tokens_total{model}` - Token usage
- `akidb_embedding_duration_seconds{model}` - Latency histogram
- `akidb_embedding_batch_size{model}` - Batch size histogram

### 7.2 Logging

Use `tracing` with structured fields:
- Collection CRUD: `collection_id`, `database_id`, `operation`
- Embedding: `model`, `batch_size`, `duration_ms`

---

## 8. Migration Path from Phase 1

### 8.1 Database Changes

**New Table:** `collections`
**Migration:** `002_collections_table.sql`
**Backward Compatible:** Yes (no breaking changes to existing tables)

### 8.2 Code Changes

**akidb-core:**
- Add: `collection.rs` (CollectionDescriptor, DistanceMetric)
- Update: `traits.rs` (add CollectionRepository)
- Update: `lib.rs` (export new types)

**akidb-metadata:**
- Add: `collection_repository.rs` (SqliteCollectionRepository)
- Add: `migrations/002_collections_table.sql`
- Update: `lib.rs` (export repository)

**New Crate: akidb-embedding**
- Add: `lib.rs`, `provider.rs`, `mock.rs`, `types.rs`
- Add: `Cargo.toml`

---

## 9. Testing Checklist

### Phase 2 Exit Criteria

- ✅ Collection CRUD operations (10+ tests)
- ✅ Embedding provider interface (5+ tests)
- ✅ SQLite migration applies cleanly
- ✅ Clippy passes with -D warnings
- ✅ cargo fmt compliance
- ✅ Documentation complete (cargo doc)
- ✅ Integration tests pass (100%)
- ✅ CLAUDE.md updated with Phase 2 info

---

## 10. Timeline

**Estimated Effort:** 1 day (accelerated implementation)

**Tasks:**
1. Add Collection domain model (30 min)
2. Add CollectionRepository trait (15 min)
3. Create SQLite migration (15 min)
4. Implement SqliteCollectionRepository (1 hour)
5. Create akidb-embedding crate (30 min)
6. Implement MockEmbeddingProvider (30 min)
7. Write integration tests (1.5 hours)
8. Documentation and validation (1 hour)

---

**Document Version:** 1.0
**Last Updated:** 2025-11-06
**Status:** APPROVED for Implementation
**Prepared by:** Claude Code (AI Assistant)

---

**END OF PHASE 2 DESIGN DOCUMENT**
