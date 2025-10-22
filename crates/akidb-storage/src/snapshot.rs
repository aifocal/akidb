use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use akidb_core::CollectionManifest;

use crate::error::Result;

/// Metadata describing a durable snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDescriptor {
    pub snapshot_id: SnapshotId,
    pub collection: String,
    pub manifest_version: u64,
    pub created_at: DateTime<Utc>,
    pub total_bytes: u64,
}

/// Stable identifier for a snapshot artifact.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SnapshotId(pub Uuid);

/// Coordinates creation of consistent snapshots across storage backends.
#[async_trait]
pub trait SnapshotCoordinator: Send + Sync {
    async fn begin_snapshot(&self, collection: &str) -> Result<SnapshotDescriptor>;
    async fn materialize(
        &self,
        descriptor: &SnapshotDescriptor,
        manifest: &CollectionManifest,
    ) -> Result<()>;
    async fn finalize(&self, descriptor: SnapshotDescriptor) -> Result<()>;
}

/// Interface for streaming snapshot contents back into a running node.
#[async_trait]
pub trait SnapshotReader: Send + Sync {
    async fn list(&self, collection: &str) -> Result<Vec<SnapshotDescriptor>>;
    async fn load(&self, descriptor: &SnapshotDescriptor) -> Result<Vec<Bytes>>;
}
