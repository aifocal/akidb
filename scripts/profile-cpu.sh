#!/bin/bash
# CPU profiling with flamegraph
#
# Usage: ./scripts/profile-cpu.sh [target]
# Examples:
#   ./scripts/profile-cpu.sh parallel_upload_bench
#   ./scripts/profile-cpu.sh akidb-rest

set -e

TARGET="${1:-parallel_upload_bench}"

echo "🔥 CPU Profiling: $TARGET"
echo "================================"
echo ""

# Check if flamegraph is installed
if ! command -v cargo-flamegraph &> /dev/null; then
    echo "Installing cargo-flamegraph..."
    cargo install flamegraph
fi

# Run profiling based on target type
if [[ "$TARGET" == *"_bench" ]]; then
    echo "Profiling benchmark: $TARGET"
    sudo cargo flamegraph --bench "$TARGET" --output "flamegraph-$TARGET.svg"
elif [[ "$TARGET" == akidb-* ]]; then
    echo "Profiling binary: $TARGET"
    sudo cargo flamegraph --bin "$TARGET" --output "flamegraph-$TARGET.svg"
else
    echo "Profiling test: $TARGET"
    sudo cargo flamegraph --test "$TARGET" --output "flamegraph-$TARGET.svg"
fi

echo ""
echo "✅ Flamegraph saved to: flamegraph-$TARGET.svg"
echo ""
echo "To view:"
echo "  open flamegraph-$TARGET.svg"
echo ""
