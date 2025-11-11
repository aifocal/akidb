# AkiDB Helm Chart

Official Helm chart for deploying AkiDB - an ARM-optimized vector database with tiered storage.

## TL;DR

```bash
helm repo add akidb https://charts.akidb.com
helm install akidb akidb/akidb
```

## Introduction

This chart bootstraps an AkiDB deployment on a Kubernetes cluster using the Helm package manager.

**Features:**
- StatefulSet for data persistence
- Horizontal Pod Autoscaler for automatic scaling
- Pod Disruption Budget for high availability
- Configurable resource limits and requests
- S3/MinIO tiered storage support
- Prometheus metrics integration
- Ingress support for external access

## Prerequisites

- Kubernetes 1.25+
- Helm 3.12+
- PV provisioner support in the underlying infrastructure (for persistence)
- S3-compatible storage (AWS S3, MinIO, etc.) for cold tier

## Installing the Chart

### Basic Installation

```bash
helm install akidb akidb/akidb
```

### Production Installation

```bash
helm install akidb akidb/akidb \
  --namespace akidb \
  --create-namespace \
  --set replicaCount=5 \
  --set resources.requests.memory=8Gi \
  --set resources.limits.memory=16Gi \
  --set persistence.size=500Gi \
  --set config.coldTier.bucket=my-akidb-bucket \
  --set s3.accessKeyId=YOUR_ACCESS_KEY \
  --set s3.secretAccessKey=YOUR_SECRET_KEY
```

### With Custom Values File

```bash
helm install akidb akidb/akidb -f custom-values.yaml
```

## Uninstalling the Chart

```bash
helm uninstall akidb
```

This removes all Kubernetes resources associated with the chart and deletes the release.

**Note:** Persistent volumes are not automatically deleted. To delete them:

```bash
kubectl delete pvc -l app.kubernetes.io/name=akidb
```

## Configuration

### Common Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `replicaCount` | Number of AkiDB replicas | `3` |
| `image.repository` | AkiDB image repository | `ghcr.io/yourusername/akidb2` |
| `image.tag` | AkiDB image tag | `2.0.0` |
| `image.pullPolicy` | Image pull policy | `IfNotPresent` |

### Resource Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `resources.requests.memory` | Memory request | `4Gi` |
| `resources.requests.cpu` | CPU request | `2000m` |
| `resources.limits.memory` | Memory limit | `8Gi` |
| `resources.limits.cpu` | CPU limit | `4000m` |

### Persistence Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `persistence.enabled` | Enable persistence | `true` |
| `persistence.storageClass` | Storage class | `fast-ssd` |
| `persistence.size` | Volume size | `100Gi` |
| `persistence.accessMode` | Access mode | `ReadWriteOnce` |

### Autoscaling Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `autoscaling.enabled` | Enable HPA | `true` |
| `autoscaling.minReplicas` | Minimum replicas | `3` |
| `autoscaling.maxReplicas` | Maximum replicas | `10` |
| `autoscaling.targetCPUUtilizationPercentage` | CPU target | `70` |
| `autoscaling.targetMemoryUtilizationPercentage` | Memory target | `80` |

### Tier Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `config.hotTier.maxMemoryBytes` | Hot tier RAM limit | `8589934592` (8GB) |
| `config.hotTier.maxCollections` | Hot tier collection limit | `1000` |
| `config.warmTier.path` | Warm tier path | `/data/warm` |
| `config.warmTier.maxSizeBytes` | Warm tier size limit | `107374182400` (100GB) |
| `config.coldTier.enabled` | Enable S3 cold tier | `true` |
| `config.coldTier.type` | Cold tier type | `s3` |
| `config.coldTier.bucket` | S3 bucket name | `akidb-cold` |
| `config.coldTier.region` | S3 region | `us-west-2` |

### S3 Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `s3.accessKeyId` | AWS access key ID | `""` |
| `s3.secretAccessKey` | AWS secret access key | `""` |
| `s3.sessionToken` | AWS session token (optional) | `""` |
| `s3.endpoint` | Custom endpoint (MinIO) | `""` |

### Ingress Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `ingress.enabled` | Enable ingress | `true` |
| `ingress.className` | Ingress class | `nginx` |
| `ingress.hosts[0].host` | Hostname | `akidb.example.com` |
| `ingress.tls[0].secretName` | TLS secret name | `akidb-tls` |

## Examples

### Minimal Configuration

```yaml
# minimal-values.yaml
replicaCount: 1
persistence:
  size: 50Gi
config:
  coldTier:
    enabled: false
autoscaling:
  enabled: false
```

```bash
helm install akidb akidb/akidb -f minimal-values.yaml
```

### Production Configuration

```yaml
# production-values.yaml
replicaCount: 5

resources:
  requests:
    memory: 16Gi
    cpu: 4000m
  limits:
    memory: 32Gi
    cpu: 8000m

persistence:
  storageClass: fast-nvme
  size: 1Ti

autoscaling:
  enabled: true
  minReplicas: 5
  maxReplicas: 20
  targetCPUUtilizationPercentage: 60
  targetMemoryUtilizationPercentage: 70

config:
  hotTier:
    maxMemoryBytes: 17179869184  # 16 GB
    maxCollections: 5000
  warmTier:
    maxSizeBytes: 1099511627776  # 1 TB
  coldTier:
    enabled: true
    bucket: akidb-production
    region: us-east-1

ingress:
  enabled: true
  hosts:
    - host: akidb.prod.example.com
      paths:
        - path: /
          pathType: Prefix
          backend: rest

serviceMonitor:
  enabled: true
  interval: 15s

affinity:
  nodeAffinity:
    requiredDuringSchedulingIgnoredDuringExecution:
      nodeSelectorTerms:
        - matchExpressions:
            - key: kubernetes.io/arch
              operator: In
              values:
                - arm64
```

```bash
helm install akidb akidb/akidb \
  -f production-values.yaml \
  --set s3.accessKeyId=$AWS_ACCESS_KEY_ID \
  --set s3.secretAccessKey=$AWS_SECRET_ACCESS_KEY
```

### MinIO Configuration

```yaml
# minio-values.yaml
config:
  coldTier:
    enabled: true
    type: s3
    bucket: akidb
    region: us-east-1  # Required but value doesn't matter for MinIO
    endpoint: http://minio.minio-system.svc.cluster.local:9000

s3:
  accessKeyId: minioadmin
  secretAccessKey: minioadmin123
```

## Post-Installation

### Verify Installation

```bash
# Check pod status
kubectl get pods -l app.kubernetes.io/name=akidb

# Check service
kubectl get svc -l app.kubernetes.io/name=akidb

# Check HPA
kubectl get hpa -l app.kubernetes.io/name=akidb

# Check PVC
kubectl get pvc -l app.kubernetes.io/name=akidb
```

### Access the API

```bash
# Port-forward for local access
kubectl port-forward svc/akidb 8080:8080 9090:9090

# REST API
curl http://localhost:8080/health

# gRPC API
grpcurl -plaintext localhost:9090 list
```

### View Logs

```bash
# All pods
kubectl logs -l app.kubernetes.io/name=akidb -f

# Specific pod
kubectl logs akidb-0 -f

# With timestamps
kubectl logs akidb-0 -f --timestamps
```

### View Metrics

```bash
# Port-forward
kubectl port-forward svc/akidb 8080:8080

# Fetch metrics
curl http://localhost:8080/metrics
```

## Upgrading

### Upgrade to New Version

```bash
helm repo update
helm upgrade akidb akidb/akidb --version 2.1.0
```

### Upgrade with Custom Values

```bash
helm upgrade akidb akidb/akidb -f custom-values.yaml
```

### Rollback

```bash
# View history
helm history akidb

# Rollback to previous version
helm rollback akidb

# Rollback to specific revision
helm rollback akidb 3
```

## Backup and Restore

### Backup SQLite Metadata

```bash
# Backup from pod
kubectl exec akidb-0 -- sqlite3 /data/metadata.db ".backup /tmp/backup.db"
kubectl cp akidb-0:/tmp/backup.db ./metadata-backup-$(date +%Y%m%d).db
```

### Backup Warm Tier

```bash
# Create VolumeSnapshot (requires CSI driver)
kubectl create volumesnapshot akidb-data-snapshot \
  --volumesnapshotclass=csi-snapshot-class \
  --source=data-akidb-0
```

### Restore from Backup

```bash
# 1. Scale down
kubectl scale statefulset akidb --replicas=0

# 2. Restore data
kubectl cp ./metadata-backup.db akidb-0:/data/metadata.db

# 3. Scale up
kubectl scale statefulset akidb --replicas=3
```

## Troubleshooting

### Pods Not Starting

```bash
# Check events
kubectl describe pod akidb-0

# Common issues:
# - Image pull errors: Check imagePullSecrets
# - PVC not binding: Check storage class
# - Resource limits: Check node capacity
```

### S3 Connection Errors

```bash
# Test S3 connectivity from pod
kubectl exec akidb-0 -- sh -c '
  export AWS_ACCESS_KEY_ID=...
  export AWS_SECRET_ACCESS_KEY=...
  aws s3 ls s3://your-bucket
'

# Check secret
kubectl get secret akidb-s3 -o yaml
```

### High Memory Usage

```bash
# Check metrics
kubectl top pods -l app.kubernetes.io/name=akidb

# Adjust hot tier limit
helm upgrade akidb akidb/akidb \
  --set config.hotTier.maxMemoryBytes=4294967296  # 4 GB
```

## Support

- Documentation: https://docs.akidb.com
- GitHub Issues: https://github.com/yourusername/akidb2/issues
- Discord: https://discord.gg/akidb

## License

Apache 2.0 License. See LICENSE file for details.
