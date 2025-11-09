"""Configuration module for MLX Embedding Service."""

import os
from pathlib import Path
from typing import Dict, Optional
import yaml


class EmbeddingConfig:
    """Configuration for MLX embedding service."""

    def __init__(
        self,
        model_name: str = "qwen3-0.6b-4bit",
        pooling: str = "mean",
        normalize: bool = True,
        max_tokens: int = 512,
        auto_download: bool = True,
        batch_size: int = 32,
    ):
        """
        Initialize configuration.

        Args:
            model_name: Name of the embedding model
            pooling: Pooling strategy ("mean" or "cls")
            normalize: Whether to L2 normalize embeddings
            max_tokens: Maximum sequence length
            auto_download: Auto-download models if not cached
            batch_size: Maximum batch size for inference
        """
        self.model_name = model_name
        self.pooling = pooling
        self.normalize = normalize
        self.max_tokens = max_tokens
        self.auto_download = auto_download
        self.batch_size = batch_size

    @classmethod
    def from_yaml(cls, yaml_path: Optional[Path] = None) -> "EmbeddingConfig":
        """
        Load configuration from YAML file.

        Args:
            yaml_path: Path to YAML config file. If None, looks for:
                1. AKIDB_CONFIG env variable
                2. ./embedding_config.yaml
                3. ~/.config/akidb/embedding_config.yaml

        Returns:
            EmbeddingConfig instance with loaded settings
        """
        # Determine config file path
        if yaml_path is None:
            yaml_path = cls._find_config_file()

        if yaml_path is None:
            print("[Config] No config file found, using defaults")
            return cls()

        # Load YAML
        try:
            with open(yaml_path, "r") as f:
                config_dict = yaml.safe_load(f) or {}

            print(f"[Config] Loaded configuration from {yaml_path}")

            # Extract embedding section
            embedding_config = config_dict.get("embedding", {})

            return cls(
                model_name=embedding_config.get("model_name", "qwen3-0.6b-4bit"),
                pooling=embedding_config.get("pooling", "mean"),
                normalize=embedding_config.get("normalize", True),
                max_tokens=embedding_config.get("max_tokens", 512),
                auto_download=embedding_config.get("auto_download", True),
                batch_size=embedding_config.get("batch_size", 32),
            )

        except Exception as e:
            print(f"[Config] Warning: Failed to load config from {yaml_path}: {e}")
            print("[Config] Using default configuration")
            return cls()

    @staticmethod
    def _find_config_file() -> Optional[Path]:
        """
        Find configuration file in standard locations.

        Priority:
        1. AKIDB_CONFIG environment variable
        2. ./embedding_config.yaml (current directory)
        3. ~/.config/akidb/embedding_config.yaml (user config)

        Returns:
            Path to config file, or None if not found
        """
        # 1. Environment variable
        env_path = os.getenv("AKIDB_CONFIG")
        if env_path:
            path = Path(env_path)
            if path.exists():
                return path
            else:
                print(f"[Config] Warning: AKIDB_CONFIG points to non-existent file: {env_path}")

        # 2. Current directory
        current_dir_config = Path("embedding_config.yaml")
        if current_dir_config.exists():
            return current_dir_config

        # 3. User config directory
        user_config = Path.home() / ".config" / "akidb" / "embedding_config.yaml"
        if user_config.exists():
            return user_config

        return None

    def to_dict(self) -> Dict:
        """Convert configuration to dictionary."""
        return {
            "model_name": self.model_name,
            "pooling": self.pooling,
            "normalize": self.normalize,
            "max_tokens": self.max_tokens,
            "auto_download": self.auto_download,
            "batch_size": self.batch_size,
        }

    def __repr__(self) -> str:
        """String representation of configuration."""
        return (
            f"EmbeddingConfig(model_name='{self.model_name}', "
            f"pooling='{self.pooling}', normalize={self.normalize}, "
            f"max_tokens={self.max_tokens}, auto_download={self.auto_download}, "
            f"batch_size={self.batch_size})"
        )
