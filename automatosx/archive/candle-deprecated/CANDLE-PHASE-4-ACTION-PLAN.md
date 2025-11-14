# Phase 4: Multi-Model Support - Detailed Action Plan
## Candle Embedding Migration - Week 4 Implementation Guide

**Version:** 1.0
**Date:** 2025-11-10
**Status:** Ready for Execution
**Timeline:** 5 days (30 development hours)

---

## Table of Contents

1. [Pre-Flight Checklist](#pre-flight-checklist)
2. [Day 1: Model Registry](#day-1-model-registry)
3. [Day 2: Multi-Model Manager](#day-2-multi-model-manager)
4. [Day 3: Quantization](#day-3-quantization)
5. [Day 4: Warm-Up + API Integration](#day-4-warm-up--api-integration)
6. [Day 5: Testing + Documentation](#day-5-testing--documentation)
7. [Phase 4 Summary](#phase-4-summary)
8. [Appendix](#appendix)

---

## Pre-Flight Checklist

### Before Starting Phase 4

```bash
# 1. Verify Phase 3 completion
cargo test --workspace --features candle
# Expected: 61 tests passing (15+21+25 Phase 1-3)

# 2. Check observability stack working
curl http://localhost:8080/metrics | grep candle
curl http://localhost:8080/health/ready

# 3. Verify disk space for model downloads
df -h
# Need: 2GB free for 4 models

# 4. Create feature branch
git checkout -b feature/candle-phase4-multi-model
git branch --set-upstream-to=origin/main

# 5. Baseline performance
cargo bench --bench candle_bench -- --save-baseline phase3
```

### Success Criteria

Phase 4 is complete when:
- ✅ 4 models registered and loadable
- ✅ Runtime model selection via API
- ✅ LRU cache with eviction working
- ✅ INT8 quantization: 75% memory savings
- ✅ Cold start <100ms with warm-up
- ✅ 20 new tests passing (total: 81)
- ✅ No performance regression from Phase 3

---

## Day 1: Model Registry
**Monday, 6 hours**

### Overview

**Goal:** Create model registry with metadata for 4 embedding models

**Deliverables:**
- `src/model_registry.rs` (~200 lines)
- `/api/v1/models` endpoint
- 4 registry tests

---

### Task 1.1: Implement ModelMetadata Struct
**Time:** 1.5 hours

#### Step 1: Create model_registry.rs

```bash
touch crates/akidb-embedding/src/model_registry.rs
```

#### Step 2: Implement Core Structures

```rust
// File: crates/akidb-embedding/src/model_registry.rs

use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// Quantization options
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Quantization {
    FP32,   // Full precision (32-bit float)
    FP16,   // Half precision (16-bit float)
    INT8,   // 8-bit integer quantization
    INT4,   // 4-bit integer quantization (future)
}

impl std::fmt::Display for Quantization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Quantization::FP32 => write!(f, "fp32"),
            Quantization::FP16 => write!(f, "fp16"),
            Quantization::INT8 => write!(f, "int8"),
            Quantization::INT4 => write!(f, "int4"),
        }
    }
}

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

    /// Model size on disk (MB) - FP32
    pub size_mb: u32,

    /// Supported languages (ISO 639-1 codes)
    pub languages: Vec<String>,

    /// Max sequence length (tokens)
    pub max_seq_length: u32,

    /// Recommended use cases
    pub use_cases: Vec<String>,

    /// Default quantization
    pub default_quantization: Quantization,

    /// Is this the default model?
    pub is_default: bool,

    /// Model description
    pub description: String,
}

impl ModelMetadata {
    /// Get estimated memory usage for this quantization
    pub fn estimated_memory_mb(&self, quantization: Quantization) -> u32 {
        match quantization {
            Quantization::FP32 => self.size_mb,
            Quantization::FP16 => self.size_mb / 2,
            Quantization::INT8 => self.size_mb / 4,
            Quantization::INT4 => self.size_mb / 8,
        }
    }

    /// Get recommended quantization based on model size
    pub fn recommended_quantization(&self) -> Quantization {
        if self.parameters_m > 100 {
            Quantization::INT8  // Large models benefit from quantization
        } else {
            Quantization::FP32  // Small models can stay full precision
        }
    }
}
```

#### Step 3: Verify Compilation

```bash
cargo check --features candle
```

**Expected Output:**
```
    Checking akidb-embedding v2.0.0
    Finished dev [unoptimized + debuginfo] target(s) in 3.1s
```

#### Checkpoint

- ✅ ModelMetadata struct defined
- ✅ Quantization enum created
- ✅ Compilation successful
- ✅ Commit: `git commit -am "Phase 4 Day 1: Add ModelMetadata structures"`

---

### Task 1.2: Implement ModelRegistry
**Time:** 2 hours

#### Step 1: Implement Registry with 4 Models

```rust
// File: crates/akidb-embedding/src/model_registry.rs (continued)

/// Model registry - catalog of supported models
pub struct ModelRegistry {
    models: HashMap<String, ModelMetadata>,
    default_model_id: String,
}

impl ModelRegistry {
    /// Create registry with built-in models
    pub fn new() -> Self {
        let mut models = HashMap::new();

        // Model 1: all-MiniLM-L6-v2 (default, lightweight, fast)
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
                    "Low memory".to_string(),
                ],
                default_quantization: Quantization::FP32,
                is_default: true,
                description: "Lightweight model for general-purpose semantic similarity. \
                             Best for short texts (<100 words) with fast inference requirements."
                    .to_string(),
            },
        );

        // Model 2: bert-base-uncased (higher quality, larger)
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
                    "Question answering".to_string(),
                ],
                default_quantization: Quantization::INT8,
                is_default: false,
                description: "Higher quality BERT model with 768-dimensional embeddings. \
                             Best for long documents and tasks requiring nuanced understanding."
                    .to_string(),
            },
        );

        // Model 3: e5-small-v2 (multilingual, balanced)
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
                    "en".to_string(),
                    "zh".to_string(),
                    "es".to_string(),
                    "fr".to_string(),
                    "de".to_string(),
                    "ja".to_string(),
                    "ko".to_string(),
                    "ar".to_string(),
                    "ru".to_string(),
                    "pt".to_string(),
                ],
                max_seq_length: 512,
                use_cases: vec![
                    "Multilingual".to_string(),
                    "Semantic search".to_string(),
                    "Retrieval".to_string(),
                    "Cross-lingual".to_string(),
                ],
                default_quantization: Quantization::FP32,
                is_default: false,
                description: "Multilingual embedding model supporting 100+ languages. \
                             Best for cross-lingual semantic search and retrieval tasks."
                    .to_string(),
            },
        );

        // Model 4: instructor-base (instruction-following)
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
                    "Custom prompts".to_string(),
                ],
                default_quantization: Quantization::INT8,
                is_default: false,
                description: "Instruction-aware embedding model that can be guided with task-specific \
                             prompts. Best for domain-specific or task-oriented embeddings."
                    .to_string(),
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
        self.models.get(&self.default_model_id)
            .expect("Default model must exist")
    }

    /// List all models
    pub fn list_models(&self) -> Vec<&ModelMetadata> {
        let mut models: Vec<&ModelMetadata> = self.models.values().collect();
        // Sort: default first, then by size
        models.sort_by(|a, b| {
            if a.is_default {
                std::cmp::Ordering::Less
            } else if b.is_default {
                std::cmp::Ordering::Greater
            } else {
                a.size_mb.cmp(&b.size_mb)
            }
        });
        models
    }

    /// Check if model is supported
    pub fn is_supported(&self, model_id: &str) -> bool {
        self.models.contains_key(model_id)
    }

    /// Get model count
    pub fn count(&self) -> usize {
        self.models.len()
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// Global registry singleton
lazy_static::lazy_static! {
    pub static ref MODEL_REGISTRY: ModelRegistry = ModelRegistry::new();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = ModelRegistry::new();
        assert_eq!(registry.count(), 4);
    }

    #[test]
    fn test_default_model() {
        let registry = ModelRegistry::new();
        let default = registry.get_default();

        assert_eq!(default.id, "all-MiniLM-L6-v2");
        assert!(default.is_default);
        assert_eq!(default.dimension, 384);
    }

    #[test]
    fn test_get_model() {
        let registry = ModelRegistry::new();
        let model = registry.get("e5-small-v2").unwrap();

        assert_eq!(model.dimension, 384);
        assert_eq!(model.parameters_m, 33);
        assert!(model.languages.len() >= 6);
    }

    #[test]
    fn test_unsupported_model() {
        let registry = ModelRegistry::new();
        assert!(registry.get("nonexistent").is_none());
        assert!(!registry.is_supported("nonexistent"));
    }

    #[test]
    fn test_list_models() {
        let registry = ModelRegistry::new();
        let models = registry.list_models();

        assert_eq!(models.len(), 4);
        // First should be default
        assert!(models[0].is_default);
    }

    #[test]
    fn test_recommended_quantization() {
        let registry = ModelRegistry::new();

        // Small model: FP32
        let minilm = registry.get("all-MiniLM-L6-v2").unwrap();
        assert_eq!(minilm.recommended_quantization(), Quantization::FP32);

        // Large model: INT8
        let bert = registry.get("bert-base-uncased").unwrap();
        assert_eq!(bert.recommended_quantization(), Quantization::INT8);
    }

    #[test]
    fn test_estimated_memory() {
        let registry = ModelRegistry::new();
        let bert = registry.get("bert-base-uncased").unwrap();

        assert_eq!(bert.estimated_memory_mb(Quantization::FP32), 440);
        assert_eq!(bert.estimated_memory_mb(Quantization::FP16), 220);
        assert_eq!(bert.estimated_memory_mb(Quantization::INT8), 110);
        assert_eq!(bert.estimated_memory_mb(Quantization::INT4), 55);
    }
}
```

#### Step 2: Update lib.rs

```rust
// File: crates/akidb-embedding/src/lib.rs

#[cfg(feature = "candle")]
pub mod model_registry;

#[cfg(feature = "candle")]
pub use model_registry::{ModelRegistry, ModelMetadata, Quantization, MODEL_REGISTRY};
```

#### Step 3: Test Registry

```bash
cargo test --features candle model_registry::tests
```

**Expected Output:**
```
running 7 tests
test model_registry::tests::test_registry_creation ... ok
test model_registry::tests::test_default_model ... ok
test model_registry::tests::test_get_model ... ok
test model_registry::tests::test_unsupported_model ... ok
test model_registry::tests::test_list_models ... ok
test model_registry::tests::test_recommended_quantization ... ok
test model_registry::tests::test_estimated_memory ... ok

test result: ok. 7 tests passed
```

#### Checkpoint

- ✅ ModelRegistry implemented (~200 lines)
- ✅ 4 models registered
- ✅ 7 tests passing
- ✅ Commit: `git commit -am "Phase 4 Day 1: Implement ModelRegistry with 4 models"`

---

### Task 1.3: Add /api/v1/models Endpoint
**Time:** 1.5 hours

#### Step 1: Create Models Handler

```bash
touch crates/akidb-rest/src/handlers/models.rs
```

```rust
// File: crates/akidb-rest/src/handlers/models.rs

use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::{Serialize, Deserialize};
use akidb_embedding::{MODEL_REGISTRY, ModelMetadata};

#[derive(Serialize, Deserialize)]
pub struct ModelsResponse {
    pub models: Vec<ModelInfo>,
    pub default_model: String,
}

#[derive(Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub dimension: u32,
    pub parameters_m: u32,
    pub size_mb: u32,
    pub languages: Vec<String>,
    pub max_seq_length: u32,
    pub use_cases: Vec<String>,
    pub default_quantization: String,
    pub is_default: bool,
    pub description: String,
}

impl From<&ModelMetadata> for ModelInfo {
    fn from(meta: &ModelMetadata) -> Self {
        Self {
            id: meta.id.clone(),
            name: meta.name.clone(),
            dimension: meta.dimension,
            parameters_m: meta.parameters_m,
            size_mb: meta.size_mb,
            languages: meta.languages.clone(),
            max_seq_length: meta.max_seq_length,
            use_cases: meta.use_cases.clone(),
            default_quantization: meta.default_quantization.to_string(),
            is_default: meta.is_default,
            description: meta.description.clone(),
        }
    }
}

/// List available models
pub async fn list_models() -> impl IntoResponse {
    let models: Vec<ModelInfo> = MODEL_REGISTRY
        .list_models()
        .into_iter()
        .map(ModelInfo::from)
        .collect();

    let default_model = MODEL_REGISTRY.get_default().id.clone();

    (
        StatusCode::OK,
        Json(ModelsResponse {
            models,
            default_model,
        })
    )
}

/// Get specific model metadata
pub async fn get_model(
    axum::extract::Path(model_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    match MODEL_REGISTRY.get(&model_id) {
        Some(metadata) => {
            (StatusCode::OK, Json(ModelInfo::from(metadata)))
        }
        None => {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": format!("Model not found: {}", model_id)
                }))
            )
        }
    }
}
```

#### Step 2: Add Routes

```rust
// File: crates/akidb-rest/src/main.rs

mod handlers;

use axum::{Router, routing::{get, post}};

async fn create_app() -> Router {
    Router::new()
        .route("/health/live", get(handlers::health::liveness))
        .route("/health/ready", get(handlers::health::readiness))
        .route("/metrics", get(handlers::metrics::metrics))
        .route("/api/v1/models", get(handlers::models::list_models))          // NEW
        .route("/api/v1/models/:model_id", get(handlers::models::get_model))  // NEW
        .route("/api/v1/embed", post(handlers::embed::embed))
}
```

#### Step 3: Test Endpoints

```bash
# Start server
cargo run -p akidb-rest --features candle &
sleep 5

# Test list models
curl http://localhost:8080/api/v1/models | jq

# Expected output:
# {
#   "models": [
#     {
#       "id": "all-MiniLM-L6-v2",
#       "name": "MiniLM L6 v2",
#       "dimension": 384,
#       ...
#       "is_default": true
#     },
#     ...
#   ],
#   "default_model": "all-MiniLM-L6-v2"
# }

# Test get specific model
curl http://localhost:8080/api/v1/models/e5-small-v2 | jq

# Test nonexistent model (should return 404)
curl -w "\nStatus: %{http_code}\n" http://localhost:8080/api/v1/models/nonexistent
```

#### Checkpoint

- ✅ /api/v1/models endpoint implemented
- ✅ /api/v1/models/:model_id endpoint implemented
- ✅ API responses validated
- ✅ Commit: `git commit -am "Phase 4 Day 1: Add model listing API endpoints"`

---

### Day 1 Checkpoint

**Accomplishments:**
- ✅ ModelMetadata struct with 4 models
- ✅ ModelRegistry with singleton
- ✅ 7 registry tests passing
- ✅ /api/v1/models API endpoints

**Verification:**
```bash
# Run tests
cargo test --features candle model_registry

# Test API
curl http://localhost:8080/api/v1/models

# Check git log
git log --oneline -3
```

**Deliverables:**
- `src/model_registry.rs` (~200 lines) ✅
- API endpoints ✅
- 7 tests passing ✅

**Time Spent:** 6 hours (on budget)

**Next:** Day 2 - Multi-model manager with LRU cache

---

## Day 2: Multi-Model Manager
**Tuesday, 6 hours**

**Summary:** Implement multi-model manager with LRU cache, lazy loading, and memory management. Due to length constraints, this will follow the same detailed pattern as Day 1, implementing:

1. **Task 2.1:** MultiModelManager struct (~2.5 hours)
2. **Task 2.2:** LRU cache with eviction (~2 hours)
3. **Task 2.3:** Memory tracking (~1 hour)
4. **Task 2.4:** Multi-model tests (~30 min)

**Key Deliverables:**
- `src/multi_model_manager.rs` (~300 lines)
- LRU cache (using `lru` crate)
- 8 multi-model tests

---

## Day 3: Quantization
**Wednesday, 6 hours**

**Summary:** Add INT8 quantization support for memory-efficient model loading.

**Key Tasks:**
1. Add Quantization enum and ModelConfig
2. Implement INT8 loading in CandleEmbeddingProvider
3. Auto-quantization selection logic
4. Quality validation tests

**Key Deliverables:**
- Quantization support (~150 lines)
- 5 quantization tests
- Quality benchmarks (>98% similarity)

---

## Day 4: Warm-Up + API Integration
**Thursday, 6 hours**

**Summary:** Implement model warm-up to eliminate cold starts, and integrate multi-model support into REST API.

**Key Tasks:**
1. Implement warm-up logic (pre-load + dummy inference)
2. Update embed handler to accept model parameter
3. Add backward compatibility (default model)
4. Integration tests

**Key Deliverables:**
- Warm-up implementation
- Updated API with model parameter
- 3 warm-up tests
- Backward compatibility maintained

---

## Day 5: Testing + Documentation
**Friday, 6 hours**

**Summary:** Complete test suite, run performance benchmarks, and document multi-model capabilities.

**Key Tasks:**
1. Complete all 20 tests
2. Run performance benchmarks (verify no regression)
3. Update API documentation
4. Write Phase 4 completion report

**Key Deliverables:**
- 20 tests passing (total: 81)
- Performance benchmarks
- API documentation
- Completion report

---

## Phase 4 Summary

### Accomplishments

**Week 4 Deliverables:**
- ✅ Model registry with 4 models
- ✅ Multi-model manager with LRU cache
- ✅ INT8 quantization (75% memory savings)
- ✅ Model warm-up (<100ms cold start)
- ✅ Runtime model selection API
- ✅ 20 new tests passing
- ✅ No performance regression

**Code Statistics:**
- Lines added: ~1,410
- Tests: 81 total (61 Phase 1-3, 20 Phase 4)
- Models supported: 4
- Memory savings: 75% (INT8 vs FP32)

**Performance:**
- Throughput: 200+ QPS (maintained)
- Latency P95: <35ms (maintained)
- Model load (cache hit): <50ms
- Model load (cache miss): <2s
- Cold start with warm-up: <100ms

### Next Steps

**Phase 5 Preview:**
- Docker/Kubernetes deployment
- Helm charts
- Blue-green deployment
- Production rollout strategy

---

## Appendix

### Quick Reference Commands

```bash
# Run all tests
cargo test --workspace --features candle

# Run specific test suite
cargo test --features candle model_registry
cargo test --features candle multi_model
cargo test --features candle quantization

# List available models
curl http://localhost:8080/api/v1/models | jq

# Embed with specific model
curl -X POST http://localhost:8080/api/v1/embed \
  -H "Content-Type: application/json" \
  -d '{"texts": ["Hello world"], "model": "e5-small-v2"}' | jq

# Performance benchmark
cargo bench --bench candle_bench -- --baseline phase3
```

### Troubleshooting

**Problem:** Model download fails
- Check: Internet connection and HF Hub access
- Fix: Set HF_HUB_CACHE environment variable
- Alternative: Download models manually to cache directory

**Problem:** LRU eviction too aggressive
- Check: Cache size configuration
- Fix: Increase max_models or max_memory_mb in config

**Problem:** Quantization quality loss >2%
- Check: Model and dataset
- Fix: Use FP32 for quality-critical applications

**Problem:** Memory leak with multiple models
- Check: Cache stats endpoint
- Fix: Ensure models are properly dropped on eviction

---

**Phase 4 Action Plan Complete! 🎉**

**Status:** Ready for Phase 5 (Docker/K8s Deployment)

**Document End**
