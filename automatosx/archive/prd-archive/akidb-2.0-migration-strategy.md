# AkiDB 2.0 Migration and Refactoring Strategy

## Overview

AkiDB 2.0 evolves the existing 1.x platform into a multi-tenant, metadata-aware vector database with first-class embeddings, policy-governed access, and unified control plane APIs. This strategy builds directly on the current workspace, prioritising pragmatic refactors over rewrites while enabling incremental delivery, rollback safety, and cross-team alignment.

## 1. Reuse vs Build Matrix

| Component | Existing Status | 2.0 Action | Effort | Risk | Priority |
|-----------|-----------------|------------|--------|------|----------|
| akidb-core | Stable domain types (tenant/collection) | Refactor to add `database` layer, Cedar-backed RBAC, new IDs | High | High | P0 |
| akidb-storage | Mature WAL + S3 | Enhance with RAM-first tiering, WAL schema update | High | High | P0 |
| akidb-index | HNSW + SIMD | Reuse; add metadata hooks for database scoped indexes | Low | Low | P2 |
| akidb-query | Query execution | Extend to resolve database-scoped collections via metadata | Medium | Medium | P1 |
| akidb-api | REST on Axum | Expand with Tonic gRPC endpoints + metadata integration | High | Medium | P0 |
| akidb-ingest | CSV/JSON/Parquet parsers | Refactor ingest flows to register metadata + trigger embeddings | Medium | Medium | P1 |
| akidb-mcp | MCP server ready | Reuse; update capabilities catalog once control plane stabilises | Low | Low | P2 |
| akidb-pkg | Package management | Reuse; ensure packages record metadata migrations | Low | Medium | P2 |
| akidb-replication | WAL-driven | Refactor reader to honour collection_id + LSN | Medium | Medium | P0 |
| akidb-benchmarks | Performance suite | Update scenarios for RAM-tier + embedding latency | Medium | Medium | P1 |
| akidb-embed (new) | N/A | Build new crate wrapping Qwen3-Embedding-8B service | High | Medium | P0 |
| akidb-metadata (new) | N/A | Build SQLite-backed metadata service + migrations | High | Medium | P0 |
| akidb-control-plane (new) | N/A | Layer gRPC control plane atop REST handlers | High | Medium | P0 |
| Observability stack | Prometheus + partial OTEL | Extend traces/metrics for new components | Medium | Medium | P1 |

## 2. Refactoring Roadmap

**Phase 1 – Foundation (Metadata & Hierarchy)**
- Deliverables: `akidb-metadata` crate, tenant→database→collection schema, migration utility, updated descriptors in `akidb-core`.
- Key work: SQLite schema design, migration scripts for existing tenants, API wiring for metadata reads.
- Exit criteria: REST API reads from metadata DB, integration tests passing, rollback script verified.

**Phase 2 – Embedding Service Integration**
- Deliverables: `akidb-embed` crate, async job orchestration, ingestion hooks.
- Key work: gRPC/HTTP client to Qwen3-Embedding-8B, batching strategy, caching/pooling controls, fallbacks to synchronous path.
- Exit criteria: Ingest pipeline populates embeddings end-to-end; feature flag guards live.

**Phase 3 – Enhanced RBAC with Cedar**
- Deliverables: Cedar policy engine integration in `akidb-core::user`, policy authoring tooling, seed policies.
- Key work: Map existing roles/permissions to Cedar schemas, implement policy store retrieval from metadata DB, audit logging.
- Exit criteria: REST & gRPC authz guard rails on; canary tenants validated.

**Phase 4 – API Unification (REST + gRPC)**
- Deliverables: `akidb-control-plane`, Tonic services mirroring REST operations, shared request/response models.
- Key work: Extract shared service layer, add API versioning, produce OpenAPI + protobuf docs, update MCP integrations.
- Exit criteria: gRPC smoke tests green, REST regression suite intact, dual-stack load test completed.

**Phase 5 – RAM-First Tiering**
- Deliverables: Memory-mapped tier in `akidb-storage`, WAL format upgrade with versioning, replication adjustments.
- Key work: Tier manager abstraction, background eviction, WAL reader upgrades, benchmark updates.
- Exit criteria: Tiering fitness tests, replication compatibility validated, latency improvements logged.

## 3. Code Migration Guides

### akidb-core
- **Keep:** Entity models, quota tracking logic, existing validation utilities.
- **Refactor:** Introduce `DatabaseDescriptor`, link `CollectionDescriptor` with `database_id`, migrate `TenantDescriptor` storage to `akidb-metadata`.
- **Add:** Cedar policy integration with policy cache, helper APIs for metadata lookups, ID generation strategy (UUIDv7 recommended).
- **Data Migration:** Script to export in-memory tenant/collection state into SQLite; include rollback plan that rehydrates legacy JSON snapshots.

### akidb-storage
- **Keep:** S3/MinIO adapters, WAL writer interfaces, compaction utilities.
- **Refactor:** Extend WAL entries with `collection_id`, `database_id`, and monotonic LSN; update readers/replication.
- **Add:** Tier manager with memory-mapped segment support, heuristics for promotion/eviction, observability hooks.
- **Data Migration:** One-time WAL format translator to append missing identifiers; simulate on staging WALs before production rollout.

### akidb-api & akidb-control-plane
- **Keep:** Axum routers, REST handlers, existing auth checks.
- **Refactor:** Extract business logic into shared service layer callable from REST and Tonic; update routing to fetch metadata entities lazily.
- **Add:** Tonic gRPC server (mutual TLS support), protobuf contracts, version negotiation middleware, API gateway documentation.
- **Data Migration:** API clients transition guide; provide shims that translate gRPC to REST for legacy consumers during overlap phase.

### akidb-query
- **Keep:** Query planner/executor, performance optimisations.
- **Refactor:** Resolve collection metadata at execution time, enforce database scoping, propagate Cedar authorisation context.
- **Add:** Query stats emitted via OTEL, optional embedding similarity operator using `akidb-embed`.
- **Data Migration:** Validate saved queries/schedules against new metadata IDs; supply compatibility remapper.

### akidb-ingest
- **Keep:** Parser pipeline, batching, error reporting.
- **Refactor:** Register datasets via metadata service, emit events to embedding queue, ensure idempotent ingestion with new IDs.
- **Add:** Backpressure awareness with embedding job acknowledgements, configurable embedding policy per collection.
- **Data Migration:** Backfill embeddings for existing collections; script replays ingestion with embeddings disabled to avoid duplication.

### akidb-embed (new)
- **Keep:** N/A.
- **Refactor:** N/A.
- **Add:** Client abstraction for Qwen3-Embedding-8B (consider gRPC over HTTP2), connection pooling, failure fallbacks (retry, degrade).
- **Data Migration:** Bootstrap cache store (e.g., RocksDB/Redis) if adopted; seed warm embeddings for frequently queried collections.

### akidb-metadata (new)
- **Keep:** N/A.
- **Refactor:** N/A.
- **Add:** SQLite schema migrations (sqlx or refinery), API for transactional metadata updates, background job for integrity checks.
- **Data Migration:** Initial migration populating tenants/collections; nightly consistency check between metadata and storage descriptors.

### akidb-replication
- **Keep:** Stream processors, S3 snapshot handling.
- **Refactor:** Recognise upgraded WAL schema, include database/collection scope in replication filters.
- **Add:** Version-aware handshake, monitoring for LSN drift, configurable lag alerts.
- **Data Migration:** Bootstrap replicants by replaying translated WAL; maintain ability to ingest legacy WAL via compatibility mode until cutover.

### akidb-index & akidb-benchmarks
- **Keep:** Current HNSW implementation, benchmark harness.
- **Refactor:** Parameterise index builds by database scope, update benchmarks to include RAM-tier interactions.
- **Add:** Benchmark cases covering embedding latency + tiering, metrics exporters.
- **Data Migration:** Validate existing index files align with new metadata IDs; provide reindex automation if mismatch detected.

## 4. Backward Compatibility Strategy

- **Data Migration:** Provide offline migration tool that exports v1.x tenant/collection JSON, writes into SQLite within a transaction, and keeps an immutable backup. Include verification step comparing counts and checksums.
- **WAL Compatibility:** Introduce schema version header; readers attempt new version first, fallback to legacy parsing. Maintain legacy writer behind feature flag until replication catches up.
- **API Versioning:** Maintain `/v1` REST endpoints unchanged; introduce `/v2` REST and `grpc.v2` namespace. Offer translation proxy (REST facade) for clients unable to move immediately.
- **Feature Flags:** Gate embeddings, Cedar enforcement, and tiering separately per tenant/database. Use configuration stored in metadata DB with rollout plans per tenant cohort.
- **Rollout Playbook:** Canary tenants migrate first with read-only fallbacks; maintain dual-write (metadata + legacy) during soak period, then disable legacy path once confidence is established.

## 5. Testing Strategy

- **Preserve:** All unit tests across `akidb-core`, `akidb-storage`, `akidb-query`, `akidb-api`. Ensure snapshot tests covering REST payloads remain authoritative for `/v1`.
- **New Unit Tests:** Metadata schema migrations (forward/backward), Cedar policy evaluation, embedding client resiliency, tier manager eviction decisions.
- **Integration Tests:** End-to-end ingest → metadata → embedding → query flow; gRPC/REST parity tests; replication replay with upgraded WAL; dual-stack authZ enforcement.
- **Performance Regression:** Extend `akidb-benchmarks` to measure RAM-tier latency, embedding throughput, gRPC vs REST overhead. Automate nightly benchmark comparisons with alert thresholds.
- **Non-Functional:** Chaos tests for embedding service outages, failover of metadata DB (SQLite WAL + backup), soak tests for Cedar policy churn.

## 6. Crate Dependency Graph (Target)

```
akidb-core
├─ akidb-metadata
├─ akidb-embed (feature: embeddings)
└─ cedar-policy (external)

akidb-storage
├─ akidb-core
└─ akidb-metadata (for descriptors)

akidb-query
├─ akidb-core
├─ akidb-storage
└─ akidb-embed (optional)

akidb-api (REST)
└─ akidb-core

akidb-control-plane (gRPC)
└─ akidb-core

akidb-ingest
├─ akidb-core
├─ akidb-storage
└─ akidb-embed

akidb-replication
├─ akidb-storage
└─ akidb-core

akidb-embed
└─ external qwen client

akidb-metadata
└─ SQLite ecosystem crates (sqlx/rusqlite)
```

Ensure circular dependencies remain resolved via traits/interfaces in `akidb-core`.

## 7. Risk Mitigation and Rollback

- **Metadata Cutover Failure:** Mitigation—dual-write to legacy JSON and SQLite during canary; automated consistency checks. Rollback—disable metadata feature flag, replay JSON snapshot.
- **Cedar Policy Misconfiguration:** Mitigation—policy linting, stage policies in dry-run mode logging denials. Rollback—fallback to role-based checks via feature flag.
- **Embedding Service Instability:** Mitigation—bulkhead pattern with circuit breaker; queue backlog monitoring. Rollback—feature-flag embeddings per collection, default to precomputed vectors.
- **gRPC Adoption Issues:** Mitigation—publish SDKs, contract tests with critical clients. Rollback—continue REST as primary; gRPC server can be disabled without impacting REST handlers.
- **RAM-Tiering Performance Regression:** Mitigation—progressive rollout per collection, metrics-based guardrails, dedicated benchmark coverage. Rollback—demote tier manager, revert to disk-first via config toggle.
- **Replication Drift Post-WAL Upgrade:** Mitigation—shadow cluster consumes new WAL alongside existing; monitor LSN parity. Rollback—revert writer to legacy format via compatibility flag.

Great architecture is invisible—it enables teams, evolves gracefully, and pays dividends over decades. This playbook gives every team the guardrails to execute confidently while preserving that principle.
