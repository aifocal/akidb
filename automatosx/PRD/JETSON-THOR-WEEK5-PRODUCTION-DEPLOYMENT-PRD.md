# Jetson Thor Week 5: Production Deployment & API Server Integration PRD

**Status:** Ready to Execute
**Timeline:** 5 days (Week 5)
**Owner:** Backend Team + DevOps Engineering
**Dependencies:** Week 1 (✅), Week 2 (✅), Week 3 (✅), Week 4 (✅)
**Target Platform:** NVIDIA Jetson Thor (Blackwell GPU, 2,000 TOPS)

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Goals & Non-Goals](#goals--non-goals)
3. [Baseline Analysis](#baseline-analysis)
4. [Production Architecture](#production-architecture)
5. [API Server Design](#api-server-design)
6. [Day-by-Day Implementation Plan](#day-by-day-implementation-plan)
7. [Docker & Kubernetes Deployment](#docker--kubernetes-deployment)
8. [Observability Strategy](#observability-strategy)
9. [Performance Validation](#performance-validation)
10. [Risk Management](#risk-management)
11. [Success Criteria](#success-criteria)
12. [Appendix: Code Examples](#appendix-code-examples)

---

## Executive Summary

Week 5 focuses on **production deployment** and **API server integration** for Jetson Thor, transforming the optimized embedding engine (Weeks 1-4) into a production-ready REST/gRPC API service. We will containerize the deployment, create Kubernetes manifests, implement observability (Prometheus, Grafana, OpenTelemetry), and prepare for automotive/robotics production workloads.

### Key Objectives

1. **REST/gRPC API Servers:** Deploy akidb-rest and akidb-grpc with Jetson Thor integration
2. **Docker Containerization:** Create production-ready Dockerfiles with TensorRT support
3. **Kubernetes Deployment:** Helm charts for edge cluster deployment
4. **Observability:** Prometheus metrics, Grafana dashboards, OpenTelemetry tracing
5. **Performance Testing:** Load testing @ 50+ QPS with <30ms P95 latency
6. **Production Hardening:** Health checks, graceful shutdown, circuit breakers

### Expected Outcomes

- ✅ Production-ready REST API on :8080 and gRPC API on :9090
- ✅ Docker images with TensorRT EP support (<2GB compressed)
- ✅ Kubernetes Helm chart with GPU scheduling, auto-scaling (HPA)
- ✅ Prometheus metrics + Grafana dashboard (15+ metrics)
- ✅ OpenTelemetry distributed tracing with sub-1ms overhead
- ✅ Load test results: >50 QPS @ <30ms P95, >150 QPS concurrent
- ✅ Zero-downtime rolling updates with health checks
- ✅ Production documentation (deployment guide, runbooks)

---

## Goals & Non-Goals

### Goals (Week 5)

**Primary Goals:**
1. ✅ **REST API Server** - akidb-rest integrated with ONNX+TensorRT provider
2. ✅ **gRPC API Server** - akidb-grpc integrated with ONNX+TensorRT provider
3. ✅ **Multi-Model API Support** - Runtime model selection (Week 4 registry)
4. ✅ **Docker Deployment** - Production Dockerfiles with NVIDIA GPU support
5. ✅ **Kubernetes Helm Chart** - Edge cluster deployment with GPU scheduling
6. ✅ **Observability** - Prometheus, Grafana, OpenTelemetry integration
7. ✅ **Load Testing** - Validate >50 QPS @ <30ms P95 latency
8. ✅ **Production Hardening** - Health checks, graceful shutdown, circuit breakers

**Secondary Goals:**
- 📊 Horizontal Pod Autoscaling (HPA) based on GPU utilization
- 📊 NVIDIA Device Plugin for Kubernetes GPU scheduling
- 📊 Custom Grafana dashboards for robotics use cases
- 📝 API documentation with OpenAPI/Swagger
- 📝 Deployment runbooks for operators

### Non-Goals (Deferred to Week 6+)

**Not in Scope for Week 5:**
- ❌ Multi-region deployment (single edge cluster only)
- ❌ Distributed tracing across multiple services
- ❌ Advanced security (mutual TLS, RBAC) - Week 6
- ❌ CI/CD pipeline automation - Week 7+
- ❌ Blue-green deployments - Week 7+
- ❌ Chaos engineering tests - Week 8+

---

## Baseline Analysis

### Week 4 Multi-Model Performance

**Baseline (from Week 4):**
- Models: 6 models (Qwen3 0.5B/1.5B/4B/7B, E5-small, BGE-small)
- Model registry: ✅ Implemented
- LRU cache: ✅ 2-3 models, 8GB limit
- Cache hit: <5ms P95
- Cache miss (cold start): <500ms P95
- Warm inference: <30ms P95 (Qwen3-4B)

**Current Limitations:**
- Standalone Python/Rust scripts only (no API server)
- Manual startup and configuration
- No observability or metrics
- No production deployment infrastructure
- No horizontal scaling capability

### Week 5 Target State

**Production-Ready Deployment:**
- REST API: `:8080` with `/api/v1/embed` endpoint
- gRPC API: `:9090` with `EmbeddingService/Embed` RPC
- Docker: Multi-stage builds, <2GB compressed images
- Kubernetes: Helm chart with GPU scheduling, HPA
- Observability: Prometheus metrics, Grafana dashboards, OpenTelemetry tracing
- Performance: >50 QPS @ <30ms P95, >150 QPS concurrent
- Reliability: 99.9% uptime, <1s graceful shutdown

---

## Production Architecture

### System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Kubernetes Edge Cluster                   │
│                    (Jetson Thor Nodes)                       │
└─────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        │                     │                     │
        ▼                     ▼                     ▼
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│ akidb-rest   │     │ akidb-grpc   │     │ Prometheus   │
│ Pod (REST)   │     │ Pod (gRPC)   │     │ (Metrics)    │
│ :8080        │     │ :9090        │     │ :9090        │
└──────┬───────┘     └──────┬───────┘     └──────┬───────┘
       │                    │                     │
       └────────────┬───────┘                     │
                    │                             │
                    ▼                             ▼
         ┌─────────────────────┐        ┌────────────────┐
         │ EmbeddingManager    │        │ Grafana        │
         │ (Multi-Model)       │        │ (Dashboards)   │
         └──────────┬──────────┘        └────────────────┘
                    │
         ┌──────────┼──────────┐
         │          │          │
         ▼          ▼          ▼
    ┌────────┐ ┌────────┐ ┌────────┐
    │ Qwen3  │ │ E5     │ │ BGE    │
    │ 4B     │ │ small  │ │ small  │
    │ (LRU)  │ │ (LRU)  │ │ (LRU)  │
    └────┬───┘ └────┬───┘ └────┬───┘
         │          │          │
         └──────────┼──────────┘
                    │
                    ▼
         ┌─────────────────────┐
         │ ONNX Runtime        │
         │ TensorRT EP (FP8)   │
         └──────────┬──────────┘
                    │
                    ▼
         ┌─────────────────────┐
         │ NVIDIA Blackwell GPU│
         │ (2,000 TOPS)        │
         └─────────────────────┘
```

### Deployment Topology

**Option 1: Single-Node Edge Deployment** (Week 5 focus)
- Single Jetson Thor device
- Both REST and gRPC servers co-located
- Local Prometheus + Grafana
- Use cases: Factory floor, autonomous vehicle (single unit)

**Option 2: Multi-Node Edge Cluster** (Future - Week 6+)
- 3+ Jetson Thor nodes
- Load balancer (NGINX/Traefik)
- Centralized Prometheus + Grafana
- Use cases: Fleet of robots, multi-zone factory

---

## API Server Design

### REST API Server (akidb-rest)

**Endpoints:**

1. **POST /api/v1/embed** - Generate embeddings
   ```json
   // Request
   {
     "model": "Qwen/Qwen2.5-4B",  // Optional, defaults to "Qwen/Qwen2.5-4B"
     "inputs": [
       "The autonomous vehicle detects pedestrians.",
       "Emergency braking system activated."
     ],
     "normalize": true
   }

   // Response (200 OK)
   {
     "embeddings": [
       [0.123, -0.456, ...],  // 4096-dim vector
       [0.789, -0.234, ...]
     ],
     "model": "Qwen/Qwen2.5-4B",
     "usage": {
       "total_tokens": 24,
       "duration_ms": 28
     }
   }

   // Error (400 Bad Request)
   {
     "error": "Model not found: invalid-model",
     "code": "MODEL_NOT_FOUND"
   }
   ```

2. **GET /health** - Health check
   ```json
   {
     "status": "healthy",
     "checks": {
       "embedding_provider": "ok",
       "model_cache": "ok",
       "gpu": "ok"
     },
     "version": "2.0.0-jetson-thor"
   }
   ```

3. **GET /metrics** - Prometheus metrics
   ```
   # HELP akidb_embed_requests_total Total embedding requests
   # TYPE akidb_embed_requests_total counter
   akidb_embed_requests_total{model="Qwen/Qwen2.5-4B",status="success"} 1234

   # HELP akidb_embed_latency_seconds Embedding latency
   # TYPE akidb_embed_latency_seconds histogram
   akidb_embed_latency_seconds_bucket{model="Qwen/Qwen2.5-4B",le="0.01"} 0
   akidb_embed_latency_seconds_bucket{model="Qwen/Qwen2.5-4B",le="0.025"} 456
   akidb_embed_latency_seconds_bucket{model="Qwen/Qwen2.5-4B",le="0.05"} 1234
   akidb_embed_latency_seconds_sum{model="Qwen/Qwen2.5-4B"} 34.567
   akidb_embed_latency_seconds_count{model="Qwen/Qwen2.5-4B"} 1234

   # HELP akidb_model_cache_hits_total Model cache hits
   # TYPE akidb_model_cache_hits_total counter
   akidb_model_cache_hits_total{model="Qwen/Qwen2.5-4B"} 1100

   # HELP akidb_model_cache_misses_total Model cache misses
   # TYPE akidb_model_cache_misses_total counter
   akidb_model_cache_misses_total{model="Qwen/Qwen2.5-4B"} 134

   # HELP akidb_gpu_memory_used_bytes GPU memory used
   # TYPE akidb_gpu_memory_used_bytes gauge
   akidb_gpu_memory_used_bytes 4294967296
   ```

4. **GET /models** - List available models (new)
   ```json
   {
     "models": [
       {
         "id": "qwen3-4b",
         "name": "Qwen/Qwen2.5-4B",
         "dimension": 4096,
         "params": 4000000000,
         "memory_mb": 4000,
         "latency_p95_ms": 25,
         "throughput_qps": 50,
         "status": "loaded"
       },
       {
         "id": "e5-small-v2",
         "name": "intfloat/e5-small-v2",
         "dimension": 384,
         "params": 33000000,
         "memory_mb": 35,
         "latency_p95_ms": 5,
         "throughput_qps": 200,
         "status": "available"
       }
     ]
   }
   ```

### gRPC API Server (akidb-grpc)

**Proto Definition:**

```protobuf
syntax = "proto3";
package akidb.embedding.v1;

service EmbeddingService {
  rpc Embed(EmbedRequest) returns (EmbedResponse);
  rpc Health(HealthRequest) returns (HealthResponse);
  rpc ListModels(ListModelsRequest) returns (ListModelsResponse);
}

message EmbedRequest {
  string model = 1;           // Optional, defaults to "Qwen/Qwen2.5-4B"
  repeated string inputs = 2;
  bool normalize = 3;
}

message EmbedResponse {
  repeated Vector embeddings = 1;
  string model = 2;
  Usage usage = 3;
}

message Vector {
  repeated float values = 1;
}

message Usage {
  int32 total_tokens = 1;
  int32 duration_ms = 2;
}

message HealthRequest {}

message HealthResponse {
  enum Status {
    UNKNOWN = 0;
    HEALTHY = 1;
    UNHEALTHY = 2;
  }
  Status status = 1;
  map<string, string> checks = 2;
  string version = 3;
}

message ListModelsRequest {}

message ListModelsResponse {
  repeated ModelInfo models = 1;
}

message ModelInfo {
  string id = 1;
  string name = 2;
  int32 dimension = 3;
  int64 params = 4;
  int32 memory_mb = 5;
  int32 latency_p95_ms = 6;
  int32 throughput_qps = 7;
  string status = 8;  // "loaded", "available", "loading"
}
```

### Integration with EmbeddingManager

**Rust Implementation:**

```rust
// crates/akidb-rest/src/handlers/embedding.rs
use akidb_service::embedding_manager::EmbeddingManager;
use axum::{Json, extract::State};
use std::sync::Arc;

#[derive(Deserialize)]
pub struct EmbedRequest {
    #[serde(default = "default_model")]
    model: String,
    inputs: Vec<String>,
    #[serde(default = "default_normalize")]
    normalize: bool,
}

fn default_model() -> String {
    "Qwen/Qwen2.5-4B".to_string()
}

fn default_normalize() -> bool {
    true
}

#[derive(Serialize)]
pub struct EmbedResponse {
    embeddings: Vec<Vec<f32>>,
    model: String,
    usage: Usage,
}

#[derive(Serialize)]
pub struct Usage {
    total_tokens: usize,
    duration_ms: u64,
}

pub async fn embed(
    State(manager): State<Arc<EmbeddingManager>>,
    Json(req): Json<EmbedRequest>,
) -> Result<Json<EmbedResponse>, ApiError> {
    let start = std::time::Instant::now();

    // Get provider for requested model (may trigger cache load)
    let provider = manager.get_provider(&req.model).await?;

    // Generate embeddings
    let batch_req = BatchEmbeddingRequest {
        model: req.model.clone(),
        inputs: req.inputs.clone(),
        normalize: req.normalize,
    };

    let batch_resp = provider.embed_batch(batch_req).await?;

    let duration_ms = start.elapsed().as_millis() as u64;

    // Record metrics
    EMBED_REQUESTS_TOTAL
        .with_label_values(&[&req.model, "success"])
        .inc();
    EMBED_LATENCY
        .with_label_values(&[&req.model])
        .observe(duration_ms as f64 / 1000.0);

    Ok(Json(EmbedResponse {
        embeddings: batch_resp.embeddings,
        model: req.model,
        usage: Usage {
            total_tokens: batch_resp.usage.total_tokens,
            duration_ms,
        },
    }))
}
```

---

## Day-by-Day Implementation Plan

### Day 1: API Server Integration

**Objective:** Integrate ONNX+TensorRT provider with akidb-rest and akidb-grpc servers

**Tasks:**

1. **Update akidb-rest** (`crates/akidb-rest/src/main.rs`, ~50 lines changed)
   - Replace `MockEmbeddingProvider` with `OnnxEmbeddingProvider`
   - Configure TensorRT execution provider from environment
   - Add `/models` endpoint for model registry
   - Update `/api/v1/embed` to accept `model` parameter
   - Add error handling for model loading failures

   ```rust
   use akidb_embedding::{OnnxConfig, ExecutionProviderConfig, OnnxEmbeddingProvider};

   #[tokio::main]
   async fn main() -> Result<(), Box<dyn std::error::Error>> {
       let config = OnnxConfig {
           model_path: PathBuf::from(env::var("AKIDB_MODEL_PATH")?),
           tokenizer_path: PathBuf::from(env::var("AKIDB_TOKENIZER_PATH")?),
           model_name: env::var("AKIDB_MODEL_NAME").unwrap_or_else(|_| "Qwen/Qwen2.5-4B".to_string()),
           dimension: env::var("AKIDB_MODEL_DIMENSION")?.parse()?,
           max_length: 512,
           execution_provider: ExecutionProviderConfig::TensorRT {
               device_id: 0,
               fp8_enable: true,
               engine_cache_path: Some(PathBuf::from("/var/cache/akidb/trt")),
           },
       };

       let provider = Arc::new(OnnxEmbeddingProvider::with_config(config).await?);
       let manager = Arc::new(EmbeddingManager::new(provider));

       // ... rest of server setup
   }
   ```

2. **Update akidb-grpc** (`crates/akidb-grpc/src/main.rs`, ~40 lines changed)
   - Same ONNX+TensorRT integration as REST server
   - Update `EmbeddingService::Embed` implementation
   - Add `EmbeddingService::ListModels` RPC

3. **Add Model Registry Endpoint** (both servers)
   - Read model metadata from Week 4 registry
   - Return available models + loaded status
   - Include performance characteristics

4. **Environment Configuration**
   ```bash
   # Required environment variables
   export AKIDB_MODEL_PATH=/opt/akidb/models/qwen3-4b-fp8.onnx
   export AKIDB_TOKENIZER_PATH=/opt/akidb/models/tokenizer.json
   export AKIDB_MODEL_NAME="Qwen/Qwen2.5-4B"
   export AKIDB_MODEL_DIMENSION=4096
   export AKIDB_TENSORRT_CACHE=/var/cache/akidb/trt
   ```

5. **Testing**
   ```bash
   # Start REST server
   cargo run -p akidb-rest

   # Test embedding endpoint
   curl -X POST http://localhost:8080/api/v1/embed \
     -H "Content-Type: application/json" \
     -d '{"inputs": ["Hello, Jetson Thor!"], "model": "Qwen/Qwen2.5-4B"}'

   # Test models endpoint
   curl http://localhost:8080/models

   # Test health
   curl http://localhost:8080/health
   ```

**Success Criteria:**
- ✅ REST and gRPC servers start successfully with ONNX+TensorRT
- ✅ `/api/v1/embed` endpoint returns 4096-dim embeddings
- ✅ `/models` endpoint lists available models
- ✅ Health check passes
- ✅ P95 latency <30ms (single request)

**Completion Report:** `automatosx/tmp/jetson-thor-week5-day1-api-integration-complete.md`

---

### Day 2: Docker Containerization

**Objective:** Create production-ready Docker images with TensorRT support

**Tasks:**

1. **Create Multi-Stage Dockerfile** (`docker/Dockerfile.jetson-rest`, ~80 lines)

   ```dockerfile
   # Stage 1: Builder
   FROM nvcr.io/nvidia/l4t-base:r36.4.0 AS builder

   # Install Rust
   RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
   ENV PATH="/root/.cargo/bin:${PATH}"

   # Install build dependencies
   RUN apt-get update && apt-get install -y \
       build-essential \
       pkg-config \
       libssl-dev \
       && rm -rf /var/lib/apt/lists/*

   # Set working directory
   WORKDIR /build

   # Copy source code
   COPY Cargo.toml Cargo.lock ./
   COPY crates/ ./crates/

   # Build release binary
   RUN cargo build --release -p akidb-rest

   # Stage 2: Runtime
   FROM nvcr.io/nvidia/l4t-base:r36.4.0

   # Install runtime dependencies
   RUN apt-get update && apt-get install -y \
       libssl3 \
       ca-certificates \
       libtensorrt8 \
       tensorrt \
       && rm -rf /var/lib/apt/lists/*

   # Create non-root user
   RUN useradd -m -u 1000 akidb

   # Create directories
   RUN mkdir -p /opt/akidb/models /var/cache/akidb/trt /var/log/akidb \
       && chown -R akidb:akidb /opt/akidb /var/cache/akidb /var/log/akidb

   # Copy binary
   COPY --from=builder /build/target/release/akidb-rest /usr/local/bin/

   # Copy models (placeholder - mount volume in production)
   # COPY models/ /opt/akidb/models/

   # Switch to non-root user
   USER akidb

   # Environment variables
   ENV RUST_LOG=info
   ENV AKIDB_HOST=0.0.0.0
   ENV AKIDB_REST_PORT=8080
   ENV AKIDB_MODEL_PATH=/opt/akidb/models/qwen3-4b-fp8.onnx
   ENV AKIDB_TOKENIZER_PATH=/opt/akidb/models/tokenizer.json
   ENV AKIDB_MODEL_NAME="Qwen/Qwen2.5-4B"
   ENV AKIDB_MODEL_DIMENSION=4096
   ENV AKIDB_TENSORRT_CACHE=/var/cache/akidb/trt

   # Expose port
   EXPOSE 8080

   # Health check
   HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
     CMD curl -f http://localhost:8080/health || exit 1

   # Run server
   CMD ["akidb-rest"]
   ```

2. **Create Dockerfile for gRPC** (`docker/Dockerfile.jetson-grpc`, similar structure)

3. **Build Script** (`scripts/build-docker-jetson.sh`)
   ```bash
   #!/bin/bash
   set -e

   VERSION=${1:-2.0.0-jetson-thor}

   echo "Building Docker images for Jetson Thor (version: $VERSION)"

   # Build REST image
   docker build \
     -f docker/Dockerfile.jetson-rest \
     -t akidb/akidb-rest:$VERSION \
     -t akidb/akidb-rest:latest-jetson \
     .

   # Build gRPC image
   docker build \
     -f docker/Dockerfile.jetson-grpc \
     -t akidb/akidb-grpc:$VERSION \
     -t akidb/akidb-grpc:latest-jetson \
     .

   echo "✅ Docker images built successfully"
   docker images | grep akidb
   ```

4. **Docker Compose for Jetson** (`docker-compose.jetson.yaml`)
   ```yaml
   version: '3.8'

   services:
     akidb-rest:
       image: akidb/akidb-rest:2.0.0-jetson-thor
       container_name: akidb-rest
       ports:
         - "8080:8080"
       volumes:
         - ./models:/opt/akidb/models:ro
         - tensorrt-cache:/var/cache/akidb/trt
         - logs:/var/log/akidb
       environment:
         - RUST_LOG=info
         - AKIDB_HOST=0.0.0.0
         - AKIDB_REST_PORT=8080
       runtime: nvidia
       deploy:
         resources:
           reservations:
             devices:
               - driver: nvidia
                 count: 1
                 capabilities: [gpu]
       restart: unless-stopped
       healthcheck:
         test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
         interval: 30s
         timeout: 3s
         retries: 3

     akidb-grpc:
       image: akidb/akidb-grpc:2.0.0-jetson-thor
       container_name: akidb-grpc
       ports:
         - "9090:9090"
       volumes:
         - ./models:/opt/akidb/models:ro
         - tensorrt-cache:/var/cache/akidb/trt
         - logs:/var/log/akidb
       environment:
         - RUST_LOG=info
         - AKIDB_HOST=0.0.0.0
         - AKIDB_GRPC_PORT=9090
       runtime: nvidia
       deploy:
         resources:
           reservations:
             devices:
               - driver: nvidia
                 count: 1
                 capabilities: [gpu]
       restart: unless-stopped

   volumes:
     tensorrt-cache:
     logs:
   ```

5. **Testing Docker Deployment**
   ```bash
   # Build images
   bash scripts/build-docker-jetson.sh 2.0.0-jetson-thor

   # Start containers
   docker-compose -f docker-compose.jetson.yaml up -d

   # Check logs
   docker-compose -f docker-compose.jetson.yaml logs -f

   # Test REST API
   curl -X POST http://localhost:8080/api/v1/embed \
     -H "Content-Type: application/json" \
     -d '{"inputs": ["Test embedding"]}'

   # Check GPU usage
   nvidia-smi
   ```

**Success Criteria:**
- ✅ Docker images build successfully (<2GB compressed)
- ✅ Containers start and pass health checks
- ✅ TensorRT engine compiles on first run (~4min)
- ✅ Subsequent restarts use cached engine (<1s startup)
- ✅ GPU memory usage <4GB
- ✅ API endpoints respond correctly

**Completion Report:** `automatosx/tmp/jetson-thor-week5-day2-docker-complete.md`

---

### Day 3: Kubernetes Deployment

**Objective:** Create Kubernetes Helm chart for edge cluster deployment

**Tasks:**

1. **Create Helm Chart Structure** (`deploy/helm/akidb-jetson/`)
   ```
   akidb-jetson/
   ├── Chart.yaml
   ├── values.yaml
   ├── templates/
   │   ├── deployment-rest.yaml
   │   ├── deployment-grpc.yaml
   │   ├── service-rest.yaml
   │   ├── service-grpc.yaml
   │   ├── configmap.yaml
   │   ├── pvc.yaml
   │   └── hpa.yaml
   └── README.md
   ```

2. **Chart.yaml**
   ```yaml
   apiVersion: v2
   name: akidb-jetson
   description: AkiDB embedding service for NVIDIA Jetson Thor
   version: 2.0.0-jetson-thor
   appVersion: "2.0.0"
   keywords:
     - vector-database
     - embeddings
     - jetson
     - tensorrt
   maintainers:
     - name: AkiDB Team
       email: team@akidb.io
   ```

3. **values.yaml** (configurable parameters)
   ```yaml
   # Default values for akidb-jetson

   image:
     repository: akidb/akidb-rest
     tag: 2.0.0-jetson-thor
     pullPolicy: IfNotPresent

   rest:
     enabled: true
     replicaCount: 1
     port: 8080
     resources:
       limits:
         nvidia.com/gpu: 1
         memory: 16Gi
       requests:
         nvidia.com/gpu: 1
         memory: 8Gi

   grpc:
     enabled: true
     replicaCount: 1
     port: 9090
     resources:
       limits:
         nvidia.com/gpu: 1
         memory: 16Gi
       requests:
         nvidia.com/gpu: 1
         memory: 8Gi

   models:
     # Model storage configuration
     persistentVolume:
       enabled: true
       storageClass: local-path
       size: 20Gi
       mountPath: /opt/akidb/models

   tensorrtCache:
     # TensorRT engine cache
     persistentVolume:
       enabled: true
       storageClass: local-path
       size: 10Gi
       mountPath: /var/cache/akidb/trt

   autoscaling:
     enabled: false  # Enable in production
     minReplicas: 1
     maxReplicas: 3
     targetCPUUtilizationPercentage: 70
     targetGPUUtilizationPercentage: 80

   monitoring:
     prometheus:
       enabled: true
       port: 9090
       path: /metrics

   env:
     RUST_LOG: info
     AKIDB_MODEL_NAME: "Qwen/Qwen2.5-4B"
     AKIDB_MODEL_DIMENSION: "4096"
   ```

4. **deployment-rest.yaml**
   ```yaml
   apiVersion: apps/v1
   kind: Deployment
   metadata:
     name: {{ .Release.Name }}-rest
     labels:
       app: akidb-rest
       release: {{ .Release.Name }}
   spec:
     replicas: {{ .Values.rest.replicaCount }}
     selector:
       matchLabels:
         app: akidb-rest
         release: {{ .Release.Name }}
     template:
       metadata:
         labels:
           app: akidb-rest
           release: {{ .Release.Name }}
         annotations:
           prometheus.io/scrape: "true"
           prometheus.io/port: "8080"
           prometheus.io/path: "/metrics"
       spec:
         containers:
         - name: akidb-rest
           image: "{{ .Values.image.repository }}:{{ .Values.image.tag }}"
           imagePullPolicy: {{ .Values.image.pullPolicy }}
           ports:
           - name: http
             containerPort: {{ .Values.rest.port }}
             protocol: TCP
           env:
           {{- range $key, $value := .Values.env }}
           - name: {{ $key }}
             value: {{ $value | quote }}
           {{- end }}
           - name: AKIDB_REST_PORT
             value: {{ .Values.rest.port | quote }}
           - name: AKIDB_MODEL_PATH
             value: "{{ .Values.models.persistentVolume.mountPath }}/qwen3-4b-fp8.onnx"
           - name: AKIDB_TOKENIZER_PATH
             value: "{{ .Values.models.persistentVolume.mountPath }}/tokenizer.json"
           - name: AKIDB_TENSORRT_CACHE
             value: {{ .Values.tensorrtCache.persistentVolume.mountPath }}
           volumeMounts:
           - name: models
             mountPath: {{ .Values.models.persistentVolume.mountPath }}
             readOnly: true
           - name: tensorrt-cache
             mountPath: {{ .Values.tensorrtCache.persistentVolume.mountPath }}
           resources:
             {{- toYaml .Values.rest.resources | nindent 12 }}
           livenessProbe:
             httpGet:
               path: /health
               port: http
             initialDelaySeconds: 60
             periodSeconds: 30
             timeoutSeconds: 5
           readinessProbe:
             httpGet:
               path: /health
               port: http
             initialDelaySeconds: 10
             periodSeconds: 10
             timeoutSeconds: 3
         volumes:
         - name: models
           persistentVolumeClaim:
             claimName: {{ .Release.Name }}-models
         - name: tensorrt-cache
           persistentVolumeClaim:
             claimName: {{ .Release.Name }}-tensorrt-cache
         nodeSelector:
           nvidia.com/gpu.present: "true"
         tolerations:
         - key: nvidia.com/gpu
           operator: Exists
           effect: NoSchedule
   ```

5. **service-rest.yaml**
   ```yaml
   apiVersion: v1
   kind: Service
   metadata:
     name: {{ .Release.Name }}-rest
     labels:
       app: akidb-rest
       release: {{ .Release.Name }}
   spec:
     type: ClusterIP
     ports:
     - port: {{ .Values.rest.port }}
       targetPort: http
       protocol: TCP
       name: http
     selector:
       app: akidb-rest
       release: {{ .Release.Name }}
   ```

6. **hpa.yaml** (Horizontal Pod Autoscaler)
   ```yaml
   {{- if .Values.autoscaling.enabled }}
   apiVersion: autoscaling/v2
   kind: HorizontalPodAutoscaler
   metadata:
     name: {{ .Release.Name }}-rest
   spec:
     scaleTargetRef:
       apiVersion: apps/v1
       kind: Deployment
       name: {{ .Release.Name }}-rest
     minReplicas: {{ .Values.autoscaling.minReplicas }}
     maxReplicas: {{ .Values.autoscaling.maxReplicas }}
     metrics:
     - type: Resource
       resource:
         name: cpu
         target:
           type: Utilization
           averageUtilization: {{ .Values.autoscaling.targetCPUUtilizationPercentage }}
     - type: Pods
       pods:
         metric:
           name: nvidia_gpu_duty_cycle
         target:
           type: AverageValue
           averageValue: {{ .Values.autoscaling.targetGPUUtilizationPercentage }}
   {{- end }}
   ```

7. **Deploy Script** (`scripts/deploy-k8s-jetson.sh`)
   ```bash
   #!/bin/bash
   set -e

   NAMESPACE=${1:-akidb}
   RELEASE_NAME=${2:-akidb-jetson}

   echo "Deploying AkiDB to Kubernetes..."

   # Create namespace
   kubectl create namespace $NAMESPACE --dry-run=client -o yaml | kubectl apply -f -

   # Install NVIDIA Device Plugin (if not already installed)
   kubectl apply -f https://raw.githubusercontent.com/NVIDIA/k8s-device-plugin/v0.16.2/deployments/static/nvidia-device-plugin.yml

   # Install Helm chart
   helm upgrade --install $RELEASE_NAME deploy/helm/akidb-jetson \
     --namespace $NAMESPACE \
     --create-namespace \
     --wait \
     --timeout 10m

   echo "✅ Deployment complete!"

   # Show status
   kubectl get pods -n $NAMESPACE
   kubectl get svc -n $NAMESPACE
   ```

8. **Testing Kubernetes Deployment**
   ```bash
   # Deploy to cluster
   bash scripts/deploy-k8s-jetson.sh akidb akidb-jetson

   # Check pods
   kubectl get pods -n akidb

   # Check logs
   kubectl logs -n akidb -l app=akidb-rest -f

   # Port-forward for testing
   kubectl port-forward -n akidb svc/akidb-jetson-rest 8080:8080

   # Test API
   curl -X POST http://localhost:8080/api/v1/embed \
     -H "Content-Type: application/json" \
     -d '{"inputs": ["Kubernetes test"]}'
   ```

**Success Criteria:**
- ✅ Helm chart installs successfully
- ✅ Pods start and become Ready
- ✅ GPU resources allocated correctly
- ✅ Health checks pass
- ✅ Services accessible via ClusterIP
- ✅ PersistentVolumes mounted correctly

**Completion Report:** `automatosx/tmp/jetson-thor-week5-day3-kubernetes-complete.md`

---

### Day 4: Observability Integration

**Objective:** Implement comprehensive observability with Prometheus, Grafana, and OpenTelemetry

**Tasks:**

1. **Add Prometheus Metrics** (`crates/akidb-rest/src/metrics.rs`, ~150 lines)

   ```rust
   use prometheus::{
       Counter, Histogram, IntGauge, Registry, Encoder, TextEncoder,
       HistogramOpts, Opts,
   };
   use lazy_static::lazy_static;

   lazy_static! {
       pub static ref REGISTRY: Registry = Registry::new();

       // Request counters
       pub static ref EMBED_REQUESTS_TOTAL: CounterVec = CounterVec::new(
           Opts::new("akidb_embed_requests_total", "Total embedding requests"),
           &["model", "status"]
       ).unwrap();

       // Latency histograms
       pub static ref EMBED_LATENCY: HistogramVec = HistogramVec::new(
           HistogramOpts::new("akidb_embed_latency_seconds", "Embedding latency")
               .buckets(vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]),
           &["model"]
       ).unwrap();

       // Model cache metrics
       pub static ref MODEL_CACHE_HITS: CounterVec = CounterVec::new(
           Opts::new("akidb_model_cache_hits_total", "Model cache hits"),
           &["model"]
       ).unwrap();

       pub static ref MODEL_CACHE_MISSES: CounterVec = CounterVec::new(
           Opts::new("akidb_model_cache_misses_total", "Model cache misses"),
           &["model"]
       ).unwrap();

       pub static ref MODEL_CACHE_SIZE: IntGauge = IntGauge::new(
           "akidb_model_cache_size", "Current model cache size"
       ).unwrap();

       pub static ref MODEL_LOAD_DURATION: HistogramVec = HistogramVec::new(
           HistogramOpts::new("akidb_model_load_seconds", "Model load duration")
               .buckets(vec![0.1, 0.5, 1.0, 5.0, 10.0, 30.0, 60.0]),
           &["model"]
       ).unwrap();

       // GPU metrics
       pub static ref GPU_MEMORY_USED: IntGauge = IntGauge::new(
           "akidb_gpu_memory_used_bytes", "GPU memory used"
       ).unwrap();

       pub static ref GPU_UTILIZATION: IntGauge = IntGauge::new(
           "akidb_gpu_utilization_percent", "GPU utilization percentage"
       ).unwrap();

       // TensorRT metrics
       pub static ref TENSORRT_ENGINE_BUILDS: Counter = Counter::new(
           "akidb_tensorrt_engine_builds_total", "TensorRT engine builds"
       ).unwrap();

       pub static ref TENSORRT_ENGINE_CACHE_HITS: Counter = Counter::new(
           "akidb_tensorrt_engine_cache_hits_total", "TensorRT engine cache hits"
       ).unwrap();

       // Batch metrics
       pub static ref BATCH_SIZE: HistogramVec = HistogramVec::new(
           HistogramOpts::new("akidb_batch_size", "Request batch size")
               .buckets(vec![1.0, 2.0, 4.0, 8.0, 16.0, 32.0]),
           &["model"]
       ).unwrap();

       pub static ref TOKENS_PROCESSED: CounterVec = CounterVec::new(
           Opts::new("akidb_tokens_processed_total", "Total tokens processed"),
           &["model"]
       ).unwrap();
   }

   pub fn register_metrics() {
       REGISTRY.register(Box::new(EMBED_REQUESTS_TOTAL.clone())).unwrap();
       REGISTRY.register(Box::new(EMBED_LATENCY.clone())).unwrap();
       REGISTRY.register(Box::new(MODEL_CACHE_HITS.clone())).unwrap();
       REGISTRY.register(Box::new(MODEL_CACHE_MISSES.clone())).unwrap();
       REGISTRY.register(Box::new(MODEL_CACHE_SIZE.clone())).unwrap();
       REGISTRY.register(Box::new(MODEL_LOAD_DURATION.clone())).unwrap();
       REGISTRY.register(Box::new(GPU_MEMORY_USED.clone())).unwrap();
       REGISTRY.register(Box::new(GPU_UTILIZATION.clone())).unwrap();
       REGISTRY.register(Box::new(TENSORRT_ENGINE_BUILDS.clone())).unwrap();
       REGISTRY.register(Box::new(TENSORRT_ENGINE_CACHE_HITS.clone())).unwrap();
       REGISTRY.register(Box::new(BATCH_SIZE.clone())).unwrap();
       REGISTRY.register(Box::new(TOKENS_PROCESSED.clone())).unwrap();
   }

   pub async fn metrics_handler() -> String {
       let encoder = TextEncoder::new();
       let metric_families = REGISTRY.gather();
       let mut buffer = vec![];
       encoder.encode(&metric_families, &mut buffer).unwrap();
       String::from_utf8(buffer).unwrap()
   }
   ```

2. **Add OpenTelemetry Tracing** (`crates/akidb-rest/src/tracing.rs`)

   ```rust
   use opentelemetry::{global, sdk::trace, trace::TracerProvider};
   use opentelemetry_otlp::WithExportConfig;
   use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

   pub fn init_telemetry() -> Result<(), Box<dyn std::error::Error>> {
       // OTLP exporter (to Grafana Tempo or Jaeger)
       let otlp_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
           .unwrap_or_else(|_| "http://localhost:4317".to_string());

       let tracer = opentelemetry_otlp::new_pipeline()
           .tracing()
           .with_exporter(
               opentelemetry_otlp::new_exporter()
                   .tonic()
                   .with_endpoint(otlp_endpoint)
           )
           .with_trace_config(
               trace::config()
                   .with_resource(opentelemetry::sdk::Resource::new(vec![
                       opentelemetry::KeyValue::new("service.name", "akidb-rest"),
                       opentelemetry::KeyValue::new("service.version", "2.0.0-jetson-thor"),
                   ]))
           )
           .install_batch(opentelemetry::runtime::Tokio)?;

       // Tracing subscriber
       tracing_subscriber::registry()
           .with(tracing_subscriber::EnvFilter::from_default_env())
           .with(tracing_subscriber::fmt::layer())
           .with(tracing_opentelemetry::layer().with_tracer(tracer))
           .init();

       Ok(())
   }

   pub fn shutdown_telemetry() {
       global::shutdown_tracer_provider();
   }
   ```

3. **Instrument Embedding Handler** (add spans)

   ```rust
   use tracing::{info, instrument};

   #[instrument(skip(manager), fields(model = %req.model, batch_size = req.inputs.len()))]
   pub async fn embed(
       State(manager): State<Arc<EmbeddingManager>>,
       Json(req): Json<EmbedRequest>,
   ) -> Result<Json<EmbedResponse>, ApiError> {
       let start = std::time::Instant::now();

       info!("Starting embedding request");

       // Get provider (creates span: "get_provider")
       let provider = manager.get_provider(&req.model).await?;

       // Generate embeddings (creates span: "embed_batch")
       let batch_resp = provider.embed_batch(batch_req).await?;

       let duration_ms = start.elapsed().as_millis() as u64;

       info!(duration_ms = duration_ms, "Embedding request completed");

       // Record metrics...

       Ok(Json(response))
   }
   ```

4. **Create Grafana Dashboard** (`deploy/grafana/akidb-jetson-dashboard.json`)

   Panels:
   - **Request Rate:** `rate(akidb_embed_requests_total[5m])`
   - **P95 Latency:** `histogram_quantile(0.95, akidb_embed_latency_seconds)`
   - **Error Rate:** `rate(akidb_embed_requests_total{status="error"}[5m])`
   - **Model Cache Hit Rate:** `rate(akidb_model_cache_hits_total[5m]) / rate(akidb_model_cache_hits_total[5m] + akidb_model_cache_misses_total[5m])`
   - **GPU Memory:** `akidb_gpu_memory_used_bytes`
   - **GPU Utilization:** `akidb_gpu_utilization_percent`
   - **Batch Size Distribution:** `histogram_quantile(0.95, akidb_batch_size)`

5. **Deploy Observability Stack** (`deploy/k8s/observability.yaml`)

   ```yaml
   apiVersion: v1
   kind: ConfigMap
   metadata:
     name: prometheus-config
     namespace: akidb
   data:
     prometheus.yml: |
       global:
         scrape_interval: 15s
       scrape_configs:
       - job_name: 'akidb-rest'
         kubernetes_sd_configs:
         - role: pod
           namespaces:
             names:
             - akidb
         relabel_configs:
         - source_labels: [__meta_kubernetes_pod_label_app]
           action: keep
           regex: akidb-rest
         - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_scrape]
           action: keep
           regex: true
         - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_path]
           action: replace
           target_label: __metrics_path__
           regex: (.+)
         - source_labels: [__address__, __meta_kubernetes_pod_annotation_prometheus_io_port]
           action: replace
           regex: ([^:]+)(?::\d+)?;(\d+)
           replacement: $1:$2
           target_label: __address__

   ---
   apiVersion: apps/v1
   kind: Deployment
   metadata:
     name: prometheus
     namespace: akidb
   spec:
     replicas: 1
     selector:
       matchLabels:
         app: prometheus
     template:
       metadata:
         labels:
           app: prometheus
       spec:
         containers:
         - name: prometheus
           image: prom/prometheus:v2.47.0
           args:
           - '--config.file=/etc/prometheus/prometheus.yml'
           - '--storage.tsdb.path=/prometheus'
           ports:
           - containerPort: 9090
           volumeMounts:
           - name: config
             mountPath: /etc/prometheus
           - name: storage
             mountPath: /prometheus
         volumes:
         - name: config
           configMap:
             name: prometheus-config
         - name: storage
           emptyDir: {}

   ---
   apiVersion: v1
   kind: Service
   metadata:
     name: prometheus
     namespace: akidb
   spec:
     ports:
     - port: 9090
       targetPort: 9090
     selector:
       app: prometheus
   ```

6. **Testing Observability**
   ```bash
   # Deploy observability stack
   kubectl apply -f deploy/k8s/observability.yaml

   # Port-forward Prometheus
   kubectl port-forward -n akidb svc/prometheus 9090:9090

   # Query metrics
   curl http://localhost:9090/api/v1/query?query=akidb_embed_requests_total

   # Port-forward Grafana (if deployed)
   kubectl port-forward -n akidb svc/grafana 3000:3000
   ```

**Success Criteria:**
- ✅ Prometheus scrapes metrics successfully
- ✅ 15+ custom metrics exported
- ✅ Grafana dashboard displays real-time metrics
- ✅ OpenTelemetry traces collected (if enabled)
- ✅ Metrics overhead <1% latency impact

**Completion Report:** `automatosx/tmp/jetson-thor-week5-day4-observability-complete.md`

---

### Day 5: Load Testing & Production Validation

**Objective:** Validate production-ready performance with load testing and stress testing

**Tasks:**

1. **Create Load Test Script** (`scripts/load-test-jetson.sh`)

   ```bash
   #!/bin/bash
   set -e

   HOST=${1:-http://localhost:8080}
   DURATION=${2:-60}
   CONCURRENCY=${3:-10}

   echo "Load Testing AkiDB Jetson Thor Deployment"
   echo "Host: $HOST"
   echo "Duration: ${DURATION}s"
   echo "Concurrency: $CONCURRENCY"
   echo ""

   # Install wrk if not present
   if ! command -v wrk &> /dev/null; then
       echo "Installing wrk..."
       sudo apt-get update && sudo apt-get install -y wrk
   fi

   # Create wrk Lua script for embedding requests
   cat > /tmp/wrk-embed.lua <<'EOF'
   wrk.method = "POST"
   wrk.headers["Content-Type"] = "application/json"

   local texts = {
       "The autonomous vehicle detects pedestrians using LiDAR.",
       "Emergency braking system activated.",
       "Robotic arm picks up component from assembly line.",
       "Quality inspection complete. No defects detected.",
       "AGV navigates to loading dock station 3."
   }

   local counter = 0

   request = function()
       counter = counter + 1
       local text = texts[(counter % #texts) + 1]
       local body = string.format('{"inputs":["%s"],"model":"Qwen/Qwen2.5-4B"}', text)
       return wrk.format(nil, "/api/v1/embed", nil, body)
   end

   response = function(status, headers, body)
       if status ~= 200 then
           print("Error: " .. status)
           print(body)
       end
   end
   EOF

   # Run load test
   echo "Starting load test..."
   wrk -t $CONCURRENCY -c $CONCURRENCY -d ${DURATION}s \
       -s /tmp/wrk-embed.lua \
       $HOST/api/v1/embed

   echo ""
   echo "Load test complete!"
   ```

2. **Run Performance Benchmarks**

   ```bash
   # Test 1: Single-threaded baseline
   echo "=== Test 1: Single-threaded (10 QPS target) ==="
   bash scripts/load-test-jetson.sh http://localhost:8080 60 1

   # Test 2: Medium concurrency (50 QPS target)
   echo "=== Test 2: Medium concurrency (50 QPS target) ==="
   bash scripts/load-test-jetson.sh http://localhost:8080 60 5

   # Test 3: High concurrency (150+ QPS target)
   echo "=== Test 3: High concurrency (150+ QPS target) ==="
   bash scripts/load-test-jetson.sh http://localhost:8080 60 15

   # Test 4: Stress test (find breaking point)
   echo "=== Test 4: Stress test (30 concurrent) ==="
   bash scripts/load-test-jetson.sh http://localhost:8080 120 30
   ```

3. **Create Benchmark Report Script** (`scripts/generate-benchmark-report.py`)

   ```python
   import json
   import subprocess
   import time
   import statistics

   def run_benchmark(host, duration, concurrency):
       """Run single benchmark and return results"""
       cmd = [
           "wrk",
           "-t", str(concurrency),
           "-c", str(concurrency),
           "-d", f"{duration}s",
           "-s", "/tmp/wrk-embed.lua",
           "--latency",
           f"{host}/api/v1/embed"
       ]

       result = subprocess.run(cmd, capture_output=True, text=True)
       return result.stdout

   def parse_wrk_output(output):
       """Parse wrk output and extract metrics"""
       lines = output.split('\n')

       metrics = {
           'requests_total': 0,
           'duration_sec': 0,
           'throughput_qps': 0,
           'latency_mean_ms': 0,
           'latency_p50_ms': 0,
           'latency_p75_ms': 0,
           'latency_p90_ms': 0,
           'latency_p95_ms': 0,
           'latency_p99_ms': 0,
       }

       for line in lines:
           if 'Requests/sec:' in line:
               metrics['throughput_qps'] = float(line.split()[-1])
           elif 'Latency' in line and 'Avg' in line:
               # Parse: Latency     10.23ms   12.45ms   50.00ms   95.00%
               parts = line.split()
               metrics['latency_mean_ms'] = parse_ms(parts[1])
           elif '50%' in line:
               metrics['latency_p50_ms'] = parse_ms(line.split()[-1])
           elif '75%' in line:
               metrics['latency_p75_ms'] = parse_ms(line.split()[-1])
           elif '90%' in line:
               metrics['latency_p90_ms'] = parse_ms(line.split()[-1])
           elif '95%' in line:
               metrics['latency_p95_ms'] = parse_ms(line.split()[-1])
           elif '99%' in line:
               metrics['latency_p99_ms'] = parse_ms(line.split()[-1])

       return metrics

   def parse_ms(value):
       """Parse latency value (e.g., '10.23ms' -> 10.23)"""
       if 'ms' in value:
           return float(value.replace('ms', ''))
       elif 's' in value:
           return float(value.replace('s', '')) * 1000
       return 0.0

   def main():
       host = "http://localhost:8080"

       tests = [
           {"name": "Low Load (1 concurrent)", "concurrency": 1, "duration": 60},
           {"name": "Medium Load (5 concurrent)", "concurrency": 5, "duration": 60},
           {"name": "High Load (10 concurrent)", "concurrency": 10, "duration": 60},
           {"name": "Peak Load (15 concurrent)", "concurrency": 15, "duration": 60},
           {"name": "Stress Test (30 concurrent)", "concurrency": 30, "duration": 120},
       ]

       results = []

       for test in tests:
           print(f"\nRunning: {test['name']}...")
           output = run_benchmark(host, test['duration'], test['concurrency'])
           metrics = parse_wrk_output(output)
           metrics['test_name'] = test['name']
           metrics['concurrency'] = test['concurrency']
           results.append(metrics)

           # Print summary
           print(f"  QPS: {metrics['throughput_qps']:.2f}")
           print(f"  P95: {metrics['latency_p95_ms']:.2f}ms")
           print(f"  P99: {metrics['latency_p99_ms']:.2f}ms")

           time.sleep(5)  # Cool-down period

       # Generate report
       report = {
           "timestamp": time.strftime("%Y-%m-%d %H:%M:%S"),
           "platform": "NVIDIA Jetson Thor",
           "model": "Qwen/Qwen2.5-4B",
           "execution_provider": "TensorRT FP8",
           "results": results
       }

       with open("/tmp/benchmark-results.json", "w") as f:
           json.dump(report, f, indent=2)

       print("\n✅ Benchmark complete! Results saved to /tmp/benchmark-results.json")

       # Check if targets met
       peak_test = results[3]  # Peak Load test
       if peak_test['throughput_qps'] >= 50 and peak_test['latency_p95_ms'] <= 30:
           print("✅ Performance targets MET: >50 QPS @ <30ms P95")
       else:
           print("❌ Performance targets NOT MET")
           print(f"   Expected: >50 QPS @ <30ms P95")
           print(f"   Actual: {peak_test['throughput_qps']:.2f} QPS @ {peak_test['latency_p95_ms']:.2f}ms P95")

   if __name__ == "__main__":
       main()
   ```

4. **Run Full Benchmark Suite**
   ```bash
   python3 scripts/generate-benchmark-report.py
   ```

5. **Validation Checklist**

   Create validation report:
   ```bash
   # Create validation script
   cat > scripts/validate-production.sh <<'EOF'
   #!/bin/bash

   echo "AkiDB Jetson Thor Production Validation"
   echo "========================================"
   echo ""

   ERRORS=0

   # Test 1: Health check
   echo "1. Health Check..."
   HEALTH=$(curl -s http://localhost:8080/health | jq -r '.status')
   if [ "$HEALTH" = "healthy" ]; then
       echo "   ✅ Health check PASS"
   else
       echo "   ❌ Health check FAIL"
       ERRORS=$((ERRORS + 1))
   fi

   # Test 2: Embedding generation
   echo "2. Embedding Generation..."
   EMBED_DIM=$(curl -s -X POST http://localhost:8080/api/v1/embed \
       -H "Content-Type: application/json" \
       -d '{"inputs":["test"]}' | jq '.embeddings[0] | length')
   if [ "$EMBED_DIM" = "4096" ]; then
       echo "   ✅ Embedding dimension correct (4096)"
   else
       echo "   ❌ Embedding dimension incorrect: $EMBED_DIM"
       ERRORS=$((ERRORS + 1))
   fi

   # Test 3: Model registry
   echo "3. Model Registry..."
   MODEL_COUNT=$(curl -s http://localhost:8080/models | jq '.models | length')
   if [ "$MODEL_COUNT" -ge "1" ]; then
       echo "   ✅ Model registry available ($MODEL_COUNT models)"
   else
       echo "   ❌ Model registry empty"
       ERRORS=$((ERRORS + 1))
   fi

   # Test 4: Prometheus metrics
   echo "4. Prometheus Metrics..."
   METRICS=$(curl -s http://localhost:8080/metrics | grep akidb_embed_requests_total | wc -l)
   if [ "$METRICS" -gt "0" ]; then
       echo "   ✅ Metrics endpoint working"
   else
       echo "   ❌ Metrics endpoint failed"
       ERRORS=$((ERRORS + 1))
   fi

   # Test 5: GPU availability
   echo "5. GPU Availability..."
   if nvidia-smi &> /dev/null; then
       echo "   ✅ GPU accessible"
   else
       echo "   ❌ GPU not accessible"
       ERRORS=$((ERRORS + 1))
   fi

   # Test 6: TensorRT cache
   echo "6. TensorRT Engine Cache..."
   if [ -d "/var/cache/akidb/trt" ] && [ "$(ls -A /var/cache/akidb/trt)" ]; then
       echo "   ✅ TensorRT engine cached"
   else
       echo "   ⚠️  TensorRT engine not cached (first run expected)"
   fi

   echo ""
   if [ $ERRORS -eq 0 ]; then
       echo "✅ All validation checks PASSED"
       exit 0
   else
       echo "❌ $ERRORS validation check(s) FAILED"
       exit 1
   fi
   EOF

   chmod +x scripts/validate-production.sh
   bash scripts/validate-production.sh
   ```

6. **Create Final Completion Report** (`automatosx/tmp/jetson-thor-week5-completion-report.md`)

   Template:
   ```markdown
   # Jetson Thor Week 5: Production Deployment - Completion Report

   **Date:** 2025-11-XX
   **Status:** ✅ COMPLETE
   **Duration:** 5 days

   ## Executive Summary

   Successfully deployed production-ready REST/gRPC API servers for Jetson Thor...

   ## Deliverables

   1. ✅ REST API Server - integrated with ONNX+TensorRT
   2. ✅ gRPC API Server - integrated with ONNX+TensorRT
   3. ✅ Docker images - <2GB compressed
   4. ✅ Kubernetes Helm chart - GPU scheduling, HPA
   5. ✅ Observability - Prometheus + Grafana
   6. ✅ Load testing - validated performance targets

   ## Performance Results

   | Metric | Target | Actual | Status |
   |--------|--------|--------|--------|
   | P95 Latency | <30ms | XXms | ✅ |
   | Throughput (peak) | >50 QPS | XX QPS | ✅ |
   | Throughput (concurrent) | >150 QPS | XX QPS | ✅ |
   | GPU Memory | <4GB | X.XGB | ✅ |
   | Docker Image | <2GB | X.XGB | ✅ |
   | Startup Time | <10s | Xs | ✅ |

   ## Next Steps (Week 6)

   - Production hardening (circuit breakers, rate limiting)
   - Multi-region deployment
   - Advanced security (mTLS, RBAC)
   - CI/CD pipeline automation
   ```

**Success Criteria:**
- ✅ Load test: >50 QPS @ <30ms P95 latency
- ✅ Load test: >150 QPS @ <50ms P95 (concurrent)
- ✅ Zero errors during 5-minute stress test
- ✅ GPU memory stable <4GB
- ✅ All validation checks pass
- ✅ Performance targets met or exceeded

**Completion Report:** `automatosx/tmp/jetson-thor-week5-completion-report.md`

---

## Docker & Kubernetes Deployment

### Production Docker Best Practices

1. **Multi-stage builds** - Separate builder and runtime images
2. **Minimal base images** - Use NVIDIA L4T base for Jetson
3. **Non-root user** - Run as `akidb` user (UID 1000)
4. **Health checks** - Built-in Docker HEALTHCHECK
5. **Volume mounts** - Models and cache as persistent volumes
6. **GPU support** - NVIDIA Container Runtime required
7. **Graceful shutdown** - Handle SIGTERM for rolling updates

### Kubernetes Requirements

1. **NVIDIA Device Plugin** - GPU scheduling
   ```bash
   kubectl apply -f https://raw.githubusercontent.com/NVIDIA/k8s-device-plugin/v0.16.2/deployments/static/nvidia-device-plugin.yml
   ```

2. **Local Storage** - For models and TensorRT cache
   ```yaml
   apiVersion: storage.k8s.io/v1
   kind: StorageClass
   metadata:
     name: local-path
   provisioner: rancher.io/local-path
   volumeBindingMode: WaitForFirstConsumer
   ```

3. **Node Selectors** - Ensure pods run on Jetson nodes
   ```yaml
   nodeSelector:
     nvidia.com/gpu.present: "true"
   ```

4. **Resource Limits** - Prevent OOM
   ```yaml
   resources:
     limits:
       nvidia.com/gpu: 1
       memory: 16Gi
     requests:
       nvidia.com/gpu: 1
       memory: 8Gi
   ```

---

## Observability Strategy

### Metrics to Monitor

**Service Level Indicators (SLIs):**
1. **Availability:** % of successful health checks (target: >99.9%)
2. **Latency:** P95 embedding latency (target: <30ms)
3. **Throughput:** Requests per second (target: >50 QPS)
4. **Error Rate:** % of failed requests (target: <0.1%)

**Resource Metrics:**
5. **GPU Utilization:** % GPU busy (target: 60-80%)
6. **GPU Memory:** Bytes used (target: <4GB)
7. **CPU Utilization:** % CPU busy (target: <50%)
8. **Memory Usage:** Bytes used (target: <8GB)

**Application Metrics:**
9. **Model Cache Hit Rate:** % cache hits (target: >90%)
10. **Model Load Time:** Cold start duration (target: <500ms)
11. **Batch Size:** Average batch size (monitor distribution)
12. **Token Count:** Tokens processed per request

**TensorRT Metrics:**
13. **Engine Builds:** Count of engine compilations (target: 1 per model)
14. **Engine Cache Hits:** Count of cached engine loads

### Alerting Rules

```yaml
groups:
- name: akidb-alerts
  interval: 30s
  rules:
  - alert: HighLatency
    expr: histogram_quantile(0.95, akidb_embed_latency_seconds) > 0.030
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "High P95 latency: {{ $value }}s"

  - alert: LowThroughput
    expr: rate(akidb_embed_requests_total[5m]) < 50
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "Low throughput: {{ $value }} QPS"

  - alert: HighErrorRate
    expr: rate(akidb_embed_requests_total{status="error"}[5m]) / rate(akidb_embed_requests_total[5m]) > 0.01
    for: 2m
    labels:
      severity: critical
    annotations:
      summary: "High error rate: {{ $value | humanizePercentage }}"

  - alert: GPUMemoryHigh
    expr: akidb_gpu_memory_used_bytes > 4_000_000_000
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "GPU memory high: {{ $value | humanize1024 }}"
```

---

## Performance Validation

### Expected Performance Targets

**Week 5 Targets (Production Deployment):**

| Metric | Target | Baseline (Week 3) | Improvement |
|--------|--------|-------------------|-------------|
| P95 Latency (single) | <30ms | <30ms | Maintained |
| P50 Latency (single) | <15ms | <15ms | Maintained |
| Throughput (single-threaded) | >50 QPS | >50 QPS | Maintained |
| Throughput (concurrent 15x) | >150 QPS | >150 QPS | Maintained |
| GPU Memory | <4GB | ~4GB | Maintained |
| Cold Start | <10s | N/A | New |
| Rolling Update Downtime | 0s | N/A | New |
| Request Error Rate | <0.1% | N/A | New |

**Load Test Scenarios:**

1. **Scenario 1: Sustained Load**
   - Duration: 5 minutes
   - Concurrency: 10
   - Expected QPS: 80-100
   - Expected P95: <30ms

2. **Scenario 2: Burst Load**
   - Duration: 1 minute
   - Concurrency: 30
   - Expected QPS: 200+
   - Expected P95: <50ms

3. **Scenario 3: Stress Test**
   - Duration: 10 minutes
   - Concurrency: 50
   - Find breaking point (P95 >100ms)

---

## Risk Management

### Risks and Mitigation

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| **Docker image >2GB** | High (slow deploys) | Medium | Multi-stage build, minimize dependencies |
| **TensorRT engine fails to compile** | Critical | Low | Pre-compile engines, include in image |
| **GPU OOM during high load** | High | Medium | Set memory limits, implement backpressure |
| **Kubernetes GPU scheduling fails** | Critical | Low | Test NVIDIA Device Plugin, add node affinity |
| **Prometheus metrics overhead** | Medium | Low | Use sampling, test with profiling |
| **Health check false positives** | Medium | Medium | Test under load, tune timeouts |
| **Rolling update downtime** | High | Low | Use readiness probes, pre-warm cache |
| **Model load timeout (cold start)** | Medium | Medium | Increase readiness probe timeout to 60s |

### Production Readiness Checklist

**Deployment:**
- [ ] Docker images build successfully
- [ ] Images <2GB compressed
- [ ] TensorRT engines cached in image or volume
- [ ] Kubernetes Helm chart installs
- [ ] GPU resources allocated correctly
- [ ] PersistentVolumes mounted

**Reliability:**
- [ ] Health checks pass under load
- [ ] Graceful shutdown (<1s)
- [ ] Circuit breakers configured
- [ ] Retry logic implemented
- [ ] Timeout handling correct
- [ ] Error messages informative

**Performance:**
- [ ] P95 latency <30ms @ 50 QPS
- [ ] Throughput >150 QPS concurrent
- [ ] GPU memory stable <4GB
- [ ] No memory leaks (5min+ test)
- [ ] Cold start <10s

**Observability:**
- [ ] Prometheus metrics exported
- [ ] Grafana dashboard created
- [ ] Alerts configured
- [ ] Logs structured (JSON)
- [ ] Tracing enabled (optional)

**Security:**
- [ ] Non-root user in container
- [ ] No secrets in image
- [ ] Read-only filesystem (where possible)
- [ ] Resource limits set
- [ ] Network policies defined (optional)

---

## Success Criteria

### Week 5 Completion Criteria

| Criterion | Target | Measurement | Priority |
|-----------|--------|-------------|----------|
| **REST API Deployed** | ✅ | Server responds on :8080 | P0 |
| **gRPC API Deployed** | ✅ | Server responds on :9090 | P0 |
| **Docker Images** | <2GB | `docker images` | P0 |
| **Kubernetes Deployment** | ✅ | Helm chart installs, pods Ready | P0 |
| **Health Checks** | ✅ | Pass under load | P0 |
| **P95 Latency** | <30ms | Load test @ 50 QPS | P0 |
| **Throughput** | >50 QPS | Single-threaded | P0 |
| **Concurrent Throughput** | >150 QPS | 15x concurrent | P0 |
| **GPU Memory** | <4GB | nvidia-smi | P0 |
| **Prometheus Metrics** | 15+ metrics | /metrics endpoint | P1 |
| **Grafana Dashboard** | ✅ | Dashboard created | P1 |
| **Load Test Suite** | ✅ | Scripts + report | P1 |
| **Zero Errors** | ✅ | 5min stress test | P1 |
| **Rolling Updates** | 0s downtime | kubectl rollout | P2 |
| **OpenTelemetry** | ✅ | Tracing enabled | P2 |

**Overall Success:** All P0 criteria + 80% of P1 criteria + 50% of P2 criteria

---

## Appendix: Code Examples

### Example 1: Full REST Server with ONNX+TensorRT

```rust
// crates/akidb-rest/src/main.rs
use akidb_embedding::{OnnxConfig, ExecutionProviderConfig, OnnxEmbeddingProvider};
use akidb_service::embedding_manager::EmbeddingManager;
use axum::{
    routing::{get, post},
    Router,
    extract::State,
    Json,
};
use std::sync::Arc;
use std::path::PathBuf;
use tokio::signal;

mod handlers;
mod metrics;
mod tracing_setup;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize telemetry
    tracing_setup::init_telemetry()?;

    // Register Prometheus metrics
    metrics::register_metrics();

    // Load ONNX provider configuration
    let config = OnnxConfig {
        model_path: PathBuf::from(
            std::env::var("AKIDB_MODEL_PATH")
                .unwrap_or_else(|_| "/opt/akidb/models/qwen3-4b-fp8.onnx".to_string())
        ),
        tokenizer_path: PathBuf::from(
            std::env::var("AKIDB_TOKENIZER_PATH")
                .unwrap_or_else(|_| "/opt/akidb/models/tokenizer.json".to_string())
        ),
        model_name: std::env::var("AKIDB_MODEL_NAME")
            .unwrap_or_else(|_| "Qwen/Qwen2.5-4B".to_string()),
        dimension: std::env::var("AKIDB_MODEL_DIMENSION")
            .unwrap_or_else(|_| "4096".to_string())
            .parse()?,
        max_length: 512,
        execution_provider: ExecutionProviderConfig::TensorRT {
            device_id: 0,
            fp8_enable: true,
            engine_cache_path: Some(PathBuf::from(
                std::env::var("AKIDB_TENSORRT_CACHE")
                    .unwrap_or_else(|_| "/var/cache/akidb/trt".to_string())
            )),
        },
    };

    tracing::info!("Initializing ONNX provider with TensorRT...");
    let provider = Arc::new(OnnxEmbeddingProvider::with_config(config).await?);

    // Create embedding manager
    let manager = Arc::new(EmbeddingManager::new(provider));

    // Build router
    let app = Router::new()
        .route("/health", get(handlers::health))
        .route("/metrics", get(handlers::metrics))
        .route("/models", get(handlers::list_models))
        .route("/api/v1/embed", post(handlers::embed))
        .with_state(manager);

    // Start server
    let host = std::env::var("AKIDB_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("AKIDB_REST_PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()?;

    let addr = format!("{}:{}", host, port);
    tracing::info!("Starting REST server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // Cleanup
    tracing_setup::shutdown_telemetry();

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received, starting graceful shutdown");
}
```

### Example 2: Load Test Analysis Script

```python
# scripts/analyze-load-test.py
import json
import sys
from datetime import datetime

def analyze_results(results_file):
    with open(results_file, 'r') as f:
        data = json.load(f)

    print("=" * 60)
    print("AkiDB Jetson Thor Load Test Analysis")
    print("=" * 60)
    print(f"Platform: {data['platform']}")
    print(f"Model: {data['model']}")
    print(f"Timestamp: {data['timestamp']}")
    print("")

    # Analyze each test
    for result in data['results']:
        print(f"\n{result['test_name']}")
        print("-" * 60)
        print(f"  Concurrency: {result['concurrency']}")
        print(f"  Throughput: {result['throughput_qps']:.2f} QPS")
        print(f"  Latency P50: {result['latency_p50_ms']:.2f}ms")
        print(f"  Latency P95: {result['latency_p95_ms']:.2f}ms")
        print(f"  Latency P99: {result['latency_p99_ms']:.2f}ms")

        # Check targets
        if result['concurrency'] == 10:  # Peak load test
            if result['throughput_qps'] >= 50:
                print("  ✅ Throughput target MET (>50 QPS)")
            else:
                print(f"  ❌ Throughput target MISSED (got {result['throughput_qps']:.2f} QPS)")

            if result['latency_p95_ms'] <= 30:
                print("  ✅ Latency target MET (<30ms P95)")
            else:
                print(f"  ❌ Latency target MISSED (got {result['latency_p95_ms']:.2f}ms P95)")

    # Overall summary
    print("\n" + "=" * 60)
    peak = [r for r in data['results'] if r['concurrency'] == 10][0]
    if peak['throughput_qps'] >= 50 and peak['latency_p95_ms'] <= 30:
        print("✅ OVERALL: Production targets MET")
        return 0
    else:
        print("❌ OVERALL: Production targets NOT MET")
        return 1

if __name__ == "__main__":
    results_file = sys.argv[1] if len(sys.argv) > 1 else "/tmp/benchmark-results.json"
    exit_code = analyze_results(results_file)
    sys.exit(exit_code)
```

---

**End of Week 5 PRD**

**Next Steps:** Week 6 - Production Hardening (Circuit Breakers, Rate Limiting, Multi-Region)
