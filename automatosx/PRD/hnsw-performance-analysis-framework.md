# HNSW Phase 3 M2 Performance Analysis Framework

**Owner**: Bob (Senior Backend Engineer)  
**Date**: 2025-10-25  
**Scope**: HNSW ANN search vs. native (Phase 2) implementation on AKIDB query workloads  
**Purpose**: Provide a repeatable methodology that proves whether Phase 3 M2 performance targets are achieved, isolates regression risks, and drives production tuning recommendations.

---

## 1. Comparison Framework: HNSW vs. Native

### 1.1 Workload Coverage
- **Datasets**: 10K, 100K, 1M vectors (128-dim), future extension to 10M if capacity allows.
- **Query Types**: Cosine vs. L2 distance, filtered vs. unfiltered, top-k ∈ {10, 50, 100}.
- **Concurrency**: Single-thread baseline, 8-way, 32-way (align with service thread pools).
- **Hardware Matrix**: M-series laptop (developer), x86 server (CI or cloud), production-like instance (target deployment).
- **Data Freshness**: Cold start (fresh index build), warm cache (steady-state), mixed update/query (if supported).

### 1.2 Measurement Procedure
1. **Environment Control**: Pin Rust release build, lock CPU frequency (disable turbo if possible), isolate benchmark host.
2. **Warmup**: 2x dataset sweep discarded from metrics to stabilize caches and branch predictors.
3. **Runs**: ≥5 measurement runs per scenario; compute median of medians for stability.
4. **Sampling**: Record per-request latency histogram, throughput, CPU, RSS memory every 1s. Capture GC/alloc stats via `MALLOC_CONF`/jemalloc profiling if available.
5. **Baseline Alignment**: Re-run native implementation under identical conditions (same dataset, same concurrency, same query mix).
6. **Normalization**: Express improvements as Δ% vs. native and absolute values; include speedup multipliers.

### 1.3 Comparative Dimensions
- **Latency Profile**: P50/P95/P99, worst-case (max), standard deviation.
- **Throughput Scaling**: QPS vs. concurrency; saturation point.
- **Resource Efficiency**: CPU utilization (%), context switches, RSS, index footprint on disk, build time.
- **Quality**: Recall@K, precision@K (if labels exist), filter correctness audit.
- **Operational**: Build cost (time + memory), incremental update cost, restart time (index load).
- **Stability**: Error rates, timeouts, retry counts under stress.

### 1.4 Decision Criteria
- **Pass** if HNSW meets or exceeds Phase 3 M2 targets and offers ≥20% throughput uplift with comparable or lower resource usage.
- **Investigate** if latency goals met but throughput improvement <20% or recall < target.
- **Fail** if latency/throughput targets missed or stability issues occur (e.g., error rate >0.1%).

---

## 2. Key Metrics to Monitor

| Category | Metric | Definition | Tooling |
|----------|--------|------------|---------|
| Latency | P50, P95, P99, max | Response time distribution per query | hdrhistogram / custom telemetry |
| Throughput | QPS | Successful queries per second | Criterion benches / k6 / custom harness |
| Recall | Recall@K | Fraction of true neighbors returned | HNSW recall stress test |
| CPU | CPU% (user/system), cycles/query | Processor load, efficiency | `perf`, `dtrace`, `perf stat` |
| Memory | RSS, heap allocations/query | Runtime footprint | `ps`, jemalloc stats, `heaptrack` |
| Index Build | Build time, peak memory | Offline index construction cost | Benchmark harness logs |
| IO | Disk read/write MB/s, page faults | Storage behavior during load | `iostat`, `vm_stat` |
| Concurrency | Queue depth, lock contention | Thread scalability, lock hotspots | `perf lock`, flamegraphs |
| Stability | Error rate, retries, tail anomalies | Reliability at scale | Service logs, metrics pipeline |
| Config Sensitivity | ef_search sweeps vs. recall/latency | Tunable parameter trade-offs | Parameter grid automation |

---

## 3. Phase 3 M2 Target Verification

### 3.1 Targets (For 1M vectors, top-k=50, cosine distance with filter)
- **Latency**: P95 ≤ 150 ms; P99 ≤ 250 ms.
- **Throughput**: ≥ 1.20 × Phase 2 native baseline QPS under matched concurrency.
- **Recall**: Recall@50 ≥ 0.95 (relative to brute-force/ground truth).
- **Resource Budget**: RSS ≤ 1.25 × native; CPU ≤ 1.10 × native at steady-state.

### 3.2 Verification Steps
1. **Baseline Capture**: Run Phase 2 native benchmark on 1M dataset; log metrics in `/automatosx/tmp/phase2-native-1m.json`.
2. **HNSW Run**: Execute Phase 3 HNSW benchmark with identical workload script and concurrency.
3. **Data Integrity Check**: Validate dataset parity (checksum embeddings, ensure identical filters).
4. **Metric Aggregation**: Use reporting script to compute percentiles, throughput, resource deltas with 95% confidence intervals (bootstrap).
5. **Recall Audit**: Cross-check sample queries against brute-force results; flag any recall < target.
6. **Acceptance Gate**:
   - Latency thresholds satisfied.
   - Throughput uplift ≥ 20% with CI lower bound ≥ baseline.
   - Recall ≥ 0.95.
   - Resource usage within budget.
7. **Regression Watch**: Compare against prior HNSW runs; if delta >10% in tail latencies, open investigation ticket.

### 3.3 Evidence Package
- Benchmark logs (raw JSON/CSV).
- Flamegraphs or perf reports for top scenarios.
- Recall validation worksheet.
- Summary dashboard (Grafana snapshot or generated charts).

---

## 4. Performance Analysis Report Template

1. **Executive Overview**
   - Key findings (latency, throughput, recall).
   - Pass/Fail verdict on M2 targets.
   - Top recommendations.
2. **Benchmark Scope**
   - Objectives, scenarios, configurations.
   - Hardware/software details.
3. **Methodology**
   - Data sources, workload generation, warmup protocol.
   - Tooling and instrumentation.
4. **Results**
   - Tables/graphs for each dataset and concurrency level.
   - Latency/throughput comparisons (HNSW vs. native).
   - Resource usage charts.
5. **Phase 3 M2 Verification**
   - Target thresholds.
   - Measured values with confidence intervals.
   - Acceptance decision and rationale.
6. **Analysis & Insights**
   - Bottlenecks identified (CPU, memory, algorithmic limits).
   - Parameter sensitivity (ef_search, ef_construction).
   - Scaling behavior projection (10K → 1M → 10M if data available).
7. **Security & Reliability Checks**
   - Error analysis, failure modes, mitigation.
   - Data integrity validation results.
8. **Recommendations**
   - Tuning adjustments for production.
   - Follow-up experiments or tooling improvements.
9. **Appendix**
   - Raw metric links, scripts, environment configs.
   - Change log of benchmark harness.

---

## 5. Execution Checklist

- [ ] Confirm dataset parity across native vs. HNSW runs.
- [ ] Freeze benchmark binaries (tag commit hash).
- [ ] Automate parameter sweep and result aggregation.
- [ ] Capture flamegraphs at saturation point.
- [ ] Store raw artifacts under `/automatosx/tmp/hnsw-benchmarks/`.
- [ ] Publish report to `/docs/performance/phase3-m2-hnsw.md`.

Performance is measured, security is verified, architecture is proven, mathematics is validated.
