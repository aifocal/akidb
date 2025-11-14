# Jetson Thor Week 4: Multi-Model Support & Model Registry PRD

**Status:** Ready to Execute
**Timeline:** 5 days (Week 4)
**Owner:** Backend Team + ML Engineering
**Dependencies:** Week 1 (✅), Week 2 (✅), Week 3 (✅)
**Target Platform:** NVIDIA Jetson Thor (Blackwell GPU, 2,000 TOPS)

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Goals & Non-Goals](#goals--non-goals)
3. [Baseline Analysis](#baseline-analysis)
4. [Multi-Model Architecture](#multi-model-architecture)
5. [Model Registry Design](#model-registry-design)
6. [Day-by-Day Implementation Plan](#day-by-day-implementation-plan)
7. [Model Catalog](#model-catalog)
8. [Performance Validation](#performance-validation)
9. [Risk Management](#risk-management)
10. [Success Criteria](#success-criteria)
11. [Appendix: Code Examples](#appendix-code-examples)

---

## Executive Summary

Week 4 focuses on **multi-model support**, enabling runtime selection between different embedding models to serve diverse use cases on Jetson Thor. Building on Week 3's optimized single-model performance (<30ms, >50 QPS), we will implement a model registry, LRU caching, hot-swapping, and support for 5+ embedding models.

### Key Objectives

1. **Model Registry:** Centralized catalog of available embedding models with metadata
2. **Multi-Model Support:** Support 5+ models (Qwen3 0.5B/1.5B/4B/7B, E5-small, BGE-small)
3. **Runtime Selection:** API for selecting models dynamically per request
4. **LRU Caching:** Intelligent model caching (max 2-3 models in GPU memory)
5. **Hot-Swapping:** Load/unload models without server restart
6. **Performance:** Maintain <30ms P95 for loaded models, <500ms cold start

### Expected Outcomes

- ✅ 5+ embedding models supported (Qwen3 family + E5 + BGE)
- ✅ Model registry with metadata (params, dims, memory, latency)
- ✅ LRU cache with configurable capacity (2-3 models, ~8GB GPU memory)
- ✅ Hot model loading <500ms (cold start from disk)
- ✅ Warm model switching <5ms (already in cache)
- ✅ API: `POST /api/v1/embed` with `model` parameter
- ✅ Zero performance regression for single-model workloads

---

## Goals & Non-Goals

### Goals (Week 4)

**Primary Goals:**
1. ✅ **Model Registry** - Catalog of available models with specs
2. ✅ **Multi-Model API** - Runtime model selection via API parameter
3. ✅ **LRU Cache** - Intelligent model caching (2-3 models, 8GB limit)
4. ✅ **5+ Models Supported:**
   - Qwen3-0.5B (896-dim, 0.5B params, ~500MB)
   - Qwen3-1.5B (1536-dim, 1.5B params, ~1.5GB)
   - Qwen3-4B (4096-dim, 4B params, ~4GB) - **baseline from Week 3**
   - Qwen3-7B (4096-dim, 7B params, ~7GB)
   - E5-small-v2 (384-dim, 33M params, ~35MB)
   - BGE-small-en-v1.5 (384-dim, 33M params, ~35MB)
5. ✅ **Hot-Swapping** - Load/unload models without restart
6. ✅ **Performance Benchmarks** - Per-model latency/throughput profiles

**Secondary Goals:**
- 📊 Model comparison dashboard (latency vs quality vs memory)
- 📊 Auto-model-selection based on workload (e.g., low-latency → 0.5B)
- 📊 Multi-tenant model isolation (different models per tenant)
- 📝 Model conversion pipeline documentation

### Non-Goals (Deferred to Week 5+)

**Not in Scope for Week 4:**
- ❌ Custom model training/fine-tuning - Future
- ❌ Model quantization beyond FP8 (INT8, INT4) - Week 7+
- ❌ Distributed multi-GPU inference - Week 8+
- ❌ Kubernetes deployment - Week 5
- ❌ Production API server (REST/gRPC) - Week 5
- ❌ Model versioning and A/B testing - Week 6+

---

## Baseline Analysis

### Week 3 Single-Model Performance

**Qwen3-4B FP8 (Optimized):**
- Latency: <30ms P95 (single embedding)
- Throughput: >50 QPS (single-threaded), >150 QPS (concurrent)
- GPU Memory: ~4GB (model + TensorRT workspace)
- Quality: >0.99 cosine similarity vs HuggingFace

**Constraints:**
- Single model loaded at a time
- Changing models requires server restart
- No runtime model selection API
- No model metadata (have to know model specs externally)

### Multi-Model Requirements

**Use Cases:**

1. **Latency-Sensitive Applications** (autonomous driving, real-time commands)
   - Need: Fast inference (<10ms)
   - Model: Qwen3-0.5B (896-dim, 500MB)
   - Trade-off: Lower quality for speed

2. **Quality-Sensitive Applications** (document retrieval, semantic search)
   - Need: High-quality embeddings
   - Model: Qwen3-7B (4096-dim, 7GB)
   - Trade-off: Higher latency (50-80ms)

3. **Balanced Applications** (chatbots, general RAG)
   - Need: Good quality + reasonable speed
   - Model: Qwen3-4B (4096-dim, 4GB) - **current baseline**
   - Trade-off: Balanced

4. **Multilingual Applications** (global automotive markets)
   - Need: Multilingual support
   - Model: Qwen3 family (all models)
   - Alternative: E5-small-v2 (multilingual, 384-dim)

5. **English-Only Applications** (US/UK markets)
   - Need: High performance for English
   - Model: BGE-small-en-v1.5 (384-dim, optimized for English)
   - Trade-off: English-only

**Memory Budget (Jetson Thor):**
- Total GPU memory: 64GB (unified)
- Reserved for OS/drivers: ~2GB
- Available for models: **~62GB**
- Target: Support 2-3 models simultaneously (LRU cache)

**Example Multi-Model Scenarios:**

```
Scenario 1: Latency + Quality
- Qwen3-0.5B (500MB) + Qwen3-4B (4GB) = 4.5GB total
- Fast queries use 0.5B, important queries use 4B

Scenario 2: Quality Range
- Qwen3-1.5B (1.5GB) + Qwen3-4B (4GB) + Qwen3-7B (7GB) = 12.5GB total
- Low/medium/high quality tiers

Scenario 3: Multilingual + English
- Qwen3-4B (4GB) + BGE-small-en-v1.5 (35MB) = 4.035GB total
- Multilingual for international, BGE for English-specific
```

---

## Multi-Model Architecture

### High-Level Design

```
┌─────────────────────────────────────────────────────────────────┐
│                    AkiDB Multi-Model Layer                       │
│                    (Week 4 Implementation)                       │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│  REST API: POST /api/v1/embed                                   │
│  Body: {                                                        │
│    "model": "qwen3-4b",  // <-- NEW: Runtime model selection   │
│    "texts": ["..."],                                            │
│    "normalize": true                                            │
│  }                                                              │
└────────────────────┬────────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────────┐
│  ModelRouter (NEW)                                              │
│  - Parse model parameter                                        │
│  - Lookup model in registry                                     │
│  - Route to appropriate provider                                │
└────────────────────┬────────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────────┐
│  ModelRegistry (NEW)                                            │
│  - Catalog of available models                                  │
│  - Model metadata (params, dims, memory, latency)               │
│  - Model loading/unloading                                      │
└────────────────────┬────────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────────┐
│  ModelCache (NEW - LRU)                                         │
│  - In-memory cache of loaded models (2-3 models)                │
│  - LRU eviction when capacity exceeded                          │
│  - Reference counting for concurrent requests                   │
└────────────────────┬────────────────────────────────────────────┘
                     │
         ┌───────────┴───────────┬───────────┬──────────┐
         ▼                       ▼           ▼          ▼
┌─────────────────┐  ┌─────────────────┐  ┌──────┐  ┌──────┐
│ Qwen3-0.5B      │  │ Qwen3-1.5B      │  │ 4B   │  │ 7B   │
│ OnnxProvider    │  │ OnnxProvider    │  │      │  │      │
│ (896-dim)       │  │ (1536-dim)      │  │      │  │      │
└─────────────────┘  └─────────────────┘  └──────┘  └──────┘

┌─────────────────┐  ┌─────────────────┐
│ E5-small-v2     │  │ BGE-small-en    │
│ OnnxProvider    │  │ OnnxProvider    │
│ (384-dim)       │  │ (384-dim)       │
└─────────────────┘  └─────────────────┘
```

### Component Design

#### 1. ModelMetadata

```rust
pub struct ModelMetadata {
    pub id: String,               // "qwen3-4b", "e5-small-v2", etc.
    pub name: String,             // "Qwen/Qwen2.5-4B"
    pub family: ModelFamily,      // Qwen3, E5, BGE, etc.
    pub params: u64,              // 4_000_000_000 (4B)
    pub dimension: u32,           // 4096
    pub max_length: usize,        // 512
    pub memory_mb: usize,         // ~4000 (4GB)
    pub latency_p95_ms: u32,      // 25 (from benchmarks)
    pub throughput_qps: u32,      // 50 (single-threaded)
    pub multilingual: bool,       // true for Qwen3, false for BGE-en
    pub languages: Vec<String>,   // ["en", "zh", "ja", "de", ...]
    pub license: String,          // "Apache 2.0"
    pub model_path: PathBuf,      // "/opt/akidb/models/qwen3-4b-onnx-fp8/model.onnx"
    pub tokenizer_path: PathBuf,
}

pub enum ModelFamily {
    Qwen3,
    E5,
    BGE,
    MiniLM,
    Custom,
}
```

#### 2. ModelRegistry

```rust
use std::collections::HashMap;

pub struct ModelRegistry {
    models: HashMap<String, ModelMetadata>,
    default_model: String,
}

impl ModelRegistry {
    pub fn new() -> Self {
        let mut models = HashMap::new();

        // Register Qwen3 family
        models.insert("qwen3-0.5b".to_string(), ModelMetadata {
            id: "qwen3-0.5b".to_string(),
            name: "Qwen/Qwen2.5-0.5B".to_string(),
            family: ModelFamily::Qwen3,
            params: 500_000_000,
            dimension: 896,
            memory_mb: 500,
            latency_p95_ms: 8,
            throughput_qps: 120,
            multilingual: true,
            languages: vec!["en", "zh", "ja", "ko", "de", "fr", "es"].into_iter().map(String::from).collect(),
            license: "Apache 2.0".to_string(),
            model_path: PathBuf::from("/opt/akidb/models/qwen3-0.5b-onnx-fp8/model.onnx"),
            tokenizer_path: PathBuf::from("/opt/akidb/models/qwen3-0.5b-onnx-fp8/tokenizer.json"),
            max_length: 512,
        });

        models.insert("qwen3-1.5b".to_string(), ModelMetadata {
            id: "qwen3-1.5b".to_string(),
            name: "Qwen/Qwen2.5-1.5B".to_string(),
            family: ModelFamily::Qwen3,
            params: 1_500_000_000,
            dimension: 1536,
            memory_mb: 1500,
            latency_p95_ms: 15,
            throughput_qps: 65,
            multilingual: true,
            languages: vec!["en", "zh", "ja", "ko", "de", "fr", "es"].into_iter().map(String::from).collect(),
            license: "Apache 2.0".to_string(),
            model_path: PathBuf::from("/opt/akidb/models/qwen3-1.5b-onnx-fp8/model.onnx"),
            tokenizer_path: PathBuf::from("/opt/akidb/models/qwen3-1.5b-onnx-fp8/tokenizer.json"),
            max_length: 512,
        });

        models.insert("qwen3-4b".to_string(), ModelMetadata {
            id: "qwen3-4b".to_string(),
            name: "Qwen/Qwen2.5-4B".to_string(),
            family: ModelFamily::Qwen3,
            params: 4_000_000_000,
            dimension: 4096,
            memory_mb: 4000,
            latency_p95_ms: 25,  // Week 3 optimized
            throughput_qps: 50,
            multilingual: true,
            languages: vec!["en", "zh", "ja", "ko", "de", "fr", "es"].into_iter().map(String::from).collect(),
            license: "Apache 2.0".to_string(),
            model_path: PathBuf::from("/opt/akidb/models/qwen3-4b-onnx-fp8/model.onnx"),
            tokenizer_path: PathBuf::from("/opt/akidb/models/qwen3-4b-onnx-fp8/tokenizer.json"),
            max_length: 512,
        });

        models.insert("qwen3-7b".to_string(), ModelMetadata {
            id: "qwen3-7b".to_string(),
            name: "Qwen/Qwen2.5-7B".to_string(),
            family: ModelFamily::Qwen3,
            params: 7_000_000_000,
            dimension: 4096,
            memory_mb: 7000,
            latency_p95_ms: 50,  // Estimated (2x 4B)
            throughput_qps: 25,
            multilingual: true,
            languages: vec!["en", "zh", "ja", "ko", "de", "fr", "es"].into_iter().map(String::from).collect(),
            license: "Apache 2.0".to_string(),
            model_path: PathBuf::from("/opt/akidb/models/qwen3-7b-onnx-fp8/model.onnx"),
            tokenizer_path: PathBuf::from("/opt/akidb/models/qwen3-7b-onnx-fp8/tokenizer.json"),
            max_length: 512,
        });

        // Register E5-small-v2
        models.insert("e5-small-v2".to_string(), ModelMetadata {
            id: "e5-small-v2".to_string(),
            name: "intfloat/e5-small-v2".to_string(),
            family: ModelFamily::E5,
            params: 33_000_000,
            dimension: 384,
            memory_mb: 35,
            latency_p95_ms: 3,  // Very fast (small model)
            throughput_qps: 300,
            multilingual: true,
            languages: vec!["en", "zh", "ja", "ko", "de", "fr", "es"].into_iter().map(String::from).collect(),
            license: "MIT".to_string(),
            model_path: PathBuf::from("/opt/akidb/models/e5-small-v2-onnx/model.onnx"),
            tokenizer_path: PathBuf::from("/opt/akidb/models/e5-small-v2-onnx/tokenizer.json"),
            max_length: 512,
        });

        // Register BGE-small-en
        models.insert("bge-small-en-v1.5".to_string(), ModelMetadata {
            id: "bge-small-en-v1.5".to_string(),
            name: "BAAI/bge-small-en-v1.5".to_string(),
            family: ModelFamily::BGE,
            params: 33_000_000,
            dimension: 384,
            memory_mb: 35,
            latency_p95_ms: 3,
            throughput_qps: 300,
            multilingual: false,
            languages: vec!["en".to_string()],
            license: "MIT".to_string(),
            model_path: PathBuf::from("/opt/akidb/models/bge-small-en-v1.5-onnx/model.onnx"),
            tokenizer_path: PathBuf::from("/opt/akidb/models/bge-small-en-v1.5-onnx/tokenizer.json"),
            max_length: 512,
        });

        Self {
            models,
            default_model: "qwen3-4b".to_string(),  // Default to Week 3 baseline
        }
    }

    pub fn get(&self, model_id: &str) -> Option<&ModelMetadata> {
        self.models.get(model_id)
    }

    pub fn list(&self) -> Vec<&ModelMetadata> {
        self.models.values().collect()
    }

    pub fn default_model(&self) -> &ModelMetadata {
        self.models.get(&self.default_model).unwrap()
    }
}
```

#### 3. ModelCache (LRU)

```rust
use lru::LruCache;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct ModelCache {
    cache: Arc<RwLock<LruCache<String, Arc<OnnxEmbeddingProvider>>>>,
    capacity: usize,
    memory_limit_mb: usize,
    current_memory_mb: Arc<RwLock<usize>>,
}

impl ModelCache {
    pub fn new(capacity: usize, memory_limit_mb: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(LruCache::new(capacity))),
            capacity,
            memory_limit_mb,
            current_memory_mb: Arc::new(RwLock::new(0)),
        }
    }

    pub async fn get_or_load(
        &self,
        model_id: &str,
        metadata: &ModelMetadata,
    ) -> Result<Arc<OnnxEmbeddingProvider>, EmbeddingError> {
        // Check cache first
        {
            let mut cache = self.cache.write().await;
            if let Some(provider) = cache.get(model_id) {
                eprintln!("✅ Cache hit: {}", model_id);
                return Ok(provider.clone());
            }
        }

        eprintln!("⏳ Cache miss: {} (loading...)", model_id);

        // Evict models if memory limit exceeded
        self.evict_if_needed(metadata.memory_mb).await?;

        // Load model (cold start)
        let provider = self.load_model(metadata).await?;

        // Insert into cache
        {
            let mut cache = self.cache.write().await;
            cache.put(model_id.to_string(), provider.clone());

            let mut current_memory = self.current_memory_mb.write().await;
            *current_memory += metadata.memory_mb;

            eprintln!("✅ Model loaded: {} ({} MB)", model_id, metadata.memory_mb);
            eprintln!("   Cache: {}/{} models, {} MB / {} MB",
                cache.len(), self.capacity,
                *current_memory, self.memory_limit_mb);
        }

        Ok(provider)
    }

    async fn evict_if_needed(&self, required_mb: usize) -> Result<(), EmbeddingError> {
        let current_memory = *self.current_memory_mb.read().await;

        if current_memory + required_mb > self.memory_limit_mb {
            eprintln!("⚠️  Memory limit exceeded, evicting LRU models...");

            let mut cache = self.cache.write().await;
            let mut current_memory = self.current_memory_mb.write().await;

            // Evict LRU until we have enough space
            while *current_memory + required_mb > self.memory_limit_mb && !cache.is_empty() {
                if let Some((evicted_id, _)) = cache.pop_lru() {
                    // Find evicted model's memory size (approximate)
                    // TODO: Store memory size in cache metadata
                    let evicted_mb = 1000;  // Placeholder
                    *current_memory = current_memory.saturating_sub(evicted_mb);

                    eprintln!("   Evicted: {} (freed ~{} MB)", evicted_id, evicted_mb);
                }
            }
        }

        Ok(())
    }

    async fn load_model(&self, metadata: &ModelMetadata) -> Result<Arc<OnnxEmbeddingProvider>, EmbeddingError> {
        let start = std::time::Instant::now();

        let config = OnnxConfig {
            model_path: metadata.model_path.clone(),
            tokenizer_path: metadata.tokenizer_path.clone(),
            model_name: metadata.name.clone(),
            dimension: metadata.dimension,
            max_length: metadata.max_length,
            execution_provider: ExecutionProviderConfig::TensorRT {
                device_id: 0,
                fp8_enable: true,
                engine_cache_path: Some(PathBuf::from(format!("/var/cache/akidb/trt/{}", metadata.id))),
            },
        };

        let provider = OnnxEmbeddingProvider::with_config(config).await?;

        let duration = start.elapsed();
        eprintln!("   Load time: {:.2}s", duration.as_secs_f64());

        Ok(Arc::new(provider))
    }

    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();

        let mut current_memory = self.current_memory_mb.write().await;
        *current_memory = 0;

        eprintln!("✅ Model cache cleared");
    }
}
```

#### 4. ModelRouter

```rust
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
        // Use default model if not specified
        let model_id = model_id.unwrap_or(&self.registry.default_model);

        // Lookup model metadata
        let metadata = self.registry.get(model_id)
            .ok_or_else(|| EmbeddingError::ModelNotFound(model_id.to_string()))?;

        // Get or load model from cache
        let provider = self.cache.get_or_load(model_id, metadata).await?;

        // Generate embeddings
        let request = BatchEmbeddingRequest {
            model: metadata.name.clone(),
            inputs: texts,
            normalize: true,
        };

        let response = provider.embed_batch(request).await?;

        Ok(response.embeddings)
    }

    pub async fn list_models(&self) -> Vec<&ModelMetadata> {
        self.registry.list()
    }
}
```

---

## Day-by-Day Implementation Plan

### Day 1: Model Registry & Metadata (Monday)

**Objective:** Create model registry with metadata for 6 models.

#### Morning (3 hours)

**Task 1.1: Model Registry Implementation** [2 hours]
```rust
// Create crates/akidb-embedding/src/registry.rs

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
        // Register 6 models (see architecture section above)
        // ...
    }

    pub fn get(&self, model_id: &str) -> Option<&ModelMetadata> {
        self.models.get(model_id)
    }

    pub fn list(&self) -> Vec<&ModelMetadata> {
        self.models.values().collect()
    }
}
```

**Test registry:**
```bash
cd ~/akidb2

# Build registry module
cargo build --release -p akidb-embedding --features onnx

# Unit tests
cargo test -p akidb-embedding --features onnx --test registry_test -- --nocapture
```

**Success Metric:** 6 models registered, tests pass

**Task 1.2: Model Metadata JSON Export** [1 hour]
```rust
// Add to registry.rs

impl ModelRegistry {
    pub fn export_json(&self) -> String {
        serde_json::to_string_pretty(&self.models).unwrap()
    }
}

// CLI tool to export metadata
// crates/akidb-cli/src/commands/list_models.rs

pub async fn list_models() {
    let registry = ModelRegistry::new();
    println!("{}", registry.export_json());
}
```

**Test export:**
```bash
cargo run -p akidb-cli -- list-models > /tmp/models.json
cat /tmp/models.json
```

**Success Metric:** JSON export with all 6 models

#### Afternoon (4 hours)

**Task 1.3: Convert Additional Models to ONNX** [4 hours]

**Download and convert 5 additional models:**

```bash
cd /opt/akidb

# Create bulk conversion script
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

  echo "🔧 Converting $model_name → $model_id (${dimension}-dim)"
  echo "=================================================="

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
    provider_options={
        "trt_fp8_enable": True if "Qwen" in model_name else False,
        "trt_engine_cache_enable": True,
    }
)

tokenizer = AutoTokenizer.from_pretrained(model_name)

print(f"Saving to {output_dir}...")
model.save_pretrained(output_dir)
tokenizer.save_pretrained(output_dir)

print(f"✅ Converted: {model_name}")
EOF

done

echo "✅ All models converted!"
SCRIPT

chmod +x scripts/convert_all_models.sh

# Run conversion (will take ~1 hour total: 5 models × ~12 min each)
./scripts/convert_all_models.sh 2>&1 | tee /tmp/model_conversion.log
```

**Success Metric:** 5 additional models converted to ONNX

**Estimated Time (Day 1):** 7 hours (2h registry + 1h export + 4h conversion)

---

### Day 2: LRU Cache Implementation (Tuesday)

**Objective:** Implement LRU model cache with memory management.

#### Morning (4 hours)

**Task 2.1: LRU Cache Setup** [2 hours]
```rust
// Add dependency to Cargo.toml
[dependencies]
lru = "0.12"

// Create crates/akidb-embedding/src/cache.rs

use lru::LruCache;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct ModelCache {
    cache: Arc<RwLock<LruCache<String, Arc<OnnxEmbeddingProvider>>>>,
    capacity: usize,                // Max number of models (2-3)
    memory_limit_mb: usize,         // Max total memory (8000 MB)
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
```

**Task 2.2: Cache Get/Put Logic** [2 hours]
```rust
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
        let start = std::time::Instant::now();

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

        let duration = start.elapsed();
        eprintln!("⏱️  Model loaded in {:.2}s: {}", duration.as_secs_f64(), metadata.id);

        Ok(Arc::new(provider))
    }
}
```

#### Afternoon (3 hours)

**Task 2.3: Eviction Logic** [2 hours]
```rust
impl ModelCache {
    async fn evict_if_needed(&self, required_mb: usize) -> Result<(), EmbeddingError> {
        let current_memory = *self.current_memory_mb.read().await;

        if current_memory + required_mb > self.memory_limit_mb {
            eprintln!("⚠️  Memory limit exceeded: {} + {} > {}",
                current_memory, required_mb, self.memory_limit_mb);

            let mut cache = self.cache.write().await;
            let mut current_memory = self.current_memory_mb.write().await;

            // Evict LRU models until enough space
            while *current_memory + required_mb > self.memory_limit_mb && !cache.is_empty() {
                if let Some((evicted_id, _provider)) = cache.pop_lru() {
                    // Get evicted model's memory size
                    if let Some(metadata) = self.registry.get(&evicted_id) {
                        *current_memory = current_memory.saturating_sub(metadata.memory_mb);
                        eprintln!("   Evicted: {} (freed {} MB)", evicted_id, metadata.memory_mb);
                    }
                }
            }

            eprintln!("✅ Memory after eviction: {} MB", *current_memory);
        }

        Ok(())
    }
}
```

**Task 2.4: Unit Tests** [1 hour]
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_hit() {
        let registry = Arc::new(ModelRegistry::new());
        let cache = ModelCache::new(3, 8000, registry.clone());

        // First access: cache miss
        let provider1 = cache.get_or_load("qwen3-4b").await.unwrap();

        // Second access: cache hit (should be fast)
        let start = std::time::Instant::now();
        let provider2 = cache.get_or_load("qwen3-4b").await.unwrap();
        let duration = start.elapsed();

        assert!(duration.as_millis() < 10, "Cache hit should be <10ms");
        assert!(Arc::ptr_eq(&provider1, &provider2), "Should return same instance");
    }

    #[tokio::test]
    async fn test_lru_eviction() {
        let registry = Arc::new(ModelRegistry::new());
        let cache = ModelCache::new(2, 6000, registry.clone());  // Max 2 models, 6GB

        // Load model 1: qwen3-1.5b (1.5GB)
        cache.get_or_load("qwen3-1.5b").await.unwrap();

        // Load model 2: qwen3-4b (4GB)
        cache.get_or_load("qwen3-4b").await.unwrap();

        // Load model 3: qwen3-7b (7GB) - should evict qwen3-1.5b (LRU)
        cache.get_or_load("qwen3-7b").await.unwrap();

        // Check cache state
        let cache_guard = cache.cache.read().await;
        assert!(!cache_guard.contains("qwen3-1.5b"), "LRU model should be evicted");
        assert!(cache_guard.contains("qwen3-4b"), "Recently used model should remain");
        assert!(cache_guard.contains("qwen3-7b"), "New model should be loaded");
    }

    #[tokio::test]
    async fn test_memory_limit() {
        let registry = Arc::new(ModelRegistry::new());
        let cache = ModelCache::new(5, 8000, registry.clone());  // Max 8GB

        // Load qwen3-4b (4GB)
        cache.get_or_load("qwen3-4b").await.unwrap();

        // Load qwen3-7b (7GB) - should evict qwen3-4b to stay under 8GB
        cache.get_or_load("qwen3-7b").await.unwrap();

        let current_memory = *cache.current_memory_mb.read().await;
        assert!(current_memory <= 8000, "Memory should not exceed limit");
    }
}
```

**Estimated Time (Day 2):** 7 hours (2h setup + 2h get/put + 2h eviction + 1h tests)

---

### Day 3: Model Router & API Integration (Wednesday)

**Objective:** Implement model router and integrate with API.

#### Morning (4 hours)

**Task 3.1: Model Router** [2 hours]
```rust
// Create crates/akidb-embedding/src/router.rs

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

pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub dimension: u32,
    pub params: u64,
    pub latency_p95_ms: u32,
    pub throughput_qps: u32,
    pub multilingual: bool,
}
```

**Task 3.2: Update EmbeddingManager** [2 hours]
```rust
// Update crates/akidb-service/src/embedding_manager.rs

pub struct EmbeddingManager {
    router: Arc<ModelRouter>,  // NEW: Replace single provider
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
        model_id: Option<&str>,  // NEW: Optional model selection
        texts: Vec<String>,
    ) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        self.router.embed(model_id, texts).await
    }

    pub async fn list_models(&self) -> Vec<ModelInfo> {
        self.router.list_models().await
    }
}
```

#### Afternoon (3 hours)

**Task 3.3: REST API Update** [2 hours]
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

// New endpoint: List available models
pub async fn list_models_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ListModelsResponse>, ApiError> {
    let models = state.embedding_manager.list_models().await;

    Ok(Json(ListModelsResponse { models }))
}

#[derive(Serialize)]
pub struct ListModelsResponse {
    pub models: Vec<ModelInfo>,
}
```

**Task 3.4: Integration Test** [1 hour]
```bash
# Test multi-model API
cd ~/akidb2

cargo test -p akidb-rest --features onnx --test multi_model_test -- --nocapture
```

**Estimated Time (Day 3):** 7 hours (2h router + 2h manager + 2h API + 1h test)

---

### Day 4: Performance Benchmarking (Thursday)

**Objective:** Benchmark all 6 models and document performance profiles.

#### Morning (4 hours)

**Task 4.1: Per-Model Benchmarks** [4 hours]
```bash
cat > ~/akidb2/scripts/benchmark_all_models.sh << 'SCRIPT'
#!/bin/bash
set -e

MODELS=(
  "qwen3-0.5b"
  "qwen3-1.5b"
  "qwen3-4b"
  "qwen3-7b"
  "e5-small-v2"
  "bge-small-en-v1.5"
)

echo "🚀 Benchmarking All Models"
echo "=========================="
echo

for model in "${MODELS[@]}"; do
  echo "📊 Benchmarking: $model"
  echo "----------------------------"

  # Latency benchmark (batch size 1)
  cargo bench -p akidb-embedding --features onnx \
    --bench multi_model_bench -- \
    --measurement-time 30 \
    "latency/$model" 2>&1 | \
    grep -E "(time:|P95:)" | tee -a /tmp/model_benchmarks.txt

  # Throughput benchmark (batch size 8)
  cargo bench -p akidb-embedding --features onnx \
    --bench multi_model_bench -- \
    --measurement-time 30 \
    "throughput/$model" 2>&1 | \
    grep -E "(QPS:|throughput:)" | tee -a /tmp/model_benchmarks.txt

  echo
done

echo "✅ Benchmark complete!"
echo "Results: /tmp/model_benchmarks.txt"
SCRIPT

chmod +x ~/akidb2/scripts/benchmark_all_models.sh
./benchmark_all_models.sh
```

**Expected Results:**

| Model | Params | Dim | Latency (P95) | Throughput (QPS) | Memory (MB) |
|-------|--------|-----|---------------|------------------|-------------|
| qwen3-0.5b | 500M | 896 | ~8ms | ~120 | 500 |
| qwen3-1.5b | 1.5B | 1536 | ~15ms | ~65 | 1500 |
| qwen3-4b | 4B | 4096 | ~25ms | ~50 | 4000 |
| qwen3-7b | 7B | 4096 | ~50ms | ~25 | 7000 |
| e5-small-v2 | 33M | 384 | ~3ms | ~300 | 35 |
| bge-small-en-v1.5 | 33M | 384 | ~3ms | ~300 | 35 |

#### Afternoon (3 hours)

**Task 4.2: Cache Performance Testing** [2 hours]
```bash
# Test cache hit/miss performance
cat > ~/akidb2/tests/cache_performance_test.rs << 'SCRIPT'
#[tokio::test]
async fn test_cache_hit_performance() {
    let router = setup_router();

    // First request: cache miss (cold start)
    let start = std::time::Instant::now();
    router.embed(Some("qwen3-4b"), vec!["Test".to_string()]).await.unwrap();
    let cold_start = start.elapsed();

    // Second request: cache hit (should be fast)
    let start = std::time::Instant::now();
    router.embed(Some("qwen3-4b"), vec!["Test".to_string()]).await.unwrap();
    let warm_start = start.elapsed();

    println!("Cold start: {:.2}s", cold_start.as_secs_f64());
    println!("Warm start: {}ms", warm_start.as_millis());

    assert!(warm_start.as_millis() < 30, "Warm start should be <30ms");
    assert!(cold_start.as_secs() < 1, "Cold start should be <1s (cached TensorRT engine)");
}

#[tokio::test]
async fn test_model_switching() {
    let router = setup_router();

    // Load 3 models in sequence
    let models = ["qwen3-0.5b", "qwen3-1.5b", "qwen3-4b"];

    for model in &models {
        let start = std::time::Instant::now();
        router.embed(Some(model), vec!["Test".to_string()]).await.unwrap();
        let duration = start.elapsed();

        println!("{}: {:.2}s", model, duration.as_secs_f64());
    }

    // Switch back to first model (should be in cache)
    let start = std::time::Instant::now();
    router.embed(Some("qwen3-0.5b"), vec!["Test".to_string()]).await.unwrap();
    let duration = start.elapsed();

    println!("Switch to qwen3-0.5b: {}ms", duration.as_millis());

    assert!(duration.as_millis() < 30, "Switching to cached model should be <30ms");
}
SCRIPT

cargo test -p akidb-embedding --features onnx --test cache_performance_test -- --nocapture
```

**Task 4.3: Document Performance Profiles** [1 hour]
```bash
# Create performance comparison matrix
cat > ~/akidb2/docs/MODEL-PERFORMANCE-COMPARISON.md << 'DOC'
# Model Performance Comparison (Jetson Thor)

## Overview

Performance benchmarks for all supported embedding models on NVIDIA Jetson Thor with TensorRT FP8 optimization.

## Benchmark Results

| Model | Parameters | Dimensions | Latency (P95) | Throughput | Memory | Use Case |
|-------|------------|------------|---------------|------------|--------|----------|
| **Qwen3-0.5B** | 500M | 896 | 8ms | 120 QPS | 500 MB | Low-latency, real-time |
| **Qwen3-1.5B** | 1.5B | 1536 | 15ms | 65 QPS | 1.5 GB | Balanced |
| **Qwen3-4B** | 4B | 4096 | 25ms | 50 QPS | 4 GB | High-quality, multilingual |
| **Qwen3-7B** | 7B | 4096 | 50ms | 25 QPS | 7 GB | Highest quality |
| **E5-small-v2** | 33M | 384 | 3ms | 300 QPS | 35 MB | Ultra-fast, multilingual |
| **BGE-small-en** | 33M | 384 | 3ms | 300 QPS | 35 MB | Ultra-fast, English-only |

## Model Selection Guide

### Use Case: Real-Time Autonomous Driving
**Recommended:** Qwen3-0.5B or E5-small-v2
- Latency: <10ms
- Trade-off: Lower embedding quality for speed
- Example: Voice commands, real-time sensor interpretation

### Use Case: Document Retrieval (RAG)
**Recommended:** Qwen3-4B or Qwen3-7B
- Latency: 25-50ms (acceptable for retrieval)
- Trade-off: Higher quality for moderate latency
- Example: Manual lookup, knowledge base search

### Use Case: Multilingual Support
**Recommended:** Qwen3 family (any size)
- Languages: English, Chinese, Japanese, Korean, German, French, Spanish
- Trade-off: None (Qwen3 is multilingual by default)

### Use Case: English-Only (US/UK Markets)
**Recommended:** BGE-small-en-v1.5
- Latency: <5ms
- Trade-off: English-only, but optimized
- Example: US-only applications

## Cache Performance

**Cache Hit (Warm Start):**
- Latency: <5ms (model already in memory)
- No TensorRT engine build needed

**Cache Miss (Cold Start):**
- Latency: <500ms (load from disk + TensorRT engine cache)
- First-ever load: ~5s (TensorRT engine compilation)

**Model Switching:**
- Between cached models: <5ms
- Loading new model: <500ms
DOC
```

**Estimated Time (Day 4):** 7 hours (4h benchmarks + 2h cache tests + 1h docs)

---

### Day 5: Testing & Documentation (Friday)

**Objective:** Comprehensive testing and documentation.

#### Morning (3 hours)

**Task 5.1: End-to-End Multi-Model Testing** [3 hours]
```bash
cat > ~/akidb2/tests/e2e_multi_model_test.rs << 'SCRIPT'
#[tokio::test]
async fn test_multi_model_embedding() {
    let router = setup_router();

    let test_cases = vec![
        ("qwen3-0.5b", "Fast latency test"),
        ("qwen3-1.5b", "Balanced test"),
        ("qwen3-4b", "High quality test"),
        ("e5-small-v2", "Multilingual test"),
        ("bge-small-en-v1.5", "English-only test"),
    ];

    for (model, text) in test_cases {
        println!("Testing model: {}", model);

        let embeddings = router.embed(
            Some(model),
            vec![text.to_string()]
        ).await.unwrap();

        assert_eq!(embeddings.len(), 1);

        // Check embedding dimension matches model
        let metadata = router.registry.get(model).unwrap();
        assert_eq!(embeddings[0].len(), metadata.dimension as usize);

        println!("✅ {}: {} dims", model, embeddings[0].len());
    }
}

#[tokio::test]
async fn test_concurrent_multi_model() {
    let router = Arc::new(setup_router());

    let mut handles = vec![];

    // Concurrent requests with different models
    for i in 0..10 {
        let router = router.clone();
        let model = match i % 3 {
            0 => "qwen3-0.5b",
            1 => "qwen3-4b",
            _ => "e5-small-v2",
        };

        let handle = tokio::spawn(async move {
            router.embed(
                Some(model),
                vec![format!("Test {}", i)]
            ).await.unwrap()
        });

        handles.push(handle);
    }

    // Wait for all requests
    for handle in handles {
        handle.await.unwrap();
    }

    println!("✅ Concurrent multi-model test passed");
}

#[tokio::test]
async fn test_default_model() {
    let router = setup_router();

    // Request without specifying model (should use default: qwen3-4b)
    let embeddings = router.embed(
        None,
        vec!["Test default model".to_string()]
    ).await.unwrap();

    assert_eq!(embeddings[0].len(), 4096);  // Qwen3-4B dimension

    println!("✅ Default model test passed");
}
SCRIPT

cargo test -p akidb-embedding --features onnx --test e2e_multi_model_test -- --nocapture
```

#### Afternoon (4 hours)

**Task 5.2: Week 4 Completion Report** [2 hours]
```bash
cat > ~/akidb2/automatosx/tmp/JETSON-THOR-WEEK4-COMPLETION-REPORT.md << 'REPORT'
# Jetson Thor Week 4: Multi-Model Support - Completion Report

**Date:** $(date +%Y-%m-%d)
**Status:** ✅ COMPLETE
**Duration:** 5 days

## Executive Summary

Week 4 implemented multi-model support with model registry, LRU caching, and runtime model selection for 6 embedding models on Jetson Thor.

## Achievements

### Models Supported (6 total)
- ✅ Qwen3-0.5B (896-dim, 500MB)
- ✅ Qwen3-1.5B (1536-dim, 1.5GB)
- ✅ Qwen3-4B (4096-dim, 4GB) - Baseline from Week 3
- ✅ Qwen3-7B (4096-dim, 7GB)
- ✅ E5-small-v2 (384-dim, 35MB)
- ✅ BGE-small-en-v1.5 (384-dim, 35MB)

### Infrastructure
- ✅ Model registry with metadata
- ✅ LRU cache (3 models, 8GB limit)
- ✅ Model router with dynamic selection
- ✅ REST API: `POST /api/v1/embed` with `model` parameter
- ✅ REST API: `GET /api/v1/models` (list available models)

### Performance
- ✅ Cache hit: <5ms (warm start)
- ✅ Cache miss: <500ms (cold start from cached TensorRT engine)
- ✅ Zero performance regression for single-model workloads
- ✅ Model switching: <5ms (between cached models)

## Performance Comparison

| Model | Latency (P95) | Throughput (QPS) | Memory | Quality |
|-------|---------------|------------------|--------|---------|
| Qwen3-0.5B | 8ms | 120 | 500 MB | Good |
| Qwen3-1.5B | 15ms | 65 | 1.5 GB | Very Good |
| Qwen3-4B | 25ms | 50 | 4 GB | Excellent |
| Qwen3-7B | 50ms | 25 | 7 GB | Best |
| E5-small-v2 | 3ms | 300 | 35 MB | Good |
| BGE-small-en | 3ms | 300 | 35 MB | Good |

## Next Steps (Week 5)

1. **API Server Deployment** (Week 5)
   - Production REST/gRPC servers
   - Kubernetes Helm charts
   - Load testing at scale

2. **Production Hardening** (Week 6)
   - Monitoring & alerting
   - Circuit breakers
   - Health checks

---

**Report Prepared By:** Backend Team
**Project:** AkiDB - Jetson Thor Multi-Model Support
REPORT
```

**Task 5.3: API Documentation** [2 hours]
```markdown
# Multi-Model Embedding API

## Endpoints

### 1. Generate Embeddings

**POST** `/api/v1/embed`

Generate embeddings using specified model.

**Request Body:**
```json
{
  "model": "qwen3-4b",  // Optional, defaults to "qwen3-4b"
  "texts": [
    "Hello, world!",
    "How are you?"
  ],
  "normalize": true  // Optional, defaults to true
}
```

**Response:**
```json
{
  "embeddings": [
    [0.123, 0.456, ...],  // 4096 dimensions for qwen3-4b
    [0.789, 0.012, ...]
  ],
  "model": "qwen3-4b",
  "dimension": 4096,
  "count": 2
}
```

### 2. List Available Models

**GET** `/api/v1/models`

List all available embedding models.

**Response:**
```json
{
  "models": [
    {
      "id": "qwen3-4b",
      "name": "Qwen/Qwen2.5-4B",
      "dimension": 4096,
      "params": 4000000000,
      "latency_p95_ms": 25,
      "throughput_qps": 50,
      "multilingual": true,
      "languages": ["en", "zh", "ja", "ko", "de", "fr", "es"],
      "memory_mb": 4000
    },
    ...
  ]
}
```

## Model Selection Guide

Choose model based on use case:

- **Low Latency (<10ms):** qwen3-0.5b, e5-small-v2, bge-small-en-v1.5
- **Balanced:** qwen3-1.5b, qwen3-4b
- **High Quality:** qwen3-4b, qwen3-7b
- **Multilingual:** Qwen3 family, e5-small-v2
- **English-Only:** bge-small-en-v1.5
```

**Estimated Time (Day 5):** 7 hours (3h E2E tests + 2h report + 2h API docs)

---

## Success Criteria

### Week 4 Completion Checklist

**Model Support:**
- [ ] 6 models converted to ONNX FP8
- [ ] Model registry with metadata
- [ ] All models benchmarked

**Infrastructure:**
- [ ] LRU cache implemented (3 models, 8GB limit)
- [ ] Model router with dynamic selection
- [ ] Cache hit <5ms, cache miss <500ms

**API Integration:**
- [ ] REST API: `POST /api/v1/embed` with `model` parameter
- [ ] REST API: `GET /api/v1/models`
- [ ] Backward compatible (default model: qwen3-4b)

**Testing:**
- [ ] Unit tests (registry, cache, router)
- [ ] Integration tests (multi-model, concurrent)
- [ ] Performance benchmarks (all 6 models)

**Documentation:**
- [ ] Model performance comparison
- [ ] API documentation with examples
- [ ] Week 4 completion report

### Success Metrics

| Metric | Target | Status |
|--------|--------|--------|
| **Models Supported** | ≥5 | TBD |
| **Cache Hit Latency** | <5ms | TBD |
| **Cache Miss Latency** | <500ms | TBD |
| **Memory Limit** | <8GB (2-3 models) | TBD |
| **Zero Regression** | Week 3 perf maintained | TBD |
| **API Tests** | 100% passing | TBD |

---

## Risk Management

### High Risks

**Risk 1: Model Conversion Quality** 🟡 **MEDIUM**
- **Impact:** Some models may have quality degradation after ONNX conversion
- **Mitigation:**
  - Validate all models against HuggingFace baseline (>0.99 similarity)
  - Use FP8 only for Qwen3 (FP16 for E5/BGE)
  - Test on real-world tasks (MTEB benchmarks)

**Risk 2: Memory Management Bugs** 🟡 **MEDIUM**
- **Impact:** Memory leaks or OOM errors with cache
- **Mitigation:**
  - Extensive testing with memory sanitizers
  - Monitor GPU memory usage during testing
  - Implement graceful degradation (evict all models if OOM)

### Medium Risks

**Risk 3: Cache Eviction Performance** 🟢 **LOW**
- **Impact:** Frequent cache misses may degrade throughput
- **Mitigation:**
  - Use LRU (keeps most-used models loaded)
  - Monitor cache hit rate
  - Adjust cache capacity based on workload

**Risk 4: TensorRT Engine Build Time** 🟢 **LOW**
- **Impact:** First-time model loading takes ~5s (engine compilation)
- **Mitigation:**
  - Pre-build TensorRT engines during installation
  - Cache engines persistently (/var/cache/akidb/trt)
  - Document expected first-run delay

---

## Appendix: Code Examples

### Example 1: Multi-Model Embedding API

```bash
# Example 1: Use default model (qwen3-4b)
curl -X POST http://localhost:8080/api/v1/embed \
  -H "Content-Type: application/json" \
  -d '{
    "texts": ["Hello, world!"]
  }'

# Example 2: Specify low-latency model
curl -X POST http://localhost:8080/api/v1/embed \
  -H "Content-Type: application/json" \
  -d '{
    "model": "qwen3-0.5b",
    "texts": ["Fast embedding generation"]
  }'

# Example 3: High-quality embeddings
curl -X POST http://localhost:8080/api/v1/embed \
  -H "Content-Type: application/json" \
  -d '{
    "model": "qwen3-7b",
    "texts": ["High-quality semantic search"]
  }'

# Example 4: List available models
curl http://localhost:8080/api/v1/models
```

### Example 2: Model Selection in Code

```rust
use akidb_embedding::ModelRouter;

let router = ModelRouter::new(registry, cache);

// Low-latency use case
let embeddings = router.embed(
    Some("qwen3-0.5b"),
    vec!["Real-time command".to_string()]
).await?;

// High-quality use case
let embeddings = router.embed(
    Some("qwen3-7b"),
    vec!["Important document retrieval".to_string()]
).await?;

// Default model
let embeddings = router.embed(
    None,
    vec!["General purpose".to_string()]
).await?;
```

---

**PRD Version:** 1.0
**Last Updated:** $(date +%Y-%m-%d)
**Next Review:** End of Week 4

---

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>
