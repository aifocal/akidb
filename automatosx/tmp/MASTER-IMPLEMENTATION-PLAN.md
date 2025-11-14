# Master Implementation Plan - ONNX+CoreML Embedding Provider

**Status**: Day 1 Complete ✅, Ready for Day 2 Execution
**Timeline**: 2-3 days total
**Target**: <20ms embedding generation on Apple Silicon

---

## Quick Reference

### Where We Are

**Day 1 Complete** (✅):
- Python environment setup
- Qwen3 model validation
- CoreML EP testing
- Performance analysis
- Comprehensive planning

**Key Finding**: ONNX is 117x faster than Candle, but Qwen3's large vocabulary prevents full CoreML EP acceleration.

**Current Performance**: 118ms (Qwen3 ONNX CPU)
**Target**: <20ms

### Next Immediate Actions

**Day 2 Morning** (2-3 hours):
1. Download all-MiniLM-L6-v2 ONNX
2. Run validation and CoreML EP tests
3. Make go/no-go decision

**Day 2 Afternoon** (6-9 hours):
4. Implement Rust ONNX provider
5. Write tests
6. Integration

**Day 3** (6-8 hours):
7. Comprehensive testing
8. Performance optimization
9. Documentation

---

## Documentation Index

### Planning Documents (Read First)

1. **automatosx/tmp/SESSION-SUMMARY-2025-11-10.md**
   - Original session summary
   - Context from previous work

2. **automatosx/tmp/DAY-1-COMPLETION-SUMMARY.md** (300 lines)
   - Day 1 achievements
   - Files created
   - Critical findings
   - Decision points

3. **automatosx/tmp/DAY-1-POST-ANALYSIS-MEGATHINK.md** (600+ lines)
   - Deep analysis of all options
   - Decision framework
   - Weighted scoring
   - Recommended path: Option A (MiniLM)

4. **automatosx/tmp/DAY-2-EXECUTION-MEGATHINK.md** (700+ lines)
   - Hour-by-hour execution plan
   - Complete Rust code examples
   - Testing strategy
   - Success criteria

### Technical Analysis

5. **automatosx/tmp/PYTHON-COREML-BASELINE-DAY1.md** (600 lines)
   - Qwen3 performance metrics
   - CoreML EP limitation analysis
   - Alternative approaches
   - Comparison matrices

### Scripts Created

6. **scripts/validate_qwen3_onnx.py** (220 lines)
   - ONNX model validation
   - Structure inspection
   - Operator analysis

7. **scripts/test_qwen3_coreml.py** (450 lines)
   - CoreML EP performance testing
   - Quality validation
   - Batch processing tests

8. **scripts/download_qwen3_onnx.py** (160 lines)
   - Qwen3 model downloader
   - Can be adapted for MiniLM

---

## Three Paths Forward

### Path A: MiniLM (RECOMMENDED - 70-80% success)

**Model**: all-MiniLM-L6-v2
**Expected Performance**: 8-15ms P95 ✅
**Quality**: Good (384-dim)
**Timeline**: 2-3 days

**Pros**:
- ✅ Highest probability of <20ms
- ✅ Quick to validate (1-2 hours)
- ✅ Well-tested, widely used
- ✅ ONNX export likely available

**Cons**:
- ⚠️ Lower dimension (384 vs 1024)
- ⚠️ Moderate quality vs Qwen3

**Action**: Try first thing Day 2 morning

### Path B: Accept Qwen3 CPU (Fallback - 100% guaranteed)

**Model**: Qwen3-Embedding-0.6B
**Performance**: 118ms median
**Quality**: Excellent (1024-dim)
**Timeline**: 2 days

**Pros**:
- ✅ Already validated
- ✅ 117x better than Candle
- ✅ Highest quality
- ✅ No uncertainty

**Cons**:
- ❌ 6x slower than target
- ❌ Limited throughput (25 QPS max)
- ❌ Doesn't use GPU/ANE

**Action**: Fall back if Path A fails

### Path C: MLX Investigation (Risky - 40-50% success)

**Framework**: MLX (Apple's ML framework)
**Expected Performance**: 20-30ms (uncertain)
**Quality**: Excellent (1024-dim)
**Timeline**: 4-5 days (risky)

**Pros**:
- ✅ No dimension limits
- ✅ Full Apple Silicon utilization
- ✅ Potentially best long-term

**Cons**:
- ❌ High uncertainty
- ❌ Need model conversion
- ❌ More complex
- ❌ Longer timeline

**Action**: Only if Path A fails AND time allows

---

## Day-by-Day Breakdown

### Day 1: Complete ✅

**Achievements**:
- Python environment setup ✅
- ONNX validation scripts ✅
- CoreML EP testing ✅
- Qwen3 baseline established ✅
- Comprehensive analysis ✅

**Key Metrics**:
- Qwen3 performance: 118ms median
- vs Candle: 117x faster
- Quality: Excellent (L2 norm 1.0, separation 0.63)

**Documentation**: 2,500+ lines created

### Day 2: In Progress

#### Morning (2-3 hours): MiniLM Validation

**Session 1.1**: Search & Download (30-60 min)
```bash
# Search HuggingFace
Repository: Xenova/all-MiniLM-L6-v2 (most likely)

# Download
python3 scripts/download_minilm_onnx.py

# Expected: 50-200MB model
```

**Session 1.2**: Validate & Test (60-90 min)
```bash
# Validate structure
python3 scripts/validate_qwen3_onnx.py \
  --model models/minilm-l6-v2/onnx/model.onnx

# Test CoreML EP
python3 scripts/test_qwen3_coreml.py \
  --model models/minilm-l6-v2/onnx/model.onnx \
  --pooling mean

# Expected P95: 8-15ms ✅
```

**Session 1.3**: Decision Point (immediate)
```
IF P95 < 20ms:
  ✅ Proceed to Rust implementation

ELSE:
  ⚠️ Try alternatives or accept Qwen3
```

#### Afternoon (6-9 hours): Rust Implementation

**Session 2.1**: API Research & Planning (1-2 hours)
- Study ort v2.0.0-rc.10 API
- Review existing onnx.rs
- Plan module structure

**Session 2.2**: Core Implementation (2-3 hours)
```rust
// Modules to implement:
- onnx/session.rs      // ONNX session management
- onnx/pooling.rs      // Mean pooling + L2 norm
- onnx/tokenization.rs // Tokenizer wrapper
- onnx/provider.rs     // Main provider
```

**Session 2.3**: Testing (1-2 hours)
```bash
cargo test -p akidb-embedding --features onnx

# Expected: 4+ tests passing
```

**Session 2.4**: Integration (1-2 hours)
- Integrate with akidb-service
- End-to-end test
- Performance comparison

**End of Day 2 Target**:
- [ ] Rust provider compiles
- [ ] Basic tests passing
- [ ] Performance close to Python

### Day 3: Final

#### Morning (3-4 hours): Comprehensive Testing
- Integration tests
- Performance benchmarks
- Quality validation
- Edge case testing

#### Afternoon (2-3 hours): Optimization
- Profile performance
- Optimize hot paths
- Fine-tune parameters

#### Evening (2-3 hours): Documentation
- Update README
- API documentation
- Examples
- Migration guide

**Delivery Target**: End of Day 3

---

## Success Metrics

### Must Have (Required for Production)

- [ ] **Performance**: P95 < 30ms (at least 4x better than Qwen3)
- [ ] **Quality**: L2 norm ≈ 1.0, good similarity scores
- [ ] **Tests**: All tests passing (unit + integration)
- [ ] **Stability**: No crashes or memory leaks
- [ ] **Documentation**: Clear usage instructions

### Should Have (Target Goals)

- [ ] **Performance**: P95 < 20ms (original target)
- [ ] **Throughput**: >100 QPS batch processing
- [ ] **Quality**: Comparable to Qwen3 for common use cases
- [ ] **Tests**: >90% code coverage
- [ ] **Documentation**: Examples and migration guide

### Nice to Have (Future Improvements)

- [ ] **Performance**: P95 < 15ms
- [ ] **Features**: Multi-provider support (ONNX + MLX)
- [ ] **Tests**: Property-based testing
- [ ] **Documentation**: Performance tuning guide

---

## Risk Management

### Risk Matrix

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| MiniLM doesn't achieve <20ms | 20-30% | High | Try E5/BGE, fall back to Qwen3 |
| ONNX export not available | 10% | Medium | Manual export with Optimum |
| Rust API issues | 30-40% | Medium | Extensive debugging, compare to Python |
| CoreML EP fails | 15% | High | Use CPU-only (still fast) |
| Quality insufficient | 20% | Medium | Try larger model (E5, BGE) |

### Contingency Plans

**If MiniLM >20ms but <30ms**:
1. Try E5-small-v2 (may be more optimized)
2. Try BGE-small-en (newer model)
3. Accept if quality good, optimize later

**If all small models fail**:
1. Quick MLX feasibility check (2 hours)
2. If promising, continue MLX (3-5 days)
3. Otherwise, accept Qwen3 CPU (118ms)

**If Rust integration difficult**:
1. Use Python baseline as reference
2. Debug with extensive logging
3. Compare outputs step by step
4. Seek community help if needed

---

## Code Structure (Target)

```
crates/akidb-embedding/
├── src/
│   ├── lib.rs              # Main exports
│   ├── provider.rs         # EmbeddingProvider trait
│   ├── types.rs            # Request/Response types
│   ├── error.rs            # Error types
│   ├── mock.rs             # Mock provider (testing)
│   ├── mlx.rs              # MLX provider (existing)
│   └── onnx/               # ONNX provider (NEW)
│       ├── mod.rs          # Module exports
│       ├── session.rs      # ONNX session mgmt
│       ├── pooling.rs      # Pooling operations
│       ├── tokenization.rs # Tokenizer wrapper
│       └── provider.rs     # Main implementation
├── Cargo.toml
└── tests/
    └── integration_test.rs

models/
├── minilm-l6-v2/           # NEW
│   ├── onnx/
│   │   └── model.onnx
│   ├── tokenizer.json
│   └── config.json
└── qwen3-embedding-0.6b/   # EXISTING
    └── onnx/
        ├── model_fp16.onnx
        └── ...

scripts/
├── download_minilm_onnx.py  # NEW
├── validate_qwen3_onnx.py   # EXISTING
└── test_qwen3_coreml.py     # EXISTING

automatosx/
├── PRD/                     # Requirements & design
└── tmp/                     # Analysis & planning
    ├── SESSION-SUMMARY-2025-11-10.md
    ├── DAY-1-COMPLETION-SUMMARY.md
    ├── DAY-1-POST-ANALYSIS-MEGATHINK.md
    ├── DAY-2-EXECUTION-MEGATHINK.md
    ├── PYTHON-COREML-BASELINE-DAY1.md
    └── MASTER-IMPLEMENTATION-PLAN.md  # THIS FILE
```

---

## Performance Targets

### Python Baseline (Reference)

| Metric | Qwen3 | MiniLM (Expected) |
|--------|-------|-------------------|
| P95 latency | 171ms | 8-15ms |
| Median | 118ms | 8-12ms |
| Throughput (batch 32) | 25 QPS | 300+ QPS |
| Dimension | 1024 | 384 |
| Quality (separation) | 0.63 | ~0.60 |

### Rust Implementation (Target)

| Metric | Target | Acceptable |
|--------|--------|------------|
| P95 latency | <20ms | <30ms |
| Rust vs Python overhead | <20% | <50% |
| L2 norm accuracy | 1.0 ± 0.01 | 1.0 ± 0.05 |
| Tests passing | 100% | >90% |

---

## Quick Start Guide (Day 2 Morning)

### Step 1: Download MiniLM (15-30 min)

```bash
cd /Users/akiralam/code/akidb2

# Search HuggingFace
open https://huggingface.co/models?search=all-MiniLM-L6-v2%20onnx

# Download (try Xenova first)
python3 -c "
from huggingface_hub import snapshot_download
snapshot_download(
    repo_id='Xenova/all-MiniLM-L6-v2',
    local_dir='models/minilm-l6-v2',
    allow_patterns=['onnx/*', '*.json']
)
"
```

### Step 2: Validate Model (5 min)

```bash
python3 scripts/validate_qwen3_onnx.py \
  --model models/minilm-l6-v2/onnx/model.onnx

# Look for:
# ✅ Vocab: 30,522 (<16K limit)
# ✅ Hidden dim: 384
# ✅ No warnings
```

### Step 3: Test Performance (10-15 min)

```bash
# Update test script for mean pooling
# Then run:
python3 scripts/test_qwen3_coreml.py \
  --model models/minilm-l6-v2/onnx/model.onnx \
  --pooling mean

# Look for:
# ✅ P95 < 20ms
# ✅ CoreML EP activated
# ✅ Quality good
```

### Step 4: Decision

```
IF P95 < 20ms:
  📝 Document results
  ✅ Proceed to Rust implementation
  📅 Expected completion: End of Day 3

ELSE:
  🔍 Debug or try alternatives
  📋 Reassess timeline
```

---

## Contact & Resources

### Documentation

- **This Plan**: automatosx/tmp/MASTER-IMPLEMENTATION-PLAN.md
- **Day 1 Analysis**: automatosx/tmp/DAY-1-POST-ANALYSIS-MEGATHINK.md
- **Day 2 Execution**: automatosx/tmp/DAY-2-EXECUTION-MEGATHINK.md
- **Python Baseline**: automatosx/tmp/PYTHON-COREML-BASELINE-DAY1.md

### Scripts

- **Validate**: scripts/validate_qwen3_onnx.py
- **Test**: scripts/test_qwen3_coreml.py
- **Download**: scripts/download_qwen3_onnx.py (adapt for MiniLM)

### External Resources

- **ONNX Runtime**: https://onnxruntime.ai/
- **CoreML EP Docs**: https://onnxruntime.ai/docs/execution-providers/CoreML-ExecutionProvider.html
- **ort crate**: https://docs.rs/ort/2.0.0-rc.10/
- **HuggingFace Models**: https://huggingface.co/models

---

## Summary

**Current Status**: Day 1 complete with comprehensive analysis and planning

**Next Action**: Download and validate all-MiniLM-L6-v2 ONNX model

**Primary Path**: MiniLM → Rust implementation → Production (2-3 days)

**Fallback**: Accept Qwen3 CPU (118ms, still 117x better than Candle)

**Confidence**: High (75-80%) for successful completion

**Expected Outcome**: Production-ready ONNX embedding provider with <20ms latency

---

**Last Updated**: November 10, 2025 (end of Day 1)
**Next Update**: Day 2 evening (after implementation)

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>
