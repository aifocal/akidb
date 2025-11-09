"""AkiDB MLX Embedding Service - Python module for Apple Silicon-accelerated embeddings."""

from .embedding_service import EmbeddingService
from .mlx_inference import MLXEmbeddingModel
from .config import EmbeddingConfig
from .model_loader import (
    download_model,
    get_cache_dir,
    get_model_info,
    is_model_cached,
    list_cached_models,
    clear_cache,
    MODELS,
)

__version__ = "0.4.0"  # Updated for Day 5 (YAML config support)
__all__ = [
    "EmbeddingService",
    "MLXEmbeddingModel",
    "EmbeddingConfig",
    "download_model",
    "get_cache_dir",
    "get_model_info",
    "is_model_cached",
    "list_cached_models",
    "clear_cache",
    "MODELS",
]
