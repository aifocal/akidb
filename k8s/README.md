# AkiDB Kubernetes Manifests

This directory contains production-ready Kubernetes manifests for deploying AkiDB.

## Quick Start

```bash
# Deploy all resources
kubectl apply -f .

# Check deployment status
kubectl get all -n akidb

# View logs
kubectl logs -f -n akidb -l app=akidb-grpc
kubectl logs -f -n akidb -l app=akidb-rest
```

## Manifest Files

| File | Description |
|------|-------------|
| `namespace.yaml` | Creates `akidb` namespace |
| `configmap.yaml` | Configuration environment variables |
| `persistentvolume.yaml` | PersistentVolume for SQLite storage |
| `persistentvolumeclaim.yaml` | PersistentVolumeClaim (100Gi) |
| `deployment-grpc.yaml` | gRPC server deployment (2 replicas) |
| `deployment-rest.yaml` | REST server deployment (2 replicas) |
| `service-grpc.yaml` | ClusterIP service for gRPC (port 9090) |
| `service-rest.yaml` | ClusterIP service for REST (port 8080) |
| `servicemonitor.yaml` | Prometheus ServiceMonitor (optional) |
| `ingress.yaml` | Ingress for external access (optional) |
| `hpa.yaml` | Horizontal Pod Autoscaler (optional) |
| `networkpolicy.yaml` | Network policies for security (optional) |

## Prerequisites

- Kubernetes cluster (v1.24+)
- kubectl configured
- Persistent volume provisioner
- Optional: Prometheus Operator (for ServiceMonitor)
- Optional: nginx-ingress-controller (for Ingress)
- Optional: metrics-server (for HPA)

## Deployment Order

1. **Core Resources** (required):
   ```bash
   kubectl apply -f namespace.yaml
   kubectl apply -f configmap.yaml
   kubectl apply -f persistentvolume.yaml
   kubectl apply -f persistentvolumeclaim.yaml
   kubectl apply -f deployment-grpc.yaml
   kubectl apply -f deployment-rest.yaml
   kubectl apply -f service-grpc.yaml
   kubectl apply -f service-rest.yaml
   ```

2. **Optional Resources**:
   ```bash
   # Prometheus monitoring
   kubectl apply -f servicemonitor.yaml

   # External access
   kubectl apply -f ingress.yaml

   # Auto-scaling
   kubectl apply -f hpa.yaml

   # Network security
   kubectl apply -f networkpolicy.yaml
   ```

## Configuration

### Storage

By default, uses `hostPath` for local development. For production, update `persistentvolume.yaml`:

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

### Environment Variables

Edit `configmap.yaml` to customize:

```yaml
data:
  RUST_LOG: "info"                           # Log level
  AKIDB_GRPC_PORT: "9090"                   # gRPC port
  AKIDB_REST_PORT: "8080"                   # REST port
  AKIDB_DB_PATH: "sqlite:///data/akidb/metadata.db"
  AKIDB_METRICS_ENABLED: "true"             # Enable metrics
  AKIDB_VECTOR_PERSISTENCE_ENABLED: "true"  # Enable vector persistence
```

### Resource Limits

Default resource requests/limits (per pod):

```yaml
resources:
  requests:
    cpu: 500m
    memory: 1Gi
  limits:
    cpu: 2000m
    memory: 4Gi
```

Adjust in `deployment-grpc.yaml` and `deployment-rest.yaml` based on your workload.

### Ingress

Update `ingress.yaml` with your domain:

```yaml
spec:
  tls:
    - hosts:
        - akidb.example.com  # Change this
      secretName: akidb-tls
  rules:
    - host: akidb.example.com  # Change this
```

## Scaling

### Manual Scaling

```bash
# Scale gRPC server
kubectl scale deployment akidb-grpc --replicas=5 -n akidb

# Scale REST server
kubectl scale deployment akidb-rest --replicas=5 -n akidb
```

### Auto-Scaling

Apply `hpa.yaml` to enable automatic scaling based on CPU/memory:

```bash
kubectl apply -f hpa.yaml

# Check HPA status
kubectl get hpa -n akidb
```

## Monitoring

### Prometheus

If Prometheus Operator is installed:

```bash
kubectl apply -f servicemonitor.yaml
```

Metrics will be scraped from `/metrics` endpoint every 30 seconds.

### View Metrics

```bash
# Port-forward to access metrics
kubectl port-forward -n akidb svc/akidb-rest 8080:8080

# View metrics
curl http://localhost:8080/metrics
```

## Troubleshooting

### Check Pod Status

```bash
kubectl get pods -n akidb
kubectl describe pod <pod-name> -n akidb
```

### View Logs

```bash
# gRPC server logs
kubectl logs -f -n akidb -l app=akidb-grpc

# REST server logs
kubectl logs -f -n akidb -l app=akidb-rest

# Previous container logs (if crashed)
kubectl logs -n akidb <pod-name> --previous
```

### Health Checks

```bash
# Port-forward REST service
kubectl port-forward -n akidb svc/akidb-rest 8080:8080

# Check health
curl http://localhost:8080/health
```

### Database Access

```bash
# Access pod shell
kubectl exec -it -n akidb <pod-name> -- /bin/sh

# Check database
sqlite3 /data/akidb/metadata.db "SELECT * FROM collections;"
```

### Events

```bash
kubectl get events -n akidb --sort-by='.lastTimestamp'
```

## Backup

### Manual Backup

```bash
# Copy database from pod
kubectl cp -n akidb <pod-name>:/data/akidb/metadata.db ./backup-$(date +%Y%m%d).db
```

### VolumeSnapshot

If your cluster supports CSI VolumeSnapshots:

```bash
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

## Cleanup

```bash
# Delete all resources
kubectl delete -f .

# Or delete namespace (removes everything)
kubectl delete namespace akidb
```

## See Also

- [Deployment Guide](/docs/DEPLOYMENT-GUIDE.md) - Complete deployment documentation
- [Quickstart Guide](/docs/QUICKSTART.md) - Getting started with AkiDB
- [Migration Guide](/docs/MIGRATION-V1-TO-V2.md) - Upgrading from v1.x
