use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use akidb_benchmarks::{
    create_collection_from_arc, generate_payloads, generate_random_vectors, runtime, Collection,
    DEFAULT_DIMENSION,
};
use akidb_core::collection::DistanceMetric;
use akidb_index::{IndexKind, IndexProvider, NativeIndexProvider};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use once_cell::sync::OnceCell;
use sysinfo::{get_current_pid, ProcessExt, System, SystemExt};

const DATASET_SIZES: [usize; 3] = [10_000, 100_000, 1_000_000];
const INDEX_KINDS: [IndexKind; 1] = [IndexKind::Native];

fn index_build_benchmarks(c: &mut Criterion) {
    let rt = runtime();
    let mut group = c.benchmark_group("index_build");
    group.sample_size(10);
    // Disable HTML plots to avoid Plotters NaN issues with pre-measured builds
    // Use: cargo bench --package akidb-benchmarks --bench index_build -- --noplot

    for &size in DATASET_SIZES.iter() {
        for &kind in INDEX_KINDS.iter() {
            let metrics = cached_index_metrics(size, kind, rt);
            let size_label = format_dataset_size(size);
            let kind_label = index_kind_label(kind);

            println!(
                "index_build/{}/{} => build={:.3}s rebuild={:.3}s rss={:.1}MB size={:.2}MB",
                kind_label,
                size_label,
                metrics.build_time_secs,
                metrics.rebuild_time_secs,
                metrics.peak_rss_mb,
                metrics.index_size_mb,
            );

            let build_metrics = metrics.clone();
            group.bench_function(
                BenchmarkId::new(format!("{}:{}", kind_label, size_label), "build"),
                move |b| {
                    let metrics = build_metrics.clone();
                    b.iter_custom(|iters| {
                        Duration::from_secs_f64(metrics.build_time_secs * iters as f64)
                    });
                },
            );

            let rebuild_metrics = metrics.clone();
            group.bench_function(
                BenchmarkId::new(format!("{}:{}", kind_label, size_label), "rebuild"),
                move |b| {
                    let metrics = rebuild_metrics.clone();
                    b.iter_custom(|iters| {
                        Duration::from_secs_f64(metrics.rebuild_time_secs * iters as f64)
                    });
                },
            );
        }
    }

    group.finish();
}

#[derive(Clone)]
struct BuildMetrics {
    build_time_secs: f64,
    rebuild_time_secs: f64,
    peak_rss_mb: f64,
    index_size_mb: f64,
}

fn cached_index_metrics(
    size: usize,
    kind: IndexKind,
    rt: &tokio::runtime::Runtime,
) -> BuildMetrics {
    static CACHE: OnceCell<Mutex<HashMap<(usize, u8), BuildMetrics>>> = OnceCell::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (size, index_kind_id(kind));

    {
        let guard = cache.lock().unwrap();
        if let Some(metrics) = guard.get(&key).cloned() {
            return metrics;
        }
    }

    let computed = measure_index_build(size, kind, rt).expect("failed to measure index build");

    let mut guard = cache.lock().unwrap();
    guard.insert(key, computed.clone());
    computed
}

fn measure_index_build(
    size: usize,
    kind: IndexKind,
    rt: &tokio::runtime::Runtime,
) -> akidb_core::Result<BuildMetrics> {
    let vectors = dataset_for_size(size);
    let payloads = generate_payloads(size, true);
    let collection_name = format!("index-{}-{}", index_kind_label(kind), size);
    let collection: Collection =
        create_collection_from_arc(Some(collection_name), vectors, payloads);

    let mut sys = System::new_all();
    let mut peak_rss = current_rss_mb(&mut sys);

    let build_start = Instant::now();
    let (provider, handle) = match kind {
        IndexKind::Native => collection.build_native_index(rt, DistanceMetric::Cosine)?,
        IndexKind::Hnsw | IndexKind::Faiss => {
            return Err(akidb_core::Error::NotImplemented(format!(
                "Index kind {:?} not yet supported",
                kind
            )))
        }
    };
    let build_time_secs = build_start.elapsed().as_secs_f64();

    peak_rss = peak_rss.max(current_rss_mb(&mut sys));

    let serialized = provider.serialize(&handle)?;
    let index_size_mb = serialized.len() as f64 / (1024.0 * 1024.0);

    let rebuild_start = Instant::now();
    let rebuild_provider = NativeIndexProvider::new();
    rebuild_provider.deserialize(&serialized)?;
    let rebuild_time_secs = rebuild_start.elapsed().as_secs_f64();
    peak_rss = peak_rss.max(current_rss_mb(&mut sys));

    Ok(BuildMetrics {
        build_time_secs,
        rebuild_time_secs,
        peak_rss_mb: peak_rss,
        index_size_mb,
    })
}

type DatasetCache = HashMap<usize, Arc<Vec<Vec<f32>>>>;

fn dataset_for_size(size: usize) -> Arc<Vec<Vec<f32>>> {
    static CACHE: OnceCell<Mutex<DatasetCache>> = OnceCell::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    {
        let guard = cache.lock().unwrap();
        if let Some(existing) = guard.get(&size).cloned() {
            return existing;
        }
    }

    let generated = Arc::new(generate_random_vectors(size, DEFAULT_DIMENSION));
    let mut guard = cache.lock().unwrap();
    guard.insert(size, Arc::clone(&generated));
    generated
}

fn current_rss_mb(system: &mut System) -> f64 {
    let pid = match get_current_pid() {
        Ok(pid) => pid,
        Err(_) => return 0.0,
    };
    system.refresh_process(pid);
    system
        .process(pid)
        .map(|p| p.memory() as f64 / 1024.0)
        .unwrap_or(0.0)
}

fn index_kind_label(kind: IndexKind) -> &'static str {
    match kind {
        IndexKind::Native => "native",
        IndexKind::Hnsw => "hnsw",
        IndexKind::Faiss => "faiss",
    }
}

fn index_kind_id(kind: IndexKind) -> u8 {
    match kind {
        IndexKind::Faiss => 0,
        IndexKind::Hnsw => 1,
        IndexKind::Native => 2,
    }
}

fn format_dataset_size(size: usize) -> String {
    match size {
        10_000 => "10k".to_string(),
        100_000 => "100k".to_string(),
        1_000_000 => "1m".to_string(),
        other => other.to_string(),
    }
}

criterion_group!(benches, index_build_benchmarks);
criterion_main!(benches);
