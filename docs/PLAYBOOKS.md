# AkiDB Incident Response Playbooks

**Version**: 2.0.0
**Last Updated**: November 9, 2025
**Maintainer**: AkiDB SRE Team

---

## Table of Contents

1. [Playbook 1: High Error Rate](#playbook-1-high-error-rate)
2. [Playbook 2: High Latency](#playbook-2-high-latency)
3. [Playbook 3: Data Loss Suspected](#playbook-3-data-loss-suspected)
4. [Playbook 4: S3 Outage](#playbook-4-s3-outage)
5. [Escalation Matrix](#escalation-matrix)
6. [Common Commands](#common-commands)

---

## Playbook 1: High Error Rate

### Alert Metadata

- **Alert Name**: `HighErrorRate`
- **Severity**: **CRITICAL** 🔴
- **Threshold**: >5% 5xx errors for 5 minutes
- **Component**: REST API, gRPC API
- **SLO Impact**: YES (availability)

### Immediate Actions (0-5 minutes)

**STOP - Read this first:**
- Do not restart pods immediately
- Do not rollback without investigation
- Document all actions in incident channel

**1. Assess Impact**

```bash
# Check error rate by endpoint
kubectl exec -n production $(kubectl get pod -l app=prometheus -n production -o jsonpath='{.items[0].metadata.name}') -- \
  wget -qO- 'http://localhost:9090/api/v1/query?query=rate(http_requests_total{status=~"5.."}[5m])' \
  | jq '.data.result'

# Check affected users/tenants
kubectl logs -l app=akidb -n production --tail=100 | grep ERROR | cut -d'"' -f4 | sort | uniq -c

# Check current error rate
curl -s http://akidb-metrics/metrics | grep 'http_requests_total{status="500"}'
```

**2. Check Recent Changes**

```bash
# Recent deployments
kubectl rollout history statefulset/akidb -n production

# Recent config changes
kubectl get configmap akidb-config -n production -o yaml | grep -A 20 "data:"

# Recent scaling events
kubectl get events -n production --sort-by='.lastTimestamp' | grep akidb | head -20
```

**3. Review Dashboards**

- **Grafana Errors Dashboard**: http://grafana/d/akidb-errors
- Check: Circuit breaker states, DLQ size, S3 errors
- Compare: Current vs 1-hour-ago baseline

### Diagnosis (5-15 minutes)

#### Common Cause 1: S3 Unavailable

**Symptoms:**
- Errors contain "S3", "object store", or "upload failed"
- Circuit breaker state = OPEN (1)
- DLQ size increasing rapidly

**Check:**
```bash
# S3 health check
curl -I http://minio-service:9000/minio/health/live

# S3 error rate
kubectl exec prometheus-0 -n production -- \
  wget -qO- 'http://localhost:9090/api/v1/query?query=rate(s3_errors_total[5m])'

# Circuit breaker state (0=closed, 1=open, 2=half-open)
curl -s http://akidb/metrics | grep 'circuit_breaker_state{service="s3"}'
```

**Mitigation:** See [Playbook 4: S3 Outage](#playbook-4-s3-outage)

---

#### Common Cause 2: Database Pool Exhaustion

**Symptoms:**
- Errors contain "unable to open database", "locked", "busy"
- High number of concurrent connections
- Slow query log shows long-running queries

**Check:**
```bash
# Check SQLite connection pool
kubectl exec akidb-0 -n production -- \
  sqlite3 /data/metadata.db "PRAGMA busy_timeout;"

# Check active connections
kubectl exec akidb-0 -n production -- \
  lsof | grep metadata.db | wc -l

# Check for locked tables
kubectl logs akidb-0 -n production | grep "database is locked"
```

**Mitigation:**
```bash
# Increase max_connections in config
kubectl edit configmap akidb-config -n production
# Set: AKIDB_DB_MAX_CONNECTIONS=50 (default: 20)

# Restart pods to apply config
kubectl rollout restart statefulset/akidb -n production
kubectl rollout status statefulset/akidb -n production

# Monitor improvement
watch -n 5 'curl -s http://akidb/metrics | grep http_requests_total'
```

---

#### Common Cause 3: Memory Pressure

**Symptoms:**
- OOMKilled pod restarts
- Swap usage high
- "out of memory" errors in logs

**Check:**
```bash
# Check memory usage
kubectl top pods -n production -l app=akidb

# Check OOMKills
kubectl get pods -n production -o jsonpath='{.items[*].status.containerStatuses[*].lastState.terminated.reason}' | grep OOMKilled

# Check hot tier memory
curl -s http://akidb/metrics | grep 'memory_usage_bytes{tier="hot"}'
```

**Mitigation:**
```bash
# Demote large collections to warm tier
curl -X POST http://akidb/api/v1/admin/collections/{large-collection-id}/demote

# Scale up replicas (horizontal scaling)
kubectl scale statefulset akidb --replicas=5 -n production

# Increase memory limits (if needed)
kubectl set resources statefulset akidb --limits=memory=16Gi -n production
```

---

#### Common Cause 4: Index Corruption

**Symptoms:**
- Errors contain "failed to search index", "invalid index"
- Search queries fail but inserts work
- Specific collections affected

**Check:**
```bash
# List collections with errors
kubectl logs akidb-0 -n production | grep "index error" | grep -oP 'collection_id=\K[^"]+' | sort | uniq

# Check collection integrity
curl http://akidb/api/v1/collections/{collection-id}
```

**Mitigation:**
```bash
# Restore collection from snapshot
curl -X POST http://akidb/api/v1/admin/collections/{collection-id}/restore \
  -H "Content-Type: application/json" \
  -d '{"snapshot_id":"latest"}'

# If no snapshot available, rebuild index
curl -X POST http://akidb/api/v1/admin/collections/{collection-id}/rebuild-index
```

---

### Permanent Fixes

1. **If S3-related:** Implement S3 retry backoff tuning
2. **If DB-related:** Increase connection pool, optimize queries
3. **If memory-related:** Enable auto-tiering, tune thresholds
4. **If index-related:** Enable periodic snapshot backups

### Escalation

- **Error rate >10%**: Page on-call engineer immediately
- **Data loss suspected**: Escalate to incident commander
- **Unresolved after 30 minutes**: Escalate to senior SRE

---

## Playbook 2: High Latency

### Alert Metadata

- **Alert Name**: `HighSearchLatency`
- **Severity**: **WARNING** 🟡
- **Threshold**: P95 >25ms for 10 minutes
- **Component**: Vector search
- **SLO Impact**: YES (latency)

### Immediate Actions

**1. Identify Latency Source**

```bash
# Check P50/P95/P99 latencies
kubectl exec prometheus-0 -n production -- \
  wget -qO- 'http://localhost:9090/api/v1/query?query=histogram_quantile(0.95,rate(vector_search_duration_seconds_bucket[5m]))'

# Check latency by tier
curl -s http://akidb/metrics | grep 'vector_search_duration_seconds' | grep 'tier='

# Check if cold tier is being accessed
kubectl logs akidb-0 -n production | grep "cold tier access" | tail -20
```

**2. Check Tier Distribution**

```bash
# Check collection tier distribution
curl -s http://akidb/metrics | grep 'collection_tier'

# Identify hot collections
kubectl exec prometheus-0 -n production -- \
  wget -qO- 'http://localhost:9090/api/v1/query?query=topk(10,rate(vector_search_duration_seconds_count[5m]))'
```

### Diagnosis

#### Common Cause 1: Cold Tier Access

**Symptoms:**
- High latency on specific collections
- S3 download metrics spiking
- "S3 fetch" in logs

**Mitigation:**
```bash
# Promote frequently accessed collections to hot tier
curl -X POST http://akidb/api/v1/admin/collections/{collection-id}/promote \
  -H "Content-Type: application/json" \
  -d '{"target_tier":"hot"}'

# Adjust tiering policy to be less aggressive
kubectl edit configmap akidb-config -n production
# Increase: AKIDB_HOT_TO_WARM_THRESHOLD (e.g., 24h -> 7days)
```

---

#### Common Cause 2: Large Result Sets

**Symptoms:**
- High latency on searches with large `k` parameter
- Slow serialization time

**Mitigation:**
```bash
# Set max result limit
kubectl edit configmap akidb-config -n production
# Add: AKIDB_MAX_SEARCH_RESULTS=1000

# Educate users to use pagination
```

---

#### Common Cause 3: Index Degradation

**Symptoms:**
- Latency increasing over time
- Recent inserts to collection

**Mitigation:**
```bash
# Trigger HNSW compaction
curl -X POST http://akidb/api/v1/admin/collections/{collection-id}/compact

# Rebuild index if needed
curl -X POST http://akidb/api/v1/admin/collections/{collection-id}/rebuild-index
```

---

### Permanent Fixes

1. Enable auto-promotion for hot collections
2. Tune HNSW parameters (increase `ef_construction`)
3. Implement caching layer for popular queries
4. Horizontal scaling (add read replicas)

### Escalation

- **P95 >100ms**: Escalate to on-call engineer
- **Unresolved after 1 hour**: Review capacity planning

---

## Playbook 3: Data Loss Suspected

### Alert Metadata

- **Severity**: **CRITICAL** 🔴
- **SLO Impact**: YES (durability)

### Immediate Actions

**⚠️ CRITICAL: Activate incident commander immediately**

**1. Verify Claim**

```bash
# Check collection metadata
curl http://akidb/api/v1/collections/{collection-id}

# Check vector count
curl http://akidb/api/v1/collections/{collection-id}/stats | jq '.vector_count'

# Search audit logs for deletions
kubectl logs akidb-0 -n production | grep "collection_id={collection-id}" | grep -E "(DELETE|TRUNCATE)"
```

**2. Check WAL Integrity**

```bash
# Check WAL files exist
kubectl exec akidb-0 -n production -- ls -lh /data/wal/ | grep {collection-id}

# Check WAL size (should be >0)
kubectl exec akidb-0 -n production -- du -sh /data/wal/{collection-id}

# Check WAL corruption
kubectl exec akidb-0 -n production -- \
  cat /data/wal/{collection-id}/latest.wal | wc -l
```

**3. Check S3 Snapshots**

```bash
# List snapshots (using AWS CLI or MinIO CLI)
aws s3 ls s3://akidb-snapshots/{collection-id}/ --recursive | sort -r | head -10

# Check latest snapshot timestamp
aws s3api head-object --bucket akidb-snapshots \
  --key {collection-id}/snapshot-latest.parquet \
  | jq '.LastModified'

# Download snapshot for inspection
aws s3 cp s3://akidb-snapshots/{collection-id}/snapshot-latest.parquet /tmp/
```

### Mitigation

**Option 1: Restore from S3 Snapshot**

```bash
# Restore collection from latest snapshot
curl -X POST http://akidb/api/v1/admin/collections/{collection-id}/restore \
  -H "Content-Type: application/json" \
  -d '{
    "snapshot_id": "latest",
    "verify_integrity": true
  }'

# Monitor restore progress
kubectl logs akidb-0 -n production -f | grep "restore progress"

# Verify restoration
curl http://akidb/api/v1/collections/{collection-id}/stats | jq '.vector_count'
```

**Option 2: Restore from WAL**

```bash
# Replay WAL from specific timestamp
curl -X POST http://akidb/api/v1/admin/wal/replay \
  -H "Content-Type: application/json" \
  -d '{
    "collection_id": "{collection-id}",
    "from_timestamp": "2025-11-09T00:00:00Z",
    "verify": true
  }'
```

**Option 3: Recovery from Backup**

If neither S3 nor WAL are available:

1. Check off-site backups (if configured)
2. Contact backup admin
3. Prepare for data recovery from user backups

### Post-Incident Actions

**Required:**

1. **Root Cause Analysis**: Within 24 hours
2. **Postmortem**: Within 72 hours
3. **User Communication**: Immediate (if confirmed)
4. **Backup Verification**: Test all snapshots

**Checklist:**
- [ ] Identify root cause
- [ ] Document timeline
- [ ] Verify all snapshots intact
- [ ] Review WAL configuration
- [ ] Test restore procedures
- [ ] Update runbooks

### Escalation

- **IMMEDIATE**: Activate incident commander
- **IMMEDIATE**: Notify affected users
- **IMMEDIATE**: All hands on deck if >10% data loss

---

## Playbook 4: S3 Outage

### Alert Metadata

- **Alert Name**: `S3ErrorRateHigh`, `CircuitBreakerOpen`
- **Severity**: **CRITICAL** 🔴
- **Threshold**: S3 errors >10% for 5 minutes
- **Component**: Cold tier, snapshots
- **SLO Impact**: YES (durability)

### Immediate Actions

**1. Verify Circuit Breaker**

```bash
# Check circuit breaker state (0=closed, 1=open, 2=half-open)
curl -s http://akidb/metrics | grep 'circuit_breaker_state{service="s3"}'

# Check DLQ size
curl -s http://akidb/metrics | grep 'dlq_size'

# Check S3 error breakdown
kubectl exec prometheus-0 -n production -- \
  wget -qO- 'http://localhost:9090/api/v1/query?query=s3_errors_total' | jq
```

**2. Check S3 Health**

```bash
# For MinIO
curl -I http://minio-service:9000/minio/health/live
kubectl get pods -l app=minio -n production

# For AWS S3
aws s3 ls s3://akidb-snapshots/ --region us-east-1

# Check S3 endpoint connectivity
kubectl exec akidb-0 -n production -- \
  curl -I -m 5 http://minio-service:9000
```

**3. Check Network**

```bash
# Check DNS resolution
kubectl exec akidb-0 -n production -- nslookup minio-service

# Check network policies
kubectl get networkpolicies -n production

# Check for network partitions
kubectl exec akidb-0 -n production -- ping -c 3 minio-service
```

### Mitigation

**Option 1: Operate in Degraded Mode (Hot Tier Only)**

```bash
# Disable S3 uploads temporarily
kubectl set env statefulset/akidb AKIDB_COLD_TIER_ENABLED=false -n production

# Wait for rollout
kubectl rollout status statefulset/akidb -n production

# Verify S3 uploads stopped
kubectl logs akidb-0 -n production | grep "S3 upload" | tail -10

# Notify users
echo "⚠️  WARNING: Operating without S3 backup. Durability reduced." | \
  kubectl exec -i alert-manager-0 -n production -- \
  /bin/alertmanager-cli alert add --labels=severity=warning
```

**Option 2: Switch to Alternative S3 Endpoint**

```bash
# Update S3 endpoint to failover region
kubectl set env statefulset/akidb \
  AKIDB_COLD_TIER_ENDPOINT=https://s3.us-west-2.amazonaws.com \
  -n production

# Or use backup MinIO instance
kubectl set env statefulset/akidb \
  AKIDB_COLD_TIER_ENDPOINT=http://minio-backup:9000 \
  -n production
```

**Option 3: Retry DLQ When S3 Recovers**

```bash
# Wait for S3 recovery
until curl -sf http://minio-service:9000/minio/health/live; do
  echo "Waiting for S3..."
  sleep 10
done

# Verify S3 is healthy
kubectl wait --for=condition=ready pod -l app=minio -n production --timeout=10m

# Process DLQ (retry failed uploads)
curl -X POST http://akidb/api/v1/admin/dlq/retry-all \
  -H "Content-Type: application/json" \
  -d '{"max_retries": 3, "batch_size": 100}'

# Monitor DLQ draining
watch -n 5 'curl -s http://akidb/metrics | grep dlq_size'
```

### Post-Recovery Actions

**Verify Data Integrity:**

```bash
# Check all snapshots are intact
aws s3 ls s3://akidb-snapshots/ --recursive | tail -100

# Verify DLQ is empty
curl -s http://akidb/metrics | grep 'dlq_size 0'

# Test snapshot restore
curl -X POST http://akidb/api/v1/admin/collections/{test-collection}/restore \
  -d '{"snapshot_id":"latest","verify_only":true}'
```

**Enable S3 Again:**

```bash
# Re-enable S3
kubectl set env statefulset/akidb AKIDB_COLD_TIER_ENABLED=true -n production

# Monitor S3 upload success rate
kubectl logs akidb-0 -n production -f | grep "S3 upload success"
```

### Permanent Fixes

1. Implement S3 multi-region replication
2. Add backup S3 endpoint
3. Increase circuit breaker thresholds (if too sensitive)
4. Review S3 IAM permissions

### Escalation

- **S3 down >1 hour**: Escalate to infrastructure team
- **Data loss risk**: Notify users and stakeholders
- **AWS S3 outage**: Monitor AWS status page, prepare for extended downtime

---

## Escalation Matrix

| Severity | First Response | Escalation (30min) | Escalation (1hr) | Escalation (2hr) |
|----------|----------------|--------------------|-----------------|--------------------|
| **P0 (Critical)** | On-call SRE | Incident Commander | VP Engineering | CTO |
| **P1 (High)** | On-call SRE | Senior SRE | Engineering Manager | VP Engineering |
| **P2 (Medium)** | On-call SRE | Senior SRE | - | Engineering Manager |
| **P3 (Low)** | Ticket assigned | - | - | - |

### Incident Commander Activation

Activate incident commander for:
- Data loss confirmed
- SLO breach >30 minutes
- Multi-component cascading failure
- Security incident

---

## Common Commands

### Quick Health Checks

```bash
# Overall health
curl http://akidb/health

# Metrics endpoint
curl http://akidb/metrics

# Pod status
kubectl get pods -l app=akidb -n production

# Recent logs
kubectl logs akidb-0 -n production --tail=50 -f
```

### Performance Debugging

```bash
# Check latency distribution
kubectl exec prometheus-0 -n production -- \
  wget -qO- 'http://localhost:9090/api/v1/query?query=histogram_quantile(0.95,vector_search_duration_seconds_bucket)'

# Check error rate
kubectl exec prometheus-0 -n production -- \
  wget -qO- 'http://localhost:9090/api/v1/query?query=rate(http_requests_total{status=~"5.."}[5m])'

# Check resource usage
kubectl top pods -l app=akidb -n production
```

### Collection Management

```bash
# List all collections
curl http://akidb/api/v1/collections

# Get collection details
curl http://akidb/api/v1/collections/{collection-id}

# Delete collection
curl -X DELETE http://akidb/api/v1/collections/{collection-id}
```

### Rollback

```bash
# Rollback to previous version
kubectl rollout undo statefulset/akidb -n production

# Rollback to specific revision
kubectl rollout undo statefulset/akidb --to-revision=3 -n production

# Check rollout status
kubectl rollout status statefulset/akidb -n production
```

---

## References

- **Runbooks**: `/docs/runbooks/` (detailed technical procedures)
- **Architecture**: `/docs/ARCHITECTURE.md`
- **Metrics Guide**: `/docs/METRICS.md`
- **API Documentation**: `/docs/openapi.yaml`
- **SLO Dashboard**: http://grafana/d/akidb-slo

---

**Last Reviewed**: November 9, 2025
**Next Review**: December 9, 2025
**Playbook Version**: 2.0.0
