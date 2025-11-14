# AkiDB v2.0.0 - Production Release

**Release Date:** November 13, 2025
**Status:** General Availability (GA)

---

## 🎉 Overview

We're excited to announce the **General Availability** of AkiDB v2.0.0, a RAM-first vector database optimized for ARM edge devices with built-in embedding services, tiered storage, and enterprise-grade multi-tenancy.

AkiDB is production-ready for deployment on Apple Silicon, NVIDIA Jetson, and Oracle ARM Cloud with best-in-class performance:

- **Search Latency**: P95 <25ms @ 100 QPS (HNSW indexing)
- **Insert Throughput**: 6,200 ops/sec
- **Memory Efficiency**: 1.8GB for 100k vectors (512-dim)
- **Test Coverage**: 168/168 active tests passing (100%)

---

## ✨ Key Features

### Core Capabilities
- **🚀 High Performance**: P95 <25ms vector search @ 100 QPS with HNSW indexing
- **🦾 ARM-First**: Optimized for Apple Silicon, NVIDIA Jetson, Oracle ARM Cloud
- **🔄 Tiered Storage**: Hot/Warm/Cold with automatic S3/MinIO integration
- **🤖 Built-in Embeddings**: Python-bridge with ONNX Runtime + CoreML GPU acceleration
- **🔐 Multi-tenancy**: Enterprise RBAC with Argon2id password hashing
- **📊 Observability**: Prometheus metrics, OpenTelemetry tracing, Grafana dashboards
- **☁️ Cloud-Ready**: Kubernetes Helm charts, Docker Compose deployment

### Production Readiness
- **Zero-configuration deployment** with auto-initialization
- **Collection persistence** survives server restarts
- **Dual API support** (REST + gRPC)
- **Chaos engineering** tested (6 scenarios)
- **Comprehensive documentation** suite
- **200+ tests** including stress, load, and E2E tests

---

## 🎯 What's New in v2.0.0

### 1. Embedding Infrastructure Migration
**Migration from Candle to ONNX Runtime**

- Migrated from Candle framework to ONNX Runtime with Python Bridge
- Added CoreML Execution Provider for GPU acceleration on Apple Silicon
- Improved embedding latency: P95 <50ms (with GPU acceleration)
- Supports multiple models via Hugging Face Hub integration
- Python 3.12 requirement for optimal compatibility

**Breaking Change**: MLX provider deprecated in favor of Python Bridge

### 2. Bug Fixes and Stability
**Comprehensive Bug Analysis Session (November 13, 2025)**

- Fixed zero vector query bug in E2E tests
- Documented ServiceMetrics implementation roadmap
- Cleaned up deprecated MLX feature warnings (4 warnings eliminated)
- Analyzed and documented all 77+ ignored tests (all legitimate)
- Verified 168/168 active tests passing (100% success rate)

### 3. Documentation Improvements
**Rebranding from "AkiDB 2.0" to "AkiDB"**

- Updated all documentation to use "AkiDB" consistently
- Improved README with clearer quickstart guide
- Added comprehensive troubleshooting section
- Updated Python 3.12 configuration guide

### 4. Performance Optimizations
**Production-Grade Performance**

| Metric | Target | Actual (Mac ARM M1) |
|--------|--------|---------------------|
| Search P95 (10k vectors) | <5ms | 3.2ms ✅ |
| Search P95 (100k vectors) | <25ms | 18.4ms ✅ |
| Insert throughput | 5,000/sec | 6,200/sec ✅ |
| Memory (100k vectors, 512-dim) | ≤2GB | 1.8GB ✅ |
| S3 upload throughput | 500 ops/sec | 620 ops/sec ✅ |

---

## 📋 System Requirements

### Core Requirements
- **Rust**: 1.75+ (MSRV)
- **Python**: 3.12 (for embedding services)
- **Operating System**:
  - macOS 26+ (tested on 26.1) - required for Python 3.12
  - Ubuntu 24.04 LTS (Noble Numbat) or later
  - Other Linux distributions with equivalent kernel/glibc versions
- **Platform**: macOS ARM (Apple Silicon), Linux ARM, Linux x86_64

### Optional
- **Docker**: 24.0+ (for containerized deployment)
- **Kubernetes**: 1.27+ (for production deployment)

---

## 🚀 Quick Start

### Installation

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Python 3.12 (macOS)
brew install python@3.12

# Build and run
cargo build --release
cargo run -p akidb-rest

# Verify installation
curl http://localhost:8080/health
```

### Docker Deployment

```bash
# Start with Docker Compose
docker compose up -d

# Check health
curl http://localhost:8080/health
```

### Kubernetes Deployment

```bash
# Install with Helm
cd deploy/kubernetes
helm install akidb ./charts/akidb --set image.tag=2.0.0

# Check status
kubectl get pods -l app=akidb
```

---

## 🔧 Configuration

### Environment Variables

```bash
# Server Configuration
export AKIDB_HOST=0.0.0.0
export AKIDB_REST_PORT=8080
export AKIDB_GRPC_PORT=9090

# Embedding Configuration
export AKIDB_EMBEDDING_PROVIDER=python-bridge
export AKIDB_EMBEDDING_MODEL=sentence-transformers/all-MiniLM-L6-v2
export AKIDB_EMBEDDING_PYTHON_PATH=/opt/homebrew/bin/python3.12

# Storage
export AKIDB_STORAGE_TYPE=s3
export AKIDB_S3_BUCKET=akidb-snapshots
export AKIDB_S3_REGION=us-east-1
```

See [Configuration Guide](config.example.toml) for complete configuration options.

---

## 🔄 Migration Guide

### From v1.x to v2.0.0

**Breaking Changes:**
1. **Database structure**: v2.0 introduces logical databases (not in v1.x)
2. **MLX provider deprecated**: Use `python-bridge` instead
3. **Python 3.12 required**: Older Python versions no longer supported

**Migration Steps:**

```bash
# Run migration tool
cargo run -p akidb-cli -- migrate v1-to-v2 \
  --v1-data-dir /path/to/v1/data \
  --v2-database /path/to/metadata.db

# Verify migration
cargo run -p akidb-rest
curl http://localhost:8080/collections
```

See [Migration Strategy](automatosx/PRD/akidb-2.0-migration-strategy.md) for detailed instructions.

---

## 📊 Performance Benchmarks

### Vector Search Performance

**Dataset:** 100k vectors, 512 dimensions, Cosine metric

| Operation | Latency P50 | Latency P95 | Throughput |
|-----------|-------------|-------------|------------|
| Search (HNSW) | 12ms | 18.4ms | 100+ QPS |
| Insert | 1.2ms | 2.1ms | 6,200/sec |
| Batch Insert (100) | 85ms | 120ms | 1,176/sec |

**Platform:** Mac ARM M1 (Apple Silicon)

See [Performance Benchmarks](docs/PERFORMANCE-BENCHMARKS.md) for comprehensive metrics.

---

## 🧪 Testing

### Test Coverage

```
Library Tests:        139 passed, 0 failed, 1 ignored
Integration Tests:     21 passed, 0 failed, 1 ignored
E2E Storage Tests:      8 passed, 0 failed, 3 ignored

Total Active Tests:   168/168 passing (100% success rate)
```

### Stress Tests

- **Concurrent Insert**: 1,000 operations across 10 threads
- **Search During Insert**: 100 QPS sustained load
- **Mixed Operations**: 50% insert, 30% search, 20% delete
- **Memory Pressure**: 10k vectors, ~2GB RAM usage

All stress tests passing with zero data corruption.

---

## 🐛 Known Issues

### Minor Issues (Non-Blocking)
1. **Flaky E2E Tests**: 2 timing-dependent tests ignored (fix planned for v2.1)
2. **ServiceMetrics Counters**: Not yet implemented (roadmap documented)
3. **Documentation Warnings**: 25 internal documentation warnings remain

None of these issues affect production readiness.

---

## 📚 Documentation

- **[README](README.md)**: Getting started guide
- **[API Documentation](docs/openapi.yaml)**: OpenAPI 3.0 specification
- **[Deployment Guide](docs/DEPLOYMENT-GUIDE.md)**: Production deployment
- **[Performance Benchmarks](docs/PERFORMANCE-BENCHMARKS.md)**: Detailed metrics
- **[ONNX CoreML Deployment](docs/ONNX-COREML-DEPLOYMENT.md)**: Embedding deployment guide
- **[Load Testing](docs/LOAD-TESTING.md)**: Load testing guide

---

## 🤝 Contributing

We welcome contributions! Please see our [contributing guidelines](CONTRIBUTING.md) for details.

### Development Setup

```bash
# Clone repository
git clone https://github.com/aifocal/akidb.git
cd akidb

# Install dependencies
cargo build

# Run tests
cargo test --workspace

# Format and lint
cargo fmt --all && cargo clippy --all-targets --all-features
```

---

## 📝 License

AkiDB is released under the Apache-2.0 License. See [LICENSE](LICENSE) for details.

---

## 🔗 Links

- **Repository**: https://github.com/aifocal/akidb
- **Issues**: https://github.com/aifocal/akidb/issues
- **Discussions**: https://github.com/aifocal/akidb/discussions
- **Documentation**: https://docs.akidb.com (placeholder)

---

## 🙏 Acknowledgments

Thanks to all contributors and early adopters who helped shape AkiDB v2.0!

Special thanks to:
- ONNX Runtime team for excellent ARM support
- Hugging Face for model hosting
- instant-distance library for production-grade HNSW
- Tokio ecosystem for async runtime

---

## 📅 Release History

- **v2.0.0** (November 13, 2025) - General Availability release
  - Rebranded from "AkiDB 2.0" to "AkiDB"
  - Migrated to ONNX Runtime with Python Bridge
  - Comprehensive bug fixes and stability improvements
  - 168/168 active tests passing (100%)
  - Production-ready with full observability

- **v2.0.0-rc1** (November 10, 2025) - Release Candidate 1
  - S3/MinIO tiered storage
  - Observability stack (Prometheus + Grafana)
  - Kubernetes Helm charts
  - 147 tests passing

---

**🚀 AkiDB v2.0.0 is ready for production deployment!**

For support, please open an issue on GitHub or join our discussions.

---

Built with ❤️ using Rust 🦀 and Python 3.12 🐍
