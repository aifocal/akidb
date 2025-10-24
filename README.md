# AkiDB

[![CI](https://github.com/defai-digital/akidb/workflows/CI/badge.svg)](https://github.com/defai-digital/akidb/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**AkiDB: The S3-Native Vector Database for the AI Era.**

AkiDB is a high-performance, open-source vector database built from the ground up for S3-compatible storage. Written in Rust, it's designed to make vector search affordable, simple, and scalable for modern AI applications.

---

## 1. Why AkiDB Matters

In the age of AI, **vector databases are critical infrastructure**. Every AI application—from semantic search to RAG systems to recommendation engines—relies on efficient similarity search over high-dimensional embeddings.

However, existing solutions force a difficult choice:

*   **Managed Services (e.g., Pinecone):** Convenient but expensive, with costs that scale unpredictably. They create vendor lock-in with proprietary APIs and give you little control over your own data.
*   **Self-Hosted Solutions (e.g., Milvus, Weaviate):** Open-source but notoriously complex. They require extensive Kubernetes knowledge and a fleet of microservices (like etcd, MinIO), turning database management into a full-time job.

**AkiDB offers a third way.** By embracing a **true S3-native architecture**, we solve the core problems of cost and complexity.

## 2. Core Value Proposition

AkiDB's design philosophy is simple: use the right tool for the job. For durable, scalable, and cost-effective storage, nothing beats object storage like S3.

#### ✅ **80%+ Cost Reduction**
AkiDB leverages S3 as its primary storage layer. Instead of paying for expensive, always-on block storage or memory, you pay S3's low commodity rates ($0.023/GB). This fundamentally changes the cost equation for large-scale vector search.

*   **Example:** Storing 10 million `text-embedding-ada-002` vectors (1536-dim) costs:
    *   **Pinecone (p1.x1 pod):** ~$70/month
    *   **AkiDB on S3:** ~$1.50/month (storage) + stateless compute

#### ✅ **Radical Simplicity**
Our architecture consists of two components: a **stateless API server** and an **S3 bucket**. That's it.
*   **No Raft Consensus:** We offload state management to S3, eliminating the need for complex coordination protocols.
*   **No Sidecars:** No need to manage separate clusters for etcd, MinIO, or message queues.
*   **Deploy in Minutes:** A single Docker container is all you need to get started.

#### ✅ **Open Source & No Vendor Lock-in**
AkiDB is MIT licensed. Your data, your infrastructure, your control.
*   **Use Any S3 Provider:** AWS S3, Google Cloud Storage, Cloudflare R2, MinIO.
*   **Transparent:** Audit every line of code. The roadmap is shaped by the community.

#### ✅ **Performance & Safety in Rust**
Built in Rust, AkiDB provides the trifecta of performance, memory safety, and fearless concurrency. This means fewer bugs, predictable performance, and a smaller operational footprint.

#### ✅ **Composable Architecture**
AkiDB's plugin-based design lets you choose your own:
*   **Index Provider:** Native brute-force, HNSW, or bring your own (FAISS, etc.)
*   **Storage Backend:** S3, Google Cloud Storage, Cloudflare R2, or MinIO
*   **Clear Trait Abstractions:** Easy to understand, extend, and customize

This prevents vendor lock-in and allows seamless integration with your existing infrastructure.

---

## 3. Who Should Use AkiDB?

AkiDB is designed for teams who value **simplicity, cost-efficiency, and control**:

### Primary Audience
*   **AI Application Builders** 🤖
    - Building RAG systems, semantic search, or recommendation engines
    - Need vector search but don't want to become database administrators
    - Using Python/TypeScript/Rust tech stacks on AWS/GCP/Azure

*   **Cost-Conscious Startups** 💰
    - Scaling AI products without exponential infrastructure costs
    - Budget-conscious teams ($100-$1000/month range)
    - Want predictable pricing based on S3 storage + compute

### Secondary Audience
*   **Rust Enthusiasts** 🦀
    - Building high-performance systems in Rust
    - Need embedded vector search without FFI overhead
    - Value type safety and zero-cost abstractions

*   **Enterprise Teams** 🏢
    - Require full data sovereignty and control
    - Need customizable, composable architectures
    - Deploy on private infrastructure or multi-cloud environments

---

## 4. Competitive Advantage

| Feature                | AkiDB (S3-Native)                               | Pinecone (Managed)      | Weaviate / Qdrant (Self-Hosted) | Milvus (Self-Hosted)                  |
| ---------------------- | ----------------------------------------------- | ----------------------- | ------------------------------- | ------------------------------------- |
| **Architecture**       | **Stateless API + S3**                          | Managed SaaS            | Stateful Node + Raft            | Microservices Cluster                 |
| **Storage Cost**       | **$0.023/GB** (S3)                              | ~$0.77/GB (p1 pod)      | ~$0.10/GB (EBS)                 | ~$0.10/GB (EBS)                       |
| **Deployment**         | **Single Docker Container**                     | N/A                     | Docker / K8s                    | **K8s Required** (etcd, MinIO, etc.)  |
| **Vendor Lock-in**     | **None (MIT)**                                  | High                    | Low (BSD/Apache)                | Low (Apache)                          |
| **Operational Burden** | **Minimal**                                     | None                    | Moderate                        | **High**                              |

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
    git clone https://github.com/defai-digital/akidb.git
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

AkiDB uses a clean, layered architecture designed for simplicity and performance.

```
┌─────────────────────────────────────────┐
│         REST API (Axum)                 │  ← Stateless, horizontally scalable
├─────────────────────────────────────────┤
│   Query Layer (Planner/Executor)        │  ← Query optimization & filtering
├─────────────────────────────────────────┤
│     Index Layer (HNSW)                  │  ← In-memory ANN search
├─────────────────────────────────────────┤
│ Storage Layer (WAL / Segments / S3)     │  ← S3-native persistence & recovery
├─────────────────────────────────────────┤
│    Core Types (Collection/Manifest)     │  ← Domain models
└─────────────────────────────────────────┘
```

The key innovation is in the **Storage Layer**. Data is written to a Write-Ahead Log (WAL) for durability and buffered. Once a certain size is reached, vectors are compressed into an immutable **Segment** file and uploaded to S3. The WAL is then truncated. This design combines the low latency of local writes with the cost-effectiveness and durability of S3.

---

## 📈 Project Status & Roadmap

### 🔄 **Current Status: Active Development**

**What's Working**:
- ✅ Core architecture and trait abstractions (StorageBackend, IndexProvider)
- ✅ S3 storage integration with object_store crate
- ✅ Development environment (Docker + MinIO)
- ✅ Basic API endpoints (collections, insert, search)
- ✅ 21 tests passing (validation, bootstrap, e2e flows)

**In Progress** (Phase 3 M2):
- 🔄 S3 Storage Backend - Full implementation of core methods
- 🔄 WAL Operations - Crash-safe append/replay
- 🔄 Index Provider - Wire native index to storage layer
- 🔄 Production readiness - Integration tests and observability

### ⏳ **Phase 3: Core Implementation (In Progress)**
- **Goal:** Complete storage, WAL, and index implementation
- **Key Milestones:**
    - M1: ✅ Benchmark harness and baseline metrics
    - M2: 🔄 S3 backend + WAL + Index provider (current)
    - M3: Query planner optimizations
    - M4: Production monitoring and observability

### 🚀 **Phase 4: Cloud-Native Differentiation (Q1 2025)**
- **Goal:** Establish competitive advantages in cloud-native vector search
- **Key Initiatives:**
    - **S3 Optimization Layer:** Smart caching, S3 Select integration, lifecycle management
    - **Distributed Query (MVP):** Basic sharding and query routing for 10M+ vectors
    - **Zero-Ops Deployment:** Terraform modules, Kubernetes Helm charts, one-click AWS/GCP deployment
    - **Rust SDK & Ecosystem:** LangChain, LlamaIndex, Hugging Face integrations

### 🌐 **Phase 5: Scale & Enterprise (Q2 2025+)**
- **Goal:** Production-grade features for enterprise adoption
- **Key Initiatives:**
    - Multi-tenancy and RBAC
    - Hybrid search (vectors + metadata filtering)
    - Multi-language clients (Python, TypeScript, Go)
    - Advanced observability with OpenTelemetry

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