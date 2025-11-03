pub mod backend;
pub mod error;
pub mod memory;
pub mod metadata;
pub mod metadata_store;
pub mod s3;
pub mod segment_format;
pub mod snapshot;
pub mod wal;
pub mod wal_append_only;

// Phase 7 M1: Multi-Tenancy
pub mod tenant_store;

// Phase 7 M2: Namespace Isolation
pub mod tenant_backend;

pub use backend::{StorageBackend, StorageStatus};
pub use error::Result;
pub use memory::MemoryStorageBackend;
pub use metadata::{CompressionType as MetadataCompressionType, MetadataBlock};
pub use metadata_store::{MemoryMetadataStore, MetadataStore};
pub use s3::{S3Config, S3RetryConfig, S3StorageBackend};
pub use segment_format::{
    ChecksumType, CompressionType, SegmentData, SegmentReader, SegmentWriter,
};
pub use snapshot::{SnapshotCoordinator, SnapshotReader};
pub use wal::{
    LogSequence, RecoveryStats, ReplayStats, S3WalBackend, WalAppender, WalEntry, WalRecord,
    WalRecovery, WalReplayer, WalStreamId,
};
pub use wal_append_only::{AppendOnlyWalBackend, SegmentMetadata, WalManifest};
pub use tenant_store::{S3TenantStore, TenantStore};
pub use tenant_backend::TenantStorageBackend;
