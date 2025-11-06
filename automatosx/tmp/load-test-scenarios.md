# AkiDB 2.0 Load Test Scenarios

Quality is not an act, it's a habit. Test early, test often, test everything.

## Test Environment & Instrumentation Baseline
- **Target build**: AkiDB 2.0 RC (commit hash recorded per run); baseline cluster size 3 primary nodes + 2 replicas.
- **Resources**: 32 vCPU / 128 GB RAM per node, SSD-backed storage, dedicated embedding service tier, S3-compatible object store, Cedar control plane.
- **Traffic driver**: k6 (HTTP/gRPC) + custom Go harness for vector RPCs; orchestrated via GitHub Actions nightly and on-demand.
- **Observability**: Prometheus (system + app metrics), Jaeger (traces), Loki (logs), Grafana dashboards with scenario-specific panels.
- **Data reset**: Snapshots restored before each scenario; WAL archives trimmed.

## Scenario 1 – 100 QPS Hybrid Search Mix
**Objective**: Validate that AkiDB 2.0 sustains 100 QPS with SLA adherence under realistic hybrid workload composition.

- **Workload mix (per minute)**:
  - 60% vector similarity queries (single collection, top-k = 20, cosine metric).
  - 30% metadata-only filtered lookups (B-tree indexes on tags, range filters).
  - 10% hybrid (vector pre-filter + metadata post-filter) queries.
- **Traffic model**: Poisson arrivals, constant 100 QPS after 5-minute ramp (20→60→100 QPS); burndown 2 minutes.
- **Payloads**: 512-dim float32 vectors (average size 2 KB); metadata documents ~1.5 KB JSON.
- **Data volume**: 50M vectors, 120M metadata records; 30% of queries target “hot” partitions.
- **Cache policy**: Query cache cold start at run start; warm-up run separate to compare.
- **Test duration**: 30 min steady-state.
- **Metrics tracked**:
  - P50/P95/P99 latency per query type (<40 ms / <80 ms / <120 ms respectively).
  - Error budget (>=99.9% success), broken down by gRPC status / HTTP codes.
  - Resource: CPU <75%, memory <70%, IO wait <5%, GPU (if enabled) <60%.
  - Index refresh lag <5 s.
  - Embedding queue depth <100 pending.
- **Instrumentation hooks**:
  - Trace percent: 5% vector, 5% hybrid, 1% metadata queries.
  - Custom counters for cache hit/miss, vector distance calculations.
- **Pass / Fail**:
  - All latency SLOs met with <0.1% error rate.
  - No auto-scaling churn or throttling events.
  - Snapshot of metrics exported for regression comparison.

## Scenario 2 – Failover & Resiliency Drills
**Objective**: Validate graceful degradation and recovery under key dependency failures while under 60 QPS background load (same mix as Scenario 1 scaled down proportionally).

Conduct each sub-scenario independently; allow system to stabilize between runs.

### 2A: Embedding Service Outage
- **Failure injection**: Terminate embedding pods or block network for 10 minutes after 5-minute warm steady load.
- **Expected behavior**: Query path continues (using cached embeddings); ingest path queues without data loss; retries capped.
- **Metrics**: Ingest queue age <15 min, retry success >=95%, fallback latency <20 ms overhead.
- **Validation**: After restoration, backlog drains within 10 minutes, no data inconsistency.

### 2B: S3/Object Store Partial Outage
- **Failure injection**: Introduce 500 ms latency + 5% error on S3 writes via chaos proxy for 15 minutes.
- **Expected behavior**: WAL continues locally, snapshot uploads throttled but no data loss; hybrid query latency increases <30%.
- **Metrics**: WAL flush age <60 s, snapshot retry success >99%, no client-facing 5xx errors.
- **Validation**: Reconcile ensures consistency post-recovery; audit sample of recovered objects.

### 2C: WAL Disk Saturation / Unavailability
- **Failure injection**: Fill WAL volume to 95% and simulate disk pause (fsfreeze) for 90 s during steady load.
- **Expected behavior**: Backpressure triggers; system rejects writes with clear 429 messaging; read latency unaffected.
- **Metrics**: Number of rejected writes logged, recovery time <5 min, zero data corruption, replay successful.

### 2D: Cedar Control Plane Outage
- **Failure injection**: Disable Cedar API for 7 minutes.
- **Expected behavior**: Existing connections continue; no schema drift; management operations queued.
- **Metrics**: Config sync lag <2 cycles, admin API 503s flagged, no query failures.
- **Recovery**: Admin queue drains <5 min after Cedar restored.

## Scenario 3 – Multi-Tenancy Stress (10 Tenants)
**Objective**: Demonstrate isolation and fairness across tenants under combined 200 QPS (20 QPS per tenant) workload.

- **Tenant profile**:
  - Tenants split across 3 clusters sharing storage; per-tenant quotas applied.
  - 3 premium tenants (vector-heavy 70/20/10 mix), 7 standard tenants (40/50/10 mix).
- **Data**: Distinct collections; overlapping metadata keys to test namespace isolation.
- **Traffic model**: Closed workload with per-tenant virtual users (VU) count tuned to sustain 20 QPS.
- **Metrics**:
  - Per-tenant latency SLOs (P95 <90 ms) and error rate (<0.2%).
  - Quota enforcement: CPU usage variance <10% between tenants at same tier.
  - No noisy neighbor impact (resource saturation triggered throttle per-tenant not global).
  - Background maintenance (compaction, defrag) does not breach SLOs.
- **Instrumentation**: Tenant-tagged logs/metrics, distributed traces carrying tenant ID.
- **Pass criteria**: All tenants within SLO; if one violates, capture and analyze resource contention.

## Scenario 4 – Regression Baselines vs AkiDB 1.x
**Objective**: Ensure v2.0 meets or exceeds v1.x performance across representative workloads.

- **Benchmark matrix**:
  - Workloads: Hybrid 100 QPS (Scenario 1), ingest-heavy (500 docs/s with vector embeds), analytic scans (1 GB range queries).
  - Environments: v1.x reference cluster (matching hardware), v2.0 cluster.
- **Procedure**:
  1. Run each workload on v1.x → capture throughput, latency, resource, cost metrics.
  2. Repeat on v2.0 with identical data + configuration.
  3. Use automated diff to compare P50/P95, CPU/memory, storage throughput.
- **Metrics target**:
  - Latency: v2.0 should be <= v1.x +5% for P95; improvements noted.
  - Throughput: >= v1.x.
  - Resource: Cost-normalized efficiency improvements >=10% or documented rationale.
  - Feature parity: Validate new features do not regress (e.g., hybrid filter accuracy).
- **Reporting**: Generate markdown summary + Grafana snapshot; archive under `automatosx/PRD/perf/regression-report-<date>.md`.

## Scenario 5 – Test Data Generators
**Objective**: Provide reusable generators for consistent load test data.

- **Vector corpus generator**:
  - Tooling: Python script leveraging `numpy` to create Gaussian-clustered vectors with controllable centroid drift.
  - Parameters: cluster count, dimensionality, noise ratio, label distribution.
  - Output: Parquet segments + JSON metadata referencing S3 URIs.
  - Automation: Store script under `tests/load/generators/vector_corpus.py`; schedule regeneration weekly.

- **Metadata document synthesizer**:
  - Tooling: Go-based generator (shared with ingest harness) producing JSON docs with configurable schema (nested tags, timestamps, status enums).
  - Supports tenant-specific overrides and skew injection (hot keys, missing fields).
  - Exports to NDJSON for bulk ingest + gRPC streaming payloads.

- **Hybrid query sampler**:
  - Generate query templates mixing vector IDs and metadata filters; optionally seeded from production traffic anonymized logs.
  - Provide YAML configuration per scenario listing query mix, expected cardinality, and target accuracy thresholds.

- **Failure injection scripts**:
  - Chaos mesh manifests for dependency outages (embedding deployment, Cedar API).
  - Bash utilities to manipulate S3 proxy latency/error rates and WAL disk saturation.

- **Data validation harness**:
  - After generation, run checksum + schema validation, verify referential integrity between vector embeddings and metadata docs.

## Execution Cadence & Automation
- Nightly pipeline runs Scenario 1 & 3; weekly pipeline rotates through failover drills (Scenario 2) and regression comparison (Scenario 4).
- Publish KPIs to Slack/perf channel with red/yellow/green status.
- All scenarios gated in CI before release candidate cut; failure blocks release until remedied.

## Open Questions / Follow-Ups
- Confirm Cedar control plane failure impact on tenant creation flows.
- Align S3 chaos injection with infra team’s tooling (Gremlin vs. in-house proxy).
- Need GPU vs CPU embedding service matrix if GPU acceleration toggled.

