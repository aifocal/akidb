use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Simple background job definition executed by MCP scheduler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundJob {
    pub name: String,
    pub interval_ms: u64,
}

/// Trait for scheduling periodic background jobs (snapshots, compactions, etc.).
#[async_trait]
pub trait JobScheduler: Send + Sync {
    async fn register(&self, job: BackgroundJob) -> Result<(), akidb_core::Error>;
    async fn run(&self) -> Result<(), akidb_core::Error>;
}
