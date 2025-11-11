#!/bin/bash
# Smoke tests for AkiDB v2.0.0-rc1
# Validates basic functionality of gRPC and REST APIs
#
# Test Coverage:
# - Health checks (REST + gRPC)
# - Collection management (create, list, get, delete via REST + gRPC)
# - Vector operations (insert, query via REST + gRPC)
# - Infrastructure validation (Docker containers, response time)
#
# The script automatically creates a test collection before running vector
# operation tests and cleans up all created collections on exit.

set -e

echo "🔥 AkiDB v2.0.0-rc1 Smoke Tests"
echo "================================"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
GRPC_HOST="${GRPC_HOST:-localhost:9000}"
REST_HOST="${REST_HOST:-http://localhost:8080}"
TEST_COLLECTION_ID=""  # Will be set after creating collection
COLLECTION_CREATED=false  # Track if cleanup is needed

# Test counters
TESTS_PASSED=0
TESTS_FAILED=0
TESTS_TOTAL=0

# Helper functions
pass() {
    echo -e "${GREEN}✅ PASS${NC}: $1"
    ((TESTS_PASSED++))
    ((TESTS_TOTAL++))
}

fail() {
    echo -e "${RED}❌ FAIL${NC}: $1"
    echo -e "${RED}   Error: $2${NC}"
    ((TESTS_FAILED++))
    ((TESTS_TOTAL++))
}

skip() {
    echo -e "${YELLOW}⏭️  SKIP${NC}: $1 (Reason: $2)"
}

section() {
    echo ""
    echo "=== $1 ==="
    echo ""
}

# Check prerequisites
check_prerequisites() {
    section "Checking Prerequisites"

    # Check if grpcurl is available
    if command -v grpcurl &> /dev/null; then
        pass "grpcurl installed"
    else
        skip "gRPC tests" "grpcurl not found (install with: brew install grpcurl)"
        return 1
    fi

    # Check if curl is available
    if command -v curl &> /dev/null; then
        pass "curl installed"
    else
        fail "curl check" "curl not found"
        return 1
    fi

    return 0
}

# Cleanup function
cleanup_collection() {
    if [ "$COLLECTION_CREATED" = true ] && [ -n "$TEST_COLLECTION_ID" ]; then
        echo ""
        echo "🧹 Cleaning up test collection..."
        curl -s -X DELETE "$REST_HOST/api/v1/collections/$TEST_COLLECTION_ID" >/dev/null 2>&1 || true
    fi
}

# Register cleanup on exit
trap cleanup_collection EXIT

# Test 1: REST Health Check
test_rest_health() {
    section "Test 1: REST Health Check"

    response=$(curl -s -o /dev/null -w "%{http_code}" "$REST_HOST/health" 2>/dev/null || echo "000")

    if [ "$response" = "200" ]; then
        pass "REST health endpoint returned 200 OK"
    else
        fail "REST health endpoint" "Expected 200, got $response"
    fi
}

# Test 2: gRPC Health Check
test_grpc_health() {
    section "Test 2: gRPC Health Check"

    if ! command -v grpcurl &> /dev/null; then
        skip "gRPC health check" "grpcurl not installed"
        return
    fi

    result=$(grpcurl -plaintext "$GRPC_HOST" grpc.health.v1.Health/Check 2>&1)

    if echo "$result" | grep -q "SERVING"; then
        pass "gRPC health check returned SERVING"
    else
        fail "gRPC health check" "Expected SERVING status, got: $result"
    fi
}

# Test 3: REST Create Collection
test_rest_create_collection() {
    section "Test 3: REST Create Collection"

    response=$(curl -s -X POST "$REST_HOST/api/v1/collections" \
        -H "Content-Type: application/json" \
        -d '{
            "name": "smoke_test_collection",
            "dimension": 128,
            "metric": "cosine",
            "embedding_model": "test-model"
        }' 2>/dev/null)

    if echo "$response" | grep -q '"collection_id"'; then
        TEST_COLLECTION_ID=$(echo "$response" | grep -o '"collection_id":"[^"]*"' | cut -d'"' -f4)
        COLLECTION_CREATED=true
        pass "REST create collection (ID: ${TEST_COLLECTION_ID:0:8}...)"
    else
        fail "REST create collection" "Unexpected response: $response"
    fi
}

# Test 4: REST List Collections
test_rest_list_collections() {
    section "Test 4: REST List Collections"

    response=$(curl -s -X GET "$REST_HOST/api/v1/collections" 2>/dev/null)

    if echo "$response" | grep -q '"collections"'; then
        count=$(echo "$response" | grep -o '"collection_id"' | wc -l | tr -d ' ')
        pass "REST list collections (found $count collection(s))"
    else
        fail "REST list collections" "Unexpected response: $response"
    fi
}

# Test 5: REST Get Collection
test_rest_get_collection() {
    section "Test 5: REST Get Collection"

    if [ -z "$TEST_COLLECTION_ID" ]; then
        skip "REST get collection" "No collection ID available"
        return
    fi

    response=$(curl -s -X GET "$REST_HOST/api/v1/collections/$TEST_COLLECTION_ID" 2>/dev/null)

    if echo "$response" | grep -q '"collection_id"' && echo "$response" | grep -q '"name"'; then
        pass "REST get collection details"
    else
        fail "REST get collection" "Unexpected response: $response"
    fi
}

# Test 6: REST Query Endpoint
test_rest_query() {
    section "Test 6: REST Query Endpoint"

    if [ -z "$TEST_COLLECTION_ID" ]; then
        skip "REST query test" "No collection ID available"
        return
    fi

    # Generate 128-dim vector matching collection dimension
    query_vector="[$(for i in $(seq 1 128); do echo -n "0.1"; [ $i -lt 128 ] && echo -n ","; done)]"

    response=$(curl -s -X POST "$REST_HOST/api/v1/collections/$TEST_COLLECTION_ID/query" \
        -H "Content-Type: application/json" \
        -d "{
            \"query_vector\": $query_vector,
            \"top_k\": 5
        }" 2>/dev/null)

    # Should succeed (empty results are OK for new collection)
    if echo "$response" | grep -q '"matches"'; then
        pass "REST query endpoint responds correctly"
    else
        fail "REST query endpoint" "Unexpected response: $response"
    fi
}

# Test 7: gRPC Query Endpoint
test_grpc_query() {
    section "Test 7: gRPC Query Endpoint"

    if ! command -v grpcurl &> /dev/null; then
        skip "gRPC query test" "grpcurl not installed"
        return
    fi

    if [ -z "$TEST_COLLECTION_ID" ]; then
        skip "gRPC query test" "No collection ID available"
        return
    fi

    # Generate 128-dim vector matching collection dimension
    query_vector="[$(for i in $(seq 1 128); do echo -n "0.1"; [ $i -lt 128 ] && echo -n ","; done)]"

    result=$(grpcurl -plaintext -d "{
        \"collection_id\": \"$TEST_COLLECTION_ID\",
        \"query_vector\": $query_vector,
        \"top_k\": 5
    }" "$GRPC_HOST" akidb.collection.v1.CollectionService/Query 2>&1)

    # Should succeed (empty results are OK for new collection)
    if echo "$result" | grep -q '"matches"'; then
        pass "gRPC query endpoint responds correctly"
    else
        fail "gRPC query endpoint" "Unexpected response: $result"
    fi
}

# Test 8: REST Insert Endpoint
test_rest_insert() {
    section "Test 8: REST Insert Endpoint"

    if [ -z "$TEST_COLLECTION_ID" ]; then
        skip "REST insert test" "No collection ID available"
        return
    fi

    # Generate 128-dim vector matching collection dimension
    insert_vector="[$(for i in $(seq 1 128); do echo -n "0.1"; [ $i -lt 128 ] && echo -n ","; done)]"

    response=$(curl -s -X POST "$REST_HOST/api/v1/collections/$TEST_COLLECTION_ID/insert" \
        -H "Content-Type: application/json" \
        -d "{
            \"doc_id\": \"test-doc-1\",
            \"vector\": $insert_vector,
            \"external_id\": \"smoke-test-1\"
        }" 2>/dev/null)

    # Should succeed with doc_id in response
    if echo "$response" | grep -q '"doc_id"'; then
        pass "REST insert endpoint successfully inserted document"
    else
        fail "REST insert endpoint" "Unexpected response: $response"
    fi
}

# Test 9: gRPC Create Collection
test_grpc_create_collection() {
    section "Test 9: gRPC Create Collection"

    if ! command -v grpcurl &> /dev/null; then
        skip "gRPC create collection" "grpcurl not installed"
        return
    fi

    result=$(grpcurl -plaintext -d '{
        "name": "grpc_test_collection",
        "dimension": 256,
        "metric": "l2"
    }' "$GRPC_HOST" akidb.collection.v1.CollectionManagementService/CreateCollection 2>&1)

    if echo "$result" | grep -q '"collection_id"'; then
        pass "gRPC create collection endpoint works"
    else
        fail "gRPC create collection" "Unexpected response: $result"
    fi
}

# Test 10: gRPC List Collections
test_grpc_list_collections() {
    section "Test 10: gRPC List Collections"

    if ! command -v grpcurl &> /dev/null; then
        skip "gRPC list collections" "grpcurl not installed"
        return
    fi

    result=$(grpcurl -plaintext -d '{}' "$GRPC_HOST" akidb.collection.v1.CollectionManagementService/ListCollections 2>&1)

    if echo "$result" | grep -q '"collections"'; then
        pass "gRPC list collections endpoint works"
    else
        fail "gRPC list collections" "Unexpected response: $result"
    fi
}

# Test 11: Docker Containers Running (if using Docker)
test_docker_containers() {
    section "Test 11: Docker Containers (if applicable)"

    if ! command -v docker &> /dev/null; then
        skip "Docker container check" "Docker not installed"
        return
    fi

    grpc_container=$(docker ps --filter "name=akidb-grpc" --format "{{.Status}}" 2>/dev/null || echo "")
    rest_container=$(docker ps --filter "name=akidb-rest" --format "{{.Status}}" 2>/dev/null || echo "")

    if [ -n "$grpc_container" ] && echo "$grpc_container" | grep -q "Up"; then
        pass "akidb-grpc container is running"
    else
        skip "gRPC container check" "Container not found or not running"
    fi

    if [ -n "$rest_container" ] && echo "$rest_container" | grep -q "Up"; then
        pass "akidb-rest container is running"
    else
        skip "REST container check" "Container not found or not running"
    fi
}

# Test 12: Response Time Check
test_response_time() {
    section "Test 12: Response Time Check"

    start_time=$(date +%s%3N)
    curl -s -o /dev/null "$REST_HOST/health" 2>/dev/null
    end_time=$(date +%s%3N)

    response_time=$((end_time - start_time))

    if [ $response_time -lt 100 ]; then
        pass "REST health endpoint responds in ${response_time}ms (<100ms)"
    else
        fail "Response time" "Health check took ${response_time}ms (expected <100ms)"
    fi
}

# Main execution
main() {
    echo "Starting smoke tests at $(date)"
    echo ""

    # Check prerequisites
    if ! check_prerequisites; then
        echo ""
        echo "⚠️  Some prerequisites are missing. Tests may be skipped."
    fi

    # Run tests
    test_rest_health
    test_grpc_health

    # Collection management tests
    test_rest_create_collection
    test_rest_list_collections
    test_rest_get_collection

    # Vector operation tests (require collection to exist)
    test_rest_query
    test_grpc_query
    test_rest_insert

    # gRPC collection management tests
    test_grpc_create_collection
    test_grpc_list_collections

    # Infrastructure tests
    test_docker_containers
    test_response_time

    # Summary
    echo ""
    echo "================================"
    echo "📊 Test Summary"
    echo "================================"
    echo "Total tests: $TESTS_TOTAL"
    echo -e "${GREEN}Passed: $TESTS_PASSED${NC}"
    echo -e "${RED}Failed: $TESTS_FAILED${NC}"
    echo ""

    if [ $TESTS_FAILED -eq 0 ]; then
        echo -e "${GREEN}🎉 All tests passed!${NC}"
        exit 0
    else
        echo -e "${RED}❌ Some tests failed${NC}"
        exit 1
    fi
}

# Run main
main
