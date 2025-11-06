# AkiDB 2.0 Pre-Flight Checklist

## Go/No-Go Checklist (Complete Before Week 1 Kickoff)
- [ ] Confirm Qwen3-Embedding-8B commercial licensing rights for edge redistribution; document approved usage terms and constraints — Owner: Legal + ML Eng Lead — Due: 2025-11-13
- [ ] Secure procurement or reservation proof for 4× Jetson Orin dev nodes and verified OCI ARM (Ampere) quota for staging/prod parity — Owner: DevOps — Due: 2025-11-13
- [ ] Capture AkiDB v1.x performance baselines (ingest throughput, P95 latency, resource footprint) on current customer-like workloads for regression targets — Owner: Performance Eng — Due: 2025-11-15
- [ ] Finalize 12-week staffing plan with named engineers, confirming Cedar policy expertise and embedding runtime coverage; identify backfills for PTO/conflicts — Owner: Engineering Director — Due: 2025-11-12
- [ ] Secure budget approval memo (engineering + infra) with CFO sign-off, including 10% contingency and procurement timelines — Owner: Product + Finance — Due: 2025-11-12

## Missing Artifacts to Author
- [ ] Detailed migration playbooks (SQL + automation scripts) covering schema bootstrap, dual-write cutover, rollback for tenants >1 TB — Owner: Storage Team — Due: 2025-11-20
- [ ] Team onboarding & enablement plan for new architecture (Rust workspace map, Cedar authoring primer, RAM-tier operations lab) — Owner: Engineering Enablement — Due: 2025-11-25
- [ ] Customer communication pack (release narrative, upgrade FAQs, deprecation timeline, success metrics) for v1.x → v2.0 transition — Owner: Product Marketing — Due: 2025-11-27
- [ ] Pilot program brief defining candidate selection criteria, success KPIs, incentive structure, and Week 9-13 schedule — Owner: Product — Due: 2025-11-22
- [ ] Integration testing strategy doc mapping new services → existing tooling (CLI, SDKs, MCP) with test harness ownership — Owner: Quality — Due: 2025-11-21

## Dependency Risks & Mitigations
- [ ] Qwen3-Embedding-8B supply risk: validate license + availability; line up Embedding-Gemma or Voyage-large as CPU-friendly fallback with quantization benchmarks — Owner: ML Eng — Due: 2025-11-18
- [ ] Cedar policy engine dependency: benchmark P99 latency at 10k policies/tenant; prep OPA-based policy interpreter fallback package — Owner: Platform Security — Due: 2025-11-24
- [ ] OCI ARM quota saturation: confirm secondary cloud provider (AWS Graviton) deployment template; document switch-over runbook — Owner: DevOps — Due: 2025-11-26
- [ ] Edge hardware logistics: create vendor escalation contact list and spares plan (≥1 hot spare Jetson, replacement SLA <48h) — Owner: Operations — Due: 2025-11-19

## Team Readiness Assessment
- [ ] Close Cedar policy authoring skill gap via 2-day workshop + sample policies review; ensure at least 2 engineers can review/author Cedar — Owner: Platform Security Lead — Due: 2025-11-19
- [ ] Assign dedicated embedding runtime specialist (ONNX/TensorRT) and document coverage plan for after-hours incidents — Owner: ML Eng Manager — Due: 2025-11-18
- [ ] Validate SRE coverage for edge deployments (Jetson + OCI) with updated on-call rotation including hardware triage expertise — Owner: SRE Lead — Due: 2025-11-20
- [ ] Confirm QA lab capability for ARM hardware-in-the-loop with device scheduling and automation scripts ready — Owner: Quality Lead — Due: 2025-11-17

## Communication Plan
- [ ] Publish stakeholder RACI + cadence (weekly core standup, exec bi-weekly readout, customer advisory call schedule) — Owner: Product — Due: 2025-11-14
- [ ] Draft external announcement timeline (T-6 weeks teaser, T-2 weeks feature brief, GA day blog) aligned with GTM — Owner: Product Marketing — Due: 2025-11-21
- [ ] Prepare customer upgrade notification template including opt-in pilot invite and support contact tree — Owner: Customer Success — Due: 2025-11-18
- [ ] Set up internal status dashboard (metrics, blockers, risk burndown) in shared workspace; review cadence agreed with leadership — Owner: Program Manager — Due: 2025-11-15

## Quick Wins While Approvals Land
- [ ] Kick off Cedar policy modeling in sandbox using synthetic tenants to accelerate later integration — Owner: Platform Security — Start: 2025-11-08
- [ ] Draft developer quickstart outline (Mac ARM install → first index) to shorten documentation lead time — Owner: Developer Relations — Start: 2025-11-09
- [ ] Begin load test scenario design for 100 QPS hybrid search and failover drills so scripts are ready once hardware arrives — Owner: Quality — Start: 2025-11-10
- [ ] Prototype monitoring dashboards (Prometheus + Grafana) with placeholder metrics to validate data model and layout — Owner: SRE — Start: 2025-11-11
