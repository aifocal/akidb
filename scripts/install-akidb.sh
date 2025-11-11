#!/bin/bash
#
# AkiDB Installation Script
# Installs AkiDB on bare metal Linux systems
#
# Usage: sudo ./install-akidb.sh
#
# Requirements: Ubuntu 20.04+ or Debian 11+

set -euo pipefail

# Configuration
INSTALL_USER="akidb"
INSTALL_GROUP="akidb"
DATA_DIR="/var/lib/akidb"
LOG_DIR="/var/log/akidb"
CONFIG_DIR="/etc/akidb"
BIN_DIR="/usr/local/bin"

# Logging
log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*"
}

error() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] ERROR: $*" >&2
}

# Check root
if [ "$EUID" -ne 0 ]; then
    error "This script must be run as root (use sudo)"
    exit 1
fi

log "Starting AkiDB installation..."

# Detect OS
if [ -f /etc/os-release ]; then
    . /etc/os-release
    OS=$ID
    VERSION=$VERSION_ID
    log "Detected OS: $OS $VERSION"
else
    error "Cannot detect OS. Only Ubuntu/Debian supported."
    exit 1
fi

# Check supported OS
if [[ "$OS" != "ubuntu" ]] && [[ "$OS" != "debian" ]]; then
    error "Unsupported OS: $OS. Only Ubuntu/Debian supported."
    exit 1
fi

# Install dependencies
log "Installing dependencies..."
apt-get update -qq
apt-get install -y \
    sqlite3 \
    logrotate \
    curl \
    ca-certificates

# Create user and group
if ! id -u $INSTALL_USER > /dev/null 2>&1; then
    log "Creating user: $INSTALL_USER"
    useradd -r -s /bin/false -d $DATA_DIR $INSTALL_USER
else
    log "User already exists: $INSTALL_USER"
fi

# Create directories
log "Creating directories..."
mkdir -p $DATA_DIR/data
mkdir -p $LOG_DIR
mkdir -p $CONFIG_DIR

# Set ownership
chown -R $INSTALL_USER:$INSTALL_GROUP $DATA_DIR
chown -R $INSTALL_USER:$INSTALL_GROUP $LOG_DIR
chown root:$INSTALL_GROUP $CONFIG_DIR

log "Directories created:"
log "  Data: $DATA_DIR"
log "  Logs: $LOG_DIR"
log "  Config: $CONFIG_DIR"

# Check for binaries
if [ ! -f "target/release/akidb-grpc" ] || [ ! -f "target/release/akidb-rest" ]; then
    error "Binaries not found. Please build first:"
    error "  cargo build --release --workspace"
    exit 1
fi

# Install binaries
log "Installing binaries..."
install -m 755 target/release/akidb-grpc $BIN_DIR/akidb-grpc
install -m 755 target/release/akidb-rest $BIN_DIR/akidb-rest
log "Binaries installed to $BIN_DIR"

# Install configuration
if [ ! -f "$CONFIG_DIR/config.toml" ]; then
    if [ -f "config.example.toml" ]; then
        log "Installing default configuration..."
        cp config.example.toml $CONFIG_DIR/config.toml

        # Update paths for production
        sed -i "s|path = \"sqlite://akidb.db\"|path = \"sqlite://$DATA_DIR/data/metadata.db\"|" $CONFIG_DIR/config.toml
        sed -i 's|format = "pretty"|format = "json"|' $CONFIG_DIR/config.toml

        chown root:$INSTALL_GROUP $CONFIG_DIR/config.toml
        chmod 640 $CONFIG_DIR/config.toml
        log "Configuration installed: $CONFIG_DIR/config.toml"
    else
        log "WARNING: config.example.toml not found. Skipping configuration."
    fi
else
    log "Configuration already exists: $CONFIG_DIR/config.toml"
fi

# Install systemd service files
log "Installing systemd services..."

# gRPC service
cat > /etc/systemd/system/akidb-grpc.service <<'EOF'
[Unit]
Description=AkiDB gRPC Server
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=akidb
Group=akidb
WorkingDirectory=/var/lib/akidb

ExecStart=/usr/local/bin/akidb-grpc
ExecReload=/bin/kill -HUP $MAINPID

Restart=on-failure
RestartSec=10s

# Environment
Environment="RUST_LOG=info"
Environment="AKIDB_CONFIG=/etc/akidb/config.toml"

# Security
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/akidb /var/log/akidb

# Resource limits
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF

# REST service
cat > /etc/systemd/system/akidb-rest.service <<'EOF'
[Unit]
Description=AkiDB REST Server
After=network.target akidb-grpc.service
Wants=network-online.target

[Service]
Type=simple
User=akidb
Group=akidb
WorkingDirectory=/var/lib/akidb

ExecStart=/usr/local/bin/akidb-rest
ExecReload=/bin/kill -HUP $MAINPID

Restart=on-failure
RestartSec=10s

# Environment
Environment="RUST_LOG=info"
Environment="AKIDB_CONFIG=/etc/akidb/config.toml"

# Security
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/akidb /var/log/akidb

# Resource limits
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF

log "Systemd services installed"

# Install logrotate configuration
log "Installing logrotate configuration..."
cat > /etc/logrotate.d/akidb <<'EOF'
/var/log/akidb/*.log {
    daily
    rotate 14
    compress
    delaycompress
    notifempty
    missingok
    copytruncate
    postrotate
        systemctl reload akidb-grpc akidb-rest > /dev/null 2>&1 || true
    endscript
}
EOF

log "Logrotate configuration installed"

# Reload systemd
log "Reloading systemd daemon..."
systemctl daemon-reload

# Enable services
log "Enabling services..."
systemctl enable akidb-grpc
systemctl enable akidb-rest

log "AkiDB installation completed successfully!"
log ""
log "Next steps:"
log "  1. Review configuration: $CONFIG_DIR/config.toml"
log "  2. Start services:"
log "       sudo systemctl start akidb-grpc"
log "       sudo systemctl start akidb-rest"
log "  3. Check status:"
log "       sudo systemctl status akidb-grpc"
log "       sudo systemctl status akidb-rest"
log "  4. View logs:"
log "       sudo journalctl -u akidb-grpc -f"
log "       sudo journalctl -u akidb-rest -f"
log "  5. Test health:"
log "       curl http://localhost:8080/health"
log ""
log "For firewall configuration:"
log "  sudo ufw allow 9090/tcp  # gRPC"
log "  sudo ufw allow 8080/tcp  # REST"

exit 0
