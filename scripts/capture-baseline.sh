#!/bin/bash
set -euo pipefail

echo "[AkiDB] Capturing Phase 2 performance baseline..."

# Criterion baseline capture (run benchmarks and save as baseline)
cargo bench --package akidb-benchmarks --bench vector_search
cargo bench --package akidb-benchmarks --bench index_build
cargo bench --package akidb-benchmarks --bench metadata_ops

echo ""
echo "✅ Phase 2 baseline captured successfully!"
echo "📊 Results saved to: target/criterion/"
echo "📈 View HTML report: open target/criterion/report/index.html"
