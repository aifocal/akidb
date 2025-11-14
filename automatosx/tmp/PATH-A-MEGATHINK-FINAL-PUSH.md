# Path A Megathink: Final Push with AutomatosX Agent Assistance

**Date:** November 11, 2025
**Status:** 12 attempts failed, exploring agent-assisted solutions
**Goal:** Exhaust ALL Path A possibilities before deploying Path B

---

## Current Situation Analysis

### Attempts 1-12 Failure Summary

| # | Strategy | Eigen Version | Failed At | Blocker Type |
|---|----------|---------------|-----------|--------------|
| 1-5 | Homebrew Eigen | 5.0.0 | 66% | API incompatibility (XOR) |
| 6 | Different ONNX version | 5.0.0 | 17% | Protobuf namespace |
| 7-9 | macOS SDK tuning | 5.0.0 | 30-59% | SDK version mismatch |
| 10 | Manual Eigen | 3.3.7 | 53% | Missing APIs (too old) |
| 11 | Conda Eigen | 3.4.0 | 75% | FP16 NEON bug |
| **12** | **ONNX Bundled** | **(download)** | **CMake** | **SHA1 hash mismatch** |

### Key Insight from Attempt #12

**Critical Finding:** ONNX Runtime's official build path (bundled Eigen) FAILED due to:
```
Expected SHA1: be8be39fdbc6e60e94fa7870b280707069b5b81a
Actual SHA1:   32b145f525a8308d7ab1c09388b2e288312d8eba
Source: https://gitlab.com/libeigen/eigen/-/archive/e7248b26.../eigen-e7248b26....zip
```

**Root Cause:** GitLab's archive generation is non-deterministic. Same commit produces different ZIP files with different SHA1 hashes each time.

**Significance:** This means Microsoft's own build system might be broken or uses a cached/mirrored Eigen source.

---

## Unexplored Strategies (Agent-Assisted)

### Strategy #7: Bypass SHA1 Verification

**Approach:** Modify ONNX Runtime's CMake files to skip Eigen SHA1 check

**Implementation:**
```cmake
# File: /Users/akiralam/onnxruntime-build/cmake/external/eigen.cmake
# Find line with:
URL_HASH SHA1=be8be39fdbc6e60e94fa7870b280707069b5b81a

# Change to:
# URL_HASH SHA1=be8be39fdbc6e60e94fa7870b280707069b5b81a  # Disabled - GitLab non-deterministic
```

**Or use CMAKE flag:**
```bash
cmake -DCMAKE_TLS_VERIFY=OFF ...
```

**Estimated Success Rate:** 60% - might work if Eigen download succeeds
**Time:** 30 minutes
**Risk:** Medium - downloaded Eigen might still have FP16 bug

---

### Strategy #8: Use Local Eigen Clone

**Approach:** Clone Eigen from GitLab at the exact commit ONNX expects, bypass download

**Implementation:**
```bash
# Find commit hash from ONNX CMake files
EIGEN_COMMIT=e7248b26a1ed53fa030c5c459f7ea095dfd276ac

# Clone Eigen locally
cd ~/
git clone https://gitlab.com/libeigen/eigen.git eigen-local
cd eigen-local
git checkout $EIGEN_COMMIT

# Modify ONNX CMake to use local path
# In cmake/external/eigen.cmake, change from:
FetchContent_Declare(eigen
  URL https://gitlab.com/libeigen/eigen/-/archive/${EIGEN_COMMIT}/eigen-${EIGEN_COMMIT}.zip
  URL_HASH SHA1=...
)

# To:
FetchContent_Declare(eigen
  SOURCE_DIR /Users/akiralam/eigen-local
)
```

**Estimated Success Rate:** 70% - bypasses download issue entirely
**Time:** 45 minutes
**Risk:** Medium - still might hit FP16 bug if commit has it

---

### Strategy #9: Check for Prebuilt ARM Binaries

**Approach:** Microsoft might provide official macOS ARM builds with CoreML

**Research Needed:**
- Check https://github.com/microsoft/onnxruntime/releases
- Look for `onnxruntime-osx-arm64-coreml-*.tgz` artifacts
- Check conda-forge for `onnxruntime-coreml` binary packages
- Investigate Microsoft's NuGet packages

**Estimated Success Rate:** 40% - might not have CoreML builds
**Time:** 30 minutes
**Risk:** Low - if found, it's production-ready

---

### Strategy #10: Search ONNX Runtime Issue Tracker

**Approach:** Check if others have solved this exact problem

**Search Queries:**
1. "eigen sha1 mismatch" site:github.com/microsoft/onnxruntime
2. "build coreml macos arm64" site:github.com/microsoft/onnxruntime
3. "eigen fp16 inverse" site:github.com/microsoft/onnxruntime
4. "build from source arm64" label:bug site:github.com/microsoft/onnxruntime

**Estimated Success Rate:** 30% - community might have workarounds
**Time:** 30 minutes
**Risk:** Low - just research

---

### Strategy #11: Use ONNX Runtime Docker Build

**Approach:** Microsoft maintains Docker images for building ONNX Runtime

**Implementation:**
```bash
# Check for official build containers
docker search onnxruntime

# Use official build environment
docker run -it --rm \
  -v ~/onnxruntime-build:/workspace \
  mcr.microsoft.com/onnxruntime/build:latest \
  bash -c "cd /workspace && ./build.sh --use_coreml ..."
```

**Estimated Success Rate:** 50% - might not support macOS-specific builds
**Time:** 1 hour
**Risk:** Medium - Docker might not have macOS SDKs

---

### Strategy #12: Patch Eigen FP16 Locally (Agent-Guided)

**Approach:** Let ax agent analyze Eigen source and create targeted patch

**What Agent Should Do:**
1. Read Eigen commit `e7248b26` source code
2. Identify FP16 NEON `pset1` implementation
3. Generate minimal patch to fix type conversion
4. Test patch compiles without breaking other code

**Estimated Success Rate:** 65% - agent can do targeted analysis
**Time:** 1-2 hours
**Risk:** Medium - requires careful patch validation

---

## Agent Task Specification

### Task for ax Backend Agent

**Objective:** Fix ONNX Runtime v1.16.3 CoreML build on macOS ARM

**Context:**
- 12 build attempts failed over 6+ hours
- Attempt #11: Failed at 75% with Eigen 3.4.0 FP16 `pset1` error
- Attempt #12: Failed at CMake with Eigen download SHA1 mismatch
- Current barrier: GitLab's non-deterministic ZIP generation

**Agent Mission:**

1. **Priority 1: Fix SHA1 Mismatch (Strategy #7-#8)**
   - Analyze `/Users/akiralam/onnxruntime-build/cmake/external/eigen.cmake`
   - Determine safest way to bypass SHA1 check OR use local Eigen clone
   - Provide exact CMake modification commands
   - Expected time: 30 minutes

2. **Priority 2: Research Prebuilt Binaries (Strategy #9)**
   - Search Microsoft's release artifacts for macOS ARM + CoreML builds
   - Check conda-forge, PyPI, Homebrew for prebuilt libraries
   - If found, provide installation instructions
   - Expected time: 20 minutes

3. **Priority 3: Search Issue Tracker (Strategy #10)**
   - Search ONNX Runtime GitHub issues for:
     - "eigen sha1" OR "hash mismatch"
     - "build coreml arm64" OR "apple silicon"
     - "fp16 inverse" OR "half precision neon"
   - Extract any workarounds or patches mentioned
   - Expected time: 15 minutes

4. **Priority 4: Patch Eigen FP16 (Strategy #12)**
   - Clone Eigen at commit `e7248b26a1ed53fa030c5c459f7ea095dfd276ac`
   - Analyze `Eigen/src/Core/PartialReduxEvaluator.h:57`
   - Generate minimal patch for `pset1<PacketType>(0)` type conversion
   - Test patch compiles on small example
   - Expected time: 1 hour

**Success Criteria:**
- At least one viable path to successful build identified
- Concrete commands/patches provided (not just theory)
- Risk assessment for each approach

**Deliverables:**
1. Analysis report: `automatosx/tmp/AGENT-PATH-A-FIX-ANALYSIS.md`
2. If patches found: Patch files in `automatosx/tmp/patches/`
3. If prebuilt found: Installation script

**Time Limit:** 2 hours max for agent investigation

---

## Decision Tree Post-Agent Analysis

```
Agent completes investigation
│
├─ Found prebuilt binaries (Priority 2)?
│  ├─ Yes → Install and test (15 min)
│  │       └─ Success → DONE (Path A works!)
│  │       └─ Failure → Continue tree
│  └─ No → Continue tree
│
├─ Found SHA1 bypass (Priority 1)?
│  ├─ Yes → Apply fix and rebuild (Attempt #13)
│  │       └─ Success @ >75% → DONE (Path A works!)
│  │       └─ Fails @ 75% (FP16) → Try Priority 4 patch
│  │       └─ Fails @ <75% (new error) → Analyze
│  └─ No → Continue tree
│
├─ Found issue tracker workaround (Priority 3)?
│  ├─ Yes → Apply workaround and rebuild
│  │       └─ Success → DONE (Path A works!)
│  │       └─ Failure → Continue tree
│  └─ No → Continue tree
│
└─ Found Eigen FP16 patch (Priority 4)?
   ├─ Yes → Apply patch and rebuild
   │       └─ Success → DONE (Path A works!)
   │       └─ Failure → Path A EXHAUSTED
   └─ No → Path A EXHAUSTED
```

**If Path A exhausted:** Deploy Path B (Python ONNX Runtime)

---

## Expected Outcomes

### Best Case (30% probability)
- Agent finds prebuilt ARM+CoreML binaries
- Install in 15 minutes
- Test confirms <20ms P95
- **Total time: 2.5 hours (agent) + 0.25 hours (test) = 2.75 hours**

### Good Case (40% probability)
- Agent fixes SHA1 mismatch
- Attempt #13 builds to completion
- Might hit FP16 error at 75%, but agent's patch fixes it
- **Total time: 2.5 hours (agent) + 1 hour (build+patch) = 3.5 hours**

### Acceptable Case (20% probability)
- Agent finds workaround in issue tracker
- Requires manual intervention but clear path
- **Total time: 2.5 hours (agent) + 1.5 hours (implement) = 4 hours**

### Worst Case (10% probability)
- Agent exhausts all strategies
- No viable path found
- **Total time: 2.5 hours (agent) + 0 = 2.5 hours wasted**
- **Action: Deploy Path B immediately**

---

## Cost-Benefit Analysis (Updated)

### Path A with Agent Assistance

**Investment:**
- ✅ Sunk: 6 hours (attempts 1-12)
- ⏳ Agent investigation: 2-2.5 hours
- ⏳ Implementation: 0.5-1.5 hours
- **Total: 8.5-10 hours**

**Benefits if successful:**
- ⭐ P95: 10-12ms (vs 15ms Path B)
- ⭐ Direct FFI (no Python bridge)
- ⭐ Full control over build

**ROI Analysis:**
- Time premium: 7-8 hours vs Path B (2 hours)
- Performance gain: 3-5ms (20-33% better)
- **Cost per ms:** ~2 hours/ms improvement
- **Verdict:** Marginal ROI unless passion project

### Path B (Unchanged)

**Investment:**
- ⏳ 1-2 hours (PyO3 integration)

**Benefits:**
- ⭐ P95: 15ms (meets <20ms SLA)
- ⭐ Zero build maintenance
- ⭐ Official support

**ROI:** ✅ Strongly positive (low risk, proven)

---

## Recommendation

### Execute Agent-Assisted Investigation (Time-Boxed)

**Rationale:**
1. **Attempt #12's failure is different** - not Eigen incompatibility, but download issue
2. **Microsoft likely has workarounds** - their CI/CD must build successfully
3. **2.5 hours is acceptable** - not excessive given 6 hours already invested
4. **Clear decision point** - agent results determine next action

**Time-Box:**
- **Agent investigation:** Maximum 2.5 hours
- **Implementation if viable:** Maximum 1.5 hours
- **Hard stop:** 4 hours total

**If agent finds viable solution:** Implement Attempt #13
**If agent exhausts options:** Deploy Path B immediately (no further attempts)

---

## Agent Invocation Command

```bash
ax run backend "Fix ONNX Runtime v1.16.3 CoreML build on macOS ARM after 12 failed attempts.

**Context:**
- Goal: Build ONNX Runtime with CoreML Execution Provider for <20ms embedding inference
- Platform: macOS ARM (Apple Silicon)
- Blockers: Eigen version incompatibility + download SHA1 mismatch
- Time invested: 6 hours, 12 attempts

**Attempt #12 Failure:**
File: cmake/external/eigen.cmake
Error: Eigen download SHA1 mismatch
  Expected: be8be39fdbc6e60e94fa7870b280707069b5b81a
  Actual: 32b145f525a8308d7ab1c09388b2e288312d8eba
  URL: https://gitlab.com/libeigen/eigen/-/archive/e7248b26a1ed53fa030c5c459f7ea095dfd276ac/eigen-e7248b26a1ed53fa030c5c459f7ea095dfd276ac.zip

**Attempt #11 Failure:**
File: onnxruntime/contrib_ops/cpu/inverse.cc:62
Error: no matching function for call to 'pset1'
  Eigen 3.4.0 FP16 NEON bug: pset1<PacketType>(0) expects Eigen::half, got int

**Your Mission (Priority Order):**

1. **Fix SHA1 Mismatch** (30 min)
   - Analyze /Users/akiralam/onnxruntime-build/cmake/external/eigen.cmake
   - Provide patch to bypass SHA1 check OR use local Eigen clone
   - Include exact commands to apply fix

2. **Search for Prebuilt Binaries** (20 min)
   - Check Microsoft's GitHub releases for macOS ARM + CoreML builds
   - Search conda-forge, PyPI for prebuilt onnxruntime-coreml
   - If found, provide installation commands

3. **Search Issue Tracker** (15 min)
   - GitHub issues: 'eigen sha1' OR 'hash mismatch' OR 'build coreml arm64'
   - Extract any workarounds, patches, or build flags mentioned

4. **Create Eigen FP16 Patch** (1 hour)
   - Clone Eigen commit e7248b26a1ed53fa030c5c459f7ea095dfd276ac
   - Fix pset1<PacketType>(0) type conversion in PartialReduxEvaluator.h:57
   - Provide minimal patch file

**Deliverables:**
- Report: automatosx/tmp/AGENT-PATH-A-FIX-ANALYSIS.md
- Patches: automatosx/tmp/patches/ (if created)
- Installation script: automatosx/tmp/install-prebuilt.sh (if found)

**Success Criteria:**
- At least one concrete, actionable solution identified
- Commands/patches ready to execute (not just theory)

**Time Limit:** 2.5 hours maximum

Report all findings in detail - we've invested 6 hours already, need definitive answer on Path A viability."
```

---

## Status: Ready to Deploy Agent

**Next Action:** Invoke ax backend agent with above specification
**Expected Duration:** 2-2.5 hours
**Decision Point:** After agent completes, evaluate findings and choose:
- Option A: Implement agent's solution (Attempt #13)
- Option B: Deploy Path B (Python bridge)

**Time Budget Remaining:** 4 hours max (including implementation)

---

**Report Generated:** 2025-11-11
**Status:** Awaiting agent deployment approval
