# Jetson Thor Week 7: CI/CD Pipeline & Multi-Region Deployment PRD

**Status:** Ready to Execute
**Timeline:** 5 days (Week 7)
**Owner:** Backend Team + DevOps + Platform Engineering
**Dependencies:** Week 1-6 (✅ Complete)
**Target Platform:** NVIDIA Jetson Thor (Blackwell GPU, 2,000 TOPS) - Multi-Region Edge

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Goals & Non-Goals](#goals--non-goals)
3. [Baseline Analysis](#baseline-analysis)
4. [CI/CD Architecture](#cicd-architecture)
5. [Multi-Region Architecture](#multi-region-architecture)
6. [Day-by-Day Implementation Plan](#day-by-day-implementation-plan)
7. [GitOps with ArgoCD](#gitops-with-argocd)
8. [Deployment Strategies](#deployment-strategies)
9. [Disaster Recovery](#disaster-recovery)
10. [Risk Management](#risk-management)
11. [Success Criteria](#success-criteria)
12. [Appendix: Code Examples](#appendix-code-examples)

---

## Executive Summary

Week 7 focuses on **CI/CD automation** and **multi-region deployment**, implementing GitOps workflows with ArgoCD, blue-green and canary deployment strategies, and active-passive multi-region failover for edge clusters. This enables automated, safe deployments with zero downtime and geographic disaster recovery capabilities.

### Key Objectives

1. **GitOps Pipeline:** ArgoCD for declarative deployments from Git
2. **CI/CD Automation:** GitHub Actions for build, test, and image push
3. **Blue-Green Deployments:** Zero-downtime updates with traffic switching
4. **Canary Releases:** Progressive rollout with 10% → 50% → 100% traffic split
5. **Multi-Region:** Active-passive failover across 2+ edge clusters
6. **Disaster Recovery:** Automated failover with <5 minute RTO, <1 hour RPO

### Expected Outcomes

- ✅ GitOps: All deployments from Git, zero manual kubectl
- ✅ CI/CD: Automated build → test → push → deploy on every commit
- ✅ Blue-Green: <1 second traffic switch, instant rollback
- ✅ Canary: 10% → 50% → 100% progressive rollout with auto-rollback
- ✅ Multi-Region: 2 edge clusters (primary + DR), active-passive
- ✅ Failover: Automated DNS failover, RTO <5min, RPO <1hr
- ✅ Image Registry: Harbor or Docker Hub with vulnerability scanning
- ✅ Secrets Management: Sealed Secrets or External Secrets Operator

---

## Goals & Non-Goals

### Goals (Week 7)

**Primary Goals:**
1. ✅ **GitOps with ArgoCD** - Declarative deployments, auto-sync from Git
2. ✅ **CI/CD Pipeline** - GitHub Actions: build → test → push → deploy
3. ✅ **Blue-Green Deployments** - Zero-downtime updates
4. ✅ **Canary Releases** - Progressive rollout (10% → 50% → 100%)
5. ✅ **Multi-Region Setup** - 2 edge clusters (primary in US, DR in EU/Asia)
6. ✅ **Automated Failover** - DNS-based, RTO <5min
7. ✅ **Image Registry** - Harbor with vulnerability scanning
8. ✅ **Secrets Management** - Sealed Secrets for GitOps

**Secondary Goals:**
- 📊 Automated rollback on SLO violations
- 📊 Progressive delivery with Flagger
- 📊 Multi-cluster observability with Thanos
- 📝 Deployment runbooks and playbooks
- 📝 Compliance automation (SOC2, HIPAA prep)

### Non-Goals (Deferred to Week 8+)

**Not in Scope for Week 7:**
- ❌ Active-active multi-region (requires data replication) - Week 8
- ❌ Service mesh (Istio/Linkerd) for advanced traffic control - Week 8
- ❌ Cost optimization and autoscaling tuning - Week 9
- ❌ Compliance certifications (SOC2, HIPAA) - Week 10+
- ❌ Advanced ML model versioning - Week 11+

---

## Baseline Analysis

### Week 6 Production Status

**Deployed Infrastructure:**
- Single edge cluster (primary region)
- Production hardening: Circuit breakers, rate limiting, mTLS, RBAC
- Security: API authentication, network policies, chaos-tested
- Performance: >50 QPS @ <30ms P95, 99.9% availability

**Current Limitations:**
- ❌ Manual deployments (kubectl apply)
- ❌ No CI/CD automation
- ❌ No blue-green or canary strategies
- ❌ Single region (no disaster recovery)
- ❌ Manual rollback (error-prone)
- ❌ No image vulnerability scanning
- ❌ Secrets in plain Kubernetes secrets

### Week 7 Target State

**Automated CI/CD:**
- ✅ GitOps: All deployments from Git (single source of truth)
- ✅ CI: Automated build, test, lint, scan on every PR
- ✅ CD: ArgoCD auto-deploys on Git merge
- ✅ Blue-Green: <1s traffic switch with instant rollback
- ✅ Canary: Progressive rollout with automatic rollback on errors

**Multi-Region DR:**
- ✅ 2 edge clusters: US-West (primary), EU-Central (DR)
- ✅ Active-passive: Primary serves all traffic, DR warm standby
- ✅ Automated failover: DNS-based with health checks
- ✅ RTO: <5 minutes, RPO: <1 hour
- ✅ Data sync: Model cache + TensorRT engines replicated

---

## CI/CD Architecture

### GitOps Workflow with ArgoCD

```
┌─────────────────────────────────────────────────────────────────┐
│                        Developer                                 │
│                                                                  │
│  1. Code change (Rust, Dockerfile, Helm chart)                 │
│  2. git commit + git push to feature branch                     │
│  3. Open Pull Request                                           │
└─────────────────────────┬───────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│                    GitHub Actions (CI)                           │
│                                                                  │
│  1. Trigger on PR                                               │
│  2. cargo build --workspace --release                           │
│  3. cargo test --workspace                                      │
│  4. cargo clippy -- -D warnings                                 │
│  5. Trivy scan (image vulnerabilities)                          │
│  6. If all pass: PR ready for review                            │
└─────────────────────────┬───────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Code Review + Merge                           │
│                                                                  │
│  1. Team reviews PR                                             │
│  2. Merge to main branch                                        │
└─────────────────────────┬───────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│                GitHub Actions (CD - Build & Push)                │
│                                                                  │
│  1. Trigger on push to main                                     │
│  2. Build Docker images (multi-stage, ARM64)                    │
│  3. Tag with git SHA + version                                  │
│  4. Push to Harbor registry (harbor.akidb.io)                   │
│  5. Update Helm chart image tag in Git                          │
│  6. git commit + push (triggers ArgoCD)                         │
└─────────────────────────┬───────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│                       ArgoCD (CD - Deploy)                       │
│                                                                  │
│  1. Detect Git change (auto-sync enabled)                       │
│  2. Compare desired state (Git) vs actual state (K8s)           │
│  3. Sync: kubectl apply Helm chart                              │
│  4. Health check: Wait for pods Ready                           │
│  5. Slack notification: Deployment success/failure              │
└─────────────────────────┬───────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│                 Kubernetes Edge Cluster                          │
│                 (Jetson Thor Nodes)                              │
│                                                                  │
│  - Blue-Green: New pods created, traffic switch                 │
│  - Canary: Progressive rollout 10% → 50% → 100%                │
│  - Health checks: Liveness + readiness probes                   │
│  - Auto-rollback: On health check failures                      │
└─────────────────────────────────────────────────────────────────┘
```

### CI Pipeline (GitHub Actions)

**Pull Request Workflow (.github/workflows/pr.yml):**

```yaml
name: Pull Request CI

on:
  pull_request:
    branches: [main, develop]

jobs:
  build-and-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          target: aarch64-unknown-linux-gnu

      - name: Cache dependencies
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Build
        run: cargo build --workspace --release --target aarch64-unknown-linux-gnu

      - name: Test
        run: cargo test --workspace

      - name: Clippy
        run: cargo clippy --all-targets --all-features -- -D warnings

      - name: Format check
        run: cargo fmt --all -- --check

      - name: Security audit
        uses: actions-rs/audit-check@v1
        with:
          token: ${{ secrets.GITHUB_TOKEN }}
```

**Release Workflow (.github/workflows/release.yml):**

```yaml
name: Release

on:
  push:
    branches: [main]

jobs:
  build-and-push:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Set up QEMU
        uses: docker/setup-qemu-action@v3

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Login to Harbor
        uses: docker/login-action@v3
        with:
          registry: harbor.akidb.io
          username: ${{ secrets.HARBOR_USERNAME }}
          password: ${{ secrets.HARBOR_PASSWORD }}

      - name: Extract metadata
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: harbor.akidb.io/akidb/akidb-rest
          tags: |
            type=sha,prefix={{branch}}-
            type=semver,pattern={{version}}

      - name: Build and push REST
        uses: docker/build-push-action@v5
        with:
          context: .
          file: docker/Dockerfile.jetson-rest
          platforms: linux/arm64
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          cache-from: type=gha
          cache-to: type=gha,mode=max

      - name: Build and push gRPC
        uses: docker/build-push-action@v5
        with:
          context: .
          file: docker/Dockerfile.jetson-grpc
          platforms: linux/arm64
          push: true
          tags: harbor.akidb.io/akidb/akidb-grpc:${{ steps.meta.outputs.tags }}

      - name: Update Helm values
        run: |
          git config user.name "GitHub Actions"
          git config user.email "actions@github.com"

          # Update image tag in values.yaml
          NEW_TAG="${{ steps.meta.outputs.version }}"
          sed -i "s/tag: .*/tag: $NEW_TAG/" deploy/helm/akidb-jetson/values.yaml

          git add deploy/helm/akidb-jetson/values.yaml
          git commit -m "chore: update image tag to $NEW_TAG [skip ci]"
          git push
```

### Harbor Image Registry

**Configuration:**
- **URL:** `harbor.akidb.io`
- **Projects:**
  - `akidb/akidb-rest` (REST API images)
  - `akidb/akidb-grpc` (gRPC API images)
- **Features:**
  - Vulnerability scanning (Trivy)
  - Image signing (Notary v2)
  - Replication to DR cluster
  - Retention policy (keep last 10 versions)

---

## Multi-Region Architecture

### Active-Passive Topology

```
                          ┌─────────────────┐
                          │   Route 53 DNS  │
                          │  (Health Check) │
                          └────────┬────────┘
                                   │
              ┌────────────────────┴────────────────────┐
              │                                         │
              ▼                                         ▼
    ┌───────────────────┐                    ┌───────────────────┐
    │  PRIMARY REGION   │                    │    DR REGION      │
    │    (US-West)      │                    │   (EU-Central)    │
    │                   │                    │                   │
    │  ┌─────────────┐  │                    │  ┌─────────────┐  │
    │  │ K8s Cluster │  │                    │  │ K8s Cluster │  │
    │  │ (3 nodes)   │  │                    │  │ (3 nodes)   │  │
    │  │             │  │                    │  │             │  │
    │  │ akidb-rest  │  │  Model Sync        │  │ akidb-rest  │  │
    │  │ akidb-grpc  │◄─┼────────────────────┼──│ akidb-grpc  │  │
    │  │             │  │  (S3 replication)  │  │             │  │
    │  └─────────────┘  │                    │  └─────────────┘  │
    │                   │                    │                   │
    │  Status: ACTIVE   │                    │  Status: STANDBY  │
    │  Traffic: 100%    │                    │  Traffic: 0%      │
    └───────────────────┘                    └───────────────────┘
            │                                         │
            │                                         │
            ▼                                         ▼
    ┌───────────────────┐                    ┌───────────────────┐
    │  S3 Bucket (US)   │──────────────────► │  S3 Bucket (EU)   │
    │  (Models, Cache)  │  Cross-region      │  (Models, Cache)  │
    └───────────────────┘  replication       └───────────────────┘
```

### Failover Sequence

**Normal Operation (Primary Active):**
1. Route 53 DNS points to US-West cluster: `api.akidb.io → 1.2.3.4` (primary)
2. All traffic served by US-West cluster
3. EU-Central cluster warm standby (pods running, no traffic)
4. Model files synced to EU S3 bucket (cross-region replication)

**Failover Triggered (Primary Down):**
1. Route 53 health check fails (3 consecutive failures, 30s interval)
2. Route 53 auto-updates DNS: `api.akidb.io → 5.6.7.8` (DR)
3. Traffic routes to EU-Central cluster
4. EU cluster already warm (no cold start delay)
5. RTO: ~5 minutes (DNS TTL + propagation)

**Failback (Primary Restored):**
1. US-West cluster restored and healthy
2. Manual or automated failback decision
3. Route 53 DNS updated back to primary
4. Traffic returns to US-West cluster

### Data Replication Strategy

**Model Files (ONNX + TensorRT engines):**
- **Storage:** S3 buckets in both regions
- **Replication:** Cross-region replication (CRR)
- **RPO:** <15 minutes (async replication)
- **Files:** Qwen3 4B ONNX model, TensorRT engines, tokenizers

**Configuration:**
- **Storage:** Kubernetes ConfigMaps/Secrets replicated via ArgoCD
- **Replication:** Git as single source of truth
- **RPO:** Real-time (Git sync)

**Metrics:**
- **Storage:** Prometheus remote write to Thanos (optional Week 8)
- **Replication:** Each cluster has local Prometheus
- **RPO:** <5 minutes (acceptable loss for metrics)

---

## Day-by-Day Implementation Plan

### Day 1: ArgoCD Setup & GitOps Foundation

**Objective:** Install ArgoCD and configure GitOps repository structure

**Tasks:**

1. **Install ArgoCD**

```bash
# Create ArgoCD namespace
kubectl create namespace argocd

# Install ArgoCD
kubectl apply -n argocd -f https://raw.githubusercontent.com/argoproj/argo-cd/stable/manifests/install.yaml

# Wait for pods
kubectl wait --for=condition=Ready pods --all -n argocd --timeout=5m

# Get admin password
kubectl -n argocd get secret argocd-initial-admin-secret -o jsonpath="{.data.password}" | base64 -d

# Port-forward UI
kubectl port-forward -n argocd svc/argocd-server 8080:443 &
```

2. **Configure ArgoCD Repository**

```bash
# Login to ArgoCD CLI
argocd login localhost:8080

# Add Git repository
argocd repo add https://github.com/your-org/akidb-deploy.git \
  --username your-username \
  --password $GITHUB_TOKEN

# Verify
argocd repo list
```

3. **Create GitOps Repository Structure**

```bash
# Create new repo: akidb-deploy
mkdir akidb-deploy
cd akidb-deploy

# Structure
mkdir -p {envs/{dev,staging,prod},apps,base}

# Base Helm chart (template)
cp -r ../akidb2/deploy/helm/akidb-jetson base/

# Environment-specific values
cat > envs/prod/values.yaml <<'EOF'
image:
  repository: harbor.akidb.io/akidb/akidb-rest
  tag: main-abc123

rest:
  replicaCount: 2
  resources:
    requests:
      memory: 8Gi
      nvidia.com/gpu: 1
    limits:
      memory: 16Gi
      nvidia.com/gpu: 1

monitoring:
  enabled: true

autoscaling:
  enabled: true
  minReplicas: 2
  maxReplicas: 5
EOF

# Commit
git init
git add .
git commit -m "Initial GitOps structure"
git remote add origin https://github.com/your-org/akidb-deploy.git
git push -u origin main
```

4. **Create ArgoCD Application**

```yaml
# apps/akidb-prod.yaml
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: akidb-prod
  namespace: argocd
spec:
  project: default
  source:
    repoURL: https://github.com/your-org/akidb-deploy.git
    targetRevision: main
    path: base
    helm:
      valueFiles:
      - ../../envs/prod/values.yaml
  destination:
    server: https://kubernetes.default.svc
    namespace: akidb
  syncPolicy:
    automated:
      prune: true
      selfHeal: true
      allowEmpty: false
    syncOptions:
    - CreateNamespace=true
    retry:
      limit: 5
      backoff:
        duration: 5s
        factor: 2
        maxDuration: 3m
```

```bash
# Deploy ArgoCD Application
kubectl apply -f apps/akidb-prod.yaml

# Watch sync
argocd app get akidb-prod --watch
```

5. **Test GitOps Flow**

```bash
# Update image tag in Git
cd akidb-deploy
sed -i 's/tag: .*/tag: main-def456/' envs/prod/values.yaml
git add envs/prod/values.yaml
git commit -m "Update image tag"
git push

# Watch ArgoCD auto-sync (should deploy new version)
argocd app get akidb-prod --watch
```

**Success Criteria:**
- [ ] ArgoCD installed and accessible
- [ ] GitOps repo structured correctly
- [ ] ArgoCD Application syncing from Git
- [ ] Manual tag update triggers auto-deployment
- [ ] Pods updated to new image version

**Completion:** `automatosx/tmp/jetson-thor-week7-day1-completion.md`

---

### Day 2: CI/CD Pipeline with GitHub Actions

**Objective:** Automate build, test, and image push on every commit

**Tasks:**

1. **Create GitHub Actions Workflows**

```bash
cd akidb2
mkdir -p .github/workflows

# PR workflow
cat > .github/workflows/pr.yml <<'EOF'
name: Pull Request CI

on:
  pull_request:
    branches: [main]

jobs:
  build-and-test:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: aarch64-unknown-linux-gnu

      - name: Cache Cargo
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Install cross-compilation tools
        run: |
          sudo apt-get update
          sudo apt-get install -y gcc-aarch64-linux-gnu

      - name: Build
        run: cargo build --workspace --release --target aarch64-unknown-linux-gnu

      - name: Test
        run: cargo test --workspace

      - name: Clippy
        run: cargo clippy --all-targets --all-features -- -D warnings

      - name: Format check
        run: cargo fmt --all -- --check

      - name: Security audit
        uses: rustsec/audit-check@v1
        with:
          token: ${{ secrets.GITHUB_TOKEN }}
EOF

# Release workflow
cat > .github/workflows/release.yml <<'EOF'
name: Release

on:
  push:
    branches: [main]

jobs:
  build-and-push:
    runs-on: ubuntu-latest

    permissions:
      contents: write
      packages: write

    steps:
      - uses: actions/checkout@v4
        with:
          token: ${{ secrets.GH_PAT }}

      - name: Set up QEMU
        uses: docker/setup-qemu-action@v3

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Login to Harbor
        uses: docker/login-action@v3
        with:
          registry: harbor.akidb.io
          username: ${{ secrets.HARBOR_USERNAME }}
          password: ${{ secrets.HARBOR_PASSWORD }}

      - name: Generate image tag
        id: image_tag
        run: |
          SHORT_SHA=$(git rev-parse --short HEAD)
          echo "tag=main-${SHORT_SHA}" >> $GITHUB_OUTPUT

      - name: Build and push REST
        uses: docker/build-push-action@v5
        with:
          context: .
          file: docker/Dockerfile.jetson-rest
          platforms: linux/arm64
          push: true
          tags: |
            harbor.akidb.io/akidb/akidb-rest:${{ steps.image_tag.outputs.tag }}
            harbor.akidb.io/akidb/akidb-rest:latest
          cache-from: type=gha
          cache-to: type=gha,mode=max

      - name: Build and push gRPC
        uses: docker/build-push-action@v5
        with:
          context: .
          file: docker/Dockerfile.jetson-grpc
          platforms: linux/arm64
          push: true
          tags: |
            harbor.akidb.io/akidb/akidb-grpc:${{ steps.image_tag.outputs.tag }}
            harbor.akidb.io/akidb/akidb-grpc:latest

      - name: Update GitOps repo
        env:
          GH_TOKEN: ${{ secrets.GH_PAT }}
        run: |
          git clone https://github.com/your-org/akidb-deploy.git
          cd akidb-deploy

          NEW_TAG="${{ steps.image_tag.outputs.tag }}"
          sed -i "s/tag: .*/tag: $NEW_TAG/" envs/prod/values.yaml

          git config user.name "GitHub Actions"
          git config user.email "actions@github.com"
          git add envs/prod/values.yaml
          git commit -m "chore: update image tag to $NEW_TAG [skip ci]"
          git push
EOF

git add .github/workflows/
git commit -m "Add CI/CD workflows"
git push
```

2. **Setup Harbor Registry**

```bash
# Install Harbor (on separate VM or use cloud service)
# For demo, use Docker Hub or GitHub Container Registry

# Create Harbor robot account
# UI: Administration → Robot Accounts → New Robot Account
# Name: github-actions
# Permissions: Push/Pull artifacts

# Add secrets to GitHub repo
# Settings → Secrets → Actions
# Add:
# - HARBOR_USERNAME: robot$github-actions
# - HARBOR_PASSWORD: <token>
# - GH_PAT: <personal access token with repo write>
```

3. **Test CI/CD Pipeline**

```bash
# Create test branch
git checkout -b test/cicd
echo "// test change" >> crates/akidb-rest/src/main.rs
git add .
git commit -m "test: trigger CI/CD"
git push origin test/cicd

# Open PR on GitHub
# → Should trigger pr.yml workflow
# → Check Actions tab for build status

# Merge PR
# → Should trigger release.yml workflow
# → Builds images, pushes to Harbor
# → Updates GitOps repo → ArgoCD deploys
```

**Success Criteria:**
- [ ] PR workflow runs on pull requests
- [ ] All tests pass (build, test, clippy, fmt, audit)
- [ ] Release workflow runs on merge to main
- [ ] Docker images pushed to Harbor
- [ ] GitOps repo updated with new image tag
- [ ] ArgoCD auto-deploys new version

**Completion:** `automatosx/tmp/jetson-thor-week7-day2-completion.md`

---

### Day 3: Blue-Green Deployments

**Objective:** Implement zero-downtime blue-green deployments

**Tasks:**

1. **Create Blue-Green Deployment Script**

```bash
cat > scripts/blue-green-deploy.sh <<'EOF'
#!/bin/bash
set -e

NAMESPACE=${1:-akidb}
NEW_VERSION=${2:-latest}
DEPLOYMENT_NAME="akidb-rest"

echo "Blue-Green Deployment: $DEPLOYMENT_NAME to version $NEW_VERSION"

# Step 1: Deploy GREEN (new version)
echo "1. Deploying GREEN version..."
kubectl apply -f - <<YAML
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ${DEPLOYMENT_NAME}-green
  namespace: $NAMESPACE
  labels:
    app: akidb-rest
    version: green
spec:
  replicas: 2
  selector:
    matchLabels:
      app: akidb-rest
      version: green
  template:
    metadata:
      labels:
        app: akidb-rest
        version: green
    spec:
      containers:
      - name: akidb-rest
        image: harbor.akidb.io/akidb/akidb-rest:$NEW_VERSION
        ports:
        - containerPort: 8080
YAML

# Step 2: Wait for GREEN to be ready
echo "2. Waiting for GREEN to be ready..."
kubectl wait --for=condition=Available \
  deployment/${DEPLOYMENT_NAME}-green \
  -n $NAMESPACE \
  --timeout=5m

# Step 3: Run health checks
echo "3. Running health checks on GREEN..."
GREEN_POD=$(kubectl get pod -n $NAMESPACE -l app=akidb-rest,version=green -o jsonpath="{.items[0].metadata.name}")
kubectl exec -n $NAMESPACE $GREEN_POD -- curl -f http://localhost:8080/health

# Step 4: Switch service to GREEN
echo "4. Switching traffic to GREEN..."
kubectl patch service ${DEPLOYMENT_NAME} -n $NAMESPACE -p \
  '{"spec":{"selector":{"version":"green"}}}'

# Step 5: Wait and monitor
echo "5. Monitoring for 60 seconds..."
sleep 60

# Step 6: Check metrics
echo "6. Checking error rate..."
ERROR_RATE=$(curl -s 'http://localhost:9090/api/v1/query?query=sum(rate(akidb_embed_requests_total{status="error"}[5m])) / sum(rate(akidb_embed_requests_total[5m]))' | jq -r '.data.result[0].value[1]')

if (( $(echo "$ERROR_RATE > 0.01" | bc -l) )); then
  echo "❌ Error rate too high ($ERROR_RATE), rolling back..."
  kubectl patch service ${DEPLOYMENT_NAME} -n $NAMESPACE -p \
    '{"spec":{"selector":{"version":"blue"}}}'
  exit 1
fi

# Step 7: Delete BLUE (old version)
echo "7. Deleting BLUE version..."
kubectl delete deployment ${DEPLOYMENT_NAME}-blue -n $NAMESPACE

# Step 8: Rename GREEN to BLUE
echo "8. Renaming GREEN to BLUE..."
kubectl label deployment ${DEPLOYMENT_NAME}-green version=blue --overwrite -n $NAMESPACE

echo "✅ Blue-Green deployment complete!"
EOF

chmod +x scripts/blue-green-deploy.sh
```

2. **Test Blue-Green Deployment**

```bash
# Initial deployment (BLUE)
kubectl apply -f deploy/helm/akidb-jetson/templates/deployment-rest.yaml
kubectl label deployment akidb-rest version=blue -n akidb

# Run blue-green deployment
bash scripts/blue-green-deploy.sh akidb main-abc123

# Verify traffic switch
kubectl get svc akidb-rest -n akidb -o yaml | grep -A 2 selector
```

3. **Integrate with ArgoCD**

```yaml
# Update ArgoCD Application for blue-green
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: akidb-prod
spec:
  # ... existing config
  syncPolicy:
    automated:
      prune: false  # Don't auto-delete old deployment
      selfHeal: true
    syncOptions:
    - CreateNamespace=true
    - RespectIgnoreDifferences=true
  # Post-sync hook for traffic switch
  sync:
    hooks:
    - name: post-sync-traffic-switch
      hook: PostSync
      manifest: |
        apiVersion: batch/v1
        kind: Job
        metadata:
          name: traffic-switch
        spec:
          template:
            spec:
              containers:
              - name: switch
                image: bitnami/kubectl
                command:
                - /bin/sh
                - -c
                - |
                  kubectl patch service akidb-rest -p '{"spec":{"selector":{"version":"green"}}}'
              restartPolicy: Never
```

**Success Criteria:**
- [ ] Blue-green script deploys new version (green)
- [ ] Health checks pass before traffic switch
- [ ] Traffic switches to green in <1 second
- [ ] Old version (blue) kept for rollback
- [ ] Automatic rollback on high error rate
- [ ] ArgoCD integration working

**Completion:** `automatosx/tmp/jetson-thor-week7-day3-completion.md`

---

### Day 4: Canary Releases with Flagger

**Objective:** Implement progressive canary deployments

**Tasks:**

1. **Install Flagger**

```bash
# Install Flagger
kubectl apply -k github.com/fluxcd/flagger//kustomize/kubernetes

# Verify
kubectl get pods -n flagger-system
```

2. **Create Canary Resource**

```yaml
# deploy/canary/akidb-canary.yaml
apiVersion: flagger.app/v1beta1
kind: Canary
metadata:
  name: akidb-rest
  namespace: akidb
spec:
  # Deployment reference
  targetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: akidb-rest

  # Service reference
  service:
    port: 8080

  # Progressive traffic shift
  analysis:
    interval: 1m
    threshold: 5  # Max failed checks before rollback
    maxWeight: 50  # Max canary weight
    stepWeight: 10  # Traffic increment per interval

    # Metrics for promotion/rollback
    metrics:
    - name: request-success-rate
      thresholdRange:
        min: 99  # 99% success rate required
      interval: 1m

    - name: request-duration
      thresholdRange:
        max: 500  # P99 < 500ms
      interval: 1m

    # Prometheus queries
    metricsServer: http://prometheus:9090

  # Webhooks for notifications
  webhooks:
  - name: slack
    url: $SLACK_WEBHOOK_URL
    metadata:
      channel: deployments
```

```bash
kubectl apply -f deploy/canary/akidb-canary.yaml
```

3. **Configure Prometheus Metrics**

```yaml
# deploy/canary/prometheus-queries.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: flagger-metrics
  namespace: akidb
data:
  request-success-rate: |
    sum(rate(akidb_embed_requests_total{status="success"}[1m])) /
    sum(rate(akidb_embed_requests_total[1m])) * 100

  request-duration: |
    histogram_quantile(0.99,
      rate(akidb_embed_latency_seconds_bucket[1m])
    ) * 1000
```

4. **Test Canary Release**

```bash
# Trigger canary by updating image
kubectl set image deployment/akidb-rest \
  akidb-rest=harbor.akidb.io/akidb/akidb-rest:main-new123 \
  -n akidb

# Watch canary progress
watch kubectl get canary -n akidb

# Monitor traffic split
watch 'kubectl get pods -n akidb -l app=akidb-rest -L version'

# Expected progression:
# t=0:   primary: 100%, canary: 0%
# t=1m:  primary: 90%,  canary: 10%
# t=2m:  primary: 80%,  canary: 20%
# t=3m:  primary: 70%,  canary: 30%
# t=4m:  primary: 60%,  canary: 40%
# t=5m:  primary: 50%,  canary: 50%
# t=6m:  primary: 0%,   canary: 100% (promotion)
```

5. **Test Automatic Rollback**

```bash
# Deploy bad version (simulate high error rate)
kubectl set image deployment/akidb-rest \
  akidb-rest=harbor.akidb.io/akidb/akidb-rest:bad-version \
  -n akidb

# Flagger should detect high error rate and rollback
# Watch logs
kubectl logs -n flagger-system deployment/flagger -f
```

**Success Criteria:**
- [ ] Flagger installed and running
- [ ] Canary resource created
- [ ] Progressive rollout: 10% → 20% → ... → 100%
- [ ] Metrics evaluated at each step
- [ ] Automatic promotion on success
- [ ] Automatic rollback on high error rate
- [ ] Slack notifications working

**Completion:** `automatosx/tmp/jetson-thor-week7-day4-completion.md`

---

### Day 5: Multi-Region Deployment & Disaster Recovery

**Objective:** Deploy to 2 regions with automated failover

**Tasks:**

1. **Setup Second Cluster (DR)**

```bash
# Provision second K8s cluster in EU region
# (Using cloud provider or kubeadm)

# Get kubeconfig
export KUBECONFIG=~/.kube/config-eu

# Install ArgoCD in DR cluster
kubectl create namespace argocd
kubectl apply -n argocd -f https://raw.githubusercontent.com/argoproj/argo-cd/stable/manifests/install.yaml

# Add DR cluster to primary ArgoCD
argocd cluster add eu-central-cluster \
  --name dr-cluster
```

2. **Create Multi-Region ArgoCD ApplicationSet**

```yaml
# apps/akidb-multi-region.yaml
apiVersion: argoproj.io/v1alpha1
kind: ApplicationSet
metadata:
  name: akidb-multi-region
  namespace: argocd
spec:
  generators:
  - list:
      elements:
      - cluster: us-west
        url: https://kubernetes.default.svc
        region: us-west-1
        environment: production
      - cluster: eu-central
        url: https://eu-central.k8s.akidb.io
        region: eu-central-1
        environment: dr

  template:
    metadata:
      name: 'akidb-{{cluster}}'
    spec:
      project: default
      source:
        repoURL: https://github.com/your-org/akidb-deploy.git
        targetRevision: main
        path: base
        helm:
          valueFiles:
          - ../../envs/{{environment}}/values.yaml
          parameters:
          - name: region
            value: '{{region}}'
      destination:
        server: '{{url}}'
        namespace: akidb
      syncPolicy:
        automated:
          prune: true
          selfHeal: true
```

```bash
kubectl apply -f apps/akidb-multi-region.yaml

# Verify both clusters synced
argocd app list
```

3. **Setup S3 Cross-Region Replication**

```bash
# Create S3 buckets
aws s3 mb s3://akidb-models-us-west --region us-west-1
aws s3 mb s3://akidb-models-eu-central --region eu-central-1

# Enable versioning (required for CRR)
aws s3api put-bucket-versioning \
  --bucket akidb-models-us-west \
  --versioning-configuration Status=Enabled

aws s3api put-bucket-versioning \
  --bucket akidb-models-eu-central \
  --versioning-configuration Status=Enabled

# Create replication configuration
cat > replication.json <<'EOF'
{
  "Role": "arn:aws:iam::ACCOUNT:role/s3-replication-role",
  "Rules": [{
    "Status": "Enabled",
    "Priority": 1,
    "DeleteMarkerReplication": { "Status": "Enabled" },
    "Filter": {},
    "Destination": {
      "Bucket": "arn:aws:s3:::akidb-models-eu-central",
      "ReplicationTime": {
        "Status": "Enabled",
        "Time": { "Minutes": 15 }
      }
    }
  }]
}
EOF

aws s3api put-bucket-replication \
  --bucket akidb-models-us-west \
  --replication-configuration file://replication.json
```

4. **Setup Route 53 Failover**

```bash
# Create health check for primary
aws route53 create-health-check \
  --health-check-config \
    IPAddress=1.2.3.4,\
    Port=443,\
    Type=HTTPS,\
    ResourcePath=/health,\
    RequestInterval=30,\
    FailureThreshold=3

# Get health check ID
HEALTH_CHECK_ID=<id>

# Create Route 53 hosted zone records
cat > route53-records.json <<'EOF'
{
  "Changes": [
    {
      "Action": "CREATE",
      "ResourceRecordSet": {
        "Name": "api.akidb.io",
        "Type": "A",
        "SetIdentifier": "Primary-US-West",
        "Failover": "PRIMARY",
        "HealthCheckId": "HEALTH_CHECK_ID",
        "TTL": 60,
        "ResourceRecords": [{ "Value": "1.2.3.4" }]
      }
    },
    {
      "Action": "CREATE",
      "ResourceRecordSet": {
        "Name": "api.akidb.io",
        "Type": "A",
        "SetIdentifier": "Secondary-EU-Central",
        "Failover": "SECONDARY",
        "TTL": 60,
        "ResourceRecords": [{ "Value": "5.6.7.8" }]
      }
    }
  ]
}
EOF

aws route53 change-resource-record-sets \
  --hosted-zone-id Z123456 \
  --change-batch file://route53-records.json
```

5. **Test Disaster Recovery**

```bash
# Test 1: Verify both clusters running
kubectl --context=us-west get pods -n akidb
kubectl --context=eu-central get pods -n akidb

# Test 2: Simulate primary failure
kubectl --context=us-west scale deployment akidb-rest --replicas=0 -n akidb

# Test 3: Wait for Route 53 failover (~3 minutes)
watch dig api.akidb.io +short

# Test 4: Verify traffic going to DR
curl https://api.akidb.io/health

# Test 5: Restore primary
kubectl --context=us-west scale deployment akidb-rest --replicas=2 -n akidb

# Test 6: Manual failback
# Update Route 53 or wait for auto-failback
```

6. **Create DR Runbook**

```bash
cat > docs/DR-RUNBOOK.md <<'EOF'
# Disaster Recovery Runbook

## Failover Procedure

**RTO:** 5 minutes
**RPO:** 1 hour

### Automatic Failover (Route 53)

1. Primary cluster failure detected by health checks (3 failures, 90 seconds)
2. Route 53 automatically switches DNS to DR cluster
3. Traffic flows to EU-Central cluster
4. Monitor: `watch dig api.akidb.io +short`

### Manual Failover

If automatic failover fails:

```bash
# 1. Update Route 53
aws route53 change-resource-record-sets \
  --hosted-zone-id Z123456 \
  --change-batch file://failover-to-dr.json

# 2. Verify DNS propagation
dig api.akidb.io +short

# 3. Scale up DR if needed
kubectl --context=eu-central scale deployment akidb-rest --replicas=5 -n akidb
```

### Failback Procedure

1. Verify primary cluster healthy
2. Sync latest models from S3 EU to S3 US
3. Update Route 53 to point back to primary
4. Monitor for 1 hour
5. Scale down DR to standby (2 replicas)

---

**Report Generated:** $(date)
EOF
```

**Success Criteria:**
- [ ] 2 clusters deployed (US-West, EU-Central)
- [ ] Both clusters synced via ArgoCD ApplicationSet
- [ ] S3 cross-region replication working
- [ ] Route 53 health checks configured
- [ ] Automatic failover tested (DNS switches)
- [ ] Manual failover procedure documented
- [ ] RTO <5 minutes achieved
- [ ] DR runbook created

**Completion:** `automatosx/tmp/jetson-thor-week7-completion-report.md`

---

## GitOps with ArgoCD

### Best Practices

1. **Repository Structure:**
   - Separate Git repos for application code and deployment manifests
   - Environment-specific values files (dev, staging, prod)
   - Base Helm charts with overlays

2. **Auto-Sync:**
   - Enable automated sync for production
   - Use sync waves for ordered deployments
   - Implement health checks before sync

3. **RBAC:**
   - Separate ArgoCD projects for teams
   - Least privilege access
   - Audit logging enabled

4. **Secrets Management:**
   - Use Sealed Secrets or External Secrets Operator
   - Never commit plaintext secrets to Git
   - Rotate secrets regularly

---

## Deployment Strategies

### Comparison Matrix

| Strategy | Downtime | Rollback Speed | Resource Cost | Complexity | Use Case |
|----------|----------|----------------|---------------|------------|----------|
| **Rolling** | Zero | Medium (gradual) | Low (1x) | Low | Default strategy |
| **Blue-Green** | Zero | Instant (<1s) | High (2x) | Medium | Critical updates |
| **Canary** | Zero | Automatic | Medium (1.1-1.5x) | High | Risk mitigation |

### When to Use Each

**Rolling:**
- Default for most deployments
- Low risk changes
- Resource-constrained environments

**Blue-Green:**
- Database migrations
- Major version upgrades
- Need instant rollback capability

**Canary:**
- New features with unknown impact
- A/B testing
- Progressive rollout to subset of users

---

## Disaster Recovery

### RTO/RPO Targets

| Tier | RTO | RPO | Strategy | Cost |
|------|-----|-----|----------|------|
| **Bronze** | <1 hour | <24 hours | Cold standby | Low |
| **Silver** | <15 minutes | <4 hours | Warm standby | Medium |
| **Gold** | <5 minutes | <1 hour | Warm standby + DNS failover | High |
| **Platinum** | <1 minute | <15 minutes | Active-active | Very High |

**Week 7 Target:** Silver-Gold (RTO <5min, RPO <1hr)

### Disaster Scenarios

1. **Primary Cluster Failure:**
   - Trigger: Health check failures
   - Action: Route 53 auto-failover to DR
   - Recovery: 3-5 minutes

2. **Region Outage:**
   - Trigger: All health checks fail
   - Action: Manual failover to DR region
   - Recovery: 5-15 minutes

3. **Data Corruption:**
   - Trigger: Manual detection
   - Action: Restore from S3 backup
   - Recovery: 30-60 minutes

---

## Risk Management

### Production Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| **ArgoCD sync fails** | High | Medium | Manual sync, rollback to previous version |
| **CI/CD pipeline broken** | Medium | Medium | Manual deployment fallback, fix pipeline |
| **Blue-green switch fails** | Critical | Low | Instant rollback, health check validation |
| **Canary auto-rollback false positive** | Medium | Medium | Tune metrics thresholds, manual override |
| **Multi-region sync lag** | High | Low | S3 replication monitoring, manual sync |
| **DNS failover delay** | High | Low | Reduce TTL to 60s, monitor health checks |
| **Harbor registry down** | Critical | Low | Multi-region registry replication |

### Rollback Procedures

**ArgoCD Rollback:**
```bash
argocd app rollback akidb-prod <revision>
argocd app sync akidb-prod
```

**Blue-Green Rollback:**
```bash
kubectl patch service akidb-rest -p '{"spec":{"selector":{"version":"blue"}}}'
```

**Canary Rollback:**
```bash
# Automatic by Flagger, or manual:
kubectl delete canary akidb-rest -n akidb
```

---

## Success Criteria

### Week 7 Completion Criteria

| Criterion | Target | Measurement | Priority |
|-----------|--------|-------------|----------|
| **ArgoCD Setup** | Working GitOps | Applications syncing | P0 |
| **CI Pipeline** | Auto-build on PR | GitHub Actions passing | P0 |
| **CD Pipeline** | Auto-deploy on merge | Images pushed, deployed | P0 |
| **Blue-Green** | Zero downtime | Traffic switch <1s | P0 |
| **Canary** | Progressive rollout | 10% → 50% → 100% | P0 |
| **Multi-Region** | 2 clusters deployed | US-West + EU-Central | P0 |
| **Automated Failover** | DNS-based | RTO <5min | P0 |
| **S3 Replication** | Cross-region sync | RPO <1hr | P1 |
| **Rollback Success** | Instant blue-green | <5s | P1 |
| **Auto-Rollback** | On high error rate | Canary detects | P1 |
| **DR Runbook** | Complete procedures | 10+ scenarios | P2 |

**Overall Success:** All P0 criteria + 80% of P1 criteria + 50% of P2 criteria

---

## Appendix: Code Examples

### Example 1: Complete GitHub Actions Workflow

(See Day 2 implementation plan for full workflows)

### Example 2: ArgoCD Application with Sync Waves

```yaml
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: akidb-prod
  annotations:
    # Sync wave order: namespace → secrets → deployments → services
    argocd.argoproj.io/sync-wave: "0"
spec:
  # ... (see Day 1 for full spec)
```

### Example 3: Sealed Secret for Harbor Credentials

```bash
# Install Sealed Secrets controller
kubectl apply -f https://github.com/bitnami-labs/sealed-secrets/releases/download/v0.24.0/controller.yaml

# Create secret
kubectl create secret docker-registry harbor-creds \
  --docker-server=harbor.akidb.io \
  --docker-username=robot$github-actions \
  --docker-password=$HARBOR_TOKEN \
  -n akidb \
  --dry-run=client -o yaml | \
  kubeseal -o yaml > sealed-harbor-creds.yaml

# Commit to Git
git add sealed-harbor-creds.yaml
git commit -m "Add sealed Harbor credentials"
git push
```

---

**End of Week 7 PRD**

**Next Steps:** Week 8 - Active-Active Multi-Region & Service Mesh
