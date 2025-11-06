//! Performance benchmarks for vector index implementations.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::Rng;

use akidb_core::{DistanceMetric, DocumentId, VectorDocument, VectorIndex};
use akidb_index::BruteForceIndex;

fn generate_random_vector(dim: usize) -> Vec<f32> {
    let mut rng = rand::thread_rng();
    (0..dim).map(|_| rng.gen::<f32>()).collect()
}

fn bench_brute_force_search_1k(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let index = BruteForceIndex::new(512, DistanceMetric::Cosine);

    // Insert 1k vectors
    runtime.block_on(async {
        for _ in 0..1_000 {
            let vec = generate_random_vector(512);
            let doc = VectorDocument::new(DocumentId::new(), vec);
            index.insert(doc).await.unwrap();
        }
    });

    let query = generate_random_vector(512);

    c.bench_function("brute_force_search_1k_512d", |b| {
        b.to_async(&runtime).iter(|| async {
            let results = index.search(black_box(&query), 10, None).await.unwrap();
            black_box(results);
        });
    });
}

fn bench_brute_force_search_10k(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let index = BruteForceIndex::new(512, DistanceMetric::Cosine);

    // Insert 10k vectors
    runtime.block_on(async {
        for _ in 0..10_000 {
            let vec = generate_random_vector(512);
            let doc = VectorDocument::new(DocumentId::new(), vec);
            index.insert(doc).await.unwrap();
        }
    });

    let query = generate_random_vector(512);

    c.bench_function("brute_force_search_10k_512d", |b| {
        b.to_async(&runtime).iter(|| async {
            let results = index.search(black_box(&query), 10, None).await.unwrap();
            black_box(results);
        });
    });
}

fn bench_brute_force_insert(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("brute_force_insert_512d", |b| {
        let index = BruteForceIndex::new(512, DistanceMetric::Cosine);
        let vec = generate_random_vector(512);

        b.to_async(&runtime).iter(|| async {
            let doc = VectorDocument::new(DocumentId::new(), vec.clone());
            index.insert(black_box(doc)).await.unwrap();
        });
    });
}

criterion_group!(
    benches,
    bench_brute_force_search_1k,
    bench_brute_force_search_10k,
    bench_brute_force_insert
);
criterion_main!(benches);
