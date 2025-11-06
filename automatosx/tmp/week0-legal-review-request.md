# Legal Review Request: Qwen3-Embedding-8B Licensing

**To:** Legal Department, Open Source Compliance Team
**From:** Product Lead, AkiDB 2.0
**Date:** 2025-11-06
**Subject:** Urgent License Review - Qwen3-Embedding-8B Embedding Model
**Priority:** HIGH
**Decision Required By:** 2025-11-13 (Friday COB)

---

## Executive Summary

AkiDB 2.0 requires an embedding model for its built-in embedding service. Our primary candidate is **Qwen3-Embedding-8B** from Alibaba Cloud. We need legal clearance for commercial redistribution on ARM edge devices (Mac ARM, NVIDIA Jetson, Oracle ARM Cloud) before proceeding with Week 1 development.

**Critical Path Impact:** This is a Go/No-Go blocker for Phase 2 (Embedding Service Integration, Weeks 5-8). Without approval by Nov 13, we must pivot to fallback alternatives, adding 2-week delay.

---

## Model Details

### Qwen3-Embedding-8B Overview

- **Provider:** Alibaba Cloud (Tongyi Qianwen Team)
- **Model Type:** Text embedding model for semantic search
- **Size:** 8 billion parameters (quantized to int8: ~2GB, fp16: ~4GB)
- **License:** Apache 2.0 (as stated in model card)
- **Repository:** https://huggingface.co/Alibaba-NLP/gte-Qwen2-7B-instruct
- **Intended Use:** General-purpose text embeddings for RAG, semantic search, clustering

### Our Intended Use Case

1. **Deployment Context:**
   - Embedded within AkiDB 2.0 as built-in embedding service
   - Runs locally on customer edge devices (offline-first architecture)
   - No data sent to external APIs or Alibaba Cloud services

2. **Distribution Model:**
   - AkiDB 2.0 packages will include quantized model weights
   - Customers download complete bundle (AkiDB binary + model weights)
   - Deployed on Mac ARM (Apple Silicon), NVIDIA Jetson, Oracle ARM Cloud

3. **Commercial Terms:**
   - AkiDB 2.0 is proprietary software (not open source)
   - Customers pay for AkiDB licenses ($50k-$100k ARR per enterprise)
   - Model weights bundled as part of licensed product

---

## Key Legal Questions

### 1. Commercial Redistribution Rights

**Question:** Does Apache 2.0 license permit us to redistribute Qwen3-Embedding-8B model weights as part of a proprietary commercial product?

**Context:**
- Apache 2.0 typically allows commercial use and redistribution
- Need confirmation that model weights fall under Apache 2.0 (not just inference code)
- Model card states Apache 2.0, but we need verification from official Alibaba Cloud license documentation

**Required Deliverable:** Written confirmation that redistribution is permitted under Apache 2.0 for model weights (not just code).

### 2. Attribution Requirements

**Question:** What attribution and notice requirements must we comply with?

**Context:**
- Apache 2.0 requires preservation of copyright notices
- Need clarity on:
  - Where to place attribution (documentation, UI, LICENSE file?)
  - Exact wording required
  - Whether we must disclose modifications (e.g., quantization to int8)

**Required Deliverable:** Template attribution text and placement guidelines.

### 3. Modification and Derivative Works

**Question:** Are we permitted to quantize the model (fp16 → int8) and redistribute the quantized version?

**Context:**
- Our use case requires quantization to reduce memory footprint (8GB → 2GB)
- Quantization changes model weights (derivative work?)
- Apache 2.0 generally allows modifications, but need confirmation for model weights

**Required Deliverable:** Confirmation that quantization is permitted without additional restrictions.

### 4. Geographic and Use Restrictions

**Question:** Are there any geographic restrictions (e.g., US export controls, China data residency) or prohibited use cases?

**Context:**
- Model developed by Chinese company (Alibaba Cloud)
- Our customers span US, EU, APAC
- Use cases include healthcare (HIPAA), financial services (SOC 2), government (FedRAMP potential)

**Required Deliverable:** Confirmation of any geographic or industry-specific restrictions.

### 5. Sublicensing and End-User Licensing

**Question:** Can we sublicense the model to our enterprise customers as part of AkiDB EULA?

**Context:**
- Our EULA grants customers right to use AkiDB (including bundled embeddings)
- Need to ensure Apache 2.0 permits sublicensing in this manner
- Customers deploy on their own infrastructure (not SaaS)

**Required Deliverable:** Approval of sublicensing approach or required modifications to EULA.

### 6. Warranty and Liability

**Question:** What warranty disclaimers and liability limitations must we include?

**Context:**
- Apache 2.0 includes "AS IS" warranty disclaimer
- Need to assess:
  - Whether Alibaba Cloud provides any warranties
  - Our liability exposure if model produces biased/harmful outputs
  - Required indemnification language in customer contracts

**Required Deliverable:** Risk assessment and recommended contract language.

### 7. Model Card and Documentation Compliance

**Question:** Are we required to distribute Qwen3-Embedding-8B model card and documentation alongside the model?

**Context:**
- Model card includes:
  - Training data details
  - Evaluation benchmarks
  - Intended use and limitations
  - Bias and fairness disclosures
- Unclear if Apache 2.0 requires distributing these artifacts

**Required Deliverable:** Guidance on documentation obligations.

---

## Fallback Alternatives (If Qwen3-Embedding-8B Blocked)

If legal review identifies blocking issues, we have pre-vetted alternatives:

### Option 1: Embedding-Gemma (Google)
- **License:** Gemma Terms of Use (permissive commercial use)
- **Size:** 2B parameters (smaller, faster)
- **Pros:** Google backing, clear commercial licensing, multilingual
- **Cons:** Lower performance than Qwen3 on Chinese text, less mature
- **Decision Timeline:** +1 week for legal review, +1 week for benchmarking

### Option 2: Voyage-large (Voyage AI)
- **License:** Commercial API license
- **Size:** Proprietary model (API-only, no local deployment)
- **Pros:** Best-in-class performance, clear commercial terms
- **Cons:** Requires internet connectivity (breaks offline-first architecture), ongoing API costs
- **Decision Timeline:** Immediate (SaaS, no redistribution concerns)

### Option 3: all-MiniLM-L6-v2 (Sentence-Transformers)
- **License:** Apache 2.0 (well-established)
- **Size:** 22M parameters (extremely lightweight)
- **Pros:** Proven licensing, minimal resource footprint, fast inference
- **Cons:** Lower accuracy than Qwen3, English-only, older architecture
- **Decision Timeline:** Immediate (no legal review needed)

**Recommendation:** If Qwen3-Embedding-8B blocked, pivot to **Embedding-Gemma** (best balance of performance and licensing clarity).

---

## Timeline and Approval Workflow

### Critical Dates

| Date | Milestone | Owner |
|------|-----------|-------|
| Nov 6 (Today) | Legal review request submitted | Product Lead |
| Nov 8 | Initial legal triage, assign attorney | Legal Ops |
| Nov 11 | Draft legal opinion circulated | Assigned Attorney |
| Nov 12 | Internal review with Engineering + Product | Legal, Product, Engineering |
| Nov 13 (DEADLINE) | Final Go/No-Go decision | Legal Department |
| Nov 14 | If approved: Proceed with Phase 2 planning | Engineering |
| Nov 14 | If blocked: Initiate fallback model evaluation | ML Engineering |

### Escalation Path

- **Primary Contact:** [Legal Department Manager]
- **Escalation 1:** VP Legal (if decision delayed beyond Nov 10)
- **Escalation 2:** General Counsel (if fundamental licensing conflict identified)

### Decision Criteria

**GREEN (Approved):** Proceed with Qwen3-Embedding-8B as planned
**YELLOW (Conditional):** Approved with modifications (e.g., additional attribution, EULA changes) - assess 1-week delay
**RED (Blocked):** Pivot to fallback alternative - 2-week delay, reassess Phase 2 timeline

---

## Supporting Documents

1. **Qwen3-Embedding-8B Model Card:** https://huggingface.co/Alibaba-NLP/gte-Qwen2-7B-instruct
2. **Apache 2.0 License Text:** https://www.apache.org/licenses/LICENSE-2.0
3. **AkiDB 2.0 PRD:** `automatosx/PRD/akidb-2.0-improved-prd.md`
4. **Technical Architecture:** `automatosx/PRD/akidb-2.0-technical-architecture.md` (Section 5: Embedding Service)
5. **Draft EULA:** [INSERT LINK - Legal to provide]

---

## Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| License incompatible with commercial redistribution | Low | Critical | Fallback to Embedding-Gemma (+2 weeks) |
| Attribution requirements too burdensome | Low | Medium | Negotiate with Alibaba or modify packaging |
| Geographic restrictions (China export) | Medium | High | Consult trade compliance, consider US-based alternative |
| End-user sublicensing prohibited | Low | High | Restructure licensing (SaaS model or per-seat fees) |
| Model card disclosure required | Medium | Low | Bundle documentation with product (+0 cost) |

**Overall Risk:** MEDIUM - Apache 2.0 generally permissive, but model weight licensing less established than software licensing.

---

## Requested Deliverables from Legal

1. **Written Legal Opinion** (PDF or memo) summarizing findings on 7 key questions
2. **Go/No-Go Recommendation** with conditional approval terms if applicable
3. **Approved Attribution Text** (exact wording for LICENSE and documentation)
4. **EULA Modifications** (redline changes to standard AkiDB license agreement)
5. **Risk Mitigation Checklist** (any additional compliance steps required)

**Delivery Format:** Email summary by Nov 12 COB, formal memo by Nov 13 COB
**Confidentiality:** Internal Use Only (attorney-client privilege)

---

## Point of Contact

**Product Lead:** [INSERT NAME]
**Email:** [INSERT EMAIL]
**Slack:** #akidb-2.0-legal
**Phone:** [INSERT PHONE] (for urgent escalations)

**Engineering Lead:** [INSERT NAME]
**Email:** [INSERT EMAIL]

**Open Source Compliance Lead:** [INSERT NAME]
**Email:** [INSERT EMAIL]

---

## Appendix: Sample Attribution Language (Draft)

**Proposed Attribution (for Legal Review):**

```
AkiDB 2.0 includes Qwen3-Embedding-8B, developed by Alibaba Cloud (Tongyi Qianwen Team).

Copyright (c) 2024 Alibaba Cloud
Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.

Model repository: https://huggingface.co/Alibaba-NLP/gte-Qwen2-7B-instruct
```

**Placement:** NOTICE file in AkiDB distribution, "About" section in documentation

---

**Prepared by:** Product Lead, AkiDB 2.0
**Reviewed by:** Engineering Lead, ML Engineering Lead
**Document Version:** 1.0
**Confidentiality:** Internal Use Only (Attorney-Client Privileged)
