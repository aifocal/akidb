//! In-memory storage backend for testing

use async_trait::async_trait;
use bytes::Bytes;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use akidb_core::{CollectionDescriptor, CollectionManifest, Result, SegmentDescriptor};

use crate::backend::{StorageBackend, StorageStatus};

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
            metric: descriptor.distance.clone(),
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
        let data = serde_json::to_vec(manifest)
            .map_err(|e| akidb_core::Error::Storage(e.to_string()))?;
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
}
