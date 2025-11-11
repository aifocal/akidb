# AkiDB Pilot Feedback Collection Guide

**Purpose:** Structured feedback collection from design partners during the RC1 pilot program
**Duration:** 2 weeks (Week 1 + Week 2)
**Submission Methods:** Google Forms (primary), Email (fallback), Slack (informal)

---

## Table of Contents

1. [Daily Check-in Template](#daily-check-in-template)
2. [Weekly Feedback Form](#weekly-feedback-form)
3. [Exit Interview Guide](#exit-interview-guide)
4. [Submission Guidelines](#submission-guidelines)
5. [Feedback Analysis Process](#feedback-analysis-process)

---

## Daily Check-in Template

### Purpose
Quick async updates to track progress, identify blockers, and maintain communication.

### Format
Post daily in `#pilot-design-partners` Slack channel using this template:

```markdown
📅 **Day [X] Update - [Partner Name]**

🚦 **Status:** [🟢 Green / 🟡 Yellow / 🔴 Red]

💡 **Today's Focus:**
- [What you tested/worked on today]
- [Key activities completed]

✅ **Completed:**
- [List of completed tasks]

❓ **Issues Encountered:**
- [Issue 1: Description and severity (P0/P1/P2/P3)]
- [Issue 2: Description and severity]
- OR "None"

📊 **Key Metrics:**
- Total vectors: [count]
- Collections: [count]
- Query P95 latency: [X ms]
- Insert throughput: [X vectors/sec]
- Memory usage: [X GB]
- Error rate: [X%]
- Dashboard link: [URL if available]

❓ **Questions for Engineering Team:**
- [Question 1]
- [Question 2]
- OR "None"

📈 **Tomorrow's Plan:**
- [What you'll test/work on tomorrow]

---
Submitted: [Date/Time]
```

### Example

```markdown
📅 **Day 3 Update - Acme Corp**

🚦 **Status:** 🟢 Green

💡 **Today's Focus:**
- Tested batch insert operations with 10k vectors
- Explored metadata filtering capabilities
- Integrated AkiDB with our RAG pipeline

✅ **Completed:**
- Successfully imported 25k document embeddings (384-dim)
- Created 3 collections for different content types
- Ran 500+ search queries to validate accuracy

❓ **Issues Encountered:**
- P2: Metadata filtering on nested JSON feels clunky (minor)
- P3: API docs unclear on max batch size limit

📊 **Key Metrics:**
- Total vectors: 25,127
- Collections: 3
- Query P95 latency: 12.3 ms
- Insert throughput: 850 vectors/sec
- Memory usage: 2.1 GB
- Error rate: 0.1%
- Dashboard: https://grafana.acme.internal/akidb

❓ **Questions for Engineering Team:**
- What's the recommended batch size for inserts? Currently using 100.
- Can we use regex in metadata filters, or exact match only?

📈 **Tomorrow's Plan:**
- Test concurrent query load (simulate 50 QPS)
- Experiment with different HNSW parameters
- Measure recall accuracy vs. our current solution (Pinecone)

---
Submitted: 2025-11-10 17:45 UTC
```

---

## Weekly Feedback Form

### Purpose
Comprehensive structured feedback at the end of Week 1 and Week 2.

### Submission
- **Week 1 Feedback:** Due Friday, Day 5, before weekly sync call
- **Week 2 Feedback:** Due Wednesday, Day 9, before exit interview
- **Method:** Google Form (link: https://forms.akidb.io/pilot-weekly-feedback)

---

## Week 1 & Week 2 Feedback Form Structure

### Section 1: Deployment Experience

**1.1 How difficult was the deployment process?**
- ⭐ 1 - Very difficult (many blockers)
- ⭐ 2 - Difficult (several blockers)
- ⭐ 3 - Moderate (some blockers)
- ⭐ 4 - Easy (minor issues)
- ⭐ 5 - Very easy (smooth)

**1.2 Which deployment method did you use?**
- [ ] Docker (docker-compose)
- [ ] Kubernetes
- [ ] Binary (bare metal)
- [ ] Other: ___________

**1.3 How long did it take to get AkiDB running successfully?**
- [ ] < 30 minutes
- [ ] 30-60 minutes
- [ ] 1-2 hours
- [ ] 2-4 hours
- [ ] > 4 hours
- [ ] Still not fully deployed

**1.4 Were the deployment docs clear and helpful?**
- ⭐ 1 - Very unclear
- ⭐ 2 - Somewhat unclear
- ⭐ 3 - Acceptable
- ⭐ 4 - Clear
- ⭐ 5 - Very clear

**1.5 Did you encounter any deployment blockers?**
- [ ] Yes (please describe below)
- [ ] No

**1.6 If yes, describe deployment blockers:**
```
[Free text response]

Please include:
- What went wrong
- Error messages (if any)
- How you resolved it (or if still blocked)
- Suggestions for improvement
```

---

### Section 2: Functionality

**2.1 Which core features did you test?** (Check all that apply)
- [ ] Create collection
- [ ] Delete collection
- [ ] List collections
- [ ] Insert single vector
- [ ] Insert batch vectors
- [ ] Search/query vectors
- [ ] Get vector by ID
- [ ] Delete vector
- [ ] Metadata filtering
- [ ] Different distance metrics (Cosine, L2, Dot)
- [ ] Server health check
- [ ] Metrics endpoint

**2.2 Which features worked well?** (Check all that apply and add notes)
- [ ] Collection management: ___________
- [ ] Vector insert: ___________
- [ ] Vector search: ___________
- [ ] Metadata filtering: ___________
- [ ] Performance: ___________
- [ ] API design: ___________
- [ ] Error handling: ___________
- [ ] Other: ___________

**2.3 Which features had issues or didn't work as expected?**
```
[Free text response]

For each issue, please provide:
- Feature name
- What you expected
- What actually happened
- Severity (P0/P1/P2/P3)
- Workaround (if found)
```

**2.4 What features are missing that you need?**
```
[Free text response]

For each missing feature:
- Feature description
- Your use case
- Priority (Critical / High / Medium / Low)
- Can you launch without it? (Yes / No / Maybe)
```

---

### Section 3: Performance

**3.1 What is your dataset size?**
- Vector count: ___________
- Vector dimension: ___________
- Metadata per vector: [ ] None [ ] Small (<1KB) [ ] Medium (1-10KB) [ ] Large (>10KB)
- Total dataset size: ___________

**3.2 Search Query Performance**
- P50 latency: __________ ms
- P95 latency: __________ ms
- P99 latency: __________ ms
- Query load tested: __________ QPS
- Did performance meet your requirements? [ ] Yes [ ] No [ ] Mostly

**3.3 Insert/Update Performance**
- Single insert: __________ ms/vector
- Batch insert: __________ vectors/sec (batch size: ______)
- Did performance meet your requirements? [ ] Yes [ ] No [ ] Mostly

**3.4 Resource Usage**
- Memory usage (steady state): __________ GB
- Memory usage (peak): __________ GB
- Disk usage: __________ GB
- CPU usage (average): __________ %
- Were resource requirements acceptable? [ ] Yes [ ] No [ ] Mostly

**3.5 Did you encounter any performance issues?**
```
[Free text response]

Please describe:
- Type of issue (latency spike, memory leak, slow startup, etc.)
- When it occurred (load level, time of day, etc.)
- Impact (minor slowdown, complete degradation, crash)
- Reproducibility (always, sometimes, once)
```

---

### Section 4: API & Developer Experience

**4.1 How would you rate the API ease of use?**
- ⭐ 1 - Very difficult to use
- ⭐ 2 - Difficult
- ⭐ 3 - Acceptable
- ⭐ 4 - Easy
- ⭐ 5 - Very easy

**4.2 How would you rate the API documentation quality?**
- ⭐ 1 - Very poor
- ⭐ 2 - Poor
- ⭐ 3 - Acceptable
- ⭐ 4 - Good
- ⭐ 5 - Excellent

**4.3 Were error messages helpful and actionable?**
- ⭐ 1 - Not helpful at all
- ⭐ 2 - Rarely helpful
- ⭐ 3 - Sometimes helpful
- ⭐ 4 - Usually helpful
- ⭐ 5 - Always helpful

**4.4 Please provide example error messages (good and bad):**
```
Good examples (helpful error messages):
[Paste error message and explain why it was helpful]

Bad examples (unhelpful error messages):
[Paste error message and explain what was confusing]
```

**4.5 Which programming language(s) are you using?**
- [ ] Python
- [ ] JavaScript/Node.js
- [ ] Go
- [ ] Rust
- [ ] Java
- [ ] C#
- [ ] Other: ___________

**4.6 Would you like official client libraries?**
- [ ] Yes, essential for adoption
- [ ] Yes, nice to have
- [ ] No, REST/gRPC is sufficient
- Preferred language(s): ___________

---

### Section 5: Production Readiness

**5.1 Based on your testing, would you deploy AkiDB RC1 to production today?**
- [ ] Yes, ready now
- [ ] Probably, with minor fixes
- [ ] Maybe, with significant improvements
- [ ] No, too many issues
- [ ] Unsure, need more testing

**5.2 What is blocking production deployment?** (Check all that apply)
- [ ] Critical bugs (P0)
- [ ] Performance issues
- [ ] Missing features
- [ ] Stability concerns
- [ ] Documentation gaps
- [ ] Security concerns
- [ ] Operational concerns (monitoring, backup, etc.)
- [ ] Nothing blocking
- [ ] Other: ___________

**5.3 What are the CRITICAL features/fixes needed for production?**
```
[Free text response]

List in priority order:
1. [Feature/fix description and why it's critical]
2. [Feature/fix description and why it's critical]
3. ...
```

**5.4 What are nice-to-have improvements (not blocking)?**
```
[Free text response]

List features that would improve experience but aren't blockers:
- [Improvement 1]
- [Improvement 2]
- ...
```

**5.5 How does AkiDB compare to your current solution (if applicable)?**
```
Current solution: ___________

Better:
- [What AkiDB does better]

Worse:
- [What AkiDB does worse]

Similar:
- [What's comparable]
```

---

### Section 6: Open Feedback

**6.1 What did you like most about AkiDB?**
```
[Free text response]

Tell us what impressed you or exceeded expectations.
```

**6.2 What needs the most improvement?**
```
[Free text response]

Tell us the biggest pain point or area needing work.
```

**6.3 Did you encounter any unexpected behaviors or surprises?**
```
[Free text response]

Good surprises:
- [Pleasant unexpected features/behaviors]

Bad surprises:
- [Confusing or problematic unexpected behaviors]
```

**6.4 Any other comments, questions, or feedback?**
```
[Free text response]

Anything else you want to share with the team.
```

---

## Exit Interview Guide

### Purpose
Deep-dive discussion at the end of the pilot to gather final insights and make GA commitment decisions.

### Format
- **Duration:** 60 minutes
- **Participants:** Design partner lead(s) + AkiDB engineering team
- **Method:** Zoom video call
- **Recording:** With permission (for internal review only)

---

### Interview Structure

#### Part 1: Overall Experience (10 minutes)

**Questions:**
1. **How would you describe your overall pilot experience?**
   - What went well?
   - What was challenging?
   - Did the pilot meet your expectations?

2. **How responsive and helpful was the AkiDB team?**
   - Support quality rating (1-5)
   - Communication frequency (too much / just right / too little)
   - Any suggestions for improvement?

3. **Would you recommend AkiDB to colleagues?** (NPS-style)
   - 0-6: No, 7-8: Maybe, 9-10: Yes
   - Why or why not?

---

#### Part 2: Technical Deep Dive (25 minutes)

**Critical Issues:**
1. **Walk us through the most critical issues you encountered.**
   - Issue description and impact
   - When/how discovered
   - Workaround used (if any)
   - Suggested fix/improvement

2. **Were there any show-stoppers or dealbreakers?**
   - Issues that would prevent production deployment
   - Severity and urgency

**Performance:**
3. **How did performance compare to your expectations?**
   - Latency (better/worse/as expected)
   - Throughput (better/worse/as expected)
   - Resource usage (better/worse/as expected)
   - Stability (better/worse/as expected)

4. **Did you do any comparative benchmarking?**
   - vs. current solution (if applicable)
   - Results and insights

**Functionality:**
5. **Which features did you use most?**
   - Most valuable features
   - Least valuable features (if any)

6. **Which features are absolutely critical for your use case?**
   - Must-haves for GA
   - Nice-to-haves for future

---

#### Part 3: Feature Priorities (10 minutes)

**Roadmap Discussion:**
1. **From the issues/features you mentioned, which are TOP 3 priorities for RC2?**
   - Priority 1: ___________
   - Priority 2: ___________
   - Priority 3: ___________

2. **Are there any features on our roadmap you're excited about?**
   - Future features mentioned in docs or calls
   - New ideas from your experience

3. **What would make AkiDB a "must-have" vs. "nice-to-have" for your team?**
   - Differentiators vs. competitors
   - Unique value propositions

---

#### Part 4: Production Readiness & Commitment (10 minutes)

**Deployment Decision:**
1. **Would you deploy RC2 to production (assuming P0/P1 fixes)?**
   - [ ] Yes, definitely
   - [ ] Probably
   - [ ] Maybe (needs discussion)
   - [ ] Unlikely
   - [ ] No

2. **What's your timeline for potential production deployment?**
   - Immediately after GA
   - Within 1 month of GA
   - Within 3 months of GA
   - Within 6 months of GA
   - Unsure / No timeline

3. **Would you be willing to participate in GA testing?**
   - [ ] Yes, committed
   - [ ] Probably, pending internal approval
   - [ ] Maybe, need to see RC2 first
   - [ ] No

**Case Study & Testimonial:**
4. **Would you be interested in a case study or testimonial?**
   - [ ] Yes, public case study
   - [ ] Yes, private case study (anonymized)
   - [ ] Yes, written testimonial only
   - [ ] Yes, video testimonial
   - [ ] No, but okay with logo on website
   - [ ] No, prefer to stay private

---

#### Part 5: Wrap-up & Next Steps (5 minutes)

**Final Questions:**
1. **Anything else you'd like to share that we haven't covered?**

2. **Any questions for us?**

**Next Steps:**
- Confirm GA participation status
- Schedule follow-up call for RC2 launch (if participating)
- Discuss case study/testimonial logistics (if interested)
- Thank you + next steps

---

## Submission Guidelines

### Daily Check-ins
- **Frequency:** Every working day during pilot
- **Method:** Slack (#pilot-design-partners channel)
- **Time:** End of your working day (flexible)
- **Required:** Yes (helps us track progress and identify issues early)

### Weekly Feedback Forms
- **Frequency:** End of Week 1 (Day 5) and Week 2 (Day 9)
- **Method:** Google Form (link: https://forms.akidb.io/pilot-weekly-feedback)
- **Time:** Before weekly sync call (Week 1) or exit interview (Week 2)
- **Required:** Yes (critical for structured feedback)

### Exit Interview
- **Frequency:** Once (end of pilot, Day 10)
- **Method:** Zoom video call (scheduled in advance)
- **Duration:** 60 minutes
- **Required:** Strongly encouraged (80%+ participation target)

### Ad-hoc Feedback
- **Method:** Slack, email (pilot@akidb.io), or GitHub issues
- **Encouraged:** Anytime you have insights, ideas, or issues
- **Not required:** But highly valued!

---

## Feedback Analysis Process

### How We Use Your Feedback

**Daily Analysis (During Pilot):**
- Engineering team reviews daily check-ins every morning
- Triage P0/P1 issues immediately
- Respond to questions and blockers
- Track metrics and trends

**Weekly Analysis (Friday Review):**
- Aggregate weekly feedback forms
- Identify common themes and patterns
- Prioritize bug fixes for RC2
- Update roadmap based on needs

**Post-Pilot Analysis (After Week 2):**
- Compile comprehensive feedback report
- Analyze all metrics and performance data
- Create prioritized RC2 backlog
- Publish anonymized insights document

### Feedback Report Structure

At the end of the pilot, we'll create a comprehensive feedback analysis report:

**Contents:**
1. **Executive Summary**
   - Pilot overview and participation
   - Key findings (top 3-5 insights)
   - Critical issues discovered
   - Overall satisfaction and NPS scores

2. **Partner Profiles**
   - Anonymous partner summaries
   - Use cases, scales, and deployment types
   - Success stories and challenges

3. **Feedback by Category**
   - Deployment experience
   - Functionality and features
   - Performance and stability
   - API/developer experience
   - Production readiness

4. **Issues & Bugs**
   - P0/P1/P2/P3 breakdown
   - Status (fixed, in progress, backlog)
   - Common patterns and root causes

5. **RC2 Roadmap**
   - Must-have fixes (from feedback)
   - Performance improvements
   - Feature additions
   - Documentation updates

6. **GA Readiness**
   - Partner commitment status
   - Timeline to GA
   - Remaining work
   - Risk assessment

### Feedback Privacy

**What we share publicly:**
- Aggregated, anonymized insights
- Overall satisfaction scores (without attribution)
- Common themes and patterns
- Bug fixes and improvements

**What stays private:**
- Individual partner feedback (unless you approve sharing)
- Specific performance metrics (unless you approve sharing)
- Critical issues (until fixed)
- Competitive information

**Your control:**
- You can request removal of any feedback
- You can approve/deny inclusion in case studies
- You control use of your logo and testimonials

---

## Contact

**Questions about feedback submission?**
- Slack: #pilot-design-partners
- Email: pilot@akidb.io

**Need help with forms or interviews?**
- Contact: pilot@akidb.io
- Response time: <24 hours

---

**Document Version:** 1.0
**Last Updated:** November 7, 2025
**Status:** Active
