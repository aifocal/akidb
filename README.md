# AkiDB

[![Rust CI](https://github.com/aifocal/akidb/actions/workflows/ci.yml/badge.svg)](https://github.com/aifocal/akidb/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust Version](https://img.shields.io/badge/rust-1.77%2B-orange.svg)](https://www.rust-lang.org)
[![GitHub Stars](https://img.shields.io/github/stars/aifocal/akidb?style=social)](https://github.com/aifocal/akidb)
[![GitHub Issues](https://img.shields.io/github/issues/aifocal/akidb)](https://github.com/aifocal/akidb/issues)
[![GitHub Release](https://img.shields.io/github/v/release/aifocal/akidb?include_prereleases)](https://github.com/aifocal/akidb/releases)

**AkiDB: The MinIO-Native Vector Database for Sovereign AI & Offline RAG.**

AkiDB is a high-performance, open-source vector database built from the ground up for MinIO and S3-compatible storage. Written in Rust, it's designed for **air-gapped deployments, data sovereignty, and auditable offline RAG systems** where cost, compliance, and control matter most.

---

## 1. Why AkiDB Matters

In regulated industries and sovereign AI deployments, **vector databases face unique challenges**:

*   **Managed Services (Pinecone, Weaviate Cloud):** Cannot operate in air-gapped networks. Data leaves your premises, violating sovereignty requirements. Costs scale unpredictably.
*   **Cloud-First Solutions (Milvus, Weaviate self-hosted):** Designed for cloud, not offline. Complex multi-component stacks (etcd, Kafka, etc.) are hard to audit and certify. No built-in compliance features.
*   **Embedding-Only Libraries:** Lack versioning, auditability, ILM, and production-grade durability.

**AkiDB offers a sovereign alternative.** Built for **MinIO-native offline deployments**, it brings:

*   **Air-Gap Ready:** Zero cloud dependencies. Runs entirely on your infrastructure.
*   **Auditable & Compliant:** Object Lock, versioning, audit trails. Meets Protected B / Confidential requirements.
*   **Cost-Optimized:** MinIO cold storage + tiered caching = 90%+ cost reduction vs. cloud vector DBs.
*   **Portable:** Package indices as `.akipkg` for cross-site migration and forensic replay.

## 2. Core Value Proposition

AkiDB is built for **MinIO-first deployments** where sovereignty, auditability, and TCO are non-negotiable.

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
*   **Tiered Caching:** Hot (NVMe cache) → Warm (RocksDB/DuckDB) → Cold (MinIO/Zstd).
*   **Example TCO (10M vectors, 1536-dim):**
    *   **Pinecone p1.x1:** ~$70/month
    *   **AkiDB on MinIO:** ~$0.50/month (storage) + stateless compute

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

AkiDB is designed for **regulated industries, government, and sovereign AI deployments**:

### Primary Audience

#### 🏛️ **Government & Public Sector**
*   **Air-Gapped Networks:** Defense, intelligence, classified systems (Protected B/C, Top Secret).
*   **Data Sovereignty:** Municipal/provincial/federal systems where data cannot leave national borders.
*   **Compliance Requirements:** PIPEDA, FedRAMP, GDPR, HIPAA—need audit trails and WORM storage.
*   **Multi-Language RAG:** Bilingual (EN/FR, ZH/EN) document search for government services.

#### 🏦 **Regulated Industries**
*   **Financial Services:** Trade surveillance, compliance document search, KYC/AML systems.
*   **Healthcare:** Clinical trial data, patient record search (HIPAA/PHIPA compliant).
*   **Legal & Professional Services:** Document discovery, contract analysis, case law search.
*   **Energy & Utilities:** SCADA/OT network isolation, operational document retrieval.

#### 🏭 **Private Infrastructure / On-Prem**
*   **Cost-Conscious Enterprises:** Million+ documents with predictable TCO on commodity hardware.
*   **Multi-Site Deployments:** Branch offices, factories, ships—MinIO site replication + offline sync.
*   **Custom Embedding Models:** Private fine-tuned models, domain-specific embeddings.

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

1. **Append-Only WAL:** O(1) sync writes to local disk, then async flush to MinIO.
2. **Immutable Segments:** Once sealed, segments become WORM objects—tamper-proof.
3. **Versioned Manifests:** Optimistic locking with MinIO versioning for concurrent writers.
4. **`.akipkg` Snapshots:** Package index + manifest + metadata for offline migration.

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

**In Progress** (Phase 4 M2):
- 🔄 OpenTelemetry distributed tracing
- 🔄 Jaeger exporter integration
- 🔄 Query profiling tools
- 🔄 Production deployment automation

### ✅ **Phase 3: Core Implementation (Complete)**
- **Goal:** Complete storage, WAL, and index implementation
- **Key Milestones:**
    - M1: ✅ Benchmark harness and baseline metrics
    - M2: ✅ S3 backend + WAL + HNSW index
    - M3: ✅ hnsw_rs migration (2.86x performance improvement)
    - M4: ✅ Production monitoring and observability

### ⏳ **Phase 4: Production Features (In Progress - 60% Complete)**
- **Goal:** Production-ready monitoring and deployment
- **Key Milestones:**
    - M1: ✅ Metrics & Monitoring (Prometheus, health checks, structured logging)
    - M2: 🔄 Observability (OpenTelemetry, Jaeger) - Current
    - M3: Operational Features (graceful shutdown, config management)
    - M4: Documentation (deployment guides, API reference)

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

### 🌐 **Phase 6: Offline RAG & Air-Gap Features (Q2 2025)**
- **Goal:** Complete offline operation capabilities
- **Key Initiatives:**
    - **Offline Ingest:** CSV/JSONL/Parquet batch import with zero internet
    - **Multi-Site Sync:** MinIO Site Replication integration for DR
    - **Embedding Portability:** Package custom models in `.akipkg`
    - **Air-Gap Tooling:** Offline installation scripts, dependency bundling
    - **Multi-Language Support:** EN/FR/ZH/ES/JA document processing

### 🚀 **Phase 7: Enterprise Scale (Q3 2025+)**
- **Goal:** Production-grade features for large deployments
- **Key Initiatives:**
    - Multi-tenancy with namespace isolation
    - RBAC with MinIO policy integration
    - Advanced query caching and materialized views
    - DiskANN for billion-scale indices
    - Distributed query coordination (sharding)
    - Python/TypeScript/Go client SDKs

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

AkiDB is licensed under the [MIT License](LICENSE).