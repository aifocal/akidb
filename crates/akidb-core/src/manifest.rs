use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::collection::DistanceMetric;
use crate::segment::SegmentDescriptor;

/// Logical manifest describing the active set of segments for a collection.
/// Supports both legacy format and MANIFESTv1 format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionManifest {
    pub collection: String,
    /// Legacy field: latest manifest version
    #[serde(default)]
    pub latest_version: u64,
    pub updated_at: DateTime<Utc>,

    /// MANIFESTv1 fields
    #[serde(default)]
    pub dimension: u32,
    #[serde(default)]
    pub metric: DistanceMetric,
    #[serde(default)]
    pub total_vectors: u64,
    #[serde(default)]
    pub epoch: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,

    /// Legacy format uses snapshot, new format uses direct segments list
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<ManifestSnapshot>,
    #[serde(default)]
    pub segments: Vec<SegmentDescriptor>,
}

/// Immutable snapshot of segments at a particular manifest version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestSnapshot {
    pub manifest_id: Uuid,
    pub entries: Vec<ManifestEntry>,
    pub created_at: DateTime<Utc>,
}

/// Entry describing a single segment in the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub descriptor: SegmentDescriptor,
    pub vector_uri: String,
    pub payload_uri: String,
    pub bitmap_uri: Option<String>,
}
