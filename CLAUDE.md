# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

---

# AkiDB - Distributed Vector Database

AkiDB is a distributed vector database written in Rust with S3-compatible storage backend, designed for high-performance similarity search and vector operations.

## 🎯 Project Status

**Phase 2**: ✅ **COMPLETE** (100%)
- S3-native storage with metadata persistence
- Restart recovery from S3 with collection loading
- WAL with crash recovery
- Arrow IPC payload format for metadata
- 67/67 integration tests passing

**Phase 3**: 🟢 **IN PROGRESS** (Milestone M2)
- **M1 (Benchmarking Foundation)**: ✅ Complete
  - Criterion benchmark harness implemented
  - Phase 2 baselines captured (P50: 0.53-0.69ms, throughput: 1,890 QPS)
  - Performance guide documented

- **M2 (HNSW Index Tuning)**: 🔄 Current (Weeks 3-4)
  - Goal: P95 ≤150ms, P99 ≤250ms (1M vectors, k=50)
  - Target: +20% throughput, -15% index rebuild time
  - Branch: `feature/phase3-m2-hnsw-tuning`

**Performance Baseline (10K vectors, 128-dim)**:
- P50 latency: 0.53-0.69ms (k=10 queries)
- Throughput: up to 1,890 QPS (L2 distance)
- L2 distance 23% faster than Cosine similarity
- Sub-millisecond P99 latency across all scenarios

---

## Architecture Overview

### Crate Structure (Workspace)

The project is organized as a Cargo workspace with the following layered architecture:

**Core Libraries (`crates/`):**
- `akidb-core` - Core data types and schemas (collections, segments, manifests, distance metrics)
- `akidb-storage` - Persistence abstraction layer (StorageBackend trait, S3 implementation, WAL, snapshots)
- `akidb-index` - ANN index providers (IndexProvider trait for FAISS, HNSW, etc.)
- `akidb-query` - Query planning and execution engine (QueryPlanner, ExecutionEngine, PhysicalPlan)
- `akidb-benchmarks` - Performance benchmarking with Criterion.rs

**Services (`services/`):**
- `akidb-api` - REST + gRPC API server (Axum + Tonic)
- `akidb-mcp` - Cluster management, coordination, and balancing (membership, scheduler, balancer)

### Key Domain Concepts

**Collection:** A named vector dataset with configuration (vector_dim, distance metric, replication, shard_count, payload_schema). Defined in `crates/akidb-core/src/collection.rs:5`.

**Segment:** A persisted chunk of vectors with state tracking (Active, Sealed, Compacting, Archived) and LSN ranges for ordering. Defined in `crates/akidb-core/src/segment.rs:9`.

**Manifest:** Collection-level metadata tracking all segments and their states. Defined in `crates/akidb-core/src/manifest.rs`.

**StorageBackend:** Pluggable persistence layer (currently S3-compatible). See trait at `crates/akidb-storage/src/backend.rs:16`.

**IndexProvider:** Pluggable ANN index implementations. See trait at `crates/akidb-index/src/provider.rs:10`.

---

## Development Commands

### Testing and Validation

```bash
# Run full test suite with formatting and linting
./scripts/dev-test.sh

# Individual commands (from dev-test.sh):
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --workspace -- -D warnings
cargo test --workspace --all-targets --all-features

# Run tests for a specific crate
cargo test -p akidb-core
cargo test -p akidb-storage

# Run a single test
cargo test --package akidb-storage --test test_name

# Run tests with backtrace
RUST_BACKTRACE=1 cargo test

# Run tests with logging
RUST_LOG=debug cargo test
```

### Building

```bash
# Development build
cargo build --workspace

# Release build with optimizations
cargo build --workspace --release

# API service only
cargo build --package akidb-api --release

# Build release artifact with stripped symbols
./scripts/build-release.sh
# Output: dist/akidb-server

# Check compilation without building
cargo check --workspace
```

### Performance Benchmarking

```bash
# Run all benchmarks
cargo bench --package akidb-benchmarks

# Run specific benchmark suite
cargo bench --package akidb-benchmarks --bench vector_search
cargo bench --package akidb-benchmarks --bench index_build
cargo bench --package akidb-benchmarks --bench metadata_ops

# Capture Phase 2 baseline
./scripts/capture-baseline.sh

# View benchmark results
open target/criterion/report/index.html

# Compare benchmarks
./scripts/compare-benchmarks.sh phase2 m2
```

### Linting

```bash
# Check code formatting
cargo fmt --all -- --check

# Auto-fix formatting
cargo fmt --all

# Run Clippy with warnings-as-errors
cargo clippy --all-targets --all-features --workspace -- -D warnings

# Run Clippy with automatic fixes
cargo clippy --fix --all-targets --all-features --workspace
```

### Local Development Environment

```bash
# Bootstrap Docker Compose stack (MinIO + akidb-server)
./scripts/dev-init.sh

# Force rebuild and recreate containers
./scripts/dev-init.sh --force-recreate

# Tear down the stack
docker compose down -v
```

**Services:**
- MinIO (S3): http://localhost:9000 (API), http://localhost:9001 (Console)
- AkiDB API: http://localhost:8080

**Environment:** Copy `.env.example` to `.env` to configure ports and credentials.

### Docker Development

```bash
# Start development container with mounted workspace
docker compose --profile devtools up -d devtools
docker compose exec devtools bash

# Build Docker image manually
docker build \
  --build-arg RUST_VERSION=1.77 \
  --build-arg APP_NAME=akidb-api \
  -t akidb/server:dev .
```

---

## Important Implementation Notes

### Storage Layer

- **S3 Backend:** Primary implementation at `crates/akidb-storage/src/s3.rs:1` uses `object_store` crate with AWS/GCP support
- **WAL (Write-Ahead Log):** See `crates/akidb-storage/src/wal.rs:1` for append-only log operations
- **Snapshots:** Managed by `SnapshotCoordinator` at `crates/akidb-storage/src/snapshot.rs:1`
- **Retry Logic:** S3 operations include configurable retry with exponential backoff (RetryConfig)
- **Metadata Format:** Arrow IPC format for efficient payload storage and deserialization
- **Bootstrap Recovery:** Collection loading from S3 on restart at `services/akidb-api/src/bootstrap.rs`

### Data Types

- **Distance Metrics:** L2, Cosine, Dot (default: Cosine) at `crates/akidb-core/src/collection.rs:16`
- **Payload Types:** Boolean, Integer, Float, Text, Keyword, GeoPoint, Timestamp, Json at `crates/akidb-core/src/collection.rs:44`
- **Segment States:** Active → Sealed → Compacting → Archived lifecycle at `crates/akidb-core/src/segment.rs:22`

### Query Execution

- Query flow: `QueryRequest` → `QueryPlanner` → `PhysicalPlan` → `ExecutionEngine` → `QueryResponse`
- See `crates/akidb-query/src/` for components

### API Layer

- **REST API:** Axum-based at `services/akidb-api/src/rest.rs:1`
- **gRPC API:** Tonic-based at `services/akidb-api/src/grpc.rs:1`
- Both share common middleware at `services/akidb-api/src/middleware.rs:1`
- Entry point: `services/akidb-api/src/lib.rs:13` (`run_server()`)

### Cluster Management (MCP)

- **Membership:** Cluster state and node discovery at `services/akidb-mcp/src/membership.rs:1`
- **Balancer:** Shard rebalancing logic at `services/akidb-mcp/src/balancer.rs:1`
- **Scheduler:** Background job coordination at `services/akidb-mcp/src/scheduler.rs:1`

---

## Performance Optimization (Phase 3)

### Benchmarking

AkiDB uses **Criterion.rs** for performance testing:

**Key Metrics:**
- **P50/P95/P99 Latency** (milliseconds)
- **Throughput** (QPS - queries per second)
- **Memory Usage** (Peak RSS)

**Benchmark Suites:**
- `vector_search.rs` - Search latency and throughput across dataset sizes (10K, 100K, 1M)
- `index_build.rs` - Index construction time and memory usage
- `metadata_ops.rs` - Metadata filter performance

**Phase 2 Baseline (10K vectors, 128-dim):**
- Cosine k=10: P50=0.69ms, P95=0.82ms, P99=0.94ms, 1,450 QPS
- L2 k=10: P50=0.53ms, P95=0.57ms, P99=0.62ms, 1,890 QPS (23% faster)

See `docs/performance-guide.md` for detailed benchmarking guide.

### Phase 3 Goals

**M2 (HNSW Index Tuning)** - Current:
- P95 latency ≤150ms (1M vectors, k=50)
- P99 latency ≤250ms (1M vectors, k=50)
- Throughput +20% vs Phase 2 baseline
- Index rebuild time -15%

**M3 (Query Planner Enhancements)** - Upcoming:
- Filter pushdown to index layer
- Batch query optimization
- Parallel segment scanning

**M4 (Production Monitoring)** - Upcoming:
- Prometheus metrics integration
- Distributed tracing with OpenTelemetry
- Query profiling and slow query logs

---

## Known TODOs

- `akidb-core/Cargo.toml:7` - Arrow dependency disabled due to chrono compatibility; re-enable when implementing SEGv1 format
- M2: HNSW parameter exploration and dataset-aware presets
- M3: Query planner optimizations
- M4: Production monitoring and observability

---

# AutomatosX Integration

This project uses [AutomatosX](https://github.com/defai-digital/automatosx) - an AI agent orchestration platform with persistent memory and multi-agent collaboration.

## Quick Start

### Available Commands

```bash
# List all available agents
ax list agents

# Run an agent with a task
ax run <agent-name> "your task description"

# Example: Ask the backend agent to create an API
ax run backend "create a REST API for user management"

# Search memory for past conversations
ax memory search "keyword"

# View system status
ax status
```

### Using AutomatosX in Claude Code

You can interact with AutomatosX agents directly in Claude Code using natural language or slash commands:

**Natural Language (Recommended)**:
```
"Please work with ax agent backend to implement user authentication"
"Ask the ax security agent to audit this code for vulnerabilities"
"Have the ax quality agent write tests for this feature"
```

**Slash Command**:
```
/ax-agent backend, create a REST API for user management
/ax-agent security, audit the authentication flow
/ax-agent quality, write unit tests for the API
```

### Available Agents

This project includes the following specialized agents:

- **backend** - Backend development (Go/Rust/Python systems)
- **frontend** - Frontend development (React/Next.js/Swift)
- **fullstack** - Full-stack development (Node.js/TypeScript + Python)
- **mobile** - Mobile development (iOS/Android, Swift/Kotlin/Flutter)
- **devops** - DevOps and infrastructure
- **security** - Security auditing and threat modeling
- **data** - Data engineering and ETL
- **quality** - QA and testing
- **design** - UX/UI design
- **writer** - Technical writing
- **product** - Product management
- **cto** - Technical strategy
- **ceo** - Business leadership
- **researcher** - Research and analysis

For a complete list with capabilities, run: `ax list agents --format json`

## Key Features

### 1. Persistent Memory

AutomatosX agents remember all previous conversations and decisions:

```bash
# First task - design is saved to memory
ax run product "Design a calculator with add/subtract features"

# Later task - automatically retrieves the design from memory
ax run backend "Implement the calculator API"
```

### 2. Multi-Agent Collaboration

Agents can delegate tasks to each other automatically:

```bash
ax run product "Build a complete user authentication feature"
# → Product agent designs the system
# → Automatically delegates implementation to backend agent
# → Automatically delegates security audit to security agent
```

### 3. Cross-Provider Support

AutomatosX supports multiple AI providers with automatic fallback:
- Claude (Anthropic)
- Gemini (Google)
- OpenAI (GPT)

Configuration is in `automatosx.config.json`.

## Configuration

### Project Configuration

Edit `automatosx.config.json` to customize:

```json
{
  "providers": {
    "claude-code": {
      "enabled": true,
      "priority": 1
    },
    "gemini-cli": {
      "enabled": true,
      "priority": 2
    }
  },
  "execution": {
    "defaultTimeout": 1500000,  // 25 minutes
    "maxRetries": 3
  },
  "memory": {
    "enabled": true,
    "maxEntries": 10000
  }
}
```

### Agent Customization

Create custom agents in `.automatosx/agents/`:

```bash
ax agent create my-agent --template developer --interactive
```

## Memory System

### Search Memory

```bash
# Search for past conversations
ax memory search "authentication"
ax memory search "API design"

# List recent memories
ax memory list --limit 10

# Export memory for backup
ax memory export > backup.json
```

### How Memory Works

- **Automatic**: All agent conversations are saved automatically
- **Fast**: SQLite FTS5 full-text search (< 1ms)
- **Local**: 100% private, data never leaves your machine
- **Cost**: $0 (no API calls for memory operations)

## Advanced Usage

### Parallel Execution (v5.6.0+)

Run multiple agents in parallel for faster workflows:

```bash
ax run product "Design authentication system" --parallel
```

### Resumable Runs (v5.3.0+)

For long-running tasks, enable checkpoints:

```bash
ax run backend "Refactor entire codebase" --resumable

# If interrupted, resume with:
ax resume <run-id>

# List all runs
ax runs list
```

### Streaming Output (v5.6.5+)

See real-time output from AI providers:

```bash
ax run backend "Explain this codebase" --streaming
```

## Troubleshooting

### Common Issues

**"Agent not found"**
```bash
# List available agents
ax list agents

# Make sure agent name is correct
ax run backend "task"  # ✓ Correct
ax run Backend "task"  # ✗ Wrong (case-sensitive)
```

**"Provider not available"**
```bash
# Check system status
ax status

# View configuration
ax config show
```

**"Out of memory"**
```bash
# Clear old memories
ax memory clear --before "2024-01-01"

# View memory stats
ax cache stats
```

### Getting Help

```bash
# View command help
ax --help
ax run --help

# Enable debug mode
ax --debug run backend "task"

# Search memory for similar past tasks
ax memory search "similar task"
```

## Best Practices

1. **Use Natural Language in Claude Code**: Let Claude Code coordinate with agents for complex tasks
2. **Leverage Memory**: Reference past decisions and designs
3. **Start Simple**: Test with small tasks before complex workflows
4. **Review Configurations**: Check `automatosx.config.json` for timeouts and retries
5. **Keep Agents Specialized**: Use the right agent for each task type

## Documentation

- **AutomatosX Docs**: https://github.com/defai-digital/automatosx
- **Agent Directory**: `.automatosx/agents/`
- **Configuration**: `automatosx.config.json`
- **Memory Database**: `.automatosx/memory/memories.db`
- **Workspace**: `automatosx/PRD/` (planning docs) and `automatosx/tmp/` (temporary files)

## Support

- Issues: https://github.com/defai-digital/automatosx/issues
- NPM: https://www.npmjs.com/package/@defai.digital/automatosx
