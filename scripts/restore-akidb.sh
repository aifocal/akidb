#!/bin/bash
#
# AkiDB Restore Script
# Restores AkiDB from backup archive
#
# Usage: ./restore-akidb.sh <backup-file> [--no-confirm]
#
# WARNING: This will stop AkiDB services and replace the current database!
#
# Environment Variables:
#   DB_PATH        - SQLite database path (default: /var/lib/akidb/data/metadata.db)
#   CONFIG_PATH    - Config file path (default: /etc/akidb/config.toml)
#   SKIP_CONFIRM   - Skip confirmation prompt (default: false)

set -euo pipefail

# Configuration
BACKUP_FILE="${1:-}"
SKIP_CONFIRM="${SKIP_CONFIRM:-false}"
DB_PATH="${DB_PATH:-/var/lib/akidb/data/metadata.db}"
CONFIG_PATH="${CONFIG_PATH:-/etc/akidb/config.toml}"
TEMP_DIR=$(mktemp -d)

# Parse arguments
if [ "$#" -ge 2 ] && [ "$2" = "--no-confirm" ]; then
    SKIP_CONFIRM=true
fi

# Logging
log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*"
}

error() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] ERROR: $*" >&2
}

cleanup() {
    rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

# Usage
if [ -z "$BACKUP_FILE" ]; then
    echo "Usage: $0 <backup-file> [--no-confirm]"
    echo
    echo "Example:"
    echo "  $0 /backup/akidb/akidb-backup-20250107-120000.tar.gz"
    echo "  $0 /backup/akidb/akidb-backup-latest.tar.gz --no-confirm"
    exit 1
fi

# Checks
if [ ! -f "$BACKUP_FILE" ]; then
    error "Backup file not found: $BACKUP_FILE"
    exit 1
fi

# Decrypt if encrypted
if [[ "$BACKUP_FILE" == *.gpg ]]; then
    log "Decrypting backup with GPG..."
    DECRYPTED="${TEMP_DIR}/backup.tar.gz"
    gpg --decrypt --output "$DECRYPTED" "$BACKUP_FILE"
    BACKUP_FILE="$DECRYPTED"
fi

# Extract backup
log "Extracting backup: $BACKUP_FILE"
tar -xzf "$BACKUP_FILE" -C "$TEMP_DIR"

# Find database file
BACKUP_DB=$(find "$TEMP_DIR" -name "metadata.db" -type f | head -n 1)
if [ -z "$BACKUP_DB" ]; then
    error "metadata.db not found in backup archive"
    exit 1
fi
log "Found database: $BACKUP_DB"

# Verify backup integrity
log "Verifying backup integrity..."
INTEGRITY_CHECK=$(sqlite3 "$BACKUP_DB" "PRAGMA integrity_check;" 2>&1)
if [ "$INTEGRITY_CHECK" != "ok" ]; then
    error "Backup integrity check failed: $INTEGRITY_CHECK"
    exit 1
fi
log "Integrity check: OK"

# Show backup metadata
BACKUP_JSON=$(find "$TEMP_DIR" -name "backup.json" -type f | head -n 1)
if [ -n "$BACKUP_JSON" ]; then
    log "Backup metadata:"
    cat "$BACKUP_JSON" | while read line; do
        log "  $line"
    done
fi

# Confirmation
if [ "$SKIP_CONFIRM" != "true" ]; then
    echo
    echo "WARNING: This will:"
    echo "  1. Stop AkiDB services (akidb-grpc, akidb-rest)"
    echo "  2. Replace current database: $DB_PATH"
    echo "  3. Restart AkiDB services"
    echo
    read -p "Continue? [y/N] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        log "Restore cancelled by user"
        exit 0
    fi
fi

# Stop services
log "Stopping AkiDB services..."
if command -v systemctl &> /dev/null; then
    sudo systemctl stop akidb-rest akidb-grpc 2>&1 | while read line; do
        log "  $line"
    done
    log "Services stopped"
elif command -v docker &> /dev/null; then
    docker stop akidb-rest akidb-grpc 2>&1 | while read line; do
        log "  $line"
    done
    log "Containers stopped"
else
    log "WARNING: Could not detect service manager. Please stop AkiDB manually."
    read -p "Press Enter when AkiDB is stopped..."
fi

# Backup current database
if [ -f "$DB_PATH" ]; then
    BACKUP_CURRENT="${DB_PATH}.pre-restore-$(date +%Y%m%d-%H%M%S)"
    log "Backing up current database to: $BACKUP_CURRENT"
    cp "$DB_PATH" "$BACKUP_CURRENT"
fi

# Restore database
log "Restoring database to: $DB_PATH"
mkdir -p "$(dirname $DB_PATH)"
cp "$BACKUP_DB" "$DB_PATH"

# Set ownership (if running as root)
if [ "$EUID" -eq 0 ]; then
    chown akidb:akidb "$DB_PATH"
    chmod 600 "$DB_PATH"
    log "Set ownership: akidb:akidb"
fi

# Restore configuration (if exists)
BACKUP_CONFIG=$(find "$TEMP_DIR" -name "config.toml" -type f | head -n 1)
if [ -n "$BACKUP_CONFIG" ] && [ -f "$CONFIG_PATH" ]; then
    log "Restoring configuration to: $CONFIG_PATH"
    if [ "$EUID" -eq 0 ]; then
        cp "$BACKUP_CONFIG" "$CONFIG_PATH"
        chown root:akidb "$CONFIG_PATH"
        chmod 640 "$CONFIG_PATH"
    else
        sudo cp "$BACKUP_CONFIG" "$CONFIG_PATH"
        sudo chown root:akidb "$CONFIG_PATH"
        sudo chmod 640 "$CONFIG_PATH"
    fi
fi

# Verify restored database
log "Verifying restored database..."
INTEGRITY_CHECK=$(sqlite3 "$DB_PATH" "PRAGMA integrity_check;" 2>&1)
if [ "$INTEGRITY_CHECK" != "ok" ]; then
    error "Restored database integrity check failed: $INTEGRITY_CHECK"
    if [ -f "$BACKUP_CURRENT" ]; then
        log "Reverting to previous database..."
        cp "$BACKUP_CURRENT" "$DB_PATH"
    fi
    exit 1
fi
log "Integrity check: OK"

# Restart services
log "Starting AkiDB services..."
if command -v systemctl &> /dev/null; then
    sudo systemctl start akidb-grpc akidb-rest 2>&1 | while read line; do
        log "  $line"
    done
    sleep 3
    sudo systemctl status akidb-grpc akidb-rest --no-pager | while read line; do
        log "  $line"
    done
elif command -v docker &> /dev/null; then
    docker start akidb-grpc akidb-rest 2>&1 | while read line; do
        log "  $line"
    done
    sleep 3
    docker ps | grep akidb | while read line; do
        log "  $line"
    done
fi

# Verify services
log "Verifying services..."
sleep 5

# Check REST health
if command -v curl &> /dev/null; then
    if curl -sf http://localhost:8080/health > /dev/null 2>&1; then
        log "REST server health check: OK"
    else
        log "WARNING: REST server health check failed"
    fi
fi

# Check gRPC health
if command -v grpcurl &> /dev/null; then
    if grpcurl -plaintext localhost:9090 grpc.health.v1.Health/Check > /dev/null 2>&1; then
        log "gRPC server health check: OK"
    else
        log "WARNING: gRPC server health check failed"
    fi
fi

log "Restore completed successfully!"
log "Restored from: $BACKUP_FILE"
log "Database: $DB_PATH"

if [ -f "$BACKUP_CURRENT" ]; then
    log "Previous database backed up to: $BACKUP_CURRENT"
    log "You can delete it after verifying the restore"
fi

exit 0
