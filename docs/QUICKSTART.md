# AkiDB v2.0.0-rc1 Quick Start Guide

Get started with AkiDB in 5 minutes using Docker Compose.

## Prerequisites

- Docker 20.10+ and Docker Compose
- 2GB RAM minimum
- 10GB disk space

## Quick Start (Docker Compose)

### Step 1: Start Services

```bash
# Clone repository
git clone https://github.com/akidb/akidb2.git
cd akidb2
git checkout v2.0.0-rc1

# Start services
docker-compose up -d

# Check status
docker-compose ps

# Expected output:
#   akidb-grpc    running (healthy)   0.0.0.0:9000->9000/tcp
#   akidb-rest    running (healthy)   0.0.0.0:8080->8080/tcp
```

### Step 2: Verify Health

```bash
# REST health check
curl http://localhost:8080/health
# Expected: {"status":"healthy","version":"2.0.0-rc1"}

# gRPC health check (requires grpcurl)
grpcurl -plaintext localhost:9000 grpc.health.v1.Health/Check
# Expected: {"status": "SERVING"}
```

### Step 3: Run Smoke Tests

```bash
./scripts/smoke-test.sh

# Expected output:
# 🔥 AkiDB v2.0.0-rc1 Smoke Tests
# ================================
# ✅ PASS: All tests passed
```

## Basic Operations

### Create a Collection (REST)

```bash
curl -X POST http://localhost:8080/api/v1/collections \
  -H "Content-Type: application/json" \
  -d '{
    "name": "my-embeddings",
    "dimension": 128,
    "metric": "cosine"
  }'

# Response: {"collection_id": "550e8400-..."}
```

### Insert Vectors (REST)

```bash
curl -X POST http://localhost:8080/api/v1/collections/{COLLECTION_ID}/insert \
  -H "Content-Type: application/json" \
  -d '{
    "doc_id": "doc-1",
    "vector": [0.1, 0.2, 0.3, ...],  # 128 dimensions
    "external_id": "my-doc-1"
  }'

# Response: {"doc_id": "doc-1", "latency_ms": 1.2}
```

### Query Vectors (REST)

```bash
curl -X POST http://localhost:8080/api/v1/collections/{COLLECTION_ID}/query \
  -H "Content-Type: application/json" \
  -d '{
    "query_vector": [0.1, 0.2, 0.3, ...],  # 128 dimensions
    "top_k": 10
  }'

# Response:
# {
#   "matches": [
#     {"doc_id": "doc-1", "distance": 0.95, "external_id": "my-doc-1"},
#     ...
#   ],
#   "latency_ms": 2.3
# }
```

### Query Vectors (gRPC)

```bash
# Requires grpcurl
grpcurl -plaintext -d '{
  "collection_id": "{COLLECTION_ID}",
  "query_vector": [0.1, 0.2, 0.3, ...],
  "top_k": 10
}' localhost:9000 akidb.collection.v1.CollectionService/Query

# Response: Same as REST, but in protobuf format
```

## Data Persistence

### Automatic Initialization

AkiDB automatically creates the required database structures on first startup:

```bash
# On first startup, you'll see:
🔍 Initializing default tenant and database...
📝 Creating default tenant...
✅ Created default tenant: 019a5f5e-c827-73a2...
📝 Creating default database...
✅ Created default database: 019a5f5e-c827-73a2...
✅ Using default database_id: 019a5f5e-c827-73a2...
```

**No manual setup required!** AkiDB creates:
- Default tenant (single-tenant mode for RC1)
- Default database (collections stored here)
- SQLite metadata database (ACID guarantees)

### Collection Persistence

Collections persist to SQLite and survive server restarts:

```bash
# Create a collection
curl -X POST http://localhost:8080/api/v1/collections \
  -d '{"name":"persistent-collection","dimension":128,"metric":"cosine"}'

# Restart server
docker-compose restart akidb-rest

# Collection is still there!
curl http://localhost:8080/api/v1/collections
# Returns: {"collections":[{"name":"persistent-collection",...}]}
```

**What's Persisted:**
- ✅ Collection metadata (name, dimension, metric, created_at)
- ✅ Vector indexes (rebuilt on startup from metadata)
- ⏸️ Vector data (coming in Phase 5 with S3/MinIO)

**Database Location:**
- Docker: `/data/akidb/akidb.db` (mount as volume for durability)
- Native: `./akidb.db` (configurable via `AKIDB_DB_PATH`)

### Volume Configuration (Recommended for Production)

```yaml
# docker-compose.yaml
services:
  akidb-rest:
    volumes:
      - akidb-data:/data/akidb  # Persistent storage

volumes:
  akidb-data:
    driver: local
```

**Benefits:**
- Collections survive container restarts
- Data persists across deployments
- Easy backups (just copy the volume)

## Configuration

### Default Configuration

- **gRPC Port:** 9000 (recommended for production)
- **REST Port:** 8080 (compatibility mode)
- **Data Directory:** `/data/akidb` (in containers)
- **Metadata DB:** `/data/akidb/akidb.db` (SQLite)
- **Log Level:** info

### Custom Configuration

```bash
# Edit docker-compose.yaml
nano docker-compose.yaml

# Change environment variables:
# - RUST_LOG=debug (for more logging)
# - AKIDB_PORT=9001 (custom port)
# - AKIDB_DB_PATH=sqlite:///custom/path/db.db (custom database location)

# Restart services
docker-compose restart
```

**Environment Variables:**
- `AKIDB_HOST`: Bind address (default: `0.0.0.0`)
- `AKIDB_PORT`: Server port (REST: 8080, gRPC: 9000)
- `AKIDB_DB_PATH`: SQLite database path (default: `sqlite://akidb.db`)
- `RUST_LOG`: Log level (`error`, `warn`, `info`, `debug`, `trace`)

## Performance Expectations

**Hardware:** Apple M3, 16GB RAM, 128-dim vectors

| Operation | Latency (P95) | Throughput |
|-----------|---------------|------------|
| Insert (1 vector) | <2ms | 500 QPS |
| Query (k=10, 1k vectors) | <3ms | 300 QPS |
| Query (k=10, 100k vectors) | <3ms | 300 QPS (HNSW) |

## Next Steps

- **Full Documentation:** [docs/](./docs/)
- **API Reference:** [docs/API-REFERENCE.md](./API-REFERENCE.md)
- **Migration Guide:** [docs/MIGRATION-V1-TO-V2.md](./MIGRATION-V1-TO-V2.md)
- **Examples:** [examples/](../examples/)

## Troubleshooting

### Ports Already in Use

```bash
# Check what's using the port
lsof -i :9000
lsof -i :8080

# Change ports in docker-compose.yaml
# or stop conflicting services
```

### Containers Not Starting

```bash
# Check logs
docker-compose logs akidb-grpc
docker-compose logs akidb-rest

# Check disk space
df -h

# Restart with clean state
docker-compose down -v
docker-compose up -d
```

### Connection Refused

```bash
# Ensure services are running
docker-compose ps

# Wait for health checks to pass
docker-compose ps | grep healthy

# Check firewall (if on remote host)
sudo ufw status
sudo ufw allow 9000/tcp
sudo ufw allow 8080/tcp
```

## Support

- **GitHub Issues:** https://github.com/akidb/akidb2/issues
- **Documentation:** https://docs.akidb.io
- **Discord:** Coming soon

## License

Apache 2.0 - See [LICENSE](../LICENSE) for details.
