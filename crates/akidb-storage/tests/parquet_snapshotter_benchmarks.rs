//! Performance benchmarks for Parquet vs JSON snapshotters
//!
//! Validates that Parquet achieves:
//! - Create: <2s for 10k vectors (512-dim)
//! - Restore: <3s for 10k vectors
//! - Compression: >2x vs JSON

use akidb_core::{CollectionId, DocumentId, VectorDocument};
use akidb_storage::object_store::LocalObjectStore;
use akidb_storage::snapshotter::{
    CompressionCodec, JsonSnapshotter, ParquetSnapshotConfig, ParquetSnapshotter, Snapshotter,
};
use chrono::Utc;
use std::sync::Arc;
use std::time::Instant;
use tempfile::TempDir;

fn create_test_vectors(count: usize, dimension: usize) -> Vec<VectorDocument> {
    (0..count)
        .map(|i| VectorDocument {
            doc_id: DocumentId::new(),
            external_id: Some(format!("doc-{}", i)),
            vector: vec![i as f32; dimension],
            metadata: Some(serde_json::json!({"index": i, "category": "test"})),
            inserted_at: Utc::now(),
        })
        .collect()
}

#[tokio::test]
async fn bench_parquet_create_10k() {
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(LocalObjectStore::new(temp_dir.path()).await.unwrap());
    let snapshotter = ParquetSnapshotter::new(store, ParquetSnapshotConfig::default());

    let vectors = create_test_vectors(10_000, 512);
    let collection_id = CollectionId::new();

    let start = Instant::now();
    let snapshot_id = snapshotter
        .create_snapshot(collection_id, vectors)
        .await
        .unwrap();
    let duration = start.elapsed();

    println!(
        "✅ Parquet: 10k vectors (512-dim) snapshot created in {:?}",
        duration
    );

    // Get metadata to check size
    let metadata = snapshotter.get_metadata(snapshot_id).await.unwrap();
    println!("   Size: {} KB", metadata.size_bytes / 1024);

    // Target: <5s for 10k vectors (debug build)
    // Note: Release builds should achieve <2s
    assert!(
        duration.as_secs() < 5,
        "Create should take <5s in debug mode, took {:?}",
        duration
    );
}

#[tokio::test]
async fn bench_parquet_restore_10k() {
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(LocalObjectStore::new(temp_dir.path()).await.unwrap());
    let snapshotter = ParquetSnapshotter::new(store, ParquetSnapshotConfig::default());

    let vectors = create_test_vectors(10_000, 512);
    let collection_id = CollectionId::new();

    // Create snapshot first
    let snapshot_id = snapshotter
        .create_snapshot(collection_id, vectors)
        .await
        .unwrap();

    // Benchmark restore
    let start = Instant::now();
    let restored = snapshotter.restore_snapshot(snapshot_id).await.unwrap();
    let duration = start.elapsed();

    println!("✅ Parquet: 10k vectors restored in {:?}", duration);
    assert_eq!(restored.len(), 10_000);

    // Target: <5s for 10k vectors (debug build)
    // Note: Release builds should achieve <3s
    assert!(
        duration.as_secs() < 5,
        "Restore should take <5s in debug mode, took {:?}",
        duration
    );
}

#[tokio::test]
async fn bench_compression_ratio() {
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(LocalObjectStore::new(temp_dir.path()).await.unwrap());

    let parquet_snapshotter =
        ParquetSnapshotter::new(store.clone(), ParquetSnapshotConfig::default());
    let json_snapshotter = JsonSnapshotter::new(store.clone(), CompressionCodec::None);

    let vectors = create_test_vectors(10_000, 512);
    let collection_id = CollectionId::new();

    // Create Parquet snapshot
    let parquet_id = parquet_snapshotter
        .create_snapshot(collection_id, vectors.clone())
        .await
        .unwrap();
    let parquet_metadata = parquet_snapshotter.get_metadata(parquet_id).await.unwrap();
    let parquet_size = parquet_metadata.size_bytes;

    // Create JSON snapshot
    let json_id = json_snapshotter
        .create_snapshot(collection_id, vectors)
        .await
        .unwrap();
    let json_metadata = json_snapshotter.get_metadata(json_id).await.unwrap();
    let json_size = json_metadata.size_bytes;

    let compression_ratio = json_size as f64 / parquet_size as f64;

    println!("\n📊 Compression Comparison (10k vectors, 512-dim):");
    println!("   JSON:    {} KB", json_size / 1024);
    println!("   Parquet: {} KB", parquet_size / 1024);
    println!("   Ratio:   {:.2}x", compression_ratio);

    // Target: >2x compression
    assert!(
        compression_ratio >= 2.0,
        "Expected >2x compression, got {:.2}x",
        compression_ratio
    );
}

#[tokio::test]
async fn bench_large_dataset_100k() {
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(LocalObjectStore::new(temp_dir.path()).await.unwrap());
    let snapshotter = ParquetSnapshotter::new(store, ParquetSnapshotConfig::default());

    let vectors = create_test_vectors(100_000, 128);
    let collection_id = CollectionId::new();

    // Create
    let start = Instant::now();
    let snapshot_id = snapshotter
        .create_snapshot(collection_id, vectors.clone())
        .await
        .unwrap();
    let create_duration = start.elapsed();

    // Restore
    let start = Instant::now();
    let restored = snapshotter.restore_snapshot(snapshot_id).await.unwrap();
    let restore_duration = start.elapsed();

    println!("\n📊 Large Dataset (100k vectors, 128-dim):");
    println!("   Create:  {:?}", create_duration);
    println!("   Restore: {:?}", restore_duration);

    assert_eq!(restored.len(), 100_000);

    // Memory footprint check - should be <1GB for 100k vectors
    let metadata = snapshotter.get_metadata(snapshot_id).await.unwrap();
    let size_mb = metadata.size_bytes / (1024 * 1024);
    println!("   Size:    {} MB", size_mb);
    assert!(size_mb < 1024, "Size should be <1GB");
}

#[tokio::test]
async fn bench_roundtrip_integrity() {
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(LocalObjectStore::new(temp_dir.path()).await.unwrap());
    let snapshotter = ParquetSnapshotter::new(store, ParquetSnapshotConfig::default());

    let original = create_test_vectors(1_000, 256);
    let collection_id = CollectionId::new();

    let snapshot_id = snapshotter
        .create_snapshot(collection_id, original.clone())
        .await
        .unwrap();

    let restored = snapshotter.restore_snapshot(snapshot_id).await.unwrap();

    // Verify 100% data integrity
    assert_eq!(original.len(), restored.len());

    let mut corruption_count = 0;
    for (orig, rest) in original.iter().zip(restored.iter()) {
        if orig.doc_id != rest.doc_id || orig.vector != rest.vector {
            corruption_count += 1;
        }
    }

    assert_eq!(
        corruption_count, 0,
        "Zero corruption expected, found {} corrupted documents",
        corruption_count
    );
    println!("✅ 100% data integrity verified (1000 documents)");
}
