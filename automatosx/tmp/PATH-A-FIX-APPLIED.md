# Path A: ONNX Runtime with CoreML EP - Fix Applied

**Date**: November 11, 2025
**Status**: 🔄 Build in Progress
**Goal**: Achieve <20ms P95 embedding inference with native Rust ONNX + CoreML

---

## Root Cause Analysis

### Problem
ONNX Runtime v1.16.3 build failed during CMake configuration with error:
```
CMake Error at .../date-src/CMakeLists.txt:1 (cmake_minimum_required):
  Compatibility with CMake < 3.5 has been removed from CMake.
```

### Root Cause
The `date` library dependency (Howard Hinnant's date library) has an outdated CMakeLists.txt requiring CMake 3.1.0, but modern CMake (≥3.30) requires minimum version 3.5.

### Impact
- CMake configuration failed before compilation could begin
- Build directory: `/Users/akiralam/onnxruntime-build`
- Blocking: CoreML Execution Provider integration

---

## Solution Applied

### Fix
Patched the date library's CMakeLists.txt to update minimum CMake version:

```bash
# Original
cmake_minimum_required( VERSION 3.1.0 )

# Patched
cmake_minimum_required( VERSION 3.5.0 )
```

**File**: `/Users/akiralam/onnxruntime-build/build/MacOS/Release/_deps/date-src/CMakeLists.txt`

### Build Command
```bash
cd /Users/akiralam/onnxruntime-build
./build.sh \
    --config Release \
    --use_coreml \
    --build_shared_lib \
    --parallel \
    --skip_tests \
    --cmake_extra_defines \
        CMAKE_OSX_ARCHITECTURES=arm64 \
        CMAKE_OSX_DEPLOYMENT_TARGET=11.0
```

**Build Started**: Background process (ea56c2)
**Expected Duration**: 20-30 minutes
**Expected Output**: `libonnxruntime.dylib` (~65MB) with CoreML EP enabled

---

## Next Steps

### 1. Monitor Build Progress
```bash
# Check build status
ax runs list

# View build output
tail -f /tmp/onnx-build.log  # if logging enabled
```

### 2. Verify CoreML EP (After Build Completes)
```bash
# Check if libonnxruntime.dylib exists
ls -lh /Users/akiralam/onnxruntime-build/build/MacOS/Release/libonnxruntime.dylib

# Verify CoreML symbols in library
nm /Users/akiralam/onnxruntime-build/build/MacOS/Release/libonnxruntime.dylib | grep -i coreml
```

### 3. Configure Environment
```bash
# Add to ~/.zshrc or ~/.bashrc
export ORT_STRATEGY=system
export ORT_DYLIB_PATH="/Users/akiralam/onnxruntime-build/build/MacOS/Release/libonnxruntime.dylib"
```

### 4. Update Cargo.toml
```toml
[dependencies]
ort = { version = "2.0.0-rc.10", features = ["download-binaries"] }

# Change to:
ort = { version = "2.0.0-rc.10", default-features = false }
```

### 5. Enable CoreML EP in Code
Update `crates/akidb-embedding/src/onnx.rs`:
```rust
let session = SessionBuilder::new(&env)?
    .with_optimization_level(GraphOptimizationLevel::Level3)?
    .with_execution_providers([
        CoreMLExecutionProvider::default().build(),  // Add this
        CPUExecutionProvider::default().build(),
    ])?
    .with_model_from_file(model_path)?;
```

### 6. Test Performance
```bash
# Run integration test
cargo test -p akidb-embedding --features onnx test_onnx -- --nocapture

# Expected results:
# - P50: ~9ms
# - P95: ~10ms (vs current 43ms CPU-only)
# - P99: ~11ms
```

---

## Alternative: Path B (Python Bridge)

If Path A build encounters further issues, **Path B is fully implemented and ready**:

### Status
✅ **Python Bridge Provider**: Fully functional
- File: `crates/akidb-embedding/src/python_bridge.rs` (320 lines)
- Server: `crates/akidb-embedding/python/onnx_server.py` (270 lines)
- Protocol: JSON-RPC over stdin/stdout IPC
- Performance: Expected ~15ms P95 (validated in Day 2 Python tests)

### Quick Start Path B
```bash
# 1. Enable python-bridge feature
cargo build -p akidb-embedding --features python-bridge

# 2. Install Python dependencies
pip install onnxruntime transformers optimum[onnxruntime]

# 3. Convert model to ONNX
./scripts/convert-model-to-onnx.sh

# 4. Test
cargo test -p akidb-embedding --features python-bridge
```

---

## Success Criteria

### Path A Success
- ✅ libonnxruntime.dylib built successfully
- ✅ CoreML EP symbols present in library
- ✅ Rust code compiles with system ONNX Runtime
- ✅ P95 latency < 20ms (target: ~10ms)
- ✅ >95% recall maintained

### Fallback to Path B
If any of the following occur:
- Build fails after 1 hour
- CoreML EP not available in built library
- Compilation errors with system library
- Performance doesn't improve (<40ms P95)

→ Switch to Path B (Python bridge) which is production-ready

---

## Timeline

| Phase | Duration | Status |
|-------|----------|--------|
| Root cause diagnosis | 5 min | ✅ Complete |
| Apply CMake patch | 1 min | ✅ Complete |
| ONNX Runtime build | 20-30 min | 🔄 In Progress |
| Environment setup | 5 min | ⏳ Pending |
| Code integration | 10 min | ⏳ Pending |
| Performance testing | 10 min | ⏳ Pending |
| **Total** | **50-60 min** | **🔄 Active** |

---

## Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Build fails with new error | Low (20%) | Medium | Path B ready |
| CoreML EP not in binary | Very Low (5%) | High | Verify build flags |
| Performance not improved | Very Low (5%) | Medium | Debug EP activation |
| Integration issues | Low (15%) | Low | Clear examples exist |

**Overall Confidence**: 80% Path A succeeds, 95% overall success (with Path B fallback)

---

## Key Decisions

### Why Path A is Preferred
1. **Native Performance**: No IPC overhead, direct Rust ↔ ONNX
2. **Deployment Simplicity**: Single binary, no Python runtime
3. **Memory Efficiency**: No subprocess, shared memory
4. **Type Safety**: Compile-time guarantees
5. **Expected Performance**: ~10ms P95 (vs 15ms with Path B)

### Why Path B is Valuable
1. **Proven to Work**: Day 2 validation showed 10ms P95 in Python
2. **Quick to Deploy**: Already implemented and compiles
3. **Reliable Fallback**: If Path A encounters issues
4. **Still Meets Goals**: 15ms < 20ms target

---

## Collaboration Notes

### Backend Agent Involvement
- **Agent Task**: Investigate ONNX build failure
- **Status**: Running in parallel (78ef1d, 0282f1)
- **Discovery**: Human identified root cause faster via direct log inspection
- **Outcome**: Fix applied before agent completed analysis
- **Value**: Agent provides comprehensive analysis for documentation

### Human-Agent Synergy
- Human: Quick diagnosis via error logs → Immediate fix
- Agent: Comprehensive investigation → Thorough documentation
- Result: Faster resolution + Better understanding

---

## References

- **Build Script**: `scripts/build-onnxruntime-coreml.sh`
- **ONNX Provider**: `crates/akidb-embedding/src/onnx.rs`
- **Python Bridge**: `crates/akidb-embedding/src/python_bridge.rs`
- **Day 2 Validation**: `automatosx/tmp/SESSION-COMPLETION-SUMMARY.md`
- **Strategic Planning**: `automatosx/tmp/DAY-3-FINAL-MEGATHINK.md`

---

**Next Update**: After build completes (~20-30 minutes)
