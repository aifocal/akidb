# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

---

## 🤖 How AI Agents Should Work in This Codebase

### Coding Standards

- **Language**: Rust 2021 edition, following idiomatic Rust patterns
- **Error Handling**: Use `thiserror` for error types, never `unwrap()` or `expect()` in production code
- **Async Runtime**: Tokio-based, all I/O operations must be async
- **Testing**: Every new function requires unit tests, integration tests for cross-crate workflows

### Development Workflow

1. **Before Implementation**: Read `tmp/current-status-analysis.md` for accurate project status
2. **Make Changes**: Implement feature with tests
3. **Validation**: Run `./scripts/dev-test.sh` (format + lint + test)
4. **Commit**: Use conventional commits (feat:, fix:, docs:, refactor:)

### Escalation Policy

- **Blocked by Missing Dependency**: Check `Cargo.toml` workspace dependencies first
- **Uncertain Architecture**: Consult `CLAUDE.md` architecture section or ask user
- **Test Failures**: Enable `RUST_LOG=debug` and `RUST_BACKTRACE=1` for debugging
- **S3/MinIO Issues**: Verify Docker containers are running (`docker compose ps`)

### Truth Sources

- **Implementation Status**: `tmp/current-status-analysis.md` (NOT the "Project Status" section below)
- **Test Coverage**: `cargo test --workspace` output
- **Dependencies**: `Cargo.toml` workspace section

---

# AkiDB - Distributed Vector Database

AkiDB is a distributed vector database written in Rust with S3-compatible storage backend, designed for high-performance similarity search and vector operations.

## 🎯 Project Status

**Current Focus**: Phase 3 M2 - Storage Backend & Index Implementation

**What's Working**:
- ✅ Project structure and workspace setup
- ✅ Core type definitions (Collection, Segment, Manifest)
- ✅ Storage trait abstractions (StorageBackend, IndexProvider)
- ✅ Development environment (Docker + MinIO)
- ✅ Benchmark harness with Phase 2 baselines

**What Needs Implementation** (Priority Order):
1. 🔄 **S3 Storage Backend** - Core methods (write_segment, seal_segment, manifests)
2. 🔄 **WAL Operations** - Proper append/replay with crash safety
3. 🔄 **Index Provider** - Wire native brute-force engine to storage
4. ⏳ **API Integration** - Connect REST endpoints to business logic
5. ⏳ **Integration Tests** - End-to-end collection → insert → search flows

**Branch**: `feature/phase3-m2-hnsw-tuning`

> **Note**: See `tmp/current-status-analysis.md` for detailed implementation status.
> Run `cargo test --workspace` to verify current test coverage.

---

## 🚀 Quick Start for New Developers

### First Time Setup (5 minutes)

**Prerequisites**:
- Rust 1.77+ installed (`rustup` recommended)
- Docker and Docker Compose
- Git

**Setup Steps**:

1. **Clone and configure environment**:
   ```bash
   git clone https://github.com/defai-digital/akidb.git
   cd akidb
   cp .env.example .env
   ```

2. **Start local development environment**:
   ```bash
   ./scripts/dev-init.sh
   ```
   This script will:
   - Start MinIO (S3-compatible storage) on ports 9000/9001
   - Start akidb-server on port 8080
   - Create the default S3 bucket

3. **Verify your setup**:
   ```bash
   # Run the full test suite
   ./scripts/dev-test.sh

   # Or run tests individually
   cargo test --workspace
   ```

**What's Running**:
- MinIO S3 API: http://localhost:9000
- MinIO Console: http://localhost:9001 (credentials: akidb / akidbsecret)
- AkiDB API: http://localhost:8080

### Making Your First Change

1. **Understand the architecture**: Read [Architecture Overview](#architecture-overview) below
2. **Explore core types**: Start with `crates/akidb-core/src/collection.rs:5`
3. **Try a simple change**: Add a test or modify a struct field
4. **Run validation**: Use `./scripts/dev-test.sh` to ensure quality

### Recommended Learning Path

1. **Core Concepts** (30 min): Read [Key Domain Concepts](#key-domain-concepts)
2. **Development Commands** (15 min): Skim [Development Commands](#development-commands) for reference
3. **Code Exploration** (60 min): Navigate the codebase starting from `crates/akidb-core/`
4. **Make a Change** (variable): Pick an issue and implement it
5. **Performance** (optional): Read [Performance Guide](docs/performance-guide.md) for Phase 3 work

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

#### Environment Configuration

**Core Variables**:
```bash
# S3 Storage (Required)
AKIDB_S3_ENDPOINT=http://minio:9000   # S3-compatible endpoint
AKIDB_S3_BUCKET=akidb                  # Bucket name
AKIDB_S3_REGION=us-east-1              # AWS region or compatible
AKIDB_S3_ACCESS_KEY=akidb              # S3 access key
AKIDB_S3_SECRET_KEY=akidbsecret        # S3 secret key

# API Server (Required)
AKIDB_BIND_ADDRESS=0.0.0.0:8080        # Server bind address
AKIDB_PORT=8080                         # HTTP API port

# Logging (Optional)
RUST_LOG=info                           # Global log level (error, warn, info, debug, trace)
AKIDB_LOG_LEVEL=info                    # AkiDB-specific log level
```

**For Local Development**:
- Use the defaults from `.env.example` (matches `docker-compose.yml`)
- MinIO credentials: `akidb` / `akidbsecret`
- No changes needed for basic development

**For Production / Cloud Deployment**:
- Set `AKIDB_S3_ENDPOINT` to real AWS S3: `https://s3.amazonaws.com`
- Use IAM credentials or instance profiles for `ACCESS_KEY` / `SECRET_KEY`
- Enable TLS/HTTPS for all endpoints
- Use environment-specific bucket names (e.g., `akidb-prod`, `akidb-staging`)

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

## 🔄 Common Workflows

This section provides step-by-step workflows for common development tasks.

### Adding a New Collection Feature

**Example**: Adding a new field to `CollectionDescriptor`

1. **Define the data type** in `crates/akidb-core/src/collection.rs`:
   ```rust
   pub struct CollectionDescriptor {
       pub name: String,
       pub vector_dim: u16,
       pub distance: DistanceMetric,
       pub your_new_field: YourType,  // Add here
       // ...
   }
   ```

2. **Update storage operations** in `crates/akidb-storage/src/s3.rs`:
   - Modify `create_collection()` to handle the new field
   - Update manifest serialization if needed

3. **Add API endpoint** in `services/akidb-api/src/handlers/collections.rs`:
   - Update request/response types
   - Add validation in `services/akidb-api/src/validation.rs`

4. **Write integration test** in `services/akidb-api/tests/integration_test.rs`:
   ```rust
   #[tokio::test]
   async fn test_new_collection_field() {
       // Test your new field
   }
   ```

5. **Run full validation**:
   ```bash
   ./scripts/dev-test.sh
   ```

### Debugging a Failing Test

**Scenario**: A test is failing and you need to understand why.

1. **Run the specific test with logging**:
   ```bash
   RUST_LOG=debug cargo test --package akidb-storage test_name -- --nocapture
   ```

2. **Enable full backtrace**:
   ```bash
   RUST_BACKTRACE=full cargo test test_name
   ```

3. **For integration tests** (requires Docker):
   ```bash
   # Ensure environment is running
   ./scripts/dev-init.sh

   # Run integration tests
   cargo test --workspace -- --include-ignored

   # Check Docker logs if S3/MinIO related
   docker compose logs -f minio
   ```

4. **Common debugging patterns**:
   - **S3 errors**: Check `.env` file and MinIO container status
   - **Serialization errors**: Verify Arrow schema compatibility
   - **Timeout errors**: Check network connectivity to MinIO

### Performance Tuning Workflow (Phase 3)

**Goal**: Optimize HNSW index parameters for better latency/throughput.

1. **Capture baseline metrics**:
   ```bash
   ./scripts/capture-baseline.sh
   ```
   Results saved to `target/criterion/` and documented in `tmp/PHASE2-BASELINE-METRICS.md`

2. **Make targeted changes**:
   - **Note**: Currently uses brute-force index implementation
   - HNSW parameter tuning will be available in future milestones (M2+)
   - For now, focus on query execution and metadata filtering optimizations

3. **Run focused benchmarks**:
   ```bash
   # Test specific dataset size and k value
   cargo bench --package akidb-benchmarks --bench vector_search -- 10k/k=10

   # Or run full suite
   cargo bench --package akidb-benchmarks
   ```

4. **Compare results**:
   ```bash
   # Compare before/after metrics
   ./scripts/compare-benchmarks.sh baseline current

   # View detailed HTML report
   open target/criterion/report/index.html
   ```

5. **Validate against Phase 3 goals**:
   - P95 latency ≤150ms (1M vectors, k=50)
   - P99 latency ≤250ms (1M vectors, k=50)
   - Throughput +20% vs baseline

### Making Your First Pull Request

1. **Create a feature branch**:
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make changes and test**:
   ```bash
   # Make your changes
   ./scripts/dev-test.sh  # Ensure all checks pass
   ```

3. **Commit your changes**:
   ```bash
   git add .
   git commit -m "Brief description of your changes"
   ```
   Note: Do NOT mention AI assistance in commits (per CLAUDE.md guidelines)

4. **Push and create PR**:
   ```bash
   git push -u origin feature/your-feature-name
   # Then create PR via GitHub UI
   ```

### Command Cheat Sheet

**Daily Development**:
```bash
./scripts/dev-test.sh              # Full test suite with linting
cargo test --workspace             # Quick test (no linting)
cargo fmt --all                    # Format code
cargo clippy --fix --workspace     # Auto-fix clippy warnings
```

**Debugging**:
```bash
RUST_LOG=debug cargo test test_name -- --nocapture   # Test with logs
RUST_BACKTRACE=full cargo test test_name             # Test with backtrace
cargo check --workspace                               # Quick compile check
```

**Performance**:
```bash
./scripts/capture-baseline.sh                         # Capture baseline
cargo bench --bench vector_search                     # Run benchmarks
open target/criterion/report/index.html              # View results
```

**Docker**:
```bash
./scripts/dev-init.sh                     # Start environment
./scripts/dev-init.sh --force-recreate    # Force rebuild
docker compose down -v                     # Teardown
docker compose logs -f akidb-server       # View logs
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
- **Payload Types:** Boolean, Integer, Float, Text, Keyword, GeoPoint, Timestamp, Json at `crates/akidb-core/src/collection.rs:39`
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

- `akidb-core/Cargo.toml:7` - Arrow dependency currently disabled in core crate (SEGv1/Arrow IPC is implemented in `akidb-storage` instead)
- M2: HNSW index implementation and parameter exploration
- M3: Query planner optimizations (filter pushdown, batch queries)
- M4: Production monitoring and observability

---

## Migration Guides

- **[Manifest V1 Migration](docs/migrations/manifest_v1.md)** - Atomic manifest operations and optimistic locking for concurrent writes
- **[Storage API Migration](docs/migration-guide.md)** - Migrating from `write_segment` to `write_segment_with_data` with SEGv1 format
- **[Index Providers Guide](docs/index-providers.md)** - Vector index implementation guide and contract testing

---

## 🔧 Troubleshooting

This section covers common issues and their solutions.

### Build and Compilation Issues

**Problem**: `error: failed to run custom build command for...`

**Causes**:
- Missing Rust toolchain or outdated version
- Missing system dependencies

**Solutions**:
```bash
# Update Rust toolchain
rustup update
cargo --version  # Should be 1.77+

# Clean and rebuild
cargo clean
cargo build --workspace
```

---

**Problem**: `error[E0433]: failed to resolve: use of undeclared crate or module`

**Cause**: Dependency issues or incorrect feature flags

**Solution**:
```bash
# Update dependencies
cargo update

# Check Cargo.toml for correct dependency versions
# Ensure all features are correctly specified
```

### Environment and Docker Issues

**Problem**: `MinIO connection refused` or `Failed to connect to S3`

**Causes**:
- MinIO container not running
- Wrong endpoint in `.env` file
- Network issues between containers

**Solutions**:
```bash
# Check if MinIO is running
docker compose ps

# Restart development environment
docker compose down -v
./scripts/dev-init.sh

# Verify MinIO is accessible
curl http://localhost:9000/minio/health/live

# Check environment variables
cat .env | grep AKIDB_S3
```

---

**Problem**: `docker compose` command not found

**Cause**: Using older Docker versions with `docker-compose` (hyphenated)

**Solution**:
```bash
# Update Docker to latest version, or
# Use legacy command
docker-compose up -d

# Better: Install Docker Compose V2 plugin
```

### Test Failures

**Problem**: `cargo test` fails with S3 errors

**Causes**:
- Missing or incorrect `.env` file
- MinIO not running
- Stale test data

**Solutions**:
```bash
# Ensure .env exists and matches docker-compose.yml
cp .env.example .env
cat .env  # Verify credentials: akidb / akidbsecret

# Start fresh environment
docker compose down -v  # -v removes volumes
./scripts/dev-init.sh

# Run tests again
cargo test --workspace
```

---

**Problem**: Integration tests hang or timeout

**Cause**: Network connectivity issues or resource constraints

**Solutions**:
```bash
# Check Docker network
docker network ls
docker network inspect akidb_default

# Increase timeout in test
# Edit test file to extend timeout durations

# Check system resources
docker stats  # Monitor CPU/Memory usage
```

---

**Problem**: `cargo test --benches` fails

**Cause**: Benchmark compilation issues

**Solution**:
```bash
# This is just a guard to ensure benchmarks compile
# Run in release mode
cargo test --workspace --benches --all-features --release

# If still failing, check benchmark code for syntax errors
```

### Performance and Benchmarking Issues

**Problem**: Benchmarks are too slow or inconsistent

**Causes**:
- Running in debug mode
- Background processes consuming resources
- CPU throttling

**Solutions**:
```bash
# Always use release mode for benchmarks
cargo bench --package akidb-benchmarks  # Criterion does this automatically

# Close background applications
# On macOS, disable CPU throttling:
sudo systemsetup -setcomputersleep Never

# Check system load
top  # or htop
```

---

**Problem**: `out of memory` during benchmarks

**Cause**: Large dataset benchmarks (1M vectors)

**Solutions**:
```bash
# Run smaller dataset benchmarks first
cargo bench --bench vector_search -- 10k

# Increase system memory if needed
# Or reduce benchmark dataset sizes in code
```

### Git and Version Control Issues

**Problem**: `git status` shows many untracked files in `tmp/`

**Explanation**: This is **expected behavior**
- `tmp/` contains temporary development notes and planning docs
- These files should NOT be committed (see `.gitignore`)
- They are for local development only

**Action**:
```bash
# Safely ignore these files
git status --ignored  # View what's being ignored

# tmp/ is already in .gitignore
cat .gitignore | grep tmp
```

---

**Problem**: Merge conflicts in `Cargo.lock`

**Solution**:
```bash
# For Cargo.lock conflicts, regenerate it:
git checkout --theirs Cargo.lock  # or --ours
cargo update
git add Cargo.lock
git commit
```

### Debugging Tips

**Enable detailed logging**:
```bash
# Global Rust logging
RUST_LOG=trace cargo test

# AkiDB-specific logging
RUST_LOG=akidb_storage=debug,akidb_api=debug cargo test

# Full backtrace on panic
RUST_BACKTRACE=full cargo test
```

**Debug specific component**:
```bash
# Storage layer
RUST_LOG=akidb_storage=trace cargo test -p akidb-storage -- --nocapture

# API layer
RUST_LOG=akidb_api=debug cargo test -p akidb-api -- --nocapture
```

**Check Docker logs**:
```bash
# View all logs
docker compose logs

# Follow specific service
docker compose logs -f minio
docker compose logs -f akidb-server

# Last 100 lines
docker compose logs --tail=100
```

### Getting Help

If you're stuck:

1. **Search existing issues**: Check GitHub issues for similar problems
2. **Check documentation**: Review `docs/` directory and this CLAUDE.md
3. **Enable debug logging**: Use `RUST_LOG=debug` to get more context
4. **Isolate the problem**: Create a minimal reproduction case
5. **Ask for help**: Open a GitHub issue with:
   - Steps to reproduce
   - Error messages (full output)
   - Environment details (`rustc --version`, `docker --version`)
   - Relevant logs

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
