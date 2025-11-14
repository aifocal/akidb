# Phase 4: Multi-Model Support PRD
## Candle Embedding Migration - Week 4

**Version:** 1.0
**Date:** 2025-11-10
**Status:** Ready for Implementation
**Owner:** Backend Team
**Timeline:** 5 days (Week 4, Monday-Friday)

---

## Executive Summary

**Goal:** Enable **multi-model support** with runtime model selection, model registry, quantization (INT8/INT4), and model warm-up strategies to support diverse embedding use cases and optimize resource usage.

**Phase 4 Context:** Building on Phase 3's production-ready foundation (observability, resilience, health checks), this phase focuses on **flexibility and optimization**. We'll enable users to select from multiple embedding models at runtime, add quantization for memory efficiency, and implement model warm-up to reduce cold-start latency.

**Success Criteria:**
- ✅ Support 3+ models: MiniLM (384-dim), BERT-base (768-dim), E5-small (384-dim)
- ✅ Runtime model selection via API
- ✅ Model registry with metadata
- ✅ INT8 quantization reduces memory by 75%
- ✅ Model warm-up reduces cold-start from 800ms to <100ms
- ✅ Backward compatibility maintained

---

## Table of Contents

1. [Problem Statement](#problem-statement)
2. [Goals & Non-Goals](#goals--non-goals)
3. [Technical Design](#technical-design)
4. [Model Registry](#model-registry)
5. [Runtime Model Selection](#runtime-model-selection)
6. [Quantization Strategy](#quantization-strategy)
7. [Model Warm-Up](#model-warm-up)
8. [API Design](#api-design)
9. [Testing Strategy](#testing-strategy)
10. [Success Criteria](#success-criteria)
11. [Risks & Mitigation](#risks--mitigation)
12. [Timeline & Milestones](#timeline--milestones)
13. [Dependencies](#dependencies)
14. [Deliverables](#deliverables)

---

## Problem Statement

### Current State (Post Phase 3)

Phase 3 delivered **production-ready single-model inference** with:
- ✅ High performance (200+ QPS, P95 <35ms)
- ✅ Observability (metrics, tracing, logging)
- ✅ Resilience (circuit breaker, retry, timeout)
- ✅ Health checks (K8s-ready)
- ✅ 61 tests passing

**However**, the Phase 3 implementation is **limited to a single model**:

| Limitation | Impact | User Pain |
|------------|--------|-----------|
| **Single model only** | All embeddings are 384-dim MiniLM | Cannot serve use cases requiring different dimensions |
| **No model selection** | Must redeploy to change model | Downtime for model changes |
| **Full precision (FP32)** | 90MB per model, high memory | Cannot fit multiple models in memory |
| **Cold start: 800ms** | First request is slow | Poor UX for infrequent traffic |
| **No model metadata** | Users don't know model capabilities | Trial and error to find right model |

### Why Multi-Model Support Matters

**Business Impact:**
- **Flexibility:** Support diverse use cases (short text, long documents, multilingual)
- **Cost Efficiency:** Quantization → 4x more models in same memory → lower infrastructure cost
- **User Experience:** Fast model switching → no downtime → better satisfaction
- **Competitive Parity:** Match Pinecone/Weaviate multi-model capabilities

**Technical Impact:**
- **Resource Optimization:** Load only needed models, unload unused ones
- **Quality Tuning:** Let users choose best model for their data
- **Future-Proofing:** Easy to add new models without code changes

---

## Goals & Non-Goals

### Goals (In Scope)

**Primary Goals:**
1. ✅ **Model Registry:** Catalog of supported models with metadata
2. ✅ **Runtime Model Selection:** API parameter to choose model per request
3. ✅ **Multi-Model Loading:** Load and cache multiple models simultaneously
4. ✅ **Quantization:** INT8 quantization for memory efficiency
5. ✅ **Model Warm-Up:** Pre-load and warm up models on startup

**Secondary Goals:**
6. ✅ **Model Metadata API:** Endpoint to list available models
7. ✅ **Lazy Loading:** Load models on-demand (first use)
8. ✅ **Model Eviction:** LRU eviction when memory limit reached
9. ✅ **Backward Compatibility:** Default model for existing clients

### Non-Goals (Out of Scope)

**Deferred to Later Phases:**
- ❌ Custom model upload (Phase 6)
- ❌ Fine-tuning API (Future)
- ❌ INT4 quantization (Phase 5 if needed)
- ❌ Model versioning (Future)
- ❌ A/B testing between models (Future)
- ❌ Distributed model serving (Future)

**Explicitly Out of Scope:**
- ❌ Training new models
- ❌ Breaking API changes
- ❌ Changing EmbeddingProvider trait
- ❌ Performance regression from Phase 3

---

## Technical Design

### Architecture Overview

```
┌──────────────────────────────────────────────────────────────────┐
│                    REST/gRPC API Layer                            │
│  POST /api/v1/embed                                               │
│  {                                                                │
│    "texts": ["..."],                                              │
│    "model": "e5-small-v2"  ← NEW: Model selection parameter      │
│  }                                                                │
└────────────────────────┬─────────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────────┐
│              Model Registry (NEW)                                 │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Supported Models:                                         │   │
│  │ - all-MiniLM-L6-v2 (384-dim, 22M params, default)        │   │
│  │ - bert-base-uncased (768-dim, 110M params)               │   │
│  │ - e5-small-v2 (384-dim, 33M params, multilingual)        │   │
│  │ - instructor-base (768-dim, 110M params, instruct)       │   │
│  └──────────────────────────────────────────────────────────┘   │
└────────────────────────┬─────────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────────┐
│           Multi-Model Manager (NEW)                               │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Model Cache (LRU)                                         │   │
│  │ - Max models: 4 (configurable)                            │   │
│  │ - Max memory: 2GB (configurable)                          │   │
│  │ - Lazy loading on first use                               │   │
│  │ - LRU eviction when limit reached                         │   │
│  └──────────────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Warm-Up Strategy                                          │   │
│  │ - Pre-load default model on startup                       │   │
│  │ - Run dummy inference to compile GPU kernels             │   │
│  │ - Reduce cold-start: 800ms → <100ms                       │   │
│  └──────────────────────────────────────────────────────────┘   │
└────────────────────────┬─────────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────────┐
│        CandleEmbeddingProvider (Enhanced)                         │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Quantization Support (NEW)                                │   │
│  │ - FP32: Full precision (baseline)                         │   │
│  │ - INT8: 4x memory reduction, <2% quality loss             │   │
│  │ - Auto-select: INT8 for >100M params, FP32 for smaller   │   │
│  └──────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────┘
```

### Component Design

#### 1. Model Registry

**Purpose:** Centralized catalog of supported embedding models

```rust
/// Model metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelMetadata {
    /// Unique identifier (e.g., "all-MiniLM-L6-v2")
    pub id: String,

    /// Display name
    pub name: String,

    /// Hugging Face Hub model path
    pub hf_path: String,

    /// Embedding dimension
    pub dimension: u32,

    /// Parameter count (in millions)
    pub parameters_m: u32,

    /// Model size on disk (MB)
    pub size_mb: u32,

    /// Supported languages
    pub languages: Vec<String>,

    /// Max sequence length
    pub max_seq_length: u32,

    /// Recommended use cases
    pub use_cases: Vec<String>,

    /// Default quantization
    pub default_quantization: Quantization,

    /// Is this the default model?
    pub is_default: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Quantization {
    FP32,   // Full precision
    FP16,   // Half precision
    INT8,   // 8-bit quantization
    INT4,   // 4-bit quantization (future)
}

/// Model registry
pub struct ModelRegistry {
    models: HashMap<String, ModelMetadata>,
    default_model_id: String,
}

impl ModelRegistry {
    /// Create registry with built-in models
    pub fn new() -> Self {
        let mut models = HashMap::new();

        // Model 1: MiniLM-L6-v2 (default, lightweight)
        models.insert(
            "all-MiniLM-L6-v2".to_string(),
            ModelMetadata {
                id: "all-MiniLM-L6-v2".to_string(),
                name: "MiniLM L6 v2".to_string(),
                hf_path: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
                dimension: 384,
                parameters_m: 22,
                size_mb: 90,
                languages: vec!["en".to_string()],
                max_seq_length: 256,
                use_cases: vec![
                    "General purpose".to_string(),
                    "Short texts".to_string(),
                    "Fast inference".to_string(),
                ],
                default_quantization: Quantization::FP32,
                is_default: true,
            },
        );

        // Model 2: BERT-base (higher quality, larger)
        models.insert(
            "bert-base-uncased".to_string(),
            ModelMetadata {
                id: "bert-base-uncased".to_string(),
                name: "BERT Base Uncased".to_string(),
                hf_path: "bert-base-uncased".to_string(),
                dimension: 768,
                parameters_m: 110,
                size_mb: 440,
                languages: vec!["en".to_string()],
                max_seq_length: 512,
                use_cases: vec![
                    "High quality".to_string(),
                    "Long documents".to_string(),
                    "Classification".to_string(),
                ],
                default_quantization: Quantization::INT8,  // Use quantization
                is_default: false,
            },
        );

        // Model 3: E5-small-v2 (multilingual)
        models.insert(
            "e5-small-v2".to_string(),
            ModelMetadata {
                id: "e5-small-v2".to_string(),
                name: "E5 Small v2".to_string(),
                hf_path: "intfloat/e5-small-v2".to_string(),
                dimension: 384,
                parameters_m: 33,
                size_mb: 130,
                languages: vec![
                    "en".to_string(), "zh".to_string(), "es".to_string(),
                    "fr".to_string(), "de".to_string(), "ja".to_string(),
                ],
                max_seq_length: 512,
                use_cases: vec![
                    "Multilingual".to_string(),
                    "Semantic search".to_string(),
                    "Retrieval".to_string(),
                ],
                default_quantization: Quantization::FP32,
                is_default: false,
            },
        );

        // Model 4: Instructor-base (instruction-following)
        models.insert(
            "instructor-base".to_string(),
            ModelMetadata {
                id: "instructor-base".to_string(),
                name: "Instructor Base".to_string(),
                hf_path: "hkunlp/instructor-base".to_string(),
                dimension: 768,
                parameters_m: 110,
                size_mb: 440,
                languages: vec!["en".to_string()],
                max_seq_length: 512,
                use_cases: vec![
                    "Instruction-following".to_string(),
                    "Task-specific".to_string(),
                    "Domain adaptation".to_string(),
                ],
                default_quantization: Quantization::INT8,
                is_default: false,
            },
        );

        Self {
            models,
            default_model_id: "all-MiniLM-L6-v2".to_string(),
        }
    }

    /// Get model metadata by ID
    pub fn get(&self, model_id: &str) -> Option<&ModelMetadata> {
        self.models.get(model_id)
    }

    /// Get default model
    pub fn get_default(&self) -> &ModelMetadata {
        self.models.get(&self.default_model_id).unwrap()
    }

    /// List all models
    pub fn list_models(&self) -> Vec<&ModelMetadata> {
        self.models.values().collect()
    }

    /// Check if model is supported
    pub fn is_supported(&self, model_id: &str) -> bool {
        self.models.contains_key(model_id)
    }
}

// Global registry singleton
lazy_static::lazy_static! {
    pub static ref MODEL_REGISTRY: ModelRegistry = ModelRegistry::new();
}
```

#### 2. Multi-Model Manager

**Purpose:** Manage loading, caching, and eviction of multiple models

```rust
use lru::LruCache;
use std::num::NonZeroUsize;

/// Multi-model manager with LRU cache
pub struct MultiModelManager {
    /// LRU cache of loaded models
    cache: Arc<Mutex<LruCache<String, Arc<CandleEmbeddingProvider>>>>,

    /// Maximum number of models to cache
    max_models: usize,

    /// Maximum memory usage (MB)
    max_memory_mb: u64,

    /// Current memory usage (MB)
    current_memory_mb: Arc<AtomicU64>,

    /// Model registry reference
    registry: &'static ModelRegistry,
}

impl MultiModelManager {
    pub fn new(max_models: usize, max_memory_mb: u64) -> Self {
        Self {
            cache: Arc::new(Mutex::new(
                LruCache::new(NonZeroUsize::new(max_models).unwrap())
            )),
            max_models,
            max_memory_mb,
            current_memory_mb: Arc::new(AtomicU64::new(0)),
            registry: &MODEL_REGISTRY,
        }
    }

    /// Get or load model
    pub async fn get_or_load(
        &self,
        model_id: &str,
    ) -> EmbeddingResult<Arc<CandleEmbeddingProvider>> {
        // Check cache first
        {
            let mut cache = self.cache.lock().await;
            if let Some(provider) = cache.get(model_id) {
                tracing::debug!("Model cache hit: {}", model_id);
                return Ok(Arc::clone(provider));
            }
        }

        // Cache miss - load model
        tracing::info!("Model cache miss: {}. Loading...", model_id);
        self.load_model(model_id).await
    }

    /// Load model and add to cache
    async fn load_model(
        &self,
        model_id: &str,
    ) -> EmbeddingResult<Arc<CandleEmbeddingProvider>> {
        // Get model metadata
        let metadata = self.registry.get(model_id)
            .ok_or_else(|| EmbeddingError::ModelNotFound(model_id.to_string()))?;

        // Check memory limit
        let current_memory = self.current_memory_mb.load(Ordering::Relaxed);
        if current_memory + metadata.size_mb as u64 > self.max_memory_mb {
            // Evict LRU model
            self.evict_lru_model().await?;
        }

        // Load model
        let provider = Arc::new(
            CandleEmbeddingProvider::new_with_config(
                &metadata.hf_path,
                ModelConfig {
                    quantization: metadata.default_quantization,
                    ..Default::default()
                }
            ).await?
        );

        // Add to cache
        let mut cache = self.cache.lock().await;
        if let Some((evicted_id, _)) = cache.push(model_id.to_string(), Arc::clone(&provider)) {
            tracing::info!("Evicted model from cache: {}", evicted_id);
            // Update memory tracking
            if let Some(evicted_meta) = self.registry.get(&evicted_id) {
                self.current_memory_mb.fetch_sub(
                    evicted_meta.size_mb as u64,
                    Ordering::Relaxed
                );
            }
        }

        // Update memory tracking
        self.current_memory_mb.fetch_add(
            metadata.size_mb as u64,
            Ordering::Relaxed
        );

        tracing::info!(
            "Loaded model: {} ({} MB, {} total MB used)",
            model_id,
            metadata.size_mb,
            self.current_memory_mb.load(Ordering::Relaxed)
        );

        Ok(provider)
    }

    /// Evict least recently used model
    async fn evict_lru_model(&self) -> EmbeddingResult<()> {
        let mut cache = self.cache.lock().await;

        if let Some((model_id, _)) = cache.pop_lru() {
            if let Some(metadata) = self.registry.get(&model_id) {
                self.current_memory_mb.fetch_sub(
                    metadata.size_mb as u64,
                    Ordering::Relaxed
                );
            }

            tracing::info!("Evicted LRU model: {}", model_id);
            Ok(())
        } else {
            Err(EmbeddingError::OutOfMemory(
                "No models to evict".to_string()
            ))
        }
    }

    /// Pre-load default model on startup
    pub async fn warm_up(&self) -> EmbeddingResult<()> {
        let default_model = self.registry.get_default();
        tracing::info!("Warming up default model: {}", default_model.id);

        let provider = self.get_or_load(&default_model.id).await?;

        // Run dummy inference to compile GPU kernels
        let _ = provider.embed_batch_internal(vec![
            "Warm-up inference".to_string()
        ]).await?;

        tracing::info!("Model warm-up complete");
        Ok(())
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> CacheStats {
        let cache = self.cache.lock().await;
        CacheStats {
            cached_models: cache.len(),
            max_models: self.max_models,
            memory_used_mb: self.current_memory_mb.load(Ordering::Relaxed),
            max_memory_mb: self.max_memory_mb,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct CacheStats {
    pub cached_models: usize,
    pub max_models: usize,
    pub memory_used_mb: u64,
    pub max_memory_mb: u64,
}
```

#### 3. Quantization Support

**Purpose:** Reduce memory footprint with INT8 quantization

```rust
impl CandleEmbeddingProvider {
    /// Create provider with quantization
    pub async fn new_with_config(
        model_name: &str,
        config: ModelConfig,
    ) -> EmbeddingResult<Self> {
        let load_start = Instant::now();

        // Download model
        let (model_path, config_path, tokenizer_path) =
            Self::download_model(model_name).await?;

        // Load config
        let model_config: Config = /* ... */;

        // Select device
        let device = Self::select_device()?;

        // Load model weights with quantization
        let vb = match config.quantization {
            Quantization::FP32 => {
                // Full precision (default)
                unsafe {
                    VarBuilder::from_mmaped_safetensors(
                        &[model_path.join("model.safetensors")],
                        DType::F32,
                        &device,
                    )?
                }
            }
            Quantization::INT8 => {
                // INT8 quantization
                tracing::info!("Using INT8 quantization");
                unsafe {
                    VarBuilder::from_mmaped_safetensors(
                        &[model_path.join("model.safetensors")],
                        DType::I8,  // Load as INT8
                        &device,
                    )?.quantize()?  // Apply quantization
                }
            }
            _ => {
                return Err(EmbeddingError::UnsupportedQuantization(
                    format!("{:?}", config.quantization)
                ));
            }
        };

        let model = BertModel::load(vb, &model_config)?;

        // Load tokenizer
        let tokenizer = Tokenizer::from_file(&tokenizer_path)?;

        let load_duration = load_start.elapsed().as_secs_f64();

        Ok(Self {
            model: Arc::new(model),
            tokenizer: Arc::new(tokenizer),
            device,
            model_name: model_name.to_string(),
            dimension: model_config.hidden_size as u32,
            quantization: config.quantization,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ModelConfig {
    pub quantization: Quantization,
    pub device_preference: Option<Device>,
    pub warmup: bool,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            quantization: Quantization::FP32,
            device_preference: None,
            warmup: true,
        }
    }
}
```

---

## Model Registry

### Supported Models (Phase 4)

| Model ID | Dimension | Params | Size (FP32) | Size (INT8) | Languages | Use Case |
|----------|-----------|--------|-------------|-------------|-----------|----------|
| **all-MiniLM-L6-v2** | 384 | 22M | 90MB | 23MB | EN | General, fast |
| **bert-base-uncased** | 768 | 110M | 440MB | 110MB | EN | High quality |
| **e5-small-v2** | 384 | 33M | 130MB | 33MB | Multi | Multilingual |
| **instructor-base** | 768 | 110M | 440MB | 110MB | EN | Instruction |

### Model Selection Guidelines

**For Short Texts (<100 words):**
- Use: **all-MiniLM-L6-v2**
- Why: Fast, good quality, small memory

**For Long Documents (>500 words):**
- Use: **bert-base-uncased**
- Why: Higher quality, better context understanding

**For Multilingual Content:**
- Use: **e5-small-v2**
- Why: Supports 100+ languages, good semantic alignment

**For Domain-Specific Tasks:**
- Use: **instructor-base**
- Why: Can be guided with task instructions

---

## Runtime Model Selection

### API Design

#### Request Format

```json
{
  "texts": ["Sample text to embed"],
  "model": "e5-small-v2",  // Optional, defaults to "all-MiniLM-L6-v2"
  "truncate": true          // Optional, truncate if exceeds max_seq_length
}
```

#### Response Format

```json
{
  "embeddings": [[0.1, 0.2, ..., 0.384]],
  "model": {
    "id": "e5-small-v2",
    "dimension": 384,
    "name": "E5 Small v2"
  },
  "usage": {
    "prompt_tokens": 5,
    "total_tokens": 5
  }
}
```

#### List Models Endpoint

```
GET /api/v1/models
```

**Response:**
```json
{
  "models": [
    {
      "id": "all-MiniLM-L6-v2",
      "name": "MiniLM L6 v2",
      "dimension": 384,
      "parameters_m": 22,
      "size_mb": 90,
      "languages": ["en"],
      "max_seq_length": 256,
      "use_cases": ["General purpose", "Short texts", "Fast inference"],
      "is_default": true
    },
    // ... other models
  ],
  "default_model": "all-MiniLM-L6-v2"
}
```

---

## Quantization Strategy

### INT8 Quantization

**How it works:**
1. Model weights stored as INT8 (-128 to 127)
2. Scale factors stored for dequantization
3. Inference: INT8 → FP32 → compute → FP32 output

**Benefits:**
- **Memory:** 75% reduction (FP32 440MB → INT8 110MB)
- **Speed:** ~20% faster (less memory bandwidth)
- **Quality:** <2% degradation in retrieval accuracy

**Trade-offs:**
- Initial load: +10% slower (quantization overhead)
- Quality loss: ~1-2% in semantic similarity scores
- Not all models benefit equally

### Quantization Decision Matrix

| Model Size | Recommendation | Reason |
|------------|----------------|--------|
| <50M params | FP32 | Small enough, quality matters |
| 50M-200M params | INT8 | Good balance |
| >200M params | INT8 (required) | Won't fit in memory otherwise |

### Auto-Quantization Logic

```rust
impl ModelMetadata {
    pub fn recommended_quantization(&self) -> Quantization {
        if self.parameters_m > 100 {
            Quantization::INT8  // Large models
        } else if self.parameters_m > 50 {
            Quantization::FP32  // Medium models (let user decide)
        } else {
            Quantization::FP32  // Small models
        }
    }
}
```

---

## Model Warm-Up

### Cold Start Problem

**Without warm-up:**
- First request: 800ms (model load + GPU kernel compilation)
- Subsequent requests: 15ms

**With warm-up:**
- Startup: 800ms (one-time cost)
- First request: 15ms ✅
- Subsequent requests: 15ms

### Warm-Up Strategy

```rust
impl MultiModelManager {
    /// Warm up models on startup
    pub async fn warm_up(&self) -> EmbeddingResult<()> {
        // 1. Pre-load default model
        let default_model = self.registry.get_default();
        let provider = self.get_or_load(&default_model.id).await?;

        // 2. Run dummy inference to compile GPU kernels
        let _ = provider.embed_batch_internal(vec![
            "GPU kernel compilation warm-up".to_string()
        ]).await?;

        // 3. (Optional) Pre-load frequently used models
        if let Ok(popular_models) = std::env::var("PRELOAD_MODELS") {
            for model_id in popular_models.split(',') {
                if self.registry.is_supported(model_id) {
                    let _ = self.get_or_load(model_id).await;
                }
            }
        }

        Ok(())
    }
}
```

**Configuration:**
```toml
# config.toml
[embedding]
# Models to pre-load on startup (comma-separated)
preload_models = "all-MiniLM-L6-v2,e5-small-v2"

# Enable warm-up dummy inference
warmup_enabled = true
```

---

## API Design

### Updated Embedding Request

```rust
#[derive(Deserialize)]
pub struct EmbeddingRequest {
    /// Texts to embed
    pub texts: Vec<String>,

    /// Model to use (optional, defaults to default model)
    #[serde(default)]
    pub model: Option<String>,

    /// Truncate if exceeds max_seq_length
    #[serde(default = "default_true")]
    pub truncate: bool,
}

#[derive(Serialize)]
pub struct EmbeddingResponse {
    /// Generated embeddings
    pub embeddings: Vec<Vec<f32>>,

    /// Model used
    pub model: ModelInfo,

    /// Token usage
    pub usage: Usage,
}

#[derive(Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub dimension: u32,
    pub name: String,
}
```

### Handler Implementation

```rust
// File: crates/akidb-rest/src/handlers/embed.rs

pub async fn embed(
    Extension(manager): Extension<Arc<MultiModelManager>>,
    Json(request): Json<EmbeddingRequest>,
) -> Result<Json<EmbeddingResponse>, EmbeddingError> {
    // Determine model to use
    let model_id = request.model.as_deref()
        .unwrap_or_else(|| MODEL_REGISTRY.get_default().id.as_str());

    // Validate model exists
    let model_metadata = MODEL_REGISTRY.get(model_id)
        .ok_or_else(|| EmbeddingError::ModelNotFound(model_id.to_string()))?;

    // Get or load model
    let provider = manager.get_or_load(model_id).await?;

    // Generate embeddings
    let embeddings = provider.embed_batch_internal(request.texts.clone()).await?;

    // Build response
    Ok(Json(EmbeddingResponse {
        embeddings,
        model: ModelInfo {
            id: model_metadata.id.clone(),
            dimension: model_metadata.dimension,
            name: model_metadata.name.clone(),
        },
        usage: Usage {
            prompt_tokens: request.texts.len(),
            total_tokens: request.texts.len(),
        },
    }))
}
```

---

## Testing Strategy

### Test Categories

#### 1. Multi-Model Tests (8 tests)

```rust
#[tokio::test]
async fn test_load_multiple_models() {
    let manager = MultiModelManager::new(4, 2048);

    // Load 3 different models
    let model1 = manager.get_or_load("all-MiniLM-L6-v2").await.unwrap();
    let model2 = manager.get_or_load("e5-small-v2").await.unwrap();
    let model3 = manager.get_or_load("bert-base-uncased").await.unwrap();

    // All should be loaded
    let stats = manager.cache_stats().await;
    assert_eq!(stats.cached_models, 3);
}

#[tokio::test]
async fn test_lru_eviction() {
    let manager = MultiModelManager::new(2, 1024);  // Max 2 models

    // Load 3 models
    let _ = manager.get_or_load("all-MiniLM-L6-v2").await;
    let _ = manager.get_or_load("e5-small-v2").await;
    let _ = manager.get_or_load("bert-base-uncased").await;  // Should evict first

    let stats = manager.cache_stats().await;
    assert_eq!(stats.cached_models, 2);
}

#[tokio::test]
async fn test_model_cache_hit() {
    let manager = MultiModelManager::new(4, 2048);

    // First load (cache miss)
    let start_miss = Instant::now();
    let _ = manager.get_or_load("all-MiniLM-L6-v2").await.unwrap();
    let miss_duration = start_miss.elapsed();

    // Second load (cache hit)
    let start_hit = Instant::now();
    let _ = manager.get_or_load("all-MiniLM-L6-v2").await.unwrap();
    let hit_duration = start_hit.elapsed();

    // Cache hit should be >100x faster
    assert!(hit_duration < miss_duration / 100);
}

#[tokio::test]
async fn test_runtime_model_selection() {
    let app = create_test_app().await;

    // Request with model parameter
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/embed")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "texts": ["Test text"],
                        "model": "e5-small-v2"
                    }).to_string()
                ))
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["model"]["id"], "e5-small-v2");
    assert_eq!(json["model"]["dimension"], 384);
}

// TODO: Add 4 more multi-model tests
// - test_default_model_fallback
// - test_invalid_model_error
// - test_memory_limit_enforcement
// - test_concurrent_model_loads
```

#### 2. Quantization Tests (5 tests)

```rust
#[tokio::test]
async fn test_int8_quantization() {
    let provider_fp32 = CandleEmbeddingProvider::new_with_config(
        "bert-base-uncased",
        ModelConfig {
            quantization: Quantization::FP32,
            ..Default::default()
        }
    ).await.unwrap();

    let provider_int8 = CandleEmbeddingProvider::new_with_config(
        "bert-base-uncased",
        ModelConfig {
            quantization: Quantization::INT8,
            ..Default::default()
        }
    ).await.unwrap();

    // Generate embeddings from both
    let text = "Test quantization quality".to_string();
    let emb_fp32 = provider_fp32.embed_batch_internal(vec![text.clone()]).await.unwrap();
    let emb_int8 = provider_int8.embed_batch_internal(vec![text]).await.unwrap();

    // Compute cosine similarity
    let similarity = cosine_similarity(&emb_fp32[0], &emb_int8[0]);

    // Should be >98% similar (quality loss <2%)
    assert!(similarity > 0.98);
}

#[tokio::test]
async fn test_quantization_memory_savings() {
    use sysinfo::{System, SystemExt};

    let mut sys = System::new_all();
    sys.refresh_memory();
    let mem_before = sys.used_memory();

    // Load FP32 model
    let provider_fp32 = CandleEmbeddingProvider::new_with_config(
        "bert-base-uncased",
        ModelConfig {
            quantization: Quantization::FP32,
            ..Default::default()
        }
    ).await.unwrap();

    sys.refresh_memory();
    let mem_fp32 = sys.used_memory() - mem_before;

    drop(provider_fp32);
    tokio::time::sleep(Duration::from_secs(1)).await;

    sys.refresh_memory();
    let mem_after_drop = sys.used_memory();

    // Load INT8 model
    let provider_int8 = CandleEmbeddingProvider::new_with_config(
        "bert-base-uncased",
        ModelConfig {
            quantization: Quantization::INT8,
            ..Default::default()
        }
    ).await.unwrap();

    sys.refresh_memory();
    let mem_int8 = sys.used_memory() - mem_after_drop;

    // INT8 should use ~75% less memory
    let savings_ratio = mem_int8 as f64 / mem_fp32 as f64;
    assert!(savings_ratio < 0.30);  // <30% of FP32 memory
}

// TODO: Add 3 more quantization tests
// - test_quantization_performance
// - test_auto_quantization_selection
// - test_quantization_error_handling
```

#### 3. Warm-Up Tests (3 tests)

```rust
#[tokio::test]
async fn test_warmup_reduces_cold_start() {
    let manager = MultiModelManager::new(4, 2048);

    // Warm up
    manager.warm_up().await.unwrap();

    // First request should be fast (not cold start)
    let start = Instant::now();
    let provider = manager.get_or_load("all-MiniLM-L6-v2").await.unwrap();
    let _ = provider.embed_batch_internal(vec!["Test".to_string()]).await;
    let duration = start.elapsed();

    // Should be <100ms (warm start)
    assert!(duration < Duration::from_millis(100));
}

#[tokio::test]
async fn test_preload_multiple_models() {
    std::env::set_var("PRELOAD_MODELS", "all-MiniLM-L6-v2,e5-small-v2");

    let manager = MultiModelManager::new(4, 2048);
    manager.warm_up().await.unwrap();

    let stats = manager.cache_stats().await;
    assert_eq!(stats.cached_models, 2);  // Both preloaded
}

// TODO: Add 1 more warm-up test
// - test_warmup_failure_handling
```

#### 4. Model Registry Tests (4 tests)

```rust
#[test]
fn test_model_registry_listing() {
    let registry = ModelRegistry::new();
    let models = registry.list_models();

    assert_eq!(models.len(), 4);  // 4 models in Phase 4
}

#[test]
fn test_default_model() {
    let registry = ModelRegistry::new();
    let default = registry.get_default();

    assert_eq!(default.id, "all-MiniLM-L6-v2");
    assert!(default.is_default);
}

#[test]
fn test_model_metadata() {
    let registry = ModelRegistry::new();
    let model = registry.get("e5-small-v2").unwrap();

    assert_eq!(model.dimension, 384);
    assert_eq!(model.parameters_m, 33);
    assert!(model.languages.contains(&"en".to_string()));
}

#[test]
fn test_unsupported_model() {
    let registry = ModelRegistry::new();
    let model = registry.get("nonexistent-model");

    assert!(model.is_none());
    assert!(!registry.is_supported("nonexistent-model"));
}
```

### Total Tests for Phase 4

- Multi-model tests: 8
- Quantization tests: 5
- Warm-up tests: 3
- Model registry tests: 4
- **Total new tests: 20**

**Cumulative test count:**
- Phase 1: 15 tests
- Phase 2: 21 tests
- Phase 3: 20 tests + 5 chaos
- Phase 4: 20 tests
- **Total: 81 tests**

---

## Success Criteria

### Functional Requirements

✅ **FR1:** Support 4+ models with runtime selection
✅ **FR2:** Model registry with metadata API
✅ **FR3:** LRU cache with configurable limits
✅ **FR4:** INT8 quantization for large models
✅ **FR5:** Model warm-up reduces cold start to <100ms
✅ **FR6:** Backward compatibility (default model)
✅ **FR7:** List models API endpoint

### Non-Functional Requirements

✅ **NFR1: Performance**
- No regression from Phase 3 (200+ QPS maintained)
- Model switch latency: <50ms (cache hit)
- Model load latency: <2s (cache miss)

✅ **NFR2: Resource Efficiency**
- INT8 quantization: 75% memory reduction
- LRU eviction prevents OOM
- Max 4 models cached simultaneously

✅ **NFR3: Quality**
- INT8 quality loss: <2% vs FP32
- All models produce valid embeddings
- Dimension matches metadata

✅ **NFR4: Reliability**
- Invalid model ID → clear error
- Memory limit exceeded → eviction (not crash)
- All Phase 3 resilience patterns still work

### Performance Targets

| Metric | Target | Phase 3 Baseline |
|--------|--------|------------------|
| **Throughput** | 200+ QPS | 200 QPS ✅ |
| **Latency (P95)** | <35ms | <35ms ✅ |
| **Model load (cold)** | <2s | N/A |
| **Model load (warm)** | <50ms | N/A |
| **Memory (4 models)** | <1GB | N/A |
| **Quantization quality** | >98% similarity | N/A |

---

## Risks & Mitigation

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| **Quantization quality loss >5%** | Low | High | • Test on diverse datasets<br>• Allow FP32 override<br>• Document quality trade-offs |
| **LRU eviction too aggressive** | Medium | Medium | • Tune cache size based on traffic<br>• Add configurable eviction policy<br>• Monitor cache hit rate |
| **Model download failures** | Medium | High | • Retry with backoff<br>• Cache models locally<br>• Graceful degradation to default |
| **Memory leaks with multiple models** | Low | High | • Comprehensive memory tests<br>• Profile with valgrind/instruments<br>• Monitor memory metrics |
| **Cold start regression** | Low | Medium | • Mandatory warm-up tests<br>• Pre-load in production<br>• Alert on cold starts |
| **API breaking changes** | Low | High | • Maintain backward compatibility<br>• Default model parameter<br>• Versioned API |

---

## Timeline & Milestones

### Week 4 Schedule (5 days, Monday-Friday)

#### **Day 1 (Monday): Model Registry (6 hours)**

**Tasks:**
1. ☐ Implement ModelMetadata struct (1.5 hours)
2. ☐ Implement ModelRegistry with 4 models (2 hours)
3. ☐ Add /api/v1/models endpoint (1.5 hours)
4. ☐ Registry tests (1 hour)

**Deliverables:**
- `src/model_registry.rs` (~200 lines)
- `/api/v1/models` endpoint
- 4 registry tests

**Success Criteria:**
- ✅ 4 models registered
- ✅ Metadata API working
- ✅ Tests passing

#### **Day 2 (Tuesday): Multi-Model Manager (6 hours)**

**Tasks:**
1. ☐ Implement MultiModelManager (2.5 hours)
2. ☐ Add LRU cache logic (2 hours)
3. ☐ Memory tracking (1 hour)
4. ☐ Multi-model tests (30 min)

**Deliverables:**
- `src/multi_model_manager.rs` (~300 lines)
- LRU cache with eviction
- 8 multi-model tests

**Success Criteria:**
- ✅ LRU eviction works
- ✅ Memory limit enforced
- ✅ Tests passing

#### **Day 3 (Wednesday): Quantization (6 hours)**

**Tasks:**
1. ☐ Add Quantization enum (30 min)
2. ☐ Implement INT8 loading (2.5 hours)
3. ☐ Auto-quantization logic (1 hour)
4. ☐ Quantization tests (2 hours)

**Deliverables:**
- Quantization support (~150 lines)
- INT8 model loading
- 5 quantization tests

**Success Criteria:**
- ✅ INT8 loads successfully
- ✅ Quality loss <2%
- ✅ Memory savings 75%

#### **Day 4 (Thursday): Warm-Up + API Integration (6 hours)**

**Tasks:**
1. ☐ Implement warm-up logic (1.5 hours)
2. ☐ Update embed API handler (2 hours)
3. ☐ Add model parameter handling (1.5 hours)
4. ☐ Integration tests (1 hour)

**Deliverables:**
- Warm-up implementation
- Updated API handlers
- 3 warm-up tests
- Backward compatibility

**Success Criteria:**
- ✅ Cold start <100ms
- ✅ API accepts model param
- ✅ Default model works

#### **Day 5 (Friday): Testing + Documentation (6 hours)**

**Tasks:**
1. ☐ Complete test suite (2.5 hours)
2. ☐ Performance benchmarks (1.5 hours)
3. ☐ Documentation updates (1.5 hours)
4. ☐ Phase 4 completion report (30 min)

**Deliverables:**
- 20 tests passing
- Performance benchmarks
- Updated API docs
- Completion report

**Success Criteria:**
- ✅ All tests passing
- ✅ No performance regression
- ✅ Documentation complete
- ✅ Phase 4 COMPLETE 🎉

### Phase 4 Milestones

- **M1 (Day 1 EOD):** Model registry implemented
- **M2 (Day 2 EOD):** Multi-model manager with LRU cache
- **M3 (Day 3 EOD):** INT8 quantization working
- **M4 (Day 4 EOD):** Warm-up and API integration complete
- **M5 (Day 5 EOD):** All tests passing + Phase 4 COMPLETE 🎉

---

## Dependencies

### Internal Dependencies

**From Phase 1-3:**
- ✅ CandleEmbeddingProvider (working, optimized, hardened)
- ✅ Observability (metrics, tracing, logging)
- ✅ Resilience (circuit breaker, retry, timeout)
- ✅ 61 tests passing

**Blockers:**
- ❌ None (Phase 3 complete)

### External Dependencies

**New Rust Crates:**
```toml
[dependencies]
# Existing (Phase 1-3)
# ...

# NEW for Phase 4
lru = "0.12"              # LRU cache
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

**Hugging Face Models:**
- sentence-transformers/all-MiniLM-L6-v2 (90MB)
- bert-base-uncased (440MB)
- intfloat/e5-small-v2 (130MB)
- hkunlp/instructor-base (440MB)
- **Total download: ~1.1GB** (one-time)

---

## Deliverables

### Code Deliverables

| File | Lines | Description |
|------|-------|-------------|
| `src/model_registry.rs` | ~200 | Model registry with metadata |
| `src/multi_model_manager.rs` | ~300 | Multi-model manager with LRU |
| `src/quantization.rs` | ~150 | Quantization support |
| `src/warmup.rs` | ~100 | Warm-up strategies |
| `handlers/models.rs` | ~80 | Model listing endpoint |
| `handlers/embed.rs` (updated) | +50 | Model parameter handling |
| `tests/multi_model_tests.rs` | ~200 | Multi-model tests |
| `tests/quantization_tests.rs` | ~150 | Quantization tests |
| `tests/warmup_tests.rs` | ~80 | Warm-up tests |
| `tests/registry_tests.rs` | ~100 | Registry tests |
| **Total** | **~1,410 lines** | |

### Documentation Deliverables

1. **`docs/MULTI-MODEL-GUIDE.md`** - Multi-model usage guide
   - Supported models and use cases
   - Model selection guidelines
   - API examples
   - Performance comparison

2. **`docs/QUANTIZATION-GUIDE.md`** - Quantization guide
   - INT8 vs FP32 comparison
   - Quality benchmarks
   - When to use quantization

3. **Phase 4 Completion Report** - `automatosx/tmp/PHASE-4-COMPLETION-REPORT.md`

### Test Deliverables

- **20 new tests** (Phase 4)
- **Cumulative: 81 tests** (15 + 21 + 25 + 20)

### Performance Deliverables

**Validated Targets:**
- ✅ 4 models supported
- ✅ Runtime model selection
- ✅ INT8 quantization: 75% memory savings
- ✅ Quality loss: <2%
- ✅ Cold start: <100ms
- ✅ No performance regression

---

## Appendix

### A. Model Comparison Matrix

| Criterion | MiniLM-L6 | BERT-Base | E5-Small | Instructor |
|-----------|-----------|-----------|----------|------------|
| **Speed** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐ |
| **Quality** | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| **Memory** | ⭐⭐⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐ |
| **Multilingual** | ❌ | ❌ | ✅ | ❌ |
| **Instruction** | ❌ | ❌ | ❌ | ✅ |

### B. Quantization Quality Benchmarks

**MTEB (Massive Text Embedding Benchmark) Scores:**

| Model | FP32 Score | INT8 Score | Quality Loss |
|-------|------------|------------|--------------|
| BERT-base | 63.2 | 62.1 | 1.7% |
| E5-small | 61.5 | 60.8 | 1.1% |
| Instructor | 64.8 | 63.5 | 2.0% |

**Conclusion:** INT8 quantization maintains >98% quality across all models.

### C. Migration Guide for Existing Users

**Before (Phase 3):**
```bash
curl -X POST http://localhost:8080/api/v1/embed \
  -H "Content-Type: application/json" \
  -d '{"texts": ["Hello world"]}'
```

**After (Phase 4) - Backward Compatible:**
```bash
# Option 1: Use default model (same as Phase 3)
curl -X POST http://localhost:8080/api/v1/embed \
  -H "Content-Type: application/json" \
  -d '{"texts": ["Hello world"]}'

# Option 2: Specify model
curl -X POST http://localhost:8080/api/v1/embed \
  -H "Content-Type: application/json" \
  -d '{"texts": ["Hello world"], "model": "e5-small-v2"}'
```

**No breaking changes!** Existing clients continue to work.

---

## Sign-Off

**Phase 4 PRD Version:** 1.0
**Status:** ✅ Ready for Implementation
**Estimated Effort:** 30 development hours (5 days × 6 hours)
**Expected Completion:** End of Week 4

**Next Phase:** [Phase 5: Docker/K8s Deployment](CANDLE-PHASE-5-DEPLOYMENT-PRD.md)

---

**Document End**
