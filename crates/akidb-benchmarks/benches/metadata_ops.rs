use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use akidb_benchmarks::{
    create_collection_from_arc, generate_payloads, generate_random_vectors, Collection,
    DEFAULT_DIMENSION,
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use once_cell::sync::OnceCell;

const DATASET_SIZES: [usize; 3] = [10_000, 100_000, 1_000_000];

fn metadata_ops_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("metadata_ops");
    group.sample_size(15);

    for &size in DATASET_SIZES.iter() {
        let collection = collection_for_size(size);
        let size_label = format_dataset_size(size);

        group.bench_function(BenchmarkId::new(size_label.clone(), "build_filter"), |b| {
            let collection = collection.clone();
            b.iter(|| {
                let bitmap = collection.build_filter_bitmap("tag", &["alpha", "beta"]);
                black_box(bitmap)
            });
        });

        group.bench_function(BenchmarkId::new(size_label.clone(), "filter_count"), |b| {
            let collection = collection.clone();
            b.iter(|| {
                let bitmap = collection.build_filter_bitmap("tag", &["gamma"]);
                black_box(bitmap.len())
            });
        });
    }

    group.finish();
}

fn collection_for_size(size: usize) -> Collection {
    static CACHE: OnceCell<Mutex<HashMap<usize, Collection>>> = OnceCell::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    {
        // BUGFIX: Handle poisoned mutex gracefully in benchmarks
        let guard = cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(existing) = guard.get(&size).cloned() {
            return existing;
        }
    }

    let vectors = Arc::new(generate_random_vectors(size, DEFAULT_DIMENSION));
    let payloads = generate_payloads(size, true);
    let collection = create_collection_from_arc(None, vectors, payloads);

    let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    guard.insert(size, collection.clone());
    collection
}

fn format_dataset_size(size: usize) -> String {
    match size {
        10_000 => "10k".to_string(),
        100_000 => "100k".to_string(),
        1_000_000 => "1m".to_string(),
        other => other.to_string(),
    }
}

criterion_group!(benches, metadata_ops_benchmarks);
criterion_main!(benches);
