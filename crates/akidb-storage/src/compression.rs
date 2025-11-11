//! Gzip compression for S3 uploads

use akidb_core::error::{CoreError, CoreResult};
use bytes::Bytes;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::{Read, Write};

/// Compression configuration
#[derive(Debug, Clone, Copy)]
pub struct CompressionConfig {
    /// Compression level (0-9, default: 6)
    pub level: u32,
    /// Enable compression (default: true)
    pub enabled: bool,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            level: 6, // Balanced compression
            enabled: true,
        }
    }
}

impl CompressionConfig {
    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.level > 9 {
            return Err(format!(
                "Invalid compression level: {} (max: 9)",
                self.level
            ));
        }
        Ok(())
    }

    /// Convert compression level to flate2::Compression
    pub fn compression_level(&self) -> Compression {
        match self.level {
            0 => Compression::none(),
            1 => Compression::fast(),
            6 => Compression::default(),
            9 => Compression::best(),
            n => Compression::new(n),
        }
    }
}

/// Compress bytes with gzip
pub fn compress(data: &[u8], config: CompressionConfig) -> CoreResult<Bytes> {
    if !config.enabled {
        return Ok(Bytes::copy_from_slice(data));
    }

    let mut encoder = GzEncoder::new(Vec::new(), config.compression_level());
    encoder
        .write_all(data)
        .map_err(|e| CoreError::SerializationError(format!("Gzip compression failed: {}", e)))?;

    let compressed = encoder
        .finish()
        .map_err(|e| CoreError::SerializationError(format!("Gzip finish failed: {}", e)))?;

    Ok(Bytes::from(compressed))
}

/// Decompress gzip bytes
pub fn decompress(data: &[u8]) -> CoreResult<Bytes> {
    let mut decoder = GzDecoder::new(data);
    let mut decompressed = Vec::new();

    decoder.read_to_end(&mut decompressed).map_err(|e| {
        CoreError::DeserializationError(format!("Gzip decompression failed: {}", e))
    })?;

    Ok(Bytes::from(decompressed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gzip_compress_decompress() {
        let data = b"Hello, world! This is test data for compression.";
        let config = CompressionConfig::default();

        let compressed = compress(data, config).unwrap();
        let decompressed = decompress(&compressed).unwrap();

        // Round-trip should preserve data
        assert_eq!(decompressed.as_ref(), data);

        // Note: For small data, gzip overhead might make it larger
        // This is expected and acceptable
        println!(
            "Original: {} bytes, Compressed: {} bytes, Ratio: {:.2}x",
            data.len(),
            compressed.len(),
            data.len() as f64 / compressed.len() as f64
        );
    }

    #[test]
    fn test_gzip_repetitive_data() {
        // Highly compressible data
        let data = vec![0u8; 10_000];
        let config = CompressionConfig::default();

        let compressed = compress(&data, config).unwrap();

        let ratio = data.len() as f64 / compressed.len() as f64;

        println!(
            "Repetitive data: {} bytes → {} bytes, Ratio: {:.2}x",
            data.len(),
            compressed.len(),
            ratio
        );

        // Should get excellent compression on repetitive data
        assert!(
            ratio >= 100.0,
            "Expected high compression ratio, got {:.2}x",
            ratio
        );
    }

    #[test]
    fn test_gzip_random_data() {
        // Not compressible (random)
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let data: Vec<u8> = (0..10_000).map(|_| rng.gen()).collect();

        let config = CompressionConfig::default();
        let compressed = compress(&data, config).unwrap();

        let ratio = data.len() as f64 / compressed.len() as f64;

        println!(
            "Random data: {} bytes → {} bytes, Ratio: {:.2}x",
            data.len(),
            compressed.len(),
            ratio
        );

        // Random data should barely compress (might even expand slightly)
        assert!(ratio >= 0.9, "Unexpected expansion: {:.2}x", ratio);
    }

    #[test]
    fn test_gzip_disabled() {
        let data = b"Test data";
        let config = CompressionConfig {
            level: 6,
            enabled: false, // Disabled
        };

        let compressed = compress(data, config).unwrap();

        // Should be unchanged
        assert_eq!(compressed.as_ref(), data);
    }

    #[test]
    fn test_compression_levels() {
        let data = vec![0u8; 10_000];

        for level in 0..=9 {
            let config = CompressionConfig {
                level,
                enabled: true,
            };

            let compressed = compress(&data, config).unwrap();

            println!(
                "Level {}: {} bytes ({:.2}x)",
                level,
                compressed.len(),
                data.len() as f64 / compressed.len() as f64
            );

            // Higher levels should generally compress more (for repetitive data)
            if level > 0 {
                assert!(compressed.len() < data.len());
            }
        }
    }
}
