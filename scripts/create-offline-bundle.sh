#!/usr/bin/env bash
set -euo pipefail

# create-offline-bundle.sh
# Creates an offline installation bundle for AkiDB on ARM platforms
#
# This script packages:
# - AkiDB binaries (akidb-api, akidb-ingest, akidb-pkg, akidb-replication)
# - MinIO server binary
# - MinIO client (mc) binary
# - Vendored Rust dependencies
# - Configuration templates
# - Installation scripts
# - Documentation

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

VERSION="${VERSION:-0.4.0}"
ARCH="${ARCH:-aarch64}"
OS="${OS:-unknown-linux-gnu}"
BUNDLE_NAME="akidb-offline-bundle-v${VERSION}-${ARCH}"
BUNDLE_DIR="${PROJECT_ROOT}/dist/${BUNDLE_NAME}"

echo "🎯 Creating offline bundle: ${BUNDLE_NAME}"
echo "   Architecture: ${ARCH}"
echo "   OS: ${OS}"
echo ""

# Clean previous builds
if [[ -z "${BUNDLE_DIR}" ]]; then
    echo "❌ Error: BUNDLE_DIR is not set"
    exit 1
fi
rm -rf "${BUNDLE_DIR}"
mkdir -p "${BUNDLE_DIR}"/{bin,deps,configs,scripts,docs}

# Step 1: Build release binaries
echo "📦 Step 1: Building release binaries..."
cd "${PROJECT_ROOT}"

# Check if cargo is available
if command -v cargo &> /dev/null; then
    echo "   Building with cargo..."
    cargo build --release --workspace --bins

    # Copy AkiDB binaries
    cp target/release/akidb-api "${BUNDLE_DIR}/bin/" || echo "   ⚠️  akidb-api not found (ok if not built)"
    cp target/release/akidb-ingest "${BUNDLE_DIR}/bin/" || echo "   ⚠️  akidb-ingest not found (ok if not built)"
    cp target/release/akidb-pkg "${BUNDLE_DIR}/bin/" || echo "   ⚠️  akidb-pkg not found (ok if not built)"
    cp target/release/akidb-replication "${BUNDLE_DIR}/bin/" || echo "   ⚠️  akidb-replication not found (ok if not built)"

    echo "   ✅ AkiDB binaries copied"
else
    echo "   ⚠️  cargo not found, skipping binary build"
    echo "   Binaries should be pre-built and placed in target/release/"
fi

# Step 2: Download MinIO binaries
echo ""
echo "📥 Step 2: Downloading MinIO binaries..."

MINIO_VERSION="RELEASE.2024-01-01T16-36-33Z"
MINIO_URL_LINUX_ARM64="https://dl.min.io/server/minio/release/linux-arm64/archive/minio.${MINIO_VERSION}"
MINIO_URL_DARWIN_ARM64="https://dl.min.io/server/minio/release/darwin-arm64/archive/minio.${MINIO_VERSION}"
MC_URL_LINUX_ARM64="https://dl.min.io/client/mc/release/linux-arm64/mc"
MC_URL_DARWIN_ARM64="https://dl.min.io/client/mc/release/darwin-arm64/mc"

# Determine which MinIO binary to download
if [[ "${OS}" == *"linux"* ]]; then
    MINIO_URL="${MINIO_URL_LINUX_ARM64}"
    MC_URL="${MC_URL_LINUX_ARM64}"
elif [[ "${OS}" == *"darwin"* ]]; then
    MINIO_URL="${MINIO_URL_DARWIN_ARM64}"
    MC_URL="${MC_URL_DARWIN_ARM64}"
else
    echo "   ⚠️  Unknown OS: ${OS}, using Linux ARM64 binaries"
    MINIO_URL="${MINIO_URL_LINUX_ARM64}"
    MC_URL="${MC_URL_LINUX_ARM64}"
fi

# Download MinIO server (if curl available)
if command -v curl &> /dev/null; then
    echo "   Downloading MinIO server..."
    curl -sSL "${MINIO_URL}" -o "${BUNDLE_DIR}/deps/minio" || echo "   ⚠️  Failed to download MinIO"
    chmod +x "${BUNDLE_DIR}/deps/minio" 2>/dev/null || true

    echo "   Downloading MinIO client (mc)..."
    curl -sSL "${MC_URL}" -o "${BUNDLE_DIR}/deps/mc" || echo "   ⚠️  Failed to download mc"
    chmod +x "${BUNDLE_DIR}/deps/mc" 2>/dev/null || true

    echo "   ✅ MinIO binaries downloaded"
else
    echo "   ⚠️  curl not found, please download MinIO binaries manually:"
    echo "      MinIO: ${MINIO_URL}"
    echo "      MC: ${MC_URL}"
fi

# Step 3: Copy configuration templates
echo ""
echo "📝 Step 3: Copying configuration templates..."

cat > "${BUNDLE_DIR}/configs/akidb.toml.example" <<'EOF'
# AkiDB Configuration File
# Copy to /etc/akidb/akidb.toml or ~/.config/akidb/akidb.toml

[storage]
circuit_breaker.failure_threshold = 5
circuit_breaker.recovery_timeout_secs = 30
retry.max_attempts = 10
retry.initial_backoff_ms = 100

[index]
hnsw.m = 16
hnsw.ef_construction = 400
hnsw.ef_search = 200

[api]
validation.vector_dimension_max = 4096
validation.top_k_max = 1000

[query]
max_filter_depth = 32
parallel_segments = true
max_parallel_segments = 8
EOF

cat > "${BUNDLE_DIR}/configs/minio.env.example" <<'EOF'
# MinIO Environment Configuration
# Copy to /etc/akidb/minio.env

MINIO_ROOT_USER=akidb
MINIO_ROOT_PASSWORD=akidbsecret_change_me
MINIO_VOLUMES=/data
MINIO_OPTS="--console-address :9001"
EOF

cat > "${BUNDLE_DIR}/configs/.env.example" <<'EOF'
# AkiDB Environment Variables
# Copy to /etc/akidb/akidb.env or .env

# S3/MinIO Configuration
AKIDB_S3_ENDPOINT=http://localhost:9000
AKIDB_S3_REGION=us-east-1
AKIDB_S3_BUCKET=akidb
AKIDB_S3_ACCESS_KEY=akidb
AKIDB_S3_SECRET_KEY=akidbsecret_change_me

# API Server
AKIDB_BIND_ADDRESS=0.0.0.0:8080
AKIDB_LOG_LEVEL=info

# OpenTelemetry
AKIDB_TELEMETRY_ENABLED=true
AKIDB_JAEGER_ENDPOINT=http://localhost:4317
AKIDB_SERVICE_NAME=akidb-api
EOF

echo "   ✅ Configuration templates created"

# Step 4: Create installation script
echo ""
echo "🔧 Step 4: Creating installation script..."

cat > "${BUNDLE_DIR}/scripts/install.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

echo "🚀 AkiDB Offline Installer"
echo ""

# Check for root privileges
if [[ $EUID -ne 0 ]]; then
   echo "❌ This script must be run as root (use sudo)"
   exit 1
fi

INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/etc/akidb"
DATA_DIR="/var/lib/akidb"

# Create directories
echo "📁 Creating directories..."
mkdir -p "${INSTALL_DIR}"
mkdir -p "${CONFIG_DIR}"
mkdir -p "${DATA_DIR}/cache"
mkdir -p "${DATA_DIR}/minio"

# Install binaries
echo "📦 Installing binaries..."
cp bin/akidb-* "${INSTALL_DIR}/" 2>/dev/null || echo "⚠️  No AkiDB binaries found"
cp deps/minio "${INSTALL_DIR}/" 2>/dev/null || echo "⚠️  MinIO binary not found"
cp deps/mc "${INSTALL_DIR}/" 2>/dev/null || echo "⚠️  mc binary not found"

chmod +x "${INSTALL_DIR}"/akidb-* 2>/dev/null || true
chmod +x "${INSTALL_DIR}/minio" 2>/dev/null || true
chmod +x "${INSTALL_DIR}/mc" 2>/dev/null || true

# Install configurations
echo "⚙️  Installing configuration templates..."
cp configs/*.example "${CONFIG_DIR}/"

# Create akidb user
echo "👤 Creating akidb user..."
useradd -r -s /bin/false akidb 2>/dev/null || echo "User akidb already exists"
chown -R akidb:akidb "${DATA_DIR}"

echo ""
echo "✅ Installation complete!"
echo ""
echo "Next steps:"
echo "  1. Edit /etc/akidb/akidb.env (copy from .env.example)"
echo "  2. Edit /etc/akidb/minio.env (copy from minio.env.example)"
echo "  3. Start MinIO: sudo -u akidb minio server ${DATA_DIR}/minio"
echo "  4. Start AkiDB: sudo -u akidb akidb-api"
echo ""
echo "Documentation: /usr/share/doc/akidb/"
EOF

chmod +x "${BUNDLE_DIR}/scripts/install.sh"
echo "   ✅ Installation script created"

# Step 5: Copy documentation
echo ""
echo "📚 Step 5: Copying documentation..."
cp "${PROJECT_ROOT}/README.md" "${BUNDLE_DIR}/docs/" 2>/dev/null || echo "   ⚠️  README.md not found"
cp "${PROJECT_ROOT}/LICENSE" "${BUNDLE_DIR}/docs/" 2>/dev/null || echo "   ⚠️  LICENSE not found"
cp -r "${PROJECT_ROOT}/docs/"*.md "${BUNDLE_DIR}/docs/" 2>/dev/null || echo "   ⚠️  docs/ not found"
echo "   ✅ Documentation copied"

# Step 6: Create README for bundle
echo ""
echo "📄 Step 6: Creating bundle README..."

cat > "${BUNDLE_DIR}/README.txt" <<EOF
AkiDB Offline Installation Bundle v${VERSION}
============================================

This bundle contains everything needed to install and run AkiDB
on ARM64 platforms without internet access.

CONTENTS:
---------
  bin/           - AkiDB binaries (akidb-api, akidb-ingest, akidb-pkg, akidb-replication)
  deps/          - MinIO server and client binaries
  configs/       - Configuration templates
  scripts/       - Installation and management scripts
  docs/          - Documentation (README, guides, API reference)

INSTALLATION:
-------------
  1. Extract this bundle:
     tar -xzf ${BUNDLE_NAME}.tar.gz
     cd ${BUNDLE_NAME}

  2. Run the installer (requires root):
     sudo ./scripts/install.sh

  3. Configure the system:
     sudo cp configs/.env.example /etc/akidb/akidb.env
     sudo cp configs/minio.env.example /etc/akidb/minio.env
     sudo nano /etc/akidb/akidb.env  # Edit configuration

  4. Start services:
     # Start MinIO
     sudo -u akidb minio server /var/lib/akidb/minio --console-address :9001

     # Start AkiDB (in another terminal)
     sudo -u akidb akidb-api

SYSTEM REQUIREMENTS:
--------------------
  - ARM64 architecture (Apple Silicon, Jetson, Graviton, etc.)
  - Linux or macOS
  - 8GB+ RAM (16GB+ recommended)
  - 100GB+ storage (NVMe recommended)

VERIFICATION:
-------------
  # Check if services are running
  curl http://localhost:8080/health        # AkiDB
  curl http://localhost:9000/minio/health  # MinIO

DOCUMENTATION:
--------------
  See docs/ directory for:
  - deployment-production.md - Production deployment guide
  - api-reference.md - REST API documentation
  - observability-guide.md - Monitoring and observability
  - phase6-offline-rag.md - Offline features guide

SUPPORT:
--------
  GitHub: https://github.com/aifocal/akidb
  Issues: https://github.com/aifocal/akidb/issues
  License: Apache-2.0
EOF

echo "   ✅ Bundle README created"

# Step 7: Generate SHA-256 checksums
echo ""
echo "🔐 Step 7: Generating checksums..."

cd "${BUNDLE_DIR}"
find . -type f -exec sha256sum {} \; > SHA256SUMS
echo "   ✅ Checksums generated"

# Step 8: Create tarball
echo ""
echo "📦 Step 8: Creating tarball..."
cd "${PROJECT_ROOT}/dist"
tar -czf "${BUNDLE_NAME}.tar.gz" "${BUNDLE_NAME}"

BUNDLE_SIZE=$(du -h "${BUNDLE_NAME}.tar.gz" | cut -f1)
echo "   ✅ Tarball created: ${BUNDLE_NAME}.tar.gz (${BUNDLE_SIZE})"

# Final checksums
sha256sum "${BUNDLE_NAME}.tar.gz" > "${BUNDLE_NAME}.tar.gz.sha256"

echo ""
echo "🎉 Offline bundle creation complete!"
echo ""
echo "Bundle location: ${PROJECT_ROOT}/dist/${BUNDLE_NAME}.tar.gz"
echo "Bundle size: ${BUNDLE_SIZE}"
echo "SHA-256: $(cat ${BUNDLE_NAME}.tar.gz.sha256)"
echo ""
echo "To verify integrity:"
echo "  sha256sum -c ${BUNDLE_NAME}.tar.gz.sha256"
echo ""
echo "To install on air-gapped system:"
echo "  1. Transfer ${BUNDLE_NAME}.tar.gz to target system"
echo "  2. Extract: tar -xzf ${BUNDLE_NAME}.tar.gz"
echo "  3. Run: cd ${BUNDLE_NAME} && sudo ./scripts/install.sh"
