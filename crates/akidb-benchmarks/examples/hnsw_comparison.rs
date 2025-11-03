//! Quick PoC: instant-distance vs hnsw_rs performance comparison
//!
//! Usage: cargo run --example hnsw_comparison --release

use rand::Rng;
use std::time::Instant;

// instant-distance types
use instant_distance::{Builder, Point, Search};

// hnsw_rs types
use hnsw_rs::prelude::*;

const DIM: usize = 128;
const NUM_VECTORS: usize = 100_000; // 100K for quick iteration
const NUM_QUERIES: usize = 100;
const K: usize = 10;
const EF_CONSTRUCTION: usize = 400;
const EF_SEARCH: usize = 200;
const M: usize = 16;

fn generate_random_vectors(count: usize, dim: usize) -> Vec<Vec<f32>> {
    let mut rng = rand::thread_rng();
    (0..count)
        .map(|_| (0..dim).map(|_| rng.gen::<f32>()).collect())
        .collect()
}

// Custom Point type for instant-distance
#[derive(Clone, Debug)]
struct VectorPoint {
    vector: Vec<f32>,
}

impl Point for VectorPoint {
    fn distance(&self, other: &Self) -> f32 {
        // L2 distance
        self.vector
            .iter()
            .zip(other.vector.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f32>()
            .sqrt()
    }
}

fn test_instant_distance(
    vectors: &[Vec<f32>],
    queries: &[Vec<f32>],
) -> (std::time::Duration, std::time::Duration) {
    println!("\n=== Testing instant-distance ===");

    // Build index
    print!("Building index... ");
    let start = Instant::now();

    let points: Vec<VectorPoint> = vectors
        .iter()
        .map(|v| VectorPoint { vector: v.clone() })
        .collect();

    let values: Vec<usize> = (0..vectors.len()).collect();

    let hnsw = Builder::default()
        .ef_construction(EF_CONSTRUCTION)
        .ef_search(EF_SEARCH)
        .build(points, values);

    let build_time = start.elapsed();
    println!("Done in {:?}", build_time);

    // Search
    print!("Running {} searches... ", NUM_QUERIES);
    let start = Instant::now();
    for query in queries {
        let query_point = VectorPoint {
            vector: query.clone(),
        };
        let mut search = Search::default();
        let _results: Vec<_> = hnsw.search(&query_point, &mut search).take(K).collect();
    }
    let search_time = start.elapsed();
    let avg_search_time = search_time / NUM_QUERIES as u32;

    println!("Done");
    println!("  Total: {:?}", search_time);
    println!("  Avg per query: {:?}", avg_search_time);

    (build_time, avg_search_time)
}

fn test_hnsw_rs(
    vectors: &[Vec<f32>],
    queries: &[Vec<f32>],
) -> (std::time::Duration, std::time::Duration) {
    println!("\n=== Testing hnsw_rs ===");

    // Build index
    print!("Building index... ");
    let start = Instant::now();

    // Create HNSW index with L2 distance
    let hnsw: Hnsw<f32, DistL2> = Hnsw::new(
        M,
        NUM_VECTORS,
        16, // max_layer
        EF_CONSTRUCTION,
        DistL2,
    );

    // Insert vectors
    for (id, vec) in vectors.iter().enumerate() {
        hnsw.insert((vec.as_slice(), id));
    }

    let build_time = start.elapsed();
    println!("Done in {:?}", build_time);

    // Search
    print!("Running {} searches... ", NUM_QUERIES);
    let start = Instant::now();
    for query in queries {
        let _results = hnsw.search(query.as_slice(), K, EF_SEARCH);
    }
    let search_time = start.elapsed();
    let avg_search_time = search_time / NUM_QUERIES as u32;

    println!("Done");
    println!("  Total: {:?}", search_time);
    println!("  Avg per query: {:?}", avg_search_time);

    (build_time, avg_search_time)
}

fn main() {
    println!("Quick PoC: HNSW Library Comparison");
    println!("===================================");
    println!("Config:");
    println!("  Vectors: {}", NUM_VECTORS);
    println!("  Dimension: {}", DIM);
    println!("  Queries: {}", NUM_QUERIES);
    println!("  k: {}", K);
    println!("  ef_construction: {}", EF_CONSTRUCTION);
    println!("  ef_search: {}", EF_SEARCH);
    println!("  M: {}", M);

    // Generate data
    print!("\nGenerating {} random vectors... ", NUM_VECTORS);
    let vectors = generate_random_vectors(NUM_VECTORS, DIM);
    println!("Done");

    print!("Generating {} query vectors... ", NUM_QUERIES);
    let queries = generate_random_vectors(NUM_QUERIES, DIM);
    println!("Done");

    // Test instant-distance
    let (instant_build, instant_search) = test_instant_distance(&vectors, &queries);

    // Test hnsw_rs
    let (hnsw_build, hnsw_search) = test_hnsw_rs(&vectors, &queries);

    // Results
    println!("\n=== RESULTS ===");
    println!("\nBuild Time:");
    println!("  instant-distance: {:?}", instant_build);
    println!("  hnsw_rs:          {:?}", hnsw_build);
    let build_speedup = instant_build.as_secs_f64() / hnsw_build.as_secs_f64();
    if build_speedup > 1.0 {
        println!("  → hnsw_rs is {:.2}x FASTER at building", build_speedup);
    } else {
        println!(
            "  → instant-distance is {:.2}x faster at building",
            1.0 / build_speedup
        );
    }

    println!("\nSearch Time (avg per query):");
    println!("  instant-distance: {:?}", instant_search);
    println!("  hnsw_rs:          {:?}", hnsw_search);

    let search_speedup = instant_search.as_secs_f64() / hnsw_search.as_secs_f64();
    if search_speedup > 1.0 {
        println!("  → hnsw_rs is {:.2}x FASTER at searching", search_speedup);
    } else {
        println!(
            "  → instant-distance is {:.2}x faster at searching",
            1.0 / search_speedup
        );
    }

    // Decision
    println!("\n=== DECISION ===");
    if search_speedup > 1.5 {
        println!("🚀 RECOMMEND: Create full benchmark with 1M vectors");
        println!("   Potential for >50% performance improvement");
        println!(
            "   hnsw_rs shows {:.1}% faster search",
            (search_speedup - 1.0) * 100.0
        );
    } else if search_speedup > 1.2 {
        println!(
            "✅ CONSIDER: Moderate improvement ({:.1}%)",
            (search_speedup - 1.0) * 100.0
        );
        println!("   Worth switching if combined with other benefits:");
        println!("   - Thread-safe concurrent search (+600% throughput)");
        println!("   - Better algorithm implementation");
    } else if search_speedup > 1.0 {
        println!(
            "⚠️ MARGINAL: Small improvement ({:.1}%)",
            (search_speedup - 1.0) * 100.0
        );
        println!("   Consider other optimization strategies");
    } else {
        println!("❌ SKIP: No advantage");
        println!(
            "   instant-distance is {:.1}% faster",
            (1.0 / search_speedup - 1.0) * 100.0
        );
        println!("   Explore other optimization strategies");
    }

    // Throughput estimate
    println!("\n=== THROUGHPUT ESTIMATE ===");
    let instant_qps = 1.0 / instant_search.as_secs_f64();
    let hnsw_qps = 1.0 / hnsw_search.as_secs_f64();
    println!("Single-threaded:");
    println!("  instant-distance: {:.1} QPS", instant_qps);
    println!("  hnsw_rs:          {:.1} QPS", hnsw_qps);

    println!("\nWith 8-thread parallelism (Batch API):");
    println!("  instant-distance: ~{:.1} QPS", instant_qps * 8.0);
    println!("  hnsw_rs:          ~{:.1} QPS", hnsw_qps * 8.0);
    println!("  (Assuming thread-safe concurrent access)");
}
