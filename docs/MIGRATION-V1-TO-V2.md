# Migration Guide: AkiDB v1.x → v2.0

## Quick Reference

| Aspect | v1.x | v2.0 | Migration Tool |
|--------|------|------|----------------|
| API Protocol | HTTP/JSON | gRPC + REST | Manual (client code) |
| Port | 8000 | 9000 (gRPC), 8080 (REST) | Config change |
| IDs | Slug | UUID v7 | Automated |
| Schema | Flat | Hierarchical | Automated |
| Config | JSON | TOML | Manual |
| Auth | External | Built-in RBAC | Manual setup |

**Estimated Migration Time:** 2-4 hours (depends on dataset size)

**Risk Level:** 🟡 MEDIUM (automated tool + rollback available)

---

## Overview

This guide walks you through migrating from AkiDB v1.x to v2.0.

**What's Changing:**
1. API protocol (HTTP → gRPC recommended, REST available)
2. Data schema (Tenants → Collections becomes Tenants → Databases → Collections)
3. Configuration format (JSON → TOML)
4. ID format (slugs → UUIDs)
5. Built-in authentication (external → RBAC)

**What's Preserved:**
- Your vector data (automatically migrated)
- Tenant metadata (automatically migrated)
- Collection configurations (automatically migrated)

---

## Prerequisites

**Before You Begin:**
1. Backup v1.x data (REQUIRED)
2. Install Docker or build v2.0 from source
3. Allocate 2-4 hours for migration
4. Test in development environment first

**System Requirements:**
- Rust 1.75+ (if building from source)
- Docker 20.10+ (if using containers)
- 2x current data size in disk space (temporary)
- Same RAM as v1.x deployment

---

## Step-by-Step Migration

### Step 1: Backup v1.x Data (5 minutes, CRITICAL)

**Why:** If migration fails, you can rollback to v1.x

```bash
# Backup data directory
tar -czf akidb-v1-backup-$(date +%Y%m%d-%H%M%S).tar.gz \
  /path/to/v1/data

# Backup metadata (if using SQLite in v1.x)
sqlite3 /path/to/v1/metadata.db ".dump" > metadata-v1-backup.sql

# Verify backups
ls -lh akidb-v1-backup-*.tar.gz
ls -lh metadata-v1-backup.sql

# Store backups in safe location
mv akidb-v1-backup-*.tar.gz /path/to/safe/location/
mv metadata-v1-backup.sql /path/to/safe/location/
```

**✅ Verification:** Backup files exist and are readable

---

### Step 2: Install AkiDB v2.0 (2 minutes)

**Option A: Docker (Recommended)**
```bash
# Pull images
docker pull akidb/akidb-grpc:2.0.0-rc1
docker pull akidb/akidb-rest:2.0.0-rc1

# Verify
docker images | grep akidb
```

**Option B: Build from Source**
```bash
# Clone repository
git clone https://github.com/akiradb/akidb2.git
cd akidb2
git checkout v2.0.0-rc1

# Build release binaries
cargo build --release --workspace

# Verify
./target/release/akidb-grpc --version
# Output: akidb-grpc 2.0.0-rc1
```

---

### Step 3: Run Migration Tool (30-120 minutes, depends on data size)

**Dry-Run (RECOMMENDED FIRST):**
```bash
# Preview changes without modifying data
akidb-cli migrate v1-to-v2 \
  --v1-data-dir /path/to/v1/data \
  --v2-database /path/to/v2/metadata.db \
  --dry-run

# Example output:
# 📋 Dry-run mode: No data will be modified
#
# Tenants to migrate: 5
#   - tenant-1 (ID: 550e8400-...) → slug: tenant-1
#   - tenant-2 (ID: 650e8400-...) → slug: tenant-2
#   ...
#
# Collections to migrate: 12
#   - embeddings-prod (dim: 128, metric: cosine) → database: default
#   - images-dev (dim: 512, metric: l2) → database: default
#   ...
#
# Vectors to migrate: 1,234,567
#   Estimated time: 25 minutes
#   Estimated disk space: 4.2 GB
#
# ✅ Dry-run complete. No data modified.
# 💡 Run without --dry-run to execute migration.
```

**Execute Migration:**
```bash
# Run actual migration
akidb-cli migrate v1-to-v2 \
  --v1-data-dir /path/to/v1/data \
  --v2-database /path/to/v2/metadata.db

# Example output:
# 🚀 Starting migration: v1.x → v2.0
#
# [1/4] Migrating tenants...
#   ✅ tenant-1 (550e8400-...)
#   ✅ tenant-2 (650e8400-...)
#   ...
#   Migrated: 5/5 tenants (1.2s)
#
# [2/4] Creating default databases...
#   ✅ default database for tenant-1
#   ✅ default database for tenant-2
#   ...
#   Created: 5/5 databases (0.5s)
#
# [3/4] Migrating collections...
#   ✅ embeddings-prod (128-dim, cosine)
#   ✅ images-dev (512-dim, l2)
#   ...
#   Migrated: 12/12 collections (2.1s)
#
# [4/4] Migrating vectors...
#   Progress: [████████████████████] 1,234,567/1,234,567 (100%)
#   Migrated: 1,234,567 vectors (24m 32s)
#
# ✅ Migration complete!
#
# Summary:
#   Tenants: 5
#   Databases: 5 (auto-created)
#   Collections: 12
#   Vectors: 1,234,567
#   Duration: 24 minutes 36 seconds
#   Database size: 4.1 GB
```

**Post-Migration Validation:**
```bash
# Verify migration
akidb-cli verify \
  --v1-data-dir /path/to/v1/data \
  --v2-database /path/to/v2/metadata.db

# Example output:
# 🔍 Verifying migration...
#
# ✅ Tenant count matches: 5
# ✅ Collection count matches: 12
# ✅ Vector count matches: 1,234,567
# ✅ Metadata integrity check passed
# ✅ Foreign key constraints valid
#
# ✅ Migration verified successfully!
```

---

### Step 4: Update Configuration (10 minutes)

**v1.x config.json:**
```json
{
  "port": 8000,
  "data_dir": "/data/akidb",
  "log_level": "info",
  "max_connections": 100
}
```

**v2.0 config/akidb.toml:**
```toml
# AkiDB v2.0 Configuration

[server]
# gRPC server (recommended for production)
grpc_host = "0.0.0.0"
grpc_port = 9000

# REST server (compatibility mode)
rest_host = "0.0.0.0"
rest_port = 8080

[storage]
# SQLite metadata database
metadata_db = "/data/akidb/metadata.db"

# Vector data directory (in-memory, rc1 only)
data_dir = "/data/akidb/vectors"

[performance]
# Connection pooling
max_connections = 100
min_connections = 2

# Query timeouts
query_timeout_ms = 5000
insert_timeout_ms = 10000

[logging]
# Log level: trace, debug, info, warn, error
level = "info"

# Log format: json, pretty
format = "json"

# Log file (optional, stdout if not set)
# file = "/var/log/akidb/akidb.log"

[limits]
# Resource quotas (per tenant)
default_memory_quota_bytes = 10737418240  # 10 GB
default_storage_quota_bytes = 107374182400  # 100 GB
default_qps_quota = 100

# Global limits
max_vector_dimension = 4096
max_batch_size = 1000
```

**Create config file:**
```bash
mkdir -p /etc/akidb
cp config/akidb.toml.example /etc/akidb/akidb.toml
nano /etc/akidb/akidb.toml  # Edit as needed
```

---

### Step 5: Start v2.0 Servers (5 minutes)

**Option A: Docker Compose (Recommended)**
```bash
# Start services
docker-compose up -d

# Check logs
docker-compose logs -f

# Verify health
curl http://localhost:8080/health
# Expected: {"status":"healthy","version":"2.0.0-rc1"}

grpcurl -plaintext localhost:9000 grpc.health.v1.Health/Check
# Expected: {"status": "SERVING"}
```

**Option B: Manual Start**
```bash
# Start gRPC server
./target/release/akidb-grpc \
  --config /etc/akidb/akidb.toml \
  --host 0.0.0.0 \
  --port 9000 &

# Start REST server
./target/release/akidb-rest \
  --config /etc/akidb/akidb.toml \
  --host 0.0.0.0 \
  --port 8080 &

# Check processes
ps aux | grep akidb
```

---

### Step 6: Update Client Code (30-60 minutes)

**v1.x Python Client:**
```python
import requests

# v1.x HTTP/JSON API
response = requests.post(
    "http://localhost:8000/api/search",
    json={
        "tenant": "my-tenant",
        "collection": "embeddings",
        "query": [0.1, 0.2, ...],
        "k": 10
    }
)
results = response.json()
```

**v2.0 Python Client (gRPC):**
```python
import grpc
from akidb.collection.v1 import collection_pb2, collection_pb2_grpc

# v2.0 gRPC API
channel = grpc.insecure_channel('localhost:9000')
stub = collection_pb2_grpc.CollectionServiceStub(channel)

# Query vectors
response = stub.Query(collection_pb2.QueryRequest(
    collection_id="550e8400-e29b-41d4-a716-446655440000",
    query_vector=[0.1, 0.2, ...],
    top_k=10
))

for match in response.matches:
    print(f"Doc: {match.doc_id}, Distance: {match.distance}")
```

**v2.0 Python Client (REST - Compatibility):**
```python
import requests

# v2.0 REST API (similar to v1.x)
response = requests.post(
    "http://localhost:8080/api/v1/collections/550e8400-e29b-41d4-a716-446655440000/query",
    json={
        "query_vector": [0.1, 0.2, ...],
        "top_k": 10
    }
)
results = response.json()
```

---

### Step 7: Validation & Testing (15 minutes)

**Smoke Tests:**
```bash
# Test gRPC insert
grpcurl -plaintext -d '{
  "collection_id": "YOUR_COLLECTION_ID",
  "doc_id": "test-doc-1",
  "vector": [0.1, 0.2, 0.3, ...]
}' localhost:9000 akidb.collection.v1.CollectionService/Insert

# Test gRPC query
grpcurl -plaintext -d '{
  "collection_id": "YOUR_COLLECTION_ID",
  "query_vector": [0.1, 0.2, 0.3, ...],
  "top_k": 5
}' localhost:9000 akidb.collection.v1.CollectionService/Query

# Test REST query
curl -X POST http://localhost:8080/api/v1/collections/YOUR_COLLECTION_ID/query \
  -H "Content-Type: application/json" \
  -d '{
    "query_vector": [0.1, 0.2, 0.3, ...],
    "top_k": 5
  }'
```

**Performance Validation:**
```bash
# Compare v1.x vs v2.0 latency
# Expected: v2.0 should be similar or better
```

---

## Rollback Procedure

**If migration fails or v2.0 has issues:**

```bash
# Step 1: Stop v2.0 servers
docker-compose down
# or
killall akidb-grpc akidb-rest

# Step 2: Restore v1.x data
cd /path/to/safe/location
tar -xzf akidb-v1-backup-YYYYMMDD-HHMMSS.tar.gz -C /

# Step 3: Restore v1.x metadata (if applicable)
sqlite3 /path/to/v1/metadata.db < metadata-v1-backup.sql

# Step 4: Restart v1.x
docker-compose -f docker-compose-v1.yml up -d
# or start v1.x manually

# Step 5: Verify v1.x works
curl http://localhost:8000/api/health
```

---

## Troubleshooting

### Issue: Migration tool fails with "dimension mismatch"

**Cause:** v1.x collections have inconsistent vector dimensions

**Solution:**
```bash
# Identify problematic collections
akidb-cli validate v1-to-v2 \
  --v1-data-dir /path/to/v1/data

# Fix or exclude problematic collections
akidb-cli migrate v1-to-v2 \
  --v1-data-dir /path/to/v1/data \
  --v2-database /path/to/v2/metadata.db \
  --exclude-collection problematic-collection-1 \
  --exclude-collection problematic-collection-2
```

### Issue: "Cannot connect to gRPC server"

**Cause:** Firewall or port conflict

**Solution:**
```bash
# Check if port is in use
lsof -i :9000

# Check firewall
sudo ufw status

# Allow port
sudo ufw allow 9000/tcp
```

### Issue: Migration is very slow (>2 hours for 1M vectors)

**Cause:** Disk I/O bottleneck

**Solution:**
```bash
# Use SSD for temporary migration files
akidb-cli migrate v1-to-v2 \
  --v1-data-dir /path/to/v1/data \
  --v2-database /mnt/ssd/metadata.db \
  --batch-size 10000  # Larger batches = fewer writes
```

---

## Post-Migration Checklist

- [ ] Migration completed successfully (no errors)
- [ ] Verification passed (counts match)
- [ ] v2.0 servers started and healthy
- [ ] Client code updated and tested
- [ ] Smoke tests passing
- [ ] Performance validation passed
- [ ] v1.x backups stored safely
- [ ] Documentation updated (API endpoints, ports)
- [ ] Team notified of migration
- [ ] Monitoring configured (optional for RC1)

---

## Support

**Getting Help:**
- GitHub Issues: https://github.com/akiradb/akidb2/issues
- Documentation: https://docs.akidb.io
- Discord: https://discord.gg/akidb (coming soon)

**Reporting Migration Issues:**
```bash
# Include this info in bug reports
akidb-cli version
akidb-cli migrate v1-to-v2 --v1-data-dir /path/to/v1/data --dry-run --verbose
cat /var/log/akidb/migration.log
```

---

## FAQ

**Q: Can I run v1.x and v2.0 side-by-side?**
A: Yes! They use different ports (v1.x: 8000, v2.0 gRPC: 9000, REST: 8080)

**Q: How long does migration take?**
A: ~50 vectors/second. For 1M vectors: ~6 hours. Use `--dry-run` for estimate.

**Q: Is v2.0 backward compatible?**
A: No. You must migrate data and update client code.

**Q: Can I skip the REST API and use only gRPC?**
A: Yes! REST is optional. gRPC is recommended for production.

**Q: What happens to v1.x after migration?**
A: v1.x remains functional. You can keep it running until v2.0 is validated.

**Q: Do I need to migrate all tenants at once?**
A: No. You can migrate tenants incrementally using `--tenant` flag.

**Q: How do I migrate back from v2.0 to v1.x?**
A: Use your v1.x backups (see Rollback Procedure above).

---

**Migration Support:** For migration assistance, open an issue on GitHub with the `migration` label.
