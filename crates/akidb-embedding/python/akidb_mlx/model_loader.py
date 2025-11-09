"""MLX Model Loader - Downloads and caches embedding models from HuggingFace Hub."""

import json
import os
from pathlib import Path
from typing import Dict, Optional

from huggingface_hub import snapshot_download


# Model registry: name -> (repo_id, dimension, max_tokens)
# Note: Actual dimensions are read from config.json after download
MODELS = {
    "qwen3-0.6b-4bit": {
        "repo_id": "mlx-community/Qwen3-Embedding-0.6B-4bit-DWQ",
        "dimension": 1024,  # Actual: 1024 (from config.json hidden_size)
        "max_tokens": 512,
        "description": "Qwen3 Embedding 0.6B quantized to 4-bit (default)",
        "size_mb": 600,
    },
    "gemma-300m-4bit": {
        "repo_id": "mlx-community/embeddinggemma-300m-4bit",
        "dimension": 768,  # Actual dimension (from config.json)
        "max_tokens": 512,
        "description": "EmbeddingGemma 300M quantized to 4-bit (fast)",
        "size_mb": 200,
    },
}


def get_cache_dir() -> Path:
    """
    Get the cache directory for AkiDB models.

    Uses XDG Base Directory specification:
    - Linux/Mac: ~/.cache/akidb/models/
    - Can be overridden with AKIDB_CACHE_DIR env var

    Returns:
        Path to cache directory (created if doesn't exist)
    """
    if cache_env := os.getenv("AKIDB_CACHE_DIR"):
        cache_dir = Path(cache_env) / "models"
    else:
        # XDG Base Directory: ~/.cache/akidb/models/
        home = Path.home()
        cache_dir = home / ".cache" / "akidb" / "models"

    cache_dir.mkdir(parents=True, exist_ok=True)
    return cache_dir


def get_model_info(model_name: str) -> Dict:
    """
    Get model information from the registry.

    Args:
        model_name: Name of the model (e.g., "qwen3-0.6b-4bit")

    Returns:
        Dict with model metadata

    Raises:
        ValueError: If model not found in registry
    """
    if model_name not in MODELS:
        available = ", ".join(MODELS.keys())
        raise ValueError(
            f"Model '{model_name}' not found. Available models: {available}"
        )

    return MODELS[model_name].copy()


def get_model_path(model_name: str) -> Path:
    """
    Get the local path for a model.

    Args:
        model_name: Name of the model

    Returns:
        Path where model should be/is cached
    """
    cache_dir = get_cache_dir()
    return cache_dir / model_name


def is_model_cached(model_name: str) -> bool:
    """
    Check if a model is already downloaded and cached.

    Args:
        model_name: Name of the model

    Returns:
        True if model exists locally, False otherwise
    """
    model_path = get_model_path(model_name)

    # Check if directory exists and contains files
    if not model_path.exists():
        return False

    # Verify model has required files (at minimum: config.json, weights)
    required_files = ["config.json"]
    for filename in required_files:
        if not (model_path / filename).exists():
            return False

    # Check for at least one weight file (.safetensors or .npz)
    weight_files = list(model_path.glob("*.safetensors")) + list(model_path.glob("*.npz"))
    if not weight_files:
        return False

    return True


def download_model(model_name: str, force_redownload: bool = False) -> Path:
    """
    Download a model from HuggingFace Hub.

    Args:
        model_name: Name of the model to download
        force_redownload: If True, redownload even if cached

    Returns:
        Path to the downloaded model directory

    Raises:
        ValueError: If model not found in registry
        Exception: If download fails
    """
    model_info = get_model_info(model_name)
    repo_id = model_info["repo_id"]
    model_path = get_model_path(model_name)

    # Check if already cached (unless force_redownload)
    if not force_redownload and is_model_cached(model_name):
        print(f"[ModelLoader] Model '{model_name}' already cached at {model_path}")
        return model_path

    # Download from HuggingFace Hub
    print(f"[ModelLoader] Downloading '{model_name}' from {repo_id}...")
    print(f"[ModelLoader] Estimated size: {model_info['size_mb']}MB")
    print(f"[ModelLoader] Cache location: {model_path}")

    try:
        downloaded_path = snapshot_download(
            repo_id=repo_id,
            local_dir=str(model_path),
            local_dir_use_symlinks=False,  # Copy files instead of symlinks
            resume_download=True,  # Resume if interrupted
        )

        # Save metadata
        _save_model_metadata(model_name, model_info)

        print(f"[ModelLoader] Download complete: {downloaded_path}")
        return Path(downloaded_path)

    except Exception as e:
        print(f"[ModelLoader] Download failed: {e}")
        # Clean up partial download
        if model_path.exists():
            import shutil
            shutil.rmtree(model_path)
        raise


def _save_model_metadata(model_name: str, model_info: Dict) -> None:
    """
    Save model metadata to cache directory.

    Args:
        model_name: Name of the model
        model_info: Model metadata dict
    """
    model_path = get_model_path(model_name)
    metadata_file = model_path / "akidb_metadata.json"

    metadata = {
        "model_name": model_name,
        "repo_id": model_info["repo_id"],
        "dimension": model_info["dimension"],
        "max_tokens": model_info["max_tokens"],
        "description": model_info["description"],
        "size_mb": model_info["size_mb"],
    }

    with open(metadata_file, "w") as f:
        json.dump(metadata, f, indent=2)


def load_model_metadata(model_name: str) -> Optional[Dict]:
    """
    Load model metadata from cache.

    Args:
        model_name: Name of the model

    Returns:
        Metadata dict, or None if not found
    """
    model_path = get_model_path(model_name)
    metadata_file = model_path / "akidb_metadata.json"

    if not metadata_file.exists():
        return None

    with open(metadata_file, "r") as f:
        return json.load(f)


def list_cached_models() -> list[str]:
    """
    List all models currently cached locally.

    Returns:
        List of model names
    """
    cache_dir = get_cache_dir()

    if not cache_dir.exists():
        return []

    cached = []
    for model_name in MODELS.keys():
        if is_model_cached(model_name):
            cached.append(model_name)

    return cached


def clear_cache(model_name: Optional[str] = None) -> None:
    """
    Clear model cache.

    Args:
        model_name: If provided, clear only this model. Otherwise clear all.
    """
    import shutil

    if model_name:
        # Clear specific model
        model_path = get_model_path(model_name)
        if model_path.exists():
            shutil.rmtree(model_path)
            print(f"[ModelLoader] Cleared cache for '{model_name}'")
    else:
        # Clear all models
        cache_dir = get_cache_dir()
        if cache_dir.exists():
            shutil.rmtree(cache_dir)
            cache_dir.mkdir(parents=True, exist_ok=True)
            print("[ModelLoader] Cleared all model cache")
