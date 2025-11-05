use crate::manifest::PackageManifest;
use crate::verify::verify_package;
use akidb_storage::{S3Config, S3StorageBackend, StorageBackend};
use bytes::Bytes;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tar::Archive;
use tracing::{info, warn};
use zstd::Decoder;

// Security limits (same as verify.rs)
const MAX_FILE_SIZE_BYTES: u64 = 1_000_000_000; // 1GB per file
const MAX_TOTAL_EXTRACTED_BYTES: u64 = 10_000_000_000; // 10GB total

/// Validate TAR entry path for security (prevent path traversal attacks)
fn validate_tar_path(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let path_str = path.to_string_lossy();

    // Reject absolute paths
    if path.is_absolute() {
        return Err(format!("Absolute paths not allowed in archives: {}", path_str).into());
    }

    // Reject paths with parent directory references (..)
    for component in path.components() {
        if component == std::path::Component::ParentDir {
            return Err(format!("Path traversal detected in archive: {}", path_str).into());
        }
    }

    Ok(())
}

/// Import an .akipkg package to a collection
#[allow(clippy::too_many_arguments)]
pub async fn import_package(
    file: PathBuf,
    collection: Option<String>,
    verify_signature: bool,
    s3_endpoint: String,
    s3_access_key: String,
    s3_secret_key: String,
    s3_bucket: String,
    s3_region: String,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Importing package: {}", file.display());

    // 1. Verify package integrity (checksums + optional signature)
    if verify_signature {
        info!("Verifying package integrity and signature...");
        let verification_result = verify_package(file.clone(), None).await?;

        if !verification_result.is_valid() {
            return Err(format!(
                "Package verification failed with {} errors: {:?}",
                verification_result.errors.len(),
                verification_result.errors
            )
            .into());
        }

        info!("✅ Package verification passed");
    } else {
        info!("⚠️  Skipping package verification (--verify-signature=false)");
    }

    // 2. Decompress and extract package
    info!("Decompressing package...");
    let compressed_file = File::open(&file)?;
    let decoder = Decoder::new(compressed_file)?;
    let mut archive = Archive::new(decoder);

    // 3. Extract all files and store in memory
    info!("Extracting package contents...");
    let mut manifest_opt: Option<PackageManifest> = None;
    let mut collection_manifest_data_opt: Option<Vec<u8>> = None;
    let mut checksums_opt: Option<Vec<(usize, String)>> = None;
    let mut segment_data_map: HashMap<usize, Vec<u8>> = HashMap::new();
    let mut total_extracted_bytes: u64 = 0;

    for entry_result in archive.entries()? {
        let mut entry = entry_result?;
        let path = entry.path()?.to_path_buf();
        let path_str = path.to_string_lossy().to_string();

        // SECURITY: Validate path to prevent path traversal attacks
        validate_tar_path(&path)?;

        let file_size = entry.size();

        // SECURITY: Check individual file size limit
        if file_size > MAX_FILE_SIZE_BYTES {
            return Err(format!(
                "File {} exceeds maximum size limit ({} > {} bytes)",
                path_str, file_size, MAX_FILE_SIZE_BYTES
            )
            .into());
        }

        // SECURITY: Check total extracted size (zip bomb protection)
        total_extracted_bytes = total_extracted_bytes
            .checked_add(file_size)
            .ok_or("Total extracted size overflow")?;

        if total_extracted_bytes > MAX_TOTAL_EXTRACTED_BYTES {
            return Err(format!(
                "Total extracted size exceeds limit ({} > {} bytes). Possible zip bomb attack.",
                total_extracted_bytes, MAX_TOTAL_EXTRACTED_BYTES
            )
            .into());
        }

        // Read entry data
        let mut data = Vec::new();
        entry.read_to_end(&mut data)?;

        match path_str.as_str() {
            "manifest.json" => {
                let manifest_json = String::from_utf8(data)?;
                manifest_opt = Some(PackageManifest::from_json(&manifest_json)?);
                info!("✅ Package manifest loaded");
            }
            "collection_manifest.json" => {
                collection_manifest_data_opt = Some(data);
                info!("✅ Collection manifest loaded");
            }
            "checksums.json" => {
                let checksums_json = String::from_utf8(data)?;
                checksums_opt = Some(serde_json::from_str(&checksums_json)?);
                info!("✅ Checksums loaded");
            }
            _ if path_str.starts_with("segments/segment_") => {
                // Extract segment ID from filename
                let filename = path_str.trim_start_matches("segments/segment_");
                let segment_id_str = filename.trim_end_matches(".seg");
                if let Ok(segment_id) = segment_id_str.parse::<usize>() {
                    segment_data_map.insert(segment_id, data);
                    info!("✅ Extracted segment {}", segment_id);
                }
            }
            _ => {
                warn!("Skipping unknown file: {}", path_str);
            }
        }
    }

    // 4. Get package manifest
    let manifest = manifest_opt.ok_or("Package manifest not found in archive")?;
    info!(
        "Package: {} (version {}, {} vectors, {} segments)",
        manifest.collection_name, manifest.version, manifest.total_vectors, manifest.total_segments
    );

    // Determine target collection name
    let target_collection = collection.unwrap_or_else(|| manifest.collection_name.clone());
    info!("Target collection: {}", target_collection);

    // 5. Verify checksums before upload
    if let Some(checksums) = checksums_opt {
        info!("Verifying {} segment checksums before upload...", checksums.len());

        for (segment_id, expected_checksum) in checksums {
            match segment_data_map.get(&segment_id) {
                Some(segment_data) => {
                    let mut hasher = Sha256::new();
                    hasher.update(segment_data);
                    let actual_checksum = format!("{:x}", hasher.finalize());

                    if actual_checksum != expected_checksum {
                        return Err(format!(
                            "Checksum mismatch for segment {}: expected {}, got {}",
                            segment_id, expected_checksum, actual_checksum
                        )
                        .into());
                    }
                }
                None => {
                    return Err(format!("Segment {} missing from archive", segment_id).into());
                }
            }
        }

        info!("✅ All checksums verified");
    }

    // 6. Connect to S3
    info!("Connecting to S3: {}", s3_endpoint);
    let s3_config = S3Config {
        endpoint: s3_endpoint.clone(),
        region: s3_region.clone(),
        access_key: s3_access_key.clone(),
        secret_key: s3_secret_key.clone(),
        bucket: s3_bucket.clone(),
        ..Default::default()
    };

    let storage = Arc::new(S3StorageBackend::new(s3_config)?);
    info!("✅ Connected to S3");

    // 7. Upload segments to S3
    info!("Uploading {} segments to S3...", segment_data_map.len());
    let mut uploaded_segments = 0;

    for (segment_id, segment_data) in segment_data_map.iter() {
        let segment_key = format!(
            "collections/{}/segments/segment_{:06}.seg",
            target_collection, segment_id
        );

        storage.put_object(&segment_key, Bytes::from(segment_data.clone())).await?;
        uploaded_segments += 1;

        if uploaded_segments % 10 == 0 {
            info!(
                "Uploaded {}/{} segments ({:.1}%)",
                uploaded_segments,
                segment_data_map.len(),
                (uploaded_segments as f64 / segment_data_map.len() as f64) * 100.0
            );
        }
    }

    info!("✅ All {} segments uploaded to S3", uploaded_segments);

    // 8. Upload collection manifest to S3
    if let Some(collection_manifest_data) = collection_manifest_data_opt {
        info!("Uploading collection manifest to S3...");
        let manifest_key = format!("collections/{}/manifest.json", target_collection);

        // Update collection name in manifest if target differs from source
        if target_collection != manifest.collection_name {
            let mut collection_manifest: serde_json::Value =
                serde_json::from_slice(&collection_manifest_data)?;

            if let Some(obj) = collection_manifest.as_object_mut() {
                obj.insert(
                    "collection_name".to_string(),
                    serde_json::Value::String(target_collection.clone()),
                );
            }

            let updated_manifest_data = serde_json::to_vec_pretty(&collection_manifest)?;
            storage.put_object(&manifest_key, Bytes::from(updated_manifest_data)).await?;
        } else {
            storage.put_object(&manifest_key, Bytes::from(collection_manifest_data.clone())).await?;
        }

        info!("✅ Collection manifest uploaded");
    } else {
        warn!("⚠️  No collection manifest found in package, creating minimal manifest");

        // Create minimal collection manifest
        let minimal_manifest = serde_json::json!({
            "collection_name": target_collection,
            "version": manifest.version,
            "snapshot_version": manifest.snapshot_version,
            "created_at": chrono::Utc::now().to_rfc3339(),
            "vector_dim": manifest.vector_dim,
            "distance_metric": manifest.distance_metric,
            "segment_count": manifest.total_segments,
            "total_vectors": manifest.total_vectors,
        });

        let manifest_key = format!("collections/{}/manifest.json", target_collection);
        let manifest_data = serde_json::to_vec_pretty(&minimal_manifest)?;
        storage.put_object(&manifest_key, Bytes::from(manifest_data)).await?;

        info!("✅ Minimal collection manifest created");
    }

    // 9. Verify import by reading back manifest
    info!("Verifying import...");
    let manifest_key = format!("collections/{}/manifest.json", target_collection);
    let _manifest_data = storage.get_object(&manifest_key).await?;
    info!("✅ Import verified - collection manifest readable from S3");

    info!("✅ Import complete!");
    info!("   Collection: {}", target_collection);
    info!("   Vectors: {}", manifest.total_vectors);
    info!("   Segments uploaded: {}", uploaded_segments);
    info!("   Vector dimension: {}", manifest.vector_dim);
    info!("   Distance metric: {}", manifest.distance_metric);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires S3/MinIO and a valid package file
    async fn test_import_basic() {
        // This would require:
        // 1. A valid .akipkg file
        // 2. A running MinIO instance
        // 3. S3 credentials
        //
        // For now, this is a placeholder for future integration testing
    }
}
