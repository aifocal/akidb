# AkiDB 2.0 Technical Architecture

_Last synced with `automatosx/PRD/akidb-2.0-improved-prd.md` (v1.0, 2024-05-08). Maintainer: Avery (Architecture Guild)._

This document translates the product goals into implementable architectural guidance for AkiDB 2.0. It owns the authoritative technical decisions and must stay consistent with the ADR register in `.automatosx/abilities/our-architecture-decisions.md`.

## 0. Scope & Design Principles
- Optimized for ARM edge hardware (Apple Silicon, NVIDIA Jetson, Oracle ARM) while remaining portable to x86 for CI.
- Metadata and control-plane state are persisted in SQLite FTS5 (ADR-001) per node; vector payloads live in memory-mapped segment files orchestrated by the storage layer.
- Favors modular crates with explicit traits to keep the HNSW engine, storage, and embedding services independently testable.
- All multi-tenant guarantees—security, quotas, observability—are enforced at every layer (API → core services → storage) to prevent privilege escalation.

## 1. Metadata & Control Plane Schema
### 1.1 Entities Overview
| Entity | Table | Purpose |
|--------|-------|---------|
| Tenant | `tenants` | Provisioning, quotas, billing metadata |
| User | `users` | Authenticated actors scoped to a tenant |
| Database | `databases` | Logical namespace under a tenant |
| Collection | `collections` | Vector index + metadata config |
| Document | `docs` | Logical records with vector + payload pointers |
| Snapshot | `snapshots` | Durable point-in-time collection images |
| WAL Segment | `wal_segments` | Write-ahead log chunks awaiting or after replay |

### 1.2 Table Definitions
All tables are defined as `STRICT` tables in SQLite. Timestamps use UTC ISO-8601 strings. Numeric quotas are stored in bytes to avoid unit ambiguity.

#### tenants
| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `tenant_id` | `BLOB` | `PRIMARY KEY` | UUID v7, binary representation |
| `external_id` | `TEXT` | `UNIQUE NULL` | Optional customer-provided identifier |
| `name` | `TEXT` | `NOT NULL` | Display name |
| `slug` | `TEXT` | `NOT NULL UNIQUE` | URL-safe identifier |
| `status` | `TEXT` | `CHECK(status IN ('provisioning','active','suspended','decommissioned'))` | Drives quota enforcement |
| `memory_quota_bytes` | `INTEGER` | `NOT NULL DEFAULT 34359738368` | Default 32 GiB |
| `storage_quota_bytes` | `INTEGER` | `NOT NULL DEFAULT 1099511627776` | Default 1 TiB |
| `qps_quota` | `INTEGER` | `NOT NULL DEFAULT 200` | Aggregate read/write limit |
| `metadata` | `TEXT` |  | JSON blob for custom attributes |
| `created_at` | `TEXT` | `NOT NULL` | UTC timestamp |
| `updated_at` | `TEXT` | `NOT NULL` | Maintained by triggers |

#### users
| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `user_id` | `BLOB` | `PRIMARY KEY` | UUID v7 |
| `tenant_id` | `BLOB` | `NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE` | Multi-tenant scoping |
| `email` | `TEXT` | `NOT NULL` | Normalized lowercase |
| `password_hash` | `BLOB` | `NOT NULL` | Argon2id hash |
| `status` | `TEXT` | `CHECK(status IN ('pending','active','locked','revoked'))` | |
| `last_login_at` | `TEXT` |  | Nullable |
| `created_at` | `TEXT` | `NOT NULL` | |
| `updated_at` | `TEXT` | `NOT NULL` | |
Unique index: `CREATE UNIQUE INDEX ux_users_tenant_email ON users(tenant_id, email);`

#### databases
| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `database_id` | `BLOB` | `PRIMARY KEY` | UUID v7 |
| `tenant_id` | `BLOB` | `NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE` | |
| `name` | `TEXT` | `NOT NULL` | |
| `description` | `TEXT` |  | |
| `state` | `TEXT` | `CHECK(state IN ('provisioning','ready','migrating','deleting'))` | |
| `schema_version` | `INTEGER` | `NOT NULL DEFAULT 1` | For collection evolution |
| `created_at` | `TEXT` | `NOT NULL` | |
| `updated_at` | `TEXT` | `NOT NULL` | |
Unique index: `CREATE UNIQUE INDEX ux_databases_tenant_name ON databases(tenant_id, name);`

#### collections
| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `collection_id` | `BLOB` | `PRIMARY KEY` | UUID v7 |
| `database_id` | `BLOB` | `NOT NULL REFERENCES databases(database_id) ON DELETE CASCADE` | |
| `name` | `TEXT` | `NOT NULL` | |
| `dimension` | `INTEGER` | `NOT NULL CHECK(dimension BETWEEN 16 AND 4096)` | |
| `metric` | `TEXT` | `NOT NULL CHECK(metric IN ('cosine','dot','l2'))` | |
| `tiering_policy` | `TEXT` | `NOT NULL CHECK(tiering_policy IN ('memory','memory_s3','s3_only'))` | Aligns with PRD tiering |
| `replica_factor` | `INTEGER` | `NOT NULL DEFAULT 1 CHECK(replica_factor BETWEEN 1 AND 3)` | Logical replicas |
| `hnsw_m` | `INTEGER` | `NOT NULL DEFAULT 32` | Default graph degree |
| `hnsw_ef_construction` | `INTEGER` | `NOT NULL DEFAULT 200` | |
| `wal_retention_seconds` | `INTEGER` | `NOT NULL DEFAULT 604800` | 7 days |
| `max_doc_count` | `INTEGER` | `NOT NULL DEFAULT 50000000` | Guardrail |
| `embedding_model` | `TEXT` | `NOT NULL` | Current default model ID |
| `sync_policy` | `TEXT` | `NOT NULL CHECK(sync_policy IN ('continuous','scheduled','manual'))` | Governs S3 sync |
| `created_at` | `TEXT` | `NOT NULL` | |
| `updated_at` | `TEXT` | `NOT NULL` | |
Unique index: `CREATE UNIQUE INDEX ux_collections_db_name ON collections(database_id, name);`

#### docs
| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `doc_id` | `BLOB` | `PRIMARY KEY` | UUID v7 |
| `collection_id` | `BLOB` | `NOT NULL REFERENCES collections(collection_id) ON DELETE CASCADE` | |
| `external_id` | `TEXT` | `NOT NULL` | Caller-provided identifier |
| `version` | `INTEGER` | `NOT NULL DEFAULT 1` | Bumps on update |
| `vector_checksum` | `BLOB` | `NOT NULL` | 128-bit xxHash to detect drift |
| `vector_offset` | `INTEGER` | `NOT NULL` | Byte offset inside segment file |
| `vector_length` | `INTEGER` | `NOT NULL` | Bytes |
| `payload` | `TEXT` |  | JSON metadata |
| `embedding_model` | `TEXT` | `NOT NULL` | Snapshot of model used |
| `tags` | `TEXT` |  | CSV or JSON array |
| `created_at` | `TEXT` | `NOT NULL` | |
| `updated_at` | `TEXT` | `NOT NULL` | |
| `deleted_at` | `TEXT` |  | Soft delete |
Indexes:
- `CREATE UNIQUE INDEX ux_docs_collection_external ON docs(collection_id, external_id) WHERE deleted_at IS NULL;`
- `CREATE INDEX ix_docs_collection_deleted ON docs(collection_id, deleted_at);`

#### snapshots
| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `snapshot_id` | `BLOB` | `PRIMARY KEY` | UUID v7 |
| `collection_id` | `BLOB` | `NOT NULL REFERENCES collections(collection_id) ON DELETE CASCADE` | |
| `generation` | `INTEGER` | `NOT NULL` | Monotonic counter per collection |
| `format_version` | `INTEGER` | `NOT NULL DEFAULT 2` | Snapshot schema version |
| `base_wal_sequence` | `INTEGER` | `NOT NULL` | LSN included in snapshot |
| `last_wal_sequence` | `INTEGER` | `NOT NULL` | Highest LSN covered |
| `storage_uri` | `TEXT` | `NOT NULL` | Local path or S3 URI |
| `size_bytes` | `INTEGER` | `NOT NULL` | |
| `checksum` | `BLOB` | `NOT NULL` | SHA-256 |
| `created_at` | `TEXT` | `NOT NULL` | |
| `expires_at` | `TEXT` |  | Optional TTL |
Unique index: `CREATE UNIQUE INDEX ux_snapshots_collection_generation ON snapshots(collection_id, generation);`

#### wal_segments
| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `segment_id` | `BLOB` | `PRIMARY KEY` | UUID v7 |
| `collection_id` | `BLOB` | `NOT NULL REFERENCES collections(collection_id) ON DELETE CASCADE` | |
| `start_sequence` | `INTEGER` | `NOT NULL` | Inclusive LSN |
| `end_sequence` | `INTEGER` | `NOT NULL` | Exclusive LSN |
| `min_timestamp` | `TEXT` | `NOT NULL` | |
| `max_timestamp` | `TEXT` | `NOT NULL` | |
| `file_path` | `TEXT` | `NOT NULL` | Local WAL file |
| `sealed_at` | `TEXT` |  | Populated when upload complete |
| `size_bytes` | `INTEGER` | `NOT NULL` | |
| `crc32` | `INTEGER` | `NOT NULL` | Fast integrity |
| `uploaded_at` | `TEXT` |  | Timestamp after S3 sync |
Indexes:
- `CREATE UNIQUE INDEX ux_wal_segments_collection_start ON wal_segments(collection_id, start_sequence);`
- `CREATE INDEX ix_wal_segments_uploaded ON wal_segments(collection_id, uploaded_at);`

### 1.3 Relationship & Integrity Rules
- `tenants` → `databases` → `collections` → `docs` form a strict containment hierarchy; cascading deletes are limited to soft deletions (logical `deleted_at`) with asynchronous cleanup tasks.
- Every WAL segment and snapshot references exactly one collection; background processes ensure no dangling references exist.
- Database triggers maintain `updated_at` and enforce quota budgets by comparing aggregate metrics from materialized views (`tenant_usage_v`, `collection_usage_v`).
- A `metadata_schema_version` singleton table guards rolling upgrades. Application start-up aborts if expected version mismatches.

### 1.4 Schema Migration Strategy
- Managed through `sqlx::migrate!` embedded migrations. Directory layout: `migrations/metadata/{epoch}_{description}.sql` plus Rust integration tests that validate idempotency on empty and seeded databases.
- Backwards-compatible changes (add columns, add tables) use normal migrations. Breaking changes require dual-write shims and a two-step plan (expand → backfill → contract) coordinated with the runtime feature flags stored in `tenants.metadata`.
- Node startup performs: (1) verify database snapshot, (2) run migrations inside a single transaction, (3) re-hydrate prepared statements.
- For fleet upgrades, the control plane enforces phased rollout: canary nodes upgrade first, then wave-based deployment with automated health verification (metrics + WAL lag).

## 2. Rust Implementation Architecture
### 2.1 Workspace & Crate Layout
```text
akidb/
├── Cargo.toml (workspace)
├── crates/
│   ├── akidb-core/          # Domain models, error types, shared traits
│   ├── akidb-metadata/      # SQLite data access (sqlx) + schema migrations
│   ├── akidb-storage/       # WAL writer, snapshot engine, memory-mapped segments
│   ├── akidb-index/         # HNSW implementation + SIMD distance kernels
│   ├── akidb-embed/         # Embedding service, model runtime integration
│   ├── akidb-control-plane/ # gRPC + REST API surface (tonic + axum)
│   ├── akidb-scheduler/     # Background jobs (sync, compaction, quota enforcer)
│   ├── akidb-observability/ # Metrics, tracing, logging utilities
│   └── akidb-cli/           # Admin / troubleshooting CLI
└── apps/
    ├── akidbd/              # Primary daemon binary (Tokio runtime)
    └── tools/               # Benchmarks, migration helpers
```

Crates adhere to hexagonal architecture: `akidb-core` defines ports (traits), outer crates provide adapters.

### 2.2 Key Traits & Service Boundaries
| Trait | Defined In | Responsibility | Implementations |
|-------|------------|----------------|-----------------|
| `TenantCatalog` | `akidb-core` | List/resolve tenants, enforce lifecycle states | `akidb-metadata::SqliteTenantCatalog` |
| `QuotaEnforcer` | `akidb-core` | Calculate & enforce memory/QPS/storage budgets | `akidb-scheduler::QuotaSupervisor` |
| `CollectionRepository` | `akidb-core` | CRUD + configuration for collections | `akidb-metadata::SqliteCollectionRepo` |
| `VectorIndex` | `akidb-core` | Insert/search/delete vector payloads | `akidb-index::HnswIndex` |
| `WALAppender` | `akidb-core` | Serialize logical operations → WAL | `akidb-storage::WalWriter` |
| `Snapshotter` | `akidb-core` | Full + incremental snapshot orchestration | `akidb-storage::SnapshotManager` |
| `ObjectStore` | `akidb-core` | Abstract S3/MinIO interactions | `akidb-storage::ObjectStoreClient` |
| `EmbeddingProvider` | `akidb-core` | Synchronous/batch embedding API | `akidb-embed::{OnnxRuntimeProvider, TensorRtProvider}` |
| `PolicyEngine` | `akidb-core` | Evaluate RBAC policies | `akidb-control-plane::CedarPolicyEngine` |

Each API request flows: API adapter (gRPC/REST) → AuthN/AuthZ middleware → Tenant resolution → Quota check → Vector/WAL operations → Response serialization.

### 2.3 Tokio Runtime Strategy
- Primary daemon uses `tokio::runtime::Builder::new_multi_thread()` with worker count = `min(physical_cores, 16)` for edge predictability. Jetson defaults to 6 workers, Apple M-series to 8.
- A dedicated `LocalSet` hosts latency-sensitive read paths (vector search). Write-heavy tasks (ingest, snapshot upload) land in background `tokio::task::spawn_blocking` pools with bounded concurrency to avoid starving searches.
- Critical paths leverage `tokio::sync::Semaphore` to backpressure before violating quotas. Each tenant owns a semaphore keyed by operation type (query, ingest, admin).
- Runtime metrics (queue depth, task latency) exported via `tokio-metrics` integration into Prometheus.

### 2.4 Memory Management for ARM
- Use `mimalloc` allocator on macOS (better cache locality) and `jemalloc` on Jetson/Oracle ARM (tuned via `malloc_conf` env) selectable with cargo feature flags.
- Vector buffers allocated with `aligned_alloc::AlignedVec<f32, 64>` (from the `aligned-vec` crate) to guarantee 64-byte alignment for NEON instructions.
- Introduce slab allocators for fixed-size graph nodes (`slotmap` + pre-reserved capacity) reducing fragmentation under high churn.
- Apply `crossbeam_utils::CachePadded` to frequently mutated counters to avoid false sharing on big.LITTLE cores.
- Use zero-copy deserialization (`bytemuck`, `zerocopy`) when replaying WAL entries to reduce extra allocations.

### 2.5 SIMD Optimization (ARM NEON)
- Build HNSW distance kernels using `std::arch::aarch64::{float32x4_t, vld1q_f32, vfmaq_laneq_f32}` intrinsics compiled behind `#[target_feature(enable = "neon")]`.
- Provide portable fallback using `core::simd::Simd<f32, 16>` (requires Rust 1.75+) selected at runtime via CPUID detection (`is_aarch64_feature_detected!("neon")`).
- Batch process 32 vectors at a time: load candidate vectors with `memcpy` into aligned scratch space, compute partial dot-products using fused multiply-add, accumulate in registers, flush to scalar.
- Guard critical kernels with property-based tests comparing NEON + scalar results within 1e-5 tolerance. Perf harnesses live in `crates/akidb-index/benches` (criterion) and run on Jetson CI nightly.

## 3. HNSW Index Implementation Details
### 3.1 Parameter Tuning Guide
| Workload Profile | Data Scale | Latency Target | Recommended `(M, efConstruction, efSearch)` | Notes |
|------------------|------------|----------------|---------------------------------------------|-------|
| Edge Cache | ≤5M vectors | P95 ≤15 ms | `(16, 80, 64)` | Low-degree graph reduces memory; rebuild weekly |
| Balanced Default | 5M–30M | P95 ≤25 ms | `(32, 200, 128)` | Ships as collection default; good mix of recall/latency |
| High Recall | 30M–100M | P95 ≤40 ms | `(48, 320, 256)` | Requires ~1.5× RAM; enable only for premium tenants |

- `efSearch` is dynamically increased under low load (adaptive recall) bounded by tenant query budget.
- Warmup routine runs top-layer random walks after restart to pre-populate CPU caches.

### 3.2 Memory Layout for Cache Efficiency
- Node IDs map to a contiguous `Vec<NodeHeader>` (`struct NodeHeader { level: u8, vector_offset: u64, tombstone: AtomicBool }`).
- Level 0 neighbors stored in a single `Vec<AtomicU32>` chunk; upper levels kept in separate fixed-stride arrays to keep hot paths together.
- Distance computations operate on memory-mapped vector blobs: sequential layout `[vector][metadata padding]` ensures prefetching works. Each vector is padded to 64-byte boundary.
- Use a lock-free freelist (`crossbeam_queue::SegQueue`) for recycled node slots to minimize fragmentation after deletes.

### 3.3 Concurrency Model
- Read paths are lock-free aside from `ArcSwap` pointer dereferences. Each collection exposes a `SearcherHandle` containing an immutable snapshot of the graph.
- Writes acquire per-collection `parking_lot::RwLock` on upper-level structures while Level 0 updates use fine-grained `Mutex` shards (`num_shards = 2 × worker_threads`).
- Background compaction produces a shadow graph; `ArcSwap` atomically swaps in the new searcher once all in-flight readers drain (tracked through epoch counters).
- WAL appends are serialized to preserve ordering (`tokio::sync::Mutex`), but insert pipelines buffer candidate vectors to enable write batching.

### 3.4 Incremental Updates Strategy
- Every insert/update generates a WAL record (`InsertVector`, `UpdateVector`, `SoftDelete`) with deterministic serialization. WAL sequence numbers (LSN) match node IDs to simplify replay.
- Periodic maintenance merges recent WAL entries into the in-memory graph: apply delta graph on top of base snapshot, then flush new adjacency lists and vector segments.
- Deletions mark tombstones immediately; a compaction job rebuilds affected neighborhoods if tombstone ratio >20% in any layer.
- Hot-standby replicas tail the WAL and apply changes asynchronously. Divergence checks compare snapshot checksums every 15 minutes.

## 4. Storage Layer Design
### 4.1 WAL Format & Replay
- File layout (`.wal`):
  - File header: `MAGIC (u32='AKIW'), VERSION (u16=2), CRC (u32), reserved (8 bytes)`.
  - Each record: `[len:u32][lsn:u64][tenant_id:16B][collection_id:16B][timestamp:u64 ns][kind:u8][payload...][crc32:u32]`.
  - Payload variants (MessagePack encoded):
    - `InsertVector { doc_id, external_id, level_entries[], vector_bytes }`
    - `UpdatePayload { doc_id, payload_delta }`
    - `SoftDelete { doc_id }`
    - `Checkpoint { snapshot_generation }`.
- Replay algorithm:
  1. Validate header + CRC, then binary-search for last clean record using checksum.
  2. Derive collection execution order from `collection_id` groupings to parallelize reconstruction.
  3. Apply records into HNSW builder while verifying quotas. Insert vector segments into memory-mapped region at recorded offsets.
  4. Record latest LSN per collection in `wal_applied_progress` table for crash recovery.

### 4.2 Snapshot Format & Incrementals
- Snapshot stored as directory (`snapshot-<collection>-<generation>/`):
  - `manifest.json`: metadata (collection config, LSN window, checksum list).
  - `vectors.bin`: contiguous vector payloads compressed with `zstd --adapt`.
  - `graph.lvl0.bin`, `graph.lvlX.bin`: adjacency arrays per level.
  - `docs.parquet`: document metadata for quick cold-start.
- Incremental snapshots (every N WAL MB, default 512 MB): store only changed vectors + adjacency segments with reference to base generation via manifest pointer. Apply by overlaying delta segments while verifying checksums.
- Snapshots are memory-mapped for fast restoration; manifest includes `page_size` ensuring compatibility across devices.

### 4.3 Memory-Mapped File Strategy
- Vector files managed via `memmap2::MmapOptions` with `MAP_SHARED`. Preallocation uses `fallocate`/`posix_fallocate` to avoid runtime growth fragmentation.
- Writes use copy-on-write buffers: new vectors appended to `mutable` region; once sealed, region is remapped as read-only to allow crash-resistant restarts.
- Jetson devices: use 2 MiB huge pages where available (`/sys/kernel/mm/hugepages`). Fallback to 64 KiB alignment on macOS due to APFS constraints.
- Periodic `msync(MS_ASYNC)` ensures OS flushes dirty pages without blocking read queries.

### 4.4 S3/MinIO Sync Protocol
- Each collection maintains a `sync_manifest.json` containing list of WAL segments and snapshot generations uploaded. Manifest stored both locally and remote for verification.
- Upload pipeline:
  1. Detect sealed WAL segment or snapshot.
  2. Chunk into 64 MiB parts; upload via multipart with `s3-crt` client (minimized CPU usage) and SSE-S3 encryption by default. Customer-supplied keys stored in tenant metadata.
  3. After upload, write `etag` + checksum back to `wal_segments.uploaded_at` and update manifest.
  4. Verification job downloads random 1% sample monthly, performing checksum comparison.
- Sync supports resumable uploads by persisting `upload_session` state in `automatosx/tmp/` (ignored by git). Control plane monitors RPO (<15 minutes) using LSN lag metrics.

## 5. Embedding Service Architecture
### 5.1 Model Lifecycle & Quantization
- Models stored in `tenants/<tenant_id>/models/<model_id>/`. Lifecycle:
  1. Download base model artifact (ONNX/gguf) via signed URL.
  2. Run quantization pipeline (`akidb-embed::quantize`) producing int8/float16 variants using `ggml` + per-channel scaling. Outputs include calibration stats.
  3. Register in metadata (`embedding_models` table) with footprint, supported dimensions, and default precision.
- Quantization profiles: `edge_q8` (balanced), `edge_q4` (extreme memory saving), `fp16` (high precision). Model selection per collection stored in `collections.embedding_model`.

### 5.2 Batch Processing & Queueing
- API requests arrive via `EmbeddingProvider` trait. They enqueue into a `tokio::mpsc` channel sized to `tenant.batch_queue_depth` (default 2048 vectors).
- A `Batcher` coalesces requests by model + tenant with 10 ms max latency or 512 vector max size, whichever first.
- Execution workers pinned to dedicated Tokio worker set; each holds a warm model context. Results streamed back via `oneshot` channels.
- Expose metrics: batch size distribution, queue depth, tokens/sec, failure counts.

### 5.3 Model Switching without Downtime
- Maintain `ModelRouter` with `ArcSwap<ModelHandle>`. Deployments follow blue/green:
  1. Load new model in background, run health probes (self-check embeddings, cosine similarity drift vs baseline ≤0.01).
  2. Flip router pointer atomically once success criteria met.
  3. Keep previous model alive for configurable drain window (default 5 minutes) to serve in-flight requests.
  4. WAL records (`UpdateCollectionModel`) capture change for replicas.
- Config knob for canary routing (e.g., 5% traffic) before full flip.

### 5.4 GPU Delegation Strategy (Jetson)
- Prefer TensorRT backend using `onnxruntime` with `providers = ["TensorrtExecutionProvider", "CUDAExecutionProvider", "CPUExecutionProvider"]`.
- GPU executor pool sized to (`min(cuda_sms × 2, 4)`). CPU fallback automatically enables when GPU temp >80°C or driver resets detected.
- Model artifacts include both TensorRT engine cache and CPU fallback to avoid on-device compilation during rollout.
- Embedding jobs scheduled by quota-aware executor: ensures GPU time is apportioned per tenant and preempts non-critical batches when high-priority traffic arrives.

## 6. Multi-tenancy Implementation
### 6.1 Tenant Isolation Mechanisms
- Namespace all on-disk assets: `data/<tenant_id>/<database_id>/<collection_id>/...`.
- Per-tenant encryption keys stored in HashiCorp Vault or local KMS; keys fetched at startup and cached in enclave (in-memory, sealed with AES-GCM) with rotation support.
- API auth tokens encode `tenant_id` + roles; enforcement happens before hitting business logic. Cross-tenant requests are rejected at routing layer.
- Background jobs run in tenant-specific tasks; no shared mutable state without `TenantScope` guard.

### 6.2 Resource Quota Enforcement
- Real-time usage tracked via `TenantUsage` struct (atomic counters). Reads update QPS counters using sliding windows (`ratelimit_meter`). Writes track memory by monitoring vector segment allocations.
- `QuotaSupervisor` polls metrics every 5 seconds, comparing against quotas. When exceeded: degrade gracefully (throttle new ingest) and emit audit events.
- Persistent usage snapshots stored in `tenant_usage_daily` table for billing/export downstream.

### 6.3 RBAC Policy Engine Design
- Policy documents follow Cedar syntax (AWS Verified Permissions) stored in `tenant_policies` table (`policy_id`, `policy_text`, `version`, `created_by`).
- Engine compiles policies on load and caches in `ArcSwap`. Structural validation occurs on write using Cedar parser; simulation endpoint allows dry-run before activation.
- Roles (`admin`, `developer`, `viewer`, `auditor`) map to collections/databases via `RoleBinding` records. Fine-grained permissions (e.g., `collection:write`, `embedding:invoke`) enforced at API layer and background job scheduler.
- Every decision logs policy ID + evaluation result into audit stream, satisfying compliance requirements.

### 6.4 Performance Isolation Strategies
- Tokio runtime uses per-tenant `coop` budgets to prevent noisy neighbors; each request tagged with `TenantContext` to attribute CPU time.
- Memory segregation: `collection` memory arenas limited via `VecPool` quotas; exceeding allocations triggers backpressure.
- WAL flush frequency adaptively throttled per tenant to avoid shared disk contention.
- Embedding GPU scheduler enforces fair-share; optionally integrate with Linux cgroups on Jetson to cap CPU usage.

## 7. API Layer Architecture
### 7.1 Interface Strategy (gRPC vs REST)
| Consideration | gRPC (tonic) | REST (axum) | Decision |
|---------------|--------------|-------------|----------|
| Latency | HTTP/2 multiplexing, binary proto | JSON serialization overhead | **Use gRPC** for data plane (ingest/query); provide REST facade for admin surfaces |
| Streaming | Native bidirectional streams | Manual SSE/WebSocket | gRPC for real-time data replication |
| Tooling | Strong typing, codegen (Go, Rust, Python) | Broad ecosystem, curl-friendly | REST for onboarding UI, SDKs wrap gRPC channels |

- `akidb-control-plane` exposes both: `/v1/*` REST (OpenAPI) for provisioning + metrics, `proto.akidb.v1` gRPC for hot paths.

### 7.2 Connection Management & Pooling
- gRPC server uses `tonic::transport::Server` with connection keepalive tuned for edge networks (ping every 15s, 45s timeout).
- Client SDKs leverage `tonic::transport::Channel` with shared `Endpoint` per tenant, max 4 connections default.
- Metadata store connection pooling via `sqlx::SqlitePool` (max 16 connections) with `tokio::sync::Semaphore` to cap concurrency.
- Object storage interactions reuse `aws-crt` connection pools; keepalive with exponential backoff.

### 7.3 Rate Limiting
- Layered approach using `tower` middleware:
  - Global limiter for entire cluster to protect from overload.
  - Per-tenant leaky bucket (`governor` crate) configured from quota table (different buckets for query vs ingest).
  - Per-user short-term burst limits (100 req/s by default) to mitigate abusive clients.
- Rate-limit decisions surfaced via specific gRPC status `RESOURCE_EXHAUSTED` with retry hints.

### 7.4 Error Handling Patterns
- Unified error type (`akidb-core::Error`) maps to gRPC statuses and REST `application/problem+json` responses.
- Each error carries `error_code`, `tenant_id`, `correlation_id`. Logging uses structured events with span context.
- Client-safe messages separated from server diagnostics to avoid information leaks. Detailed stack traces only in debug builds.
- Background jobs push failure events to `event_bus` (internal) so ops tooling can react.

## 8. Observability Architecture
### 8.1 Metrics Collection Points
- Instrumented via `prometheus-client` crate:
  - API: QPS per method, latency histograms, auth failures.
  - HNSW: search depth, candidate count, graph rebuild duration.
  - Storage: WAL lag, flush throughput, snapshot size.
  - Embedding: queue depth, batch size, tokens/sec, GPU utilization.
  - Quota: per-tenant usage vs budget.
- Metrics endpoint exposed on `:9100/metrics` using text exposition format; Jetson builds include lightweight exporter to fit resource limits.

### 8.2 Distributed Tracing Strategy
- Use `tracing` + `opentelemetry` to emit OTLP spans. Default collector: `otlp-http` to support restricted environments.
- Trace propagation via `traceparent` headers for REST and gRPC metadata `grpc-trace-bin`.
- Critical spans: request entry, quota check, index search, WAL append, S3 upload. Sampling: 1% baseline, elevated dynamically for error rates.
- Provide `akidb-cli trace tail` tool to fetch traces during on-call.

### 8.3 Structured Logging Format
- JSON logs (`tracing_subscriber::fmt().json()`) with fields: `timestamp`, `level`, `tenant_id`, `span`, `event`, `correlation_id`, `message`, `kv` (structured context).
- Log sinks:
  - Local file ring buffer (`data/logs/akidbd.log`, 200 MB max, rotated).
  - Optional Loki/Grafana agent shipping (edge safe) configurable per tenant.
- Sensitive payloads (vectors, raw text) are redacted before logging.

### 8.4 Health Check Protocol
- Endpoints:
  - `/health/live`: process up, event loop responsive (<200 ms response).
  - `/health/ready`: metadata DB reachable, WAL lag < threshold, snapshot manager idle.
  - `/health/tenant/{tenant_id}`: verifies quota state, policy engine load, embedding service status.
- Health responses include build info, git SHA, active feature flags. Control plane uses results for rollout gating and auto-remediation.

## 9. Architecture Runway & Open Items
- Finalize allocator selection benchmarks on Apple M2 Ultra vs Jetson AGX (owner: Core Performance, due 2024-05-29).
- Validate Cedar policy performance under 10k policies/tenant; fall back to OPA mini-bundle if latency >5 ms (owner: Platform Security).
- Complete design of WAL multi-producer pipeline for sharded ingest nodes (ADR candidate).
- Document migration playbook from AkiDB 1.x HNSW layout to new segmented format (ties to PRD migration requirement).

Great architecture is invisible - it enables teams, evolves gracefully, and pays dividends over decades.
