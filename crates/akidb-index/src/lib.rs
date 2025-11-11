//! Vector indexing implementations for AkiDB 2.0.
//!
//! This crate provides vector index implementations:
//! - `BruteForceIndex`: Simple linear scan (baseline for correctness)
//! - `HnswIndex`: HNSW graph-based ANN for approximate nearest neighbor search

// Conditional compilation for Loom testing vs production
// This allows us to swap std::sync/parking_lot types with Loom's instrumented versions
#[cfg(feature = "loom")]
mod sync {
    pub use loom::sync::{Arc, RwLock};
    pub type RwLockReadGuard<'a, T> = loom::sync::RwLockReadGuard<'a, T>;
    pub type RwLockWriteGuard<'a, T> = loom::sync::RwLockWriteGuard<'a, T>;
}

#[cfg(not(feature = "loom"))]
mod sync {
    pub use parking_lot::RwLock;
    pub use std::sync::Arc;
    #[allow(dead_code)]
    pub type RwLockReadGuard<'a, T> = parking_lot::RwLockReadGuard<'a, T>;
    #[allow(dead_code)]
    pub type RwLockWriteGuard<'a, T> = parking_lot::RwLockWriteGuard<'a, T>;
}

// Re-export for internal use
pub(crate) use sync::{Arc, RwLock};

mod brute_force;
mod hnsw;
mod instant_hnsw;

pub use brute_force::BruteForceIndex;
pub use hnsw::{HnswConfig, HnswIndex};
pub use instant_hnsw::{InstantDistanceConfig, InstantDistanceIndex};
