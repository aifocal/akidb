#!/bin/bash
# Build ONNX Runtime with CoreML Execution Provider for Apple Silicon
# This script builds ONNX Runtime from source with CoreML support

set -e  # Exit on error
set -u  # Exit on undefined variable
set -o pipefail

echo "🏗️  Building ONNX Runtime with CoreML Support"
echo "=============================================="
echo

# Configuration
ONNXRUNTIME_VERSION="v1.16.3"  # Match ort crate version
BUILD_DIR="$HOME/onnxruntime-build"
INSTALL_DIR="/usr/local/onnxruntime"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

info() {
    echo -e "${GREEN}✅ $1${NC}"
}

warn() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

error() {
    echo -e "${RED}❌ $1${NC}"
    exit 1
}

apply_date_coreml_patch() {
    local deps_file="$BUILD_DIR/cmake/external/onnxruntime_external_deps.cmake"
    local patch_dir="$BUILD_DIR/patches/date"
    local patch_file="$patch_dir/cmake_minimum_version.patch"

    if [ ! -f "$deps_file" ]; then
        warn "ONNX Runtime sources not found at $deps_file. Skipping CoreML compatibility patch."
        return
    fi

    mkdir -p "$patch_dir"
    cat <<'EOF' > "$patch_file"
--- a/CMakeLists.txt
+++ b/CMakeLists.txt
@@
-cmake_minimum_required( VERSION 3.1.0 )
+cmake_minimum_required( VERSION 3.5.0 )
EOF

    if grep -q "ONNXRUNTIME_DATE_PATCH_COMMAND" "$deps_file"; then
        info "Date dependency already patched for modern CMake."
        return
    fi

    python3 - "$deps_file" <<'PY'
import pathlib
import sys

deps_path = pathlib.Path(sys.argv[1])
text = deps_path.read_text()

if "ONNXRUNTIME_DATE_PATCH_COMMAND" in text:
    sys.exit(0)

needle = """FetchContent_Declare(
      date
      URL ${DEP_URL_date}
      URL_HASH SHA1=${DEP_SHA1_date}
    )"""

replacement = """set(ONNXRUNTIME_DATE_PATCH_COMMAND "")
if(Patch_FOUND)
  set(ONNXRUNTIME_DATE_PATCH_COMMAND ${Patch_EXECUTABLE} --binary --ignore-whitespace -p1 < ${PROJECT_SOURCE_DIR}/patches/date/cmake_minimum_version.patch)
endif()

FetchContent_Declare(
      date
      URL ${DEP_URL_date}
      URL_HASH SHA1=${DEP_SHA1_date}
      PATCH_COMMAND ${ONNXRUNTIME_DATE_PATCH_COMMAND}
    )"""

if needle not in text:
    sys.exit("Failed to locate the FetchContent block for 'date'.")

deps_path.write_text(text.replace(needle, replacement, 1))
PY

    info "Enabled CMake patch to bump Howard Hinnant date dependency to CMake >=3.5."
}

# Step 1: Check prerequisites
echo "📋 Step 1: Checking prerequisites..."
command -v cmake >/dev/null 2>&1 || error "CMake not found. Install with: brew install cmake"
command -v ninja >/dev/null 2>&1 || error "Ninja not found. Install with: brew install ninja"
command -v protoc >/dev/null 2>&1 || error "Protobuf not found. Install with: brew install protobuf"
command -v xcodebuild >/dev/null 2>&1 || error "Xcode not found. Install from App Store"

info "All prerequisites met"
echo

# Step 2: Clone ONNX Runtime
echo "📦 Step 2: Cloning ONNX Runtime..."
if [ -d "$BUILD_DIR" ]; then
    warn "Build directory exists: $BUILD_DIR"
    read -p "Remove and re-clone? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        rm -rf "$BUILD_DIR"
    else
        info "Using existing directory"
    fi
fi

if [ ! -d "$BUILD_DIR" ]; then
    git clone --recursive --branch "$ONNXRUNTIME_VERSION" \
        https://github.com/microsoft/onnxruntime.git "$BUILD_DIR" \
        || error "Failed to clone ONNX Runtime"

    cd "$BUILD_DIR"
    info "Cloned ONNX Runtime $ONNXRUNTIME_VERSION"
else
    cd "$BUILD_DIR"
    info "Using existing ONNX Runtime directory"
fi
echo

# Step 3: Patch dependencies for modern CMake
echo "🩹 Step 3: Applying CoreML compatibility patches..."
apply_date_coreml_patch
echo

# Step 4: Build with CoreML
echo "🔨 Step 4: Building ONNX Runtime with CoreML..."
echo "This will take 20-30 minutes. Go get coffee ☕"
echo

./build.sh \
    --config Release \
    --use_coreml \
    --build_shared_lib \
    --parallel \
    --skip_tests \
    --cmake_extra_defines \
        CMAKE_OSX_ARCHITECTURES=arm64 \
        CMAKE_OSX_DEPLOYMENT_TARGET=11.0 \
    || error "Build failed"

info "Build completed successfully"
echo

# Step 5: Verify build artifacts
echo "🔍 Step 5: Verifying build artifacts..."
DYLIB_PATH="$BUILD_DIR/build/MacOS/Release/libonnxruntime.dylib"

if [ ! -f "$DYLIB_PATH" ]; then
    error "libonnxruntime.dylib not found at: $DYLIB_PATH"
fi

info "Found libonnxruntime.dylib"
ls -lh "$DYLIB_PATH"
echo

# Step 6: Optional install to system
echo "📍 Step 6: Installation"
echo "The library is ready at: $DYLIB_PATH"
echo
read -p "Install to /usr/local/onnxruntime? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    sudo mkdir -p "$INSTALL_DIR/lib"
    sudo mkdir -p "$INSTALL_DIR/include"

    sudo cp "$DYLIB_PATH" "$INSTALL_DIR/lib/"
    sudo cp -r "$BUILD_DIR/include/onnxruntime" "$INSTALL_DIR/include/"

    info "Installed to $INSTALL_DIR"
else
    info "Skipped installation"
fi
echo

# Step 7: Print environment setup
echo "🔧 Step 7: Environment Setup"
echo "Add these to your ~/.zshrc or ~/.bashrc:"
echo
echo "export ORT_STRATEGY=system"
echo "export ORT_DYLIB_PATH=\"$DYLIB_PATH\""
echo
info "Build complete! 🎉"
echo
echo "Next steps:"
echo "1. Add environment variables to your shell config"
echo "2. Update Cargo.toml to use system ONNX Runtime"
echo "3. Run: cargo build -p akidb-embedding"
