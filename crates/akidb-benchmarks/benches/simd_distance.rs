//! SIMD vs Scalar Distance Calculation Benchmarks
//!
//! This benchmark suite directly compares SIMD-optimized distance calculations
//! against scalar implementations to measure actual acceleration.
//!
//! Purpose:
//! 1. Verify SIMD is actually being used (not falling back to scalar)
//! 2. Measure real-world speedup across different vector dimensions
//! 3. Identify memory-bound scenarios where SIMD gains diminish

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use akidb_index::simd::{
    compute_distance_simd, compute_l2_simd, compute_cosine_simd, compute_dot_simd,
    compute_l2_scalar, compute_cosine_scalar, compute_dot_scalar,
};
use akidb_core::DistanceMetric;

/// Benchmark L2 distance across different vector dimensions
fn bench_l2_different_dims(c: &mut Criterion) {
    let mut group = c.benchmark_group("l2_distance_by_dimension");

    for dim in [32, 128, 256, 512, 1024, 2048] {
        // Create test vectors with realistic values
        let a: Vec<f32> = (0..dim).map(|i| (i as f32 * 0.01).sin()).collect();
        let b: Vec<f32> = (0..dim).map(|i| (i as f32 * 0.02).cos()).collect();

        group.throughput(Throughput::Elements(dim as u64));

        // Benchmark SIMD version
        group.bench_with_input(BenchmarkId::new("simd", dim), &dim, |bencher, _| {
            bencher.iter(|| compute_l2_simd(black_box(&a), black_box(&b)));
        });

        // Benchmark Scalar version
        group.bench_with_input(BenchmarkId::new("scalar", dim), &dim, |bencher, _| {
            bencher.iter(|| compute_l2_scalar(black_box(&a), black_box(&b)));
        });
    }

    group.finish();
}

/// Benchmark Cosine similarity across different dimensions
fn bench_cosine_different_dims(c: &mut Criterion) {
    let mut group = c.benchmark_group("cosine_similarity_by_dimension");

    for dim in [32, 128, 256, 512, 1024] {
        let a: Vec<f32> = (0..dim).map(|i| (i as f32 * 0.01).sin()).collect();
        let b: Vec<f32> = (0..dim).map(|i| (i as f32 * 0.02).cos()).collect();

        group.throughput(Throughput::Elements(dim as u64));

        group.bench_with_input(BenchmarkId::new("simd", dim), &dim, |bencher, _| {
            bencher.iter(|| compute_cosine_simd(black_box(&a), black_box(&b)));
        });

        group.bench_with_input(BenchmarkId::new("scalar", dim), &dim, |bencher, _| {
            bencher.iter(|| compute_cosine_scalar(black_box(&a), black_box(&b)));
        });
    }

    group.finish();
}

/// Benchmark Dot Product across different dimensions
fn bench_dot_different_dims(c: &mut Criterion) {
    let mut group = c.benchmark_group("dot_product_by_dimension");

    for dim in [32, 128, 256, 512, 1024] {
        let a: Vec<f32> = (0..dim).map(|i| (i as f32 * 0.01).sin()).collect();
        let b: Vec<f32> = (0..dim).map(|i| (i as f32 * 0.02).cos()).collect();

        group.throughput(Throughput::Elements(dim as u64));

        group.bench_with_input(BenchmarkId::new("simd", dim), &dim, |bencher, _| {
            bencher.iter(|| compute_dot_simd(black_box(&a), black_box(&b)));
        });

        group.bench_with_input(BenchmarkId::new("scalar", dim), &dim, |bencher, _| {
            bencher.iter(|| compute_dot_scalar(black_box(&a), black_box(&b)));
        });
    }

    group.finish();
}

/// Compare all three distance metrics at standard embedding dimension (128)
fn bench_all_metrics_128dim(c: &mut Criterion) {
    let dim = 128;
    let a: Vec<f32> = (0..dim).map(|i| (i as f32 * 0.01).sin()).collect();
    let b: Vec<f32> = (0..dim).map(|i| (i as f32 * 0.02).cos()).collect();

    let mut group = c.benchmark_group("all_metrics_128dim");
    group.throughput(Throughput::Elements(dim as u64));

    // L2
    group.bench_function("l2_simd", |bencher| {
        bencher.iter(|| compute_l2_simd(black_box(&a), black_box(&b)));
    });
    group.bench_function("l2_scalar", |bencher| {
        bencher.iter(|| compute_l2_scalar(black_box(&a), black_box(&b)));
    });

    // Cosine
    group.bench_function("cosine_simd", |bencher| {
        bencher.iter(|| compute_cosine_simd(black_box(&a), black_box(&b)));
    });
    group.bench_function("cosine_scalar", |bencher| {
        bencher.iter(|| compute_cosine_scalar(black_box(&a), black_box(&b)));
    });

    // Dot Product
    group.bench_function("dot_simd", |bencher| {
        bencher.iter(|| compute_dot_simd(black_box(&a), black_box(&b)));
    });
    group.bench_function("dot_scalar", |bencher| {
        bencher.iter(|| compute_dot_scalar(black_box(&a), black_box(&b)));
    });

    group.finish();
}

/// Benchmark throughput: computing distances for many vector pairs
///
/// Simulates HNSW search where we compute distance between query vector
/// and many candidate vectors
fn bench_batch_throughput(c: &mut Criterion) {
    let dim = 128;
    let query: Vec<f32> = (0..dim).map(|i| (i as f32 * 0.01).sin()).collect();

    // Create 1000 candidate vectors (typical for ef_search=200 in HNSW)
    let candidates: Vec<Vec<f32>> = (0..1000)
        .map(|i| (0..dim).map(|j| ((i + j) as f32 * 0.01).sin()).collect())
        .collect();

    let mut group = c.benchmark_group("batch_throughput");
    group.throughput(Throughput::Elements(1000)); // 1000 distance computations

    group.bench_function("l2_1000_vectors_simd", |bencher| {
        bencher.iter(|| {
            for candidate in &candidates {
                let _dist = compute_l2_simd(black_box(&query), black_box(candidate));
                black_box(_dist);
            }
        });
    });

    group.bench_function("l2_1000_vectors_scalar", |bencher| {
        bencher.iter(|| {
            for candidate in &candidates {
                let _dist = compute_l2_scalar(black_box(&query), black_box(candidate));
                black_box(_dist);
            }
        });
    });

    group.finish();
}

/// Benchmark using the public API (compute_distance_simd with metric enum)
fn bench_public_api(c: &mut Criterion) {
    let dim = 128;
    let a: Vec<f32> = (0..dim).map(|i| (i as f32 * 0.01).sin()).collect();
    let b: Vec<f32> = (0..dim).map(|i| (i as f32 * 0.02).cos()).collect();

    let mut group = c.benchmark_group("public_api_128dim");

    group.bench_function("distance_api_l2", |bencher| {
        bencher.iter(|| {
            compute_distance_simd(
                black_box(DistanceMetric::L2),
                black_box(&a),
                black_box(&b),
            )
        });
    });

    group.bench_function("distance_api_cosine", |bencher| {
        bencher.iter(|| {
            compute_distance_simd(
                black_box(DistanceMetric::Cosine),
                black_box(&a),
                black_box(&b),
            )
        });
    });

    group.bench_function("distance_api_dot", |bencher| {
        bencher.iter(|| {
            compute_distance_simd(
                black_box(DistanceMetric::Dot),
                black_box(&a),
                black_box(&b),
            )
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_l2_different_dims,
    bench_cosine_different_dims,
    bench_dot_different_dims,
    bench_all_metrics_128dim,
    bench_batch_throughput,
    bench_public_api,
);

criterion_main!(benches);
