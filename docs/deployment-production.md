# AkiDB Production Deployment Guide

**Version:** 1.0
**Last Updated:** 2025-11-03
**Target:** Production deployments on ARM platforms

---

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Deployment Architecture](#deployment-architecture)
3. [Installation](#installation)
4. [Configuration](#configuration)
5. [Running the Service](#running-the-service)
6. [Monitoring & Observability](#monitoring--observability)
7. [Security](#security)
8. [High Availability](#high-availability)
9. [Troubleshooting](#troubleshooting)

---

## Prerequisites

### Hardware Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| **ARM CPU** | 4 cores | 8+ cores (Apple Silicon M2/M3, Graviton 3/4, Jetson Orin) |
| **RAM** | 8 GB | 16+ GB |
| **Storage (NVMe)** | 100 GB | 500+ GB (hot tier cache) |
| **Storage (S3/MinIO)** | Unlimited | Based on data volume |
| **Network** | 1 Gbps | 10 Gbps |

### Software Requirements

- **OS:** Linux ARM64 or macOS ARM64 (Apple Silicon)
  - Ubuntu 22.04+ ARM64
  - RHEL/Rocky Linux 9+ ARM64
  - macOS 13+ (Ventura or later)
- **Rust:** 1.77+ (for building from source)
- **MinIO:** Latest version (or S3-compatible storage)
- **Docker:** 20.10+ (optional, for containerized deployment)

---

## Deployment Architecture

### Single-Node Deployment

```
┌──────────────────────────────────────────────────────┐
│  ARM Server (Mac Mini, Jetson, Graviton instance)   │
├──────────────────────────────────────────────────────┤
│                                                      │
│  ┌────────────┐    ┌──────────────┐                 │
│  │ AkiDB API  │───▶│ MinIO (S3)   │                 │
│  │ :8080      │    │ :9000        │                 │
│  └────────────┘    └──────────────┘                 │
│         │                                            │
│         └─────────▶ Local NVMe Cache                │
│                     /var/lib/akidb/cache             │
└──────────────────────────────────────────────────────┘
```

### Multi-Node High Availability

```
┌────────────────┐       ┌────────────────┐
│  AkiDB Node 1  │       │  AkiDB Node 2  │
│  (Primary)     │       │  (Standby)     │
└────────┬───────┘       └────────┬───────┘
         │                        │
         └────────┬───────────────┘
                  │
         ┌────────▼──────────┐
         │  MinIO Cluster    │
         │  (Distributed)    │
         │  - 4+ nodes       │
         │  - Erasure coding │
         └───────────────────┘
```

---

## Installation

### Option 1: Pre-built Binary (Recommended)

```bash
# Download latest release for ARM64
VERSION=0.4.0
ARCH=aarch64-unknown-linux-gnu  # or aarch64-apple-darwin for macOS

wget https://github.com/aifocal/akidb/releases/download/v${VERSION}/akidb-api-${ARCH}.tar.gz

# Extract
tar -xzf akidb-api-${ARCH}.tar.gz

# Install to /usr/local/bin
sudo cp akidb-api /usr/local/bin/
sudo chmod +x /usr/local/bin/akidb-api

# Verify installation
akidb-api --version
```

### Option 2: Build from Source

```bash
# Clone repository
git clone https://github.com/aifocal/akidb.git
cd akidb

# Build release binary
./scripts/build-release.sh

# Binary will be at: target/release/akidb-api
sudo cp target/release/akidb-api /usr/local/bin/
```

### Option 3: Docker Container

```bash
# Pull official image
docker pull ghcr.io/aifocal/akidb:latest

# Or build locally
docker build -t akidb:latest -f docker/Dockerfile .
```

---

## Configuration

### Environment Variables

Create `/etc/akidb/akidb.env`:

```bash
# === S3 Storage Configuration ===
AKIDB_S3_ENDPOINT=http://minio:9000
AKIDB_S3_REGION=us-east-1
AKIDB_S3_BUCKET=akidb
AKIDB_S3_ACCESS_KEY=your_access_key
AKIDB_S3_SECRET_KEY=your_secret_key

# === API Server Configuration ===
AKIDB_BIND_ADDRESS=0.0.0.0:8080
AKIDB_LOG_LEVEL=info

# === OpenTelemetry / Jaeger ===
AKIDB_TELEMETRY_ENABLED=true
AKIDB_JAEGER_ENDPOINT=http://jaeger:4317
AKIDB_SERVICE_NAME=akidb-api
AKIDB_SAMPLING_RATIO=1.0  # Sample all traces (adjust for production)

# === Memory Backend (for testing only) ===
AKIDB_USE_MEMORY_BACKEND=false
```

### YAML Configuration (Optional)

Create `/etc/akidb/akidb.yaml`:

```yaml
storage:
  circuit_breaker:
    failure_threshold: 5
    recovery_timeout_secs: 30
  retry:
    max_attempts: 10
    initial_backoff_ms: 100
    max_backoff_ms: 5000
    backoff_multiplier: 2.0
  manifest_retry:
    max_attempts: 20
    initial_backoff_ms: 50
    max_backoff_ms: 2000

index:
  hnsw:
    m: 16
    ef_construction: 400
    ef_search: 200
    min_vectors_threshold: 100

api:
  validation:
    collection_name_max_length: 255
    vector_dimension_max: 4096
    top_k_max: 1000

query:
  max_filter_depth: 32
  max_filter_clauses: 100
  parallel_segments: true
  max_parallel_segments: 8
```

---

## Running the Service

### Systemd Service (Linux)

Create `/etc/systemd/system/akidb.service`:

```ini
[Unit]
Description=AkiDB Vector Database API
After=network.target minio.service
Requires=minio.service

[Service]
Type=simple
User=akidb
Group=akidb
EnvironmentFile=/etc/akidb/akidb.env
ExecStart=/usr/local/bin/akidb-api
Restart=on-failure
RestartSec=5s
TimeoutStopSec=30s

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/akidb

# Resource limits
LimitNOFILE=65536
MemoryMax=8G
CPUQuota=400%  # 4 cores

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
# Create user
sudo useradd -r -s /bin/false akidb

# Create directories
sudo mkdir -p /var/lib/akidb/cache
sudo chown -R akidb:akidb /var/lib/akidb

# Enable service
sudo systemctl daemon-reload
sudo systemctl enable akidb
sudo systemctl start akidb

# Check status
sudo systemctl status akidb
sudo journalctl -u akidb -f
```

### Launchd Service (macOS)

Create `~/Library/LaunchAgents/com.aifocal.akidb.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.aifocal.akidb</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/akidb-api</string>
    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>AKIDB_S3_ENDPOINT</key>
        <string>http://localhost:9000</string>
        <key>AKIDB_S3_ACCESS_KEY</key>
        <string>your_key</string>
        <key>AKIDB_S3_SECRET_KEY</key>
        <string>your_secret</string>
    </dict>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/akidb.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/akidb.err</string>
</dict>
</plist>
```

Load and start:

```bash
launchctl load ~/Library/LaunchAgents/com.aifocal.akidb.plist
launchctl start com.aifocal.akidb
```

### Docker Compose

Create `docker-compose.yml`:

```yaml
version: '3.9'

services:
  minio:
    image: minio/minio:latest
    platform: linux/arm64
    command: server /data --console-address ":9001"
    environment:
      MINIO_ROOT_USER: akidb
      MINIO_ROOT_PASSWORD: akidbsecret
    volumes:
      - minio-data:/data
    ports:
      - "9000:9000"
      - "9001:9001"
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:9000/minio/health/live"]
      interval: 30s
      timeout: 10s
      retries: 3

  akidb:
    image: ghcr.io/aifocal/akidb:latest
    platform: linux/arm64
    depends_on:
      - minio
    environment:
      AKIDB_S3_ENDPOINT: http://minio:9000
      AKIDB_S3_ACCESS_KEY: akidb
      AKIDB_S3_SECRET_KEY: akidbsecret
      AKIDB_S3_BUCKET: akidb
      AKIDB_BIND_ADDRESS: 0.0.0.0:8080
      AKIDB_TELEMETRY_ENABLED: "true"
      AKIDB_JAEGER_ENDPOINT: http://jaeger:4317
    ports:
      - "8080:8080"
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health/live"]
      interval: 30s
      timeout: 10s
      retries: 3
    restart: unless-stopped

  jaeger:
    image: jaegertracing/all-in-one:latest
    platform: linux/arm64
    ports:
      - "16686:16686"  # Jaeger UI
      - "4317:4317"    # OTLP gRPC receiver
    environment:
      COLLECTOR_OTLP_ENABLED: "true"

volumes:
  minio-data:
```

Start services:

```bash
docker-compose up -d
docker-compose logs -f akidb
```

---

## Monitoring & Observability

### Health Checks

```bash
# Liveness probe (Kubernetes)
curl http://localhost:8080/health/live
# Returns: 200 OK (always, unless server is dead)

# Readiness probe (Kubernetes)
curl http://localhost:8080/health/ready
# Returns: 200 OK (only if dependencies are healthy)

# Detailed health status
curl http://localhost:8080/health | jq .
# Returns: JSON with component health details
```

Example detailed health response:

```json
{
  "status": "healthy",
  "version": "0.4.0",
  "uptime_seconds": 3600,
  "components": {
    "storage": {
      "status": "healthy",
      "message": "Storage backend operational"
    },
    "wal": {
      "status": "healthy",
      "message": "WAL operational"
    },
    "index": {
      "status": "healthy",
      "message": "Index provider operational"
    }
  }
}
```

### Prometheus Metrics

Metrics are exposed at `/metrics`:

```bash
curl http://localhost:8080/metrics
```

**Key Metrics:**

| Metric | Type | Description |
|--------|------|-------------|
| `akidb_api_requests_total` | Counter | Total API requests by method, endpoint, status |
| `akidb_api_request_duration_seconds` | Histogram | Request latency (P50/P95/P99) |
| `akidb_storage_operations_total` | Counter | S3 operations (get/put/delete) |
| `akidb_storage_latency_seconds` | Histogram | S3 operation latency |
| `akidb_index_search_duration_seconds` | Histogram | Index search latency |
| `akidb_wal_operations_total` | Counter | WAL operations |
| `akidb_slow_queries_total` | Counter | Slow queries detected |
| `akidb_active_connections` | Gauge | Active client connections |

### Jaeger Distributed Tracing

Access Jaeger UI at `http://localhost:16686`:

- View trace timelines for API requests
- Identify bottlenecks in query execution
- Debug slow queries across multiple services

**Example Trace:**

```
http_request (120ms)
├── search_vectors (110ms)
│   ├── load_segment (50ms)
│   ├── hnsw_search (40ms)
│   └── filter_results (20ms)
└── response_serialization (10ms)
```

### Grafana Dashboard

Import `grafana/akidb-dashboard.json` (create this separately) to visualize:

- Request rate (requests/sec)
- Latency percentiles (P50/P95/P99)
- Error rate
- S3 operation counts
- Cache hit rate

---

## Security

### TLS/HTTPS

Run AkiDB behind a reverse proxy (Nginx, Traefik, Caddy) with TLS termination:

**Nginx Example:**

```nginx
server {
    listen 443 ssl http2;
    server_name akidb.example.com;

    ssl_certificate /etc/letsencrypt/live/akidb.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/akidb.example.com/privkey.pem;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

### MinIO Encryption

Enable server-side encryption with KES (Key Encryption Service):

```bash
# Set up KES with Vault/AWS KMS
# MinIO documentation: https://min.io/docs/minio/linux/operations/server-side-encryption.html
```

### Firewall Rules

```bash
# Allow only necessary ports
sudo ufw allow 8080/tcp  # AkiDB API
sudo ufw allow 9000/tcp  # MinIO API
sudo ufw allow 9001/tcp  # MinIO Console
sudo ufw enable
```

---

## High Availability

### Load Balancing (Multiple AkiDB Nodes)

Use HAProxy or Nginx to load balance across AkiDB instances:

```
Client
  ↓
HAProxy (port 8080)
  ├──▶ AkiDB Node 1:8080
  ├──▶ AkiDB Node 2:8080
  └──▶ AkiDB Node 3:8080
        ↓
  MinIO Cluster (shared)
```

**HAProxy Configuration:**

```
frontend akidb_frontend
    bind *:8080
    default_backend akidb_nodes

backend akidb_nodes
    balance roundrobin
    option httpchk GET /health/ready
    server node1 10.0.1.10:8080 check
    server node2 10.0.1.11:8080 check
    server node3 10.0.1.12:8080 check
```

### MinIO Distributed Mode

Deploy MinIO in distributed mode for high availability:

```bash
# 4-node MinIO cluster (minimum for HA)
minio server http://minio{1...4}/data{1...4}
```

See `docs/minio-integration.md` for details.

---

## Troubleshooting

### Common Issues

**1. "Storage backend not ready"**

- **Symptom:** Readiness check fails with 503
- **Cause:** MinIO is down or inaccessible
- **Solution:**
  ```bash
  # Check MinIO status
  curl http://localhost:9000/minio/health/live

  # Check S3 credentials
  docker logs akidb | grep "Failed to initialize S3"
  ```

**2. "High latency on search queries"**

- **Symptom:** P99 latency > 1s
- **Cause:** Index not built or too many vectors
- **Solution:**
  ```bash
  # Check index status via Prometheus
  curl http://localhost:8080/metrics | grep akidb_index_vectors_total

  # Consider switching to HNSW if > 100K vectors
  ```

**3. "Jaeger traces not appearing"**

- **Symptom:** No traces in Jaeger UI
- **Cause:** Wrong endpoint or telemetry disabled
- **Solution:**
  ```bash
  # Check telemetry config
  docker logs akidb | grep "OpenTelemetry initialized"

  # Verify Jaeger endpoint
  curl http://localhost:4317
  ```

### Logs

```bash
# Systemd
sudo journalctl -u akidb -f

# Docker
docker logs -f akidb

# Increase log level for debugging
export RUST_LOG=debug
export AKIDB_LOG_LEVEL=debug
```

---

## Performance Tuning

### Recommended Settings for Production

**For 1M vectors, 768-dim:**

```yaml
index:
  hnsw:
    m: 32                    # Higher M for better recall
    ef_construction: 800     # Slower build, better index quality
    ef_search: 400           # Trade recall for latency
```

**For 10M+ vectors:**

```yaml
index:
  hnsw:
    m: 48
    ef_construction: 1000
    ef_search: 600

query:
  max_parallel_segments: 16  # Use more CPU cores
```

### Cache Sizing

- **Hot Cache (NVMe):** 10-20% of total data size
- **MinIO Cache:** Not needed if NVMe cache is sufficient

---

## Backup & Disaster Recovery

See `docs/phase6-offline-rag.md` for `.akipkg` packaging and multi-site replication.

```bash
# Export collection snapshot
akidb-pkg export --collection products --output products.akipkg

# Copy to DR site (air-gapped)
scp products.akipkg dr-site:/backups/

# Import at DR site
akidb-pkg import --file products.akipkg
```

---

## Next Steps

1. **Set up monitoring:** Configure Prometheus + Grafana
2. **Enable backups:** Schedule daily .akipkg exports
3. **Tune performance:** Adjust HNSW parameters based on workload
4. **Scale horizontally:** Add more AkiDB nodes behind load balancer
5. **Multi-site replication:** See `docs/phase6-offline-rag.md` (Q2 2025)

---

**Support:** https://github.com/aifocal/akidb/issues
**Documentation:** https://github.com/aifocal/akidb/tree/main/docs
