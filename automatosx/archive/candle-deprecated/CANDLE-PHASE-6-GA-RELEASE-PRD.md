# Phase 6: GA Release & Production Rollout PRD
## Candle Embedding Migration - Final Phase (6 Weeks)

**Version:** 1.0
**Date:** 2025-11-10
**Status:** Ready for Implementation
**Owner:** Backend Team + DevOps + Product
**Timeline:** 6 weeks (30 working days)

---

## Executive Summary

**Goal:** Execute **safe, gradual production rollout** of Candle embedding service, achieve **GA (General Availability) release**, decommission MLX, and close the migration project with **36x performance improvement delivered to production users**.

**Phase 6 Context:** This is the **culmination** of 5 phases of development. We have a production-ready Candle service (observability, resilience, multi-model, Dockerized, Helm-ready). Phase 6 is about **execution**: rolling out to production safely, monitoring closely, and celebrating success.

**Success Criteria:**
- ✅ 100% production traffic on Candle (MLX decommissioned)
- ✅ 99.9% uptime during migration
- ✅ Zero data loss, zero breaking API changes
- ✅ Users experience 36x performance improvement
- ✅ GA release (v2.0.0) published
- ✅ Post-mortem and lessons learned documented
- ✅ Migration project closed

---

## Table of Contents

1. [Problem Statement](#problem-statement)
2. [Goals & Non-Goals](#goals--non-goals)
3. [Rollout Strategy](#rollout-strategy)
4. [Week-by-Week Plan](#week-by-week-plan)
5. [Risk Management](#risk-management)
6. [Monitoring & Success Metrics](#monitoring--success-metrics)
7. [Rollback Procedures](#rollback-procedures)
8. [User Communication](#user-communication)
9. [GA Release Checklist](#ga-release-checklist)
10. [Decommissioning MLX](#decommissioning-mlx)
11. [Post-Mortem & Lessons Learned](#post-mortem--lessons-learned)
12. [Success Criteria](#success-criteria)
13. [Timeline & Milestones](#timeline--milestones)
14. [Deliverables](#deliverables)

---

## Problem Statement

### Current State (Post Phase 5)

Phase 5 delivered **deployment-ready infrastructure** with:
- ✅ Multi-arch Docker images
- ✅ Kubernetes manifests + Helm chart
- ✅ CI/CD pipeline (GitHub Actions)
- ✅ Blue-green deployment automation
- ✅ Staging environment validated

**However**, Candle is **not yet in production**:

| Gap | Impact | Risk |
|-----|--------|------|
| **0% production traffic** | Users don't benefit from 36x improvement | Wasted investment |
| **MLX still serving 100%** | Old system with GIL bottleneck | Poor performance |
| **No production validation** | Unknown production issues | Could fail at scale |
| **No GA release** | Users uncertain about stability | Low adoption |
| **No decommission plan** | Running two systems (expensive) | Technical debt |

### Why GA Release Matters

**Business Impact:**
- **Revenue:** Faster embeddings → better UX → higher retention → more revenue
- **Cost Savings:** 36x throughput → fewer servers → lower cloud costs
- **Competitive Edge:** Match/exceed Pinecone/Weaviate performance
- **Customer Trust:** GA release signals stability and commitment

**Technical Impact:**
- **Validation:** Production traffic validates all design decisions
- **Learning:** Real-world usage reveals optimization opportunities
- **Closure:** Complete migration, clean up technical debt
- **Foundation:** Production-ready embedding service for future features

---

## Goals & Non-Goals

### Goals (In Scope)

**Primary Goals:**
1. ✅ **Production Rollout:** 0% → 100% traffic over 5 weeks
2. ✅ **Zero Downtime:** 99.9% uptime during migration
3. ✅ **GA Release:** v2.0.0 with release notes and announcement
4. ✅ **MLX Decommission:** Shut down legacy system
5. ✅ **Documentation:** User guides, migration guides, API docs

**Secondary Goals:**
6. ✅ **Monitoring:** Comprehensive dashboards and alerts
7. ✅ **Performance Validation:** Verify 36x improvement in production
8. ✅ **User Feedback:** Collect and address user concerns
9. ✅ **Post-Mortem:** Document lessons learned
10. ✅ **Celebrate:** Recognize team achievement

### Non-Goals (Out of Scope)

**Deferred to Future:**
- ❌ New features beyond Candle migration
- ❌ Multi-region deployment (Future)
- ❌ Custom model training (Future)
- ❌ Fine-tuning API (Future)

**Explicitly Out of Scope:**
- ❌ Breaking API changes
- ❌ Infrastructure changes (K8s cluster upgrades)
- ❌ Major architectural changes
- ❌ Cost optimization beyond what Candle provides

---

## Rollout Strategy

### 6-Week Gradual Rollout

```
Week 1: Staging Validation + Final Prep
  └─ Staging: 100% traffic
  └─ Production: 0% traffic (MLX)
  └─ Status: Pre-production validation

Week 2: Canary (1% Production Traffic)
  └─ Staging: 100%
  └─ Production: 1% Candle, 99% MLX
  └─ Status: Initial production exposure

Week 3: Ramp to 10%
  └─ Staging: 100%
  └─ Production: 10% Candle, 90% MLX
  └─ Status: Early adopters

Week 4: Ramp to 50%
  └─ Staging: 100%
  └─ Production: 50% Candle, 50% MLX
  └─ Status: Majority validation

Week 5: Ramp to 100%
  └─ Staging: 100%
  └─ Production: 100% Candle, 0% MLX
  └─ Status: Full cutover

Week 6: GA Release + MLX Decommission
  └─ Staging: 100% Candle
  └─ Production: 100% Candle
  └─ MLX: Decommissioned
  └─ Status: GA v2.0.0 released 🎉
```

### Traffic Splitting Mechanism

**Using Kubernetes Ingress with Weighted Routing:**

```yaml
# Week 2: 1% canary
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  annotations:
    nginx.ingress.kubernetes.io/canary: "true"
    nginx.ingress.kubernetes.io/canary-weight: "1"  # 1% to Candle
---
# Week 3: 10%
nginx.ingress.kubernetes.io/canary-weight: "10"

# Week 4: 50%
nginx.ingress.kubernetes.io/canary-weight: "50"

# Week 5: 100% (remove canary annotation)
nginx.ingress.kubernetes.io/canary: "false"
```

---

## Week-by-Week Plan

### Week 1: Staging Validation + Final Prep

**Goals:**
- Validate staging environment 100% stable
- Fix any remaining issues
- Prepare monitoring dashboards
- Brief team on rollout plan

**Tasks:**
1. **Staging Load Test** (2 days)
   - Run 48-hour sustained load test
   - Target: 500 QPS (2.5x production peak)
   - Verify: P95 <35ms, error rate <0.1%
   - Monitor: Memory leaks, resource usage

2. **Monitoring Setup** (1 day)
   - Create Grafana dashboards
   - Configure Prometheus alerts
   - Set up PagerDuty integration
   - Test alert notifications

3. **Rollback Drill** (1 day)
   - Practice rollback procedure
   - Time rollback duration (target: <5 min)
   - Document rollback playbook
   - Train on-call engineers

4. **User Communication** (1 day)
   - Draft rollout announcement
   - Prepare changelog
   - Write migration guide for users
   - Schedule office hours

**Success Criteria:**
- ✅ Staging: 48-hour load test passed
- ✅ Dashboards: All metrics visible
- ✅ Rollback: Tested and <5 min
- ✅ Documentation: Complete

**Deliverables:**
- Load test report
- Monitoring dashboards
- Rollback playbook
- User communication draft

---

### Week 2: Canary (1% Production Traffic)

**Goals:**
- Deploy Candle to production (1% traffic)
- Monitor for any production-specific issues
- Validate performance improvements
- Build confidence for larger rollout

**Monday: Deploy Candle to Production**
```bash
# Deploy Candle (blue-green, not receiving traffic yet)
helm install akidb-candle ./helm/akidb-candle \
  --namespace production \
  --set replicaCount=2 \
  --set image.tag=v2.0.0-rc1

# Health check
kubectl wait --for=condition=ready pod \
  -l app=akidb-candle -n production --timeout=300s

# Verify readiness
curl https://api.akidb.example.com/health/ready
```

**Tuesday: Enable 1% Canary**
```bash
# Update Ingress to send 1% traffic to Candle
kubectl apply -f k8s/ingress-canary-1pct.yaml

# Monitor traffic split
kubectl logs -f -l app=nginx-ingress -n ingress-nginx | grep candle
```

**Wednesday-Friday: Monitor & Analyze**
- **Metrics to Watch:**
  - Error rate: Should be <0.1%
  - Latency P95: Should be <35ms (vs MLX baseline ~180ms)
  - Throughput: Validate handling 1% of production load
  - Memory usage: Should be stable (no leaks)

- **Dashboards:**
  - Real-time comparison: Candle vs MLX
  - User-facing metrics: Success rate, latency
  - Resource metrics: CPU, memory, GPU

**Go/No-Go Decision (Friday EOD):**
- ✅ GO: Error rate <0.1%, no crashes, performance as expected
- ❌ NO-GO: Rollback to 0%, investigate issues

**Success Criteria:**
- ✅ 1% traffic on Candle for 72+ hours
- ✅ Zero critical incidents
- ✅ Performance validated (5x faster than MLX)
- ✅ Go decision for Week 3

**Deliverables:**
- Week 2 status report
- Performance comparison (Candle vs MLX)
- Issues log (even if empty)

---

### Week 3: Ramp to 10%

**Goals:**
- Increase to 10% production traffic
- Validate Candle handles early adopters
- Collect user feedback
- Continue monitoring

**Monday: Ramp to 10%**
```bash
# Update canary weight to 10%
kubectl patch ingress akidb-candle-canary -n production \
  -p '{"metadata":{"annotations":{"nginx.ingress.kubernetes.io/canary-weight":"10"}}}'

# Scale up Candle replicas (2 → 3 for safety margin)
kubectl scale deployment akidb-candle --replicas=3 -n production
```

**Tuesday-Thursday: Monitor at 10%**
- Automated monitoring (alerts on any anomalies)
- Manual checks twice daily
- User feedback collection (support tickets, Slack)

**Friday: Performance Analysis**
- Generate week 3 performance report
- Compare Candle vs MLX metrics
- Document any issues and resolutions

**Success Criteria:**
- ✅ 10% traffic on Candle for 4+ days
- ✅ Error rate <0.1%
- ✅ User feedback mostly positive
- ✅ Go decision for Week 4

**Deliverables:**
- Week 3 status report
- User feedback summary
- Performance metrics

---

### Week 4: Ramp to 50%

**Goals:**
- Increase to 50% production traffic (majority)
- Validate Candle as primary system
- Stress test autoscaling
- Final validation before 100%

**Monday: Ramp to 50%**
```bash
# Update canary weight to 50%
kubectl patch ingress akidb-candle-canary -n production \
  -p '{"metadata":{"annotations":{"nginx.ingress.kubernetes.io/canary-weight":"50"}}}'

# Scale up for 50% traffic (estimate 5 replicas)
kubectl scale deployment akidb-candle --replicas=5 -n production
```

**Tuesday-Wednesday: Stress Test**
- Simulate traffic spike (2x normal load)
- Verify HPA scales correctly (5 → 8 replicas)
- Monitor scale-up and scale-down behavior

**Thursday-Friday: Soak Test**
- Run at 50% for 48 hours
- Monitor for any gradual degradation
- Check for memory leaks, resource exhaustion

**Success Criteria:**
- ✅ 50% traffic on Candle for 4+ days
- ✅ HPA scales correctly (tested)
- ✅ No performance degradation over 48h
- ✅ Go decision for Week 5 (100%)

**Deliverables:**
- Week 4 status report
- Stress test results
- Autoscaling validation

---

### Week 5: Ramp to 100% (Full Cutover)

**Goals:**
- Move 100% production traffic to Candle
- MLX goes to 0% (standby for 48h)
- Final production validation
- Prepare for GA release

**Monday: Final Pre-Flight**
- Review all metrics from Week 1-4
- Confirm team availability for cutover
- Notify users of final cutover
- Prepare rollback plan

**Tuesday: Cutover to 100%**
```bash
# Remove canary annotation (100% to Candle)
kubectl patch ingress akidb-candle -n production \
  -p '{"metadata":{"annotations":{"nginx.ingress.kubernetes.io/canary":"false"}}}'

# Verify traffic routing
curl -H "Host: api.akidb.example.com" https://api.akidb.example.com/metrics | grep candle_requests_total

# Scale down MLX to 1 replica (standby)
kubectl scale deployment akidb-mlx --replicas=1 -n production
```

**Wednesday-Thursday: Monitor 100%**
- Continuous monitoring (24/7 on-call)
- Automated alerts on any anomalies
- User feedback collection
- Performance validation

**Friday: Celebrate! 🎉**
- 100% traffic on Candle successfully
- Schedule GA release for Week 6
- Internal team celebration
- Prepare GA release notes

**Success Criteria:**
- ✅ 100% traffic on Candle for 72+ hours
- ✅ Zero rollbacks needed
- ✅ Performance targets met (200+ QPS, P95 <35ms)
- ✅ Users happy (positive feedback)

**Deliverables:**
- Week 5 status report
- 100% cutover confirmation
- GA release preparation

---

### Week 6: GA Release + MLX Decommission

**Goals:**
- Publish GA release (v2.0.0)
- Decommission MLX completely
- Project closure and post-mortem
- Team celebration

**Monday-Tuesday: GA Release**
```bash
# Tag GA release
git tag -a v2.0.0 -m "GA Release: Candle Embedding Service"
git push origin v2.0.0

# Publish Docker image
docker tag akidb/embedding:candle-latest akidb/embedding:v2.0.0
docker push akidb/embedding:v2.0.0

# Publish Helm chart
helm package helm/akidb-candle
helm push akidb-candle-2.0.0.tgz oci://ghcr.io/yourusername/charts

# Publish release notes
gh release create v2.0.0 \
  --title "AkiDB 2.0 GA: Candle Embedding Service" \
  --notes-file RELEASE_NOTES.md
```

**Wednesday: Decommission MLX**
```bash
# Scale down MLX to 0
kubectl scale deployment akidb-mlx --replicas=0 -n production

# Wait 24 hours (safety buffer)

# Delete MLX resources (Thursday)
helm uninstall akidb-mlx -n production
kubectl delete pvc akidb-mlx-cache -n production

# Remove MLX code from repository (optional)
git rm -r crates/akidb-embedding/src/mlx.rs
git rm -r crates/akidb-embedding/python/
```

**Thursday: Post-Mortem**
- Schedule post-mortem meeting
- Document lessons learned
- Identify improvement opportunities
- Archive project artifacts

**Friday: Project Closure & Celebration**
- Final project report
- Team retrospective
- Share success story (blog post)
- Team celebration (dinner, awards)

**Success Criteria:**
- ✅ v2.0.0 GA released
- ✅ MLX decommissioned
- ✅ Post-mortem complete
- ✅ Project closed

**Deliverables:**
- GA release (v2.0.0)
- Release notes and announcement
- MLX decommission confirmation
- Post-mortem report
- Final project report

---

## Risk Management

### Identified Risks and Mitigation

| Risk | Probability | Impact | Mitigation | Owner |
|------|-------------|--------|------------|-------|
| **Production traffic spike during rollout** | Medium | High | • Gradual rollout (1% → 10% → 50% → 100%)<br>• HPA configured for 2-10 replicas<br>• Pre-scale before each ramp | DevOps |
| **Candle performance worse than staging** | Low | Critical | • Staging matches production (same load)<br>• Rollback plan ready (<5 min)<br>• Canary deployment allows quick detection | Backend |
| **Memory leak in production** | Low | High | • 48-hour soak test in staging<br>• Memory monitoring alerts<br>• Automatic pod restarts if memory >90% | Backend |
| **User-facing errors** | Medium | Critical | • Error rate alerts (<0.1% threshold)<br>• Automatic rollback on high errors<br>• On-call 24/7 during rollout | Backend |
| **MLX dependencies not cleanly removed** | Low | Medium | • Decommission checklist<br>• Grace period (24h at 0% before delete)<br>• Backups of MLX config | DevOps |
| **Communication gaps** | Medium | Medium | • Weekly status updates<br>• Slack channel for rollout<br>• User announcements at each phase | Product |
| **Rollback takes >5 minutes** | Low | High | • Rollback drill in Week 1<br>• Automated rollback script<br>• Traffic can be redirected in <1 min | DevOps |

---

## Monitoring & Success Metrics

### Key Performance Indicators (KPIs)

**1. Reliability (SLI/SLO)**
- **Uptime:** 99.9% (SLO)
- **Error Rate:** <0.1% (SLO)
- **Incident Count:** 0 critical incidents during rollout

**2. Performance**
- **Throughput:** 200+ QPS (target), measured in production
- **Latency P50:** <20ms
- **Latency P95:** <35ms (vs MLX baseline ~180ms = 5x improvement)
- **Latency P99:** <50ms

**3. Resource Efficiency**
- **CPU Utilization:** 60-80% (not maxed out, not wasted)
- **Memory Usage:** <4GB per pod
- **Pod Count:** 2-10 (autoscaling working)
- **Cost Savings:** 36x throughput → fewer pods → lower cost

**4. User Satisfaction**
- **Support Tickets:** <5 complaints during rollout
- **Positive Feedback:** >80% of users report faster performance
- **API Breaking Changes:** 0 (backward compatible)

### Dashboards

**Dashboard 1: Rollout Progress**
- Current traffic split (Candle vs MLX)
- Deployment status (pods ready)
- Error rate comparison
- Latency comparison (P50/P95/P99)

**Dashboard 2: Candle Performance**
- Requests per second
- Latency distribution
- Error count and rate
- Resource usage (CPU, memory)

**Dashboard 3: User Impact**
- API success rate
- Request duration (user-facing)
- Error types and counts
- Traffic patterns

### Alerts

```yaml
# Critical Alerts (Page on-call immediately)
- High Error Rate: >0.5% errors for 5 minutes
- Service Down: <1 healthy pod for 2 minutes
- High Latency: P95 >100ms for 10 minutes

# Warning Alerts (Slack notification)
- Elevated Error Rate: >0.1% errors for 10 minutes
- Scaling Issues: HPA not scaling when CPU >80%
- Memory Pressure: Memory usage >90% for 5 minutes
```

---

## Rollback Procedures

### When to Rollback

**Immediate Rollback Triggers:**
- Error rate >1% for 5 minutes
- Service completely unavailable (all pods crashing)
- Critical security vulnerability discovered
- Data corruption detected

**Planned Rollback Triggers:**
- Error rate >0.5% sustained for 30 minutes
- P95 latency >100ms sustained for 1 hour
- User complaints exceed threshold (>10 tickets/hour)
- Memory leak detected (gradual degradation)

### Rollback Steps

```bash
#!/bin/bash
# scripts/rollback-to-mlx.sh

echo "EMERGENCY ROLLBACK TO MLX"

# Step 1: Redirect all traffic to MLX immediately
kubectl patch ingress akidb-candle -n production \
  -p '{"spec":{"rules":[{"http":{"paths":[{"backend":{"service":{"name":"akidb-mlx"}}}]}}]}}'

echo "Traffic redirected to MLX"

# Step 2: Scale up MLX (if scaled down)
kubectl scale deployment akidb-mlx --replicas=5 -n production

echo "MLX scaled up"

# Step 3: Verify MLX is handling traffic
sleep 30
ERROR_RATE=$(curl -s http://akidb-mlx.production.svc:8080/metrics | grep errors_total | awk '{print $2}')

if [ "$ERROR_RATE" -lt 10 ]; then
  echo "Rollback successful. MLX is healthy."
else
  echo "WARNING: MLX also has errors. Escalate to senior engineers."
fi

# Step 4: Scale down Candle to save resources
kubectl scale deployment akidb-candle --replicas=1 -n production

# Step 5: Notify team
curl -X POST https://hooks.slack.com/... \
  -d '{"text": "ROLLBACK EXECUTED: Traffic redirected to MLX. Candle investigation needed."}'

echo "Rollback complete. Duration: $SECONDS seconds"
```

**Expected Rollback Duration:** <5 minutes (1 minute for traffic redirect, 4 minutes for MLX scale-up)

---

## User Communication

### Communication Timeline

**Week 0 (Before Week 1):**
- Email to all users: "Upcoming performance improvements"
- Blog post: "Candle migration overview"
- Slack announcement: Office hours scheduled

**Week 1:**
- Status update: "Staging validation in progress"

**Week 2:**
- Announcement: "1% canary deployment started"
- Request: "Report any issues to support@akidb.com"

**Week 3:**
- Update: "10% rollout, performance improvements confirmed"
- Early feedback: Share user testimonials

**Week 4:**
- Update: "50% rollout, majority of traffic on Candle"

**Week 5:**
- Announcement: "100% cutover complete! 🎉"
- Highlight: "36x performance improvement now available to all"

**Week 6:**
- GA Release: "AkiDB 2.0 GA available"
- Thank you: "Thanks for your patience during migration"

### Sample Communication: GA Release Announcement

```markdown
# AkiDB 2.0 GA: 36x Faster Embeddings with Candle

We're excited to announce the General Availability of AkiDB 2.0, featuring our new Candle embedding service!

**What's New:**
- 🚀 **36x faster throughput**: 200+ QPS (vs 5.5 QPS previously)
- ⚡ **5x lower latency**: P95 <35ms (vs ~180ms previously)
- 🌍 **Multi-model support**: Choose from 4 embedding models
- 🔧 **Runtime model selection**: Switch models via API parameter
- 📦 **Kubernetes-native**: Easy deployment with Helm charts

**For Users:**
- No action required - all existing API calls work as before
- Enjoy significantly faster embedding generation
- Explore new models: `e5-small-v2` (multilingual), `bert-base-uncased` (high quality)

**Migration Notes:**
- Zero breaking changes - fully backward compatible
- MLX service decommissioned (Candle is now the default)
- See migration guide: https://docs.akidb.io/migration-v2

**Try It:**
```bash
curl -X POST https://api.akidb.example.com/api/v1/embed \
  -H "Content-Type: application/json" \
  -d '{"texts": ["Your text here"], "model": "e5-small-v2"}'
```

**Thank You:**
This migration was a 6-week journey. Thank you for your patience and feedback!

Questions? support@akidb.io
```

---

## GA Release Checklist

### Pre-Release (Week 5)

- [ ] All tests passing (81+ tests)
- [ ] Performance benchmarks run and documented
- [ ] Security audit completed
- [ ] Documentation updated (API docs, user guides)
- [ ] CHANGELOG.md updated
- [ ] Release notes drafted
- [ ] Docker images tagged with v2.0.0
- [ ] Helm chart versioned to 2.0.0

### Release Day (Week 6 Monday)

- [ ] Create git tag: `v2.0.0`
- [ ] Push Docker images: `akidb/embedding:v2.0.0`
- [ ] Publish Helm chart
- [ ] Create GitHub release with notes
- [ ] Update website (docs.akidb.io)
- [ ] Send email to users
- [ ] Post on blog
- [ ] Announce on social media (Twitter, LinkedIn)
- [ ] Update status page (status.akidb.io)

### Post-Release (Week 6 Tuesday-Friday)

- [ ] Monitor for issues (24h intensive)
- [ ] Respond to user questions
- [ ] Collect user feedback
- [ ] Address any hotfixes needed
- [ ] Update FAQ based on questions
- [ ] Schedule post-release retrospective

---

## Decommissioning MLX

### Decommission Checklist

**Phase 1: Reduce to Standby (Week 5)**
- [ ] Scale MLX to 1 replica (standby for emergencies)
- [ ] Update monitoring to de-emphasize MLX metrics
- [ ] Document rollback procedure (if needed)

**Phase 2: Zero Traffic (Week 6 Wednesday)**
- [ ] Verify 100% traffic on Candle for 7+ days
- [ ] Scale MLX to 0 replicas
- [ ] Wait 24 hours (safety buffer)

**Phase 3: Delete Resources (Week 6 Thursday)**
- [ ] Delete MLX Deployment
- [ ] Delete MLX Service
- [ ] Delete MLX Ingress
- [ ] Delete MLX ConfigMap/Secrets
- [ ] Delete MLX PVC (model cache)

**Phase 4: Code Cleanup (Week 6 Friday)**
- [ ] Remove MLX code from repository
  - `crates/akidb-embedding/src/mlx.rs`
  - `crates/akidb-embedding/python/`
- [ ] Remove MLX feature flag from Cargo.toml
- [ ] Remove MLX from documentation
- [ ] Archive MLX deployment manifests (for reference)

**Phase 5: Final Cleanup (Post Week 6)**
- [ ] Remove MLX from monitoring dashboards
- [ ] Remove MLX alerts
- [ ] Update runbooks to remove MLX references
- [ ] Archive MLX Docker images (keep for 30 days, then delete)

---

## Post-Mortem & Lessons Learned

### Post-Mortem Agenda

**1. Project Overview (10 min)**
- Timeline: 6 weeks (Phases 1-6)
- Goals: Replace MLX with Candle, achieve 36x improvement
- Outcome: Success! 100% production traffic on Candle

**2. What Went Well (20 min)**
- Gradual rollout prevented major incidents
- Comprehensive testing caught issues early
- Team collaboration was excellent
- User communication was clear
- Performance targets exceeded

**3. What Didn't Go Well (20 min)**
- [To be filled based on actual rollout]
- Example: Initial staging load test revealed memory leak
- Example: Week 3 had brief latency spike due to HPA tuning

**4. Lessons Learned (20 min)**
- Always test at >100% expected load (found edge cases)
- Rollback drills are essential (built confidence)
- User communication should start early (reduced anxiety)
- Gradual rollout (1% → 10% → 50% → 100%) was the right approach

**5. Action Items (10 min)**
- Apply learnings to future migrations
- Document best practices
- Update runbooks based on production experience

### Success Metrics Summary

| Metric | Target | Actual | Status |
|--------|--------|--------|--------|
| **Uptime during migration** | 99.9% | TBD | ✅ |
| **Throughput** | 200+ QPS | TBD | ✅ |
| **Latency P95** | <35ms | TBD | ✅ |
| **Error rate** | <0.1% | TBD | ✅ |
| **Rollbacks** | 0 | TBD | ✅ |
| **User complaints** | <10 | TBD | ✅ |
| **Timeline** | 6 weeks | TBD | ✅ |

---

## Success Criteria

### Functional Requirements

✅ **FR1:** 100% production traffic on Candle
✅ **FR2:** MLX fully decommissioned
✅ **FR3:** GA release (v2.0.0) published
✅ **FR4:** Zero breaking API changes
✅ **FR5:** User documentation complete
✅ **FR6:** Post-mortem documented
✅ **FR7:** Project formally closed

### Non-Functional Requirements

✅ **NFR1: Reliability**
- 99.9% uptime during 6-week rollout
- Zero critical incidents
- Zero data loss

✅ **NFR2: Performance**
- Throughput: 200+ QPS in production
- Latency P95: <35ms (5x improvement vs MLX)
- Users report faster performance

✅ **NFR3: Safety**
- Gradual rollout (1% → 10% → 50% → 100%)
- Rollback tested and <5 min
- No forced downtime

✅ **NFR4: User Experience**
- Clear communication at each phase
- Backward compatibility maintained
- Positive user feedback

✅ **NFR5: Cost**
- 36x throughput → fewer servers → lower cost
- Old MLX infrastructure decommissioned → no dual-running cost

---

## Timeline & Milestones

### 6-Week Schedule

| Week | Phase | Traffic Split | Milestones |
|------|-------|---------------|------------|
| **1** | Staging Validation | 0% Candle | • Load test passed<br>• Monitoring ready<br>• Rollback drill complete |
| **2** | Canary | 1% Candle | • Candle in production<br>• 72h at 1% successful<br>• Go for Week 3 |
| **3** | Early Adopters | 10% Candle | • 4+ days at 10%<br>• User feedback positive<br>• Go for Week 4 |
| **4** | Majority | 50% Candle | • Stress test passed<br>• HPA validated<br>• Go for Week 5 |
| **5** | Full Cutover | 100% Candle | • 72h at 100% successful<br>• MLX at standby<br>• Prepare GA |
| **6** | GA Release | 100% Candle | • v2.0.0 released 🎉<br>• MLX decommissioned<br>• Project closed |

### Critical Milestones

- **M1 (Week 1 EOD):** Staging validated, ready for production
- **M2 (Week 2 EOD):** 1% canary successful, go for 10%
- **M3 (Week 3 EOD):** 10% successful, go for 50%
- **M4 (Week 4 EOD):** 50% successful, go for 100%
- **M5 (Week 5 EOD):** 100% cutover, prepare GA
- **M6 (Week 6 EOD):** GA released, MLX decommissioned, **PROJECT COMPLETE** 🎉

---

## Dependencies

### Internal Dependencies

**From Phase 1-5:**
- ✅ Candle service (production-ready)
- ✅ Multi-model support (4 models)
- ✅ Observability (metrics, tracing, logging)
- ✅ Docker images (multi-arch)
- ✅ Kubernetes manifests + Helm chart
- ✅ CI/CD pipeline

**Blockers:**
- ❌ None (all phases complete)

### External Dependencies

**Infrastructure:**
- Production Kubernetes cluster (stable)
- Monitoring stack (Prometheus, Grafana, Jaeger)
- Ingress controller (nginx with canary support)
- On-call rotation (24/7 during rollout)

**Team:**
- Backend engineers (implementation, monitoring)
- DevOps (deployment, infrastructure)
- Product (user communication)
- Support (user questions)

---

## Deliverables

### Code Deliverables

| File | Description |
|------|-------------|
| `scripts/rollback-to-mlx.sh` | Automated rollback script |
| `k8s/ingress-canary-*.yaml` | Canary Ingress configs (1%, 10%, 50%) |
| `RELEASE_NOTES.md` | v2.0.0 release notes |
| `CHANGELOG.md` | Complete changelog |
| `docs/MIGRATION-GUIDE.md` | MLX → Candle migration guide |

### Documentation Deliverables

1. **Weekly Status Reports** (Weeks 1-6)
   - Traffic split progress
   - Performance metrics
   - Issues and resolutions
   - Go/no-go decisions

2. **GA Release Package**
   - Release notes
   - User announcement
   - API documentation
   - Migration guide

3. **Post-Mortem Report**
   - What went well
   - What didn't go well
   - Lessons learned
   - Action items

4. **Final Project Report**
   - Executive summary
   - Timeline and milestones
   - Success metrics
   - Team contributions

---

## Sign-Off

**Phase 6 PRD Version:** 1.0
**Status:** ✅ Ready for Implementation
**Estimated Effort:** 6 weeks (30 working days)
**Expected Completion:** End of Week 6 → **GA RELEASE v2.0.0** 🎉

**End of Candle Migration:** This is the final phase. Upon completion, the Candle migration project will be **complete and closed**.

---

**Document End**
