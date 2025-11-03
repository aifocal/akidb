# Phase 6: Offline RAG & Air-Gap Features

**Target:** Q2 2025 (12 weeks)
**Status:** Planning
**Dependencies:** Phase 4 (Observability) - 60% Complete

---

## Executive Summary

Phase 6 transforms AkiDB into a fully offline-capable, air-gapped vector database for sovereign deployments. The goal is to enable **100% offline operation** from installation to production, with multi-site replication and portable package management.

### Success Criteria

- ✅ Install and run AkiDB without internet access
- ✅ Ingest 100M+ vectors from local files (CSV/JSONL/Parquet)
- ✅ Package and migrate collections across air-gapped sites
- ✅ Replicate data across geographic regions using MinIO Site Replication
- ✅ Support 5 languages (EN/FR/ZH/ES/JA) for document processing

### Business Impact

| Capability | Before Phase 6 | After Phase 6 | Value |
|------------|----------------|---------------|-------|
| **Air-Gap Installation** | Requires internet | Fully offline bundle | Government/defense contracts |
| **Data Migration** | Manual S3 sync | `.akipkg` snapshots | Cross-site deployment |
| **Multi-Site DR** | Custom scripts | MinIO Site Replication | Enterprise reliability |
| **Batch Ingest** | API-only (slow) | Parallel file import | 100x faster onboarding |
| **Multi-Language** | English only | 5 languages | Global deployments |

---

## 1. Offline Ingest System

### 1.1 Goals

Enable **zero-internet** bulk vector ingestion from local files with parallel processing and progress tracking.

### 1.2 Supported Formats

| Format | Use Case | Library | Priority |
|--------|----------|---------|----------|
| **CSV** | Simple exports, spreadsheets | `csv` crate | P0 |
| **JSONL** | MongoDB exports, streaming logs | `serde_json` | P0 |
| **Parquet** | Analytics, Spark/Pandas exports | `arrow-rs` | P1 |
| **HDF5** | Scientific computing, ML datasets | `hdf5` crate | P2 |

### 1.3 Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                   akidb-ingest CLI Tool                     │
│  Usage: akidb-ingest --collection=docs --file=vectors.csv   │
└─────────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────────┐
│              File Format Detection & Validation             │
│  • Detect: CSV/JSONL/Parquet via magic bytes + extension    │
│  • Validate: Schema, vector dimensions, required fields     │
└─────────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────────┐
│                 Parallel Chunk Processing                   │
│  • Split file into 10MB chunks                              │
│  • Parse in parallel (rayon thread pool)                    │
│  • Transform to VectorRecord { id, vector, payload }        │
└─────────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────────┐
│                    Batch Insert Pipeline                    │
│  • Buffer 10,000 vectors per batch                          │
│  • Write to WAL (crash recovery)                            │
│  • Flush to MinIO when segment reaches 100MB                │
│  • Progress bar with ETA (indicatif crate)                  │
└─────────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────────┐
│                  Post-Ingest Index Build                    │
│  • Seal segment (mark immutable)                            │
│  • Build HNSW index                                         │
│  • Update collection manifest                               │
└─────────────────────────────────────────────────────────────┘
```

### 1.4 Implementation Plan

**Milestone 1: CLI Tool (Week 1-2)**
- Create `services/akidb-ingest` binary crate
- Implement CLI argument parsing (clap)
- Add progress bar and logging (indicatif, tracing)

**Milestone 2: CSV/JSONL Parsers (Week 2-3)**
- Implement CSV parser with schema inference
- Implement JSONL streaming parser
- Add validation and error handling

**Milestone 3: Batch Pipeline (Week 3-4)**
- Integrate with akidb-storage WAL
- Implement chunked parallel processing
- Add crash recovery and resume from checkpoint

**Milestone 4: Parquet Support (Week 4)**
- Add Apache Arrow dependency
- Implement Parquet reader
- Benchmark vs CSV (expect 3-5x faster)

### 1.5 Example Usage

```bash
# Ingest 10M vectors from CSV (with progress bar)
akidb-ingest \
  --collection products \
  --file embeddings.csv \
  --id-column product_id \
  --vector-column embedding \
  --payload-columns name,price,category \
  --batch-size 10000 \
  --parallel 8

# Output:
# [████████████████████████████████] 10,000,000/10,000,000 vectors
# Ingested in 3m 24s (49,019 vectors/sec)
# Created 24 segments (2.4GB compressed)
# Building HNSW index... done in 1m 12s
```

---

## 2. .akipkg Package Format

### 2.1 Goals

Create a **portable, verifiable package** for migrating collections across air-gapped sites with cryptographic signatures.

### 2.2 Package Structure

```
products_v3.akipkg
├── manifest.json              # Metadata + signature
├── collection.json            # CollectionDescriptor
├── segments/
│   ├── seg_001.bin            # SEGv1 binary format (compressed)
│   ├── seg_002.bin
│   └── seg_003.bin
├── indices/
│   ├── seg_001.hnsw           # HNSW index files
│   ├── seg_002.hnsw
│   └── seg_003.hnsw
└── checksums.sha256           # File integrity hashes
```

### 2.3 Manifest Format

```json
{
  "version": "1.0",
  "collection_name": "products",
  "snapshot_version": 3,
  "created_at": "2025-02-15T10:30:00Z",
  "akidb_version": "0.4.0",
  "total_vectors": 10000000,
  "total_segments": 24,
  "compressed_size_bytes": 2400000000,
  "uncompressed_size_bytes": 12000000000,
  "vector_dim": 1536,
  "distance_metric": "Cosine",
  "signature": {
    "algorithm": "Ed25519",
    "public_key": "...",
    "signature": "..."
  }
}
```

### 2.4 Operations

```bash
# Export collection to .akipkg
akidb-pkg export \
  --collection products \
  --output products_v3.akipkg \
  --sign-key ./deploy.key

# Verify package integrity
akidb-pkg verify products_v3.akipkg

# Import to new site (air-gapped)
akidb-pkg import \
  --file products_v3.akipkg \
  --collection products_backup \
  --verify-signature

# List package contents
akidb-pkg inspect products_v3.akipkg
```

### 2.5 Implementation Plan

**Milestone 5: Package Format (Week 5-6)**
- Design akipkg TAR+Zstd structure
- Implement manifest serialization
- Add Ed25519 signature support (ring crate)

**Milestone 6: Export Pipeline (Week 6-7)**
- Stream segments from MinIO to TAR
- Compute SHA-256 checksums
- Sign manifest with private key

**Milestone 7: Import Pipeline (Week 7-8)**
- Verify signatures and checksums
- Extract and validate segments
- Import to MinIO with new collection name

---

## 3. MinIO Site Replication Integration

### 3.1 Goals

Leverage **MinIO Site Replication** for automatic multi-site data synchronization and disaster recovery.

### 3.2 Architecture

```
┌──────────────────────┐         ┌──────────────────────┐
│  Site A (Primary)    │         │  Site B (DR)         │
│  MinIO Cluster       │ ←─────→ │  MinIO Cluster       │
│  - Bucket: akidb     │  Async  │  - Bucket: akidb     │
│  - Region: us-west   │  Repl.  │  - Region: us-east   │
└──────────────────────┘         └──────────────────────┘
         ↑                                  ↑
         │                                  │
    ┌────┴────┐                        ┌────┴────┐
    │ AkiDB   │                        │ AkiDB   │
    │ Node A  │                        │ Node B  │
    └─────────┘                        └─────────┘
```

### 3.3 MinIO Replication Features

| Feature | Benefit for AkiDB |
|---------|-------------------|
| **Bi-Directional Sync** | Active-active deployments |
| **Versioning** | Point-in-time recovery |
| **Metadata Replication** | Object tags, user metadata preserved |
| **Bandwidth Control** | Rate limit for WAN links |
| **Encryption in Transit** | TLS for cross-site traffic |

### 3.4 Implementation Plan

**Milestone 8: Replication Setup Automation (Week 9)**
- Create `akidb-replication` CLI tool
- Generate MinIO replication configuration
- Automate site pairing and bucket setup

**Milestone 9: Monitoring Integration (Week 9-10)**
- Add Prometheus metrics for replication lag
- Alert on replication failures
- Dashboard for cross-site sync status

**Milestone 10: Failover Automation (Week 10)**
- Detect site failures via health checks
- Promote DR site to primary
- Update DNS/load balancer automatically

### 3.5 Example Usage

```bash
# Configure replication between two sites
akidb-replication setup \
  --primary https://minio-us-west.example.com \
  --dr https://minio-us-east.example.com \
  --bucket akidb \
  --bandwidth-limit 100MB/s

# Check replication status
akidb-replication status
# Output:
# Site A → Site B: 12.5GB replicated, 45s lag
# Site B → Site A: 8.2GB replicated, 30s lag

# Trigger failover to DR site
akidb-replication failover --to us-east
```

---

## 4. Air-Gap Tooling

### 4.1 Goals

Enable **installation and operation without internet** access.

### 4.2 Offline Installation Bundle

```
akidb-offline-bundle-v0.4.0-arm64.tar.gz
├── bin/
│   ├── akidb-api                # API server binary
│   ├── akidb-ingest             # Ingest tool
│   ├── akidb-pkg                # Package manager
│   └── akidb-replication        # Replication tool
├── deps/
│   ├── minio-server             # MinIO binary
│   └── mc                       # MinIO client
├── configs/
│   ├── akidb.toml.example       # Example config
│   └── minio.env.example        # MinIO env vars
├── scripts/
│   ├── install.sh               # Offline installer
│   ├── start-services.sh        # Systemd/launchd setup
│   └── health-check.sh          # Verification
└── docs/
    └── offline-install-guide.md # Step-by-step instructions
```

### 4.3 Implementation Plan

**Milestone 11: Bundle Creation (Week 11)**
- Script to download all dependencies
- Create TAR bundle with SHA-256 checksums
- Test installation on clean ARM systems

**Milestone 12: Dependency Management (Week 11-12)**
- Vendor all Rust dependencies (cargo vendor)
- Include MinIO binaries (Linux ARM64, macOS ARM64)
- Add license compliance check

---

## 5. Multi-Language Document Processing

### 5.1 Goals

Support **5 languages** for text processing and semantic search.

### 5.2 Language Support Matrix

| Language | ISO 639-1 | Tokenizer | Embedding Model | Priority |
|----------|-----------|-----------|-----------------|----------|
| English | en | whitespace + stemming | sentence-transformers | P0 |
| French | fr | whitespace + stemming | CamemBERT | P1 |
| Chinese | zh | jieba segmentation | ernie-3.0 | P1 |
| Spanish | es | whitespace + stemming | BETO | P2 |
| Japanese | ja | mecab tokenization | BERT-base-ja | P2 |

### 5.3 Implementation Plan

**Milestone 13: Language Detection (Week 12)**
- Integrate whichlang or lingua-rs
- Auto-detect language in payload text
- Store language tag in metadata

**Milestone 14: Tokenization Pipeline (Week 12)**
- Add language-specific tokenizers
- Support for CJK (Chinese, Japanese, Korean)
- Normalize text (lowercase, accents, etc.)

---

## 6. Testing Strategy

### 6.1 Integration Tests

```rust
#[test]
#[ignore] // Requires large test files
fn test_csv_ingest_10m_vectors() {
    // Generate 10M vector CSV file
    // Ingest via akidb-ingest
    // Verify all vectors searchable
    // Assert < 5 min ingestion time
}

#[test]
fn test_akipkg_export_import_roundtrip() {
    // Create collection with 100K vectors
    // Export to .akipkg
    // Import to new collection
    // Verify identical search results
}

#[test]
#[ignore] // Requires multi-site MinIO
fn test_site_replication_sync() {
    // Insert vectors to Site A
    // Wait for replication to Site B
    // Query Site B, verify results
    // Measure replication lag
}
```

### 6.2 Performance Benchmarks

| Benchmark | Target | Measurement |
|-----------|--------|-------------|
| CSV Ingest (1M vectors, 768-dim) | < 60s | Throughput (vectors/sec) |
| JSONL Ingest (1M vectors) | < 90s | Throughput (vectors/sec) |
| Parquet Ingest (1M vectors) | < 30s | Throughput (3x faster) |
| .akipkg Export (100K vectors) | < 10s | Compression ratio |
| .akipkg Import (100K vectors) | < 15s | Decompression + validation |
| Site Replication (1GB data) | < 5 min lag | Replication delay |

---

## 7. Documentation Deliverables

1. **User Guides**
   - Offline Installation Guide
   - Bulk Ingest Tutorial (CSV/JSONL/Parquet)
   - Package Migration Guide (.akipkg)
   - Multi-Site Replication Setup

2. **API References**
   - Ingest CLI options
   - Package CLI options
   - Replication API endpoints

3. **Runbooks**
   - Air-Gap Deployment Checklist
   - DR Failover Procedures
   - Troubleshooting Common Issues

---

## 8. Risks & Mitigations

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| **Large file parsing OOM** | High | Medium | Streaming parsers, chunk processing |
| **Corrupted .akipkg imports** | High | Low | Checksum verification, atomic imports |
| **Replication lag > 1 hour** | Medium | Medium | Bandwidth monitoring, alerting |
| **Language detection accuracy** | Low | Medium | Manual language override option |
| **Dependency version conflicts** | High | Low | Vendor all deps, lockfile |

---

## 9. Success Metrics

### 9.1 Functional Metrics

- ✅ 100% offline installation success rate
- ✅ Ingest 100M vectors in < 2 hours
- ✅ Package export/import < 1% error rate
- ✅ Site replication lag < 5 minutes (P99)

### 9.2 Performance Metrics

- CSV ingest: 50,000 vectors/sec (target)
- Parquet ingest: 150,000 vectors/sec (target)
- Package export: 100MB/s throughput
- Package import: 80MB/s throughput

### 9.3 Operational Metrics

- Zero internet access required post-install
- Cross-site migration < 30 minutes (1GB collection)
- DR failover < 5 minutes (RTO)

---

## 10. Timeline

| Week | Milestone | Deliverable |
|------|-----------|-------------|
| 1-2  | M1: CLI Tool | `akidb-ingest` binary |
| 2-3  | M2: CSV/JSONL | Parser implementation |
| 3-4  | M3: Batch Pipeline | Integration with WAL |
| 4    | M4: Parquet | Parquet reader + benchmarks |
| 5-6  | M5: Package Format | `.akipkg` structure |
| 6-7  | M6: Export | Package export tool |
| 7-8  | M7: Import | Package import tool |
| 9    | M8: Replication Setup | MinIO replication config |
| 9-10 | M9: Monitoring | Prometheus metrics |
| 10   | M10: Failover | DR automation |
| 11   | M11: Bundle | Offline installer |
| 11-12| M12: Dependencies | Vendor deps, licenses |
| 12   | M13-14: Multi-Language | Language detection, tokenization |

**Total Duration:** 12 weeks (Q2 2025)

---

## 11. Dependencies

### 11.1 Phase 4 Completion Required

- ✅ OpenTelemetry tracing (for debugging ingest pipeline)
- ✅ Prometheus metrics (for monitoring replication lag)

### 11.2 External Libraries

```toml
# Add to Cargo.toml workspace dependencies
csv = "1.3"                          # CSV parsing
arrow = "50.0"                       # Parquet support
tar = "0.4"                          # TAR archive creation
zstd = "0.13"                        # Compression
ring = "0.17"                        # Ed25519 signatures
sha2 = "0.10"                        # SHA-256 checksums (already present)
indicatif = "0.17"                   # Progress bars
clap = { version = "4.5", features = ["derive"] }  # CLI parsing
whichlang = "0.1"                    # Language detection
```

---

## 12. Next Steps

1. **Review & Approval** (Week 0)
   - Architecture review with team
   - Security audit of .akipkg format
   - MinIO replication POC

2. **Kickoff** (Week 1)
   - Create feature branch: `feature/phase6-offline-rag`
   - Setup CI jobs for integration tests
   - Create tracking issues in GitHub

3. **Development** (Week 1-12)
   - Follow milestone timeline
   - Weekly demos to stakeholders
   - Continuous benchmarking

4. **Release** (End of Week 12)
   - Tag v0.4.0-beta
   - Publish offline bundle
   - Write blog post + case studies

---

## 13. Open Questions

1. **Q:** Should .akipkg support incremental updates (delta packages)?
   **A:** Defer to Phase 7. For Phase 6, full snapshots only.

2. **Q:** How to handle schema evolution in .akipkg format?
   **A:** Use semantic versioning in manifest. Reject imports if incompatible.

3. **Q:** Support for custom embedding models in .akipkg?
   **A:** Yes, include model files in `models/` directory. Add to M13.

4. **Q:** MinIO replication for multi-region writes (active-active)?
   **A:** Supported by MinIO. Add conflict resolution strategy in M8.

5. **Q:** HDF5 vs Parquet for scientific datasets?
   **A:** Both. HDF5 is P2 (Week 12 if time permits).

---

**Document Version:** 1.0
**Last Updated:** 2025-11-03
**Owner:** AkiDB Team
**Reviewers:** TBD
