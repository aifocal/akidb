#!/bin/bash
# Run all performance benchmarks
#
# Usage: ./scripts/run-benchmarks.sh [target]
# Examples:
#   ./scripts/run-benchmarks.sh all
#   ./scripts/run-benchmarks.sh batch
#   ./scripts/run-benchmarks.sh parallel

set -e

TARGET="${1:-all}"

echo "⚡ AkiDB Performance Benchmarks"
echo "================================"
echo "Target: $TARGET"
echo ""

if [[ "$TARGET" == "all" || "$TARGET" == "batch" ]]; then
    echo "Running batch upload benchmarks..."
    cargo bench -p akidb-storage --bench batch_upload_bench
    echo ""
fi

if [[ "$TARGET" == "all" || "$TARGET" == "parallel" ]]; then
    echo "Running parallel upload benchmarks..."
    cargo bench -p akidb-storage --bench parallel_upload_bench
    echo ""
fi

if [[ "$TARGET" == "all" || "$TARGET" == "mock" ]]; then
    echo "Running MockS3 benchmarks..."
    cargo bench -p akidb-storage --bench mock_s3_bench
    echo ""
fi

echo "✅ Benchmarks complete"
echo ""
echo "View detailed results:"
echo "  open target/criterion/report/index.html"
echo ""
