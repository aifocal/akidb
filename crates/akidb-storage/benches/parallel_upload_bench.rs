//! Benchmark for parallel upload performance
//!
//! Target: >600 ops/sec (3x improvement over sequential baseline)

use akidb_core::ids::{CollectionId, DocumentId};
use akidb_core::vector::VectorDocument;
use akidb_storage::batch_config::S3BatchConfig;
use akidb_storage::object_store::{LocalObjectStore, ObjectStore};
use akidb_storage::parallel_uploader::{ParallelConfig, ParallelUploader};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::path::PathBuf;
use std::sync::Arc;

fn bench_parallel_vs_sequential(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("parallel_upload");

    for upload_count in [100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*upload_count as u64));

        // Sequential uploads (baseline)
        group.bench_with_input(
            BenchmarkId::new("sequential", upload_count),
            upload_count,
            |b, &count| {
                b.to_async(&rt).iter(|| async {
                    let temp_dir = tempfile::tempdir().unwrap();
                    let store = Arc::new(LocalObjectStore::new(PathBuf::from(temp_dir.path())).await.unwrap());
                    let collection_id = CollectionId::new();

                    for i in 0..count {
                        let key = format!("tenant1/db1/{}/snap{}.parquet", collection_id, i);
                        let data = vec![0u8; 1024]; // 1KB snapshot
                        store.put(&key, data.into()).await.unwrap();
                    }
                });
            },
        );

        // Parallel uploads (concurrency=10)
        group.bench_with_input(
            BenchmarkId::new("parallel_10", upload_count),
            upload_count,
            |b, &count| {
                b.to_async(&rt).iter(|| async {
                    let temp_dir = tempfile::tempdir().unwrap();
                    let store = Arc::new(LocalObjectStore::new(PathBuf::from(temp_dir.path())).await.unwrap());

                    let config = ParallelConfig {
                        batch: S3BatchConfig {
                            batch_size: 10,
                            max_wait_ms: 5000,
                            enable_compression: true,
                        },
                        max_concurrency: 10,
                    };

                    let uploader = ParallelUploader::new(store, config).unwrap();
                    let collection_id = CollectionId::new();

                    for _ in 0..count {
                        let doc = VectorDocument::new(DocumentId::new(), vec![0.1; 128]);
                        uploader
                            .add_document(collection_id, 128, doc)
                            .await
                            .unwrap();
                    }

                    uploader.flush_all_parallel().await.unwrap();
                });
            },
        );

        // Parallel uploads (concurrency=20)
        group.bench_with_input(
            BenchmarkId::new("parallel_20", upload_count),
            upload_count,
            |b, &count| {
                b.to_async(&rt).iter(|| async {
                    let temp_dir = tempfile::tempdir().unwrap();
                    let store = Arc::new(LocalObjectStore::new(PathBuf::from(temp_dir.path())).await.unwrap());

                    let config = ParallelConfig {
                        batch: S3BatchConfig {
                            batch_size: 10,
                            max_wait_ms: 5000,
                            enable_compression: true,
                        },
                        max_concurrency: 20,
                    };

                    let uploader = ParallelUploader::new(store, config).unwrap();
                    let collection_id = CollectionId::new();

                    for _ in 0..count {
                        let doc = VectorDocument::new(DocumentId::new(), vec![0.1; 128]);
                        uploader
                            .add_document(collection_id, 128, doc)
                            .await
                            .unwrap();
                    }

                    uploader.flush_all_parallel().await.unwrap();
                });
            },
        );
    }

    group.finish();
}

fn bench_concurrency_levels(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("concurrency_levels");
    group.throughput(Throughput::Elements(500));

    for concurrency in [1, 5, 10, 20, 50].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrency", concurrency),
            concurrency,
            |b, &level| {
                b.to_async(&rt).iter(|| async {
                    let temp_dir = tempfile::tempdir().unwrap();
                    let store = Arc::new(LocalObjectStore::new(PathBuf::from(temp_dir.path())).await.unwrap());

                    let config = ParallelConfig {
                        batch: S3BatchConfig {
                            batch_size: 10,
                            max_wait_ms: 5000,
                            enable_compression: true,
                        },
                        max_concurrency: level,
                    };

                    let uploader = ParallelUploader::new(store, config).unwrap();
                    let collection_id = CollectionId::new();

                    for _ in 0..500 {
                        let doc = VectorDocument::new(DocumentId::new(), vec![0.1; 128]);
                        uploader
                            .add_document(collection_id, 128, doc)
                            .await
                            .unwrap();
                    }

                    uploader.flush_all_parallel().await.unwrap();
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_parallel_vs_sequential,
    bench_concurrency_levels
);
criterion_main!(benches);
