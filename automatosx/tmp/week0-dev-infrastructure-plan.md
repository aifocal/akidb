# Week 0 Development Infrastructure Setup Plan

Great architecture is invisible - it enables teams, evolves gracefully, and pays dividends over decades. This plan establishes the Week 0 foundation for AkiDB 2.0 Phase 1 delivery.

## 1. Objectives

- Stand up core development infrastructure that mirrors target production characteristics (ARM64-first, accelerated inference paths).
- Ensure every engineer can build, test, and observe AkiDB 2.0 components by Phase 1 kickoff.
- Bake in governance (access, monitoring, backups) from day zero to minimize downstream architecture debt.

## 2. Scope & Dependencies

- Covers shared infrastructure: CI runners, staging, observability, databases, edge devices.
- Assumes procurement approvals submitted (Week -1) and hardware orders in flight.
- Alignment with ADR-002 (ESM), ADR-003 (TypeScript strict), ADR-007 (lazy loading) informs tooling versions.

## 3. Infrastructure Components

- **ARM64 GitHub Actions Runners**
  - Hardware: Mac mini M2 Pro (preferred) or OCI Ampere A1 instances.
  - Image: macOS Sonoma 14.4 (Mac mini) / Ubuntu 22.04 ARM64 (OCI).
  - Required tooling: Xcode CLI, Node 20, Rust stable, Docker, cedar-policy-cli.
  - Capacity: 6 concurrent jobs baseline, burst to 12 with auto-scaling on OCI.

- **Development Jetson Orin Cluster**
  - Target: 2× Jetson Orin AGX 64GB.
  - OS: JetPack 6.x, containerized workloads via `nvcr.io`.
  - Usage: On-device inference experiments, data pipeline validation.
  - Interim plan: Use OCI A1 GPU-enabled emulation until hardware arrives (Week 0, Nov 6-13).

- **OCI ARM Staging Environment**
  - Shape: `VM.Standard.A1.Flex` (8 OCPUs, 64 GB RAM) x 3 nodes.
  - Networking: Private VCN, service gateway for artifact pulls, bastion host with MFA.
  - Deployment: Nomad or Kubernetes (decision pending ADR review); start with Docker Swarm for Week 0 to unblock builds.

- **Observability Stack**
  - Prometheus + Grafana (ARM64 images) deployed via Helmfile.
  - Log aggregation: Loki stack with S3-compatible object storage (OCI Object Storage).
  - Alerts routed to PagerDuty (engineering) and Slack #akidb-infra.

- **SQLite Test Databases**
  - Pre-populated fixtures: 5 tenants, synthetic workloads (aligned with Cedar sandbox schema).
  - Distribution: Provide `.db` files via internal artifact registry; refresh weekly.

## 4. Setup Timeline

### Week 0 (Nov 6–13): Procurement & Planning

- Finalize BOM for Mac mini and Jetson Orin hardware; submit purchase orders.
- Reserve OCI tenancy quotas for ARM compute, block storage, and object storage.
- Draft CI/CD architecture ADR (link to `.automatosx/PRD/adr-ci-platform.md` once created).
- Author access-control matrix and submit to Security for review.
- Prepare IaC repositories (Terraform / Pulumi) skeleton with placeholders.

### Week 0 (Nov 14–20): Initial Setup & Validation

- Rack and baseline Mac mini runners; enroll in GitHub Actions self-hosted fleet.
- Provision OCI ARM staging cluster; deploy bootstrap services (Docker registry mirror, secrets management).
- Install Prometheus + Grafana + Loki; configure basic dashboards (build latency, test pass-rate, cedar evaluation latency).
- Load SQLite fixtures into shared artifact store; publish usage guide.
- Validate end-to-end CI pipeline: commit → build (ARM64) → deploy to staging → run smoke tests.
- Document runbooks in `automatosx/PRD/` for each component.

## 5. Access Control & Permissions

- **Identity Source**: Okta SSO with GitHub Enterprise integration.
- **Principals**:
  - Platform Engineering (admin on CI/staging)
  - Application Engineering (deploy + read observability)
  - Security (read observability, manage Cedar sandbox)
  - QA (trigger staging deployments, read metrics)
- **Controls**:
  - Bastion host with short-lived SSH certificates (10 min).
  - GitHub branch protections with CODEOWNERS.
  - Secrets stored in HashiCorp Vault (transit + audit logs).
  - Jetson Orin physical access logged and limited to Platform Engineering.

## 6. Cost Estimates & Quotas (Monthly)

- Mac mini hosting (on-prem colocation): ~$250 (power + rack) per unit × 4 = $1,000.
- OCI A1 instances: $0.0105 per OCPU-hour → ~$600 for continuous staging cluster.
- Object storage for logs/metrics: 2 TB at $0.0255/GB → ~$51.
- Prometheus/Grafana managed maintenance: 40 engineer-hours ($150/hr blended) → $6,000.
- Contingency (15%): ~$1,160.
- Total forecast: **$8,811/month** (update ADR-010 once confirmed).

Quotas requested:
- 96 OCPUs (ARM) reserved.
- 20 TB outbound bandwidth.
- 10 TB object storage.

## 7. Monitoring & Alerting

- Prometheus scrape targets: CI runners, staging nodes, Jetson Orin once online.
- Dashboards:
  - Build success/fail counts.
  - Cedar evaluation latency (P50/P99).
  - DB migration duration.
  - GPU utilization (Jetson).
- Alerts:
  - `ci-runner.capacity.exhausted` (critical).
  - `staging.deployment.failure` (warning).
  - `observability.pipeline.stalled` (critical).
  - `cedar.latency.p99>5ms` sustained 15 minutes (warning).

Escalation path: PagerDuty → Slack → On-call engineer; Security notified for Cedar latency breaches.

## 8. Backup & Disaster Recovery

- **CI Runner Images**: Weekly snapshot via Ansible + Packer; stored in OCI Object Storage.
- **Staging Cluster**: Terraform state backed up to versioned bucket; daily database dumps (SQLite fixtures) replicated to cold storage.
- **Observability Stack**: Retain metrics 14 days, logs 30 days; export Grafana dashboards as JSON to git.
- **Jetson Orin**: Clone SD/NVMe images monthly; keep golden image for rapid reflash.
- DR rehearsal: Quarterly failover drill (simulate node loss, verify rebuild in <2 hours).

## 9. Developer Onboarding Checklist

- [ ] GitHub access with correct team membership.
- [ ] Install toolchain (Node 20, Rust stable, Docker, cedar-policy-cli).
- [ ] Configure `akidb` repo with pre-commit hooks (lint, typecheck, cedar validation).
- [ ] Pull latest SQLite fixtures and Cedar sandbox policies.
- [ ] Validate ability to run ARM64 Docker builds locally (or via Colima/UTM).
- [ ] Access Grafana dashboards (read-only).
- [ ] Review Week 0 runbooks and ADRs.

## 10. Troubleshooting Guide

- **CI Runner Offline**
  - Check GitHub Actions self-hosted status.
  - Validate VPN/bastion connectivity.
  - Inspect `systemd` services (`runner.service`, `docker.service`).
  - If Mac mini hardware fault → failover to OCI A1, open hardware ticket.

- **Staging Deployment Failure**
  - Inspect Nomad/K8s events (depending on selected orchestrator).
  - Review container logs via Loki.
  - Roll back to previous deployment through GitHub Actions workflow.

- **Observability Gaps**
  - Confirm Prometheus scrape configs; redeploy Helmfile.
  - Check object storage quota and credentials.
  - Verify Grafana datasource secrets in Vault.

- **Jetson Orin Provisioning Delayed**
  - Use OCI Ampere GPU emulation; flag risk in Architecture Debt log.
  - Schedule follow-up once hardware arrives; update runbook.

- **SQLite Fixture Drift**
  - Re-run fixture generator pipeline; compare schema with Cedar sandbox entities.
  - Document schema changes in ADR and notify Platform Security.

## 11. Governance & Next Steps

- Log all infrastructure decisions in ADR register; link from `.automatosx/abilities/our-architecture-decisions.md`.
- Schedule first Architecture Runway sync (Nov 21) to assess readiness and adjust backlog.
- Prepare Phase 1 sprint goals contingent on successful Week 0 execution.
- Monitor costs bi-weekly; adjust quotas to prevent overruns.

By completing this plan during Week 0, we equip engineering teams with a resilient, observable, and secure foundation—letting future architecture remain invisible yet invaluable.
