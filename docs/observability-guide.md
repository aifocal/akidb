# AkiDB Observability Guide

**Version:** 1.0
**Last Updated:** 2025-11-03
**Phase 4 M2:** OpenTelemetry & Jaeger Integration

---

## Table of Contents

1. [Overview](#overview)
2. [OpenTelemetry Integration](#opentelemetry-integration)
3. [Jaeger Distributed Tracing](#jaeger-distributed-tracing)
4. [Prometheus Metrics](#prometheus-metrics)
5. [Structured Logging](#structured-logging)
6. [Alerting Rules](#alerting-rules)
7. [Troubleshooting](#troubleshooting)

---

## Overview

AkiDB provides three pillars of observability:

1. **📊 Metrics (Prometheus)** - Quantitative measurements (latency, throughput, errors)
2. **🔍 Traces (OpenTelemetry + Jaeger)** - Request flow across services
3. **📝 Logs (Tracing-subscriber)** - Structured event logs

### Architecture

```
┌──────────────┐
│  AkiDB API   │
│              │
│ OpenTelemetry├────▶ Jaeger (Traces)
│   + Metrics  ├────▶ Prometheus (Metrics)
│   + Logging  ├────▶ Stdout/Files (Logs)
└──────────────┘
```

---

## OpenTelemetry Integration

### Configuration

AkiDB uses **OpenTelemetry Protocol (OTLP)** to export traces to Jaeger.

**Environment Variables:**

```bash
# Enable/disable telemetry
AKIDB_TELEMETRY_ENABLED=true  # Set to 'false' to disable

# Jaeger OTLP endpoint (gRPC)
AKIDB_JAEGER_ENDPOINT=http://localhost:4317

# Service name (appears in Jaeger UI)
AKIDB_SERVICE_NAME=akidb-api

# Sampling ratio (0.0 to 1.0)
AKIDB_SAMPLING_RATIO=1.0  # 1.0 = sample all requests

# Export timeout (seconds)
AKIDB_EXPORT_TIMEOUT_SECS=10
```

### Sampling Strategies

| Environment | Sampling Ratio | Rationale |
|-------------|----------------|-----------|
| **Development** | `1.0` (100%) | Trace all requests for debugging |
| **Staging** | `1.0` (100%) | Full observability before production |
| **Production (low traffic)** | `1.0` (100%) | < 1000 req/sec, trace everything |
| **Production (high traffic)** | `0.1` (10%) | > 10K req/sec, reduce overhead |

**Example (Production with 10% sampling):**

```bash
AKIDB_SAMPLING_RATIO=0.1
```

### Disabling Telemetry

For air-gapped deployments without Jaeger:

```bash
AKIDB_TELEMETRY_ENABLED=false
```

AkiDB will fall back to **logging-only mode** (no trace export).

---

## Jaeger Distributed Tracing

### Running Jaeger

**Docker (All-in-One):**

```bash
docker run -d \
  --name jaeger \
  -e COLLECTOR_OTLP_ENABLED=true \
  -p 16686:16686 \
  -p 4317:4317 \
  jaegertracing/all-in-one:latest
```

**Access Jaeger UI:**
```
http://localhost:16686
```

### Trace Structure

**Example Trace: Vector Search Request**

```
http_request (120ms)
├── search_vectors handler (115ms)
│   ├── validate_request (2ms)
│   ├── load_collection_manifest (15ms)
│   │   └── s3_get_object (12ms)
│   ├── load_segments (40ms)
│   │   ├── cache_lookup (1ms) [HIT]
│   │   └── s3_get_object (35ms) [MISS]
│   ├── hnsw_index_search (50ms)
│   │   ├── navigate_graph (30ms)
│   │   └── compute_distances (20ms)
│   └── apply_filter (8ms)
│       └── payload_match (7ms)
└── serialize_response (5ms)
```

### Viewing Traces in Jaeger

1. **Select Service:** `akidb-api`
2. **Select Operation:** `http_request` or `search_vectors`
3. **Lookback:** Last 1 hour
4. **Click "Find Traces"**

### Key Trace Attributes

| Attribute | Example | Description |
|-----------|---------|-------------|
| `http.method` | `POST` | HTTP method |
| `http.url` | `/collections/products/search` | Request path |
| `http.status_code` | `200` | Response status |
| `collection.name` | `products` | Collection being queried |
| `vector.dimension` | `768` | Vector dimension |
| `top_k` | `10` | Number of results requested |
| `filter.enabled` | `true` | Whether filter was applied |
| `s3.operation` | `GetObject` | S3 operation type |
| `index.type` | `HNSW` | Index provider used |

### Performance Insights from Traces

**1. Identify Slow S3 Operations**

Look for spans with `s3.operation` and long duration:
- Normal: < 50ms
- Degraded: 50-200ms
- Problem: > 200ms (check MinIO network/storage)

**2. Detect Index Inefficiencies**

Check `hnsw_index_search` span:
- If `navigate_graph` > 50ms → Increase `ef_search`
- If `compute_distances` > 50ms → Too many vectors, consider sharding

**3. Find Cache Misses**

Look for `cache_lookup` tags:
- `cache.hit=true` → Fast (< 5ms)
- `cache.hit=false` → Slow (requires S3 fetch)

If cache miss rate > 30%, increase NVMe cache size.

---

## Prometheus Metrics

### Scraping Configuration

**Prometheus `prometheus.yml`:**

```yaml
scrape_configs:
  - job_name: 'akidb'
    static_configs:
      - targets: ['akidb:8080']
    metrics_path: '/metrics'
    scrape_interval: 15s
```

### Key Metrics Reference

#### API Metrics

**`akidb_api_requests_total` (Counter)**
- Total requests by method, endpoint, and status
- **Labels:** `method`, `endpoint`, `status`
- **Use:** Track request volume and error rate

**Example Query:**
```promql
# Request rate (requests/sec)
rate(akidb_api_requests_total[5m])

# Error rate (%)
100 * sum(rate(akidb_api_requests_total{status=~"5.."}[5m])) /
      sum(rate(akidb_api_requests_total[5m]))
```

**`akidb_api_request_duration_seconds` (Histogram)**
- Request latency distribution
- **Labels:** `method`, `endpoint`
- **Buckets:** 1ms, 5ms, 10ms, 25ms, 50ms, 100ms, 250ms, 500ms, 1s, 2.5s, 5s, 10s

**Example Query:**
```promql
# P99 latency for search endpoint
histogram_quantile(0.99,
  rate(akidb_api_request_duration_seconds_bucket{endpoint="/search"}[5m]))

# P50 latency (median)
histogram_quantile(0.50,
  rate(akidb_api_request_duration_seconds_bucket[5m]))
```

#### Storage Metrics

**`akidb_storage_operations_total` (Counter)**
- S3 operations by type and status
- **Labels:** `operation` (get/put/delete), `status` (success/error)

**Example Query:**
```promql
# S3 error rate
rate(akidb_storage_operations_total{status="error"}[5m])
```

**`akidb_storage_latency_seconds` (Histogram)**
- S3 operation latency

**Example Query:**
```promql
# P95 S3 latency
histogram_quantile(0.95,
  rate(akidb_storage_latency_seconds_bucket[5m]))
```

**`akidb_circuit_breaker_state` (Gauge)**
- Circuit breaker state (0=closed, 1=open, 2=half-open)
- **Labels:** `backend` (s3, wal, etc.)

**Example Query:**
```promql
# Alert when circuit breaker opens
akidb_circuit_breaker_state{backend="s3"} > 0
```

#### Index Metrics

**`akidb_index_search_duration_seconds` (Histogram)**
- Index search latency
- **Labels:** `index_type` (hnsw, native), `distance_metric`

**Example Query:**
```promql
# HNSW search P99 latency
histogram_quantile(0.99,
  rate(akidb_index_search_duration_seconds_bucket{index_type="hnsw"}[5m]))
```

**`akidb_index_vectors_total` (Gauge)**
- Total vectors in index
- **Labels:** `collection`, `index_type`

#### WAL Metrics

**`akidb_wal_operations_total` (Counter)**
- WAL operations by type and status
- **Labels:** `operation` (append, flush, replay), `status`

**`akidb_wal_size_bytes` (Gauge)**
- Current WAL size per collection
- **Labels:** `collection`

**Example Query:**
```promql
# Alert if WAL size > 1GB
akidb_wal_size_bytes > 1e9
```

#### Query Profiling Metrics

**`akidb_slow_queries_total` (Counter)**
- Number of slow queries detected
- **Labels:** `collection`, `threshold_ms`

**`akidb_query_stage_duration_seconds` (Histogram)**
- Query execution stage timings
- **Labels:** `stage` (planning, execution, filtering, serialization)

### Dashboards

**Grafana Dashboard Example:**

```json
{
  "dashboard": {
    "title": "AkiDB Overview",
    "panels": [
      {
        "title": "Request Rate (req/sec)",
        "targets": [
          {
            "expr": "rate(akidb_api_requests_total[5m])"
          }
        ]
      },
      {
        "title": "Latency Percentiles",
        "targets": [
          {
            "expr": "histogram_quantile(0.50, rate(akidb_api_request_duration_seconds_bucket[5m]))",
            "legendFormat": "P50"
          },
          {
            "expr": "histogram_quantile(0.95, rate(akidb_api_request_duration_seconds_bucket[5m]))",
            "legendFormat": "P95"
          },
          {
            "expr": "histogram_quantile(0.99, rate(akidb_api_request_duration_seconds_bucket[5m]))",
            "legendFormat": "P99"
          }
        ]
      },
      {
        "title": "Error Rate (%)",
        "targets": [
          {
            "expr": "100 * sum(rate(akidb_api_requests_total{status=~\"5..\"}[5m])) / sum(rate(akidb_api_requests_total[5m]))"
          }
        ]
      }
    ]
  }
}
```

---

## Structured Logging

AkiDB uses **`tracing`** for structured, leveled logging.

### Log Levels

Set via `RUST_LOG` environment variable:

```bash
# Production (default)
RUST_LOG=info

# Debug mode
RUST_LOG=debug

# Trace mode (very verbose)
RUST_LOG=trace

# Per-module filtering
RUST_LOG=akidb_api=debug,akidb_storage=info
```

### Log Format

**JSON Format (for log aggregation):**

```json
{
  "timestamp": "2025-01-15T10:30:45.123Z",
  "level": "INFO",
  "target": "akidb_api::handlers::search",
  "message": "Search request completed",
  "fields": {
    "request_id": "550e8400-e29b-41d4-a716-446655440000",
    "collection": "products",
    "top_k": 10,
    "latency_ms": 45,
    "results": 10
  },
  "span": {
    "name": "search_vectors",
    "http.method": "POST",
    "http.url": "/collections/products/search"
  }
}
```

**Human-Readable Format (development):**

```
2025-01-15T10:30:45.123Z  INFO  search_vectors{request_id=550e8400} Search request completed latency_ms=45 results=10
```

### Log Shipping

**To Elasticsearch/Loki:**

```bash
# JSON logs to stdout
RUST_LOG=info

# Ship via Fluentd/Promtail
docker logs -f akidb | fluentd -c /etc/fluentd/fluent.conf
```

**To CloudWatch (AWS):**

```bash
# Use awslogs driver
docker run -d \
  --log-driver=awslogs \
  --log-opt awslogs-group=/ecs/akidb \
  --log-opt awslogs-region=us-east-1 \
  ghcr.io/aifocal/akidb:latest
```

---

## Alerting Rules

### Prometheus Alerts

**`/etc/prometheus/alerts/akidb.yml`:**

```yaml
groups:
  - name: akidb_alerts
    interval: 30s
    rules:
      # High error rate
      - alert: AkiDBHighErrorRate
        expr: |
          100 * sum(rate(akidb_api_requests_total{status=~"5.."}[5m])) /
                sum(rate(akidb_api_requests_total[5m])) > 5
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "AkiDB error rate > 5%"
          description: "Error rate is {{ $value | humanizePercentage }}"

      # High latency
      - alert: AkiDBHighLatency
        expr: |
          histogram_quantile(0.99,
            rate(akidb_api_request_duration_seconds_bucket[5m])) > 1.0
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "AkiDB P99 latency > 1s"
          description: "P99 latency is {{ $value }}s"

      # Circuit breaker open
      - alert: AkiDBCircuitBreakerOpen
        expr: akidb_circuit_breaker_state{backend="s3"} > 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "AkiDB S3 circuit breaker open"
          description: "Storage backend is degraded or unavailable"

      # WAL size growing
      - alert: AkiDBWALSizeLarge
        expr: akidb_wal_size_bytes > 1e9  # 1GB
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "AkiDB WAL size > 1GB"
          description: "WAL for collection {{ $labels.collection }} is {{ $value | humanizeBytes }}"

      # Slow queries
      - alert: AkiDBSlowQueries
        expr: rate(akidb_slow_queries_total[5m]) > 0.1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "AkiDB detecting slow queries"
          description: "Slow query rate: {{ $value }} queries/sec"

      # Service down
      - alert: AkiDBDown
        expr: up{job="akidb"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "AkiDB is down"
          description: "Cannot scrape metrics from AkiDB"
```

### Alert Routing (AlertManager)

**`/etc/alertmanager/alertmanager.yml`:**

```yaml
route:
  group_by: ['alertname', 'cluster']
  group_wait: 10s
  group_interval: 5m
  repeat_interval: 4h
  receiver: 'default'
  routes:
    - match:
        severity: critical
      receiver: 'pagerduty'
    - match:
        severity: warning
      receiver: 'slack'

receivers:
  - name: 'pagerduty'
    pagerduty_configs:
      - service_key: '<pagerduty_key>'

  - name: 'slack'
    slack_configs:
      - api_url: '<slack_webhook_url>'
        channel: '#akidb-alerts'
        text: '{{ range .Alerts }}{{ .Annotations.summary }}: {{ .Annotations.description }}{{ end }}'
```

---

## Troubleshooting

### Jaeger Not Receiving Traces

**Symptoms:**
- Jaeger UI shows no traces
- AkiDB logs: `Error exporting traces: connection refused`

**Diagnosis:**

```bash
# Check if Jaeger is running
curl http://localhost:4317

# Check AkiDB logs
docker logs akidb | grep -i jaeger

# Verify OTLP endpoint
docker exec akidb env | grep JAEGER_ENDPOINT
```

**Solution:**

```bash
# Ensure Jaeger is accessible
docker network inspect bridge | grep -A 3 akidb
docker network inspect bridge | grep -A 3 jaeger

# Update endpoint to use container name
AKIDB_JAEGER_ENDPOINT=http://jaeger:4317
```

### High Metric Cardinality

**Symptoms:**
- Prometheus memory usage growing
- Slow `/metrics` endpoint (> 500ms)

**Diagnosis:**

```bash
# Check metric cardinality
curl -s http://localhost:8080/metrics | grep akidb | wc -l

# Expected: < 1000 metrics
# Problem: > 10,000 metrics (too many label values)
```

**Solution:**

```bash
# Reduce label cardinality (avoid high-cardinality labels like user_id, request_id)
# Metrics should use labels like:
#   - collection_name (low cardinality: 10-100 collections)
#   - endpoint (low cardinality: 10-20 endpoints)
# NOT:
#   - vector_id (high cardinality: millions)
#   - request_id (high cardinality: unique per request)
```

### Missing Logs

**Symptoms:**
- No logs appearing in stdout

**Diagnosis:**

```bash
# Check RUST_LOG level
docker exec akidb env | grep RUST_LOG

# Try setting to debug
docker restart akidb -e RUST_LOG=debug
```

---

## Best Practices

1. **Always enable telemetry in staging** - Catch performance regressions before production
2. **Use sampling in high-traffic production** - Reduce overhead while maintaining visibility
3. **Set up alerting for P99 latency** - Catch tail latency issues early
4. **Monitor circuit breaker state** - Indicates S3/storage problems
5. **Track WAL size** - Large WAL indicates flush issues
6. **Review Jaeger traces weekly** - Identify optimization opportunities

---

## Performance Impact

| Feature | Overhead | Recommendation |
|---------|----------|----------------|
| OpenTelemetry tracing (100% sampling) | ~5-10% CPU, ~2-5ms latency | Use in dev/staging |
| OpenTelemetry tracing (10% sampling) | < 1% CPU, < 0.5ms latency | Use in production |
| Prometheus metrics | < 1% CPU | Always enable |
| Structured logging (info level) | < 1% CPU | Always enable |
| Structured logging (debug level) | ~3-5% CPU | Development only |

---

**Support:** https://github.com/aifocal/akidb/issues
**OpenTelemetry Docs:** https://opentelemetry.io/docs/
**Jaeger Docs:** https://www.jaegertracing.io/docs/
