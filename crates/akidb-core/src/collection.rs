use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Core descriptor describing a collection's vector and payload configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CollectionDescriptor {
    pub name: String,
    pub vector_dim: u16,
    pub distance: DistanceMetric,
    pub replication: u8,
    pub shard_count: u16,
    pub payload_schema: PayloadSchema,
    /// WAL stream ID for this collection (None for backward compatibility with old collections)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wal_stream_id: Option<Uuid>,
}

/// Distance metric used for ANN calculations.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum DistanceMetric {
    L2,
    #[default]
    Cosine,
    Dot,
}

/// Declarative schema for payload fields stored alongside vectors.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PayloadSchema {
    pub fields: Vec<PayloadField>,
}

/// Single payload field descriptor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PayloadField {
    pub name: String,
    pub data_type: PayloadDataType,
    pub indexed: bool,
}

/// Supported payload data types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PayloadDataType {
    Boolean,
    Integer,
    Float,
    Text,
    Keyword,
    GeoPoint,
    Timestamp,
    Json,
}
