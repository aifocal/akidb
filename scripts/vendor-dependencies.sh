#!/usr/bin/env bash
set -euo pipefail

# vendor-dependencies.sh
# Vendors Rust dependencies for offline builds in air-gap environments
#
# This script:
# - Vendors all cargo dependencies
# - Creates .cargo/config.toml for vendored builds
# - Packages vendored deps into a tarball
# - Generates checksums for integrity verification

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

VERSION="${VERSION:-0.4.0}"
VENDOR_DIR="${PROJECT_ROOT}/vendor"
VENDOR_TARBALL="akidb-vendor-v${VERSION}.tar.gz"

echo "📦 Vendoring Rust dependencies for AkiDB v${VERSION}"
echo ""

# Check if cargo is available
if ! command -v cargo &> /dev/null; then
    echo "❌ Error: cargo not found"
    echo "   Please install Rust: https://rustup.rs"
    exit 1
fi

# Step 1: Clean previous vendor directory
echo "🧹 Step 1: Cleaning previous vendor directory..."
rm -rf "${VENDOR_DIR}"
rm -f "${PROJECT_ROOT}/${VENDOR_TARBALL}"
rm -f "${PROJECT_ROOT}/${VENDOR_TARBALL}.sha256"
echo "   ✅ Cleaned"

# Step 2: Vendor dependencies
echo ""
echo "📥 Step 2: Vendoring dependencies..."
cd "${PROJECT_ROOT}"
cargo vendor "${VENDOR_DIR}" --versioned-dirs > /dev/null 2>&1

VENDOR_SIZE=$(du -sh "${VENDOR_DIR}" | cut -f1)
CRATE_COUNT=$(find "${VENDOR_DIR}" -name "Cargo.toml" | wc -l | tr -d ' ')
echo "   ✅ Vendored ${CRATE_COUNT} crates (${VENDOR_SIZE})"

# Step 3: Create cargo config for vendored builds
echo ""
echo "⚙️  Step 3: Creating cargo config..."

mkdir -p "${VENDOR_DIR}/.cargo"
cat > "${VENDOR_DIR}/.cargo/config.toml" <<'EOF'
# Cargo configuration for vendored offline builds
# Copy this file to .cargo/config.toml in your project root

[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"

[build]
# Incremental compilation for faster rebuilds
incremental = true

# Use all available CPU cores
jobs = 8

[profile.release]
# Optimize for performance
opt-level = 3
lto = true
codegen-units = 1
strip = true

[target.aarch64-unknown-linux-gnu]
linker = "aarch64-linux-gnu-gcc"

[target.aarch64-apple-darwin]
rustflags = ["-C", "target-cpu=native"]
EOF

echo "   ✅ Config created at vendor/.cargo/config.toml"

# Step 4: Create README for vendor package
echo ""
echo "📄 Step 4: Creating vendor README..."

cat > "${VENDOR_DIR}/README.txt" <<EOF
AkiDB Vendored Dependencies v${VERSION}
=======================================

This package contains all Rust dependencies needed to build AkiDB
offline without internet access.

CONTENTS:
---------
  vendor/           - ${CRATE_COUNT} vendored crates
  .cargo/           - Cargo configuration for offline builds
  README.txt        - This file

USAGE:
------
1. Extract vendor tarball in your AkiDB source directory:
   tar -xzf ${VENDOR_TARBALL} -C /path/to/akidb/

2. Copy cargo config:
   cp vendor/.cargo/config.toml .cargo/config.toml

3. Build AkiDB (no internet required):
   cargo build --release --offline

4. Verify offline build works:
   cargo build --release --offline --workspace

VERIFICATION:
-------------
# Check vendor directory exists
ls -la vendor/

# Verify cargo uses vendored sources
cargo build --offline --dry-run

# Build specific service
cargo build --release --offline -p akidb-api

NOTES:
------
- This vendor package was created on: $(date -u +"%Y-%m-%d %H:%M:%S UTC")
- Rust version: $(rustc --version 2>/dev/null || echo "unknown")
- Cargo version: $(cargo --version 2>/dev/null || echo "unknown")
- Platform: $(uname -sm)

SYSTEM REQUIREMENTS:
--------------------
- Rust 1.70+ (rustc and cargo)
- ARM64 architecture (Apple Silicon, Jetson, Graviton)
- Linux or macOS
- GCC/Clang toolchain
- OpenSSL development libraries (libssl-dev)

TROUBLESHOOTING:
----------------
If build fails with "can't find crate":
  1. Ensure .cargo/config.toml is in project root
  2. Check vendor/ directory is at project root
  3. Use --offline flag: cargo build --release --offline

If linker errors occur:
  1. Install build essentials: apt-get install build-essential
  2. Install OpenSSL: apt-get install libssl-dev pkg-config
  3. For cross-compilation, install target toolchain

SUPPORT:
--------
  GitHub: https://github.com/aifocal/akidb
  Issues: https://github.com/aifocal/akidb/issues
EOF

echo "   ✅ README created"

# Step 5: Create vendor verification script
echo ""
echo "🔍 Step 5: Creating verification script..."

cat > "${VENDOR_DIR}/verify-vendor.sh" <<'VERIFY_EOF'
#!/usr/bin/env bash
set -euo pipefail

echo "🔍 Verifying vendored dependencies..."
echo ""

# Check vendor directory structure
if [[ ! -d "vendor" ]]; then
    echo "❌ Error: vendor/ directory not found"
    echo "   Expected: $(pwd)/vendor/"
    exit 1
fi

if [[ ! -f ".cargo/config.toml" ]]; then
    echo "⚠️  Warning: .cargo/config.toml not found"
    echo "   Copy vendor/.cargo/config.toml to .cargo/config.toml"
fi

# Check cargo can see vendored crates
echo "📦 Checking vendored crates..."
CRATE_COUNT=$(find vendor -name "Cargo.toml" | wc -l | tr -d ' ')
echo "   Found ${CRATE_COUNT} vendored crates"

# Test offline build (dry run)
echo ""
echo "🧪 Testing offline build (dry run)..."
if cargo build --offline --dry-run > /dev/null 2>&1; then
    echo "   ✅ Offline build configuration is valid"
else
    echo "   ❌ Offline build failed"
    echo "   Run: cargo build --offline --dry-run"
    exit 1
fi

# Check for common issues
echo ""
echo "🔎 Checking for common issues..."

if ! command -v rustc &> /dev/null; then
    echo "   ⚠️  rustc not found - install Rust toolchain"
fi

if ! command -v cargo &> /dev/null; then
    echo "   ⚠️  cargo not found - install Rust toolchain"
fi

# Check OpenSSL (common dependency)
if ! pkg-config --exists openssl 2>/dev/null; then
    echo "   ⚠️  OpenSSL development libraries not found"
    echo "      Install: apt-get install libssl-dev pkg-config"
fi

echo ""
echo "✅ Verification complete!"
echo ""
echo "To build offline:"
echo "  cargo build --release --offline --workspace"
VERIFY_EOF

chmod +x "${VENDOR_DIR}/verify-vendor.sh"
echo "   ✅ Verification script created"

# Step 6: Generate checksums for all vendored crates
echo ""
echo "🔐 Step 6: Generating checksums..."

cd "${VENDOR_DIR}"
find . -type f -name "*.rs" -o -name "Cargo.toml" | sort | xargs sha256sum > CHECKSUMS.txt
CHECKSUM_COUNT=$(wc -l < CHECKSUMS.txt | tr -d ' ')
echo "   ✅ Generated checksums for ${CHECKSUM_COUNT} files"

# Step 7: Create tarball
echo ""
echo "📦 Step 7: Creating vendor tarball..."
cd "${PROJECT_ROOT}"
tar -czf "${VENDOR_TARBALL}" vendor/

TARBALL_SIZE=$(du -h "${VENDOR_TARBALL}" | cut -f1)
echo "   ✅ Tarball created: ${VENDOR_TARBALL} (${TARBALL_SIZE})"

# Generate tarball checksum
sha256sum "${VENDOR_TARBALL}" > "${VENDOR_TARBALL}.sha256"
TARBALL_SHA=$(cut -d' ' -f1 "${VENDOR_TARBALL}.sha256")

echo ""
echo "🎉 Dependency vendoring complete!"
echo ""
echo "Vendor tarball: ${PROJECT_ROOT}/${VENDOR_TARBALL}"
echo "Size: ${TARBALL_SIZE}"
echo "Crates: ${CRATE_COUNT}"
echo "SHA-256: ${TARBALL_SHA}"
echo ""
echo "To use on air-gap system:"
echo "  1. Transfer ${VENDOR_TARBALL} to target system"
echo "  2. Extract: tar -xzf ${VENDOR_TARBALL}"
echo "  3. Verify: cd vendor && ./verify-vendor.sh"
echo "  4. Copy config: cp vendor/.cargo/config.toml .cargo/config.toml"
echo "  5. Build: cargo build --release --offline --workspace"
echo ""
echo "To verify tarball integrity:"
echo "  sha256sum -c ${VENDOR_TARBALL}.sha256"
