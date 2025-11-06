# AkiDB 2.0 Comprehensive Testing Plan: macOS MLX

**Platform:** macOS with Apple Silicon (M1/M2/M3 Max/Ultra)
**Framework:** MLX (Apple's ML framework for Metal acceleration)
**Timeline:** 16 weeks (Nov 2025 - Feb 2026)
**Owner:** QA Engineering Team + Platform Engineering

---

## Executive Summary

This comprehensive testing plan covers all aspects of AkiDB 2.0 on macOS with Apple MLX, leveraging Apple Silicon's unified memory architecture and Metal GPU acceleration. NVIDIA Jetson and Oracle ARM Cloud testing will be handled by separate projects.

### Why MLX-First Strategy

**Strategic Advantages:**
1. **Unified Memory Architecture**: 32-128GB shared between CPU and GPU (no PCIe bottleneck)
2. **Metal GPU Acceleration**: Native Apple GPU via MLX (optimized for embeddings)
3. **NEON SIMD**: ARM NEON intrinsics for HNSW vector operations
4. **Developer Accessibility**: Mac ARM machines are standard dev hardware
5. **Simplified Deployment**: No external GPU dependencies, single binary

**MLX vs Alternatives:**
| Framework | Platform | Performance | Complexity | Decision |
|-----------|----------|-------------|------------|----------|
| **MLX** | Mac ARM | ✅ Optimized | ✅ Simple | ✅ Chosen for Phase 1 |
| ONNX | Cross-platform | ⚠️ Generic | ⚠️ Medium | Defer to Phase 2 |
| TensorRT | NVIDIA only | ❌ N/A on Mac | ❌ Complex | Defer to Jetson project |

---

## Test Environment Specifications

### Hardware Requirements

**Primary Test Bed (Recommended):**
- **Model:** MacBook Pro M2 Max or M3 Max
- **CPU:** 12-core ARM (8 performance + 4 efficiency cores)
- **GPU:** 38-core Metal GPU (M2 Max) or 40-core (M3 Max)
- **Memory:** 64GB unified memory (minimum 32GB)
- **Storage:** 1TB NVMe SSD (APFS)
- **OS:** macOS 14.6+ (Sonoma) or macOS 15+ (Sequoia)

**Secondary Test Bed (Budget):**
- **Model:** Mac Mini M2 Pro
- **CPU:** 10-core ARM
- **GPU:** 16-core Metal GPU
- **Memory:** 32GB unified memory
- **Storage:** 512GB NVMe SSD

**Stress Test Bed (High-End):**
- **Model:** Mac Studio M2 Ultra
- **CPU:** 24-core ARM (16 performance + 8 efficiency)
- **GPU:** 76-core Metal GPU
- **Memory:** 128GB unified memory
- **Storage:** 2TB NVMe SSD

### Software Stack

**Core Dependencies:**
```toml
# Cargo.toml additions for MLX
[dependencies]
mlx-rs = "0.1"  # Rust bindings for MLX
metal = "0.27"  # Apple Metal framework
tokio = { version = "1.35", features = ["full"] }
sqlx = { version = "0.7", features = ["sqlite", "runtime-tokio-rustls"] }
cedar-policy = "4.0"
tonic = "0.11"  # gRPC
axum = "0.7"    # REST API
```

**MLX Installation:**
```bash
# Install MLX framework
pip install mlx

# Install MLX Rust bindings (if not in crates.io)
cargo install mlx-rs --git https://github.com/ml-explore/mlx-rs
```

**System Tools:**
```bash
brew install hyperfine wrk vegeta   # Performance testing
brew install prometheus grafana      # Observability
brew install minio/stable/minio     # S3-compatible storage
```

---

## Testing Strategy Overview

### Test Pyramid

```
           /\
          /  \    E2E Tests (5%)
         /____\   - User scenarios, full stack
        /      \
       /  Inte  \ Integration Tests (15%)
      /  gration\ - Component interactions
     /___________\
    /             \ Unit Tests (80%)
   /   Unit Tests  \ - Functions, modules
  /__________________\
```

**Coverage Targets:**
- Unit Tests: 80% code coverage
- Integration Tests: 100% API endpoints
- E2E Tests: 100% critical user journeys
- Performance Tests: P95 < 25ms query latency
- Chaos Tests: 99.9% availability under failure

---

## 1. Unit Testing (80% Coverage Target)

### 1.1 Core Domain Logic (`akidb-core`)

**Test Files:**
```rust
// tests/tenant_test.rs
#[cfg(test)]
mod tenant_tests {
    use akidb_core::tenant::{TenantDescriptor, TenantQuota};

    #[test]
    fn test_tenant_creation() {
        let tenant = TenantDescriptor::new("test-tenant", TenantQuota::default());
        assert_eq!(tenant.name, "test-tenant");
        assert_eq!(tenant.status, TenantStatus::Active);
    }

    #[test]
    fn test_quota_enforcement() {
        let mut tenant = TenantDescriptor::new("test", TenantQuota::default());
        tenant.quotas.max_storage_bytes = 1_000_000;

        assert!(tenant.can_allocate(500_000).is_ok());
        assert!(tenant.can_allocate(1_500_000).is_err());  // Exceeds quota
    }
}

// tests/collection_test.rs
#[cfg(test)]
mod collection_tests {
    use akidb_core::collection::{CollectionDescriptor, HnswParams};

    #[test]
    fn test_collection_with_database_hierarchy() {
        let collection = CollectionDescriptor::new(
            "test-collection",
            database_id,  // NEW: database_id field
            512,          // vector_dim
            DistanceMetric::Cosine,
        );
        assert_eq!(collection.database_id, database_id);
    }

    #[test]
    fn test_hnsw_param_validation() {
        let params = HnswParams { M: 16, ef_construction: 200 };
        assert!(params.validate().is_ok());

        let invalid_params = HnswParams { M: 0, ef_construction: 10 };
        assert!(invalid_params.validate().is_err());  // M must be > 0
    }
}
```

**Coverage Areas:**
- [x] Tenant CRUD operations
- [x] Quota enforcement logic
- [x] Collection descriptor with database_id
- [x] HNSW parameter validation
- [x] User/Role/Permission structures
- [x] ID generation (UUIDv7)

---

### 1.2 SQLite Metadata Store (`akidb-metadata`)

**Test Strategy:**
```rust
// tests/metadata_store_test.rs
use sqlx::SqlitePool;
use akidb_metadata::MetadataStore;

#[sqlx::test]
async fn test_create_tenant_transaction() {
    let pool = SqlitePool::connect(":memory:").await.unwrap();
    let store = MetadataStore::new(pool).await.unwrap();

    let tenant = store.create_tenant("test-tenant", TenantQuota::default()).await.unwrap();
    assert_eq!(tenant.name, "test-tenant");

    // Verify foreign key constraints
    let database = store.create_database(tenant.id, "test-db").await.unwrap();
    assert_eq!(database.tenant_id, tenant.id);
}

#[sqlx::test]
async fn test_fts5_search() {
    let store = setup_metadata_store().await;

    // Insert tenants with searchable names
    store.create_tenant("healthcare-platform", TenantQuota::default()).await.unwrap();
    store.create_tenant("fintech-analytics", TenantQuota::default()).await.unwrap();

    // FTS5 search
    let results = store.search_tenants("healthcare").await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "healthcare-platform");
}

#[sqlx::test]
async fn test_metadata_crash_recovery() {
    let pool = SqlitePool::connect("test.db").await.unwrap();
    let store = MetadataStore::new(pool).await.unwrap();

    store.create_tenant("tenant-1", TenantQuota::default()).await.unwrap();

    // Simulate crash (drop connection without commit)
    drop(store);

    // Reopen and verify WAL recovery
    let pool2 = SqlitePool::connect("test.db").await.unwrap();
    let store2 = MetadataStore::new(pool2).await.unwrap();

    let tenants = store2.list_tenants().await.unwrap();
    assert_eq!(tenants.len(), 1);  // WAL replay successful
}
```

**Coverage Areas:**
- [x] CRUD operations (Create, Read, Update, Delete)
- [x] Transaction isolation (ACID guarantees)
- [x] Foreign key constraints
- [x] FTS5 full-text search
- [x] Schema migrations (forward/backward)
- [x] WAL crash recovery
- [x] Concurrent access (1 writer + N readers)

---

### 1.3 MLX Embedding Service (`akidb-embed`)

**Test Strategy:**
```rust
// tests/mlx_embedding_test.rs
use akidb_embed::{MLXEmbeddingService, EmbedRequest};

#[tokio::test]
async fn test_mlx_model_loading() {
    let service = MLXEmbeddingService::new("mlx-community/qwen3-embedding-8b-int8").await.unwrap();
    assert!(service.is_ready());
}

#[tokio::test]
async fn test_single_text_embedding() {
    let service = setup_mlx_service().await;

    let req = EmbedRequest {
        texts: vec!["Hello, world!".to_string()],
        normalize: true,
    };

    let response = service.embed(req).await.unwrap();
    assert_eq!(response.embeddings.len(), 1);
    assert_eq!(response.embeddings[0].len(), 512);  // 512-dim embeddings

    // Verify normalization (L2 norm ≈ 1.0)
    let norm: f32 = response.embeddings[0].iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 0.01);
}

#[tokio::test]
async fn test_batch_embedding_throughput() {
    let service = setup_mlx_service().await;

    // Generate 1000 texts
    let texts: Vec<String> = (0..1000).map(|i| format!("Test text {}", i)).collect();

    let start = Instant::now();
    let response = service.embed_batch(texts, 100).await.unwrap();  // batch_size=100
    let elapsed = start.elapsed();

    assert_eq!(response.embeddings.len(), 1000);

    // Target: >200 embeddings/sec on M2 Max
    let throughput = 1000.0 / elapsed.as_secs_f32();
    assert!(throughput > 200.0, "Throughput: {} vec/sec", throughput);
}

#[tokio::test]
async fn test_mlx_metal_acceleration() {
    let service = setup_mlx_service().await;

    // Verify Metal GPU is being used (not CPU fallback)
    let device_info = service.get_device_info();
    assert_eq!(device_info.backend, "metal");
    assert!(device_info.gpu_cores > 0);
}

#[tokio::test]
async fn test_quantization_accuracy() {
    let service_int8 = MLXEmbeddingService::new("model-int8").await.unwrap();
    let service_fp16 = MLXEmbeddingService::new("model-fp16").await.unwrap();

    let text = "Machine learning on Apple Silicon";

    let emb_int8 = service_int8.embed_text(text).await.unwrap();
    let emb_fp16 = service_fp16.embed_text(text).await.unwrap();

    // Cosine similarity should be >0.98 (int8 quantization loss <2%)
    let similarity = cosine_similarity(&emb_int8, &emb_fp16);
    assert!(similarity > 0.98, "Quantization degradation too high: {}", 1.0 - similarity);
}
```

**Coverage Areas:**
- [x] MLX model loading and initialization
- [x] Single text embedding
- [x] Batch embedding with throughput target (>200 vec/sec)
- [x] Metal GPU acceleration verification
- [x] Int8 quantization accuracy (<2% loss)
- [x] Memory management (unified memory usage)
- [x] Error handling (model not found, OOM)

---

### 1.4 HNSW Index with NEON SIMD (`akidb-index`)

**Test Strategy:**
```rust
// tests/hnsw_neon_test.rs
use akidb_index::hnsw::{HnswIndex, DistanceMetric};
use std::arch::aarch64::*;  // ARM NEON intrinsics

#[test]
fn test_hnsw_build_on_arm() {
    let vectors: Vec<Vec<f32>> = generate_random_vectors(10_000, 512);
    let params = HnswParams { M: 16, ef_construction: 200 };

    let index = HnswIndex::build(&vectors, DistanceMetric::Cosine, params).unwrap();

    assert_eq!(index.num_vectors(), 10_000);
    assert_eq!(index.dimension(), 512);
}

#[test]
fn test_neon_simd_distance() {
    // Verify NEON SIMD is used for distance calculations
    let v1: Vec<f32> = vec![1.0; 512];
    let v2: Vec<f32> = vec![0.5; 512];

    let distance = akidb_index::distance::cosine_neon(&v1, &v2);

    // NEON should compute same result as scalar
    let distance_scalar = akidb_index::distance::cosine_scalar(&v1, &v2);
    assert!((distance - distance_scalar).abs() < 1e-6);
}

#[test]
fn test_hnsw_query_performance() {
    let index = build_test_index(1_000_000, 512);  // 1M vectors
    let query = generate_random_vector(512);

    let start = Instant::now();
    let results = index.search(&query, 10, 64).unwrap();  // top-10, ef_search=64
    let elapsed = start.elapsed();

    assert_eq!(results.len(), 10);
    assert!(elapsed.as_millis() < 25, "Query too slow: {}ms", elapsed.as_millis());  // P95 <25ms
}

#[test]
fn test_hnsw_incremental_insert() {
    let mut index = HnswIndex::new(512, DistanceMetric::Cosine, HnswParams::default());

    // Insert vectors one by one
    for i in 0..1000 {
        let vector = generate_random_vector(512);
        index.insert(i, &vector).unwrap();
    }

    assert_eq!(index.num_vectors(), 1000);

    // Verify searchability after incremental inserts
    let query = generate_random_vector(512);
    let results = index.search(&query, 10, 64).unwrap();
    assert_eq!(results.len(), 10);
}
```

**Coverage Areas:**
- [x] HNSW index build (10k, 100k, 1M vectors)
- [x] NEON SIMD distance calculations
- [x] Query performance (P95 <25ms @ 1M vectors)
- [x] Incremental insert performance
- [x] Memory-mapped index files (APFS)
- [x] Concurrent read access (multi-threaded queries)
- [x] Index serialization/deserialization

---

### 1.5 Cedar Policy Engine (`akidb-core::user`)

**Test Strategy:**
```rust
// tests/cedar_policy_test.rs
use akidb_core::user::{PolicyEngine, AuthzRequest};
use cedar_policy::{PolicySet, Entities, Context};

#[tokio::test]
async fn test_tenant_admin_policy() {
    let engine = setup_policy_engine().await;

    let req = AuthzRequest {
        principal: "User::tenant-alpha-admin".to_string(),
        action: "Action::collection::delete".to_string(),
        resource: "Collection::tenant-alpha#db-1#coll-99".to_string(),
        context: json!({"tenant": "tenant-alpha"}),
    };

    let decision = engine.is_authorized(req).await.unwrap();
    assert_eq!(decision.decision, Decision::Allow);
}

#[tokio::test]
async fn test_tenant_isolation() {
    let engine = setup_policy_engine().await;

    // User from tenant-alpha tries to access tenant-beta resource
    let req = AuthzRequest {
        principal: "User::tenant-alpha-user-1".to_string(),
        action: "Action::collection::read".to_string(),
        resource: "Collection::tenant-beta#db-1#coll-1".to_string(),  // Different tenant!
        context: json!({}),
    };

    let decision = engine.is_authorized(req).await.unwrap();
    assert_eq!(decision.decision, Decision::Deny);  // Tenant isolation enforced
    assert!(decision.reasons.contains(&"tenant mismatch".to_string()));
}

#[tokio::test]
async fn test_policy_evaluation_latency() {
    let engine = setup_policy_engine_with_10k_policies().await;

    let req = AuthzRequest {
        principal: "User::test".to_string(),
        action: "Action::collection::read".to_string(),
        resource: "Collection::test#db#coll".to_string(),
        context: json!({}),
    };

    // Benchmark 1000 evaluations
    let start = Instant::now();
    for _ in 0..1000 {
        engine.is_authorized(req.clone()).await.unwrap();
    }
    let elapsed = start.elapsed();

    let avg_latency = elapsed.as_micros() / 1000;
    assert!(avg_latency < 5000, "Cedar P99 too high: {}µs", avg_latency);  // Target: <5ms
}
```

**Coverage Areas:**
- [x] Admin, developer, viewer, auditor roles
- [x] Tenant isolation enforcement
- [x] Policy evaluation latency (<5ms P99 with 10k policies)
- [x] Attribute-based policies (user.tier, resource.sensitivity)
- [x] Deny rules override allow rules
- [x] Policy cache invalidation
- [x] Audit logging integration

---

## 2. Integration Testing (15% of Tests)

### 2.1 End-to-End API Testing

**Test Strategy:**
```rust
// tests/integration/api_test.rs
use akidb_api::ApiServer;
use reqwest::Client;

#[tokio::test]
async fn test_create_tenant_via_rest() {
    let server = start_test_server().await;
    let client = Client::new();

    let response = client
        .post(&format!("{}/api/v2/tenants", server.url()))
        .json(&json!({
            "name": "test-tenant",
            "quotas": {"max_storage_bytes": 10_000_000_000}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 201);

    let tenant: TenantDescriptor = response.json().await.unwrap();
    assert_eq!(tenant.name, "test-tenant");
}

#[tokio::test]
async fn test_ingest_with_mlx_embeddings() {
    let server = start_test_server().await;
    let client = Client::new();

    // Create tenant and collection
    let tenant = create_test_tenant(&client, &server).await;
    let collection = create_test_collection(&client, &server, tenant.id).await;

    // Ingest documents with automatic embeddings
    let response = client
        .post(&format!("{}/api/v2/collections/{}/ingest", server.url(), collection.id))
        .json(&json!({
            "documents": [
                {"id": "doc-1", "text": "Apple Silicon is fast", "metadata": {"category": "tech"}},
                {"id": "doc-2", "text": "MLX accelerates ML workloads", "metadata": {"category": "ml"}},
            ],
            "embed": true  // Trigger MLX embedding service
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let ingest_result: IngestResponse = response.json().await.unwrap();
    assert_eq!(ingest_result.ingested_count, 2);
    assert!(ingest_result.embedding_latency_ms > 0.0);  // Embeddings were computed
}

#[tokio::test]
async fn test_hybrid_search() {
    let (server, client, collection_id) = setup_test_collection_with_data().await;

    // Query with vector + metadata filter
    let response = client
        .post(&format!("{}/api/v2/collections/{}/query", server.url(), collection_id))
        .json(&json!({
            "query_text": "machine learning on Apple",  // Will be embedded via MLX
            "top_k": 10,
            "filter": {"category": "ml"}  // Metadata filter
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let query_result: QueryResponse = response.json().await.unwrap();
    assert_eq!(query_result.matches.len(), 10);
    assert!(query_result.latency_ms < 25.0);  // P95 <25ms target
}
```

**Coverage Areas:**
- [x] REST API (v1 backward compat + v2 new endpoints)
- [x] gRPC API (data plane operations)
- [x] Tenant/Database/Collection CRUD
- [x] Ingest with MLX embeddings
- [x] Query (simple, filtered, hybrid)
- [x] User/RBAC management
- [x] Error handling (400, 401, 403, 404, 500)

---

### 2.2 Storage Layer Integration

**Test Strategy:**
```rust
// tests/integration/storage_test.rs
use akidb_storage::{StorageEngine, WalWriter};

#[tokio::test]
async fn test_wal_write_and_replay() {
    let storage = StorageEngine::new("test-storage").await.unwrap();

    // Write 1000 vectors to WAL
    for i in 0..1000 {
        let vector = generate_random_vector(512);
        storage.write_vector(collection_id, i, &vector).await.unwrap();
    }

    // Simulate crash
    drop(storage);

    // Reopen and replay WAL
    let storage2 = StorageEngine::open("test-storage").await.unwrap();
    let vectors = storage2.read_collection(collection_id).await.unwrap();

    assert_eq!(vectors.len(), 1000);  // All vectors recovered
}

#[tokio::test]
async fn test_s3_minio_sync() {
    let storage = StorageEngine::new_with_s3("s3://minio:9000/akidb-test").await.unwrap();

    // Write to local WAL
    storage.write_vector(collection_id, 1, &vec![1.0; 512]).await.unwrap();

    // Trigger S3 sync
    storage.sync_to_s3().await.unwrap();

    // Verify S3 object exists
    let s3_client = setup_minio_client();
    let object = s3_client.get_object("akidb-test", &format!("collection-{}/wal-0001.bin", collection_id)).await.unwrap();
    assert!(object.size > 0);
}

#[tokio::test]
async fn test_ram_first_tiering() {
    let storage = StorageEngine::new_with_tiering(TieringConfig {
        ram_tier_size: 1_000_000_000,  // 1GB
        disk_tier_path: "/var/akidb/disk".to_string(),
    }).await.unwrap();

    // Ingest 2GB of vectors (exceeds RAM tier)
    for i in 0..4_000_000 {  // 4M vectors × 512 dims × 4 bytes ≈ 8GB
        storage.write_vector(collection_id, i, &generate_random_vector(512)).await.unwrap();
    }

    // Verify hot vectors in RAM, cold vectors on disk
    let stats = storage.get_tier_stats().await.unwrap();
    assert!(stats.ram_tier_bytes > 0);
    assert!(stats.disk_tier_bytes > 0);
    assert_eq!(stats.ram_tier_bytes + stats.disk_tier_bytes, stats.total_bytes);
}
```

**Coverage Areas:**
- [x] WAL write and crash recovery
- [x] S3/MinIO sync
- [x] RAM-first tiering (hot/cold data)
- [x] Snapshot creation and restore
- [x] APFS-optimized file I/O
- [x] Concurrent writes (1 writer, N readers)

---

## 3. End-to-End Testing (5% of Tests)

### 3.1 Critical User Journeys

**Journey 1: New User Onboarding**
```gherkin
Feature: New User Onboarding on macOS MLX

Scenario: Developer sets up first vector collection
  Given I have installed AkiDB 2.0 on Mac ARM
  When I run "akidb init --tenant my-startup"
  Then I should see "Tenant 'my-startup' created"

  When I run "akidb database create --name projects"
  Then I should see "Database 'projects' created"

  When I run "akidb collection create --name documents --dim 512"
  Then I should see "Collection 'documents' created with HNSW index"

  When I ingest 1000 documents with "akidb ingest --embed mlx documents.csv"
  Then I should see "Ingested 1000 documents (MLX embeddings: 4.2s)"

  When I query with "akidb query --text 'machine learning' --top-k 10"
  Then I should see 10 results in < 25ms
```

**Journey 2: Multi-Tenant Isolation**
```gherkin
Scenario: Ensure tenant A cannot access tenant B data
  Given Tenant "healthcare" with collection "patient-records"
  And Tenant "fintech" with collection "transactions"

  When User "healthcare-admin" queries "patient-records"
  Then Query succeeds

  When User "healthcare-admin" queries "transactions" (from fintech)
  Then Query fails with 403 Forbidden
  And Audit log shows "tenant isolation violation"
```

**Journey 3: Embedding Service with MLX**
```gherkin
Scenario: Ingest documents with automatic MLX embeddings
  Given Collection "articles" with dimension 512

  When I POST to /api/v2/collections/articles/ingest
  With JSON:
    {
      "documents": [
        {"id": "1", "text": "Apple M3 Max performance"},
        {"id": "2", "text": "MLX framework benchmarks"}
      ],
      "embed": true
    }

  Then Response status is 200
  And Response contains:
    {
      "ingested_count": 2,
      "embedding_latency_ms": 18.5,
      "embedding_backend": "mlx-metal"
    }

  When I query with text "M3 benchmarks"
  Then Document "2" is ranked #1
  And Query latency < 20ms
```

### 3.2 E2E Test Automation

**Test Runner:**
```rust
// tests/e2e/user_journeys.rs
use akidb_test_harness::TestCluster;

#[tokio::test]
async fn test_new_user_onboarding_journey() {
    let cluster = TestCluster::start().await.unwrap();

    // Step 1: Create tenant
    let tenant = cluster.create_tenant("my-startup").await.unwrap();
    assert_eq!(tenant.status, TenantStatus::Active);

    // Step 2: Create database
    let database = cluster.create_database(tenant.id, "projects").await.unwrap();

    // Step 3: Create collection
    let collection = cluster.create_collection(database.id, CollectionConfig {
        name: "documents".to_string(),
        vector_dim: 512,
        distance: DistanceMetric::Cosine,
        hnsw_params: HnswParams { M: 16, ef_construction: 200 },
    }).await.unwrap();

    // Step 4: Ingest with MLX embeddings
    let documents = load_test_documents("fixtures/documents_1000.csv");
    let ingest_result = cluster.ingest_with_embeddings(collection.id, documents).await.unwrap();

    assert_eq!(ingest_result.ingested_count, 1000);
    assert!(ingest_result.embedding_latency_ms < 5000.0);  // <5s for 1000 docs
    assert_eq!(ingest_result.embedding_backend, "mlx-metal");

    // Step 5: Query
    let query_result = cluster.query(collection.id, QueryRequest {
        query_text: Some("machine learning".to_string()),
        top_k: 10,
        filter: None,
    }).await.unwrap();

    assert_eq!(query_result.matches.len(), 10);
    assert!(query_result.latency_ms < 25.0);

    cluster.shutdown().await.unwrap();
}
```

**Coverage Areas:**
- [x] New user onboarding (tenant → database → collection → ingest → query)
- [x] Multi-tenant isolation enforcement
- [x] MLX embedding service end-to-end
- [x] Hybrid search (vector + metadata)
- [x] RBAC policy enforcement
- [x] Crash recovery
- [x] Performance under load

---

## 4. Performance Testing

### 4.1 Benchmark Suite

**Ingest Throughput:**
```rust
// benchmarks/ingest_throughput.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_mlx_embedding_throughput(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let service = rt.block_on(setup_mlx_service());

    c.bench_function("mlx_embed_1000_docs", |b| {
        b.iter(|| {
            let texts: Vec<String> = (0..1000).map(|i| format!("Test document {}", i)).collect();
            rt.block_on(service.embed_batch(black_box(texts), 100))
        });
    });
}

fn bench_hnsw_insert(c: &mut Criterion) {
    let mut index = HnswIndex::new(512, DistanceMetric::Cosine, HnswParams::default());

    c.bench_function("hnsw_insert_1000_vectors", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let vector = generate_random_vector(512);
                index.insert(black_box(i), &vector).unwrap();
            }
        });
    });
}

criterion_group!(benches, bench_mlx_embedding_throughput, bench_hnsw_insert);
criterion_main!(benches);
```

**Query Latency:**
```bash
# Run with hyperfine
hyperfine --warmup 10 --runs 1000 \
  --export-json results/query-latency-mlx.json \
  'akidb query --collection test --vector-file queries.json --top-k 10'

# Extract percentiles
jq '.results[0].times | [min, (length*0.5|floor) as $p50 | sort|.[$p50], (length*0.95|floor) as $p95 | sort|.[$p95], (length*0.99|floor) as $p99 | sort|.[$p99], max]' results/query-latency-mlx.json
```

### 4.2 Load Testing (100 QPS Sustained)

**Scenario:**
```bash
# Install vegeta
brew install vegeta

# Generate request payload
cat > targets.txt <<EOF
POST http://localhost:8080/api/v2/collections/{collection_id}/query
Content-Type: application/json
@query_payload.json

EOF

# Run 100 QPS for 5 minutes
echo "POST http://localhost:8080/api/v2/collections/test/query" | \
  vegeta attack -rate=100 -duration=5m -body=query_payload.json | \
  vegeta report -type=text

# Expected output:
# Requests      [total, rate, throughput]  30000, 100.00, 99.97
# Duration      [total, attack, wait]      5m0s, 5m0s, 12.3ms
# Latencies     [mean, 50, 95, 99, max]    18.2ms, 16.5ms, 24.1ms, 31.2ms, 102ms
# Success       [ratio]                     100.00%
```

### 4.3 Stress Testing (Resource Exhaustion)

**Test Scenarios:**
1. **Memory Pressure**: Ingest until RAM tier full, verify disk spillover
2. **CPU Saturation**: 100% CPU load with concurrent queries
3. **Disk I/O**: Sustained writes to S3/MinIO backend
4. **Network**: Simulate S3 latency/throttling

```rust
// tests/stress/memory_exhaustion.rs
#[tokio::test]
async fn test_memory_exhaustion_graceful_degradation() {
    let cluster = TestCluster::start_with_limits(MemoryLimit::MB(4096)).await;  // 4GB RAM limit

    // Ingest 10M vectors (would require ~20GB RAM)
    let mut ingested = 0;
    let mut oom_handled = false;

    for batch in 0..100 {
        let vectors = generate_random_vectors(100_000, 512);
        match cluster.ingest_vectors(collection_id, vectors).await {
            Ok(_) => ingested += 100_000,
            Err(Error::OutOfMemory) => {
                oom_handled = true;
                break;  // Graceful OOM handling
            }
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }

    // Should have ingested some data before hitting OOM
    assert!(ingested > 1_000_000);
    assert!(oom_handled);  // Graceful degradation, not crash

    // Verify cluster still responsive after OOM
    let status = cluster.health_check().await.unwrap();
    assert_eq!(status, HealthStatus::Degraded);  // Degraded but not down
}
```

---

## 5. Chaos Engineering (Failure Testing)

### 5.1 Chaos Scenarios

**Scenario 1: MLX Embedding Service Crash**
```rust
// tests/chaos/embedding_failure.rs
#[tokio::test]
async fn test_embedding_service_circuit_breaker() {
    let cluster = TestCluster::start().await;

    // Kill MLX embedding service
    cluster.kill_service("akidb-embed").await;

    // Ingest should fail gracefully
    let result = cluster.ingest_with_embeddings(collection_id, docs).await;
    assert!(matches!(result, Err(Error::EmbeddingServiceUnavailable)));

    // Circuit breaker should open
    let cb_status = cluster.get_circuit_breaker_status("embedding").await.unwrap();
    assert_eq!(cb_status.state, CircuitBreakerState::Open);

    // Restart embedding service
    cluster.restart_service("akidb-embed").await;
    tokio::time::sleep(Duration::from_secs(5)).await;  // Wait for recovery

    // Circuit breaker should close
    let cb_status = cluster.get_circuit_breaker_status("embedding").await.unwrap();
    assert_eq!(cb_status.state, CircuitBreakerState::Closed);

    // Ingest should work again
    let result = cluster.ingest_with_embeddings(collection_id, docs).await;
    assert!(result.is_ok());
}
```

**Scenario 2: S3/MinIO Network Partition**
```rust
#[tokio::test]
async fn test_s3_unavailability_fallback() {
    let cluster = TestCluster::start_with_minio().await;

    // Block network to MinIO
    cluster.network_partition("akidb-storage", "minio").await;

    // Writes should continue to local WAL
    let result = cluster.ingest_vectors(collection_id, vectors).await;
    assert!(result.is_ok());  // Local WAL still works

    // S3 sync should fail but not block writes
    let sync_status = cluster.get_s3_sync_status().await.unwrap();
    assert_eq!(sync_status.state, SyncState::Retrying);

    // Restore network
    cluster.network_heal().await;
    tokio::time::sleep(Duration::from_secs(10)).await;

    // S3 sync should catch up
    let sync_status = cluster.get_s3_sync_status().await.unwrap();
    assert_eq!(sync_status.state, SyncState::Synced);
}
```

**Scenario 3: SQLite Metadata Corruption**
```rust
#[tokio::test]
async fn test_metadata_corruption_recovery() {
    let cluster = TestCluster::start().await;

    // Corrupt SQLite database file
    cluster.corrupt_file("metadata.db").await;

    // Restart cluster
    cluster.restart().await;

    // Should recover from S3 backup
    let recovery_status = cluster.get_recovery_status().await.unwrap();
    assert_eq!(recovery_status.source, RecoverySource::S3Backup);
    assert!(recovery_status.recovered_tenants > 0);

    // Verify data integrity after recovery
    let tenants = cluster.list_tenants().await.unwrap();
    assert!(!tenants.is_empty());
}
```

### 5.2 Chaos Testing Framework

```rust
// chaos_framework.rs
pub struct ChaosScenario {
    name: String,
    failure: FailureType,
    duration: Duration,
    validation: Box<dyn Fn(&TestCluster) -> bool>,
}

pub enum FailureType {
    ProcessCrash(String),               // Kill a service
    NetworkPartition(String, String),   // Network split
    FileCorruption(String),              // Corrupt a file
    ResourceExhaustion(ResourceType),    // CPU/Memory/Disk exhaustion
    Latency(String, Duration),           // Inject latency
}

pub async fn run_chaos_suite(scenarios: Vec<ChaosScenario>) {
    for scenario in scenarios {
        println!("Running chaos scenario: {}", scenario.name);

        let cluster = TestCluster::start().await.unwrap();

        // Inject failure
        scenario.failure.inject(&cluster).await;
        tokio::time::sleep(scenario.duration).await;

        // Validate system behavior
        assert!((scenario.validation)(&cluster), "Validation failed for {}", scenario.name);

        // Cleanup
        cluster.shutdown().await.unwrap();
    }
}
```

**Coverage Areas:**
- [x] MLX embedding service crash (circuit breaker)
- [x] S3/MinIO unavailability (local fallback)
- [x] SQLite corruption (backup recovery)
- [x] Cedar policy service latency (timeout handling)
- [x] WAL corruption (snapshot restore)
- [x] Network partitions (split-brain scenarios)
- [x] Resource exhaustion (OOM, disk full)

---

## 6. Observability Testing

### 6.1 Metrics Validation

**Prometheus Metrics to Test:**
```rust
// tests/observability/metrics_test.rs
use prometheus::Registry;

#[tokio::test]
async fn test_prometheus_metrics_exposed() {
    let cluster = TestCluster::start_with_prometheus().await;

    // Scrape Prometheus metrics endpoint
    let metrics = reqwest::get("http://localhost:9090/metrics")
        .await.unwrap()
        .text()
        .await.unwrap();

    // Verify key metrics exist
    assert!(metrics.contains("akidb_query_latency_seconds"));
    assert!(metrics.contains("akidb_ingest_throughput_vectors_per_second"));
    assert!(metrics.contains("akidb_mlx_embedding_latency_seconds"));
    assert!(metrics.contains("akidb_cedar_policy_evaluation_latency_seconds"));
    assert!(metrics.contains("akidb_hnsw_index_size_bytes"));
    assert!(metrics.contains("akidb_metadata_db_connections"));
}

#[tokio::test]
async fn test_metrics_accuracy() {
    let cluster = TestCluster::start().await;

    // Perform 100 queries
    for _ in 0..100 {
        cluster.query(collection_id, QueryRequest::default()).await.unwrap();
    }

    // Scrape metrics
    let metrics = cluster.scrape_metrics().await.unwrap();

    // Verify query count metric
    let query_count = metrics.get_counter("akidb_query_total");
    assert_eq!(query_count, 100.0);

    // Verify latency histogram
    let p95_latency = metrics.get_histogram_percentile("akidb_query_latency_seconds", 0.95);
    assert!(p95_latency < 0.025);  // <25ms P95
}
```

### 6.2 Distributed Tracing

**OpenTelemetry Integration:**
```rust
// tests/observability/tracing_test.rs
use opentelemetry::trace::Tracer;

#[tokio::test]
async fn test_distributed_tracing() {
    let cluster = TestCluster::start_with_jaeger().await;

    // Execute traced operation
    let span_id = cluster.ingest_with_trace(collection_id, docs).await.unwrap();

    // Query Jaeger for trace
    tokio::time::sleep(Duration::from_secs(2)).await;  // Wait for export
    let trace = cluster.get_jaeger_trace(span_id).await.unwrap();

    // Verify trace spans
    assert_span_exists(&trace, "ingest_request");
    assert_span_exists(&trace, "mlx_embedding");
    assert_span_exists(&trace, "hnsw_insert");
    assert_span_exists(&trace, "wal_write");
    assert_span_exists(&trace, "s3_sync");

    // Verify span relationships (parent-child)
    let ingest_span = trace.find_span("ingest_request");
    let embedding_span = trace.find_span("mlx_embedding");
    assert_eq!(embedding_span.parent_id, ingest_span.span_id);
}
```

### 6.3 Structured Logging

**Log Validation:**
```rust
#[tokio::test]
async fn test_structured_logging() {
    let cluster = TestCluster::start_with_log_capture().await;

    // Trigger error condition
    let result = cluster.query_nonexistent_collection("fake-id").await;
    assert!(result.is_err());

    // Capture logs
    let logs = cluster.get_logs().await;

    // Verify structured log entry
    let error_log = logs.iter().find(|log| log.level == "ERROR").unwrap();
    assert_eq!(error_log.message, "Collection not found");
    assert_eq!(error_log.fields["collection_id"], "fake-id");
    assert_eq!(error_log.fields["error_type"], "NotFound");
    assert!(error_log.fields.contains_key("tenant_id"));
    assert!(error_log.fields.contains_key("trace_id"));
}
```

---

## 7. Test Execution Plan

### Week 0-4 (Foundation Phase)
- [ ] Unit tests for `akidb-core` (tenant, collection, user)
- [ ] Unit tests for `akidb-metadata` (SQLite CRUD, FTS5, migrations)
- [ ] Integration tests for metadata store
- [ ] CI/CD pipeline on GitHub Actions (ARM64 runners)

### Week 5-8 (Embedding Service Phase)
- [ ] Unit tests for `akidb-embed` (MLX model loading, inference)
- [ ] Integration tests for ingest with embeddings
- [ ] Performance benchmarks (embedding throughput >200 vec/sec)
- [ ] E2E test: ingest → embed → query

### Week 9-12 (Enhanced RBAC Phase)
- [ ] Unit tests for Cedar policy engine
- [ ] Integration tests for authorization middleware
- [ ] Tenant isolation tests
- [ ] Policy evaluation latency benchmarks (<5ms P99)

### Week 13-16 (API Unification Phase)
- [ ] Integration tests for gRPC API
- [ ] REST vs gRPC parity tests
- [ ] Load testing (100 QPS sustained)
- [ ] Chaos engineering scenarios
- [ ] E2E test suite completion

---

## 8. Test Infrastructure

### 8.1 Test Harness

```rust
// test_harness/mod.rs
pub struct TestCluster {
    config: ClusterConfig,
    processes: Vec<Child>,
    minio: Option<MinioContainer>,
    prometheus: Option<PrometheusContainer>,
}

impl TestCluster {
    pub async fn start() -> Result<Self> {
        // Start MinIO for S3 testing
        let minio = MinioContainer::start().await?;

        // Start AkiDB services
        let mut processes = vec![];
        processes.push(start_akidb_api().await?);
        processes.push(start_akidb_embed().await?);

        Ok(Self {
            config: ClusterConfig::default(),
            processes,
            minio: Some(minio),
            prometheus: None,
        })
    }

    pub async fn create_tenant(&self, name: &str) -> Result<TenantDescriptor> {
        // Create tenant via API
    }

    pub async fn ingest_with_embeddings(&self, collection_id: Uuid, docs: Vec<Document>) -> Result<IngestResponse> {
        // Ingest with automatic MLX embeddings
    }

    pub async fn query(&self, collection_id: Uuid, req: QueryRequest) -> Result<QueryResponse> {
        // Execute query
    }

    pub async fn kill_service(&self, name: &str) {
        // Kill specific service for chaos testing
    }

    pub async fn shutdown(self) -> Result<()> {
        for mut process in self.processes {
            process.kill().await?;
        }
        if let Some(minio) = self.minio {
            minio.stop().await?;
        }
        Ok(())
    }
}
```

### 8.2 Continuous Integration

**GitHub Actions Workflow:**
```yaml
# .github/workflows/test.yml
name: AkiDB 2.0 Tests

on: [push, pull_request]

jobs:
  test-mlx-macos:
    runs-on: macos-14  # M1 runners
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          target: aarch64-apple-darwin

      - name: Install MLX
        run: pip install mlx

      - name: Run Unit Tests
        run: cargo test --all --lib

      - name: Run Integration Tests
        run: cargo test --all --test '*'

      - name: Run Benchmarks
        run: cargo bench --no-run

      - name: Upload Coverage
        uses: codecov/codecov-action@v3
        with:
          file: ./coverage.xml

  test-load:
    runs-on: macos-14
    steps:
      - uses: actions/checkout@v4

      - name: Build Release Binary
        run: cargo build --release

      - name: Run Load Tests (100 QPS)
        run: |
          ./target/release/akidb start &
          sleep 10
          vegeta attack -rate=100 -duration=1m -targets=targets.txt | vegeta report
```

---

## 9. Success Criteria

### 9.1 Quantitative Metrics

| Metric | Target | Measurement Method |
|--------|--------|-------------------|
| **Unit Test Coverage** | >80% | `cargo tarpaulin` |
| **Integration Test Coverage** | 100% APIs | Manual verification |
| **E2E Test Coverage** | 100% journeys | Test suite |
| **Query Latency (P95)** | <25ms | `hyperfine` |
| **Query Latency (P99)** | <50ms | `hyperfine` |
| **Ingest Throughput** | >10k vec/sec | Benchmark |
| **MLX Embedding Throughput** | >200 vec/sec | Benchmark |
| **Cedar Evaluation (P99)** | <5ms | Benchmark |
| **Crash Recovery Time** | <30s | Integration test |
| **Availability (Chaos)** | >99.9% | Chaos suite |

### 9.2 Qualitative Criteria

- [ ] All tests pass on macOS 14+ (Sonoma/Sequoia)
- [ ] All tests pass on M1, M2, M3 hardware
- [ ] MLX embeddings work with Metal GPU acceleration
- [ ] NEON SIMD optimizations validated
- [ ] Tenant isolation 100% effective
- [ ] No memory leaks (Valgrind/Instruments)
- [ ] No data corruption under chaos
- [ ] Documentation complete for all tests

---

## 10. Test Reporting

### 10.1 Daily Test Reports

**Format:**
```markdown
# AkiDB 2.0 Test Report - 2025-11-15

## Summary
- Unit Tests: 487 passed, 0 failed (coverage: 82%)
- Integration Tests: 45 passed, 0 failed
- E2E Tests: 12 passed, 0 failed
- Performance Tests: 8 passed, 0 failed
- Chaos Tests: 5 passed, 0 failed

## Performance Benchmarks
- Query Latency (P95): 18.2ms ✅
- Query Latency (P99): 31.5ms ✅
- MLX Embedding Throughput: 247 vec/sec ✅
- Cedar Evaluation (P99): 3.8ms ✅

## Failures
None.

## Blockers
None.

## Next Actions
- Continue Phase 2 integration tests
- Add Cedar policy cache invalidation tests
```

### 10.2 Weekly Executive Summary

**Format:**
```markdown
# Week 8 Test Summary (Embedding Service Phase)

## Key Achievements
✅ MLX embedding service integration complete
✅ All unit tests passing (coverage: 82%)
✅ Performance targets met (P95 <25ms)
✅ Chaos tests validate 99.9% availability

## Test Metrics
- Total Tests: 562 (487 unit + 45 integration + 12 E2E + 18 perf/chaos)
- Pass Rate: 100%
- Code Coverage: 82%
- Performance: All targets met

## Risk Assessment
🟢 Low Risk - All critical paths tested and validated

## Next Week Focus
- Phase 3: Enhanced RBAC (Cedar policy engine)
- Add policy evaluation performance tests
- Tenant isolation stress tests
```

---

## 11. Tools and Frameworks

### Testing Tools
- **Unit Testing:** `cargo test` (Rust native)
- **Property Testing:** `proptest` (fuzz testing)
- **Benchmarking:** `criterion`, `hyperfine`
- **Load Testing:** `vegeta`, `wrk`
- **Chaos Engineering:** Custom framework (Rust)
- **Coverage:** `cargo tarpaulin`

### MLX Tools
- **MLX Framework:** `pip install mlx`
- **Model Hub:** https://huggingface.co/mlx-community
- **Profiling:** Xcode Instruments (Metal GPU profiler)

### Observability Tools
- **Metrics:** Prometheus + Grafana
- **Tracing:** OpenTelemetry + Jaeger
- **Logging:** `tracing` crate (structured logs)

---

## 12. Risks and Mitigation

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| MLX model compatibility issues | Medium | High | Test with multiple MLX models, document compatibility matrix |
| Metal GPU driver bugs | Low | High | Fallback to CPU inference, report to Apple |
| NEON SIMD regressions | Low | Medium | Comprehensive benchmarks, compare vs scalar |
| Test infrastructure instability | Medium | Medium | Isolated test environments, idempotent tests |
| Performance regressions | Medium | High | Automated benchmarking in CI, alert on >10% degradation |
| Insufficient M-series hardware | Low | Medium | Use cloud ARM instances (AWS Graviton) as fallback |

---

## Prepared By

**QA Engineering Team**
**Platform Engineering Team**
**Date:** 2025-11-06
**Version:** 1.0
**Confidentiality:** Internal Use Only

---

## Appendix A: MLX-Specific Considerations

### A.1 MLX Model Compatibility

**Supported Models:**
- `mlx-community/qwen3-embedding-8b-int8` (recommended)
- `mlx-community/gte-large-en-v1.5`
- `mlx-community/bge-large-en-v1.5`

**Model Selection Criteria:**
- Int8 quantization support (memory efficiency)
- Output dimension: 512 or 1024 (HNSW compatibility)
- Inference latency: <10ms per document

### A.2 Metal GPU Profiling

**Xcode Instruments:**
```bash
# Profile Metal GPU usage
instruments -t "Metal System Trace" -D trace.trace ./target/release/akidb

# Analyze GPU occupancy
open trace.trace  # Opens in Xcode Instruments
```

**Expected Metrics:**
- GPU Utilization: >70% during embedding inference
- Memory Bandwidth: <80GB/s (within M2 Max bandwidth)
- Compute Throughput: >10 TFLOPS (M2 Max theoretical max: 13.6 TFLOPS)

### A.3 Unified Memory Testing

**Test Unified Memory Advantage:**
```rust
#[test]
fn test_unified_memory_zero_copy() {
    let vectors: Vec<f32> = vec![1.0; 512];

    // CPU → GPU transfer should be zero-copy on Apple Silicon
    let start = Instant::now();
    let gpu_vectors = mlx::array(&vectors);  // No memcpy!
    let elapsed = start.elapsed();

    assert!(elapsed.as_micros() < 10);  // <10µs for 512 floats (zero-copy)
}
```

---

**Document Complete.**

This comprehensive testing plan ensures AkiDB 2.0 achieves production-grade quality on macOS with Apple MLX, covering unit, integration, E2E, performance, and chaos testing with clear success criteria and execution timeline.
