# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

---

# AkiDB - Distributed Vector Database

AkiDB is a distributed vector database written in Rust with S3-compatible storage backend, designed for high-performance similarity search and vector operations.

## Architecture Overview

### Crate Structure (Workspace)

The project is organized as a Cargo workspace with the following layered architecture:

**Core Libraries (`crates/`):**
- `akidb-core` - Core data types and schemas (collections, segments, manifests, distance metrics)
- `akidb-storage` - Persistence abstraction layer (StorageBackend trait, S3 implementation, WAL, snapshots)
- `akidb-index` - ANN index providers (IndexProvider trait for FAISS, HNSW, etc.)
- `akidb-query` - Query planning and execution engine (QueryPlanner, ExecutionEngine, PhysicalPlan)

**Services (`services/`):**
- `akidb-api` - REST + gRPC API server (Axum + Tonic)
- `akidb-mcp` - Cluster management, coordination, and balancing (membership, scheduler, balancer)

### Key Domain Concepts

**Collection:** A named vector dataset with configuration (vector_dim, distance metric, replication, shard_count, payload_schema). Defined in `crates/akidb-core/src/collection.rs:5`.

**Segment:** A persisted chunk of vectors with state tracking (Active, Sealed, Compacting, Archived) and LSN ranges for ordering. Defined in `crates/akidb-core/src/segment.rs:9`.

**Manifest:** Collection-level metadata tracking all segments and their states. Defined in `crates/akidb-core/src/manifest.rs`.

**StorageBackend:** Pluggable persistence layer (currently S3-compatible). See trait at `crates/akidb-storage/src/backend.rs:16`.

**IndexProvider:** Pluggable ANN index implementations. See trait at `crates/akidb-index/src/provider.rs:10`.

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
```

### Building

```bash
# Development build
cargo build --workspace

# Release build with optimizations
cargo build --workspace --release

# Build API service only
cargo build --package akidb-api --release

# Build release artifact with stripped symbols
./scripts/build-release.sh
# Output: dist/akidb-server
```

### Linting

```bash
# Check code formatting
cargo fmt --all -- --check

# Auto-fix formatting
cargo fmt --all

# Run Clippy with warnings-as-errors
cargo clippy --all-targets --all-features --workspace -- -D warnings
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

## Important Implementation Notes

### Storage Layer

- **S3 Backend:** Primary implementation at `crates/akidb-storage/src/s3.rs:1` uses `object_store` crate with AWS/GCP support
- **WAL (Write-Ahead Log):** See `crates/akidb-storage/src/wal.rs:1` for append-only log operations
- **Snapshots:** Managed by `SnapshotCoordinator` at `crates/akidb-storage/src/snapshot.rs:1`
- **Retry Logic:** S3 operations include configurable retry with exponential backoff (RetryConfig)

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

## Known TODOs

- `akidb-core/Cargo.toml:7` - Arrow dependency disabled due to chrono compatibility; re-enable when implementing SEGv1 format
- Current implementation is in early phase; many modules are placeholder stubs

#

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
