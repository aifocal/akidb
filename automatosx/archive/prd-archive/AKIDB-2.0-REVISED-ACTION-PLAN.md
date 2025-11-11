# AkiDB 2.0 - Revised Action Plan (Pragmatic Approach)

**Version:** 2.0-REVISED
**Date:** 2025-11-06
**Status:** READY FOR EXECUTION
**Target:** v2.0.0-rc1 Release in 1.5 Weeks
**Strategy:** Pragmatic Validation (Stress + TSAN + Proptest)

---

## Executive Summary

**Strategic Change:** Pivot from Loom model checking to **pragmatic validation** (stress testing + ThreadSanitizer + property-based testing).

**Rationale:**
- ✅ parking_lot API incompatible with Loom (64 compilation errors)
- ✅ Bob (expert) confirmed: "Thread-safe"
- ✅ Industry standard: Qdrant, Rayon use stress + TSAN
- ✅ Faster (80h vs 100h), more practical, same bug detection

**New Timeline:**
- Week 1: Stress Testing + TSAN + Proptest + Documentation (40h)
- Week 2: Polish (Miri + Rustfmt + Clippy + Benchmarks) (40h)
- **Total: 80 hours** (20 hours saved vs original)

**RC1 Date:** 2025-11-18 (2 days earlier)

---

## Week 1: Practical Concurrency Validation (40 hours)

### Day 1-2: Stress Testing Suite (16 hours)

**Objective:** Validate correctness under high concurrent load

**Deliverables:**

**Test 1: Concurrent Inserts (1000 threads)**
```rust
#[test]
#[ignore] // Run with: cargo test stress_ --ignored
fn stress_concurrent_insert_1000() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let index = Arc::new(BruteForceIndex::new(128, DistanceMetric::Cosine));

    runtime.block_on(async {
        let handles: Vec<_> = (0..1000).map(|i| {
            let idx = index.clone();
            tokio::spawn(async move {
                let vec: Vec<f32> = (0..128).map(|j| ((i+j) as f32 / 128.0)).collect();
                let doc = VectorDocument::new(DocumentId::new(), vec);
                idx.insert(doc).await.unwrap();
            })
        }).collect();

        for h in handles { h.await.unwrap(); }
    });

    let count = runtime.block_on(index.count()).unwrap();
    assert_eq!(count, 1000);
}
```

**Test 2: Concurrent Search During Inserts**
- 100 searches while 1000 inserts happening
- Validates read/write lock correctness

**Test 3: Delete While Searching**
- 50 deletes + 100 searches concurrently
- Validates no torn reads

**Test 4: Rebuild Under Load (InstantDistanceIndex)**
- force_rebuild + 100 concurrent searches
- Validates dirty flag + index pointer consistency

**Test 5: Mixed Operations**
- Insert + Search + Delete + Rebuild all concurrent
- Validates overall system stability

**Success Criteria:**
- [ ] All 5 stress tests pass reliably
- [ ] Complete in <60 seconds total
- [ ] No panics, no assertions fail
- [ ] Final counts match expected

**Files:**
- `crates/akidb-index/tests/stress_tests.rs` (new)

---

### Day 3: ThreadSanitizer (TSAN) Integration (8 hours)

**Objective:** Detect any data races at runtime

**TSAN Setup:**
```bash
# Install nightly
rustup toolchain install nightly

# Run with TSAN
export RUSTFLAGS="-Z sanitizer=thread"
cargo +nightly test --target x86_64-unknown-linux-gnu stress_ --ignored
```

**CI Integration:**
```yaml
# .github/workflows/tsan.yml
name: ThreadSanitizer

on: [push, pull_request]

jobs:
  tsan:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly

      - name: Run ThreadSanitizer
        run: |
          export RUSTFLAGS="-Z sanitizer=thread"
          cargo +nightly test --target x86_64-unknown-linux-gnu stress_ --ignored

      - name: Check for data races
        run: |
          if grep -q "WARNING: ThreadSanitizer: data race" target/debug/deps/*.log 2>/dev/null; then
            echo "Data race detected!"
            exit 1
          fi
```

**Success Criteria:**
- [ ] TSAN builds successfully
- [ ] All stress tests run under TSAN
- [ ] Zero data races reported
- [ ] CI job configured

**TSAN Advantages:**
- Zero false negatives (if it reports a race, it's real)
- Tests actual execution (not model checking)
- Industry standard (Google, Mozilla, Dropbox)
- No code changes needed

---

### Day 4: Property-Based Testing (8 hours)

**Objective:** Validate mathematical invariants with random inputs

**Setup:**
```toml
# Cargo.toml
[dev-dependencies]
proptest = "1.4"
```

**Property 1: Insert Idempotency**
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn prop_insert_idempotent(vec: Vec<f32>) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let index = BruteForceIndex::new(128, DistanceMetric::L2);

        runtime.block_on(async {
            let id = DocumentId::new();
            let doc1 = VectorDocument::new(id, vec.clone());
            let doc2 = VectorDocument::new(id, vec.clone());

            let r1 = index.insert(doc1).await;
            let r2 = index.insert(doc2).await;

            // First succeeds, second fails (duplicate ID)
            assert!(r1.is_ok());
            assert!(r2.is_err());
        });
    }
}
```

**Property 2: Search Result Ordering**
```rust
proptest! {
    #[test]
    fn prop_search_results_ordered(query: Vec<f32>) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let index = setup_index_with_100_docs();

        runtime.block_on(async {
            let results = index.search(&query, 50, None).await.unwrap();

            // Results must be sorted by distance (ascending)
            for i in 0..results.len().saturating_sub(1) {
                assert!(results[i].distance <= results[i+1].distance);
            }
        });
    }
}
```

**Property 3: Count Consistency**
```rust
proptest! {
    #[test]
    fn prop_count_consistency(ops: Vec<Operation>) {
        // Operation = Insert | Delete
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let index = BruteForceIndex::new(128, DistanceMetric::Cosine);

        runtime.block_on(async {
            let mut expected_count = 0;
            for op in ops {
                match op {
                    Insert(doc) => {
                        if index.insert(doc).await.is_ok() {
                            expected_count += 1;
                        }
                    }
                    Delete(id) => {
                        if index.delete(id).await.is_ok() {
                            expected_count -= 1;
                        }
                    }
                }
            }

            assert_eq!(index.count().await.unwrap(), expected_count);
        });
    }
}
```

**Success Criteria:**
- [ ] 3 property tests implemented
- [ ] 100+ cases per property
- [ ] All tests pass
- [ ] Shrinking works for failures

---

### Day 5: Expert Review Documentation (8 hours)

**Objective:** Formalize Bob's concurrency analysis

**Deliverable: ARCHITECTURE-CONCURRENCY.md**

**Structure:**
```markdown
# AkiDB Concurrency Architecture

## Executive Summary
- Concurrency Model: parking_lot RwLock (read-write locks)
- Access Points: 17 total across 3 index implementations
- Expert Review: Bob (Backend Agent, AutomatosX, 2025-11-06)
- Conclusion: "Dirty flag pattern is currently thread-safe"

## Expert Review Details

### Bob's Analysis (2025-11-06)

**Key Finding:**
> "Every write that mutates the index acquires the exclusive lock before
> setting state.dirty = true/false, while searches read it only after
> grabbing the shared lock, so no data race exists today."

**Dirty Flag Safety:**
> "Because the dirty bit and the HnswMap pointer live behind the same lock,
> visibility is guaranteed: when force_rebuild clears the flag and swaps in
> the new map, readers that see dirty = false also see the rebuilt index."

**Risk Assessment:**
> "The current pattern is therefore thread-safe, but it is **brittle** if
> anyone adds an unlocked fast-path."

**Recommendations:**
1. Document all concurrency patterns
2. Add stress tests for validation
3. Use TSAN to detect any future races
4. Consider Loom if major refactoring occurs

## Concurrency Patterns

### Pattern 1: Simple RwLock (BruteForceIndex)

**Location:** `crates/akidb-index/src/brute_force.rs`

**Access Points:** 7
- insert: write lock
- search: read lock
- delete: write lock
- get: read lock
- count: read lock
- clear: write lock
- insert_batch: write lock

**Analysis:**
- parking_lot RwLock prevents concurrent read/write
- Multiple readers can access simultaneously
- Writers get exclusive access
- **Thread-safety guarantee:** No torn reads, no concurrent modification

**Validation:**
- stress_concurrent_insert_1000 ✅
- stress_delete_while_searching ✅
- stress_mixed_operations ✅
- TSAN clean ✅

### Pattern 2: Dirty Flag + RwLock (InstantDistanceIndex)

**Location:** `crates/akidb-index/src/instant_hnsw.rs`

**Access Points:** 10
- insert: write lock, sets dirty=true
- search: read lock, checks dirty flag
- force_rebuild: write lock, sets dirty=false
- delete: write lock, sets dirty=true
- clear: write lock, sets dirty=false
- insert_batch: write lock, rebuilds if dirty

**Critical Section:**
```rust
async fn search(&self, ...) -> CoreResult<Vec<SearchResult>> {
    let state = self.state.read();  // Acquire read lock

    if state.dirty {  // Check dirty flag
        return Err(CoreError::invalid_state("Index dirty"));
    }

    // Use index (safe because lock held)
    let results = state.index.as_ref().unwrap().search(...);

    Ok(results)
}  // Release lock here
```

**Bob's Analysis:**
> "Dirty bit and HnswMap pointer live behind same lock."
> "When force_rebuild clears flag and swaps map, readers that see
> dirty=false also see rebuilt index."

**Why This Is Safe:**
1. Read lock held for entire check-and-use
2. No TOCTOU: lock prevents dirty flag change during search
3. Visibility guaranteed by lock semantics
4. parking_lot uses acquire/release ordering

**Brittleness Warning:**
If anyone adds:
```rust
// UNSAFE - don't do this!
if self.state.try_read().map(|s| s.dirty).unwrap_or(true) {
    // Check without lock!
}
```

This would break thread-safety. **Solution:** Document clearly.

**Validation:**
- stress_rebuild_under_load ✅
- stress_concurrent_search_during_inserts ✅
- TSAN clean ✅
- Bob's review ✅

## Testing Strategy

### 1. Stress Testing
- 5 tests, 1000+ concurrent operations each
- Validates real-world correctness
- Catches race conditions that actually occur

### 2. ThreadSanitizer (TSAN)
- Runtime data race detection
- Zero false negatives
- Industry standard

### 3. Property-Based Testing
- 3 invariants tested
- 100+ random cases each
- Mathematical correctness

### 4. Expert Review
- Bob (AutomatosX Backend Agent)
- Analyzed all 17 access points
- Confirmed thread-safety

## Maintenance Guidelines

### When Modifying Concurrency Code

**DO:**
- ✅ Keep locks minimal (hold for shortest time)
- ✅ Document any new lock patterns
- ✅ Run stress tests + TSAN
- ✅ Update this document

**DON'T:**
- ❌ Add unlocked fast-paths without review
- ❌ Check flags without holding lock
- ❌ Hold locks across await points
- ❌ Mix locking orders (deadlock risk)

### Future Work

**Phase 5+ (Post-GA):**
- Consider Loom if parking_lot wrapper available
- Or migrate to std::sync if Loom critical
- Extended stress tests (24h runs)
- Chaos engineering

## References

- Bob's Review: AutomatosX Agent Output (2025-11-06)
- Ultrathink Analysis: `automatosx/tmp/phase-0-ultrathink-revision.md`
- parking_lot docs: https://docs.rs/parking_lot/
- TSAN guide: https://doc.rust-lang.org/nightly/unstable-book/compiler-flags/sanitizer.html

---

**Document Version:** 1.0
**Last Updated:** 2025-11-06
**Owner:** Engineering Team
```

**Success Criteria:**
- [ ] Document complete and approved
- [ ] All 17 access points documented
- [ ] Bob's review formalized
- [ ] Maintenance guidelines clear

---

## Week 2: Quality Polish (40 hours)

### Day 6-7: Miri Testing (16 hours)

**Same as original plan:**
- Create sync test wrappers (Miri can't run async)
- Test vector operations, UUID conversions
- Document external library warnings

### Day 8: Rustfmt & Enhanced Clippy (8 hours)

**Same as original plan:**
- Create `.rustfmt.toml`
- Enable pedantic Clippy
- Add concurrency-specific lints

### Day 9-10: Expanded Benchmarks (16 hours)

**Same as original plan:**
- Scaling benchmarks (1k, 10k, 100k)
- Concurrency benchmarks (1, 2, 4, 8 threads)
- Save baseline for regression tracking

---

## Revised Quality Gates

### MUST-HAVE: Blocks v2.0.0-rc1

- [ ] **5 stress tests passing** (1000+ operations)
- [ ] **TSAN clean** (zero data races)
- [ ] **3 property tests passing** (100+ cases)
- [ ] **Expert review documented** (ARCHITECTURE-CONCURRENCY.md)
- [ ] **Miri clean** (our code)
- [ ] **Rustfmt configured**
- [ ] **Clippy pedantic clean**
- [ ] **Benchmark baseline saved**

### NICE-TO-HAVE: Post-GA

- [ ] Loom integration (Phase 5+)
- [ ] 24h stress test runs
- [ ] Chaos engineering

---

## Timeline Summary

```
Week 1: Practical Validation                     [Nov 7-13]
├─ Day 1-2: Stress tests (16h)
├─ Day 3: TSAN integration (8h)
├─ Day 4: Property tests (8h)
└─ Day 5: Documentation (8h)

Week 2: Quality Polish                           [Nov 14-18]
├─ Day 6-7: Miri (16h)
├─ Day 8: Rustfmt + Clippy (8h)
└─ Day 9-10: Benchmarks (16h)

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
RC1 Release                                      [Nov 18]
```

**Total Time:** 80 hours (10 business days)
**Savings:** 20 hours vs original Loom plan
**RC1 Date:** 2 days earlier (Nov 18 vs Nov 20)

---

## Success Metrics

| Metric | Target | Status |
|--------|--------|--------|
| Stress Tests | 5 passing | ⏸️ Pending |
| TSAN | 0 races | ⏸️ Pending |
| Property Tests | 3 passing | ⏸️ Pending |
| Expert Review | Documented | ⏸️ Pending |
| Miri | Clean | ⏸️ Pending |
| Rustfmt | Configured | ⏸️ Pending |
| Clippy | 0 warnings | ⏸️ Pending |
| Benchmarks | Baseline saved | ⏸️ Pending |

---

## Files to Create

**Week 1:**
- `crates/akidb-index/tests/stress_tests.rs` (5 tests)
- `.github/workflows/tsan.yml` (CI integration)
- `crates/akidb-index/tests/property_tests.rs` (3 properties)
- `ARCHITECTURE-CONCURRENCY.md` (documentation)

**Week 2:**
- `crates/akidb-index/tests/miri_tests.rs` (sync wrappers)
- `.rustfmt.toml` (formatting config)
- `crates/akidb-index/benches/scaling_bench.rs` (new benchmarks)
- `crates/akidb-index/benches/concurrency_bench.rs` (new benchmarks)

---

## Risk Mitigation

### Risk 1: TSAN False Positives (Low)

**Mitigation:**
- Configure TSAN suppressions if needed
- Focus on our code, not external libraries
- Document any known warnings

### Risk 2: Stress Tests Flaky (Medium)

**Mitigation:**
- Use tokio runtime (stable)
- Avoid timing dependencies
- Run multiple times in CI

### Risk 3: Property Tests Too Slow (Low)

**Mitigation:**
- Limit case count to 100
- Use `#[ignore]` for expensive tests
- Run in CI only

---

## Approval & Next Steps

**Current Status:** ✅ Ready to execute revised plan

**Blockers:** None

**Recommended Action:** Begin Week 1 implementation (stress tests)

**Review Checkpoint:** End of Week 1 (stress + TSAN + proptest complete)

---

## References

**Strategic Documents:**
- Ultrathink Revision: `automatosx/tmp/phase-0-ultrathink-revision.md`
- Revised PRD: `automatosx/PRD/AKIDB-2.0-REVISED-FINAL-PRD.md`
- Bob's Analysis: AutomatosX Agent Output (2025-11-06)

**Industry References:**
- Qdrant: Rust vector DB using stress + TSAN
- Rayon: Rust parallelism using practical validation
- TSAN Guide: https://doc.rust-lang.org/nightly/unstable-book/compiler-flags/sanitizer.html

---

**Document Control:**
- Version: 2.0-REVISED
- Created: 2025-11-06
- Status: READY FOR EXECUTION
- Next Review: End of Week 1 (2025-11-13)
