# AkiDB

[![CI](https://github.com/defai-digital/akidb/workflows/CI/badge.svg)](https://github.com/defai-digital/akidb/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A high-performance distributed vector database written in Rust, designed for similarity search and vector operations with S3-compatible storage backend.

## Features

- **S3-Compatible Storage**: Built on object storage for scalability and durability
- **SEGv1 Binary Format**: Efficient vector storage with Zstd compression (~60% size reduction)
- **Data Integrity**: XXH3 and CRC32C checksum validation
- **Write-Ahead Log**: Ensures data durability and crash recovery
- **REST API**: Complete HTTP API for vector operations
- **High Performance**: 2ms to process 1000×128 dimensional vectors
- **Modular Architecture**: Clean separation of storage, indexing, and query layers

## Quick Start

### Prerequisites

- Rust 1.77 or later
- Docker and Docker Compose (for local development)

### Installation

1. Clone the repository:
```bash
git clone https://github.com/defai-digital/akidb.git
cd akidb
```

2. Start the development environment:
```bash
./scripts/dev-init.sh
```

This will start:
- MinIO (S3-compatible storage) on http://localhost:9000
- AkiDB API server on http://localhost:8080

### Usage Example

#### Create a collection:
```bash
curl -X POST http://localhost:8080/collections \
  -H "Content-Type: application/json" \
  -d '{
    "name": "products",
    "vector_dim": 128,
    "distance": "Cosine"
  }'
```

#### Insert vectors:
```bash
curl -X POST http://localhost:8080/collections/products/vectors \
  -H "Content-Type: application/json" \
  -d '{
    "vectors": [
      {
        "id": "product_1",
        "vector": [0.1, 0.2, ...],
        "payload": {"name": "Product A", "price": 99.99}
      }
    ]
  }'
```

#### Search similar vectors:
```bash
curl -X POST http://localhost:8080/collections/products/search \
  -H "Content-Type: application/json" \
  -d '{
    "vector": [0.1, 0.2, ...],
    "top_k": 10
  }'
```

## Architecture

AkiDB is built with a layered architecture:

```
┌─────────────────────────────────────────┐
│           REST API (Axum)               │
├─────────────────────────────────────────┤
│      Query Layer (Planner/Executor)     │
├─────────────────────────────────────────┤
│       Index Layer (HNSW/Native)         │
├─────────────────────────────────────────┤
│   Storage Layer (S3/WAL/SEGv1 Format)   │
├─────────────────────────────────────────┤
│      Core Types (Collection/Segment)    │
└─────────────────────────────────────────┘
```

### Components

- **akidb-core**: Core data types and schemas
- **akidb-storage**: Storage backend abstraction with S3 implementation
- **akidb-index**: ANN index providers (HNSW)
- **akidb-query**: Query planning and execution engine
- **akidb-api**: REST API server
- **akidb-mcp**: Cluster management and coordination

## Development

### Running Tests

```bash
# Run all tests
cargo test --workspace

# Run tests for a specific package
cargo test -p akidb-storage

# Run with logging
RUST_LOG=debug cargo test
```

### Building

```bash
# Development build
cargo build --workspace

# Release build
cargo build --workspace --release

# Or use the build script
./scripts/build-release.sh
```

### Code Quality

```bash
# Format code
cargo fmt --all

# Run linter
cargo clippy --all-targets --all-features --workspace -- -D warnings

# Run all checks (format + clippy + test)
./scripts/dev-test.sh
```

## SEGv1 Binary Format

AkiDB uses a custom binary format for efficient vector storage:

```
┌─────────────────────────────────────────┐
│ Header (64 bytes)                       │
│ - Magic: b"SEGv"                        │
│ - Version: 1                            │
│ - Dimension, Vector Count               │
│ - Offsets for extensibility             │
├─────────────────────────────────────────┤
│ Vector Data Block                       │
│ - Zstd compressed                       │
│ - ~60% compression ratio                │
├─────────────────────────────────────────┤
│ Footer (32 bytes)                       │
│ - XXH3/CRC32C checksum                  │
└─────────────────────────────────────────┘
```

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Health check |
| GET | `/collections` | List all collections |
| POST | `/collections` | Create a collection |
| GET | `/collections/:name` | Get collection info |
| DELETE | `/collections/:name` | Delete a collection |
| POST | `/collections/:name/vectors` | Insert vectors |
| POST | `/collections/:name/search` | Search similar vectors |

## Configuration

Create a `.env` file from `.env.example`:

```bash
cp .env.example .env
```

Key configuration options:
- `AKIDB_API_PORT`: API server port (default: 8080)
- `MINIO_ENDPOINT`: S3 endpoint (default: http://localhost:9000)
- `MINIO_ACCESS_KEY`: S3 access key
- `MINIO_SECRET_KEY`: S3 secret key

## Performance

Benchmarks on M1/M2 MacBook:

| Vectors | Dimensions | Operation | Time |
|---------|------------|-----------|------|
| 1,000 | 128 | Serialize (SEGv1) | 2ms |
| 1,000 | 768 | Serialize (SEGv1) | 10ms |
| 10,000 | 128 | Serialize (SEGv1) | 20ms |
| 500 | 128 | Insert via API | <100ms |

Compression ratio: ~60% (Zstd level 3)

## Roadmap

### Phase 1 (Current) ✅
- [x] Core types and schemas
- [x] S3 storage backend
- [x] SEGv1 binary format
- [x] WAL implementation
- [x] REST API
- [x] E2E test suite

### Phase 2 (Planned)
- [ ] Metadata block (Arrow IPC)
- [ ] Bitmap index (Roaring)
- [ ] HNSW graph storage
- [ ] Multipart upload for large segments
- [ ] Query filters and pagination

### Phase 3 (Future)
- [ ] Distributed coordination
- [ ] Replication and sharding
- [ ] gRPC API
- [ ] Advanced index types
- [ ] Performance optimizations

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](docs/CONTRIBUTING.md) for guidelines.

## Testing

Test coverage: 44 tests passing
- akidb-api: 18 tests (including 5 E2E tests)
- akidb-storage: 19 tests (including 6 SEGv1 format tests)
- akidb-index: 3 tests
- akidb-query: 4 tests

4 WAL integration tests are ignored by default (require MinIO):
```bash
# Run all tests including ignored ones
docker compose up -d
cargo test --workspace -- --include-ignored
```

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

Built with:
- [Rust](https://www.rust-lang.org/) - Systems programming language
- [Axum](https://github.com/tokio-rs/axum) - Web framework
- [object_store](https://github.com/apache/arrow-rs/tree/master/object_store) - Object storage abstraction
- [Zstd](https://facebook.github.io/zstd/) - Compression algorithm
- [xxHash](https://github.com/Cyan4973/xxHash) - Fast hash algorithm

## Support

- Issues: https://github.com/defai-digital/akidb/issues
- Documentation: https://github.com/defai-digital/akidb/tree/main/docs

---

Built with ❤️ in Rust
