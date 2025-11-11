# AkiDB 2.0 - Comprehensive Load Test Design

**Date**: November 9, 2025
**Author**: Claude (AI Assistant)
**Status**: Design Document - Ready for Implementation

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Test Objectives](#test-objectives)
3. [Performance Targets](#performance-targets)
4. [Load Test Scenarios](#load-test-scenarios)
5. [Test Framework Architecture](#test-framework-architecture)
6. [Metrics & Monitoring](#metrics--monitoring)
7. [Implementation Plan](#implementation-plan)
8. [Success Criteria](#success-criteria)

---

## Executive Summary

This document outlines a comprehensive load testing strategy for AkiDB 2.0, designed to validate performance under production-like conditions and identify bottlenecks before GA release.

**Key Goals**:
- Validate P95 latency <25ms @ 100 QPS
- Ensure system stability under sustained load
- Identify memory leaks and resource exhaustion
- Test tiered storage under realistic workloads
- Validate multi-tenant isolation and performance

**Test Duration**: 4-6 hours total (across all scenarios)
**Recommended Timeline**: Run load tests during final week before GA release

---

## Test Objectives

### Primary Objectives

1. **Performance Validation**
   - Confirm P95 search latency <25ms @ 100 QPS
   - Verify insert throughput >5,000 ops/sec
   - Validate memory footprint ≤100GB for target dataset

2. **Stability Testing**
   - No crashes or panics during extended runs
   - No memory leaks over 10-minute sustained load
   - Graceful degradation under overload

3. **Correctness Under Load**
   - Zero data corruption
   - Consistent search recall >95%
   - All CRUD operations succeed

4. **Tiered Storage Performance**
   - Hot tier: <5ms search latency
   - Warm tier: <25ms search latency (including load time)
   - Cold tier: <2s retrieval from S3

### Secondary Objectives

5. **Multi-Tenancy Isolation**
   - Tenant A heavy load doesn't impact Tenant B
   - Fair resource allocation across tenants
   - No cross-tenant data leaks

6. **Resource Utilization**
   - CPU utilization <80% average
   - Memory growth linear with dataset size
   - S3 bandwidth within expected limits

7. **Failure Resilience**
   - Graceful handling of S3 rate limits
   - Recovery from transient network errors
   - Circuit breaker effectiveness

---

## Performance Targets

### Core Metrics

| Metric | Target | Measurement Method |
|--------|--------|-------------------|
| **Search Latency P50** | <10ms | Direct measurement |
| **Search Latency P95** | <25ms | Direct measurement |
| **Search Latency P99** | <50ms | Direct measurement |
| **Insert Throughput** | >5,000 ops/sec | Direct measurement |
| **Error Rate** | <0.1% | Failed requests / total |
| **Memory Footprint** | ≤100GB @ 100k vectors | Process RSS |
| **CPU Utilization** | <80% average | System metrics |
| **S3 Upload Rate** | >500 ops/sec | Batch uploader metrics |

### Tiered Storage Targets

| Tier | Search Latency | Promotion Time | Demotion Time |
|------|---------------|----------------|---------------|
| **Hot** | P95 <5ms | N/A | <100ms |
| **Warm** | P95 <25ms | <200ms | <500ms |
| **Cold** | P95 <2s | <2s | <1s |

### Scalability Targets

| Dataset Size | P95 Latency | Memory | Status |
|--------------|-------------|--------|--------|
| 1k vectors | <5ms | <100MB | ✅ Validated |
| 10k vectors | <15ms | <1GB | ✅ Validated |
| 100k vectors | <25ms | <10GB | ⏳ To test |
| 1M vectors | <50ms | <100GB | 🎯 Target |

---

## Load Test Scenarios

### Scenario 1: Baseline Performance (30 minutes)

**Purpose**: Establish baseline metrics under normal load

**Configuration**:
- **Duration**: 30 minutes
- **QPS**: 100 (constant)
- **Workload Mix**:
  - 70% search operations
  - 20% insert operations
  - 10% metadata operations (list, get)
- **Dataset**: 10k vectors (512-dim, cosine similarity)
- **Concurrency**: 10 concurrent clients

**Success Criteria**:
- ✅ P95 latency <25ms
- ✅ Error rate <0.1%
- ✅ Memory stable (no growth >10MB/min)
- ✅ CPU <70% average

**Metrics to Collect**:
- Latency distribution (P50, P95, P99)
- Throughput (requests/sec)
- Error rate
- Memory usage (RSS, heap)
- CPU utilization

---

### Scenario 2: Sustained High Load (60 minutes)

**Purpose**: Validate stability under sustained production load

**Configuration**:
- **Duration**: 60 minutes
- **QPS**: 200 (constant, 2x baseline)
- **Workload Mix**: Same as Scenario 1
- **Dataset**: 50k vectors
- **Concurrency**: 20 concurrent clients

**Success Criteria**:
- ✅ P95 latency <50ms (degraded but acceptable)
- ✅ Error rate <0.5%
- ✅ No crashes or panics
- ✅ Memory growth <100MB over duration
- ✅ CPU <85% average

**What We're Testing**:
- Long-running stability
- Memory leak detection
- Resource cleanup
- Connection pooling effectiveness

---

### Scenario 3: Spike Load (15 minutes)

**Purpose**: Test system response to sudden traffic spikes

**Configuration**:
- **Duration**: 15 minutes total
- **Load Pattern**:
  - 0-3 min: 100 QPS (baseline)
  - 3-5 min: Ramp to 500 QPS (spike)
  - 5-10 min: 500 QPS (sustained)
  - 10-12 min: Ramp down to 100 QPS
  - 12-15 min: 100 QPS (recovery)
- **Dataset**: 10k vectors
- **Concurrency**: 50 concurrent clients during spike

**Success Criteria**:
- ✅ System remains responsive during spike
- ✅ P95 latency <100ms during spike
- ✅ Error rate <1% during spike
- ✅ Full recovery to baseline performance after spike
- ✅ No memory leaks post-spike

**What We're Testing**:
- Circuit breaker activation
- Queue backpressure handling
- Graceful degradation
- Recovery after overload

---

### Scenario 4: Tiered Storage Workflow (45 minutes)

**Purpose**: Validate hot/warm/cold tier transitions under load

**Configuration**:
- **Duration**: 45 minutes
- **QPS**: 100 (constant)
- **Tiering Policy**:
  - Hot → Warm: No access for 5 minutes
  - Warm → Cold: No access for 10 minutes
  - Cold → Warm: On first access
  - Warm → Hot: 10 accesses in 1 minute
- **Workload**:
  - Create 100 collections
  - Access collections with Zipf distribution (80/20 rule)
  - Monitor tier transitions

**Success Criteria**:
- ✅ Hot tier: P95 <5ms
- ✅ Warm tier: P95 <25ms
- ✅ Cold tier: First access <2s, subsequent <25ms
- ✅ Automatic promotions/demotions working
- ✅ No data loss during transitions

**What We're Testing**:
- Tier transition logic
- S3 upload/download performance
- Access pattern tracking
- LRU eviction correctness

---

### Scenario 5: Multi-Tenant Load (30 minutes)

**Purpose**: Validate tenant isolation under concurrent load

**Configuration**:
- **Duration**: 30 minutes
- **Tenants**: 10 tenants
- **Per-Tenant Load**:
  - Tenant 1-5: 10 QPS each (normal)
  - Tenant 6: 100 QPS (heavy user)
  - Tenant 7-10: 5 QPS each (light users)
- **Total QPS**: 175
- **Dataset**: 5k vectors per tenant (50k total)

**Success Criteria**:
- ✅ Tenant 6 high load doesn't impact others
- ✅ All tenants achieve target latency
- ✅ Fair resource allocation (CPU, memory)
- ✅ No cross-tenant data leaks (audit logs verified)

**What We're Testing**:
- Tenant isolation
- Resource fairness
- RBAC enforcement
- Audit log completeness

---

### Scenario 6: Large Dataset (60 minutes)

**Purpose**: Validate performance with 100k+ vectors

**Configuration**:
- **Duration**: 60 minutes
- **QPS**: 100 (constant)
- **Dataset**: 100k vectors (512-dim)
- **Workload Mix**:
  - 80% search operations
  - 15% insert operations
  - 5% metadata operations
- **Index Type**: InstantDistanceIndex (HNSW)

**Success Criteria**:
- ✅ P95 latency <25ms
- ✅ Recall >95%
- ✅ Memory <15GB
- ✅ Stable memory usage (no growth)

**What We're Testing**:
- HNSW index performance at scale
- Memory management for large datasets
- Search accuracy under load
- Background compaction effectiveness

---

### Scenario 7: Failure Injection (20 minutes)

**Purpose**: Test resilience to infrastructure failures

**Configuration**:
- **Duration**: 20 minutes
- **QPS**: 100 (constant)
- **Failure Scenarios**:
  - S3 rate limiting (simulated)
  - Transient network errors (10% failure rate)
  - Slow S3 responses (500ms delays)
  - Circuit breaker tripping
- **Dataset**: 10k vectors

**Success Criteria**:
- ✅ Circuit breaker activates correctly
- ✅ DLQ captures failed uploads
- ✅ Retry logic works (exponential backoff)
- ✅ System recovers when failures stop
- ✅ No data loss (all operations eventually succeed)

**What We're Testing**:
- Circuit breaker effectiveness
- Dead letter queue functionality
- Retry policy correctness
- Error handling robustness

---

### Scenario 8: Mixed Workload Chaos (30 minutes)

**Purpose**: Simulate realistic production chaos

**Configuration**:
- **Duration**: 30 minutes
- **QPS**: Variable (50-300, random spikes)
- **Workload**:
  - Random mix of all operations
  - Unpredictable access patterns
  - Concurrent tenant operations
  - Random tier transitions
- **Dataset**: 50k vectors across 20 collections

**Success Criteria**:
- ✅ System remains stable
- ✅ P95 latency <50ms (degraded acceptable)
- ✅ Error rate <2%
- ✅ No data corruption
- ✅ Audit logs complete

**What We're Testing**:
- Real-world unpredictability
- Concurrent operation safety
- Overall system robustness
- Edge case handling

---

## Test Framework Architecture

### Component Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Load Test Orchestrator                    │
│  - Scenario selection                                        │
│  - Load generation                                           │
│  - Metrics collection                                        │
└───────────────┬─────────────────────────────────────────────┘
                │
    ┌───────────┴───────────┬───────────────┬─────────────┐
    │                       │               │             │
┌───▼────┐            ┌────▼─────┐    ┌───▼────┐   ┌───▼────┐
│ Client │            │  Client  │    │ Client │   │ Client │
│  Pool  │            │   Pool   │    │  Pool  │   │  Pool  │
│  (T1)  │            │   (T2)   │    │  (T3)  │   │  (Tn)  │
└───┬────┘            └────┬─────┘    └───┬────┘   └───┬────┘
    │                      │               │             │
    └──────────────────────┴───────────────┴─────────────┘
                           │
                ┌──────────▼──────────┐
                │   AkiDB REST API    │
                │   (localhost:8080)  │
                └──────────┬──────────┘
                           │
        ┌──────────────────┼──────────────────┐
        │                  │                  │
  ┌─────▼──────┐   ┌──────▼──────┐   ┌──────▼──────┐
  │ Collection │   │   Storage   │   │  Metadata   │
  │  Service   │   │   Backend   │   │   (SQLite)  │
  └────────────┘   └──────┬──────┘   └─────────────┘
                          │
                   ┌──────▼──────┐
                   │  S3/MinIO   │
                   │ (Mock/Real) │
                   └─────────────┘
```

### Load Generator Design

**File**: `crates/akidb-storage/tests/comprehensive_load_test.rs`

```rust
/// Main load test orchestrator
pub struct LoadTestOrchestrator {
    scenarios: Vec<LoadTestScenario>,
    metrics_collector: MetricsCollector,
    result_writer: ResultWriter,
}

/// Individual load test scenario
pub struct LoadTestScenario {
    name: String,
    duration: Duration,
    load_profile: LoadProfile,
    workload_mix: WorkloadMix,
    success_criteria: SuccessCriteria,
}

/// Load profile (constant, ramp, spike, random)
pub enum LoadProfile {
    Constant { qps: usize },
    Ramp { from_qps: usize, to_qps: usize, duration: Duration },
    Spike { baseline_qps: usize, spike_qps: usize, spike_duration: Duration },
    Random { min_qps: usize, max_qps: usize },
}

/// Workload mix percentages
pub struct WorkloadMix {
    search_pct: f32,
    insert_pct: f32,
    update_pct: f32,
    delete_pct: f32,
    metadata_pct: f32,
}
```

### Metrics Collection

**Real-time Metrics**:
- Request latency (histogram)
- Throughput (requests/sec)
- Error rate
- Active connections
- Queue depth

**System Metrics**:
- CPU utilization (via sysinfo)
- Memory usage (RSS, heap)
- Disk I/O
- Network bandwidth

**Application Metrics**:
- Tier state distribution (hot/warm/cold)
- S3 upload queue depth
- Circuit breaker state
- Cache hit rate
- Search recall accuracy

### Results Reporting

**Output Format**:
- JSON for machine processing
- Markdown for human reading
- Grafana-compatible time series

**Report Sections**:
1. Scenario summary
2. Latency percentiles (P50, P90, P95, P99)
3. Error analysis
4. Resource utilization graphs
5. Pass/fail assessment
6. Recommendations

---

## Metrics & Monitoring

### Key Performance Indicators (KPIs)

| KPI | Measurement | Target | Alert Threshold |
|-----|-------------|--------|-----------------|
| **Availability** | Successful requests / total | >99.9% | <99% |
| **Latency P95** | 95th percentile response time | <25ms | >50ms |
| **Throughput** | Requests/second | >100 QPS | <50 QPS |
| **Error Rate** | Failed requests / total | <0.1% | >1% |
| **Memory Growth** | MB/minute | <10 MB/min | >50 MB/min |
| **CPU Utilization** | Average over window | <80% | >90% |

### Monitoring Stack

**Option 1: Built-in (Recommended for CI)**
- Metrics: In-process collection (no external dependencies)
- Output: JSON + Markdown reports
- Visualization: Terminal-based charts (ratatui)

**Option 2: Full Observability (Recommended for Production)**
- Metrics: Prometheus exporter
- Tracing: OpenTelemetry → Jaeger
- Visualization: Grafana dashboards
- Logs: Structured JSON → Loki

### Alerting Rules

```yaml
# Example Prometheus alerts
groups:
  - name: akidb_load_test
    interval: 10s
    rules:
      - alert: HighLatency
        expr: histogram_quantile(0.95, akidb_search_duration_seconds) > 0.025
        for: 1m
        annotations:
          summary: "P95 latency exceeded 25ms"

      - alert: HighErrorRate
        expr: rate(akidb_errors_total[1m]) / rate(akidb_requests_total[1m]) > 0.01
        for: 1m
        annotations:
          summary: "Error rate exceeded 1%"

      - alert: MemoryLeak
        expr: deriv(process_resident_memory_bytes[5m]) > 10485760  # 10MB/min
        for: 5m
        annotations:
          summary: "Memory growth detected"
```

---

## Implementation Plan

### Phase 1: Framework Setup (Week 1, Days 1-2)

**Tasks**:
1. Create `comprehensive_load_test.rs` file structure
2. Implement `LoadTestOrchestrator` core
3. Add `LoadProfile` variants (constant, ramp, spike, random)
4. Implement basic metrics collection
5. Create result writer (JSON + Markdown)

**Deliverables**:
- [ ] Load test framework compiles
- [ ] Can run Scenario 1 (baseline)
- [ ] Metrics collected correctly
- [ ] Report generated

**Files to Create**:
- `crates/akidb-storage/tests/comprehensive_load_test.rs`
- `crates/akidb-storage/tests/load_test_framework/mod.rs`
- `crates/akidb-storage/tests/load_test_framework/orchestrator.rs`
- `crates/akidb-storage/tests/load_test_framework/metrics.rs`
- `crates/akidb-storage/tests/load_test_framework/reporter.rs`

### Phase 2: Scenario Implementation (Week 1, Days 3-4)

**Tasks**:
1. Implement Scenario 1: Baseline Performance
2. Implement Scenario 2: Sustained High Load
3. Implement Scenario 3: Spike Load
4. Implement Scenario 4: Tiered Storage Workflow
5. Add pass/fail assessment logic

**Deliverables**:
- [ ] All 4 scenarios runnable
- [ ] Success criteria validated automatically
- [ ] Detailed reports for each scenario

### Phase 3: Advanced Scenarios (Week 1, Day 5)

**Tasks**:
1. Implement Scenario 5: Multi-Tenant Load
2. Implement Scenario 6: Large Dataset
3. Implement Scenario 7: Failure Injection
4. Implement Scenario 8: Mixed Workload Chaos
5. Add failure injection framework

**Deliverables**:
- [ ] All 8 scenarios complete
- [ ] Failure injection working
- [ ] Chaos testing functional

### Phase 4: Integration & Validation (Week 2, Days 1-2)

**Tasks**:
1. Run all scenarios against live AkiDB instance
2. Validate metrics accuracy
3. Tune success criteria based on results
4. Fix any bugs discovered
5. Document findings

**Deliverables**:
- [ ] All scenarios pass
- [ ] Performance targets validated
- [ ] Bottlenecks identified
- [ ] Optimization recommendations documented

### Phase 5: CI Integration (Week 2, Day 3)

**Tasks**:
1. Add load tests to CI pipeline
2. Create smoke test suite (5-minute run)
3. Add nightly full test suite (4-hour run)
4. Set up result archiving
5. Configure failure notifications

**Deliverables**:
- [ ] CI pipeline runs load tests
- [ ] Results archived for trending
- [ ] Team notified of failures
- [ ] Performance regression detection

---

## Success Criteria

### Must-Have (GA Release Blockers)

- ✅ **Scenario 1 passes** - Baseline performance validated
- ✅ **Scenario 2 passes** - System stable under sustained load
- ✅ **Scenario 6 passes** - 100k vector performance acceptable
- ✅ **Zero data corruption** - All tests verify data integrity
- ✅ **No memory leaks** - Memory stable over 60-minute runs

### Should-Have (High Priority)

- ✅ **Scenario 3 passes** - Spike load handled gracefully
- ✅ **Scenario 4 passes** - Tiered storage working correctly
- ✅ **Scenario 7 passes** - Failure resilience validated
- ✅ **P95 latency <25ms** @ 100 QPS sustained

### Nice-to-Have (Post-GA)

- ✅ **Scenario 5 passes** - Multi-tenant isolation verified
- ✅ **Scenario 8 passes** - Chaos testing successful
- ✅ **1M vector performance** - Scalability to 1M validated
- ✅ **Grafana dashboards** - Real-time monitoring setup

---

## Test Execution Guide

### Prerequisites

1. **Hardware Requirements**:
   - 16GB RAM minimum (32GB recommended)
   - 8 CPU cores minimum
   - 50GB free disk space
   - S3/MinIO instance (can use mock for most tests)

2. **Software Requirements**:
   - Rust 1.75+ (stable)
   - Docker (for MinIO)
   - Python 3.13 (for MLX embedding tests)

3. **Environment Setup**:
   ```bash
   # Start MinIO (optional, for S3 tests)
   docker-compose up -d minio

   # Start AkiDB REST server
   cargo run -p akidb-rest --release

   # In separate terminal, run load tests
   cargo test --release --test comprehensive_load_test -- --nocapture
   ```

### Running Individual Scenarios

```bash
# Scenario 1: Baseline (30 min)
cargo test --release scenario_baseline -- --nocapture

# Scenario 2: Sustained Load (60 min)
cargo test --release scenario_sustained_load -- --nocapture

# Scenario 3: Spike Load (15 min)
cargo test --release scenario_spike_load -- --nocapture

# Run all scenarios (4-6 hours)
cargo test --release --test comprehensive_load_test -- --nocapture
```

### Interpreting Results

**Example Output**:

```
=== Load Test Report ===
Scenario: Baseline Performance
Duration: 30 minutes
Total Requests: 180,000

Latency (ms):
  P50: 8.2
  P90: 15.1
  P95: 21.3 ✅ (target: <25ms)
  P99: 45.2

Throughput:
  Average: 100.1 QPS ✅
  Peak: 125.3 QPS

Error Rate: 0.02% ✅ (target: <0.1%)

Memory:
  Start: 512 MB
  End: 518 MB
  Growth: 6 MB (0.2 MB/min) ✅

CPU:
  Average: 65% ✅ (target: <80%)
  Peak: 82%

Status: ✅ PASSED
```

---

## Risk Mitigation

### Known Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| **Test environment differs from prod** | High | Medium | Use production-like hardware; test on ARM |
| **Load test consumes too many resources** | Medium | High | Run during off-hours; use isolated environment |
| **False positives from CI flakiness** | Medium | Medium | Retry failed tests; tune success criteria |
| **Long test duration delays development** | Low | High | Implement smoke tests (5 min) for CI |

### Contingency Plans

**If Load Tests Fail**:
1. Analyze failure mode (latency, errors, crashes)
2. Identify bottleneck (CPU, memory, I/O, lock contention)
3. Profile with perf/flamegraph
4. Fix root cause
5. Re-run affected scenario
6. Document findings in issue tracker

**If Performance Targets Missed**:
1. Determine gap (e.g., P95 30ms vs target 25ms)
2. Assess impact (blocker vs acceptable degradation)
3. Optimize hot paths (profiling-guided)
4. Re-test with optimizations
5. Update targets if architectural limitations found

---

## Appendix A: Sample Load Test Code

### Basic Scenario Structure

```rust
#[tokio::test]
#[ignore] // Run explicitly: cargo test --ignored scenario_baseline
async fn scenario_baseline() {
    let config = ScenarioConfig {
        name: "Baseline Performance".to_string(),
        duration: Duration::from_secs(1800), // 30 min
        load_profile: LoadProfile::Constant { qps: 100 },
        workload_mix: WorkloadMix {
            search_pct: 0.7,
            insert_pct: 0.2,
            metadata_pct: 0.1,
            ..Default::default()
        },
        dataset_size: 10_000,
        dimension: 512,
    };

    let success_criteria = SuccessCriteria {
        max_p95_latency_ms: 25.0,
        max_error_rate: 0.001,
        max_memory_growth_mb_per_min: 10.0,
        max_cpu_utilization: 0.80,
    };

    let orchestrator = LoadTestOrchestrator::new(config);
    let result = orchestrator.run().await.expect("Load test failed");

    // Validate success criteria
    assert!(result.passes(&success_criteria),
        "Scenario failed: {}", result.failure_summary());

    // Write detailed report
    result.write_report("target/load_test_reports/baseline.md")
        .expect("Failed to write report");

    println!("\n✅ Baseline scenario PASSED");
    println!("   P95 latency: {:.2}ms", result.p95_latency_ms);
    println!("   Error rate: {:.4}%", result.error_rate * 100.0);
    println!("   Memory stable: {} MB/min", result.memory_growth_mb_per_min);
}
```

---

## Appendix B: Metrics Schema

### JSON Output Format

```json
{
  "scenario": "Baseline Performance",
  "start_time": "2025-11-09T00:00:00Z",
  "end_time": "2025-11-09T00:30:00Z",
  "duration_seconds": 1800,
  "total_requests": 180000,
  "successful_requests": 179964,
  "failed_requests": 36,
  "error_rate": 0.0002,
  "latency_ms": {
    "p50": 8.2,
    "p90": 15.1,
    "p95": 21.3,
    "p99": 45.2,
    "max": 127.5
  },
  "throughput": {
    "average_qps": 100.1,
    "peak_qps": 125.3
  },
  "memory_mb": {
    "start": 512,
    "end": 518,
    "peak": 524,
    "growth_per_min": 0.2
  },
  "cpu_percent": {
    "average": 65,
    "peak": 82
  },
  "success_criteria": {
    "passed": true,
    "failures": []
  }
}
```

---

## Appendix C: Next Steps

### Immediate Actions (This Week)

1. **Review this design** - Get team feedback
2. **Create framework skeleton** - Setup basic structure
3. **Implement Scenario 1** - Validate approach
4. **Run first load test** - Get baseline metrics

### Short-term (Next 2 Weeks)

1. **Complete all scenarios** - Implement Scenarios 2-8
2. **Run full test suite** - Validate against real AkiDB instance
3. **Document bottlenecks** - Identify optimization opportunities
4. **Integrate with CI** - Automate load testing

### Long-term (Post-GA)

1. **Production load testing** - Run against staging environment
2. **Chaos engineering** - Advanced failure injection
3. **Multi-region testing** - Test distributed setup
4. **1M+ vector testing** - Push scalability limits

---

**Document Status**: ✅ Ready for Implementation
**Next Milestone**: Phase 1 Complete (Framework Setup)
**Estimated Effort**: 2 weeks (1 engineer)
**Dependencies**: AkiDB REST API, SQLite, S3/MinIO (optional)

