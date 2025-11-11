//! File-based Write-Ahead Log implementation
//!
//! Stores WAL entries as JSON lines in append-only log files with fsync for durability.
//! Supports automatic rotation, crash recovery, and old file cleanup.

use super::{LogEntry, LogSequenceNumber, WriteAheadLog};
use akidb_core::CoreResult;
use async_trait::async_trait;
use parking_lot::RwLock;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Configuration for FileWAL
#[derive(Debug, Clone)]
pub struct FileWALConfig {
    /// Maximum log file size in bytes before rotation (default: 100MB)
    pub max_file_size_bytes: u64,

    /// fsync on every write for durability (default: true)
    /// Set to false for higher performance but risk of data loss on crash
    pub sync_on_write: bool,

    /// Number of old log files to retain after checkpoint (default: 10)
    pub retention_count: usize,
}

impl Default for FileWALConfig {
    fn default() -> Self {
        Self {
            max_file_size_bytes: 100 * 1024 * 1024, // 100MB
            sync_on_write: true,
            retention_count: 10,
        }
    }
}

/// File-based WAL implementation
///
/// # File Format
/// - Filename: `wal-{lsn_hex}.log` where lsn_hex is the starting LSN in hex
/// - Content: JSON lines, one entry per line: `(LSN, LogEntry)`
/// - Encoding: UTF-8
///
/// # Crash Recovery
/// On startup, scans all WAL files to find the highest LSN and checkpoint LSN.
/// `replay()` reads all entries >= from_lsn across all files.
///
/// # Thread Safety
/// Uses `parking_lot::RwLock` for concurrent access. Writes are serialized,
/// reads (replay) can happen concurrently.
pub struct FileWAL {
    /// Base directory for WAL files
    dir: PathBuf,

    /// Current active log file (wrapped for thread-safe writes)
    current_file: Arc<RwLock<BufWriter<File>>>,

    /// Current LSN counter
    current_lsn: Arc<RwLock<LogSequenceNumber>>,

    /// Last checkpoint LSN (entries before this can be discarded)
    checkpoint_lsn: Arc<RwLock<LogSequenceNumber>>,

    /// Configuration
    config: FileWALConfig,

    /// Path to current log file
    current_log_path: Arc<RwLock<PathBuf>>,
}

impl FileWAL {
    /// Create a new FileWAL
    ///
    /// # Arguments
    /// * `dir` - Directory to store WAL files
    /// * `config` - WAL configuration
    ///
    /// # Errors
    /// Returns error if directory creation fails or existing WAL files are corrupted
    pub async fn new(dir: impl AsRef<Path>, config: FileWALConfig) -> CoreResult<Self> {
        let dir = dir.as_ref().to_path_buf();

        // Create directory if not exists
        tokio::fs::create_dir_all(&dir).await?;

        // Recover state from existing WAL files
        let (current_lsn, checkpoint_lsn) = Self::recover_state(&dir).await?;

        // Open or create current log file
        let log_path = dir.join(format!("wal-{:016x}.log", current_lsn.value()));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;

        let current_file = Arc::new(RwLock::new(BufWriter::new(file)));

        Ok(Self {
            dir,
            current_file,
            current_lsn: Arc::new(RwLock::new(current_lsn)),
            checkpoint_lsn: Arc::new(RwLock::new(checkpoint_lsn)),
            config,
            current_log_path: Arc::new(RwLock::new(log_path)),
        })
    }

    /// Recover LSN state from existing WAL files
    ///
    /// Scans directory for `wal-*.log` files and determines:
    /// - Highest LSN seen (for continuing sequence)
    /// - Latest checkpoint LSN (for cleanup)
    async fn recover_state(dir: &Path) -> CoreResult<(LogSequenceNumber, LogSequenceNumber)> {
        let mut max_lsn = LogSequenceNumber::ZERO;
        let mut checkpoint_lsn = LogSequenceNumber::ZERO;

        // Find all WAL files
        let mut entries = tokio::fs::read_dir(dir).await?;
        let mut all_files = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "log") {
                all_files.push(path);
            }
        }

        // Process each file
        for path in all_files {
            // Parse starting LSN from filename: wal-{lsn}.log
            if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
                if let Some(lsn_str) = file_stem.strip_prefix("wal-") {
                    if let Ok(lsn_value) = u64::from_str_radix(lsn_str, 16) {
                        let _file_start_lsn = LogSequenceNumber::new(lsn_value);

                        // Read file to find highest LSN and checkpoints
                        let file = File::open(&path)?;
                        let reader = BufReader::new(file);

                        for line in reader.lines() {
                            let line = line?;
                            if let Ok((lsn, entry)) =
                                serde_json::from_str::<(LogSequenceNumber, LogEntry)>(&line)
                            {
                                // Update max LSN
                                if lsn > max_lsn {
                                    max_lsn = lsn;
                                }

                                // Check for checkpoint
                                if let LogEntry::Checkpoint {
                                    lsn: checkpoint, ..
                                } = entry
                                {
                                    if checkpoint > checkpoint_lsn {
                                        checkpoint_lsn = checkpoint;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok((max_lsn, checkpoint_lsn))
    }

    /// Write an entry to the current log file
    ///
    /// # Thread Safety
    /// Acquires write lock on current_file
    fn write_entry_sync(&self, lsn: LogSequenceNumber, entry: &LogEntry) -> CoreResult<()> {
        let mut file = self.current_file.write();

        // Serialize as JSON line
        let json = serde_json::to_string(&(lsn, entry))?;
        writeln!(file, "{}", json)?;

        // fsync if configured
        if self.config.sync_on_write {
            file.flush()?;
            file.get_ref().sync_all()?;
        }

        Ok(())
    }

    /// Check if current log file needs rotation
    async fn needs_rotation(&self) -> CoreResult<bool> {
        let log_path = self.current_log_path.read().clone();

        if let Ok(metadata) = tokio::fs::metadata(&log_path).await {
            Ok(metadata.len() >= self.config.max_file_size_bytes)
        } else {
            Ok(false)
        }
    }

    /// Clean up old WAL files before checkpoint LSN
    ///
    /// Keeps only the last `retention_count` files before checkpoint
    async fn cleanup_old_files(&self, checkpoint_lsn: LogSequenceNumber) -> CoreResult<()> {
        let mut old_files = Vec::new();
        let mut entries = tokio::fs::read_dir(&self.dir).await?;

        // Find all WAL files before checkpoint
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "log") {
                if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Some(lsn_str) = file_stem.strip_prefix("wal-") {
                        if let Ok(lsn_value) = u64::from_str_radix(lsn_str, 16) {
                            let lsn = LogSequenceNumber::new(lsn_value);
                            if lsn < checkpoint_lsn {
                                old_files.push((lsn, path));
                            }
                        }
                    }
                }
            }
        }

        // Sort by LSN (oldest first)
        old_files.sort_by_key(|(lsn, _)| *lsn);

        // Delete all but last retention_count
        let to_delete = old_files.len().saturating_sub(self.config.retention_count);
        for (_, path) in old_files.iter().take(to_delete) {
            tokio::fs::remove_file(path).await?;
        }

        Ok(())
    }

    /// Get list of all WAL files >= from_lsn
    async fn get_wal_files(
        &self,
        from_lsn: LogSequenceNumber,
    ) -> CoreResult<Vec<(LogSequenceNumber, PathBuf)>> {
        let mut wal_files = Vec::new();
        let mut entries = tokio::fs::read_dir(&self.dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "log") {
                if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Some(lsn_str) = file_stem.strip_prefix("wal-") {
                        if let Ok(lsn_value) = u64::from_str_radix(lsn_str, 16) {
                            let lsn = LogSequenceNumber::new(lsn_value);
                            if lsn >= from_lsn {
                                wal_files.push((lsn, path));
                            }
                        }
                    }
                }
            }
        }

        // Sort by LSN
        wal_files.sort_by_key(|(lsn, _)| *lsn);
        Ok(wal_files)
    }
}

#[async_trait]
impl WriteAheadLog for FileWAL {
    async fn append(&self, entry: LogEntry) -> CoreResult<LogSequenceNumber> {
        // Get next LSN (thread-safe)
        let lsn = {
            let mut current = self.current_lsn.write();
            let next = current.next();
            *current = next;
            next
        };

        // Write to file (sync if configured)
        self.write_entry_sync(lsn, &entry)?;

        // Check if rotation needed
        if self.needs_rotation().await? {
            self.rotate().await?;
        }

        Ok(lsn)
    }

    async fn append_batch(&self, entries: Vec<LogEntry>) -> CoreResult<Vec<LogSequenceNumber>> {
        if entries.is_empty() {
            return Ok(Vec::new());
        }

        let mut lsns = Vec::with_capacity(entries.len());

        // Assign consecutive LSNs
        {
            let mut current = self.current_lsn.write();
            for _ in 0..entries.len() {
                let next = current.next();
                *current = next;
                lsns.push(next);
            }
        }

        // Write all entries
        for (lsn, entry) in lsns.iter().zip(entries.iter()) {
            self.write_entry_sync(*lsn, entry)?;
        }

        // Check if rotation needed
        if self.needs_rotation().await? {
            self.rotate().await?;
        }

        Ok(lsns)
    }

    async fn replay(
        &self,
        from_lsn: LogSequenceNumber,
    ) -> CoreResult<Vec<(LogSequenceNumber, LogEntry)>> {
        let mut entries = Vec::new();

        // Get all WAL files >= from_lsn
        let wal_files = self.get_wal_files(from_lsn).await?;

        // Read entries from each file
        for (_, path) in wal_files {
            let file = File::open(&path)?;
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let line = line?;
                // Parse JSON line
                match serde_json::from_str::<(LogSequenceNumber, LogEntry)>(&line) {
                    Ok((lsn, entry)) => {
                        if lsn >= from_lsn {
                            entries.push((lsn, entry));
                        }
                    }
                    Err(e) => {
                        // Log error but continue (tolerate corrupted entries)
                        eprintln!("Warning: Failed to parse WAL entry: {}", e);
                    }
                }
            }
        }

        // Sort by LSN (should already be sorted, but ensure it)
        entries.sort_by_key(|(lsn, _)| *lsn);

        Ok(entries)
    }

    async fn checkpoint(&self, lsn: LogSequenceNumber) -> CoreResult<()> {
        // Update checkpoint LSN
        *self.checkpoint_lsn.write() = lsn;

        // Write checkpoint marker to WAL
        let entry = LogEntry::Checkpoint {
            lsn,
            timestamp: chrono::Utc::now(),
        };
        self.append(entry).await?;

        // Clean up old files
        self.cleanup_old_files(lsn).await?;

        Ok(())
    }

    async fn rotate(&self) -> CoreResult<()> {
        // Flush current file
        self.flush().await?;

        // FIX BUG #17: Name file with NEXT LSN (first entry it will contain)
        // Otherwise replay filtering breaks: file named "wal-1000" contains LSN 1001,
        // but replay(from_lsn=1001) filters it out because 1000 < 1001
        let current_lsn = *self.current_lsn.read();
        let next_lsn = current_lsn.next();
        let new_log_path = self.dir.join(format!("wal-{:016x}.log", next_lsn.value()));

        let new_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&new_log_path)?;

        // Swap file handle (thread-safe)
        *self.current_file.write() = BufWriter::new(new_file);
        *self.current_log_path.write() = new_log_path;

        Ok(())
    }

    async fn current_lsn(&self) -> CoreResult<LogSequenceNumber> {
        Ok(*self.current_lsn.read())
    }

    async fn flush(&self) -> CoreResult<()> {
        let mut file = self.current_file.write();
        file.flush()?;
        file.get_ref().sync_all()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use akidb_core::{CollectionId, DocumentId};
    use tempfile::TempDir;

    async fn create_test_wal() -> (FileWAL, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let wal = FileWAL::new(temp_dir.path(), FileWALConfig::default())
            .await
            .unwrap();
        (wal, temp_dir)
    }

    #[tokio::test]
    async fn test_file_wal_creation() {
        let (wal, _dir) = create_test_wal().await;
        let lsn = wal.current_lsn().await.unwrap();
        assert_eq!(lsn, LogSequenceNumber::ZERO);
    }

    #[tokio::test]
    async fn test_file_wal_append() {
        let (wal, _dir) = create_test_wal().await;

        let entry = LogEntry::CreateCollection {
            collection_id: CollectionId::new(),
            dimension: 128,
            timestamp: chrono::Utc::now(),
        };

        let lsn = wal.append(entry).await.unwrap();
        assert_eq!(lsn, LogSequenceNumber::new(1));

        let current = wal.current_lsn().await.unwrap();
        assert_eq!(current, LogSequenceNumber::new(1));
    }

    #[tokio::test]
    async fn test_file_wal_batch_append() {
        let (wal, _dir) = create_test_wal().await;

        let entries = vec![
            LogEntry::CreateCollection {
                collection_id: CollectionId::new(),
                dimension: 128,
                timestamp: chrono::Utc::now(),
            },
            LogEntry::Upsert {
                collection_id: CollectionId::new(),
                doc_id: DocumentId::new(),
                vector: vec![1.0, 2.0, 3.0],
                external_id: None,
                metadata: None,
                timestamp: chrono::Utc::now(),
            },
        ];

        let lsns = wal.append_batch(entries).await.unwrap();
        assert_eq!(lsns.len(), 2);
        assert_eq!(lsns[0], LogSequenceNumber::new(1));
        assert_eq!(lsns[1], LogSequenceNumber::new(2));
    }

    #[tokio::test]
    async fn test_file_wal_replay() {
        let (wal, _dir) = create_test_wal().await;

        // Append some entries
        for i in 0..5 {
            let entry = LogEntry::Upsert {
                collection_id: CollectionId::new(),
                doc_id: DocumentId::new(),
                vector: vec![i as f32],
                external_id: Some(format!("doc-{}", i)),
                metadata: None,
                timestamp: chrono::Utc::now(),
            };
            wal.append(entry).await.unwrap();
        }

        // Replay from beginning
        let entries = wal.replay(LogSequenceNumber::ZERO).await.unwrap();
        assert_eq!(entries.len(), 5);

        // Verify LSNs are consecutive
        for (i, (lsn, _)) in entries.iter().enumerate() {
            assert_eq!(lsn.value(), (i + 1) as u64);
        }
    }

    #[tokio::test]
    async fn test_file_wal_crash_recovery() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().to_path_buf();

        // Create WAL and write entries
        {
            let wal = FileWAL::new(&dir_path, FileWALConfig::default())
                .await
                .unwrap();

            for i in 0..10 {
                let entry = LogEntry::Upsert {
                    collection_id: CollectionId::new(),
                    doc_id: DocumentId::new(),
                    vector: vec![i as f32],
                    external_id: None,
                    metadata: None,
                    timestamp: chrono::Utc::now(),
                };
                wal.append(entry).await.unwrap();
            }
        } // WAL dropped (simulates crash)

        // Recover from crash
        {
            let wal = FileWAL::new(&dir_path, FileWALConfig::default())
                .await
                .unwrap();

            // Verify LSN recovered
            let current_lsn = wal.current_lsn().await.unwrap();
            assert_eq!(current_lsn.value(), 10);

            // Verify entries can be replayed
            let entries = wal.replay(LogSequenceNumber::ZERO).await.unwrap();
            assert_eq!(entries.len(), 10);
        }
    }

    #[tokio::test]
    async fn test_file_wal_checkpoint() {
        let (wal, _dir) = create_test_wal().await;

        // Write 100 entries
        for i in 0..100 {
            let entry = LogEntry::Upsert {
                collection_id: CollectionId::new(),
                doc_id: DocumentId::new(),
                vector: vec![i as f32],
                external_id: None,
                metadata: None,
                timestamp: chrono::Utc::now(),
            };
            wal.append(entry).await.unwrap();
        }

        // Checkpoint at LSN 50
        wal.checkpoint(LogSequenceNumber::new(50)).await.unwrap();

        // Replay should still get all entries (checkpoint just marks for cleanup)
        let entries = wal.replay(LogSequenceNumber::ZERO).await.unwrap();
        assert!(entries.len() >= 100); // >= because checkpoint adds an entry
    }

    #[tokio::test]
    async fn test_file_wal_flush() {
        let (wal, _dir) = create_test_wal().await;

        let entry = LogEntry::CreateCollection {
            collection_id: CollectionId::new(),
            dimension: 256,
            timestamp: chrono::Utc::now(),
        };

        wal.append(entry).await.unwrap();
        wal.flush().await.unwrap();

        // If we get here, flush succeeded
        assert!(true);
    }
}
