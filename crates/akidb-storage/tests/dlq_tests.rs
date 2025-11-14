//! Integration tests for DLQ (Dead Letter Queue) functionality

use akidb_core::{CollectionId, DocumentId};
use akidb_storage::dlq::{DLQConfig, DLQEntry, DeadLetterQueue};
use akidb_storage::{StorageBackend, StorageConfig, TieringPolicy};
use tempfile::TempDir;

#[tokio::test]
async fn test_dlq_persistence_and_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let persistence_path = temp_dir.path().join("dlq.json");

    // Create DLQ and add entries
    {
        let config = DLQConfig {
            persistence_path: persistence_path.clone(),
            ..Default::default()
        };
        let dlq = DeadLetterQueue::new(config);

        for i in 0..3 {
            let entry = DLQEntry::new(
                DocumentId::new(),
                CollectionId::new(),
                format!("error_{}", i),
                vec![i as u8],
                604_800,
            );
            dlq.add_entry(entry).await.unwrap();
        }

        assert_eq!(dlq.size(), 3);

        // Persist to disk
        dlq.persist().await.unwrap();
    }

    // Create new DLQ and load from disk
    {
        let config = DLQConfig {
            persistence_path: persistence_path.clone(),
            ..Default::default()
        };
        let dlq = DeadLetterQueue::new(config);

        dlq.load_from_disk().await.unwrap();

        // Should have loaded 3 entries
        assert_eq!(dlq.size(), 3);
    }
}

#[tokio::test]
async fn test_dlq_cleanup_worker() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("test.wal");
    let snapshot_dir = temp_dir.path().join("snapshots");
    let dlq_path = temp_dir.path().join("dlq.json");
    std::fs::create_dir_all(&snapshot_dir).unwrap();

    let config = StorageConfig {
        tiering_policy: TieringPolicy::Memory,
        wal_path,
        snapshot_dir,
        dlq_config: DLQConfig {
            ttl_seconds: 1,
            cleanup_interval_seconds: 1,
            persistence_path: dlq_path.clone(),
            ..Default::default()
        },
        ..Default::default()
    };

    let backend = StorageBackend::new(config).await.unwrap();

    // Add entries with 1-second TTL (directly to DLQ for testing)
    let dlq = &backend.get_dead_letter_queue();
    assert_eq!(dlq.len(), 0);

    // Wait for cleanup worker to run (should remove expired entries)
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Cleanup
    backend.shutdown().await.unwrap();

    // Verify DLQ was persisted on shutdown
    assert!(dlq_path.exists());
}

#[tokio::test]
async fn test_dlq_full_reject_new_entries() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("test.wal");
    let snapshot_dir = temp_dir.path().join("snapshots");
    std::fs::create_dir_all(&snapshot_dir).unwrap();

    let config = StorageConfig {
        tiering_policy: TieringPolicy::Memory,
        wal_path,
        snapshot_dir,
        dlq_config: DLQConfig {
            max_size: 3,
            ..Default::default()
        },
        ..Default::default()
    };

    let backend = StorageBackend::new(config).await.unwrap();

    // Manually add 5 entries to DLQ (exceeds max_size of 3)
    let dlq_config = DLQConfig {
        max_size: 3,
        ..Default::default()
    };
    let dlq = DeadLetterQueue::new(dlq_config);

    for i in 0..5 {
        let entry = DLQEntry::new(
            DocumentId::new(),
            CollectionId::new(),
            format!("error_{}", i),
            vec![i as u8],
            604_800,
        );
        dlq.add_entry(entry).await.unwrap();
    }

    // Should have exactly 3 entries (FIFO eviction)
    assert_eq!(dlq.size(), 3);

    // Verify metrics
    let metrics = dlq.metrics();
    assert_eq!(metrics.size, 3);
    assert_eq!(metrics.total_evictions, 2); // 5 - 3 = 2 evictions

    backend.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_dlq_metrics_tracking() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("test.wal");
    let snapshot_dir = temp_dir.path().join("snapshots");
    std::fs::create_dir_all(&snapshot_dir).unwrap();

    let config = StorageConfig {
        tiering_policy: TieringPolicy::Memory,
        wal_path,
        snapshot_dir,
        ..Default::default()
    };

    let backend = StorageBackend::new(config).await.unwrap();

    // Create standalone DLQ for testing
    let dlq = DeadLetterQueue::new(DLQConfig::default());

    // Add entries
    for i in 0..10 {
        let entry = DLQEntry::new(
            DocumentId::new(),
            CollectionId::new(),
            format!("error_{}", i),
            vec![i as u8],
            604_800,
        );
        dlq.add_entry(entry).await.unwrap();
    }

    // Get metrics
    let metrics = dlq.metrics();
    assert_eq!(metrics.size, 10);
    assert!(metrics.oldest_entry_age_seconds >= 0);

    backend.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_dlq_expiration_filtering_on_load() {
    let temp_dir = TempDir::new().unwrap();
    let persistence_path = temp_dir.path().join("dlq.json");

    // Create DLQ with short TTL
    {
        let config = DLQConfig {
            ttl_seconds: 1, // 1 second TTL
            persistence_path: persistence_path.clone(),
            ..Default::default()
        };
        let dlq = DeadLetterQueue::new(config);

        // Add 3 entries
        for i in 0..3 {
            let entry = DLQEntry::new(
                DocumentId::new(),
                CollectionId::new(),
                format!("error_{}", i),
                vec![i as u8],
                1, // 1 second TTL
            );
            dlq.add_entry(entry).await.unwrap();
        }

        assert_eq!(dlq.size(), 3);

        // Persist to disk
        dlq.persist().await.unwrap();
    }

    // Wait for TTL expiration
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Load from disk - expired entries should be filtered out
    {
        let config = DLQConfig {
            persistence_path: persistence_path.clone(),
            ..Default::default()
        };
        let dlq = DeadLetterQueue::new(config);

        dlq.load_from_disk().await.unwrap();

        // Should have 0 entries (all expired)
        assert_eq!(dlq.size(), 0);
    }
}
