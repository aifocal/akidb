"""MLX Embedding Service - Main service class for embedding generation."""

from pathlib import Path
from typing import List, Optional
import numpy as np

from .model_loader import download_model, get_model_info, is_model_cached
from .mlx_inference import MLXEmbeddingModel
from .config import EmbeddingConfig


class EmbeddingService:
    """
    MLX-powered embedding service for AkiDB.

    Day 3: MLX inference integrated with mean pooling and L2 normalization.
    """

    def __init__(
        self,
        model_name: Optional[str] = None,
        auto_download: Optional[bool] = None,
        pooling: Optional[str] = None,
        normalize: Optional[bool] = None,
        config: Optional[EmbeddingConfig] = None,
        config_path: Optional[Path] = None,
    ):
        """
        Initialize the embedding service.

        Args:
            model_name: Name of the embedding model to use.
            auto_download: If True, automatically download model if not cached.
            pooling: Pooling strategy ("mean" or "cls"). Default: "mean"
            normalize: Whether to L2 normalize embeddings. Default: True
            config: EmbeddingConfig object (overrides other args)
            config_path: Path to YAML config file (loads config from file)

        Priority (highest to lowest):
        1. Explicit parameters (model_name, pooling, etc.)
        2. config object
        3. config_path (YAML file)
        4. Default values
        """
        # Load config from file if provided
        if config_path is not None:
            config = EmbeddingConfig.from_yaml(config_path)
        elif config is None:
            # Try to find and load config automatically
            config = EmbeddingConfig.from_yaml()

        # Override with explicit parameters
        self.model_name = model_name if model_name is not None else config.model_name
        self.pooling = pooling if pooling is not None else config.pooling
        self.normalize = normalize if normalize is not None else config.normalize
        auto_download = auto_download if auto_download is not None else config.auto_download

        # Get model metadata
        model_info = get_model_info(self.model_name)
        self.dimension = model_info["dimension"]
        self.max_tokens = model_info["max_tokens"]

        # Check if model is cached
        if is_model_cached(model_name):
            self.model_path = Path(download_model(model_name, force_redownload=False))
            print(f"[EmbeddingService] Using cached model: {model_name}")
        elif auto_download:
            print(f"[EmbeddingService] Model not cached, downloading: {model_name}")
            self.model_path = download_model(model_name)
            print(f"[EmbeddingService] Download complete!")
        else:
            raise ValueError(
                f"Model '{model_name}' not cached and auto_download=False"
            )

        print(f"[EmbeddingService] Initialized with model: {model_name}")
        print(f"[EmbeddingService] Model path: {self.model_path}")
        print(f"[EmbeddingService] Output dimension: {self.dimension}")
        print(f"[EmbeddingService] Pooling: {pooling}, Normalize: {normalize}")

        # Load MLX model
        print(f"[EmbeddingService] Loading MLX inference engine...")
        self.mlx_model = MLXEmbeddingModel(self.model_path)
        print(f"[EmbeddingService] MLX engine ready!")

    def embed(self, texts: List[str]) -> List[List[float]]:
        """
        Generate embeddings for a list of texts using MLX.

        Args:
            texts: List of input texts to embed.

        Returns:
            List of embedding vectors (each is a list of floats).
        """
        print(f"[EmbeddingService] Embedding {len(texts)} texts with MLX...")

        # Use MLX inference engine
        embeddings_np = self.mlx_model.embed(
            texts,
            pooling=self.pooling,
            normalize=self.normalize,
        )

        # Convert to list of lists for JSON serialization
        embeddings = embeddings_np.tolist()

        print(f"[EmbeddingService] Generated {len(embeddings)} embeddings")
        return embeddings

    def get_model_info(self) -> dict:
        """Get information about the loaded model."""
        return {
            "model_name": self.model_name,
            "dimension": self.dimension,
            "status": "initialized",
            "backend": "mlx (placeholder)"
        }
