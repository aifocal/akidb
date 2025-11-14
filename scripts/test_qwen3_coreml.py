#!/usr/bin/env python3
"""
Test Qwen3-Embedding-0.6B with ONNX Runtime CoreML Execution Provider.

This script:
1. Tests ONNX Runtime with CoreML EP activation
2. Measures inference performance (latency, throughput)
3. Validates embedding quality (normalization, dimensions)
4. Compares CPU vs CoreML EP performance
5. Tests batch processing

Model: Qwen3-Embedding-0.6B-ONNX (1024-dim embeddings)
Expected: <20ms single text with CoreML EP on Apple Silicon
"""

import sys
import time
import numpy as np
from pathlib import Path
from typing import List, Tuple
import onnxruntime as ort
from transformers import AutoTokenizer


def normalize_embeddings(embeddings: np.ndarray) -> np.ndarray:
    """L2 normalize embeddings."""
    norms = np.linalg.norm(embeddings, axis=1, keepdims=True)
    return embeddings / norms


def get_detailed_instruct(task_description: str, query: str) -> str:
    """Format query with task instruction (Qwen3-Embedding requirement)."""
    return f"Instruct: {task_description}\nQuery: {query}"


def last_token_pool(
    last_hidden_states: np.ndarray,
    attention_mask: np.ndarray
) -> np.ndarray:
    """
    Pool embeddings by taking the last non-padding token.

    Args:
        last_hidden_states: Shape (batch, seq_len, hidden_dim)
        attention_mask: Shape (batch, seq_len)

    Returns:
        embeddings: Shape (batch, hidden_dim)
    """
    batch_size = last_hidden_states.shape[0]
    sequence_lengths = attention_mask.sum(axis=1) - 1  # Last token index

    # Extract last token embeddings for each sequence
    embeddings = np.zeros((batch_size, last_hidden_states.shape[2]), dtype=last_hidden_states.dtype)
    for i in range(batch_size):
        last_idx = int(sequence_lengths[i])
        embeddings[i] = last_hidden_states[i, last_idx, :]

    return embeddings


def create_session(
    model_path: str,
    use_coreml: bool = True,
    verbose: bool = True
) -> ort.InferenceSession:
    """Create ONNX Runtime session with optional CoreML EP."""

    providers = []
    if use_coreml:
        # CoreML Execution Provider configuration for Mac
        coreml_options = {
            'MLComputeUnits': 'ALL',  # Use all compute units (GPU+ANE+CPU)
            'ModelFormat': 'MLProgram',  # Newer CoreML format (macOS 12+)
            'RequireStaticInputShapes': False,  # Allow dynamic shapes
            'EnableOnSubgraphs': False,  # Disable for compatibility
        }
        providers.append(('CoreMLExecutionProvider', coreml_options))

    # CPU fallback
    providers.append('CPUExecutionProvider')

    # Create session
    session = ort.InferenceSession(
        model_path,
        providers=providers
    )

    if verbose:
        print(f"📦 Session created with providers:")
        for provider in session.get_providers():
            print(f"   - {provider}")

    return session


def embed_texts(
    session: ort.InferenceSession,
    tokenizer,
    texts: List[str],
    task_description: str = None,
    max_length: int = 512,
    normalize: bool = True
) -> Tuple[np.ndarray, dict]:
    """
    Generate embeddings for texts using ONNX Runtime.

    Returns:
        embeddings: Shape (batch, hidden_dim), L2-normalized
        metadata: Dict with timing and other info
    """
    # Add task instruction if provided
    if task_description:
        texts = [get_detailed_instruct(task_description, text) for text in texts]

    # Tokenize
    start_tokenize = time.perf_counter()
    inputs = tokenizer(
        texts,
        padding=True,
        truncation=True,
        max_length=max_length,
        return_tensors="np"
    )
    tokenize_time = time.perf_counter() - start_tokenize

    # Prepare ONNX inputs (need to handle optional KV cache inputs)
    batch_size = inputs['input_ids'].shape[0]
    seq_len = inputs['input_ids'].shape[1]

    # Required inputs
    onnx_inputs = {
        'input_ids': inputs['input_ids'].astype(np.int64),
        'attention_mask': inputs['attention_mask'].astype(np.int64),
    }

    # Optional: position_ids (auto-generate if not provided)
    position_ids = np.arange(seq_len, dtype=np.int64).reshape(1, -1).repeat(batch_size, axis=0)
    onnx_inputs['position_ids'] = position_ids

    # Optional: past_key_values (initialize as zeros for first pass)
    # For embedding, we don't need KV cache, but model expects these inputs
    # Initialize with zeros and shape [batch, num_heads, 0, head_dim] for past_sequence_length=0
    for i in range(28):  # Qwen3-0.6B has 28 layers
        # Empty KV cache (past_sequence_length = 0)
        empty_kv = np.zeros((batch_size, 8, 0, 128), dtype=np.float16)
        onnx_inputs[f'past_key_values.{i}.key'] = empty_kv
        onnx_inputs[f'past_key_values.{i}.value'] = empty_kv

    # Run inference
    start_inference = time.perf_counter()
    outputs = session.run(None, onnx_inputs)
    inference_time = time.perf_counter() - start_inference

    # Extract last_hidden_state (first output)
    last_hidden_states = outputs[0]  # Shape: (batch, seq_len, 1024)

    # Apply last token pooling
    start_pool = time.perf_counter()
    embeddings = last_token_pool(last_hidden_states, inputs['attention_mask'])
    pool_time = time.perf_counter() - start_pool

    # L2 normalize
    if normalize:
        embeddings = normalize_embeddings(embeddings)

    total_time = time.perf_counter() - start_tokenize

    metadata = {
        'tokenize_ms': tokenize_time * 1000,
        'inference_ms': inference_time * 1000,
        'pool_ms': pool_time * 1000,
        'total_ms': total_time * 1000,
        'batch_size': batch_size,
        'seq_len': seq_len,
    }

    return embeddings, metadata


def test_single_text_performance(
    session: ort.InferenceSession,
    tokenizer,
    num_runs: int = 10
):
    """Test single text embedding performance."""
    print(f"\n{'='*70}")
    print(f"Test 1: Single Text Performance ({num_runs} runs)")
    print(f"{'='*70}")

    task = "Given a search query, retrieve relevant documents"
    text = "What is the capital of France?"

    # Warmup (first run is slower)
    print(f"🔥 Warmup run...")
    _, warmup_meta = embed_texts(session, tokenizer, [text], task_description=task)
    print(f"   Warmup time: {warmup_meta['total_ms']:.2f}ms")

    # Benchmark runs
    print(f"\n📊 Running {num_runs} iterations...")
    timings = []
    for i in range(num_runs):
        embeddings, meta = embed_texts(session, tokenizer, [text], task_description=task)
        timings.append(meta['total_ms'])

        if i == 0:
            # Verify embedding quality on first run
            norm = np.linalg.norm(embeddings[0])
            dim = embeddings.shape[1]
            print(f"\n✅ First run validation:")
            print(f"   Embedding dimension: {dim}")
            print(f"   L2 norm: {norm:.6f} (should be ~1.0)")
            print(f"   First 5 values: {embeddings[0][:5]}")

    # Statistics
    timings = np.array(timings)
    print(f"\n📈 Performance Statistics:")
    print(f"   Mean:   {np.mean(timings):.2f}ms")
    print(f"   Median: {np.median(timings):.2f}ms")
    print(f"   P95:    {np.percentile(timings, 95):.2f}ms")
    print(f"   P99:    {np.percentile(timings, 99):.2f}ms")
    print(f"   Min:    {np.min(timings):.2f}ms")
    print(f"   Max:    {np.max(timings):.2f}ms")

    # Check if target met
    p95 = np.percentile(timings, 95)
    if p95 < 20:
        print(f"\n✅ TARGET MET: P95 {p95:.2f}ms < 20ms")
    else:
        print(f"\n⚠️  TARGET MISSED: P95 {p95:.2f}ms >= 20ms")

    return np.median(timings)


def test_batch_performance(
    session: ort.InferenceSession,
    tokenizer,
    batch_sizes: List[int] = [1, 2, 4, 8, 16, 32]
):
    """Test batch processing performance."""
    print(f"\n{'='*70}")
    print(f"Test 2: Batch Processing Performance")
    print(f"{'='*70}")

    task = "Given a search query, retrieve relevant documents"
    test_texts = [
        "What is machine learning?",
        "How does neural network work?",
        "Explain deep learning",
        "What is artificial intelligence?",
    ]

    print(f"\n{'Batch Size':<12} {'Total (ms)':<12} {'Per Text (ms)':<15} {'Throughput (QPS)':<20}")
    print(f"{'-'*12} {'-'*12} {'-'*15} {'-'*20}")

    for batch_size in batch_sizes:
        # Create batch by repeating test texts
        texts = (test_texts * ((batch_size // len(test_texts)) + 1))[:batch_size]

        # Warmup
        _, _ = embed_texts(session, tokenizer, texts, task_description=task)

        # Benchmark
        embeddings, meta = embed_texts(session, tokenizer, texts, task_description=task)

        per_text_ms = meta['total_ms'] / batch_size
        qps = 1000 / per_text_ms

        print(f"{batch_size:<12} {meta['total_ms']:<12.2f} {per_text_ms:<15.2f} {qps:<20.1f}")

    print()


def test_embedding_quality(
    session: ort.InferenceSession,
    tokenizer
):
    """Test embedding quality and similarity."""
    print(f"\n{'='*70}")
    print(f"Test 3: Embedding Quality & Similarity")
    print(f"{'='*70}")

    task = "Given a search query, retrieve relevant documents"

    # Similar queries
    queries = [
        "What is the capital of France?",
        "What is the capital city of France?",
    ]

    # Different query
    different = ["How to cook pasta?"]

    # Generate embeddings
    query_embs, _ = embed_texts(session, tokenizer, queries, task_description=task)
    diff_emb, _ = embed_texts(session, tokenizer, different, task_description=task)

    # Compute cosine similarity
    def cosine_similarity(a, b):
        return np.dot(a, b) / (np.linalg.norm(a) * np.linalg.norm(b))

    similar_score = cosine_similarity(query_embs[0], query_embs[1])
    different_score = cosine_similarity(query_embs[0], diff_emb[0])

    print(f"\n📏 Similarity Scores:")
    print(f"   Similar queries:    {similar_score:.4f}")
    print(f"   Different queries:  {different_score:.4f}")
    print(f"   Difference:         {similar_score - different_score:.4f}")

    if similar_score > different_score:
        print(f"\n✅ QUALITY CHECK PASSED: Similar queries have higher similarity")
    else:
        print(f"\n⚠️  QUALITY CHECK FAILED: Similar queries should have higher similarity")


def compare_providers(
    model_path: str,
    tokenizer
):
    """Compare CoreML EP vs CPU performance."""
    print(f"\n{'='*70}")
    print(f"Test 4: CoreML EP vs CPU Comparison")
    print(f"{'='*70}")

    task = "Given a search query, retrieve relevant documents"
    text = "What is the capital of France?"

    # Test CoreML EP
    print(f"\n🚀 Testing CoreML Execution Provider...")
    coreml_session = create_session(model_path, use_coreml=True, verbose=True)
    coreml_time = test_single_text_performance(coreml_session, tokenizer, num_runs=10)

    # Test CPU
    print(f"\n💻 Testing CPU Execution Provider...")
    cpu_session = create_session(model_path, use_coreml=False, verbose=True)
    cpu_time = test_single_text_performance(cpu_session, tokenizer, num_runs=10)

    # Compare
    speedup = cpu_time / coreml_time
    print(f"\n📊 Provider Comparison:")
    print(f"   CoreML EP: {coreml_time:.2f}ms")
    print(f"   CPU:       {cpu_time:.2f}ms")
    print(f"   Speedup:   {speedup:.2f}x")

    if speedup > 1.2:
        print(f"\n✅ CoreML EP is faster ({speedup:.2f}x speedup)")
    elif speedup < 0.8:
        print(f"\n⚠️  CoreML EP is slower ({1/speedup:.2f}x slowdown)")
    else:
        print(f"\n⚠️  Performance is similar (check if CoreML EP is actually used)")


def main():
    # Model paths
    fp16_model_path = "models/qwen3-embedding-0.6b/onnx/model_fp16.onnx"
    fp32_model_path = "models/qwen3-embedding-0.6b/onnx/model.onnx"

    # Use FP16 if available (recommended for CoreML)
    model_path = fp16_model_path if Path(fp16_model_path).exists() else fp32_model_path

    if not Path(model_path).exists():
        print(f"❌ Model not found at {model_path}")
        print(f"\nPlease run: python3 scripts/download_qwen3_onnx.py")
        sys.exit(1)

    print(f"=" * 70)
    print(f"Qwen3-Embedding-0.6B ONNX Runtime CoreML EP Test")
    print(f"=" * 70)
    print(f"\nModel: {model_path}")

    # Load tokenizer
    print(f"\n📥 Loading tokenizer...")
    tokenizer = AutoTokenizer.from_pretrained("models/qwen3-embedding-0.6b")
    print(f"✅ Tokenizer loaded: {len(tokenizer)} tokens")

    # Create session with CoreML EP
    print(f"\n📦 Creating ONNX Runtime session with CoreML EP...")
    session = create_session(model_path, use_coreml=True, verbose=True)

    # Run tests
    test_single_text_performance(session, tokenizer, num_runs=10)
    test_batch_performance(session, tokenizer, batch_sizes=[1, 2, 4, 8, 16, 32])
    test_embedding_quality(session, tokenizer)

    # Optional: Compare providers (commented out to save time)
    # compare_providers(model_path, tokenizer)

    print(f"\n{'='*70}")
    print(f"✅ ALL TESTS COMPLETE")
    print(f"{'='*70}")
    print(f"\n📝 Next Steps:")
    print(f"   1. Review performance metrics above")
    print(f"   2. Document baseline in automatosx/tmp/PYTHON-COREML-BASELINE.md")
    print(f"   3. Begin Rust implementation: cargo build --features onnx")
    print()


if __name__ == "__main__":
    main()
