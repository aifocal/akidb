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
use sysinfo::{get_current_pid, ProcessExt, System, SystemExt};

const DATASET_SIZES: [usize; 3] = [10_000, 100_000, 1_000_000];
const TOP_K_VALUES: [u16; 3] = [10, 50, 100];
const DISTANCE_METRICS: [DistanceMetric; 3] = [
    DistanceMetric::Cosine,
    DistanceMetric::L2,
    DistanceMetric::Dot,
];

fn vector_search_benchmarks(c: &mut Criterion) {
    let rt = runtime();
    let mut group = c.benchmark_group("vector_search");
    group.sample_size(10);

    for &size in DATASET_SIZES.iter() {
        let size_label = format_dataset_size(size);
        let vectors = Arc::new(generate_random_vectors(size, DEFAULT_DIMENSION));

        run_vector_search_scenarios(&mut group, &size_label, Arc::clone(&vectors), true, rt);

        run_vector_search_scenarios(&mut group, &size_label, Arc::clone(&vectors), false, rt);
    }

    group.finish();
}

fn run_vector_search_scenarios(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    size_label: &str,
    vectors: Arc<Vec<Vec<f32>>>,
    with_filters: bool,
    rt: &tokio::runtime::Runtime,
) {
    let payloads = generate_payloads(vectors.len(), with_filters);
    let name_suffix = if with_filters {
        "with-filter"
    } else {
        "no-filter"
    };
    let collection_name = format!("search-{}-{}", size_label, name_suffix);
    let collection: Collection =
        create_collection_from_arc(Some(collection_name), vectors, payloads);

    for &metric in DISTANCE_METRICS.iter() {
        let (base_provider, base_handle) = collection
            .build_native_index(rt, metric)
            .expect("failed to build index");

        let base_filter = if with_filters {
            Some(collection.build_filter_bitmap("tag", &["alpha", "beta"]))
        } else {
            None
        };

        for &top_k in TOP_K_VALUES.iter() {
            let provider = Arc::clone(&base_provider);
            let handle = base_handle.clone();
            let filter_bitmap = base_filter.clone();
            let collection = collection.clone();

            let bench_id = BenchmarkId::new(
                format!(
                    "{}/k={}/{}{}",
                    size_label,
                    top_k,
                    metric_label(metric),
                    if with_filters { "_with_filter" } else { "" }
                ),
                "vector_search",
            );

            group.bench_function(bench_id, move |b| {
                let latencies = Arc::new(Mutex::new(Vec::<f64>::new()));
                let mut sys = System::new_all();
                let mut peak_rss = current_rss_mb(&mut sys);
                let query_rng = Arc::new(Mutex::new(StdRng::seed_from_u64(
                    0xCAFE_BABE ^ (collection.vectors.len() as u64)
                        ^ (top_k as u64)
                        ^ (metric as u8 as u64)
                        ^ if with_filters { 1 } else { 0 },
                )));

                peak_rss = peak_rss.max(current_rss_mb(&mut sys));

                b.iter_custom(|iters| {
                    let mut total = Duration::ZERO;

                    for _ in 0..iters {
                        let start = Instant::now();
                        let idx = {
                            let mut rng = query_rng.lock().unwrap();
                            rng.gen_range(0..collection.vectors.len())
                        };

                        let query = QueryVector {
                            components: collection.vectors[idx].clone(),
                        };

                        let options = SearchOptions {
                            top_k,
                            filter: filter_bitmap.clone(),
                            timeout_ms: 5_000,
                        };

                        rt.block_on(async {
                            provider
                                .search(&handle, query, options)
                                .await
                                .expect("search failed")
                        });

                        let elapsed = start.elapsed();
                        total += elapsed;
                        latencies
                            .lock()
                            .unwrap()
                            .push(elapsed.as_secs_f64() * 1_000.0);
                    }

                    peak_rss = peak_rss.max(current_rss_mb(&mut sys));
                    total
                });

                let latency_values = Arc::try_unwrap(latencies)
                    .expect("latency samples still borrowed")
                    .into_inner()
                    .expect("mutex poisoned");

                let stats = compute_stats(&latency_values);
                let throughput = if stats.total_ms > 0.0 {
                    (latency_values.len() as f64) / (stats.total_ms / 1_000.0)
                } else {
                    0.0
                };

                println!(
                    "vector_search/{}/k={}/{}{} => p50={:.3}ms p95={:.3}ms p99={:.3}ms throughput={:.2} QPS rss={:.1} MB",
                    size_label,
                    top_k,
                    metric_label(metric),
                    if with_filters { "_with_filter" } else { "" },
                    stats.p50,
                    stats.p95,
                    stats.p99,
                    throughput,
                    peak_rss,
                );
            });
        }
    }
}

fn compute_stats(latencies_ms: &[f64]) -> LatencyStats {
    if latencies_ms.is_empty() {
        return LatencyStats::default();
    }

    let mut data = latencies_ms.to_vec();
    data.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let p50 = percentile(&data, 0.50);
    let p95 = percentile(&data, 0.95);
    let p99 = percentile(&data, 0.99);
    let total_ms: f64 = latencies_ms.iter().sum();

    LatencyStats {
        p50,
        p95,
        p99,
        total_ms,
    }
}

fn percentile(sorted: &[f64], quantile: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }

    let pos = quantile * (sorted.len() as f64 - 1.0);
    let low = pos.floor() as usize;
    let high = pos.ceil() as usize;

    if low == high {
        sorted[low]
    } else {
        let weight = pos - low as f64;
        sorted[low] + (sorted[high] - sorted[low]) * weight
    }
}

#[derive(Default)]
struct LatencyStats {
    p50: f64,
    p95: f64,
    p99: f64,
    total_ms: f64,
}

fn current_rss_mb(system: &mut System) -> f64 {
    let pid = match get_current_pid() {
        Ok(pid) => pid,
        Err(_) => return 0.0,
    };

    system.refresh_process(pid);
    if let Some(process) = system.process(pid) {
        process.memory() as f64 / 1024.0
    } else {
        0.0
    }
}

fn metric_label(metric: DistanceMetric) -> &'static str {
    match metric {
        DistanceMetric::Cosine => "cosine",
        DistanceMetric::L2 => "l2",
        DistanceMetric::Dot => "dot",
    }
}

fn format_dataset_size(size: usize) -> String {
    match size {
        10_000 => "10k".to_string(),
        100_000 => "100k".to_string(),
        1_000_000 => "1m".to_string(),
        other => format!("{}", other),
    }
}

criterion_group!(benches, vector_search_benchmarks);
criterion_main!(benches);
