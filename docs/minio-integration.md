# MinIO Integration Guide

AkiDB is designed as a **MinIO-native vector database** with deep integration into MinIO's security, durability, and lifecycle features. This guide covers deployment patterns, security configuration, and operational best practices for MinIO-first environments.

---

## 1. Architecture Overview

### MinIO-First Design Philosophy

AkiDB treats MinIO as a **first-class storage backend**, not just "S3-compatible" object storage. We leverage MinIO-specific features:

- **KES/Vault Integration** for encryption key management
- **Object Lock (WORM)** for immutable index segments
- **Versioning** for forensic rollback and snapshot management
- **Site Replication** for multi-site DR and geo-distribution
- **Bucket Notifications** for event-driven index rebuilds
- **ILM Policies** for automatic tier transitions (Hot → Warm → Cold)

### Reference Architecture

```
┌────────────────────────────────────────────────────────────────┐
│                      AkiDB Stateless Cluster                   │
│                  (3+ nodes, load balanced)                     │
│                                                                │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │  AkiDB Node  │  │  AkiDB Node  │  │  AkiDB Node  │        │
│  │              │  │              │  │              │        │
│  │  Hot Cache:  │  │  Hot Cache:  │  │  Hot Cache:  │        │
│  │  NVMe (SSD)  │  │  NVMe (SSD)  │  │  NVMe (SSD)  │        │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘        │
│         │                  │                  │                │
└─────────┼──────────────────┼──────────────────┼────────────────┘
          │                  │                  │
          └──────────────────┼──────────────────┘
                             ↓
          ┌─────────────────────────────────────────────────┐
          │         MinIO Distributed Cluster               │
          │         (Erasure Coding: 12D+4P)                │
          │                                                 │
          │  ┌─────────┐  ┌─────────┐  ┌─────────┐        │
          │  │ MinIO 1 │  │ MinIO 2 │  │ MinIO N │        │
          │  │ HDD/SSD │  │ HDD/SSD │  │ HDD/SSD │        │
          │  └─────────┘  └─────────┘  └─────────┘        │
          │                                                 │
          │  Security:  KES/Vault (SSE-KMS)                │
          │  Durability: Object Lock, Versioning           │
          │  Events:    Bucket Notifications → NATS        │
          └─────────────────────────────────────────────────┘
```

---

## 2. Security & Compliance

### 2.1 Encryption at Rest (SSE-KMS)

AkiDB supports MinIO's Server-Side Encryption with KES (Key Encryption Service).

#### Setup: MinIO + KES + HashiCorp Vault

**1. Deploy KES with Vault backend:**

```bash
# kes-config.yaml
address: 0.0.0.0:7373
root: disabled

tls:
  key: /certs/kes-server.key
  cert: /certs/kes-server.cert

policy:
  akidb-policy:
    allow:
      - /v1/key/create/akidb-*
      - /v1/key/generate/akidb-*
      - /v1/key/decrypt/akidb-*

keys:
  - name: akidb-master-key

keystore:
  vault:
    endpoint: https://vault.example.com:8200
    approle:
      id: "kes-approle-id"
      secret: "kes-approle-secret"
    prefix: "akidb/"
```

**2. Configure MinIO to use KES:**

```bash
# Set MinIO environment variables
export MINIO_KMS_KES_ENDPOINT=https://kes.example.com:7373
export MINIO_KMS_KES_KEY_FILE=/certs/kes-client.key
export MINIO_KMS_KES_CERT_FILE=/certs/kes-client.cert
export MINIO_KMS_KES_KEY_NAME=akidb-master-key

# Start MinIO
minio server /data --console-address :9001
```

**3. Configure AkiDB to use SSE-KMS:**

```bash
# .env
AKIDB_S3_ENDPOINT=https://minio.example.com:9000
AKIDB_S3_BUCKET=akidb
AKIDB_S3_REGION=us-east-1
AKIDB_S3_ACCESS_KEY=akidb-service-account
AKIDB_S3_SECRET_KEY=<secret>
AKIDB_S3_ENCRYPTION=sse-kms
AKIDB_S3_KMS_KEY_ID=akidb-master-key
```

**Result:** All segments and manifests are encrypted at rest with Vault-managed keys.

---

### 2.2 Object Lock (WORM) for Immutable Segments

Enable **Write-Once Read-Many** mode to prevent tampering with sealed index segments.

#### Enable Object Lock on Bucket

```bash
# Create bucket with Object Lock enabled
mc mb --with-lock minio/akidb

# Set default retention (e.g., 7 days)
mc retention set --default GOVERNANCE 7d minio/akidb
```

#### AkiDB Behavior

- **Active Segments:** Written normally (no lock).
- **Sealed Segments:** When sealing, AkiDB sets Object Lock with retention period.
- **Manifests:** Locked after commit to prevent rollback attacks.

**Configuration:**

```bash
# .env
AKIDB_S3_OBJECT_LOCK_ENABLED=true
AKIDB_S3_OBJECT_LOCK_RETENTION_DAYS=30
AKIDB_S3_OBJECT_LOCK_MODE=GOVERNANCE  # or COMPLIANCE
```

**Compliance Impact:**
- **Forensic Integrity:** Sealed segments cannot be deleted or modified within retention period.
- **Audit Trail:** Manifest versions are immutable, providing provable audit chain.

---

### 2.3 Versioning for Forensic Rollback

MinIO versioning allows AkiDB to maintain snapshot history for rollback and forensic analysis.

#### Enable Versioning

```bash
mc version enable minio/akidb
```

#### AkiDB Snapshot API

```bash
# Create a named snapshot
curl -X POST http://localhost:8080/collections/my_collection/snapshots \
  -H "Content-Type: application/json" \
  -d '{"name": "pre-migration-snapshot"}'

# List snapshots
curl http://localhost:8080/collections/my_collection/snapshots

# Revert to snapshot
curl -X POST http://localhost:8080/collections/my_collection/snapshots/pre-migration-snapshot/revert
```

**Use Cases:**
- **Bad Data Rollback:** Revert after ingesting corrupted embeddings.
- **Forensic Analysis:** Replay queries against historical index versions.
- **Compliance Audits:** Prove data state at specific timestamps.

---

### 2.4 Legal Hold for Litigation

MinIO Legal Hold prevents deletion of specific objects during legal proceedings.

```bash
# Set legal hold on specific segment
mc legalhold set minio/akidb/collections/sensitive_docs/segments/seg_123.bin

# AkiDB automatically respects legal hold during segment cleanup
```

---

## 3. Storage Optimization

### 3.1 Tiered Caching (Hot/Warm/Cold)

AkiDB implements a 3-tier caching strategy to balance performance and cost.

```
HOT:  Local NVMe SSD (LRU cache, pinned segments)  →  P95 < 5ms
WARM: RocksDB/DuckDB (segment metadata, bloom filters) →  P95 < 50ms
COLD: MinIO HDD/Tape (compressed segments)  →  P95 < 500ms
```

#### Configuration

```bash
# .env
# Hot tier (local cache)
AKIDB_CACHE_HOT_SIZE_GB=100
AKIDB_CACHE_HOT_PATH=/mnt/nvme/akidb-cache

# Warm tier (metadata store)
AKIDB_CACHE_WARM_BACKEND=rocksdb
AKIDB_CACHE_WARM_PATH=/var/lib/akidb/warm

# Cold tier (MinIO)
AKIDB_CACHE_COLD_BACKEND=minio
AKIDB_CACHE_COLD_COMPRESSION=zstd
AKIDB_CACHE_COLD_COMPRESSION_LEVEL=9
```

#### Cache Pinning

Pin frequently accessed collections to Hot tier:

```bash
curl -X POST http://localhost:8080/collections/critical_collection/cache/pin
```

---

### 3.2 Segment Merging & Multipart Uploads

**Problem:** Small segments create excessive S3 API calls.

**Solution:** Merge segments and use multipart uploads.

```bash
# .env
AKIDB_SEGMENT_MIN_SIZE_MB=32
AKIDB_SEGMENT_TARGET_SIZE_MB=128
AKIDB_SEGMENT_MERGE_ENABLED=true

# Multipart upload (for segments > 64MB)
AKIDB_S3_MULTIPART_THRESHOLD_MB=64
AKIDB_S3_MULTIPART_CHUNK_SIZE_MB=16
AKIDB_S3_MULTIPART_CONCURRENCY=4
```

**Result:** Fewer S3 API calls, better upload throughput.

---

### 3.3 Range GET Pre-Fetching

For sparse reads (e.g., metadata lookups), use HTTP Range requests to fetch only needed bytes.

```bash
# .env
AKIDB_S3_RANGE_GET_ENABLED=true
AKIDB_S3_RANGE_PREFETCH_KB=256  # Pre-fetch 256KB blocks
```

---

## 4. Events & Automation

### 4.1 MinIO Bucket Notifications → NATS

Trigger index rebuilds when new data arrives in MinIO.

#### Setup: MinIO → NATS

**1. Configure MinIO bucket notification:**

```bash
mc admin config set minio notify_nats:1 \
  address="nats://nats.example.com:4222" \
  subject="akidb.events" \
  username="minio" \
  password="<password>"

mc admin service restart minio

# Enable notification for bucket
mc event add minio/akidb arn:minio:sqs::1:nats --event put,delete
```

**2. AkiDB subscribes to NATS events:**

```bash
# .env
AKIDB_EVENTS_ENABLED=true
AKIDB_EVENTS_BACKEND=nats
AKIDB_EVENTS_NATS_URL=nats://nats.example.com:4222
AKIDB_EVENTS_NATS_SUBJECT=akidb.events
```

**Workflow:**
1. User uploads embeddings to `minio/akidb/ingest/`
2. MinIO sends `PUT` event to NATS
3. AkiDB worker picks up event and triggers index rebuild
4. New segments written to `minio/akidb/collections/`

---

### 4.2 ILM Policies for Tier Transitions

MinIO lifecycle policies automatically transition objects between storage classes.

```bash
# ilm-policy.json
{
  "Rules": [
    {
      "ID": "hot-to-warm",
      "Status": "Enabled",
      "Filter": {
        "Prefix": "collections/"
      },
      "Transitions": [
        {
          "Days": 7,
          "StorageClass": "WARM"
        },
        {
          "Days": 30,
          "StorageClass": "COLD"
        }
      ]
    }
  ]
}

mc ilm import minio/akidb < ilm-policy.json
```

**AkiDB automatically detects storage class and adjusts cache strategy.**

---

## 5. Durability & DR

### 5.1 Erasure Coding

MinIO Erasure Coding provides Reed-Solomon redundancy.

**Recommended:** 12 data + 4 parity drives (tolerates 4 drive failures).

```bash
# Start MinIO with EC
minio server \
  http://minio{1...16}.example.com/mnt/disk{1...4}/data
```

**AkiDB benefits:**
- No application-level replication needed
- Automatic repair of corrupted segments
- Better storage efficiency than 3x replication

---

### 5.2 Site Replication for Multi-Site DR

MinIO Site Replication synchronizes buckets across geographically distributed clusters.

```bash
# Site 1 (Primary)
mc admin replicate add minio1 minio2 \
  --deployment-id site1-site2-dr \
  --all

# AkiDB writes to Site 1, MinIO replicates to Site 2
```

**Use Cases:**
- **Disaster Recovery:** Failover to Site 2 if Site 1 is down
- **Geo-Distribution:** Serve queries from nearest site
- **Air-Gap Migration:** Replicate encrypted data between isolated networks

---

## 6. Observability

### 6.1 MinIO Metrics Integration

AkiDB exposes MinIO-specific metrics via Prometheus:

```
# Prometheus endpoint: http://localhost:8080/metrics

# MinIO API call latency
akidb_minio_request_duration_seconds{operation="GetObject"} histogram
akidb_minio_request_duration_seconds{operation="PutObject"} histogram

# MinIO API call count
akidb_minio_requests_total{operation="GetObject",status="success"} counter
akidb_minio_requests_total{operation="PutObject",status="error"} counter

# Cache hit rates
akidb_cache_hit_ratio{tier="hot"} gauge
akidb_cache_hit_ratio{tier="warm"} gauge
```

### 6.2 Health Checks

```bash
# Kubernetes liveness probe
curl http://localhost:8080/health/live

# Readiness probe (checks MinIO connectivity)
curl http://localhost:8080/health/ready

# Response includes MinIO status
{
  "status": "healthy",
  "minio": {
    "reachable": true,
    "bucket_exists": true,
    "encryption_enabled": true,
    "versioning_enabled": true
  }
}
```

---

## 7. Best Practices

### 7.1 Deployment Checklist

- [ ] Enable SSE-KMS with KES/Vault
- [ ] Enable Object Lock for immutable segments
- [ ] Enable Versioning for snapshots
- [ ] Configure Erasure Coding (12D+4P or higher)
- [ ] Set up Site Replication for DR
- [ ] Configure ILM policies for tier transitions
- [ ] Enable Bucket Notifications to NATS (if using events)
- [ ] Size Hot cache to achieve ≥80% hit rate
- [ ] Monitor MinIO API call latency via Prometheus
- [ ] Test snapshot/revert procedures

### 7.2 Performance Tuning

1. **Cache Sizing:** Target 80%+ hot cache hit rate. Monitor `akidb_cache_hit_ratio`.
2. **Parallelism:** Increase multipart concurrency for large segments.
3. **Connection Pooling:** Use persistent connections to MinIO (default: enabled).
4. **Compression:** Adjust Zstd level (1-22) based on CPU/network tradeoff.

### 7.3 Security Hardening

1. **Least Privilege:** AkiDB service account should only have `s3:GetObject`, `s3:PutObject`, `s3:ListBucket`.
2. **Network Isolation:** Run MinIO and AkiDB in private subnet, no public internet access.
3. **Audit Logging:** Enable MinIO audit logs and ship to SIEM.

---

## 8. Troubleshooting

### 8.1 Common Issues

**Issue:** AkiDB cannot connect to MinIO

```bash
# Check health endpoint
curl http://localhost:8080/health/ready

# Response:
{
  "status": "unhealthy",
  "minio": {
    "reachable": false,
    "error": "connection refused"
  }
}

# Solution: Verify MinIO endpoint and credentials in .env
```

**Issue:** Segments not encrypted

```bash
# Verify SSE-KMS is enabled
mc stat minio/akidb/collections/test/segments/seg_001.bin

# Should show: X-Amz-Server-Side-Encryption: aws:kms
```

**Issue:** High MinIO API call costs

```bash
# Check segment size distribution
curl http://localhost:8080/metrics | grep akidb_segment_size_bytes

# If segments are < 32MB, enable segment merging
AKIDB_SEGMENT_MERGE_ENABLED=true
```

---

## 9. References

- **MinIO Documentation:** https://min.io/docs
- **KES Documentation:** https://github.com/minio/kes/wiki
- **MinIO Site Replication:** https://min.io/docs/minio/linux/operations/install-deploy-manage/multi-site-replication.html
- **Object Locking:** https://min.io/docs/minio/linux/administration/object-management/object-retention.html
