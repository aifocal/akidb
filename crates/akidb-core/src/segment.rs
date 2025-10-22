use std::ops::RangeInclusive;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Descriptor for a persisted segment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SegmentDescriptor {
    pub segment_id: Uuid,
    pub collection: String,
    pub record_count: u32,
    pub vector_dim: u16,
    pub lsn_range: RangeInclusive<u64>,
    pub compression_level: u8,
    pub created_at: DateTime<Utc>,
    pub state: SegmentState,
}

/// Lifecycle state for a segment.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SegmentState {
    Active,
    Sealed,
    Compacting,
    Archived,
}
