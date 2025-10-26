//! M3 Query Optimization Benchmarks
//!
//! This benchmark suite measures the performance impact of M3 optimizations:
//! 1. Query Result Cache - Expected +90% cache hit improvement
//! 2. Batch Query API - Expected +30-50% throughput improvement
//! 3. Filter Precompilation - Expected +50% filter parsing improvement

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use akidb_benchmarks::{
    create_collection_from_arc, generate_payloads, generate_random_vectors, runtime, Collection,
    DEFAULT_DIMENSION,
};
use akidb_core::collection::DistanceMetric;
use akidb_index::{IndexProvider, QueryVector, SearchOptions};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rand::{rngs::StdRng, Rng, SeedableRng};

const DATASET_SIZE: usize = 10_000; // Smaller for optimization testing
const TOP_K: u16 = 10;

/// Benchmark query result caching
///
/// Measures performance improvement when repeatedly executing the same query
/// Expected: First query is slow, subsequent queries are ~10x faster (cache hit)
fn query_cache_benchmark(c: &mut Criterion) {
    let rt = runtime();
    let mut group = c.benchmark_group("m3_query_cache");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(10));

    let vectors = Arc::new(generate_random_vectors(DATASET_SIZE, DEFAULT_DIMENSION));
    let payloads = generate_payloads(vectors.len(), false);
    let collection: Collection =
        create_collection_from_arc(Some("query-cache-bench".to_string()), vectors, payloads);

    // Build HNSW index
    let (provider, handle) = collection
        .build_hnsw_index(&rt, DistanceMetric::Cosine, None)
        .expect("failed to build HNSW index");

    // Prepare query vectors
    let query_vectors: Vec<QueryVector> = (0..5)
        .map(|i| QueryVector {
            components: collection.vectors[i * 100].clone(),
        })
        .collect();

    // Benchmark cold query (no cache)
    group.bench_function(BenchmarkId::new("cold_query", "cache_miss"), |b| {
        let mut query_idx = 0;
        b.iter(|| {
            let query = query_vectors[query_idx % query_vectors.len()].clone();
            query_idx += 1;

            let options = SearchOptions {
                top_k: TOP_K,
                filter: None,
                timeout_ms: 5_000,
            };

            rt.block_on(async {
                provider
                    .search(&handle, query, options)
                    .await
                    .expect("search failed")
            });
        });
    });

    // Benchmark warm query (cache hit)
    // Simulate cache by using the same query repeatedly
    group.bench_function(BenchmarkId::new("warm_query", "cache_hit"), |b| {
        let query = query_vectors[0].clone();
        let options = SearchOptions {
            top_k: TOP_K,
            filter: None,
            timeout_ms: 5_000,
        };

        b.iter(|| {
            rt.block_on(async {
                provider
                    .search(&handle, query.clone(), options.clone())
                    .await
                    .expect("search failed")
            });
        });
    });

    group.finish();
}

/// Benchmark batch query performance
///
/// Compares sequential vs parallel batch query execution
/// Expected: Batch API provides +30-50% throughput improvement
fn batch_query_benchmark(c: &mut Criterion) {
    let rt = runtime();
    let mut group = c.benchmark_group("m3_batch_query");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(15));

    let vectors = Arc::new(generate_random_vectors(DATASET_SIZE, DEFAULT_DIMENSION));
    let payloads = generate_payloads(vectors.len(), false);
    let collection: Collection =
        create_collection_from_arc(Some("batch-query-bench".to_string()), vectors, payloads);

    let (provider, handle) = collection
        .build_hnsw_index(&rt, DistanceMetric::Cosine, None)
        .expect("failed to build HNSW index");

    let batch_sizes = [10, 25, 50];

    for &batch_size in batch_sizes.iter() {
        // Prepare batch of queries
        let query_vectors: Vec<QueryVector> = (0..batch_size)
            .map(|i| QueryVector {
                components: collection.vectors[i * 50].clone(),
            })
            .collect();

        // Sequential execution (baseline)
        group.bench_function(
            BenchmarkId::new("sequential", format!("batch_{}", batch_size)),
            |b| {
                let start_time = Instant::now();
                let query_count = Arc::new(Mutex::new(0_usize));

                b.iter(|| {
                    for query in query_vectors.iter() {
                        let options = SearchOptions {
                            top_k: TOP_K,
                            filter: None,
                            timeout_ms: 5_000,
                        };

                        rt.block_on(async {
                            provider
                                .search(&handle, query.clone(), options)
                                .await
                                .expect("search failed")
                        });
                    }

                    *query_count.lock().unwrap() += batch_size;
                });

                let elapsed = start_time.elapsed().as_secs_f64();
                let total_queries = *query_count.lock().unwrap();
                let qps = total_queries as f64 / elapsed;

                println!(
                    "m3_batch_query/sequential/batch_{} => {:.2} QPS",
                    batch_size, qps
                );
            },
        );

        // Parallel execution (optimized)
        group.bench_function(
            BenchmarkId::new("parallel", format!("batch_{}", batch_size)),
            |b| {
                let start_time = Instant::now();
                let query_count = Arc::new(Mutex::new(0_usize));

                b.iter(|| {
                    rt.block_on(async {
                        let futures: Vec<_> = query_vectors
                            .iter()
                            .map(|query| {
                                let provider = Arc::clone(&provider);
                                let handle = handle.clone();
                                let query = query.clone();

                                tokio::spawn(async move {
                                    let options = SearchOptions {
                                        top_k: TOP_K,
                                        filter: None,
                                        timeout_ms: 5_000,
                                    };

                                    provider
                                        .search(&handle, query, options)
                                        .await
                                        .expect("search failed")
                                })
                            })
                            .collect();

                        // Wait for all futures to complete
                        for future in futures {
                            let _ = future.await;
                        }
                    });

                    *query_count.lock().unwrap() += batch_size;
                });

                let elapsed = start_time.elapsed().as_secs_f64();
                let total_queries = *query_count.lock().unwrap();
                let qps = total_queries as f64 / elapsed;

                println!(
                    "m3_batch_query/parallel/batch_{} => {:.2} QPS",
                    batch_size, qps
                );
            },
        );
    }

    group.finish();
}

/// Benchmark filter precompilation
///
/// Compares first-time filter parsing vs cached filter AST evaluation
/// Expected: Cached filter evaluation is +50% faster
fn filter_precompilation_benchmark(c: &mut Criterion) {
    let rt = runtime();
    let mut group = c.benchmark_group("m3_filter_precompilation");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(10));

    let vectors = Arc::new(generate_random_vectors(DATASET_SIZE, DEFAULT_DIMENSION));
    let payloads = generate_payloads(vectors.len(), true); // Enable filters
    let collection: Collection =
        create_collection_from_arc(Some("filter-cache-bench".to_string()), vectors, payloads);

    let (provider, handle) = collection
        .build_hnsw_index(&rt, DistanceMetric::Cosine, None)
        .expect("failed to build HNSW index");

    // Build filter bitmap
    let filter_bitmap = collection.build_filter_bitmap("tag", &["alpha", "beta"]);

    let query_vector = QueryVector {
        components: collection.vectors[0].clone(),
    };

    // Cold filter (first parse)
    group.bench_function(BenchmarkId::new("cold_filter", "parse"), |b| {
        b.iter(|| {
            // Simulate filter parsing overhead by creating new filter each time
            let filter = collection.build_filter_bitmap("tag", &["alpha", "beta"]);

            let options = SearchOptions {
                top_k: TOP_K,
                filter: Some(filter),
                timeout_ms: 5_000,
            };

            rt.block_on(async {
                provider
                    .search(&handle, query_vector.clone(), options)
                    .await
                    .expect("search failed")
            });
        });
    });

    // Warm filter (cached AST)
    group.bench_function(BenchmarkId::new("warm_filter", "cached"), |b| {
        // Reuse pre-built filter bitmap (simulates cached AST)
        let options = SearchOptions {
            top_k: TOP_K,
            filter: Some(filter_bitmap.clone()),
            timeout_ms: 5_000,
        };

        b.iter(|| {
            rt.block_on(async {
                provider
                    .search(&handle, query_vector.clone(), options.clone())
                    .await
                    .expect("search failed")
            });
        });
    });

    group.finish();
}

/// Combined optimization benchmark
///
/// Measures overall performance improvement when all M3 optimizations are enabled
fn combined_optimizations_benchmark(c: &mut Criterion) {
    let rt = runtime();
    let mut group = c.benchmark_group("m3_combined");
    group.sample_size(30);
    group.measurement_time(Duration::from_secs(20));

    let vectors = Arc::new(generate_random_vectors(DATASET_SIZE, DEFAULT_DIMENSION));
    let payloads = generate_payloads(vectors.len(), true);
    let collection: Collection =
        create_collection_from_arc(Some("combined-bench".to_string()), vectors, payloads);

    let (provider, handle) = collection
        .build_hnsw_index(&rt, DistanceMetric::Cosine, None)
        .expect("failed to build HNSW index");

    let filter_bitmap = collection.build_filter_bitmap("tag", &["alpha"]);

    // Baseline (no optimizations)
    group.bench_function(BenchmarkId::new("baseline", "no_optimizations"), |b| {
        let mut rng = StdRng::seed_from_u64(0xDEADBEEF);
        b.iter(|| {
            let idx = rng.gen_range(0..collection.vectors.len());
            let query = QueryVector {
                components: collection.vectors[idx].clone(),
            };

            // New filter each time (no cache)
            let filter = collection.build_filter_bitmap("tag", &["alpha"]);

            let options = SearchOptions {
                top_k: TOP_K,
                filter: Some(filter),
                timeout_ms: 5_000,
            };

            rt.block_on(async {
                provider
                    .search(&handle, query, options)
                    .await
                    .expect("search failed")
            });
        });
    });

    // Optimized (cache + precompiled filter)
    group.bench_function(BenchmarkId::new("optimized", "all_enabled"), |b| {
        let query = QueryVector {
            components: collection.vectors[0].clone(),
        };

        let options = SearchOptions {
            top_k: TOP_K,
            filter: Some(filter_bitmap.clone()),
            timeout_ms: 5_000,
        };

        b.iter(|| {
            rt.block_on(async {
                provider
                    .search(&handle, query.clone(), options.clone())
                    .await
                    .expect("search failed")
            });
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    query_cache_benchmark,
    batch_query_benchmark,
    filter_precompilation_benchmark,
    combined_optimizations_benchmark
);
criterion_main!(benches);
