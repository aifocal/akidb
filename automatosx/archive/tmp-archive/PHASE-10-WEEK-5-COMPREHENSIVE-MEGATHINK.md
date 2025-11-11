# Phase 10 Week 5: Comprehensive Megathink - Observability (Prometheus/Grafana/OpenTelemetry)

**Status:** Planning
**Created:** 2025-11-09
**Phase:** Phase 10 (S3/MinIO Tiered Storage) - Week 5
**Part:** Part B - Production Hardening

---

## Executive Summary

Week 5 establishes comprehensive observability infrastructure for AkiDB 2.0, enabling production monitoring, alerting, and distributed tracing. This is a critical foundation for operating AkiDB in production environments.

**Goals:**
- Prometheus metrics exporter (12+ metrics)
- Grafana dashboards (4 dashboards)
- OpenTelemetry distributed tracing
- Alert rules and runbook

**Why This Matters:**
- **Operational Excellence**: Proactive monitoring prevents outages
- **Performance Insights**: Identify bottlenecks before they impact users
- **Debugging**: Distributed tracing reveals complex failure scenarios
- **SLO Tracking**: Measure and maintain service level objectives

---

## Table of Contents

1. [Background and Context](#background-and-context)
2. [Observability Architecture](#observability-architecture)
3. [Prometheus Metrics](#prometheus-metrics)
4. [Grafana Dashboards](#grafana-dashboards)
5. [OpenTelemetry Distributed Tracing](#opentelemetry-distributed-tracing)
6. [Alert Rules and Runbook](#alert-rules-and-runbook)
7. [Implementation Plan](#implementation-plan)
8. [Testing Strategy](#testing-strategy)
9. [Success Criteria](#success-criteria)
10. [Risk Analysis](#risk-analysis)

---

## Background and Context

### Why Observability Matters

Production systems require three pillars of observability:
1. **Metrics**: Time-series data for trends and alerting
2. **Logs**: Structured event data for debugging
3. **Traces**: Request flow through distributed systems

AkiDB 2.0 is a complex system with multiple layers:
- REST/gRPC API servers
- Collection management service
- Vector indexing (HNSW)
- Tiered storage (hot/warm/cold)
- S3/MinIO object storage
- Background workers (tiering, DLQ)

Without observability, operators are blind to:
- Performance degradation
- Resource exhaustion
- S3 API errors
- Tier distribution imbalance
- Memory leaks
- Slow queries

### Current State (Week 4 End)

**What We Have:**
- Basic logging with `tracing` crate
- Manual performance benchmarks
- Unit and integration tests

**What's Missing:**
- Real-time metrics collection
- Production dashboards
- Distributed request tracing
- Automated alerting
- Operational runbook

### Week 5 Objectives

1. **Prometheus Integration**
   - Export 12+ key metrics
   - Instrument all critical paths
   - Expose `/metrics` endpoint

2. **Grafana Dashboards**
   - Overview dashboard (system health)
   - Performance dashboard (latency, throughput)
   - Storage dashboard (tier distribution, S3 metrics)
   - Error dashboard (error rates, DLQ status)

3. **OpenTelemetry Tracing**
   - Distributed traces for all API requests
   - Span hierarchy (REST → Service → Index → S3)
   - Jaeger integration for visualization

4. **Alerting**
   - 10+ alert rules (P50/P95 latency, error rates, resource usage)
   - Prometheus AlertManager configuration
   - Runbook for common alerts

---

## Observability Architecture

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      AkiDB 2.0                              │
├─────────────────────────────────────────────────────────────┤
│  REST API          │  gRPC API                              │
│  /metrics          │  /metrics                              │
└────────┬────────────┴───────────┬────────────────────────────┘
         │                        │
         │ Prometheus scrape      │ OpenTelemetry export
         │                        │
┌────────▼────────┐      ┌────────▼────────┐
│  Prometheus     │      │  Jaeger         │
│  Time-series DB │      │  Trace storage  │
└────────┬────────┘      └─────────────────┘
         │
         │ Query
         │
┌────────▼────────┐
│  Grafana        │
│  Visualization  │
└─────────────────┘
         │
         │ Alerts
         │
┌────────▼────────┐
│  AlertManager   │
│  Notifications  │
└─────────────────┘
```

### Component Responsibilities

**Prometheus:**
- Scrapes `/metrics` endpoint every 15 seconds
- Stores time-series metrics data
- Evaluates alert rules
- Provides query API (PromQL)

**Grafana:**
- Visualizes metrics from Prometheus
- Custom dashboards for different personas
- Drill-down analysis
- Panel annotations

**Jaeger:**
- Collects distributed traces
- Stores trace spans
- Trace visualization and analysis
- Service dependency graphs

**AlertManager:**
- Routes alerts to appropriate channels
- Alert grouping and deduplication
- Notification integrations (Slack, PagerDuty)

---

## Prometheus Metrics

### Metric Categories

We'll export 12+ metrics across 4 categories:

1. **Request Metrics** (4 metrics)
   - `akidb_http_requests_total` (counter)
   - `akidb_http_request_duration_seconds` (histogram)
   - `akidb_grpc_requests_total` (counter)
   - `akidb_grpc_request_duration_seconds` (histogram)

2. **Vector Operations** (3 metrics)
   - `akidb_vector_search_duration_seconds` (histogram)
   - `akidb_vector_insert_duration_seconds` (histogram)
   - `akidb_collection_size_bytes` (gauge)

3. **Storage Metrics** (3 metrics)
   - `akidb_tier_distribution` (gauge, by tier)
   - `akidb_s3_operations_total` (counter, by operation)
   - `akidb_s3_operation_duration_seconds` (histogram)

4. **System Metrics** (2 metrics)
   - `akidb_memory_usage_bytes` (gauge)
   - `akidb_background_worker_runs_total` (counter)

### Metric Design Principles

1. **Naming Convention**: `{namespace}_{subsystem}_{metric}_{unit}`
   - Namespace: `akidb`
   - Subsystem: `http`, `grpc`, `vector`, `storage`, `system`
   - Unit: `seconds`, `bytes`, `total`

2. **Label Strategy**:
   - Use labels for high-cardinality dimensions (collection_id, operation)
   - Avoid unbounded cardinality (no user IDs)
   - Common labels: `method`, `status_code`, `tier`, `operation`

3. **Metric Types**:
   - **Counter**: Monotonically increasing (requests, errors)
   - **Gauge**: Current value (memory, collection count)
   - **Histogram**: Distribution (latency buckets)

### Detailed Metric Specifications

#### 1. `akidb_http_requests_total`

**Type:** Counter
**Description:** Total number of HTTP requests by method and status code
**Labels:**
- `method`: GET, POST, PUT, DELETE
- `path`: /collections, /search, /insert, /metrics
- `status_code`: 200, 400, 500, etc.

**Usage:**
```promql
# Request rate
rate(akidb_http_requests_total[5m])

# Error rate (5xx)
rate(akidb_http_requests_total{status_code=~"5.."}[5m])

# Request count by endpoint
sum by (path) (akidb_http_requests_total)
```

**Implementation:**
```rust
use prometheus::{Counter, register_counter_vec};

lazy_static! {
    static ref HTTP_REQUESTS: CounterVec = register_counter_vec!(
        "akidb_http_requests_total",
        "Total number of HTTP requests",
        &["method", "path", "status_code"]
    ).unwrap();
}

// In request handler:
HTTP_REQUESTS
    .with_label_values(&["POST", "/collections", "201"])
    .inc();
```

#### 2. `akidb_http_request_duration_seconds`

**Type:** Histogram
**Description:** HTTP request latency distribution
**Labels:**
- `method`: GET, POST, PUT, DELETE
- `path`: /collections, /search, /insert

**Buckets:** [0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]

**Usage:**
```promql
# P95 latency
histogram_quantile(0.95, rate(akidb_http_request_duration_seconds_bucket[5m]))

# P50 latency
histogram_quantile(0.50, rate(akidb_http_request_duration_seconds_bucket[5m]))

# Average latency
rate(akidb_http_request_duration_seconds_sum[5m]) / rate(akidb_http_request_duration_seconds_count[5m])
```

**Implementation:**
```rust
use prometheus::{Histogram, HistogramOpts, register_histogram_vec};

lazy_static! {
    static ref HTTP_DURATION: HistogramVec = register_histogram_vec!(
        "akidb_http_request_duration_seconds",
        "HTTP request latency in seconds",
        &["method", "path"],
        vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
    ).unwrap();
}

// In request handler:
let timer = HTTP_DURATION
    .with_label_values(&["POST", "/collections"])
    .start_timer();

// ... handle request ...

timer.observe_duration();
```

#### 3. `akidb_grpc_requests_total`

**Type:** Counter
**Description:** Total number of gRPC requests by method and status
**Labels:**
- `method`: CreateCollection, Search, Insert, GetCollectionInfo
- `status`: OK, INVALID_ARGUMENT, INTERNAL, etc.

**Implementation:**
```rust
// In gRPC handler:
GRPC_REQUESTS
    .with_label_values(&["Search", "OK"])
    .inc();
```

#### 4. `akidb_grpc_request_duration_seconds`

**Type:** Histogram
**Description:** gRPC request latency distribution
**Labels:** `method`

**Buckets:** Same as HTTP (0.001 to 10.0)

#### 5. `akidb_vector_search_duration_seconds`

**Type:** Histogram
**Description:** Vector search latency by index type
**Labels:**
- `index_type`: brute_force, hnsw
- `tier`: hot, warm, cold

**Buckets:** [0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]

**Usage:**
```promql
# Search P95 by tier
histogram_quantile(0.95, rate(akidb_vector_search_duration_seconds_bucket[5m])) by (tier)

# Hot tier search latency
histogram_quantile(0.95, rate(akidb_vector_search_duration_seconds_bucket{tier="hot"}[5m]))
```

**Implementation:**
```rust
lazy_static! {
    static ref SEARCH_DURATION: HistogramVec = register_histogram_vec!(
        "akidb_vector_search_duration_seconds",
        "Vector search latency in seconds",
        &["index_type", "tier"],
        vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]
    ).unwrap();
}

// In search handler:
let timer = SEARCH_DURATION
    .with_label_values(&["hnsw", "hot"])
    .start_timer();

let results = index.search(query, k).await?;

timer.observe_duration();
```

#### 6. `akidb_vector_insert_duration_seconds`

**Type:** Histogram
**Description:** Vector insert latency by index type
**Labels:** `index_type`

#### 7. `akidb_collection_size_bytes`

**Type:** Gauge
**Description:** Memory footprint of each collection
**Labels:** `collection_id`, `tier`

**Usage:**
```promql
# Total memory usage by tier
sum by (tier) (akidb_collection_size_bytes)

# Largest collections
topk(10, akidb_collection_size_bytes)
```

**Implementation:**
```rust
lazy_static! {
    static ref COLLECTION_SIZE: GaugeVec = register_gauge_vec!(
        "akidb_collection_size_bytes",
        "Memory footprint of collection in bytes",
        &["collection_id", "tier"]
    ).unwrap();
}

// Update on insert/delete:
COLLECTION_SIZE
    .with_label_values(&[collection_id.as_str(), "hot"])
    .set(new_size as f64);
```

#### 8. `akidb_tier_distribution`

**Type:** Gauge
**Description:** Number of collections in each tier
**Labels:** `tier`

**Usage:**
```promql
# Tier distribution
akidb_tier_distribution

# Hot tier saturation
akidb_tier_distribution{tier="hot"} / on() sum(akidb_tier_distribution)
```

**Implementation:**
```rust
// Update on tier transition:
TIER_DISTRIBUTION
    .with_label_values(&["hot"])
    .inc();

TIER_DISTRIBUTION
    .with_label_values(&["warm"])
    .dec();
```

#### 9. `akidb_s3_operations_total`

**Type:** Counter
**Description:** Total S3 operations by type
**Labels:** `operation` (put, get, delete, list)

**Usage:**
```promql
# S3 operation rate
rate(akidb_s3_operations_total[5m])

# S3 upload rate
rate(akidb_s3_operations_total{operation="put"}[5m])
```

#### 10. `akidb_s3_operation_duration_seconds`

**Type:** Histogram
**Description:** S3 operation latency
**Labels:** `operation`

**Buckets:** [0.01, 0.05, 0.1, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0]

**Usage:**
```promql
# S3 upload P95 latency
histogram_quantile(0.95, rate(akidb_s3_operation_duration_seconds_bucket{operation="put"}[5m]))
```

#### 11. `akidb_memory_usage_bytes`

**Type:** Gauge
**Description:** Total process memory usage
**Labels:** None

**Implementation:**
```rust
use sysinfo::{System, SystemExt, ProcessExt};

// Update every 10 seconds:
let mut system = System::new_all();
system.refresh_all();

if let Some(process) = system.process(sysinfo::get_current_pid().unwrap()) {
    MEMORY_USAGE.set(process.memory() as f64);
}
```

#### 12. `akidb_background_worker_runs_total`

**Type:** Counter
**Description:** Background worker cycle count
**Labels:** `worker_type` (tiering, dlq_cleanup)

---

## Grafana Dashboards

### Dashboard 1: System Overview

**Purpose:** High-level system health snapshot for SREs

**Panels (8 total):**

1. **Request Rate (Time-series)**
   - Query: `rate(akidb_http_requests_total[5m])`
   - Grouped by: `method`
   - Display: Line chart, stacked

2. **Error Rate (Time-series)**
   - Query: `rate(akidb_http_requests_total{status_code=~"5.."}[5m])`
   - Display: Line chart, red color
   - Alert: Error rate > 1%

3. **P95 Latency (Time-series)**
   - Query: `histogram_quantile(0.95, rate(akidb_http_request_duration_seconds_bucket[5m]))`
   - Display: Line chart
   - Alert: P95 > 25ms

4. **Memory Usage (Time-series)**
   - Query: `akidb_memory_usage_bytes`
   - Display: Line chart with threshold line
   - Alert: Memory > 80% quota

5. **Tier Distribution (Pie chart)**
   - Query: `akidb_tier_distribution`
   - Display: Pie chart (hot/warm/cold)

6. **S3 Operation Rate (Time-series)**
   - Query: `rate(akidb_s3_operations_total[5m]) by (operation)`
   - Display: Line chart, grouped by operation

7. **Collection Count (Single stat)**
   - Query: `sum(akidb_tier_distribution)`
   - Display: Large number

8. **Uptime (Single stat)**
   - Query: `time() - process_start_time_seconds`
   - Display: Duration format

**Dashboard JSON Export:**
```json
{
  "dashboard": {
    "title": "AkiDB - System Overview",
    "panels": [
      {
        "id": 1,
        "title": "Request Rate",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(akidb_http_requests_total[5m])",
            "legendFormat": "{{method}}"
          }
        ]
      }
      // ... other panels ...
    ]
  }
}
```

### Dashboard 2: Performance Dashboard

**Purpose:** Detailed performance analysis for engineers

**Panels (10 total):**

1. **Search Latency by Tier (Heatmap)**
   - Query: `akidb_vector_search_duration_seconds_bucket` by tier
   - Display: Heatmap showing latency distribution

2. **Search P50/P95/P99 (Time-series)**
   - Queries:
     - P50: `histogram_quantile(0.50, ...)`
     - P95: `histogram_quantile(0.95, ...)`
     - P99: `histogram_quantile(0.99, ...)`
   - Display: Multi-line chart

3. **Insert Latency (Histogram)**
   - Query: `akidb_vector_insert_duration_seconds_bucket`
   - Display: Histogram showing distribution

4. **Request Throughput (Time-series)**
   - Query: `rate(akidb_http_requests_total[5m])`
   - Display: Area chart

5. **Hot Tier Search Performance (Time-series)**
   - Query: `histogram_quantile(0.95, rate(akidb_vector_search_duration_seconds_bucket{tier="hot"}[5m]))`
   - Display: Line chart
   - Target: <5ms

6. **Warm Tier Search Performance (Time-series)**
   - Query: Same as above, tier="warm"
   - Target: <25ms

7. **Cold Tier Search Performance (Time-series)**
   - Query: Same as above, tier="cold"
   - Target: <10s

8. **Batch Upload Throughput (Time-series)**
   - Query: `rate(akidb_s3_operations_total{operation="put"}[5m])`
   - Display: Line chart
   - Target: >500 ops/sec

9. **Collection Size Distribution (Bar chart)**
   - Query: `akidb_collection_size_bytes`
   - Display: Horizontal bar chart, top 20 collections

10. **Background Worker Frequency (Time-series)**
    - Query: `rate(akidb_background_worker_runs_total[5m])`
    - Display: Line chart by worker_type

### Dashboard 3: Storage Dashboard

**Purpose:** Monitor tiered storage and S3 operations

**Panels (8 total):**

1. **Tier Distribution Over Time (Stacked area)**
   - Query: `akidb_tier_distribution` by tier
   - Display: Stacked area chart

2. **Hot → Warm Demotion Rate (Time-series)**
   - Query: Derived from tier transition events
   - Display: Line chart

3. **Warm → Hot Promotion Rate (Time-series)**
   - Query: Derived from tier transition events
   - Display: Line chart

4. **S3 Upload Success Rate (Time-series)**
   - Query: `rate(akidb_s3_operations_total{operation="put", status="success"}[5m])`
   - Display: Line chart with 100% reference line

5. **S3 Upload P95 Latency (Time-series)**
   - Query: `histogram_quantile(0.95, rate(akidb_s3_operation_duration_seconds_bucket{operation="put"}[5m]))`
   - Display: Line chart

6. **S3 Download P95 Latency (Time-series)**
   - Query: Same as above, operation="get"
   - Display: Line chart

7. **Cold Tier Size (Gauge)**
   - Query: `sum(akidb_collection_size_bytes{tier="cold"})`
   - Display: Gauge with max capacity threshold

8. **Snapshot Creation Rate (Time-series)**
   - Query: Derived from snapshot events
   - Display: Line chart

### Dashboard 4: Error Dashboard

**Purpose:** Monitor errors and failures for incident response

**Panels (8 total):**

1. **Error Rate by Endpoint (Time-series)**
   - Query: `rate(akidb_http_requests_total{status_code=~"5.."}[5m]) by (path)`
   - Display: Line chart, grouped by path

2. **DLQ Size (Time-series)**
   - Query: `akidb_dlq_size`
   - Display: Line chart
   - Alert: DLQ size > 100

3. **S3 Error Rate (Time-series)**
   - Query: `rate(akidb_s3_operations_total{status="error"}[5m])`
   - Display: Line chart
   - Alert: S3 error rate > 1%

4. **Circuit Breaker Status (Single stat)**
   - Query: `akidb_circuit_breaker_state{breaker="s3"}`
   - Display: Traffic light (green=closed, yellow=half_open, red=open)

5. **Failed Demotion Count (Counter)**
   - Query: `akidb_tier_demotion_failures_total`
   - Display: Single stat

6. **Failed Promotion Count (Counter)**
   - Query: `akidb_tier_promotion_failures_total`
   - Display: Single stat

7. **Recent Error Logs (Table)**
   - Query: Loki integration for ERROR level logs
   - Display: Table with timestamp, message, context

8. **Alert Status (Table)**
   - Query: ALERTS{alertstate="firing"}
   - Display: Table with alert name, severity, duration

---

## OpenTelemetry Distributed Tracing

### Why Distributed Tracing?

AkiDB requests flow through multiple layers:
```
REST API → CollectionService → IndexManager → ObjectStore → S3
```

Without tracing, debugging slow requests requires:
- Manual log correlation
- Guessing which layer is slow
- Reproducing issues locally

With tracing:
- See full request timeline
- Identify bottleneck layer immediately
- Correlate across services

### Tracing Architecture

**Components:**
1. **OpenTelemetry SDK**: Instrumentation library
2. **Jaeger**: Trace storage and UI
3. **Trace Exporter**: OTLP (OpenTelemetry Protocol)

**Trace Hierarchy:**
```
Span: POST /search (50ms total)
  └─ Span: CollectionService::search (45ms)
      ├─ Span: TieringManager::record_access (1ms)
      ├─ Span: IndexManager::search (40ms)
      │   ├─ Span: HNSW::search (35ms)
      │   └─ Span: BruteForce fallback (5ms)
      └─ Span: ObjectStore::get (4ms)
          └─ Span: S3::GetObject (3ms)
```

### Instrumentation Strategy

**Auto-instrumentation:**
- HTTP requests (axum middleware)
- gRPC requests (tonic interceptor)

**Manual instrumentation:**
- Service layer methods
- Index operations
- S3 operations
- Background workers

### Span Attributes

**Standard Attributes:**
- `http.method`: GET, POST, etc.
- `http.url`: /collections/123/search
- `http.status_code`: 200, 500
- `db.system`: akidb
- `db.operation`: search, insert

**Custom Attributes:**
- `akidb.collection_id`: UUID
- `akidb.tier`: hot, warm, cold
- `akidb.index_type`: hnsw, brute_force
- `akidb.vector_dimension`: 512
- `akidb.search_k`: 10

### Implementation Example

**File:** `crates/akidb-core/src/tracing.rs`

```rust
use opentelemetry::{global, trace::Tracer, KeyValue};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_tracing(service_name: &str) -> anyhow::Result<()> {
    // Initialize OpenTelemetry
    global::set_text_map_propagator(opentelemetry_jaeger::Propagator::new());

    let tracer = opentelemetry_jaeger::new_agent_pipeline()
        .with_service_name(service_name)
        .with_endpoint("127.0.0.1:6831")
        .install_simple()?;

    // Create tracing layer
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    // Initialize subscriber
    tracing_subscriber::registry()
        .with(telemetry)
        .with(tracing_subscriber::fmt::layer())
        .init();

    Ok(())
}
```

**Instrumented Search Handler:**

```rust
use tracing::{info_span, instrument};

#[instrument(
    name = "search_vectors",
    skip(service, query),
    fields(
        collection_id = %collection_id,
        k = %k,
    )
)]
pub async fn search(
    service: Arc<CollectionService>,
    collection_id: CollectionId,
    query: Vec<f32>,
    k: usize,
) -> CoreResult<Vec<SearchResult>> {
    // This function automatically creates a span

    // Record custom attributes
    tracing::Span::current().record("vector_dimension", query.len());

    // Service layer automatically creates child span
    let results = service.search(collection_id, query, k).await?;

    Ok(results)
}
```

**Instrumenting S3 Operations:**

```rust
#[instrument(
    name = "s3_put",
    skip(self, value),
    fields(
        key = %key,
        size_bytes = value.len(),
    )
)]
async fn put(&self, key: &str, value: Bytes) -> CoreResult<()> {
    let span = tracing::Span::current();

    // Record S3 endpoint
    span.record("s3.endpoint", &self.config.endpoint);

    // Perform operation
    let result = self.client.put_object()
        .bucket(&self.config.bucket)
        .key(key)
        .body(value.into())
        .send()
        .await;

    match result {
        Ok(_) => {
            span.record("s3.status", "success");
            Ok(())
        }
        Err(e) => {
            span.record("s3.status", "error");
            span.record("s3.error", &e.to_string());
            Err(CoreError::Storage(e.to_string()))
        }
    }
}
```

### Trace Sampling

**Problem:** 100% trace sampling = high overhead

**Solution:** Head-based sampling
- Sample 10% of requests in production
- Always sample errors (status_code >= 500)
- Always sample slow requests (latency > 1s)

**Configuration:**
```rust
use opentelemetry::sdk::trace::Sampler;

let sampler = Sampler::ParentBased(Box::new(
    Sampler::TraceIdRatioBased(0.1) // 10% sampling
));
```

---

## Alert Rules and Runbook

### Alert Design Principles

1. **Actionable**: Every alert should have a clear action
2. **Symptom-based**: Alert on user impact, not internal metrics
3. **Severity Levels**:
   - **Critical**: Immediate action required, page on-call
   - **Warning**: Investigate during business hours
   - **Info**: For awareness, no action needed

### Alert Rules

#### 1. High Error Rate (Critical)

**Condition:** HTTP 5xx error rate > 1% for 5 minutes

**PromQL:**
```promql
(
  rate(akidb_http_requests_total{status_code=~"5.."}[5m])
  /
  rate(akidb_http_requests_total[5m])
) > 0.01
```

**Alert:**
```yaml
groups:
  - name: akidb_alerts
    rules:
      - alert: HighErrorRate
        expr: |
          (
            rate(akidb_http_requests_total{status_code=~"5.."}[5m])
            /
            rate(akidb_http_requests_total[5m])
          ) > 0.01
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "High error rate detected"
          description: "Error rate is {{ $value | humanizePercentage }} (threshold: 1%)"
          runbook: https://docs.akidb.com/runbook/high-error-rate
```

**Runbook:**
1. Check Error Dashboard for affected endpoints
2. Check Jaeger for failed traces
3. Check application logs for stack traces
4. Common causes:
   - S3 outage (check S3 error rate metric)
   - Database corruption (check SQLite logs)
   - OOM (check memory usage metric)

#### 2. High P95 Latency (Warning)

**Condition:** P95 search latency > 25ms for 10 minutes

**PromQL:**
```promql
histogram_quantile(0.95, rate(akidb_vector_search_duration_seconds_bucket[5m])) > 0.025
```

**Alert:**
```yaml
- alert: HighSearchLatency
  expr: |
    histogram_quantile(0.95, rate(akidb_vector_search_duration_seconds_bucket[5m])) > 0.025
  for: 10m
  labels:
    severity: warning
  annotations:
    summary: "Search P95 latency exceeds SLO"
    description: "P95 latency is {{ $value }}s (SLO: 25ms)"
```

**Runbook:**
1. Check Performance Dashboard for tier-specific latency
2. Identify slow tier (hot/warm/cold)
3. Actions:
   - Hot tier slow: Check memory pressure, consider scaling
   - Warm tier slow: Check disk I/O, optimize Parquet reads
   - Cold tier slow: Check S3 latency, network issues

#### 3. Memory Pressure (Warning)

**Condition:** Memory usage > 80% of quota for 5 minutes

**PromQL:**
```promql
akidb_memory_usage_bytes > (0.8 * akidb_memory_quota_bytes)
```

**Alert:**
```yaml
- alert: MemoryPressure
  expr: akidb_memory_usage_bytes > (0.8 * akidb_memory_quota_bytes)
  for: 5m
  labels:
    severity: warning
  annotations:
    summary: "Memory usage high"
    description: "Memory usage is {{ $value | humanize1024 }} (80% of quota)"
```

**Runbook:**
1. Check Tier Distribution dashboard
2. Check for memory leaks (heaptrack)
3. Actions:
   - Force demote hot tier collections
   - Increase memory quota (if appropriate)
   - Restart service (temporary fix)

#### 4. S3 Error Rate High (Critical)

**Condition:** S3 error rate > 1% for 5 minutes

**PromQL:**
```promql
(
  rate(akidb_s3_operations_total{status="error"}[5m])
  /
  rate(akidb_s3_operations_total[5m])
) > 0.01
```

**Runbook:**
1. Check S3 service status (AWS Status Page)
2. Check circuit breaker status
3. Check DLQ size (failed uploads queued)
4. Actions:
   - S3 outage: Wait for recovery, circuit breaker handles retries
   - Credentials expired: Rotate IAM credentials
   - Rate limiting: Reduce upload concurrency

#### 5. DLQ Size Growing (Warning)

**Condition:** DLQ size > 100 for 10 minutes

**PromQL:**
```promql
akidb_dlq_size > 100
```

**Runbook:**
1. Check DLQ entries (retrieve from storage)
2. Identify common failure pattern
3. Actions:
   - S3 recoverable errors: Wait for retry
   - S3 permanent errors: Fix configuration
   - Application bugs: Deploy hotfix

#### 6. Circuit Breaker Open (Critical)

**Condition:** Circuit breaker open for S3

**PromQL:**
```promql
akidb_circuit_breaker_state{breaker="s3"} == 2
```

**Runbook:**
1. Check S3 error rate and latency
2. Check S3 service status
3. Actions:
   - S3 outage: Wait for recovery (circuit breaker auto-closes)
   - False positive: Adjust circuit breaker thresholds

#### 7. Background Worker Not Running (Critical)

**Condition:** No background worker runs in 10 minutes

**PromQL:**
```promql
rate(akidb_background_worker_runs_total[10m]) == 0
```

**Runbook:**
1. Check application logs for worker crash
2. Restart service
3. Check for deadlocks (if repeats)

#### 8. Hot Tier Full (Warning)

**Condition:** Hot tier collections > 90% of capacity

**PromQL:**
```promql
akidb_tier_distribution{tier="hot"} > (0.9 * akidb_hot_tier_capacity)
```

**Runbook:**
1. Force demote least recently used collections
2. Increase hot tier capacity (if appropriate)

#### 9. Cold Tier Growth Rate High (Warning)

**Condition:** Cold tier growing > 10GB/hour

**PromQL:**
```promql
deriv(sum(akidb_collection_size_bytes{tier="cold"})[1h]) > 10e9
```

**Runbook:**
1. Check for unexpected data growth
2. Review tiering policies (demotion too aggressive?)
3. Verify S3 bucket retention policies

#### 10. Search Throughput Drop (Critical)

**Condition:** Search throughput drops > 50% in 5 minutes

**PromQL:**
```promql
(
  rate(akidb_vector_search_total[5m])
  /
  rate(akidb_vector_search_total[5m] offset 10m)
) < 0.5
```

**Runbook:**
1. Check request rate (is traffic down?)
2. Check error rate (are searches failing?)
3. Check P95 latency (are searches timing out?)

---

## Implementation Plan

### Day-by-Day Breakdown

**Day 1: Prometheus Metrics Infrastructure**
- Morning: Create metrics module, define metric types
- Afternoon: Instrument REST/gRPC handlers

**Day 2: Service Layer Instrumentation**
- Morning: Instrument CollectionService, IndexManager
- Afternoon: Instrument S3 operations, background workers

**Day 3: Grafana Dashboards**
- Morning: Create System Overview and Performance dashboards
- Afternoon: Create Storage and Error dashboards

**Day 4: OpenTelemetry Tracing**
- Morning: Set up Jaeger, initialize OpenTelemetry
- Afternoon: Instrument critical paths, test trace collection

**Day 5: Alerting and Runbook**
- Morning: Configure Prometheus AlertManager, define alert rules
- Afternoon: Write runbook, test alert firing

---

## Testing Strategy

### Metrics Testing

**Test 1: Metrics Endpoint Returns Valid Prometheus Format**
```rust
#[tokio::test]
async fn test_metrics_endpoint() {
    let response = client.get("/metrics").send().await.unwrap();
    assert_eq!(response.status(), 200);

    let body = response.text().await.unwrap();

    // Verify Prometheus format
    assert!(body.contains("akidb_http_requests_total"));
    assert!(body.contains("akidb_http_request_duration_seconds"));
}
```

**Test 2: Request Counter Increments**
```rust
#[tokio::test]
async fn test_request_counter() {
    let initial = get_metric_value("akidb_http_requests_total{method=\"POST\"}");

    // Make request
    client.post("/collections").send().await.unwrap();

    let final_value = get_metric_value("akidb_http_requests_total{method=\"POST\"}");
    assert_eq!(final_value, initial + 1);
}
```

**Test 3: Latency Histogram Records Values**
```rust
#[tokio::test]
async fn test_latency_histogram() {
    // Make 100 requests
    for _ in 0..100 {
        client.get("/collections").send().await.unwrap();
    }

    // Check histogram has samples
    let metrics = scrape_metrics().await;
    let histogram = metrics.get("akidb_http_request_duration_seconds");

    assert!(histogram.count > 0);
    assert!(histogram.sum > 0.0);
}
```

### Dashboard Testing

**Test 4: Dashboard Queries Return Data**
```promql
# Test in Prometheus UI
rate(akidb_http_requests_total[5m])

# Expected: Non-zero values if traffic exists
```

### Tracing Testing

**Test 5: Traces Are Exported to Jaeger**
```rust
#[tokio::test]
async fn test_trace_export() {
    // Make traced request
    client.post("/search").json(&query).send().await.unwrap();

    // Wait for export
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Query Jaeger API
    let traces = jaeger_client.get_traces("akidb-rest").await.unwrap();
    assert!(!traces.is_empty());
}
```

**Test 6: Span Hierarchy Is Correct**
```rust
#[tokio::test]
async fn test_span_hierarchy() {
    let trace = make_traced_request().await;

    // Verify parent-child relationships
    let root_span = trace.root_span();
    assert_eq!(root_span.operation_name, "POST /search");

    let child_spans = root_span.children();
    assert!(child_spans.iter().any(|s| s.operation_name == "CollectionService::search"));
}
```

### Alert Testing

**Test 7: Alert Fires When Threshold Exceeded**
```bash
# Inject errors to trigger alert
for i in {1..100}; do
  curl -X POST http://localhost:8080/invalid-endpoint
done

# Wait for alert evaluation (5 minutes)
sleep 300

# Check AlertManager
curl http://localhost:9093/api/v1/alerts | jq '.data[] | select(.labels.alertname=="HighErrorRate")'
```

---

## Success Criteria

### Metrics
- ✅ 12+ metrics exported
- ✅ `/metrics` endpoint returns valid Prometheus format
- ✅ Metrics update in real-time

### Dashboards
- ✅ 4 Grafana dashboards created
- ✅ All panels render without errors
- ✅ Dashboard JSON exported for version control

### Tracing
- ✅ Traces exported to Jaeger
- ✅ Span hierarchy correct (parent-child relationships)
- ✅ Critical paths instrumented (REST → Service → Index → S3)

### Alerting
- ✅ 10+ alert rules defined
- ✅ Alerts fire when thresholds exceeded (tested)
- ✅ Runbook documented for each alert

### Testing
- ✅ 7+ tests passing (metrics, dashboards, tracing, alerts)

---

## Risk Analysis

### Risk 1: Metrics Overhead

**Impact:** High
**Probability:** Medium

**Concern:** Excessive metrics collection impacts performance

**Mitigation:**
- Use efficient metric types (counters, histograms)
- Avoid high-cardinality labels
- Benchmark metrics overhead (<1% CPU)

### Risk 2: Trace Sampling Issues

**Impact:** Medium
**Probability:** Medium

**Concern:** Important traces not sampled

**Mitigation:**
- Sample 10% of normal requests
- Always sample errors and slow requests
- Adjust sampling rate based on traffic

### Risk 3: Alert Fatigue

**Impact:** High
**Probability:** High

**Concern:** Too many alerts, on-call ignores them

**Mitigation:**
- Tune thresholds to reduce false positives
- Use severity levels (critical vs warning)
- Group related alerts

### Risk 4: Jaeger Storage Growth

**Impact:** Medium
**Probability:** High

**Concern:** Trace storage grows unbounded

**Mitigation:**
- Configure Jaeger retention (7 days)
- Use Cassandra backend for production
- Monitor Jaeger disk usage

---

## Code Metrics

| Component | Lines of Code | Tests |
|-----------|--------------|-------|
| Prometheus metrics | ~400 | 3 |
| OpenTelemetry tracing | ~300 | 3 |
| Grafana dashboards | ~200 (JSON) | 1 |
| Alert rules | ~100 (YAML) | 1 |
| **Total** | **~1,000** | **8** |

---

## Dependencies

### Internal Dependencies
- ✅ Week 1-4: Existing infrastructure (servers, services, storage)

### External Dependencies
- **Rust Crates:**
  - `prometheus` (metrics)
  - `opentelemetry` (tracing)
  - `opentelemetry-jaeger` (Jaeger exporter)
  - `tracing-opentelemetry` (bridge)

- **Infrastructure:**
  - Prometheus server
  - Grafana server
  - Jaeger all-in-one
  - AlertManager

---

## Performance Impact

**Metrics Collection:**
- CPU overhead: <1%
- Memory overhead: ~10MB
- Scrape frequency: 15 seconds

**Tracing:**
- CPU overhead: <2% (with 10% sampling)
- Memory overhead: ~20MB (trace buffers)
- Network: ~1KB per trace

**Total Overhead:**
- CPU: <3%
- Memory: <30MB
- Network: <100KB/s

---

## Week 5 Summary

Week 5 establishes comprehensive observability infrastructure:
- 12+ Prometheus metrics for real-time monitoring
- 4 Grafana dashboards for operational visibility
- OpenTelemetry distributed tracing for debugging
- 10+ alert rules with runbooks for incident response

This enables production operation with confidence, proactive issue detection, and rapid troubleshooting.

---

**End of Comprehensive Megathink**
