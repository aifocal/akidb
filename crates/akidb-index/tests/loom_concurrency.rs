//! Loom concurrency tests for akidb-index.
//!
//! These tests use Loom to perform model checking on concurrent access patterns,
//! exploring all possible thread interleavings to detect race conditions, deadlocks,
//! and other concurrency bugs.
//!
//! Run with: cargo test --release --features loom --test loom_concurrency
//!
//! Loom configuration:
//! - LOOM_MAX_PREEMPTIONS=2 (default, limits state space exploration)
//! - LOOM_CHECKPOINT_INTERVAL=10 (enables checkpoint/resume for long runs)

#![cfg(feature = "loom")]

use akidb_core::{DistanceMetric, DocumentId, VectorDocument, VectorIndex};
use akidb_index::{BruteForceIndex, InstantDistanceConfig, InstantDistanceIndex};
use loom::model;
use loom::thread;

/// Helper function to create test vectors with a given seed
fn make_test_vector(dim: usize, seed: usize) -> Vec<f32> {
    (0..dim).map(|i| ((seed + i) as f32) / dim as f32).collect()
}

/// Smoke test: Basic BruteForceIndex insert in single thread
///
/// This test verifies:
/// - Loom infrastructure is working
/// - Basic single-threaded operations function under Loom
/// - Test harness can execute futures::executor::block_on
#[test]
fn loom_smoke_test_single_thread() {
    model(|| {
        use loom::sync::Arc;

        let index = Arc::new(BruteForceIndex::new(3, DistanceMetric::L2));

        let doc = VectorDocument::new(DocumentId::new(), vec![1.0, 2.0, 3.0]);
        let result = futures::executor::block_on(index.insert(doc));

        assert!(result.is_ok());

        let count = futures::executor::block_on(index.count()).unwrap();
        assert_eq!(count, 1);
    });
}

/// Smoke test: Two threads inserting into BruteForceIndex
///
/// This test verifies:
/// - Basic concurrent insert operations
/// - RwLock prevents data races
/// - Both inserts succeed
#[test]
fn loom_smoke_test_concurrent_insert() {
    model(|| {
        use loom::sync::Arc;

        let index = Arc::new(BruteForceIndex::new(3, DistanceMetric::L2));

        // Thread 1: Insert first document
        let idx1 = index.clone();
        let h1 = thread::spawn(move || {
            let doc = VectorDocument::new(DocumentId::new(), vec![1.0, 2.0, 3.0]);
            futures::executor::block_on(idx1.insert(doc))
        });

        // Thread 2: Insert second document
        let idx2 = index.clone();
        let h2 = thread::spawn(move || {
            let doc = VectorDocument::new(DocumentId::new(), vec![4.0, 5.0, 6.0]);
            futures::executor::block_on(idx2.insert(doc))
        });

        // Both inserts must succeed
        let r1 = h1.join().unwrap();
        let r2 = h2.join().unwrap();

        assert!(r1.is_ok());
        assert!(r2.is_ok());

        // Final count must be 2 (both documents inserted exactly once)
        let count = futures::executor::block_on(index.count()).unwrap();
        assert_eq!(count, 2);
    });
}

// ============================================================================
// CRITICAL SCENARIO TESTS (from Bob's recommendations)
// ============================================================================

// TODO: Scenario 1 - Insert vs Search (Dirty Flag TOCTOU)
// Test concurrent insert (sets dirty=true) + search (checks dirty) + rebuild (sets dirty=false)
// Verifies no TOCTOU race between dirty flag check and index access
//
// #[test]
// fn loom_scenario_1_insert_search_dirty_flag() {
//     // Implementation coming in next phase
// }

// TODO: Scenario 2 - Concurrent Rebuild + Search
// Test rebuild (swaps index pointer) + concurrent searches
// Verifies readers never observe index=None while dirty=false
//
// #[test]
// fn loom_scenario_2_rebuild_search_race() {
//     // Implementation coming in next phase
// }

// TODO: Scenario 3 - Delete/Clear vs Search
// Test delete (removes metadata) + concurrent search
// Verifies search never returns deleted documents
//
// #[test]
// fn loom_scenario_3_delete_search_race() {
//     // Implementation coming in next phase
// }

// TODO: Scenario 4 - Insert Batch + External Rebuild
// Test insert_batch (internal rebuild) + external force_rebuild
// Verifies no double-rebuild race on dirty flag
//
// #[test]
// fn loom_scenario_4_batch_rebuild() {
//     // Implementation coming in next phase
// }

// TODO: Scenario 5 - BruteForce Concurrent Access
// Test concurrent insert + search + count operations
// Verifies HashMap reads/writes never overlap without RwLock
//
// #[test]
// fn loom_scenario_5_brute_force_concurrent() {
//     // Implementation coming in next phase
// }

// ============================================================================
// Test Execution Notes
// ============================================================================
//
// Running Loom tests:
//
// # Run all Loom tests (with default preemptions=2)
// cargo test --release --features loom --test loom_concurrency
//
// # Run with increased exploration (slower, more thorough)
// LOOM_MAX_PREEMPTIONS=3 cargo test --release --features loom --test loom_concurrency
//
// # Run with checkpointing (for long-running tests)
// LOOM_CHECKPOINT_INTERVAL=10 cargo test --release --features loom --test loom_concurrency
//
// # Run specific test
// cargo test --release --features loom --test loom_concurrency loom_smoke_test_single_thread
//
// Expected runtime:
// - Smoke tests: <5 seconds each
// - Scenario tests: 30-120 seconds each (depends on LOOM_MAX_PREEMPTIONS)
//
// Debugging failures:
// - Set RUST_BACKTRACE=1 for stack traces
// - Reduce vector dimensions (3-dim recommended for Loom)
// - Reduce operation counts
// - Lower LOOM_MAX_PREEMPTIONS if tests timeout
