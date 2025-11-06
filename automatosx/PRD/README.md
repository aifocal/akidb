# AkiDB 2.0 - PRD Package

**Version:** 1.0 | **Date:** 2025-11-06 | **Status:** Draft - Ready for Executive Review

---

## Executive Summary

This PRD package provides a comprehensive blueprint for AkiDB 2.0 that **builds upon the existing akidb v1.x codebase** (`/Users/akiralam/code/akidb`) rather than starting from scratch. This is a **strategic refactoring project** (~70% reuse, ~30% new development), not a greenfield rewrite.

### Key Finding

Analysis reveals substantial v1.x infrastructure:
- **Existing:** TenantDescriptor, WAL, HNSW indexing, S3/MinIO, REST API, MCP server, comprehensive tests
- **NEW for 2.0:** Embedding service, SQLite metadata, gRPC API (3 new crates)
- **ENHANCED:** Database hierarchy, RAM-first tiering, Cedar RBAC (7 enhanced crates)

---

## Documents in This Package

### 1. [Improved PRD](./akidb-2.0-improved-prd.md) - Strategic Product Requirements
**Purpose:** Market positioning, user stories, API specs, cost analysis
**Use for:** Executive presentations, customer roadmaps, go-to-market strategy
**Key Sections:** User stories (Section 4), Competitive analysis (Section 6), Success metrics

### 2. [Technical Architecture](./akidb-2.0-technical-architecture.md) - Implementation Blueprint
**Purpose:** SQLite schema, Rust crates, HNSW tuning, embedding service design
**Use for:** Engineering implementation, code reviews, performance optimization
**Key Sections:** Database schema (Section 1), Rust architecture (Section 2), HNSW tuning (Section 3)

### 3. [Migration Strategy](./akidb-2.0-migration-strategy.md) - v1.x → 2.0 Refactoring Guide
**Purpose:** Reuse vs build matrix, 5-phase roadmap, per-crate migration guides
**Use for:** Sprint planning, effort estimation, backward compatibility
**Key Sections:** Reuse matrix (Section 1), Refactoring roadmap (Section 2), Component guides (Section 3)

### 4. [Executive Summary](./akidb-2.0-executive-summary.md) - Strategic Recommendations
**Purpose:** Analysis, decision framework, execution roadmap, quality gates
**Use for:** Executive reviews, budget approvals, risk assessment
**Key Sections:** Recommendations (Section 2), Execution roadmap (Section 3), Metrics (Section 4)

---

## Quick Start Guide

### For Product Managers
1. Read: [Executive Summary](./akidb-2.0-executive-summary.md) → Big picture
2. Focus on: User personas, success criteria, go-to-market strategy

### For Engineering Leads
1. Read: [Migration Strategy](./akidb-2.0-migration-strategy.md) → Refactoring plan
2. Focus on: Reuse vs Build Matrix, 5-phase roadmap, task breakdown

### For Architects
1. Read: [Technical Architecture](./akidb-2.0-technical-architecture.md) → System design
2. Focus on: Database schema, Rust traits, SIMD optimization, observability

### For Executives
1. Read: [Executive Summary](./akidb-2.0-executive-summary.md) → Strategic overview
2. Focus on: Cost ($270k + $9.75k/3mo), success metrics, risk register

---

## Critical Recommendations

### 1. Adopt Hybrid Route (Balance Risk & Differentiation)
- **Phase 1 (Weeks 0-8):** Low-risk core (Mac ARM + OCI ARM, single model, CPU inference)
- **Phase 2 (Weeks 9-12):** Selective high-risk (Jetson if benchmarks pass, second model if demand)
- **Rationale:** Reduces integration risk, allows customer feedback, preserves 3-month timeline

### 2. Validate Early (Weeks 0-2)
- [ ] Benchmark HNSW on Jetson Orin (512-dim vectors)
- [ ] Confirm embedding model fits <8GB memory
- [ ] Legal review of Qwen3-Embedding-8B license
- [ ] Profile allocator performance (mimalloc vs jemalloc)

### 3. Leverage Existing Codebase
- **Reuse:** `akidb-core/tenant.rs` (multi-tenancy), `akidb-storage/wal.rs`, `akidb-index` (HNSW)
- **Enhance:** Add database hierarchy, Cedar RBAC, RAM-first tiering
- **NEW:** `akidb-embed`, `akidb-metadata`, `akidb-control-plane` (gRPC)

---

## Implementation Phases

### Phase 1: Foundation (Weeks 1-4)
- `akidb-metadata` crate with SQLite schema
- DatabaseDescriptor in `akidb-core`
- Migration tool for v1.x tenants

### Phase 2: Embedding Service (Weeks 5-8)
- `akidb-embed` with Qwen3-Embedding-8B
- Integration with `akidb-ingest`
- Feature flags for rollout

### Phase 3: Enhanced RBAC (Weeks 9-12)
- Cedar policy engine
- Policy authoring tooling
- Audit logging

### Phase 4: API Unification (Weeks 13-16)
- gRPC via `akidb-control-plane`
- REST /v1 preserved, /v2 added
- SDK updates

### Phase 5: RAM-First Tiering (Weeks 17-20)
- Memory-mapped storage
- WAL format upgrade
- Performance benchmarks

---

## Success Metrics

### Engineering (Track Weekly)
- Code Coverage: >80%
- P95 Query Latency: <25ms (1M vectors, 512-dim)
- Memory Footprint: <12GB per 1M vectors
- Crash Recovery: <60s

### Product (Track Monthly)
- Design Partner Adoption: 3 by Month 6
- NPS Score: ≥30
- Onboarding Time: <2 hours
- Support Tickets: <10/month P0+P1

---

## Next Steps (This Week)

### Executive Team
- [ ] Schedule PRD review (by 2025-11-08)
- [ ] Decide on hybrid route
- [ ] Approve $35k budget increase

### Engineering Team
- [ ] Procure Jetson Orin + OCI ARM instances
- [ ] Set up ARM64 CI/CD pipeline
- [ ] Baseline v1.x performance benchmarks

### Architecture Team
- [ ] Review migration strategy with leads
- [ ] Create GitHub project with milestones
- [ ] Initialize Rust workspace structure

---

## Reuse Guide: Existing v1.x Components

### ✓ Reuse (Minimal Changes)
- `akidb-core/tenant.rs` → Migrate to SQLite storage, keep domain logic
- `akidb-storage/wal.rs` → Add collection_id + LSN fields
- `akidb-index` → Add metadata hooks for database scoping
- `akidb-api/tests/` → Preserve E2E and integration tests
- `akidb-benchmarks/` → Extend for embedding + tiering

### ✓ Enhance (Refactor)
- `akidb-core/collection.rs` → Add database_id field
- `akidb-core/user.rs` → Integrate Cedar policy engine
- `akidb-ingest` → Add embedding pipeline hooks
- `akidb-api` → Add gRPC endpoints alongside REST

### ✨ NEW (Build from Scratch)
- `akidb-embed` → Embedding service with Qwen3-Embedding-8B
- `akidb-metadata` → SQLite metadata database
- `akidb-control-plane` → gRPC control plane

---

## Document Conventions

- **✓ Reuse:** Use as-is with minimal changes
- **✓ Enhance:** Needs refactoring or extension
- **✨ NEW:** Build from scratch
- **P0/P1/P2:** Priority levels (P0 = critical path)

---

## Support & Feedback

**Product Questions:** Product Lead (TBD)
**Technical Questions:** Architecture Lead (TBD)
**Migration Questions:** Engineering Lead (TBD)

---

**Prepared by:**
- AutomatosX Product Agent (Paris) - Strategic PRD
- AutomatosX Architecture Agent (Avery) - Technical Architecture & Migration Strategy
- Claude Code - Analysis, Synthesis & Executive Summary

**Review Status:** Draft | **Next Review:** 2025-11-08 (Executive Team)
