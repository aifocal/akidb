#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${PROJECT_ROOT}"

TARGET_DIR="${PROJECT_ROOT}/target/release"
DIST_DIR="${PROJECT_ROOT}/dist"
BIN_NAME="akidb-server"

echo "Building optimized release binary..."
cargo build --package akidb-api --release

mkdir -p "${DIST_DIR}"

if command -v strip &>/dev/null; then
  echo "Stripping binary symbols..."
  strip "${TARGET_DIR}/akidb-api" || true
fi

cp "${TARGET_DIR}/akidb-api" "${DIST_DIR}/${BIN_NAME}"

echo "Release binary available at ${DIST_DIR}/${BIN_NAME}"
