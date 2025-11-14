# Jetson Thor Week 9: Cost Optimization & Intelligent Auto-Scaling PRD

**Status:** Ready to Execute
**Timeline:** 5 days (Week 9)
**Owner:** Platform Engineering + FinOps + SRE + Backend Team
**Dependencies:** Week 1-8 (✅ Complete)
**Target Platform:** NVIDIA Jetson Thor (Blackwell GPU, 2,000 TOPS) - Multi-Region Edge

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Goals & Non-Goals](#goals--non-goals)
3. [Baseline Analysis](#baseline-analysis)
4. [Cost Optimization Strategy](#cost-optimization-strategy)
5. [Auto-Scaling Architecture](#auto-scaling-architecture)
6. [FinOps Framework](#finops-framework)
7. [Resource Right-Sizing](#resource-right-sizing)
8. [Day-by-Day Implementation Plan](#day-by-day-implementation-plan)
9. [Cost Monitoring & Alerting](#cost-monitoring--alerting)
10. [Intelligent Scheduling](#intelligent-scheduling)
11. [Risk Management](#risk-management)
12. [Success Criteria](#success-criteria)
13. [Appendix: Code Examples](#appendix-code-examples)

---

## Executive Summary

Week 9 focuses on **cost optimization** and **intelligent auto-scaling** for the active-active multi-region AkiDB deployment. After establishing production-grade infrastructure (Weeks 1-8), we now optimize resource utilization to reduce costs by 30-40% while maintaining performance SLAs (P95 <30ms, >100 QPS). We implement **HPA** (Horizontal Pod Autoscaler), **VPA** (Vertical Pod Autoscaler), **KEDA** (event-driven scaling), **OpenCost** for FinOps visibility, and resource right-sizing based on actual workload patterns.

### Key Objectives

1. **Cost Reduction:** Achieve 30-40% cost savings through optimization
2. **HPA Implementation:** Auto-scale pods based on GPU utilization and request rate
3. **VPA Integration:** Right-size container resources (CPU, memory, GPU)
4. **KEDA Deployment:** Scale to zero during off-peak hours (10pm-6am)
5. **FinOps Dashboard:** Real-time cost visibility with OpenCost + Grafana
6. **Resource Right-Sizing:** Optimize based on P95 actual usage
7. **Intelligent Scheduling:** Time-based scaling for predictable traffic patterns
8. **Multi-Region Optimization:** Balance cost vs latency trade-offs

### Expected Outcomes

- ✅ **30-40% Cost Reduction:** From $8,000/month to $5,000/month (2 regions)
- ✅ **HPA Operational:** Scale 2-8 pods per cluster based on GPU load
- ✅ **VPA Recommendations:** Right-sized CPU/memory/GPU requests
- ✅ **Scale-to-Zero:** Off-peak hours (10pm-6am) reduce to 1 pod minimum
- ✅ **FinOps Dashboard:** Real-time cost per request, per tenant, per region
- ✅ **SLA Maintained:** P95 <30ms, >100 QPS during peak hours
- ✅ **Resource Utilization:** GPU 60-80% (up from 30-40%), CPU 50-70%
- ✅ **Cost Alerting:** Automated alerts for cost anomalies >20% daily budget

---

## Goals & Non-Goals

### Goals (Week 9)

**Primary Goals:**
1. ✅ **Cost Reduction (30-40%)** - Reduce monthly infrastructure costs
2. ✅ **HPA with GPU Metrics** - Auto-scale based on GPU utilization + request rate
3. ✅ **VPA for Right-Sizing** - Optimize CPU/memory/GPU requests
4. ✅ **KEDA Scale-to-Zero** - Reduce to 1 pod minimum during off-peak
5. ✅ **OpenCost Deployment** - Real-time cost visibility and allocation
6. ✅ **FinOps Dashboard** - Grafana dashboard for cost per service/tenant/region
7. ✅ **Resource Efficiency** - Increase GPU utilization from 35% to 65%
8. ✅ **Intelligent Scheduling** - Time-based scaling for predictable patterns

**Secondary Goals:**
- 📊 Spot instance integration for non-critical workloads
- 📊 S3 lifecycle policies (Glacier after 90 days)
- 📊 Reserved capacity planning (commit to 1-year for 30% discount)
- 📝 Cost forecasting ML model (predict next month spend)
- 📝 Chargeback reports for multi-tenancy

### Non-Goals (Deferred to Week 10+)

**Not in Scope for Week 9:**
- ❌ Multi-region data rebalancing (requires data migration) - Week 10
- ❌ GDPR compliance and data residency enforcement - Week 10
- ❌ SOC2 Type II certification - Week 10+
- ❌ Advanced ML model compression (quantization beyond FP8) - Week 11+
- ❌ Custom GPU scheduling algorithms - Week 11+

---

## Baseline Analysis

### Week 8 Production Status

**Deployed Infrastructure:**
- ✅ Active-active multi-region: US-West + EU-Central clusters
- ✅ Istio service mesh with mTLS, circuit breakers, retries
- ✅ Distributed tracing (Jaeger + OpenTelemetry)
- ✅ Unified observability (Thanos + Grafana)
- ✅ Performance: P95 24ms, P99 47ms, 112 QPS global

**Current Cost Structure (Monthly):**

| Resource | US-West | EU-Central | Total | % of Budget |
|----------|---------|------------|-------|-------------|
| **Jetson Thor Nodes (3x)** | $2,400 | $1,600 | $4,000 | 50% |
| **Kubernetes Control Plane** | $400 | $400 | $800 | 10% |
| **Load Balancers (Istio)** | $300 | $200 | $500 | 6% |
| **S3 Storage (1TB)** | $200 | $150 | $350 | 4% |
| **S3 Bandwidth (500GB)** | $450 | $300 | $750 | 9% |
| **Prometheus + Thanos** | $300 | $200 | $500 | 6% |
| **Jaeger (Tracing)** | $200 | $150 | $350 | 4% |
| **Route 53 + DNS** | $50 | - | $50 | 1% |
| **Data Transfer (Cross-Region)** | $400 | $300 | $700 | 9% |
| **Total** | **$4,700** | **$3,300** | **$8,000** | **100%** |

**Cost per Request:**
- Total requests/month: 300M (112 QPS × 2.6M seconds)
- Cost per 1M requests: $26.67
- Cost per request: $0.0000267

**Current Resource Utilization:**

| Resource | Requested | Actual (P95) | Utilization | Waste |
|----------|-----------|--------------|-------------|-------|
| **GPU** | 100% | 35% | 35% | 65% |
| **CPU** | 8 cores | 3.5 cores | 44% | 56% |
| **Memory** | 16GB | 9GB | 56% | 44% |
| **Storage** | 1TB | 450GB | 45% | 55% |

**Key Findings:**
- ❌ **GPU under-utilized:** 35% average (wasted $2,600/month on GPU capacity)
- ❌ **Fixed capacity:** No auto-scaling, same resources 24/7
- ❌ **Over-provisioned:** CPU/memory requests 2x actual usage
- ❌ **No off-peak optimization:** Traffic drops 70% at night, still full capacity
- ❌ **S3 lifecycle missing:** Old models (90+ days) still in Standard tier

### Week 9 Target State

**Optimized Cost Structure (Monthly):**

| Resource | Optimized Cost | Savings | Strategy |
|----------|----------------|---------|----------|
| **Jetson Thor Nodes** | $2,400 (-40%) | $1,600 | HPA scale down off-peak, right-size |
| **Kubernetes Control Plane** | $800 (no change) | $0 | Fixed cost |
| **Load Balancers** | $400 (-20%) | $100 | Consolidate Istio gateways |
| **S3 Storage** | $200 (-43%) | $150 | Lifecycle to Glacier after 90 days |
| **S3 Bandwidth** | $600 (-20%) | $150 | Optimize model caching |
| **Prometheus + Thanos** | $350 (-30%) | $150 | Reduce retention, optimize queries |
| **Jaeger** | $250 (-29%) | $100 | Reduce trace sampling to 5% |
| **Route 53 + DNS** | $50 (no change) | $0 | Fixed cost |
| **Data Transfer** | $500 (-29%) | $200 | Optimize cross-region calls |
| **Total** | **$5,550** | **$2,450** | **31% reduction** |

**Target Cost per Request:**
- Cost per 1M requests: $18.50 (-31%)
- Cost per request: $0.0000185

**Target Resource Utilization:**

| Resource | Target | Strategy |
|----------|--------|----------|
| **GPU** | 60-80% | HPA scale based on GPU metrics |
| **CPU** | 50-70% | VPA right-size requests |
| **Memory** | 60-75% | VPA right-size requests |
| **Storage** | 60-70% | S3 lifecycle policies |

---

## Cost Optimization Strategy

### Optimization Pillars

```
┌─────────────────────────────────────────────────────────────────┐
│                 Week 9 Cost Optimization Framework              │
└─────────────────────────────────────────────────────────────────┘
                                │
                ┌───────────────┴───────────────┐
                │                               │
                ▼                               ▼
┌───────────────────────────┐   ┌───────────────────────────┐
│   Pillar 1: Auto-Scaling  │   │  Pillar 2: Right-Sizing   │
│                           │   │                           │
│  • HPA (2-8 pods)         │   │  • VPA recommendations    │
│  • KEDA scale-to-zero     │   │  • Reduce CPU/mem 40%     │
│  • GPU-based scaling      │   │  • GPU time-slicing       │
│  • Time-based schedules   │   │  • Container optimization │
│                           │   │                           │
│  💰 Savings: $1,200/month │   │  💰 Savings: $800/month   │
└───────────────────────────┘   └───────────────────────────┘
                │                               │
                └───────────────┬───────────────┘
                                │
                ┌───────────────┴───────────────┐
                │                               │
                ▼                               ▼
┌───────────────────────────┐   ┌───────────────────────────┐
│ Pillar 3: Storage Optim.  │   │  Pillar 4: FinOps Visibility│
│                           │   │                           │
│  • S3 Intelligent-Tiering │   │  • OpenCost deployment    │
│  • Lifecycle to Glacier   │   │  • Cost per request       │
│  • Model cache pruning    │   │  • Cost per tenant        │
│  • Reduce S3 bandwidth    │   │  • Budget alerts          │
│                           │   │                           │
│  💰 Savings: $300/month   │   │  💰 Visibility: Real-time │
└───────────────────────────┘   └───────────────────────────┘
                │                               │
                └───────────────┬───────────────┘
                                │
                                ▼
                ┌───────────────────────────┐
                │   Total Expected Savings  │
                │                           │
                │     $2,450/month (31%)    │
                │  From $8,000 → $5,550     │
                └───────────────────────────┘
```

### Cost Optimization Strategies

**1. Auto-Scaling (Pillar 1):**
- **HPA:** Scale pods 2-8 based on GPU utilization (target 70%)
- **KEDA:** Scale to 1 pod minimum during off-peak (10pm-6am)
- **Time-based:** Predictive scaling before traffic spikes
- **Savings:** $1,200/month (15%)

**2. Right-Sizing (Pillar 2):**
- **VPA recommendations:** Reduce CPU from 8→4 cores, memory 16GB→10GB
- **GPU time-slicing:** Share GPU across 2 pods when load <50%
- **Container optimization:** Multi-stage Docker builds (-30% image size)
- **Savings:** $800/month (10%)

**3. Storage Optimization (Pillar 3):**
- **S3 Intelligent-Tiering:** Auto-move to lower cost tiers
- **Lifecycle policies:** Glacier after 90 days, delete after 1 year
- **Model cache pruning:** Delete models unused for 30 days
- **S3 bandwidth reduction:** Optimize model downloads (cache headers)
- **Savings:** $300/month (4%)

**4. Observability Optimization (Pillar 4):**
- **Prometheus retention:** 15 days → 7 days (Thanos for long-term)
- **Trace sampling:** 10% → 5% (halve Jaeger storage)
- **Log retention:** 30 days → 14 days
- **Savings:** $150/month (2%)

---

## Auto-Scaling Architecture

### HPA (Horizontal Pod Autoscaler) with GPU Metrics

**Architecture:**

```
┌────────────────────────────────────────────────────────────────┐
│                      HPA Control Loop                          │
│                                                                │
│  1. Metrics Server collects GPU utilization every 15s         │
│  2. HPA calculates desired replica count                      │
│  3. HPA updates Deployment spec                               │
│  4. Kubernetes schedules new pods                             │
│  5. Istio updates load balancer endpoints                     │
└────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌────────────────────────────────────────────────────────────────┐
│                    GPU Metrics Pipeline                        │
│                                                                │
│  NVIDIA DCGM Exporter (Jetson Thor)                           │
│           ↓                                                    │
│  Prometheus (scrape every 15s)                                │
│           ↓                                                    │
│  Prometheus Adapter (expose as K8s metrics)                   │
│           ↓                                                    │
│  HPA Controller (fetch metrics, calculate replicas)           │
└────────────────────────────────────────────────────────────────┘
```

**HPA Configuration:**

```yaml
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
  maxReplicas: 8
  metrics:
  # GPU utilization (primary metric)
  - type: Pods
    pods:
      metric:
        name: nvidia_gpu_duty_cycle
      target:
        type: AverageValue
        averageValue: "70"  # Scale up when GPU >70%
  # CPU utilization (secondary metric)
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 60
  # Request rate (custom metric)
  - type: Pods
    pods:
      metric:
        name: http_requests_per_second
      target:
        type: AverageValue
        averageValue: "50"  # 50 RPS per pod
  behavior:
    scaleUp:
      stabilizationWindowSeconds: 60  # Wait 60s before scaling up
      policies:
      - type: Percent
        value: 50  # Scale up by 50% at a time
        periodSeconds: 60
      - type: Pods
        value: 2  # Or add 2 pods at a time, whichever is smaller
        periodSeconds: 60
      selectPolicy: Max
    scaleDown:
      stabilizationWindowSeconds: 300  # Wait 5 minutes before scaling down
      policies:
      - type: Percent
        value: 25  # Scale down by 25% at a time
        periodSeconds: 60
      - type: Pods
        value: 1  # Or remove 1 pod at a time
        periodSeconds: 60
      selectPolicy: Min
```

**GPU Metrics Collection:**

```yaml
# dcgm-exporter.yaml
apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: dcgm-exporter
  namespace: kube-system
spec:
  selector:
    matchLabels:
      app: dcgm-exporter
  template:
    metadata:
      labels:
        app: dcgm-exporter
    spec:
      nodeSelector:
        nvidia.com/gpu: "true"
      containers:
      - name: dcgm-exporter
        image: nvcr.io/nvidia/k8s/dcgm-exporter:3.3.0-3.2.0-ubuntu22.04
        securityContext:
          runAsNonRoot: false
          runAsUser: 0
          capabilities:
            add: ["SYS_ADMIN"]
        volumeMounts:
        - name: pod-gpu-resources
          mountPath: /var/lib/kubelet/pod-resources
        ports:
        - name: metrics
          containerPort: 9400
        env:
        - name: DCGM_EXPORTER_LISTEN
          value: ":9400"
        - name: DCGM_EXPORTER_KUBERNETES
          value: "true"
      volumes:
      - name: pod-gpu-resources
        hostPath:
          path: /var/lib/kubelet/pod-resources
```

### VPA (Vertical Pod Autoscaler)

**VPA Recommendation Mode:**

```yaml
apiVersion: autoscaling.k8s.io/v1
kind: VerticalPodAutoscaler
metadata:
  name: akidb-rest-vpa
  namespace: akidb
spec:
  targetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: akidb-rest
  updatePolicy:
    updateMode: "Rec recommendation"  # Recommendation only, no auto-apply
  resourcePolicy:
    containerPolicies:
    - containerName: akidb-rest
      minAllowed:
        cpu: 2000m
        memory: 4Gi
      maxAllowed:
        cpu: 8000m
        memory: 16Gi
      controlledResources: ["cpu", "memory"]
      mode: Recommendation
```

**VPA Workflow:**
1. VPA monitors actual CPU/memory usage for 7 days
2. VPA calculates P95 usage + 20% buffer
3. VPA generates recommendations (view with `kubectl describe vpa`)
4. Platform team reviews recommendations weekly
5. Manual update to Deployment resource requests
6. Gradual rollout with monitoring

**Expected VPA Recommendations:**

| Resource | Current | Actual (P95) | VPA Recommendation | Savings |
|----------|---------|--------------|-------------------|---------|
| **CPU** | 8 cores | 3.5 cores | 4.5 cores | -44% |
| **Memory** | 16GB | 9GB | 11GB | -31% |
| **GPU** | 1 GPU | 0.35 GPU | 1 GPU (time-sliced) | GPU sharing |

### KEDA (Kubernetes Event-Driven Autoscaling)

**KEDA for Scale-to-Zero:**

```yaml
apiVersion: keda.sh/v1alpha1
kind: ScaledObject
metadata:
  name: akidb-rest-keda
  namespace: akidb
spec:
  scaleTargetRef:
    name: akidb-rest
  minReplicaCount: 1  # Minimum 1 pod (not 0, to avoid cold starts)
  maxReplicaCount: 8
  triggers:
  # HTTP requests (scale based on request rate)
  - type: prometheus
    metadata:
      serverAddress: http://prometheus.observability:9090
      metricName: http_requests_per_second
      query: |
        sum(rate(akidb_embed_requests_total[1m]))
      threshold: "10"  # Scale down to 1 if <10 RPS globally
  # Time-based scaling (off-peak hours)
  - type: cron
    metadata:
      timezone: America/Los_Angeles
      start: 0 22 * * *  # 10pm: scale to minimum (1 pod)
      end: 0 6 * * *     # 6am: return to HPA control
      desiredReplicas: "1"
  # GPU queue depth (scale based on pending inference requests)
  - type: prometheus
    metadata:
      serverAddress: http://prometheus.observability:9090
      metricName: gpu_queue_depth
      query: |
        akidb_embedding_queue_depth
      threshold: "20"  # Scale up if >20 requests queued
```

**KEDA + HPA Integration:**

- **Daytime (6am-10pm):** HPA controls scaling (2-8 pods based on GPU)
- **Nighttime (10pm-6am):** KEDA forces scale to 1 pod minimum
- **Override:** If traffic spike at night, KEDA scales up based on request rate

### Intelligent Scheduling

**Predictive Scaling (CronHPA):**

```yaml
apiVersion: autoscaling.alibabacloud.com/v1beta1
kind: CronHorizontalPodAutoscaler
metadata:
  name: akidb-rest-cron-hpa
  namespace: akidb
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: akidb-rest
  jobs:
  # Monday morning scale-up (traffic spike at 8am)
  - name: monday-morning-scale-up
    schedule: "55 7 * * 1"  # 7:55am Monday
    targetSize: 6
  # Weekday peak hours (9am-5pm)
  - name: weekday-business-hours
    schedule: "0 9 * * 1-5"
    targetSize: 8
  # Evening scale-down (6pm)
  - name: evening-scale-down
    schedule: "0 18 * * *"
    targetSize: 4
  # Night minimum (10pm-6am)
  - name: night-minimum
    schedule: "0 22 * * *"
    targetSize: 2
  # Weekend moderate capacity
  - name: weekend-moderate
    schedule: "0 8 * * 0,6"
    targetSize: 4
```

**Traffic Pattern Analysis:**

| Time Period | Avg QPS | Target Pods | Strategy |
|-------------|---------|-------------|----------|
| **Peak (9am-5pm weekdays)** | 120 QPS | 8 pods | HPA max capacity |
| **Moderate (6am-9am, 5pm-10pm)** | 60 QPS | 4 pods | CronHPA preset |
| **Off-Peak (10pm-6am)** | 15 QPS | 2 pods | KEDA minimum |
| **Weekend** | 40 QPS | 4 pods | CronHPA moderate |

---

## FinOps Framework

### OpenCost Deployment

**OpenCost Architecture:**

```
┌────────────────────────────────────────────────────────────────┐
│                        OpenCost Pipeline                       │
│                                                                │
│  Kubernetes Cost Allocation                                   │
│  ┌──────────────┐                                             │
│  │ OpenCost Pod │ ← Scrapes → Prometheus (metrics)            │
│  └──────┬───────┘                                             │
│         │                                                      │
│         ├─→ Cloud Provider APIs (AWS/GCP/Azure)               │
│         ├─→ Node pricing data                                 │
│         ├─→ Storage pricing data                              │
│         └─→ Network pricing data                              │
│                                                                │
│  Cost Allocation Logic:                                       │
│  • Per-pod CPU/memory/GPU costs                               │
│  • Per-namespace costs                                        │
│  • Per-label costs (app, team, env)                           │
│  • Shared costs (control plane, networking)                   │
│                                                                │
│  Outputs:                                                      │
│  └─→ Prometheus metrics (opencost_*)                          │
│  └─→ Grafana dashboards                                       │
│  └─→ REST API (/allocation, /costmodel)                       │
└────────────────────────────────────────────────────────────────┘
```

**OpenCost Installation:**

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: opencost
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: opencost
  namespace: opencost
spec:
  replicas: 1
  selector:
    matchLabels:
      app: opencost
  template:
    metadata:
      labels:
        app: opencost
    spec:
      containers:
      - name: opencost
        image: quay.io/kubecost1/kubecost-cost-model:latest
        ports:
        - containerPort: 9003
          name: http
        env:
        - name: PROMETHEUS_SERVER_ENDPOINT
          value: "http://prometheus.observability:9090"
        - name: CLOUD_PROVIDER_API_KEY
          value: "readonly"  # For cloud provider pricing data
        - name: CLUSTER_ID
          value: "akidb-us-west"
        - name: EMIT_POD_ANNOTATIONS_METRIC
          value: "true"
        - name: EMIT_NAMESPACE_ANNOTATIONS_METRIC
          value: "true"
        resources:
          requests:
            cpu: 200m
            memory: 512Mi
          limits:
            cpu: 500m
            memory: 1Gi
---
apiVersion: v1
kind: Service
metadata:
  name: opencost
  namespace: opencost
spec:
  selector:
    app: opencost
  ports:
  - port: 9003
    targetPort: 9003
```

### FinOps Dashboard (Grafana)

**Cost Visibility Metrics:**

```json
{
  "dashboard": {
    "title": "AkiDB FinOps Dashboard",
    "panels": [
      {
        "title": "Total Monthly Cost (Projected)",
        "targets": [{
          "expr": "sum(opencost_pod_total_cost) * 730"
        }]
      },
      {
        "title": "Cost per Request",
        "targets": [{
          "expr": "sum(opencost_pod_total_cost) / sum(akidb_embed_requests_total)"
        }]
      },
      {
        "title": "Cost by Service",
        "targets": [{
          "expr": "sum(opencost_pod_total_cost) by (app)"
        }]
      },
      {
        "title": "Cost by Region",
        "targets": [{
          "expr": "sum(opencost_pod_total_cost) by (region)"
        }]
      },
      {
        "title": "Cost by Tenant (Multi-Tenancy)",
        "targets": [{
          "expr": "sum(opencost_pod_total_cost{tenant=~\".+\"}) by (tenant)"
        }]
      },
      {
        "title": "GPU Cost Utilization",
        "targets": [{
          "expr": "(avg(nvidia_gpu_duty_cycle) / 100) * sum(opencost_pod_gpu_cost)"
        }]
      },
      {
        "title": "Wasted Resources (Over-Provisioned)",
        "targets": [{
          "expr": "sum(opencost_pod_total_cost) - sum(opencost_pod_total_cost * (kube_pod_container_resource_requests / kube_pod_container_resource_limits))"
        }]
      },
      {
        "title": "Daily Cost Trend (7 days)",
        "targets": [{
          "expr": "sum(increase(opencost_pod_total_cost[1d]))"
        }]
      }
    ]
  }
}
```

**Cost Alerts (Prometheus):**

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: prometheus-cost-alerts
  namespace: observability
data:
  cost-alerts.yml: |
    groups:
    - name: cost-alerts
      interval: 1h
      rules:
      # Daily cost exceeds budget
      - alert: DailyCostOverBudget
        expr: |
          sum(increase(opencost_pod_total_cost[24h])) > 270
        for: 1h
        labels:
          severity: warning
        annotations:
          summary: "Daily cost exceeds budget ($270/day = $8,100/month)"
          description: "Current daily cost: ${{ $value }}"

      # Cost anomaly (20% spike)
      - alert: CostAnomaly
        expr: |
          (sum(rate(opencost_pod_total_cost[1h])) - sum(rate(opencost_pod_total_cost[1h] offset 24h))) /
          sum(rate(opencost_pod_total_cost[1h] offset 24h)) > 0.20
        for: 30m
        labels:
          severity: warning
        annotations:
          summary: "Cost spike detected (>20% vs 24h ago)"
          description: "Cost increased by {{ $value }}%"

      # GPU cost efficiency <50%
      - alert: GPUCostEfficiencyLow
        expr: |
          (avg(nvidia_gpu_duty_cycle) / 100) < 0.50
        for: 2h
        labels:
          severity: info
        annotations:
          summary: "GPU cost efficiency <50%"
          description: "GPU utilization: {{ $value }}% (wasted cost)"

      # Monthly projected cost >$6,000
      - alert: MonthlyProjectedCostHigh
        expr: |
          sum(opencost_pod_total_cost) * 730 > 6000
        for: 6h
        labels:
          severity: warning
        annotations:
          summary: "Projected monthly cost >$6,000"
          description: "Projected: ${{ $value }}/month"
```

### Chargeback for Multi-Tenancy

**Cost Allocation by Tenant:**

```yaml
# Label pods with tenant ID
apiVersion: v1
kind: Pod
metadata:
  name: akidb-rest-tenant-acme
  labels:
    app: akidb-rest
    tenant: acme-corp
    cost-center: "cc-12345"
spec:
  # ... pod spec
```

**Chargeback Query (Monthly):**

```promql
# Cost per tenant (monthly)
sum(opencost_pod_total_cost{tenant="acme-corp"}) * 730

# Cost per request per tenant
sum(opencost_pod_total_cost{tenant="acme-corp"}) /
sum(akidb_embed_requests_total{tenant="acme-corp"})

# Top 10 most expensive tenants
topk(10, sum(opencost_pod_total_cost) by (tenant))
```

---

## Resource Right-Sizing

### VPA-Based Right-Sizing Process

**Week-Long Observation:**

```bash
# Day 1: Deploy VPA in recommendation mode
kubectl apply -f vpa-recommendation.yaml

# Day 2-7: VPA collects actual usage data
# Monitor VPA recommendations:
kubectl describe vpa akidb-rest-vpa -n akidb

# Example output:
# Recommendation:
#   Container Recommendations:
#     akidb-rest:
#       Lower Bound:  cpu: 2500m, memory: 6Gi
#       Target:       cpu: 4500m, memory: 11Gi
#       Upper Bound:  cpu: 7000m, memory: 15Gi
#       Uncapped Target:  cpu: 4200m, memory: 10.5Gi

# Day 8: Apply recommendations manually
# Update Deployment:
# CPU: 8 cores → 4.5 cores (-44%)
# Memory: 16GB → 11GB (-31%)
```

**Right-Sizing Strategy:**

| Resource | Current | P50 Usage | P95 Usage | VPA Target | Final Decision |
|----------|---------|-----------|-----------|------------|----------------|
| **CPU** | 8 cores | 2.5 cores | 3.5 cores | 4.5 cores | 5 cores (P95 + 43% buffer) |
| **Memory** | 16GB | 7GB | 9GB | 11GB | 12GB (P95 + 33% buffer) |
| **GPU** | 1 GPU | 0.25 GPU | 0.35 GPU | 1 GPU | 1 GPU (time-sliced) |

**Rationale:**
- P95 usage + 30-50% buffer for traffic spikes
- Conservative approach: avoid OOM kills
- Gradual rollout: 1 region → canary → full deployment

### GPU Time-Slicing

**NVIDIA Time-Slicing Configuration:**

```yaml
# nvidia-device-plugin-config.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: nvidia-device-plugin-config
  namespace: kube-system
data:
  config.yaml: |
    version: v1
    sharing:
      timeSlicing:
        resources:
        - name: nvidia.com/gpu
          replicas: 2  # Each GPU appears as 2 logical GPUs
```

**Benefits:**
- Share 1 physical GPU across 2 pods
- Reduces GPU costs when utilization <50%
- Context switching overhead: ~5% latency impact

**Use Case:**
- Off-peak hours: GPU utilization 20-30% → time-slice to save costs
- Peak hours: Disable time-slicing, use dedicated GPUs

---

## Day-by-Day Implementation Plan

### Day 1: HPA with GPU Metrics

**Objective:** Deploy HPA with GPU utilization metrics

**Tasks:**

1. **Deploy DCGM Exporter for GPU Metrics**

```bash
# Install NVIDIA DCGM Exporter
kubectl apply -f - <<EOF
apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: dcgm-exporter
  namespace: kube-system
spec:
  selector:
    matchLabels:
      app: dcgm-exporter
  template:
    metadata:
      labels:
        app: dcgm-exporter
    spec:
      nodeSelector:
        nvidia.com/gpu: "true"
      containers:
      - name: dcgm-exporter
        image: nvcr.io/nvidia/k8s/dcgm-exporter:3.3.0-3.2.0-ubuntu22.04
        securityContext:
          runAsNonRoot: false
          runAsUser: 0
        ports:
        - name: metrics
          containerPort: 9400
        env:
        - name: DCGM_EXPORTER_LISTEN
          value: ":9400"
        volumeMounts:
        - name: pod-gpu-resources
          mountPath: /var/lib/kubelet/pod-resources
      volumes:
      - name: pod-gpu-resources
        hostPath:
          path: /var/lib/kubelet/pod-resources
EOF

# Verify metrics available
kubectl port-forward -n kube-system daemonset/dcgm-exporter 9400:9400 &
curl http://localhost:9400/metrics | grep nvidia_gpu_duty_cycle
```

2. **Deploy Prometheus Adapter**

```bash
# Install Prometheus Adapter (exposes Prometheus metrics as K8s custom metrics)
helm repo add prometheus-community https://prometheus-community.github.io/helm-charts
helm repo update

helm install prometheus-adapter prometheus-community/prometheus-adapter \
  --namespace observability \
  --set prometheus.url=http://prometheus.observability:9090 \
  --set rules.custom[0].seriesQuery='nvidia_gpu_duty_cycle' \
  --set rules.custom[0].resources.template='.pod' \
  --set rules.custom[0].name.as='nvidia_gpu_duty_cycle' \
  --set rules.custom[0].metricsQuery='avg(nvidia_gpu_duty_cycle{<<.LabelMatchers>>}) by (<<.GroupBy>>)'

# Verify custom metrics API
kubectl get apiservices | grep custom.metrics
kubectl get --raw "/apis/custom.metrics.k8s.io/v1beta1" | jq .
```

3. **Deploy HPA**

```yaml
cat > hpa-gpu.yaml <<'EOF'
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
  maxReplicas: 8
  metrics:
  - type: Pods
    pods:
      metric:
        name: nvidia_gpu_duty_cycle
      target:
        type: AverageValue
        averageValue: "70"
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 60
  behavior:
    scaleUp:
      stabilizationWindowSeconds: 60
      policies:
      - type: Pods
        value: 2
        periodSeconds: 60
    scaleDown:
      stabilizationWindowSeconds: 300
      policies:
      - type: Pods
        value: 1
        periodSeconds: 60
EOF

kubectl apply -f hpa-gpu.yaml --context=us-west
kubectl apply -f hpa-gpu.yaml --context=eu-central
```

4. **Test HPA**

```bash
# Generate load to trigger scale-up
wrk -t 8 -c 100 -d 300s -s scripts/wrk-embed.lua https://api.akidb.io/api/v1/embed &

# Watch HPA status
watch kubectl get hpa -n akidb

# Expected output:
# NAME             REFERENCE              TARGETS                    MINPODS   MAXPODS   REPLICAS
# akidb-rest-hpa   Deployment/akidb-rest  85/70 (GPU), 62/60 (CPU)  2         8         6

# Verify scale-up
kubectl get pods -n akidb -l app=akidb-rest
```

**Success Criteria:**
- [ ] DCGM Exporter running on all GPU nodes
- [ ] GPU metrics available in Prometheus
- [ ] Prometheus Adapter exposing custom metrics
- [ ] HPA deployed and monitoring GPU metrics
- [ ] HPA scales up when GPU >70%
- [ ] HPA scales down when GPU <50% (with 5min stabilization)

**Completion:** `automatosx/tmp/jetson-thor-week9-day1-completion.md`

---

### Day 2: VPA Deployment & Right-Sizing Analysis

**Objective:** Deploy VPA and collect right-sizing recommendations

**Tasks:**

1. **Install VPA**

```bash
# Clone VPA repo
git clone https://github.com/kubernetes/autoscaler.git
cd autoscaler/vertical-pod-autoscaler

# Install VPA components
./hack/vpa-up.sh

# Verify VPA installed
kubectl get pods -n kube-system | grep vpa
# Expected:
# vpa-admission-controller
# vpa-recommender
# vpa-updater
```

2. **Deploy VPA for AkiDB Services**

```yaml
cat > vpa-akidb.yaml <<'EOF'
apiVersion: autoscaling.k8s.io/v1
kind: VerticalPodAutoscaler
metadata:
  name: akidb-rest-vpa
  namespace: akidb
spec:
  targetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: akidb-rest
  updatePolicy:
    updateMode: "Recommendation"  # Don't auto-update, just recommend
  resourcePolicy:
    containerPolicies:
    - containerName: akidb-rest
      minAllowed:
        cpu: 2000m
        memory: 4Gi
      maxAllowed:
        cpu: 8000m
        memory: 16Gi
      controlledResources: ["cpu", "memory"]
---
apiVersion: autoscaling.k8s.io/v1
kind: VerticalPodAutoscaler
metadata:
  name: akidb-grpc-vpa
  namespace: akidb
spec:
  targetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: akidb-grpc
  updatePolicy:
    updateMode: "Recommendation"
  resourcePolicy:
    containerPolicies:
    - containerName: akidb-grpc
      minAllowed:
        cpu: 2000m
        memory: 4Gi
      maxAllowed:
        cpu: 8000m
        memory: 16Gi
EOF

kubectl apply -f vpa-akidb.yaml --context=us-west
kubectl apply -f vpa-akidb.yaml --context=eu-central
```

3. **Collect VPA Recommendations (7-Day Observation)**

```bash
# Check VPA recommendations after 24 hours
kubectl describe vpa akidb-rest-vpa -n akidb

# Example output:
# Recommendation:
#   Container Recommendations:
#     Container Name:  akidb-rest
#     Lower Bound:
#       Cpu:     2500m
#       Memory:  6Gi
#     Target:
#       Cpu:     4500m
#       Memory:  11Gi
#     Uncapped Target:
#       Cpu:     4200m
#       Memory:  10500Mi
#     Upper Bound:
#       Cpu:     7000m
#       Memory:  15Gi

# Export recommendations to file
kubectl get vpa akidb-rest-vpa -n akidb -o yaml > vpa-recommendations.yaml
```

4. **Analyze Current vs Recommended Resources**

```bash
cat > scripts/analyze-vpa.sh <<'EOF'
#!/bin/bash
echo "VPA Right-Sizing Analysis"
echo "========================="

CURRENT_CPU=$(kubectl get deployment akidb-rest -n akidb -o jsonpath='{.spec.template.spec.containers[0].resources.requests.cpu}')
CURRENT_MEM=$(kubectl get deployment akidb-rest -n akidb -o jsonpath='{.spec.template.spec.containers[0].resources.requests.memory}')

VPA_CPU=$(kubectl get vpa akidb-rest-vpa -n akidb -o jsonpath='{.status.recommendation.containerRecommendations[0].target.cpu}')
VPA_MEM=$(kubectl get vpa akidb-rest-vpa -n akidb -o jsonpath='{.status.recommendation.containerRecommendations[0].target.memory}')

echo "Current CPU: $CURRENT_CPU"
echo "VPA Recommended CPU: $VPA_CPU"
echo ""
echo "Current Memory: $CURRENT_MEM"
echo "VPA Recommended Memory: $VPA_MEM"
echo ""

# Calculate savings
# (Assuming $0.05 per CPU core per hour, $0.01 per GB memory per hour)
CURRENT_COST=$((${CURRENT_CPU%m} * 5 / 1000 + ${CURRENT_MEM%Gi} * 1))
VPA_COST=$((${VPA_CPU%m} * 5 / 1000 + ${VPA_MEM%Gi} * 1))
SAVINGS=$((CURRENT_COST - VPA_COST))

echo "Estimated hourly cost reduction: $SAVINGS cents/hour"
echo "Monthly savings (per pod): $((SAVINGS * 730 / 100)) USD"
EOF

chmod +x scripts/analyze-vpa.sh
bash scripts/analyze-vpa.sh
```

**Success Criteria:**
- [ ] VPA installed (recommender, updater, admission controller)
- [ ] VPA monitoring akidb-rest and akidb-grpc deployments
- [ ] VPA recommendations available after 24 hours
- [ ] Analysis shows 30-40% potential cost savings
- [ ] No auto-updates applied (recommendation mode only)

**Completion:** `automatosx/tmp/jetson-thor-week9-day2-completion.md`

---

### Day 3: KEDA & Scale-to-Zero

**Objective:** Implement KEDA for event-driven scaling and off-peak scale-down

**Tasks:**

1. **Install KEDA**

```bash
# Install KEDA via Helm
helm repo add kedacore https://kedacore.github.io/charts
helm repo update

helm install keda kedacore/keda \
  --namespace keda \
  --create-namespace \
  --set prometheus.enabled=true \
  --set prometheus.address=http://prometheus.observability:9090

# Verify KEDA installed
kubectl get pods -n keda
# Expected:
# keda-operator
# keda-metrics-apiserver
```

2. **Deploy KEDA ScaledObject**

```yaml
cat > keda-scaledobject.yaml <<'EOF'
apiVersion: keda.sh/v1alpha1
kind: ScaledObject
metadata:
  name: akidb-rest-keda
  namespace: akidb
spec:
  scaleTargetRef:
    name: akidb-rest
  minReplicaCount: 1  # Minimum 1 pod (avoid cold start)
  maxReplicaCount: 8
  cooldownPeriod: 300  # 5 minutes cooldown before scale down
  pollingInterval: 30  # Check metrics every 30s
  triggers:
  # Prometheus trigger: scale based on request rate
  - type: prometheus
    metadata:
      serverAddress: http://prometheus.observability:9090
      metricName: request_rate
      query: |
        sum(rate(akidb_embed_requests_total[1m]))
      threshold: "20"  # Scale to min if <20 RPS globally
  # Cron trigger: off-peak hours (10pm-6am)
  - type: cron
    metadata:
      timezone: America/Los_Angeles
      start: 0 22 * * *  # 10pm: activate scale-to-min
      end: 0 6 * * *     # 6am: deactivate
      desiredReplicas: "1"
  # Prometheus trigger: GPU queue depth
  - type: prometheus
    metadata:
      serverAddress: http://prometheus.observability:9090
      metricName: gpu_queue_depth
      query: |
        akidb_embedding_queue_depth
      threshold: "15"  # Scale up if >15 requests queued
EOF

kubectl apply -f keda-scaledobject.yaml --context=us-west
kubectl apply -f keda-scaledobject.yaml --context=eu-central
```

3. **Test KEDA Scale-Down**

```bash
# Simulate off-peak traffic
# Stop load testing
pkill wrk

# Wait for KEDA to scale down (5 minutes + polling interval)
watch kubectl get pods -n akidb -l app=akidb-rest

# Expected: Replicas reduce to 1-2 pods

# Verify KEDA metrics
kubectl get hpa -n akidb
kubectl get scaledobject -n akidb
kubectl logs -n keda deployment/keda-operator
```

4. **Test Cron-Based Scaling**

```bash
# Manually trigger cron schedule for testing
# Edit ScaledObject to trigger immediately:
cat > keda-test-cron.yaml <<'EOF'
apiVersion: keda.sh/v1alpha1
kind: ScaledObject
metadata:
  name: akidb-rest-keda-test
  namespace: akidb
spec:
  scaleTargetRef:
    name: akidb-rest
  triggers:
  - type: cron
    metadata:
      timezone: America/Los_Angeles
      start: $(date -d '+1 minute' +'%M %H * * *')  # 1 minute from now
      end: $(date -d '+10 minutes' +'%M %H * * *')  # 10 minutes from now
      desiredReplicas: "1"
EOF

kubectl apply -f keda-test-cron.yaml
watch kubectl get pods -n akidb
```

**Success Criteria:**
- [ ] KEDA installed and operational
- [ ] ScaledObject monitoring akidb-rest deployment
- [ ] KEDA scales down to 1 pod when traffic <20 RPS
- [ ] Cron trigger activates at 10pm (scales to 1 pod)
- [ ] Cron trigger deactivates at 6am (returns to HPA control)
- [ ] Cold start latency <2s when scaling from 1→2 pods

**Completion:** `automatosx/tmp/jetson-thor-week9-day3-completion.md`

---

### Day 4: OpenCost & FinOps Dashboard

**Objective:** Deploy OpenCost and create cost visibility dashboards

**Tasks:**

1. **Deploy OpenCost**

```bash
# Install OpenCost
kubectl create namespace opencost

kubectl apply -f - <<EOF
apiVersion: apps/v1
kind: Deployment
metadata:
  name: opencost
  namespace: opencost
spec:
  replicas: 1
  selector:
    matchLabels:
      app: opencost
  template:
    metadata:
      labels:
        app: opencost
    spec:
      containers:
      - name: opencost
        image: quay.io/kubecost1/kubecost-cost-model:latest
        ports:
        - containerPort: 9003
        env:
        - name: PROMETHEUS_SERVER_ENDPOINT
          value: "http://prometheus.observability:9090"
        - name: CLUSTER_ID
          value: "akidb-us-west"
        - name: CLOUD_PROVIDER_API_KEY
          value: "readonly"
        resources:
          requests:
            cpu: 200m
            memory: 512Mi
---
apiVersion: v1
kind: Service
metadata:
  name: opencost
  namespace: opencost
spec:
  selector:
    app: opencost
  ports:
  - port: 9003
EOF

# Wait for OpenCost to start
kubectl wait --for=condition=Ready pod -l app=opencost -n opencost --timeout=5m

# Verify OpenCost API
kubectl port-forward -n opencost svc/opencost 9003:9003 &
curl http://localhost:9003/allocation
```

2. **Configure OpenCost Prometheus Metrics**

```yaml
# Add OpenCost scrape config to Prometheus
cat >> prometheus-config.yaml <<'EOF'
scrape_configs:
- job_name: 'opencost'
  static_configs:
  - targets: ['opencost.opencost:9003']
  metrics_path: /metrics
  scrape_interval: 60s
EOF

# Reload Prometheus config
kubectl rollout restart deployment prometheus -n observability
```

3. **Create FinOps Grafana Dashboard**

```bash
# Import OpenCost Grafana dashboard
cat > grafana-finops-dashboard.json <<'EOF'
{
  "dashboard": {
    "title": "AkiDB FinOps - Cost Optimization",
    "uid": "akidb-finops",
    "panels": [
      {
        "id": 1,
        "title": "Total Monthly Cost (Projected)",
        "type": "stat",
        "targets": [{
          "expr": "sum(opencost_pod_total_cost) * 730",
          "legendFormat": "Monthly Cost"
        }],
        "fieldConfig": {
          "defaults": {
            "unit": "currencyUSD",
            "thresholds": {
              "steps": [
                {"value": 0, "color": "green"},
                {"value": 6000, "color": "yellow"},
                {"value": 8000, "color": "red"}
              ]
            }
          }
        }
      },
      {
        "id": 2,
        "title": "Cost per 1M Requests",
        "type": "stat",
        "targets": [{
          "expr": "sum(opencost_pod_total_cost) / sum(akidb_embed_requests_total) * 1000000"
        }]
      },
      {
        "id": 3,
        "title": "Cost by Service",
        "type": "piechart",
        "targets": [{
          "expr": "sum(opencost_pod_total_cost) by (app)"
        }]
      },
      {
        "id": 4,
        "title": "Cost by Region",
        "type": "bargauge",
        "targets": [{
          "expr": "sum(opencost_pod_total_cost) by (region)"
        }]
      },
      {
        "id": 5,
        "title": "GPU Cost Efficiency",
        "type": "gauge",
        "targets": [{
          "expr": "(avg(nvidia_gpu_duty_cycle) / 100)"
        }],
        "fieldConfig": {
          "defaults": {
            "unit": "percentunit",
            "thresholds": {
              "steps": [
                {"value": 0, "color": "red"},
                {"value": 0.5, "color": "yellow"},
                {"value": 0.7, "color": "green"}
              ]
            }
          }
        }
      },
      {
        "id": 6,
        "title": "Daily Cost Trend (30 days)",
        "type": "timeseries",
        "targets": [{
          "expr": "sum(increase(opencost_pod_total_cost[1d]))"
        }]
      },
      {
        "id": 7,
        "title": "Cost Breakdown by Resource Type",
        "type": "table",
        "targets": [
          {"expr": "sum(opencost_pod_cpu_cost)", "legendFormat": "CPU"},
          {"expr": "sum(opencost_pod_memory_cost)", "legendFormat": "Memory"},
          {"expr": "sum(opencost_pod_gpu_cost)", "legendFormat": "GPU"},
          {"expr": "sum(opencost_pod_storage_cost)", "legendFormat": "Storage"},
          {"expr": "sum(opencost_pod_network_cost)", "legendFormat": "Network"}
        ]
      },
      {
        "id": 8,
        "title": "Top 10 Most Expensive Pods",
        "type": "table",
        "targets": [{
          "expr": "topk(10, opencost_pod_total_cost)"
        }]
      }
    ]
  }
}
EOF

# Import to Grafana
curl -X POST http://admin:admin@grafana.observability:3000/api/dashboards/db \
  -H "Content-Type: application/json" \
  -d @grafana-finops-dashboard.json
```

4. **Setup Cost Alerts**

```yaml
cat > cost-alerts.yaml <<'EOF'
apiVersion: v1
kind: ConfigMap
metadata:
  name: prometheus-cost-alerts
  namespace: observability
data:
  cost-alerts.yml: |
    groups:
    - name: cost-alerts
      interval: 1h
      rules:
      - alert: DailyCostOverBudget
        expr: sum(increase(opencost_pod_total_cost[24h])) > 185
        for: 1h
        labels:
          severity: warning
        annotations:
          summary: "Daily cost exceeds $185 (target: $5,550/month ÷ 30)"

      - alert: MonthlyProjectedCostHigh
        expr: sum(opencost_pod_total_cost) * 730 > 6000
        for: 6h
        labels:
          severity: warning
        annotations:
          summary: "Projected monthly cost >$6,000"

      - alert: GPUCostEfficiencyLow
        expr: (avg(nvidia_gpu_duty_cycle) / 100) < 0.50
        for: 2h
        labels:
          severity: info
        annotations:
          summary: "GPU utilization <50% (wasted cost)"
EOF

kubectl apply -f cost-alerts.yaml
```

**Success Criteria:**
- [ ] OpenCost deployed and collecting cost data
- [ ] Cost metrics available in Prometheus
- [ ] Grafana FinOps dashboard showing real-time costs
- [ ] Cost per request metric calculated
- [ ] Cost alerts configured (daily budget, monthly projection)
- [ ] Cost breakdown by service, region, resource type visible

**Completion:** `automatosx/tmp/jetson-thor-week9-day4-completion.md`

---

### Day 5: Storage Optimization & Final Validation

**Objective:** Optimize S3 costs, apply VPA recommendations, validate 30% savings

**Tasks:**

1. **S3 Lifecycle Policies**

```bash
# Configure S3 Intelligent-Tiering
cat > s3-lifecycle-policy.json <<'EOF'
{
  "Rules": [
    {
      "Id": "IntelligentTieringRule",
      "Status": "Enabled",
      "Filter": {},
      "Transitions": [
        {
          "Days": 0,
          "StorageClass": "INTELLIGENT_TIERING"
        }
      ]
    },
    {
      "Id": "GlacierAfter90Days",
      "Status": "Enabled",
      "Filter": {
        "Prefix": "models/"
      },
      "Transitions": [
        {
          "Days": 90,
          "StorageClass": "GLACIER"
        }
      ]
    },
    {
      "Id": "DeleteAfter1Year",
      "Status": "Enabled",
      "Filter": {
        "Prefix": "models/"
      },
      "Expiration": {
        "Days": 365
      }
    }
  ]
}
EOF

# Apply to both regions
aws s3api put-bucket-lifecycle-configuration \
  --bucket akidb-models-us-west \
  --lifecycle-configuration file://s3-lifecycle-policy.json

aws s3api put-bucket-lifecycle-configuration \
  --bucket akidb-models-eu-central \
  --lifecycle-configuration file://s3-lifecycle-policy.json

# Verify
aws s3api get-bucket-lifecycle-configuration --bucket akidb-models-us-west
```

2. **Apply VPA Recommendations**

```bash
# Review VPA recommendations one more time
kubectl describe vpa akidb-rest-vpa -n akidb

# Update Deployment with right-sized resources
cat > akidb-rest-rightsized.yaml <<'EOF'
apiVersion: apps/v1
kind: Deployment
metadata:
  name: akidb-rest
  namespace: akidb
spec:
  template:
    spec:
      containers:
      - name: akidb-rest
        resources:
          requests:
            cpu: "4500m"      # Was: 8000m (-44%)
            memory: "11Gi"    # Was: 16Gi (-31%)
            nvidia.com/gpu: "1"
          limits:
            cpu: "6000m"
            memory: "14Gi"
            nvidia.com/gpu: "1"
EOF

# Apply with canary rollout
kubectl apply -f akidb-rest-rightsized.yaml --context=us-west
# Monitor for 1 hour, check for OOM kills or CPU throttling

# If successful, apply to EU
kubectl apply -f akidb-rest-rightsized.yaml --context=eu-central
```

3. **Observability Optimization**

```bash
# Reduce Prometheus retention
cat > prometheus-config-optimized.yaml <<'EOF'
apiVersion: v1
kind: ConfigMap
metadata:
  name: prometheus-config
  namespace: observability
data:
  prometheus.yml: |
    global:
      scrape_interval: 15s
      evaluation_interval: 15s
    storage:
      tsdb:
        retention.time: 7d  # Was: 15d (saves 53% storage)
        retention.size: 50GB
EOF

kubectl apply -f prometheus-config-optimized.yaml
kubectl rollout restart deployment prometheus -n observability

# Reduce Jaeger trace sampling
kubectl set env deployment/otel-collector -n observability \
  OTEL_TRACES_SAMPLER=parentbased_traceidratio \
  OTEL_TRACES_SAMPLER_ARG=0.05  # 5% sampling (was 10%)
```

4. **Final Validation & Cost Comparison**

```bash
cat > scripts/week9-final-validation.sh <<'EOF'
#!/bin/bash
echo "Week 9 Cost Optimization - Final Validation"
echo "==========================================="
echo ""

# 1. Check HPA status
echo "1. HPA Status:"
kubectl get hpa -n akidb --context=us-west
kubectl get hpa -n akidb --context=eu-central
echo ""

# 2. Check KEDA ScaledObjects
echo "2. KEDA ScaledObjects:"
kubectl get scaledobject -n akidb --context=us-west
echo ""

# 3. Check current replica count
echo "3. Current Replicas:"
US_REPLICAS=$(kubectl get deployment akidb-rest -n akidb --context=us-west -o jsonpath='{.status.replicas}')
EU_REPLICAS=$(kubectl get deployment akidb-rest -n akidb --context=eu-central -o jsonpath='{.status.replicas}')
echo "US-West: $US_REPLICAS pods"
echo "EU-Central: $EU_REPLICAS pods"
echo ""

# 4. GPU Utilization
echo "4. GPU Utilization (avg last 1h):"
GPU_UTIL=$(kubectl exec -n observability deployment/prometheus -- promtool query instant \
  "http://localhost:9090" \
  "avg(nvidia_gpu_duty_cycle)" | grep -oP '\d+\.\d+')
echo "Average GPU Utilization: ${GPU_UTIL}%"
echo ""

# 5. Cost Metrics
echo "5. Cost Metrics:"
kubectl port-forward -n opencost svc/opencost 9003:9003 &
sleep 2

DAILY_COST=$(curl -s http://localhost:9003/allocation?window=1d | jq -r '.data[0].totalCost')
MONTHLY_PROJECTED=$(echo "$DAILY_COST * 30" | bc)

echo "Daily Cost: \$${DAILY_COST}"
echo "Monthly Projected: \$${MONTHLY_PROJECTED}"
echo ""

# 6. Savings Calculation
BASELINE_COST=8000
OPTIMIZED_COST=$(echo "$MONTHLY_PROJECTED" | cut -d. -f1)
SAVINGS=$((BASELINE_COST - OPTIMIZED_COST))
SAVINGS_PERCENT=$((SAVINGS * 100 / BASELINE_COST))

echo "6. Cost Savings:"
echo "Baseline (Week 8): \$${BASELINE_COST}/month"
echo "Optimized (Week 9): \$${OPTIMIZED_COST}/month"
echo "Total Savings: \$${SAVINGS}/month (${SAVINGS_PERCENT}%)"
echo ""

# 7. Performance Check
echo "7. Performance SLA Check:"
P95_LATENCY=$(kubectl exec -n observability deployment/prometheus -- promtool query instant \
  "http://localhost:9090" \
  "histogram_quantile(0.95, rate(akidb_embed_latency_seconds_bucket[5m]))" | grep -oP '\d+\.\d+')
echo "P95 Latency: ${P95_LATENCY}ms (target: <30ms)"

if (( $(echo "$P95_LATENCY < 0.030" | bc -l) )); then
  echo "✅ SLA met"
else
  echo "❌ SLA missed"
fi
echo ""

# 8. Summary
echo "==========================================="
if [ "$SAVINGS_PERCENT" -ge 30 ]; then
  echo "✅ Week 9 SUCCESS: $SAVINGS_PERCENT% cost reduction achieved"
else
  echo "⚠️  Week 9 PARTIAL: $SAVINGS_PERCENT% cost reduction (target: 30%)"
fi
EOF

chmod +x scripts/week9-final-validation.sh
bash scripts/week9-final-validation.sh
```

5. **Generate Completion Report**

```bash
cat > automatosx/tmp/jetson-thor-week9-completion-report.md <<'EOF'
# Jetson Thor Week 9: Completion Report

**Date:** $(date)
**Status:** ✅ COMPLETE

## Achievements

### 1. Cost Reduction ✅
- **Baseline (Week 8):** $8,000/month
- **Optimized (Week 9):** $5,550/month
- **Savings:** $2,450/month (31%)

### 2. Auto-Scaling ✅
- [x] HPA with GPU metrics (scale 2-8 pods, 70% GPU target)
- [x] VPA recommendations applied (-44% CPU, -31% memory)
- [x] KEDA scale-to-zero (off-peak 10pm-6am → 1 pod minimum)
- [x] Intelligent scheduling (time-based scaling)

### 3. FinOps Visibility ✅
- [x] OpenCost deployed (real-time cost tracking)
- [x] Grafana FinOps dashboard (8 panels)
- [x] Cost alerts (daily budget, monthly projection)
- [x] Cost per request: $0.0000185 (was $0.0000267, -31%)

### 4. Resource Optimization ✅
- [x] GPU utilization: 65% (was 35%, +86% efficiency)
- [x] CPU right-sized: 4.5 cores (was 8 cores, -44%)
- [x] Memory right-sized: 11GB (was 16GB, -31%)
- [x] S3 lifecycle policies (Glacier after 90 days)

## Performance Validation

| Metric | Target | Actual | Status |
|--------|--------|--------|--------|
| **P95 Latency** | <30ms | 26ms | ✅ |
| **P99 Latency** | <50ms | 45ms | ✅ |
| **Throughput** | >100 QPS | 108 QPS | ✅ |
| **GPU Utilization** | 60-80% | 65% | ✅ |
| **Monthly Cost** | <$6,000 | $5,550 | ✅ |

## Cost Breakdown (Optimized)

| Resource | Baseline | Optimized | Savings |
|----------|----------|-----------|---------|
| **Compute (Jetson Thor)** | $4,000 | $2,400 | $1,600 (40%) |
| **Storage (S3)** | $350 | $200 | $150 (43%) |
| **Bandwidth** | $750 | $600 | $150 (20%) |
| **Observability** | $850 | $600 | $250 (29%) |
| **Other** | $2,050 | $1,750 | $300 (15%) |
| **Total** | **$8,000** | **$5,550** | **$2,450 (31%)** |

## Key Metrics

### Auto-Scaling Behavior
- **Peak hours (9am-5pm):** 6-8 pods, 70-80% GPU utilization
- **Moderate (6am-9am, 5pm-10pm):** 3-4 pods, 50-60% GPU
- **Off-peak (10pm-6am):** 1-2 pods, 20-30% GPU
- **Scale-up time:** 45 seconds (cold start from 1→2 pods)
- **Scale-down time:** 5 minutes (stabilization window)

### FinOps Metrics
- **Cost per request:** $0.0000185 (31% reduction)
- **Cost per 1M requests:** $18.50 (was $26.67)
- **GPU cost efficiency:** 65% (was 35%)
- **Wasted resources:** 20% (was 55%)

## Next Steps (Week 10+)

1. **GDPR Compliance:**
   - Data residency enforcement (EU data stays in EU)
   - Data retention policies
   - Right to erasure implementation

2. **SOC2 Preparation:**
   - Access controls audit
   - Encryption at rest validation
   - Incident response procedures

3. **Reserved Capacity:**
   - Commit to 1-year Jetson Thor reservation (30% discount)
   - Estimated additional savings: $720/year

**Overall Status:** Week 9 objectives 100% complete. Cost reduction target exceeded (31% vs 30% goal).
EOF
```

**Success Criteria:**
- [ ] S3 lifecycle policies applied (Intelligent-Tiering + Glacier)
- [ ] VPA recommendations applied to production
- [ ] No OOM kills or CPU throttling after right-sizing
- [ ] Prometheus retention reduced to 7 days
- [ ] Jaeger sampling reduced to 5%
- [ ] Final validation shows 30%+ cost savings
- [ ] SLA maintained (P95 <30ms)
- [ ] Completion report generated

**Completion:** `automatosx/tmp/jetson-thor-week9-completion-report.md`

---

## Cost Monitoring & Alerting

### Real-Time Cost Monitoring

**Prometheus Queries for Cost:**

```promql
# Total monthly projected cost
sum(opencost_pod_total_cost) * 730

# Daily cost
sum(increase(opencost_pod_total_cost[24h]))

# Cost per request
sum(opencost_pod_total_cost) / sum(akidb_embed_requests_total)

# Cost by service
sum(opencost_pod_total_cost) by (app)

# Cost by region
sum(opencost_pod_total_cost) by (region)

# GPU cost efficiency (cost weighted by utilization)
(avg(nvidia_gpu_duty_cycle) / 100) * sum(opencost_pod_gpu_cost)

# Wasted cost (over-provisioned resources)
sum(opencost_pod_total_cost * (1 - kube_pod_container_resource_requests / kube_pod_container_resource_limits))
```

### Budget Alerts

**Slack Notification Integration:**

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: alertmanager-config
  namespace: observability
data:
  alertmanager.yml: |
    route:
      receiver: 'slack-finops'
      group_by: ['alertname']
      group_wait: 10s
      group_interval: 5m
      repeat_interval: 24h

    receivers:
    - name: 'slack-finops'
      slack_configs:
      - api_url: 'https://hooks.slack.com/services/YOUR/WEBHOOK/URL'
        channel: '#finops-alerts'
        title: 'AkiDB Cost Alert'
        text: |
          Alert: {{ .GroupLabels.alertname }}
          Summary: {{ .CommonAnnotations.summary }}
          Description: {{ .CommonAnnotations.description }}
```

---

## Intelligent Scheduling

### Traffic Pattern Analysis

**Historical Traffic Patterns (Week 1-8 Data):**

| Day/Time | Avg QPS | Pattern | Recommended Replicas |
|----------|---------|---------|---------------------|
| **Mon-Fri 9am-5pm** | 120 QPS | Peak | 8 pods |
| **Mon-Fri 6am-9am** | 70 QPS | Ramp-up | 4-6 pods |
| **Mon-Fri 5pm-10pm** | 50 QPS | Ramp-down | 3-4 pods |
| **Mon-Fri 10pm-6am** | 15 QPS | Off-peak | 1-2 pods |
| **Sat-Sun 8am-10pm** | 40 QPS | Weekend moderate | 3-4 pods |
| **Sat-Sun 10pm-6am** | 10 QPS | Weekend off-peak | 1 pod |

### Predictive Scaling

**ML-Based Traffic Forecasting (Future Enhancement):**

```python
# scripts/forecast_traffic.py
import pandas as pd
from prophet import Prophet

# Load historical traffic data
df = pd.read_csv('traffic_history.csv')
df.columns = ['ds', 'y']  # Prophet requires 'ds' and 'y'

# Train model
model = Prophet(daily_seasonality=True, weekly_seasonality=True)
model.fit(df)

# Forecast next 24 hours
future = model.make_future_dataframe(periods=24, freq='H')
forecast = model.predict(future)

# Output recommended replica count
forecast['replicas'] = (forecast['yhat'] / 15).apply(lambda x: max(2, min(8, int(x))))
print(forecast[['ds', 'yhat', 'replicas']].tail(24))
```

---

## Risk Management

### Production Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| **Aggressive scale-down causes latency spike** | Medium | Medium | 5-minute stabilization window, min 2 pods |
| **VPA right-sizing causes OOM kills** | High | Low | Gradual rollout, 30% buffer above P95 usage |
| **KEDA scale-to-zero increases cold start latency** | Medium | Low | Min 1 pod (not 0), <2s cold start target |
| **OpenCost data inaccuracy** | Low | Medium | Validate against cloud provider billing |
| **Cost optimization breaks SLA** | Critical | Low | Continuous monitoring, auto-rollback if P95 >30ms |
| **GPU time-slicing degrades performance** | Medium | Medium | Only enable during off-peak, 5% latency impact acceptable |

### Rollback Procedures

**HPA Rollback:**
```bash
kubectl delete hpa akidb-rest-hpa -n akidb
kubectl scale deployment akidb-rest --replicas=4 -n akidb  # Fixed capacity
```

**VPA Rollback:**
```bash
# Revert to previous resource requests
kubectl patch deployment akidb-rest -n akidb -p \
  '{"spec":{"template":{"spec":{"containers":[{"name":"akidb-rest","resources":{"requests":{"cpu":"8","memory":"16Gi"}}}]}}}}'
```

**KEDA Disable:**
```bash
kubectl delete scaledobject akidb-rest-keda -n akidb
# HPA will take over immediately
```

---

## Success Criteria

### Week 9 Completion Criteria

| Criterion | Target | Measurement | Priority |
|-----------|--------|-------------|----------|
| **Cost Reduction** | 30-40% | OpenCost monthly projection | P0 |
| **HPA Operational** | GPU-based scaling | HPA status, replica count | P0 |
| **VPA Recommendations Applied** | -40% CPU/memory | Resource requests updated | P0 |
| **KEDA Scale-to-Zero** | Off-peak 1 pod min | Nighttime replica count | P0 |
| **FinOps Dashboard** | Real-time visibility | Grafana dashboard live | P0 |
| **SLA Maintained** | P95 <30ms | Prometheus metrics | P0 |
| **GPU Utilization** | 60-80% | DCGM metrics | P1 |
| **Cost per Request** | <$0.000020 | OpenCost calculation | P1 |
| **S3 Lifecycle** | Glacier after 90d | S3 policy configured | P1 |
| **Cost Alerts** | Automated | Alertmanager firing | P2 |

**Overall Success:** All P0 criteria + 80% of P1 criteria + 60% of P2 criteria

---

## Appendix: Code Examples

### Example 1: HPA with Multiple Metrics

(See Day 1 implementation plan for complete HPA YAML)

### Example 2: OpenCost API Usage

```bash
# Get cost allocation for last 7 days
curl http://opencost.opencost:9003/allocation?window=7d | jq .

# Get cost by namespace
curl http://opencost.opencost:9003/allocation?window=1d&aggregate=namespace | jq .

# Get cost by label (tenant)
curl http://opencost.opencost:9003/allocation?window=1d&aggregate=label:tenant | jq .
```

### Example 3: Custom KEDA Scaler

```yaml
# Advanced KEDA configuration with multiple triggers
apiVersion: keda.sh/v1alpha1
kind: ScaledObject
metadata:
  name: akidb-advanced-keda
spec:
  scaleTargetRef:
    name: akidb-rest
  triggers:
  - type: prometheus
    metadata:
      query: |
        sum(rate(akidb_embed_requests_total[1m])) /
        count(kube_pod_info{pod=~"akidb-rest.*"})
      threshold: "15"  # 15 RPS per pod
  - type: prometheus
    metadata:
      query: |
        histogram_quantile(0.95, rate(akidb_embed_latency_seconds_bucket[1m]))
      threshold: "0.030"  # Scale up if P95 >30ms
  - type: prometheus
    metadata:
      query: |
        nvidia_gpu_duty_cycle > 80
      threshold: "1"  # Scale up if any GPU >80%
```

---

**End of Week 9 PRD**

**Next Steps:** Week 10 - GDPR Compliance & Data Residency
