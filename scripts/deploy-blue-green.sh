#!/bin/bash
#
# Blue-Green Deployment Script for AkiDB
#
# This script performs zero-downtime deployments by:
# 1. Deploying new version to inactive environment (blue/green)
# 2. Running smoke tests
# 3. Monitoring error rates
# 4. Switching traffic
# 5. Cleaning up old environment
#
# Usage: ./deploy-blue-green.sh <version> [namespace]
#
# Example: ./deploy-blue-green.sh v2.0.0 production

set -euo pipefail

# ==============================================================================
# Configuration
# ==============================================================================

NEW_VERSION="${1:?Usage: $0 <version> [namespace]}"
NAMESPACE="${2:-default}"
CHART_PATH="${CHART_PATH:-k8s/helm/akidb}"
SMOKE_TEST_DURATION="${SMOKE_TEST_DURATION:-300}"  # 5 minutes
OBSERVATION_WINDOW="${OBSERVATION_WINDOW:-300}"    # 5 minutes
ERROR_THRESHOLD="${ERROR_THRESHOLD:-0.01}"         # 1% error rate
TIMEOUT="${TIMEOUT:-600}"                          # 10 minutes

# ==============================================================================
# Colors and Logging
# ==============================================================================

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $(date '+%Y-%m-%d %H:%M:%S') - $*"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $(date '+%Y-%m-%d %H:%M:%S') - $*"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $(date '+%Y-%m-%d %H:%M:%S') - $*"
}

log_step() {
    echo -e "${BLUE}[STEP]${NC} $(date '+%Y-%m-%d %H:%M:%S') - $*"
}

# ==============================================================================
# Cleanup on Exit
# ==============================================================================

cleanup() {
    local exit_code=$?
    if [[ $exit_code -ne 0 ]]; then
        log_error "Deployment failed with exit code $exit_code"
        log_error "Cleaning up..."
        # Rollback if needed
        if [[ -n "${NEW_ENV:-}" ]]; then
            log_warn "Cleaning up failed deployment: akidb-$NEW_ENV"
            helm uninstall "akidb-$NEW_ENV" -n "$NAMESPACE" 2>/dev/null || true
        fi
    fi
}

trap cleanup EXIT

# ==============================================================================
# Prerequisites Check
# ==============================================================================

check_prerequisites() {
    log_step "Checking prerequisites..."

    # Check kubectl
    if ! command -v kubectl &> /dev/null; then
        log_error "kubectl not found. Please install kubectl."
        exit 1
    fi

    # Check helm
    if ! command -v helm &> /dev/null; then
        log_error "helm not found. Please install helm."
        exit 1
    fi

    # Check cluster connectivity
    if ! kubectl cluster-info &> /dev/null; then
        log_error "Cannot connect to Kubernetes cluster."
        exit 1
    fi

    # Check namespace exists
    if ! kubectl get namespace "$NAMESPACE" &> /dev/null; then
        log_error "Namespace '$NAMESPACE' does not exist."
        exit 1
    fi

    # Check chart exists
    if [[ ! -f "$CHART_PATH/Chart.yaml" ]]; then
        log_error "Helm chart not found at $CHART_PATH"
        exit 1
    fi

    log_info "Prerequisites check passed ✓"
}

# ==============================================================================
# Determine Current and New Environments
# ==============================================================================

determine_environments() {
    log_step "Determining current environment..."

    # Try to get current environment from service selector
    CURRENT_ENV=$(kubectl get svc akidb -n "$NAMESPACE" \
        -o jsonpath='{.spec.selector.environment}' 2>/dev/null || echo "")

    if [[ -z "$CURRENT_ENV" ]]; then
        # No existing deployment, default to blue
        CURRENT_ENV="none"
        NEW_ENV="blue"
        log_info "No existing deployment found. Deploying to: $NEW_ENV"
    elif [[ "$CURRENT_ENV" == "blue" ]]; then
        NEW_ENV="green"
        log_info "Current environment: $CURRENT_ENV → New environment: $NEW_ENV"
    else
        NEW_ENV="blue"
        log_info "Current environment: $CURRENT_ENV → New environment: $NEW_ENV"
    fi
}

# ==============================================================================
# Deploy New Environment
# ==============================================================================

deploy_new_environment() {
    log_step "Deploying version $NEW_VERSION to $NEW_ENV environment..."

    # Create values override for environment
    cat > "/tmp/akidb-$NEW_ENV-values.yaml" <<EOF
environment: $NEW_ENV

podLabels:
  environment: $NEW_ENV

service:
  selector:
    environment: $NEW_ENV

image:
  tag: $NEW_VERSION
EOF

    # Deploy with Helm
    if ! helm upgrade --install "akidb-$NEW_ENV" "$CHART_PATH" \
        --namespace "$NAMESPACE" \
        --values "/tmp/akidb-$NEW_ENV-values.yaml" \
        --wait \
        --timeout="${TIMEOUT}s"; then
        log_error "Helm deployment failed"
        exit 1
    fi

    log_info "Deployment successful ✓"
}

# ==============================================================================
# Wait for Pods Ready
# ==============================================================================

wait_for_pods() {
    log_step "Waiting for pods to be ready..."

    if ! kubectl wait --for=condition=ready pod \
        -l "app=akidb,environment=$NEW_ENV" \
        -n "$NAMESPACE" \
        --timeout="${TIMEOUT}s"; then
        log_error "Pods failed to become ready"
        exit 1
    fi

    # Get pod count
    POD_COUNT=$(kubectl get pods -l "app=akidb,environment=$NEW_ENV" \
        -n "$NAMESPACE" -o jsonpath='{.items}' | jq '. | length')

    log_info "All $POD_COUNT pods are ready ✓"
}

# ==============================================================================
# Run Smoke Tests
# ==============================================================================

run_smoke_tests() {
    log_step "Running smoke tests..."

    # Get pod IP for direct testing
    POD_NAME=$(kubectl get pod -l "app=akidb,environment=$NEW_ENV" \
        -n "$NAMESPACE" -o jsonpath='{.items[0].metadata.name}')

    log_info "Testing pod: $POD_NAME"

    # Test 1: Health check
    log_info "Test 1: Health check"
    if ! kubectl exec -n "$NAMESPACE" "$POD_NAME" -- \
        curl -sf http://localhost:8080/health > /dev/null; then
        log_error "Health check failed"
        return 1
    fi
    log_info "Health check passed ✓"

    # Test 2: Metrics endpoint
    log_info "Test 2: Metrics endpoint"
    if ! kubectl exec -n "$NAMESPACE" "$POD_NAME" -- \
        curl -sf http://localhost:8080/metrics | grep -q "akidb"; then
        log_error "Metrics endpoint failed"
        return 1
    fi
    log_info "Metrics endpoint passed ✓"

    # Test 3: Create test collection
    log_info "Test 3: Create collection"
    COLLECTION_RESPONSE=$(kubectl exec -n "$NAMESPACE" "$POD_NAME" -- \
        curl -sf -X POST http://localhost:8080/api/v1/collections \
        -H "Content-Type: application/json" \
        -d '{"name":"smoke-test","dimension":128,"metric":"cosine"}' || echo "")

    if [[ -z "$COLLECTION_RESPONSE" ]]; then
        log_error "Failed to create collection"
        return 1
    fi

    COLLECTION_ID=$(echo "$COLLECTION_RESPONSE" | jq -r '.collection_id // empty')
    if [[ -z "$COLLECTION_ID" ]]; then
        log_error "Collection ID not returned"
        return 1
    fi
    log_info "Collection created: $COLLECTION_ID ✓"

    # Test 4: Insert vectors
    log_info "Test 4: Insert vectors"
    for i in {1..10}; do
        kubectl exec -n "$NAMESPACE" "$POD_NAME" -- \
            curl -sf -X POST "http://localhost:8080/api/v1/collections/$COLLECTION_ID/vectors" \
            -H "Content-Type: application/json" \
            -d "{\"id\":\"vec-$i\",\"vector\":$(seq 128 | jq -s 'map(. / 128)')}" \
            > /dev/null || {
                log_error "Failed to insert vector $i"
                return 1
            }
    done
    log_info "Inserted 10 vectors ✓"

    # Test 5: Search vectors
    log_info "Test 5: Search vectors"
    SEARCH_RESULT=$(kubectl exec -n "$NAMESPACE" "$POD_NAME" -- \
        curl -sf -X POST "http://localhost:8080/api/v1/collections/$COLLECTION_ID/search" \
        -H "Content-Type: application/json" \
        -d "{\"vector\":$(seq 128 | jq -s 'map(. / 128)'),\"k\":5}" || echo "")

    if [[ -z "$SEARCH_RESULT" ]]; then
        log_error "Search failed"
        return 1
    fi

    RESULT_COUNT=$(echo "$SEARCH_RESULT" | jq '.results | length')
    if [[ "$RESULT_COUNT" -lt 5 ]]; then
        log_error "Search returned only $RESULT_COUNT results (expected 5+)"
        return 1
    fi
    log_info "Search returned $RESULT_COUNT results ✓"

    # Test 6: Delete collection
    log_info "Test 6: Delete collection"
    if ! kubectl exec -n "$NAMESPACE" "$POD_NAME" -- \
        curl -sf -X DELETE "http://localhost:8080/api/v1/collections/$COLLECTION_ID" \
        > /dev/null; then
        log_warn "Failed to delete test collection (non-critical)"
    else
        log_info "Collection deleted ✓"
    fi

    log_info "All smoke tests passed ✓"
    return 0
}

# ==============================================================================
# Monitor Error Rate
# ==============================================================================

monitor_error_rate() {
    log_step "Monitoring error rate for ${SMOKE_TEST_DURATION}s..."

    local start_time=$(date +%s)
    local end_time=$((start_time + SMOKE_TEST_DURATION))
    local check_interval=30

    while [[ $(date +%s) -lt $end_time ]]; do
        # Check if Prometheus is available
        if kubectl get pod -l "app=prometheus" -n "$NAMESPACE" &> /dev/null; then
            # Query Prometheus for error rate
            local prom_pod=$(kubectl get pod -l "app=prometheus" -n "$NAMESPACE" \
                -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || echo "")

            if [[ -n "$prom_pod" ]]; then
                local error_rate=$(kubectl exec -n "$NAMESPACE" "$prom_pod" -- \
                    wget -qO- "http://localhost:9090/api/v1/query?query=rate(http_requests_total{status=~\"5..\",environment=\"$NEW_ENV\"}[5m])/rate(http_requests_total{environment=\"$NEW_ENV\"}[5m])" \
                    2>/dev/null | jq -r '.data.result[0].value[1] // "0"' || echo "0")

                log_info "Current error rate: $(printf "%.4f" "$error_rate") (threshold: $ERROR_THRESHOLD)"

                # Check if error rate exceeds threshold
                if (( $(echo "$error_rate > $ERROR_THRESHOLD" | bc -l 2>/dev/null || echo 0) )); then
                    log_error "Error rate $error_rate exceeds threshold $ERROR_THRESHOLD"
                    return 1
                fi
            fi
        else
            log_warn "Prometheus not available, skipping error rate monitoring"
        fi

        # Calculate remaining time
        local remaining=$((end_time - $(date +%s)))
        if [[ $remaining -gt 0 ]]; then
            log_info "Monitoring continues... ${remaining}s remaining"
            sleep "$check_interval"
        fi
    done

    log_info "Error rate monitoring completed ✓"
    return 0
}

# ==============================================================================
# Switch Traffic
# ==============================================================================

switch_traffic() {
    log_step "Switching traffic from $CURRENT_ENV to $NEW_ENV..."

    if [[ "$CURRENT_ENV" == "none" ]]; then
        # First deployment, create service
        log_info "Creating service for $NEW_ENV"
        kubectl patch svc akidb -n "$NAMESPACE" \
            -p "{\"spec\":{\"selector\":{\"environment\":\"$NEW_ENV\"}}}" 2>/dev/null || \
            log_warn "Service might not exist yet, will be created by Helm"
    else
        # Update existing service selector
        kubectl patch svc akidb -n "$NAMESPACE" \
            -p "{\"spec\":{\"selector\":{\"environment\":\"$NEW_ENV\"}}}"
    fi

    log_info "Traffic switched to $NEW_ENV ✓"
}

# ==============================================================================
# Final Observation
# ==============================================================================

final_observation() {
    log_step "Observing new environment for ${OBSERVATION_WINDOW}s..."

    sleep "$OBSERVATION_WINDOW"

    # Check pod status
    if ! kubectl get pods -l "app=akidb,environment=$NEW_ENV" -n "$NAMESPACE" \
        -o jsonpath='{.items[*].status.phase}' | grep -q "Running"; then
        log_error "Pods are not running after observation period"
        return 1
    fi

    log_info "Observation completed ✓"
    return 0
}

# ==============================================================================
# Cleanup Old Environment
# ==============================================================================

cleanup_old_environment() {
    if [[ "$CURRENT_ENV" == "none" ]]; then
        log_info "No old environment to clean up"
        return 0
    fi

    log_step "Cleaning up old environment: $CURRENT_ENV"

    if helm uninstall "akidb-$CURRENT_ENV" -n "$NAMESPACE"; then
        log_info "Old environment cleaned up ✓"
    else
        log_warn "Failed to clean up old environment (may not exist)"
    fi
}

# ==============================================================================
# Main
# ==============================================================================

main() {
    log_info "==================================================================="
    log_info "  AkiDB Blue-Green Deployment"
    log_info "==================================================================="
    log_info "Version: $NEW_VERSION"
    log_info "Namespace: $NAMESPACE"
    log_info "Chart Path: $CHART_PATH"
    log_info "==================================================================="
    echo

    check_prerequisites
    determine_environments
    deploy_new_environment
    wait_for_pods

    if ! run_smoke_tests; then
        log_error "Smoke tests failed, aborting deployment"
        exit 1
    fi

    if ! monitor_error_rate; then
        log_error "Error rate monitoring failed, aborting deployment"
        exit 1
    fi

    switch_traffic

    if ! final_observation; then
        log_error "Final observation failed, consider rollback"
        exit 1
    fi

    cleanup_old_environment

    echo
    log_info "==================================================================="
    log_info "  Blue-Green Deployment Completed Successfully! 🎉"
    log_info "==================================================================="
    log_info "Version: $NEW_VERSION"
    log_info "Environment: $NEW_ENV"
    log_info "Status: ACTIVE"
    log_info "==================================================================="
}

main "$@"
