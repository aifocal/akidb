use akidb_core::Result;

use crate::{context::QueryRequest, plan::PhysicalPlan};

/// Planner transforms a high-level query request into a physical plan.
pub trait QueryPlanner: Send + Sync {
    fn plan(&self, request: &QueryRequest) -> Result<PhysicalPlan>;
}
