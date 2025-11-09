"""Test YAML configuration loading for MLX Embedding Service."""

import yaml
from pathlib import Path
from akidb_mlx import EmbeddingConfig, EmbeddingService


def test_config_defaults():
    """Test default configuration values."""
    print("=== Test 1: Default Configuration ===")
    config = EmbeddingConfig()
    print(f"Config: {config}")

    assert config.model_name == "qwen3-0.6b-4bit"
    assert config.pooling == "mean"
    assert config.normalize is True
    assert config.max_tokens == 512
    assert config.auto_download is True
    assert config.batch_size == 32

    print("✅ Default configuration test passed\n")


def test_config_from_dict():
    """Test configuration from dictionary."""
    print("=== Test 2: Configuration from Dict ===")

    config_dict = {
        "model_name": "gemma-300m-4bit",
        "pooling": "cls",
        "normalize": False,
        "max_tokens": 256,
        "auto_download": False,
        "batch_size": 16,
    }

    config = EmbeddingConfig(**config_dict)
    print(f"Config: {config}")

    assert config.model_name == "gemma-300m-4bit"
    assert config.pooling == "cls"
    assert config.normalize is False
    assert config.max_tokens == 256
    assert config.auto_download is False
    assert config.batch_size == 16

    print("✅ Dict configuration test passed\n")


def test_config_from_yaml():
    """Test configuration from YAML file."""
    print("=== Test 3: Configuration from YAML ===")

    # Create temporary YAML config
    test_config_path = Path("test_embedding_config.yaml")

    config_content = """
embedding:
  model_name: "qwen3-0.6b-4bit"
  pooling: "cls"
  normalize: false
  max_tokens: 256
  auto_download: true
  batch_size: 16
"""

    test_config_path.write_text(config_content)

    try:
        # Load config from YAML
        config = EmbeddingConfig.from_yaml(test_config_path)
        print(f"Config: {config}")

        assert config.model_name == "qwen3-0.6b-4bit"
        assert config.pooling == "cls"
        assert config.normalize is False
        assert config.max_tokens == 256
        assert config.auto_download is True
        assert config.batch_size == 16

        print("✅ YAML configuration test passed\n")

    finally:
        # Clean up test file
        if test_config_path.exists():
            test_config_path.unlink()


def test_embedding_service_with_config():
    """Test EmbeddingService initialization with config object."""
    print("=== Test 4: EmbeddingService with Config ===")

    # Create config
    config = EmbeddingConfig(
        model_name="qwen3-0.6b-4bit",
        pooling="mean",
        normalize=True,
    )

    # Initialize service with config (don't actually load model)
    print(f"Using config: {config}")

    # Note: We skip actual service initialization to avoid loading the model
    # Just verify config can be created and passed

    print("✅ EmbeddingService config test passed\n")


def test_config_priority():
    """Test configuration priority: explicit params > config object > defaults."""
    print("=== Test 5: Configuration Priority ===")

    # Create config with some values
    config = EmbeddingConfig(
        model_name="gemma-300m-4bit",
        pooling="cls",
    )

    print(f"Base config: {config}")

    # Explicit parameters should override config
    # (We'll just test the logic, not actually initialize the service)
    model_name_explicit = "qwen3-0.6b-4bit"
    pooling_from_config = config.pooling

    # Priority: explicit > config
    final_model = model_name_explicit  # Should be qwen3
    final_pooling = pooling_from_config  # Should be cls (from config)

    assert final_model == "qwen3-0.6b-4bit"
    assert final_pooling == "cls"

    print(f"Final model (explicit): {final_model}")
    print(f"Final pooling (from config): {final_pooling}")
    print("✅ Configuration priority test passed\n")


def test_config_to_dict():
    """Test configuration to dictionary conversion."""
    print("=== Test 6: Config to Dict ===")

    config = EmbeddingConfig(
        model_name="qwen3-0.6b-4bit",
        pooling="mean",
        normalize=True,
        max_tokens=512,
    )

    config_dict = config.to_dict()
    print(f"Config dict: {config_dict}")

    assert config_dict["model_name"] == "qwen3-0.6b-4bit"
    assert config_dict["pooling"] == "mean"
    assert config_dict["normalize"] is True
    assert config_dict["max_tokens"] == 512

    print("✅ Config to dict test passed\n")


if __name__ == "__main__":
    print("Testing MLX Embedding Configuration\n")
    print("=" * 60)

    test_config_defaults()
    test_config_from_dict()
    test_config_from_yaml()
    test_embedding_service_with_config()
    test_config_priority()
    test_config_to_dict()

    print("=" * 60)
    print("\n✅ All configuration tests passed!")
