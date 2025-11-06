use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::ids::{CollectionId, DatabaseId};

/// Distance metric for vector similarity search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DistanceMetric {
    /// Cosine similarity (1 - cosine distance)
    Cosine,
    /// Dot product (negative for minimization)
    Dot,
    /// Euclidean (L2) distance
    L2,
}

impl DistanceMetric {
    /// Returns the canonical lowercase string stored in SQLite.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Cosine => "cosine",
            Self::Dot => "dot",
            Self::L2 => "l2",
        }
    }
}

impl Default for DistanceMetric {
    fn default() -> Self {
        Self::Cosine
    }
}

impl FromStr for DistanceMetric {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "cosine" => Ok(Self::Cosine),
            "dot" => Ok(Self::Dot),
            "l2" => Ok(Self::L2),
            _ => Err(()),
        }
    }
}

/// Configuration parameters for a vector collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionDescriptor {
    /// Stable collection identifier.
    pub collection_id: CollectionId,
    /// Owning database identifier.
    pub database_id: DatabaseId,
    /// Human-readable name for the collection.
    pub name: String,
    /// Vector dimension (16-4096).
    pub dimension: u32,
    /// Distance metric for similarity search.
    pub metric: DistanceMetric,
    /// Embedding model identifier (e.g., "qwen3-embed-8b").
    pub embedding_model: String,
    /// HNSW graph degree (M parameter).
    pub hnsw_m: u32,
    /// HNSW construction EF parameter.
    pub hnsw_ef_construction: u32,
    /// Maximum document count (guardrail).
    pub max_doc_count: u64,
    /// Creation timestamp in UTC.
    pub created_at: DateTime<Utc>,
    /// Update timestamp in UTC.
    pub updated_at: DateTime<Utc>,
}

impl CollectionDescriptor {
    /// Default HNSW M parameter (graph degree).
    pub const DEFAULT_HNSW_M: u32 = 32;
    /// Default HNSW EF construction parameter.
    pub const DEFAULT_HNSW_EF_CONSTRUCTION: u32 = 200;
    /// Default maximum document count (50 million).
    pub const DEFAULT_MAX_DOC_COUNT: u64 = 50_000_000;

    /// Minimum vector dimension.
    pub const MIN_DIMENSION: u32 = 16;
    /// Maximum vector dimension.
    pub const MAX_DIMENSION: u32 = 4096;

    /// Creates a new collection descriptor with default HNSW parameters.
    #[must_use]
    pub fn new(
        database_id: DatabaseId,
        name: impl Into<String>,
        dimension: u32,
        embedding_model: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            collection_id: CollectionId::new(),
            database_id,
            name: name.into(),
            dimension,
            metric: DistanceMetric::default(),
            embedding_model: embedding_model.into(),
            hnsw_m: Self::DEFAULT_HNSW_M,
            hnsw_ef_construction: Self::DEFAULT_HNSW_EF_CONSTRUCTION,
            max_doc_count: Self::DEFAULT_MAX_DOC_COUNT,
            created_at: now,
            updated_at: now,
        }
    }

    /// Validates that the dimension is within acceptable bounds.
    ///
    /// # Errors
    ///
    /// Returns an error if dimension is outside [MIN_DIMENSION, MAX_DIMENSION].
    pub fn validate_dimension(&self) -> Result<(), String> {
        if self.dimension < Self::MIN_DIMENSION || self.dimension > Self::MAX_DIMENSION {
            return Err(format!(
                "dimension {} is outside valid range [{}, {}]",
                self.dimension,
                Self::MIN_DIMENSION,
                Self::MAX_DIMENSION
            ));
        }
        Ok(())
    }

    /// Updates the `updated_at` timestamp to the current time.
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}
