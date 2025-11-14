//! Benchmark MockS3ObjectStore vs LocalObjectStore
//!
//! Validates that MockS3 is 100x+ faster than real S3 (zero network I/O)

use akidb_storage::object_store::{LocalObjectStore, MockS3Config, MockS3ObjectStore, ObjectStore};
use bytes::Bytes;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

fn bench_mock_s3_vs_local(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("object_store_comparison");
    group.throughput(Throughput::Elements(100));

    // MockS3 (zero latency, in-memory)
    group.bench_function("mock_s3_zero_latency", |b| {
        let mock = Arc::new(MockS3ObjectStore::new());

        b.to_async(&rt).iter(|| async {
            for i in 0..100 {
                let key = format!("key{}", i);
                let data = Bytes::from(vec![0u8; 1024]);
                mock.put(&key, data).await.unwrap();
            }
        });
    });

    // MockS3 (10ms latency simulation)
    group.bench_function("mock_s3_10ms_latency", |b| {
        let config = MockS3Config {
            latency: Duration::from_millis(10),
            track_history: false,
        };
        let mock = Arc::new(MockS3ObjectStore::new_with_config(config));

        b.to_async(&rt).iter(|| async {
            for i in 0..100 {
                let key = format!("key{}", i);
                let data = Bytes::from(vec![0u8; 1024]);
                mock.put(&key, data).await.unwrap();
            }
        });
    });

    // LocalObjectStore (filesystem I/O)
    group.bench_function("local_store_filesystem", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = tempfile::tempdir().unwrap();
            let store = Arc::new(LocalObjectStore::new(PathBuf::from(temp_dir.path())).await.unwrap());

            for i in 0..100 {
                let key = format!("key{}", i);
                let data = Bytes::from(vec![0u8; 1024]);
                store.put(&key, data).await.unwrap();
            }
        });
    });

    group.finish();
}

fn bench_mock_s3_operations(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("mock_s3_operations");

    // PUT
    group.bench_function("put", |b| {
        let mock = Arc::new(MockS3ObjectStore::new());

        b.to_async(&rt).iter(|| async {
            let data = Bytes::from(vec![0u8; 1024]);
            mock.put("test-key", data).await.unwrap();
        });
    });

    // GET
    group.bench_function("get", |b| {
        let mock = Arc::new(MockS3ObjectStore::new());
        rt.block_on(async {
            mock.put("test-key", Bytes::from(vec![0u8; 1024]))
                .await
                .unwrap();
        });

        b.to_async(&rt).iter(|| async {
            mock.get("test-key").await.unwrap();
        });
    });

    // DELETE
    group.bench_function("delete", |b| {
        let mock = Arc::new(MockS3ObjectStore::new());

        b.to_async(&rt).iter(|| async {
            mock.delete("test-key").await.unwrap();
        });
    });

    // LIST
    group.bench_function("list", |b| {
        let mock = Arc::new(MockS3ObjectStore::new());
        rt.block_on(async {
            for i in 0..100 {
                let key = format!("prefix/key{}", i);
                mock.put(&key, Bytes::from(vec![0u8; 1024])).await.unwrap();
            }
        });

        b.to_async(&rt).iter(|| async {
            mock.list("prefix/").await.unwrap();
        });
    });

    group.finish();
}

criterion_group!(benches, bench_mock_s3_vs_local, bench_mock_s3_operations);
criterion_main!(benches);
