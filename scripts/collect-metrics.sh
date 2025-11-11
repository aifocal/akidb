#!/usr/bin/env bash
set -euo pipefail

# collect-metrics.sh - Production-ready Prometheus metrics collector for AkiDB
# Usage: ./collect-metrics.sh [--csv] [--archive-days N]

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
METRICS_DIR="${PROJECT_ROOT}/metrics"
ARCHIVE_DIR="${METRICS_DIR}/archive"
METRICS_ENDPOINT="${METRICS_ENDPOINT:-http://localhost:8080/metrics}"
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
OUTPUT_FILE="${METRICS_DIR}/${TIMESTAMP}.txt"

# Configurable parameters
CSV_MODE=false
ARCHIVE_DAYS=7

# Parse command-line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --csv)
            CSV_MODE=true
            shift
            ;;
        --archive-days)
            ARCHIVE_DAYS="$2"
            shift 2
            ;;
        --help)
            echo "Usage: $0 [--csv] [--archive-days N]"
            echo ""
            echo "Options:"
            echo "  --csv              Enable CSV output mode"
            echo "  --archive-days N   Archive metrics older than N days (default: 7)"
            echo ""
            echo "Environment Variables:"
            echo "  METRICS_ENDPOINT   Prometheus endpoint (default: http://localhost:8080/metrics)"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Ensure metrics directories exist
mkdir -p "$METRICS_DIR"
mkdir -p "$ARCHIVE_DIR"

# Fetch metrics with timeout and error handling
fetch_metrics() {
    local response
    local http_code

    response=$(curl -s -w "\n%{http_code}" --connect-timeout 5 --max-time 10 "$METRICS_ENDPOINT" 2>/dev/null || echo -e "\n000")
    http_code=$(echo "$response" | tail -n1)

    if [[ "$http_code" != "200" ]]; then
        echo "ERROR: Failed to fetch metrics from $METRICS_ENDPOINT (HTTP $http_code)" >&2
        return 1
    fi

    echo "$response" | head -n -1
}

# Extract metric value (handles both gauge and histogram formats)
extract_metric() {
    local metrics_data="$1"
    local metric_name="$2"
    local label_filter="${3:-}"

    if [[ -n "$label_filter" ]]; then
        echo "$metrics_data" | grep "^${metric_name}{.*${label_filter}.*}" | tail -n1 | awk '{print $2}' || echo "0"
    else
        echo "$metrics_data" | grep "^${metric_name} " | awk '{print $2}' || echo "0"
    fi
}

# Parse Prometheus histogram quantiles
extract_histogram_quantile() {
    local metrics_data="$1"
    local metric_name="$2"
    local quantile="$3"

    echo "$metrics_data" | grep "^${metric_name}{.*quantile=\"${quantile}\".*}" | tail -n1 | awk '{print $2}' || echo "0"
}

# Main execution
main() {
    echo "=== AkiDB Metrics Collection ===" | tee "$OUTPUT_FILE"
    echo "Timestamp: $TIMESTAMP" | tee -a "$OUTPUT_FILE"
    echo "Endpoint: $METRICS_ENDPOINT" | tee -a "$OUTPUT_FILE"
    echo "" | tee -a "$OUTPUT_FILE"

    # Fetch raw metrics
    METRICS_DATA=$(fetch_metrics)
    if [[ $? -ne 0 ]]; then
        echo "FAILED" | tee -a "$OUTPUT_FILE"
        exit 1
    fi

    # Extract key metrics
    COLLECTIONS_TOTAL=$(extract_metric "$METRICS_DATA" "akidb_collections_total")
    VECTORS_INSERTED=$(extract_metric "$METRICS_DATA" "akidb_vectors_inserted_total")
    SEARCHES_PERFORMED=$(extract_metric "$METRICS_DATA" "akidb_searches_performed_total")

    # Extract search latency quantiles (in seconds, convert to ms)
    SEARCH_P50=$(extract_histogram_quantile "$METRICS_DATA" "akidb_search_latency_seconds" "0.5")
    SEARCH_P95=$(extract_histogram_quantile "$METRICS_DATA" "akidb_search_latency_seconds" "0.95")
    SEARCH_P99=$(extract_histogram_quantile "$METRICS_DATA" "akidb_search_latency_seconds" "0.99")

    # Extract insert latency quantiles
    INSERT_P50=$(extract_histogram_quantile "$METRICS_DATA" "akidb_insert_latency_seconds" "0.5")
    INSERT_P95=$(extract_histogram_quantile "$METRICS_DATA" "akidb_insert_latency_seconds" "0.95")
    INSERT_P99=$(extract_histogram_quantile "$METRICS_DATA" "akidb_insert_latency_seconds" "0.99")

    # Convert seconds to milliseconds
    SEARCH_P50_MS=$(echo "$SEARCH_P50 * 1000" | bc -l 2>/dev/null | xargs printf "%.2f" || echo "0.00")
    SEARCH_P95_MS=$(echo "$SEARCH_P95 * 1000" | bc -l 2>/dev/null | xargs printf "%.2f" || echo "0.00")
    SEARCH_P99_MS=$(echo "$SEARCH_P99 * 1000" | bc -l 2>/dev/null | xargs printf "%.2f" || echo "0.00")
    INSERT_P50_MS=$(echo "$INSERT_P50 * 1000" | bc -l 2>/dev/null | xargs printf "%.2f" || echo "0.00")
    INSERT_P95_MS=$(echo "$INSERT_P95 * 1000" | bc -l 2>/dev/null | xargs printf "%.2f" || echo "0.00")
    INSERT_P99_MS=$(echo "$INSERT_P99 * 1000" | bc -l 2>/dev/null | xargs printf "%.2f" || echo "0.00")

    # Human-readable summary
    {
        echo "--- Collection Metrics ---"
        echo "Total Collections: $COLLECTIONS_TOTAL"
        echo ""
        echo "--- Vector Operations ---"
        echo "Vectors Inserted: $VECTORS_INSERTED"
        echo "Searches Performed: $SEARCHES_PERFORMED"
        echo ""
        echo "--- Search Latency (ms) ---"
        echo "P50: $SEARCH_P50_MS"
        echo "P95: $SEARCH_P95_MS"
        echo "P99: $SEARCH_P99_MS"
        echo ""
        echo "--- Insert Latency (ms) ---"
        echo "P50: $INSERT_P50_MS"
        echo "P95: $INSERT_P95_MS"
        echo "P99: $INSERT_P99_MS"
        echo ""
    } | tee -a "$OUTPUT_FILE"

    # CSV mode
    if [[ "$CSV_MODE" == true ]]; then
        CSV_FILE="${METRICS_DIR}/metrics.csv"

        # Create CSV header if file doesn't exist
        if [[ ! -f "$CSV_FILE" ]]; then
            echo "timestamp,collections_total,vectors_inserted,searches_performed,search_p50_ms,search_p95_ms,search_p99_ms,insert_p50_ms,insert_p95_ms,insert_p99_ms" > "$CSV_FILE"
        fi

        # Append data row
        echo "$TIMESTAMP,$COLLECTIONS_TOTAL,$VECTORS_INSERTED,$SEARCHES_PERFORMED,$SEARCH_P50_MS,$SEARCH_P95_MS,$SEARCH_P99_MS,$INSERT_P50_MS,$INSERT_P95_MS,$INSERT_P99_MS" >> "$CSV_FILE"

        echo "CSV updated: $CSV_FILE" | tee -a "$OUTPUT_FILE"
    fi

    echo "Metrics saved: $OUTPUT_FILE"

    # Archive old metrics
    archive_old_metrics
}

# Archive metrics older than N days
archive_old_metrics() {
    local cutoff_date
    cutoff_date=$(date -u -v"-${ARCHIVE_DAYS}d" +"%Y-%m-%d" 2>/dev/null || date -u -d "${ARCHIVE_DAYS} days ago" +"%Y-%m-%d" 2>/dev/null || echo "")

    if [[ -z "$cutoff_date" ]]; then
        return 0
    fi

    local archived_count=0

    while IFS= read -r -d '' file 2>/dev/null; do
        local filename
        filename=$(basename "$file")
        local file_date
        file_date="${filename:0:10}"

        if [[ "$file_date" < "$cutoff_date" ]]; then
            mv "$file" "$ARCHIVE_DIR/"
            ((archived_count++))
        fi
    done < <(find "$METRICS_DIR" -maxdepth 1 -name "*.txt" -type f -print0 2>/dev/null)

    if [[ $archived_count -gt 0 ]]; then
        echo "Archived $archived_count old metric files (older than $ARCHIVE_DAYS days)"
    fi
}

# Execute main function
main
