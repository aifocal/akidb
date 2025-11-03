# Phase 6: Milestone Tracking

**Epic:** Offline RAG & Air-Gap Features
**Target:** Q2 2025 (12 weeks)
**Status:** 🟡 Planning → Ready for Development

---

## Milestone Overview

| # | Milestone | Duration | Status | Assignee |
|---|-----------|----------|--------|----------|
| M1 | Ingest CLI Tool | 2 weeks | 🔵 Not Started | TBD |
| M2 | CSV/JSONL Parsers | 1 week | 🔵 Not Started | TBD |
| M3 | Batch Pipeline | 1 week | 🔵 Not Started | TBD |
| M4 | Parquet Support | 1 week | 🔵 Not Started | TBD |
| M5 | Package Format | 2 weeks | 🔵 Not Started | TBD |
| M6 | Export Pipeline | 1 week | 🔵 Not Started | TBD |
| M7 | Import Pipeline | 1 week | 🔵 Not Started | TBD |
| M8 | Replication Setup | 1 week | 🔵 Not Started | TBD |
| M9 | Monitoring | 1 week | 🔵 Not Started | TBD |
| M10 | Failover | 1 week | 🔵 Not Started | TBD |
| M11 | Bundle Creation | 1 week | 🔵 Not Started | TBD |
| M12 | Dependency Mgmt | 1 week | 🔵 Not Started | TBD |
| M13-14 | Multi-Language | 1 week | 🔵 Not Started | TBD |

**Legend:**
- 🔵 Not Started
- 🟡 In Progress
- 🟢 Completed
- 🔴 Blocked

---

## M1: Ingest CLI Tool (Week 1-2)

**Goal:** Create `akidb-ingest` binary with CLI argument parsing and progress tracking.

### Tasks

- [ ] Create `services/akidb-ingest` crate
- [ ] Add clap dependency for CLI parsing
- [ ] Implement argument validation
- [ ] Add progress bar (indicatif)
- [ ] Add structured logging (tracing)
- [ ] Write CLI integration tests
- [ ] Add --help documentation

### Acceptance Criteria

- ✅ CLI binary builds successfully
- ✅ All arguments parsed and validated
- ✅ Progress bar shows during ingestion
- ✅ Errors logged with clear messages
- ✅ --help shows all available options

### Dependencies

- None (can start immediately)

### Estimated Effort

- 10-12 hours

---

## M2: CSV/JSONL Parsers (Week 2-3)

**Goal:** Implement streaming parsers for CSV and JSONL formats.

### Tasks

- [ ] Add csv crate dependency
- [ ] Implement CSV schema inference
- [ ] Implement CSV streaming parser
- [ ] Add JSONL streaming parser
- [ ] Validate vector dimensions
- [ ] Handle malformed records gracefully
- [ ] Write parser unit tests
- [ ] Benchmark parser throughput

### Acceptance Criteria

- ✅ Parse 1M vector CSV in < 60s
- ✅ Parse 1M vector JSONL in < 90s
- ✅ Handle missing/malformed records
- ✅ Schema validation errors are clear
- ✅ Memory usage < 100MB (streaming)

### Dependencies

- M1 (CLI Tool)

### Estimated Effort

- 8-10 hours

---

## M3: Batch Pipeline (Week 3-4)

**Goal:** Integrate parsers with WAL and implement chunked parallel processing.

### Tasks

- [ ] Implement batch buffer (10K vectors)
- [ ] Integrate with akidb-storage WAL
- [ ] Add rayon parallel processing
- [ ] Implement crash recovery checkpoints
- [ ] Add resume-from-checkpoint logic
- [ ] Write integration tests
- [ ] Benchmark end-to-end ingest

### Acceptance Criteria

- ✅ Ingest 10M vectors without OOM
- ✅ Parallel processing utilizes all cores
- ✅ Crash recovery works (resume from checkpoint)
- ✅ WAL writes are durable
- ✅ Segments flushed to MinIO correctly

### Dependencies

- M2 (Parsers)
- akidb-storage WAL implementation

### Estimated Effort

- 12-14 hours

---

## M4: Parquet Support (Week 4)

**Goal:** Add Apache Arrow/Parquet reader for high-performance ingestion.

### Tasks

- [ ] Add arrow dependency to workspace
- [ ] Implement Parquet reader
- [ ] Map Parquet schema to VectorRecord
- [ ] Add columnar processing optimizations
- [ ] Benchmark vs CSV (expect 3-5x faster)
- [ ] Write Parquet integration tests

### Acceptance Criteria

- ✅ Parse 1M vector Parquet in < 30s
- ✅ 3x faster than CSV (benchmarked)
- ✅ Correctly handles nested Parquet schemas
- ✅ Memory usage similar to CSV parser

### Dependencies

- M3 (Batch Pipeline)

### Estimated Effort

- 6-8 hours

---

## M5: Package Format (Week 5-6)

**Goal:** Design and implement .akipkg TAR+Zstd package structure.

### Tasks

- [ ] Design .akipkg directory structure
- [ ] Implement manifest.json serialization
- [ ] Add tar + zstd compression
- [ ] Implement Ed25519 signature generation (ring crate)
- [ ] Add SHA-256 checksum computation
- [ ] Write package validation logic
- [ ] Create unit tests for package format

### Acceptance Criteria

- ✅ .akipkg contains all required files
- ✅ Manifest is JSON-parseable
- ✅ Signatures verify correctly
- ✅ Checksums prevent corruption
- ✅ Compression ratio > 5:1 (typical)

### Dependencies

- Phase 4 (needs stable segment format)

### Estimated Effort

- 12-15 hours

---

## M6: Export Pipeline (Week 6-7)

**Goal:** Implement `akidb-pkg export` to create .akipkg from collection.

### Tasks

- [ ] Create `services/akidb-pkg` binary crate
- [ ] Implement segment streaming from MinIO
- [ ] Add TAR archive generation
- [ ] Compute checksums during export
- [ ] Sign manifest with private key
- [ ] Add progress tracking
- [ ] Write export integration tests

### Acceptance Criteria

- ✅ Export 100K vector collection in < 10s
- ✅ Package is byte-for-byte verifiable
- ✅ Signature validates
- ✅ Can export from production MinIO

### Dependencies

- M5 (Package Format)

### Estimated Effort

- 10-12 hours

---

## M7: Import Pipeline (Week 7-8)

**Goal:** Implement `akidb-pkg import` to restore collection from .akipkg.

### Tasks

- [ ] Implement signature verification
- [ ] Verify SHA-256 checksums
- [ ] Extract TAR archive
- [ ] Validate manifest compatibility
- [ ] Import segments to MinIO
- [ ] Rebuild HNSW indices
- [ ] Write import integration tests
- [ ] Add rollback on failure

### Acceptance Criteria

- ✅ Import 100K vector package in < 15s
- ✅ Signature verification rejects tampered packages
- ✅ Checksum validation detects corruption
- ✅ Imported collection is searchable
- ✅ Rollback works on error

### Dependencies

- M6 (Export Pipeline)

### Estimated Effort

- 12-14 hours

---

## M8: Replication Setup (Week 9)

**Goal:** Automate MinIO Site Replication configuration.

### Tasks

- [ ] Create `services/akidb-replication` binary
- [ ] Generate MinIO replication config
- [ ] Implement site pairing automation
- [ ] Add bucket setup scripts
- [ ] Validate replication status
- [ ] Write replication setup guide
- [ ] Test bi-directional sync

### Acceptance Criteria

- ✅ Sites pair automatically
- ✅ Replication config is correct
- ✅ Bi-directional sync verified
- ✅ No manual MinIO commands needed

### Dependencies

- MinIO cluster (test environment)

### Estimated Effort

- 8-10 hours

---

## M9: Monitoring Integration (Week 9-10)

**Goal:** Add Prometheus metrics for replication lag and health.

### Tasks

- [ ] Add replication lag metric
- [ ] Add replication failure counter
- [ ] Create Grafana dashboard JSON
- [ ] Add alerting rules (Prometheus)
- [ ] Document metric meanings
- [ ] Test alert triggers

### Acceptance Criteria

- ✅ Replication lag visible in Prometheus
- ✅ Alerts trigger on lag > 5 min
- ✅ Dashboard shows cross-site status
- ✅ Metrics documented

### Dependencies

- M8 (Replication Setup)
- Phase 4 (Prometheus metrics)

### Estimated Effort

- 6-8 hours

---

## M10: Failover Automation (Week 10)

**Goal:** Implement automatic DR failover.

### Tasks

- [ ] Implement site health checks
- [ ] Add failover trigger logic
- [ ] Promote DR site to primary
- [ ] Update DNS/load balancer (integration)
- [ ] Add failback automation
- [ ] Write failover runbook
- [ ] Test failover scenario

### Acceptance Criteria

- ✅ Failover completes in < 5 min
- ✅ No data loss (verified)
- ✅ Health checks detect failures
- ✅ Runbook is clear and actionable

### Dependencies

- M8 (Replication Setup)

### Estimated Effort

- 10-12 hours

---

## M11: Bundle Creation (Week 11)

**Goal:** Create offline installation bundle.

### Tasks

- [ ] Write bundle creation script
- [ ] Download MinIO binaries (ARM64)
- [ ] Include all AkiDB binaries
- [ ] Add example configs
- [ ] Write offline install script
- [ ] Test installation on clean ARM system
- [ ] Compute SHA-256 for bundle

### Acceptance Criteria

- ✅ Bundle installs without internet
- ✅ All binaries execute correctly
- ✅ Install script is idempotent
- ✅ Works on macOS ARM64 and Linux ARM64

### Dependencies

- All binary crates (M1-M10)

### Estimated Effort

- 8-10 hours

---

## M12: Dependency Management (Week 11-12)

**Goal:** Vendor all dependencies for offline builds.

### Tasks

- [ ] Run `cargo vendor`
- [ ] Test build from vendored deps
- [ ] Add license compliance check
- [ ] Document vendored dep update process
- [ ] Create Cargo.toml with vendored config
- [ ] Test on air-gapped VM

### Acceptance Criteria

- ✅ `cargo build` works offline
- ✅ All licenses documented
- ✅ No network access during build
- ✅ Vendored deps < 500MB

### Dependencies

- M11 (Bundle Creation)

### Estimated Effort

- 4-6 hours

---

## M13-14: Multi-Language Support (Week 12)

**Goal:** Add language detection and tokenization for 5 languages.

### Tasks

- [ ] Add whichlang dependency
- [ ] Implement language detection
- [ ] Add language-specific tokenizers
- [ ] Support CJK (Chinese, Japanese, Korean)
- [ ] Store language tag in metadata
- [ ] Write multi-language tests
- [ ] Document language support

### Acceptance Criteria

- ✅ Auto-detect EN/FR/ZH/ES/JA
- ✅ CJK tokenization works correctly
- ✅ Language tag searchable
- ✅ Accuracy > 95% on test corpus

### Dependencies

- None (independent feature)

### Estimated Effort

- 8-10 hours

---

## Total Estimated Effort

- **Development:** ~120-140 hours (3-4 sprints)
- **Testing:** ~40 hours
- **Documentation:** ~20 hours
- **Total:** ~180-200 hours (12 weeks with 1-2 developers)

---

## Phase 6 GitHub Issues

To create tracking issues, use this template:

```markdown
**Title:** [Phase 6 M1] Create akidb-ingest CLI tool

**Description:**
Implement the `akidb-ingest` binary crate with CLI argument parsing and progress tracking.

**Tasks:**
- [ ] Create `services/akidb-ingest` crate
- [ ] Add clap dependency for CLI parsing
- [ ] Implement argument validation
- [ ] Add progress bar (indicatif)
- [ ] Add structured logging (tracing)
- [ ] Write CLI integration tests
- [ ] Add --help documentation

**Acceptance Criteria:**
- ✅ CLI binary builds successfully
- ✅ All arguments parsed and validated
- ✅ Progress bar shows during ingestion
- ✅ Errors logged with clear messages
- ✅ --help shows all available options

**Dependencies:** None

**Estimated Effort:** 10-12 hours

**Labels:** phase-6, milestone-1, enhancement
**Assignee:** TBD
**Milestone:** Phase 6 - Offline RAG
```

---

**Next Steps:**
1. Review this milestone plan with team
2. Create GitHub milestone "Phase 6 - Offline RAG"
3. Create 14 GitHub issues (one per milestone task)
4. Assign to developers
5. Begin M1 implementation
