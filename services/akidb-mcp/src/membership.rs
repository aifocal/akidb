use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::watch;
use uuid::Uuid;

use akidb_core::CollectionDescriptor;

/// Description of a cluster member participating in the control plane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberDescriptor {
    pub id: Uuid,
    pub address: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub capabilities: Vec<String>,
}

/// Snapshot of cluster membership state.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClusterState {
    pub epoch: u64,
    pub members: HashMap<Uuid, MemberDescriptor>,
    pub collections: HashMap<String, CollectionDescriptor>,
}

/// Trait managing membership reconciliation against an external store (e.g., etcd).
#[async_trait]
pub trait MembershipCoordinator: Send + Sync {
    async fn current_state(&self) -> ClusterState;
    async fn watch(&self) -> Result<watch::Receiver<ClusterState>, akidb_core::Error>;
    async fn heartbeat(
        &self,
        member: MemberDescriptor,
        ttl: Duration,
    ) -> Result<(), akidb_core::Error>;
}
