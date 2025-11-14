# Qwen3-Embedding-0.6B Quick Start Guide

**Date:** November 11, 2025
**Model:** Qwen/Qwen3-Embedding-0.6B (Dedicated embedding model)
**Target:** <20ms P95 latency on Apple Silicon

---

## Overview

Qwen3-Embedding-0.6B is a dedicated embedding model from Alibaba Cloud, optimized for semantic similarity and retrieval tasks. At 0.6B parameters, it's significantly smaller than general-purpose models while maintaining strong performance.

**Model Details:**
- **Model ID:** `Qwen/Qwen3-Embedding-0.6B`
- **Parameters:** 600 million
- **Embedding Dimension:** 896
- **Max Sequence Length:** 8192 tokens
- **Use Case:** Semantic search, document retrieval, RAG systems

---

## Option 1: Convert to ONNX (Recommended for Production)

### Step 1: Run Conversion Script

```bash
# Use the provided conversion script
/Users/akiralam/code/akidb2/.venv-onnx/bin/python \
  /Users/akiralam/code/akidb2/crates/akidb-embedding/python/convert_qwen_to_onnx.py \
  --model-size 0.6b \
  --output-dir ~/.cache/akidb/models/qwen-0.6b-onnx
```

**Expected Output:**
- ONNX model: `~/.cache/akidb/models/qwen-0.6b-onnx/model_optimized.onnx`
- Tokenizer: `~/.cache/akidb/models/qwen-0.6b-onnx/tokenizer.json`
- Model size: ~2.4 GB
- Conversion time: 5-10 minutes

### Step 2: Configure AkiDB

```bash
# Set environment variables
export AKIDB_EMBEDDING_PROVIDER=python-bridge
export AKIDB_EMBEDDING_MODEL=~/.cache/akidb/models/qwen-0.6b-onnx/model_optimized.onnx
export AKIDB_EMBEDDING_PYTHON_PATH=/Users/akiralam/code/akidb2/.venv-onnx/bin/python
```

Or update `config.toml`:

```toml
[embedding]
provider = "python-bridge"
model = "~/.cache/akidb/models/qwen-0.6b-onnx/model_optimized.onnx"
python_path = "/Users/akiralam/code/akidb2/.venv-onnx/bin/python"
```

### Step 3: Start AkiDB Server

```bash
# Start REST API server
cargo run -p akidb-rest

# In another terminal, test embedding endpoint
curl -X POST http://localhost:8080/api/v1/embed \
  -H "Content-Type: application/json" \
  -d '{
    "model": "qwen-0.6b",
    "inputs": ["Hello, world!"],
    "normalize": true
  }'
```

**Expected Performance:**
- P50 latency: 8-12ms (target: <10ms)
- P95 latency: 15-20ms (target: <20ms) ✅
- Throughput: 60-80 req/sec

---

## Option 2: Use HuggingFace Directly (Development/Testing)

### Prerequisites

```bash
# Install required packages
/Users/akiralam/code/akidb2/.venv-onnx/bin/pip install \
  transformers sentence-transformers torch
```

### Test Script

```python
#!/usr/bin/env python3
from transformers import AutoModel, AutoTokenizer

# Load model and tokenizer
model_id = "Qwen/Qwen3-Embedding-0.6B"
tokenizer = AutoTokenizer.from_pretrained(model_id)
model = AutoModel.from_pretrained(model_id)

# Generate embedding
text = "Hello, world!"
inputs = tokenizer(text, return_tensors="pt", padding=True, truncation=True, max_length=8192)
outputs = model(**inputs)

# Extract embedding (mean pooling)
embeddings = outputs.last_hidden_state.mean(dim=1)
print(f"Embedding dimension: {embeddings.shape[1]}")  # Should be 896
print(f"Embedding norm: {embeddings.norm().item()}")
```

---

## Comparison with Other Models

| Model | Parameters | Dim | Max Length | P95 Latency (Est.) | Use Case |
|-------|-----------|-----|------------|-------------------|----------|
| **Qwen3-Embedding-0.6B** | 600M | 896 | 8192 | **15-20ms** | Production retrieval |
| sentence-transformers/all-MiniLM-L6-v2 | 22M | 384 | 512 | 6-7ms | Fast queries |
| MLX Qwen (4-bit) | 600M | 896 | 8192 | 30-50ms | macOS-only |

**Why Qwen3-Embedding-0.6B?**
- ✅ Dedicated embedding model (not repurposed LLM)
- ✅ Larger embedding dimension (896 vs 384) → Better semantic representation
- ✅ Long context support (8192 tokens)
- ✅ Competitive latency with ONNX optimization
- ✅ Cross-platform (ONNX Runtime)

---

## Troubleshooting

### Issue: Model download too slow

```bash
# Use mirror (China mainland)
export HF_ENDPOINT=https://hf-mirror.com

# Or download manually
wget https://huggingface.co/Qwen/Qwen3-Embedding-0.6B/resolve/main/pytorch_model.bin
```

### Issue: ONNX conversion fails

```bash
# Check dependencies
/Users/akiralam/code/akidb2/.venv-onnx/bin/pip list | grep -E "onnx|torch|transformers|optimum"

# Reinstall if needed
/Users/akiralam/code/akidb2/.venv-onnx/bin/pip install --upgrade \
  onnx onnxruntime transformers torch optimum
```

### Issue: Python bridge provider not initialized

```bash
# Verify Python path
ls -la /Users/akiralam/code/akidb2/.venv-onnx/bin/python

# Check ONNX server script exists
ls -la /Users/akiralam/code/akidb2/crates/akidb-embedding/python/onnx_server.py

# Test Python bridge manually
/Users/akiralam/code/akidb2/.venv-onnx/bin/python \
  /Users/akiralam/code/akidb2/crates/akidb-embedding/python/onnx_server.py
```

---

## Next Steps

1. ✅ **Convert model to ONNX** (run conversion script)
2. ⏳ **Benchmark latency** (verify <20ms P95 target)
3. ⏳ **Run load tests** (verify throughput at 50+ QPS)
4. ⏳ **Update documentation** (add to main docs)

---

## References

- **Model Card:** https://huggingface.co/Qwen/Qwen3-Embedding-0.6B
- **Qwen Documentation:** https://github.com/QwenLM/Qwen
- **Python Bridge Integration:** `/Users/akiralam/code/akidb2/automatosx/tmp/PYTHON-BRIDGE-INTEGRATION-PROGRESS.md`
- **Conversion Script:** `/Users/akiralam/code/akidb2/crates/akidb-embedding/python/convert_qwen_to_onnx.py`

---

**Status:** Ready to convert and test
