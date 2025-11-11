# Phase 10 Week 5: Observability (Prometheus/Grafana/OpenTelemetry)

**Status:** Draft
**Author:** AkiDB Team
**Created:** 2025-11-09
**Phase:** Phase 10 (S3/MinIO Tiered Storage) - Week 5
**Part:** Part B - Production Hardening

---

## Executive Summary

Week 5 establishes comprehensive observability infrastructure for AkiDB 2.0, enabling production monitoring, alerting, and distributed tracing. This is essential for operating AkiDB reliably in production environments.

**Business Value:**
- **Proactive Monitoring**: Detect issues before users report them
- **Faster Incident Response**: Mean time to detection (MTTD) < 5 minutes
- **Performance Insights**: Identify bottlenecks and optimization opportunities
- **Operational Confidence**: SREs have visibility into system health

**Key Deliverables:**
1. Prometheus metrics exporter (12+ metrics)
2. Grafana dashboards (4 dashboards: Overview, Performance, Storage, Errors)
3. OpenTelemetry distributed tracing with Jaeger
4. Alert rules (10+ alerts) with operational runbook

**Dependencies:** Weeks 1-4 (existing infrastructure)

**Timeline:** 5 days

---

## Goals and Non-Goals

### Goals

1. **Metrics Collection**
   - Export 12+ Prometheus metrics
   - Instrument all critical paths (API, Service, Index, Storage)
   - Expose `/metrics` endpoint for scraping

2. **Visualization**
   - 4 Grafana dashboards for different personas (SRE, Engineer, DevOps)
   - Real-time metrics display (<15s latency)
   - Dashboard JSON versioned in repository

3. **Distributed Tracing**
   - OpenTelemetry integration
   - Trace export to Jaeger
   - Instrument HTTP/gRPC requests and internal operations

4. **Alerting**
   - 10+ alert rules covering SLO violations
   - Prometheus AlertManager configuration
   - Runbook for each alert with remediation steps

### Non-Goals

1. **Out of Scope for Week 5**
   - Log aggregation (Loki/ELK) - Future enhancement
   - Custom alerting UI - Use AlertManager
   - Multi-region monitoring - Phase 9+
   - Cost monitoring - Future enhancement

2. **Not Changing**
   - Application architecture (stable from Week 1-4)
   - Existing logging infrastructure (keep `tracing` crate)

---

## User Stories

### SRE (Primary Persona)

**Story 1: System Health at a Glance**
> As an SRE, I want a **System Overview dashboard** showing request rate, error rate, latency, and resource usage so that I can quickly assess system health.

**Acceptance Criteria:**
- Dashboard shows 8 key metrics (request rate, error rate, P95 latency, memory, tier distribution, S3 ops, collection count, uptime)
- Updates in real-time (<15s refresh)
- Accessible at http://grafana:3000/d/akidb-overview

**Story 2: Proactive Alerting**
> As an SRE, I want to receive **alerts when SLOs are violated** (error rate >1%, P95 latency >25ms) so that I can respond before users are impacted.

**Acceptance Criteria:**
- Alerts fire within 5 minutes of threshold breach
- Alerts include runbook link
- Alerts route to appropriate channel (Slack/PagerDuty)

### Software Engineer (Secondary Persona)

**Story 3: Performance Analysis**
> As an engineer, I want a **Performance Dashboard** showing latency by tier, throughput, and operation timings so that I can identify optimization opportunities.

**Acceptance Criteria:**
- Dashboard shows P50/P95/P99 latency histograms
- Search latency broken down by tier (hot/warm/cold)
- Insert and S3 operation timings visible

**Story 4: Distributed Request Tracing**
> As an engineer, I want to see **distributed traces** for slow requests so that I can identify which layer is the bottleneck.

**Acceptance Criteria:**
- Traces viewable in Jaeger UI
- Span hierarchy shows REST → Service → Index → S3
- Traces include custom attributes (collection_id, tier, operation)

### DevOps Engineer (Secondary Persona)

**Story 5: Storage Monitoring**
> As a DevOps engineer, I want a **Storage Dashboard** showing tier distribution, S3 upload/download rates, and snapshot creation so that I can manage storage capacity.

**Acceptance Criteria:**
- Dashboard shows tier distribution over time
- S3 operation success rate and latency visible
- Cold tier size tracked against quota

---

## Technical Specification

### 1. Prometheus Metrics

**Metric Categories:**
1. **Request Metrics** (4 metrics)
2. **Vector Operations** (3 metrics)
3. **Storage Metrics** (3 metrics)
4. **System Metrics** (2 metrics)

**Total:** 12 metrics

#### Request Metrics

**1.1 `akidb_http_requests_total`**
- Type: Counter
- Description: Total HTTP requests by method, path, and status code
- Labels: `method`, `path`, `status_code`

**1.2 `akidb_http_request_duration_seconds`**
- Type: Histogram
- Description: HTTP request latency distribution
- Labels: `method`, `path`
- Buckets: [0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]

**1.3 `akidb_grpc_requests_total`**
- Type: Counter
- Description: Total gRPC requests by method and status
- Labels: `method`, `status`

**1.4 `akidb_grpc_request_duration_seconds`**
- Type: Histogram
- Description: gRPC request latency distribution
- Labels: `method`

#### Vector Operation Metrics

**2.1 `akidb_vector_search_duration_seconds`**
- Type: Histogram
- Description: Vector search latency by index type and tier
- Labels: `index_type`, `tier`
- Buckets: [0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]

**2.2 `akidb_vector_insert_duration_seconds`**
- Type: Histogram
- Description: Vector insert latency by index type
- Labels: `index_type`

**2.3 `akidb_collection_size_bytes`**
- Type: Gauge
- Description: Memory footprint of each collection
- Labels: `collection_id`, `tier`

#### Storage Metrics

**3.1 `akidb_tier_distribution`**
- Type: Gauge
- Description: Number of collections in each tier
- Labels: `tier` (hot, warm, cold)

**3.2 `akidb_s3_operations_total`**
- Type: Counter
- Description: Total S3 operations by type and status
- Labels: `operation` (put, get, delete, list), `status` (success, error)

**3.3 `akidb_s3_operation_duration_seconds`**
- Type: Histogram
- Description: S3 operation latency
- Labels: `operation`
- Buckets: [0.01, 0.05, 0.1, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0]

#### System Metrics

**4.1 `akidb_memory_usage_bytes`**
- Type: Gauge
- Description: Total process memory usage

**4.2 `akidb_background_worker_runs_total`**
- Type: Counter
- Description: Background worker cycle count
- Labels: `worker_type` (tiering, dlq_cleanup)

### 2. Grafana Dashboards

#### Dashboard 1: System Overview

**Purpose:** High-level system health for SREs

**Panels (8):**
1. Request Rate (time-series, by method)
2. Error Rate (time-series, 5xx only)
3. P95 Latency (time-series)
4. Memory Usage (time-series with threshold)
5. Tier Distribution (pie chart)
6. S3 Operation Rate (time-series, by operation)
7. Collection Count (single stat)
8. Uptime (single stat, duration format)

**Access:** http://grafana:3000/d/akidb-overview

#### Dashboard 2: Performance Dashboard

**Purpose:** Detailed performance analysis for engineers

**Panels (10):**
1. Search Latency by Tier (heatmap)
2. Search P50/P95/P99 (multi-line chart)
3. Insert Latency (histogram)
4. Request Throughput (area chart)
5. Hot Tier Search Performance (line chart, target <5ms)
6. Warm Tier Search Performance (line chart, target <25ms)
7. Cold Tier Search Performance (line chart, target <10s)
8. Batch Upload Throughput (line chart, target >500 ops/sec)
9. Collection Size Distribution (bar chart, top 20)
10. Background Worker Frequency (line chart by worker_type)

**Access:** http://grafana:3000/d/akidb-performance

#### Dashboard 3: Storage Dashboard

**Purpose:** Monitor tiered storage and S3 operations

**Panels (8):**
1. Tier Distribution Over Time (stacked area)
2. Hot → Warm Demotion Rate (line chart)
3. Warm → Hot Promotion Rate (line chart)
4. S3 Upload Success Rate (line chart with 100% ref)
5. S3 Upload P95 Latency (line chart)
6. S3 Download P95 Latency (line chart)
7. Cold Tier Size (gauge with max capacity)
8. Snapshot Creation Rate (line chart)

**Access:** http://grafana:3000/d/akidb-storage

#### Dashboard 4: Error Dashboard

**Purpose:** Monitor errors and failures for incident response

**Panels (8):**
1. Error Rate by Endpoint (line chart, by path)
2. DLQ Size (line chart, alert at >100)
3. S3 Error Rate (line chart, alert at >1%)
4. Circuit Breaker Status (single stat, traffic light)
5. Failed Demotion Count (single stat)
6. Failed Promotion Count (single stat)
7. Recent Error Logs (table, Loki integration)
8. Alert Status (table, firing alerts)

**Access:** http://grafana:3000/d/akidb-errors

### 3. OpenTelemetry Distributed Tracing

**Architecture:**
```
AkiDB (REST/gRPC) → OpenTelemetry SDK → Jaeger Agent → Jaeger Collector → Jaeger UI
```

**Instrumentation Levels:**
1. **Auto-instrumentation:**
   - HTTP requests (axum middleware)
   - gRPC requests (tonic interceptor)

2. **Manual instrumentation:**
   - CollectionService methods
   - IndexManager operations
   - ObjectStore (S3) operations
   - Background workers

**Span Hierarchy Example:**
```
Span: POST /search (50ms)
  └─ Span: CollectionService::search (45ms)
      ├─ Span: TieringManager::record_access (1ms)
      ├─ Span: IndexManager::search (40ms)
      │   └─ Span: HNSW::search (35ms)
      └─ Span: ObjectStore::get (4ms)
          └─ Span: S3::GetObject (3ms)
```

**Span Attributes:**
- Standard: `http.method`, `http.url`, `http.status_code`
- Custom: `akidb.collection_id`, `akidb.tier`, `akidb.index_type`, `akidb.vector_dimension`

**Sampling Strategy:**
- 10% of normal requests
- 100% of errors (status_code >= 500)
- 100% of slow requests (latency > 1s)

**Jaeger Access:** http://jaeger:16686

### 4. Alert Rules

**Total Alerts:** 10

#### Critical Alerts (3)

**Alert 1: HighErrorRate**
- Condition: HTTP 5xx error rate > 1% for 5 minutes
- Severity: Critical
- Action: Page on-call
- Runbook: https://docs.akidb.com/runbook/high-error-rate

**Alert 2: S3ErrorRateHigh**
- Condition: S3 error rate > 1% for 5 minutes
- Severity: Critical
- Runbook: Check S3 service status, circuit breaker

**Alert 3: CircuitBreakerOpen**
- Condition: Circuit breaker open for S3
- Severity: Critical
- Runbook: Wait for S3 recovery, check false positives

#### Warning Alerts (7)

**Alert 4: HighSearchLatency**
- Condition: P95 search latency > 25ms for 10 minutes
- Severity: Warning
- Runbook: Check tier-specific latency, identify bottleneck

**Alert 5: MemoryPressure**
- Condition: Memory usage > 80% of quota for 5 minutes
- Severity: Warning
- Runbook: Force demote collections, check for leaks

**Alert 6: DLQSizeGrowing**
- Condition: DLQ size > 100 for 10 minutes
- Severity: Warning
- Runbook: Check DLQ entries, identify failure pattern

**Alert 7: BackgroundWorkerNotRunning**
- Condition: No background worker runs in 10 minutes
- Severity: Critical
- Runbook: Check logs for crash, restart service

**Alert 8: HotTierFull**
- Condition: Hot tier > 90% of capacity
- Severity: Warning
- Runbook: Force demote LRU collections

**Alert 9: ColdTierGrowthRateHigh**
- Condition: Cold tier growing > 10GB/hour
- Severity: Warning
- Runbook: Check for unexpected data growth

**Alert 10: SearchThroughputDrop**
- Condition: Search throughput drops > 50% in 5 minutes
- Severity: Critical
- Runbook: Check request rate, error rate, latency

---

## Implementation Details

### Prometheus Integration

**File:** `crates/akidb-rest/src/metrics.rs`

```rust
use lazy_static::lazy_static;
use prometheus::{
    register_counter_vec, register_gauge_vec, register_histogram_vec,
    CounterVec, GaugeVec, HistogramVec, Encoder, TextEncoder
};

lazy_static! {
    pub static ref HTTP_REQUESTS: CounterVec = register_counter_vec!(
        "akidb_http_requests_total",
        "Total HTTP requests",
        &["method", "path", "status_code"]
    ).unwrap();

    pub static ref HTTP_DURATION: HistogramVec = register_histogram_vec!(
        "akidb_http_request_duration_seconds",
        "HTTP request latency",
        &["method", "path"],
        vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
    ).unwrap();

    // ... other metrics ...
}

pub async fn metrics_handler() -> String {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = vec![];
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}
```

**Middleware:**
```rust
pub async fn metrics_middleware<B>(
    req: Request<B>,
    next: Next<B>,
) -> Response {
    let method = req.method().to_string();
    let path = req.uri().path().to_string();

    let timer = HTTP_DURATION
        .with_label_values(&[&method, &path])
        .start_timer();

    let response = next.run(req).await;

    let status = response.status().as_u16().to_string();
    HTTP_REQUESTS
        .with_label_values(&[&method, &path, &status])
        .inc();

    timer.observe_duration();

    response
}
```

### OpenTelemetry Integration

**File:** `crates/akidb-core/src/tracing.rs`

```rust
use opentelemetry::global;
use opentelemetry_jaeger::Propagator;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_tracing(service_name: &str) -> anyhow::Result<()> {
    global::set_text_map_propagator(Propagator::new());

    let tracer = opentelemetry_jaeger::new_agent_pipeline()
        .with_service_name(service_name)
        .with_endpoint("127.0.0.1:6831")
        .install_simple()?;

    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    tracing_subscriber::registry()
        .with(telemetry)
        .with(tracing_subscriber::fmt::layer())
        .init();

    Ok(())
}
```

### Docker Compose Setup

**File:** `docker-compose.yaml` (add observability services)

```yaml
services:
  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./observability/prometheus.yml:/etc/prometheus/prometheus.yml
      - ./observability/alerts.yml:/etc/prometheus/alerts.yml

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
    volumes:
      - ./observability/grafana/dashboards:/etc/grafana/provisioning/dashboards

  jaeger:
    image: jaegertracing/all-in-one:latest
    ports:
      - "16686:16686"  # UI
      - "6831:6831/udp"  # Agent

  alertmanager:
    image: prom/alertmanager:latest
    ports:
      - "9093:9093"
    volumes:
      - ./observability/alertmanager.yml:/etc/alertmanager/alertmanager.yml
```

---

## Test Strategy

### Metrics Tests (3 tests)

**Test 1: Metrics Endpoint Returns Valid Format**
```rust
#[tokio::test]
async fn test_metrics_endpoint() {
    let response = client.get("/metrics").send().await.unwrap();
    assert_eq!(response.status(), 200);

    let body = response.text().await.unwrap();
    assert!(body.contains("akidb_http_requests_total"));
}
```

**Test 2: Request Counter Increments**
**Test 3: Latency Histogram Records Values**

### Dashboard Tests (1 test)

**Test 4: Dashboard Queries Return Data**
- Verify PromQL queries in Prometheus UI
- Check all panels render without errors

### Tracing Tests (3 tests)

**Test 5: Traces Exported to Jaeger**
**Test 6: Span Hierarchy Correct**
**Test 7: Custom Attributes Present**

### Alert Tests (1 test)

**Test 8: Alerts Fire When Threshold Exceeded**
- Inject errors to trigger alert
- Verify alert appears in AlertManager

**Total Tests:** 8

---

## Dependencies

### Internal Dependencies
- ✅ Week 1-4: Existing infrastructure (servers, services, storage)

### External Dependencies

**Rust Crates:**
- `prometheus` (0.13+)
- `opentelemetry` (0.20+)
- `opentelemetry-jaeger` (0.19+)
- `tracing-opentelemetry` (0.21+)

**Infrastructure:**
- Prometheus server (latest)
- Grafana server (latest)
- Jaeger all-in-one (latest)
- AlertManager (latest)

**Configuration Files:**
- `observability/prometheus.yml`
- `observability/alerts.yml`
- `observability/alertmanager.yml`
- `observability/grafana/dashboards/*.json`

---

## Success Criteria

### Metrics
- ✅ 12+ metrics exported
- ✅ `/metrics` endpoint returns valid Prometheus format
- ✅ Metrics update in real-time (<15s latency)
- ✅ CPU overhead <1%, memory overhead <10MB

### Dashboards
- ✅ 4 Grafana dashboards created (Overview, Performance, Storage, Errors)
- ✅ All panels render without errors
- ✅ Dashboard JSON exported and versioned
- ✅ Dashboards accessible at documented URLs

### Tracing
- ✅ Traces exported to Jaeger
- ✅ Span hierarchy correct (parent-child relationships)
- ✅ Critical paths instrumented (REST → Service → Index → S3)
- ✅ Custom attributes included (collection_id, tier, operation)

### Alerting
- ✅ 10+ alert rules defined
- ✅ Alerts fire when thresholds exceeded (tested)
- ✅ Runbook documented for each alert
- ✅ AlertManager configured for notification routing

### Testing
- ✅ 8 tests passing (3 metrics + 1 dashboard + 3 tracing + 1 alert)
- ✅ 100% test pass rate

---

## Performance Impact

**Metrics Collection:**
- CPU overhead: <1%
- Memory overhead: ~10MB
- Network: Scrape every 15 seconds

**Tracing:**
- CPU overhead: <2% (with 10% sampling)
- Memory overhead: ~20MB
- Network: ~1KB per trace

**Total Overhead:**
- CPU: <3%
- Memory: <30MB
- Network: <100KB/s

**Validation:**
- Run performance benchmarks before/after instrumentation
- Verify <5% latency increase

---

## Risks and Mitigations

### Risk 1: Metrics Overhead

**Impact:** High
**Probability:** Medium

**Mitigation:**
- Use efficient metric types
- Avoid high-cardinality labels
- Benchmark overhead (<1% CPU target)

### Risk 2: Alert Fatigue

**Impact:** High
**Probability:** High

**Mitigation:**
- Tune thresholds to reduce false positives
- Use severity levels appropriately
- Group related alerts
- Review alert firing rate weekly

### Risk 3: Jaeger Storage Growth

**Impact:** Medium
**Probability:** High

**Mitigation:**
- Configure 7-day retention
- Use Cassandra backend for production
- Monitor Jaeger disk usage

### Risk 4: Dashboard Maintenance

**Impact:** Medium
**Probability:** Medium

**Mitigation:**
- Version control dashboard JSON
- Document dashboard purpose and queries
- Regular dashboard review (monthly)

---

## Timeline

**Duration:** 5 days

### Day 1: Prometheus Metrics Infrastructure
- Morning: Create metrics module, define metric types
- Afternoon: Instrument REST/gRPC handlers
- **Deliverable:** `/metrics` endpoint working, 12+ metrics exported

### Day 2: Service Layer Instrumentation
- Morning: Instrument CollectionService, IndexManager
- Afternoon: Instrument S3 operations, background workers
- **Deliverable:** All critical paths instrumented

### Day 3: Grafana Dashboards
- Morning: Create System Overview and Performance dashboards
- Afternoon: Create Storage and Error dashboards
- **Deliverable:** 4 dashboards accessible, JSON versioned

### Day 4: OpenTelemetry Tracing
- Morning: Set up Jaeger, initialize OpenTelemetry
- Afternoon: Instrument critical paths, test trace collection
- **Deliverable:** Traces viewable in Jaeger UI

### Day 5: Alerting and Runbook
- Morning: Configure AlertManager, define 10+ alert rules
- Afternoon: Write runbook, test alert firing
- **Deliverable:** Alerts firing, runbook documented

---

## Approval

**Stakeholders:**
- Engineering Lead: ___________________
- SRE Lead: ___________________
- Product Manager: ___________________
- DevOps Lead: ___________________

**Approval Date:** ___________________

---

## Appendix

### A. Prometheus Configuration

**File:** `observability/prometheus.yml`

```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

alerting:
  alertmanagers:
    - static_configs:
        - targets:
            - alertmanager:9093

rule_files:
  - /etc/prometheus/alerts.yml

scrape_configs:
  - job_name: 'akidb-rest'
    static_configs:
      - targets: ['akidb-rest:8080']

  - job_name: 'akidb-grpc'
    static_configs:
      - targets: ['akidb-grpc:9090']
```

### B. AlertManager Configuration

**File:** `observability/alertmanager.yml`

```yaml
global:
  resolve_timeout: 5m

route:
  receiver: 'slack'
  group_by: ['alertname', 'severity']
  group_wait: 30s
  group_interval: 5m
  repeat_interval: 4h

receivers:
  - name: 'slack'
    slack_configs:
      - api_url: 'YOUR_SLACK_WEBHOOK_URL'
        channel: '#akidb-alerts'
        title: '{{ .GroupLabels.alertname }}'
        text: '{{ range .Alerts }}{{ .Annotations.description }}{{ end }}'
```

### C. Sample Alert Rule

**File:** `observability/alerts.yml`

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
          description: "Error rate is {{ $value | humanizePercentage }}"
          runbook: https://docs.akidb.com/runbook/high-error-rate
```

### D. Quick Commands

**Start Observability Stack:**
```bash
docker-compose up prometheus grafana jaeger alertmanager
```

**Access UIs:**
- Prometheus: http://localhost:9090
- Grafana: http://localhost:3000 (admin/admin)
- Jaeger: http://localhost:16686
- AlertManager: http://localhost:9093

**Test Metrics:**
```bash
curl http://localhost:8080/metrics
```

**Test Tracing:**
```bash
# Make request
curl -X POST http://localhost:8080/search

# View trace in Jaeger
open http://localhost:16686
```

---

**End of Document**
