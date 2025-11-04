# AkiDB Architecture

AkiDB is designed with a **MinIO-first, stateless architecture** optimized for ARM-native platforms and air-gapped deployments. This architecture prioritizes simplicity, scalability, and data sovereignty.

## Core Principles

-   **Stateless Application Layer:** The API servers are stateless, allowing for easy horizontal scaling. All persistent state is managed in the storage layer.
-   **S3 as the Source of Truth:** AkiDB uses an S3-compatible object store (like MinIO) as its primary data store. This leverages the durability, scalability, and cost-effectiveness of object storage.
-   **ARM-Native Focus:** The entire system is designed and optimized for ARM-based hardware, from Apple Silicon to NVIDIA Jetson and ARM cloud servers.

## Application Layer

The application layer is responsible for handling API requests, planning and executing queries, and managing the vector index.

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

## Storage Layer

AkiDB employs a tiered storage strategy to balance performance and cost.

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

### Key Components

-   **Write-Ahead Log (WAL):** All incoming writes are first appended to a WAL on local disk for durability. The WAL is then asynchronously flushed to the S3 backend.
-   **Segments:** Vectors are grouped into immutable segments, which are stored as objects in S3. This simplifies data management and allows for efficient versioning and caching.
-   **Manifest:** Each collection has a manifest file that keeps track of the collection's segments and other metadata. The manifest is also stored in S3 and is updated atomically.

## Indexing

AkiDB uses a pluggable indexing architecture, with HNSW (Hierarchical Navigable Small World) as the default Approximate Nearest Neighbor (ANN) index.

-   **In-Memory Index:** The HNSW index is held in memory for fast searching.
-   **Pre-filtering:** AkiDB can apply filters to narrow down the search space before performing the vector search, improving performance.

## Multi-Tenancy

Tenant data is isolated at the storage layer. Each tenant has its own set of collections, and all of a tenant's data is stored under a unique prefix in the S3 bucket. Quotas are enforced at the API layer.
