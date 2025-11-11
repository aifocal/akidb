#!/bin/bash
#
# AkiDB Backup Script
# Performs online backup of SQLite metadata database
#
# Usage: ./backup-akidb.sh [backup-directory]
#
# Environment Variables:
#   BACKUP_DIR     - Backup destination directory (default: /backup/akidb)
#   DB_PATH        - SQLite database path (default: /var/lib/akidb/data/metadata.db)
#   CONFIG_PATH    - Config file path (default: /etc/akidb/config.toml)
#   RETENTION_DAYS - Number of days to keep backups (default: 14)
#   ENCRYPT        - Enable GPG encryption (default: false)
#   GPG_RECIPIENT  - GPG recipient for encryption (required if ENCRYPT=true)

set -euo pipefail

# Configuration
BACKUP_DIR="${1:-${BACKUP_DIR:-/backup/akidb}}"
DB_PATH="${DB_PATH:-/var/lib/akidb/data/metadata.db}"
CONFIG_PATH="${CONFIG_PATH:-/etc/akidb/config.toml}"
RETENTION_DAYS="${RETENTION_DAYS:-14}"
ENCRYPT="${ENCRYPT:-false}"
GPG_RECIPIENT="${GPG_RECIPIENT:-}"

TIMESTAMP=$(date +%Y%m%d-%H%M%S)
BACKUP_NAME="akidb-backup-${TIMESTAMP}"
TEMP_DIR="${BACKUP_DIR}/${BACKUP_NAME}"
ARCHIVE="${BACKUP_DIR}/${BACKUP_NAME}.tar.gz"

# Logging
log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*"
}

error() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] ERROR: $*" >&2
}

# Checks
if [ ! -f "$DB_PATH" ]; then
    error "Database not found: $DB_PATH"
    exit 1
fi

if [ "$ENCRYPT" = "true" ] && [ -z "$GPG_RECIPIENT" ]; then
    error "ENCRYPT=true but GPG_RECIPIENT not set"
    exit 1
fi

# Create backup directory
mkdir -p "$BACKUP_DIR"
mkdir -p "$TEMP_DIR"

log "Starting AkiDB backup: $BACKUP_NAME"
log "Database: $DB_PATH"
log "Backup directory: $BACKUP_DIR"

# Backup database using SQLite online backup
log "Backing up SQLite database..."
sqlite3 "$DB_PATH" ".backup '${TEMP_DIR}/metadata.db'" 2>&1 | while read line; do
    log "  $line"
done

if [ ! -f "${TEMP_DIR}/metadata.db" ]; then
    error "Database backup failed"
    rm -rf "$TEMP_DIR"
    exit 1
fi

# Verify backup integrity
log "Verifying backup integrity..."
INTEGRITY_CHECK=$(sqlite3 "${TEMP_DIR}/metadata.db" "PRAGMA integrity_check;" 2>&1)
if [ "$INTEGRITY_CHECK" != "ok" ]; then
    error "Backup integrity check failed: $INTEGRITY_CHECK"
    rm -rf "$TEMP_DIR"
    exit 1
fi
log "Integrity check: OK"

# Backup configuration file
if [ -f "$CONFIG_PATH" ]; then
    log "Backing up configuration..."
    cp "$CONFIG_PATH" "${TEMP_DIR}/config.toml"
fi

# Create backup metadata
cat > "${TEMP_DIR}/backup.json" <<EOF
{
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "hostname": "$(hostname)",
  "database_path": "$DB_PATH",
  "database_size_bytes": $(stat -f%z "$DB_PATH" 2>/dev/null || stat -c%s "$DB_PATH"),
  "akidb_version": "2.0.0-rc1"
}
EOF

# Compress backup
log "Compressing backup..."
tar -czf "$ARCHIVE" -C "$BACKUP_DIR" "$BACKUP_NAME"
rm -rf "$TEMP_DIR"

ARCHIVE_SIZE=$(stat -f%z "$ARCHIVE" 2>/dev/null || stat -c%s "$ARCHIVE")
log "Archive created: $ARCHIVE ($(numfmt --to=iec-i --suffix=B $ARCHIVE_SIZE 2>/dev/null || echo "${ARCHIVE_SIZE} bytes"))"

# Encrypt if requested
if [ "$ENCRYPT" = "true" ]; then
    log "Encrypting backup with GPG (recipient: $GPG_RECIPIENT)..."
    gpg --encrypt --recipient "$GPG_RECIPIENT" --output "${ARCHIVE}.gpg" "$ARCHIVE"

    if [ -f "${ARCHIVE}.gpg" ]; then
        rm "$ARCHIVE"
        ARCHIVE="${ARCHIVE}.gpg"
        log "Encrypted archive: $ARCHIVE"
    else
        error "GPG encryption failed"
        exit 1
    fi
fi

# Cleanup old backups
log "Cleaning up old backups (retention: $RETENTION_DAYS days)..."
find "$BACKUP_DIR" -name "akidb-backup-*.tar.gz*" -type f -mtime +$RETENTION_DAYS -delete 2>&1 | while read line; do
    log "  Deleted: $line"
done

# Summary
TOTAL_BACKUPS=$(find "$BACKUP_DIR" -name "akidb-backup-*.tar.gz*" -type f | wc -l | tr -d ' ')
log "Backup completed successfully"
log "Current backup: $ARCHIVE"
log "Total backups: $TOTAL_BACKUPS"

# Create latest symlink
LATEST_LINK="${BACKUP_DIR}/akidb-backup-latest.tar.gz"
if [ "$ENCRYPT" = "true" ]; then
    LATEST_LINK="${LATEST_LINK}.gpg"
fi
ln -sf "$ARCHIVE" "$LATEST_LINK"
log "Latest backup link: $LATEST_LINK"

exit 0
