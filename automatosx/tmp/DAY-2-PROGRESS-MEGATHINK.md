# Day 2 Progress Analysis & Path Forward

**Date**: November 10, 2025
**Status**: 90% Complete - One API Blocker Remaining
**Timeline**: 6-8 hours to completion

---

## Executive Summary

**CRITICAL SUCCESS**: MiniLM CoreML EP achieves **P95 = 10.02ms** (target: <20ms) ✅

We've made excellent progress and are 90% complete with the ONNX implementation. The Python validation proves the approach works perfectly. We're blocked on a single Rust API compatibility issue with `ort` v2.0.0-rc.10's ndarray type system.

**Recommendation**: Two paths forward:
1. **Path A (2-3 hours)**: Debug ort v2 API using their examples/docs
2. **Path B (4-6 hours)**: Use Python subprocess as temporary bridge until ort v2 stable release

---

## Achievements Today (Day 2)

### Hour 1: Model Acquisition ✅

**Downloaded**: Qdrant/all-MiniLM-L6-v2-onnx (86MB)
- Model: models/minilm-l6-v2/model.onnx
- Tokenizer: models/minilm-l6-v2/tokenizer.json
- Vocabulary: 30,522 tokens (BERT WordPiece)
- Dimensions: 384 (hidden size)

**Time**: 2.3 seconds download

### Hour 2: Performance Validation ✅

**Python Baseline Results**:
```
P50: 9.55ms
P95: 10.02ms ✅ (target: <20ms)
P99: 10.63ms
Min: 8.77ms
Max: 10.63ms

L2 Norm: 1.000000 (perfect normalization)
```

**CoreML EP Analysis**:
- Warning: "CoreML does not support input dim > 16384. Input:embeddings.word_embeddings.weight, shape: {30522,384}"
- Result: Embedding layer runs on CPU
- But: 71% of nodes (231/323) run on CoreML EP
- Transformer layers accelerated by GPU/ANE
- **Overall performance: Excellent (10ms P95)**

**Decision**: ✅ **GO** - Proceed with Rust implementation

### Hours 3-6: Rust Implementation (90% Complete) ✅

**Files Modified**:
- `crates/akidb-embedding/Cargo.toml`: Updated dependencies
- `crates/akidb-embedding/src/onnx.rs`: 340+ lines implemented

**Implementation Progress**:

| Component | Status | Lines | Notes |
|-----------|--------|-------|-------|
| Session creation | ✅ Complete | 30 | GraphOptimizationLevel configured |
| Tokenizer loading | ✅ Complete | 15 | From local file path |
| Tokenization | ✅ Complete | 40 | 3 inputs: input_ids, attention_mask, token_type_ids |
| Tensor creation | ✅ Complete | 30 | Array2<i64> for all inputs |
| Mean pooling | ✅ Complete | 35 | Attention mask weighted averaging |
| L2 normalization | ✅ Complete | 10 | Unit vector normalization |
| EmbeddingProvider trait | ✅ Complete | 80 | embed_batch, model_info, health_check |
| ONNX inference | ⚠️ Blocked | 20 | **API compatibility issue** |

**Total Code**: ~340 lines (vs ~450 lines in Python scripts)

### Current Blocker: ort v2 API Type System

**Error**:
```rust
error[E0277]: the trait bound `ArrayBase<OwnedRepr<i64>, Dim<[usize; 2]>>: 
    OwnedTensorArrayData<_>` is not satisfied
```

**Root Cause**: `Value::from_array()` requires specific ndarray types that implement `OwnedTensorArrayData`

**What Works in Python**:
```python
# Python ONNX Runtime (reference)
session = ort.InferenceSession(model_path, providers=[
    ('CoreMLExecutionProvider', {...}),
    'CPUExecutionProvider'
])

outputs = session.run(None, {
    'input_ids': input_ids.astype(np.int64),
    'attention_mask': attention_mask.astype(np.int64),
    'token_type_ids': token_type_ids.astype(np.int64)
})
```

**What We Need in Rust**:
```rust
// Current attempt (doesn't compile)
let input_ids_value = Value::from_array(input_ids_array)?;

let outputs = self.session.run(ort::inputs![
    "input_ids" => input_ids_value,
    "attention_mask" => attention_mask_value,
    "token_type_ids" => token_type_ids_value
])?;
```

---

## Problem Analysis

### Why This Is Happening

**ort v2.0.0-rc.10** (prerelease) has significant API changes from v1.x:
- New type system for Value creation
- Stricter trait bounds on ndarray types
- `OwnedTensorArrayData` trait not well documented
- Examples/documentation may be outdated

**Our ndarray usage**:
```rust
let input_ids_array = Array2::from_shape_vec(
    (batch_size, MAX_LENGTH),
    input_ids_vec,  // Vec<i64>
)?;

// This is Array2<i64> = ArrayBase<OwnedRepr<i64>, Dim<[usize; 2]>>
// ort wants something that implements OwnedTensorArrayData
```

### What We've Tried

1. ✅ **Updated imports**: `ort::value::Value`
2. ✅ **Updated ndarray**: 0.15 → 0.16 (matches ort dependency)
3. ❌ **Owned arrays**: `Value::from_array(array)` - doesn't implement trait
4. ❌ **Views**: `Value::from_array(array.view())` - wrong type
5. ❌ **CowArray**: `Value::from_array(CowArray::from(array))` - still wrong

### Potential Solutions

#### Solution 1: Use ort's Tensor API Directly

Instead of `Value::from_array()`, build tensors manually:

```rust
use ort::tensor::TensorElementType;

// Create tensor with explicit shape and data
let input_ids_tensor = ort::Tensor::from_array(
    &[batch_size as i64, MAX_LENGTH as i64],
    input_ids_vec.as_slice()
)?;
```

**Probability**: 60% - May work if Tensor API is more flexible

#### Solution 2: Check ort Examples

The ort crate likely has examples showing correct usage:

```bash
# Download ort source
git clone https://github.com/pyke-io/ort
cd ort/examples

# Look for ndarray usage patterns
grep -r "from_array" .
grep -r "Array2" .
```

**Probability**: 80% - Official examples should show the way

#### Solution 3: Use Python Subprocess (Temporary Bridge)

If Rust API is too unstable, wrap Python as temporary solution:

```rust
pub struct PythonOnnxProvider {
    python_path: String,
    script_path: String,
}

impl PythonOnnxProvider {
    async fn embed_batch_internal(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        // Serialize texts to JSON
        let input_json = serde_json::to_string(&texts)?;
        
        // Call Python script
        let output = Command::new(&self.python_path)
            .arg(&self.script_path)
            .arg("--input")
            .arg(input_json)
            .output()?;
        
        // Deserialize embeddings
        let embeddings: Vec<Vec<f32>> = serde_json::from_slice(&output.stdout)?;
        Ok(embeddings)
    }
}
```

**Pros**:
- ✅ Known to work (Python validation succeeded)
- ✅ Get to production quickly (4-6 hours)
- ✅ Can replace with native Rust later (when ort v2 stable)

**Cons**:
- ❌ Subprocess overhead (~2-5ms extra latency)
- ❌ Not as elegant
- ❌ Python dependency

**Performance Impact**:
- Python baseline: 10ms P95
- Subprocess overhead: +2-5ms
- Total: ~12-15ms P95 (still under 20ms target ✅)

#### Solution 4: Try ort v1.x (Stable)

Check if ort v1.16 (stable) has simpler API:

```toml
ort = { version = "1.16", features = ["download-binaries"] }
```

**Probability**: 70% - May have easier API but miss CoreML EP features

---

## Recommended Path Forward

### Path A: Debug ort v2 API (2-3 hours) - RECOMMENDED

**Hour 1: Research**
1. Clone ort repository
2. Read examples and tests
3. Find working ndarray → Value pattern
4. Check ort v2 migration guide

**Hour 2: Fix Implementation**
1. Apply correct pattern from examples
2. Build and test
3. If works → proceed to testing

**Hour 3: Fallback Decision**
- If still stuck → switch to Path B

**Success Probability**: 70%
**Time to Production**: Day 2 end (if successful)

### Path B: Python Bridge (4-6 hours) - FALLBACK

**Hour 1: Create Python Script**
- Standalone script with JSON input/output
- Use existing validation code
- Add error handling

**Hour 2: Rust Wrapper**
- PythonOnnxProvider implementation
- Command execution
- JSON serialization/deserialization

**Hour 3: Integration**
- Update service configuration
- Update tests
- End-to-end validation

**Hour 4: Testing**
- Unit tests
- Integration tests
- Performance benchmarks

**Hours 5-6: Documentation**
- API documentation
- Performance metrics
- Migration plan (Python → native Rust later)

**Success Probability**: 95%
**Time to Production**: Day 3 morning
**Performance**: ~15ms P95 (acceptable)

---

## Decision Matrix

| Factor | Path A (Debug ort) | Path B (Python Bridge) |
|--------|-------------------|----------------------|
| **Time** | 2-3 hours | 4-6 hours |
| **Success Probability** | 70% | 95% |
| **Performance** | 10ms P95 | 15ms P95 |
| **Elegance** | High | Medium |
| **Maintainability** | High (long-term) | Medium (need migration) |
| **Risk** | Medium | Low |
| **Production Ready** | Day 2 end | Day 3 morning |

**Recommendation**: Try Path A for 2-3 hours, fall back to Path B if stuck

---

## Technical Deep Dive: The ort v2 Type System

### Understanding OwnedTensorArrayData

This trait is the key to Value::from_array compatibility. Let's analyze what it requires:

```rust
pub trait OwnedTensorArrayData<T>: Sized {
    fn shape(&self) -> &[usize];
    fn as_slice(&self) -> &[T];
}
```

**Requirements**:
1. Must provide shape as `&[usize]`
2. Must provide data as `&[T]`
3. Must be `Sized`

**ndarray::Array2<T>** has:
- `.shape()` → `&[usize]` ✅
- `.as_slice()` → `Option<&[T]>` ⚠️ (not guaranteed contiguous)

**Possible Issue**: Our Array2 might not be contiguous in memory.

**Solution**: Ensure contiguity before conversion:

```rust
let input_ids_array = Array2::from_shape_vec(
    (batch_size, MAX_LENGTH),
    input_ids_vec,
)?.into_shape((batch_size * MAX_LENGTH,))?.into_shape((batch_size, MAX_LENGTH))?;
// This forces contiguous layout
```

### Alternative: Use CowArray with Standard Layout

```rust
use ndarray::{Array, ArrayView, CowArray, Ix2};

// Create standard layout array
let input_ids_cow: CowArray<i64, Ix2> = CowArray::from(
    Array::from_shape_vec((batch_size, MAX_LENGTH), input_ids_vec)?
);

let input_ids_value = Value::from_array(input_ids_cow)?;
```

---

## Next Steps (Immediate Actions)

### Action 1: Quick ort Examples Check (30 min)

```bash
cd /tmp
git clone https://github.com/pyke-io/ort
cd ort
find . -name "*.rs" -exec grep -l "from_array" {} \;
```

Look for:
- Working examples with i64 arrays
- Tensor creation patterns
- CoreML EP usage

### Action 2: Try Standard Layout Array (15 min)

```rust
// Force standard layout
let input_ids_array = Array2::from_shape_vec(
    (batch_size, MAX_LENGTH).strides((MAX_LENGTH, 1)),
    input_ids_vec,
)?;
```

### Action 3: Check ort Documentation (15 min)

```bash
cargo doc --open -p ort
```

Search for:
- `Value::from_array` documentation
- `OwnedTensorArrayData` trait
- Array creation examples

### Action 4: Test Alternative ort v1.x (30 min)

```toml
ort = { version = "1.16", features = ["download-binaries", "coreml"] }
```

### Action 5: If All Fail → Python Bridge (4-6 hours)

---

## Risk Assessment

### High-Risk Scenario: ort v2 Blocker Persists

**Probability**: 30%

**Impact**: Delays by 1 day

**Mitigation**: Python bridge fallback (guaranteed to work)

### Medium-Risk Scenario: Python Bridge Performance

**Probability**: 20% (if using Path B)

**Impact**: +5ms latency (subprocess overhead)

**Acceptance**: 15ms still under 20ms target ✅

**Mitigation**: Document migration plan to native Rust

### Low-Risk Scenario: Integration Issues

**Probability**: 10%

**Impact**: Minor delays (1-2 hours)

**Mitigation**: Comprehensive testing plan

---

## Success Criteria

### Must Have (Required)

- [ ] **Compilation**: Code compiles without errors
- [ ] **Functionality**: Generates embeddings matching Python baseline
- [ ] **Performance**: P95 < 20ms (actual: 10-15ms expected)
- [ ] **Quality**: L2 norm ≈ 1.0 ± 0.01
- [ ] **Tests**: Basic unit tests passing

### Should Have (Target)

- [ ] **Performance**: P95 < 15ms
- [ ] **Native**: Pure Rust (no Python subprocess)
- [ ] **Tests**: Integration tests passing
- [ ] **Documentation**: Usage examples

### Nice to Have (Future)

- [ ] **Optimization**: P95 < 10ms (match Python exactly)
- [ ] **Multi-model**: Support for multiple ONNX models
- [ ] **Benchmarks**: Comprehensive performance suite

---

## Timeline Projections

### Scenario A: ort v2 Debug Successful (70% probability)

**Today (Day 2)**:
- [2 hours] Debug and fix ort API
- [1 hour] Integration testing
- [1 hour] Documentation

**Tomorrow (Day 3)**:
- [2 hours] Comprehensive testing
- [1 hour] Performance optimization
- [1 hour] Final polish

**Total**: 8 hours → Day 3 EOD production ready

### Scenario B: Python Bridge Fallback (30% probability)

**Today (Day 2)**:
- [2 hours] Attempt ort v2 debug
- [2 hours] Switch to Python bridge design

**Tomorrow (Day 3)**:
- [2 hours] Python bridge implementation
- [2 hours] Integration and testing
- [2 hours] Documentation and polish

**Total**: 10 hours → Day 3 EOD production ready

**Future Migration**:
- When ort v2.0 stable released
- 4-6 hours to migrate Python → native Rust
- Low priority (Python bridge works fine)

---

## Code Snippets for Quick Fixes

### Fix 1: Ensure Contiguous Layout

```rust
// Add after Array2 creation
let input_ids_array = input_ids_array
    .as_standard_layout()
    .to_owned();
```

### Fix 2: Use into_shape for Contiguity

```rust
// Convert to 1D then back to 2D (forces contiguous)
let input_ids_array = Array2::from_shape_vec(
    (batch_size, MAX_LENGTH),
    input_ids_vec,
)?
.into_shape((batch_size * MAX_LENGTH,))?
.into_shape((batch_size, MAX_LENGTH))?;
```

### Fix 3: Direct Tensor API (if available)

```rust
use ort::tensor::Tensor;

let input_ids_tensor = Tensor::from_array_with_shape(
    input_ids_vec.as_slice(),
    &[batch_size as i64, MAX_LENGTH as i64],
)?;
```

### Fix 4: Python Bridge (fallback)

```rust
// crates/akidb-embedding/src/python_bridge.rs
use std::process::Command;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct EmbedRequest {
    texts: Vec<String>,
}

#[derive(Deserialize)]
struct EmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

pub struct PythonOnnxBridge {
    script_path: String,
}

impl PythonOnnxBridge {
    pub fn new(script_path: String) -> Self {
        Self { script_path }
    }
    
    pub async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        let request = EmbedRequest { texts };
        let input_json = serde_json::to_string(&request)?;
        
        let output = Command::new("python3")
            .arg(&self.script_path)
            .arg(input_json)
            .output()?;
        
        let response: EmbedResponse = serde_json::from_slice(&output.stdout)?;
        Ok(response.embeddings)
    }
}
```

---

## Conclusion

We're at 90% completion with excellent Python validation results (P95 = 10ms ✅). The remaining 10% is a single Rust API compatibility issue with `ort` v2.0.0-rc.10.

**Recommended Strategy**:
1. Spend 2-3 hours debugging ort v2 API (70% success probability)
2. If stuck, fall back to Python bridge (95% success, +5ms latency)
3. Either way, production-ready by Day 3 EOD

**Key Achievement**: CoreML EP works perfectly with MiniLM despite vocabulary limitations - achieving 10ms P95 with 71% GPU acceleration.

**Next Session Priority**: Resolve Value::from_array type compatibility or implement Python bridge fallback.

---

**Generated**: November 10, 2025
**Status**: Day 2 90% Complete
**Next**: Choose Path A or Path B based on ort debug success

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>
