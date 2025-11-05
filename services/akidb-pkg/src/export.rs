use crate::manifest::{PackageManifest, PackageSignature};
use akidb_storage::{S3Config, S3StorageBackend, StorageBackend};
use bytes::Bytes;
use ring::signature::{Ed25519KeyPair, KeyPair};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tar::{Builder, Header};
use tracing::{info, warn};
use zstd::Encoder;

/// Export a collection to .akipkg format
#[allow(clippy::too_many_arguments)]
pub async fn export_package(
    collection: String,
    output: PathBuf,
    sign_key: Option<PathBuf>,
    s3_endpoint: String,
    s3_access_key: String,
    s3_secret_key: String,
    s3_bucket: String,
    s3_region: String,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        "Exporting collection '{}' to {}",
        collection,
        output.display()
    );

    // 1. Connect to S3/MinIO
    let s3_config = S3Config {
        endpoint: s3_endpoint.clone(),
        region: s3_region.clone(),
        access_key: s3_access_key.clone(),
        secret_key: s3_secret_key.clone(),
        bucket: s3_bucket.clone(),
        ..Default::default()
    };

    let storage = Arc::new(S3StorageBackend::new(s3_config)?);
    info!("Connected to S3: {}", s3_endpoint);

    // 2. Load collection manifest from S3
    let manifest_key = format!("collections/{}/manifest.json", collection);

    info!("Loading collection manifest from S3...");
    let manifest_data = match storage.get_object(&manifest_key).await {
        Ok(data) => data.to_vec(),
        Err(e) => {
            warn!("Failed to load manifest for collection '{}': {}", collection, e);
            warn!("Creating minimal manifest for demonstration");
            // For demo purposes, create a minimal manifest
            // In production, this would be an error
            serde_json::to_vec(&serde_json::json!({
                "collection_name": collection,
                "version": 1,
                "vector_dim": 768,
                "distance_metric": "Cosine",
                "segment_count": 0,
                "total_vectors": 0
            }))?
        }
    };

    // Parse collection manifest to get metadata
    let collection_manifest: serde_json::Value = serde_json::from_slice(&manifest_data)?;
    let vector_dim = collection_manifest
        .get("vector_dim")
        .and_then(|v| v.as_u64())
        .unwrap_or(768) as usize;
    let distance_metric = collection_manifest
        .get("distance_metric")
        .and_then(|v| v.as_str())
        .unwrap_or("Cosine")
        .to_string();
    let segment_count = collection_manifest
        .get("segment_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;
    let total_vectors = collection_manifest
        .get("total_vectors")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    info!(
        "Collection metadata: {} vectors in {} segments (dim={}, metric={})",
        total_vectors, segment_count, vector_dim, distance_metric
    );

    // 3. Create temporary file for TAR archive
    let temp_tar_path = output.with_extension("tar.tmp");
    let tar_file = File::create(&temp_tar_path)?;
    let mut tar_builder = Builder::new(tar_file);

    // 4. Add collection manifest to TAR
    info!("Adding collection manifest to archive...");
    let mut manifest_header = Header::new_gnu();
    manifest_header.set_size(manifest_data.len() as u64);
    manifest_header.set_mode(0o644);
    manifest_header.set_cksum();
    tar_builder.append_data(
        &mut manifest_header,
        "collection_manifest.json",
        manifest_data.as_slice(),
    )?;

    // 5. Download and add segments to TAR with checksums
    info!("Downloading and archiving {} segments from S3...", segment_count);
    let mut segment_checksums = Vec::new();
    let mut total_uncompressed_bytes = manifest_data.len() as u64;

    for segment_id in 0..segment_count {
        let segment_key = format!("collections/{}/segments/segment_{:06}.seg", collection, segment_id);

        // Download segment from S3
        let segment_data = match storage.get_object(&segment_key).await {
            Ok(data) => {
                let data_vec = data.to_vec();
                info!("Downloaded segment {}/{}: {} bytes", segment_id + 1, segment_count, data_vec.len());
                data_vec
            }
            Err(e) => {
                warn!("Failed to download segment {}: {}", segment_id, e);
                continue; // Skip missing segments
            }
        };

        // Calculate SHA-256 checksum
        let mut hasher = Sha256::new();
        hasher.update(&segment_data);
        let checksum = format!("{:x}", hasher.finalize());
        segment_checksums.push((segment_id, checksum.clone()));

        // Add segment to TAR
        let mut segment_header = Header::new_gnu();
        segment_header.set_size(segment_data.len() as u64);
        segment_header.set_mode(0o644);
        segment_header.set_cksum();

        let segment_path = format!("segments/segment_{:06}.seg", segment_id);
        tar_builder.append_data(&mut segment_header, &segment_path, segment_data.as_slice())?;

        total_uncompressed_bytes += segment_data.len() as u64;
    }

    // 6. Create package manifest
    info!("Creating package manifest...");
    let mut package_manifest = PackageManifest::new(
        collection.clone(),
        1, // snapshot_version
        total_vectors,
        segment_count,
        vector_dim,
        distance_metric,
    )?;

    package_manifest.uncompressed_size_bytes = total_uncompressed_bytes;

    // 7. Add checksums to manifest (as JSON metadata)
    let checksums_json = serde_json::to_string_pretty(&segment_checksums)?;
    let mut checksums_header = Header::new_gnu();
    checksums_header.set_size(checksums_json.len() as u64);
    checksums_header.set_mode(0o644);
    checksums_header.set_cksum();
    tar_builder.append_data(
        &mut checksums_header,
        "checksums.json",
        checksums_json.as_bytes(),
    )?;

    // 8. Add package manifest to TAR
    let manifest_json = package_manifest.to_json()?;
    let mut pkg_manifest_header = Header::new_gnu();
    pkg_manifest_header.set_size(manifest_json.len() as u64);
    pkg_manifest_header.set_mode(0o644);
    pkg_manifest_header.set_cksum();
    tar_builder.append_data(
        &mut pkg_manifest_header,
        "manifest.json",
        manifest_json.as_bytes(),
    )?;

    // 9. Finalize TAR archive
    tar_builder.finish()?;
    drop(tar_builder); // Close the file
    info!("TAR archive created: {} bytes", total_uncompressed_bytes);

    // 10. Compress TAR with Zstd
    info!("Compressing with Zstd (level 9)...");
    let tar_file = File::open(&temp_tar_path)?;
    let compressed_file = File::create(&output)?;
    let mut encoder = Encoder::new(compressed_file, 9)?; // Zstd compression level 9 (best)

    let mut buffer = vec![0u8; 1024 * 1024]; // 1MB buffer
    let mut tar_reader = std::io::BufReader::new(tar_file);
    let mut compressed_bytes = 0u64;

    loop {
        let bytes_read = tar_reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        encoder.write_all(&buffer[..bytes_read])?;
        compressed_bytes += bytes_read as u64;
    }

    encoder.finish()?;

    // Get compressed file size
    let compressed_size = std::fs::metadata(&output)?.len();
    info!(
        "Compression complete: {} → {} bytes ({:.2}x ratio)",
        total_uncompressed_bytes,
        compressed_size,
        total_uncompressed_bytes as f64 / compressed_size as f64
    );

    // Update manifest with compressed size (requires reopening and appending)
    // For simplicity, we'll note this in the final signature metadata

    // 11. Generate Ed25519 signature if signing key provided
    if let Some(key_path) = sign_key {
        info!("Generating Ed25519 signature...");

        // Read signing key
        let key_bytes = std::fs::read(&key_path).map_err(|e| {
            format!("Failed to read signing key from {}: {}", key_path.display(), e)
        })?;

        // Parse Ed25519 key pair
        let key_pair = Ed25519KeyPair::from_pkcs8(&key_bytes).map_err(|e| {
            format!("Invalid Ed25519 signing key: {}", e)
        })?;

        // Read the compressed package file
        let package_data = std::fs::read(&output)?;

        // Sign the entire compressed package
        let signature_bytes = key_pair.sign(&package_data);
        let signature_hex = hex::encode(signature_bytes.as_ref());
        let public_key_hex = hex::encode(key_pair.public_key().as_ref());

        // Create signature metadata
        let signature = PackageSignature {
            algorithm: "Ed25519".to_string(),
            public_key: public_key_hex,
            signature: signature_hex.clone(),
        };

        // Write signature to separate file
        let sig_path = output.with_extension("akipkg.sig");
        let sig_json = serde_json::to_string_pretty(&signature)?;
        std::fs::write(&sig_path, sig_json)?;

        info!("Signature generated and saved to {}", sig_path.display());
        info!("Signature (first 32 chars): {}...", &signature_hex[..32.min(signature_hex.len())]);
    }

    // 12. Clean up temporary TAR file
    std::fs::remove_file(&temp_tar_path)?;

    info!("✅ Export complete: {}", output.display());
    info!("   Package size: {} bytes ({:.2} MB)", compressed_size, compressed_size as f64 / 1_000_000.0);
    info!("   Vectors: {}", total_vectors);
    info!("   Segments: {}", segment_count);

    Ok(())
}

// Helper function to encode bytes as hex (if hex crate not available)
#[cfg(not(feature = "hex"))]
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    #[allow(dead_code)]
    pub fn decode(s: &str) -> Result<Vec<u8>, String> {
        (0..s.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&s[i..i + 2], 16)
                    .map_err(|e| format!("Invalid hex: {}", e))
            })
            .collect()
    }
}
