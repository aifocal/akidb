#!/bin/bash
# Run load test (short or full)
#
# Usage: ./scripts/run-load-test.sh [short|full]
# Examples:
#   ./scripts/run-load-test.sh short  # 5 seconds
#   ./scripts/run-load-test.sh full   # 10 minutes (requires --ignored flag)

set -e

MODE="${1:-short}"

echo "🚀 AkiDB Load Test"
echo "================================"
echo "Mode: $MODE"
echo ""

if [[ "$MODE" == "short" ]]; then
    echo "Running short load test (5 seconds, 10 QPS)..."
    cargo test -p akidb-storage test_load_test_short_duration -- --nocapture

elif [[ "$MODE" == "full" ]]; then
    echo "Running full load test (10 minutes, 100 QPS)..."
    echo "This will take approximately 10 minutes..."
    echo ""

    cargo test -p akidb-storage test_load_test_full_10_min -- --ignored --nocapture

else
    echo "Error: Unknown mode '$MODE'"
    echo ""
    echo "Usage: ./scripts/run-load-test.sh [short|full]"
    exit 1
fi

echo ""
echo "✅ Load test complete"
echo ""
