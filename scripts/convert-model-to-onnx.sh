#!/bin/bash
# Convert HuggingFace model to ONNX format for inference
# Uses optimum-cli for conversion with CoreML compatibility

set -e

MODEL_NAME="${1:-sentence-transformers/all-MiniLM-L6-v2}"
CACHE_DIR="${2:-$HOME/.cache/akidb/models}"

echo "🔄 Converting model to ONNX: $MODEL_NAME"
echo "Cache directory: $CACHE_DIR"
echo

# Check if optimum-cli is installed
if ! command -v optimum-cli &> /dev/null; then
    echo "❌ optimum-cli not found"
    echo "Install with: pip install optimum[onnxruntime]"
    exit 1
fi

# Create cache directory
mkdir -p "$CACHE_DIR"

# Convert model safe name (replace / with _)
MODEL_SAFE_NAME=$(echo "$MODEL_NAME" | tr '/' '_')
ONNX_PATH="$CACHE_DIR/${MODEL_SAFE_NAME}.onnx"

if [ -f "$ONNX_PATH" ]; then
    echo "✅ Model already exists: $ONNX_PATH"
    exit 0
fi

echo "📦 Downloading and converting $MODEL_NAME..."
echo

# Convert to ONNX using optimum-cli
optimum-cli export onnx \
    --model "$MODEL_NAME" \
    --task feature-extraction \
    --optimize O2 \
    "$CACHE_DIR/${MODEL_SAFE_NAME}-temp/"

# Move the model.onnx to expected location
mv "$CACHE_DIR/${MODEL_SAFE_NAME}-temp/model.onnx" "$ONNX_PATH"

# Clean up temporary directory
rm -rf "$CACHE_DIR/${MODEL_SAFE_NAME}-temp/"

echo
echo "✅ Model converted successfully!"
echo "📍 Location: $ONNX_PATH"
echo "📊 Size: $(du -h "$ONNX_PATH" | cut -f1)"
echo
echo "You can now use this model with the Python bridge provider."
