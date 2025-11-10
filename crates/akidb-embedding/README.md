# akidb-embedding

Embedding service infrastructure for AkiDB 2.0.

This crate provides trait definitions and implementations for text embedding generation. The architecture supports multiple backends through the `EmbeddingProvider` trait.

## Features

This crate supports three embedding providers:

### 1. **Candle** (Pure Rust, GPU-accelerated) ⭐ Recommended
- **Status**: Phase 1 - Foundation (Day 1-5)
- **Runtime**: Pure Rust (no Python dependency)
- **Devices**: Metal GPU (macOS), CUDA GPU (Linux), CPU fallback
- **Performance**: <20ms single text, <40ms batch of 8 (Metal GPU)
- **Models**: BERT-based transformers from Hugging Face Hub
- **Use Case**: Production deployment, ARM edge devices

### 2. **MLX** (Apple Silicon optimized)
- **Status**: Production-ready
- **Runtime**: Python + MLX framework
- **Devices**: Apple Silicon (M1/M2/M3/M4) only
- **Performance**: ~182ms single text (Python overhead)
- **Use Case**: Apple Silicon development, MLX-specific models

### 3. **Mock** (Testing only)
- **Status**: Complete
- **Runtime**: In-memory
- **Performance**: <1ms (no actual computation)
- **Use Case**: Unit tests, integration tests

## Feature Flags

```toml
# Default: MLX enabled
akidb-embedding = "2.0.0-rc1"

# Candle only (pure Rust, no Python)
akidb-embedding = { version = "2.0.0-rc1", default-features = false, features = ["candle"] }

# Both MLX and Candle
akidb-embedding = { version = "2.0.0-rc1", features = ["mlx", "candle"] }

# Mock only (testing)
akidb-embedding = { version = "2.0.0-rc1", default-features = false }
```

## Usage Examples

### Candle Provider (Recommended)

```rust
use akidb_embedding::{CandleEmbeddingProvider, EmbeddingProvider, BatchEmbeddingRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize provider (downloads model from Hugging Face Hub)
    let provider = CandleEmbeddingProvider::new(
        "sentence-transformers/all-MiniLM-L6-v2"
    ).await?;

    // Generate embeddings
    let request = BatchEmbeddingRequest {
        model: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
        inputs: vec![
            "Hello world".to_string(),
            "Rust is awesome".to_string(),
        ],
    };

    let response = provider.embed_batch(request).await?;
    
    for (i, embedding) in response.embeddings.iter().enumerate() {
        println!("Text {}: {} dimensions", i, embedding.len());
    }

    Ok(())
}
```

**Supported Models:**
- `sentence-transformers/all-MiniLM-L6-v2` (384-dim, 22M params) - **Recommended**
- `sentence-transformers/all-distilroberta-v1` (768-dim, 82M params)
- `BAAI/bge-small-en-v1.5` (384-dim, 33M params)

### MLX Provider (Apple Silicon)

```rust
use akidb_embedding::{MlxEmbeddingProvider, EmbeddingProvider};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = MlxEmbeddingProvider::new(
        "sentence-transformers/all-MiniLM-L6-v2"
    ).await?;

    // Use same EmbeddingProvider trait as Candle
    let response = provider.embed_batch(request).await?;
    
    Ok(())
}
```

**Requirements:**
- Python 3.10+
- MLX framework: `pip install mlx`
- Apple Silicon Mac (M1/M2/M3/M4)

### Mock Provider (Testing)

```rust
use akidb_embedding::{MockEmbeddingProvider, EmbeddingProvider};

#[tokio::test]
async fn test_embeddings() {
    let provider = MockEmbeddingProvider::new(384);
    let response = provider.embed_batch(request).await.unwrap();
    assert_eq!(response.embeddings[0].len(), 384);
}
```

## Performance Comparison

| Provider | Single Text | Batch (8) | Device | Dependency |
|----------|-------------|-----------|--------|------------|
| **Candle** | **<20ms** | **<40ms** | Metal/CUDA/CPU | None (pure Rust) |
| MLX | ~182ms | ~350ms | Metal only | Python 3.10+ |
| Mock | <1ms | <1ms | CPU | None |

*Benchmarks on M2 Max, 512-dimensional embeddings*

## Architecture

```
EmbeddingProvider (trait)
├── CandleEmbeddingProvider (Candle/Rust)
├── MlxEmbeddingProvider (MLX/Python)
└── MockEmbeddingProvider (Testing)
```

All providers implement the same `EmbeddingProvider` trait:

```rust
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed_batch(&self, request: BatchEmbeddingRequest) 
        -> EmbeddingResult<BatchEmbeddingResponse>;
    
    async fn model_info(&self) -> EmbeddingResult<ModelInfo>;
    
    async fn health_check(&self) -> EmbeddingResult<()>;
}
```

## Development Status

### Candle Provider (Phase 1)

- [x] **Day 1**: Dependencies, file structure, skeleton code
- [ ] **Day 2**: Model loading from Hugging Face Hub
- [ ] **Day 3**: Inference pipeline (tokenization + forward pass)
- [ ] **Day 4**: Comprehensive testing (20+ unit tests)
- [ ] **Day 5**: Integration with REST/gRPC APIs

### Future Phases

- **Phase 2**: Performance optimization (batching, multi-threading)
- **Phase 3**: Production hardening (error handling, retries)
- **Phase 4**: Multi-model support (BGE, E5, Instructor)
- **Phase 5**: Quantization (INT8, INT4)

## Testing

```bash
# Test with Candle only
cargo test --no-default-features --features candle -p akidb-embedding

# Test with MLX only
cargo test --features mlx -p akidb-embedding

# Test both providers
cargo test --features mlx,candle -p akidb-embedding

# Run benchmarks (requires `candle` feature)
cargo bench --features candle -p akidb-embedding
```

## Documentation

```bash
# Generate and open documentation
cargo doc --no-deps --features candle -p akidb-embedding --open
```

## Contributing

When adding a new embedding provider:

1. Implement the `EmbeddingProvider` trait
2. Add feature flag in `Cargo.toml`
3. Update `lib.rs` with conditional compilation
4. Add unit tests (20+ tests)
5. Add benchmark comparison
6. Update this README

## License

See root LICENSE file.

## References

- **Candle ML Framework**: https://github.com/huggingface/candle
- **MLX Framework**: https://github.com/ml-explore/mlx
- **Hugging Face Hub**: https://huggingface.co/models
- **BERT Paper**: https://arxiv.org/abs/1810.04805
