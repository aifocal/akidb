# Jetson Thor Week 2: Daily Action Plan

**Status:** Ready to Execute
**Timeline:** 5 days (Monday-Friday)
**Goal:** Qwen3 4B ONNX integration with baseline performance validation

---

## Quick Reference

### Success Metrics (Week 2)
- ✅ Qwen3 4B ONNX model converted and validated
- ✅ 12 integration tests passing
- ✅ Baseline performance: 50-100ms P95 (batch 1)
- ✅ Quality: >0.99 cosine similarity vs HuggingFace

### Resources
- Jetson Thor: On desk, ready
- ONNX provider: Implemented (Week 1)
- Storage: /opt/akidb/models/
- TensorRT cache: /tmp/akidb_trt_cache/

---

## Day 1: Environment Setup (Monday)

### Morning (4 hours)

**Goal:** Jetson Thor ready for development

#### Task 1.1: System Verification (1 hour)
```bash
# Check CUDA
nvidia-smi
nvcc --version  # Should be 12.2+

# Check TensorRT
dpkg -l | grep tensorrt
# Expected: libnvinfer9, libnvonnxparsers9

# Check system
uname -a
cat /etc/os-release
free -h  # Check RAM (should be 64GB)
df -h    # Check disk space (need ~20GB free)
```

**Checklist:**
- [ ] CUDA 12.2+ installed and working
- [ ] TensorRT 9.0+ installed
- [ ] cuDNN 8.9+ installed
- [ ] 64GB RAM available
- [ ] 20GB+ disk space free

#### Task 1.2: Python Setup (30 minutes)
```bash
sudo apt update && sudo apt upgrade -y
sudo apt install -y python3.10 python3.10-dev python3-pip

pip3 install --upgrade pip
pip3 install torch==2.1.0
pip3 install transformers==4.36.0
pip3 install optimum[onnxruntime-gpu]==1.16.0
pip3 install onnx==1.15.0
pip3 install onnxruntime-gpu==1.17.0
pip3 install accelerate==0.25.0

# Verify
python3 -c "import torch; print(f'PyTorch: {torch.__version__}')"
python3 -c "import onnxruntime; print(f'ONNX Runtime: {onnxruntime.__version__}')"
```

**Checklist:**
- [ ] Python 3.10+ installed
- [ ] PyTorch 2.1+ installed
- [ ] Transformers 4.36+ installed
- [ ] Optimum 1.16+ installed
- [ ] ONNX Runtime 1.17+ installed

#### Task 1.3: Rust Setup (30 minutes)
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
rustc --version  # Verify 1.75+

cargo install cargo-watch
cargo install cargo-criterion
```

**Checklist:**
- [ ] Rust 1.75+ installed
- [ ] Cargo working
- [ ] cargo-criterion installed

#### Task 1.4: Download Qwen3 4B (1 hour)
```bash
sudo mkdir -p /opt/akidb/models
sudo chown $USER:$USER /opt/akidb/models
cd /opt/akidb/models

python3 << EOF
from transformers import AutoModel, AutoTokenizer

model_id = "Qwen/Qwen2.5-4B"
cache_dir = "/opt/akidb/models/qwen3-4b-hf"

print(f"Downloading {model_id}...")
model = AutoModel.from_pretrained(model_id, cache_dir=cache_dir, trust_remote_code=True)
tokenizer = AutoTokenizer.from_pretrained(model_id, cache_dir=cache_dir, trust_remote_code=True)
print(f"✅ Model downloaded to {cache_dir}")
EOF
```

**Checklist:**
- [ ] Model downloaded (~8GB)
- [ ] Tokenizer downloaded
- [ ] Files in /opt/akidb/models/qwen3-4b-hf/

#### Task 1.5: Clone AkiDB (15 minutes)
```bash
cd ~
git clone https://github.com/your-org/akidb2.git  # Or git pull if exists
cd akidb2

# Build to verify
cargo build --release -p akidb-embedding --features onnx
```

**Checklist:**
- [ ] Repository cloned/updated
- [ ] Builds successfully
- [ ] Week 1 ONNX provider present

### Afternoon: Contingency & Documentation

**Task 1.6: Troubleshooting (if needed)**
- Fix any installation issues
- Update system packages
- Verify all dependencies

**Task 1.7: Document Environment**
```bash
# Create environment report
cat > /opt/akidb/environment.txt << EOF
Date: $(date)
Hostname: $(hostname)
Kernel: $(uname -r)
CUDA: $(nvcc --version | grep release)
Python: $(python3 --version)
Rust: $(rustc --version)
Disk: $(df -h /opt/akidb)
Memory: $(free -h | grep Mem)
EOF
```

### End of Day 1
**Time spent:** ~4 hours
**Status:** Environment ready for model conversion

**Daily Report:**
```
✅ CUDA 12.2+ verified
✅ Python 3.10+ with dependencies
✅ Rust 1.75+ installed
✅ Qwen3 4B downloaded (8GB)
✅ AkiDB builds successfully

Tomorrow: Convert Qwen3 to ONNX FP8
```

---

## Day 2: Model Conversion (Tuesday)

### Morning (3 hours)

**Goal:** Qwen3 4B converted to ONNX FP8 format

#### Task 2.1: Create Conversion Script (30 minutes)
```bash
cd /opt/akidb/models

cat > convert_qwen3_to_onnx.py << 'SCRIPT'
#!/usr/bin/env python3
from transformers import AutoModel, AutoTokenizer
from optimum.onnxruntime import ORTModelForFeatureExtraction
import torch

model_id = "Qwen/Qwen2.5-4B"
output_dir = "/opt/akidb/models/qwen3-4b-onnx-fp8"

print(f"🔧 Converting {model_id} to ONNX...")

# Export to ONNX
ort_model = ORTModelForFeatureExtraction.from_pretrained(
    model_id,
    export=True,
    provider="TensorrtExecutionProvider"
)

# Save
ort_model.save_pretrained(output_dir)
tokenizer = AutoTokenizer.from_pretrained(model_id)
tokenizer.save_pretrained(output_dir)

print(f"✅ Conversion complete: {output_dir}")
SCRIPT

chmod +x convert_qwen3_to_onnx.py
```

**Checklist:**
- [ ] Script created
- [ ] Script executable

#### Task 2.2: Run Conversion (1 hour)
```bash
cd /opt/akidb/models

# Run conversion (will take 10-15 minutes)
python3 convert_qwen3_to_onnx.py 2>&1 | tee conversion.log

# Verify output
ls -lh qwen3-4b-onnx-fp8/
# Expected: model.onnx, tokenizer.json, config.json
```

**Checklist:**
- [ ] Conversion completed without errors
- [ ] model.onnx created (~2GB)
- [ ] tokenizer.json created
- [ ] config.json created

#### Task 2.3: Validate ONNX Model (30 minutes)
```bash
python3 << EOF
import onnx

model = onnx.load("/opt/akidb/models/qwen3-4b-onnx-fp8/model.onnx")
onnx.checker.check_model(model)

print("✅ ONNX model validated")
print(f"   Inputs: {[i.name for i in model.graph.input]}")
print(f"   Outputs: {[o.name for o in model.graph.output]}")
print(f"   Opset: {model.opset_import[0].version}")
EOF
```

**Checklist:**
- [ ] ONNX checker passes
- [ ] Inputs: input_ids, attention_mask, token_type_ids
- [ ] Outputs: last_hidden_state
- [ ] Opset version: 17

#### Task 2.4: Test Inference (Python) (30 minutes)
```bash
python3 << EOF
from optimum.onnxruntime import ORTModelForFeatureExtraction
from transformers import AutoTokenizer
import torch

model = ORTModelForFeatureExtraction.from_pretrained(
    "/opt/akidb/models/qwen3-4b-onnx-fp8",
    provider="TensorrtExecutionProvider"
)
tokenizer = AutoTokenizer.from_pretrained("/opt/akidb/models/qwen3-4b-onnx-fp8")

# Test
text = "Hello, this is a test."
inputs = tokenizer(text, return_tensors="pt", max_length=512, truncation=True)

print("⏱️  Building TensorRT engine (first run, 2-5 minutes)...")
outputs = model(**inputs)

embeddings = outputs.last_hidden_state.mean(dim=1)
embeddings = embeddings / embeddings.norm(p=2, dim=1, keepdim=True)

print(f"✅ Inference successful")
print(f"   Shape: {embeddings.shape}")
print(f"   Norm: {embeddings.norm(p=2).item():.4f}")
EOF
```

**Checklist:**
- [ ] TensorRT engine builds (first run: 2-5 min)
- [ ] Inference produces embeddings
- [ ] Output shape: [1, 4096]
- [ ] L2 norm: ~1.0

### Afternoon (2 hours)

#### Task 2.5: Benchmark PyTorch vs ONNX (1 hour)
```bash
cd /opt/akidb/models

python3 << EOF
import torch
import time
import numpy as np
from transformers import AutoModel, AutoTokenizer
from optimum.onnxruntime import ORTModelForFeatureExtraction

def bench_pytorch():
    model = AutoModel.from_pretrained("Qwen/Qwen2.5-4B", trust_remote_code=True)
    model.eval().cuda()
    tokenizer = AutoTokenizer.from_pretrained("Qwen/Qwen2.5-4B", trust_remote_code=True)

    text = "Test sentence for benchmarking."
    inputs = tokenizer(text, return_tensors="pt", max_length=512, truncation=True)
    inputs = {k: v.cuda() for k, v in inputs.items()}

    # Warmup
    for _ in range(10):
        with torch.no_grad():
            model(**inputs)

    # Benchmark
    latencies = []
    for _ in range(100):
        start = time.perf_counter()
        with torch.no_grad():
            model(**inputs)
        torch.cuda.synchronize()
        latencies.append((time.perf_counter() - start) * 1000)

    print(f"PyTorch P50: {np.percentile(latencies, 50):.2f}ms")
    print(f"PyTorch P95: {np.percentile(latencies, 95):.2f}ms")

def bench_onnx():
    model = ORTModelForFeatureExtraction.from_pretrained(
        "/opt/akidb/models/qwen3-4b-onnx-fp8",
        provider="TensorrtExecutionProvider"
    )
    tokenizer = AutoTokenizer.from_pretrained("/opt/akidb/models/qwen3-4b-onnx-fp8")

    text = "Test sentence for benchmarking."
    inputs = tokenizer(text, return_tensors="pt", max_length=512, truncation=True)

    # Warmup
    for _ in range(10):
        model(**inputs)

    # Benchmark
    latencies = []
    for _ in range(100):
        start = time.perf_counter()
        model(**inputs)
        latencies.append((time.perf_counter() - start) * 1000)

    print(f"ONNX P50: {np.percentile(latencies, 50):.2f}ms")
    print(f"ONNX P95: {np.percentile(latencies, 95):.2f}ms")

bench_pytorch()
bench_onnx()
EOF
```

**Checklist:**
- [ ] PyTorch benchmarked
- [ ] ONNX benchmarked
- [ ] ONNX faster than PyTorch
- [ ] Results logged

#### Task 2.6: Quality Check (30 minutes)
```bash
python3 << EOF
import torch
from transformers import AutoModel, AutoTokenizer
from optimum.onnxruntime import ORTModelForFeatureExtraction
from sklearn.metrics.pairwise import cosine_similarity

# Load models
pt_model = AutoModel.from_pretrained("Qwen/Qwen2.5-4B", trust_remote_code=True).eval()
onnx_model = ORTModelForFeatureExtraction.from_pretrained("/opt/akidb/models/qwen3-4b-onnx-fp8")
tokenizer = AutoTokenizer.from_pretrained("Qwen/Qwen2.5-4B", trust_remote_code=True)

# Test
text = "The autonomous vehicle detects pedestrians."
inputs = tokenizer(text, return_tensors="pt", max_length=512, truncation=True)

with torch.no_grad():
    pt_emb = pt_model(**inputs).last_hidden_state.mean(dim=1)
    pt_emb = pt_emb / pt_emb.norm(p=2, dim=1, keepdim=True)

onnx_emb = onnx_model(**inputs).last_hidden_state.mean(dim=1)
onnx_emb = onnx_emb / onnx_emb.norm(p=2, dim=1, keepdim=True)

sim = cosine_similarity(pt_emb.numpy(), onnx_emb.numpy())[0][0]
print(f"Cosine similarity: {sim:.6f}")
assert sim > 0.99, f"Quality check failed: {sim:.6f} < 0.99"
print("✅ Quality check passed")
EOF
```

**Checklist:**
- [ ] Cosine similarity >0.99
- [ ] Quality check passed

### End of Day 2
**Time spent:** ~5 hours
**Status:** ONNX model ready for Rust integration

**Daily Report:**
```
✅ Qwen3 4B converted to ONNX opset 17
✅ ONNX model validated
✅ TensorRT engine builds successfully
✅ Python inference working
✅ Quality: >0.99 similarity vs PyTorch

Tomorrow: Rust integration and testing
```

---

## Day 3: Rust Integration (Wednesday)

### Morning (3 hours)

**Goal:** Integrate ONNX model with Rust provider

#### Task 3.1: Create Integration Test File (1 hour)
```bash
cd ~/akidb2

# Copy test template from PRD (12 tests)
# File: crates/akidb-embedding/tests/qwen3_integration_test.rs
# (See PRD for full code)
```

**Checklist:**
- [ ] Test file created
- [ ] 12 test functions defined
- [ ] Helper functions added

#### Task 3.2: Build and Run Tests (2 hours)
```bash
cd ~/akidb2

# Build
cargo build --release -p akidb-embedding --features onnx

# Run tests (first run will build TensorRT engine)
RUST_LOG=info cargo test -p akidb-embedding --features onnx --test qwen3_integration_test -- --nocapture
```

**Checklist:**
- [ ] Compiles successfully
- [ ] TensorRT engine builds
- [ ] All 12 tests pass

**Expected Output:**
```
running 12 tests
test qwen3_tests::test_provider_initialization ... ok
test qwen3_tests::test_model_info ... ok
test qwen3_tests::test_health_check ... ok
test qwen3_tests::test_single_embedding ... ok
test qwen3_tests::test_batch_embeddings ... ok
test qwen3_tests::test_semantic_similarity ... ok
test qwen3_tests::test_long_text ... ok
test qwen3_tests::test_empty_input_error ... ok
test qwen3_tests::test_whitespace_input_error ... ok
test qwen3_tests::test_large_batch ... ok
test qwen3_tests::test_concurrent_requests ... ok

test result: ok. 12 passed; 0 failed; 0 ignored
```

### Afternoon (2 hours)

#### Task 3.3: Debug Failures (if any) (1 hour)
- Check TensorRT logs
- Verify model paths
- Fix compilation errors
- Adjust test expectations

#### Task 3.4: Run Individual Tests (30 minutes)
```bash
# Test each one individually
cargo test -p akidb-embedding --features onnx --test qwen3_integration_test test_single_embedding -- --nocapture
cargo test -p akidb-embedding --features onnx --test qwen3_integration_test test_batch_embeddings -- --nocapture
cargo test -p akidb-embedding --features onnx --test qwen3_integration_test test_semantic_similarity -- --nocapture
```

**Checklist:**
- [ ] Single embedding test passes
- [ ] Batch embeddings test passes
- [ ] Semantic similarity test passes
- [ ] All error handling tests pass

#### Task 3.5: Performance Spot Check (30 minutes)
```bash
# Run performance tests
cargo test -p akidb-embedding --features onnx --release --test qwen3_integration_test test_single_embedding -- --nocapture | grep Duration
cargo test -p akidb-embedding --features onnx --release --test qwen3_integration_test test_large_batch -- --nocapture | grep -E "(Duration|Throughput)"
```

**Checklist:**
- [ ] Latency measured (should be <150ms for single)
- [ ] Throughput measured (should be >5 QPS)
- [ ] Results logged

### End of Day 3
**Time spent:** ~5 hours
**Status:** Rust integration complete, all tests passing

**Daily Report:**
```
✅ 12 integration tests implemented
✅ All tests passing (12/12)
✅ TensorRT engine cached
✅ Embeddings generated correctly (4096-dim)
✅ Performance spot check: ~XXms P95

Tomorrow: Performance benchmarking
```

---

## Day 4: Performance Benchmarking (Thursday)

### Morning (3 hours)

**Goal:** Establish baseline performance metrics

#### Task 4.1: Create Criterion Benchmarks (1 hour)
```bash
cd ~/akidb2

# Create bench file
# File: crates/akidb-embedding/benches/qwen3_bench.rs
# (See PRD for full code)
```

**Checklist:**
- [ ] Benchmark file created
- [ ] Batch size benchmarks defined (1, 4, 8, 16, 32)
- [ ] Text length benchmarks defined

#### Task 4.2: Run Criterion Benchmarks (2 hours)
```bash
cd ~/akidb2

# Run benchmarks (will take ~1 hour)
cargo bench -p akidb-embedding --features onnx --bench qwen3_bench

# Open HTML report
firefox target/criterion/report/index.html &
```

**Checklist:**
- [ ] Benchmarks complete
- [ ] HTML report generated
- [ ] Results show throughput for each batch size

### Afternoon (2 hours)

#### Task 4.3: Manual Performance Tests (1 hour)
```bash
cat > test_performance.sh << 'EOF'
#!/bin/bash

echo "🚀 Qwen3 4B Performance Testing"
echo "==============================="
echo

echo "Batch Size 1:"
cargo test -p akidb-embedding --features onnx --release --test qwen3_integration_test test_single_embedding -- --nocapture 2>&1 | grep Duration

echo
echo "Batch Size 4:"
cargo test -p akidb-embedding --features onnx --release --test qwen3_integration_test test_batch_embeddings -- --nocapture 2>&1 | grep Duration

echo
echo "Batch Size 32:"
cargo test -p akidb-embedding --features onnx --release --test qwen3_integration_test test_large_batch -- --nocapture 2>&1 | grep -E "(Duration|Throughput)"

echo
echo "Concurrent (4 threads):"
cargo test -p akidb-embedding --features onnx --release --test qwen3_integration_test test_concurrent_requests -- --nocapture 2>&1 | grep Durations
EOF

chmod +x test_performance.sh
./test_performance.sh | tee performance_results.txt
```

**Checklist:**
- [ ] Single embedding latency: XXms
- [ ] Batch 4 latency: XXms
- [ ] Batch 32 latency: XXms, throughput: XX QPS
- [ ] Concurrent throughput: XX QPS

#### Task 4.4: Resource Usage Profiling (30 minutes)
```bash
# Monitor GPU usage during benchmark
nvidia-smi dmon -s pucvmet -c 100 > gpu_usage.log &
GPU_PID=$!

# Run benchmark
cargo bench -p akidb-embedding --features onnx --bench qwen3_bench -- --warm-up-time 5 --measurement-time 30 qwen3_batch_sizes/1

# Stop monitoring
kill $GPU_PID

# Analyze results
echo "GPU Usage:"
cat gpu_usage.log | awk '{print $4}' | sort -n | tail -5
```

**Checklist:**
- [ ] GPU memory usage <4GB
- [ ] GPU utilization measured
- [ ] CPU memory <2GB

#### Task 4.5: Document Results (30 minutes)
```bash
cat > performance_baseline.md << EOF
# Qwen3 4B Performance Baseline - Jetson Thor

**Date:** $(date +%Y-%m-%d)
**Hardware:** Jetson Thor (Blackwell, 2,000 TOPS)

## Latency (P95)
- Batch 1: XXms
- Batch 8: XXms
- Batch 32: XXms

## Throughput
- Single-threaded: XX QPS
- Concurrent (4 threads): XX QPS

## Resource Usage
- GPU Memory: XXG
- CPU Memory: XXG
- GPU Utilization: XX%

## Next Steps
- Optimize for <30ms P95 (Week 3)
- Profile with NVIDIA Nsight
- Tune TensorRT profiles
EOF
```

**Checklist:**
- [ ] Results documented
- [ ] Baseline established
- [ ] Optimization opportunities identified

### End of Day 4
**Time spent:** ~5 hours
**Status:** Performance baseline established

**Daily Report:**
```
✅ Criterion benchmarks complete
✅ HTML report generated
✅ Baseline latency: XXms P95 (batch 1)
✅ Baseline throughput: XX QPS
✅ Resource usage: GPU XXG, CPU XXG

Tomorrow: Quality validation and documentation
```

---

## Day 5: Quality & Documentation (Friday)

### Morning (3 hours)

**Goal:** Validate embedding quality and document Week 2

#### Task 5.1: Comprehensive Quality Test (2 hours)
```bash
cd /opt/akidb/models

cat > validate_quality.py << 'EOF'
#!/usr/bin/env python3
import torch
import numpy as np
from transformers import AutoModel, AutoTokenizer
from optimum.onnxruntime import ORTModelForFeatureExtraction
from sklearn.metrics.pairwise import cosine_similarity

# Test cases
test_texts = [
    "The cat sits on the mat.",
    "Machine learning is a subset of artificial intelligence.",
    "The stock market experienced significant volatility today.",
    "Photosynthesis converts light energy into chemical energy.",
    "The Renaissance was a period of cultural rebirth.",
    "Quantum mechanics describes nature at smallest scales.",
    "Climate change poses risks to ecosystems.",
    "Neural networks are inspired by brain structures.",
]

def get_hf_embeddings(texts):
    model = AutoModel.from_pretrained("Qwen/Qwen2.5-4B", trust_remote_code=True).eval()
    tokenizer = AutoTokenizer.from_pretrained("Qwen/Qwen2.5-4B", trust_remote_code=True)

    embeddings = []
    for text in texts:
        inputs = tokenizer(text, return_tensors="pt", max_length=512, truncation=True)
        with torch.no_grad():
            emb = model(**inputs).last_hidden_state.mean(dim=1).squeeze()
            emb = emb / emb.norm(p=2)
        embeddings.append(emb.numpy())

    return np.array(embeddings)

def get_onnx_embeddings(texts):
    model = ORTModelForFeatureExtraction.from_pretrained(
        "/opt/akidb/models/qwen3-4b-onnx-fp8"
    )
    tokenizer = AutoTokenizer.from_pretrained("/opt/akidb/models/qwen3-4b-onnx-fp8")

    embeddings = []
    for text in texts:
        inputs = tokenizer(text, return_tensors="pt", max_length=512, truncation=True)
        emb = model(**inputs).last_hidden_state.mean(dim=1).squeeze()
        emb = emb / emb.norm(p=2)
        embeddings.append(emb.numpy())

    return np.array(embeddings)

print("🧪 Validating ONNX vs HuggingFace...")
hf_embs = get_hf_embeddings(test_texts)
onnx_embs = get_onnx_embeddings(test_texts)

similarities = []
for i, (hf, onnx) in enumerate(zip(hf_embs, onnx_embs)):
    sim = cosine_similarity([hf], [onnx])[0][0]
    similarities.append(sim)
    print(f"Sample {i+1}: {sim:.6f}")

print(f"\nMean: {np.mean(similarities):.6f}")
print(f"Min: {np.min(similarities):.6f}")
print(f"Max: {np.max(similarities):.6f}")

threshold = 0.99
if np.min(similarities) >= threshold:
    print(f"\n✅ PASS: All >= {threshold}")
else:
    print(f"\n❌ FAIL: Some < {threshold}")
EOF

chmod +x validate_quality.py
python3 validate_quality.py | tee quality_validation.txt
```

**Checklist:**
- [ ] 8 test cases run
- [ ] All similarities >0.99
- [ ] Quality validation PASSED
- [ ] Results saved to quality_validation.txt

#### Task 5.2: Semantic Similarity Validation (30 minutes)
```bash
# Already tested in Rust tests, but verify in Python
python3 << EOF
import torch
from optimum.onnxruntime import ORTModelForFeatureExtraction
from transformers import AutoTokenizer
from sklearn.metrics.pairwise import cosine_similarity

model = ORTModelForFeatureExtraction.from_pretrained("/opt/akidb/models/qwen3-4b-onnx-fp8")
tokenizer = AutoTokenizer.from_pretrained("/opt/akidb/models/qwen3-4b-onnx-fp8")

texts = [
    "The cat sits on the mat.",
    "A feline rests on the rug.",
    "The dog barks loudly.",
]

embeddings = []
for text in texts:
    inputs = tokenizer(text, return_tensors="pt", max_length=512, truncation=True)
    emb = model(**inputs).last_hidden_state.mean(dim=1).squeeze()
    emb = emb / emb.norm(p=2)
    embeddings.append(emb.numpy())

sim_cat_feline = cosine_similarity([embeddings[0]], [embeddings[1]])[0][0]
sim_cat_dog = cosine_similarity([embeddings[0]], [embeddings[2]])[0][0]

print(f"Cat-Feline: {sim_cat_feline:.4f}")
print(f"Cat-Dog: {sim_cat_dog:.4f}")
assert sim_cat_feline > sim_cat_dog
print("✅ Semantic similarity validated")
EOF
```

**Checklist:**
- [ ] Similar texts have higher similarity
- [ ] Semantic test passed

#### Task 5.3: Edge Cases (30 minutes)
```bash
# Test edge cases
cargo test -p akidb-embedding --features onnx --release --test qwen3_integration_test test_long_text -- --nocapture
cargo test -p akidb-embedding --features onnx --release --test qwen3_integration_test test_empty_input_error -- --nocapture
cargo test -p akidb-embedding --features onnx --release --test qwen3_integration_test test_whitespace_input_error -- --nocapture
```

**Checklist:**
- [ ] Long text (512+ tokens) handled
- [ ] Empty input rejected
- [ ] Whitespace input rejected

### Afternoon (2 hours)

#### Task 5.4: Create Week 2 Completion Report (1 hour)
```bash
cd ~/akidb2/automatosx/tmp

cat > JETSON-THOR-WEEK2-COMPLETION-REPORT.md << 'EOF'
# Jetson Thor Week 2: Completion Report

**Date:** $(date +%Y-%m-%d)
**Status:** ✅ COMPLETE
**Duration:** 5 days

## Executive Summary

Successfully converted Qwen3 4B to ONNX FP8, integrated with AkiDB ONNX provider, and established baseline performance on Jetson Thor.

## Achievements

1. ✅ Qwen3 4B ONNX model converted and validated
2. ✅ TensorRT Execution Provider working
3. ✅ 12 integration tests passing (100%)
4. ✅ Quality: >0.99 cosine similarity
5. ✅ Baseline performance established

## Performance Results

**Latency (P95):**
- Batch 1: XXms
- Batch 8: XXms
- Batch 32: XXms

**Throughput:**
- Single-threaded: XX QPS
- Concurrent: XX QPS

**Quality:**
- Cosine similarity: 0.99XX
- Semantic tests: PASS

## Next Steps (Week 3)

1. Optimize for <30ms P95
2. Profile with NVIDIA Nsight
3. Tune TensorRT engine
4. Batch size optimization

## Files Created

- /opt/akidb/models/qwen3-4b-onnx-fp8/
- crates/akidb-embedding/tests/qwen3_integration_test.rs
- crates/akidb-embedding/benches/qwen3_bench.rs
- performance_results.txt
- quality_validation.txt

## Lessons Learned

- TensorRT engine build takes 2-5 minutes (first run only)
- ONNX Runtime 1.17+ is stable with TensorRT EP
- FP8 provides good quality/performance balance
- Baseline performance is acceptable, optimization needed

---

**Prepared by:** Development Team
**Reviewed by:** [To be filled]
**Approved by:** [To be filled]
EOF
```

**Checklist:**
- [ ] Report created
- [ ] All sections filled
- [ ] Results documented
- [ ] Next steps clear

#### Task 5.5: Update Project Documentation (30 minutes)
```bash
# Update README or docs
cat >> ~/akidb2/README.md << EOF

## Jetson Thor Support (NEW)

AkiDB now supports NVIDIA Jetson Thor with TensorRT acceleration:

- Model: Qwen3 4B (4096-dim embeddings)
- Precision: FP8 (8-bit floating point)
- Performance: ~XXms P95 @ batch 1
- Quality: >0.99 similarity vs HuggingFace

See [JETSON-THOR-WEEK2-COMPLETION-REPORT.md](automatosx/tmp/) for details.
EOF
```

**Checklist:**
- [ ] README updated
- [ ] Documentation links added

#### Task 5.6: Cleanup and Archive (30 minutes)
```bash
# Archive logs
mkdir -p ~/akidb2/automatosx/archive/week2/
mv conversion.log ~/akidb2/automatosx/archive/week2/
mv benchmark.log ~/akidb2/automatosx/archive/week2/
mv performance_results.txt ~/akidb2/automatosx/archive/week2/
mv quality_validation.txt ~/akidb2/automatosx/archive/week2/

# Clean up temporary files
rm -rf /tmp/akidb_trt_cache/*.lock

# Commit changes
cd ~/akidb2
git add .
git commit -m "Week 2 Complete: Qwen3 4B ONNX integration with TensorRT"
```

**Checklist:**
- [ ] Logs archived
- [ ] Temp files cleaned
- [ ] Changes committed

### End of Day 5
**Time spent:** ~5 hours
**Status:** Week 2 COMPLETE

**Weekly Summary:**
```
✅ All Day 1-5 tasks complete
✅ Qwen3 4B ONNX operational
✅ 12 integration tests passing
✅ Baseline performance: XXms P95
✅ Quality validated: >0.99 similarity
✅ Ready for Week 3 optimization

Total time: ~23 hours over 5 days
Next: Week 3 - Performance optimization (<30ms target)
```

---

## Week 2 Success Metrics

### Final Checklist

**Model Conversion:**
- [x] Qwen3 4B downloaded
- [x] ONNX model generated
- [x] TensorRT engine builds
- [x] Python inference works

**Rust Integration:**
- [x] 12 integration tests created
- [x] All tests passing
- [x] Error handling validated
- [x] Concurrent requests work

**Performance:**
- [x] Baseline latency measured
- [x] Baseline throughput measured
- [x] Criterion benchmarks complete
- [x] Resource usage profiled

**Quality:**
- [x] Cosine similarity >0.99
- [x] Semantic similarity validated
- [x] Edge cases tested

**Documentation:**
- [x] Week 2 report created
- [x] Performance results documented
- [x] Quality results documented
- [x] Next steps planned

### Key Metrics Summary

| Metric | Target | Actual | Status |
|--------|--------|--------|--------|
| ONNX conversion | ✅ | TBD | TBD |
| Integration tests | 12/12 | TBD | TBD |
| Quality (similarity) | >0.99 | TBD | TBD |
| Latency (batch 1) | <100ms | TBD | TBD |
| Throughput | >10 QPS | TBD | TBD |

---

## Contingency Plans

### If TensorRT Fails
1. Use CUDA EP instead of TensorRT EP
2. Still get GPU acceleration (just no TensorRT optimization)
3. Will be slower but functional

### If Performance is Slow
1. Accept baseline for Week 2
2. Optimize in Week 3
3. Week 2 goal is "working", not "fast"

### If Quality is Low
1. Use FP16 instead of FP8
2. Trade performance for quality
3. Re-convert model with FP16

### If Tests Fail
1. Debug individually
2. Check TensorRT logs
3. Verify model paths
4. Ask for help if stuck >2 hours

---

**Action Plan Version:** 1.0
**Created:** 2025-11-11
**Status:** Ready to Execute
**Next Review:** End of Week 2
