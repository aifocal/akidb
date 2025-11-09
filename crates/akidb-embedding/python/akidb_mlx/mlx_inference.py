"""MLX Inference Engine - Core embedding generation using Apple MLX."""

import json
from pathlib import Path
from typing import List, Optional

import mlx.core as mx
import mlx.nn as nn
import numpy as np
from mlx_lm import load


class MLXEmbeddingModel:
    """
    MLX-powered embedding model.

    Loads SafeTensors weights and runs inference on Apple Silicon.
    """

    def __init__(self, model_path: Path):
        """
        Initialize MLX embedding model.

        Args:
            model_path: Path to the model directory (contains config.json, weights)
        """
        self.model_path = model_path

        # Load configuration
        config_path = model_path / "config.json"
        with open(config_path, "r") as f:
            self.config = json.load(f)

        # Extract key parameters from config
        self.hidden_size = self.config.get("hidden_size", 1024)
        self.num_hidden_layers = self.config.get("num_hidden_layers", 28)
        self.vocab_size = self.config.get("vocab_size", 151669)

        print(f"[MLXInference] Model config loaded:")
        print(f"  - hidden_size: {self.hidden_size}")
        print(f"  - num_layers: {self.num_hidden_layers}")
        print(f"  - vocab_size: {self.vocab_size}")

        # Load model and tokenizer using mlx-lm
        print(f"[MLXInference] Loading model with mlx-lm from {model_path}...")
        self.model, self.tokenizer = load(str(model_path))
        print(f"[MLXInference] Model loaded successfully")
        print(f"[MLXInference] Model type: {type(self.model)}")

        # Get the inner Qwen3Model for direct forward pass
        self.qwen_model = self.model.model

    def tokenize(self, texts: List[str], max_length: int = 512) -> dict:
        """
        Tokenize input texts using the real tokenizer (batch-optimized).

        Day 8: Optimized tokenization with list comprehensions and pre-allocation.

        Args:
            texts: List of input texts
            max_length: Maximum sequence length

        Returns:
            Dict with 'input_ids' and 'attention_mask'
        """
        batch_size = len(texts)

        # Tokenize all texts (mlx-lm tokenizers don't support true batch encoding)
        # But we optimize with list comprehensions and vectorized padding
        all_token_ids = [self.tokenizer.encode(text) for text in texts]

        # Pre-allocate arrays for better performance
        all_input_ids = []
        all_attention_masks = []

        # Pad token ID
        pad_token_id = self.tokenizer.eos_token_id

        # Process each tokenized sequence (optimized)
        for token_ids in all_token_ids:
            seq_len = len(token_ids)

            # Truncate if needed
            if seq_len > max_length:
                token_ids = token_ids[:max_length]
                attention_mask = [1] * max_length
            else:
                # Pad if needed (use list extension for efficiency)
                padding_length = max_length - seq_len
                if padding_length > 0:
                    token_ids = token_ids + [pad_token_id] * padding_length
                    attention_mask = [1] * seq_len + [0] * padding_length
                else:
                    attention_mask = [1] * max_length

            all_input_ids.append(token_ids)
            all_attention_masks.append(attention_mask)

        # Convert to MLX arrays (batch operation)
        input_ids = mx.array(all_input_ids, dtype=mx.int32)
        attention_mask = mx.array(all_attention_masks, dtype=mx.int32)

        print(f"[MLXInference] Tokenized {len(texts)} texts (batch-optimized)")
        print(f"[MLXInference] Token IDs shape: {input_ids.shape}")

        return {
            "input_ids": input_ids,
            "attention_mask": attention_mask,
        }

    def forward(self, input_ids: mx.array, attention_mask: mx.array) -> mx.array:
        """
        Forward pass through the real MLX model.

        Args:
            input_ids: Token IDs [batch_size, seq_len]
            attention_mask: Attention mask [batch_size, seq_len]

        Returns:
            Hidden states [batch_size, seq_len, hidden_size]
        """
        batch_size, seq_len = input_ids.shape

        # Run forward pass through the actual Qwen3 model
        # The model expects input_ids and returns hidden states
        hidden_states = self.qwen_model(input_ids)

        print(f"[MLXInference] Forward pass (real model): {hidden_states.shape}")

        return hidden_states

    def embed(
        self,
        texts: List[str],
        pooling: str = "mean",
        normalize: bool = True,
    ) -> np.ndarray:
        """
        Generate embeddings for input texts.

        Args:
            texts: List of input texts
            pooling: Pooling strategy ("mean" or "cls")
            normalize: Whether to L2 normalize embeddings

        Returns:
            Embeddings as numpy array [batch_size, hidden_size]
        """
        print(f"[MLXInference] Generating embeddings for {len(texts)} texts...")
        print(f"[MLXInference] Pooling: {pooling}, Normalize: {normalize}")

        # Tokenize
        inputs = self.tokenize(texts)
        input_ids = inputs["input_ids"]
        attention_mask = inputs["attention_mask"]

        # Forward pass
        hidden_states = self.forward(input_ids, attention_mask)

        # Pooling
        if pooling == "mean":
            embeddings = self._mean_pooling(hidden_states, attention_mask)
        elif pooling == "cls":
            embeddings = self._cls_pooling(hidden_states)
        else:
            raise ValueError(f"Unknown pooling strategy: {pooling}")

        # L2 normalization
        if normalize:
            embeddings = self._l2_normalize(embeddings)

        # Convert to numpy
        embeddings_np = np.array(embeddings)

        print(f"[MLXInference] Generated embeddings: {embeddings_np.shape}")

        return embeddings_np

    def _mean_pooling(self, hidden_states: mx.array, attention_mask: mx.array) -> mx.array:
        """
        Mean pooling over sequence dimension.

        Args:
            hidden_states: [batch_size, seq_len, hidden_size]
            attention_mask: [batch_size, seq_len]

        Returns:
            Pooled embeddings [batch_size, hidden_size]
        """
        # Expand attention mask to match hidden_states shape
        attention_mask_expanded = mx.expand_dims(attention_mask, axis=-1).astype(hidden_states.dtype)

        # Mask and sum
        sum_embeddings = mx.sum(hidden_states * attention_mask_expanded, axis=1)
        sum_mask = mx.sum(attention_mask_expanded, axis=1)
        sum_mask = mx.maximum(sum_mask, mx.array(1e-9))  # Avoid division by zero

        # Mean
        mean_embeddings = sum_embeddings / sum_mask

        print(f"[MLXInference] Mean pooling: {hidden_states.shape} -> {mean_embeddings.shape}")

        return mean_embeddings

    def _cls_pooling(self, hidden_states: mx.array) -> mx.array:
        """
        CLS pooling: use first token embedding.

        Args:
            hidden_states: [batch_size, seq_len, hidden_size]

        Returns:
            CLS embeddings [batch_size, hidden_size]
        """
        cls_embeddings = hidden_states[:, 0, :]

        print(f"[MLXInference] CLS pooling: {hidden_states.shape} -> {cls_embeddings.shape}")

        return cls_embeddings

    def _l2_normalize(self, embeddings: mx.array) -> mx.array:
        """
        L2 normalize embeddings.

        Args:
            embeddings: [batch_size, hidden_size]

        Returns:
            Normalized embeddings [batch_size, hidden_size]
        """
        # Compute L2 norm
        norms = mx.sqrt(mx.sum(embeddings ** 2, axis=-1, keepdims=True))
        norms = mx.maximum(norms, mx.array(1e-12))  # Avoid division by zero

        # Normalize
        normalized = embeddings / norms

        print(f"[MLXInference] L2 normalized embeddings")

        return normalized


def test_mlx_inference():
    """Test MLX inference with cached model."""
    import os
    from pathlib import Path

    # Get model path
    cache_dir = Path.home() / ".cache" / "akidb" / "models"
    model_path = cache_dir / "qwen3-0.6b-4bit"

    if not model_path.exists():
        print(f"Model not found at {model_path}")
        print("Run tests first to download the model")
        return

    # Load model
    print("Loading model...")
    model = MLXEmbeddingModel(model_path)

    # Generate embeddings
    texts = ["Hello, world!", "This is a test."]
    embeddings = model.embed(texts, pooling="mean", normalize=True)

    print(f"\nFinal embeddings shape: {embeddings.shape}")
    print(f"First embedding (first 10 dims): {embeddings[0][:10]}")
    print(f"Embedding L2 norm: {np.linalg.norm(embeddings[0]):.6f} (should be ~1.0)")


if __name__ == "__main__":
    test_mlx_inference()
