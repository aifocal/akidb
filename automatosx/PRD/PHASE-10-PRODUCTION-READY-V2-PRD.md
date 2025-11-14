# Phase 10: Production-Ready AkiDB v2.0 GA Release

**Status**: 🚧 ACTIVE
**Timeline**: 6 weeks (Weeks 1-6)
**Goal**: Complete S3/MinIO tiered storage + production hardening for GA release
**Target**: AkiDB v2.0 General Availability (RC2 → GA)

---

## Executive Summary

Phase 10 consolidates the remaining work from **Phase 6 (S3/MinIO Tiered Storage)** and **Phase 7 (Production Hardening)** into a unified 6-week sprint to deliver AkiDB v2.0 GA release.

**What's Complete** (Baseline):
- ✅ Phases 1-5: Core infrastructure, vector indexing, REST/gRPC APIs (RC1)
- ✅ MLX Embedding Integration: Production-ready Apple Silicon embeddings
- ✅ Phase 6 Weeks 1-2: WAL + S3/ObjectStore (40% complete, 37 tests passing)
- ✅ Phase 7 Week 1: Reliability hardening (circuit breaker, DLQ, 142 tests)

**What Remains** (Phase 10 Scope):
- 🎯 Phase 6 completion: Parquet snapshots, tiering policies, RC2 release
- 🎯 Phase 7 completion: Performance, observability, Kubernetes, chaos tests
- 🎯 Production deployment infrastructure
- 🎯 GA release readiness

---

## Phase 10 Structure

### Part A: S3/MinIO Tiered Storage Completion (Weeks 1-3)

Complete Phase 6 remaining work:
- **Week 1**: Parquet Snapshotter
- **Week 2**: Hot/Warm/Cold Tiering Policies
- **Week 3**: Integration Testing + RC2 Release

### Part B: Production Hardening Completion (Weeks 4-6)

Complete Phase 7 remaining work:
- **Week 4**: Performance Optimization + E2E Testing
- **Week 5**: Observability (Prometheus, Grafana, OpenTelemetry)
- **Week 6**: Operations + GA Release

---

## Week-by-Week Action Plan

### Week 1: Parquet Snapshotter (Phase 6 Week 3)

**Objective**: Implement Parquet-based vector snapshots for S3/MinIO storage

**Deliverables**:

1. **Parquet Snapshotter Module** (`crates/akidb-storage/src/snapshotter.rs`, ~500 lines)
   - `SnapshotterConfig` struct (snapshot interval, retention policy)
   - `VectorSnapshot` struct (collection metadata + vector data)
   - `ParquetSnapshotter` implementation
   - Async snapshot creation from in-memory index
   - Async snapshot restoration to in-memory index
   - Snapshot versioning with timestamp-based naming
   - Compression (Snappy or Zstd)

2. **Parquet Schema** for vectors:
   ```rust
   struct VectorRecord {
       document_id: String,        // UUID v7 as string
       external_id: Option<String>,
       vector: Vec<f32>,           // Dense embedding
       metadata: Option<String>,   // JSON
       inserted_at: i64,           // Unix timestamp
   }
   ```

3. **Snapshot Lifecycle**:
   - Create snapshot: `snapshotter.create_snapshot(collection_id, vectors).await`
   - Upload to S3: `object_store.put(snapshot_key, parquet_bytes).await`
   - List snapshots: `snapshotter.list_snapshots(collection_id).await`
   - Restore snapshot: `snapshotter.restore_snapshot(collection_id, snapshot_id).await`
   - Delete old snapshots (retention policy)

4. **Testing** (10+ tests):
   - Snapshot creation from 10k vectors
   - Parquet compression verification
   - Round-trip test (create → restore → verify)
   - Large dataset (100k vectors, 512-dim)
   - Snapshot metadata validation
   - Concurrent snapshot operations
   - S3 upload/download integration
   - Snapshot listing and filtering
   - Retention policy enforcement
   - Edge cases (empty collection, single vector)

**Dependencies**:
- `parquet` crate for Parquet I/O
- `arrow` crate for columnar data
- Existing `ObjectStore` trait (from Phase 6 Week 2)
- Existing WAL infrastructure (from Phase 6 Week 1)

**Success Criteria**:
- ✅ 10+ tests passing
- ✅ Snapshot creation <2s for 10k vectors
- ✅ Parquet file size <5MB for 10k 512-dim vectors (with compression)
- ✅ Restore accuracy: 100% vector data integrity
- ✅ S3/MinIO compatible

**Completion Report**: `automatosx/tmp/phase-10-week1-parquet-snapshotter-completion.md`

---

### Week 2: Hot/Warm/Cold Tiering Policies (Phase 6 Week 4)

**Objective**: Implement automatic tiering based on access patterns and data age

**Deliverables**:

1. **Tiering Policy Engine** (`crates/akidb-storage/src/tiering.rs`, ~400 lines)
   - `TieringConfig` struct (thresholds, rules)
   - `DataTier` enum: Hot (RAM) | Warm (Local SSD) | Cold (S3/MinIO)
   - `TieringManager` implementation
   - Access tracking (last_accessed, access_count)
   - Automatic promotion/demotion rules
   - Background worker for tiering decisions

2. **Tiering Rules**:
   ```rust
   struct TieringConfig {
       hot_to_warm_threshold: Duration,      // e.g., 7 days no access
       warm_to_cold_threshold: Duration,     // e.g., 30 days no access
       cold_to_warm_promotion: AccessCount,  // e.g., 3 accesses in 1 hour
       max_hot_memory: usize,                // e.g., 80GB
       max_warm_disk: usize,                 // e.g., 200GB
   }
   ```

3. **Tiering Operations**:
   - `promote(collection_id, tier)`: Move data to hotter tier
   - `demote(collection_id, tier)`: Move data to colder tier
   - `get_tier(collection_id)`: Query current tier
   - `track_access(collection_id)`: Update access metadata
   - `run_tiering_worker()`: Background decision loop

4. **Integration with Existing Components**:
   - Hook into collection search (track access)
   - Hook into snapshot creation (warm tier)
   - Hook into WAL replay (hot tier)
   - S3 upload/download for cold tier

5. **Testing** (12+ tests):
   - Hot → Warm demotion after threshold
   - Warm → Cold demotion after threshold
   - Cold → Hot promotion on access
   - Memory limit enforcement (evict LRU)
   - Disk limit enforcement
   - Concurrent access tracking
   - Multi-collection tiering
   - Tiering policy configuration
   - Background worker lifecycle
   - Tier migration integrity
   - Metadata persistence
   - Edge cases (pinned collections, minimum tier)

**Dependencies**:
- ParquetSnapshotter (Week 1)
- ObjectStore (Phase 6 Week 2)
- WAL (Phase 6 Week 1)

**Success Criteria**:
- ✅ 12+ tests passing
- ✅ Automatic tiering decisions <100ms
- ✅ Zero data loss during tier migrations
- ✅ Memory limit enforcement (no OOM)
- ✅ Observable metrics (tier distribution, migration events)

**Completion Report**: `automatosx/tmp/phase-10-week2-tiering-policies-completion.md`

---

### Week 3: Integration Testing + RC2 Release (Phase 6 Week 5)

**Objective**: End-to-end integration testing and release AkiDB RC2

**Deliverables**:

1. **Integration Tests** (`crates/akidb-storage/tests/integration_test.rs`, ~600 lines)
   - Full WAL → Snapshot → Tiering workflow
   - Crash recovery with S3 snapshots
   - Multi-collection tiering scenarios
   - High write load with WAL + snapshots
   - Concurrent read/write with tiering
   - S3/MinIO compatibility tests
   - Local ObjectStore fallback tests
   - Memory pressure scenarios
   - Disk pressure scenarios
   - Network failure handling (S3 retries)

2. **E2E Workflow Tests**:
   - Insert 100k vectors → WAL → Hot tier
   - Wait 7 days (simulated) → Demote to Warm
   - Access once → Restore to Hot
   - Wait 30 days → Demote to Cold (S3)
   - Access again → Restore from S3 to Hot
   - Verify: 100% data integrity throughout

3. **Performance Benchmarks**:
   - WAL write throughput: >10k ops/sec
   - Snapshot creation: <3s for 100k vectors
   - Snapshot restore: <5s for 100k vectors
   - S3 upload bandwidth: >50 MB/s
   - S3 download bandwidth: >50 MB/s
   - Tiering decision latency: <100ms
   - Memory footprint: <100GB for 100k collections

4. **RC2 Release Preparation**:
   - Update CHANGELOG.md with Phase 6 features
   - Update docs/DEPLOYMENT-GUIDE.md with S3/MinIO setup
   - Add S3 configuration examples to config.example.toml
   - Update docker-compose.yaml with MinIO service
   - Create Helm chart for Kubernetes (basic)
   - Tag release: `v2.0.0-rc2`

5. **Documentation**:
   - S3/MinIO configuration guide
   - Tiering policy tuning guide
   - Snapshot restore procedures
   - Disaster recovery runbook
   - Performance tuning recommendations

**Success Criteria**:
- ✅ All integration tests passing (20+ tests)
- ✅ Zero data corruption under stress
- ✅ RC2 release tagged and documented
- ✅ Docker Compose + MinIO verified
- ✅ Benchmarks meet targets

**Completion Report**: `automatosx/tmp/phase-10-week3-rc2-release-completion.md`

---

### Week 4: Performance Optimization + E2E Testing (Phase 7 Week 2)

**Objective**: Optimize critical paths and build comprehensive E2E test suite

**Deliverables**:

1. **Performance Optimization**:
   - **Batch S3 uploads** (target: 500 ops/sec)
     - Implement `BatchUploader` with async queue
     - Bundle small objects into larger batches
     - Configurable batch size (e.g., 100 objects or 10MB)
     - Automatic flush on timeout or size threshold

   - **Parallel S3 uploads** (target: 600 ops/sec)
     - Use `tokio::spawn` for concurrent S3 PUT operations
     - Semaphore to limit concurrency (e.g., 10 parallel uploads)
     - Progress tracking and error handling

   - **Connection pooling**:
     - HTTP/2 multiplexing for S3 connections
     - Keep-alive connections
     - Retry with exponential backoff

   - **Compression optimization**:
     - Benchmark Snappy vs Zstd vs LZ4
     - Tune compression levels for speed/size trade-off
     - Async compression in background threads

2. **E2E Test Infrastructure**:
   - **Mock S3 service** (`tests/mock_s3.rs`, ~300 lines)
     - In-memory S3-compatible server
     - Simulate network latency
     - Inject failures (timeouts, 500 errors)
     - Verify S3 API contract compliance

   - **E2E test scenarios** (15+ tests):
     - Retry logic on S3 failures
     - DLQ behavior under load
     - Circuit breaker state transitions
     - Graceful degradation (S3 unavailable)
     - Recovery after network outage
     - Concurrent collection operations
     - High-throughput insert + snapshot
     - Cold start recovery from S3
     - Multi-tenant isolation
     - Quota enforcement under load

3. **Load Testing**:
   - `wrk` scripts for REST API
   - gRPC load testing with `ghz`
   - Sustained 100 QPS for 1 hour
   - Measure P50/P95/P99 latency
   - Monitor memory/CPU usage
   - Identify bottlenecks

4. **Profiling and Optimization**:
   - `cargo flamegraph` for CPU profiling
   - Memory profiling with `valgrind`/`heaptrack`
   - Async task monitoring
   - Lock contention analysis
   - Database query optimization (SQLite)

**Success Criteria**:
- ✅ S3 batch uploads: >500 ops/sec
- ✅ S3 parallel uploads: >600 ops/sec
- ✅ 15+ E2E tests passing
- ✅ P95 latency <25ms under load
- ✅ Zero memory leaks detected
- ✅ All load tests pass

**Completion Report**: `automatosx/tmp/phase-10-week4-performance-e2e-completion.md`

---

### Week 5: Observability (Prometheus, Grafana, OpenTelemetry) (Phase 7 Week 3)

**Objective**: Production-grade observability with metrics, dashboards, and tracing

**Deliverables**:

1. **Prometheus Metrics Exporter** (`crates/akidb-rest/src/metrics.rs`, ~400 lines)

   **12 Key Metrics**:
   ```rust
   // Request metrics
   http_requests_total{method, path, status}         // Counter
   http_request_duration_seconds{method, path}       // Histogram
   grpc_requests_total{service, method, status}      // Counter
   grpc_request_duration_seconds{service, method}    // Histogram

   // Vector operations
   vector_search_duration_seconds                     // Histogram
   vector_upsert_duration_seconds                     // Histogram
   vectors_indexed_total                              // Counter

   // Storage metrics
   wal_write_duration_seconds                         // Histogram
   s3_upload_duration_seconds                         // Histogram
   s3_upload_bytes_total                              // Counter
   snapshot_creation_duration_seconds                 // Histogram

   // Tiering metrics
   collection_tier{tier}                              // Gauge
   tier_migration_duration_seconds{from_tier, to_tier} // Histogram

   // Reliability metrics
   circuit_breaker_state{service}                     // Gauge (0=closed, 1=open, 2=half-open)
   dlq_size                                           // Gauge
   dlq_items_added_total                              // Counter
   retry_attempts_total{operation}                    // Counter

   // Resource metrics
   memory_usage_bytes{component}                      // Gauge
   active_connections                                 // Gauge
   ```

2. **Grafana Dashboards** (4 dashboards, JSON + screenshots)

   **Dashboard 1: Request Performance**
   - HTTP request rate (req/sec)
   - P50/P95/P99 latency
   - Error rate by status code
   - Request duration heatmap

   **Dashboard 2: Vector Operations**
   - Search QPS
   - Search latency distribution
   - Upsert throughput
   - Index size over time
   - Recall metrics (if available)

   **Dashboard 3: Storage & Tiering**
   - WAL write latency
   - S3 upload/download bandwidth
   - Snapshot creation frequency
   - Tier distribution (hot/warm/cold)
   - Tier migration events
   - Storage usage by tier

   **Dashboard 4: Reliability & Health**
   - Circuit breaker states
   - DLQ depth over time
   - Retry success/failure rates
   - Active connections
   - Memory usage by component
   - Error budget tracking

3. **OpenTelemetry Distributed Tracing**
   - **Setup**:
     - Add `opentelemetry` + `opentelemetry-jaeger` crates
     - Initialize tracer in main.rs
     - Configure Jaeger collector endpoint

   - **Instrumented Spans**:
     - HTTP request → collection search → HNSW query → result serialization
     - gRPC stream → batch embedding → MLX inference
     - Vector upsert → WAL write → index update → snapshot trigger
     - Snapshot creation → Parquet encoding → S3 upload
     - Tier migration → data fetch → tier change → metadata update

   - **Trace Context Propagation**:
     - Extract trace context from HTTP headers
     - Propagate context across async boundaries
     - Add custom attributes (tenant_id, collection_id, operation_type)

4. **Alert Rules** (`k8s/alerts.yaml`, Prometheus AlertManager)

   **Critical Alerts**:
   - High error rate: >5% errors for 5 minutes
   - High latency: P95 >100ms for 5 minutes
   - Circuit breaker open: Any service open for 2 minutes
   - DLQ overflow: >5000 items in DLQ
   - Memory pressure: >90% memory usage
   - Disk pressure: >85% disk usage

   **Warning Alerts**:
   - Elevated error rate: >1% errors for 10 minutes
   - Slow S3 uploads: P95 >10s for 5 minutes
   - High retry rate: >10% retries for 5 minutes
   - Stale snapshots: No snapshot in 24 hours

5. **Runbook** (`docs/RUNBOOK.md`)
   - Alert response procedures
   - Common issues and resolutions
   - Debugging checklists
   - Escalation paths
   - Recovery procedures

**Success Criteria**:
- ✅ All 12 metrics exported and scraped
- ✅ 4 Grafana dashboards deployed
- ✅ Distributed tracing working end-to-end
- ✅ Alert rules firing in test scenarios
- ✅ Runbook validated with dry-run exercises

**Completion Report**: `automatosx/tmp/phase-10-week5-observability-completion.md`

---

### Week 6: Operations + GA Release (Phase 7 Week 4)

**Objective**: Production deployment infrastructure and GA release

**Deliverables**:

1. **Kubernetes Helm Charts** (`k8s/helm/akidb/`, ~800 lines YAML)

   **Chart Structure**:
   ```
   akidb/
   ├── Chart.yaml
   ├── values.yaml
   ├── templates/
   │   ├── deployment.yaml       # StatefulSet for akidb-rest
   │   ├── service.yaml          # LoadBalancer for REST API
   │   ├── grpc-service.yaml     # LoadBalancer for gRPC
   │   ├── configmap.yaml        # Configuration
   │   ├── secrets.yaml          # S3 credentials
   │   ├── pvc.yaml              # Persistent volume for WAL
   │   ├── prometheus-service-monitor.yaml
   │   └── hpa.yaml              # Horizontal Pod Autoscaler
   ```

   **Key Features**:
   - ConfigMap for `config.toml`
   - Secret for S3/MinIO credentials
   - PVC for WAL persistence (100GB)
   - Resource limits (CPU/memory)
   - Liveness/readiness probes
   - Horizontal pod autoscaling (2-10 replicas)
   - Service annotations for Prometheus scraping
   - Ingress configuration (optional)

2. **Blue-Green Deployment Automation** (`scripts/deploy-blue-green.sh`)
   - Deploy new version to "green" namespace
   - Run smoke tests against green deployment
   - Switch traffic from blue to green (update Service selector)
   - Monitor for errors (5 minute observation window)
   - Automatic rollback if error rate >1%
   - Cleanup old "blue" deployment after success

3. **Incident Response Playbooks** (`docs/PLAYBOOKS.md`)

   **Playbook 1: High Error Rate**
   - Check: Recent deployments? Configuration changes?
   - Action: Review logs, check circuit breaker states
   - Escalation: Rollback if deployment-related

   **Playbook 2: High Latency**
   - Check: Database locks? S3 throttling? Memory pressure?
   - Action: Scale up replicas, check slow query log
   - Escalation: Engage on-call engineer

   **Playbook 3: Data Loss Suspected**
   - Check: WAL files intact? S3 snapshots available?
   - Action: Restore from latest snapshot
   - Escalation: Incident commander activation

   **Playbook 4: S3 Outage**
   - Check: Circuit breaker protecting workloads?
   - Action: Operate in degraded mode (hot tier only)
   - Escalation: Notify users of reduced durability

4. **Chaos Engineering Tests** (`tests/chaos_tests.rs`, ~400 lines)

   **Chaos Scenarios** (5 tests):
   - **Pod termination**: Kill random pod during write load
     - Expected: Zero data loss (WAL), graceful failover

   - **Network partition**: Simulate S3 network failure
     - Expected: Circuit breaker opens, DLQ captures failures

   - **Resource starvation**: CPU throttling to 10%
     - Expected: Increased latency, no crashes

   - **Disk full**: Fill WAL disk to 100%
     - Expected: Log rotation frees space, no data loss

   - **Cascading failure**: Kill DB + S3 + 2 pods simultaneously
     - Expected: System recovers within 60 seconds

   **Chaos Testing Framework**:
   - Use `toxiproxy` for network chaos
   - Use `stress-ng` for resource chaos
   - Automated chaos injection with `chaos-mesh` (Kubernetes)

5. **GA Release Checklist** (`docs/GA-RELEASE-CHECKLIST.md`)

   **Pre-Release**:
   - ✅ All tests passing (147+ tests)
   - ✅ Zero known critical bugs
   - ✅ Performance benchmarks meet targets
   - ✅ Security audit complete (OWASP top 10)
   - ✅ Documentation complete
   - ✅ Migration guide (v1.x → v2.0)
   - ✅ Helm chart tested on GKE/EKS/AKS
   - ✅ Load testing (1000 QPS sustained)
   - ✅ Chaos testing passed

   **Release**:
   - Tag: `v2.0.0`
   - GitHub release with changelog
   - Docker images published (Docker Hub)
   - Helm chart published (Artifact Hub)
   - Update website/docs
   - Announcement blog post
   - Community notification (Reddit, HN, Twitter)

**Success Criteria**:
- ✅ Helm chart deploys on K8s (1 command)
- ✅ Blue-green deployment verified
- ✅ All 5 chaos tests pass
- ✅ Playbooks validated with tabletop exercise
- ✅ GA release tagged and published

**Completion Report**: `automatosx/tmp/phase-10-week6-ga-release-completion.md`

---

## Testing Strategy

### Unit Tests
- Target: 200+ tests passing (current: 147)
- New tests: ~60 for Phase 10 work
- Coverage: >85% for all new code

### Integration Tests
- WAL + Snapshot + Tiering integration
- S3/MinIO compatibility suite
- Multi-collection scenarios
- Concurrent operations stress tests

### E2E Tests
- Full workflow tests (insert → tier → snapshot → restore)
- Failure recovery scenarios
- Mock S3 integration tests
- Load testing with realistic workloads

### Chaos Tests
- 5 chaos scenarios (pod kill, network partition, resource starvation, disk full, cascading)
- Automated chaos injection
- Resilience verification

### Performance Benchmarks
- Search P95 <25ms @ 100 QPS
- WAL write >10k ops/sec
- S3 upload >500 ops/sec (batched), >600 ops/sec (parallel)
- Snapshot creation <3s for 100k vectors
- Memory footprint <100GB for 100k collections

---

## Dependencies

### External Crates (New)
- `parquet` (Parquet I/O)
- `arrow` (columnar data for Parquet)
- `prometheus` (metrics export)
- `opentelemetry` + `opentelemetry-jaeger` (distributed tracing)
- `toxiproxy-rust` (chaos testing)

### Infrastructure
- MinIO (S3-compatible storage)
- Prometheus (metrics collection)
- Grafana (dashboards)
- Jaeger (tracing backend)
- Kubernetes (orchestration)

---

## Risk Mitigation

### Risk 1: Parquet Performance
**Mitigation**: Benchmark early (Week 1 Day 1), tune compression, consider columnar optimizations

### Risk 2: S3 Rate Limits
**Mitigation**: Implement exponential backoff, request rate limiting, batch uploads

### Risk 3: Tiering Complexity
**Mitigation**: Start simple (2 tiers), add cold tier in Week 2, iterate based on testing

### Risk 4: K8s Deployment Issues
**Mitigation**: Test on minikube first, use managed K8s (GKE/EKS) for validation

### Risk 5: Observability Overhead
**Mitigation**: Make tracing sampling configurable (e.g., 1% sampling), optimize metrics cardinality

---

## Success Metrics

### Functional
- ✅ All Phase 6 features complete (Parquet, tiering, RC2)
- ✅ All Phase 7 features complete (performance, observability, K8s)
- ✅ 200+ tests passing (zero failures)
- ✅ GA release published and documented

### Performance
- ✅ Search P95 <25ms @ 100 QPS
- ✅ S3 upload >500 ops/sec (batched)
- ✅ Snapshot restore <5s for 100k vectors
- ✅ Zero memory leaks

### Operational
- ✅ Kubernetes deployment works (1 command)
- ✅ Blue-green deployment automated
- ✅ Chaos tests pass
- ✅ Observability stack deployed (Prometheus + Grafana + Jaeger)

### Documentation
- ✅ Deployment guide updated
- ✅ Runbook complete
- ✅ Playbooks validated
- ✅ API documentation current

---

## Deliverables Summary

**Week 1**: Parquet Snapshotter (500 lines, 10 tests)
**Week 2**: Tiering Policies (400 lines, 12 tests)
**Week 3**: Integration Testing + RC2 Release (20 tests, docs)
**Week 4**: Performance Optimization + E2E (15 tests, benchmarks)
**Week 5**: Observability (12 metrics, 4 dashboards, tracing)
**Week 6**: Operations + GA Release (Helm chart, chaos tests, release)

**Total New Code**: ~2,500 lines Rust + ~800 lines YAML/config
**Total New Tests**: ~60 tests (unit + integration + E2E + chaos)
**Total Documentation**: ~20 pages (guides, runbooks, playbooks)

---

## Timeline

```
Week 1 (Days 1-5):   Parquet Snapshotter
Week 2 (Days 6-10):  Tiering Policies
Week 3 (Days 11-15): Integration + RC2 Release
Week 4 (Days 16-20): Performance + E2E
Week 5 (Days 21-25): Observability
Week 6 (Days 26-30): Operations + GA Release

Day 30: AkiDB v2.0 GA 🎉
```

---

## Post-GA Roadmap (Future)

After Phase 10 completion, consider:
- **Phase 11**: Cedar policy engine (ABAC upgrade)
- **Phase 12**: Multi-region deployment
- **Phase 13**: Distributed vector search (sharding)
- **Phase 14**: Advanced ML features (query expansion, reranking)
- **Phase 15**: Enterprise features (SSO, audit logging enhancements)

---

## References

- **Phase 6 Week 1 Report**: `automatosx/tmp/phase-6-week1-wal-completion-report.md`
- **Phase 6 Week 2 Report**: `automatosx/tmp/phase-6-week2-objectstore-completion-report.md`
- **Phase 7 Week 1 Report**: `automatosx/tmp/PHASE-7-WEEK-1-COMPLETION-REPORT.md`
- **MLX Integration**: `automatosx/tmp/MLX-INTEGRATION-COMPLETE.md`
- **Current Codebase**: 147 tests passing, RC1 released

---

**Phase 10 Goal**: Ship AkiDB v2.0 GA with production-grade S3/MinIO tiered storage, comprehensive observability, and enterprise deployment infrastructure.

**Let's build it! 🚀**
