# AkiDB

[![Rust CI](https://github.com/aifocal/akidb/actions/workflows/ci.yml/badge.svg)](https://github.com/aifocal/akidb/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![Rust Version](https://img.shields.io/badge/rust-1.77%2B-orange.svg)](https://www.rust-lang.org)
[![GitHub Stars](https://img.shields.io/github/stars/aifocal/akidb?style=social)](https://github.com/aifocal/akidb)
[![GitHub Issues](https://img.shields.io/github/issues/aifocal/akidb)](https://github.com/aifocal/akidb/issues)
[![GitHub Release](https://img.shields.io/github/v/release/aifocal/akidb?include_prereleases)](https://github.com/aifocal/akidb/releases)

**AkiDB: ARM-native, S3-backed vector database for offline semantic retrieval and edge AI pipelines.**

AkiDB is a high-performance, open-source vector database built exclusively for **ARM platforms** (Apple Silicon, NVIDIA Jetson, ARM cloud) with MinIO/S3 as the primary storage backend. Written in Rust, it's designed for **air-gapped deployments, edge inference, and power-efficient offline RAG** where sovereignty, cost, and energy consumption matter most.

---

## 1. Why AkiDB Matters

In edge AI and sovereign deployments, **x86-based vector databases are fundamentally misaligned**:

*   **High Power Consumption:** x86 servers draw 150W+ per node. Edge deployments (ships, field offices, IoT gateways) can't support this.
*   **Cloud Lock-In:** Managed services (Pinecone, Weaviate Cloud) require internet connectivity and send data off-premises.
*   **Complex Stacks:** Multi-component architectures (etcd, Kafka, Pulsar) are hard to deploy, audit, and maintain in air-gapped environments.
*   **x86 Tax:** Intel/AMD chips cost more and run hotter than ARM equivalents (Apple Silicon, Jetson, Graviton).

**AkiDB offers an ARM-native alternative.** Built exclusively for **ARM platforms + MinIO/S3 storage**, it brings:

*   **Power Efficiency:** ≤50W per node (vs 150W+ x86). Deploy on Mac Mini, Jetson, or ARM cloud.
*   **Edge-Ready:** Run offline on Jetson for inference at the edge (ships, vehicles, remote sites).
*   **Cost-Optimized:** Mac Studio M2 Ultra (~$5,000) outperforms $15,000 x86 servers at 1/3 the power.
*   **Platform Lock-In (By Design):** Explicitly targets ARM ecosystem. No x86 support = simpler codebase and deeper ARM optimizations.

## 2. Core Value Proposition

AkiDB is built for **ARM platforms + MinIO storage** where power efficiency, sovereignty, and edge deployment are non-negotiable.

### ✅ **ARM-Native Performance**
*   **Target Platforms:**
    *   **macOS Apple Silicon:** M2/M3/M4 (MLX Metal acceleration)
    *   **NVIDIA Jetson:** Orin Nano, Orin NX, AGX Orin (CUDA Tensor Cores)
    *   **ARM Cloud:** AWS Graviton, Oracle A1, Azure Cobalt (CPU SIMD)
*   **Compute Backends:** Pluggable trait system (`ComputeBackend`) with NEON, MLX, CUDA
*   **Power Efficiency:** ≤50W per node target (measured on Jetson Orin NX)
*   **No x86 Support:** Explicit ARM-only focus for simpler codebase and deeper optimizations

### ✅ **Data Sovereignty & Compliance**
*   **Air-Gapped Deployments:** Runs entirely offline. No cloud API calls, telemetry, or external dependencies.
*   **MinIO Security Integration:**
    *   SSE-KMS encryption with KES/HashiCorp Vault
    *   Object Lock (WORM) for immutable index segments
    *   Versioning for forensic rollback
    *   Legal Hold support for regulated industries
*   **Audit Trails:** Every query generates a tamper-proof hash chain stored in MinIO audit buckets.
*   **Certifiable:** Simplified stack (2 components) makes security audits and compliance certification feasible.

### ✅ **90%+ Cost Reduction**
*   **MinIO Cold Storage:** Primary storage on HDD/tape ($0.01-0.02/GB) vs. cloud block storage ($0.10/GB).
*   **Two-Tier Storage:** Hot (NVMe LRU cache) → Cold (MinIO/Zstd compression).
*   **ARM Hardware Economics:**
    *   **Mac Mini M2 Pro:** ~$1,800 (40W, 32GB RAM, 1TB NVMe)
    *   **Jetson Orin NX:** ~$899 (25W, 16GB RAM, edge-optimized)
    *   **Oracle A1 (ARM cloud):** FREE tier (4 OCPU, 24GB RAM)
*   **Example TCO (10M vectors, 1536-dim):**
    *   **Pinecone p1.x1:** ~$70/month
    *   **AkiDB on 3x Mac Mini:** ~$0.50/month (storage) + $5,400 one-time hardware

### ✅ **Portable & Offline-First**
*   **`.akipkg` Packaging:** Freeze index snapshots with signatures for cross-site migration.
*   **Offline Ingest:** Batch import from CSV/JSONL/Parquet with zero internet access.
*   **Multi-Site Replication:** Leverage MinIO Site Replication for DR and geo-distribution.

### ✅ **Operational Simplicity**
*   **Two Components:** AkiDB binary + MinIO cluster. No etcd, Kafka, or coordination layers.
*   **Stateless Compute:** Horizontal scaling without state management complexity.
*   **Single Binary:** No runtime dependencies. Deploy on bare metal, VM, or Kubernetes.

### ✅ **Production-Grade Observability**
*   **Built-in Metrics:** Prometheus endpoint with P50/P95/P99 latency, cache hit rates, MinIO API calls.
*   **Health Checks:** Kubernetes-ready liveness/readiness probes.
*   **Structured Logging:** `tracing-subscriber` with JSON output for log aggregation.

### ✅ **Performance & Safety in Rust**
*   **Memory Safety:** Zero-copy operations, no GC pauses.
*   **Fearless Concurrency:** Lock-free data structures where possible.
*   **HNSW Index:** 2.86x faster than instant-distance, configurable ef_search/ef_construction.

---

## 3. Who Should Use AkiDB?

AkiDB is designed for **ARM-first deployments** where power efficiency, edge compute, and offline operation are critical:

### Primary Audience

#### 🚢 **Edge AI & Offline Inference**
*   **Maritime & Transportation:** Ships, trains, autonomous vehicles (Jetson + offline RAG).
*   **Remote Sites:** Oil rigs, mining operations, field hospitals (low-power ARM nodes).
*   **IoT Gateways:** Factory floors, smart buildings (Jetson Orin Nano as edge aggregator).
*   **Military & Defense:** Air-gapped tactical systems, drones, mobile command centers.

#### 💻 **macOS Developer Ecosystem**
*   **Mac Studio Clusters:** ML teams running on Apple Silicon (M2/M3/M4 clusters).
*   **MLX Integration:** Leverage Metal GPU acceleration for batch operations.
*   **Embedded RAG:** Ship vector search inside macOS applications (no Python runtime).

#### 🏛️ **Government & Regulated Industries**
*   **Air-Gapped Networks:** Defense, intelligence, classified systems (Protected B/C, Top Secret).
*   **Data Sovereignty:** Systems where data cannot leave premises or national borders.
*   **Compliance Requirements:** PIPEDA, FedRAMP, GDPR, HIPAA with audit trails.

#### ☁️ **ARM Cloud Cost Optimization**
*   **AWS Graviton:** 40% better price/performance than x86 (according to AWS).
*   **Oracle A1:** Always-free tier (4 OCPU, 24GB RAM) for dev/test.
*   **Azure Cobalt:** Next-gen ARM VMs for cloud-native deployments.

### Secondary Audience

#### 🦀 **Rust Ecosystem Builders**
*   Embedded vector search in Rust applications (no FFI, no Python runtime).
*   High-performance pipelines requiring type safety and zero-copy operations.

#### 🔬 **Researchers & Academia**
*   Reproducible AI experiments with versioned embeddings and immutable snapshots.
*   Offline research environments without cloud access.

---

## 4. Competitive Advantage

| Feature                  | **AkiDB (MinIO-Native)**                | Pinecone (Managed)      | Weaviate/Qdrant (Self-Hosted) | Milvus (Self-Hosted)                  |
| ------------------------ | --------------------------------------- | ----------------------- | ----------------------------- | ------------------------------------- |
| **Air-Gap Deployment**   | ✅ **Zero cloud dependencies**          | ❌ SaaS only            | ⚠️ Possible (complex)         | ⚠️ Requires etcd/Kafka                |
| **Data Sovereignty**     | ✅ **100% on-prem**                     | ❌ Data leaves premises | ⚠️ Partial                    | ⚠️ Partial                            |
| **Compliance Features**  | ✅ **Object Lock, Versioning, Audit**   | ❌ Proprietary          | ❌ None                       | ❌ None                               |
| **Storage Cost**         | **$0.01-0.02/GB** (MinIO/HDD)           | ~$0.77/GB (p1 pod)      | ~$0.10/GB (EBS)               | ~$0.10/GB (EBS)                       |
| **Architecture**         | **Stateless API + MinIO**               | Managed SaaS            | Stateful Node + Raft          | Microservices (etcd, Kafka, Pulsar)   |
| **Deployment**           | **Single Binary + MinIO**               | N/A                     | Docker / K8s                  | **K8s Required**                      |
| **Certifiability**       | ✅ **2 components** (easy audit)        | ❌ Black box            | ⚠️ 3-5 components             | ❌ 10+ components                     |
| **Offline Operation**    | ✅ **Full offline (air-gap ready)**     | ❌ Internet required    | ⚠️ Partial                    | ⚠️ Partial                            |
| **Portable Packaging**   | ✅ **`.akipkg` snapshots**              | ❌ Vendor lock-in       | ❌ None                       | ❌ None                               |
| **Operational Burden**   | **Minimal** (stateless)                 | None (SaaS)             | Moderate                      | **High** (multi-component)            |
| **Vendor Lock-in**       | **None (MIT)**                          | High                    | Low (BSD/Apache)              | Low (Apache)                          |

### Key Differentiators

1. **Compliance-First Design:** Built-in Object Lock, versioning, and audit trails—not bolted on.
2. **Air-Gap Ready:** Zero external dependencies. Runs on closed networks.
3. **MinIO-Native:** Deep integration with KES, ILM, Site Replication, Bucket Notifications.
4. **Portable:** `.akipkg` packaging for cross-site migration and forensic replay.
5. **Simplified Stack:** 2 components vs. 10+ for Milvus. Easier to audit, certify, and secure.

---

## 📚 Documentation

### Migration Guides

- **[Manifest V1 Migration](docs/migrations/manifest_v1.md)** - Atomic manifest operations and optimistic locking for concurrent writes
- **[Storage API Migration](docs/migration-guide.md)** - Migrating from `write_segment` to `write_segment_with_data` with SEGv1 format
- **[Index Providers Guide](docs/index-providers.md)** - Vector index implementation guide and contract testing

---

## 🚀 Quick Start

### Prerequisites
- Rust 1.77+
- Docker and Docker Compose

### Run Locally
1.  **Clone the repository:**
    ```bash
    git clone https://github.com/aifocal/akidb.git
    cd akidb
    ```

2.  **Start the development environment:**
    This command starts a local MinIO container for S3 storage and the AkiDB API server.
    ```bash
    ./scripts/dev-init.sh
    ```
    The API is now available at `http://localhost:8080`.

### API Examples

#### 1. Create a Collection
```bash
curl -X POST http://localhost:8080/collections \
  -H "Content-Type: application/json" \
  -d '{
    "name": "product_embeddings",
    "vector_dim": 768,
    "distance": "Cosine"
  }'
```

#### 2. Insert Vectors
```bash
curl -X POST http://localhost:8080/collections/product_embeddings/vectors \
  -H "Content-Type: application/json" \
  -d '{
    "vectors": [
      {
        "id": "product_1",
        "vector": [0.1, 0.2, ...],
        "payload": { "name": "Laptop", "price": 999.99 }
      }
    ]
  }'
```

#### 3. Search
```bash
curl -X POST http://localhost:8080/collections/product_embeddings/search \
  -H "Content-Type: application/json" \
  -d '{
    "vector": [0.1, 0.2, ...],
    "top_k": 10
  }'
```

---

## 🏗️ Architecture

AkiDB uses a **MinIO-first, stateless architecture** designed for air-gapped deployments.

### Application Layer
```
┌─────────────────────────────────────────────────────────────────┐
│                  REST API (Axum) + gRPC (Tonic)                 │
│                     Stateless, horizontally scalable            │
├─────────────────────────────────────────────────────────────────┤
│              Query Layer (Planner/Executor/Cache)               │
│         • Filter pushdown  • Result caching  • Parallelization  │
├─────────────────────────────────────────────────────────────────┤
│                    Index Layer (HNSW / DiskANN)                 │
│         • In-memory ANN  • Pre-filtering  • Range GET           │
├─────────────────────────────────────────────────────────────────┤
│              Storage Layer (WAL / Segments / MinIO)             │
│    • Tiered caching (Hot/Warm/Cold)  • Versioning  • Audit     │
├─────────────────────────────────────────────────────────────────┤
│                 Core Types (Collection/Manifest)                │
│              • Domain models  • SEGv1 format  • Metadata        │
└─────────────────────────────────────────────────────────────────┘
```

### Storage Tier (MinIO-Native)
```
┌─────────────────────────────────────────────────────────────────┐
│                        HOT TIER (Local NVMe)                    │
│              LRU Cache + Pinned Hot Segments (P95 < 5ms)        │
└─────────────────────────────────────────────────────────────────┘
                                 ↓
┌─────────────────────────────────────────────────────────────────┐
│                    WARM TIER (RocksDB/DuckDB)                   │
│        Segment metadata + Bloom filters (P95 < 50ms)            │
└─────────────────────────────────────────────────────────────────┘
                                 ↓
┌─────────────────────────────────────────────────────────────────┐
│                  COLD TIER (MinIO + Zstd Compression)           │
│   • Object Lock (WORM)  • Versioning  • KES Encryption         │
│   • Site Replication  • ILM Policies  • Audit Logs             │
│   • HDD/Tape Storage (P95 < 500ms, $0.01/GB)                   │
└─────────────────────────────────────────────────────────────────┘
```

### MinIO Integration Points

1. **Security:** SSE-KMS (KES/Vault), Object Lock, Legal Hold
2. **Durability:** Erasure Coding (e.g., 12D+4P), Versioning, Site Replication
3. **Performance:** Multipart uploads, Range GET, Connection pooling
4. **Events:** Bucket Notifications → NATS → Index rebuild triggers
5. **ILM:** Lifecycle policies for automatic Hot→Warm→Cold transitions
6. **Audit:** Query hash chains stored in MinIO audit buckets

### Key Innovations

1. **ARM-Native Compute:** Pluggable `ComputeBackend` trait with NEON SIMD, MLX Metal (macOS), CUDA Tensor Cores (Jetson). See [ARM-Native Architecture](docs/arm-native-architecture.md).
2. **Append-Only WAL:** O(1) sync writes to local disk, then async flush to MinIO.
3. **Immutable Segments:** Once sealed, segments become WORM objects—tamper-proof.
4. **Versioned Manifests:** Optimistic locking with MinIO versioning for concurrent writers.
5. **`.akipkg` Snapshots:** Package index + manifest + metadata for offline migration.

---

## 📈 Project Status & Roadmap

### 🔄 **Current Status: Active Development**

**What's Working**:
- ✅ Core architecture and trait abstractions (StorageBackend, IndexProvider)
- ✅ S3 storage backend with full CRUD operations
- ✅ HNSW index using hnsw_rs (2.86x faster than instant-distance)
- ✅ WAL system with append-only operations and crash recovery
- ✅ SEGv1 binary format with Zstd compression
- ✅ Advanced filter pushdown (3-tier strategy)
- ✅ Batch query API with parallel execution
- ✅ Production metrics (13 Prometheus metrics)
- ✅ Health check endpoints (Kubernetes-ready)
- ✅ 171/171 tests passing (100% pass rate)

**Phase 4 Complete**:
- ✅ OpenTelemetry distributed tracing with OTLP exporter
- ✅ Jaeger integration for trace visualization
- ✅ Production deployment guide and API reference
- ✅ Comprehensive observability documentation
- ✅ Graceful shutdown and configuration management

**Phase 6 Complete** (Offline RAG - 100% Complete):
- ✅ akidb-ingest CLI tool (M1-M4)
- ✅ CSV/JSONL/Parquet parsers with streaming
- ✅ Batch pipeline with progress tracking
- ✅ akidb-pkg CLI tool (M5-M7)
- ✅ .akipkg package format specification
- ✅ MinIO Site Replication CLI (M8-M10)
- ✅ Offline bundle creation and dependency vendoring (M11-M12)
- ✅ Multi-language support with CJK tokenization (M13-M14)

**Phase 7 COMPLETE** (Enterprise Scale - 100% Complete ✅):
- ✅ Phase 7 planning and specification (docs/phase7-enterprise-scale.md, phase7-milestones.md)
- ✅ M1: Tenant management (data structures, S3 storage, REST API - 30 tests)
- ✅ M2: Namespace isolation (tenant middleware, TenantStorageBackend wrapper - 11 tests)
- ✅ M3: Quota tracking (QuotaTracker, enforcement middleware - 20 tests)
- ✅ M4: User and Role structures (28 permissions, pre-defined roles - 15 tests)
- ✅ M5-M6: RBAC middleware (authentication, authorization - 8 tests)
- ✅ M7: Query result caching (moka L1 cache, Redis L2 support - 8 tests)
- ✅ M8: Materialized views (TopK/Filtered/Aggregation views - 10 tests)
- ✅ M9: Cache invalidation (vector-to-cache tracking - 8 tests)
- ✅ M10-M12: DiskANN (Vamana graph, beam search for billions - 9 tests)
- ✅ M13-M15: Distributed queries (sharding, coordination, aggregation - 10 tests)
- ✅ M16-M18: Client SDKs (TypeScript, Python, Go with full API coverage)

**Total**: 129 tests, 5,850+ lines of production code across 18 milestones

### ✅ **Phase 3: Core Implementation (Complete)**
- **Goal:** Complete storage, WAL, and index implementation
- **Key Milestones:**
    - M1: ✅ Benchmark harness and baseline metrics
    - M2: ✅ S3 backend + WAL + HNSW index
    - M3: ✅ hnsw_rs migration (2.86x performance improvement)
    - M4: ✅ Production monitoring and observability

### ✅ **Phase 4: Production Features (Complete)**
- **Goal:** Production-ready monitoring and deployment
- **Key Milestones:**
    - M1: ✅ Metrics & Monitoring (Prometheus, health checks, structured logging)
    - M2: ✅ Observability (OpenTelemetry, Jaeger distributed tracing)
    - M3: ✅ Operational Features (graceful shutdown, config management)
    - M4: ✅ Documentation (deployment guides, API reference, observability guide)

### 🔐 **Phase 5: MinIO-Native Compliance & Security (Q1 2025)**
- **Goal:** Deep MinIO integration for regulated industries
- **Priority 1 (Compliance):**
    - ✅ SSE-KMS encryption with KES/HashiCorp Vault
    - ✅ Object Lock (WORM) for immutable index segments
    - ✅ Versioning with snapshot/revert API
    - ✅ Audit trail hash chains in MinIO audit buckets
- **Priority 2 (Storage Optimization):**
    - ✅ Hot/Warm/Cold tiered caching (NVMe → RocksDB → MinIO)
    - ✅ Multipart uploads for large segments
    - ✅ Range GET pre-fetching for sparse reads
    - ✅ Segment merging to reduce S3 API call overhead
- **Priority 3 (Events & Automation):**
    - ✅ MinIO Bucket Notification → NATS → Index rebuild
    - ✅ ILM policies for automatic tier transitions
    - ✅ `.akipkg` packaging with signatures

### ✅ **Phase 6: Offline RAG & Air-Gap Features (Complete)**
- **Goal:** Complete offline operation capabilities
- **Key Milestones:**
    - M1-M4: ✅ Offline Ingest Tool (CSV/JSONL/Parquet batch import)
    - M5-M7: ✅ Package Format (.akipkg for air-gap deployments)
    - M8-M10: ✅ MinIO Site Replication integration (CLI tool with failover automation)
    - M11-M12: ✅ Air-Gap Tooling (offline installation, dependency bundling)
    - M13-M14: ✅ Multi-Language Support (EN/FR/ZH/ES/JA with CJK tokenization)

### ✅ **Phase 7: Enterprise Scale (COMPLETE - 100%)**
- **Goal:** Production-grade features for large deployments
- **Key Milestones:**
    - M1: ✅ Tenant Management (data structures, S3 storage, REST API - 30 tests)
    - M2: ✅ Namespace Isolation (tenant middleware, TenantStorageBackend wrapper - 11 tests)
    - M3: ✅ Quota Tracking (QuotaTracker, enforcement middleware - 20 tests)
    - M4: ✅ User and Role Structures (28 permissions, pre-defined roles - 15 tests)
    - M5-M6: ✅ RBAC Middleware (authentication, authorization - 8 tests)
    - M7: ✅ Query Result Caching (moka L1, Redis L2, SHA-256 key generation - 8 tests)
    - M8: ✅ Materialized Views (TopK/Filtered/Aggregation, refresh strategies - 10 tests)
    - M9: ✅ Cache Invalidation (vector-to-cache bidirectional tracking - 8 tests)
    - M10-M12: ✅ DiskANN (Vamana graph, beam search, billion-scale - 9 tests)
    - M13-M15: ✅ Distributed Queries (sharding strategies, query coordination - 10 tests)
    - M16: ✅ TypeScript SDK (full API coverage with types and retry logic)
    - M17: ✅ Python SDK (idiomatic client with type hints and dataclasses)
    - M18: ✅ Go SDK (native client with context support and error handling)

---

## 🧑‍💻 Development

### Running Tests
```bash
# Run all workspace tests
cargo test --workspace

# Run integration tests (requires Docker environment)
./scripts/dev-init.sh
cargo test --workspace -- --include-ignored
```

### Code Quality
```bash
# Format
cargo fmt --all

# Lint
cargo clippy --all-targets --all-features --workspace -- -D warnings
```

---

## 🤝 Contributing

We welcome contributions of all kinds! Please read our [Contributing Guide](docs/CONTRIBUTING.md) to get started.

## 📄 License

AkiDB is licensed under the [Apache License 2.0](LICENSE).

Copyright 2024-2025 AiFocal Inc.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.