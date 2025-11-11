# AkiDB Scripts

Utility scripts for deployment, backup, and maintenance of AkiDB.

## Available Scripts

| Script | Description |
|--------|-------------|
| `install-akidb.sh` | Install AkiDB on bare metal Linux (Ubuntu/Debian) |
| `backup-akidb.sh` | Perform online backup of SQLite database |
| `restore-akidb.sh` | Restore AkiDB from backup archive |

---

## install-akidb.sh

Automated installation script for bare metal Linux deployments.

### Features

- Creates system user and directories
- Installs binaries to `/usr/local/bin`
- Configures systemd services
- Sets up logrotate
- Configures security (file permissions, systemd hardening)

### Requirements

- Ubuntu 20.04+ or Debian 11+
- Root/sudo access
- Pre-built binaries in `target/release/`

### Usage

```bash
# Build binaries first
cargo build --release --workspace

# Run installer
sudo ./scripts/install-akidb.sh
```

### What It Does

1. Installs dependencies (sqlite3, logrotate, curl)
2. Creates `akidb` system user
3. Creates directories:
   - `/var/lib/akidb/data` - Database storage
   - `/var/log/akidb` - Log files
   - `/etc/akidb` - Configuration
4. Installs binaries:
   - `/usr/local/bin/akidb-grpc`
   - `/usr/local/bin/akidb-rest`
5. Installs systemd services:
   - `akidb-grpc.service`
   - `akidb-rest.service`
6. Configures logrotate
7. Enables services (does not start)

### Post-Installation

```bash
# Start services
sudo systemctl start akidb-grpc akidb-rest

# Check status
sudo systemctl status akidb-grpc akidb-rest

# View logs
sudo journalctl -u akidb-grpc -f

# Configure firewall
sudo ufw allow 9090/tcp  # gRPC
sudo ufw allow 8080/tcp  # REST
```

---

## backup-akidb.sh

Performs online backup of SQLite metadata database using SQLite's `.backup` command.

### Features

- Online backup (no service interruption)
- Integrity verification
- Compression (tar.gz)
- Optional GPG encryption
- Automatic cleanup (retention policy)
- Backup metadata (JSON)

### Requirements

- `sqlite3` command-line tool
- Optional: `gpg` for encryption

### Usage

**Basic Usage:**
```bash
./scripts/backup-akidb.sh
```

**Custom Backup Directory:**
```bash
./scripts/backup-akidb.sh /mnt/backup/akidb
```

**With Environment Variables:**
```bash
# Encrypted backup with 30-day retention
ENCRYPT=true \
GPG_RECIPIENT="backup@example.com" \
RETENTION_DAYS=30 \
./scripts/backup-akidb.sh
```

**Automated Backups (cron):**
```bash
# Edit crontab
sudo crontab -e

# Add backup job (every 6 hours)
0 */6 * * * /usr/local/bin/backup-akidb.sh >> /var/log/akidb/backup.log 2>&1

# Or with encryption
0 */6 * * * ENCRYPT=true GPG_RECIPIENT="backup@example.com" /usr/local/bin/backup-akidb.sh >> /var/log/akidb/backup.log 2>&1
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `BACKUP_DIR` | Backup destination directory | `/backup/akidb` |
| `DB_PATH` | SQLite database path | `/var/lib/akidb/data/metadata.db` |
| `CONFIG_PATH` | Config file path | `/etc/akidb/config.toml` |
| `RETENTION_DAYS` | Days to keep backups | `14` |
| `ENCRYPT` | Enable GPG encryption | `false` |
| `GPG_RECIPIENT` | GPG recipient email | (required if ENCRYPT=true) |

### Backup Contents

Each backup archive contains:

- `metadata.db` - SQLite database (verified)
- `config.toml` - Configuration file
- `backup.json` - Metadata (timestamp, hostname, size, version)

### Output

```
[2025-01-07 12:00:00] Starting AkiDB backup: akidb-backup-20250107-120000
[2025-01-07 12:00:00] Database: /var/lib/akidb/data/metadata.db
[2025-01-07 12:00:00] Backup directory: /backup/akidb
[2025-01-07 12:00:01] Backing up SQLite database...
[2025-01-07 12:00:05] Verifying backup integrity...
[2025-01-07 12:00:05] Integrity check: OK
[2025-01-07 12:00:05] Backing up configuration...
[2025-01-07 12:00:05] Compressing backup...
[2025-01-07 12:00:10] Archive created: /backup/akidb/akidb-backup-20250107-120000.tar.gz (1.2 GiB)
[2025-01-07 12:00:10] Cleaning up old backups (retention: 14 days)...
[2025-01-07 12:00:10] Backup completed successfully
[2025-01-07 12:00:10] Current backup: /backup/akidb/akidb-backup-20250107-120000.tar.gz
[2025-01-07 12:00:10] Total backups: 42
[2025-01-07 12:00:10] Latest backup link: /backup/akidb/akidb-backup-latest.tar.gz
```

---

## restore-akidb.sh

Restores AkiDB from a backup archive.

### Features

- Automatic backup extraction
- GPG decryption support
- Integrity verification
- Service management (stop/start)
- Safety backup before restore
- Configuration restore

### Requirements

- Root/sudo access
- `sqlite3` command-line tool
- Optional: `gpg` for encrypted backups

### Usage

**Interactive Restore:**
```bash
sudo ./scripts/restore-akidb.sh /backup/akidb/akidb-backup-20250107-120000.tar.gz
```

**Non-Interactive Restore:**
```bash
sudo ./scripts/restore-akidb.sh /backup/akidb/akidb-backup-latest.tar.gz --no-confirm
```

**With Environment Variables:**
```bash
SKIP_CONFIRM=true \
sudo ./scripts/restore-akidb.sh /backup/akidb/akidb-backup-latest.tar.gz
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DB_PATH` | SQLite database path | `/var/lib/akidb/data/metadata.db` |
| `CONFIG_PATH` | Config file path | `/etc/akidb/config.toml` |
| `SKIP_CONFIRM` | Skip confirmation prompt | `false` |

### What It Does

1. Verifies backup file exists
2. Decrypts if encrypted (.gpg)
3. Extracts archive
4. Verifies backup integrity
5. Shows confirmation prompt (unless --no-confirm)
6. Stops AkiDB services
7. Backs up current database (safety)
8. Restores database and config
9. Verifies restored database
10. Restarts services
11. Runs health checks

### Output

```
[2025-01-07 12:30:00] Extracting backup: /backup/akidb/akidb-backup-20250107-120000.tar.gz
[2025-01-07 12:30:05] Found database: /tmp/tmp.XxXxXx/akidb-backup-20250107-120000/metadata.db
[2025-01-07 12:30:05] Verifying backup integrity...
[2025-01-07 12:30:05] Integrity check: OK
[2025-01-07 12:30:05] Backup metadata:
[2025-01-07 12:30:05]   {"timestamp":"2025-01-07T12:00:00Z","hostname":"prod-01","database_path":"/var/lib/akidb/data/metadata.db","database_size_bytes":1234567890,"akidb_version":"2.0.0-rc1"}

WARNING: This will:
  1. Stop AkiDB services (akidb-grpc, akidb-rest)
  2. Replace current database: /var/lib/akidb/data/metadata.db
  3. Restart AkiDB services

Continue? [y/N] y

[2025-01-07 12:30:15] Stopping AkiDB services...
[2025-01-07 12:30:20] Services stopped
[2025-01-07 12:30:20] Backing up current database to: /var/lib/akidb/data/metadata.db.pre-restore-20250107-123020
[2025-01-07 12:30:25] Restoring database to: /var/lib/akidb/data/metadata.db
[2025-01-07 12:30:30] Set ownership: akidb:akidb
[2025-01-07 12:30:30] Restoring configuration to: /etc/akidb/config.toml
[2025-01-07 12:30:30] Verifying restored database...
[2025-01-07 12:30:30] Integrity check: OK
[2025-01-07 12:30:30] Starting AkiDB services...
[2025-01-07 12:30:35] Verifying services...
[2025-01-07 12:30:40] REST server health check: OK
[2025-01-07 12:30:40] gRPC server health check: OK
[2025-01-07 12:30:40] Restore completed successfully!
[2025-01-07 12:30:40] Restored from: /backup/akidb/akidb-backup-20250107-120000.tar.gz
[2025-01-07 12:30:40] Database: /var/lib/akidb/data/metadata.db
[2025-01-07 12:30:40] Previous database backed up to: /var/lib/akidb/data/metadata.db.pre-restore-20250107-123020
[2025-01-07 12:30:40] You can delete it after verifying the restore
```

### Safety Features

- **Confirmation prompt** - Prevents accidental restores
- **Safety backup** - Current database backed up before restore
- **Integrity verification** - Validates backup and restored database
- **Service management** - Automatically stops/starts services
- **Rollback support** - Can revert to pre-restore backup if needed

### Error Handling

If restore fails:
1. Original database remains intact (or restored from safety backup)
2. Services remain stopped (manual intervention required)
3. Check logs for details: `sudo journalctl -u akidb-grpc -n 100`

---

## Best Practices

### Backup Strategy

**Frequency:**
- Production: Every 6 hours
- Development: Daily

**Retention:**
- Production: 30 days minimum
- Development: 7-14 days

**Storage:**
- Keep backups on separate disk/server
- Use off-site backups for disaster recovery
- Encrypt sensitive data

**Verification:**
- Test restores quarterly
- Monitor backup success/failure
- Alert on backup failures

### Automation

**Systemd Timer (alternative to cron):**

Create `/etc/systemd/system/akidb-backup.timer`:
```ini
[Unit]
Description=AkiDB Backup Timer
Requires=akidb-backup.service

[Timer]
OnCalendar=*-*-* 00/6:00:00
Persistent=true

[Install]
WantedBy=timers.target
```

Create `/etc/systemd/system/akidb-backup.service`:
```ini
[Unit]
Description=AkiDB Backup Service

[Service]
Type=oneshot
ExecStart=/usr/local/bin/backup-akidb.sh
User=root
StandardOutput=journal
StandardError=journal
```

Enable:
```bash
sudo systemctl daemon-reload
sudo systemctl enable akidb-backup.timer
sudo systemctl start akidb-backup.timer
```

### Monitoring

**Alert on Backup Failures:**
```bash
# Add to backup-akidb.sh or create wrapper
if ! /usr/local/bin/backup-akidb.sh; then
    echo "AkiDB backup failed!" | mail -s "ALERT: AkiDB Backup Failure" admin@example.com
fi
```

**Prometheus Metrics:**
```bash
# Export backup metrics
cat > /var/lib/node_exporter/textfile_collector/akidb_backup.prom <<EOF
# HELP akidb_backup_last_success_timestamp_seconds Last successful backup timestamp
# TYPE akidb_backup_last_success_timestamp_seconds gauge
akidb_backup_last_success_timestamp_seconds $(date +%s)

# HELP akidb_backup_size_bytes Size of last backup
# TYPE akidb_backup_size_bytes gauge
akidb_backup_size_bytes $(stat -f%z /backup/akidb/akidb-backup-latest.tar.gz)
EOF
```

---

## Troubleshooting

### Backup Issues

**"Database is locked":**
```bash
# Check for WAL mode
sqlite3 /var/lib/akidb/data/metadata.db "PRAGMA journal_mode;"

# Enable WAL if needed
sqlite3 /var/lib/akidb/data/metadata.db "PRAGMA journal_mode=WAL;"
```

**"Integrity check failed":**
```bash
# Check database manually
sqlite3 /var/lib/akidb/data/metadata.db "PRAGMA integrity_check;"

# If corrupted, restore from previous backup
```

**"Permission denied":**
```bash
# Run as root or with sudo
sudo ./scripts/backup-akidb.sh

# Or fix permissions
sudo chown akidb:akidb /var/lib/akidb/data/metadata.db
sudo chmod 600 /var/lib/akidb/data/metadata.db
```

### Restore Issues

**"Services failed to start":**
```bash
# Check logs
sudo journalctl -u akidb-grpc -n 100
sudo journalctl -u akidb-rest -n 100

# Start manually
sudo systemctl start akidb-grpc
sudo systemctl start akidb-rest
```

**"Health check failed":**
```bash
# Wait longer (services may be initializing)
sleep 10
curl http://localhost:8080/health

# Check if ports are in use
sudo lsof -i :8080
sudo lsof -i :9090
```

---

## See Also

- [Deployment Guide](/docs/DEPLOYMENT-GUIDE.md) - Complete deployment documentation
- [Quickstart Guide](/docs/QUICKSTART.md) - Getting started
- [Migration Guide](/docs/MIGRATION-V1-TO-V2.md) - Upgrading from v1.x
