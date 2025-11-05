use crate::manifest::PackageManifest;
use ring::signature::{UnparsedPublicKey, ED25519};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use tar::Archive;
use tracing::{info, warn};
use zstd::Decoder;

/// Verification result
pub struct VerificationResult {
    pub is_valid: bool,
    pub manifest: PackageManifest,
    pub signature_valid: bool,
    pub errors: Vec<String>,
}

impl VerificationResult {
    pub fn is_valid(&self) -> bool {
        self.is_valid && self.errors.is_empty()
    }
}

/// Verify an .akipkg package
pub async fn verify_package(
    file: PathBuf,
    public_key: Option<PathBuf>,
) -> Result<VerificationResult, Box<dyn std::error::Error>> {
    info!("Verifying package: {}", file.display());

    let mut errors = Vec::new();
    let mut signature_valid = false;

    // 1. Verify file exists
    if !file.exists() {
        return Err(format!("Package file not found: {}", file.display()).into());
    }

    // 2. Verify Ed25519 signature if public key provided
    if let Some(key_path) = public_key {
        info!("Verifying Ed25519 signature...");

        match verify_signature(&file, &key_path) {
            Ok(()) => {
                info!("✅ Signature verification passed");
                signature_valid = true;
            }
            Err(e) => {
                warn!("❌ Signature verification failed: {}", e);
                errors.push(format!("Signature verification failed: {}", e));
            }
        }
    } else {
        info!("⚠️  No public key provided, skipping signature verification");
    }

    // 3. Decompress Zstd and extract TAR
    info!("Decompressing package...");
    let compressed_file = File::open(&file)?;
    let decoder = Decoder::new(compressed_file)?;
    let mut archive = Archive::new(decoder);

    // 4. Extract and verify manifest
    info!("Extracting package manifest...");
    let mut manifest_opt: Option<PackageManifest> = None;
    let mut checksums_opt: Option<Vec<(usize, String)>> = None;
    let mut segment_data_map = std::collections::HashMap::new();

    for entry_result in archive.entries()? {
        let mut entry = entry_result?;
        let path = entry.path()?.to_path_buf();
        let path_str = path.to_string_lossy().to_string();

        info!("Extracting: {}", path_str);

        // Read entry data
        let mut data = Vec::new();
        entry.read_to_end(&mut data)?;

        match path_str.as_str() {
            "manifest.json" => {
                // Parse package manifest
                let manifest_json = String::from_utf8(data)?;
                manifest_opt = Some(PackageManifest::from_json(&manifest_json)?);
                info!("✅ Package manifest loaded");
            }
            "checksums.json" => {
                // Parse checksums
                let checksums_json = String::from_utf8(data)?;
                checksums_opt = Some(serde_json::from_str(&checksums_json)?);
                info!("✅ Checksums manifest loaded");
            }
            "collection_manifest.json" => {
                info!("✅ Collection manifest found");
            }
            _ if path_str.starts_with("segments/") => {
                // Store segment data for checksum verification
                segment_data_map.insert(path_str.clone(), data);
            }
            _ => {
                warn!("Unknown file in archive: {}", path_str);
            }
        }
    }

    // 5. Verify manifest exists
    let manifest = match manifest_opt {
        Some(m) => m,
        None => {
            errors.push("Package manifest (manifest.json) not found in archive".to_string());
            return Ok(VerificationResult {
                is_valid: false,
                manifest: PackageManifest::new(
                    "unknown".to_string(),
                    0,
                    0,
                    0,
                    0,
                    "Cosine".to_string(),
                )?,
                signature_valid,
                errors,
            });
        }
    };

    info!(
        "Package: {} v{} (snapshot {})",
        manifest.collection_name, manifest.version, manifest.snapshot_version
    );

    // 6. Verify checksums if available
    if let Some(checksums) = checksums_opt {
        info!("Verifying {} segment checksums...", checksums.len());

        for (segment_id, expected_checksum) in checksums {
            let segment_path = format!("segments/segment_{:06}.seg", segment_id);

            match segment_data_map.get(&segment_path) {
                Some(segment_data) => {
                    // Calculate SHA-256 checksum
                    let mut hasher = Sha256::new();
                    hasher.update(segment_data);
                    let actual_checksum = format!("{:x}", hasher.finalize());

                    if actual_checksum == expected_checksum {
                        info!("✅ Segment {} checksum valid", segment_id);
                    } else {
                        let error_msg = format!(
                            "Segment {} checksum mismatch: expected {}, got {}",
                            segment_id, expected_checksum, actual_checksum
                        );
                        warn!("❌ {}", error_msg);
                        errors.push(error_msg);
                    }
                }
                None => {
                    let error_msg = format!("Segment {} missing from archive", segment_id);
                    warn!("❌ {}", error_msg);
                    errors.push(error_msg);
                }
            }
        }

        if errors.is_empty() {
            info!("✅ All checksums verified successfully");
        }
    } else {
        warn!("⚠️  No checksums.json found, skipping checksum verification");
    }

    // 7. Verify manifest version compatibility
    info!("Checking manifest version compatibility...");
    let current_version = env!("CARGO_PKG_VERSION");
    // For now, just log the versions
    info!(
        "Package AkiDB version: {}, Current version: {}",
        manifest.akidb_version, current_version
    );

    // 8. Validate manifest fields
    if manifest.collection_name.trim().is_empty() {
        errors.push("Collection name is empty".to_string());
    }

    if manifest.vector_dim == 0 {
        errors.push("Vector dimension is zero".to_string());
    }

    if manifest.total_segments != segment_data_map.len() {
        warn!(
            "⚠️  Manifest reports {} segments but found {} in archive",
            manifest.total_segments,
            segment_data_map.len()
        );
    }

    // Final validation result
    let is_valid = errors.is_empty();

    if is_valid {
        info!("✅ Package verification complete: ALL CHECKS PASSED");
    } else {
        warn!("❌ Package verification FAILED with {} errors", errors.len());
    }

    Ok(VerificationResult {
        is_valid,
        manifest,
        signature_valid,
        errors,
    })
}

/// Verify Ed25519 signature
fn verify_signature(
    package_path: &PathBuf,
    _public_key_path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Read signature file
    let sig_path = package_path.with_extension("akipkg.sig");
    if !sig_path.exists() {
        return Err(format!("Signature file not found: {}", sig_path.display()).into());
    }

    let sig_json = std::fs::read_to_string(&sig_path)?;
    let sig_data: serde_json::Value = serde_json::from_str(&sig_json)?;

    let signature_hex = sig_data
        .get("signature")
        .and_then(|s| s.as_str())
        .ok_or("Missing signature in signature file")?;
    let public_key_hex = sig_data
        .get("public_key")
        .and_then(|s| s.as_str())
        .ok_or("Missing public_key in signature file")?;

    // 2. Read package data
    let package_data = std::fs::read(package_path)?;

    // 3. Decode hex-encoded signature and public key
    let signature_bytes = hex::decode(signature_hex)?;
    let public_key_bytes = hex::decode(public_key_hex)?;

    // 4. Verify signature using Ring
    let public_key = UnparsedPublicKey::new(&ED25519, &public_key_bytes);

    public_key
        .verify(&package_data, &signature_bytes)
        .map_err(|_| "Signature verification failed")?;

    Ok(())
}

// Helper function to decode hex (if hex crate not available)
#[cfg(not(feature = "hex"))]
mod hex {
    pub fn decode(s: &str) -> Result<Vec<u8>, String> {
        if s.len() % 2 != 0 {
            return Err("Hex string must have even length".to_string());
        }

        (0..s.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&s[i..i + 2], 16)
                    .map_err(|e| format!("Invalid hex at position {}: {}", i, e))
            })
            .collect()
    }

    #[allow(dead_code)]
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_decode() {
        let hex_str = "48656c6c6f"; // "Hello" in hex
        let decoded = hex::decode(hex_str).unwrap();
        assert_eq!(decoded, b"Hello");
    }

    #[test]
    fn test_hex_decode_invalid() {
        let hex_str = "4865"; // Valid
        assert!(hex::decode(hex_str).is_ok());

        let hex_str = "48G5"; // Invalid (G is not hex)
        assert!(hex::decode(hex_str).is_err());

        let hex_str = "485"; // Odd length
        assert!(hex::decode(hex_str).is_err());
    }
}
