# AkiDB RC1 Design Partner Pilot Program

**Program Duration:** 2 weeks (November 2025)
**Participant Commitment:** 5-10 hours/week
**Number of Partners:** 3-5 organizations
**Program Status:** 🚀 OPEN FOR APPLICATIONS

---

## Table of Contents

1. [Program Overview](#program-overview)
2. [Design Partner Benefits](#design-partner-benefits)
3. [Selection Criteria](#selection-criteria)
4. [Pilot Timeline](#pilot-timeline)
5. [Deployment Package](#deployment-package)
6. [Feedback Collection](#feedback-collection)
7. [Success Metrics](#success-metrics)
8. [Support & Communication](#support--communication)
9. [Application Process](#application-process)
10. [FAQ](#faq)

---

## Program Overview

### What is the Design Partner Pilot?

The AkiDB Design Partner Pilot Program invites 3-5 select organizations to deploy and test **AkiDB v2.0.0-rc1** in real-world scenarios before the general availability (GA) release. This is an exclusive opportunity to shape the future of AkiDB while gaining early access to cutting-edge vector database technology.

### Why Participate?

**For Design Partners:**
- 🎯 **Early Access** - Test RC1 before public release
- 🤝 **Direct Support** - 1:1 engineering support throughout pilot
- 💡 **Influence Roadmap** - Your feedback directly shapes RC2 and GA
- 🎁 **Extended Benefits** - 3 months free support post-GA
- 🏆 **Recognition** - Featured in launch announcements and case studies

**For AkiDB:**
- 📊 **Real-World Validation** - Test with diverse use cases and workloads
- 🐛 **Bug Discovery** - Identify and fix issues before GA
- 📈 **Performance Insights** - Optimize based on actual usage patterns
- 💬 **User Feedback** - Understand developer pain points and needs

### Program Timeline

**Week 1 (Days 1-5): Deployment & Initial Testing**
- Deploy RC1 to staging environment
- Verify basic functionality
- Begin initial testing workflows
- Daily check-ins with engineering team

**Week 2 (Days 6-10): Production Testing & Feedback**
- Scale to production workloads
- Comprehensive testing across all features
- Submit detailed feedback
- Exit interview and commitment decision

---

## Design Partner Benefits

### During Pilot (2 Weeks)

**Direct Engineering Support:**
- Dedicated Slack channel: `#pilot-design-partners`
- Daily monitoring and assistance
- <24 hour response time for critical issues
- 1:1 video calls for troubleshooting

**Early Access & Influence:**
- Test RC1 before public release (2-4 week advantage)
- Feature request priority
- Direct input on API design decisions
- Preview of roadmap and upcoming features

**Comprehensive Documentation:**
- Pilot-specific deployment guides
- Troubleshooting playbooks
- Example code and workflows
- Performance tuning recommendations

### Post-GA Benefits

**Extended Support (3 Months):**
- Free support normally valued at $2,000/month
- Priority issue resolution
- Dedicated success manager
- Migration assistance

**Strategic Partnership:**
- Case study development (optional)
- Co-marketing opportunities
- Speaking slot at AkiDB launch event
- Logo featured on akidb.io website

**Recognition:**
- Named in GA announcement (with permission)
- "Founding Design Partner" badge
- LinkedIn recommendations from AkiDB team
- Priority access to future beta programs

---

## Selection Criteria

### Ideal Design Partner Profile

We're looking for organizations that represent diverse use cases, scales, and deployment environments:

#### **Use Case Diversity**

**Partner Type 1: RAG (Retrieval-Augmented Generation)**
- Use case: Document Q&A, knowledge base search, chatbot context retrieval
- Scale: 10k-100k document chunks
- Requirements: Low latency (<50ms P95), high recall (>95%)

**Partner Type 2: Semantic Search**
- Use case: E-commerce product search, content discovery, similar item recommendations
- Scale: 50k-500k product/content embeddings
- Requirements: Sub-second response times, metadata filtering

**Partner Type 3: Recommendation System**
- Use case: Personalized recommendations, content matching, user similarity
- Scale: 100k-1M user/item embeddings
- Requirements: High throughput (>100 QPS), batch operations

**Partner Type 4: ARM Edge Deployment**
- Use case: On-device search, local inference, edge AI
- Platform: NVIDIA Jetson Orin, Mac ARM (M1/M2/M3), Oracle ARM Cloud
- Requirements: Low memory footprint, energy efficiency

**Partner Type 5: High-Throughput Scenario**
- Use case: Real-time analytics, fraud detection, log analysis
- Scale: 1M+ vectors, continuous ingestion
- Requirements: High write throughput, concurrent queries

#### **Technical Capabilities**

**Required:**
- Experience with Docker or Kubernetes deployment
- Ability to share anonymized metrics and logs
- Comfortable with REST/gRPC APIs
- Can dedicate 5-10 hours/week for 2 weeks

**Preferred:**
- Prior vector database experience (Pinecone, Milvus, Qdrant, Weaviate)
- Python or Rust development experience
- Monitoring infrastructure (Prometheus/Grafana)
- CI/CD pipelines for automated testing

#### **Scale Diversity**

We aim to test across different scales:

- **Small Scale (1 partner):** 1k-10k vectors, single-node deployment
- **Medium Scale (2 partners):** 10k-100k vectors, typical production workload
- **Large Scale (1-2 partners):** 100k-1M+ vectors, high-throughput scenarios

#### **Engagement Level**

**Commitment Requirements:**
- **Week 1:** 6-8 hours (deployment + initial testing)
- **Week 2:** 4-6 hours (production testing + feedback)
- **Daily:** 15-minute Slack check-in
- **Weekly:** 1-hour video call with engineering team

### Selection Process

**Step 1: Application Submission**
- Fill out design partner application form
- Provide use case details, scale estimates, and technical environment
- Estimated time: 15 minutes

**Step 2: Application Review**
- Engineering team reviews all applications
- Evaluate diversity, technical fit, and engagement potential
- Timeline: 48 hours

**Step 3: Interview**
- 30-minute video call to discuss:
  - Use case deep dive
  - Technical requirements
  - Success criteria
  - Commitment confirmation
- Timeline: Within 1 week of application

**Step 4: Selection & Onboarding**
- 3-5 partners selected based on diversity criteria
- 1-hour kickoff call covering:
  - Pilot timeline and expectations
  - Deployment walkthrough
  - Support channels and communication norms
  - Feedback collection process
- Timeline: Within 2 weeks of application

---

## Pilot Timeline

### Week 1: Deployment & Initial Testing

#### **Day 1: Kickoff & Deployment**

**Morning (2 hours):**
- Kickoff call with AkiDB engineering team
- Receive pilot deployment package
- Review deployment guide and support channels
- Set up communication (Slack, email)

**Afternoon (3 hours):**
- Deploy AkiDB RC1 to staging environment
- Run automated verification script
- Verify REST and gRPC endpoints
- Enable metrics collection
- Submit deployment confirmation

**Deliverables:**
- ✅ RC1 deployed and healthy
- ✅ Sample collection created
- ✅ Metrics collection configured
- ✅ Deployment report submitted

---

#### **Day 2: Initial Testing**

**Morning (2 hours):**
- Create first production collection
- Import sample data (100-1000 vectors)
- Test basic CRUD operations
- Verify search functionality
- Check metrics dashboard

**Afternoon (2 hours):**
- Test API workflows relevant to your use case
- Measure baseline performance (latency, throughput)
- Identify any immediate issues
- Submit Day 2 check-in

**Deliverables:**
- ✅ First collection operational
- ✅ Sample data imported successfully
- ✅ Baseline performance metrics
- ✅ Initial feedback submitted

---

#### **Day 3: Feature Exploration**

**Morning (2 hours):**
- Test metadata filtering (if applicable)
- Test batch operations
- Explore configuration options
- Review API documentation

**Afternoon (1 hour):**
- Experiment with different distance metrics
- Test collection management (create/delete)
- Document any API questions
- Submit Day 3 check-in

**Deliverables:**
- ✅ Core features tested
- ✅ API questions documented
- ✅ Feature feedback submitted

---

#### **Day 4: Integration Testing**

**Full Day (4 hours):**
- Integrate AkiDB with your application
- Test your actual workflows end-to-end
- Measure performance under realistic load
- Identify integration challenges
- Document any blockers

**Deliverables:**
- ✅ Integration with your app complete
- ✅ Real-world workflows tested
- ✅ Integration challenges documented
- ✅ Week 1 preliminary feedback submitted

---

#### **Day 5: Week 1 Review**

**Morning (2 hours):**
- Complete Week 1 feedback form
- Review metrics from past 5 days
- Prepare questions for weekly call
- Document progress and blockers

**Afternoon (1 hour):**
- Weekly sync call with AkiDB engineering team
- Discuss findings, issues, and next steps
- Align on Week 2 testing plan
- Address any technical blockers

**Deliverables:**
- ✅ Week 1 feedback form submitted
- ✅ Weekly sync call completed
- ✅ Week 2 plan confirmed

---

### Week 2: Production Testing & Feedback

#### **Day 6-7: Production Load Testing**

**Both Days (3 hours each):**
- Scale up to production data volume
- Run sustained load tests
- Monitor performance and stability
- Track memory usage and resource consumption
- Log any degradation or issues

**Deliverables:**
- ✅ Production data volume imported
- ✅ Performance under load measured
- ✅ Stability monitoring data collected
- ✅ Daily check-ins submitted

---

#### **Day 8: Stress Testing**

**Full Day (4 hours):**
- Run peak load scenarios
- Test concurrent operations
- Test edge cases (large vectors, high QPS)
- Verify graceful degradation
- Document breaking points

**Deliverables:**
- ✅ Stress test results documented
- ✅ Peak capacity identified
- ✅ Edge cases tested
- ✅ Performance limits defined

---

#### **Day 9: Comprehensive Feedback**

**Full Day (4 hours):**
- Complete detailed feedback survey
- Analyze all metrics collected
- Document all issues encountered
- Prepare recommendations for RC2
- List missing features and improvements

**Deliverables:**
- ✅ Comprehensive feedback survey submitted
- ✅ All issues documented with severity
- ✅ RC2 recommendations provided
- ✅ Feature requests prioritized

---

#### **Day 10: Exit Interview & Decision**

**Morning (1 hour):**
- 1-hour exit interview with AkiDB team
- Discuss overall experience
- Deep dive on critical findings
- Review roadmap and feature priorities
- Make commitment decision (continue to GA or not)

**Afternoon (2 hours):**
- Finalize feedback documentation
- Submit any remaining questions
- Provide testimonial (optional)
- Confirm GA participation decision

**Deliverables:**
- ✅ Exit interview completed
- ✅ Final feedback submitted
- ✅ GA commitment decision confirmed
- ✅ Testimonial provided (optional)

---

## Deployment Package

Each design partner receives a comprehensive **Pilot Deployment Kit** containing everything needed for a successful pilot:

### Package Contents

```
pilot-kit-${PARTNER_ID}/
├── README.md                     # Quick start guide
├── docker-compose.yaml           # Pre-configured Docker deployment
├── config.toml                   # Optimized configuration template
├── .env.template                 # Environment variables
├── scripts/
│   ├── deploy.sh                # Automated deployment (all platforms)
│   ├── backup-akidb.sh          # Backup script
│   ├── restore-akidb.sh         # Restore script
│   ├── health-check.sh          # Health monitoring
│   ├── collect-metrics.sh       # Metrics collection
│   └── smoke-test.sh            # Smoke testing
├── k8s/
│   ├── namespace.yaml           # Kubernetes namespace
│   ├── configmap.yaml           # Configuration
│   ├── deployment-rest.yaml     # REST server deployment
│   ├── deployment-grpc.yaml     # gRPC server deployment
│   ├── service-rest.yaml        # REST service
│   ├── service-grpc.yaml        # gRPC service
│   ├── persistentvolume.yaml    # Storage
│   └── ingress.yaml             # Ingress controller
├── docs/
│   ├── QUICKSTART.md            # Getting started (10 minutes)
│   ├── DEPLOYMENT-GUIDE.md      # Full deployment guide
│   ├── API-TUTORIAL.md          # API usage examples
│   ├── TROUBLESHOOTING.md       # Common issues and solutions
│   ├── PERFORMANCE-TUNING.md    # Optimization tips
│   └── FEEDBACK-TEMPLATE.md     # Feedback collection template
├── examples/
│   ├── import-data.py           # Python data import script
│   ├── basic-workflow.py        # Common operations
│   ├── batch-operations.py      # Batch insert/search
│   ├── integration-example.py   # Integration patterns
│   └── sample-data/
│       ├── vectors-1k.json      # Sample 1k vectors
│       ├── vectors-10k.json     # Sample 10k vectors
│       └── metadata-example.json # Metadata examples
└── monitoring/
    ├── grafana-dashboard.json   # Grafana dashboard
    ├── prometheus-config.yaml   # Prometheus configuration
    └── alerts.yaml              # Alert rules
```

### Deployment Options

The pilot kit supports three deployment methods:

**Option 1: Docker (Recommended for Quick Start)**
```bash
cd pilot-kit-${PARTNER_ID}
./scripts/deploy.sh docker
# Services start on localhost:8080 (REST) and localhost:9090 (gRPC)
```

**Option 2: Kubernetes (Recommended for Production Testing)**
```bash
cd pilot-kit-${PARTNER_ID}
./scripts/deploy.sh kubernetes
# Configurable namespace, ingress, and storage
```

**Option 3: Binary (Bare Metal/ARM Edge)**
```bash
cd pilot-kit-${PARTNER_ID}
./scripts/deploy.sh binary
# Runs as systemd service or standalone process
```

### Configuration Template

The `config.toml` is pre-configured with pilot-optimized settings:

```toml
# AkiDB Configuration - Pilot Partner
# Partner ID: ${PARTNER_ID}

[server]
host = "0.0.0.0"
rest_port = 8080
grpc_port = 9090

[database]
path = "./data/akidb.db"
max_connections = 10

[features]
enable_persistence = true
enable_metrics = true

[hnsw]
m = 32                    # Balanced for most use cases
ef_construction = 200     # Good recall/build time trade-off
ef_search = 100           # Fast search with >95% recall

[logging]
level = "info"            # Change to "debug" for troubleshooting
format = "json"           # Structured logging
output = "file"
file_path = "./logs/akidb.log"

[metrics]
enabled = true
path = "/metrics"

[pilot]
partner_id = "${PARTNER_ID}"
telemetry_enabled = true  # Opt-in metrics sharing
telemetry_endpoint = "https://telemetry.akidb.io/pilot"
```

---

## Feedback Collection

### Daily Check-ins (Async via Slack)

Post a brief update in `#pilot-design-partners` each day:

```
📅 Day X Update - ${PARTNER_NAME}

🚦 Status: [🟢 Green / 🟡 Yellow / 🔴 Red]
- Green: All systems operational
- Yellow: Minor issues, workarounds available
- Red: Blocked, need immediate assistance

💡 Today's Focus:
- [What you tested/worked on]

❓ Issues Encountered:
- [List issues or "None"]

📊 Metrics:
- Vectors: [count]
- Query P95 latency: [Xms]
- Memory usage: [XGB]
- Dashboard: [link if available]

❓ Questions:
- [List questions or "None"]

📈 Next:
- [Tomorrow's plan]
```

### Weekly Feedback Form

Complete at the end of Week 1 and Week 2. See [FEEDBACK-COLLECTION.md](./FEEDBACK-COLLECTION.md) for the full template.

**Key Sections:**
1. Deployment Experience (ease, time, blockers)
2. Functionality (features tested, what worked, what didn't)
3. Performance (latency, throughput, memory, stability)
4. API/Developer Experience (ease of use, docs quality, error messages)
5. Production Readiness (would you deploy? what's blocking?)
6. Open Feedback (likes, improvements, surprises)

### Exit Interview

At the end of Week 2, a 1-hour video call covering:

**Discussion Topics:**
- Overall experience summary (5 minutes)
- Deep dive on critical issues (20 minutes)
- Feature priority discussion (15 minutes)
- Roadmap alignment (10 minutes)
- GA commitment decision (5 minutes)
- Testimonial/case study discussion (5 minutes)

**Outcomes:**
- Clear list of P0/P1 issues for RC2
- Feature priority ranking
- Commitment to GA testing (yes/no/maybe)
- Optional testimonial/case study agreement

---

## Success Metrics

### Program Success Criteria

**Deployment Success:**
- ✅ 80%+ partners deploy successfully within 48 hours
- ✅ 100% partners reach functional deployment by Day 5
- ✅ Average deployment time < 1 hour (Docker path)

**Usage Success:**
- ✅ 100% partners test core CRUD workflows
- ✅ 80%+ partners test advanced features
- ✅ 60%+ partners import production-scale data
- ✅ 40%+ partners test at production load

**Feedback Success:**
- ✅ 100% partners submit daily check-ins
- ✅ 100% partners submit weekly feedback forms
- ✅ 80%+ partners complete exit interview
- ✅ Average satisfaction score ≥ 4/5

**Bug Discovery:**
- ✅ Identify 5-10 real-world bugs
- ✅ All P0 bugs fixed within 24 hours
- ✅ All P1 bugs fixed before RC2 release

**Outcome Metrics:**
- ✅ 60%+ partners commit to GA testing
- ✅ RC2 roadmap clear with prioritized improvements
- ✅ At least 2 partners willing to provide testimonials
- ✅ Zero partners report data loss or corruption

---

## Support & Communication

### Primary Channels

**Slack: #pilot-design-partners**
- Real-time questions and updates
- Daily check-ins
- Quick troubleshooting
- Response time: <4 hours during business hours

**Email: pilot@akidb.io**
- Formal feedback submission
- Sensitive issues
- Billing/legal questions
- Response time: <24 hours

**Weekly Office Hours**
- When: Fridays 10:00-11:00 AM PT
- Where: Zoom (link in Slack channel)
- Format: Open Q&A, live troubleshooting

**Emergency Contact**
- Critical P0 issues only (data loss, security)
- Phone: [Provided in pilot kit]
- Available 24/7

### Response Time SLAs

| Priority | Response Time | Resolution Time |
|----------|---------------|-----------------|
| P0 (Critical) | <2 hours | <24 hours |
| P1 (High) | <4 hours | <3 days |
| P2 (Medium) | <24 hours | Best effort |
| P3 (Low) | <48 hours | Backlog |

**Priority Definitions:**
- **P0:** Data loss, security issue, complete service outage
- **P1:** Major feature broken, performance degradation >50%
- **P2:** Minor feature issue, workaround available
- **P3:** Feature request, documentation issue, cosmetic bug

---

## Application Process

### How to Apply

**Step 1: Submit Application**

Visit: **https://forms.akidb.io/pilot-application**

Required Information:
- Organization name and website
- Primary contact (name, email, role)
- Use case description (2-3 paragraphs)
- Expected data scale (vector count, dimensions)
- Deployment environment (Docker/K8s/Edge)
- Current vector database solution (if any)
- Technical team size and expertise
- Commitment confirmation (5-10 hours/week)

**Step 2: Wait for Review** (48 hours)

Our engineering team will review your application based on:
- Use case diversity (vs. other applicants)
- Technical readiness
- Scale and workload characteristics
- Engagement commitment

**Step 3: Interview** (if selected)

30-minute video call to discuss:
- Use case deep dive
- Technical requirements and environment
- Success criteria and evaluation metrics
- Timeline and commitment confirmation

**Step 4: Onboarding** (if accepted)

- Receive pilot deployment kit
- Join #pilot-design-partners Slack
- Attend 1-hour kickoff call
- Begin Day 1 deployment

### Application Deadline

Applications accepted on a **rolling basis** until 5 partners are selected.

**Current Status:** 🟢 OPEN - 0/5 slots filled

---

## FAQ

### General

**Q: What is the time commitment?**
A: 5-10 hours/week for 2 weeks (10-20 hours total). Week 1 is heavier (6-8 hours), Week 2 is lighter (4-6 hours).

**Q: Is this free?**
A: Yes! The pilot is completely free, including post-GA support for 3 months.

**Q: What if I encounter a blocking issue?**
A: We provide <24 hour resolution for critical issues and <4 hour response times during business hours.

**Q: Can I continue using AkiDB after the pilot?**
A: Absolutely! You'll receive 3 months of free support and be first in line for the GA release.

### Technical

**Q: What platforms are supported?**
A: Docker (any platform), Kubernetes (any cluster), bare metal (Linux/macOS ARM, x86_64).

**Q: What programming languages can I use?**
A: Any language with HTTP/gRPC support. We provide Python examples and will support other languages based on demand.

**Q: How do I migrate from my current vector database?**
A: We'll provide migration guides for Pinecone, Milvus, Qdrant, and Weaviate during the pilot.

**Q: What are the resource requirements?**
A: Minimum: 2 CPU cores, 4GB RAM, 10GB disk. Recommended: 4+ cores, 8GB+ RAM, 50GB+ disk.

### Data & Privacy

**Q: Is my data safe?**
A: Yes. All data stays in your environment. Optional telemetry is anonymized and aggregated.

**Q: Can I opt out of telemetry?**
A: Yes. Set `telemetry_enabled = false` in config.toml. We only ask that you share anonymized performance metrics manually.

**Q: What happens to my data after the pilot?**
A: Your data remains yours. You can continue using AkiDB, migrate to another solution, or delete everything.

### Program

**Q: What if I can't complete the 2-week commitment?**
A: Let us know ASAP. We can extend your timeline or pause your participation.

**Q: Can I provide feedback anonymously?**
A: Yes, though we encourage transparency for better collaboration.

**Q: Will my feedback be shared publicly?**
A: Only aggregated, anonymized insights. Individual feedback remains confidential unless you approve sharing.

---

## Contact

**Questions about the pilot program?**

- Email: pilot@akidb.io
- Website: https://akidb.io/pilot
- Slack: Request invite at pilot@akidb.io

**Apply now:** https://forms.akidb.io/pilot-application

---

**Document Version:** 1.0
**Last Updated:** November 7, 2025
**Status:** Active - Accepting Applications
