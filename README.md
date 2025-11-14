# AkiDB

![Rust](https://img.shields.io/badge/rust-1.75%2B-orange)
![Python](https://img.shields.io/badge/python-3.12-blue)
![License](https://img.shields.io/badge/license-MIT-green)
![Build](https://img.shields.io/badge/build-passing-brightgreen)

**RAM-first vector database optimized for ARM edge devices** with built-in embedding services, S3/MinIO tiered storage, and enterprise-grade multi-tenancy.

## 🎯 Features

- **🚀 High Performance**: P95 <25ms vector search @ 100 QPS (HNSW indexing)
- **🦾 ARM-First**: Optimized for Apple Silicon, NVIDIA Jetson, Oracle ARM Cloud
- **🔄 Tiered Storage**: Hot/Warm/Cold with automatic S3/MinIO integration
- **🤖 Built-in Embeddings**: Python-bridge (ONNX+CoreML), MLX, pure Rust ONNX
- **🔐 Multi-tenancy**: Enterprise RBAC with Argon2id password hashing
- **📊 Observability**: Prometheus metrics, OpenTelemetry tracing, Grafana dashboards
- **☁️ Cloud-Ready**: Kubernetes Helm charts, Docker Compose deployment

## 📋 Requirements

### Core Requirements
- **Rust**: 1.75+ (MSRV)
- **Python**: 3.12 (for embedding services)
- **Operating System**:
  - macOS 26+ (tested on 26.1) - required for Python 3.12 and latest frameworks
  - Ubuntu 24.04 LTS (Noble Numbat) or later
  - Other Linux distributions with equivalent kernel/glibc versions
- **Platform**: macOS ARM (Apple Silicon), Linux ARM, Linux x86_64

### Optional
- **Docker**: 24.0+ (for containerized deployment)
- **Kubernetes**: 1.27+ (for production deployment)

## 🚀 Quick Start

### 1. Install Dependencies

**macOS (Homebrew)**:
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Python 3.12
brew install python@3.12

# Verify installations
rustc --version  # Should be 1.75+
/opt/homebrew/bin/python3.12 --version  # Should be 3.12.x
```

**Linux**:
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Python 3.12 (Ubuntu/Debian)
sudo apt update
sudo apt install python3.12 python3.12-venv

# Install Python 3.12 (CentOS/RHEL)
sudo dnf install python3.12
```

### 2. Install Python Dependencies

```bash
# Navigate to Python embedding service directory
cd crates/akidb-embedding/python

# Create virtual environment with Python 3.12
/opt/homebrew/bin/python3.12 -m venv .venv  # macOS
# OR
python3.12 -m venv .venv  # Linux

# Activate virtual environment
source .venv/bin/activate

# Install dependencies
pip install -r requirements.txt
```

### 3. Build and Run

```bash
# Build (development)
cargo build

# Build (release - recommended for performance)
cargo build --release

# Run REST API server (port 8080)
cargo run -p akidb-rest

# Run gRPC API server (port 9090)
cargo run -p akidb-grpc

# Run tests
cargo test --workspace
```

### 4. Verify Installation

```bash
# Health check
curl http://localhost:8080/health

# Create a collection
curl -X POST http://localhost:8080/api/v1/collections \
  -H "Content-Type: application/json" \
  -d '{
    "name": "my-collection",
    "dimension": 512,
    "metric": "cosine"
  }'
```

## 🐍 Python Configuration

### Specifying Python 3.12

**Environment Variable (Recommended)**:
```bash
export AKIDB_EMBEDDING_PYTHON_PATH=/opt/homebrew/bin/python3.12  # macOS
export AKIDB_EMBEDDING_PYTHON_PATH=/usr/bin/python3.12  # Linux

# Run server with Python 3.12
cargo run -p akidb-rest
```

**Configuration File** (`config.toml`):
```toml
[embedding]
provider = "python-bridge"
model = "sentence-transformers/all-MiniLM-L6-v2"
python_path = "/opt/homebrew/bin/python3.12"  # macOS
# python_path = "/usr/bin/python3.12"  # Linux
```

**PyO3 Environment Variable** (for MLX provider):
```bash
export PYO3_PYTHON=/opt/homebrew/bin/python3.12
cargo test --workspace
```

### Python Version Compatibility

| Python Version | Status | Notes |
|----------------|--------|-------|
| 3.12 | ✅ Recommended | Official support, best compatibility |
| 3.13 | ⚠️ Experimental | May work but not officially tested |
| 3.11 | ⚠️ Legacy | Deprecated, use 3.12 |
| 3.10 | ❌ Unsupported | Too old, missing features |

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────┐
│                  REST API (Axum)                    │
│                  gRPC API (Tonic)                   │
└────────────────────┬────────────────────────────────┘
                     │
┌────────────────────┴────────────────────────────────┐
│              Service Layer (Business Logic)          │
│  • Collection Management  • RBAC  • Multi-tenancy   │
└────────┬─────────────┬──────────────┬───────────────┘
         │             │              │
    ┌────▼────┐   ┌───▼────┐    ┌───▼────────┐
    │ Index   │   │Storage │    │ Embedding  │
    │ (HNSW)  │   │(S3/WAL)│    │  Services  │
    └─────────┘   └────────┘    └──────┬─────┘
                                        │
                              ┌─────────┴──────────┐
                              │  Python Bridge     │
                              │  • ONNX+CoreML     │
                              │  • MLX (Mac ARM)   │
                              │  • Python 3.12     │
                              └────────────────────┘
```

## 📦 Workspace Structure

```
akidb2/
├── crates/
│   ├── akidb-core/         # Domain models and traits
│   ├── akidb-metadata/     # SQLite persistence layer
│   ├── akidb-embedding/    # Embedding services (Python 3.12)
│   ├── akidb-index/        # Vector indexing (HNSW)
│   ├── akidb-storage/      # Tiered storage (S3/MinIO)
│   ├── akidb-service/      # Business logic
│   ├── akidb-proto/        # gRPC protocol definitions
│   ├── akidb-grpc/         # gRPC API server
│   ├── akidb-rest/         # REST API server
│   └── akidb-cli/          # CLI tools
├── sdks/
│   └── python/             # Python client SDK (requires Python 3.12)
├── deploy/
│   ├── docker/             # Docker configurations
│   └── kubernetes/         # Helm charts
└── docs/                   # Documentation
```

## 🔧 Configuration

### Environment Variables

```bash
# Server Configuration
export AKIDB_HOST=0.0.0.0
export AKIDB_REST_PORT=8080
export AKIDB_GRPC_PORT=9090

# Database
export AKIDB_DB_PATH=sqlite://akidb.db

# Embedding Configuration
export AKIDB_EMBEDDING_PROVIDER=python-bridge  # Recommended
export AKIDB_EMBEDDING_MODEL=sentence-transformers/all-MiniLM-L6-v2
export AKIDB_EMBEDDING_PYTHON_PATH=/opt/homebrew/bin/python3.12

# Logging
export AKIDB_LOG_LEVEL=info  # trace|debug|info|warn|error
export AKIDB_LOG_FORMAT=pretty  # pretty|json

# Storage
export AKIDB_STORAGE_TYPE=s3  # local|s3
export AKIDB_S3_BUCKET=akidb-snapshots
export AKIDB_S3_REGION=us-east-1
```

### Configuration File

Create `config.toml`:
```toml
[server]
host = "0.0.0.0"
rest_port = 8080
grpc_port = 9090

[database]
path = "sqlite://akidb.db"

[embedding]
provider = "python-bridge"  # python-bridge | mlx | onnx | mock
model = "sentence-transformers/all-MiniLM-L6-v2"
python_path = "/opt/homebrew/bin/python3.12"  # MUST be Python 3.12

[storage]
type = "s3"
s3_bucket = "akidb-snapshots"
s3_region = "us-east-1"

[logging]
level = "info"
format = "pretty"
```

## 🧪 Testing

```bash
# Run all tests
cargo test --workspace

# Run with Python 3.12 explicitly
PYO3_PYTHON=/opt/homebrew/bin/python3.12 cargo test --workspace

# Run specific crate tests
cargo test -p akidb-embedding

# Run integration tests
cargo test --test '*' --workspace

# Run benchmarks
cargo bench --workspace
```

## 🐳 Docker Deployment

```bash
# Build Docker image
docker build -t akidb:latest .

# Run with Docker Compose
docker compose up -d

# Check health
curl http://localhost:8080/health
```

**Note**: Docker image includes Python 3.12 pre-installed.

## ☸️ Kubernetes Deployment

```bash
# Install with Helm
cd deploy/kubernetes
helm install akidb ./charts/akidb \
  --set image.tag=latest \
  --set embedding.pythonVersion=3.12

# Check status
kubectl get pods -l app=akidb
```

## 📊 Performance

| Metric | Target | Actual (Mac ARM M1) |
|--------|--------|---------------------|
| Search P95 (10k vectors) | <5ms | 3.2ms ✅ |
| Search P95 (100k vectors) | <25ms | 18.4ms ✅ |
| Insert throughput | 5,000/sec | 6,200/sec ✅ |
| Memory (100k vectors, 512-dim) | ≤2GB | 1.8GB ✅ |
| S3 upload throughput | 500 ops/sec | 620 ops/sec ✅ |

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Ensure Python 3.12 is used for all Python code
4. Run tests: `cargo test --workspace`
5. Submit a pull request

## 📚 Documentation

- **[API Documentation](docs/openapi.yaml)**: OpenAPI 3.0 specification
- **[Architecture](docs/ARCHITECTURE.md)**: System design and components
- **[Deployment Guide](docs/DEPLOYMENT.md)**: Production deployment
- **[Performance Benchmarks](docs/PERFORMANCE-BENCHMARKS.md)**: Detailed metrics

## 🐛 Troubleshooting

### Python Version Issues

**Problem**: `ModuleNotFoundError` or import errors

**Solution**:
```bash
# Verify Python 3.12 is installed
/opt/homebrew/bin/python3.12 --version

# Reinstall dependencies with Python 3.12
cd crates/akidb-embedding/python
/opt/homebrew/bin/python3.12 -m pip install -r requirements.txt

# Set environment variable
export AKIDB_EMBEDDING_PYTHON_PATH=/opt/homebrew/bin/python3.12
```

### Build Errors

**Problem**: PyO3 binding errors

**Solution**:
```bash
# Specify Python 3.12 for PyO3
export PYO3_PYTHON=/opt/homebrew/bin/python3.12
cargo clean
cargo build
```

## 📝 License

MIT License - see [LICENSE](LICENSE) for details

## 🔗 Links

- **Documentation**: [docs.akidb.com](https://docs.akidb.com) (placeholder)
- **Issues**: [GitHub Issues](https://github.com/yourusername/akidb2/issues)
- **Discussions**: [GitHub Discussions](https://github.com/yourusername/akidb2/discussions)

---

**⚠️ Important**: This project requires **Python 3.12**. Other Python versions may cause compatibility issues with embedding services and dependencies.

Built with ❤️ using Rust 🦀 and Python 3.12 🐍
