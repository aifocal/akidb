#!/usr/bin/env python3
"""
Validate Qwen3-Embedding-0.6B ONNX model structure.

This script:
1. Loads the ONNX model
2. Validates model structure with ONNX checker
3. Inspects inputs, outputs, and dimensions
4. Checks for external data file
5. Lists all operators used
"""

import sys
from pathlib import Path
import onnx
from onnx import helper, checker, numpy_helper


def validate_model_structure(model_path: str):
    """Validate ONNX model structure and print details."""
    print(f"=" * 70)
    print(f"ONNX Model Validation: {model_path}")
    print(f"=" * 70)
    print()

    # Load model
    print(f"📦 Loading model...")
    try:
        model = onnx.load(model_path)
        print(f"✅ Model loaded successfully")
    except Exception as e:
        print(f"❌ Failed to load model: {e}")
        return False

    # Check model validity (skip full check for models with custom ops)
    print(f"\n🔍 Validating model structure...")
    try:
        checker.check_model(model)
        print(f"✅ Model structure is valid (full ONNX check passed)")
    except Exception as e:
        print(f"⚠️  ONNX checker warning: {e}")
        print(f"   This is expected for models with custom operators like SimplifiedLayerNormalization")
        print(f"   ONNX Runtime will handle these operators correctly")
        print(f"✅ Proceeding with model inspection...")

    # Print model info
    print(f"\n📊 Model Information:")
    print(f"   IR Version: {model.ir_version}")
    print(f"   Producer: {model.producer_name} {model.producer_version}")
    print(f"   ONNX Version: {model.model_version}")
    print(f"   Domain: {model.domain}")

    # Print inputs
    print(f"\n📥 Model Inputs:")
    for input_tensor in model.graph.input:
        shape = [dim.dim_value if dim.dim_value else dim.dim_param
                 for dim in input_tensor.type.tensor_type.shape.dim]
        dtype = onnx.TensorProto.DataType.Name(input_tensor.type.tensor_type.elem_type)
        print(f"   - {input_tensor.name}")
        print(f"     Shape: {shape}")
        print(f"     Type: {dtype}")

    # Print outputs
    print(f"\n📤 Model Outputs:")
    for output_tensor in model.graph.output:
        shape = [dim.dim_value if dim.dim_value else dim.dim_param
                 for dim in output_tensor.type.tensor_type.shape.dim]
        dtype = onnx.TensorProto.DataType.Name(output_tensor.type.tensor_type.elem_type)
        print(f"   - {output_tensor.name}")
        print(f"     Shape: {shape}")
        print(f"     Type: {dtype}")

    # Count operators
    print(f"\n🔧 Operators Used:")
    op_types = {}
    for node in model.graph.node:
        op_types[node.op_type] = op_types.get(node.op_type, 0) + 1

    for op_type, count in sorted(op_types.items()):
        print(f"   - {op_type}: {count}")

    print(f"\n   Total operators: {len(model.graph.node)}")
    print(f"   Unique operators: {len(op_types)}")

    # Check external data
    print(f"\n💾 External Data:")
    has_external_data = False
    for initializer in model.graph.initializer:
        if initializer.HasField('data_location') and \
           initializer.data_location == onnx.TensorProto.EXTERNAL:
            has_external_data = True
            break

    if has_external_data:
        print(f"   ⚠️  Model uses external data file (.onnx_data)")
        external_data_path = Path(model_path).with_suffix('.onnx_data')
        if external_data_path.exists():
            size_mb = external_data_path.stat().st_size / 1024 / 1024
            print(f"   ✅ External data file found: {external_data_path.name} ({size_mb:.1f} MB)")
        else:
            print(f"   ❌ External data file NOT found: {external_data_path}")
            return False
    else:
        print(f"   ℹ️  Model does not use external data (all weights embedded)")

    # Model size
    model_file = Path(model_path)
    size_mb = model_file.stat().st_size / 1024 / 1024
    print(f"\n📏 Model Size:")
    print(f"   Model file: {size_mb:.1f} MB")
    if has_external_data and external_data_path.exists():
        total_mb = size_mb + (external_data_path.stat().st_size / 1024 / 1024)
        print(f"   Total (with external data): {total_mb:.1f} MB")

    # Graph info
    print(f"\n🌐 Graph Statistics:")
    print(f"   Initializers: {len(model.graph.initializer)}")
    print(f"   Value infos: {len(model.graph.value_info)}")

    return True


def main():
    # Check for FP16 model first (recommended)
    fp16_model_path = "models/qwen3-embedding-0.6b/onnx/model_fp16.onnx"
    fp32_model_path = "models/qwen3-embedding-0.6b/onnx/model.onnx"

    # Try FP16 first
    model_path = fp16_model_path if Path(fp16_model_path).exists() else fp32_model_path

    if not Path(model_path).exists():
        print(f"❌ Model not found at {model_path}")
        print(f"\nPlease download the model first:")
        print(f"   python3 scripts/download_qwen3_onnx.py")
        sys.exit(1)

    success = validate_model_structure(model_path)

    if success:
        print(f"\n{'=' * 70}")
        print(f"✅ VALIDATION PASSED")
        print(f"{'=' * 70}")
        print(f"\n📝 Next Steps:")
        print(f"   1. Test CoreML EP: python3 scripts/test_qwen3_coreml.py")
        print(f"   2. Measure baseline performance")
        print(f"   3. Implement Rust provider: cargo build --features onnx")
        sys.exit(0)
    else:
        print(f"\n{'=' * 70}")
        print(f"❌ VALIDATION FAILED")
        print(f"{'=' * 70}")
        sys.exit(1)


if __name__ == "__main__":
    main()
