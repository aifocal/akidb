# Phase 5: Docker/Kubernetes Deployment & Production Rollout PRD
## Candle Embedding Migration - Week 5

**Version:** 1.0
**Date:** 2025-11-10
**Status:** Ready for Implementation
**Owner:** Backend Team + DevOps
**Timeline:** 5 days (Week 5, Monday-Friday)

---

## Executive Summary

**Goal:** Package Candle embedding service for **production deployment** with Docker, Kubernetes, Helm charts, CI/CD pipelines, and blue-green deployment strategy to enable safe rollout to production.

**Phase 5 Context:** Building on Phase 4's multi-model flexibility, this phase focuses on **production deployment**. We'll containerize the service, create Kubernetes manifests, build Helm charts, set up CI/CD, and implement blue-green deployment for zero-downtime rollout.

**Success Criteria:**
- ✅ Multi-arch Docker images (AMD64 + ARM64)
- ✅ Kubernetes manifests with autoscaling
- ✅ Helm chart for easy deployment
- ✅ CI/CD pipeline (GitHub Actions)
- ✅ Blue-green deployment automation
- ✅ Production deployment runbook
- ✅ Zero-downtime migration from MLX to Candle

---

## Table of Contents

1. [Problem Statement](#problem-statement)
2. [Goals & Non-Goals](#goals--non-goals)
3. [Technical Design](#technical-design)
4. [Docker Strategy](#docker-strategy)
5. [Kubernetes Architecture](#kubernetes-architecture)
6. [Helm Chart Design](#helm-chart-design)
7. [CI/CD Pipeline](#cicd-pipeline)
8. [Blue-Green Deployment](#blue-green-deployment)
9. [Migration Strategy](#migration-strategy)
10. [Monitoring & Alerting](#monitoring--alerting)
11. [Success Criteria](#success-criteria)
12. [Risks & Mitigation](#risks--mitigation)
13. [Timeline & Milestones](#timeline--milestones)
14. [Dependencies](#dependencies)
15. [Deliverables](#deliverables)

---

## Problem Statement

### Current State (Post Phase 4)

Phase 4 delivered **multi-model flexibility** with:
- ✅ 4 models with runtime selection
- ✅ INT8 quantization (75% memory savings)
- ✅ Model warm-up (<100ms cold start)
- ✅ LRU cache with eviction
- ✅ 81 tests passing
- ✅ 200+ QPS throughput

**However**, the Phase 4 implementation is **not deployed to production**:

| Gap | Impact | User Pain |
|-----|--------|-----------|
| **No containerization** | Cannot deploy to K8s | Manual deployment, not scalable |
| **No Helm chart** | Complex deployment process | High barrier to adoption |
| **No CI/CD** | Manual builds and releases | Slow iteration, error-prone |
| **No autoscaling** | Manual scaling decisions | Over/under-provisioned resources |
| **No rollback strategy** | Risky deployments | Fear of production changes |
| **MLX still in production** | Users on old system | No benefit from 36x improvement |

### Why Production Deployment Matters

**Business Impact:**
- **Time to Market:** Ship 36x performance improvement to production
- **Operational Efficiency:** Automated deployments → faster iteration
- **Cost Savings:** Autoscaling → pay only for what you use
- **Reliability:** Blue-green deployment → zero-downtime updates
- **Adoption:** Helm chart → easy for users to self-deploy

**Technical Impact:**
- **Scalability:** K8s autoscaling handles traffic spikes automatically
- **Portability:** Docker runs anywhere (cloud, on-prem, edge)
- **Consistency:** Same container dev → staging → production
- **Observability:** Integration with Prometheus/Grafana/Jaeger

---

## Goals & Non-Goals

### Goals (In Scope)

**Primary Goals:**
1. ✅ **Dockerfile:** Multi-stage, multi-arch (AMD64 + ARM64)
2. ✅ **Kubernetes Manifests:** Deployment, Service, HPA, PDB
3. ✅ **Helm Chart:** Configurable deployment package
4. ✅ **CI/CD Pipeline:** Automated build, test, publish (GitHub Actions)
5. ✅ **Blue-Green Deployment:** Zero-downtime rollout automation

**Secondary Goals:**
6. ✅ **Production Runbook:** Deployment, rollback, troubleshooting
7. ✅ **Migration Script:** MLX → Candle cutover automation
8. ✅ **Resource Sizing:** CPU/memory recommendations
9. ✅ **Monitoring Integration:** Prometheus + Grafana dashboards

### Non-Goals (Out of Scope)

**Deferred to Phase 6:**
- ❌ Multi-region deployment (Phase 6)
- ❌ GitOps (ArgoCD/Flux) (Phase 6)
- ❌ Service mesh (Istio/Linkerd) (Phase 6)
- ❌ Custom operators (Future)

**Explicitly Out of Scope:**
- ❌ Infrastructure provisioning (assume K8s cluster exists)
- ❌ Cloud-specific features (EKS/GKE/AKS)
- ❌ Cost optimization beyond autoscaling
- ❌ Breaking API changes

---

## Technical Design

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    Production Environment                        │
│                                                                  │
│  ┌────────────────────────────────────────────────────────┐   │
│  │              Load Balancer / Ingress                    │   │
│  │  - HTTPS termination                                    │   │
│  │  - Path-based routing                                   │   │
│  │  - Health check integration                             │   │
│  └──────────────────┬─────────────────────────────────────┘   │
│                     │                                           │
│           ┌─────────┴──────────┐                                │
│           │                    │                                │
│  ┌────────▼────────┐  ┌───────▼────────┐                       │
│  │  Candle Service │  │  Candle Service │                       │
│  │  (Blue)         │  │  (Green)        │                       │
│  │  - Deployment   │  │  - Deployment   │                       │
│  │  - 2-10 pods    │  │  - 2-10 pods    │                       │
│  │  - HPA enabled  │  │  - HPA enabled  │                       │
│  └─────────────────┘  └────────────────┘                       │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │              Observability Stack                          │  │
│  │  - Prometheus (metrics collection)                        │  │
│  │  - Grafana (dashboards)                                   │  │
│  │  - Jaeger (distributed tracing)                           │  │
│  │  - Loki (log aggregation)                                 │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │              Persistent Storage                           │  │
│  │  - Model cache (PVC)                                      │  │
│  │  - Configuration (ConfigMap/Secret)                       │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### Component Design

#### 1. Docker Image

**Multi-stage Dockerfile:**
```dockerfile
# Stage 1: Build
FROM rust:1.75-bullseye AS builder
WORKDIR /build
COPY . .
RUN cargo build --release --features candle --bin akidb-rest

# Stage 2: Runtime
FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/akidb-rest /usr/local/bin/

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/akidb-rest"]
```

**Image Characteristics:**
- **Base:** debian:bullseye-slim (stability)
- **Size:** ~200MB (vs ~2GB with full Rust toolchain)
- **Architectures:** AMD64, ARM64
- **Security:** Non-root user, minimal attack surface

#### 2. Kubernetes Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: akidb-candle
spec:
  replicas: 2  # Start with 2 for HA
  selector:
    matchLabels:
      app: akidb-candle
      version: candle
  template:
    metadata:
      labels:
        app: akidb-candle
        version: candle
    spec:
      containers:
      - name: akidb-rest
        image: akidb/embedding:candle-v1.0.0
        ports:
        - containerPort: 8080
          name: http
        env:
        - name: RUST_LOG
          value: "info"
        - name: AKIDB_REST_PORT
          value: "8080"
        resources:
          requests:
            cpu: 1000m      # 1 CPU
            memory: 2Gi     # 2GB (fits 2 models with INT8)
          limits:
            cpu: 2000m      # 2 CPU max
            memory: 4Gi     # 4GB max
        livenessProbe:
          httpGet:
            path: /health/live
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health/ready
            port: 8080
          initialDelaySeconds: 60
          periodSeconds: 5
        volumeMounts:
        - name: model-cache
          mountPath: /root/.cache/huggingface
      volumes:
      - name: model-cache
        persistentVolumeClaim:
          claimName: akidb-model-cache
```

#### 3. Horizontal Pod Autoscaler (HPA)

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: akidb-candle-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: akidb-candle
  minReplicas: 2
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70  # Scale when CPU > 70%
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80  # Scale when memory > 80%
  - type: Pods
    pods:
      metric:
        name: http_requests_per_second
      target:
        type: AverageValue
        averageValue: "100"  # Scale at 100 RPS per pod
  behavior:
    scaleUp:
      stabilizationWindowSeconds: 60
      policies:
      - type: Percent
        value: 50
        periodSeconds: 60  # Scale up by 50% every 60s
    scaleDown:
      stabilizationWindowSeconds: 300
      policies:
      - type: Pods
        value: 1
        periodSeconds: 120  # Scale down 1 pod every 120s
```

---

## Docker Strategy

### Multi-Stage Build

**Advantages:**
1. **Small image:** ~200MB (vs 2GB with toolchain)
2. **Fast builds:** Cached layers
3. **Secure:** Only runtime dependencies

**Build Process:**
```bash
# Build for both architectures
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t akidb/embedding:candle-v1.0.0 \
  -f docker/Dockerfile \
  --push \
  .
```

### Image Tagging Strategy

```
akidb/embedding:candle-latest           # Latest Candle build
akidb/embedding:candle-v1.0.0           # Semantic version
akidb/embedding:candle-sha-abc123       # Git commit SHA
akidb/embedding:candle-pr-123           # PR builds
akidb/embedding:mlx-v0.9.0              # MLX (legacy)
```

### Security Best Practices

```dockerfile
# Use specific base image (not :latest)
FROM debian:bullseye-slim@sha256:abc123...

# Run as non-root user
RUN useradd -m -u 1000 akidb
USER akidb

# Read-only root filesystem
COPY --chown=akidb:akidb --from=builder /build/target/release/akidb-rest /app/

# Healthcheck
HEALTHCHECK --interval=30s --timeout=3s --start-period=60s \
  CMD curl -f http://localhost:8080/health/live || exit 1
```

---

## Kubernetes Architecture

### Resource Requirements

**Per Pod:**
- CPU request: 1 core
- CPU limit: 2 cores
- Memory request: 2GB (fits 2 models with INT8)
- Memory limit: 4GB
- Ephemeral storage: 10GB (model cache)

**Cluster Sizing (Production):**
- Min replicas: 2 (HA)
- Max replicas: 10 (autoscaling)
- Expected load: 100 QPS per pod
- Total capacity: 1,000 QPS @ 10 replicas

### Networking

```yaml
apiVersion: v1
kind: Service
metadata:
  name: akidb-candle
  labels:
    app: akidb-candle
spec:
  type: ClusterIP
  ports:
  - port: 8080
    targetPort: 8080
    protocol: TCP
    name: http
  selector:
    app: akidb-candle
    version: candle
---
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: akidb-candle
  annotations:
    kubernetes.io/ingress.class: nginx
    cert-manager.io/cluster-issuer: letsencrypt-prod
spec:
  tls:
  - hosts:
    - api.akidb.example.com
    secretName: akidb-tls
  rules:
  - host: api.akidb.example.com
    http:
      paths:
      - path: /api/v1/embed
        pathType: Prefix
        backend:
          service:
            name: akidb-candle
            port:
              number: 8080
```

### Pod Disruption Budget

```yaml
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: akidb-candle-pdb
spec:
  minAvailable: 1  # Always keep at least 1 pod running
  selector:
    matchLabels:
      app: akidb-candle
```

---

## Helm Chart Design

### Chart Structure

```
akidb-candle/
├── Chart.yaml              # Metadata
├── values.yaml             # Default configuration
├── templates/
│   ├── deployment.yaml     # Deployment manifest
│   ├── service.yaml        # Service manifest
│   ├── ingress.yaml        # Ingress manifest
│   ├── hpa.yaml            # HPA manifest
│   ├── pdb.yaml            # PDB manifest
│   ├── configmap.yaml      # Configuration
│   ├── secret.yaml         # Secrets
│   ├── pvc.yaml            # Model cache PVC
│   └── servicemonitor.yaml # Prometheus scraping
├── values.schema.json      # Validation schema
└── README.md               # Installation guide
```

### values.yaml

```yaml
# values.yaml

# Image configuration
image:
  repository: akidb/embedding
  tag: candle-v1.0.0
  pullPolicy: IfNotPresent

# Replica configuration
replicaCount: 2

# Autoscaling
autoscaling:
  enabled: true
  minReplicas: 2
  maxReplicas: 10
  targetCPUUtilizationPercentage: 70
  targetMemoryUtilizationPercentage: 80

# Resources
resources:
  requests:
    cpu: 1000m
    memory: 2Gi
  limits:
    cpu: 2000m
    memory: 4Gi

# Service configuration
service:
  type: ClusterIP
  port: 8080

# Ingress
ingress:
  enabled: true
  className: nginx
  annotations:
    cert-manager.io/cluster-issuer: letsencrypt-prod
  hosts:
    - host: api.akidb.example.com
      paths:
        - path: /
          pathType: Prefix
  tls:
    - secretName: akidb-tls
      hosts:
        - api.akidb.example.com

# Model cache
persistence:
  enabled: true
  storageClass: standard
  size: 10Gi

# Environment configuration
config:
  logLevel: info
  preloadModels: "all-MiniLM-L6-v2,e5-small-v2"
  maxModels: 4
  maxMemoryMB: 2048

# Monitoring
monitoring:
  enabled: true
  serviceMonitor:
    enabled: true
    interval: 30s

# Pod disruption budget
podDisruptionBudget:
  enabled: true
  minAvailable: 1
```

### Installation Commands

```bash
# Add Helm repository
helm repo add akidb https://charts.akidb.io
helm repo update

# Install with default values
helm install akidb-candle akidb/akidb-candle

# Install with custom values
helm install akidb-candle akidb/akidb-candle \
  --set replicaCount=3 \
  --set ingress.hosts[0].host=my-api.example.com

# Upgrade
helm upgrade akidb-candle akidb/akidb-candle

# Rollback
helm rollback akidb-candle
```

---

## CI/CD Pipeline

### GitHub Actions Workflow

```yaml
# .github/workflows/candle-cicd.yml

name: Candle CI/CD

on:
  push:
    branches: [main]
    paths:
      - 'crates/akidb-embedding/**'
      - 'crates/akidb-rest/**'
      - 'Cargo.*'
      - 'docker/**'
  pull_request:
    branches: [main]
  release:
    types: [published]

env:
  REGISTRY: ghcr.io
  IMAGE_NAME: ${{ github.repository }}/akidb-embedding

jobs:
  test:
    name: Test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          profile: minimal
          override: true

      - name: Cache cargo
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Run tests
        run: cargo test --workspace --features candle

      - name: Run benchmarks
        run: cargo bench --features candle -- --test

  build-and-push:
    name: Build and Push Docker Image
    needs: test
    runs-on: ubuntu-latest
    if: github.event_name != 'pull_request'
    steps:
      - uses: actions/checkout@v4

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Log in to registry
        uses: docker/login-action@v3
        with:
          registry: ${{ env.REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - name: Extract metadata
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}
          tags: |
            type=ref,event=branch
            type=semver,pattern={{version}}
            type=semver,pattern={{major}}.{{minor}}
            type=sha,prefix=candle-sha-

      - name: Build and push
        uses: docker/build-push-action@v5
        with:
          context: .
          file: docker/Dockerfile
          platforms: linux/amd64,linux/arm64
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          cache-from: type=gha
          cache-to: type=gha,mode=max

  deploy-staging:
    name: Deploy to Staging
    needs: build-and-push
    runs-on: ubuntu-latest
    if: github.ref == 'refs/heads/main'
    environment:
      name: staging
      url: https://staging.akidb.example.com
    steps:
      - uses: actions/checkout@v4

      - name: Setup kubectl
        uses: azure/setup-kubectl@v3

      - name: Setup Helm
        uses: azure/setup-helm@v3

      - name: Deploy with Helm
        run: |
          helm upgrade --install akidb-candle ./helm/akidb-candle \
            --namespace staging \
            --create-namespace \
            --set image.tag=candle-sha-${{ github.sha }} \
            --wait --timeout 5m

      - name: Smoke tests
        run: |
          kubectl wait --for=condition=ready pod \
            -l app=akidb-candle \
            -n staging \
            --timeout=300s

          curl -f https://staging.akidb.example.com/health/ready

  deploy-production:
    name: Deploy to Production
    needs: deploy-staging
    runs-on: ubuntu-latest
    if: github.event_name == 'release'
    environment:
      name: production
      url: https://api.akidb.example.com
    steps:
      - uses: actions/checkout@v4

      - name: Blue-Green Deployment
        run: |
          # Deploy green (new version)
          helm upgrade akidb-candle-green ./helm/akidb-candle \
            --namespace production \
            --set image.tag=${{ github.event.release.tag_name }} \
            --set service.selector.slot=green \
            --wait

          # Health check green
          ./scripts/healthcheck-green.sh

          # Switch traffic to green
          kubectl patch service akidb-candle \
            -n production \
            -p '{"spec":{"selector":{"slot":"green"}}}'

          # Wait and monitor
          sleep 300

          # Cleanup blue (old version)
          helm uninstall akidb-candle-blue -n production
```

---

## Blue-Green Deployment

### Strategy

```
Initial State:
┌─────────────┐
│   Traffic   │
└──────┬──────┘
       │
   ┌───▼────┐
   │  Blue  │ ← Current version (MLX or Candle v1.0)
   │  (v1)  │
   └────────┘

Step 1: Deploy Green
┌─────────────┐
│   Traffic   │
└──────┬──────┘
       │
   ┌───▼────┐   ┌────────┐
   │  Blue  │   │ Green  │ ← New version (Candle v1.1)
   │  (v1)  │   │  (v2)  │    (not receiving traffic)
   └────────┘   └────────┘

Step 2: Switch Traffic
┌─────────────┐
│   Traffic   │
└──────┬──────┘
       │
   ┌────────┐   ┌───▼────┐
   │  Blue  │   │ Green  │ ← Now receiving traffic
   │  (v1)  │   │  (v2)  │
   └────────┘   └────────┘

Step 3: Monitor (5 minutes)
   If errors → rollback to Blue
   If success → delete Blue

Final State:
┌─────────────┐
│   Traffic   │
└──────┬──────┘
       │
       │      ┌───▼────┐
       │      │ Green  │
       │      │  (v2)  │
       │      └────────┘
```

### Automation Script

```bash
#!/bin/bash
# scripts/blue-green-deploy.sh

set -e

NAMESPACE=${1:-production}
NEW_VERSION=${2:-candle-latest}
HEALTHCHECK_RETRIES=10
MONITOR_DURATION=300  # 5 minutes

echo "Starting blue-green deployment to $NAMESPACE"

# Step 1: Deploy green
echo "Deploying green (version: $NEW_VERSION)..."
helm upgrade akidb-candle-green ./helm/akidb-candle \
  --namespace $NAMESPACE \
  --install \
  --set image.tag=$NEW_VERSION \
  --set service.selector.slot=green \
  --wait --timeout 10m

# Step 2: Health check green
echo "Health checking green..."
for i in $(seq 1 $HEALTHCHECK_RETRIES); do
  if kubectl exec -n $NAMESPACE \
    deployment/akidb-candle-green \
    -- curl -sf http://localhost:8080/health/ready; then
    echo "Green is healthy"
    break
  fi

  if [ $i -eq $HEALTHCHECK_RETRIES ]; then
    echo "Green health check failed. Aborting."
    exit 1
  fi

  echo "Retry $i/$HEALTHCHECK_RETRIES..."
  sleep 10
done

# Step 3: Switch traffic
echo "Switching traffic to green..."
kubectl patch service akidb-candle \
  -n $NAMESPACE \
  -p '{"spec":{"selector":{"slot":"green"}}}'

echo "Traffic switched. Monitoring for $MONITOR_DURATION seconds..."

# Step 4: Monitor
sleep $MONITOR_DURATION

# Check error rate
ERROR_RATE=$(kubectl exec -n $NAMESPACE \
  deployment/akidb-candle-green \
  -- curl -s http://localhost:8080/metrics | \
  grep candle_errors_total | \
  awk '{print $2}')

if [ "$ERROR_RATE" -gt 10 ]; then
  echo "ERROR: High error rate detected ($ERROR_RATE). Rolling back!"

  # Rollback to blue
  kubectl patch service akidb-candle \
    -n $NAMESPACE \
    -p '{"spec":{"selector":{"slot":"blue"}}}'

  exit 1
fi

# Step 5: Cleanup blue
echo "Deployment successful. Cleaning up blue..."
helm uninstall akidb-candle-blue -n $NAMESPACE || true

# Rename green to blue for next deployment
helm upgrade akidb-candle-blue ./helm/akidb-candle \
  --namespace $NAMESPACE \
  --set image.tag=$NEW_VERSION \
  --set service.selector.slot=blue

echo "Blue-green deployment complete!"
```

---

## Migration Strategy

### MLX → Candle Cutover

**Timeline:**
- Week 1: Deploy Candle to staging
- Week 2: Canary deployment (10% production traffic)
- Week 3: Ramp to 50% production traffic
- Week 4: Full cutover (100% traffic)
- Week 5: Decommission MLX

**Rollout Phases:**

**Phase 1: Staging Validation (Week 1)**
```bash
# Deploy to staging
helm install akidb-candle ./helm/akidb-candle \
  --namespace staging \
  --set replicaCount=1

# Run integration tests
./scripts/integration-tests.sh staging

# Load test
wrk -t 8 -c 100 -d 300s https://staging.akidb.example.com/api/v1/embed
```

**Phase 2: Canary (10% traffic, Week 2)**
```yaml
# Ingress with traffic split
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  annotations:
    nginx.ingress.kubernetes.io/canary: "true"
    nginx.ingress.kubernetes.io/canary-weight: "10"  # 10% to Candle
spec:
  ingressClassName: nginx
  rules:
  - host: api.akidb.example.com
    http:
      paths:
      - path: /api/v1/embed
        backend:
          service:
            name: akidb-candle  # Candle (10%)
```

**Phase 3: Ramp to 50% (Week 3)**
```bash
# Increase canary weight
kubectl patch ingress akidb-candle-canary -n production \
  -p '{"metadata":{"annotations":{"nginx.ingress.kubernetes.io/canary-weight":"50"}}}'
```

**Phase 4: Full Cutover (Week 4)**
```bash
# Switch 100% traffic to Candle
kubectl patch ingress akidb-candle -n production \
  -p '{"metadata":{"annotations":{"nginx.ingress.kubernetes.io/canary":"false"}}}'

# Scale down MLX
kubectl scale deployment akidb-mlx --replicas=0 -n production
```

**Phase 5: Decommission MLX (Week 5)**
```bash
# Delete MLX deployment
helm uninstall akidb-mlx -n production
```

### Rollback Plan

**Scenario: Critical issue detected in Candle**

```bash
# Immediate rollback to MLX
kubectl patch ingress akidb-candle -n production \
  -p '{"spec":{"rules":[{"http":{"paths":[{"backend":{"service":{"name":"akidb-mlx"}}}]}}]}}'

# Scale up MLX
kubectl scale deployment akidb-mlx --replicas=3 -n production

# Investigate Candle issue
kubectl logs -l app=akidb-candle -n production --tail=1000
```

---

## Monitoring & Alerting

### Grafana Dashboards

**Dashboard 1: Overview**
- Request rate (QPS)
- Latency (P50/P95/P99)
- Error rate
- Pod count (actual vs desired)
- Memory usage
- GPU utilization

**Dashboard 2: Model Performance**
- Requests per model
- Cache hit rate
- Model load latency
- Memory per model
- Eviction count

**Dashboard 3: Kubernetes Health**
- Pod status
- Node resources
- HPA scaling events
- PDB status
- Ingress traffic

### Prometheus Alerts

```yaml
# prometheus-alerts.yaml

groups:
- name: akidb-candle
  interval: 30s
  rules:
  - alert: HighErrorRate
    expr: |
      rate(candle_errors_total[5m]) > 10
    for: 5m
    labels:
      severity: critical
    annotations:
      summary: "High error rate detected"
      description: "Error rate is {{ $value }} errors/sec"

  - alert: HighLatency
    expr: |
      histogram_quantile(0.95, candle_request_duration_seconds_bucket) > 0.1
    for: 10m
    labels:
      severity: warning
    annotations:
      summary: "High P95 latency"
      description: "P95 latency is {{ $value }}s"

  - alert: PodCrashLooping
    expr: |
      rate(kube_pod_container_status_restarts_total{pod=~"akidb-candle-.*"}[15m]) > 0
    for: 5m
    labels:
      severity: critical
    annotations:
      summary: "Pod crash looping"

  - alert: MemoryPressure
    expr: |
      candle_memory_usage_bytes / (2 * 1024 * 1024 * 1024) > 0.9
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "Memory usage >90%"
```

---

## Success Criteria

### Functional Requirements

✅ **FR1:** Multi-arch Docker image (AMD64 + ARM64)
✅ **FR2:** Kubernetes manifests (Deployment, Service, HPA, PDB)
✅ **FR3:** Helm chart with configurable values
✅ **FR4:** CI/CD pipeline with automated testing
✅ **FR5:** Blue-green deployment automation
✅ **FR6:** MLX → Candle migration runbook
✅ **FR7:** Grafana dashboards and Prometheus alerts

### Non-Functional Requirements

✅ **NFR1: Zero-Downtime Deployment**
- Blue-green deployment: 0 failed requests during cutover
- Rollback time: <5 minutes

✅ **NFR2: Scalability**
- Autoscaling: 2-10 pods based on load
- Scale-up time: <2 minutes
- Scale-down grace period: 5 minutes

✅ **NFR3: Resource Efficiency**
- Pod startup time: <90 seconds
- Memory per pod: 2-4GB
- CPU per pod: 1-2 cores

✅ **NFR4: Observability**
- Metrics exported to Prometheus
- Dashboards in Grafana
- Traces in Jaeger
- Logs in Loki

✅ **NFR5: Production Readiness**
- 99.9% uptime SLA
- Automated rollback on errors
- Disaster recovery plan

---

## Risks & Mitigation

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| **Docker build failures** | Low | High | • Multi-stage caching<br>• Fallback to local builds<br>• Retry logic in CI |
| **K8s cluster capacity** | Medium | High | • Pre-provision nodes<br>• Cluster autoscaler<br>• Resource quotas |
| **Helm chart misconfiguration** | Medium | Medium | • Schema validation<br>• Dry-run before apply<br>• Staging validation |
| **Blue-green cutover issues** | Low | Critical | • Extended monitoring period<br>• Automated rollback<br>• Feature flags |
| **Model cache PVC full** | Medium | Medium | • Monitor disk usage<br>• Automatic cleanup<br>• Increase PVC size |
| **Ingress misconfiguration** | Low | High | • Test in staging first<br>• Gradual rollout<br>• Canary deployment |

---

## Timeline & Milestones

### Week 5 Schedule (5 days, Monday-Friday)

#### **Day 1 (Monday): Docker Image (6 hours)**

**Tasks:**
1. ☐ Create multi-stage Dockerfile (2 hours)
2. ☐ Build multi-arch images (2 hours)
3. ☐ Push to registry (1 hour)
4. ☐ Test image locally (1 hour)

**Deliverables:**
- Dockerfile (~50 lines)
- Docker images (AMD64 + ARM64)
- Image size <250MB

#### **Day 2 (Tuesday): Kubernetes Manifests (6 hours)**

**Tasks:**
1. ☐ Create Deployment manifest (2 hours)
2. ☐ Create Service + Ingress (1.5 hours)
3. ☐ Create HPA + PDB (1.5 hours)
4. ☐ Test on local K8s (1 hour)

**Deliverables:**
- K8s manifests (~300 lines)
- Local deployment working

#### **Day 3 (Wednesday): Helm Chart (6 hours)**

**Tasks:**
1. ☐ Create Helm chart structure (2 hours)
2. ☐ Templatize manifests (2 hours)
3. ☐ Add values.yaml (1 hour)
4. ☐ Package and test (1 hour)

**Deliverables:**
- Helm chart
- Installation tested

#### **Day 4 (Thursday): CI/CD Pipeline (6 hours)**

**Tasks:**
1. ☐ GitHub Actions workflow (3 hours)
2. ☐ Blue-green deploy script (2 hours)
3. ☐ Test CI/CD (1 hour)

**Deliverables:**
- GitHub Actions workflow
- Blue-green automation

#### **Day 5 (Friday): Documentation + Staging Deploy (6 hours)**

**Tasks:**
1. ☐ Production runbook (2 hours)
2. ☐ Deploy to staging (2 hours)
3. ☐ Grafana dashboards (1 hour)
4. ☐ Phase 5 completion report (1 hour)

**Deliverables:**
- Production runbook
- Staging deployment
- Dashboards
- Completion report

---

## Dependencies

### Internal Dependencies

**From Phase 1-4:**
- ✅ Candle embedding service (production-ready)
- ✅ Multi-model support
- ✅ Observability (metrics, traces, logs)
- ✅ 81 tests passing

**Blockers:**
- ❌ None (Phase 4 complete)

### External Dependencies

**Infrastructure:**
- Kubernetes cluster (1.24+)
- Container registry (Docker Hub, GHCR, ECR)
- Domain with DNS control
- TLS certificate (Let's Encrypt)

**Tools:**
```bash
# Required tools
docker (24.0+)
kubectl (1.24+)
helm (3.10+)
```

---

## Deliverables

### Code Deliverables

| File | Lines | Description |
|------|-------|-------------|
| `docker/Dockerfile` | ~50 | Multi-stage Dockerfile |
| `k8s/deployment.yaml` | ~80 | Deployment manifest |
| `k8s/service.yaml` | ~20 | Service manifest |
| `k8s/ingress.yaml` | ~30 | Ingress manifest |
| `k8s/hpa.yaml` | ~30 | HPA manifest |
| `k8s/pdb.yaml` | ~15 | PDB manifest |
| `helm/akidb-candle/` | ~400 | Helm chart |
| `.github/workflows/candle-cicd.yml` | ~150 | CI/CD pipeline |
| `scripts/blue-green-deploy.sh` | ~100 | Blue-green automation |
| `grafana/dashboards/` | ~200 | Grafana dashboards (JSON) |
| **Total** | **~1,075 lines** | |

### Documentation Deliverables

1. **`docs/PRODUCTION-DEPLOYMENT-GUIDE.md`**
2. **`docs/HELM-CHART-README.md`**
3. **`docs/MIGRATION-MLX-TO-CANDLE.md`**
4. **Phase 5 Completion Report**

---

## Sign-Off

**Phase 5 PRD Version:** 1.0
**Status:** ✅ Ready for Implementation
**Estimated Effort:** 30 development hours (5 days × 6 hours)
**Expected Completion:** End of Week 5 → **Production Deployment Ready**

**Next Phase:** [Phase 6: GA Release & Production Rollout](CANDLE-PHASE-6-GA-RELEASE-PRD.md)

---

**Document End**
