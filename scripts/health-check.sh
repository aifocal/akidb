#!/usr/bin/env bash
#
# health-check.sh - AkiDB 2.0 Health Check Script
#
# Usage:
#   ./health-check.sh           # Human-readable output
#   ./health-check.sh --json    # JSON output
#
# Exit Codes:
#   0 - Healthy (all checks passed)
#   1 - Degraded (warnings present, service operational)
#   2 - Unhealthy (critical failures)

set -euo pipefail

# Configuration
REST_HOST="${REST_HOST:-localhost}"
REST_PORT="${REST_PORT:-8080}"
GRPC_HOST="${GRPC_HOST:-localhost}"
GRPC_PORT="${GRPC_PORT:-9090}"
DB_PATH="${DB_PATH:-./akidb.db}"
DISK_WARN_GB="${DISK_WARN_GB:-1}"
MEMORY_WARN_PERCENT="${MEMORY_WARN_PERCENT:-90}"
TIMEOUT="${TIMEOUT:-5}"

# Output format
JSON_OUTPUT=false
if [[ "${1:-}" == "--json" ]]; then
    JSON_OUTPUT=true
fi

# Color codes (disabled for JSON output)
if [[ "$JSON_OUTPUT" == "false" ]]; then
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    RED='\033[0;31m'
    BLUE='\033[0;34m'
    NC='\033[0m' # No Color
else
    GREEN=''
    YELLOW=''
    RED=''
    BLUE=''
    NC=''
fi

# Health status tracking
declare -a CHECKS=()
OVERALL_STATUS="healthy"
EXIT_CODE=0

# JSON accumulator
JSON_CHECKS="[]"

# Helper: Add check result
add_check() {
    local name="$1"
    local status="$2"
    local message="$3"
    local details="${4:-}"

    CHECKS+=("$name|$status|$message|$details")

    # Update overall status
    if [[ "$status" == "unhealthy" ]]; then
        OVERALL_STATUS="unhealthy"
        EXIT_CODE=2
    elif [[ "$status" == "warning" && "$OVERALL_STATUS" == "healthy" ]]; then
        OVERALL_STATUS="degraded"
        EXIT_CODE=1
    fi

    # Build JSON
    if [[ "$JSON_OUTPUT" == "true" ]]; then
        local json_entry
        json_entry=$(cat <<EOF
{
  "name": "$name",
  "status": "$status",
  "message": "$message",
  "details": "$details"
}
EOF
)
        if [[ "$JSON_CHECKS" == "[]" ]]; then
            JSON_CHECKS="[$json_entry]"
        else
            JSON_CHECKS="${JSON_CHECKS%]}, $json_entry]"
        fi
    fi
}

# Helper: Print check result (human-readable)
print_check() {
    local name="$1"
    local status="$2"
    local message="$3"

    if [[ "$JSON_OUTPUT" == "true" ]]; then
        return
    fi

    local icon color
    case "$status" in
        healthy)
            icon="✅"
            color="$GREEN"
            ;;
        warning)
            icon="⚠️ "
            color="$YELLOW"
            ;;
        unhealthy)
            icon="❌"
            color="$RED"
            ;;
        *)
            icon="ℹ️ "
            color="$BLUE"
            ;;
    esac

    printf "${color}${icon} %-25s %s${NC}\n" "$name" "$message"
}

# Check 1: REST API Health Endpoint
check_rest_health() {
    local url="http://${REST_HOST}:${REST_PORT}/health"
    local response status_code

    if response=$(curl -s -f -m "$TIMEOUT" "$url" 2>&1); then
        status_code=$(curl -s -o /dev/null -w "%{http_code}" -m "$TIMEOUT" "$url" 2>/dev/null || echo "000")
        if [[ "$status_code" == "200" ]]; then
            add_check "REST API" "healthy" "Responding on :${REST_PORT}" "$response"
            print_check "REST API" "healthy" "Responding on :${REST_PORT}"
        else
            add_check "REST API" "unhealthy" "HTTP $status_code" "$response"
            print_check "REST API" "unhealthy" "HTTP $status_code"
        fi
    else
        add_check "REST API" "unhealthy" "Connection failed" "$response"
        print_check "REST API" "unhealthy" "Connection failed"
    fi
}

# Check 2: gRPC API Health
check_grpc_health() {
    if ! command -v grpcurl &> /dev/null; then
        add_check "gRPC API" "warning" "grpcurl not installed (skipped)" ""
        print_check "gRPC API" "warning" "grpcurl not installed (skipped)"
        return
    fi

    local target="${GRPC_HOST}:${GRPC_PORT}"
    local response

    if response=$(grpcurl -plaintext -max-time "$TIMEOUT" "$target" list 2>&1); then
        add_check "gRPC API" "healthy" "Responding on :${GRPC_PORT}" "$response"
        print_check "gRPC API" "healthy" "Responding on :${GRPC_PORT}"
    else
        add_check "gRPC API" "unhealthy" "Connection failed" "$response"
        print_check "gRPC API" "unhealthy" "Connection failed"
    fi
}

# Check 3: Database Connectivity
check_database() {
    if [[ ! -f "$DB_PATH" ]]; then
        add_check "Database" "warning" "File not found: $DB_PATH" ""
        print_check "Database" "warning" "File not found (expected for first run)"
        return
    fi

    if ! command -v sqlite3 &> /dev/null; then
        add_check "Database" "warning" "sqlite3 not installed (skipped)" ""
        print_check "Database" "warning" "sqlite3 not installed (skipped)"
        return
    fi

    local query_result
    if query_result=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM tenants;" 2>&1); then
        local db_size
        db_size=$(du -h "$DB_PATH" 2>/dev/null | cut -f1 || echo "unknown")
        add_check "Database" "healthy" "Connected ($db_size)" "Tenants: $query_result"
        print_check "Database" "healthy" "Connected ($db_size)"
    else
        add_check "Database" "unhealthy" "Query failed" "$query_result"
        print_check "Database" "unhealthy" "Query failed"
    fi
}

# Check 4: Disk Space
check_disk_space() {
    local disk_avail
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS
        disk_avail=$(df -g . 2>/dev/null | awk 'NR==2 {print $4}' || echo "0")
    else
        # Linux
        disk_avail=$(df -BG . 2>/dev/null | awk 'NR==2 {print $4}' | sed 's/G//' || echo "0")
    fi

    if [[ -z "$disk_avail" || "$disk_avail" == "0" ]]; then
        add_check "Disk Space" "warning" "Unable to check disk space" ""
        print_check "Disk Space" "warning" "Unable to check"
        return
    fi

    if (( disk_avail < DISK_WARN_GB )); then
        add_check "Disk Space" "warning" "${disk_avail}GB free (< ${DISK_WARN_GB}GB)" ""
        print_check "Disk Space" "warning" "${disk_avail}GB free (< ${DISK_WARN_GB}GB)"
    else
        add_check "Disk Space" "healthy" "${disk_avail}GB free" ""
        print_check "Disk Space" "healthy" "${disk_avail}GB free"
    fi
}

# Check 5: Memory Usage
check_memory() {
    local mem_percent
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS
        local mem_used mem_total
        mem_used=$(vm_stat 2>/dev/null | awk '/Pages active/ {print $3}' | sed 's/\.//' || echo "0")
        mem_total=$(sysctl -n hw.memsize 2>/dev/null | awk '{print $1/4096}' || echo "1")
        if [[ "$mem_total" != "0" && "$mem_total" != "1" ]]; then
            mem_percent=$(awk "BEGIN {printf \"%.0f\", ($mem_used/$mem_total)*100}")
        else
            mem_percent="0"
        fi
    else
        # Linux
        mem_percent=$(free 2>/dev/null | awk '/Mem:/ {printf "%.0f", $3/$2 * 100}' || echo "0")
    fi

    if [[ -z "$mem_percent" || "$mem_percent" == "0" ]]; then
        add_check "Memory Usage" "warning" "Unable to check memory" ""
        print_check "Memory Usage" "warning" "Unable to check"
        return
    fi

    if (( mem_percent > MEMORY_WARN_PERCENT )); then
        add_check "Memory Usage" "warning" "${mem_percent}% used (> ${MEMORY_WARN_PERCENT}%)" ""
        print_check "Memory Usage" "warning" "${mem_percent}% used (> ${MEMORY_WARN_PERCENT}%)"
    else
        add_check "Memory Usage" "healthy" "${mem_percent}% used" ""
        print_check "Memory Usage" "healthy" "${mem_percent}% used"
    fi
}

# Main execution
main() {
    if [[ "$JSON_OUTPUT" == "false" ]]; then
        echo -e "${BLUE}=== AkiDB 2.0 Health Check ===${NC}\n"
    fi

    check_rest_health
    check_grpc_health
    check_database
    check_disk_space
    check_memory

    # Output results
    if [[ "$JSON_OUTPUT" == "true" ]]; then
        cat <<EOF
{
  "status": "$OVERALL_STATUS",
  "timestamp": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
  "checks": $JSON_CHECKS
}
EOF
    else
        echo ""
        case "$OVERALL_STATUS" in
            healthy)
                echo -e "${GREEN}✅ Overall Status: HEALTHY${NC}"
                ;;
            degraded)
                echo -e "${YELLOW}⚠️  Overall Status: DEGRADED${NC}"
                ;;
            unhealthy)
                echo -e "${RED}❌ Overall Status: UNHEALTHY${NC}"
                ;;
        esac
        echo -e "\nExit Code: $EXIT_CODE"
    fi

    exit "$EXIT_CODE"
}

main "$@"
