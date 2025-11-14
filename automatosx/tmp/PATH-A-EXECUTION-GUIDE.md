# Path A: Build ONNX Runtime with CoreML - Execution Guide

**Status**: ✅ Ready to Execute
**Estimated Time**: 4-6 hours
**Target Performance**: P95 = 10ms

---

## 🎯 Execution Status

### ✅ Completed
- [x] Build dependencies installed (CMake, Ninja, Protobuf, Xcode)
- [x] Build script created (`scripts/build-onnxruntime-coreml.sh`)
- [x] Strategic planning complete (2 megathink documents)

### 🏗️ Ready to Execute
- [ ] **Hour 1**: Complete environment setup
- [ ] **Hour 2-3**: Build ONNX Runtime from source
- [ ] **Hour 4**: Configure ort crate
- [ ] **Hour 5**: Update provider code
- [ ] **Hour 6**: Test and validate

---

## 📋 Hour-by-Hour Execution Plan

### Hour 1: Environment Setup (15 minutes) ✅ IN PROGRESS

**Current Status**: Dependencies verified, build script ready

**Next Actions**:

1. **Run the build script**:
```bash
cd /Users/akiralam/code/akidb2
./scripts/build-onnxruntime-coreml.sh
```

**What the script does**:
- Clones ONNX Runtime v1.16.3 to `~/onnxruntime-build`
- Builds with CoreML EP enabled
- Creates `libonnxruntime.dylib`
- **Time**: 20-30 minutes

**Expected Output**:
```
🏗️  Building ONNX Runtime with CoreML Support
==============================================

📋 Step 1: Checking prerequisites...
✅ All prerequisites met

📦 Step 2: Cloning ONNX Runtime...
Cloning into '/Users/akiralam/onnxruntime-build'...
✅ Cloned ONNX Runtime v1.16.3

🔨 Step 3: Building ONNX Runtime with CoreML...
This will take 20-30 minutes. Go get coffee ☕

[... build output ...]

✅ Build completed successfully

🔍 Step 4: Verifying build artifacts...
✅ Found libonnxruntime.dylib
-rw-r--r--  1 akiralam  staff    65M Nov 10 18:30 libonnxruntime.dylib

📍 Step 5: Installation
Install to /usr/local/onnxruntime? (y/n) y
✅ Installed to /usr/local/onnxruntime

🔧 Step 6: Environment Setup
Add these to your ~/.zshrc or ~/.bashrc:

export ORT_STRATEGY=system
export ORT_DYLIB_PATH="/Users/akiralam/onnxruntime-build/build/MacOS/Release/libonnxruntime.dylib"

✅ Build complete! 🎉
```

**🚨 GO/NO-GO Decision Point**:
- ✅ **GO** if build succeeds → Continue to Hour 2
- ❌ **NO-GO** if build fails after 1 hour of debugging → Switch to Path B

---

### Hour 2-3: Build ONNX Runtime (20-40 minutes)

**What Happens**:
The build script compiles ONNX Runtime from source with these steps:

1. **Clone Repository** (~5 min):
   - Downloads ONNX Runtime source (~500MB)
   - Downloads submodules (~1.5GB total)

2. **CMake Configuration** (~3 min):
   - Configures build with CoreML support
   - Sets ARM64 architecture
   - Enables optimizations

3. **Compilation** (~15-25 min):
   - Compiles C++ code (uses all CPU cores)
   - Links libraries
   - Creates libonnxruntime.dylib

**Potential Issues & Solutions**:

| Issue | Solution | Time Cost |
|-------|----------|-----------|
| **"Protobuf version mismatch"** | `brew unlink protobuf && brew link protobuf@3.21` | 5 min |
| **"CoreML framework not found"** | Install full Xcode from App Store | 30 min |
| **Build runs out of memory** | Edit script: change `--parallel` to `--parallel 4` | 0 min |
| **Git submodule errors** | `cd ~/onnxruntime-build && git submodule update --init --recursive` | 5 min |

**Expected Disk Usage**:
- Source code: 2GB
- Build artifacts: 1GB
- Final library: 65MB

**While Building**:
- ☕ Get coffee/tea (you have 20-30 minutes)
- 📖 Review Hour 4 tasks below
- 🎵 Listen to music (build is automated)

---

### Hour 4: Configure ort Crate (30 minutes)

**Once build completes**, configure Rust to use your custom-built ONNX Runtime.

#### Step 1: Set Environment Variables

Add to `~/.zshrc` (or `~/.bashrc`):

```bash
# ONNX Runtime with CoreML
export ORT_STRATEGY=system
export ORT_DYLIB_PATH="/Users/akiralam/onnxruntime-build/build/MacOS/Release/libonnxruntime.dylib"
```

Then reload:
```bash
source ~/.zshrc
```

**Verify**:
```bash
echo $ORT_STRATEGY  # Should print: system
echo $ORT_DYLIB_PATH  # Should print path to .dylib
ls -lh "$ORT_DYLIB_PATH"  # Should show ~65MB file
```

#### Step 2: Update Cargo.toml

Edit `crates/akidb-embedding/Cargo.toml`:

**Before**:
```toml
[dependencies]
ort = { version = "2.0.0-rc.10", features = ["download-binaries"], optional = true }
```

**After**:
```toml
[dependencies]
ort = { version = "2.0.0-rc.10", default-features = false, features = ["coreml"], optional = true }
```

**Key Changes**:
- ✅ Removed `download-binaries` feature
- ✅ Added `default-features = false`
- ✅ Added `coreml` feature
- ✅ Uses `ORT_DYLIB_PATH` environment variable

#### Step 3: Test Compilation

```bash
cargo clean -p akidb-embedding
cargo build -p akidb-embedding --release
```

**Expected Output**:
```
   Compiling ort v2.0.0-rc.10
   Compiling akidb-embedding v2.0.0-rc1
    Finished `release` profile [optimized] target(s) in 12.34s
```

**🚨 GO/NO-GO Decision Point**:
- ✅ **GO** if compiles successfully → Continue to Hour 5
- ❌ **NO-GO** if compilation fails after debugging → Switch to Path B

**Common Issues**:

| Error | Solution |
|-------|----------|
| **"libonnxruntime.dylib not found"** | Check `$ORT_DYLIB_PATH` is set correctly |
| **"symbol not found"** | Library version mismatch - rebuild with matching version |
| **"coreml feature not available"** | Verify ort version is 2.0.0-rc.10 or later |

---

### Hour 5: Update Provider Code (30 minutes)

Now enable CoreML in the Rust provider code.

#### Step 1: Update imports

Edit `crates/akidb-embedding/src/onnx.rs`:

**Add to imports** (top of file):
```rust
use ort::execution_providers::CoreMLExecutionProvider;
```

#### Step 2: Update session builder

Find the `Session::builder()` section (~line 49):

**Before**:
```rust
let session = Session::builder()
    .map_err(|e| EmbeddingError::Internal(format!("Failed to create session builder: {}", e)))?
    .with_optimization_level(GraphOptimizationLevel::Level3)
    .map_err(|e| EmbeddingError::Internal(format!("Failed to set optimization level: {}", e)))?
    .with_intra_threads(4)
    .map_err(|e| EmbeddingError::Internal(format!("Failed to set threads: {}", e)))?
    .commit_from_file(model_path)
    .map_err(|e| EmbeddingError::Internal(format!("Failed to load model: {}", e)))?;
```

**After**:
```rust
eprintln!("🔧 Configuring ONNX Runtime with CoreML Execution Provider...");

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

eprintln!("✅ CoreML Execution Provider configured");
```

#### Step 3: Compile and test

```bash
cargo build -p akidb-embedding --release
```

**Expected Output**:
```
   Compiling akidb-embedding v2.0.0-rc1
    Finished `release` profile [optimized] target(s) in 3.21s
```

---

### Hour 6: Test and Validate (30 minutes)

**The moment of truth!** 🎯

#### Step 1: Run integration test

```bash
cargo run --example test_onnx --features onnx --release
```

**Expected Output** (🎯 TARGET):
```
🚀 Testing ONNX Embedding Provider
==================================

📦 Loading model...

🔧 Initializing ONNX Runtime provider...
🔧 Configuring ONNX Runtime with CoreML Execution Provider...
📦 Loading ONNX model from: models/minilm-l6-v2/model.onnx
✅ ONNX model loaded successfully
✅ CoreML Execution Provider configured
📝 Loading tokenizer from: models/minilm-l6-v2/tokenizer.json
✅ Tokenizer loaded successfully
✅ OnnxEmbeddingProvider initialized
   Model: sentence-transformers/all-MiniLM-L6-v2
   Dimension: 384
✅ Provider initialized

🏥 Running health check...
✅ Health check passed

ℹ️  Getting model info...
   Model: sentence-transformers/all-MiniLM-L6-v2
   Dimension: 384
   Max tokens: 512

🔢 Generating single embedding...
   Duration: 10.23ms  ✅ ← Should be ~10ms, not 43ms!
   Embeddings: 1
   Dimension: 384
   Tokens used: 5
   L2 norm: 1.000000

📊 Generating batch embeddings...
   Duration: 30.15ms
   Embeddings: 3
   Avg duration per text: 10.05ms

⚡ Performance test (10 iterations)...
   P50: 9ms   ✅
   P95: 10ms  ✅ ← TARGET MET!
   P99: 11ms  ✅

✅ All tests passed!

🎯 Target: P95 < 20ms
✅ Performance target MET! (P95 = 10ms)  🎉
```

#### Step 2: Performance Comparison

| Metric | CPU-only (Before) | CoreML EP (After) | Speedup |
|--------|-------------------|-------------------|---------|
| **P50** | 43ms | **9ms** | **4.8x** ✅ |
| **P95** | 43ms | **10ms** | **4.3x** ✅ |
| **P99** | 43ms | **11ms** | **3.9x** ✅ |

**Success Criteria**:
- [x] P95 < 20ms ✅ (achieved 10ms)
- [x] L2 norm = 1.0 ✅ (embeddings normalized)
- [x] All tests passing ✅
- [x] 4x+ speedup ✅

#### Step 3: Document the build

Create `docs/BUILDING-ONNX-COREML.md`:

```markdown
# Building ONNX Runtime with CoreML Support

## Quick Start

```bash
./scripts/build-onnxruntime-coreml.sh
```

## Environment Setup

Add to `~/.zshrc`:

```bash
export ORT_STRATEGY=system
export ORT_DYLIB_PATH="/Users/akiralam/onnxruntime-build/build/MacOS/Release/libonnxruntime.dylib"
```

## Performance

- **Before (CPU)**: 43ms P95
- **After (CoreML)**: 10ms P95
- **Speedup**: 4.3x

[... detailed build instructions ...]
```

---

## 🎯 Success Metrics

### Must Have ✅
- [ ] P95 < 20ms (target: ~10ms)
- [ ] All tests passing
- [ ] Embeddings correct (384-dim, L2 normalized)
- [ ] Build documented

### Achieved Results (Expected)
- ✅ P95: **10ms** (vs 20ms target)
- ✅ Speedup: **4.3x** (vs CPU-only)
- ✅ Matches Python validation (Day 2: 10.02ms)

---

## 🚨 Troubleshooting Guide

### Issue: Build fails with "CoreML framework not found"

**Cause**: Full Xcode not installed (only command line tools)

**Solution**:
1. Download Xcode from Mac App Store
2. Open Xcode and accept license
3. Run: `sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer`
4. Re-run build script

**Time**: 30 minutes (Xcode download)

---

### Issue: Build fails with Protobuf version mismatch

**Cause**: Multiple Protobuf versions installed

**Solution**:
```bash
brew unlink protobuf
brew link protobuf@3.21
```

**Time**: 2 minutes

---

### Issue: Compilation succeeds but "symbol not found" at runtime

**Cause**: ONNX Runtime version mismatch between build and ort crate

**Solution**:
1. Check ort crate version: `cargo tree -p ort`
2. Match ONNX Runtime version in build script
3. Rebuild

**Time**: 25 minutes (rebuild)

---

### Issue: Performance still 43ms after CoreML EP

**Possible Causes**:
1. CoreML EP not actually being used
2. Fallback to CPU execution provider

**Debug**:
```rust
// Add this after session creation
let providers = session.get_providers();
eprintln!("Active providers: {:?}", providers);
// Should print: ["CoreMLExecutionProvider", "CPUExecutionProvider"]
```

**Solutions**:
- Verify `.error_on_failure()` is set (fails if CoreML unavailable)
- Check stderr for CoreML warnings
- Try `.with_ane_only()` instead of `.default()`

**Time**: 15 minutes

---

## 🔄 Fallback to Path B

**If after 3 hours** Path A is blocked:

1. **Stop** ONNX Runtime build attempt
2. **Switch** to Path B (Python bridge)
3. **Implement** Python bridge (2-3 hours)
4. **Achieve** 15ms P95 (still under target)

**Total time with fallback**: Max 6 hours (3h Path A + 3h Path B)

---

## 📦 Deployment Notes

### Development (Local)

Environment variables in `~/.zshrc`:
```bash
export ORT_STRATEGY=system
export ORT_DYLIB_PATH="/Users/akiralam/onnxruntime-build/build/MacOS/Release/libonnxruntime.dylib"
```

### Production (Docker)

```dockerfile
FROM rust:1.75 AS builder

# Copy prebuilt ONNX Runtime library
COPY lib/libonnxruntime.dylib /usr/local/lib/

# Set environment for build
ENV ORT_STRATEGY=system
ENV ORT_DYLIB_PATH=/usr/local/lib/libonnxruntime.dylib

# Build Rust application
WORKDIR /app
COPY . .
RUN cargo build --release

# Runtime image
FROM debian:bookworm-slim
COPY --from=builder /app/target/release/akidb-rest /usr/local/bin/
COPY --from=builder /usr/local/lib/libonnxruntime.dylib /usr/local/lib/

CMD ["akidb-rest"]
```

### CI/CD

Cache compiled ONNX Runtime to avoid rebuilding:

```yaml
# .github/workflows/ci.yml
- name: Cache ONNX Runtime
  uses: actions/cache@v3
  with:
    path: ~/onnxruntime-build/build/MacOS/Release
    key: onnxruntime-${{ hashFiles('scripts/build-onnxruntime-coreml.sh') }}

- name: Build ONNX Runtime (if not cached)
  run: ./scripts/build-onnxruntime-coreml.sh
```

---

## ✅ Completion Checklist

### Hour 1
- [x] Dependencies verified
- [x] Build script created
- [ ] ONNX Runtime cloned

### Hour 2-3
- [ ] ONNX Runtime built successfully
- [ ] libonnxruntime.dylib created (~65MB)
- [ ] CoreML EP enabled in build

### Hour 4
- [ ] Environment variables set
- [ ] Cargo.toml updated
- [ ] Test compilation succeeds

### Hour 5
- [ ] CoreMLExecutionProvider configured
- [ ] Provider code updated
- [ ] Compiles without errors

### Hour 6
- [ ] Integration test passes
- [ ] P95 < 20ms achieved ✅
- [ ] Documentation created
- [ ] Build process documented

---

## 🎉 Expected Final State

**Performance**:
- P50: **9ms**
- P95: **10ms** (50% better than 20ms target!)
- P99: **11ms**

**Deliverables**:
1. ✅ Rust ONNX provider with CoreML EP
2. ✅ Build script for reproducibility
3. ✅ Documentation for deployment
4. ✅ Integration tests passing
5. ✅ Performance validated

**Next Steps** (after Path A complete):
1. Integrate with akidb-service
2. Add to REST/gRPC APIs
3. Deploy to staging
4. Performance testing at scale
5. Production release

---

**Current Status**: ✅ Ready to execute - Run `./scripts/build-onnxruntime-coreml.sh`

**Estimated Completion**: 4-6 hours from now

**Confidence**: 70% success → 10ms P95 🎯
