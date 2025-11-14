# Candle Phase 1 - Day 3 Ultrathink: Inference Pipeline Implementation

**Date**: November 10, 2025  
**Phase**: Candle Phase 1 - Foundation  
**Day**: 3 of 5  
**Focus**: Tokenization, BERT forward pass, mean pooling, L2 normalization  
**Estimated Time**: 5-6 hours  
**Prerequisites**: Day 2 complete (model loading working)

---

## Executive Summary

**Goal**: Implement complete inference pipeline to generate embeddings from text inputs, achieving <20ms latency for single text on Metal GPU.

**Outcome**: Fully functional `embed_batch_internal()` method that:
- ✅ Tokenizes text with padding/truncation
- ✅ Runs BERT forward pass on GPU/CPU
- ✅ Applies mean pooling over token embeddings
- ✅ L2 normalizes for semantic similarity
- ✅ Returns Vec<Vec<f32>> embeddings
- ✅ Achieves <20ms latency (10x faster than MLX)

**Success Criteria**:
```rust
let embeddings = provider.embed_batch_internal(vec![
    "Hello world".to_string()
]).await?;

assert_eq!(embeddings[0].len(), 384);  // MiniLM dimension
assert!(embeddings[0].iter().map(|x| x * x).sum::<f32>() - 1.0).abs() < 0.01);  // L2 normalized
println!("Embedding: {:?}", &embeddings[0][..5]);  // Show first 5 values
```

---

## Architecture Overview

### Inference Pipeline Flow

```
Input: Vec<String>
    │
    ├─> 1. Tokenization
    │       ├─> Convert text to token IDs
    │       ├─> Add [CLS] and [SEP] tokens
    │       ├─> Pad/truncate to max_length (512)
    │       └─> Create attention mask
    │       Output: (batch_size, seq_len) token IDs + mask
    │
    ├─> 2. BERT Forward Pass
    │       ├─> Convert token IDs to Tensor
    │       ├─> Run model.forward() on GPU/CPU
    │       └─> Get token embeddings
    │       Output: (batch_size, seq_len, hidden_size) tensor
    │
    ├─> 3. Mean Pooling
    │       ├─> Apply attention mask (ignore padding)
    │       ├─> Sum over sequence length
    │       └─> Divide by non-padding token count
    │       Output: (batch_size, hidden_size) tensor
    │
    ├─> 4. L2 Normalization
    │       ├─> Calculate L2 norm per embedding
    │       └─> Divide by norm
    │       Output: (batch_size, hidden_size) normalized tensor
    │
    └─> 5. Convert to Vec<Vec<f32>>
            └─> Extract CPU data from tensor
            Output: Vec<Vec<f32>>
```

### Tensor Shapes at Each Step

```
Input:         ["Hello world", "Rust is awesome"]
                ↓
Tokenization:  [[101, 7592, 2088, 102, 0, 0, ...],     # (2, 512)
                [101, 17054, 2003, 12476, 102, 0, ...]]
                ↓
Forward Pass:  [[[0.1, 0.2, ...],  # (2, 512, 384)
                  [0.3, 0.4, ...],
                  ...],
                 [...]]
                ↓
Mean Pooling:  [[0.15, 0.25, ...],                     # (2, 384)
                [0.18, 0.22, ...]]
                ↓
L2 Normalize:  [[0.0012, 0.0020, ...],                 # (2, 384) - unit vectors
                [0.0014, 0.0017, ...]]
```

---

## Implementation Plan (5-6 hours)

### Task 3.1: Implement Tokenization (1 hour)

**Goal**: Convert text to token IDs with padding/truncation.

**Steps**:

1. **Encode text to token IDs**:
   ```rust
   let encodings: Vec<_> = texts
       .iter()
       .map(|text| {
           tokenizer.encode(text.as_str(), true)
               .map_err(|e| EmbeddingError::Internal(format!("Tokenization failed: {}", e)))
       })
       .collect::<Result<Vec<_>, _>>()?;
   ```

2. **Extract token IDs and attention masks**:
   ```rust
   let mut token_ids_batch = Vec::new();
   let mut attention_masks_batch = Vec::new();
   
   for encoding in encodings {
       let ids = encoding.get_ids();
       let mask = encoding.get_attention_mask();
       
       token_ids_batch.push(ids.to_vec());
       attention_masks_batch.push(mask.to_vec());
   }
   ```

3. **Pad/truncate to max_length (512)**:
   ```rust
   const MAX_LENGTH: usize = 512;
   
   for token_ids in &mut token_ids_batch {
       if token_ids.len() > MAX_LENGTH {
           token_ids.truncate(MAX_LENGTH);
       } else {
           token_ids.resize(MAX_LENGTH, 0);  // Pad with 0
       }
   }
   
   // Same for attention masks
   for mask in &mut attention_masks_batch {
       if mask.len() > MAX_LENGTH {
           mask.truncate(MAX_LENGTH);
       } else {
           mask.resize(MAX_LENGTH, 0);  // Pad with 0 (ignore padding)
       }
   }
   ```

**Verification**:
```rust
let token_ids = tokenizer.encode("Hello world", true)?;
assert!(token_ids.len() <= 512);
assert_eq!(token_ids[0], 101);  // [CLS] token
```

**Time**: 1 hour

---

### Task 3.2: BERT Forward Pass (1.5 hours)

**Goal**: Run model.forward() to get token embeddings.

**Steps**:

1. **Convert token IDs to Tensor**:
   ```rust
   use candle_core::Tensor;
   
   // Flatten to 1D vector
   let token_ids_flat: Vec<u32> = token_ids_batch
       .iter()
       .flat_map(|ids| ids.iter().copied())
       .collect();
   
   // Create tensor: (batch_size, seq_len)
   let batch_size = token_ids_batch.len();
   let seq_len = MAX_LENGTH;
   
   let token_ids_tensor = Tensor::from_vec(
       token_ids_flat,
       &[batch_size, seq_len],
       &self.device
   )?;
   ```

2. **Create attention mask tensor**:
   ```rust
   let attention_mask_flat: Vec<u32> = attention_masks_batch
       .iter()
       .flat_map(|mask| mask.iter().copied())
       .collect();
   
   let attention_mask_tensor = Tensor::from_vec(
       attention_mask_flat,
       &[batch_size, seq_len],
       &self.device
   )?;
   ```

3. **Run BERT forward pass**:
   ```rust
   // model.forward() returns all token embeddings
   let embeddings = self.model.forward(&token_ids_tensor)?;
   
   // Shape: (batch_size, seq_len, hidden_size)
   // Example: (2, 512, 384) for MiniLM
   ```

**Error Handling**:
```rust
let embeddings = self.model.forward(&token_ids_tensor)
    .map_err(|e| EmbeddingError::Internal(format!("BERT forward pass failed: {}", e)))?;
```

**Verification**:
```rust
let shape = embeddings.shape();
assert_eq!(shape.dims()[0], batch_size);
assert_eq!(shape.dims()[1], 512);
assert_eq!(shape.dims()[2], 384);  // MiniLM hidden size
```

**Time**: 1.5 hours

---

### Task 3.3: Mean Pooling (1 hour)

**Goal**: Average token embeddings to get sentence embeddings.

**Mean Pooling Formula**:
```
sentence_embedding = sum(token_embeddings * attention_mask) / sum(attention_mask)
```

**Steps**:

1. **Expand attention mask to match embeddings shape**:
   ```rust
   // attention_mask: (batch_size, seq_len)
   // embeddings: (batch_size, seq_len, hidden_size)
   
   // Unsqueeze to (batch_size, seq_len, 1)
   let attention_mask_expanded = attention_mask_tensor
       .unsqueeze(2)?
       .to_dtype(candle_core::DType::F32)?;
   
   // Broadcast to (batch_size, seq_len, hidden_size)
   let attention_mask_expanded = attention_mask_expanded
       .broadcast_as(embeddings.shape())?;
   ```

2. **Apply mask and sum**:
   ```rust
   // Multiply embeddings by mask (zero out padding tokens)
   let masked_embeddings = embeddings.mul(&attention_mask_expanded)?;
   
   // Sum over sequence length (axis 1)
   let sum_embeddings = masked_embeddings.sum(1)?;  // (batch_size, hidden_size)
   ```

3. **Calculate mask sum (number of non-padding tokens)**:
   ```rust
   // Sum attention mask over sequence length
   let sum_mask = attention_mask_expanded.sum(1)?;  // (batch_size, hidden_size)
   
   // Clamp to avoid division by zero
   let sum_mask = sum_mask.clamp(1e-9, f32::MAX)?;
   ```

4. **Divide to get mean**:
   ```rust
   let mean_pooled = sum_embeddings.div(&sum_mask)?;
   
   // Shape: (batch_size, hidden_size)
   ```

**Verification**:
```rust
let shape = mean_pooled.shape();
assert_eq!(shape.dims()[0], batch_size);
assert_eq!(shape.dims()[1], 384);  // MiniLM hidden size
```

**Time**: 1 hour

---

### Task 3.4: L2 Normalization (30 minutes)

**Goal**: Normalize embeddings to unit length for cosine similarity.

**L2 Normalization Formula**:
```
normalized = embedding / ||embedding||_2
where ||embedding||_2 = sqrt(sum(embedding^2))
```

**Steps**:

1. **Calculate L2 norm**:
   ```rust
   // Square each element
   let squared = mean_pooled.sqr()?;
   
   // Sum over hidden_size (axis 1)
   let sum_squared = squared.sum(1)?;  // (batch_size,)
   
   // Square root
   let l2_norm = sum_squared.sqrt()?;
   
   // Unsqueeze to (batch_size, 1) for broadcasting
   let l2_norm = l2_norm.unsqueeze(1)?;
   ```

2. **Divide by norm**:
   ```rust
   // Clamp to avoid division by zero
   let l2_norm = l2_norm.clamp(1e-12, f32::MAX)?;
   
   // Normalize
   let normalized = mean_pooled.div(&l2_norm)?;
   ```

**Verification**:
```rust
// Check that norm is approximately 1.0
let verify_norm = normalized.sqr()?.sum(1)?.sqrt()?;
// verify_norm should be close to 1.0 for each embedding
```

**Time**: 30 minutes

---

### Task 3.5: Convert to Vec<Vec<f32>> (30 minutes)

**Goal**: Extract embeddings from GPU tensor to CPU Vec.

**Steps**:

1. **Move tensor to CPU**:
   ```rust
   let normalized_cpu = normalized.to_device(&Device::Cpu)?;
   ```

2. **Extract as Vec<f32>**:
   ```rust
   let embeddings_flat: Vec<f32> = normalized_cpu.to_vec1()?;
   ```

3. **Reshape to Vec<Vec<f32>>**:
   ```rust
   let hidden_size = self.dimension as usize;
   let embeddings: Vec<Vec<f32>> = embeddings_flat
       .chunks(hidden_size)
       .map(|chunk| chunk.to_vec())
       .collect();
   ```

**Verification**:
```rust
assert_eq!(embeddings.len(), batch_size);
assert_eq!(embeddings[0].len(), 384);  // MiniLM dimension

// Check L2 norm is approximately 1.0
let norm: f32 = embeddings[0].iter().map(|x| x * x).sum::<f32>().sqrt();
assert!((norm - 1.0).abs() < 0.01);
```

**Time**: 30 minutes

---

### Task 3.6: Complete embed_batch_internal() (30 minutes)

**Goal**: Integrate all steps into the main method.

**Implementation**:

```rust
async fn embed_batch_internal(
    &self,
    texts: Vec<String>,
) -> EmbeddingResult<Vec<Vec<f32>>> {
    use candle_core::Tensor;

    // Validate input
    if texts.is_empty() {
        return Err(EmbeddingError::InvalidInput("Empty input".to_string()));
    }

    // 1. Tokenization
    let encodings: Vec<_> = texts
        .iter()
        .map(|text| {
            self.tokenizer.encode(text.as_str(), true)
                .map_err(|e| EmbeddingError::Internal(format!("Tokenization: {}", e)))
        })
        .collect::<Result<Vec<_>, _>>()?;

    const MAX_LENGTH: usize = 512;
    let batch_size = texts.len();

    let mut token_ids_batch = Vec::new();
    let mut attention_masks_batch = Vec::new();

    for encoding in encodings {
        let mut ids = encoding.get_ids().to_vec();
        let mut mask = encoding.get_attention_mask().to_vec();

        // Pad or truncate
        if ids.len() > MAX_LENGTH {
            ids.truncate(MAX_LENGTH);
            mask.truncate(MAX_LENGTH);
        } else {
            ids.resize(MAX_LENGTH, 0);
            mask.resize(MAX_LENGTH, 0);
        }

        token_ids_batch.push(ids);
        attention_masks_batch.push(mask);
    }

    // 2. Convert to tensors
    let token_ids_flat: Vec<u32> = token_ids_batch
        .iter()
        .flat_map(|ids| ids.iter().copied())
        .collect();

    let attention_mask_flat: Vec<u32> = attention_masks_batch
        .iter()
        .flat_map(|mask| mask.iter().copied())
        .collect();

    let token_ids_tensor = Tensor::from_vec(
        token_ids_flat,
        &[batch_size, MAX_LENGTH],
        &self.device
    ).map_err(|e| EmbeddingError::Internal(format!("Token tensor: {}", e)))?;

    let attention_mask_tensor = Tensor::from_vec(
        attention_mask_flat,
        &[batch_size, MAX_LENGTH],
        &self.device
    ).map_err(|e| EmbeddingError::Internal(format!("Mask tensor: {}", e)))?;

    // 3. BERT forward pass
    let embeddings = self.model.forward(&token_ids_tensor)
        .map_err(|e| EmbeddingError::Internal(format!("Forward pass: {}", e)))?;

    // 4. Mean pooling
    let attention_mask_expanded = attention_mask_tensor
        .unsqueeze(2)?
        .to_dtype(candle_core::DType::F32)?
        .broadcast_as(embeddings.shape())?;

    let masked_embeddings = embeddings.mul(&attention_mask_expanded)?;
    let sum_embeddings = masked_embeddings.sum(1)?;
    let sum_mask = attention_mask_expanded.sum(1)?.clamp(1e-9, f32::MAX)?;
    let mean_pooled = sum_embeddings.div(&sum_mask)?;

    // 5. L2 normalization
    let squared = mean_pooled.sqr()?;
    let sum_squared = squared.sum(1)?;
    let l2_norm = sum_squared.sqrt()?.unsqueeze(1)?.clamp(1e-12, f32::MAX)?;
    let normalized = mean_pooled.div(&l2_norm)?;

    // 6. Convert to Vec<Vec<f32>>
    let normalized_cpu = normalized.to_device(&Device::Cpu)?;
    let embeddings_flat: Vec<f32> = normalized_cpu.to_vec1()?;

    let hidden_size = self.dimension as usize;
    let embeddings: Vec<Vec<f32>> = embeddings_flat
        .chunks(hidden_size)
        .map(|chunk| chunk.to_vec())
        .collect();

    Ok(embeddings)
}
```

**Time**: 30 minutes

---

### Task 3.7: Integration Testing (1 hour)

**Goal**: Write tests to verify inference works end-to-end.

**Test File**: `tests/candle_tests.rs` (add to existing)

**Tests**:

```rust
#[tokio::test]
#[ignore]
async fn test_inference_single_text() {
    let provider = CandleEmbeddingProvider::new("sentence-transformers/all-MiniLM-L6-v2")
        .await
        .expect("Failed to load model");

    let embeddings = provider.embed_batch_internal(vec![
        "Hello world".to_string()
    ]).await.expect("Failed to generate embedding");

    assert_eq!(embeddings.len(), 1);
    assert_eq!(embeddings[0].len(), 384);

    // Check L2 normalized
    let norm: f32 = embeddings[0].iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 0.01, "Embedding should be L2 normalized");

    println!("Embedding (first 5 dims): {:?}", &embeddings[0][..5]);
}

#[tokio::test]
#[ignore]
async fn test_inference_batch() {
    let provider = CandleEmbeddingProvider::new("sentence-transformers/all-MiniLM-L6-v2")
        .await
        .expect("Failed to load model");

    let embeddings = provider.embed_batch_internal(vec![
        "Hello world".to_string(),
        "Rust is awesome".to_string(),
        "Machine learning".to_string(),
    ]).await.expect("Failed to generate embeddings");

    assert_eq!(embeddings.len(), 3);
    for emb in &embeddings {
        assert_eq!(emb.len(), 384);
        let norm: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01);
    }

    // Check that different texts produce different embeddings
    let sim_01 = cosine_similarity(&embeddings[0], &embeddings[1]);
    let sim_02 = cosine_similarity(&embeddings[0], &embeddings[2]);
    
    println!("Similarity(0,1): {:.3}", sim_01);
    println!("Similarity(0,2): {:.3}", sim_02);
    
    assert!(sim_01 < 0.99);  // Different texts should not be identical
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[tokio::test]
#[ignore]
async fn test_inference_performance() {
    use std::time::Instant;

    let provider = CandleEmbeddingProvider::new("sentence-transformers/all-MiniLM-L6-v2")
        .await
        .expect("Failed to load model");

    // Warm up (first inference might be slower)
    let _ = provider.embed_batch_internal(vec!["warmup".to_string()]).await;

    // Single text benchmark
    let start = Instant::now();
    let _ = provider.embed_batch_internal(vec![
        "Hello world".to_string()
    ]).await.expect("Failed");
    let single_ms = start.elapsed().as_millis();

    // Batch of 8 benchmark
    let texts = vec!["Text".to_string(); 8];
    let start = Instant::now();
    let _ = provider.embed_batch_internal(texts).await.expect("Failed");
    let batch8_ms = start.elapsed().as_millis();

    println!("Performance:");
    println!("  Single text: {}ms (target: <20ms)", single_ms);
    println!("  Batch of 8:  {}ms (target: <40ms)", batch8_ms);

    // Soft assertions (might not meet targets on all hardware)
    if single_ms > 20 {
        eprintln!("⚠️  Single text slower than target ({}ms > 20ms)", single_ms);
    }
    if batch8_ms > 40 {
        eprintln!("⚠️  Batch of 8 slower than target ({}ms > 40ms)", batch8_ms);
    }
}
```

**Time**: 1 hour

---

### Task 3.8: Performance Benchmarking (1 hour)

**Goal**: Create Criterion benchmarks for inference.

**File**: `benches/candle_bench.rs`

**Implementation**:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use akidb_embedding::CandleEmbeddingProvider;
use tokio::runtime::Runtime;

fn bench_inference(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let provider = rt.block_on(async {
        CandleEmbeddingProvider::new("sentence-transformers/all-MiniLM-L6-v2")
            .await
            .expect("Failed to load model")
    });

    // Single text benchmark
    c.bench_function("inference_single", |b| {
        b.to_async(&rt).iter(|| async {
            let result = provider.embed_batch_internal(vec![
                black_box("Hello world".to_string())
            ]).await.unwrap();
            black_box(result)
        });
    });

    // Batch benchmarks
    for batch_size in [1, 2, 4, 8, 16, 32].iter() {
        let texts = vec!["Sample text".to_string(); *batch_size];
        
        c.bench_with_input(
            BenchmarkId::new("inference_batch", batch_size),
            batch_size,
            |b, _| {
                b.to_async(&rt).iter(|| async {
                    let result = provider.embed_batch_internal(black_box(texts.clone()))
                        .await
                        .unwrap();
                    black_box(result)
                });
            },
        );
    }
}

criterion_group!(benches, bench_inference);
criterion_main!(benches);
```

**Run Benchmarks**:
```bash
cargo bench --features candle -p akidb-embedding

# Expected output (M2 Max, Metal GPU):
# inference_single        time: [5.2 ms 5.5 ms 5.8 ms]
# inference_batch/8       time: [12 ms 13 ms 14 ms]
```

**Time**: 1 hour

---

## Verification Checklist

After implementation, verify:

- [ ] Tokenization produces correct token IDs
- [ ] Attention masks created correctly
- [ ] BERT forward pass runs on GPU/CPU
- [ ] Mean pooling applies mask correctly
- [ ] L2 normalization produces unit vectors
- [ ] Different texts produce different embeddings
- [ ] Similar texts have high cosine similarity (>0.7)
- [ ] Single text inference <20ms (Metal GPU)
- [ ] Batch of 8 inference <40ms (Metal GPU)
- [ ] All tests pass

---

## Common Issues and Solutions

### Issue 1: Tensor Shape Mismatch

**Symptom**:
```
Error: Shape mismatch in broadcasting
```

**Cause**: Attention mask dimensions don't match embeddings

**Solution**: Use `unsqueeze` and `broadcast_as` correctly

---

### Issue 2: Division by Zero in Mean Pooling

**Symptom**:
```
Error: NaN in output
```

**Cause**: All tokens masked (sum_mask = 0)

**Solution**: Clamp sum_mask to minimum 1e-9

---

### Issue 3: Slow Performance

**Symptom**: Inference takes >100ms

**Possible Causes**:
- Running on CPU instead of GPU
- Not using Metal GPU on macOS
- Large batch size

**Solution**:
- Verify device selection (should be Metal on macOS)
- Check GPU memory usage
- Reduce batch size if needed

---

### Issue 4: Incorrect Embeddings

**Symptom**: All embeddings identical or random

**Possible Causes**:
- Mean pooling not masking correctly
- Wrong axis for summing
- Not running model in eval mode

**Solution**:
- Verify attention mask multiplication
- Check tensor dimensions at each step
- BERT model should be in eval mode by default

---

## Performance Expectations

**Hardware**: M2 Max, macOS, Metal GPU

| Operation | Target | Stretch Goal | Notes |
|-----------|--------|--------------|-------|
| Single text | <20ms | <10ms | Metal GPU |
| Batch of 8 | <40ms | <25ms | Metal GPU |
| Batch of 32 | <100ms | <60ms | Metal GPU |

**Comparison to MLX**:
- MLX single text: ~182ms (Python overhead)
- Candle target: <20ms (10x faster)

---

## Success Criteria

Day 3 is complete when:

1. ✅ `embed_batch_internal()` fully implemented (no `todo!()`)
2. ✅ Tokenization working with padding/truncation
3. ✅ BERT forward pass runs on GPU
4. ✅ Mean pooling produces correct shape
5. ✅ L2 normalization creates unit vectors
6. ✅ Integration tests pass (3 tests)
7. ✅ Performance benchmarks run
8. ✅ Latency <20ms for single text (Metal GPU)
9. ✅ Git commit with descriptive message

---

## Timeline

| Task | Duration | Cumulative |
|------|----------|------------|
| 3.1: Tokenization | 1 hour | 1 hour |
| 3.2: BERT forward pass | 1.5 hours | 2.5 hours |
| 3.3: Mean pooling | 1 hour | 3.5 hours |
| 3.4: L2 normalization | 30 min | 4 hours |
| 3.5: Vec conversion | 30 min | 4.5 hours |
| 3.6: Integration | 30 min | 5 hours |
| 3.7: Testing | 1 hour | 6 hours |
| 3.8: Benchmarking | 1 hour | 7 hours |
| **Total** | **7 hours** | - |

---

## Deliverables

1. **Code**:
   - `src/candle.rs` - Complete `embed_batch_internal()`
   - `tests/candle_tests.rs` - 3 new integration tests
   - `benches/candle_bench.rs` - Criterion benchmarks

2. **Documentation**:
   - Inline docs with tensor shape comments
   - Day 3 completion report

3. **Git**:
   - Feature branch commit
   - Descriptive commit message with performance metrics

---

## Next Steps (Day 4)

After Day 3 completion:

**Day 4 Focus**: Integration with EmbeddingProvider trait + comprehensive testing

**Key Tasks**:
1. Implement `embed_batch()` trait method
2. Calculate usage statistics (tokens, duration)
3. Implement `health_check()` with test embedding
4. Write 20+ unit tests
5. Test error handling (empty input, invalid text)
6. Verify all feature combinations work

**Target**: Production-ready embedding provider

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Performance miss (<20ms) | Medium | High | Profile and optimize hot paths |
| Memory issues (large batch) | Low | Medium | Start with small batches (≤32) |
| Incorrect pooling | Low | High | Validate with known embeddings |
| GPU OOM | Low | Medium | Add error handling + CPU fallback |

---

**Prepared By**: Claude Code  
**Date**: November 10, 2025  
**Status**: Ready to Execute

---
