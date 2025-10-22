use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::membership::{ClusterState, MemberDescriptor};

/// Command emitted by the balancer to direct shard placement adjustments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceCommand {
    pub member: MemberDescriptor,
    pub shards_to_add: Vec<String>,
    pub shards_to_remove: Vec<String>,
    pub reason: String,
}

/// Balances clusters by analyzing hot shards and replica skew.
#[async_trait]
pub trait ClusterBalancer: Send + Sync {
    async fn rebalance(
        &self,
        state: ClusterState,
    ) -> Result<Vec<BalanceCommand>, akidb_core::Error>;
    async fn schedule(&self, interval: Duration) -> Result<(), akidb_core::Error>;
}
