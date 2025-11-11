#!/usr/bin/env python3
"""
Export BERT model to ONNX format for use with ONNX Runtime.

This script downloads a Hugging Face transformer model and exports it to ONNX format
with optimizations for inference.

Usage:
    python scripts/export_onnx_model.py [MODEL_NAME] [OUTPUT_PATH]

Examples:
    python scripts/export_onnx_model.py
    python scripts/export_onnx_model.py sentence-transformers/all-MiniLM-L6-v2 models/minilm.onnx
"""

import argparse
import os
import sys
from pathlib import Path

def export_model(model_name: str, output_path: str):
    """Export Hugging Face model to ONNX format."""
    try:
        from transformers import AutoTokenizer, AutoModel
        import torch
    except ImportError:
        print("❌ Error: transformers and torch are required")
        print("   Install with: pip install transformers torch onnx")
        sys.exit(1)

    print(f"📥 Loading model: {model_name}")

    # Load model and tokenizer
    tokenizer = AutoTokenizer.from_pretrained(model_name)
    model = AutoModel.from_pretrained(model_name)
    model.eval()

    print(f"✅ Model loaded successfully")
    print(f"   Parameters: {sum(p.numel() for p in model.parameters()):,}")

    # Create dummy input for tracing
    dummy_text = "Hello world"
    dummy_input = tokenizer(
        dummy_text,
        return_tensors="pt",
        padding="max_length",
        max_length=512,
        truncation=True
    )

    print(f"📦 Exporting to ONNX format...")

    # Create output directory if it doesn't exist
    output_dir = Path(output_path).parent
    output_dir.mkdir(parents=True, exist_ok=True)

    # Export to ONNX with dynamic axes for batching
    torch.onnx.export(
        model,
        (
            dummy_input["input_ids"],
            dummy_input["attention_mask"],
        ),
        output_path,
        input_names=["input_ids", "attention_mask"],
        output_names=["last_hidden_state"],
        dynamic_axes={
            "input_ids": {0: "batch_size", 1: "sequence_length"},
            "attention_mask": {0: "batch_size", 1: "sequence_length"},
            "last_hidden_state": {0: "batch_size", 1: "sequence_length"},
        },
        opset_version=14,
        do_constant_folding=True,
        verbose=False,
    )

    # Get file size
    file_size = os.path.getsize(output_path) / (1024 * 1024)  # MB

    print(f"✅ Export complete!")
    print(f"   Output: {output_path}")
    print(f"   Size: {file_size:.1f} MB")

    # Verify ONNX model
    try:
        import onnx
        onnx_model = onnx.load(output_path)
        onnx.checker.check_model(onnx_model)
        print(f"✅ ONNX model validation passed")
    except ImportError:
        print(f"⚠️  onnx package not installed, skipping validation")
        print(f"   Install with: pip install onnx")
    except Exception as e:
        print(f"❌ ONNX model validation failed: {e}")
        sys.exit(1)

    # Test inference
    print(f"\n🧪 Testing ONNX inference...")
    try:
        import onnxruntime as ort

        session = ort.InferenceSession(output_path)

        # Run inference
        outputs = session.run(
            None,
            {
                "input_ids": dummy_input["input_ids"].numpy(),
                "attention_mask": dummy_input["attention_mask"].numpy(),
            },
        )

        # Check output shape
        last_hidden_state = outputs[0]
        print(f"✅ Inference test passed")
        print(f"   Input shape: {dummy_input['input_ids'].shape}")
        print(f"   Output shape: {last_hidden_state.shape}")
        print(f"   Expected: (1, 512, {model.config.hidden_size})")

    except ImportError:
        print(f"⚠️  onnxruntime not installed, skipping inference test")
        print(f"   Install with: pip install onnxruntime")
    except Exception as e:
        print(f"❌ Inference test failed: {e}")
        sys.exit(1)

    print(f"\n🎉 Model export successful!")
    print(f"\nNext steps:")
    print(f"  1. Use the model in Rust: OnnxEmbeddingProvider::new(\"{output_path}\", \"{model_name}\")")
    print(f"  2. Run tests: cargo test --features onnx -p akidb-embedding")


def main():
    parser = argparse.ArgumentParser(
        description="Export Hugging Face transformer model to ONNX format"
    )
    parser.add_argument(
        "model_name",
        nargs="?",
        default="sentence-transformers/all-MiniLM-L6-v2",
        help="Hugging Face model name (default: sentence-transformers/all-MiniLM-L6-v2)",
    )
    parser.add_argument(
        "output_path",
        nargs="?",
        default="models/minilm-l6-v2.onnx",
        help="Output ONNX file path (default: models/minilm-l6-v2.onnx)",
    )

    args = parser.parse_args()

    print("=" * 70)
    print("ONNX Model Export Tool")
    print("=" * 70)
    print()

    export_model(args.model_name, args.output_path)


if __name__ == "__main__":
    main()
