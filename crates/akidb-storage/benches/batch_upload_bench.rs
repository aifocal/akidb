//! Benchmark for batch upload performance
//!
//! Target: >500 ops/sec (2.5x improvement over sequential baseline)

use akidb_core::ids::CollectionId;
use akidb_core::vector::VectorDocument;
use akidb_storage::batch_config::S3BatchConfig;
use akidb_storage::batch_uploader::BatchUploader;
use akidb_storage::object_store::{LocalObjectStore, ObjectStore};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::path::PathBuf;
use std::sync::Arc;

fn bench_batch_vs_sequential(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("batch_upload");

    for upload_count in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*upload_count as u64));

        // Sequential uploads (baseline)
        group.bench_with_input(
            BenchmarkId::new("sequential", upload_count),
            upload_count,
            |b, &count| {
                b.to_async(&rt).iter(|| async {
                    let temp_dir = tempfile::tempdir().unwrap();
                    let store = Arc::new(LocalObjectStore::new(PathBuf::from(temp_dir.path())));
                    let collection_id = CollectionId::new();

                    for i in 0..count {
                        let key = format!("tenant1/db1/{}/snap{}.parquet", collection_id, i);
                        let data = vec![0u8; 1024]; // 1KB snapshot
                        store.put(&key, data.into()).await.unwrap();
                    }
                });
            },
        );

        // Batch uploads
        group.bench_with_input(
            BenchmarkId::new("batch", upload_count),
            upload_count,
            |b, &count| {
                b.to_async(&rt).iter(|| async {
                    let temp_dir = tempfile::tempdir().unwrap();
                    let store = Arc::new(LocalObjectStore::new(PathBuf::from(temp_dir.path())));

                    let config = S3BatchConfig {
                        enabled: true,
                        batch_size: 10,
                        max_wait_ms: 5000,
                    };

                    let uploader = BatchUploader::new(store, config).unwrap();
                    let collection_id = CollectionId::new();

                    for i in 0..count {
                        let doc = VectorDocument::new(vec![0.1; 128]);
                        uploader
                            .add_document(collection_id, 128, doc)
                            .await
                            .unwrap();
                    }

                    // Flush remaining
                    uploader.flush_collection(collection_id).await.unwrap();
                });
            },
        );
    }

    group.finish();
}

fn bench_batch_sizes(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("batch_sizes");
    group.throughput(Throughput::Elements(100));

    for batch_size in [5, 10, 20, 50].iter() {
        group.bench_with_input(
            BenchmarkId::new("batch_size", batch_size),
            batch_size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let temp_dir = tempfile::tempdir().unwrap();
                    let store = Arc::new(LocalObjectStore::new(PathBuf::from(temp_dir.path())));

                    let config = S3BatchConfig {
                        enabled: true,
                        batch_size: size,
                        max_wait_ms: 5000,
                    };

                    let uploader = BatchUploader::new(store, config).unwrap();
                    let collection_id = CollectionId::new();

                    for _ in 0..100 {
                        let doc = VectorDocument::new(vec![0.1; 128]);
                        uploader
                            .add_document(collection_id, 128, doc)
                            .await
                            .unwrap();
                    }

                    uploader.flush_collection(collection_id).await.unwrap();
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_batch_vs_sequential, bench_batch_sizes);
criterion_main!(benches);
