#!/bin/bash
# Memory profiling with heaptrack or Instruments
#
# Usage: ./scripts/profile-memory.sh [target]
# Examples:
#   ./scripts/profile-memory.sh load_test
#   ./scripts/profile-memory.sh akidb-rest

set -e

TARGET="${1:-load_test}"

echo "💾 Memory Profiling: $TARGET"
echo "================================"
echo ""

# Check platform
if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS - use Instruments
    echo "Using macOS Instruments for memory profiling"
    echo ""

    # Build the target first
    if [[ "$TARGET" == *"_test" ]]; then
        echo "Building test: $TARGET"
        cargo test --no-run -p akidb-storage "$TARGET"

        # Find the test binary
        TEST_BINARY=$(find target/debug/deps -name "${TARGET}*" -type f -perm +111 | head -1)

        if [ -z "$TEST_BINARY" ]; then
            echo "Error: Could not find test binary for $TARGET"
            exit 1
        fi

        echo "Test binary: $TEST_BINARY"
        echo ""
        echo "Running Instruments Allocations profiler..."
        instruments -t "Allocations" -D "memory-$TARGET.trace" "$TEST_BINARY"
    elif [[ "$TARGET" == akidb-* ]]; then
        echo "Building binary: $TARGET"
        cargo build --release --bin "$TARGET"

        echo "Running Instruments Allocations profiler..."
        instruments -t "Allocations" -D "memory-$TARGET.trace" "target/release/$TARGET"
    else
        echo "Unsupported target type: $TARGET"
        exit 1
    fi

    echo ""
    echo "✅ Memory trace saved to: memory-$TARGET.trace"
    echo ""
    echo "To view:"
    echo "  open memory-$TARGET.trace"
    echo ""

elif command -v heaptrack &> /dev/null; then
    # Linux with heaptrack
    echo "Using heaptrack for memory profiling"
    echo ""

    if [[ "$TARGET" == *"_test" ]]; then
        echo "Profiling test: $TARGET"
        heaptrack cargo test -p akidb-storage "$TARGET" -- --test-threads=1
    elif [[ "$TARGET" == akidb-* ]]; then
        echo "Profiling binary: $TARGET"
        heaptrack cargo run --release --bin "$TARGET"
    else
        echo "Unsupported target type: $TARGET"
        exit 1
    fi

    echo ""
    echo "✅ Memory profile saved"
    echo ""
    echo "To view:"
    echo "  heaptrack_gui heaptrack.$TARGET.*.gz"
    echo ""

else
    echo "Error: No memory profiling tool found"
    echo ""
    echo "macOS: Instruments is available by default"
    echo "Linux: Install heaptrack with:"
    echo "  sudo apt-get install heaptrack"
    echo ""
    exit 1
fi
