"""Tests for model_loader.py - HuggingFace Hub integration."""

import os
import shutil
import tempfile
from pathlib import Path

import pytest

from akidb_mlx import (
    MODELS,
    clear_cache,
    download_model,
    get_cache_dir,
    get_model_info,
    is_model_cached,
    list_cached_models,
)


@pytest.fixture
def temp_cache_dir():
    """Create a temporary cache directory for testing."""
    temp_dir = tempfile.mkdtemp(prefix="akidb_test_")
    old_env = os.environ.get("AKIDB_CACHE_DIR")

    # Override cache dir for tests
    os.environ["AKIDB_CACHE_DIR"] = temp_dir

    yield Path(temp_dir)

    # Cleanup
    if old_env:
        os.environ["AKIDB_CACHE_DIR"] = old_env
    else:
        os.environ.pop("AKIDB_CACHE_DIR", None)

    if Path(temp_dir).exists():
        shutil.rmtree(temp_dir)


def test_model_registry():
    """Test that model registry is correctly defined."""
    assert "qwen3-0.6b-4bit" in MODELS
    assert "gemma-300m-4bit" in MODELS

    qwen_info = MODELS["qwen3-0.6b-4bit"]
    assert qwen_info["dimension"] == 512
    assert qwen_info["repo_id"] == "mlx-community/Qwen3-Embedding-0.6B-4bit-DWQ"

    gemma_info = MODELS["gemma-300m-4bit"]
    assert gemma_info["dimension"] == 768
    assert gemma_info["repo_id"] == "mlx-community/embeddinggemma-300m-4bit"


def test_get_model_info():
    """Test get_model_info function."""
    info = get_model_info("qwen3-0.6b-4bit")
    assert info["dimension"] == 512
    assert info["max_tokens"] == 512
    assert "repo_id" in info

    # Test invalid model
    with pytest.raises(ValueError, match="not found"):
        get_model_info("invalid-model")


def test_get_cache_dir_default():
    """Test default cache directory."""
    # Save and clear env var
    old_env = os.environ.pop("AKIDB_CACHE_DIR", None)

    cache_dir = get_cache_dir()
    assert cache_dir.exists()
    assert ".cache" in str(cache_dir)
    assert "akidb" in str(cache_dir)

    # Restore env var
    if old_env:
        os.environ["AKIDB_CACHE_DIR"] = old_env


def test_get_cache_dir_custom(temp_cache_dir):
    """Test custom cache directory via env var."""
    cache_dir = get_cache_dir()
    assert str(temp_cache_dir) in str(cache_dir)
    assert cache_dir.exists()


def test_is_model_cached_false(temp_cache_dir):
    """Test is_model_cached returns False for non-cached model."""
    assert not is_model_cached("qwen3-0.6b-4bit")
    assert not is_model_cached("gemma-300m-4bit")


def test_is_model_cached_true(temp_cache_dir):
    """Test is_model_cached returns True when model exists."""
    # Create fake model directory with required files
    model_path = temp_cache_dir / "models" / "qwen3-0.6b-4bit"
    model_path.mkdir(parents=True)

    # Create required files
    (model_path / "config.json").write_text('{"hidden_size": 512}')
    (model_path / "model.safetensors").write_text("fake weights")

    assert is_model_cached("qwen3-0.6b-4bit")


def test_list_cached_models_empty(temp_cache_dir):
    """Test list_cached_models with empty cache."""
    cached = list_cached_models()
    assert cached == []


def test_list_cached_models_with_models(temp_cache_dir):
    """Test list_cached_models with cached models."""
    # Create fake models
    for model_name in ["qwen3-0.6b-4bit", "gemma-300m-4bit"]:
        model_path = temp_cache_dir / "models" / model_name
        model_path.mkdir(parents=True)
        (model_path / "config.json").write_text("{}")
        (model_path / "model.safetensors").write_text("fake")

    cached = list_cached_models()
    assert set(cached) == {"qwen3-0.6b-4bit", "gemma-300m-4bit"}


def test_clear_cache_specific_model(temp_cache_dir):
    """Test clearing cache for a specific model."""
    # Create fake models
    qwen_path = temp_cache_dir / "models" / "qwen3-0.6b-4bit"
    gemma_path = temp_cache_dir / "models" / "gemma-300m-4bit"

    for model_path in [qwen_path, gemma_path]:
        model_path.mkdir(parents=True)
        (model_path / "config.json").write_text("{}")
        (model_path / "model.safetensors").write_text("fake")

    # Clear only qwen
    clear_cache("qwen3-0.6b-4bit")

    assert not qwen_path.exists()
    assert gemma_path.exists()


def test_clear_cache_all(temp_cache_dir):
    """Test clearing all cache."""
    # Create fake models
    for model_name in ["qwen3-0.6b-4bit", "gemma-300m-4bit"]:
        model_path = temp_cache_dir / "models" / model_name
        model_path.mkdir(parents=True)
        (model_path / "config.json").write_text("{}")

    clear_cache()  # Clear all

    cached = list_cached_models()
    assert cached == []


@pytest.mark.skipif(
    not os.environ.get("AKIDB_TEST_DOWNLOAD"),
    reason="Model download test disabled (set AKIDB_TEST_DOWNLOAD=1 to enable)",
)
def test_download_model_qwen3(temp_cache_dir):
    """
    Test actual model download from HuggingFace (SLOW - disabled by default).

    To run: AKIDB_TEST_DOWNLOAD=1 pytest test_model_loader.py::test_download_model_qwen3
    """
    model_path = download_model("qwen3-0.6b-4bit")

    assert model_path.exists()
    assert (model_path / "config.json").exists()
    assert is_model_cached("qwen3-0.6b-4bit")

    # Check metadata was saved
    assert (model_path / "akidb_metadata.json").exists()


@pytest.mark.skipif(
    not os.environ.get("AKIDB_TEST_DOWNLOAD"),
    reason="Model download test disabled (set AKIDB_TEST_DOWNLOAD=1 to enable)",
)
def test_download_model_gemma(temp_cache_dir):
    """
    Test actual Gemma model download (SLOW - disabled by default).

    To run: AKIDB_TEST_DOWNLOAD=1 pytest test_model_loader.py::test_download_model_gemma
    """
    model_path = download_model("gemma-300m-4bit")

    assert model_path.exists()
    assert (model_path / "config.json").exists()
    assert is_model_cached("gemma-300m-4bit")


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
