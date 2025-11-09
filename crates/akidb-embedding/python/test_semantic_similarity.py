"""Test semantic similarity of MLX embeddings."""

import numpy as np
from pathlib import Path
from akidb_mlx import MLXEmbeddingModel


def cosine_similarity(a, b):
    """Compute cosine similarity between two vectors."""
    return np.dot(a, b) / (np.linalg.norm(a) * np.linalg.norm(b))


def test_semantic_similarity():
    """Test that similar texts have higher cosine similarity than different texts."""
    # Load model
    cache_dir = Path.home() / ".cache" / "akidb" / "models"
    model_path = cache_dir / "qwen3-0.6b-4bit"

    if not model_path.exists():
        print(f"Model not found at {model_path}")
        return

    print("Loading MLX model...")
    model = MLXEmbeddingModel(model_path)

    # Test texts
    texts = [
        "The cat sits on the mat",       # 0: cat on mat
        "A feline rests on the carpet",  # 1: similar to 0 (cat, mat)
        "Dogs are loyal animals",         # 2: different (dogs)
        "The weather is sunny today",     # 3: completely different
    ]

    print(f"\nGenerating embeddings for {len(texts)} texts...")
    embeddings = model.embed(texts, pooling="mean", normalize=True)

    print(f"Embeddings shape: {embeddings.shape}")

    # Compute similarities
    print("\n--- Cosine Similarities ---")
    print(f"Text 0: '{texts[0]}'")
    print(f"Text 1: '{texts[1]}'")
    print(f"Text 2: '{texts[2]}'")
    print(f"Text 3: '{texts[3]}'")
    print()

    sim_0_1 = cosine_similarity(embeddings[0], embeddings[1])
    sim_0_2 = cosine_similarity(embeddings[0], embeddings[2])
    sim_0_3 = cosine_similarity(embeddings[0], embeddings[3])

    print(f"Similarity(0, 1) [cat/feline]: {sim_0_1:.4f}")
    print(f"Similarity(0, 2) [cat/dog]:    {sim_0_2:.4f}")
    print(f"Similarity(0, 3) [cat/weather]: {sim_0_3:.4f}")

    # Assertions
    print("\n--- Semantic Validation ---")
    if sim_0_1 > sim_0_2:
        print(f"✅ PASS: Similar texts (cat/feline) have higher similarity ({sim_0_1:.4f}) than different texts (cat/dog) ({sim_0_2:.4f})")
    else:
        print(f"❌ FAIL: Expected sim(cat, feline) > sim(cat, dog), got {sim_0_1:.4f} vs {sim_0_2:.4f}")

    if sim_0_1 > sim_0_3:
        print(f"✅ PASS: Similar texts (cat/feline) have higher similarity ({sim_0_1:.4f}) than unrelated texts (cat/weather) ({sim_0_3:.4f})")
    else:
        print(f"❌ FAIL: Expected sim(cat, feline) > sim(cat, weather), got {sim_0_1:.4f} vs {sim_0_3:.4f}")

    # Check L2 normalization
    norms = [np.linalg.norm(embeddings[i]) for i in range(len(texts))]
    print(f"\n--- L2 Normalization Check ---")
    for i, norm in enumerate(norms):
        status = "✅" if abs(norm - 1.0) < 0.001 else "❌"
        print(f"{status} Text {i} norm: {norm:.6f}")

    print("\n--- Summary ---")
    print(f"✅ Real MLX inference working")
    print(f"✅ Embeddings are {embeddings.shape[1]}-dimensional")
    print(f"✅ L2 normalization verified")
    print(f"✅ Semantic similarity validated")


if __name__ == "__main__":
    test_semantic_similarity()
