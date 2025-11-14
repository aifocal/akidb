#!/usr/bin/env python3
"""
Download Qwen3-Embedding-0.6B ONNX model from Hugging Face.

This script downloads the ONNX version of Qwen3-Embedding-0.6B model
for use with ONNX Runtime + CoreML Execution Provider on Mac ARM.
"""

import argparse
import os
import sys
from pathlib import Path

def download_qwen3_onnx(output_dir="models/qwen3-embedding-0.6b"):
    """Download Qwen3-Embedding-0.6B ONNX model from HuggingFace."""
    try:
        from huggingface_hub import snapshot_download
    except ImportError:
        print("❌ Error: huggingface-hub not installed")
        print("   Install with: pip install huggingface-hub")
        sys.exit(1)

    output_path = Path(output_dir)
    print(f"📥 Downloading Qwen3-Embedding-0.6B ONNX to {output_path}...")
    print(f"   Repository: onnx-community/Qwen3-Embedding-0.6B-ONNX")

    try:
        snapshot_download(
            repo_id="onnx-community/Qwen3-Embedding-0.6B-ONNX",
            local_dir=str(output_path),
            repo_type="model"
        )
    except Exception as e:
        print(f"❌ Download failed: {e}")
        print(f"\nTroubleshooting:")
        print(f"  1. Check internet connection")
        print(f"  2. Verify repository exists on HuggingFace")
        print(f"  3. Try: huggingface-cli login (if repo is private)")
        sys.exit(1)

    print(f"✅ Download complete: {output_path}")

    # List downloaded files
    print(f"\n📦 Downloaded files:")
    total_size = 0
    for file_path in sorted(output_path.rglob("*")):
        if file_path.is_file():
            size_mb = file_path.stat().st_size / 1024 / 1024
            total_size += size_mb
            print(f"   - {file_path.name} ({size_mb:.1f} MB)")

    print(f"\n📊 Total size: {total_size:.1f} MB")

    # Check for critical files
    critical_files = ["model.onnx", "tokenizer.json"]
    missing_files = []
    for filename in critical_files:
        file_path = output_path / filename
        if not file_path.exists():
            # Try common variations
            variations = [
                filename,
                filename.replace(".onnx", "_quantized.onnx"),
                filename.replace("model", "qwen3-embedding"),
            ]
            found = False
            for variant in variations:
                if (output_path / variant).exists():
                    found = True
                    print(f"✅ Found {variant} (variation of {filename})")
                    break
            if not found:
                missing_files.append(filename)

    if missing_files:
        print(f"\n⚠️  Warning: Expected files not found: {', '.join(missing_files)}")
        print(f"   Check repository structure manually")
    else:
        print(f"\n✅ All critical files present")

    print(f"\n📝 Next steps:")
    print(f"  1. Validate ONNX model: python scripts/validate_qwen3_onnx.py")
    print(f"  2. Test with onnxruntime: python scripts/test_qwen3_coreml.py")
    print(f"  3. Implement Rust provider: cargo build --features onnx")

    return str(output_path)


def main():
    parser = argparse.ArgumentParser(
        description="Download Qwen3-Embedding-0.6B ONNX model from HuggingFace"
    )
    parser.add_argument(
        "--output",
        default="models/qwen3-embedding-0.6b",
        help="Output directory (default: models/qwen3-embedding-0.6b)",
    )
    args = parser.parse_args()

    print("=" * 70)
    print("Qwen3-Embedding-0.6B ONNX Download Tool")
    print("=" * 70)
    print()

    download_qwen3_onnx(args.output)


if __name__ == "__main__":
    main()
