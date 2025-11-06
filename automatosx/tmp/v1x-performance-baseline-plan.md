# v1.x Performance Baseline Test Plan

**Owner:** Performance Engineering Team
**Timeline:** Week 0 (Nov 11-15, 2025)
**Purpose:** Establish performance baselines for AkiDB v1.x to enable regression detection in v2.0

---

## Executive Summary

Before implementing AkiDB 2.0 enhancements (SQLite metadata, Cedar RBAC, RAM-first tiering, embeddings), we must capture current v1.x performance characteristics. These baselines will:

1. **Validate v2.0 improvements** (target: 35% latency reduction, 40% memory efficiency)
2. **Detect regressions** (any v2.0 metric worse than v1.x triggers investigation)
3. **Guide optimization** (identify current bottlenecks to prioritize)
4. **Inform go-to-market** (quantify customer value proposition)

---

## Test Environment

### Hardware Configuration

**Primary Test Bed (Mac ARM):**
- Model: MacBook Pro M2 Max (matches target customer hardware)
- CPU: 12-core ARM (8 performance + 4 efficiency)
- Memory: 32GB unified memory
- Storage: 1TB NVMe SSD (APFS)
- OS: macOS 14.6 (Sonoma)

**Secondary Test Bed (x86 Baseline):**
- Model: AWS c6i.4xlarge (for x86 comparison)
- CPU: Intel Xeon 3.5GHz, 16 vCPU
- Memory: 32GB DDR4
- Storage: 500GB gp3 EBS
- OS: Ubuntu 22.04 LTS

### Software Configuration

- **AkiDB Version:** v1.x (latest stable from `/Users/akiralam/code/akidb`)
- **Rust Version:** 1.75.0 (stable)
- **Compiler Flags:** `--release` with LTO enabled
- **S3/MinIO:** Local MinIO instance (Docker) to isolate network latency

---

## Test Scenarios

### 1. Ingest Throughput Baseline

**Objective:** Measure vector ingestion rate (vectors/sec) and resource utilization.

**Workload:**
- Dataset: 1M vectors, 512-dim, random float32
- Batch sizes: 100, 500, 1000 vectors/batch
- Formats: CSV, JSON, Parquet (test all parsers)

**Metrics to Capture:**
| Metric | Target Range | Measurement Method |
|--------|--------------|-------------------|
| Throughput (vec/sec) | Establish baseline | Count / elapsed time |
| CPU Utilization (%) | Establish baseline | `top` / `htop` sampling |
| Memory Peak (GB) | Establish baseline | `/usr/bin/time -l` (macOS) |
| Disk Write (MB/sec) | Establish baseline | `iostat 1` |

**Test Commands:**
```bash
# CSV ingest
time akidb ingest --tenant test --collection benchmark \
  --file vectors_1m_512d.csv --batch-size 1000

# JSON ingest
time akidb ingest --tenant test --collection benchmark \
  --file vectors_1m_512d.json --batch-size 1000

# Parquet ingest
time akidb ingest --tenant test --collection benchmark \
  --file vectors_1m_512d.parquet --batch-size 1000
```

**Expected Output:**
- CSV: ~8k vectors/sec
- JSON: ~6k vectors/sec
- Parquet: ~12k vectors/sec

---

### 2. Query Latency Baseline

**Objective:** Measure P50/P95/P99 latency for similarity search queries.

**Workload:**
- Index: 1M vectors, 512-dim, HNSW (M=16, efConstruction=200)
- Query types:
  - Simple similarity search (top-10, top-100)
  - Metadata filter + similarity (10% selectivity)
  - Hybrid search (vector + complex filter)

**Metrics to Capture:**
| Query Type | P50 (ms) | P95 (ms) | P99 (ms) | QPS Limit |
|------------|----------|----------|----------|-----------|
| Simple top-10 | TBD | TBD | TBD | TBD |
| Simple top-100 | TBD | TBD | TBD | TBD |
| With metadata filter | TBD | TBD | TBD | TBD |
| Hybrid search | TBD | TBD | TBD | TBD |

**Test Commands:**
```bash
# Generate 1000 random query vectors
python scripts/gen_queries.py --count 1000 --dim 512 > queries.json

# Benchmark with hyperfine
hyperfine --warmup 10 --runs 1000 \
  --export-json results/v1x-query-latency.json \
  'akidb query --tenant test --collection benchmark --vector-file queries.json --top-k 10'

# Extract percentiles
jq '.results[0].times | [min, (length/2|floor) as $mid | sort|.[$mid], (length*0.95|floor) as $p95 | sort|.[$p95], max]' results/v1x-query-latency.json
```

**Expected Output:**
- P95 < 35ms (current guess, to be measured)
- P99 < 60ms (current guess, to be measured)

---

### 3. Memory Footprint Baseline

**Objective:** Measure RAM usage per 1M vectors at steady state.

**Workload:**
- Dataset: 1M, 5M, 10M vectors (512-dim)
- HNSW parameters: M=16, efConstruction=200
- Measure after index build completes

**Metrics to Capture:**
| Vector Count | Index Size (GB) | Resident Memory (GB) | Metadata Overhead (MB) |
|--------------|-----------------|----------------------|------------------------|
| 1M | TBD | TBD | TBD |
| 5M | TBD | TBD | TBD |
| 10M | TBD | TBD | TBD |

**Test Commands:**
```bash
# Build index and measure memory
akidb build-index --tenant test --collection benchmark &
AKIDB_PID=$!

# Poll memory every 5 seconds
while kill -0 $AKIDB_PID 2>/dev/null; do
  ps -p $AKIDB_PID -o rss,vsz | tail -1 >> memory_trace.log
  sleep 5
done

# Report peak memory
awk '{print $1/1024}' memory_trace.log | sort -n | tail -1
```

**Expected Output:**
- 1M vectors: ~6GB RAM (v1.x baseline)
- v2.0 target: <5GB RAM (40% improvement via RAM-first tiering)

---

### 4. Crash Recovery Baseline

**Objective:** Measure time to recover from unclean shutdown (simulated crash).

**Workload:**
- Ingest 100k vectors
- Kill process mid-write (`kill -9`)
- Restart and measure WAL replay time

**Metrics to Capture:**
| Scenario | WAL Size (MB) | Recovery Time (sec) | Data Loss (vectors) |
|----------|--------------|---------------------|---------------------|
| Mid-ingest crash | TBD | TBD | TBD |
| Mid-query crash | TBD | TBD | TBD |

**Test Commands:**
```bash
# Start ingest
akidb ingest --tenant test --collection benchmark --file large.csv &
AKIDB_PID=$!

# Wait 5 seconds, then crash
sleep 5 && kill -9 $AKIDB_PID

# Measure recovery time
time akidb start --tenant test
```

**Expected Output:**
- Recovery time < 60 seconds (v1.x baseline)
- v2.0 target: < 30 seconds (SQLite WAL + faster metadata recovery)

---

### 5. Multi-Tenant Isolation Baseline

**Objective:** Validate tenant isolation and measure quota enforcement overhead.

**Workload:**
- Create 10 tenants with 100k vectors each
- Concurrent queries from all tenants
- Measure cross-tenant interference

**Metrics to Capture:**
| Scenario | Tenant A Latency (P95) | Tenant B Latency (P95) | Isolation Score |
|----------|------------------------|------------------------|-----------------|
| Sequential queries | TBD | TBD | TBD |
| Concurrent queries | TBD | TBD | TBD |

**Test Commands:**
```bash
# Create 10 tenants
for i in {1..10}; do
  akidb tenant create --name "tenant-$i" --quota 10GB
  akidb ingest --tenant "tenant-$i" --file vectors_100k.csv
done

# Concurrent load test
parallel --jobs 10 'akidb query --tenant tenant-{} --file queries.json' ::: {1..10}
```

**Expected Output:**
- Isolation score > 95% (minimal cross-tenant interference)
- Quota enforcement working (reject over-quota writes)

---

## Test Execution Plan

### Day 1 (Nov 11) - Setup
- [ ] Provision Mac ARM test bed (MacBook Pro M2 Max)
- [ ] Install AkiDB v1.x from `/Users/akiralam/code/akidb`
- [ ] Generate synthetic datasets (1M, 5M, 10M vectors)
- [ ] Set up MinIO for S3 backend testing
- [ ] Install monitoring tools (Prometheus, Grafana, htop, iostat)

### Day 2 (Nov 12) - Ingest & Memory
- [ ] Run ingest throughput tests (CSV, JSON, Parquet)
- [ ] Measure memory footprint (1M, 5M, 10M vectors)
- [ ] Capture resource utilization traces

### Day 3 (Nov 13) - Query Latency
- [ ] Build HNSW indexes for 1M, 5M, 10M vectors
- [ ] Run query latency benchmarks (top-10, top-100, filtered, hybrid)
- [ ] Measure QPS limits under sustained load

### Day 4 (Nov 14) - Reliability & Multi-Tenancy
- [ ] Run crash recovery tests (mid-ingest, mid-query)
- [ ] Test multi-tenant isolation and quota enforcement
- [ ] Validate WAL replay correctness

### Day 5 (Nov 15) - Analysis & Documentation
- [ ] Analyze all results, compute percentiles
- [ ] Document baselines in `v1x-baseline-2025-11-15.md`
- [ ] Commit results to `akidb-benchmarks/baselines/`
- [ ] Present findings at Go/No-Go meeting

---

## Baseline Report Template

Save results to `akidb-benchmarks/baselines/v1x-baseline-2025-11-15.md`:

```markdown
# AkiDB v1.x Performance Baseline (2025-11-15)

## Environment
- Hardware: MacBook Pro M2 Max, 32GB RAM
- OS: macOS 14.6
- AkiDB Version: v1.x (commit: abc123)

## Ingest Throughput
- CSV: 8,200 vectors/sec
- JSON: 6,100 vectors/sec
- Parquet: 11,800 vectors/sec

## Query Latency (1M vectors, 512-dim)
- Simple top-10: P95=32ms, P99=58ms
- Simple top-100: P95=45ms, P99=72ms
- With metadata filter: P95=48ms, P99=81ms
- Hybrid search: P95=65ms, P99=95ms

## Memory Footprint
- 1M vectors: 5.8GB RAM
- 5M vectors: 28.2GB RAM (extrapolated)
- 10M vectors: N/A (exceeds test hardware)

## Crash Recovery
- Mid-ingest: 42 seconds, 0 vectors lost
- Mid-query: 18 seconds, 0 vectors lost

## Multi-Tenant Isolation
- Isolation score: 97% (minimal cross-tenant interference)
- Quota enforcement: Working (over-quota writes rejected)
```

---

## v2.0 Target Improvements

Based on v1.x baselines, v2.0 targets:

| Metric | v1.x Baseline | v2.0 Target | Improvement |
|--------|---------------|-------------|-------------|
| Query P95 latency | ~35ms | <25ms | 29% faster |
| Memory footprint | ~6GB/1M | <5GB/1M | 40% reduction |
| Ingest throughput | ~8k vec/sec | ~10k vec/sec | 25% faster |
| Crash recovery | ~40s | <30s | 25% faster |
| Metadata query | N/A (no SQL) | <5ms P99 | New capability |

---

## Tools and Scripts

### Generate Synthetic Vectors
```python
# scripts/gen_vectors.py
import numpy as np
import pandas as pd

def generate_vectors(count, dim, format='csv'):
    vectors = np.random.randn(count, dim).astype(np.float32)
    metadata = {
        'id': [f'vec-{i:08d}' for i in range(count)],
        'timestamp': pd.date_range('2025-01-01', periods=count, freq='1s'),
        'category': np.random.choice(['A', 'B', 'C'], count),
    }

    df = pd.DataFrame(metadata)
    for i in range(dim):
        df[f'dim_{i}'] = vectors[:, i]

    if format == 'csv':
        df.to_csv(f'vectors_{count}_{dim}d.csv', index=False)
    elif format == 'json':
        df.to_json(f'vectors_{count}_{dim}d.json', orient='records')
    elif format == 'parquet':
        df.to_parquet(f'vectors_{count}_{dim}d.parquet')

if __name__ == '__main__':
    generate_vectors(1_000_000, 512, 'csv')
    generate_vectors(1_000_000, 512, 'json')
    generate_vectors(1_000_000, 512, 'parquet')
```

### Monitor Resource Usage
```bash
# scripts/monitor_resources.sh
#!/bin/bash
PID=$1
OUTPUT=$2

echo "timestamp,cpu%,mem_rss_mb,mem_vsz_mb" > $OUTPUT

while kill -0 $PID 2>/dev/null; do
  TIMESTAMP=$(date +%s)
  STATS=$(ps -p $PID -o %cpu,rss,vsz | tail -1)
  CPU=$(echo $STATS | awk '{print $1}')
  RSS=$(echo $STATS | awk '{print $2/1024}')
  VSZ=$(echo $STATS | awk '{print $3/1024}')
  echo "$TIMESTAMP,$CPU,$RSS,$VSZ" >> $OUTPUT
  sleep 1
done
```

---

## Success Criteria

- [ ] All 5 test scenarios executed successfully
- [ ] Baselines documented with percentiles (P50/P95/P99)
- [ ] No critical bugs discovered (if found, escalate)
- [ ] Results reviewed and approved by Performance Engineering Lead
- [ ] Baseline report committed to repository (`akidb-benchmarks/baselines/`)
- [ ] Findings presented at Week 0 Go/No-Go meeting (Nov 15)

---

## Dependencies

- Synthetic data generators ready (Python scripts)
- Monitoring tools installed (Prometheus, Grafana, htop, iostat)
- Mac ARM test hardware provisioned
- MinIO running locally for S3 backend

---

## Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Test hardware unavailable | Low | High | Use cloud ARM instance (AWS Graviton) |
| v1.x crashes during testing | Medium | Medium | Debug and fix before v2.0 (technical debt) |
| Baseline worse than expected | Medium | Low | Document honestly, set realistic v2.0 targets |
| Insufficient time for all tests | Medium | Medium | Prioritize scenarios 1-3 (ingest, query, memory) |

---

**Prepared by:** Performance Engineering Team
**Reviewed by:** Architecture Lead, Engineering Director
**Due Date:** 2025-11-15 (Week 0, Day 10)
**Confidentiality:** Internal Use Only
