# AkiDB 2.0 - Compilation Errors Found (Bug Hunt)

**Date:** November 14, 2025
**Status:** 🔴 CRITICAL - Tests failing to compile
**Severity:** HIGH - Breaks CI/CD pipeline

---

## Summary

Comprehensive bug hunt discovered **18 compilation errors** preventing test suite from running:

### Error Categories
1. **Format String Errors (14)** - Invalid Python-style format strings in test output
2. **Module Visibility Errors (2)** - Private modules accessed in tests
3. **Missing Function Errors (2)** - Removed `EmbeddingManager::new()` called in tests

---

## Bug #1: Invalid Format Strings in large_scale_load_tests.rs

**File:** `crates/akidb-storage/tests/large_scale_load_tests.rs`
**Lines:** 111, 114, 197, 199, 214, 217, 348, 351, 482, 485, 544, 547, 602, 605
**Severity:** HIGH

### Description
Python-style format strings (`{'='*80}`) used instead of Rust's `format!()` or `.repeat()`.

### Example Errors
```rust
// WRONG (lines 111, 114, etc.)
println!("\n{'='*80}");
println!("{'='*80}\n");

// ERROR MESSAGE:
// invalid format string: expected `}`, found `\'`
```

### Root Cause
Copy-paste from Python code or incorrect syntax used for separator lines.

### Impact
- All 6 load tests fail to compile
- No load testing possible
- CI/CD pipeline broken

### Fix Required
```rust
// CORRECT
println!("\n{}", "=".repeat(80));
println!("{}\n", "=".repeat(80));
```

**Affected Tests:**
1. `test_a1_linear_qps_ramp` (lines 111, 114)
2. `test_a2_spike_recovery` (lines 197, 199)
3. `test_a3_sustained_load_24h` (lines 214, 217)
4. `test_b1_memory_limit_discovery` (lines 348, 351)
5. `test_b2_cache_thrashing` (lines 482, 485)
6. `test_c1_race_condition_hunt` (lines 544, 547, 602, 605)

---

## Bug #2: Private Module Access in large_scale_load_tests.rs

**File:** `crates/akidb-storage/tests/large_scale_load_tests.rs`
**Line:** 16
**Severity:** MEDIUM

### Description
Test file attempts to import from private `akidb_index::brute_force` module.

### Error
```rust
use akidb_index::brute_force::BruteForceIndex;

// ERROR:
// module `brute_force` is private
```

### Root Cause
`brute_force` module is declared with `mod brute_force;` instead of `pub mod brute_force;` in `akidb-index/src/lib.rs:29`.

### Impact
- Test cannot import `BruteForceIndex`
- Load tests broken

### Fix Required
Either:
1. Make module public: `pub mod brute_force;` in `lib.rs`
2. Re-export type: `pub use brute_force::BruteForceIndex;` in `lib.rs`
3. Use public API instead (RECOMMENDED): `use akidb_index::BruteForceIndex;`

---

## Bug #3: Missing config Module in akidb-index

**File:** `crates/akidb-storage/tests/large_scale_load_tests.rs`
**Line:** 17
**Severity:** MEDIUM

### Description
Test file attempts to import from non-existent `akidb_index::config` module.

### Error
```rust
use akidb_index::config::{DistanceMetric, IndexConfig};

// ERROR:
// could not find `config` in `akidb_index`
```

### Root Cause
`DistanceMetric` has been moved to `akidb_core::collection::DistanceMetric` (line 16 already imports it correctly). There is no `IndexConfig` - index construction doesn't use a config struct.

### Impact
- Duplicate/incorrect import
- Test won't compile

### Fix Required
```rust
// Remove line 17 entirely (already imported from akidb_core on line 16)
// use akidb_index::config::{DistanceMetric, IndexConfig};  // DELETE THIS
```

---

## Bug #4: Missing EmbeddingManager::new() Method

**File:** `crates/akidb-service/src/embedding_manager.rs`
**Lines:** 220, 233, 253 (in test code)
**Severity:** HIGH

### Description
Tests call `EmbeddingManager::new()` which was removed during refactoring. Only `from_config()` exists now.

### Error
```rust
let result = EmbeddingManager::new("qwen3-0.6b-4bit").await;

// ERROR:
// no function or associated item named `new` found for struct `EmbeddingManager`
```

### Root Cause
API was changed from `new()` to `from_config()` but test code wasn't updated.

### Impact
- 3 embedding manager tests fail to compile
- No test coverage for embedding functionality

### Fix Required
```rust
// WRONG (old API):
EmbeddingManager::new("qwen3-0.6b-4bit").await

// CORRECT (new API):
EmbeddingManager::from_config(
    "mock",              // provider_type
    "mock-embed-512",    // model_name
    None                 // python_path
).await
```

**Affected Tests:**
1. `test_qwen_model_info` (line 220)
2. `test_qwen_embedding_generation` (line 233)
3. `test_qwen_batch_processing` (line 253)

### Additional Issue
These tests reference "qwen3-0.6b-4bit" model which doesn't exist in mock provider. Tests should be updated to use either:
- Python bridge provider with real model (requires setup)
- Mock provider with "mock-embed-512" model (simpler, faster)

---

## Impact Summary

### Build Status
- ❌ Workspace build: BROKEN
- ❌ Test suite: 18 compilation errors
- ❌ CI/CD: Would fail

### Affected Components
1. **Large-scale load tests:** All 6 tests broken
2. **Embedding manager tests:** 3 tests broken
3. **Test coverage:** Reduced by ~9 tests

### Risk Assessment
**CRITICAL** - These errors prevent:
- Running full test suite
- Validating system under load
- Testing embedding functionality
- Merging PRs (CI would fail)
- Release verification

---

## Fix Priority

1. **IMMEDIATE (P0):** Fix format string errors (14 occurrences)
   - Simple find-replace: `{'='*80}` → `"=".repeat(80)`

2. **IMMEDIATE (P0):** Fix EmbeddingManager tests (3 occurrences)
   - Replace `new()` with `from_config("mock", "mock-embed-512", None)`

3. **HIGH (P1):** Fix import errors (2 occurrences)
   - Remove duplicate/wrong imports
   - Use public API

---

## Lessons Learned

1. **Compilation Testing:** Always run `cargo test --workspace --all-targets` before committing
2. **Refactoring:** Update ALL usages when changing public APIs
3. **Test Maintenance:** Test code needs same rigor as production code
4. **Format Strings:** Rust uses `format!()` and `.repeat()`, not Python's `{...}` expressions

---

## Next Steps

1. ✅ Document bugs (this file)
2. ⏳ Fix all 18 compilation errors
3. ⏳ Verify tests compile and pass
4. ⏳ Update megathink document with findings
5. ⏳ Continue concurrency/resource analysis

---

**Bug Hunt Status:** Highly productive - found critical issues that would block releases
