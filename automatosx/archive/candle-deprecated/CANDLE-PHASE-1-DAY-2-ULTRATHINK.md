# Candle Phase 1 - Day 2 Ultrathink: Model Loading from Hugging Face Hub

**Date**: November 10, 2025  
**Phase**: Candle Phase 1 - Foundation  
**Day**: 2 of 5  
**Focus**: Model loading, device selection, HF Hub integration  
**Estimated Time**: 4-5 hours  
**Prerequisites**: Day 1 complete (skeleton code ready)

---

## Executive Summary

**Goal**: Implement complete model loading pipeline from Hugging Face Hub, including device selection (Metal/CUDA/CPU), model download, weight loading, and tokenizer initialization.

**Outcome**: Fully functional `CandleEmbeddingProvider::new()` constructor that can:
- ✅ Automatically select best available device (Metal > CUDA > CPU)
- ✅ Download BERT models from Hugging Face Hub (with caching)
- ✅ Load model weights into GPU/CPU memory
- ✅ Initialize tokenizer for text processing
- ✅ Return ready-to-use provider instance

**Success Criteria**:
```rust
let provider = CandleEmbeddingProvider::new(
    "sentence-transformers/all-MiniLM-L6-v2"
).await?;
// ✅ Model loaded and ready for inference
println!("Device: {:?}", provider.device); // Metal, Cuda, or Cpu
```

---

## Architecture Overview

### Component Flow

```
CandleEmbeddingProvider::new()
    │
    ├─> 1. select_device()
    │       ├─> Try Metal (macOS)
    │       ├─> Try CUDA (Linux)
    │       └─> Fallback to CPU
    │
    ├─> 2. Download from HF Hub
    │       ├─> hf_hub::api::sync::Api::new()
    │       ├─> api.model(model_name)
    │       ├─> repo.get("config.json")
    │       ├─> repo.get("model.safetensors") or "pytorch_model.bin"
    │       └─> repo.get("tokenizer.json")
    │
    ├─> 3. Load BERT Model
    │       ├─> Parse config.json
    │       ├─> Create Config struct
    │       ├─> VarBuilder::from_safetensors() or from_pytorch()
    │       └─> BertModel::load(vb, config)
    │
    ├─> 4. Load Tokenizer
    │       └─> Tokenizer::from_file(tokenizer_path)
    │
    └─> 5. Return Provider
            └─> Arc::new() for model and tokenizer
```

### Key Dependencies

- **hf-hub**: Model download and caching
- **candle-core**: Device management, tensor operations
- **candle-nn**: VarBuilder for weight loading
- **candle-transformers**: BertModel, Config
- **tokenizers**: Tokenizer for text processing

---

## Implementation Plan (4-5 hours)

### Task 2.1: Implement `select_device()` (30 minutes)

**Goal**: Auto-detect best available device with Metal > CUDA > CPU priority.

**Steps**:

1. **Try Metal GPU (macOS)**:
   ```rust
   if cfg!(target_os = "macos") {
       match Device::new_metal(0) {
           Ok(device) => return Ok(device),
           Err(e) => eprintln!("Metal unavailable: {}", e),
       }
   }
   ```

2. **Try CUDA GPU (Linux/Windows)**:
   ```rust
   match Device::new_cuda(0) {
       Ok(device) => return Ok(device),
       Err(e) => eprintln!("CUDA unavailable: {}", e),
   }
   ```

3. **Fallback to CPU**:
   ```rust
   Ok(Device::Cpu)
   ```

**Implementation**:

```rust
fn select_device() -> EmbeddingResult<Device> {
    // Try Metal on macOS
    #[cfg(target_os = "macos")]
    {
        if let Ok(device) = Device::new_metal(0) {
            eprintln!("✅ Using Metal GPU (macOS)");
            return Ok(device);
        }
    }

    // Try CUDA on Linux/Windows
    #[cfg(not(target_os = "macos"))]
    {
        if let Ok(device) = Device::new_cuda(0) {
            eprintln!("✅ Using CUDA GPU");
            return Ok(device);
        }
    }

    // Fallback to CPU
    eprintln!("⚠️  Using CPU (GPU unavailable)");
    Ok(Device::Cpu)
}
```

**Verification**:
```bash
# Test on macOS - should select Metal
cargo test --features candle -p akidb-embedding select_device -- --nocapture

# Expected output:
# ✅ Using Metal GPU (macOS)
```

**Time**: 30 minutes

---

### Task 2.2: Download Model Files from HF Hub (1 hour)

**Goal**: Download 3 essential files from Hugging Face Hub with caching.

**Files Needed**:
1. `config.json` - Model architecture configuration
2. `model.safetensors` or `pytorch_model.bin` - Model weights
3. `tokenizer.json` - Tokenizer vocabulary and settings

**Steps**:

1. **Initialize HF Hub API**:
   ```rust
   use hf_hub::{api::sync::Api, Repo, RepoType};
   
   let api = Api::new().map_err(|e| {
       EmbeddingError::ModelLoadError(format!("HF Hub API error: {}", e))
   })?;
   ```

2. **Get Model Repository**:
   ```rust
   let repo = api.repo(Repo::new(
       model_name.to_string(),
       RepoType::Model
   ));
   ```

3. **Download config.json**:
   ```rust
   let config_path = repo.get("config.json").map_err(|e| {
       EmbeddingError::ModelLoadError(format!("Failed to download config.json: {}", e))
   })?;
   ```

4. **Download model weights (try safetensors first)**:
   ```rust
   let weights_path = repo.get("model.safetensors")
       .or_else(|_| repo.get("pytorch_model.bin"))
       .map_err(|e| {
           EmbeddingError::ModelLoadError(format!("Failed to download weights: {}", e))
       })?;
   ```

5. **Download tokenizer.json**:
   ```rust
   let tokenizer_path = repo.get("tokenizer.json").map_err(|e| {
       EmbeddingError::ModelLoadError(format!("Failed to download tokenizer: {}", e))
   })?;
   ```

**Implementation**:

```rust
// Inside CandleEmbeddingProvider::new()

// Initialize HF Hub API
let api = Api::new().map_err(|e| {
    EmbeddingError::ModelLoadError(format!("HF Hub API init failed: {}", e))
})?;

// Get model repository
let repo = api.repo(Repo::new(
    model_name.to_string(),
    RepoType::Model
));

// Download required files
eprintln!("📥 Downloading model from HF Hub: {}", model_name);

let config_path = repo.get("config.json").map_err(|e| {
    EmbeddingError::ModelLoadError(format!("config.json download failed: {}", e))
})?;

let weights_path = repo.get("model.safetensors")
    .or_else(|_| {
        eprintln!("⚠️  safetensors not found, trying pytorch_model.bin");
        repo.get("pytorch_model.bin")
    })
    .map_err(|e| {
        EmbeddingError::ModelLoadError(format!("Model weights download failed: {}", e))
    })?;

let tokenizer_path = repo.get("tokenizer.json").map_err(|e| {
    EmbeddingError::ModelLoadError(format!("tokenizer.json download failed: {}", e))
})?;

eprintln!("✅ Files downloaded (cached at ~/.cache/huggingface)");
```

**Caching Behavior**:
- First run: Downloads files (~22MB for MiniLM)
- Subsequent runs: Uses cached files (< 1 second)

**Verification**:
```bash
# First run (downloads files)
cargo run --features candle --example load_model

# Second run (uses cache)
cargo run --features candle --example load_model

# Check cache
ls -lh ~/.cache/huggingface/hub/models--sentence-transformers--all-MiniLM-L6-v2/
```

**Time**: 1 hour

---

### Task 2.3: Load BERT Model (1.5 hours)

**Goal**: Load BERT model weights into GPU/CPU memory using Candle.

**Steps**:

1. **Parse config.json**:
   ```rust
   use std::fs;
   use serde_json::Value;
   
   let config_json = fs::read_to_string(&config_path).map_err(|e| {
       EmbeddingError::ModelLoadError(format!("Failed to read config.json: {}", e))
   })?;
   
   let config_value: Value = serde_json::from_str(&config_json).map_err(|e| {
       EmbeddingError::ModelLoadError(format!("Invalid config.json: {}", e))
   })?;
   ```

2. **Create BertModel Config**:
   ```rust
   use candle_transformers::models::bert::Config;
   
   let config = Config {
       vocab_size: config_value["vocab_size"].as_u64().unwrap() as usize,
       hidden_size: config_value["hidden_size"].as_u64().unwrap() as usize,
       num_hidden_layers: config_value["num_hidden_layers"].as_u64().unwrap() as usize,
       num_attention_heads: config_value["num_attention_heads"].as_u64().unwrap() as usize,
       intermediate_size: config_value["intermediate_size"].as_u64().unwrap() as usize,
       hidden_act: serde_json::from_value(config_value["hidden_act"].clone()).unwrap(),
       max_position_embeddings: config_value["max_position_embeddings"].as_u64().unwrap() as usize,
       type_vocab_size: config_value.get("type_vocab_size")
           .and_then(|v| v.as_u64())
           .unwrap_or(2) as usize,
       layer_norm_eps: config_value.get("layer_norm_eps")
           .and_then(|v| v.as_f64())
           .unwrap_or(1e-12),
   };
   
   // Extract embedding dimension
   let dimension = config.hidden_size as u32;
   ```

3. **Create VarBuilder from weights file**:
   ```rust
   use candle_nn::VarBuilder;
   
   let vb = if weights_path.extension().and_then(|s| s.to_str()) == Some("safetensors") {
       // Load from safetensors (faster, safer)
       unsafe {
           VarBuilder::from_mmaped_safetensors(&[weights_path], device.clone()).map_err(|e| {
               EmbeddingError::ModelLoadError(format!("SafeTensors load failed: {}", e))
           })?
       }
   } else {
       // Load from pytorch_model.bin (slower)
       VarBuilder::from_pth(&weights_path, device.clone()).map_err(|e| {
           EmbeddingError::ModelLoadError(format!("PyTorch weights load failed: {}", e))
       })?
   };
   ```

4. **Load BertModel**:
   ```rust
   use candle_transformers::models::bert::BertModel;
   
   let model = BertModel::load(vb, &config).map_err(|e| {
       EmbeddingError::ModelLoadError(format!("BertModel::load failed: {}", e))
   })?;
   
   eprintln!("✅ BERT model loaded ({} dimensions)", dimension);
   ```

5. **Wrap in Arc for thread safety**:
   ```rust
   let model = Arc::new(model);
   ```

**Implementation**:

```rust
// Parse config
let config_json = std::fs::read_to_string(&config_path)
    .map_err(|e| EmbeddingError::ModelLoadError(format!("Read config failed: {}", e)))?;

let config_value: serde_json::Value = serde_json::from_str(&config_json)
    .map_err(|e| EmbeddingError::ModelLoadError(format!("Parse config failed: {}", e)))?;

// Build Config struct
let config = Config {
    vocab_size: config_value["vocab_size"].as_u64().unwrap() as usize,
    hidden_size: config_value["hidden_size"].as_u64().unwrap() as usize,
    num_hidden_layers: config_value["num_hidden_layers"].as_u64().unwrap() as usize,
    num_attention_heads: config_value["num_attention_heads"].as_u64().unwrap() as usize,
    intermediate_size: config_value["intermediate_size"].as_u64().unwrap() as usize,
    hidden_act: serde_json::from_value(config_value["hidden_act"].clone())
        .unwrap_or(candle_transformers::models::bert::HiddenAct::Gelu),
    max_position_embeddings: config_value["max_position_embeddings"].as_u64().unwrap() as usize,
    type_vocab_size: config_value.get("type_vocab_size")
        .and_then(|v| v.as_u64())
        .unwrap_or(2) as usize,
    layer_norm_eps: config_value.get("layer_norm_eps")
        .and_then(|v| v.as_f64())
        .unwrap_or(1e-12),
};

let dimension = config.hidden_size as u32;

// Load weights
eprintln!("📦 Loading model weights...");
let vb = if weights_path.extension().and_then(|s| s.to_str()) == Some("safetensors") {
    unsafe {
        VarBuilder::from_mmaped_safetensors(&[weights_path], device.clone())
            .map_err(|e| EmbeddingError::ModelLoadError(format!("SafeTensors: {}", e)))?
    }
} else {
    VarBuilder::from_pth(&weights_path, device.clone())
        .map_err(|e| EmbeddingError::ModelLoadError(format!("PyTorch: {}", e)))?
};

// Load BERT model
let model = BertModel::load(vb, &config)
    .map_err(|e| EmbeddingError::ModelLoadError(format!("BertModel::load: {}", e)))?;

let model = Arc::new(model);
eprintln!("✅ Model loaded: {} dimensions", dimension);
```

**Verification**:
```rust
// In test
let provider = CandleEmbeddingProvider::new("sentence-transformers/all-MiniLM-L6-v2").await?;
assert!(provider.model.is_some());
assert_eq!(provider.dimension, 384); // MiniLM dimension
```

**Time**: 1.5 hours

---

### Task 2.4: Load Tokenizer (30 minutes)

**Goal**: Initialize Hugging Face tokenizer for text processing.

**Steps**:

1. **Load tokenizer from file**:
   ```rust
   use tokenizers::Tokenizer;
   
   let tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(|e| {
       EmbeddingError::ModelLoadError(format!("Tokenizer load failed: {}", e))
   })?;
   
   eprintln!("✅ Tokenizer loaded");
   ```

2. **Wrap in Arc for thread safety**:
   ```rust
   let tokenizer = Arc::new(tokenizer);
   ```

3. **Test tokenization**:
   ```rust
   let encoding = tokenizer.encode("Hello world", true)
       .map_err(|e| EmbeddingError::TokenizationError(e.to_string()))?;
   
   eprintln!("Test tokenization: {} tokens", encoding.len());
   ```

**Implementation**:

```rust
// Load tokenizer
eprintln!("📝 Loading tokenizer...");
let tokenizer = Tokenizer::from_file(&tokenizer_path)
    .map_err(|e| EmbeddingError::ModelLoadError(format!("Tokenizer: {}", e)))?;

let tokenizer = Arc::new(tokenizer);
eprintln!("✅ Tokenizer loaded");

// Quick test
if let Ok(encoding) = tokenizer.encode("test", true) {
    eprintln!("✅ Tokenizer test: {} tokens", encoding.len());
}
```

**Verification**:
```rust
let provider = CandleEmbeddingProvider::new("sentence-transformers/all-MiniLM-L6-v2").await?;
let encoding = provider.tokenizer.encode("Hello world", true)?;
assert!(encoding.len() > 0);
```

**Time**: 30 minutes

---

### Task 2.5: Complete Constructor (30 minutes)

**Goal**: Assemble all components and return provider instance.

**Implementation**:

```rust
pub async fn new(model_name: &str) -> EmbeddingResult<Self> {
    // 1. Select device
    let device = Self::select_device()?;
    
    // 2. Download files from HF Hub
    let api = Api::new()
        .map_err(|e| EmbeddingError::ModelLoadError(format!("HF Hub API: {}", e)))?;
    
    let repo = api.repo(Repo::new(model_name.to_string(), RepoType::Model));
    
    eprintln!("📥 Downloading {} from HF Hub...", model_name);
    
    let config_path = repo.get("config.json")
        .map_err(|e| EmbeddingError::ModelLoadError(format!("config.json: {}", e)))?;
    
    let weights_path = repo.get("model.safetensors")
        .or_else(|_| repo.get("pytorch_model.bin"))
        .map_err(|e| EmbeddingError::ModelLoadError(format!("weights: {}", e)))?;
    
    let tokenizer_path = repo.get("tokenizer.json")
        .map_err(|e| EmbeddingError::ModelLoadError(format!("tokenizer.json: {}", e)))?;
    
    // 3. Parse config
    let config_json = std::fs::read_to_string(&config_path)
        .map_err(|e| EmbeddingError::ModelLoadError(format!("read config: {}", e)))?;
    
    let config_value: serde_json::Value = serde_json::from_str(&config_json)
        .map_err(|e| EmbeddingError::ModelLoadError(format!("parse config: {}", e)))?;
    
    let config = Config {
        vocab_size: config_value["vocab_size"].as_u64().unwrap() as usize,
        hidden_size: config_value["hidden_size"].as_u64().unwrap() as usize,
        num_hidden_layers: config_value["num_hidden_layers"].as_u64().unwrap() as usize,
        num_attention_heads: config_value["num_attention_heads"].as_u64().unwrap() as usize,
        intermediate_size: config_value["intermediate_size"].as_u64().unwrap() as usize,
        hidden_act: serde_json::from_value(config_value["hidden_act"].clone())
            .unwrap_or(candle_transformers::models::bert::HiddenAct::Gelu),
        max_position_embeddings: config_value["max_position_embeddings"].as_u64().unwrap() as usize,
        type_vocab_size: config_value.get("type_vocab_size")
            .and_then(|v| v.as_u64())
            .unwrap_or(2) as usize,
        layer_norm_eps: config_value.get("layer_norm_eps")
            .and_then(|v| v.as_f64())
            .unwrap_or(1e-12),
    };
    
    let dimension = config.hidden_size as u32;
    
    // 4. Load model
    eprintln!("📦 Loading model weights...");
    let vb = if weights_path.extension().and_then(|s| s.to_str()) == Some("safetensors") {
        unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path], device.clone())
                .map_err(|e| EmbeddingError::ModelLoadError(format!("safetensors: {}", e)))?
        }
    } else {
        VarBuilder::from_pth(&weights_path, device.clone())
            .map_err(|e| EmbeddingError::ModelLoadError(format!("pytorch: {}", e)))?
    };
    
    let model = BertModel::load(vb, &config)
        .map_err(|e| EmbeddingError::ModelLoadError(format!("BertModel::load: {}", e)))?;
    
    let model = Arc::new(model);
    
    // 5. Load tokenizer
    eprintln!("📝 Loading tokenizer...");
    let tokenizer = Tokenizer::from_file(&tokenizer_path)
        .map_err(|e| EmbeddingError::ModelLoadError(format!("tokenizer: {}", e)))?;
    
    let tokenizer = Arc::new(tokenizer);
    
    eprintln!("✅ CandleEmbeddingProvider initialized");
    eprintln!("   Model: {}", model_name);
    eprintln!("   Device: {:?}", device);
    eprintln!("   Dimension: {}", dimension);
    
    Ok(Self {
        model,
        tokenizer,
        device,
        model_name: model_name.to_string(),
        dimension,
    })
}
```

**Time**: 30 minutes

---

### Task 2.6: Integration Testing (1 hour)

**Goal**: Write integration test to verify model loading works end-to-end.

**Test File**: `tests/candle_tests.rs`

**Implementation**:

```rust
#[cfg(feature = "candle")]
mod candle_tests {
    use akidb_embedding::{CandleEmbeddingProvider, EmbeddingProvider};

    #[tokio::test]
    #[ignore] // Expensive test (downloads model)
    async fn test_load_minilm_model() {
        let provider = CandleEmbeddingProvider::new(
            "sentence-transformers/all-MiniLM-L6-v2"
        ).await.expect("Failed to load model");

        // Verify model info
        let info = provider.model_info().await.expect("Failed to get model info");
        assert_eq!(info.model, "sentence-transformers/all-MiniLM-L6-v2");
        assert_eq!(info.dimension, 384);
    }

    #[tokio::test]
    #[ignore]
    async fn test_device_selection() {
        let provider = CandleEmbeddingProvider::new(
            "sentence-transformers/all-MiniLM-L6-v2"
        ).await.expect("Failed to load model");

        // On macOS, should use Metal
        #[cfg(target_os = "macos")]
        {
            // Device should be Metal (can't directly assert due to private field)
            println!("Device: Metal (expected on macOS)");
        }

        // On Linux, might be CUDA or CPU
        #[cfg(target_os = "linux")]
        {
            println!("Device: CUDA or CPU (depends on GPU availability)");
        }
    }

    #[tokio::test]
    async fn test_device_selection_cpu_fallback() {
        // This should always work (CPU fallback)
        let device = CandleEmbeddingProvider::select_device().expect("Device selection failed");
        println!("Selected device: {:?}", device);
    }
}
```

**Run Tests**:
```bash
# Run expensive tests (downloads model)
cargo test --features candle -p akidb-embedding -- --ignored --nocapture

# Expected output:
# test candle_tests::test_load_minilm_model ... ok
# test candle_tests::test_device_selection ... ok
```

**Time**: 1 hour

---

## Verification Checklist

After implementation, verify:

- [ ] Device selection works (Metal/CUDA/CPU)
- [ ] Model downloads from HF Hub (first run)
- [ ] Model loads from cache (subsequent runs)
- [ ] SafeTensors format supported
- [ ] PyTorch format supported (fallback)
- [ ] Tokenizer loads successfully
- [ ] Constructor returns valid provider
- [ ] Model info accessible
- [ ] Integration tests pass

---

## Common Issues and Solutions

### Issue 1: Metal GPU Not Available

**Symptom**:
```
⚠️  Using CPU (GPU unavailable)
```

**Cause**: Metal not available (not macOS, or GPU disabled)

**Solution**: Expected behavior - CPU fallback works fine

---

### Issue 2: Model Download Timeout

**Symptom**:
```
Error: HF Hub timeout
```

**Cause**: Network issues, large model

**Solution**:
- Retry download
- Check network connection
- Use smaller model for testing

---

### Issue 3: SafeTensors Not Found

**Symptom**:
```
⚠️  safetensors not found, trying pytorch_model.bin
```

**Cause**: Model uses PyTorch format (older models)

**Solution**: Code automatically falls back to `pytorch_model.bin`

---

### Issue 4: Config Parsing Error

**Symptom**:
```
Error: Invalid config.json
```

**Cause**: Unsupported model architecture

**Solution**: Use sentence-transformers BERT models only (for now)

---

## Performance Expectations

| Operation | First Run | Cached | Notes |
|-----------|-----------|--------|-------|
| Download | 5-30s | <1s | Depends on network |
| Load weights | 1-2s | 1-2s | Depends on device |
| Initialize | 2-5s | 2-5s | Total constructor time |

**Supported Models** (Day 2):
- ✅ `sentence-transformers/all-MiniLM-L6-v2` (384-dim, 22M params)
- ✅ `sentence-transformers/all-distilroberta-v1` (768-dim, 82M params)
- ✅ `BAAI/bge-small-en-v1.5` (384-dim, 33M params)

---

## Success Criteria

Day 2 is complete when:

1. ✅ `CandleEmbeddingProvider::new()` fully implemented (no `todo!()`)
2. ✅ Device selection works (Metal/CUDA/CPU)
3. ✅ Model downloads from HF Hub
4. ✅ Model loads into memory
5. ✅ Tokenizer initializes
6. ✅ Integration test passes
7. ✅ Code compiles without warnings (except unused methods for Day 3)
8. ✅ Git commit with descriptive message

---

## Timeline

| Task | Duration | Cumulative |
|------|----------|------------|
| 2.1: Device selection | 30 min | 30 min |
| 2.2: HF Hub download | 1 hour | 1.5 hours |
| 2.3: Load BERT model | 1.5 hours | 3 hours |
| 2.4: Load tokenizer | 30 min | 3.5 hours |
| 2.5: Complete constructor | 30 min | 4 hours |
| 2.6: Integration testing | 1 hour | 5 hours |
| **Total** | **5 hours** | - |

---

## Deliverables

1. **Code**:
   - `src/candle.rs` - Complete `new()` and `select_device()`
   - `tests/candle_tests.rs` - 3 integration tests

2. **Documentation**:
   - Updated inline docs with examples
   - Day 2 completion report

3. **Git**:
   - Feature branch commit
   - Descriptive commit message

---

## Next Steps (Day 3)

After Day 2 completion:

**Day 3 Focus**: Inference pipeline (tokenization + forward pass + mean pooling)

**Key Tasks**:
1. Implement `embed_batch_internal()`
2. Tokenization with padding/truncation
3. BERT forward pass
4. Mean pooling over token embeddings
5. L2 normalization
6. Benchmark inference speed

**Target**: <20ms single text, <40ms batch of 8 (Metal GPU)

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Model download fails | Medium | High | Retry logic + offline testing |
| Device initialization fails | Low | Medium | CPU fallback guaranteed |
| Unsupported model architecture | Low | Medium | Validate with sentence-transformers only |
| Memory constraints | Low | High | Start with small models (MiniLM) |

---

**Prepared By**: Claude Code  
**Date**: November 10, 2025  
**Status**: Ready to Execute

---
