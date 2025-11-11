#!/bin/bash
# Run all quick load test scenarios and collect results
# Total duration: ~75 minutes

set -e

echo "========================================================================"
echo "AkiDB 2.0 Load Test Suite - All Quick Scenarios"
echo "========================================================================"
echo ""
echo "Total estimated time: 75 minutes"
echo "Start time: $(date)"
echo ""

# Create reports directory
mkdir -p target/load_test_reports

# Track results
PASSED=0
FAILED=0
TOTAL=8

echo "========================================================================"
echo "Scenario 1: Baseline Performance (5 minutes)"
echo "========================================================================"
if cargo test --release -p akidb-storage scenario_1_baseline_quick -- --nocapture; then
    echo "✅ Scenario 1: PASSED"
    PASSED=$((PASSED + 1))
else
    echo "❌ Scenario 1: FAILED"
    FAILED=$((FAILED + 1))
fi
echo ""

echo "========================================================================"
echo "Scenario 2: Sustained High Load (10 minutes)"
echo "========================================================================"
if cargo test --release -p akidb-storage scenario_2_sustained_load_quick -- --nocapture; then
    echo "✅ Scenario 2: PASSED"
    PASSED=$((PASSED + 1))
else
    echo "❌ Scenario 2: FAILED"
    FAILED=$((FAILED + 1))
fi
echo ""

echo "========================================================================"
echo "Scenario 3: Spike Load (5 minutes)"
echo "========================================================================"
if cargo test --release -p akidb-storage scenario_3_spike_load_quick -- --nocapture; then
    echo "✅ Scenario 3: PASSED"
    PASSED=$((PASSED + 1))
else
    echo "❌ Scenario 3: FAILED"
    FAILED=$((FAILED + 1))
fi
echo ""

echo "========================================================================"
echo "Scenario 4: Tiered Storage (10 minutes)"
echo "========================================================================"
if cargo test --release -p akidb-storage scenario_4_tiered_storage_quick -- --nocapture; then
    echo "✅ Scenario 4: PASSED"
    PASSED=$((PASSED + 1))
else
    echo "❌ Scenario 4: FAILED"
    FAILED=$((FAILED + 1))
fi
echo ""

echo "========================================================================"
echo "Scenario 5: Multi-Tenant Load (10 minutes)"
echo "========================================================================"
if cargo test --release -p akidb-storage scenario_5_multi_tenant_quick -- --nocapture; then
    echo "✅ Scenario 5: PASSED"
    PASSED=$((PASSED + 1))
else
    echo "❌ Scenario 5: FAILED"
    FAILED=$((FAILED + 1))
fi
echo ""

echo "========================================================================"
echo "Scenario 6: Large Dataset (15 minutes)"
echo "========================================================================"
if cargo test --release -p akidb-storage scenario_6_large_dataset_quick -- --nocapture; then
    echo "✅ Scenario 6: PASSED"
    PASSED=$((PASSED + 1))
else
    echo "❌ Scenario 6: FAILED"
    FAILED=$((FAILED + 1))
fi
echo ""

echo "========================================================================"
echo "Scenario 7: Failure Injection (5 minutes)"
echo "========================================================================"
if cargo test --release -p akidb-storage scenario_7_failure_injection_quick -- --nocapture; then
    echo "✅ Scenario 7: PASSED"
    PASSED=$((PASSED + 1))
else
    echo "❌ Scenario 7: FAILED"
    FAILED=$((FAILED + 1))
fi
echo ""

echo "========================================================================"
echo "Scenario 8: Mixed Workload Chaos (10 minutes)"
echo "========================================================================"
if cargo test --release -p akidb-storage scenario_8_mixed_chaos_quick -- --nocapture; then
    echo "✅ Scenario 8: PASSED"
    PASSED=$((PASSED + 1))
else
    echo "❌ Scenario 8: FAILED"
    FAILED=$((FAILED + 1))
fi
echo ""

# Summary
echo "========================================================================"
echo "LOAD TEST SUITE SUMMARY"
echo "========================================================================"
echo ""
echo "End time: $(date)"
echo ""
echo "Results: $PASSED passed, $FAILED failed (out of $TOTAL total)"
echo ""

if [ $FAILED -eq 0 ]; then
    echo "✅ ALL TESTS PASSED!"
    echo ""
    echo "Reports generated in: target/load_test_reports/"
    ls -lh target/load_test_reports/
    exit 0
else
    echo "❌ $FAILED TEST(S) FAILED"
    echo ""
    echo "See reports in: target/load_test_reports/"
    exit 1
fi
