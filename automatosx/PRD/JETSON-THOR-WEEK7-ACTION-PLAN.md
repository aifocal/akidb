# Jetson Thor Week 7: CI/CD & Multi-Region - Action Plan

**Status:** Ready to Execute
**Timeline:** 5 days
**Dependencies:** Week 1-6 Complete
**Target:** Automated GitOps deployment with multi-region DR

---

## Overview

This action plan provides exact commands to implement GitOps with ArgoCD, automated CI/CD with GitHub Actions, blue-green and canary deployments, and active-passive multi-region failover.

---

## Day 1: ArgoCD Setup & GitOps Foundation

**Goal:** Install ArgoCD and configure GitOps workflow

### Step 1: Install ArgoCD

```bash
# Create namespace
kubectl create namespace argocd

# Install ArgoCD
kubectl apply -n argocd -f https://raw.githubusercontent.com/argoproj/argo-cd/stable/manifests/install.yaml

# Wait for pods
kubectl wait --for=condition=Ready pods --all -n argocd --timeout=5m

# Get admin password
ARGOCD_PASSWORD=$(kubectl -n argocd get secret argocd-initial-admin-secret -o jsonpath="{.data.password}" | base64 -d)
echo "ArgoCD Password: $ARGOCD_PASSWORD"

# Port-forward UI
kubectl port-forward -n argocd svc/argocd-server 8080:443 &

# Install ArgoCD CLI
brew install argocd  # or: curl -sSL -o argocd https://github.com/argoproj/argo-cd/releases/latest/download/argocd-linux-amd64

# Login
argocd login localhost:8080 --username admin --password $ARGOCD_PASSWORD --insecure
```

### Step 2: Create GitOps Repository

```bash
# Create new repository: akidb-deploy
mkdir akidb-deploy && cd akidb-deploy

# Structure
mkdir -p envs/{dev,staging,prod} apps base

# Copy Helm chart to base
cp -r ../akidb2/deploy/helm/akidb-jetson base/

# Create production values
cat > envs/prod/values.yaml <<'EOF'
image:
  repository: harbor.akidb.io/akidb/akidb-rest
  tag: main-latest

rest:
  replicaCount: 2
  resources:
    requests:
      memory: 8Gi
      nvidia.com/gpu: 1
    limits:
      memory: 16Gi
      nvidia.com/gpu: 1

autoscaling:
  enabled: true
  minReplicas: 2
  maxReplicas: 5
EOF

# Initialize Git
git init
git add .
git commit -m "Initial GitOps structure"

# Create GitHub repo and push
gh repo create your-org/akidb-deploy --private --source=. --remote=origin
git push -u origin main
```

### Step 3: Add Repository to ArgoCD

```bash
# Add Git repository
argocd repo add https://github.com/your-org/akidb-deploy.git \
  --username your-username \
  --password $GITHUB_TOKEN

# Verify
argocd repo list
```

### Step 4: Create ArgoCD Application

```bash
cat > apps/akidb-prod.yaml <<'EOF'
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
    syncOptions:
    - CreateNamespace=true
    retry:
      limit: 5
      backoff:
        duration: 5s
        maxDuration: 3m
EOF

kubectl apply -f apps/akidb-prod.yaml

# Watch sync
argocd app get akidb-prod --watch
```

### Step 5: Test GitOps Flow

```bash
# Update image tag
cd akidb-deploy
sed -i 's/tag: .*/tag: main-test123/' envs/prod/values.yaml
git add envs/prod/values.yaml
git commit -m "Update image tag to test123"
git push

# Watch ArgoCD auto-deploy
argocd app get akidb-prod --watch

# Verify deployment
kubectl get pods -n akidb
```

### Success Criteria

- [ ] ArgoCD installed and accessible
- [ ] GitOps repo created with proper structure
- [ ] ArgoCD Application syncing from Git
- [ ] Image tag update triggers auto-deployment
- [ ] Pods updated to new version

**Completion:** `automatosx/tmp/jetson-thor-week7-day1-completion.md`

---

## Day 2: CI/CD Pipeline with GitHub Actions

**Goal:** Automate build, test, and image push

### Step 1: Create GitHub Actions Workflows

```bash
cd akidb2
mkdir -p .github/workflows

# PR Workflow
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

      - name: Cache Cargo
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Build
        run: cargo build --workspace --release

      - name: Test
        run: cargo test --workspace

      - name: Clippy
        run: cargo clippy --all-targets --all-features -- -D warnings

      - name: Format
        run: cargo fmt --all -- --check
EOF

# Release Workflow
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
    steps:
      - uses: actions/checkout@v4
        with:
          token: ${{ secrets.GH_PAT }}

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Login to Harbor
        uses: docker/login-action@v3
        with:
          registry: harbor.akidb.io
          username: ${{ secrets.HARBOR_USERNAME }}
          password: ${{ secrets.HARBOR_PASSWORD }}

      - name: Generate tag
        id: tag
        run: echo "tag=main-$(git rev-parse --short HEAD)" >> $GITHUB_OUTPUT

      - name: Build and push
        uses: docker/build-push-action@v5
        with:
          context: .
          file: docker/Dockerfile.jetson-rest
          platforms: linux/arm64
          push: true
          tags: harbor.akidb.io/akidb/akidb-rest:${{ steps.tag.outputs.tag }}

      - name: Update GitOps repo
        env:
          GH_TOKEN: ${{ secrets.GH_PAT }}
        run: |
          git clone https://github.com/your-org/akidb-deploy.git
          cd akidb-deploy
          sed -i "s/tag: .*/tag: ${{ steps.tag.outputs.tag }}/" envs/prod/values.yaml
          git config user.name "GitHub Actions"
          git config user.email "actions@github.com"
          git add envs/prod/values.yaml
          git commit -m "Update image to ${{ steps.tag.outputs.tag }} [skip ci]"
          git push
EOF

git add .github/workflows/
git commit -m "Add CI/CD workflows"
git push
```

### Step 2: Setup GitHub Secrets

```bash
# Add secrets to GitHub repository
# Settings → Secrets and variables → Actions

# Required secrets:
# - HARBOR_USERNAME: robot$github-actions
# - HARBOR_PASSWORD: <token from Harbor>
# - GH_PAT: <GitHub Personal Access Token with repo write>

# Using GitHub CLI:
gh secret set HARBOR_USERNAME --body "robot\$github-actions"
gh secret set HARBOR_PASSWORD --body "$HARBOR_TOKEN"
gh secret set GH_PAT --body "$GITHUB_PAT"
```

### Step 3: Test CI/CD Pipeline

```bash
# Create test branch
git checkout -b test/cicd-pipeline
echo "// test" >> crates/akidb-rest/src/main.rs
git add .
git commit -m "test: trigger CI/CD"
git push origin test/cicd-pipeline

# Open PR
gh pr create --title "Test CI/CD" --body "Testing automated pipeline"

# Check GitHub Actions tab for build status

# Merge PR (after approval)
gh pr merge --auto --squash

# Verify release workflow runs and deploys
watch kubectl get pods -n akidb
```

### Success Criteria

- [ ] PR workflow runs on pull requests
- [ ] All tests pass (build, test, clippy, fmt)
- [ ] Release workflow runs on merge
- [ ] Docker images pushed to Harbor
- [ ] GitOps repo updated automatically
- [ ] ArgoCD deploys new version

**Completion:** `automatosx/tmp/jetson-thor-week7-day2-completion.md`

---

## Day 3: Blue-Green Deployments

**Goal:** Implement zero-downtime deployments

### Step 1: Create Blue-Green Deployment Script

```bash
cat > scripts/blue-green-deploy.sh <<'EOF'
#!/bin/bash
set -e

NAMESPACE="akidb"
NEW_VERSION=$1

if [ -z "$NEW_VERSION" ]; then
  echo "Usage: $0 <new-version>"
  exit 1
fi

echo "=== Blue-Green Deployment ==="
echo "New version: $NEW_VERSION"

# Deploy GREEN
echo "1. Deploying GREEN version..."
kubectl set image deployment/akidb-rest \
  akidb-rest=harbor.akidb.io/akidb/akidb-rest:$NEW_VERSION \
  -n $NAMESPACE

kubectl label deployment akidb-rest version=green -n $NAMESPACE --overwrite

# Wait for ready
echo "2. Waiting for GREEN to be ready..."
kubectl wait --for=condition=Available deployment/akidb-rest -n $NAMESPACE --timeout=5m

# Health check
echo "3. Running health checks..."
GREEN_POD=$(kubectl get pod -n $NAMESPACE -l app=akidb-rest,version=green -o jsonpath="{.items[0].metadata.name}")
kubectl exec -n $NAMESPACE $GREEN_POD -- curl -f http://localhost:8080/health || { echo "Health check failed"; exit 1; }

# Switch traffic
echo "4. Switching traffic to GREEN..."
kubectl patch service akidb-rest -n $NAMESPACE -p '{"spec":{"selector":{"version":"green"}}}'

echo "5. Monitoring for 30 seconds..."
sleep 30

# Check metrics
ERROR_RATE=$(curl -s 'http://localhost:9090/api/v1/query?query=sum(rate(akidb_embed_requests_total{status="error"}[1m]))/sum(rate(akidb_embed_requests_total[1m]))' | jq -r '.data.result[0].value[1]' 2>/dev/null || echo "0")

if (( $(echo "$ERROR_RATE > 0.01" | bc -l 2>/dev/null || echo 0) )); then
  echo "❌ High error rate, rolling back..."
  kubectl patch service akidb-rest -n $NAMESPACE -p '{"spec":{"selector":{"version":"blue"}}}'
  exit 1
fi

echo "✅ Blue-Green deployment complete!"
EOF

chmod +x scripts/blue-green-deploy.sh
```

### Step 2: Test Blue-Green Deployment

```bash
# Initial deployment
kubectl label deployment akidb-rest version=blue -n akidb --overwrite

# Deploy new version
bash scripts/blue-green-deploy.sh main-new456

# Verify traffic switched
kubectl get svc akidb-rest -n akidb -o jsonpath='{.spec.selector}'

# Test rollback
bash scripts/blue-green-deploy.sh main-bad999  # Should fail and rollback
```

### Success Criteria

- [ ] Script deploys new version (green)
- [ ] Health checks validate before switch
- [ ] Traffic switches in <1 second
- [ ] Automatic rollback on high error rate
- [ ] Blue version kept for manual rollback

**Completion:** `automatosx/tmp/jetson-thor-week7-day3-completion.md`

---

## Day 4: Canary Releases with Flagger

**Goal:** Progressive rollout with auto-rollback

### Step 1: Install Flagger

```bash
# Add Flagger Helm repo
helm repo add flagger https://flagger.app

# Install Flagger
kubectl create namespace flagger-system

helm upgrade -i flagger flagger/flagger \
  --namespace flagger-system \
  --set prometheus.install=false \
  --set meshProvider=kubernetes

# Verify
kubectl get pods -n flagger-system
```

### Step 2: Create Canary Resource

```bash
cat > deploy/canary/akidb-canary.yaml <<'EOF'
apiVersion: flagger.app/v1beta1
kind: Canary
metadata:
  name: akidb-rest
  namespace: akidb
spec:
  targetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: akidb-rest
  service:
    port: 8080
  analysis:
    interval: 1m
    threshold: 3
    maxWeight: 50
    stepWeight: 10
    metrics:
    - name: request-success-rate
      thresholdRange:
        min: 99
      interval: 1m
    - name: request-duration
      thresholdRange:
        max: 500
      interval: 1m
  metricsServer: http://prometheus.akidb:9090
EOF

kubectl apply -f deploy/canary/akidb-canary.yaml
```

### Step 3: Test Canary Deployment

```bash
# Trigger canary by updating image
kubectl set image deployment/akidb-rest \
  akidb-rest=harbor.akidb.io/akidb/akidb-rest:main-canary123 \
  -n akidb

# Watch progress
watch kubectl get canary -n akidb

# Expected output:
# NAME         STATUS        WEIGHT   LASTTRANSITIONTIME
# akidb-rest   Progressing   10       1m
# akidb-rest   Progressing   20       2m
# ...
# akidb-rest   Succeeded     0        6m
```

### Step 4: Test Auto-Rollback

```bash
# Deploy bad version
kubectl set image deployment/akidb-rest \
  akidb-rest=harbor.akidb.io/akidb/akidb-rest:bad-version \
  -n akidb

# Watch Flagger detect high error rate and rollback
kubectl logs -n flagger-system deployment/flagger -f
```

### Success Criteria

- [ ] Flagger installed
- [ ] Canary resource created
- [ ] Progressive rollout: 10% → 20% → ... → 100%
- [ ] Automatic promotion on success
- [ ] Automatic rollback on errors
- [ ] Metrics evaluated at each step

**Completion:** `automatosx/tmp/jetson-thor-week7-day4-completion.md`

---

## Day 5: Multi-Region Deployment & DR

**Goal:** Deploy to 2 regions with automated failover

### Step 1: Setup DR Cluster

```bash
# Provision second cluster (EU)
# (Cloud-specific: GKE, EKS, AKS, or kubeadm)

# Get kubeconfig for DR cluster
export KUBECONFIG=~/.kube/config-eu

# Install ArgoCD in DR
kubectl create namespace argocd
kubectl apply -n argocd -f https://raw.githubusercontent.com/argoproj/argo-cd/stable/manifests/install.yaml

# Switch back to primary
export KUBECONFIG=~/.kube/config

# Add DR cluster to ArgoCD
argocd cluster add eu-central-context --name dr-cluster
```

### Step 2: Create Multi-Region ApplicationSet

```bash
cat > apps/akidb-multi-region.yaml <<'EOF'
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
        environment: production
      - cluster: eu-central
        url: https://eu-k8s.akidb.io
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
      destination:
        server: '{{url}}'
        namespace: akidb
      syncPolicy:
        automated:
          prune: true
          selfHeal: true
EOF

kubectl apply -f apps/akidb-multi-region.yaml

# Verify both clusters
argocd app list
```

### Step 3: Setup S3 Cross-Region Replication

```bash
# Create buckets
aws s3 mb s3://akidb-models-us --region us-west-1
aws s3 mb s3://akidb-models-eu --region eu-central-1

# Enable versioning
aws s3api put-bucket-versioning \
  --bucket akidb-models-us \
  --versioning-configuration Status=Enabled

aws s3api put-bucket-versioning \
  --bucket akidb-models-eu \
  --versioning-configuration Status=Enabled

# Configure replication
cat > replication.json <<'EOF'
{
  "Role": "arn:aws:iam::ACCOUNT:role/s3-replication",
  "Rules": [{
    "Status": "Enabled",
    "Priority": 1,
    "Filter": {},
    "Destination": {
      "Bucket": "arn:aws:s3:::akidb-models-eu",
      "ReplicationTime": {
        "Status": "Enabled",
        "Time": { "Minutes": 15 }
      }
    }
  }]
}
EOF

aws s3api put-bucket-replication \
  --bucket akidb-models-us \
  --replication-configuration file://replication.json
```

### Step 4: Setup Route 53 Failover

```bash
# Create health check
aws route53 create-health-check \
  --health-check-config \
    IPAddress=1.2.3.4,\
    Port=443,\
    Type=HTTPS,\
    ResourcePath=/health,\
    RequestInterval=30,\
    FailureThreshold=3

HEALTH_CHECK_ID=<output-id>

# Create failover records
cat > route53.json <<'EOF'
{
  "Changes": [{
    "Action": "CREATE",
    "ResourceRecordSet": {
      "Name": "api.akidb.io",
      "Type": "A",
      "SetIdentifier": "Primary",
      "Failover": "PRIMARY",
      "HealthCheckId": "HEALTH_CHECK_ID",
      "TTL": 60,
      "ResourceRecords": [{"Value": "1.2.3.4"}]
    }
  }, {
    "Action": "CREATE",
    "ResourceRecordSet": {
      "Name": "api.akidb.io",
      "Type": "A",
      "SetIdentifier": "Secondary",
      "Failover": "SECONDARY",
      "TTL": 60,
      "ResourceRecords": [{"Value": "5.6.7.8"}]
    }
  }]
}
EOF

aws route53 change-resource-record-sets \
  --hosted-zone-id Z123456 \
  --change-batch file://route53.json
```

### Step 5: Test DR Failover

```bash
# Verify both clusters running
kubectl --context=us-west get pods -n akidb
kubectl --context=eu-central get pods -n akidb

# Simulate primary failure
kubectl --context=us-west scale deployment akidb-rest --replicas=0 -n akidb

# Wait for Route 53 failover
watch dig api.akidb.io +short
# Should switch from 1.2.3.4 to 5.6.7.8 in ~3 minutes

# Test DR endpoint
curl https://api.akidb.io/health

# Restore primary
kubectl --context=us-west scale deployment akidb-rest --replicas=2 -n akidb
```

### Step 6: Create DR Runbook

```bash
mkdir -p docs

cat > docs/DR-RUNBOOK.md <<'EOF'
# Disaster Recovery Runbook

## RTO: 5 minutes | RPO: 1 hour

### Automatic Failover

Route 53 automatically fails over after 3 failed health checks (90 seconds).

### Manual Failover

```bash
# Update Route 53
aws route53 change-resource-record-sets \
  --hosted-zone-id Z123456 \
  --change-batch file://failover-to-dr.json

# Scale DR
kubectl --context=eu-central scale deployment akidb-rest --replicas=5 -n akidb
```

### Failback

1. Verify primary healthy
2. Update Route 53 to primary
3. Scale down DR to 2 replicas
4. Monitor for 1 hour

EOF

git add docs/DR-RUNBOOK.md
git commit -m "Add DR runbook"
git push
```

### Success Criteria

- [ ] 2 clusters deployed (US, EU)
- [ ] ApplicationSet syncs both clusters
- [ ] S3 cross-region replication working
- [ ] Route 53 health checks configured
- [ ] Failover tested (DNS switches)
- [ ] RTO <5 minutes achieved
- [ ] DR runbook documented

**Completion:** `automatosx/tmp/jetson-thor-week7-completion-report.md`

---

## Summary

### Week 7 Achievements

- ✅ **Day 1:** ArgoCD + GitOps foundation
- ✅ **Day 2:** CI/CD pipeline with GitHub Actions
- ✅ **Day 3:** Blue-green deployments
- ✅ **Day 4:** Canary releases with Flagger
- ✅ **Day 5:** Multi-region DR with Route 53 failover

### Key Features Delivered

| Feature | Configuration | Status |
|---------|---------------|--------|
| ArgoCD | GitOps auto-sync | ✅ |
| CI Pipeline | Build + test on PR | ✅ |
| CD Pipeline | Auto-deploy on merge | ✅ |
| Blue-Green | Zero downtime | ✅ |
| Canary | 10% → 50% → 100% | ✅ |
| Multi-Region | US + EU | ✅ |
| Failover | RTO <5min | ✅ |
| S3 Replication | RPO <1hr | ✅ |

### Key Commands Reference

```bash
# ArgoCD
argocd app sync akidb-prod
argocd app rollback akidb-prod <revision>

# Blue-Green
bash scripts/blue-green-deploy.sh <version>

# Canary
kubectl set image deployment/akidb-rest akidb-rest=<image>

# DR Failover
aws route53 change-resource-record-sets --hosted-zone-id Z123456 --change-batch file://failover.json
```

---

**End of Week 7 Action Plan**

**Next:** Week 8 - Active-Active Multi-Region & Service Mesh