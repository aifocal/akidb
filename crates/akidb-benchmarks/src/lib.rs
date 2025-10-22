use std::collections::HashSet;
use std::sync::Arc;

use akidb_core::collection::{
    CollectionDescriptor, DistanceMetric, PayloadDataType, PayloadField, PayloadSchema,
};
use akidb_core::Result;
use akidb_index::{
    BuildRequest, IndexBatch, IndexHandle, IndexKind, IndexProvider, NativeIndexProvider,
    QueryVector,
};
use once_cell::sync::OnceCell;
use rand::{rngs::StdRng, Rng, SeedableRng};
use roaring::RoaringBitmap;
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use uuid::Uuid;

/// Default vector dimension for benchmarking scenarios.
pub const DEFAULT_DIMENSION: usize = 128;

/// Wrapper representing a collection under test.
///
/// The collection stores generated vectors and payloads so we can build various index
/// configurations (distance metrics, filtering) without regenerating the dataset.
#[derive(Clone)]
pub struct Collection {
    pub name: String,
    pub descriptor: Arc<CollectionDescriptor>,
    pub vectors: Arc<Vec<Vec<f32>>>,
    pub payloads: Arc<Vec<Value>>,
}

impl Collection {
    /// Vector dimension derived from the descriptor.
    pub fn dimension(&self) -> usize {
        self.descriptor.vector_dim as usize
    }

    /// Build a native index populated with the collection vectors using the desired distance metric.
    pub fn build_native_index(
        &self,
        runtime: &Runtime,
        distance: DistanceMetric,
    ) -> Result<(Arc<NativeIndexProvider>, IndexHandle)> {
        let provider = Arc::new(NativeIndexProvider::new());
        let handle = runtime.block_on(async {
            provider
                .build(BuildRequest {
                    collection: self.name.clone(),
                    kind: IndexKind::Native,
                    distance,
                    segments: vec![test_segment_descriptor(
                        &self.name,
                        self.descriptor.vector_dim,
                        self.vectors.len(),
                    )],
                })
                .await
        })?;

        let batch = IndexBatch {
            primary_keys: (0..self.vectors.len())
                .map(|i| format!("{}-{}", self.name, i))
                .collect(),
            vectors: self
                .vectors
                .iter()
                .map(|components| QueryVector {
                    components: components.clone(),
                })
                .collect(),
            payloads: self.payloads.as_ref().clone(),
        };

        runtime.block_on(async { provider.add_batch(&handle, batch).await })?;
        Ok((provider, handle))
    }

    /// Construct a filter bitmap from payloads for metadata-aware benchmarks.
    pub fn build_filter_bitmap(&self, field: &str, allowed: &[&str]) -> RoaringBitmap {
        let mut bitmap = RoaringBitmap::new();
        let allowed: HashSet<&str> = allowed.iter().copied().collect();

        for (idx, payload) in self.payloads.iter().enumerate() {
            if let Some(value) = payload.get(field) {
                if let Some(s) = value.as_str() {
                    if allowed.contains(s) {
                        bitmap.insert(idx as u32);
                    }
                }
            }
        }

        bitmap
    }
}

/// Generate a reproducible collection of random vectors for benchmarking.
pub fn generate_random_vectors(count: usize, dim: usize) -> Vec<Vec<f32>> {
    assert!(dim > 0, "Vector dimension must be positive");

    let mut rng = StdRng::seed_from_u64((count as u64) ^ 0x5A5A_1234_ABCD);
    (0..count)
        .map(|_| (0..dim).map(|_| rng.gen::<f32>()).collect::<Vec<f32>>())
        .collect()
}

/// Generate synthetic payloads with optional filterable metadata fields.
pub fn generate_payloads(count: usize, with_filters: bool) -> Vec<Value> {
    let mut rng = StdRng::seed_from_u64(0xBEEF_DEAD ^ (count as u64));
    let tags = ["alpha", "beta", "gamma", "delta"];

    (0..count)
        .map(|i| {
            if with_filters {
                let tag = tags[rng.gen_range(0..tags.len())];
                json!({
                    "id": i,
                    "tag": tag,
                    "active": rng.gen_bool(0.5),
                })
            } else {
                json!({
                    "id": i,
                })
            }
        })
        .collect()
}

/// Assemble a collection descriptor plus backing data ready for indexing benchmarks.
pub fn create_test_collection(vectors: Vec<Vec<f32>>, payloads: Vec<Value>) -> Collection {
    let vectors = Arc::new(vectors);
    create_collection_from_arc(None, vectors, payloads)
}

/// Build a collection from an existing shared vector buffer.
pub fn create_collection_from_arc(
    name: Option<String>,
    vectors: Arc<Vec<Vec<f32>>>,
    payloads: Vec<Value>,
) -> Collection {
    assert!(
        vectors.len() == payloads.len(),
        "Payload and vector counts must match"
    );
    let dimension = vectors
        .first()
        .map(|v| v.len())
        .unwrap_or(DEFAULT_DIMENSION);

    let descriptor = CollectionDescriptor {
        name: name.unwrap_or_else(|| format!("bench-{}", Uuid::new_v4())),
        vector_dim: dimension as u16,
        distance: DistanceMetric::Cosine,
        replication: 1,
        shard_count: 1,
        payload_schema: PayloadSchema {
            fields: if payloads.iter().any(|p| p.get("tag").is_some()) {
                vec![PayloadField {
                    name: "tag".to_string(),
                    data_type: PayloadDataType::Keyword,
                    indexed: true,
                }]
            } else {
                Vec::new()
            },
        },
    };

    Collection {
        name: descriptor.name.clone(),
        descriptor: Arc::new(descriptor),
        vectors,
        payloads: Arc::new(payloads),
    }
}

/// Lazily created Tokio runtime for async benchmark helpers.
pub fn runtime() -> &'static Runtime {
    static RUNTIME: OnceCell<Runtime> = OnceCell::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to build Tokio runtime")
    })
}

/// Helper to construct a synthetic segment descriptor for benchmarking builds.
pub fn test_segment_descriptor(
    collection: &str,
    dimension: u16,
    record_count: usize,
) -> akidb_core::segment::SegmentDescriptor {
    use akidb_core::segment::{SegmentDescriptor, SegmentState};

    SegmentDescriptor {
        segment_id: Uuid::new_v4(),
        collection: collection.to_string(),
        vector_dim: dimension,
        record_count: record_count as u32,
        state: SegmentState::Active,
        lsn_range: 0..=0,
        compression_level: 0,
        created_at: chrono::Utc::now(),
    }
}

/// Convert a collection into an index batch (used for index rebuild benchmarks).
pub fn collection_to_index_batch(collection: &Collection) -> IndexBatch {
    IndexBatch {
        primary_keys: (0..collection.vectors.len())
            .map(|i| format!("{}-{}", collection.name, i))
            .collect(),
        vectors: collection
            .vectors
            .iter()
            .map(|v| QueryVector {
                components: v.clone(),
            })
            .collect(),
        payloads: collection.payloads.as_ref().clone(),
    }
}
