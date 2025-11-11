# AkiDB Deployment Quick Reference

Quick reference card for deploying and managing AkiDB.

---

## Ports

| Service | Port | Protocol | Endpoint |
|---------|------|----------|----------|
| REST API | 8080 | HTTP | `/api/v1/*` |
| Health (REST) | 8080 | HTTP | `/health` |
| Metrics (REST) | 8080 | HTTP | `/metrics` |
| gRPC API | 9090 | gRPC | `akidb.*` |
| Health (gRPC) | 9090 | gRPC | `grpc.health.v1.Health/Check` |

---

## Quick Start Commands

### Docker Compose (Recommended for Development)

```bash
# Start both servers
docker-compose up -d

# View logs
docker-compose logs -f

# Stop servers
docker-compose down

# Stop and remove data (WARNING: destroys database)
docker-compose down -v
```

### Kubernetes (Recommended for Production)

```bash
# Deploy all resources
kubectl apply -f k8s/

# Check status
kubectl get all -n akidb

# View logs
kubectl logs -f -n akidb -l app=akidb-grpc
kubectl logs -f -n akidb -l app=akidb-rest

# Scale replicas
kubectl scale deployment akidb-grpc --replicas=5 -n akidb

# Delete all resources
kubectl delete -f k8s/
```

### Bare Metal (systemd)

```bash
# Install (run once)
sudo ./scripts/install-akidb.sh

# Start services
sudo systemctl start akidb-grpc akidb-rest

# Stop services
sudo systemctl stop akidb-grpc akidb-rest

# Restart services
sudo systemctl restart akidb-grpc akidb-rest

# View logs
sudo journalctl -u akidb-grpc -f
sudo journalctl -u akidb-rest -f

# Check status
sudo systemctl status akidb-grpc akidb-rest
```

---

## Health Checks

### REST Server

```bash
# Health check
curl http://localhost:8080/health
# Response: {"status":"healthy","version":"2.0.0-rc1"}

# Metrics
curl http://localhost:8080/metrics
```

### gRPC Server

```bash
# Health check (requires grpcurl)
grpcurl -plaintext localhost:9090 grpc.health.v1.Health/Check
# Response: {"status": "SERVING"}
```

---

## Configuration

### Environment Variables

```bash
# Server configuration
export AKIDB_HOST=0.0.0.0
export AKIDB_REST_PORT=8080
export AKIDB_GRPC_PORT=9090

# Database
export AKIDB_DB_PATH=sqlite:///data/akidb/metadata.db

# Logging
export AKIDB_LOG_LEVEL=info
export AKIDB_LOG_FORMAT=json
export RUST_LOG=info

# Features
export AKIDB_METRICS_ENABLED=true
export AKIDB_VECTOR_PERSISTENCE_ENABLED=true
```

### Config File (config.toml)

```toml
[server]
host = "0.0.0.0"
rest_port = 8080
grpc_port = 9090

[database]
path = "sqlite:///var/lib/akidb/data/metadata.db"
max_connections = 10

[features]
metrics_enabled = true
vector_persistence_enabled = true

[logging]
level = "info"
format = "json"
```

---

## Backup & Restore

### Backup

```bash
# Basic backup
./scripts/backup-akidb.sh

# Custom backup directory
./scripts/backup-akidb.sh /mnt/backup/akidb

# Encrypted backup with 30-day retention
ENCRYPT=true \
GPG_RECIPIENT="backup@example.com" \
RETENTION_DAYS=30 \
./scripts/backup-akidb.sh

# Automated backups (cron - every 6 hours)
0 */6 * * * /usr/local/bin/backup-akidb.sh >> /var/log/akidb/backup.log 2>&1
```

### Restore

```bash
# Interactive restore
sudo ./scripts/restore-akidb.sh /backup/akidb/akidb-backup-20250107-120000.tar.gz

# Non-interactive restore
sudo ./scripts/restore-akidb.sh /backup/akidb/akidb-backup-latest.tar.gz --no-confirm
```

---

## File Locations

### Docker

| Path | Description |
|------|-------------|
| `/data/akidb/metadata.db` | SQLite database |
| `/var/log/akidb/` | Log files |

### Bare Metal

| Path | Description |
|------|-------------|
| `/etc/akidb/config.toml` | Configuration file |
| `/var/lib/akidb/data/metadata.db` | SQLite database |
| `/var/log/akidb/` | Log files |
| `/usr/local/bin/akidb-grpc` | gRPC binary |
| `/usr/local/bin/akidb-rest` | REST binary |

---

## Troubleshooting

### Server Won't Start

```bash
# Check if port is in use
sudo lsof -i :8080
sudo lsof -i :9090

# Kill process using port
sudo kill -9 <PID>

# Check logs
docker logs akidb-rest  # Docker
sudo journalctl -u akidb-rest -n 50  # systemd
kubectl logs -n akidb <pod-name>  # Kubernetes
```

### Database Locked

```bash
# Check WAL mode
sqlite3 /var/lib/akidb/data/metadata.db "PRAGMA journal_mode;"

# Enable WAL
sqlite3 /var/lib/akidb/data/metadata.db "PRAGMA journal_mode=WAL;"

# Checkpoint WAL
sqlite3 /var/lib/akidb/data/metadata.db "PRAGMA wal_checkpoint(TRUNCATE);"
```

### High Memory Usage

```bash
# Check collection count
curl http://localhost:8080/api/v1/collections

# Reduce HNSW parameters in config.toml
[hnsw]
m = 16                   # Lower = less memory
ef_construction = 100    # Lower = less memory
threshold = 20000        # Higher = fewer HNSW indexes
```

### Health Check Failures

```bash
# Test manually
curl -v http://localhost:8080/health
grpcurl -v -plaintext localhost:9090 grpc.health.v1.Health/Check

# Check service is running
docker ps | grep akidb  # Docker
sudo systemctl status akidb-rest  # systemd
kubectl get pods -n akidb  # Kubernetes

# View recent logs
docker logs --tail 50 akidb-rest  # Docker
sudo journalctl -u akidb-rest -n 50  # systemd
kubectl logs -n akidb <pod-name> --tail 50  # Kubernetes
```

---

## Monitoring

### Prometheus Configuration

```yaml
scrape_configs:
  - job_name: 'akidb'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: '/metrics'
    scrape_interval: 30s
```

### Key Metrics

```promql
# Request rate
rate(akidb_requests_total[5m])

# Error rate
rate(akidb_requests_total{status="error"}[5m])

# P95 latency
histogram_quantile(0.95, akidb_request_duration_seconds_bucket)

# Collections count
akidb_collections_total

# Vectors count
akidb_vectors_total
```

### Alert Examples

```yaml
# High error rate
- alert: AkiDBHighErrorRate
  expr: rate(akidb_requests_total{status="error"}[5m]) > 0.05
  for: 5m

# High latency
- alert: AkiDBHighLatency
  expr: histogram_quantile(0.95, akidb_request_duration_seconds) > 0.1
  for: 5m
```

---

## Resource Requirements

| Size | CPU | Memory | Disk | Replicas |
|------|-----|--------|------|----------|
| Small (< 1M vectors) | 1-2 cores | 2-4 GB | 50 GB | 1-2 |
| Medium (1-10M vectors) | 2-4 cores | 4-8 GB | 100 GB | 2-3 |
| Large (10-50M vectors) | 4-8 cores | 8-16 GB | 200 GB | 3-5 |
| XLarge (50M+ vectors) | 8-16 cores | 16-32 GB | 500 GB | 5-10 |

---

## API Examples

### Create Collection

```bash
curl -X POST http://localhost:8080/api/v1/collections \
  -H "Content-Type: application/json" \
  -d '{
    "name": "my-collection",
    "dimension": 384,
    "metric": "cosine"
  }'
```

### Insert Vector

```bash
curl -X POST http://localhost:8080/api/v1/collections/my-collection/vectors \
  -H "Content-Type: application/json" \
  -d '{
    "vector": [0.1, 0.2, 0.3, ...],
    "metadata": {"key": "value"}
  }'
```

### Query Vectors

```bash
curl -X POST http://localhost:8080/api/v1/collections/my-collection/query \
  -H "Content-Type: application/json" \
  -d '{
    "vector": [0.1, 0.2, 0.3, ...],
    "k": 10
  }'
```

---

## Security Checklist

- [ ] Use TLS/SSL (terminate at load balancer)
- [ ] Configure firewall rules
- [ ] Run as non-root user
- [ ] Restrict file permissions (600 for DB, 640 for config)
- [ ] Enable encryption at rest
- [ ] Use encrypted backups (GPG)
- [ ] Set resource limits
- [ ] Configure NetworkPolicy (Kubernetes)
- [ ] Use secrets management (not hardcoded credentials)
- [ ] Enable audit logging
- [ ] Monitor security alerts
- [ ] Rotate credentials regularly

---

## Production Deployment Checklist

**Pre-Deployment:**
- [ ] Build release binaries (`cargo build --release`)
- [ ] Run full test suite (`cargo test --workspace`)
- [ ] Review configuration
- [ ] Provision storage (100GB minimum)
- [ ] Configure log aggregation
- [ ] Setup monitoring (Prometheus + Grafana)
- [ ] Implement backup strategy
- [ ] Review security hardening
- [ ] Configure resource limits
- [ ] Apply firewall rules
- [ ] Setup SSL/TLS

**Post-Deployment:**
- [ ] Verify health endpoints
- [ ] Check metrics endpoint
- [ ] Test collection creation
- [ ] Verify vector persistence
- [ ] Monitor resource usage
- [ ] Configure alerts
- [ ] Test graceful shutdown
- [ ] Verify backup/restore
- [ ] Load test
- [ ] Document deployment

---

## Getting Help

**Documentation:**
- [Deployment Guide](/docs/DEPLOYMENT-GUIDE.md)
- [Quickstart Guide](/docs/QUICKSTART.md)
- [Migration Guide](/docs/MIGRATION-V1-TO-V2.md)

**Support:**
- GitHub Issues: https://github.com/your-org/akidb2/issues
- Discussions: https://github.com/your-org/akidb2/discussions

---

**Version:** 2.0.0-rc1
**Last Updated:** 2025-01-07
