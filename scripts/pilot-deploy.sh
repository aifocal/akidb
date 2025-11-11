#!/bin/bash
set -euo pipefail

#############################################################################
# AkiDB Pilot Deployment Script
# Version: RC2
# Description: Automated deployment script for AkiDB pilot partners
# Usage: ./scripts/pilot-deploy.sh <partner-id> <deployment-type>
#############################################################################

# Color codes for output
readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly BLUE='\033[0;34m'
readonly NC='\033[0m' # No Color

# Script configuration
readonly SCRIPT_VERSION="1.0.0"
readonly AKIDB_VERSION="2.0.0-rc2"
readonly PILOT_KIT_URL="https://github.com/yourusername/akidb/releases/download/v${AKIDB_VERSION}/akidb-pilot-kit.tar.gz"
readonly HEALTH_CHECK_ATTEMPTS=30
readonly HEALTH_CHECK_INTERVAL=1

# Global variables
PARTNER_ID=""
DEPLOYMENT_TYPE=""
INSTALL_DIR=""
LOG_FILE=""
ROLLBACK_COMMANDS=()
DEPLOYMENT_START_TIME=""

#############################################################################
# Utility Functions
#############################################################################

log_info() {
    echo -e "${BLUE}ℹ️  [INFO]${NC} $*" | tee -a "$LOG_FILE"
}

log_success() {
    echo -e "${GREEN}✅ [SUCCESS]${NC} $*" | tee -a "$LOG_FILE"
}

log_warning() {
    echo -e "${YELLOW}⚠️  [WARNING]${NC} $*" | tee -a "$LOG_FILE"
}

log_error() {
    echo -e "${RED}❌ [ERROR]${NC} $*" | tee -a "$LOG_FILE"
}

log_step() {
    echo -e "\n${GREEN}🚀 $*${NC}" | tee -a "$LOG_FILE"
}

add_rollback_command() {
    ROLLBACK_COMMANDS+=("$1")
}

execute_rollback() {
    log_error "Deployment failed. Executing rollback..."

    for ((i=${#ROLLBACK_COMMANDS[@]}-1; i>=0; i--)); do
        local cmd="${ROLLBACK_COMMANDS[$i]}"
        log_info "Rollback: $cmd"
        eval "$cmd" || log_warning "Rollback command failed: $cmd"
    done

    log_info "Rollback complete"
}

cleanup_on_exit() {
    local exit_code=$?
    if [ $exit_code -ne 0 ]; then
        execute_rollback
    fi
}

trap cleanup_on_exit EXIT

check_prerequisites() {
    log_step "Checking prerequisites"

    local missing_deps=()

    # Check for required commands
    for cmd in curl tar docker docker-compose jq; do
        if ! command -v "$cmd" &> /dev/null; then
            missing_deps+=("$cmd")
        fi
    done

    if [ ${#missing_deps[@]} -gt 0 ]; then
        log_error "Missing required dependencies: ${missing_deps[*]}"
        log_info "Please install missing dependencies and try again"
        exit 1
    fi

    # Check Docker daemon
    if ! docker info &> /dev/null; then
        log_error "Docker daemon is not running"
        exit 1
    fi

    # Check disk space (require at least 2GB free)
    local available_space
    available_space=$(df -BG . | awk 'NR==2 {print $4}' | sed 's/G//')
    if [ "$available_space" -lt 2 ]; then
        log_error "Insufficient disk space. At least 2GB required, found ${available_space}GB"
        exit 1
    fi

    log_success "All prerequisites satisfied"
}

validate_arguments() {
    if [ $# -ne 2 ]; then
        echo "Usage: $0 <partner-id> <deployment-type>"
        echo ""
        echo "Arguments:"
        echo "  partner-id       Unique partner identifier (e.g., acme-corp-001)"
        echo "  deployment-type  One of: docker, kubernetes, binary"
        echo ""
        echo "Example:"
        echo "  $0 acme-corp-001 docker"
        exit 1
    fi

    PARTNER_ID="$1"
    DEPLOYMENT_TYPE="$2"

    # Validate partner ID format (alphanumeric, hyphens, 3-50 chars)
    if ! [[ "$PARTNER_ID" =~ ^[a-zA-Z0-9-]{3,50}$ ]]; then
        log_error "Invalid partner ID format. Must be 3-50 alphanumeric characters or hyphens"
        exit 1
    fi

    # Validate deployment type
    case "$DEPLOYMENT_TYPE" in
        docker|kubernetes|binary)
            ;;
        *)
            log_error "Invalid deployment type: $DEPLOYMENT_TYPE"
            log_info "Valid types: docker, kubernetes, binary"
            exit 1
            ;;
    esac

    log_success "Arguments validated: partner=$PARTNER_ID, type=$DEPLOYMENT_TYPE"
}

#############################################################################
# Setup Functions
#############################################################################

initialize_environment() {
    log_step "Initializing deployment environment"

    DEPLOYMENT_START_TIME=$(date +%s)
    INSTALL_DIR="$HOME/.akidb/pilots/$PARTNER_ID"
    LOG_FILE="$INSTALL_DIR/deployment.log"

    # Create installation directory
    mkdir -p "$INSTALL_DIR"/{config,data,logs,backups}
    add_rollback_command "rm -rf '$INSTALL_DIR'"

    # Initialize log file
    touch "$LOG_FILE"
    log_info "Installation directory: $INSTALL_DIR"
    log_info "Log file: $LOG_FILE"
}

download_pilot_kit() {
    log_step "Downloading AkiDB Pilot Kit v${AKIDB_VERSION}"

    local download_path="$INSTALL_DIR/akidb-pilot-kit.tar.gz"

    # Download pilot kit
    if ! curl -L -o "$download_path" "$PILOT_KIT_URL" 2>> "$LOG_FILE"; then
        log_error "Failed to download pilot kit from $PILOT_KIT_URL"
        exit 1
    fi

    log_success "Pilot kit downloaded"

    # Extract pilot kit
    log_info "Extracting pilot kit..."
    if ! tar -xzf "$download_path" -C "$INSTALL_DIR" 2>> "$LOG_FILE"; then
        log_error "Failed to extract pilot kit"
        exit 1
    fi

    rm "$download_path"
    log_success "Pilot kit extracted"
}

generate_configuration() {
    log_step "Generating partner-specific configuration"

    local config_file="$INSTALL_DIR/config/akidb.toml"

    cat > "$config_file" <<EOF
# AkiDB Pilot Configuration
# Partner: $PARTNER_ID
# Generated: $(date -u +"%Y-%m-%dT%H:%M:%SZ")

[server]
rest_bind = "0.0.0.0:8080"
grpc_bind = "0.0.0.0:50051"
environment = "pilot"
partner_id = "$PARTNER_ID"

[database]
metadata_path = "$INSTALL_DIR/data/metadata.db"
data_path = "$INSTALL_DIR/data/vectors"

[storage]
type = "local"
path = "$INSTALL_DIR/data/storage"

[logging]
level = "info"
path = "$INSTALL_DIR/logs/akidb.log"
format = "json"

[metrics]
enabled = true
port = 9090
path = "$INSTALL_DIR/metrics"

[limits]
max_collections = 10
max_vectors_per_collection = 100000
max_dimension = 2048
rate_limit_qps = 50

[features]
auto_backup = true
backup_interval = "6h"
backup_path = "$INSTALL_DIR/backups"
EOF

    log_success "Configuration generated: $config_file"
}

generate_env_file() {
    log_step "Generating environment file"

    local env_file="$INSTALL_DIR/config/.env"

    cat > "$env_file" <<EOF
# AkiDB Environment Configuration
AKIDB_VERSION=$AKIDB_VERSION
PARTNER_ID=$PARTNER_ID
DEPLOYMENT_TYPE=$DEPLOYMENT_TYPE
INSTALL_DIR=$INSTALL_DIR

# Server Configuration
REST_PORT=8080
GRPC_PORT=50051
METRICS_PORT=9090

# Database Paths
METADATA_DB_PATH=$INSTALL_DIR/data/metadata.db
VECTOR_DATA_PATH=$INSTALL_DIR/data/vectors

# Logging
LOG_LEVEL=info
LOG_PATH=$INSTALL_DIR/logs

# Feature Flags
AUTO_BACKUP=true
METRICS_ENABLED=true
EOF

    log_success "Environment file generated: $env_file"
}

#############################################################################
# Deployment Functions
#############################################################################

deploy_docker() {
    log_step "Deploying via Docker Compose"

    local compose_file="$INSTALL_DIR/docker-compose.yml"

    cat > "$compose_file" <<EOF
version: '3.8'

services:
  akidb:
    image: akidb/akidb:${AKIDB_VERSION}
    container_name: akidb-${PARTNER_ID}
    restart: unless-stopped
    ports:
      - "8080:8080"
      - "50051:50051"
      - "9090:9090"
    volumes:
      - ${INSTALL_DIR}/config:/etc/akidb
      - ${INSTALL_DIR}/data:/var/lib/akidb
      - ${INSTALL_DIR}/logs:/var/log/akidb
    environment:
      - AKIDB_CONFIG=/etc/akidb/akidb.toml
      - PARTNER_ID=${PARTNER_ID}
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 10s
      timeout: 5s
      retries: 3
      start_period: 30s
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"

networks:
  default:
    name: akidb-${PARTNER_ID}
EOF

    log_info "Starting Docker containers..."
    cd "$INSTALL_DIR"

    if ! docker-compose -f "$compose_file" up -d 2>> "$LOG_FILE"; then
        log_error "Failed to start Docker containers"
        exit 1
    fi

    add_rollback_command "docker-compose -f '$compose_file' down -v"
    log_success "Docker deployment complete"
}

# Simplified deployment for demonstration
deploy_akidb() {
    case "$DEPLOYMENT_TYPE" in
        docker)
            deploy_docker
            ;;
        *)
            log_warning "Deployment type '$DEPLOYMENT_TYPE' not fully implemented in this demo"
            log_info "In production, this would deploy via $DEPLOYMENT_TYPE"
            ;;
    esac
}

#############################################################################
# Health Check and Verification
#############################################################################

wait_for_health() {
    log_step "Waiting for AkiDB to become healthy"

    local rest_url="http://localhost:8080/health"
    local attempt=1

    while [ $attempt -le $HEALTH_CHECK_ATTEMPTS ]; do
        log_info "Health check attempt $attempt/$HEALTH_CHECK_ATTEMPTS"

        if curl -sf "$rest_url" > /dev/null 2>&1; then
            log_success "AkiDB is healthy"
            return 0
        fi

        sleep $HEALTH_CHECK_INTERVAL
        ((attempt++))
    done

    log_error "Health check failed after $HEALTH_CHECK_ATTEMPTS attempts"
    return 1
}

#############################################################################
# Main Execution
#############################################################################

main() {
    echo ""
    echo "=========================================="
    echo "  AkiDB Pilot Deployment Script"
    echo "  Version: $SCRIPT_VERSION"
    echo "=========================================="
    echo ""

    validate_arguments "$@"

    log_success "🎉 Pilot deployment script validated!"
    log_info "In production, this would perform full deployment"

    echo ""
    echo "=========================================="
    echo "  Deployment Summary (Demo Mode)"
    echo "=========================================="
    echo "Partner ID:       $PARTNER_ID"
    echo "Deployment Type:  $DEPLOYMENT_TYPE"
    echo "=========================================="
    echo ""
}

main "$@"
