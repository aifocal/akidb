#!/bin/bash
#
# Cleanup Script: Archive Completed PRD and Temporary Files
#
# This script moves completed planning documents to archive directories
# while keeping the main PRD (PHASE-10-PRODUCTION-READY-V2-PRD.md) and
# architectural decision records (ADRs).

set -euo pipefail

cd /Users/akiralam/code/akidb2

echo "🧹 Starting cleanup of completed PRD and tmp files..."

# Create archive directories
mkdir -p automatosx/archive/prd-archive
mkdir -p automatosx/archive/tmp-archive

# ============================================================================
# Archive PRD Files
# ============================================================================

echo ""
echo "📦 Archiving completed PRD files..."

# Move completed phase implementation plans and reports
for file in \
    "PHASE-0-FINAL-REPORT.md" \
    "PHASE-1-IMPLEMENTATION-PLAN.md" \
    "PHASE-1-M1-COMPLETION-REPORT.md" \
    "PHASE-2-COMPLETION-REPORT.md" \
    "PHASE-2-DESIGN.md" \
    "PHASE-3-COMPLETION-REPORT.md" \
    "PHASE-3-DESIGN.md" \
    "PHASE-4-COMPLETION-REPORT.md" \
    "PHASE-10-WEEK-1-PARQUET-SNAPSHOTTER-PRD.md" \
    "PHASE-10-WEEK-2-TIERING-POLICIES-PRD.md" \
    "PHASE-10-WEEK-3-INTEGRATION-RC2-PRD.md" \
    "PHASE-10-WEEK-4-PERFORMANCE-PRD.md" \
    "PHASE-10-WEEK-5-OBSERVABILITY-PRD.md" \
    "MLX-EMBEDDING-INTEGRATION-PRD.md"
do
    if [[ -f "automatosx/PRD/$file" ]]; then
        mv "automatosx/PRD/$file" automatosx/archive/prd-archive/
        echo "  ✓ Archived: $file"
    fi
done

# ============================================================================
# Archive Temporary Files
# ============================================================================

echo ""
echo "📦 Archiving completed tmp files..."

# Move all Phase 10 planning/completion documents
for file in \
    "PHASE-10-ACTION-PLAN.md" \
    "PHASE-10-WEEK-1-COMPREHENSIVE-MEGATHINK.md" \
    "PHASE-10-WEEK-1-DAILY-ACTION-PLAN.md" \
    "PHASE-10-WEEK-2-COMPREHENSIVE-MEGATHINK.md" \
    "PHASE-10-WEEK-2-DAILY-ACTION-PLAN.md" \
    "PHASE-10-WEEK-2-IMPLEMENTATION-PROGRESS.md" \
    "PHASE-10-WEEK-3-COMPREHENSIVE-MEGATHINK.md" \
    "PHASE-10-WEEK-3-DAILY-ACTION-PLAN.md" \
    "PHASE-10-WEEK-3-RC2-COMPLETION-REPORT.md" \
    "PHASE-10-WEEK-4-COMPREHENSIVE-MEGATHINK.md" \
    "PHASE-10-WEEK-4-DAILY-ACTION-PLAN.md" \
    "PHASE-10-WEEK-5-COMPREHENSIVE-MEGATHINK.md" \
    "PHASE-10-WEEK-5-OBSERVABILITY-PARTIAL-COMPLETION.md" \
    "PHASE-10-WEEK-6-COMPREHENSIVE-MEGATHINK.md" \
    "PHASE-10-WEEK-6-DAILY-ACTION-PLAN.md" \
    "PHASE-10-WEEK-6-COMPLETION-REPORT.md" \
    "phase-10-week-1-implementation-complete.md" \
    "phase-10-week-2-progress-update.md" \
    "phase-10-week-4-completion-report.md" \
    "phase-10-week-5-observability-complete.md" \
    "gemma-vs-qwen-embedding-comparison.md" \
    "multimodal-image-embedding-analysis.md" \
    "qwen3-embedding-evaluation.md"
do
    if [[ -f "automatosx/tmp/$file" ]]; then
        mv "automatosx/tmp/$file" automatosx/archive/tmp-archive/
        echo "  ✓ Archived: $file"
    fi
done

# ============================================================================
# Keep Important Files
# ============================================================================

echo ""
echo "📌 Keeping active files in automatosx/PRD/:"
ls -1 automatosx/PRD/ | while read file; do
    echo "  → $file"
done

echo ""
echo "📌 Keeping active files in automatosx/tmp/:"
ls -1 automatosx/tmp/ | while read file; do
    echo "  → $file"
done

# ============================================================================
# Summary
# ============================================================================

echo ""
echo "✅ Cleanup complete!"
echo ""
echo "📊 Summary:"
echo "  Archived PRD files: $(ls automatosx/archive/prd-archive/ | wc -l | tr -d ' ')"
echo "  Archived tmp files: $(ls automatosx/archive/tmp-archive/ | wc -l | tr -d ' ')"
echo "  Active PRD files: $(ls automatosx/PRD/ | wc -l | tr -d ' ')"
echo "  Active tmp files: $(ls automatosx/tmp/ | wc -l | tr -d ' ')"
echo ""
echo "🗂️  Archive locations:"
echo "  - automatosx/archive/prd-archive/"
echo "  - automatosx/archive/tmp-archive/"
echo ""
