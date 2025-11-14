# Path A Ultrathink: Exhaustive Analysis of Remaining Build Options

**Date:** November 11, 2025
**Status:** 11 attempts failed, exploring all remaining strategies
**Goal:** Build ONNX Runtime v1.16.3 with CoreML EP for ~10ms P95 embedding inference

---

## Current Situation

**Attempt #11 Status:**
- ✅ Reached 75% compilation (best progress)
- ❌ Failed on `inverse.cc:62` - Eigen 3.4.0 FP16 NEON `pset1` type mismatch
- Error: `pset1<PacketType>(0)` expects `Eigen::half`, got `int` literal

**Eigen Version Catch-22 Confirmed:**
- 5.0.0: Too new (removed `operator^(bool)` at 66%)
- 3.4.0: Incompatible (incomplete FP16 at 75%)
- 3.3.7: Too old (missing Half.h at 53%)

---

## Remaining Strategy Options (Exhaustive)

### Strategy #5: Patch Eigen 3.4.0 FP16 Type Conversion

**Effort:** 1-2 hours
**Risk:** Medium
**Feasibility:** ✅ **VIABLE** - isolated fix

#### Analysis

The error is a **simple type conversion issue** in Eigen's ARM NEON vectorization:

```cpp
// Eigen/src/Core/PartialReduxEvaluator.h:57
template<typename PacketType, typename Func>
PacketType packetwise_redux_empty_value(const Func&) {
  return pset1<PacketType>(0);  // ❌ int literal
}

// GenericPacketMath.h:615 - Candidate function
template<typename Packet>
Packet pset1(const typename unpacket_traits<Packet>::type& a) {
  return a;
}
// Expects: const Eigen::half& for FP16 NEON vectors
// Gets: int (literal 0)
```

#### Proposed Fix

**Option 5A: Add Overload for Integer Literals**

Create patch file `/tmp/eigen-fp16-pset1.patch`:
```cpp
// Insert into Eigen/src/Core/GenericPacketMath.h after line 615

// Overload for integer literals in FP16 context
template<typename Packet>
typename std::enable_if<
  std::is_same<typename unpacket_traits<Packet>::type, Eigen::half>::value,
  Packet
>::type
pset1(int a) {
  return pset1<Packet>(static_cast<Eigen::half>(a));
}
```

**Pros:**
- ✅ Minimal change (3 lines)
- ✅ Type-safe (SFINAE ensures only FP16 paths)
- ✅ No impact on other Eigen operations
- ✅ Matches Eigen's existing pattern for other types

**Cons:**
- ⚠️ Non-standard Eigen modification
- ⚠️ Needs verification with Eigen test suite

**Implementation Steps:**
1. Apply patch to Conda Eigen 3.4.0:
   ```bash
   cd ~/miniconda3/envs/onnx-build/include/eigen3
   patch -p0 < /tmp/eigen-fp16-pset1.patch
   ```

2. Verify patch doesn't break Eigen:
   ```bash
   # Quick smoke test - compile simple FP16 program
   cat > /tmp/test_eigen_fp16.cpp << 'EOF'
   #include <Eigen/Dense>
   #include <iostream>
   int main() {
     Eigen::Matrix<Eigen::half, 4, 4> m;
     m.setZero();
     std::cout << m.sum() << std::endl;
     return 0;
   }
   EOF
   clang++ -std=c++17 -I ~/miniconda3/envs/onnx-build/include/eigen3 \
           /tmp/test_eigen_fp16.cpp -o /tmp/test_eigen_fp16
   /tmp/test_eigen_fp16
   ```

3. Rebuild ONNX Runtime (same Attempt #11 command)

4. Monitor build past 75%

**Estimated Success Rate:** 70% - error is isolated, fix is minimal

---

**Option 5B: Cast in ONNX Runtime Instead**

Patch ONNX Runtime's `inverse.cc` instead:
```cpp
// onnxruntime/contrib_ops/cpu/inverse.cc:62
// Change:
output_matrix = input_matrix.inverse();

// To:
if constexpr (std::is_same_v<T, Eigen::half>) {
  // Workaround Eigen 3.4.0 FP16 NEON bug
  Eigen::Matrix<float, -1, -1> tmp = input_matrix.template cast<float>();
  output_matrix = tmp.inverse().template cast<Eigen::half>();
} else {
  output_matrix = input_matrix.inverse();
}
```

**Pros:**
- ✅ No Eigen modification
- ✅ Clear workaround comment
- ✅ Minimal performance impact (matrix inverse is already expensive)

**Cons:**
- ⚠️ Affects ONNX Runtime source (harder to maintain across upgrades)
- ⚠️ Requires C++17 `if constexpr`

**Estimated Success Rate:** 80% - safer than patching Eigen

---

### Strategy #6: Use ONNX Runtime's Bundled Eigen

**Effort:** 30 minutes
**Risk:** Low
**Feasibility:** ✅ **VIABLE** - official build path

#### Analysis

ONNX Runtime's build script can **download and build its own Eigen** from a known-good commit:

```bash
# Check ONNX Runtime's CMakeLists.txt for Eigen dependency
grep -A 10 "eigen" ~/onnxruntime-build/cmake/external/eigen.cmake
```

ONNX Runtime v1.16.3 likely pins a specific Eigen commit that works.

#### Implementation

**Attempt #12: Let ONNX Runtime Manage Eigen**

```bash
cd ~/onnxruntime-build
rm -rf build/

./build.sh \
  --config=Release \
  --use_coreml \
  --build_shared_lib \
  --parallel \
  --skip_tests \
  --cmake_extra_defines \
    CMAKE_OSX_ARCHITECTURES=arm64 \
    CMAKE_OSX_DEPLOYMENT_TARGET=11.0 \
    CMAKE_POLICY_VERSION_MINIMUM=3.5 \
    'CMAKE_CXX_FLAGS=-Wno-error=deprecated-declarations' \
  2>&1 | tee /tmp/onnx-build-bundled-eigen.log
```

**Key Change:** Remove `--use_preinstalled_eigen` and `--eigen_path`

**Expected Behavior:**
- ONNX build script downloads Eigen from GitLab
- Uses commit hash that Microsoft tested with ONNX Runtime v1.16.3
- Should be between 3.3.7 and 3.4.0 (likely 3.3.9 or specific commit)

**Pros:**
- ✅ Official build path (Microsoft uses this internally)
- ✅ Zero patching required
- ✅ Known to work (used in CI/CD)
- ✅ Low risk

**Cons:**
- ⚠️ Downloads dependencies (adds 5-10 minutes to build time)
- ⚠️ Might fail if network issues

**Estimated Success Rate:** 85% - this is the "blessed" configuration

---

### Strategy #7: Disable FP16 Support in ONNX Runtime

**Effort:** 1 hour
**Risk:** Medium
**Feasibility:** ✅ **VIABLE** - reduces optimization but should work

#### Analysis

The error occurs because ONNX Runtime enables FP16 (half-precision) operations for ARM NEON. We can disable this:

```bash
cmake -DONNX_USE_HALF=OFF \
      -DENABLE_FP16_KERNELS=OFF \
      ...
```

**Impact on Performance:**
- Neural Engine (ANE) prefers FP16 for optimal throughput
- Disabling FP16 forces FP32 → ~10-20% slower inference
- Still better than CPU-only (43ms), estimated ~12-15ms P95

**Trade-off Analysis:**
| Config | P95 Latency | Complexity | Risk |
|--------|-------------|------------|------|
| FP16 enabled | ~10ms | High | High (current blocker) |
| FP16 disabled | ~12-15ms | Low | Low |
| Path B (Python) | ~15ms | Zero | Zero |

**Conclusion:** If FP16 disabled ≈ Path B performance, might as well use Path B (easier).

#### Implementation

**Attempt #13: Build without FP16**

```bash
cd ~/onnxruntime-build
rm -rf build/

./build.sh \
  --config=Release \
  --use_coreml \
  --build_shared_lib \
  --parallel \
  --skip_tests \
  --cmake_extra_defines \
    CMAKE_OSX_ARCHITECTURES=arm64 \
    CMAKE_OSX_DEPLOYMENT_TARGET=11.0 \
    ONNX_USE_HALF=OFF \
    'CMAKE_CXX_FLAGS=-Wno-error=deprecated-declarations' \
  2>&1 | tee /tmp/onnx-build-no-fp16.log
```

**Estimated Success Rate:** 90% - removes the blocker entirely

**ROI:** ❓ Questionable - similar performance to Path B but more effort

---

### Strategy #8: Try ONNX Runtime v1.15.x

**Effort:** 45 minutes
**Risk:** Medium
**Feasibility:** ✅ **VIABLE** - older version might have looser Eigen requirements

#### Analysis

ONNX Runtime v1.15.1 (May 2023) predates Eigen 3.4.0 release issues:
- Released: May 2023
- Eigen 3.4.0: December 2021
- Likely tested with Eigen 3.3.x series

#### Implementation

**Attempt #14: ONNX Runtime v1.15.1**

```bash
cd ~/onnxruntime-build
git fetch --tags
git checkout v1.15.1
rm -rf build/

./build.sh \
  --config=Release \
  --use_coreml \
  --build_shared_lib \
  --parallel \
  --skip_tests \
  --cmake_extra_defines \
    CMAKE_OSX_ARCHITECTURES=arm64 \
    CMAKE_OSX_DEPLOYMENT_TARGET=11.0 \
  2>&1 | tee /tmp/onnx-build-v1.15.1.log
```

**Estimated Success Rate:** 60% - might have different blockers

---

### Strategy #9: Check ONNX Runtime Issue Tracker

**Effort:** 30 minutes
**Risk:** Low
**Feasibility:** ✅ **SHOULD DO** - community might have solved this

#### Analysis

Search for existing issues/PRs related to:
- Eigen FP16 compilation errors
- CoreML build on macOS ARM
- inverse.cc compilation failures

#### Implementation

```bash
# Search GitHub issues
open "https://github.com/microsoft/onnxruntime/issues?q=eigen+fp16+arm"
open "https://github.com/microsoft/onnxruntime/issues?q=inverse.cc+compilation"
open "https://github.com/microsoft/onnxruntime/issues?q=coreml+build+mac"
```

**Possible Findings:**
- Existing patch/workaround
- Recommended Eigen version/commit
- Build flag to skip problematic operators

**Estimated Success Rate:** 30% - worth checking but no guarantees

---

### Strategy #10: Disable Inverse Operator

**Effort:** 2 hours
**Risk:** High
**Feasibility:** ⚠️ **RISKY** - might break model compatibility

#### Analysis

The error is in `contrib_ops/cpu/inverse.cc` - a **contrib operator** (not core ONNX spec).

We could disable it:
```bash
cmake -DONNXRUNTIME_DISABLE_CONTRIB_OPS=ON
```

**Risk:**
- ❌ Inverse operator used in some ML models
- ❌ Might break BERT/transformer embeddings
- ❌ Hard to debug which models need it

**Estimated Success Rate:** 50% build, 20% works with our models

**Recommendation:** ❌ Too risky - defeats purpose if models don't work

---

## Recommended Strategy Ranking

| Rank | Strategy | Effort | Success Rate | ROI | Recommendation |
|------|----------|--------|--------------|-----|----------------|
| 🥇 **1** | **#6: Use ONNX Bundled Eigen** | 30 min | **85%** | ✅ **High** | ✅ **TRY THIS FIRST** |
| 🥈 2 | #5B: Patch ONNX inverse.cc | 1 hour | 80% | Medium | ✅ Try if #6 fails |
| 🥉 3 | #5A: Patch Eigen pset1 | 1-2 hours | 70% | Medium | ⚠️ Backup option |
| 4 | #7: Disable FP16 | 1 hour | 90% | Low | ⚠️ Similar perf to Path B |
| 5 | #9: Check Issue Tracker | 30 min | 30% | Low | ✅ Quick research |
| 6 | #8: Try v1.15.1 | 45 min | 60% | Low | ⚠️ Might have other issues |
| 7 | #10: Disable Inverse Op | 2 hours | 20% | Very Low | ❌ Too risky |

---

## Time-Boxed Execution Plan

### Phase 1: Quick Wins (1 hour max)

1. **Strategy #9: Check Issue Tracker (15 min)**
   - Search GitHub for known fixes
   - If found: apply and rebuild
   - If not found: proceed to #6

2. **Strategy #6: Use ONNX Bundled Eigen (30 min)**
   - Remove `--use_preinstalled_eigen` flag
   - Let ONNX download its own Eigen
   - Monitor build progress
   - **Decision Point:**
     - ✅ Success → Test performance, DONE
     - ❌ Fails at <75% → Different blocker, analyze
     - ❌ Fails at 75% (same error) → Proceed to Phase 2

### Phase 2: Patching (2 hours max)

3. **Strategy #5B: Patch ONNX inverse.cc (1 hour)**
   - Modify `inverse.cc` to cast FP16→FP32→FP16
   - Rebuild with Conda Eigen 3.4.0
   - **Decision Point:**
     - ✅ Success → Test performance, DONE
     - ❌ Fails → Proceed to #5A

4. **Strategy #5A: Patch Eigen pset1 (1 hour)**
   - Add integer literal overload
   - Test with simple program
   - Rebuild ONNX Runtime
   - **Decision Point:**
     - ✅ Success → Test performance, DONE
     - ❌ Fails → Path A exhausted

### Phase 3: Fallback (30 min)

5. **Deploy Path B: Python ONNX Runtime**
   - Install `onnxruntime-coreml` wheel
   - Implement PyO3 bridge
   - Benchmark

---

## Success Criteria

**Path A is worth continuing if:**
- ✅ Build completes successfully
- ✅ `libonnxruntime.dylib` has CoreML EP
- ✅ P95 ≤ 15ms (better than or equal to Path B)
- ✅ Integration with Rust FFI works
- ✅ Total time investment ≤ 3 additional hours

**Path A should be abandoned if:**
- ❌ Strategy #6 fails with same error (indicates systemic issue)
- ❌ Total time exceeds 8 hours (5 already spent + 3 max)
- ❌ Performance similar to Path B (15ms)
- ❌ New blockers emerge after fixing current one

---

## Risk Assessment

### Strategy #6 (Bundled Eigen) Risks

**Low Risk:**
- ✅ Official Microsoft build path
- ✅ Used in CI/CD pipelines
- ✅ Known to work on macOS ARM (Microsoft tests this)

**Potential Issues:**
- ⚠️ Network download might fail (solution: retry)
- ⚠️ Might use Eigen 3.3.x (could hit Half.h error again)
- ⚠️ Build time increases by 5-10 minutes

**Mitigation:**
- If fails with Eigen 3.3.x error, move directly to Strategy #5B (patch ONNX)

### Strategy #5B (Patch ONNX) Risks

**Medium Risk:**
- ⚠️ Modifies ONNX source (needs tracking for upgrades)
- ⚠️ Might have performance impact (~5-10% on inverse ops)

**Mitigation:**
- Document patch location clearly
- Add comment explaining workaround
- Benchmark to quantify performance impact

### Strategy #5A (Patch Eigen) Risks

**Medium-High Risk:**
- ⚠️ Non-standard Eigen modification
- ⚠️ Could break other NEON vectorization
- ⚠️ Hard to maintain across Eigen upgrades

**Mitigation:**
- Test patch with Eigen's own test suite
- Use SFINAE to ensure type safety
- Consider upstreaming to Eigen (if it works)

---

## Cost-Benefit Analysis (Updated)

### Path A with Strategy #6
**Time Investment:**
- ✅ Sunk: 5 hours
- ⏳ Additional: 1-3 hours (strategies #6, #5B, #5A)
- **Total:** 6-8 hours

**Benefits:**
- ⭐ P95: 10-12ms (vs 15ms Path B)
- ⭐ Direct FFI (no Python bridge)
- ⭐ Full control over build

**ROI:** ✅ Positive IF Strategy #6 succeeds quickly

### Path B (Unchanged)
**Time Investment:**
- ⏳ 1-2 hours (PyO3 integration)

**Benefits:**
- ⭐ P95: 15ms (meets <20ms SLA)
- ⭐ Zero build maintenance
- ⭐ Official support

**ROI:** ✅ Strongly Positive (low risk, reliable)

---

## Final Recommendation

### Recommended Path: Try Strategy #6 First

**Rationale:**
1. **High Success Probability (85%)** - Microsoft's official build path
2. **Low Time Investment (30 min)** - quick to test
3. **Low Risk** - no patching required
4. **Clear Decision Point** - if it fails, we know for certain Path A is blocked

**Execution:**
1. ⏰ **Now:** Start Attempt #12 with bundled Eigen (Strategy #6)
2. ⏰ **+30 min:** Evaluate result
   - ✅ Success → Test FFI integration
   - ❌ Fails (same error) → Deploy Path B immediately
   - ❌ Fails (different error) → Analyze, consider Strategy #5B

3. ⏰ **+90 min:** If Strategy #5B in progress, hard stop
   - Even if close to working, diminishing returns
   - Deploy Path B for guaranteed results

**Time-Box:** Maximum 3 additional hours total for Path A

**Fallback:** Path B is production-ready, meets SLA, zero-risk option

---

## Appendix: Quick Reference Commands

### Attempt #12: ONNX Bundled Eigen
```bash
cd ~/onnxruntime-build
rm -rf build/
./build.sh \
  --config=Release \
  --use_coreml \
  --build_shared_lib \
  --parallel \
  --skip_tests \
  --cmake_extra_defines \
    CMAKE_OSX_ARCHITECTURES=arm64 \
    CMAKE_OSX_DEPLOYMENT_TARGET=11.0 \
    CMAKE_POLICY_VERSION_MINIMUM=3.5 \
    'CMAKE_CXX_FLAGS=-Wno-error=deprecated-declarations' \
  2>&1 | tee /tmp/onnx-build-bundled-eigen.log
```

### Monitor Progress
```bash
tail -f /tmp/onnx-build-bundled-eigen.log | grep -E "^\[[ 0-9]+%\]|error:|Built target"
```

### Check for Success
```bash
ls -lh ~/onnxruntime-build/build/MacOS/Release/libonnxruntime*.dylib
nm ~/onnxruntime-build/build/MacOS/Release/libonnxruntime.dylib | grep -i coreml
```

---

**Status:** Ready to execute Strategy #6 (ONNX Bundled Eigen)
**Expected Time:** 30 minutes
**Expected Success Rate:** 85%
**Fallback Ready:** Path B deployment plan documented
