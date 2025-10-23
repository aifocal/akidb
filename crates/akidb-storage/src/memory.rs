//! In-memory storage backend for testing

use async_trait::async_trait;
use bytes::Bytes;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use akidb_core::{CollectionDescriptor, CollectionManifest, Result, SegmentDescriptor};

use crate::backend::{StorageBackend, StorageStatus};
use crate::segment_format::SegmentData;

/// Simplified payload format for memory backend testing
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemorySegmentPayload {
    version: u8,
    dimension: u32,
    vectors: Vec<Vec<f32>>,
    metadata: Option<MemoryMetadataPayload>,
}

/// Metadata payload for memory backend
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoryMetadataPayload {
    format: String,      // "arrow-ipc"
    compression: String, // "none" or "zstd"
    data: Vec<u8>,      // Raw Arrow IPC bytes
}

/// In-memory storage backend (for testing)
#[derive(Clone)]
pub struct MemoryStorageBackend {
    objects: Arc<RwLock<HashMap<String, Bytes>>>,
}

impl MemoryStorageBackend {
    pub fn new() -> Self {
        Self {
            objects: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Helper to bump manifest version (mirrors S3StorageBackend)
    fn bump_manifest_revision(manifest: &mut CollectionManifest) {
        manifest.latest_version += 1;
        manifest.epoch += 1;
        manifest.updated_at = chrono::Utc::now();
    }
}

impl Default for MemoryStorageBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StorageBackend for MemoryStorageBackend {
    async fn status(&self) -> Result<StorageStatus> {
        Ok(StorageStatus::Healthy)
    }

    async fn create_collection(&self, descriptor: &CollectionDescriptor) -> Result<()> {
        // Store collection descriptor as JSON
        let key = format!("collections/{}/descriptor.json", descriptor.name);
        let data = serde_json::to_vec(descriptor)
            .map_err(|e| akidb_core::Error::Storage(e.to_string()))?;
        self.put_object(&key, Bytes::from(data)).await?;

        // Create initial manifest
        let manifest = CollectionManifest {
            collection: descriptor.name.clone(),
            dimension: descriptor.vector_dim as u32,
            metric: descriptor.distance,
            latest_version: 0,
            total_vectors: 0,
            epoch: 0,
            created_at: Some(chrono::Utc::now()),
            updated_at: chrono::Utc::now(),
            snapshot: None,
            segments: Vec::new(),
        };
        self.persist_manifest(&manifest).await
    }

    async fn drop_collection(&self, name: &str) -> Result<()> {
        // Remove all objects with this collection prefix
        let prefix = format!("collections/{}/", name);

        // Collect keys to remove (release lock before await)
        let keys_to_remove: Vec<String> = {
            let objects = self.objects.read();
            objects
                .keys()
                .filter(|k| k.starts_with(&prefix))
                .cloned()
                .collect()
        };

        for key in keys_to_remove {
            self.delete_object(&key).await?;
        }

        Ok(())
    }

    async fn write_segment(&self, descriptor: &SegmentDescriptor) -> Result<()> {
        let key = format!(
            "collections/{}/segments/{}.json",
            descriptor.collection, descriptor.segment_id
        );
        let data = serde_json::to_vec(descriptor)
            .map_err(|e| akidb_core::Error::Storage(e.to_string()))?;
        self.put_object(&key, Bytes::from(data)).await
    }

    async fn seal_segment(&self, segment_id: Uuid) -> Result<SegmentDescriptor> {
        // Find segment by scanning all collections (release lock before await)
        let mut found_descriptor = None;
        {
            let objects = self.objects.read();
            for (key, data) in objects.iter() {
                if key.contains("segments/") && key.ends_with(".json") {
                    if let Ok(descriptor) = serde_json::from_slice::<SegmentDescriptor>(data) {
                        if descriptor.segment_id == segment_id {
                            found_descriptor = Some(descriptor);
                            break;
                        }
                    }
                }
            }
        }

        if let Some(mut descriptor) = found_descriptor {
            descriptor.state = akidb_core::segment::SegmentState::Sealed;
            self.write_segment(&descriptor).await?;
            Ok(descriptor)
        } else {
            Err(akidb_core::Error::NotFound(format!(
                "Segment {} not found",
                segment_id
            )))
        }
    }

    async fn load_manifest(&self, collection: &str) -> Result<CollectionManifest> {
        let key = format!("collections/{}/manifest.json", collection);
        match self.get_object(&key).await {
            Ok(data) => {
                let manifest = serde_json::from_slice(&data)
                    .map_err(|e| akidb_core::Error::Storage(e.to_string()))?;
                Ok(manifest)
            }
            Err(akidb_core::Error::NotFound(_)) => {
                // Return empty manifest if not found
                Ok(CollectionManifest {
                    collection: collection.to_string(),
                    latest_version: 0,
                    updated_at: chrono::Utc::now(),
                    dimension: 0,
                    metric: akidb_core::collection::DistanceMetric::Cosine,
                    total_vectors: 0,
                    epoch: 0,
                    created_at: None,
                    snapshot: None,
                    segments: Vec::new(),
                })
            }
            Err(e) => Err(e),
        }
    }

    async fn persist_manifest(&self, manifest: &CollectionManifest) -> Result<()> {
        let key = format!("collections/{}/manifest.json", manifest.collection);
        let data =
            serde_json::to_vec(manifest).map_err(|e| akidb_core::Error::Storage(e.to_string()))?;
        self.put_object(&key, Bytes::from(data)).await
    }

    async fn get_object(&self, key: &str) -> Result<Bytes> {
        let objects = self.objects.read();
        objects
            .get(key)
            .cloned()
            .ok_or_else(|| akidb_core::Error::NotFound(format!("Object '{}' not found", key)))
    }

    async fn put_object(&self, key: &str, data: Bytes) -> Result<()> {
        let mut objects = self.objects.write();
        objects.insert(key.to_string(), data);
        Ok(())
    }

    async fn delete_object(&self, key: &str) -> Result<()> {
        let mut objects = self.objects.write();
        objects.remove(key);
        Ok(())
    }

    async fn object_exists(&self, key: &str) -> Result<bool> {
        let objects = self.objects.read();
        Ok(objects.contains_key(key))
    }

    async fn list_objects(&self, prefix: &str) -> Result<Vec<String>> {
        let objects = self.objects.read();
        let keys: Vec<String> = objects
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        Ok(keys)
    }

    async fn write_segment_with_data(
        &self,
        descriptor: &SegmentDescriptor,
        vectors: Vec<Vec<f32>>,
        metadata: Option<crate::metadata::MetadataBlock>,
    ) -> Result<()> {
        // 1. Load manifest and validate existence/dimension
        let mut manifest = self.load_manifest(&descriptor.collection).await?;

        // Validate collection exists (manifest with created_at != None)
        if manifest.created_at.is_none() {
            return Err(akidb_core::Error::Validation(format!(
                "Collection {} does not exist",
                descriptor.collection
            )));
        }

        // Validate dimension compatibility
        if manifest.dimension != 0 && manifest.dimension != descriptor.vector_dim as u32 {
            return Err(akidb_core::Error::Validation(format!(
                "Dimension mismatch: manifest has {}, descriptor has {}",
                manifest.dimension, descriptor.vector_dim
            )));
        }

        // 2. Run duplicate check
        if manifest
            .segments
            .iter()
            .any(|s| s.segment_id == descriptor.segment_id)
        {
            return Err(akidb_core::Error::Conflict(format!(
                "Segment {} already exists in collection {}",
                descriptor.segment_id, descriptor.collection
            )));
        }

        // 3. Validate vector count
        if vectors.len() != descriptor.record_count as usize {
            return Err(akidb_core::Error::Validation(format!(
                "Vector count mismatch: descriptor says {}, got {}",
                descriptor.record_count,
                vectors.len()
            )));
        }

        // 4. Build SegmentData for validation (dimension and metadata row count)
        let _segment_data = if let Some(ref meta) = metadata {
            SegmentData::with_metadata(
                descriptor.vector_dim as u32,
                vectors.clone(),
                meta.clone(),
            )?
        } else {
            SegmentData::new(descriptor.vector_dim as u32, vectors.clone())?
        };

        // 5. Serialize MemorySegmentPayload
        let payload = if let Some(meta) = metadata {
            let meta_bytes = meta
                .serialize()
                .map_err(|e| akidb_core::Error::Storage(format!("Metadata serialization failed: {}", e)))?;
            MemorySegmentPayload {
                version: 1,
                dimension: descriptor.vector_dim as u32,
                vectors,
                metadata: Some(MemoryMetadataPayload {
                    format: "arrow-ipc".to_string(),
                    compression: "none".to_string(),
                    data: meta_bytes,
                }),
            }
        } else {
            MemorySegmentPayload {
                version: 1,
                dimension: descriptor.vector_dim as u32,
                vectors,
                metadata: None,
            }
        };

        let payload_bytes = serde_json::to_vec(&payload)
            .map_err(|e| akidb_core::Error::Storage(format!("Failed to serialize payload: {}", e)))?;

        // 6. Store payload via put_object
        let seg_key = format!(
            "collections/{}/segments/{}.seg",
            descriptor.collection, descriptor.segment_id
        );
        self.put_object(&seg_key, Bytes::from(payload_bytes))
            .await?;

        // 7. Call existing write_segment to persist descriptor JSON
        self.write_segment(descriptor).await?;

        // 8. Update manifest
        manifest.segments.push(descriptor.clone());
        manifest.total_vectors += descriptor.record_count as u64;
        if manifest.dimension == 0 {
            manifest.dimension = descriptor.vector_dim as u32;
        }
        Self::bump_manifest_revision(&mut manifest);

        // 9. Persist manifest
        self.persist_manifest(&manifest).await?;

        Ok(())
    }
}
