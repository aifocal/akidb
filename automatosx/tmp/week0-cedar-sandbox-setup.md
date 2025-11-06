# Cedar Policy Sandbox Setup (Week 0)

Great architecture is invisible - it enables teams, evolves gracefully, and pays dividends over decades. This sandbox gives the Platform Security team a safe arena to iterate on Cedar policies before we thread them into the AkiDB 2.0 execution path.

## 1. Purpose

- Prototype Cedar authorization policies without touching production data or Akidb 2.0 services.
- Validate policy semantics, performance, and operational tooling ahead of Phase 3 integration.
- Provide a replicable playbook for security engineers and partner teams.

## 2. Prerequisites

- Rust toolchain (`rustup`, `cargo`, `rustc`) pinned to stable.
- Existing AkiDB v1.x environment to mirror tenant and user semantics.
- Local shell with write access to the repo.
- Optional: `hyperfine` or `cargo criterion` for benchmarking.

## 3. Install Cedar CLI

```bash
bash -lc 'cargo install cedar-policy-cli'
```

Verify:

```bash
bash -lc 'cedar --version'
```

## 4. Sandbox Layout

Create an isolated workspace to avoid polluting v1 assets.

```bash
bash -lc 'mkdir -p .cedar-sandbox/{policies,schemas,requests,data,reports}'
```

- `policies/`: Cedar policy files (`.cedar`)
- `schemas/`: Entity and action schemas
- `requests/`: Authorization requests for validation
- `data/`: Synthetic tenants, users, roles, resources
- `reports/`: Validation, evaluation, and benchmarking output

Version-control guidance:
- Check in canonical examples under `automatosx/PRD/` once stabilized.
- Keep experimental artifacts inside `.cedar-sandbox/` (git-ignored).

## 5. Synthetic Dataset

Targets: 3 tenants × 10 users × 5 roles. Use the helper script below to emit JSON entities and relationships compatible with Cedar schemas.

```bash
bash -lc 'cat > .cedar-sandbox/data/bootstrap.rs <<''EOF''
use rand::{seq::SliceRandom, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde_json::json;

const TENANTS: [&str; 3] = ["tenant-alpha", "tenant-beta", "tenant-gamma"];
const ROLES: [&str; 5] = ["admin", "developer", "viewer", "auditor", "support"];

fn main() {
    let mut rng = ChaCha8Rng::seed_from_u64(42);
    let mut entities = vec![];

    for tenant in TENANTS {
        entities.push(json!({
            "uid": format!("Tenant::{}", tenant),
            "attrs": {
                "name": tenant,
                "tier": "sandbox"
            }
        }));

        for idx in 0..10 {
            let role = ROLES.choose(&mut rng).unwrap();
            let user_id = format!("{}-user-{}", tenant, idx + 1);

            entities.push(json!({
                "uid": format!("User::{}", user_id),
                "attrs": {
                    "tenant": tenant,
                    "email": format!("{}@{}.akidb.dev", user_id, tenant)
                },
                "parents": [{
                    "uid": format!("Role::{}#{}", tenant, role)
                }]
            }));
        }

        for role in ROLES {
            entities.push(json!({
                "uid": format!("Role::{}#{}", tenant, role),
                "attrs": {
                    "tenant": tenant,
                    "role": role
                },
                "parents": [{
                    "uid": format!("Tenant::{}", tenant)
                }]
            }));
        }
    }

    println!("{}", serde_json::to_string_pretty(&entities).unwrap());
}
EOF'
```

Generate entities:

```bash
bash -lc 'rustc .cedar-sandbox/data/bootstrap.rs -o .cedar-sandbox/data/bootstrap && ./.cedar-sandbox/data/bootstrap > .cedar-sandbox/data/entities.json'
```

## 6. Sample Policies

Create `.cedar-sandbox/policies/base.cedar`:

```cedar
permit(
  principal in Role::"tenant-alpha#admin" || principal in Role::"tenant-beta#admin" || principal in Role::"tenant-gamma#admin",
  action,
  resource
) when { resource.tenant == principal.tenant };

permit(principal, action in [Action::"collection::read", Action::"collection::write"], resource)
when {
  principal in Role::format!("{}#developer", resource.tenant)
};

permit(principal, Action::"collection::read", resource)
when { principal in Role::format!("{}#viewer", resource.tenant) };

permit(principal, Action::"audit::read", resource)
when { principal in Role::"tenant-alpha#auditor" || principal in Role::"tenant-beta#auditor" || principal in Role::"tenant-gamma#auditor" };

forbid(principal, Action::"user::manage", resource)
when { principal in Role::format!("{}#developer", principal.tenant) || principal in Role::format!("{}#viewer", principal.tenant) };
```

Schema tips:
- Mirror `akidb-core::user::UserProfile` attributes (tenant, roles, lifecycle flags).
- Model collections as `Collection::tenant-name#collection-id`.
- Model audit logs as `AuditLog::tenant-name#date`.

## 7. Testing Workflow

1. **Author** policy in `policies/`.
2. **Validate syntax**:
   ```bash
   bash -lc 'cedar validate --policies .cedar-sandbox/policies --schema .cedar-sandbox/schemas/core.json'
   ```
3. **Craft requests** in `requests/` (JSON objects with `principal`, `action`, `resource`, `context`).
4. **Evaluate**:
   ```bash
   bash -lc 'cedar evaluate --policies .cedar-sandbox/policies --entities .cedar-sandbox/data/entities.json --request .cedar-sandbox/requests/dev-access.json'
   ```
5. **Latency measurement**:
   - Wrap evaluation in `hyperfine` or `time`.
   - Target P99 < 5 ms per call on dev hardware.
   - Record results in `reports/latency.md`.
6. **Regression checklist**:
   - Multi-tenant isolation preserved.
   - Role inheritance works for nested hierarchies.
   - Deny rules override allow when conflict detected.

## 8. Integration Points with `akidb-core::user`

- Map `UserProfile.roles` → Cedar `Role` entities.
- Surface `UserProfile.tenant_id` and `UserProfile.user_type` as attributes.
- Ensure consistency with `akidb-core::user::RoleAssignment` semantics (e.g., default roles, temporary grants).
- Align Cedar action namespace with `akidb-core::user::PermissionMatrix`.
- Prepare adapters that translate gRPC/REST requests into Cedar evaluation input, using the same schema as the sandbox.

## 9. Performance Benchmark (10k Policies)

Goal: Stress-test Cedar evaluation with large policy sets to emulate Phase 3 scale.

Procedure:

1. Generate synthetic policies:
   ```bash
   bash -lc 'python scripts/gen_policies.py --count 10000 --out .cedar-sandbox/policies/scale-test'
   ```
   - `gen_policies.py` should vary tenant, resource, and action combinations while preserving valid syntax.
2. Warm caching layer (if using Cedar policy store).
3. Run benchmark:
   ```bash
   bash -lc 'hyperfine --warmup 5 --runs 50 "cedar evaluate --policies .cedar-sandbox/policies/scale-test --entities .cedar-sandbox/data/entities.json --request .cedar-sandbox/requests/loadtest.json"'
   ```
4. Capture P50/P95/P99 latencies, memory footprint, and CPU utilization in `reports/perf-scale-10k.md`.
5. Compare results with `akidb-core` authorization latency budget (< 15 ms end-to-end).

## 10. Reporting & Governance

- Store validated policies and benchmark outcomes in `automatosx/tmp/` for rapid iteration.
- Escalate findings at Architecture Runway review (Week 1) with:
  - Policy coverage gaps
  - Performance anomalies
  - Schema adjustments needed for `akidb-core::user`

## 11. Next Steps (Phase 3 Integration Plan)

1. **Schema Hardening (Week 3)**: Lock Cedar schema aligned with `akidb-core::user` contracts.
2. **Policy Service Spike (Week 4-5)**: Backend team prototypes policy evaluation microservice using sandbox assets.
3. **Integration Testing (Week 6-7)**: Wire Cedar decisions into `akidb-core` authorization middleware.
4. **Observability Fit (Week 7)**: Add tracing, logging, and metrics for policy evaluations.
5. **Security Review (Week 8)**: Conduct joint review with Security and Compliance.
6. **Cutover Plan (Week 9)**: Plan staged rollout, fallback strategies, and tenant migration playbook.

Maintain ADR coverage and cross-link relevant decisions in `.automatosx/abilities/our-architecture-decisions.md` as policies graduate from sandbox to production.
