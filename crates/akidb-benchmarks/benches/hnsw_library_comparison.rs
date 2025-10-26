//! Comprehensive benchmark comparing instant-distance vs hnsw_rs
//!
//! This benchmark tests both libraries at 1M vectors scale to validate
//! the PoC results and make a final decision on which library to use.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, SamplingMode};
use rand::Rng;
use std::time::Duration;

// instant-distance types
use instant_distance::{Builder, Point, Search};

// hnsw_rs types
use hnsw_rs::prelude::*;

const DIM: usize = 128;
const EF_CONSTRUCTION: usize = 400;
const M: usize = 16;

// Test configurations
const CONFIGS: &[(usize, usize, usize)] = &[
    // (num_vectors, ef_search, num_queries)
    (100_000, 200, 100),   // 100K baseline
    (1_000_000, 200, 100), // 1M production scale
];

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

fn bench_instant_distance_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("instant_distance_build");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(10); // Reduce sample size for long-running benchmarks

    for &(num_vectors, _, _) in CONFIGS {
        let vectors = generate_random_vectors(num_vectors, DIM);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}k", num_vectors / 1000)),
            &num_vectors,
            |bencher, _| {
                bencher.iter(|| {
                    let points: Vec<VectorPoint> = vectors
                        .iter()
                        .map(|v| VectorPoint {
                            vector: v.clone(),
                        })
                        .collect();

                    let values: Vec<usize> = (0..vectors.len()).collect();

                    Builder::default()
                        .ef_construction(EF_CONSTRUCTION)
                        .ef_search(200)
                        .build(points, values)
                });
            },
        );
    }

    group.finish();
}

fn bench_hnsw_rs_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("hnsw_rs_build");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(10);

    for &(num_vectors, _, _) in CONFIGS {
        let vectors = generate_random_vectors(num_vectors, DIM);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}k", num_vectors / 1000)),
            &num_vectors,
            |bencher, _| {
                bencher.iter(|| {
                    let hnsw: Hnsw<f32, DistL2> = Hnsw::new(
                        M,
                        num_vectors,
                        16, // max_layer
                        EF_CONSTRUCTION,
                        DistL2,
                    );

                    for (id, vec) in vectors.iter().enumerate() {
                        hnsw.insert((vec.as_slice(), id));
                    }

                    hnsw
                });
            },
        );
    }

    group.finish();
}

fn bench_instant_distance_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("instant_distance_search");
    group.sampling_mode(SamplingMode::Auto);
    group.measurement_time(Duration::from_secs(30)); // Longer measurement for accurate P95/P99

    for &(num_vectors, ef_search, num_queries) in CONFIGS {
        // Build index once
        let vectors = generate_random_vectors(num_vectors, DIM);
        let queries = generate_random_vectors(num_queries, DIM);

        let points: Vec<VectorPoint> = vectors
            .iter()
            .map(|v| VectorPoint {
                vector: v.clone(),
            })
            .collect();

        let values: Vec<usize> = (0..vectors.len()).collect();

        let hnsw = Builder::default()
            .ef_construction(EF_CONSTRUCTION)
            .ef_search(ef_search)
            .build(points, values);

        let k = 10;

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}k_ef{}", num_vectors / 1000, ef_search)),
            &num_vectors,
            |bencher, _| {
                let mut query_idx = 0;
                bencher.iter(|| {
                    let query = &queries[query_idx % queries.len()];
                    query_idx += 1;

                    let query_point = VectorPoint {
                        vector: query.clone(),
                    };
                    let mut search = Search::default();
                    let _results: Vec<_> = hnsw.search(&query_point, &mut search).take(k).collect();
                });
            },
        );
    }

    group.finish();
}

fn bench_hnsw_rs_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("hnsw_rs_search");
    group.sampling_mode(SamplingMode::Auto);
    group.measurement_time(Duration::from_secs(30));

    for &(num_vectors, ef_search, num_queries) in CONFIGS {
        // Build index once
        let vectors = generate_random_vectors(num_vectors, DIM);
        let queries = generate_random_vectors(num_queries, DIM);

        let hnsw: Hnsw<f32, DistL2> = Hnsw::new(
            M,
            num_vectors,
            16, // max_layer
            EF_CONSTRUCTION,
            DistL2,
        );

        for (id, vec) in vectors.iter().enumerate() {
            hnsw.insert((vec.as_slice(), id));
        }

        let k = 10;

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}k_ef{}", num_vectors / 1000, ef_search)),
            &num_vectors,
            |bencher, _| {
                let mut query_idx = 0;
                bencher.iter(|| {
                    let query = &queries[query_idx % queries.len()];
                    query_idx += 1;

                    let _results = hnsw.search(query.as_slice(), k, ef_search);
                });
            },
        );
    }

    group.finish();
}

// Benchmark different ef_search values for 1M vectors
fn bench_ef_search_tuning(c: &mut Criterion) {
    let mut group = c.benchmark_group("hnsw_rs_ef_search_tuning");
    group.sampling_mode(SamplingMode::Auto);
    group.measurement_time(Duration::from_secs(20));

    let num_vectors = 1_000_000;
    let ef_search_values = [100, 150, 200, 250, 300, 400];

    // Build index once
    let vectors = generate_random_vectors(num_vectors, DIM);
    let queries = generate_random_vectors(100, DIM);

    let hnsw: Hnsw<f32, DistL2> = Hnsw::new(
        M,
        num_vectors,
        16, // max_layer
        EF_CONSTRUCTION,
        DistL2,
    );

    for (id, vec) in vectors.iter().enumerate() {
        hnsw.insert((vec.as_slice(), id));
    }

    let k = 10;

    for &ef_search in &ef_search_values {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("ef_{}", ef_search)),
            &ef_search,
            |bencher, &ef| {
                let mut query_idx = 0;
                bencher.iter(|| {
                    let query = &queries[query_idx % queries.len()];
                    query_idx += 1;

                    let _results = hnsw.search(query.as_slice(), k, ef);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default()
        .sample_size(100)
        .warm_up_time(Duration::from_secs(5));
    targets =
        bench_instant_distance_build,
        bench_hnsw_rs_build,
        bench_instant_distance_search,
        bench_hnsw_rs_search,
        bench_ef_search_tuning
);

criterion_main!(benches);
