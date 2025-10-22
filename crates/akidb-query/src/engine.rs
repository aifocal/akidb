use async_trait::async_trait;

use akidb_core::Result;

use crate::{context::QueryContext, context::QueryResponse, plan::PhysicalPlan};

/// Execution engine runs a physical plan within an async runtime.
#[async_trait]
pub trait ExecutionEngine: Send + Sync {
    async fn execute(&self, plan: PhysicalPlan, ctx: QueryContext) -> Result<QueryResponse>;
}
