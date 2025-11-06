//! Vector indexing implementations for AkiDB 2.0.
//!
//! This crate provides vector index implementations:
//! - `BruteForceIndex`: Simple linear scan (baseline for correctness)
//! - `HnswIndex`: HNSW graph-based ANN for approximate nearest neighbor search

mod brute_force;
mod hnsw;

pub use brute_force::BruteForceIndex;
pub use hnsw::{HnswConfig, HnswIndex};
