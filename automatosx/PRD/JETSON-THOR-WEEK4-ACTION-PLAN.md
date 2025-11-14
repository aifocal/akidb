# Jetson Thor Week 4: Multi-Model Support - 5-Day Action Plan

**Status:** Ready to Execute
**Timeline:** 5 days
**Team:** 1 backend engineer
**Goal:** Support 6 embedding models with LRU caching and runtime selection

---

## Day-by-Day Summary

| Day | Focus | Key Tasks | Deliverable | Time |
|-----|-------|-----------|-------------|------|
| **Mon** | Model Registry | Registry + metadata + convert 5 models | 6 models ready | 7h |
| **Tue** | LRU Cache | Cache implementation + eviction logic | Cache working | 7h |
| **Wed** | Router & API | Model router + REST API integration | API ready | 7h |
| **Thu** | Benchmarking | Per-model benchmarks + cache performance | Performance profiles | 7h |
| **Fri** | Testing & Docs | E2E tests + completion report + API docs | Documentation | 7h |

**Total:** 35 hours (1 engineer × 5 days)

---

## Day 1: Model Registry & Conversion (Monday)

### Morning Tasks (3 hours)

**Task 1: Create Model Registry** [2 hours]
```bash
cd ~/akidb2

# Create registry module
cat > crates/akidb-embedding/src/registry.rs << 'EOF'
use std::collections::HashMap;
use std::path::PathBuf;

pub struct ModelMetadata {
    pub id: String,
    pub name: String,
    pub family: ModelFamily,
    pub params: u64,
    pub dimension: u32,
    pub max_length: usize,
    pub memory_mb: usize,
    pub latency_p95_ms: u32,
    pub throughput_qps: u32,
    pub multilingual: bool,
    pub languages: Vec<String>,
    pub license: String,
    pub model_path: PathBuf,
    pub tokenizer_path: PathBuf,
}

pub enum ModelFamily {
    Qwen3,
    E5,
    BGE,
}

pub struct ModelRegistry {
    models: HashMap<String, ModelMetadata>,
    default_model: String,
}

impl ModelRegistry {
    pub fn new() -> Self {
        let mut models = HashMap::new();

        // Register 6 models (qwen3-0.5b, 1.5b, 4b, 7b, e5-small-v2, bge-small-en-v1.5)
        // ... (see PRD for full implementation)

        Self {
            models,
            default_model: "qwen3-4b".to_string(),
        }
    }

    pub fn get(&self, model_id: &str) -> Option<&ModelMetadata> {
        self.models.get(model_id)
    }

    pub fn list(&self) -> Vec<&ModelMetadata> {
        self.models.values().collect()
    }
}
EOF

# Build and test
cargo build --release -p akidb-embedding --features onnx
cargo test -p akidb-embedding --features onnx registry -- --nocapture
```

**Success Metric:** Registry compiles, 6 models registered

**Task 2: Export Model Metadata JSON** [1 hour]
```bash
# Add JSON export to registry
cargo run -p akidb-cli -- list-models > /tmp/models.json
cat /tmp/models.json

# Verify all 6 models present
jq '.models | length' /tmp/models.json  # Should output: 6
```

### Afternoon Tasks (4 hours)

**Task 3: Convert 5 Additional Models to ONNX** [4 hours]
```bash
cd /opt/akidb

# Bulk conversion script
cat > scripts/convert_all_models.sh << 'SCRIPT'
#!/bin/bash
set -e

MODELS=(
  "Qwen/Qwen2.5-0.5B:qwen3-0.5b:896"
  "Qwen/Qwen2.5-1.5B:qwen3-1.5b:1536"
  "Qwen/Qwen2.5-7B:qwen3-7b:4096"
  "intfloat/e5-small-v2:e5-small-v2:384"
  "BAAI/bge-small-en-v1.5:bge-small-en-v1.5:384"
)

for model_spec in "${MODELS[@]}"; do
  IFS=':' read -r model_name model_id dimension <<< "$model_spec"

  echo "🔧 Converting $model_name → $model_id"
  output_dir="/opt/akidb/models/${model_id}-onnx-fp8"
  mkdir -p "$output_dir"

  python3 << EOF
from optimum.onnxruntime import ORTModelForFeatureExtraction
from transformers import AutoTokenizer

model_name = "$model_name"
output_dir = "$output_dir"

print(f"Loading {model_name}...")
model = ORTModelForFeatureExtraction.from_pretrained(
    model_name,
    export=True,
    provider="TensorrtExecutionProvider",
    provider_options={"trt_fp8_enable": "Qwen" in model_name}
)

tokenizer = AutoTokenizer.from_pretrained(model_name)

print(f"Saving to {output_dir}...")
model.save_pretrained(output_dir)
tokenizer.save_pretrained(output_dir)

print(f"✅ {model_name}")
EOF

done

echo "✅ All models converted!"
SCRIPT

chmod +x scripts/convert_all_models.sh

# Run conversion (~1 hour: 5 models × 12 min each)
./scripts/convert_all_models.sh 2>&1 | tee /tmp/model_conversion.log
```

**Success Metric:** 5 additional models converted, 6 total models ready

**Day 1 Deliverable:** Model registry + 6 ONNX models

---

## Day 2: LRU Cache Implementation (Tuesday)

### Morning Tasks (4 hours)

**Task 1: LRU Cache Structure** [2 hours]
```bash
cd ~/akidb2

# Add LRU dependency to Cargo.toml
echo 'lru = "0.12"' >> crates/akidb-embedding/Cargo.toml

# Create cache module
cat > crates/akidb-embedding/src/cache.rs << 'EOF'
use lru::LruCache;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct ModelCache {
    cache: Arc<RwLock<LruCache<String, Arc<OnnxEmbeddingProvider>>>>,
    capacity: usize,              // Max 3 models
    memory_limit_mb: usize,       // Max 8000 MB (8GB)
    current_memory_mb: Arc<RwLock<usize>>,
    registry: Arc<ModelRegistry>,
}

impl ModelCache {
    pub fn new(capacity: usize, memory_limit_mb: usize, registry: Arc<ModelRegistry>) -> Self {
        Self {
            cache: Arc::new(RwLock::new(LruCache::new(capacity))),
            capacity,
            memory_limit_mb,
            current_memory_mb: Arc::new(RwLock::new(0)),
            registry,
        }
    }
}
EOF

cargo build --release -p akidb-embedding --features onnx
```

**Task 2: Get/Load Logic** [2 hours]
```rust
// Add to cache.rs

impl ModelCache {
    pub async fn get_or_load(
        &self,
        model_id: &str,
    ) -> Result<Arc<OnnxEmbeddingProvider>, EmbeddingError> {
        // Check cache
        {
            let mut cache = self.cache.write().await;
            if let Some(provider) = cache.get(model_id) {
                return Ok(provider.clone());
            }
        }

        // Get metadata
        let metadata = self.registry.get(model_id)
            .ok_or_else(|| EmbeddingError::ModelNotFound(model_id.to_string()))?;

        // Evict if needed
        self.evict_if_needed(metadata.memory_mb).await?;

        // Load model
        let provider = self.load_model(metadata).await?;

        // Insert into cache
        {
            let mut cache = self.cache.write().await;
            cache.put(model_id.to_string(), provider.clone());

            let mut current_memory = self.current_memory_mb.write().await;
            *current_memory += metadata.memory_mb;
        }

        Ok(provider)
    }

    async fn load_model(&self, metadata: &ModelMetadata) -> Result<Arc<OnnxEmbeddingProvider>, EmbeddingError> {
        let config = OnnxConfig {
            model_path: metadata.model_path.clone(),
            tokenizer_path: metadata.tokenizer_path.clone(),
            model_name: metadata.name.clone(),
            dimension: metadata.dimension,
            max_length: metadata.max_length,
            execution_provider: ExecutionProviderConfig::TensorRT {
                device_id: 0,
                fp8_enable: metadata.family == ModelFamily::Qwen3,
                engine_cache_path: Some(PathBuf::from(format!("/var/cache/akidb/trt/{}", metadata.id))),
            },
        };

        let provider = OnnxEmbeddingProvider::with_config(config).await?;
        Ok(Arc::new(provider))
    }
}
```

### Afternoon Tasks (3 hours)

**Task 3: Eviction Logic** [2 hours]
```rust
// Add to cache.rs

impl ModelCache {
    async fn evict_if_needed(&self, required_mb: usize) -> Result<(), EmbeddingError> {
        let current_memory = *self.current_memory_mb.read().await;

        if current_memory + required_mb > self.memory_limit_mb {
            let mut cache = self.cache.write().await;
            let mut current_memory = self.current_memory_mb.write().await;

            // Evict LRU until enough space
            while *current_memory + required_mb > self.memory_limit_mb && !cache.is_empty() {
                if let Some((evicted_id, _)) = cache.pop_lru() {
                    if let Some(metadata) = self.registry.get(&evicted_id) {
                        *current_memory = current_memory.saturating_sub(metadata.memory_mb);
                        eprintln!("Evicted: {} (freed {} MB)", evicted_id, metadata.memory_mb);
                    }
                }
            }
        }

        Ok(())
    }
}
```

**Task 4: Cache Unit Tests** [1 hour]
```bash
# Test cache
cargo test -p akidb-embedding --features onnx cache_test -- --nocapture

# Expected tests:
# - test_cache_hit (should be <10ms)
# - test_lru_eviction (verify LRU behavior)
# - test_memory_limit (verify memory enforcement)
```

**Day 2 Deliverable:** LRU cache working with tests passing

---

## Day 3: Model Router & API Integration (Wednesday)

### Morning Tasks (4 hours)

**Task 1: Model Router** [2 hours]
```bash
cd ~/akidb2

# Create router module
cat > crates/akidb-embedding/src/router.rs << 'EOF'
pub struct ModelRouter {
    registry: Arc<ModelRegistry>,
    cache: Arc<ModelCache>,
}

impl ModelRouter {
    pub fn new(registry: Arc<ModelRegistry>, cache: Arc<ModelCache>) -> Self {
        Self { registry, cache }
    }

    pub async fn embed(
        &self,
        model_id: Option<&str>,
        texts: Vec<String>,
    ) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let model_id = model_id.unwrap_or(&self.registry.default_model);
        let metadata = self.registry.get(model_id)
            .ok_or_else(|| EmbeddingError::ModelNotFound(model_id.to_string()))?;

        let provider = self.cache.get_or_load(model_id).await?;

        let request = BatchEmbeddingRequest {
            model: metadata.name.clone(),
            inputs: texts,
            normalize: true,
        };

        let response = provider.embed_batch(request).await?;
        Ok(response.embeddings)
    }

    pub async fn list_models(&self) -> Vec<ModelInfo> {
        self.registry.list().iter().map(|m| ModelInfo {
            id: m.id.clone(),
            name: m.name.clone(),
            dimension: m.dimension,
            params: m.params,
            latency_p95_ms: m.latency_p95_ms,
            throughput_qps: m.throughput_qps,
            multilingual: m.multilingual,
        }).collect()
    }
}
EOF

cargo build --release -p akidb-embedding --features onnx
```

**Task 2: Update EmbeddingManager** [2 hours]
```rust
// Update crates/akidb-service/src/embedding_manager.rs

pub struct EmbeddingManager {
    router: Arc<ModelRouter>,  // Replace single provider
}

impl EmbeddingManager {
    pub fn new() -> Self {
        let registry = Arc::new(ModelRegistry::new());
        let cache = Arc::new(ModelCache::new(3, 8000, registry.clone()));
        let router = Arc::new(ModelRouter::new(registry, cache));

        Self { router }
    }

    pub async fn embed(
        &self,
        model_id: Option<&str>,
        texts: Vec<String>,
    ) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        self.router.embed(model_id, texts).await
    }
}
```

### Afternoon Tasks (3 hours)

**Task 3: REST API Update** [2 hours]
```rust
// Update crates/akidb-rest/src/routes/embed.rs

#[derive(Deserialize)]
pub struct EmbedRequest {
    pub model: Option<String>,  // NEW: Optional model parameter
    pub texts: Vec<String>,
    #[serde(default = "default_normalize")]
    pub normalize: bool,
}

pub async fn embed_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<EmbedRequest>,
) -> Result<Json<EmbedResponse>, ApiError> {
    let embeddings = state.embedding_manager
        .embed(request.model.as_deref(), request.texts)
        .await?;

    Ok(Json(EmbedResponse {
        embeddings,
        model: request.model.unwrap_or_else(|| "qwen3-4b".to_string()),
    }))
}

// New endpoint: List models
pub async fn list_models_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ListModelsResponse>, ApiError> {
    let models = state.embedding_manager.list_models().await;
    Ok(Json(ListModelsResponse { models }))
}
```

**Task 4: Test API** [1 hour]
```bash
# Start server
cargo run -p akidb-rest --features onnx --release

# Test default model
curl -X POST http://localhost:8080/api/v1/embed \
  -H "Content-Type: application/json" \
  -d '{"texts": ["Hello"]}'

# Test specific model
curl -X POST http://localhost:8080/api/v1/embed \
  -H "Content-Type: application/json" \
  -d '{"model": "qwen3-0.5b", "texts": ["Fast embedding"]}'

# List models
curl http://localhost:8080/api/v1/models
```

**Day 3 Deliverable:** Multi-model API working

---

## Day 4: Performance Benchmarking (Thursday)

### All-Day Task: Benchmark All Models (7 hours)

**Task 1: Per-Model Latency Benchmarks** [4 hours]
```bash
cd ~/akidb2

# Benchmark script
cat > scripts/benchmark_all_models.sh << 'SCRIPT'
#!/bin/bash

MODELS=(qwen3-0.5b qwen3-1.5b qwen3-4b qwen3-7b e5-small-v2 bge-small-en-v1.5)

echo "🚀 Benchmarking All Models"
echo

for model in "${MODELS[@]}"; do
  echo "📊 $model"

  # Latency (batch size 1)
  cargo bench -p akidb-embedding --features onnx \
    --bench multi_model_bench -- \
    --measurement-time 30 "latency/$model" | \
    grep "time:" | tee -a /tmp/benchmarks.txt

  # Throughput (batch size 8)
  cargo bench -p akidb-embedding --features onnx \
    --bench multi_model_bench -- \
    --measurement-time 30 "throughput/$model" | \
    grep "QPS:" | tee -a /tmp/benchmarks.txt

  echo
done

echo "✅ Benchmarks complete!"
cat /tmp/benchmarks.txt
SCRIPT

chmod +x scripts/benchmark_all_models.sh
./benchmark_all_models.sh
```

**Expected Results:**
```
qwen3-0.5b:  P95 ~8ms,  120 QPS
qwen3-1.5b:  P95 ~15ms, 65 QPS
qwen3-4b:    P95 ~25ms, 50 QPS
qwen3-7b:    P95 ~50ms, 25 QPS
e5-small-v2: P95 ~3ms,  300 QPS
bge-small:   P95 ~3ms,  300 QPS
```

**Task 2: Cache Performance** [2 hours]
```bash
# Test cache hit/miss performance
cargo test -p akidb-embedding --features onnx \
  --test cache_performance_test -- --nocapture

# Expected:
# Cache hit: <5ms
# Cache miss (cached TensorRT engine): <500ms
# First-time load (build TensorRT engine): ~5s
```

**Task 3: Document Performance** [1 hour]
```bash
# Create performance comparison table
cat > docs/MODEL-PERFORMANCE-COMPARISON.md << 'DOC'
# Model Performance Comparison

| Model | Params | Dims | Latency (P95) | Throughput | Memory | Use Case |
|-------|--------|------|---------------|------------|--------|----------|
| qwen3-0.5b | 500M | 896 | 8ms | 120 QPS | 500 MB | Low-latency |
| qwen3-1.5b | 1.5B | 1536 | 15ms | 65 QPS | 1.5 GB | Balanced |
| qwen3-4b | 4B | 4096 | 25ms | 50 QPS | 4 GB | High-quality |
| qwen3-7b | 7B | 4096 | 50ms | 25 QPS | 7 GB | Best quality |
| e5-small-v2 | 33M | 384 | 3ms | 300 QPS | 35 MB | Ultra-fast |
| bge-small-en | 33M | 384 | 3ms | 300 QPS | 35 MB | English-only |
DOC
```

**Day 4 Deliverable:** Performance profiles for all 6 models

---

## Day 5: Testing & Documentation (Friday)

### Morning Task: E2E Testing (3 hours)

**Task 1: Multi-Model Integration Tests** [3 hours]
```bash
cd ~/akidb2

# Create E2E test
cat > tests/e2e_multi_model_test.rs << 'EOF'
#[tokio::test]
async fn test_multi_model_embedding() {
    let router = setup_router();

    let test_cases = vec![
        ("qwen3-0.5b", "Fast test"),
        ("qwen3-4b", "Quality test"),
        ("e5-small-v2", "Multilingual test"),
    ];

    for (model, text) in test_cases {
        let embeddings = router.embed(Some(model), vec![text.to_string()]).await.unwrap();

        let metadata = router.registry.get(model).unwrap();
        assert_eq!(embeddings[0].len(), metadata.dimension as usize);

        println!("✅ {}: {} dims", model, embeddings[0].len());
    }
}

#[tokio::test]
async fn test_concurrent_multi_model() {
    let router = Arc::new(setup_router());

    let mut handles = vec![];
    for i in 0..10 {
        let router = router.clone();
        let model = ["qwen3-0.5b", "qwen3-4b", "e5-small-v2"][i % 3];

        handles.push(tokio::spawn(async move {
            router.embed(Some(model), vec![format!("Test {}", i)]).await.unwrap()
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    println!("✅ Concurrent multi-model test passed");
}
EOF

cargo test -p akidb-embedding --features onnx --test e2e_multi_model_test -- --nocapture
```

### Afternoon Tasks: Documentation (4 hours)

**Task 2: Completion Report** [2 hours]
```bash
# Update Week 4 completion report with actual results
cd ~/akidb2/automatosx/tmp

# Extract benchmark results
QWEN_05B_P95=$(grep "qwen3-0.5b" /tmp/benchmarks.txt | head -1 | awk '{print $2}')
QWEN_4B_P95=$(grep "qwen3-4b" /tmp/benchmarks.txt | head -1 | awk '{print $2}')
# ... (extract all metrics)

# Update report
cat > JETSON-THOR-WEEK4-COMPLETION-REPORT.md << 'REPORT'
# Week 4: Multi-Model Support - Completion Report

**Status:** ✅ COMPLETE
**Date:** $(date +%Y-%m-%d)

## Achievements
- ✅ 6 models supported (Qwen3 family + E5 + BGE)
- ✅ LRU cache (3 models, 8GB limit)
- ✅ Multi-model API with runtime selection
- ✅ Cache hit <5ms, cache miss <500ms

## Performance Results
[INSERT ACTUAL BENCHMARK RESULTS]

## Next Steps
- Week 5: API server deployment (K8s, Docker)
- Week 6: Production hardening
REPORT
```

**Task 3: API Documentation** [2 hours]
```bash
# Create API docs with examples
cat > docs/MULTI-MODEL-API.md << 'DOC'
# Multi-Model Embedding API

## Endpoints

### POST /api/v1/embed

Generate embeddings with optional model selection.

**Request:**
```json
{
  "model": "qwen3-4b",  // Optional, defaults to qwen3-4b
  "texts": ["Hello, world!"],
  "normalize": true
}
```

**Response:**
```json
{
  "embeddings": [[0.123, 0.456, ...]],
  "model": "qwen3-4b",
  "dimension": 4096
}
```

### GET /api/v1/models

List available models.

**Response:**
```json
{
  "models": [
    {
      "id": "qwen3-4b",
      "name": "Qwen/Qwen2.5-4B",
      "dimension": 4096,
      "latency_p95_ms": 25,
      "throughput_qps": 50
    },
    ...
  ]
}
```

## Model Selection Guide

- **Low Latency:** qwen3-0.5b, e5-small-v2 (<10ms)
- **Balanced:** qwen3-1.5b, qwen3-4b (15-25ms)
- **High Quality:** qwen3-7b (50ms)
- **Multilingual:** Qwen3 family, E5
- **English-Only:** BGE-small-en-v1.5
DOC
```

**Day 5 Deliverable:** Complete documentation + Week 4 report

---

## Quick Command Reference

```bash
# Day 1: Create registry and convert models
cargo build -p akidb-embedding --features onnx
./scripts/convert_all_models.sh

# Day 2: Build LRU cache
cargo test -p akidb-embedding --features onnx cache_test

# Day 3: Test multi-model API
cargo run -p akidb-rest --features onnx
curl http://localhost:8080/api/v1/models

# Day 4: Run benchmarks
./scripts/benchmark_all_models.sh

# Day 5: E2E tests
cargo test -p akidb-embedding --features onnx e2e_multi_model_test
```

---

## Success Criteria Summary

| Metric | Target | Pass/Fail |
|--------|--------|-----------|
| **Models Supported** | ≥6 | TBD |
| **Cache Hit Latency** | <5ms | TBD |
| **Cache Miss Latency** | <500ms | TBD |
| **Memory Limit** | ≤8GB (2-3 models) | TBD |
| **API Tests** | 100% passing | TBD |
| **Zero Regression** | Week 3 perf maintained | TBD |

**Week 4 Status:** 🚧 **READY TO EXECUTE**

---

## Risk Mitigation Checklist

**Before Starting:**
- [ ] Week 3 performance maintained (25ms, 50 QPS)
- [ ] Sufficient disk space for 6 models (~15GB)
- [ ] Python environment for model conversion
- [ ] Backup of Week 3 code (`git tag week3-baseline`)

**During Execution:**
- [ ] Validate each model conversion (quality >0.99)
- [ ] Test cache eviction logic (no OOM)
- [ ] Monitor GPU memory during multi-model tests
- [ ] Benchmark each model after loading

**Rollback Plan:**
If multi-model support fails:
1. Revert to Week 3 single-model baseline
2. Isolate which component caused issues (registry/cache/router)
3. Debug individually
4. Document findings

---

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>
