# AkiDB

[![Rust CI](https://github.com/aifocal/akidb/actions/workflows/ci.yml/badge.svg)](https://github.com/aifocal/akidb/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/aifocal/akidb)](https://github.com/aifocal/akidb/releases)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![Rust Version](https://img.shields.io/badge/rust-1.77%2B-orange.svg)](https://www.rust-lang.org)

**AkiDB is an ARM-native, S3-backed vector database optimized for offline semantic retrieval and edge AI.**

Built in Rust, AkiDB is designed for high-performance, power-efficient deployments on ARM platforms like Apple Silicon, NVIDIA Jetson, and ARM-based cloud servers. It's the ideal choice for air-gapped environments, edge inference, and offline RAG applications where data sovereignty and low power consumption are critical.

---

## Key Features

- **ARM-Native Performance:** Optimized for Apple Silicon (M-series chips), NVIDIA Jetson, and AWS Graviton.
- **S3-Backed Storage:** Uses MinIO or any S3-compatible object store as its primary storage backend for cost-effective, scalable, and durable storage.
- **Offline-First:** Designed to run entirely in air-gapped environments with no external dependencies.
- **Multi-Tenancy:** Isolate data and manage quotas for different users and applications.
- **Write-Ahead Logging (WAL):** Ensures data durability and crash recovery.
- **HNSW Indexing:** Fast and accurate approximate nearest neighbor (ANN) search.
- **Query Caching:** Speeds up frequently executed queries.
- **Security:** Released with critical security fixes in v1.1.0.

## Documentation

- **[Getting Started](./docs/getting-started.md)**: Install and run AkiDB for the first time.
- **[API Reference](./docs/api-reference.md)**: Detailed REST API documentation.
- **[Architecture](./docs/architecture.md)**: A high-level overview of AkiDB\'s architecture.
- **[Multi-Tenancy](./docs/multi-tenancy.md)**: Learn about tenant isolation and quota management.
- **[Security](./docs/security.md)**: Understand AkiDB\'s security features and best practices.
- **[Deployment](./docs/deployment.md)**: A guide to deploying AkiDB in production.

## Quick Start

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

### Basic Usage

#### 1. Create a Collection

```bash
curl -X POST http://localhost:8080/collections \
  -H "Content-Type: application/json" \
  -d 
{
    "name": "product_embeddings",
    "vector_dim": 4,
    "distance": "Cosine"
  }
```

#### 2. Insert Vectors

```bash
curl -X POST http://localhost:8080/collections/product_embeddings/vectors \
  -H "Content-Type: application/json" \
  -d 
{
    "vectors": [
      {
        "id": "product_1",
        "vector": [0.1, 0.2, 0.3, 0.4],
        "payload": { "name": "Laptop", "price": 999.99 }
      },
      {
        "id": "product_2",
        "vector": [0.5, 0.6, 0.7, 0.8],
        "payload": { "name": "Keyboard", "price": 75.00 }
      }
    ]
  }
```

#### 3. Search

```bash
curl -X POST http://localhost:8080/collections/product_embeddings/search \
  -H "Content-Type: application/json" \
  -d 
{
    "vector": [0.1, 0.2, 0.3, 0.4],
    "top_k": 2
  }
```

## Contributing

We welcome contributions! Please read our [Contributing Guide](docs/CONTRIBUTING.md) to get started.

## License

AkiDB is licensed under the [Apache License 2.0](LICENSE).
