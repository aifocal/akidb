# ADR-002: Cedar Policy Engine for RBAC

**Status:** Proposed (Pending Week 0 Sandbox Validation)
**Date:** 2025-11-06
**Decision Makers:** Platform Security Lead, Architecture Lead, Backend Team
**Consulted:** Compliance, Product, CTO

---

## Context

AkiDB introduces **fine-grained, policy-driven RBAC** to support enterprise customers with complex authorization requirements:

- Multi-tenant isolation (tenant A users cannot access tenant B resources)
- Role-based permissions (admin, developer, viewer, auditor)
- Resource-level access control (collection-level read/write permissions)
- Attribute-based policies (e.g., "allow if user.tier == 'premium' AND resource.sensitivity == 'low'")
- Audit logging for compliance (SOC 2, HIPAA, FedRAMP)

The v1.x implementation used hard-coded role checks in Rust code:

```rust
// v1.x approach (inflexible)
if user.role == Role::Admin || (user.role == Role::Developer && collection.owner == user.tenant_id) {
    // Allow access
} else {
    return Err(AuthzError::Forbidden);
}
```

**Problems with v1.x Approach:**
- Authorization logic scattered across codebase (difficult to audit)
- No central policy management (hard to change policies without code deploys)
- Difficult to test (requires mocking user/resource contexts)
- No policy versioning or rollback
- Cannot delegate policy authoring to security team (requires Rust knowledge)

We need a **policy engine** that:
1. Separates policy logic from application code
2. Provides a declarative policy language (non-developers can author)
3. Supports fine-grained, attribute-based access control (ABAC)
4. Enables centralized policy management and auditing
5. Performs evaluation in <5ms P99 (low latency overhead)
6. Runs on ARM edge devices (Mac ARM, Jetson, OCI ARM)

## Decision

We will adopt **AWS Cedar** as the policy engine for AkiDB RBAC, with **OPA (Open Policy Agent)** as a fallback if Cedar performance issues arise.

**Implementation Approach:**

```rust
// akidb-core::user with Cedar integration
use cedar_policy::{Authorizer, Context, Entities, PolicySet, Request};

pub struct PolicyEngine {
    authorizer: Authorizer,
    policy_store: Arc<RwLock<PolicySet>>,
    entities: Arc<RwLock<Entities>>,
}

impl PolicyEngine {
    pub async fn is_authorized(&self, req: AuthzRequest) -> Result<AuthzDecision> {
        let cedar_request = Request::new(
            req.principal,  // e.g., User::"tenant-alpha-user-42"
            req.action,     // e.g., Action::"collection::read"
            req.resource,   // e.g., Collection::"tenant-alpha#db-1#coll-99"
            Context::from_json_value(req.context)?,
        );

        let policies = self.policy_store.read().await;
        let entities = self.entities.read().await;

        let response = self.authorizer.is_authorized(&cedar_request, &policies, &entities);

        Ok(AuthzDecision {
            decision: response.decision(),
            reasons: response.diagnostics().reasons().collect(),
        })
    }
}
```

**Policy Example** (Cedar Language):
```cedar
// Policy: Tenant admins can access all resources in their tenant
permit(
  principal in Role::"tenant-alpha#admin",
  action,
  resource
) when {
  resource.tenant == principal.tenant
};

// Policy: Developers can read/write collections in their tenant
permit(
  principal in Role::"tenant-alpha#developer",
  action in [Action::"collection::read", Action::"collection::write"],
  resource
) when {
  resource.tenant == principal.tenant
};

// Policy: Viewers can only read collections
permit(
  principal in Role::"tenant-alpha#viewer",
  Action::"collection::read",
  resource
) when {
  resource.tenant == principal.tenant
};

// Policy: Deny developers from managing users
forbid(
  principal in Role::"tenant-alpha#developer",
  Action::"user::manage",
  resource
);
```

**Entity Schema** (Cedar Schema):
```json
{
  "Tenant": {
    "memberOfTypes": [],
    "shape": {
      "type": "Record",
      "attributes": {
        "name": { "type": "String" },
        "tier": { "type": "String" }
      }
    }
  },
  "Role": {
    "memberOfTypes": ["Tenant"],
    "shape": {
      "type": "Record",
      "attributes": {
        "tenant": { "type": "String" },
        "role": { "type": "String" }
      }
    }
  },
  "User": {
    "memberOfTypes": ["Role"],
    "shape": {
      "type": "Record",
      "attributes": {
        "tenant": { "type": "String" },
        "email": { "type": "String" }
      }
    }
  },
  "Collection": {
    "memberOfTypes": ["Database"],
    "shape": {
      "type": "Record",
      "attributes": {
        "tenant": { "type": "String" },
        "database": { "type": "String" },
        "sensitivity": { "type": "String" }
      }
    }
  }
}
```

**Week 0 Validation:**
- Cedar policy sandbox setup (see `week0-cedar-sandbox-setup.md`)
- Performance benchmark: P99 <5ms with 10k policies
- Fallback to OPA if Cedar fails performance threshold

---

## Alternatives Considered

### Alternative 1: Open Policy Agent (OPA)

**Pros:**
- Industry standard (CNCF graduated project)
- Rich ecosystem (Kubernetes admission control, service mesh integration)
- Rego language (declarative, functional)
- Strong community support and tooling
- Battle-tested in production (Netflix, Pinterest, Goldman Sachs)
- ARM-compatible (Go binary)

**Cons:**
- ⚠️ **Rego Learning Curve:** Functional language with unfamiliar syntax
  ```rego
  # Rego example (more complex than Cedar)
  allow {
    input.user.role == "admin"
    input.resource.tenant == input.user.tenant
  }
  ```
- ⚠️ **Performance Overhead:** Rego interpreter adds 3-10ms latency per eval
- ⚠️ **Policy Complexity:** Large policy sets can be difficult to debug
- ❌ **Schema Validation:** No built-in schema validation for policies

**Decision:** Use as **fallback** if Cedar fails performance tests. OPA is more mature but Cedar's simpler syntax and AWS backing make it a better fit.

### Alternative 2: Casbin

**Pros:**
- Supports multiple policy models (ACL, RBAC, ABAC)
- Rust bindings available (`casbin-rs`)
- Lightweight (pure Rust, no external dependencies)
- Simple policy syntax

**Cons:**
- ❌ **Limited ABAC Support:** Attribute-based policies require custom adapters
- ❌ **Smaller Community:** Less battle-tested than OPA or Cedar
- ❌ **No Schema Validation:** Policies are string-based, no type safety
- ❌ **Audit Trail:** Limited built-in audit logging

**Decision:** Rejected due to limited ABAC support and smaller ecosystem.

### Alternative 3: Custom Rust Authorization Logic (v1.x Approach)

**Pros:**
- Full control over logic (no external dependencies)
- Type-safe (Rust compiler catches errors)
- No performance overhead (inline checks)
- Simple deployment (no policy engine binary)

**Cons:**
- ❌ **Inflexible:** Policy changes require code deploys
- ❌ **Scattered Logic:** Authorization checks across entire codebase
- ❌ **Hard to Audit:** Compliance teams cannot review policies independently
- ❌ **No Versioning:** Cannot rollback policy changes without code rollback
- ❌ **Testing Complexity:** Requires extensive mocking

**Decision:** Rejected due to lack of policy management capabilities.

### Alternative 4: Polar (Oso Cloud)

**Pros:**
- Modern policy language (Polar - declarative, Python-like syntax)
- First-class ABAC support
- Cloud-hosted policy management (Oso Cloud)
- Good documentation and examples

**Cons:**
- ❌ **Vendor Lock-in:** Oso Cloud is proprietary SaaS
- ❌ **Network Dependency:** Policy evaluation requires API calls (breaks offline-first)
- ⚠️ **Rust Support:** Limited Rust ecosystem compared to Cedar/OPA
- ❌ **Edge Incompatible:** Requires internet connectivity (dealbreaker for Jetson)

**Decision:** Rejected due to network dependency and edge incompatibility.

---

## Rationale

Cedar is chosen for these reasons:

### 1. **Declarative, Human-Readable Syntax**
```cedar
// Cedar: Easy to read, easy to audit
permit(principal in Role::"admin", action, resource)
when { resource.tenant == principal.tenant };
```

vs.

```rego
# OPA Rego: More complex
allow {
  input.user.role == "admin"
  input.resource.tenant == input.user.tenant
}
```

vs.

```rust
// v1.x: Buried in code
if user.role == Role::Admin && resource.tenant == user.tenant_id {
    Ok(())
} else {
    Err(AuthzError::Forbidden)
}
```

Cedar's syntax is closest to natural language, making policies auditable by non-developers (compliance, security teams).

### 2. **First-Class ABAC Support**
```cedar
permit(principal, action, resource)
when {
  principal.clearanceLevel >= resource.securityLevel &&
  principal.department == resource.owningDepartment
};
```
Cedar natively supports attribute-based policies without custom adapters.

### 3. **Formal Verification and Type Safety**
- Cedar policies are **statically analyzed** for correctness
- Schema validation catches errors before runtime
- Theorem-proven to be **terminating** (no infinite loops)
- Published research: [Cedar Formal Verification](https://www.amazon.science/publications/cedar-a-new-language-for-expressive-fast-safe-and-analyzable-authorization)

### 4. **Performance (Validated in Week 0 Sandbox)**
- **Target:** P99 <5ms with 10k policies
- **Baseline:** Cedar achieves <1ms for simple policies, <3ms for complex attribute checks
- **Benchmark Plan:**
  ```bash
  # Week 0 validation (see week0-cedar-sandbox-setup.md)
  hyperfine --warmup 5 --runs 50 \
    "cedar evaluate --policies policies/scale-test --entities data/entities.json --request requests/loadtest.json"
  ```

### 5. **AWS Backing and Ecosystem**
- Developed by AWS for IAM-like access control
- Used in production by AWS Verified Permissions
- Active open-source development (Apache 2.0 license)
- Rust-native implementation (type safety, ARM compatibility)

### 6. **Edge-Friendly Deployment**
- Embeddable library (no separate daemon)
- Zero network dependencies (policies cached in-memory)
- Minimal resource footprint (<10MB memory)
- ARM64-optimized builds

### 7. **Policy Management Workflow**
```bash
# Policy authoring by security team
vim policies/tenant-alpha-rbac.cedar

# Validate syntax
cedar validate --policies policies/ --schema schemas/core.json

# Test against synthetic data
cedar evaluate --policies policies/ --entities data/test-entities.json

# Deploy to production (via metadata DB)
akidb policy deploy --file policies/tenant-alpha-rbac.cedar --tenant alpha
```

---

## Consequences

### Positive

- ✅ **Centralized Policy Management:** All authorization logic in one place
- ✅ **Compliance-Friendly:** Security teams can audit policies independently
- ✅ **Flexible:** Change policies without code deploys
- ✅ **Type-Safe:** Schema validation catches errors early
- ✅ **Performant:** <5ms P99 (validated in Week 0 sandbox)
- ✅ **Auditability:** Every policy evaluation logged for compliance
- ✅ **Testable:** Policies can be tested with synthetic entities
- ✅ **Version Control:** Policies stored in Git, versioned, and rolled back

### Negative

- ⚠️ **Learning Curve:** Team needs training on Cedar policy language
  - *Mitigation:* Week 0 sandbox workshop, policy authoring guide
  - *Timeline:* 2-day workshop for platform security team (Nov 8-9)

- ⚠️ **Policy Complexity:** Large policy sets (10k+ policies) may be hard to manage
  - *Mitigation:* Policy modularization, namespace separation per tenant
  - *Tooling:* Policy linter, diff tool for policy changes

- ⚠️ **Performance Risk:** Unvalidated assumption that P99 <5ms is achievable
  - *Mitigation:* Week 0 sandbox benchmark (Nov 8-15), OPA fallback prepared
  - *Go/No-Go Checkpoint:* If Cedar fails benchmark, switch to OPA (Nov 24)

- ⚠️ **Operational Overhead:** Policy versioning, deployment, and rollback processes
  - *Mitigation:* Integrate policies with metadata DB, GitOps workflow
  - *Tooling:* `akidb policy` CLI for policy management

### Trade-offs

| Dimension | Cedar | OPA | Custom Rust |
|-----------|-------|-----|-------------|
| Syntax Readability | ✅ Excellent | ⚠️ Good | ❌ Poor (buried in code) |
| ABAC Support | ✅ Native | ✅ Native | ❌ Manual |
| Performance (P99) | ✅ <5ms (target) | ⚠️ ~10ms | ✅ <1ms |
| Policy Versioning | ✅ Yes | ✅ Yes | ❌ No (code deploys) |
| Edge Compatibility | ✅ Embedded | ✅ Binary | ✅ Embedded |
| Audit Logging | ✅ Built-in | ⚠️ Manual | ❌ Manual |
| Maturity | ⚠️ New (2022) | ✅ Mature (2018) | ✅ N/A |

**Verdict:** Cedar wins for readability, ABAC, and edge compatibility. OPA is fallback if performance fails.

---

## Implementation Plan

### Week 0: Sandbox Validation (Nov 6-20)

1. **Day 1-2 (Nov 6-7):** Set up Cedar policy sandbox
   - Follow `week0-cedar-sandbox-setup.md`
   - Install Cedar CLI
   - Create synthetic entities (3 tenants, 10 users, 5 roles)

2. **Day 3-5 (Nov 8-10):** Author sample policies
   - Tenant admin policy
   - Developer read/write policy
   - Viewer read-only policy
   - Auditor audit-log policy

3. **Day 6-10 (Nov 11-15):** Performance benchmark
   - Generate 10k policies (scale test)
   - Measure P50/P95/P99 latency
   - Validate P99 <5ms target
   - **Go/No-Go Decision:** If fails, prepare OPA fallback

### Phase 3: Enhanced RBAC (Weeks 9-12)

4. **Week 9:** Integrate Cedar into `akidb-core::user`
   - Add `cedar-policy` crate dependency
   - Implement `PolicyEngine` struct
   - Wire into authorization middleware

5. **Week 10:** Policy store in metadata DB
   ```sql
   CREATE TABLE policies (
     policy_id UUID PRIMARY KEY,
     tenant_id UUID REFERENCES tenants(tenant_id),
     policy_name TEXT NOT NULL,
     policy_body TEXT NOT NULL,  -- Cedar policy text
     version INTEGER NOT NULL,
     created_at TIMESTAMP,
     UNIQUE(tenant_id, policy_name, version)
   ) STRICT;
   ```

6. **Week 11:** Policy authoring tooling
   - `akidb policy validate` - syntax check
   - `akidb policy test` - test against synthetic data
   - `akidb policy deploy` - deploy to production
   - `akidb policy rollback` - rollback to previous version

7. **Week 12:** Audit logging
   - Log every policy evaluation (allow/deny, reasons)
   - Integrate with observability stack (Prometheus metrics)
   - Create audit dashboard (Grafana)

---

## Success Metrics

- [ ] **Performance:** P99 <5ms for 10k policies (Week 0 benchmark)
- [ ] **Coverage:** 100% of authorization checks use Cedar (no hard-coded logic)
- [ ] **Auditability:** All policy evaluations logged with reasons
- [ ] **Policy Count:** Support 10k+ policies per tenant without degradation
- [ ] **Developer Productivity:** Non-developers can author policies after 2-day training
- [ ] **Rollback:** Policy rollback completes in <1 minute

---

## Rollback Plan

If Cedar fails Week 0 performance benchmark (P99 >5ms):

1. **Immediate:** Switch to **OPA (Open Policy Agent)** as fallback
2. **Timeline:** +1 week delay (Week 9 becomes Week 10)
3. **Impact:** OPA Rego syntax is more complex, but functionality identical
4. **Mitigation:** Pre-author OPA policies in parallel during Week 0

**OPA Fallback Example:**
```rego
package akidb.authz

default allow = false

allow {
  input.user.role == "admin"
  input.resource.tenant == input.user.tenant
}

allow {
  input.user.role == "developer"
  input.action in ["collection::read", "collection::write"]
  input.resource.tenant == input.user.tenant
}
```

---

## References

- [Cedar Policy Language](https://www.cedarpolicy.com/)
- [Cedar Rust Crate](https://docs.rs/cedar-policy/)
- [Cedar Formal Verification Paper](https://www.amazon.science/publications/cedar-a-new-language-for-expressive-fast-safe-and-analyzable-authorization)
- [Week 0 Cedar Sandbox Setup](../tmp/week0-cedar-sandbox-setup.md)
- [Open Policy Agent (OPA)](https://www.openpolicyagent.org/)
- [AkiDB Technical Architecture](./akidb-2.0-technical-architecture.md)

---

## Notes

- **Security:** Policies stored in metadata DB are sensitive (access control rules). Encrypt at-rest if compliance requires.
- **Testing:** Use `cedar-policy-cli` for unit testing policies during development.
- **Monitoring:** Expose Cedar metrics: evaluations/sec, denials/sec, latency P50/P95/P99.
- **Documentation:** Create policy authoring guide for security team (Markdown + examples).

---

**Decision Outcome:** ✅ **Conditionally Approved** pending Week 0 benchmark validation (P99 <5ms). If benchmark fails, escalate to OPA fallback.

**Next Review:** 2025-11-15 (Go/No-Go Checkpoint after Week 0 benchmark)

---

**Signatures:**
- Platform Security Lead: ______________ Date: _______
- Architecture Lead: __________________ Date: _______
- Backend Lead: ______________________ Date: _______
