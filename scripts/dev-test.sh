#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${PROJECT_ROOT}"

export RUSTFLAGS="${RUSTFLAGS:-} -D warnings"

echo "Running cargo fmt..."
cargo fmt --all -- --check

echo "Running cargo clippy..."
cargo clippy --all-targets --all-features --workspace -- -D warnings

echo "Running cargo test..."
cargo test --workspace --all-targets --all-features

echo "Running benchmark guard..."
cargo test --workspace --benches --all-features

echo "All checks passed."
