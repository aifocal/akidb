# AkiDB Observability Runbook

**Version:** 1.0
**Last Updated:** 2025-11-08
**Maintainer:** DevOps Team

---

## Table of Contents

1. [Overview](#overview)
2. [Access URLs](#access-urls)
3. [Common Queries](#common-queries)
4. [Dashboards](#dashboards)
5. [Alert Response](#alert-response)
6. [Troubleshooting](#troubleshooting)
7. [Maintenance Tasks](#maintenance-tasks)

---

## Overview

AkiDB uses a comprehensive observability stack:

- **Prometheus**: Metrics collection and alerting
- **Grafana**: Visualization dashboards
- **Jaeger**: Distributed tracing
- **Application Logs**: Structured logging via tracing-subscriber

**Architecture:**
```
AkiDB REST API (port 8080)
  ├─ /metrics endpoint → Prometheus (port 9090)
  ├─ traces → Jaeger (port 16686)
  └─ dashboards ← Grafana (port 3000)
```

---

## Access URLs

### Production Environment

| Service    | URL                          | Credentials         | Purpose                    |
|------------|------------------------------|---------------------|----------------------------|
| AkiDB REST | http://localhost:8080        | None                | Vector database API        |
| Prometheus | http://localhost:9090        | None                | Metrics and alerts         |
| Grafana    | http://localhost:3000        | admin / admin       | Dashboards                 |
| Jaeger     | http://localhost:16686       | None                | Distributed tracing        |

### Key Endpoints

- **Health Check**: `GET http://localhost:8080/health`
- **Metrics**: `GET http://localhost:8080/metrics`
- **Prometheus Targets**: http://localhost:9090/targets
- **Prometheus Alerts**: http://localhost:9090/alerts
- **Grafana Explore**: http://localhost:3000/explore

---

## Common Queries

### Prometheus PromQL Queries

#### Service Health

```promql
# Service uptime
up{job="akidb-rest"}

# Request rate (QPS)
rate(akidb_searches_performed_total[1m])

# Error rate
rate(akidb_errors_total[1m])

# Error percentage
rate(akidb_errors_total[1m]) / rate(akidb_searches_performed_total[1m]) * 100
```

#### Performance

```promql
# Search P95 latency
akidb_search_latency_ms_p95

# Insert P95 latency
akidb_insert_latency_ms_p95

# Average search latency over 5 minutes
avg_over_time(akidb_search_latency_ms_p50[5m])
```

#### Storage

```promql
# S3 upload rate
rate(akidb_s3_uploads_total[1m])

# S3 failure rate
rate(akidb_s3_permanent_failures_total[1m])

# Circuit breaker state (0=Closed, 1=Open, 2=HalfOpen)
akidb_circuit_breaker_state

# DLQ size
akidb_dlq_size

# WAL size in MB
akidb_wal_size_bytes / 1024 / 1024

# Cache hit rate
rate(akidb_cache_hits_total[5m]) / (rate(akidb_cache_hits_total[5m]) + rate(akidb_cache_misses_total[5m]))
```

#### Resource Usage

```promql
# Vector count
akidb_vectors_inserted_total - akidb_vectors_deleted_total

# Collection count
akidb_collections_created_total - akidb_collections_deleted_total

# Compaction rate
rate(akidb_compactions_total[5m])
```

---

## Dashboards

### 1. AkiDB Overview Dashboard

**URL:** http://localhost:3000/d/akidb-overview

**Purpose:** High-level service health monitoring

**Panels:**
- Total Collections (gauge)
- Total Vectors (gauge)
- Errors (5m rate)
- Operations Throughput (time series)
- Latency (P50/P95) (time series)

**When to Use:**
- First stop for incident investigation
- Daily health checks
- Capacity planning

**Key Metrics:**
- Search P95 should be <25ms
- Error rate should be <1%

### 2. AkiDB S3 Storage Dashboard

**URL:** http://localhost:3000/d/akidb-s3-storage

**Purpose:** Storage layer monitoring (S3, DLQ, circuit breaker)

**Panels:**
- Circuit Breaker State (gauge)
- Error Rate (gauge)
- DLQ Size (gauge)
- WAL Size (gauge)
- S3 Operations Throughput (time series)
- Cache Performance (time series)
- Storage Operations (time series)
- WAL Size Over Time (time series)

**When to Use:**
- S3 connectivity issues
- Circuit breaker trips
- DLQ accumulation
- Cache performance analysis

**Key Metrics:**
- Circuit breaker should be 0 (Closed)
- DLQ size should be <100
- Cache hit rate should be >50%

### 3. Jaeger Tracing

**URL:** http://localhost:16686

**Purpose:** Request tracing and performance debugging

**Usage:**
1. Select service: `akidb-rest`
2. Select operation (e.g., `query_vectors`)
3. Set time range
4. Click "Find Traces"
5. Click on trace to see span details

**When to Use:**
- Debugging slow requests
- Understanding request flow
- Identifying bottlenecks
- Error root cause analysis

---

## Alert Response

### Service Alerts

#### HighErrorRate

**Severity:** Warning
**Trigger:** Error rate > 10/sec for 2 minutes
**Impact:** Service quality degraded

**Response:**
1. Check Grafana Overview dashboard for error spike
2. Query error logs:
   ```bash
   docker logs akidb-rest | grep -i error | tail -50
   ```
3. Check for recent deployments or configuration changes
4. Investigate specific error types (check logs)
5. If persistent, rollback recent changes

**Common Causes:**
- Bad deployment
- Database connectivity issues
- Invalid client requests
- Resource exhaustion

---

#### CriticalErrorRate

**Severity:** Critical
**Trigger:** Error rate > 50/sec for 1 minute
**Impact:** Service severely degraded or unusable

**Response:**
1. **Immediate:** Check if service is down (`docker ps`)
2. Check service logs for crash/panic:
   ```bash
   docker logs akidb-rest --tail=100
   ```
3. Restart service if crashed:
   ```bash
   docker compose restart akidb-rest
   ```
4. If issue persists, rollback deployment
5. Escalate to on-call engineer

**Escalation:** Page on-call immediately

---

#### HighSearchLatency

**Severity:** Warning
**Trigger:** P95 search latency > 25ms for 5 minutes
**Impact:** Performance degraded

**Response:**
1. Check Grafana Overview dashboard for latency trends
2. Check vector count (may need HNSW reindexing)
3. Query recent traces in Jaeger:
   - Service: `akidb-rest`
   - Operation: `query_vectors`
   - Look for slow spans
4. Check CPU/memory usage:
   ```bash
   docker stats akidb-rest
   ```
5. Consider increasing resources or optimizing index

**Common Causes:**
- Large dataset (>1M vectors)
- Suboptimal HNSW parameters
- Resource contention
- Slow storage backend

---

#### VeryHighSearchLatency

**Severity:** Critical
**Trigger:** P95 search latency > 50ms for 2 minutes
**Impact:** Service quality severely impacted

**Response:**
1. **Immediate:** Check if service is thrashing (high CPU/memory)
2. Check recent configuration changes (HNSW params, etc.)
3. Consider temporary traffic shedding if overloaded
4. Investigate in Jaeger for bottlenecks
5. If caused by bad deployment, rollback

**Escalation:** Notify incident channel

---

#### AkiDBServiceDown

**Severity:** Critical
**Trigger:** Prometheus cannot scrape metrics for 1 minute
**Impact:** Service completely unavailable

**Response:**
1. **Immediate:** Check service status:
   ```bash
   docker ps | grep akidb-rest
   docker logs akidb-rest --tail=50
   ```
2. Restart service:
   ```bash
   docker compose restart akidb-rest
   ```
3. If restart fails, check for:
   - Database corruption
   - Disk space
   - Port conflicts
4. If cannot recover, restore from backup

**Escalation:** Page on-call immediately

---

### Storage Alerts

#### CircuitBreakerOpen

**Severity:** Warning
**Trigger:** Circuit breaker state = 1 (Open) for 2 minutes
**Impact:** S3 uploads paused, vectors only in WAL/memory

**Response:**
1. Check S3 connectivity:
   ```bash
   docker logs akidb-rest | grep -i "s3\|circuit"
   ```
2. Verify S3 credentials and endpoint configuration
3. Check DLQ size (should be accumulating failed uploads)
4. Circuit breaker will auto-recover (transitions to HalfOpen after cooldown)
5. Monitor for successful uploads resuming

**Common Causes:**
- S3 service outage
- Network connectivity issues
- Invalid credentials
- S3 rate limiting

**Recovery:** Automatic (circuit breaker will retry)

---

#### HighS3ErrorRate

**Severity:** Warning
**Trigger:** S3 error rate > 5% for 5 minutes
**Impact:** Increased S3 failures, risk of circuit breaker trip

**Response:**
1. Check S3 logs:
   ```bash
   docker logs akidb-rest | grep -i s3 | grep -i error
   ```
2. Verify S3 service status (AWS status page if using AWS S3)
3. Check for specific error types (403=auth, 503=throttling, etc.)
4. Monitor circuit breaker state
5. If approaching 10% error rate, prepare for circuit breaker trip

**Common Causes:**
- S3 rate limiting
- Intermittent network issues
- Invalid S3 bucket permissions

---

#### DLQSizeHigh

**Severity:** Warning
**Trigger:** DLQ size > 100 for 10 minutes
**Impact:** Failed S3 uploads accumulating

**Response:**
1. Check circuit breaker state (likely Open or error rate high)
2. Investigate S3 connectivity issues (see CircuitBreakerOpen)
3. Monitor DLQ growth rate:
   ```promql
   rate(akidb_dlq_size[5m])
   ```
4. Check WAL size (should also be growing)
5. Once S3 recovers, DLQ will drain automatically

**Common Causes:**
- S3 outage
- Circuit breaker open
- Persistent S3 errors

**Recovery:** Automatic (DLQ retries after S3 recovers)

---

#### DLQSizeCritical

**Severity:** Critical
**Trigger:** DLQ size > 1000 for 5 minutes
**Impact:** Risk of data loss if DLQ reaches capacity (max 10,000)

**Response:**
1. **Immediate:** Investigate S3 issues urgently
2. Check if circuit breaker is Open
3. Check for permanent S3 failures:
   ```promql
   rate(akidb_s3_permanent_failures_total[5m])
   ```
4. Consider emergency mitigation:
   - Increase DLQ capacity (requires code change)
   - Pause non-critical writes
5. Escalate if S3 cannot be recovered quickly

**Escalation:** Page on-call if DLQ > 5000

**Data Loss Risk:** If DLQ reaches 10,000, oldest entries evicted (FIFO)

---

#### HighS3PermanentFailures

**Severity:** Warning
**Trigger:** S3 permanent failures > 1/sec for 5 minutes
**Impact:** Data may be lost (not in DLQ or WAL)

**Response:**
1. Check logs for permanent failure reasons:
   ```bash
   docker logs akidb-rest | grep -i "permanent.*failure"
   ```
2. Verify DLQ is capturing retryable failures
3. Check WAL for recent data (should still be persisted)
4. Investigate root cause (malformed data, S3 bucket issues, etc.)
5. If data loss suspected, check application integrity

**Common Causes:**
- Malformed S3 requests
- S3 bucket deleted
- Invalid IAM permissions

**Data Risk:** High - permanent failures not retried

---

#### WALSizeLarge

**Severity:** Warning
**Trigger:** WAL size > 100MB for 15 minutes
**Impact:** Increased memory usage, slow recovery time

**Response:**
1. Check compaction rate:
   ```promql
   rate(akidb_compactions_total[5m])
   ```
2. Check S3 upload status (WAL grows if S3 uploads fail)
3. Verify S3 upload throughput:
   ```promql
   rate(akidb_s3_uploads_total[1m])
   ```
4. If S3 uploads healthy, may need to adjust compaction interval
5. Monitor WAL growth trend

**Common Causes:**
- High write rate
- S3 upload failures
- Slow compaction
- Large vectors

**Mitigation:**
- Ensure S3 uploads are working
- Check compaction worker logs

---

#### LowCacheHitRate

**Severity:** Info
**Trigger:** Cache hit rate < 50% for 10 minutes
**Impact:** Increased S3 download costs, slower queries

**Response:**
1. Check cache size vs workload:
   ```promql
   akidb_cache_size / akidb_cache_capacity
   ```
2. Verify cache is enabled (S3Only policy)
3. Check access patterns (random vs sequential)
4. Consider increasing cache size if frequently accessed data > cache capacity
5. Monitor S3 download costs

**Common Causes:**
- Small cache size
- Random access patterns
- Cache eviction too aggressive
- Cold start (cache warming up)

**Impact:** Performance and cost, not correctness

---

### Resource Alerts

#### HighInsertLatency

**Severity:** Warning
**Trigger:** P95 insert latency > 100ms for 5 minutes
**Impact:** Write performance degraded

**Response:**
1. Check Grafana dashboard for latency trends
2. Check WAL size (may be slow to flush)
3. Check S3 upload queue depth
4. Investigate in Jaeger:
   - Service: `akidb-rest`
   - Operation: `insert_vector`
5. Check resource usage (CPU/memory/disk)

**Common Causes:**
- Slow storage backend (WAL flush)
- S3 upload backlog
- Resource contention
- Large vectors

---

#### NoRecentSearches

**Severity:** Info
**Trigger:** No searches in last 30 minutes
**Impact:** May indicate low traffic or application issue

**Response:**
1. Check if this is expected (e.g., maintenance window)
2. Verify service is healthy (`up` metric)
3. Check application logs for errors
4. Verify clients are still connected
5. Check recent deployments

**Common Causes:**
- Maintenance window
- Low traffic period
- Application bug (clients not querying)
- Network partition

**Action:** Investigate if unexpected

---

## Troubleshooting

### Service Not Starting

**Symptoms:**
- `docker ps` shows akidb-rest as "Restarting"
- Service logs show crash/panic

**Steps:**
1. Check logs:
   ```bash
   docker logs akidb-rest --tail=100
   ```
2. Common issues:
   - Database migration failure
   - Port 8080 already in use
   - Invalid configuration
   - Disk space exhausted
3. Verify database file:
   ```bash
   docker exec akidb-rest ls -lh /data/akidb/metadata.db
   ```
4. Try manual start with verbose logging:
   ```bash
   docker compose up akidb-rest
   ```

---

### Metrics Not Showing in Grafana

**Symptoms:**
- Grafana dashboards empty or "No data"
- Prometheus cannot scrape metrics

**Steps:**
1. Verify Prometheus is scraping:
   - Go to http://localhost:9090/targets
   - Check `akidb-rest` target is "UP"
2. Test metrics endpoint manually:
   ```bash
   curl http://localhost:8080/metrics
   ```
3. Check Grafana datasource:
   - Go to http://localhost:3000/datasources
   - Test Prometheus datasource connection
4. Verify time range in Grafana (not too far in past/future)

---

### Traces Not Appearing in Jaeger

**Symptoms:**
- Jaeger UI shows "No traces found"
- Service is running but no trace data

**Steps:**
1. Verify tracing is enabled:
   ```bash
   docker exec akidb-rest env | grep ENABLE_TRACING
   # Should show: ENABLE_TRACING=true
   ```
2. Check Jaeger is running:
   ```bash
   docker ps | grep jaeger
   ```
3. Check service logs for tracing initialization:
   ```bash
   docker logs akidb-rest | grep -i tracing
   # Should see: "✅ Distributed tracing initialized"
   ```
4. Generate some requests to create traces:
   ```bash
   curl -X POST http://localhost:8080/api/v1/collections/*/query
   ```
5. Check Jaeger collector logs:
   ```bash
   docker logs akidb-jaeger | tail -50
   ```

---

### High Memory Usage

**Symptoms:**
- `docker stats` shows high memory usage
- Service becomes unresponsive

**Steps:**
1. Check vector count (large datasets use more memory):
   ```promql
   akidb_vectors_inserted_total - akidb_vectors_deleted_total
   ```
2. Check WAL size (held in memory):
   ```promql
   akidb_wal_size_bytes
   ```
3. Check cache size (S3Only policy):
   ```promql
   akidb_cache_size
   ```
4. If memory usage excessive:
   - Reduce cache size in configuration
   - Trigger manual compaction (reduce WAL)
   - Consider scaling to larger instance

---

## Maintenance Tasks

### Daily Tasks

1. **Health Check:**
   - Visit Grafana Overview dashboard
   - Verify all panels show data
   - Check for any active alerts

2. **Log Review:**
   ```bash
   docker logs akidb-rest | grep -i error | tail -20
   ```

3. **Backup Verification:**
   - Verify database backup exists
   - Check backup age (should be <24h)

### Weekly Tasks

1. **Alert Review:**
   - Review fired alerts in last week
   - Identify recurring issues
   - Tune alert thresholds if needed

2. **Performance Review:**
   - Check P95 latency trends
   - Review throughput trends
   - Identify capacity needs

3. **Storage Review:**
   - Check DLQ size trends
   - Review S3 error rates
   - Verify WAL compaction working

### Monthly Tasks

1. **Capacity Planning:**
   - Review vector count growth
   - Estimate future resource needs
   - Plan scaling if needed

2. **Documentation Update:**
   - Update runbook with new insights
   - Document incident resolutions
   - Update alert response procedures

3. **Disaster Recovery Test:**
   - Test restore from backup
   - Verify observability stack in DR environment
   - Update DR procedures

---

## Useful Commands

### Docker

```bash
# View all services
docker compose ps

# View service logs
docker logs akidb-rest -f

# Restart service
docker compose restart akidb-rest

# Check resource usage
docker stats

# View service configuration
docker inspect akidb-rest
```

### Database

```bash
# Connect to SQLite database
docker exec -it akidb-rest sqlite3 /data/akidb/metadata.db

# Run query
docker exec akidb-rest sqlite3 /data/akidb/metadata.db "SELECT COUNT(*) FROM collections"

# Check database size
docker exec akidb-rest ls -lh /data/akidb/metadata.db
```

### Testing

```bash
# Test health endpoint
curl http://localhost:8080/health

# Test metrics endpoint
curl http://localhost:8080/metrics

# Create test collection
curl -X POST http://localhost:8080/api/v1/collections \
  -H "Content-Type: application/json" \
  -d '{
    "name": "test",
    "dimension": 128,
    "metric": "cosine"
  }'
```

---

## Contact

**On-Call:** +1-555-ONCALL (24/7)
**Slack:** #akidb-incidents
**Email:** akidb-ops@example.com

**Escalation:**
1. L1: DevOps on-call
2. L2: Backend engineering lead
3. L3: CTO

---

**Document Version:** 1.0
**Last Updated:** 2025-11-08
**Next Review:** 2025-12-08
