# Day 3 Continuation: Strategic Megathink - ONNX+CoreML Path Forward

**Date**: November 10, 2025
**Session**: Day 3 Evening - Strategic Planning
**Current Status**: ONNX provider complete (CPU-only, 43ms P95)
**Goal**: Achieve <20ms P95 with CoreML Execution Provider

---

## Executive Summary

**Current State**:
- ✅ Rust ONNX provider: 100% functional, all tests passing
- ✅ Embeddings correct: 384-dim, L2 normalized (norm=1.0)
- ⚠️ Performance: 43ms P95 (CPU-only, release mode)
- 🎯 Target: <20ms P95 (ideally ~10ms with CoreML)

**Critical Blocker Identified**:
- Microsoft's prebuilt ONNX Runtime binaries do NOT include CoreML support
- Must compile ONNX Runtime from source to enable CoreML EP
- This is why we're seeing 4.3x slower performance (43ms vs 10ms)

**Decision Point**: We have 4 viable paths forward, each with different trade-offs.

---

## Path Analysis: Four Options for <20ms Performance

### Path A: Build ONNX Runtime with CoreML Support ⭐ RECOMMENDED

**Objective**: Compile ONNX Runtime from source with CoreML EP enabled

**Estimated Time**: 4-6 hours (one-time investment)

**Complexity**: High (C++ build toolchain, CMake, dependencies)

**Expected Performance**: P95 = **10ms** (proven in Day 2 Python validation)

**Pros**:
1. ✅ Best performance (4.3x speedup: 43ms → 10ms)
2. ✅ Production-ready: Single native binary, no Python dependency
3. ✅ Future-proof: Full control over ONNX Runtime configuration
4. ✅ Alignment with industry standard: ONNX Runtime is widely used
5. ✅ Proven approach: Python validation already showed 10ms P95

**Cons**:
1. ❌ Complex build process (C++ toolchain, 20-30 min compile time)
2. ❌ Potential build errors on different macOS versions
3. ❌ Need to manage custom binary paths in deployment
4. ❌ CI/CD complexity (need to cache compiled binary or build in CI)

**Detailed Action Plan**:

**Hour 1: Setup Build Environment**
```bash
# Install dependencies
brew install cmake ninja protobuf

# Verify Xcode command line tools
xcode-select --install  # If needed
xcode-select -p  # Should show /Applications/Xcode.app/Contents/Developer

# Clone ONNX Runtime
git clone --recursive https://github.com/microsoft/onnxruntime.git
cd onnxruntime
git checkout v1.16.3  # Match ort crate version
```

**Hour 2-3: Build ONNX Runtime**
```bash
# Build with CoreML support
./build.sh \
  --config Release \
  --use_coreml \
  --build_shared_lib \
  --parallel \
  --skip_tests \
  --cmake_extra_defines \
    CMAKE_OSX_ARCHITECTURES=arm64 \
    CMAKE_OSX_DEPLOYMENT_TARGET=11.0

# Build output location:
# build/MacOS/Release/libonnxruntime.dylib
# build/MacOS/Release/libonnxruntime.*.dylib (versioned)
```

**Expected Issues & Solutions**:
- **Issue**: "CoreML framework not found"
  - **Solution**: Install Xcode (not just command line tools)
- **Issue**: "Protobuf version mismatch"
  - **Solution**: `brew unlink protobuf && brew link protobuf@3.21`
- **Issue**: Build fails on M1/M2
  - **Solution**: Add `--cmake_extra_defines CMAKE_OSX_ARCHITECTURES=arm64`

**Hour 4: Configure ort Crate**

Update `crates/akidb-embedding/Cargo.toml`:
```toml
[dependencies]
# Remove download-binaries, use system build
ort = { version = "2.0.0-rc.10", default-features = false, features = ["coreml"] }
ndarray = { version = "0.16" }
tokenizers = { version = "0.15.0" }
hf-hub = { version = "0.3.2", default-features = false, features = ["tokio", "online"] }
parking_lot = "0.12"

[features]
default = ["onnx"]
onnx = ["ort", "ndarray", "tokenizers", "hf-hub"]
```

Set environment variables (add to `.envrc` or `.zshrc`):
```bash
export ORT_STRATEGY=system
export ORT_DYLIB_PATH=/path/to/onnxruntime/build/MacOS/Release/libonnxruntime.dylib
```

**Hour 5: Update Provider Code**

Update `crates/akidb-embedding/src/onnx.rs`:
```rust
use ort::execution_providers::CoreMLExecutionProvider;

pub async fn new(model_path: &str, tokenizer_path: &str, model_name: &str) -> EmbeddingResult<Self> {
    eprintln!("\n🔧 Initializing ONNX Runtime provider with CoreML EP...");

    let session = Session::builder()
        .map_err(|e| EmbeddingError::Internal(format!("Failed to create session builder: {}", e)))?
        .with_execution_providers([
            CoreMLExecutionProvider::default()
                .with_ane_only()  // Use Apple Neural Engine for best performance
                .build()
                .error_on_failure()  // Fail loudly if CoreML not available
        ])
        .map_err(|e| EmbeddingError::Internal(format!("Failed to configure CoreML EP: {}", e)))?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|e| EmbeddingError::Internal(format!("Failed to set optimization level: {}", e)))?
        .with_intra_threads(4)
        .map_err(|e| EmbeddingError::Internal(format!("Failed to set threads: {}", e)))?
        .commit_from_file(model_path)
        .map_err(|e| EmbeddingError::Internal(format!("Failed to load model: {}", e)))?;

    eprintln!("✅ ONNX model loaded with CoreML EP");

    // ... rest of initialization
}
```

**Hour 6: Test & Document**

```bash
# Test
cargo run --example test_onnx --features onnx --release

# Expected output:
# ⚡ Performance test (10 iterations)...
#    P50: 10ms  ✅
#    P95: 10ms  ✅
#    P99: 11ms  ✅

# Document build process
cat > docs/BUILDING-ONNX-COREML.md <<'EOF'
# Building ONNX Runtime with CoreML Support

## Prerequisites
- macOS 11.0+
- Xcode 13.0+
- Homebrew

## Build Steps
[... detailed steps ...]
EOF
```

**Success Probability**: 70%
- Well-documented process
- Community has done this before
- But: Potential platform-specific issues

**Decision Criteria to Proceed**:
- ✅ We need best possible performance (10ms)
- ✅ We're building for production deployment
- ✅ Team has C++ build experience
- ⚠️ Willing to invest 4-6 hours upfront

---

### Path B: Python Bridge with IPC ⭐ FALLBACK

**Objective**: Wrap Python ONNX+CoreML in subprocess, communicate via JSON/IPC

**Estimated Time**: 2-3 hours

**Complexity**: Medium (process management, IPC, error handling)

**Expected Performance**: P95 = **15ms** (10ms inference + 5ms IPC overhead)

**Pros**:
1. ✅ Simpler than building ONNX Runtime (no C++ toolchain)
2. ✅ Proven approach: We have working Python code from Day 2
3. ✅ Can implement quickly (2-3 hours vs 4-6 hours)
4. ✅ Still meets target (<20ms)
5. ✅ Easy to debug (separate processes)

**Cons**:
1. ❌ IPC overhead (~5ms per request)
2. ❌ More complex deployment (need Python + dependencies)
3. ❌ Process management complexity (startup, shutdown, crashes)
4. ❌ Not as clean as native Rust solution

**Detailed Action Plan**:

**Hour 1: Python Service**

Create `scripts/onnx_embed_service.py`:
```python
#!/usr/bin/env python3
"""
ONNX+CoreML embedding service.
Communicates via JSON over stdin/stdout.
"""
import sys
import json
import numpy as np
import onnxruntime as ort
from transformers import AutoTokenizer

class EmbeddingService:
    def __init__(self, model_path: str, tokenizer_name: str):
        # Create session with CoreML EP
        providers = [
            ('CoreMLExecutionProvider', {
                'MLComputeUnits': 'ALL',
                'ModelFormat': 'MLProgram',
            }),
            'CPUExecutionProvider'
        ]

        self.session = ort.InferenceSession(model_path, providers=providers)
        self.tokenizer = AutoTokenizer.from_pretrained(tokenizer_name)

        # Log to stderr (stdout used for IPC)
        print(f"✅ Service initialized with CoreML EP", file=sys.stderr)

    def embed(self, texts: list[str]) -> dict:
        """Generate embeddings for batch of texts."""
        # Tokenize
        inputs = self.tokenizer(
            texts,
            padding=True,
            truncation=True,
            max_length=512,
            return_tensors="np"
        )

        # Run inference
        outputs = self.session.run(
            None,
            {
                "input_ids": inputs["input_ids"],
                "attention_mask": inputs["attention_mask"],
                "token_type_ids": inputs["token_type_ids"]
            }
        )

        # Mean pooling + L2 normalization
        hidden_states = outputs[0]
        attention_mask = inputs["attention_mask"]

        mask_expanded = np.expand_dims(attention_mask, -1).astype(float)
        sum_embeddings = np.sum(hidden_states * mask_expanded, axis=1)
        sum_mask = np.clip(np.sum(mask_expanded, axis=1), a_min=1e-9, a_max=None)
        embeddings = sum_embeddings / sum_mask

        # L2 normalize
        norms = np.linalg.norm(embeddings, axis=1, keepdims=True)
        embeddings = embeddings / norms

        return {
            "embeddings": embeddings.tolist(),
            "dimension": embeddings.shape[1]
        }

    def run(self):
        """Main service loop - read JSON requests from stdin, write responses to stdout."""
        for line in sys.stdin:
            try:
                request = json.loads(line)

                if request["cmd"] == "embed":
                    result = self.embed(request["texts"])
                    response = {"status": "ok", "data": result}
                elif request["cmd"] == "health":
                    response = {"status": "ok", "data": {"healthy": True}}
                else:
                    response = {"status": "error", "error": f"Unknown command: {request['cmd']}"}

                # Write response
                print(json.dumps(response), flush=True)

            except Exception as e:
                response = {"status": "error", "error": str(e)}
                print(json.dumps(response), flush=True)

if __name__ == "__main__":
    service = EmbeddingService(
        model_path="models/minilm-l6-v2/model.onnx",
        tokenizer_name="sentence-transformers/all-MiniLM-L6-v2"
    )
    service.run()
```

**Hour 2: Rust Bridge**

Create `crates/akidb-embedding/src/python_bridge.rs`:
```rust
use std::process::{Command, Stdio, Child, ChildStdin, ChildStdout};
use std::io::{BufReader, BufRead, Write};
use serde::{Serialize, Deserialize};
use crate::{BatchEmbeddingRequest, BatchEmbeddingResponse, EmbeddingError, EmbeddingResult};

#[derive(Debug, Serialize)]
struct IpcRequest {
    cmd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    texts: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct IpcResponse {
    status: String,
    #[serde(default)]
    data: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<String>,
}

pub struct PythonBridgeProvider {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    model_name: String,
}

impl PythonBridgeProvider {
    pub async fn new(script_path: &str, model_name: &str) -> EmbeddingResult<Self> {
        // Spawn Python service
        let mut child = Command::new("python3")
            .arg(script_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| EmbeddingError::Internal(format!("Failed to spawn Python: {}", e)))?;

        let stdin = child.stdin.take()
            .ok_or_else(|| EmbeddingError::Internal("Failed to get stdin".to_string()))?;
        let stdout = BufReader::new(child.stdout.take()
            .ok_or_else(|| EmbeddingError::Internal("Failed to get stdout".to_string()))?);

        let mut provider = Self {
            child,
            stdin,
            stdout,
            model_name: model_name.to_string(),
        };

        // Health check
        provider.health_check().await?;

        Ok(provider)
    }

    async fn send_request(&mut self, request: &IpcRequest) -> EmbeddingResult<IpcResponse> {
        // Send request
        let request_json = serde_json::to_string(request)
            .map_err(|e| EmbeddingError::Internal(format!("JSON serialize error: {}", e)))?;

        writeln!(self.stdin, "{}", request_json)
            .map_err(|e| EmbeddingError::Internal(format!("Write error: {}", e)))?;

        self.stdin.flush()
            .map_err(|e| EmbeddingError::Internal(format!("Flush error: {}", e)))?;

        // Read response
        let mut response_line = String::new();
        self.stdout.read_line(&mut response_line)
            .map_err(|e| EmbeddingError::Internal(format!("Read error: {}", e)))?;

        let response: IpcResponse = serde_json::from_str(&response_line)
            .map_err(|e| EmbeddingError::Internal(format!("JSON deserialize error: {}", e)))?;

        if response.status != "ok" {
            return Err(EmbeddingError::Internal(
                response.error.unwrap_or_else(|| "Unknown error".to_string())
            ));
        }

        Ok(response)
    }

    pub async fn embed_batch_internal(&mut self, texts: Vec<String>) -> EmbeddingResult<Vec<Vec<f32>>> {
        let request = IpcRequest {
            cmd: "embed".to_string(),
            texts: Some(texts),
        };

        let response = self.send_request(&request).await?;

        // Extract embeddings from response
        let data = response.data.ok_or_else(||
            EmbeddingError::Internal("Missing data in response".to_string()))?;

        let embeddings: Vec<Vec<f32>> = serde_json::from_value(data["embeddings"].clone())
            .map_err(|e| EmbeddingError::Internal(format!("Failed to parse embeddings: {}", e)))?;

        Ok(embeddings)
    }
}

impl Drop for PythonBridgeProvider {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
```

**Hour 3: Testing & Integration**

```bash
# Test Python service standalone
echo '{"cmd":"health"}' | python3 scripts/onnx_embed_service.py

# Test Rust bridge
cargo test -p akidb-embedding python_bridge -- --nocapture

# Run performance test
cargo run --example test_python_bridge --release
# Expected: P95 ~15ms (10ms inference + 5ms IPC)
```

**Success Probability**: 95%
- We already have working Python code
- IPC is straightforward (JSON over stdio)
- Well-understood approach

**Decision Criteria to Proceed**:
- ✅ Need quick solution (2-3 hours vs 4-6 hours)
- ✅ 15ms is acceptable (still under 20ms target)
- ✅ Team comfortable with Python deployment
- ⚠️ Willing to accept IPC overhead

---

### Path C: Fix Candle Metal Support 🔬 RESEARCH NEEDED

**Objective**: Debug and fix the "Metal layer-norm not implemented" error in Candle

**Estimated Time**: 4-8 hours (uncertain, research needed)

**Complexity**: High (requires understanding Candle internals, Metal API)

**Expected Performance**: P95 = **10-15ms** (if Metal GPU works)

**Pros**:
1. ✅ Native Rust solution (no Python, no custom ONNX build)
2. ✅ Candle is designed for Rust ML inference
3. ✅ Might be simpler once working (pure Rust stack)

**Cons**:
1. ❌ Unknown if fixable (Candle might not support BERT layer-norm on Metal)
2. ❌ We already deprecated this approach in Week 1
3. ❌ Performance uncertain (no validation like ONNX)
4. ❌ Candle less mature than ONNX Runtime

**Investigation Required**:

1. **Check Candle GitHub issues** for layer-norm Metal support
2. **Review Candle source** to see if layer-norm Metal kernel exists
3. **Test alternative models** that don't use layer-norm
4. **Contact Candle maintainers** for guidance

**Decision**: ⚠️ **NOT RECOMMENDED**
- We already tried this in Week 1 and hit blocker
- ONNX Runtime is more mature and proven
- Would be exploring uncharted territory
- Background task investigating this was killed (likely not worth pursuing)

---

### Path D: Accept CPU-only Performance 🏁 PRAGMATIC

**Objective**: Ship current implementation, optimize CPU performance

**Estimated Time**: 0 hours (already done) + 2-4 hours optimization

**Complexity**: Low (mostly optimization tweaks)

**Current Performance**: P95 = **43ms** (CPU-only, release mode)

**Optimized Performance**: P95 = **30-35ms** (with CPU optimizations)

**Pros**:
1. ✅ Works right now (zero setup time)
2. ✅ No build complexity
3. ✅ Portable (works on any platform)
4. ✅ Predictable performance

**Cons**:
1. ❌ **MISSES TARGET**: 43ms >> 20ms (2.15x slower)
2. ❌ Not competitive with other vector DBs
3. ❌ Wastes Apple Silicon GPU/ANE capabilities

**Potential Optimizations**:

1. **Reduce Tokenizer Overhead** (~2ms)
   - Cache tokenizer, don't reload per request
   - Use faster tokenization library

2. **Optimize Pooling** (~1-2ms)
   - Use SIMD intrinsics for mean pooling
   - Parallelize across batch dimension

3. **Better Thread Configuration** (~3-5ms)
   - Tune `intra_threads` for Apple Silicon
   - Use `inter_threads` for batch parallelism

4. **Model Quantization** (~5-10ms)
   - Use int8 quantized ONNX model
   - Trade 1-2% accuracy for 2x speed

**Best Case**: 43ms → 30ms (still misses 20ms target)

**Decision**: ❌ **NOT RECOMMENDED**
- Still 50% slower than target
- Doesn't leverage expensive Apple Silicon hardware
- Not competitive solution

---

## Recommendation Matrix

| Path | Time | Complexity | P95 | Success % | Rank |
|------|------|------------|-----|-----------|------|
| **A: Build ONNX+CoreML** | 4-6h | High | **10ms** ✅ | 70% | 🥇 1st |
| **B: Python Bridge** | 2-3h | Medium | **15ms** ✅ | 95% | 🥈 2nd |
| **C: Fix Candle** | 4-8h | High | **10-15ms** | 40% | 4th |
| **D: CPU-only** | 0-4h | Low | **30-43ms** ❌ | 100% | 3rd |

---

## Final Recommendation: Path A with Path B Fallback

### Primary: Path A (Build ONNX Runtime with CoreML)

**Commit to**: 6 hours of focused effort

**Go/No-Go Decision Points**:
- **Hour 2**: If build fails repeatedly → Switch to Path B
- **Hour 4**: If ort integration fails → Switch to Path B
- **Hour 6**: If performance < 20ms → Success, document and ship

**Why Path A**:
1. Best performance (10ms proven in Python)
2. Production-ready (single binary, no Python)
3. Industry standard (ONNX Runtime widely used)
4. Worth the investment for long-term solution

### Fallback: Path B (Python Bridge)

**Trigger**: If Path A blocked after 3 hours

**Why Path B is good fallback**:
1. Guaranteed to work (we have Python code)
2. Meets target (15ms < 20ms)
3. Can implement quickly (2-3 hours)
4. Can still migrate to Path A later

---

## Hour-by-Hour Execution Plan (Path A)

### 🕐 Hour 1: Environment Setup
- ✅ Install CMake, Ninja, Protobuf via Homebrew
- ✅ Verify Xcode command line tools
- ✅ Clone ONNX Runtime repo
- ✅ Checkout matching version (v1.16.3)
- 🎯 **Milestone**: Ready to build

### 🕑 Hour 2: Build ONNX Runtime
- ✅ Run build.sh with CoreML flags
- ⚠️ **Decision Point**: If build fails → Debug for 30min, then switch to Path B
- ✅ Verify libonnxruntime.dylib created
- 🎯 **Milestone**: ONNX Runtime built with CoreML

### 🕒 Hour 3: Build Validation
- ✅ Test ONNX Runtime binary works
- ✅ Verify CoreML EP available
- ✅ Run simple inference test
- 🎯 **Milestone**: CoreML EP confirmed working

### 🕓 Hour 4: Rust Integration
- ✅ Update Cargo.toml (remove download-binaries)
- ✅ Set ORT_STRATEGY=system environment variable
- ✅ Test cargo build succeeds
- ⚠️ **Decision Point**: If integration fails → Switch to Path B
- 🎯 **Milestone**: ort crate using custom build

### 🕔 Hour 5: Provider Code Updates
- ✅ Add CoreMLExecutionProvider to session builder
- ✅ Add .error_on_failure() to catch issues early
- ✅ Compile and fix any errors
- 🎯 **Milestone**: Code compiles with CoreML EP

### 🕕 Hour 6: Testing & Documentation
- ✅ Run integration test
- ✅ Verify P95 < 20ms (target: ~10ms)
- ✅ Document build process
- ✅ Create deployment guide
- 🎯 **Milestone**: Production-ready with <20ms P95

---

## Risk Mitigation

### Risk 1: ONNX Runtime Build Fails

**Probability**: 30%

**Mitigation**:
1. Use exact version that matches ort crate (v1.16.3)
2. Follow official build docs exactly
3. Have backup: Prebuilt CoreML binary from community
4. Fallback: Switch to Path B after 3 hours

### Risk 2: CoreML EP Not Available After Build

**Probability**: 15%

**Mitigation**:
1. Test CoreML EP separately before Rust integration
2. Use Python to verify: `ort.get_available_providers()`
3. Check build logs for CoreML compilation
4. Fallback: Try CoreMLExecutionProvider with fallback to CPU

### Risk 3: Performance Still <20ms After CoreML

**Probability**: 10%

**Mitigation**:
1. Profile to find bottleneck (likely tokenization, not inference)
2. Optimize non-inference parts (tokenizer, pooling)
3. Try different CoreML options (ANE-only vs All)
4. Worst case: 15ms is still good, acceptable compromise

### Risk 4: Deployment Complexity

**Probability**: 20%

**Mitigation**:
1. Document build process thoroughly
2. Create Docker image with prebuilt ONNX RT
3. CI/CD caching of compiled binary
4. Provide both source build and binary distribution

---

## Success Criteria

### Must Have ✅
- [ ] P95 latency < 20ms
- [ ] All tests passing
- [ ] Embeddings correct (384-dim, L2 norm = 1.0)
- [ ] Build process documented

### Should Have 🎯
- [ ] P95 latency ~10ms (matching Python validation)
- [ ] Single command build process
- [ ] CI/CD integration
- [ ] Deployment guide

### Nice to Have ⭐
- [ ] P50 < 10ms
- [ ] Docker image with prebuilt binary
- [ ] Performance comparison benchmarks
- [ ] Multiple execution provider support (CoreML + CPU fallback)

---

## Next Session Checklist

**Before Starting**:
- [ ] Clear 6-hour block of focused time
- [ ] Backup current working code (git commit)
- [ ] Read ONNX Runtime build docs
- [ ] Prepare fallback (Path B code skeleton)

**During Execution**:
- [ ] Hour 1: Environment setup
- [ ] Hour 2: Build ONNX Runtime (GO/NO-GO decision)
- [ ] Hour 3: Validate build
- [ ] Hour 4: Rust integration (GO/NO-GO decision)
- [ ] Hour 5: Provider updates
- [ ] Hour 6: Testing & docs

**After Completion**:
- [ ] Run full test suite
- [ ] Performance validation (P95 < 20ms)
- [ ] Document lessons learned
- [ ] Update project status
- [ ] Plan integration with akidb-service

---

## Conclusion

**Current State**:
- ✅ Rust ONNX provider: 100% functional (CPU-only)
- ⚠️ Performance: 43ms (needs 2.15x improvement)

**Recommended Path**:
- 🥇 **Primary**: Build ONNX Runtime with CoreML (4-6 hours, 10ms P95)
- 🥈 **Fallback**: Python bridge (2-3 hours, 15ms P95)

**Confidence**:
- 70% Path A succeeds → **10ms P95** achievement
- 95% Path B succeeds → **15ms P95** acceptable
- 100% we achieve < 20ms target (either path)

**Investment**:
- Best case: 6 hours → 10ms performance ✅
- Worst case: 9 hours (6h Path A attempt + 3h Path B) → 15ms performance ✅

**Strategic Value**:
- Production-ready embedding service
- Competitive performance (<20ms)
- Leverages Apple Silicon GPU/ANE
- Foundation for entire AkiDB 2.0 embedding pipeline

**Next Action**: Begin Path A execution (Hour 1: Environment Setup)

---

**End of Megathink - Ready to Execute** 🚀
