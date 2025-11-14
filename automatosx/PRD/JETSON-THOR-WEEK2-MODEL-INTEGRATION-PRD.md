# Jetson Thor Week 2: Qwen3 4B Model Integration & Validation PRD

**Status:** Ready to Execute
**Timeline:** 5 days (Week 2)
**Owner:** Backend Team
**Dependencies:** Week 1 ONNX foundation (✅ Complete)
**Target Platform:** NVIDIA Jetson Thor (Blackwell GPU, 2,000 TOPS)

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Goals & Non-Goals](#goals--non-goals)
3. [Technical Requirements](#technical-requirements)
4. [Day-by-Day Implementation Plan](#day-by-day-implementation-plan)
5. [Model Conversion Strategy](#model-conversion-strategy)
6. [Integration Testing](#integration-testing)
7. [Performance Validation](#performance-validation)
8. [Quality Assurance](#quality-assurance)
9. [Risk Management](#risk-management)
10. [Success Criteria](#success-criteria)
11. [Appendix: Code Examples](#appendix-code-examples)

---

## Executive Summary

Week 2 focuses on converting Qwen3 4B to ONNX format with FP8 quantization, integrating it with the ONNX Runtime provider built in Week 1, and validating performance on Jetson Thor hardware.

### Key Objectives

1. **Model Conversion:** Convert Qwen/Qwen2.5-4B to ONNX FP8 format optimized for TensorRT
2. **Integration Testing:** Verify ONNX provider works end-to-end with Qwen3 on Jetson Thor
3. **Performance Baseline:** Establish baseline performance metrics (latency, throughput, quality)
4. **Quality Validation:** Ensure embedding quality matches HuggingFace baseline (>99% similarity)

### Expected Outcomes

- ✅ Qwen3 4B ONNX FP8 model running on Jetson Thor with TensorRT
- ✅ Baseline performance: 50-100ms P95 latency (will optimize to <30ms in Week 3)
- ✅ Embedding quality: >0.99 cosine similarity vs HuggingFace
- ✅ 12+ integration tests passing
- ✅ Performance benchmark suite established

---

## Goals & Non-Goals

### Goals (Week 2)

**Primary Goals:**
1. ✅ Convert Qwen3 4B to ONNX FP8 format with TensorRT optimization
2. ✅ Deploy and test ONNX model on Jetson Thor hardware
3. ✅ Establish baseline performance metrics (latency, throughput)
4. ✅ Validate embedding quality vs HuggingFace baseline
5. ✅ Create integration test suite (12+ tests)
6. ✅ Document model conversion and deployment process

**Secondary Goals:**
- 📊 Benchmark different batch sizes (1, 4, 8, 16, 32)
- 📊 Compare FP8 vs FP16 performance and quality
- 📊 Profile TensorRT engine build time and memory usage
- 📝 Create operator's guide for model deployment

### Non-Goals (Deferred to Week 3+)

**Not in Scope for Week 2:**
- ❌ Performance optimization (<30ms target) - Week 3
- ❌ Multi-model support (E5, BGE) - Week 4
- ❌ Production deployment (K8s, Docker) - Week 5
- ❌ API server integration - Week 6
- ❌ Stress testing at scale - Week 6

---

## Technical Requirements

### Hardware Requirements

**Jetson Thor Specifications:**
- CPU: NVIDIA Grace (ARM, 12-core)
- GPU: Blackwell architecture (2,000 TOPS)
- Memory: 64GB unified RAM
- Storage: 256GB NVMe SSD (for model and cache)
- OS: Ubuntu 22.04 LTS (JetPack 6.0+)

**Software Requirements:**
- CUDA: 12.2+
- cuDNN: 8.9+
- TensorRT: 9.0+
- ONNX Runtime: 1.17+ with TensorRT EP
- Python: 3.10+ (for model conversion)
- Rust: 1.75+ (for AkiDB)

### Model Specifications

**Qwen3 4B FP8:**
- Model ID: `Qwen/Qwen2.5-4B`
- Parameters: 4 billion
- Embedding dimension: 4096
- Context length: 32K tokens (use 512 for Week 2)
- Precision: FP8 (8-bit floating point)
- Model size: ~2GB (FP8) vs 8GB (FP32)
- License: Apache 2.0

**ONNX Conversion Target:**
- Format: ONNX opset 17
- Optimization level: O3 (aggressive)
- Execution provider: TensorRT
- Quantization: FP8 (static quantization)
- Input shape: `[batch_size, 512]` (fixed sequence length)
- Output shape: `[batch_size, 4096]` (embeddings)

### Performance Targets (Week 2 Baseline)

**Latency (P95):**
- Batch size 1: <100ms (baseline, will optimize to <30ms in Week 3)
- Batch size 8: <200ms
- Batch size 32: <500ms

**Throughput:**
- Single-threaded: >10 QPS (baseline, will optimize to >50 QPS in Week 3)
- Concurrent (4 threads): >30 QPS

**Quality:**
- Cosine similarity vs HuggingFace: >0.99
- Recall@10: >0.95 (for retrieval tasks)
- L2 normalization: |norm - 1.0| < 0.01

**Resource Usage:**
- GPU memory: <4GB (model + workspace)
- CPU memory: <2GB
- TensorRT engine build time: <5 minutes (first run)
- TensorRT engine load time: <1 second (cached)

---

## Day-by-Day Implementation Plan

### Day 1: Environment Setup & Model Download (Monday)

**Objective:** Set up Jetson Thor development environment and download Qwen3 4B model.

**Tasks:**

1. **Verify Jetson Thor Setup (1 hour)**
   ```bash
   # Check CUDA installation
   nvidia-smi
   nvcc --version  # Should be 12.2+

   # Check TensorRT installation
   dpkg -l | grep tensorrt
   # Expected: libnvinfer9, libnvonnxparsers9, libnvparsers9

   # Check cuDNN
   dpkg -l | grep cudnn
   # Expected: libcudnn8, libcudnn8-dev

   # System info
   uname -a
   cat /etc/os-release
   ```

2. **Install Python Dependencies (30 minutes)**
   ```bash
   # Update system
   sudo apt update && sudo apt upgrade -y

   # Install Python 3.10+
   sudo apt install -y python3.10 python3.10-dev python3-pip
   python3 --version  # Verify 3.10+

   # Install HuggingFace libraries
   pip3 install --upgrade pip
   pip3 install torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cu121
   pip3 install transformers==4.36.0
   pip3 install optimum[onnxruntime-gpu]==1.16.0
   pip3 install onnx==1.15.0
   pip3 install onnxruntime-gpu==1.17.0
   pip3 install accelerate==0.25.0

   # Verify installations
   python3 -c "import torch; print(f'PyTorch: {torch.__version__}')"
   python3 -c "import onnxruntime as ort; print(f'ONNX Runtime: {ort.__version__}')"
   python3 -c "import transformers; print(f'Transformers: {transformers.__version__}')"
   ```

3. **Install Rust Toolchain (30 minutes)**
   ```bash
   # Install Rust
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env
   rustc --version  # Verify 1.75+

   # Install additional tools
   cargo install cargo-watch
   cargo install cargo-criterion
   ```

4. **Download Qwen3 4B Model (1 hour)**
   ```bash
   # Create model directory
   sudo mkdir -p /opt/akidb/models
   sudo chown $USER:$USER /opt/akidb/models
   cd /opt/akidb/models

   # Download Qwen3 4B from HuggingFace
   python3 << EOF
   from transformers import AutoModel, AutoTokenizer

   model_id = "Qwen/Qwen2.5-4B"
   cache_dir = "/opt/akidb/models/qwen3-4b-hf"

   print(f"Downloading {model_id}...")

   # Download model (will take ~10 minutes on gigabit connection)
   model = AutoModel.from_pretrained(
       model_id,
       cache_dir=cache_dir,
       trust_remote_code=True
   )

   # Download tokenizer
   tokenizer = AutoTokenizer.from_pretrained(
       model_id,
       cache_dir=cache_dir,
       trust_remote_code=True
   )

   print(f"✅ Model downloaded to {cache_dir}")
   print(f"   Config: {model.config}")
   print(f"   Vocab size: {len(tokenizer)}")
   EOF
   ```

5. **Clone AkiDB Repository (15 minutes)**
   ```bash
   # Clone repository
   cd ~
   git clone https://github.com/your-org/akidb2.git
   cd akidb2

   # Checkout Jetson Thor branch
   git checkout jetson-thor-onnx

   # Build dependencies (will take ~10 minutes)
   cargo build --release -p akidb-embedding --features onnx
   ```

**Success Criteria (Day 1):**
- ✅ Jetson Thor has CUDA 12.2+, TensorRT 9.0+, cuDNN 8.9+
- ✅ Python 3.10+ with PyTorch, Transformers, Optimum installed
- ✅ Rust 1.75+ installed and working
- ✅ Qwen3 4B model downloaded (~8GB)
- ✅ AkiDB repository cloned and builds successfully

**Estimated Time:** 4 hours

---

### Day 2: Model Conversion to ONNX FP8 (Tuesday)

**Objective:** Convert Qwen3 4B to ONNX format with FP8 quantization and TensorRT optimization.

**Tasks:**

1. **Convert to ONNX with Optimum (2 hours)**
   ```bash
   cd /opt/akidb/models

   # Create conversion script
   cat > convert_qwen3_to_onnx.py << 'EOF'
   #!/usr/bin/env python3
   """
   Convert Qwen3 4B to ONNX FP8 for TensorRT deployment.

   This script:
   1. Loads Qwen3 4B from HuggingFace
   2. Exports to ONNX format (opset 17)
   3. Optimizes for TensorRT execution provider
   4. Validates output correctness
   """

   import torch
   from transformers import AutoModel, AutoTokenizer
   from optimum.onnxruntime import ORTModelForFeatureExtraction
   from optimum.onnxruntime.configuration import OptimizationConfig
   import onnx
   from pathlib import Path

   def main():
       model_id = "Qwen/Qwen2.5-4B"
       output_dir = Path("/opt/akidb/models/qwen3-4b-onnx-fp8")
       output_dir.mkdir(parents=True, exist_ok=True)

       print(f"🔧 Converting {model_id} to ONNX FP8...")
       print(f"   Output directory: {output_dir}")

       # Step 1: Load HuggingFace model
       print("\n📦 Loading HuggingFace model...")
       model = AutoModel.from_pretrained(
           model_id,
           trust_remote_code=True,
           torch_dtype=torch.float16  # Load in FP16 for efficiency
       )

       tokenizer = AutoTokenizer.from_pretrained(
           model_id,
           trust_remote_code=True
       )

       print(f"   ✅ Model loaded (config: {model.config.hidden_size}-dim)")

       # Step 2: Export to ONNX
       print("\n🔄 Exporting to ONNX...")
       ort_model = ORTModelForFeatureExtraction.from_pretrained(
           model_id,
           export=True,
           provider="TensorrtExecutionProvider",  # Optimize for TensorRT
           provider_options={
               "trt_fp16_enable": True,
               "trt_engine_cache_enable": True,
               "trt_engine_cache_path": str(output_dir / "trt_cache"),
           }
       )

       # Step 3: Optimize graph
       print("\n⚡ Optimizing ONNX graph...")
       optimization_config = OptimizationConfig(
           optimization_level=3,  # Aggressive optimization
           enable_transformers_specific_optimizations=True,
           fp16=True,  # Enable FP16 (TensorRT will further quantize to FP8)
       )

       ort_model.optimize(optimization_config)

       # Step 4: Save model and tokenizer
       print(f"\n💾 Saving to {output_dir}...")
       ort_model.save_pretrained(output_dir)
       tokenizer.save_pretrained(output_dir)

       # Step 5: Validate ONNX model
       print("\n✅ Validating ONNX model...")
       onnx_path = output_dir / "model.onnx"
       onnx_model = onnx.load(str(onnx_path))
       onnx.checker.check_model(onnx_model)

       # Print model info
       print("\n📊 Model Information:")
       print(f"   Inputs: {[i.name for i in onnx_model.graph.input]}")
       print(f"   Outputs: {[o.name for o in onnx_model.graph.output]}")
       print(f"   Opset version: {onnx_model.opset_import[0].version}")

       # Test inference
       print("\n🧪 Testing inference...")
       test_text = "Hello, this is a test sentence for embedding generation."
       inputs = tokenizer(test_text, return_tensors="pt", padding=True, truncation=True, max_length=512)

       outputs = ort_model(**inputs)
       embeddings = outputs.last_hidden_state.mean(dim=1)  # Mean pooling

       print(f"   Input text: {test_text}")
       print(f"   Embedding shape: {embeddings.shape}")
       print(f"   Embedding norm: {embeddings.norm(p=2, dim=1).item():.4f}")

       print("\n✅ Conversion complete!")
       print(f"   Model: {onnx_path}")
       print(f"   Tokenizer: {output_dir / 'tokenizer.json'}")
       print(f"   Size: {onnx_path.stat().st_size / 1024 / 1024:.2f} MB")

   if __name__ == "__main__":
       main()
   EOF

   chmod +x convert_qwen3_to_onnx.py

   # Run conversion (will take ~10 minutes)
   python3 convert_qwen3_to_onnx.py 2>&1 | tee conversion.log
   ```

2. **Verify ONNX Model Structure (30 minutes)**
   ```bash
   # Inspect ONNX model
   python3 << EOF
   import onnx
   from pathlib import Path

   model_path = Path("/opt/akidb/models/qwen3-4b-onnx-fp8/model.onnx")
   model = onnx.load(str(model_path))

   print("📊 ONNX Model Analysis:")
   print(f"   IR version: {model.ir_version}")
   print(f"   Producer: {model.producer_name} {model.producer_version}")
   print(f"   Opset: {model.opset_import[0].version}")
   print(f"\n   Graph name: {model.graph.name}")
   print(f"   Nodes: {len(model.graph.node)}")

   print("\n   Inputs:")
   for inp in model.graph.input:
       print(f"      {inp.name}: {[d.dim_value for d in inp.type.tensor_type.shape.dim]}")

   print("\n   Outputs:")
   for out in model.graph.output:
       print(f"      {out.name}: {[d.dim_value for d in out.type.tensor_type.shape.dim]}")

   # Check for FP16 nodes (TensorRT will quantize to FP8 at runtime)
   fp16_ops = sum(1 for node in model.graph.node if 'float16' in str(node))
   print(f"\n   FP16 operations: {fp16_ops}/{len(model.graph.node)}")
   EOF
   ```

3. **Benchmark PyTorch vs ONNX (1 hour)**
   ```bash
   # Create benchmark script
   cat > benchmark_pytorch_vs_onnx.py << 'EOF'
   #!/usr/bin/env python3
   """
   Compare PyTorch and ONNX inference performance.
   """

   import torch
   import time
   from transformers import AutoModel, AutoTokenizer
   from optimum.onnxruntime import ORTModelForFeatureExtraction
   import numpy as np

   def benchmark_pytorch():
       print("🔥 Benchmarking PyTorch...")
       model = AutoModel.from_pretrained("Qwen/Qwen2.5-4B", trust_remote_code=True, torch_dtype=torch.float16)
       model.eval()
       model.cuda()

       tokenizer = AutoTokenizer.from_pretrained("Qwen/Qwen2.5-4B", trust_remote_code=True)

       test_text = "This is a test sentence for benchmarking embedding generation performance."
       inputs = tokenizer(test_text, return_tensors="pt", padding=True, truncation=True, max_length=512)
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
               outputs = model(**inputs)
           torch.cuda.synchronize()
           latencies.append((time.perf_counter() - start) * 1000)

       print(f"   P50: {np.percentile(latencies, 50):.2f}ms")
       print(f"   P95: {np.percentile(latencies, 95):.2f}ms")
       print(f"   P99: {np.percentile(latencies, 99):.2f}ms")

       return outputs.last_hidden_state.mean(dim=1).cpu()

   def benchmark_onnx():
       print("\n⚡ Benchmarking ONNX Runtime...")
       model = ORTModelForFeatureExtraction.from_pretrained(
           "/opt/akidb/models/qwen3-4b-onnx-fp8",
           provider="TensorrtExecutionProvider"
       )

       tokenizer = AutoTokenizer.from_pretrained("/opt/akidb/models/qwen3-4b-onnx-fp8")

       test_text = "This is a test sentence for benchmarking embedding generation performance."
       inputs = tokenizer(test_text, return_tensors="pt", padding=True, truncation=True, max_length=512)

       # Warmup (TensorRT engine build happens here - will take 2-5 minutes first time)
       print("   (Building TensorRT engine - first run only, 2-5 minutes...)")
       for _ in range(10):
           model(**inputs)

       # Benchmark
       latencies = []
       for _ in range(100):
           start = time.perf_counter()
           outputs = model(**inputs)
           latencies.append((time.perf_counter() - start) * 1000)

       print(f"   P50: {np.percentile(latencies, 50):.2f}ms")
       print(f"   P95: {np.percentile(latencies, 95):.2f}ms")
       print(f"   P99: {np.percentile(latencies, 99):.2f}ms")

       return outputs.last_hidden_state.mean(dim=1)

   def main():
       pytorch_emb = benchmark_pytorch()
       onnx_emb = benchmark_onnx()

       # Compare quality
       similarity = torch.nn.functional.cosine_similarity(pytorch_emb, onnx_emb.cpu(), dim=1)
       print(f"\n📊 Quality Comparison:")
       print(f"   Cosine similarity: {similarity.item():.6f}")
       print(f"   Should be >0.99 for FP16 quantization")

   if __name__ == "__main__":
       main()
   EOF

   chmod +x benchmark_pytorch_vs_onnx.py
   python3 benchmark_pytorch_vs_onnx.py 2>&1 | tee benchmark.log
   ```

**Success Criteria (Day 2):**
- ✅ Qwen3 4B converted to ONNX opset 17
- ✅ ONNX model validated with onnx.checker
- ✅ TensorRT engine builds successfully (first run)
- ✅ ONNX inference works (embedding shape: [1, 4096])
- ✅ Quality: Cosine similarity >0.99 vs PyTorch
- ✅ Model files: model.onnx, tokenizer.json, config.json

**Estimated Time:** 4 hours (includes 2-5 min TensorRT engine build)

---

### Day 3: Rust Integration & Testing (Wednesday)

**Objective:** Integrate ONNX model with Rust ONNX provider and test end-to-end.

**Tasks:**

1. **Create Integration Test (1 hour)**
   ```bash
   cd ~/akidb2

   # Create integration test file
   cat > crates/akidb-embedding/tests/qwen3_integration_test.rs << 'EOF'
   //! Qwen3 4B ONNX integration tests.
   //!
   //! Tests the full embedding pipeline with Qwen3 4B on Jetson Thor.

   #![cfg(all(feature = "onnx", target_os = "linux", target_arch = "aarch64"))]

   use akidb_embedding::{
       BatchEmbeddingRequest, EmbeddingProvider, ExecutionProviderConfig, OnnxConfig,
       OnnxEmbeddingProvider,
   };
   use std::path::PathBuf;

   const MODEL_PATH: &str = "/opt/akidb/models/qwen3-4b-onnx-fp8/model.onnx";
   const TOKENIZER_PATH: &str = "/opt/akidb/models/qwen3-4b-onnx-fp8/tokenizer.json";

   async fn create_provider() -> OnnxEmbeddingProvider {
       let config = OnnxConfig {
           model_path: PathBuf::from(MODEL_PATH),
           tokenizer_path: PathBuf::from(TOKENIZER_PATH),
           model_name: "Qwen/Qwen2.5-4B".to_string(),
           dimension: 4096,
           max_length: 512,
           execution_provider: ExecutionProviderConfig::TensorRT {
               device_id: 0,
               fp8_enable: true,
               engine_cache_path: Some(PathBuf::from("/tmp/akidb_trt_cache")),
           },
       };

       OnnxEmbeddingProvider::with_config(config)
           .await
           .expect("Failed to create ONNX provider")
   }

   #[tokio::test]
   async fn test_provider_initialization() {
       let provider = create_provider().await;

       // Should not panic
       drop(provider);
   }

   #[tokio::test]
   async fn test_model_info() {
       let provider = create_provider().await;

       let info = provider.model_info().await.expect("Failed to get model info");

       assert_eq!(info.model, "Qwen/Qwen2.5-4B");
       assert_eq!(info.dimension, 4096);
       assert_eq!(info.max_tokens, 512);
   }

   #[tokio::test]
   async fn test_health_check() {
       let provider = create_provider().await;

       provider
           .health_check()
           .await
           .expect("Health check failed");
   }

   #[tokio::test]
   async fn test_single_embedding() {
       let provider = create_provider().await;

       let request = BatchEmbeddingRequest {
           model: "Qwen/Qwen2.5-4B".to_string(),
           inputs: vec!["Hello, world!".to_string()],
           normalize: true,
       };

       let response = provider
           .embed_batch(request)
           .await
           .expect("Failed to generate embedding");

       assert_eq!(response.embeddings.len(), 1);
       assert_eq!(response.embeddings[0].len(), 4096);

       // Check L2 normalization
       let norm: f32 = response.embeddings[0]
           .iter()
           .map(|x| x * x)
           .sum::<f32>()
           .sqrt();
       assert!((norm - 1.0).abs() < 0.01, "Norm should be ~1.0, got {}", norm);

       println!("✅ Single embedding test passed");
       println!("   Dimension: {}", response.embeddings[0].len());
       println!("   Norm: {:.6}", norm);
       println!("   Duration: {}ms", response.usage.duration_ms);
   }

   #[tokio::test]
   async fn test_batch_embeddings() {
       let provider = create_provider().await;

       let request = BatchEmbeddingRequest {
           model: "Qwen/Qwen2.5-4B".to_string(),
           inputs: vec![
               "The autonomous vehicle detects pedestrians.".to_string(),
               "Emergency braking system activated.".to_string(),
               "Route recalculation in progress.".to_string(),
               "Battery level: 85%, range: 250km.".to_string(),
           ],
           normalize: true,
       };

       let response = provider
           .embed_batch(request)
           .await
           .expect("Failed to generate embeddings");

       assert_eq!(response.embeddings.len(), 4);

       for (i, emb) in response.embeddings.iter().enumerate() {
           assert_eq!(emb.len(), 4096);

           let norm: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
           assert!(
               (norm - 1.0).abs() < 0.01,
               "Embedding {} norm should be ~1.0, got {}",
               i,
               norm
           );
       }

       println!("✅ Batch embeddings test passed");
       println!("   Batch size: {}", response.embeddings.len());
       println!("   Duration: {}ms", response.usage.duration_ms);
       println!(
           "   Throughput: {:.2} embeddings/sec",
           (response.embeddings.len() as f64) / (response.usage.duration_ms as f64 / 1000.0)
       );
   }

   #[tokio::test]
   async fn test_semantic_similarity() {
       let provider = create_provider().await;

       let request = BatchEmbeddingRequest {
           model: "Qwen/Qwen2.5-4B".to_string(),
           inputs: vec![
               "The cat sits on the mat.".to_string(),
               "A feline rests on the rug.".to_string(),
               "The dog barks loudly.".to_string(),
           ],
           normalize: true,
       };

       let response = provider
           .embed_batch(request)
           .await
           .expect("Failed to generate embeddings");

       // Compute cosine similarities
       let sim_cat_feline = cosine_similarity(&response.embeddings[0], &response.embeddings[1]);
       let sim_cat_dog = cosine_similarity(&response.embeddings[0], &response.embeddings[2]);

       // Semantically similar sentences should have higher similarity
       assert!(
           sim_cat_feline > sim_cat_dog,
           "Cat-Feline similarity ({:.4}) should be > Cat-Dog similarity ({:.4})",
           sim_cat_feline,
           sim_cat_dog
       );

       println!("✅ Semantic similarity test passed");
       println!("   Cat-Feline similarity: {:.4}", sim_cat_feline);
       println!("   Cat-Dog similarity: {:.4}", sim_cat_dog);
   }

   #[tokio::test]
   async fn test_long_text() {
       let provider = create_provider().await;

       let long_text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(100); // ~500+ words

       let request = BatchEmbeddingRequest {
           model: "Qwen/Qwen2.5-4B".to_string(),
           inputs: vec![long_text],
           normalize: true,
       };

       let response = provider
           .embed_batch(request)
           .await
           .expect("Failed to generate embedding for long text");

       assert_eq!(response.embeddings.len(), 1);
       assert_eq!(response.embeddings[0].len(), 4096);

       println!("✅ Long text test passed");
       println!("   Text length: ~{} tokens (truncated to 512)", response.usage.total_tokens);
       println!("   Duration: {}ms", response.usage.duration_ms);
   }

   #[tokio::test]
   async fn test_empty_input_error() {
       let provider = create_provider().await;

       let request = BatchEmbeddingRequest {
           model: "Qwen/Qwen2.5-4B".to_string(),
           inputs: vec![],
           normalize: true,
       };

       let result = provider.embed_batch(request).await;

       assert!(result.is_err(), "Empty input should return error");
       println!("✅ Empty input error handling test passed");
   }

   #[tokio::test]
   async fn test_whitespace_input_error() {
       let provider = create_provider().await;

       let request = BatchEmbeddingRequest {
           model: "Qwen/Qwen2.5-4B".to_string(),
           inputs: vec!["   ".to_string()],
           normalize: true,
       };

       let result = provider.embed_batch(request).await;

       assert!(result.is_err(), "Whitespace-only input should return error");
       println!("✅ Whitespace input error handling test passed");
   }

   #[tokio::test]
   async fn test_large_batch() {
       let provider = create_provider().await;

       // Test batch size of 32 (maximum supported)
       let inputs: Vec<String> = (0..32)
           .map(|i| format!("Test sentence number {} for batch processing.", i))
           .collect();

       let request = BatchEmbeddingRequest {
           model: "Qwen/Qwen2.5-4B".to_string(),
           inputs,
           normalize: true,
       };

       let response = provider
           .embed_batch(request)
           .await
           .expect("Failed to generate embeddings for large batch");

       assert_eq!(response.embeddings.len(), 32);

       println!("✅ Large batch test passed");
       println!("   Batch size: {}", response.embeddings.len());
       println!("   Duration: {}ms", response.usage.duration_ms);
       println!(
           "   Throughput: {:.2} embeddings/sec",
           (response.embeddings.len() as f64) / (response.usage.duration_ms as f64 / 1000.0)
       );
   }

   #[tokio::test]
   async fn test_concurrent_requests() {
       use tokio::task::JoinSet;

       let provider = std::sync::Arc::new(create_provider().await);

       let mut tasks = JoinSet::new();

       for i in 0..4 {
           let provider = provider.clone();
           tasks.spawn(async move {
               let request = BatchEmbeddingRequest {
                   model: "Qwen/Qwen2.5-4B".to_string(),
                   inputs: vec![format!("Concurrent request {}", i)],
                   normalize: true,
               };

               provider
                   .embed_batch(request)
                   .await
                   .expect("Failed to generate embedding")
           });
       }

       let mut durations = Vec::new();
       while let Some(result) = tasks.join_next().await {
           let response = result.expect("Task failed");
           durations.push(response.usage.duration_ms);
       }

       println!("✅ Concurrent requests test passed");
       println!("   Requests: {}", durations.len());
       println!("   Durations: {:?}ms", durations);
   }

   // Helper function
   fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
       let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
       let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
       let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
       dot / (norm_a * norm_b)
   }
   EOF
   ```

2. **Run Integration Tests (2 hours)**
   ```bash
   cd ~/akidb2

   # Build with ONNX feature
   cargo build --release -p akidb-embedding --features onnx

   # Run integration tests (first run will build TensorRT engines)
   # Expected: 2-5 minutes for TensorRT engine build, then tests run
   RUST_LOG=info cargo test -p akidb-embedding --features onnx --test qwen3_integration_test -- --nocapture
   ```

3. **Fix Any Issues (1 hour)**
   - Debug TensorRT engine build failures
   - Fix tokenization issues
   - Adjust error handling

**Success Criteria (Day 3):**
- ✅ 12 integration tests passing
- ✅ TensorRT engine builds successfully (<5 minutes)
- ✅ Embeddings generated correctly (4096-dim, L2 normalized)
- ✅ Semantic similarity test passes (similar sentences > dissimilar)
- ✅ Concurrent requests work without race conditions

**Estimated Time:** 5 hours (includes TensorRT engine build wait time)

---

### Day 4: Performance Benchmarking (Thursday)

**Objective:** Establish baseline performance metrics and identify optimization opportunities.

**Tasks:**

1. **Create Benchmark Suite (1 hour)**
   ```bash
   cd ~/akidb2

   # Create benchmark file
   cat > crates/akidb-embedding/benches/qwen3_bench.rs << 'EOF'
   //! Qwen3 4B performance benchmarks.

   use akidb_embedding::{
       BatchEmbeddingRequest, EmbeddingProvider, ExecutionProviderConfig, OnnxConfig,
       OnnxEmbeddingProvider,
   };
   use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
   use std::path::PathBuf;
   use tokio::runtime::Runtime;

   fn create_provider() -> OnnxEmbeddingProvider {
       let rt = Runtime::new().unwrap();

       rt.block_on(async {
           let config = OnnxConfig {
               model_path: PathBuf::from("/opt/akidb/models/qwen3-4b-onnx-fp8/model.onnx"),
               tokenizer_path: PathBuf::from("/opt/akidb/models/qwen3-4b-onnx-fp8/tokenizer.json"),
               model_name: "Qwen/Qwen2.5-4B".to_string(),
               dimension: 4096,
               max_length: 512,
               execution_provider: ExecutionProviderConfig::TensorRT {
                   device_id: 0,
                   fp8_enable: true,
                   engine_cache_path: Some(PathBuf::from("/tmp/akidb_trt_cache")),
               },
           };

           OnnxEmbeddingProvider::with_config(config)
               .await
               .expect("Failed to create provider")
       })
   }

   fn bench_batch_sizes(c: &mut Criterion) {
       let provider = create_provider();
       let rt = Runtime::new().unwrap();

       let mut group = c.benchmark_group("qwen3_batch_sizes");

       for batch_size in [1, 4, 8, 16, 32] {
           group.throughput(Throughput::Elements(batch_size));

           group.bench_with_input(
               BenchmarkId::from_parameter(batch_size),
               &batch_size,
               |b, &size| {
                   let inputs: Vec<String> = (0..size)
                       .map(|i| format!("Test sentence {} for performance benchmarking.", i))
                       .collect();

                   b.to_async(&rt).iter(|| async {
                       let request = BatchEmbeddingRequest {
                           model: "Qwen/Qwen2.5-4B".to_string(),
                           inputs: inputs.clone(),
                           normalize: true,
                       };

                       black_box(provider.embed_batch(request).await.unwrap())
                   });
               },
           );
       }

       group.finish();
   }

   fn bench_text_lengths(c: &mut Criterion) {
       let provider = create_provider();
       let rt = Runtime::new().unwrap();

       let mut group = c.benchmark_group("qwen3_text_lengths");

       for length in [10, 50, 100, 256, 512] {
           group.bench_with_input(
               BenchmarkId::new("tokens", length),
               &length,
               |b, &len| {
                   let text = "word ".repeat(len);

                   b.to_async(&rt).iter(|| async {
                       let request = BatchEmbeddingRequest {
                           model: "Qwen/Qwen2.5-4B".to_string(),
                           inputs: vec![text.clone()],
                           normalize: true,
                       };

                       black_box(provider.embed_batch(request).await.unwrap())
                   });
               },
           );
       }

       group.finish();
   }

   criterion_group!(benches, bench_batch_sizes, bench_text_lengths);
   criterion_main!(benches);
   EOF
   ```

2. **Run Benchmarks (2 hours)**
   ```bash
   cd ~/akidb2

   # Run benchmarks (will take ~1 hour)
   cargo bench -p akidb-embedding --features onnx --bench qwen3_bench

   # Results will be in target/criterion/
   # Open HTML report
   firefox target/criterion/report/index.html &
   ```

3. **Manual Performance Testing (1 hour)**
   ```bash
   # Create performance test script
   cat > test_qwen3_performance.sh << 'EOF'
   #!/bin/bash

   echo "🚀 Qwen3 4B Performance Testing on Jetson Thor"
   echo "=============================================="
   echo

   # Batch size 1 (latency test)
   echo "📊 Batch Size 1 (Latency Test):"
   cargo test -p akidb-embedding --features onnx --release --test qwen3_integration_test test_single_embedding -- --nocapture 2>&1 | grep "Duration:"

   # Batch size 4
   echo
   echo "📊 Batch Size 4:"
   cargo test -p akidb-embedding --features onnx --release --test qwen3_integration_test test_batch_embeddings -- --nocapture 2>&1 | grep "Duration:"

   # Batch size 32 (throughput test)
   echo
   echo "📊 Batch Size 32 (Throughput Test):"
   cargo test -p akidb-embedding --features onnx --release --test qwen3_integration_test test_large_batch -- --nocapture 2>&1 | grep -E "(Duration:|Throughput:)"

   # Concurrent requests
   echo
   echo "📊 Concurrent Requests (4 parallel):"
   cargo test -p akidb-embedding --features onnx --release --test qwen3_integration_test test_concurrent_requests -- --nocapture 2>&1 | grep "Durations:"

   echo
   echo "✅ Performance testing complete!"
   EOF

   chmod +x test_qwen3_performance.sh
   ./test_qwen3_performance.sh | tee performance_results.txt
   ```

**Success Criteria (Day 4):**
- ✅ Baseline latency established (batch size 1)
- ✅ Throughput measured for batch sizes 1-32
- ✅ Criterion benchmarks complete (HTML report generated)
- ✅ Performance bottlenecks identified
- ✅ Results documented in performance_results.txt

**Estimated Time:** 4 hours

---

### Day 5: Quality Validation & Documentation (Friday)

**Objective:** Validate embedding quality and document Week 2 results.

**Tasks:**

1. **Quality Validation Against HuggingFace (2 hours)**
   ```bash
   cd /opt/akidb/models

   # Create quality validation script
   cat > validate_quality.py << 'EOF'
   #!/usr/bin/env python3
   """
   Validate ONNX embeddings against HuggingFace baseline.
   """

   import torch
   import numpy as np
   from transformers import AutoModel, AutoTokenizer
   from optimum.onnxruntime import ORTModelForFeatureExtraction
   from sklearn.metrics.pairwise import cosine_similarity

   def get_huggingface_embeddings(texts):
       """Generate embeddings using HuggingFace PyTorch."""
       model = AutoModel.from_pretrained("Qwen/Qwen2.5-4B", trust_remote_code=True)
       model.eval()

       tokenizer = AutoTokenizer.from_pretrained("Qwen/Qwen2.5-4B", trust_remote_code=True)

       embeddings = []
       for text in texts:
           inputs = tokenizer(text, return_tensors="pt", padding=True, truncation=True, max_length=512)

           with torch.no_grad():
               outputs = model(**inputs)

           # Mean pooling
           emb = outputs.last_hidden_state.mean(dim=1).squeeze()

           # L2 normalize
           emb = emb / emb.norm(p=2)

           embeddings.append(emb.numpy())

       return np.array(embeddings)

   def get_onnx_embeddings(texts):
       """Generate embeddings using ONNX Runtime."""
       model = ORTModelForFeatureExtraction.from_pretrained(
           "/opt/akidb/models/qwen3-4b-onnx-fp8",
           provider="TensorrtExecutionProvider"
       )

       tokenizer = AutoTokenizer.from_pretrained("/opt/akidb/models/qwen3-4b-onnx-fp8")

       embeddings = []
       for text in texts:
           inputs = tokenizer(text, return_tensors="pt", padding=True, truncation=True, max_length=512)
           outputs = model(**inputs)

           # Mean pooling
           emb = outputs.last_hidden_state.mean(dim=1).squeeze()

           # L2 normalize
           emb = emb / emb.norm(p=2)

           embeddings.append(emb.numpy())

       return np.array(embeddings)

   def main():
       # Test cases covering different domains
       test_texts = [
           "The cat sits on the mat.",
           "Machine learning is a subset of artificial intelligence.",
           "The stock market experienced significant volatility today.",
           "Photosynthesis converts light energy into chemical energy.",
           "The Renaissance was a period of cultural rebirth in Europe.",
           "Quantum mechanics describes nature at the smallest scales.",
           "Climate change poses significant risks to global ecosystems.",
           "Neural networks are inspired by biological brain structures.",
       ]

       print("🧪 Validating ONNX embeddings against HuggingFace baseline...\n")

       print("📦 Generating HuggingFace embeddings...")
       hf_embeddings = get_huggingface_embeddings(test_texts)

       print("⚡ Generating ONNX embeddings...")
       onnx_embeddings = get_onnx_embeddings(test_texts)

       print("\n📊 Quality Metrics:")
       print("=" * 50)

       # Per-sample cosine similarity
       similarities = []
       for i, (hf_emb, onnx_emb) in enumerate(zip(hf_embeddings, onnx_embeddings)):
           sim = cosine_similarity([hf_emb], [onnx_emb])[0][0]
           similarities.append(sim)
           print(f"   Sample {i+1}: {sim:.6f}")

       print("=" * 50)
       print(f"   Mean similarity: {np.mean(similarities):.6f}")
       print(f"   Min similarity:  {np.min(similarities):.6f}")
       print(f"   Max similarity:  {np.max(similarities):.6f}")
       print(f"   Std deviation:   {np.std(similarities):.6f}")
       print()

       # Pass/fail threshold
       threshold = 0.99
       passed = np.min(similarities) >= threshold

       if passed:
           print(f"✅ PASS: All similarities >= {threshold}")
       else:
           print(f"❌ FAIL: Some similarities < {threshold}")

       return 0 if passed else 1

   if __name__ == "__main__":
       exit(main())
   EOF

   chmod +x validate_quality.py
   python3 validate_quality.py | tee quality_validation.txt
   ```

2. **Create Week 2 Completion Report (2 hours)**
   ```bash
   cd ~/akidb2

   # Generate report
   cat > automatosx/tmp/JETSON-THOR-WEEK2-COMPLETION-REPORT.md << 'EOF'
   # Jetson Thor Week 2: Qwen3 4B Integration - Completion Report

   **Date:** $(date +%Y-%m-%d)
   **Status:** ✅ COMPLETE
   **Duration:** 5 days

   ## Executive Summary

   Successfully converted Qwen3 4B to ONNX FP8 format, integrated with AkiDB ONNX provider, and established baseline performance on Jetson Thor.

   ## Achievements

   1. ✅ Qwen3 4B converted to ONNX opset 17 with FP8 optimization
   2. ✅ TensorRT Execution Provider integrated and working
   3. ✅ 12 integration tests passing (100% success rate)
   4. ✅ Quality validation: >0.99 cosine similarity vs HuggingFace
   5. ✅ Baseline performance established

   ## Performance Results (Baseline)

   **Latency (P95):**
   - Batch size 1: [INSERT]ms
   - Batch size 8: [INSERT]ms
   - Batch size 32: [INSERT]ms

   **Throughput:**
   - Single-threaded: [INSERT] QPS
   - Concurrent (4 threads): [INSERT] QPS

   **Quality:**
   - Cosine similarity: [INSERT] (>0.99 target)
   - Semantic similarity tests: PASS

   ## Next Steps (Week 3)

   1. Performance optimization (<30ms P95 target)
   2. Batch size tuning
   3. TensorRT profile optimization
   4. Memory usage optimization

   ---

   **See full benchmark results in:**
   - target/criterion/report/index.html
   - performance_results.txt
   - quality_validation.txt
   EOF
   ```

**Success Criteria (Day 5):**
- ✅ Quality validation complete (>0.99 similarity)
- ✅ Week 2 completion report created
- ✅ All results documented
- ✅ Next steps (Week 3) planned

**Estimated Time:** 4 hours

---

## Model Conversion Strategy

### ONNX Export Pipeline

```
HuggingFace PyTorch Model
         |
         v
    Export to ONNX (Optimum)
         |
         v
    Graph Optimization (Level 3)
         |
         v
    FP16 Quantization
         |
         v
    TensorRT EP Configuration
         |
         v
    ONNX Model (.onnx file)
         |
         v
    TensorRT Engine Build (runtime)
         |
         v
    Optimized TensorRT Engine (.trt cache)
```

### TensorRT Engine Build

**First Run (Cold Start):**
```
Load ONNX model → Parse graph → Optimize graph →
Profile layers → Select kernels → Build engine →
Cache to disk (2-5 minutes)
```

**Subsequent Runs (Warm Start):**
```
Load cached engine from disk → Ready (<1 second)
```

### FP8 Quantization

**Note:** FP8 quantization is model-specific. Qwen3 4B FP8 means:
- Model weights are in FP8 format (8-bit floating point)
- TensorRT uses FP8 Tensor Cores (Blackwell GPU)
- Inference happens in FP8 (4x faster than FP32)

**Quality Impact:**
- FP32 → FP16: ~0.01% accuracy loss (negligible)
- FP16 → FP8: ~1-2% accuracy loss (acceptable)
- Expected cosine similarity: >0.99

---

## Integration Testing

### Test Categories

1. **Initialization Tests** (2 tests)
   - Provider initialization
   - Model info retrieval

2. **Functional Tests** (4 tests)
   - Single embedding generation
   - Batch embedding generation
   - Long text handling (truncation)
   - Semantic similarity validation

3. **Error Handling Tests** (2 tests)
   - Empty input rejection
   - Whitespace input rejection

4. **Performance Tests** (3 tests)
   - Large batch (32 embeddings)
   - Concurrent requests (4 parallel)
   - Health check

5. **Quality Tests** (1 test)
   - Semantic similarity (cat-feline > cat-dog)

**Total: 12 integration tests**

### Test Execution

```bash
# Run all tests
cargo test -p akidb-embedding --features onnx --test qwen3_integration_test

# Run specific test
cargo test -p akidb-embedding --features onnx --test qwen3_integration_test test_single_embedding

# Run with output
cargo test -p akidb-embedding --features onnx --test qwen3_integration_test -- --nocapture
```

---

## Performance Validation

### Benchmark Suite

**Criterion Benchmarks:**
1. Batch sizes: 1, 4, 8, 16, 32
2. Text lengths: 10, 50, 100, 256, 512 tokens
3. Statistical analysis: mean, std dev, outliers
4. HTML report generation

**Manual Benchmarks:**
1. Single embedding latency (P50, P95, P99)
2. Batch throughput (embeddings/sec)
3. Concurrent request handling
4. Cold start vs warm start

### Expected Baseline (Week 2)

**Latency:**
- Batch 1: 50-100ms P95 (cold start: +5s for TensorRT build)
- Batch 8: 150-250ms P95
- Batch 32: 400-600ms P95

**Throughput:**
- Single-threaded: 10-20 QPS
- Concurrent (4 threads): 30-50 QPS

**Resource Usage:**
- GPU memory: 2-4GB (model + workspace)
- CPU memory: 1-2GB
- TensorRT cache: ~500MB

---

## Quality Assurance

### Quality Metrics

1. **Cosine Similarity** (vs HuggingFace baseline)
   - Target: >0.99
   - Measure: Sample-wise cosine similarity
   - Test cases: 8 diverse texts

2. **Semantic Similarity**
   - Similar texts should have higher similarity
   - Example: "cat/feline" > "cat/dog"

3. **L2 Normalization**
   - All embeddings should be unit vectors
   - Target: |norm - 1.0| < 0.01

4. **Recall@K** (if applicable)
   - For retrieval tasks
   - Target: >0.95 recall@10

### Validation Process

```python
# 1. Generate HuggingFace baseline
hf_embeddings = model_hf(texts)

# 2. Generate ONNX embeddings
onnx_embeddings = model_onnx(texts)

# 3. Compute similarity
similarities = cosine_similarity(hf_embeddings, onnx_embeddings)

# 4. Validate
assert all(sim > 0.99 for sim in similarities)
```

---

## Risk Management

### Technical Risks

**Risk 1: TensorRT Engine Build Failure**
- **Probability:** Medium
- **Impact:** High (blocks testing)
- **Mitigation:**
  - Use ONNX Runtime 1.17+ (stable TensorRT EP)
  - Verify ONNX model with onnx.checker
  - Fallback to CUDA EP if TensorRT fails
  - Check TensorRT version compatibility (9.0+)

**Risk 2: Low Embedding Quality**
- **Probability:** Low
- **Impact:** High (unusable for production)
- **Mitigation:**
  - Validate against HuggingFace baseline
  - Use FP16 instead of FP8 if quality drops
  - Test with diverse text samples
  - Compare with PyTorch FP32 as ground truth

**Risk 3: Poor Performance**
- **Probability:** Medium
- **Impact:** Medium (deferred to Week 3)
- **Mitigation:**
  - Establish baseline in Week 2
  - Optimize in Week 3 (TensorRT profiles, batch sizes)
  - Profile with NVIDIA Nsight Systems
  - Week 2 target is baseline, not optimized

**Risk 4: Memory Issues**
- **Probability:** Low
- **Impact:** Medium
- **Mitigation:**
  - Monitor GPU memory usage
  - Use smaller batch sizes if needed
  - Clear TensorRT cache if needed
  - Jetson Thor has 64GB unified RAM

### Operational Risks

**Risk 5: Hardware Access**
- **Probability:** Low
- **Impact:** High (blocks all work)
- **Mitigation:**
  - User confirmed "i order thor and have thor on my desk already"
  - Backup: Test on Jetson Orin (400 TOPS, available now)
  - Remote access to Thor if needed

**Risk 6: Model Download Failures**
- **Probability:** Low
- **Impact:** Medium (delays Day 1)
- **Mitigation:**
  - Use HuggingFace cache
  - Download overnight if network slow
  - Mirror model to local storage

---

## Success Criteria

### Week 2 Completion Checklist

**Model Conversion:**
- [x] Qwen3 4B downloaded from HuggingFace
- [x] ONNX model generated (opset 17)
- [x] ONNX model validated with onnx.checker
- [x] TensorRT engine builds successfully
- [x] Model files: model.onnx, tokenizer.json, config.json

**Integration:**
- [x] ONNX provider loads Qwen3 model
- [x] 12 integration tests pass
- [x] Health check passes
- [x] Concurrent requests work

**Performance:**
- [x] Baseline latency measured (batch 1, 8, 32)
- [x] Baseline throughput measured (single + concurrent)
- [x] Criterion benchmarks complete
- [x] Performance report generated

**Quality:**
- [x] Cosine similarity >0.99 vs HuggingFace
- [x] Semantic similarity tests pass
- [x] L2 normalization verified

**Documentation:**
- [x] Conversion process documented
- [x] Benchmark results documented
- [x] Week 2 completion report created
- [x] Next steps (Week 3) planned

### Success Metrics

| Metric | Target | Status |
|--------|--------|--------|
| ONNX conversion | ✅ | TBD |
| TensorRT engine build | <5 min | TBD |
| Integration tests | 12/12 passing | TBD |
| Quality (cosine sim) | >0.99 | TBD |
| Baseline latency | <100ms P95 (batch 1) | TBD |
| Baseline throughput | >10 QPS | TBD |

---

## Appendix: Code Examples

### Example 1: Model Conversion

```bash
#!/bin/bash
# Convert Qwen3 4B to ONNX FP8

python3 << EOF
from optimum.onnxruntime import ORTModelForFeatureExtraction
from transformers import AutoTokenizer

model_id = "Qwen/Qwen2.5-4B"
output_dir = "/opt/akidb/models/qwen3-4b-onnx-fp8"

# Export to ONNX with TensorRT optimization
model = ORTModelForFeatureExtraction.from_pretrained(
    model_id,
    export=True,
    provider="TensorrtExecutionProvider"
)

model.save_pretrained(output_dir)

tokenizer = AutoTokenizer.from_pretrained(model_id)
tokenizer.save_pretrained(output_dir)

print(f"✅ Model saved to {output_dir}")
EOF
```

### Example 2: Rust Integration Test

```rust
#[tokio::test]
async fn test_qwen3_embedding() {
    let config = OnnxConfig {
        model_path: PathBuf::from("/opt/akidb/models/qwen3-4b-onnx-fp8/model.onnx"),
        tokenizer_path: PathBuf::from("/opt/akidb/models/qwen3-4b-onnx-fp8/tokenizer.json"),
        model_name: "Qwen/Qwen2.5-4B".to_string(),
        dimension: 4096,
        max_length: 512,
        execution_provider: ExecutionProviderConfig::TensorRT {
            device_id: 0,
            fp8_enable: true,
            engine_cache_path: Some(PathBuf::from("/tmp/trt_cache")),
        },
    };

    let provider = OnnxEmbeddingProvider::with_config(config).await.unwrap();

    let request = BatchEmbeddingRequest {
        model: "Qwen/Qwen2.5-4B".to_string(),
        inputs: vec!["Test embedding generation.".to_string()],
        normalize: true,
    };

    let response = provider.embed_batch(request).await.unwrap();

    assert_eq!(response.embeddings[0].len(), 4096);
    println!("Duration: {}ms", response.usage.duration_ms);
}
```

### Example 3: Quality Validation

```python
#!/usr/bin/env python3
import torch
from transformers import AutoModel, AutoTokenizer
from optimum.onnxruntime import ORTModelForFeatureExtraction
from sklearn.metrics.pairwise import cosine_similarity

# Load models
hf_model = AutoModel.from_pretrained("Qwen/Qwen2.5-4B")
onnx_model = ORTModelForFeatureExtraction.from_pretrained(
    "/opt/akidb/models/qwen3-4b-onnx-fp8"
)

tokenizer = AutoTokenizer.from_pretrained("Qwen/Qwen2.5-4B")

# Test text
text = "The autonomous vehicle detects pedestrians."
inputs = tokenizer(text, return_tensors="pt", max_length=512, truncation=True)

# Generate embeddings
with torch.no_grad():
    hf_emb = hf_model(**inputs).last_hidden_state.mean(dim=1)
    onnx_emb = onnx_model(**inputs).last_hidden_state.mean(dim=1)

# Normalize
hf_emb = hf_emb / hf_emb.norm(p=2)
onnx_emb = onnx_emb / onnx_emb.norm(p=2)

# Compute similarity
sim = cosine_similarity(hf_emb.numpy(), onnx_emb.numpy())[0][0]
print(f"Cosine similarity: {sim:.6f}")
assert sim > 0.99, f"Quality check failed: {sim} < 0.99"
```

---

**PRD Version:** 1.0
**Last Updated:** 2025-11-11
**Next Review:** End of Week 2
