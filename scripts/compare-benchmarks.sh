#!/bin/bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <baseline-name> [candidate-name]"
  exit 1
fi

BASELINE="$1"
CANDIDATE="${2:-candidate}"

cargo bench --package akidb-benchmarks -- --baseline "${BASELINE}" --save-baseline "${CANDIDATE}"

echo "Comparison complete. Inspect reports under target/criterion/${CANDIDATE}/report/index.html"
