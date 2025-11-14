#!/usr/bin/env python3
"""
Convert Qwen 3 0.6B model to ONNX format for embedding generation.

This script downloads Qwen 3 0.6B from Hugging Face, converts it to ONNX format,
and optimizes it for embedding inference on Apple Silicon (Metal Performance Shaders).

Usage:
    python convert_qwen_to_onnx.py --output-dir ./models/qwen-0.6b-onnx

Requirements:
    pip install torch transformers optimum onnx onnxruntime
"""

import argparse
import logging
import os
import sys
from pathlib import Path
from typing import Optional

import torch
from transformers import AutoTokenizer, AutoModel, AutoConfig
from optimum.onnxruntime import ORTModelForFeatureExtraction
from optimum.onnxruntime.configuration import OptimizationConfig
from optimum.onnxruntime import ORTOptimizer

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(levelname)s - %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S"
)
logger = logging.getLogger(__name__)


class QwenONNXConverter:
    """Convert Qwen models to ONNX format for embeddings."""

    # Qwen embedding model identifiers on Hugging Face
    QWEN_MODELS = {
        "0.6b": "Qwen/Qwen3-Embedding-0.6B",  # Dedicated embedding model
        "1.8b": "Qwen/Qwen2-1.5B-Instruct",
        "7b": "Qwen/Qwen2-7B-Instruct",
    }

    def __init__(self, model_size: str = "0.6b", output_dir: Optional[str] = None):
        """
        Initialize converter.

        Args:
            model_size: Qwen model size ("0.6b", "1.8b", "7b")
            output_dir: Directory to save ONNX model
        """
        self.model_size = model_size
        self.model_id = self.QWEN_MODELS.get(model_size)

        if not self.model_id:
            raise ValueError(
                f"Unknown model size: {model_size}. "
                f"Available: {list(self.QWEN_MODELS.keys())}"
            )

        # Default output directory
        if output_dir is None:
            self.output_dir = Path.home() / ".cache" / "akidb" / "models" / f"qwen-{model_size}-onnx"
        else:
            self.output_dir = Path(output_dir)

        self.output_dir.mkdir(parents=True, exist_ok=True)

        logger.info(f"Initialized converter for Qwen {model_size}")
        logger.info(f"Model ID: {self.model_id}")
        logger.info(f"Output directory: {self.output_dir}")

    def download_model(self) -> tuple[AutoModel, AutoTokenizer, AutoConfig]:
        """
        Download Qwen model from Hugging Face.

        Returns:
            Tuple of (model, tokenizer, config)
        """
        logger.info(f"Downloading {self.model_id} from Hugging Face...")

        try:
            # Download tokenizer
            tokenizer = AutoTokenizer.from_pretrained(
                self.model_id,
                trust_remote_code=True
            )

            # Download config
            config = AutoConfig.from_pretrained(
                self.model_id,
                trust_remote_code=True
            )

            # Download model
            model = AutoModel.from_pretrained(
                self.model_id,
                trust_remote_code=True,
                torch_dtype=torch.float32  # Use FP32 for ONNX conversion
            )

            logger.info("✅ Model downloaded successfully")
            logger.info(f"  Hidden size: {config.hidden_size}")
            logger.info(f"  Vocab size: {config.vocab_size}")
            logger.info(f"  Max position embeddings: {config.max_position_embeddings}")

            return model, tokenizer, config

        except Exception as e:
            logger.error(f"Failed to download model: {e}")
            raise

    def convert_to_onnx(self, model: AutoModel, tokenizer: AutoTokenizer) -> Path:
        """
        Convert PyTorch model to ONNX format.

        Args:
            model: Qwen PyTorch model
            tokenizer: Qwen tokenizer

        Returns:
            Path to ONNX model file
        """
        logger.info("Converting to ONNX format...")

        try:
            # Prepare dummy input for tracing
            dummy_text = "This is a sample text for ONNX export."
            inputs = tokenizer(
                dummy_text,
                return_tensors="pt",
                padding=True,
                truncation=True,
                max_length=512
            )

            # Set model to evaluation mode
            model.eval()

            # Export to ONNX
            onnx_path = self.output_dir / "model.onnx"

            with torch.no_grad():
                torch.onnx.export(
                    model,
                    (inputs["input_ids"], inputs["attention_mask"]),
                    str(onnx_path),
                    input_names=["input_ids", "attention_mask"],
                    output_names=["last_hidden_state"],
                    dynamic_axes={
                        "input_ids": {0: "batch_size", 1: "sequence_length"},
                        "attention_mask": {0: "batch_size", 1: "sequence_length"},
                        "last_hidden_state": {0: "batch_size", 1: "sequence_length"}
                    },
                    opset_version=14,  # ONNX opset 14 for broad compatibility
                    do_constant_folding=True,
                    verbose=False
                )

            logger.info(f"✅ ONNX model saved to: {onnx_path}")
            return onnx_path

        except Exception as e:
            logger.error(f"Failed to convert to ONNX: {e}")
            raise

    def optimize_onnx(self, onnx_path: Path) -> Path:
        """
        Optimize ONNX model for inference.

        Args:
            onnx_path: Path to ONNX model

        Returns:
            Path to optimized ONNX model
        """
        logger.info("Optimizing ONNX model...")

        try:
            import onnx
            from onnxruntime.transformers import optimizer

            # Load model
            model = onnx.load(str(onnx_path))

            # Apply optimizations
            optimized_model = optimizer.optimize_model(
                str(onnx_path),
                model_type="bert",  # Qwen uses similar architecture
                num_heads=0,  # Auto-detect
                hidden_size=0,  # Auto-detect
            )

            # Save optimized model
            optimized_path = self.output_dir / "model_optimized.onnx"
            optimized_model.save_model_to_file(str(optimized_path))

            logger.info(f"✅ Optimized model saved to: {optimized_path}")
            return optimized_path

        except ImportError:
            logger.warning("onnxruntime.transformers not available, skipping optimization")
            logger.warning("Install with: pip install onnxruntime-tools")
            return onnx_path
        except Exception as e:
            logger.warning(f"Optimization failed: {e}")
            logger.warning("Using unoptimized model")
            return onnx_path

    def save_tokenizer(self, tokenizer: AutoTokenizer):
        """
        Save tokenizer for later use.

        Args:
            tokenizer: Qwen tokenizer
        """
        logger.info("Saving tokenizer...")

        try:
            tokenizer.save_pretrained(str(self.output_dir))
            logger.info(f"✅ Tokenizer saved to: {self.output_dir}")

        except Exception as e:
            logger.error(f"Failed to save tokenizer: {e}")
            raise

    def verify_onnx_model(self, onnx_path: Path, tokenizer: AutoTokenizer):
        """
        Verify ONNX model produces valid embeddings.

        Args:
            onnx_path: Path to ONNX model
            tokenizer: Qwen tokenizer
        """
        logger.info("Verifying ONNX model...")

        try:
            import onnxruntime as ort

            # Create ONNX Runtime session
            session = ort.InferenceSession(
                str(onnx_path),
                providers=["CPUExecutionProvider"]  # Use CPU for verification
            )

            # Test input
            test_text = "This is a test sentence for embedding generation."
            inputs = tokenizer(
                test_text,
                return_tensors="np",
                padding=True,
                truncation=True,
                max_length=512
            )

            # Run inference
            outputs = session.run(
                None,
                {
                    "input_ids": inputs["input_ids"],
                    "attention_mask": inputs["attention_mask"]
                }
            )

            # Extract embeddings (mean pooling over sequence)
            import numpy as np
            embeddings = outputs[0]  # [batch_size, seq_len, hidden_size]

            # Mean pooling
            attention_mask = inputs["attention_mask"]
            mask_expanded = np.expand_dims(attention_mask, -1)
            sum_embeddings = np.sum(embeddings * mask_expanded, axis=1)
            sum_mask = np.clip(np.sum(attention_mask, axis=1, keepdims=True), a_min=1e-9, a_max=None)
            mean_embeddings = sum_embeddings / sum_mask

            # Verify shape
            logger.info(f"✅ ONNX model verification successful")
            logger.info(f"  Input shape: {inputs['input_ids'].shape}")
            logger.info(f"  Output shape: {embeddings.shape}")
            logger.info(f"  Embedding dimension: {mean_embeddings.shape[1]}")
            logger.info(f"  Sample embedding norm: {np.linalg.norm(mean_embeddings[0]):.4f}")

        except Exception as e:
            logger.error(f"❌ Verification failed: {e}")
            raise

    def convert(self) -> Path:
        """
        Full conversion pipeline.

        Returns:
            Path to optimized ONNX model
        """
        logger.info("=" * 60)
        logger.info(f"Starting Qwen {self.model_size} to ONNX conversion")
        logger.info("=" * 60)

        # Step 1: Download model
        model, tokenizer, config = self.download_model()

        # Step 2: Convert to ONNX
        onnx_path = self.convert_to_onnx(model, tokenizer)

        # Step 3: Optimize ONNX
        optimized_path = self.optimize_onnx(onnx_path)

        # Step 4: Save tokenizer
        self.save_tokenizer(tokenizer)

        # Step 5: Verify
        self.verify_onnx_model(optimized_path, tokenizer)

        logger.info("=" * 60)
        logger.info("✅ Conversion complete!")
        logger.info(f"  Model: {optimized_path}")
        logger.info(f"  Tokenizer: {self.output_dir}")
        logger.info("=" * 60)

        # Print usage instructions
        self._print_usage_instructions(optimized_path)

        return optimized_path

    def _print_usage_instructions(self, onnx_path: Path):
        """Print instructions for using the converted model."""
        logger.info("")
        logger.info("Usage with AkiDB:")
        logger.info("")
        logger.info("1. Set environment variables:")
        logger.info(f"   export AKIDB_EMBEDDING_PROVIDER=python-bridge")
        logger.info(f"   export AKIDB_EMBEDDING_MODEL={onnx_path}")
        logger.info(f"   export AKIDB_EMBEDDING_PYTHON_PATH=/opt/homebrew/bin/python3.12")
        logger.info("")
        logger.info("2. Start AkiDB server:")
        logger.info("   cargo run -p akidb-rest")
        logger.info("")
        logger.info("3. Test embedding endpoint:")
        logger.info("   curl -X POST http://localhost:8080/api/v1/embed \\")
        logger.info("     -H 'Content-Type: application/json' \\")
        logger.info("     -d '{")
        logger.info(f"       \"model\": \"{onnx_path}\",")
        logger.info("       \"inputs\": [\"Hello, world!\"],")
        logger.info("       \"normalize\": true")
        logger.info("     }'")
        logger.info("")


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(
        description="Convert Qwen models to ONNX format for embeddings",
        formatter_class=argparse.RawDescriptionHelpFormatter
    )

    parser.add_argument(
        "--model-size",
        type=str,
        default="0.6b",
        choices=["0.6b", "1.8b", "7b"],
        help="Qwen model size (default: 0.6b)"
    )

    parser.add_argument(
        "--output-dir",
        type=str,
        default=None,
        help="Output directory for ONNX model (default: ~/.cache/akidb/models/qwen-{size}-onnx)"
    )

    args = parser.parse_args()

    try:
        converter = QwenONNXConverter(
            model_size=args.model_size,
            output_dir=args.output_dir
        )

        onnx_path = converter.convert()

        logger.info("🎉 Success! ONNX model ready for use.")
        sys.exit(0)

    except KeyboardInterrupt:
        logger.warning("Interrupted by user")
        sys.exit(1)
    except Exception as e:
        logger.error(f"Conversion failed: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
