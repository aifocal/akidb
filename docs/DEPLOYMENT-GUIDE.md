# AkiDB 2.0 Deployment Guide

Complete guide for deploying AkiDB in production environments using Docker, Kubernetes, or bare metal.

---

## Table of Contents

- [Quick Start](#quick-start)
- [Docker Deployment](#docker-deployment)
- [Kubernetes Deployment](#kubernetes-deployment)
- [Bare Metal Deployment](#bare-metal-deployment)
- [Configuration Reference](#configuration-reference)
- [Production Checklist](#production-checklist)
- [Security Hardening](#security-hardening)
- [Monitoring and Observability](#monitoring-and-observability)
- [Backup and Disaster Recovery](#backup-and-disaster-recovery)
- [Troubleshooting](#troubleshooting)

---

## Quick Start

AkiDB provides two server binaries:

- **akidb-grpc** (recommended for production): gRPC API server on port 9090
- **akidb-rest**: REST API server on port 8080

Both servers share the same SQLite metadata database and support:
- Auto-initialization (default tenant + database created on first startup)
- Collection persistence (survives restarts)
- Vector persistence (optional, enabled by default)
- Prometheus metrics endpoint at `/metrics`
- Health checks at `/health` (REST) or `grpc.health.v1.Health/Check` (gRPC)
- Graceful shutdown (SIGTERM/SIGINT)

---

## Docker Deployment

### Using Docker Compose (Recommended)

The simplest way to deploy AkiDB is using the provided `docker-compose.yaml`:

```bash
# Clone the repository
git clone https://github.com/your-org/akidb2.git
cd akidb2

# Start both gRPC and REST servers
docker-compose up -d

# Check logs
docker-compose logs -f

# Stop servers
docker-compose down

# Stop and remove volumes (WARNING: destroys all data)
docker-compose down -v
```

**Services:**
- `akidb-grpc`: gRPC server on port 9000
- `akidb-rest`: REST server on port 8080

**Volumes:**
- `akidb-data`: Persistent storage for SQLite database
- `akidb-logs`: Application logs

### Manual Docker Deployment

#### Build Docker Images

```bash
# Build gRPC server image
docker build -f docker/Dockerfile.grpc -t akidb/akidb-grpc:2.0.0-rc1 .

# Build REST server image
docker build -f docker/Dockerfile.rest -t akidb/akidb-rest:2.0.0-rc1 .
```

#### Run gRPC Server

```bash
docker run -d \
  --name akidb-grpc \
  -p 9000:9000 \
  -v akidb-data:/data/akidb \
  -v akidb-logs:/var/log/akidb \
  -e RUST_LOG=info \
  -e AKIDB_HOST=0.0.0.0 \
  -e AKIDB_GRPC_PORT=9000 \
  -e AKIDB_DB_PATH=sqlite:///data/akidb/metadata.db \
  --restart unless-stopped \
  akidb/akidb-grpc:2.0.0-rc1
```

#### Run REST Server

```bash
docker run -d \
  --name akidb-rest \
  -p 8080:8080 \
  -v akidb-data:/data/akidb \
  -v akidb-logs:/var/log/akidb \
  -e RUST_LOG=info \
  -e AKIDB_HOST=0.0.0.0 \
  -e AKIDB_REST_PORT=8080 \
  -e AKIDB_DB_PATH=sqlite:///data/akidb/metadata.db \
  --restart unless-stopped \
  akidb/akidb-rest:2.0.0-rc1
```

### Docker Compose Configuration

The provided `docker-compose.yaml` includes:

```yaml
version: '3.8'

services:
  akidb-grpc:
    image: akidb/akidb-grpc:2.0.0-rc1
    ports:
      - "9000:9000"
    volumes:
      - akidb-data:/data/akidb
      - akidb-logs:/var/log/akidb
    environment:
      - RUST_LOG=info
      - AKIDB_HOST=0.0.0.0
      - AKIDB_GRPC_PORT=9000
      - AKIDB_DB_PATH=sqlite:///data/akidb/metadata.db
      - AKIDB_METRICS_ENABLED=true
      - AKIDB_VECTOR_PERSISTENCE_ENABLED=true
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "grpcurl", "-plaintext", "localhost:9000", "grpc.health.v1.Health/Check"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 10s

  akidb-rest:
    image: akidb/akidb-rest:2.0.0-rc1
    ports:
      - "8080:8080"
    volumes:
      - akidb-data:/data/akidb
      - akidb-logs:/var/log/akidb
    environment:
      - RUST_LOG=info
      - AKIDB_HOST=0.0.0.0
      - AKIDB_REST_PORT=8080
      - AKIDB_DB_PATH=sqlite:///data/akidb/metadata.db
      - AKIDB_METRICS_ENABLED=true
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 10s

volumes:
  akidb-data:
    driver: local
  akidb-logs:
    driver: local
```

### Health Checks

**REST Server:**
```bash
curl http://localhost:8080/health
# Response: {"status":"healthy","version":"2.0.0-rc1"}
```

**gRPC Server (requires grpcurl):**
```bash
grpcurl -plaintext localhost:9000 grpc.health.v1.Health/Check
# Response: {"status": "SERVING"}
```

---

## Kubernetes Deployment

### Prerequisites

- Kubernetes cluster (v1.24+)
- kubectl configured
- Persistent volume provisioner (for SQLite storage)
- Optional: Prometheus Operator (for metrics)

### Quick Deploy

```bash
# Create namespace and deploy all resources
kubectl apply -f k8s/

# Check deployment status
kubectl get all -n akidb

# Follow logs
kubectl logs -f -n akidb -l app=akidb-grpc
```

### Kubernetes Manifests

All manifests are located in `k8s/` directory. Create this directory structure:

```
k8s/
├── namespace.yaml
├── configmap.yaml
├── persistentvolume.yaml
├── persistentvolumeclaim.yaml
├── deployment-grpc.yaml
├── deployment-rest.yaml
├── service-grpc.yaml
├── service-rest.yaml
├── servicemonitor.yaml (optional)
└── ingress.yaml (optional)
```

#### 1. Namespace

Create `k8s/namespace.yaml`:

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: akidb
  labels:
    name: akidb
    environment: production
```

#### 2. ConfigMap

Create `k8s/configmap.yaml`:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: akidb-config
  namespace: akidb
data:
  RUST_LOG: "info"
  AKIDB_HOST: "0.0.0.0"
  AKIDB_GRPC_PORT: "9090"
  AKIDB_REST_PORT: "8080"
  AKIDB_DB_PATH: "sqlite:///data/akidb/metadata.db"
  AKIDB_METRICS_ENABLED: "true"
  AKIDB_VECTOR_PERSISTENCE_ENABLED: "true"
  AKIDB_LOG_FORMAT: "json"
```

#### 3. Persistent Volume

Create `k8s/persistentvolume.yaml`:

```yaml
apiVersion: v1
kind: PersistentVolume
metadata:
  name: akidb-pv
  namespace: akidb
spec:
  capacity:
    storage: 100Gi
  accessModes:
    - ReadWriteOnce
  persistentVolumeReclaimPolicy: Retain
  storageClassName: akidb-storage
  hostPath:
    path: /mnt/data/akidb
    type: DirectoryOrCreate
---
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: akidb-pvc
  namespace: akidb
spec:
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: 100Gi
  storageClassName: akidb-storage
```

**Note:** For production, replace `hostPath` with a cloud provider's persistent disk:

**AWS EBS:**
```yaml
spec:
  awsElasticBlockStore:
    volumeID: <volume-id>
    fsType: ext4
```

**GCP Persistent Disk:**
```yaml
spec:
  gcePersistentDisk:
    pdName: akidb-disk
    fsType: ext4
```

**Azure Disk:**
```yaml
spec:
  azureDisk:
    diskName: akidb-disk
    diskURI: /subscriptions/.../akidb-disk
```

#### 4. gRPC Deployment

Create `k8s/deployment-grpc.yaml`:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: akidb-grpc
  namespace: akidb
  labels:
    app: akidb-grpc
    component: grpc-server
spec:
  replicas: 2
  selector:
    matchLabels:
      app: akidb-grpc
  template:
    metadata:
      labels:
        app: akidb-grpc
        component: grpc-server
    spec:
      affinity:
        podAntiAffinity:
          preferredDuringSchedulingIgnoredDuringExecution:
            - weight: 100
              podAffinityTerm:
                labelSelector:
                  matchExpressions:
                    - key: app
                      operator: In
                      values:
                        - akidb-grpc
                topologyKey: kubernetes.io/hostname
      containers:
        - name: akidb-grpc
          image: akidb/akidb-grpc:2.0.0-rc1
          imagePullPolicy: IfNotPresent
          ports:
            - name: grpc
              containerPort: 9090
              protocol: TCP
          envFrom:
            - configMapRef:
                name: akidb-config
          volumeMounts:
            - name: akidb-data
              mountPath: /data/akidb
            - name: akidb-logs
              mountPath: /var/log/akidb
          livenessProbe:
            exec:
              command:
                - grpcurl
                - -plaintext
                - localhost:9090
                - grpc.health.v1.Health/Check
            initialDelaySeconds: 30
            periodSeconds: 10
            timeoutSeconds: 5
            failureThreshold: 3
          readinessProbe:
            exec:
              command:
                - grpcurl
                - -plaintext
                - localhost:9090
                - grpc.health.v1.Health/Check
            initialDelaySeconds: 10
            periodSeconds: 5
            timeoutSeconds: 3
            failureThreshold: 3
          resources:
            requests:
              cpu: 500m
              memory: 1Gi
            limits:
              cpu: 2000m
              memory: 4Gi
          securityContext:
            runAsNonRoot: true
            runAsUser: 1000
            allowPrivilegeEscalation: false
            readOnlyRootFilesystem: false
      volumes:
        - name: akidb-data
          persistentVolumeClaim:
            claimName: akidb-pvc
        - name: akidb-logs
          emptyDir: {}
      restartPolicy: Always
```

#### 5. REST Deployment

Create `k8s/deployment-rest.yaml`:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: akidb-rest
  namespace: akidb
  labels:
    app: akidb-rest
    component: rest-server
spec:
  replicas: 2
  selector:
    matchLabels:
      app: akidb-rest
  template:
    metadata:
      labels:
        app: akidb-rest
        component: rest-server
    spec:
      affinity:
        podAntiAffinity:
          preferredDuringSchedulingIgnoredDuringExecution:
            - weight: 100
              podAffinityTerm:
                labelSelector:
                  matchExpressions:
                    - key: app
                      operator: In
                      values:
                        - akidb-rest
                topologyKey: kubernetes.io/hostname
      containers:
        - name: akidb-rest
          image: akidb/akidb-rest:2.0.0-rc1
          imagePullPolicy: IfNotPresent
          ports:
            - name: http
              containerPort: 8080
              protocol: TCP
            - name: metrics
              containerPort: 8080
              protocol: TCP
          envFrom:
            - configMapRef:
                name: akidb-config
          volumeMounts:
            - name: akidb-data
              mountPath: /data/akidb
            - name: akidb-logs
              mountPath: /var/log/akidb
          livenessProbe:
            httpGet:
              path: /health
              port: 8080
            initialDelaySeconds: 30
            periodSeconds: 10
            timeoutSeconds: 5
            failureThreshold: 3
          readinessProbe:
            httpGet:
              path: /health
              port: 8080
            initialDelaySeconds: 10
            periodSeconds: 5
            timeoutSeconds: 3
            failureThreshold: 3
          resources:
            requests:
              cpu: 500m
              memory: 1Gi
            limits:
              cpu: 2000m
              memory: 4Gi
          securityContext:
            runAsNonRoot: true
            runAsUser: 1000
            allowPrivilegeEscalation: false
            readOnlyRootFilesystem: false
      volumes:
        - name: akidb-data
          persistentVolumeClaim:
            claimName: akidb-pvc
        - name: akidb-logs
          emptyDir: {}
      restartPolicy: Always
```

#### 6. Services

Create `k8s/service-grpc.yaml`:

```yaml
apiVersion: v1
kind: Service
metadata:
  name: akidb-grpc
  namespace: akidb
  labels:
    app: akidb-grpc
spec:
  type: ClusterIP
  ports:
    - port: 9090
      targetPort: 9090
      protocol: TCP
      name: grpc
  selector:
    app: akidb-grpc
```

Create `k8s/service-rest.yaml`:

```yaml
apiVersion: v1
kind: Service
metadata:
  name: akidb-rest
  namespace: akidb
  labels:
    app: akidb-rest
spec:
  type: ClusterIP
  ports:
    - port: 8080
      targetPort: 8080
      protocol: TCP
      name: http
  selector:
    app: akidb-rest
```

#### 7. ServiceMonitor (Prometheus Operator)

Create `k8s/servicemonitor.yaml`:

```yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: akidb-metrics
  namespace: akidb
  labels:
    app: akidb
spec:
  selector:
    matchLabels:
      app: akidb-rest
  endpoints:
    - port: http
      path: /metrics
      interval: 30s
```

#### 8. Ingress (Optional)

Create `k8s/ingress.yaml`:

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: akidb-ingress
  namespace: akidb
  annotations:
    nginx.ingress.kubernetes.io/rewrite-target: /
    cert-manager.io/cluster-issuer: letsencrypt-prod
spec:
  ingressClassName: nginx
  tls:
    - hosts:
        - akidb.example.com
      secretName: akidb-tls
  rules:
    - host: akidb.example.com
      http:
        paths:
          - path: /api
            pathType: Prefix
            backend:
              service:
                name: akidb-rest
                port:
                  number: 8080
          - path: /grpc
            pathType: Prefix
            backend:
              service:
                name: akidb-grpc
                port:
                  number: 9090
```

### Horizontal Pod Autoscaler (Optional)

Create `k8s/hpa.yaml`:

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: akidb-grpc-hpa
  namespace: akidb
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: akidb-grpc
  minReplicas: 2
  maxReplicas: 10
  metrics:
    - type: Resource
      resource:
        name: cpu
        target:
          type: Utilization
          averageUtilization: 70
    - type: Resource
      resource:
        name: memory
        target:
          type: Utilization
          averageUtilization: 80
---
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: akidb-rest-hpa
  namespace: akidb
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: akidb-rest
  minReplicas: 2
  maxReplicas: 10
  metrics:
    - type: Resource
      resource:
        name: cpu
        target:
          type: Utilization
          averageUtilization: 70
    - type: Resource
      resource:
        name: memory
        target:
          type: Utilization
          averageUtilization: 80
```

### Deployment Commands

```bash
# Deploy all resources
kubectl apply -f k8s/

# Check deployment status
kubectl get all -n akidb

# View logs
kubectl logs -f -n akidb -l app=akidb-grpc
kubectl logs -f -n akidb -l app=akidb-rest

# Scale deployments
kubectl scale deployment akidb-grpc --replicas=5 -n akidb

# Update image
kubectl set image deployment/akidb-grpc akidb-grpc=akidb/akidb-grpc:2.0.1 -n akidb

# Rollback deployment
kubectl rollout undo deployment/akidb-grpc -n akidb

# Delete all resources
kubectl delete -f k8s/
```

---

## Bare Metal Deployment

### Prerequisites

- Linux server (Ubuntu 22.04 LTS recommended)
- Rust 1.75+ installed
- SQLite 3.35+ installed
- systemd for service management
- Firewall (ufw, iptables, or firewalld)

### Installation Steps

#### 1. Build Binaries

```bash
# Clone repository
git clone https://github.com/your-org/akidb2.git
cd akidb2

# Build release binaries
cargo build --release --workspace

# Binaries will be at:
# - target/release/akidb-grpc
# - target/release/akidb-rest
```

#### 2. Create System User

```bash
# Create dedicated user
sudo useradd -r -s /bin/false -d /var/lib/akidb akidb

# Create directories
sudo mkdir -p /var/lib/akidb/data
sudo mkdir -p /var/log/akidb
sudo mkdir -p /etc/akidb

# Set ownership
sudo chown -R akidb:akidb /var/lib/akidb
sudo chown -R akidb:akidb /var/log/akidb
```

#### 3. Install Binaries

```bash
# Copy binaries
sudo cp target/release/akidb-grpc /usr/local/bin/
sudo cp target/release/akidb-rest /usr/local/bin/

# Set permissions
sudo chmod 755 /usr/local/bin/akidb-grpc
sudo chmod 755 /usr/local/bin/akidb-rest
```

#### 4. Create Configuration

```bash
# Copy example config
sudo cp config.example.toml /etc/akidb/config.toml

# Edit configuration
sudo nano /etc/akidb/config.toml
```

**Production Configuration (`/etc/akidb/config.toml`):**

```toml
[server]
host = "0.0.0.0"
rest_port = 8080
grpc_port = 9090
timeout_seconds = 30

[database]
path = "sqlite:///var/lib/akidb/data/metadata.db"
max_connections = 10
connection_timeout_seconds = 5

[features]
metrics_enabled = true
vector_persistence_enabled = true
auto_initialize = true

[hnsw]
m = 32
ef_construction = 200
threshold = 10000

[logging]
level = "info"
format = "json"
```

#### 5. Create systemd Service Files

**gRPC Server** (`/etc/systemd/system/akidb-grpc.service`):

```ini
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
MemoryMax=4G
CPUQuota=200%

[Install]
WantedBy=multi-user.target
```

**REST Server** (`/etc/systemd/system/akidb-rest.service`):

```ini
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
MemoryMax=4G
CPUQuota=200%

[Install]
WantedBy=multi-user.target
```

#### 6. Enable and Start Services

```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable services
sudo systemctl enable akidb-grpc
sudo systemctl enable akidb-rest

# Start services
sudo systemctl start akidb-grpc
sudo systemctl start akidb-rest

# Check status
sudo systemctl status akidb-grpc
sudo systemctl status akidb-rest

# View logs
sudo journalctl -u akidb-grpc -f
sudo journalctl -u akidb-rest -f
```

#### 7. Configure Firewall

**Using ufw:**

```bash
# Allow gRPC port
sudo ufw allow 9090/tcp

# Allow REST port
sudo ufw allow 8080/tcp

# Enable firewall
sudo ufw enable
```

**Using iptables:**

```bash
# Allow gRPC
sudo iptables -A INPUT -p tcp --dport 9090 -j ACCEPT

# Allow REST
sudo iptables -A INPUT -p tcp --dport 8080 -j ACCEPT

# Save rules
sudo iptables-save > /etc/iptables/rules.v4
```

#### 8. Setup Log Rotation

Create `/etc/logrotate.d/akidb`:

```
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
```

Test logrotate:

```bash
sudo logrotate -d /etc/logrotate.d/akidb
sudo logrotate -f /etc/logrotate.d/akidb
```

---

## Configuration Reference

### Environment Variables

All configuration options can be overridden using environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `AKIDB_CONFIG` | Path to config.toml file | `./config.toml` |
| `AKIDB_HOST` | Server bind address | `0.0.0.0` |
| `AKIDB_REST_PORT` | REST API port | `8080` |
| `AKIDB_GRPC_PORT` | gRPC API port | `9090` |
| `AKIDB_DB_PATH` | SQLite database path | `sqlite://akidb.db` |
| `AKIDB_LOG_LEVEL` | Log level (trace/debug/info/warn/error) | `info` |
| `AKIDB_LOG_FORMAT` | Log format (json/pretty) | `pretty` |
| `AKIDB_METRICS_ENABLED` | Enable metrics endpoint | `true` |
| `AKIDB_VECTOR_PERSISTENCE_ENABLED` | Enable vector persistence | `true` |
| `AKIDB_AUTO_INITIALIZE` | Auto-create default tenant/database | `true` |
| `RUST_LOG` | Rust tracing filter | `info` |

### Configuration File (config.toml)

See `config.example.toml` for full reference. Key sections:

**Server Configuration:**
```toml
[server]
host = "0.0.0.0"
rest_port = 8080
grpc_port = 9090
timeout_seconds = 30
```

**Database Configuration:**
```toml
[database]
path = "sqlite://akidb.db"
max_connections = 10
connection_timeout_seconds = 5
```

**Feature Flags:**
```toml
[features]
metrics_enabled = true
vector_persistence_enabled = true
auto_initialize = true
```

**HNSW Tuning:**
```toml
[hnsw]
m = 32                    # Higher = better recall, more memory
ef_construction = 200     # Higher = better quality, slower build
threshold = 10000         # Min docs to use HNSW (vs brute-force)
```

**Logging:**
```toml
[logging]
level = "info"           # trace, debug, info, warn, error
format = "json"          # json (production) or pretty (development)
```

---

## S3/MinIO Storage Configuration

AkiDB 2.0 supports three tiering policies for vector storage with different performance/cost trade-offs:

### Tiering Policies Overview

#### 1. Memory (Default)
- **Description:** RAM-first with WAL durability
- **Use Case:** Single-node deployments, <100GB datasets
- **Performance:** P95 <2ms insert/query
- **Durability:** Local WAL only (ephemeral on disk loss)
- **Pros:** Fastest, simplest
- **Cons:** No remote backup

```toml
[storage]
tiering_policy = "Memory"
wal_path = "/var/lib/akidb/wal"
snapshot_dir = "/var/lib/akidb/snapshots"
```

#### 2. MemoryS3 (Recommended for Production)
- **Description:** RAM-first with async S3 backup
- **Use Case:** Production deployments with backup requirement
- **Performance:** P95 <3ms insert/query
- **Durability:** S3 backup (11-nines durability)
- **Pros:** Fast, automatic backup, disaster recovery
- **Cons:** S3 costs, eventual consistency

```toml
[storage]
tiering_policy = "MemoryS3"
wal_path = "/var/lib/akidb/wal"
snapshot_dir = "/var/lib/akidb/snapshots"
s3_bucket = "s3://my-bucket/akidb"
s3_region = "us-west-2"

# AWS credentials (via environment variables)
# AWS_ACCESS_KEY_ID=xxx
# AWS_SECRET_ACCESS_KEY=xxx
```

#### 3. S3Only
- **Description:** S3 as source of truth with LRU cache
- **Use Case:** Large datasets (>100GB), cost-optimized
- **Performance:** P95 <5ms query (cache hit), <50ms (cache miss)
- **Durability:** S3 primary storage
- **Pros:** Scales beyond RAM, predictable costs
- **Cons:** Slower on cache miss

```toml
[storage]
tiering_policy = "S3Only"
s3_bucket = "s3://my-bucket/akidb"
s3_region = "us-west-2"
cache_size = 10000  # Number of vectors to cache (default: 10k)
```

### S3/MinIO Setup Instructions

#### AWS S3

**Create S3 Bucket:**
```bash
# Create bucket
aws s3 mb s3://my-akidb-bucket --region us-west-2

# Set lifecycle policy (optional, for old snapshots)
cat > lifecycle.json <<EOF
{
  "Rules": [{
    "Id": "DeleteOldSnapshots",
    "Status": "Enabled",
    "Prefix": "snapshots/",
    "Expiration": {"Days": 30}
  }]
}
EOF

aws s3api put-bucket-lifecycle-configuration \
  --bucket my-akidb-bucket \
  --lifecycle-configuration file://lifecycle.json
```

**IAM Policy Requirements:**
```json
{
  "Version": "2012-10-17",
  "Statement": [{
    "Effect": "Allow",
    "Action": [
      "s3:PutObject",
      "s3:GetObject",
      "s3:DeleteObject",
      "s3:ListBucket"
    ],
    "Resource": [
      "arn:aws:s3:::my-akidb-bucket",
      "arn:aws:s3:::my-akidb-bucket/*"
    ]
  }]
}
```

**Environment Variables:**
```bash
export AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE
export AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
export AWS_DEFAULT_REGION=us-west-2
```

#### MinIO (Self-Hosted)

**Start MinIO Server:**
```bash
# Using Docker
docker run -d \
  -p 9000:9000 \
  -p 9001:9001 \
  -e MINIO_ROOT_USER=admin \
  -e MINIO_ROOT_PASSWORD=password \
  -v /data/minio:/data \
  --name minio \
  minio/minio server /data --console-address ":9001"

# Create bucket using mc (MinIO Client)
docker exec -it minio mc alias set myminio http://localhost:9000 admin password
docker exec -it minio mc mb myminio/akidb
```

**AkiDB Config for MinIO:**
```toml
[storage]
tiering_policy = "MemoryS3"
s3_bucket = "http://localhost:9000/akidb"
s3_region = "us-east-1"  # Required but ignored by MinIO
s3_endpoint = "http://localhost:9000"

# MinIO credentials (environment variables)
# AWS_ACCESS_KEY_ID=admin
# AWS_SECRET_ACCESS_KEY=password
```

### Background Workers Configuration

```toml
[storage]
enable_background_compaction = true  # Default: true

[storage.compaction]
threshold_bytes = 104857600  # 100MB (default)
threshold_ops = 10000         # 10k operations (default)

[storage.retry]
max_retries = 5              # Default: 5
base_backoff_secs = 1        # Default: 1s
max_backoff_secs = 64        # Default: 64s
```

### Monitoring Metrics

**Prometheus Metrics:**
```promql
# Insert operations
storage_inserts{collection_id="..."}

# S3 uploads
storage_s3_uploads{collection_id="..."}
storage_s3_retries{collection_id="..."}
storage_s3_permanent_failures{collection_id="..."}

# Compaction
storage_compactions{collection_id="..."}
storage_last_snapshot_timestamp{collection_id="..."}

# Cache (S3Only policy)
storage_cache_hits{collection_id="..."}
storage_cache_misses{collection_id="..."}

# Dead Letter Queue
storage_dlq_size{collection_id="..."}
```

**Grafana Dashboard Queries:**
```promql
# Insert throughput (ops/sec)
rate(storage_inserts[5m])

# S3 upload success rate
rate(storage_s3_uploads[5m]) / (rate(storage_s3_uploads[5m]) + rate(storage_s3_permanent_failures[5m]))

# Cache hit rate (S3Only)
storage_cache_hits / (storage_cache_hits + storage_cache_misses)

# Compaction frequency
rate(storage_compactions[1h])
```

**Alert Rules:**
```yaml
groups:
  - name: akidb_storage
    rules:
      - alert: HighDLQSize
        expr: storage_dlq_size > 100
        for: 5m
        annotations:
          summary: "Dead Letter Queue size > 100"
          description: "Check S3 credentials and connectivity"

      - alert: LowS3SuccessRate
        expr: rate(storage_s3_uploads[5m]) / (rate(storage_s3_uploads[5m]) + rate(storage_s3_permanent_failures[5m])) < 0.95
        for: 10m
        annotations:
          summary: "S3 upload success rate < 95%"

      - alert: LowCacheHitRate
        expr: storage_cache_hits / (storage_cache_hits + storage_cache_misses) < 0.5
        for: 15m
        annotations:
          summary: "Cache hit rate < 50% (S3Only policy)"
          description: "Consider increasing cache_size"
```

### Troubleshooting

#### High DLQ Size

**Symptom:** `storage_dlq_size` > 100

**Diagnosis:**
```bash
# Inspect DLQ entries
curl http://localhost:8080/admin/collections/{collection_id}/dlq
```

**Common Causes:**
- Invalid S3 credentials (403 Forbidden)
- S3 bucket not found (404 Not Found)
- Network connectivity issues

**Resolution:**
1. Check S3 credentials: `aws sts get-caller-identity`
2. Verify bucket exists: `aws s3 ls s3://my-bucket`
3. Check IAM permissions
4. Retry DLQ entries (after fixing root cause):
   ```bash
   curl -X POST http://localhost:8080/admin/collections/{collection_id}/dlq/retry
   ```
5. Clear DLQ if entries are unrecoverable:
   ```bash
   curl -X DELETE http://localhost:8080/admin/collections/{collection_id}/dlq
   ```

#### Slow Inserts with MemoryS3

**Symptom:** Insert P95 > 10ms

**Diagnosis:**
```bash
# Check S3 upload queue size
curl http://localhost:8080/metrics | grep storage_s3_upload_queue_size
```

**Common Causes:**
- S3 network latency (cross-region uploads)
- Background upload worker saturated

**Resolution:**
- Switch to `TieringPolicy::Memory` (no S3 overhead)
- Increase S3 upload batch size
- Use regional S3 bucket (reduce latency)
- Check S3 endpoint configuration (MinIO)

#### Compaction Not Running

**Symptom:** `storage_compactions` not incrementing

**Diagnosis:**
```bash
# Check WAL size
ls -lh /var/lib/akidb/wal/

# Check compaction thresholds
cat config.toml | grep threshold
```

**Resolution:**
- Lower `threshold_bytes` or `threshold_ops` in config.toml
- Manually trigger compaction:
  ```bash
  curl -X POST http://localhost:8080/admin/collections/{collection_id}/compact
  ```
- Verify `enable_background_compaction = true`

#### S3 Connection Errors

**Symptom:** S3 uploads failing with connection errors

**Diagnosis:**
```bash
# Test S3 connectivity
aws s3 ls s3://my-bucket --region us-west-2

# Check MinIO endpoint
curl http://localhost:9000/minio/health/live
```

**Resolution:**
- Verify S3 endpoint URL in config
- Check network connectivity (VPC, firewalls)
- For MinIO: verify container is running
- Check S3 region matches bucket region

---

## Production Checklist

### Pre-Deployment

- [ ] Build release binaries with optimizations (`--release`)
- [ ] Run full test suite (`cargo test --workspace`)
- [ ] Review and customize `config.toml` for environment
- [ ] Set up persistent volume/storage (minimum 100GB)
- [ ] Configure log aggregation (ELK, Splunk, CloudWatch)
- [ ] Set up monitoring (Prometheus + Grafana)
- [ ] Configure backup strategy (see Backup section)
- [ ] Review security hardening (see Security section)
- [ ] Set resource limits (CPU, memory, file descriptors)
- [ ] Configure firewall rules
- [ ] Set up SSL/TLS termination (nginx, Envoy, ALB)
- [ ] Document deployment architecture
- [ ] Create runbook for common operations

### Post-Deployment

- [ ] Verify health endpoints respond
- [ ] Check Prometheus metrics endpoint (`/metrics`)
- [ ] Verify auto-initialization created default tenant/database
- [ ] Test creating/querying a collection
- [ ] Verify vector persistence (restart server, check data)
- [ ] Monitor resource usage (CPU, memory, disk I/O)
- [ ] Set up alerts (disk space, memory, error rates)
- [ ] Test graceful shutdown (SIGTERM)
- [ ] Verify backup/restore procedures
- [ ] Load test with expected traffic patterns
- [ ] Document deployed version and configuration

---

## Security Hardening

### Network Security

**1. Use TLS/SSL:**
- Terminate TLS at load balancer (recommended)
- Or use reverse proxy (nginx, Envoy)
- Never expose plain HTTP/gRPC in production

**2. Firewall Rules:**
```bash
# Allow only from specific IPs/ranges
sudo ufw allow from 10.0.0.0/8 to any port 9090
sudo ufw allow from 10.0.0.0/8 to any port 8080
```

**3. Network Policies (Kubernetes):**
```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: akidb-network-policy
  namespace: akidb
spec:
  podSelector:
    matchLabels:
      app: akidb-grpc
  policyTypes:
    - Ingress
    - Egress
  ingress:
    - from:
        - namespaceSelector:
            matchLabels:
              name: application
      ports:
        - protocol: TCP
          port: 9090
  egress:
    - to:
        - namespaceSelector:
            matchLabels:
              name: kube-system
      ports:
        - protocol: TCP
          port: 53
```

### File System Security

**1. Restrict Permissions:**
```bash
# Database files
sudo chmod 600 /var/lib/akidb/data/metadata.db
sudo chown akidb:akidb /var/lib/akidb/data/metadata.db

# Config files
sudo chmod 640 /etc/akidb/config.toml
sudo chown root:akidb /etc/akidb/config.toml
```

**2. Read-Only Root Filesystem (Docker):**
```yaml
securityContext:
  readOnlyRootFilesystem: true
volumeMounts:
  - name: tmp
    mountPath: /tmp
  - name: data
    mountPath: /data/akidb
```

### Process Security

**1. Run as Non-Root User:**
- Docker: `USER akidb` (already configured)
- systemd: `User=akidb` (already configured)
- Kubernetes: `runAsNonRoot: true` (already configured)

**2. Limit Capabilities:**
```yaml
securityContext:
  allowPrivilegeEscalation: false
  capabilities:
    drop:
      - ALL
```

**3. Resource Limits:**
```yaml
resources:
  limits:
    cpu: "2"
    memory: "4Gi"
  requests:
    cpu: "500m"
    memory: "1Gi"
```

### Data Security

**1. Encrypt Data at Rest:**
- Use encrypted volumes (LUKS, dm-crypt)
- Or cloud provider encryption (AWS EBS encryption, GCP persistent disk encryption)

**2. Encrypt Data in Transit:**
- Always use TLS for client connections
- Use mTLS for service-to-service communication

**3. Secure Backups:**
- Encrypt backup files before storing
- Use separate credentials for backup access
- Test restore procedures regularly

### Secrets Management

**1. Never commit secrets to version control**

**2. Use environment variables or secret managers:**
```bash
# Kubernetes Secrets
kubectl create secret generic akidb-secrets \
  --from-literal=db-password=<password> \
  -n akidb
```

**3. Rotate credentials regularly**

---

## Monitoring and Observability

### Metrics Endpoint

AkiDB exposes Prometheus metrics at `/metrics` (REST server):

```bash
curl http://localhost:8080/metrics
```

**Key Metrics:**
- `akidb_requests_total`: Total requests by endpoint
- `akidb_request_duration_seconds`: Request latency histogram
- `akidb_collections_total`: Number of collections
- `akidb_vectors_total`: Total vectors indexed
- `akidb_search_latency_seconds`: Vector search latency
- `akidb_index_build_duration_seconds`: HNSW index build time

### Prometheus Configuration

Add to `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'akidb'
    scrape_interval: 30s
    static_configs:
      - targets:
          - 'localhost:8080'
    metrics_path: '/metrics'
```

### Grafana Dashboard

Import the AkiDB dashboard (JSON available in `monitoring/grafana-dashboard.json`):

**Key Panels:**
- Request rate and error rate
- P50/P95/P99 latency
- Collection and vector counts
- Index build times
- Resource usage (CPU, memory)

### Alerting Rules

Create `akidb-alerts.yml`:

```yaml
groups:
  - name: akidb
    interval: 30s
    rules:
      - alert: AkiDBHighErrorRate
        expr: rate(akidb_requests_total{status="error"}[5m]) > 0.05
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High error rate detected"

      - alert: AkiDBHighLatency
        expr: histogram_quantile(0.95, akidb_request_duration_seconds) > 0.1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "P95 latency above 100ms"

      - alert: AkiDBDiskSpaceLow
        expr: node_filesystem_avail_bytes{mountpoint="/data/akidb"} < 10737418240
        for: 10m
        labels:
          severity: critical
        annotations:
          summary: "Less than 10GB disk space remaining"

      - alert: AkiDBMemoryHigh
        expr: container_memory_usage_bytes{pod=~"akidb-.*"} / container_spec_memory_limit_bytes > 0.9
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Memory usage above 90%"
```

### Logging

**JSON Logging (Production):**
```toml
[logging]
level = "info"
format = "json"
```

**Structured Log Fields:**
- `timestamp`: ISO-8601 timestamp
- `level`: Log level (ERROR, WARN, INFO, DEBUG, TRACE)
- `target`: Rust module path
- `message`: Log message
- `fields`: Additional context (request_id, collection_id, etc.)

**Log Aggregation:**

Configure log shipping to:
- ELK Stack (Elasticsearch, Logstash, Kibana)
- Splunk
- Datadog
- AWS CloudWatch Logs
- GCP Cloud Logging

**Example Fluentd Configuration:**
```yaml
<source>
  @type tail
  path /var/log/akidb/*.log
  pos_file /var/log/akidb/fluentd.pos
  tag akidb
  <parse>
    @type json
    time_key timestamp
    time_format %Y-%m-%dT%H:%M:%S.%NZ
  </parse>
</source>

<match akidb>
  @type elasticsearch
  host elasticsearch.example.com
  port 9200
  index_name akidb-%Y.%m.%d
  type_name akidb
</match>
```

---

## Backup and Disaster Recovery

### Backup Strategy

**What to Backup:**
1. SQLite metadata database (`/data/akidb/metadata.db`)
2. SQLite WAL files (`/data/akidb/metadata.db-wal`, `/data/akidb/metadata.db-shm`)
3. Configuration files (`/etc/akidb/config.toml`)

**Backup Frequency:**
- Production: Every 6 hours + before deployments
- Development: Daily

### Manual Backup

**SQLite Online Backup:**
```bash
#!/bin/bash
# backup-akidb.sh

BACKUP_DIR="/backup/akidb/$(date +%Y%m%d-%H%M%S)"
DB_PATH="/var/lib/akidb/data/metadata.db"

mkdir -p "$BACKUP_DIR"

# Online backup using SQLite .backup command
sqlite3 "$DB_PATH" ".backup '$BACKUP_DIR/metadata.db'"

# Backup configuration
cp /etc/akidb/config.toml "$BACKUP_DIR/"

# Compress
tar -czf "$BACKUP_DIR.tar.gz" -C "$(dirname $BACKUP_DIR)" "$(basename $BACKUP_DIR)"
rm -rf "$BACKUP_DIR"

# Encrypt (optional)
gpg --encrypt --recipient backup@example.com "$BACKUP_DIR.tar.gz"

echo "Backup completed: $BACKUP_DIR.tar.gz"
```

**Automated Backup (cron):**
```bash
# Edit crontab
sudo crontab -e

# Add backup job (every 6 hours)
0 */6 * * * /usr/local/bin/backup-akidb.sh >> /var/log/akidb/backup.log 2>&1
```

### Docker/Kubernetes Backup

**Docker:**
```bash
# Backup volume
docker run --rm \
  -v akidb-data:/data \
  -v /backup:/backup \
  alpine tar czf /backup/akidb-$(date +%Y%m%d-%H%M%S).tar.gz /data
```

**Kubernetes:**
```bash
# Create VolumeSnapshot (requires CSI driver)
kubectl apply -f - <<EOF
apiVersion: snapshot.storage.k8s.io/v1
kind: VolumeSnapshot
metadata:
  name: akidb-snapshot-$(date +%Y%m%d-%H%M%S)
  namespace: akidb
spec:
  volumeSnapshotClassName: csi-snapclass
  source:
    persistentVolumeClaimName: akidb-pvc
EOF
```

### Restore Procedure

**1. Stop Services:**
```bash
# systemd
sudo systemctl stop akidb-grpc akidb-rest

# Docker
docker-compose down

# Kubernetes
kubectl scale deployment akidb-grpc akidb-rest --replicas=0 -n akidb
```

**2. Restore Database:**
```bash
# Extract backup
tar -xzf /backup/akidb-20250107-120000.tar.gz -C /tmp

# Restore database
sudo cp /tmp/akidb-20250107-120000/metadata.db /var/lib/akidb/data/
sudo chown akidb:akidb /var/lib/akidb/data/metadata.db
sudo chmod 600 /var/lib/akidb/data/metadata.db
```

**3. Verify Integrity:**
```bash
sqlite3 /var/lib/akidb/data/metadata.db "PRAGMA integrity_check;"
# Should output: ok
```

**4. Restart Services:**
```bash
# systemd
sudo systemctl start akidb-grpc akidb-rest

# Docker
docker-compose up -d

# Kubernetes
kubectl scale deployment akidb-grpc akidb-rest --replicas=2 -n akidb
```

**5. Verify:**
```bash
# Check health
curl http://localhost:8080/health

# List collections
curl http://localhost:8080/api/v1/collections
```

### Disaster Recovery Planning

**RTO (Recovery Time Objective):** < 30 minutes
**RPO (Recovery Point Objective):** < 6 hours

**DR Checklist:**
- [ ] Maintain off-site backups (different region/datacenter)
- [ ] Test restore procedures quarterly
- [ ] Document recovery steps in runbook
- [ ] Keep inventory of backup locations and credentials
- [ ] Automate backup verification
- [ ] Set up backup monitoring and alerts
- [ ] Maintain restore scripts and tools
- [ ] Train team on recovery procedures

---

## Troubleshooting

### Common Issues

#### 1. Server Won't Start

**Symptoms:**
- Service fails to start
- "Address already in use" error

**Solutions:**
```bash
# Check if port is in use
sudo lsof -i :8080
sudo lsof -i :9090

# Kill process using port
sudo kill -9 <PID>

# Or change port in config
# Edit /etc/akidb/config.toml
```

#### 2. Database Locked

**Symptoms:**
- "Database is locked" errors
- Write operations fail

**Solutions:**
```bash
# Check for WAL mode
sqlite3 /var/lib/akidb/data/metadata.db "PRAGMA journal_mode;"
# Should output: wal

# Enable WAL if not enabled
sqlite3 /var/lib/akidb/data/metadata.db "PRAGMA journal_mode=WAL;"

# Check for stale lock files
ls -la /var/lib/akidb/data/
# Remove .db-shm and .db-wal if server is stopped

# Checkpoint WAL
sqlite3 /var/lib/akidb/data/metadata.db "PRAGMA wal_checkpoint(TRUNCATE);"
```

#### 3. High Memory Usage

**Symptoms:**
- Memory usage grows continuously
- OOM kills

**Solutions:**
```bash
# Check collection sizes
curl http://localhost:8080/api/v1/collections

# Reduce HNSW parameters in config.toml
[hnsw]
m = 16                   # Lower = less memory
ef_construction = 100    # Lower = less memory

# Increase threshold for brute-force
threshold = 20000        # Higher = fewer HNSW indexes

# Restart service
sudo systemctl restart akidb-grpc akidb-rest
```

#### 4. Slow Queries

**Symptoms:**
- High P95/P99 latency
- Timeout errors

**Solutions:**
```bash
# Check metrics
curl http://localhost:8080/metrics | grep latency

# Increase HNSW quality (slower build, faster search)
[hnsw]
ef_construction = 400    # Higher = better quality

# Check disk I/O (SQLite bottleneck)
iostat -x 1

# Consider using faster storage (NVMe SSD)
```

#### 5. Health Check Failures

**Symptoms:**
- Docker/K8s restarts containers frequently
- Health endpoint returns errors

**Solutions:**
```bash
# Check logs
sudo journalctl -u akidb-rest -n 100

# Test health endpoint manually
curl -v http://localhost:8080/health

# Increase health check timeouts
# In docker-compose.yaml or K8s manifest:
healthcheck:
  timeout: 10s
  retries: 5
  start_period: 30s
```

### Log Analysis

**Error Patterns:**

| Log Message | Cause | Solution |
|-------------|-------|----------|
| `Failed to connect to database` | SQLite file missing/permissions | Check file exists, fix ownership |
| `Address already in use` | Port conflict | Change port or kill conflicting process |
| `Out of memory` | Insufficient RAM | Reduce vector count or add more memory |
| `Database is locked` | Concurrent writes without WAL | Enable WAL mode |
| `HNSW build timeout` | Index too large | Increase timeout or reduce ef_construction |

**Debug Logging:**
```bash
# Enable debug logs
export RUST_LOG=debug
sudo systemctl restart akidb-grpc

# Or in config.toml
[logging]
level = "debug"
```

### Performance Tuning

**1. SQLite Optimization:**
```sql
-- Analyze query performance
EXPLAIN QUERY PLAN SELECT * FROM collections WHERE database_id = ?;

-- Rebuild indexes
VACUUM;
ANALYZE;
```

**2. HNSW Tuning:**
```toml
# For better recall (slower)
[hnsw]
m = 64
ef_construction = 400

# For faster search (lower recall)
[hnsw]
m = 16
ef_construction = 100
```

**3. Connection Pooling:**
```toml
[database]
max_connections = 20        # Increase for high concurrency
connection_timeout_seconds = 10
```

### Getting Help

**1. Check Documentation:**
- [Quickstart Guide](/docs/QUICKSTART.md)
- [Migration Guide](/docs/MIGRATION-V1-TO-V2.md)
- [Architecture Docs](/automatosx/PRD/)

**2. Review Logs:**
```bash
# systemd
sudo journalctl -u akidb-grpc -f

# Docker
docker logs -f akidb-grpc

# Kubernetes
kubectl logs -f -n akidb -l app=akidb-grpc
```

**3. Community Support:**
- GitHub Issues: https://github.com/your-org/akidb2/issues
- Discussions: https://github.com/your-org/akidb2/discussions
- Slack/Discord: (link to community chat)

**4. Professional Support:**
- Email: support@example.com
- SLA response times for paid support tiers

---

## Appendix

### Resource Recommendations

| Deployment Size | CPU | Memory | Disk | Replicas |
|----------------|-----|--------|------|----------|
| Small (< 1M vectors) | 1-2 cores | 2-4 GB | 50 GB | 1-2 |
| Medium (1-10M vectors) | 2-4 cores | 4-8 GB | 100 GB | 2-3 |
| Large (10-50M vectors) | 4-8 cores | 8-16 GB | 200 GB | 3-5 |
| XLarge (50M+ vectors) | 8-16 cores | 16-32 GB | 500 GB | 5-10 |

### Port Reference

| Service | Port | Protocol | Purpose |
|---------|------|----------|---------|
| REST API | 8080 | HTTP | REST endpoints + /health + /metrics |
| gRPC API | 9090 | gRPC | gRPC endpoints + health check |

### File Locations

| Path | Description |
|------|-------------|
| `/etc/akidb/config.toml` | Main configuration file |
| `/var/lib/akidb/data/metadata.db` | SQLite database |
| `/var/log/akidb/` | Log files |
| `/usr/local/bin/akidb-grpc` | gRPC server binary |
| `/usr/local/bin/akidb-rest` | REST server binary |

---

**Last Updated:** 2025-01-07
**Version:** 2.0.0-rc1
**Maintainer:** AkiDB Team
