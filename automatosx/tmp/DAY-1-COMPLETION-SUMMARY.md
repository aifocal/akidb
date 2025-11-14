# Day 1 Completion Summary - ONNX+CoreML Phase

**Date**: November 10, 2025
**Duration**: ~4 hours
**Status**: ✅ **COMPLETE** with critical findings

---

## What We Accomplished

### Session 1: Environment Setup ✅

**Goal**: Install Python dependencies for ONNX validation

**Completed**:
- ✅ Installed `onnx` v1.19.1
- ✅ Installed `onnxruntime` v1.19.2
- ✅ Installed `transformers` v4.57.1
- ✅ Verified all packages working

**Time**: ~5 minutes

### Session 2: ONNX Model Validation ✅

**Goal**: Validate Qwen3-Embedding-0.6B ONNX model structure

**Created**:
- `scripts/validate_qwen3_onnx.py` (~220 lines)

**Findings**:
- ✅ Model loads successfully
- ✅ FP16 model: 570KB + 1.1GB external data
- ⚠️ ONNX checker warning about `SimplifiedLayerNormalization` (expected, ONNX Runtime handles it)
- ✅ Model structure validated:
  - Inputs: 57 (input_ids, attention_mask, position_ids, + 56 KV cache inputs)
  - Outputs: 57 (last_hidden_state + 56 KV cache outputs)
  - Operators: 1,587 total, 25 unique types
  - Output dimension: **1024** (not 768 as initially thought)

**Time**: ~30 minutes

### Session 3: CoreML EP Performance Testing ✅

**Goal**: Test ONNX Runtime with CoreML Execution Provider

**Created**:
- `scripts/test_qwen3_coreml.py` (~450 lines)

**Tests Performed**:
1. ✅ Single text performance (10 runs)
2. ✅ Batch processing (batch 1-32)
3. ✅ Embedding quality validation
4. ✅ Similarity testing

**Results**:
- Performance: P95 171ms (target: <20ms) ❌
- Quality: Perfect L2 normalization (1.0), good similarity scores ✅
- Batch efficiency: 2.8x improvement at batch 32 ✅
- **CRITICAL**: CoreML EP dimension warning (151,669 > 16,384 limit) ⚠️

**Time**: ~1.5 hours

### Session 4: Documentation ✅

**Goal**: Document baseline metrics and findings

**Created**:
- `automatosx/tmp/PYTHON-COREML-BASELINE-DAY1.md` (~600 lines)

**Documented**:
- Complete performance metrics
- CoreML EP limitation analysis
- Model architecture details
- 5 alternative approaches with pros/cons
- Recommendation for path forward

**Time**: ~1.5 hours

---

## Critical Findings

### Finding 1: CoreML EP Input Dimension Limit ⚠️

**Issue**: CoreML does not support input dimensions > 16,384. Qwen3's embedding table is 151,669 x 1,024.

**Impact**:
- CoreML EP cannot process embedding layer (largest operation)
- Falls back to CPU for embedding lookup
- Only transformer layers use CoreML EP
- Performance limited to CPU speed for bottleneck operation

**Evidence**:
```
[W:onnxruntime:, helper.cc:82 IsInputSupported]
CoreML does not support input dim > 16384.
Input: model.embed_tokens.weight, shape: {151669,1024}
```

### Finding 2: ONNX 117x Faster Than Candle ✅

**Comparison**:
- Candle CPU (Week 1): 13,841ms
- ONNX CoreML EP: 118ms (median)
- **Speedup**: 117x faster!

**Why**:
- ONNX Runtime has highly optimized CPU kernels
- FP16 model (vs Candle FP32) reduces memory bandwidth
- Better compiler optimizations

**Implication**: Even without full CoreML EP, ONNX is a huge improvement.

### Finding 3: Embedding Quality is Good ✅

**Metrics**:
- L2 norm: 1.000000 (perfect normalization)
- Similar queries similarity: 0.7676
- Different queries similarity: 0.1390
- Separation: 0.6286

**Conclusion**: Model generates high-quality embeddings despite performance issues.

### Finding 4: Model is Decoder, Not Encoder ⚠️

**Discovery**: Qwen3-Embedding is based on `Qwen3ForCausalLM` (decoder) with KV cache, not typical BERT-style encoder.

**Characteristics**:
- 28 transformer layers (large for embedding model)
- 151,669 token vocabulary (multilingual + code)
- 1024-dim hidden size (larger than typical 384/768)
- Requires task instruction for best results
- Uses last-token pooling (not mean pooling)

**Implication**: More complex than typical embedding model, explains larger size and performance characteristics.

---

## Performance Summary

### Single Text (Median of 10 runs)

| Metric | Result | vs Target | vs Candle |
|--------|--------|-----------|-----------|
| **Latency** | 118ms | 6x slower ❌ | 117x faster ✅ |
| **Warmup** | 257ms | 13x slower ❌ | 54x faster ✅ |
| **P95** | 171ms | 8.6x slower ❌ | 81x faster ✅ |

### Batch Processing

| Batch | Per Text | Throughput |
|-------|----------|------------|
| 1 | 111ms | 9 QPS |
| 8 | 49ms | 20 QPS |
| 32 | 40ms | 25 QPS |

### Quality Metrics

| Metric | Result | Status |
|--------|--------|--------|
| Dimension | 1024 | ✅ |
| L2 norm | 1.000000 | ✅ |
| Similarity separation | 0.63 | ✅ |

---

## Files Created

### Python Scripts
1. **scripts/validate_qwen3_onnx.py** (~220 lines)
   - Model structure validation
   - Input/output inspection
   - Operator analysis

2. **scripts/test_qwen3_coreml.py** (~450 lines)
   - CoreML EP testing
   - Performance benchmarking
   - Quality validation
   - Batch processing tests

### Documentation
3. **automatosx/tmp/PYTHON-COREML-BASELINE-DAY1.md** (~600 lines)
   - Complete performance analysis
   - CoreML EP limitation details
   - Alternative approaches
   - Recommendations

4. **automatosx/tmp/DAY-1-COMPLETION-SUMMARY.md** (this file)
   - Session summary
   - Key findings
   - Decision points

### Logs
5. **/tmp/qwen3_coreml_test_output.txt**
   - Raw test output for reference

**Total Documentation**: ~1,300 lines created today

---

## Decision Point: Path Forward

### Option A: Smaller BERT Model (RECOMMENDED) ✅

**Model**: `sentence-transformers/all-MiniLM-L6-v2` or `intfloat/e5-small-v2`

**Pros**:
- Vocabulary <16K (fits in CoreML EP)
- Likely achieves <20ms target
- Smaller size (200-500MB vs 1.1GB)
- Faster iteration

**Cons**:
- Lower dimension (384 vs 1024)
- Potentially lower quality for specialized tasks

**Next Steps**:
1. Download ONNX version of MiniLM or E5
2. Run same validation scripts
3. If P95 <20ms, proceed to Rust implementation
4. If not, consider Option C (MLX)

**Timeline**: 1 day to test, 2-3 days for Rust implementation

### Option B: Accept Qwen3 CPU Performance ⚠️

**Accept**: 118ms median latency (6x slower than target)

**Pros**:
- Already have model downloaded
- 117x faster than Candle baseline
- Good quality embeddings

**Cons**:
- Doesn't meet <20ms target
- Not production-ready for high QPS
- Doesn't utilize Apple Silicon GPU/ANE

**Next Steps**:
1. Proceed directly to Rust implementation
2. Document limitation in README
3. Consider optimization later

**Timeline**: Start Day 2 immediately

### Option C: Investigate MLX ⚙️

**Model**: Use MLX (Apple's ML framework) instead of ONNX

**Pros**:
- Native Apple Silicon support
- No dimension limits
- Direct Metal access
- Potentially best performance

**Cons**:
- More research needed
- Need to find/create MLX model
- Different from ONNX approach

**Next Steps**:
1. Research MLX embedding models
2. Install MLX Python package
3. Test performance
4. Compare to ONNX approach

**Timeline**: 2-3 days research + testing

---

## Recommendation

### Immediate Action: Try Option A (Smaller BERT) ✅

**Rationale**:
1. Quick to test (1 day)
2. High probability of success (<20ms target)
3. If doesn't work, still have Options B & C
4. Smaller model is easier to deploy

**Action Plan for Tomorrow**:
1. Download `sentence-transformers/all-MiniLM-L6-v2` ONNX
2. Run validation and CoreML EP tests
3. Check if P95 <20ms achieved
4. If yes → Begin Rust implementation
5. If no → Reassess (try Option C or accept Option B)

### Fallback: Option B if Time Constrained

If we need to move forward quickly:
- Accept 118ms performance
- Document as known limitation
- Implement Rust provider with Qwen3
- Revisit optimization in Phase 2

---

## Lessons Learned

### What Went Well ✅

1. **ONNX Runtime is Fast**: 117x faster than Candle even on CPU
2. **Embedding Quality is Good**: Model produces high-quality embeddings
3. **Clear Problem Identification**: CoreML EP limitation identified early
4. **Comprehensive Testing**: Battery of tests gives confidence in results

### Challenges Encountered ⚠️

1. **CoreML EP Dimension Limit**: Unexpected limitation for large vocabularies
2. **Model Complexity**: Qwen3 is more complex (decoder) than typical embedding models
3. **Performance Gap**: 6x slower than target despite CoreML EP

### What We'd Do Differently

1. **Research Model First**: Check vocabulary size before downloading
2. **Start with Smaller Model**: Try MiniLM first, scale up if needed
3. **Test Multiple Models**: Have backup options ready

---

## Status Summary

### Completed ✅
- [x] Day 1 Session 1: Install Python dependencies
- [x] Day 1 Session 2: Create and run ONNX validation script
- [x] Day 1 Session 3: Create and run CoreML EP test script
- [x] Day 1 Session 4: Document Python baseline metrics

### Pending ⏳
- [ ] DECISION: Choose path forward (A, B, or C)
- [ ] Day 2: Rust implementation
- [ ] Day 3: Testing and documentation

### Blocked 🚫
- None (have clear options to proceed)

---

## Next Session Plan

**If Option A (Recommended)**:
1. Download MiniLM ONNX model
2. Test CoreML EP performance
3. If successful → Begin Rust onnx.rs implementation
4. Estimated time: 6-8 hours

**If Option B (Fallback)**:
1. Begin Rust implementation with Qwen3
2. Research ort v2.0.0-rc.10 API
3. Implement core structure
4. Estimated time: 8-10 hours

**If Option C (Research)**:
1. Install MLX and dependencies
2. Find MLX embedding model
3. Test MLX performance
4. Compare approaches
5. Estimated time: 10-12 hours

---

## Conclusion

Day 1 was **highly successful** in identifying critical issues early:

**The Good**:
- ✅ ONNX is 117x faster than Candle
- ✅ Embedding quality is excellent
- ✅ Clear path forward identified

**The Challenge**:
- ⚠️ CoreML EP limited by large vocabulary
- ⚠️ Performance 6x slower than target
- ⚠️ Need to choose path forward

**The Plan**:
- 🎯 Try smaller BERT model (Option A)
- 🎯 If successful, proceed to Rust
- 🎯 If not, reassess options

**Overall**: Strong foundation for Day 2, clear decision point, comprehensive understanding of the problem space.

---

**Time Invested**: ~4 hours
**Value Delivered**: Critical finding + clear options
**Confidence**: High (have multiple viable paths)

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>
