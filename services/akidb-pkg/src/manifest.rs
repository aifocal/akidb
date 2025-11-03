use serde::{Deserialize, Serialize};

/// Package manifest for .akipkg format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManifest {
    /// Manifest format version
    pub version: String,

    /// Collection name
    pub collection_name: String,

    /// Snapshot version number
    pub snapshot_version: u64,

    /// Creation timestamp (ISO 8601)
    pub created_at: String,

    /// AkiDB version that created this package
    pub akidb_version: String,

    /// Total number of vectors
    pub total_vectors: usize,

    /// Total number of segments
    pub total_segments: usize,

    /// Compressed package size in bytes
    pub compressed_size_bytes: u64,

    /// Uncompressed data size in bytes
    pub uncompressed_size_bytes: u64,

    /// Vector dimension
    pub vector_dim: usize,

    /// Distance metric (Cosine, Euclidean, DotProduct)
    pub distance_metric: String,

    /// Digital signature (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<PackageSignature>,
}

/// Digital signature for package verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageSignature {
    /// Signature algorithm (Ed25519)
    pub algorithm: String,

    /// Public key (hex-encoded)
    pub public_key: String,

    /// Signature bytes (hex-encoded)
    pub signature: String,
}

impl PackageManifest {
    /// Create a new manifest
    pub fn new(
        collection_name: String,
        snapshot_version: u64,
        total_vectors: usize,
        total_segments: usize,
        vector_dim: usize,
        distance_metric: String,
    ) -> Self {
        Self {
            version: "1.0".to_string(),
            collection_name,
            snapshot_version,
            created_at: chrono::Utc::now().to_rfc3339(),
            akidb_version: env!("CARGO_PKG_VERSION").to_string(),
            total_vectors,
            total_segments,
            compressed_size_bytes: 0,
            uncompressed_size_bytes: 0,
            vector_dim,
            distance_metric,
            signature: None,
        }
    }

    /// Serialize to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize from JSON
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_serialization() {
        let manifest = PackageManifest::new(
            "products".to_string(),
            1,
            10000,
            5,
            768,
            "Cosine".to_string(),
        );

        let json = manifest.to_json().unwrap();
        assert!(json.contains("products"));
        assert!(json.contains("\"version\":\"1.0\""));

        let deserialized = PackageManifest::from_json(&json).unwrap();
        assert_eq!(deserialized.collection_name, "products");
        assert_eq!(deserialized.total_vectors, 10000);
    }

    #[test]
    fn test_manifest_with_signature() {
        let mut manifest = PackageManifest::new(
            "test".to_string(),
            1,
            100,
            1,
            128,
            "Euclidean".to_string(),
        );

        manifest.signature = Some(PackageSignature {
            algorithm: "Ed25519".to_string(),
            public_key: "aabbcc".to_string(),
            signature: "ddeeff".to_string(),
        });

        let json = manifest.to_json().unwrap();
        assert!(json.contains("Ed25519"));
        assert!(json.contains("aabbcc"));
    }
}
