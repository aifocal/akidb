//! SEGv1 Binary Segment Format Implementation
//!
//! This module implements the SEGv1 binary format for persisting vector segments to S3.
//!
//! ## Format Layout
//!
//! ```text
//! Header (64 bytes):
//! ├─ magic: [u8; 4]         = b"SEGv"
//! ├─ version: u32           = 1
//! ├─ dimension: u32
//! ├─ vector_count: u64
//! ├─ vector_offset: u64
//! ├─ metadata_offset: u64
//! ├─ bitmap_offset: u64
//! ├─ hnsw_offset: u64
//! ├─ checksum_type: u8
//! └─ reserved: [u8; 15]
//!
//! Vector Data Block:
//! ├─ compression_type: u8
//! ├─ compressed_size: u64
//! ├─ uncompressed_size: u64
//! └─ data: [u8; compressed_size]
//!
//! Footer (32 bytes):
//! └─ checksum: [u8; 32]
//! ```

use std::io::{self, Cursor, Read, Write};

use bytemuck;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use tracing::{debug, info};

use akidb_core::{Error, Result};

/// Magic bytes for SEGv1 format
const MAGIC: &[u8; 4] = b"SEGv";

/// Current format version
const VERSION: u32 = 1;

/// Fixed header size
const HEADER_SIZE: usize = 64;

/// Fixed checksum size
const CHECKSUM_SIZE: usize = 32;

/// Compression type for vector data
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CompressionType {
    None = 0,
    Zstd = 1,
}

impl CompressionType {
    pub fn from_u8(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Zstd),
            _ => Err(Error::Storage(format!(
                "Invalid compression type: {}",
                value
            ))),
        }
    }
}

/// Checksum algorithm type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ChecksumType {
    XXH3 = 1,
    CRC32C = 2,
}

impl ChecksumType {
    pub fn from_u8(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::XXH3),
            2 => Ok(Self::CRC32C),
            _ => Err(Error::Storage(format!("Invalid checksum type: {}", value))),
        }
    }
}

/// SEGv1 Header structure (64 bytes)
#[derive(Debug, Clone)]
pub struct SegmentHeader {
    pub magic: [u8; 4],              // 4 bytes
    pub version: u32,                // 4 bytes
    pub dimension: u32,              // 4 bytes
    pub vector_count: u64,           // 8 bytes
    pub vector_offset: u64,          // 8 bytes
    pub metadata_offset: u64,        // 8 bytes
    pub bitmap_offset: u64,          // 8 bytes
    pub hnsw_offset: u64,            // 8 bytes
    pub checksum_type: ChecksumType, // 1 byte
    pub reserved: [u8; 11],          // 11 bytes (total = 64)
}

impl SegmentHeader {
    /// Create a new segment header with default values
    pub fn new(dimension: u32, vector_count: u64) -> Self {
        Self {
            magic: *MAGIC,
            version: VERSION,
            dimension,
            vector_count,
            vector_offset: HEADER_SIZE as u64,
            metadata_offset: 0,
            bitmap_offset: 0,
            hnsw_offset: 0,
            checksum_type: ChecksumType::XXH3,
            reserved: [0; 11],
        }
    }

    /// Serialize header to bytes
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&self.magic)?;
        writer.write_u32::<LittleEndian>(self.version)?;
        writer.write_u32::<LittleEndian>(self.dimension)?;
        writer.write_u64::<LittleEndian>(self.vector_count)?;
        writer.write_u64::<LittleEndian>(self.vector_offset)?;
        writer.write_u64::<LittleEndian>(self.metadata_offset)?;
        writer.write_u64::<LittleEndian>(self.bitmap_offset)?;
        writer.write_u64::<LittleEndian>(self.hnsw_offset)?;
        writer.write_u8(self.checksum_type as u8)?;
        writer.write_all(&self.reserved)?;
        Ok(())
    }

    /// Deserialize header from bytes
    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        let mut magic = [0u8; 4];
        reader
            .read_exact(&mut magic)
            .map_err(|e| Error::Storage(format!("Failed to read magic: {}", e)))?;

        if &magic != MAGIC {
            return Err(Error::Storage(format!(
                "Invalid magic bytes: expected {:?}, got {:?}",
                MAGIC, magic
            )));
        }

        let version = reader
            .read_u32::<LittleEndian>()
            .map_err(|e| Error::Storage(format!("Failed to read version: {}", e)))?;

        if version != VERSION {
            return Err(Error::Storage(format!(
                "Unsupported version: expected {}, got {}",
                VERSION, version
            )));
        }

        let dimension = reader
            .read_u32::<LittleEndian>()
            .map_err(|e| Error::Storage(format!("Failed to read dimension: {}", e)))?;

        let vector_count = reader
            .read_u64::<LittleEndian>()
            .map_err(|e| Error::Storage(format!("Failed to read vector_count: {}", e)))?;

        let vector_offset = reader
            .read_u64::<LittleEndian>()
            .map_err(|e| Error::Storage(format!("Failed to read vector_offset: {}", e)))?;

        let metadata_offset = reader
            .read_u64::<LittleEndian>()
            .map_err(|e| Error::Storage(format!("Failed to read metadata_offset: {}", e)))?;

        let bitmap_offset = reader
            .read_u64::<LittleEndian>()
            .map_err(|e| Error::Storage(format!("Failed to read bitmap_offset: {}", e)))?;

        let hnsw_offset = reader
            .read_u64::<LittleEndian>()
            .map_err(|e| Error::Storage(format!("Failed to read hnsw_offset: {}", e)))?;

        let checksum_type_byte = reader
            .read_u8()
            .map_err(|e| Error::Storage(format!("Failed to read checksum_type: {}", e)))?;
        let checksum_type = ChecksumType::from_u8(checksum_type_byte)?;

        let mut reserved = [0u8; 11];
        reader
            .read_exact(&mut reserved)
            .map_err(|e| Error::Storage(format!("Failed to read reserved: {}", e)))?;

        Ok(Self {
            magic,
            version,
            dimension,
            vector_count,
            vector_offset,
            metadata_offset,
            bitmap_offset,
            hnsw_offset,
            checksum_type,
            reserved,
        })
    }
}

/// Segment data for serialization
#[derive(Debug, Clone)]
pub struct SegmentData {
    pub dimension: u32,
    pub vectors: Vec<Vec<f32>>,
    /// Optional metadata block for payload storage
    pub metadata: Option<crate::metadata::MetadataBlock>,
    // TODO: Add bitmaps, HNSW graph in future
}

impl SegmentData {
    pub fn new(dimension: u32, vectors: Vec<Vec<f32>>) -> Result<Self> {
        // Validate all vectors have correct dimension
        for (idx, vec) in vectors.iter().enumerate() {
            if vec.len() != dimension as usize {
                return Err(Error::Validation(format!(
                    "Vector at index {} has dimension {}, expected {}",
                    idx,
                    vec.len(),
                    dimension
                )));
            }
        }

        Ok(Self {
            dimension,
            vectors,
            metadata: None,
        })
    }

    /// Create segment data with metadata
    pub fn with_metadata(
        dimension: u32,
        vectors: Vec<Vec<f32>>,
        metadata: crate::metadata::MetadataBlock,
    ) -> Result<Self> {
        // Validate all vectors have correct dimension
        for (idx, vec) in vectors.iter().enumerate() {
            if vec.len() != dimension as usize {
                return Err(Error::Validation(format!(
                    "Vector at index {} has dimension {}, expected {}",
                    idx,
                    vec.len(),
                    dimension
                )));
            }
        }

        // Validate metadata row count matches vector count
        if metadata.batch.num_rows() != vectors.len() {
            return Err(Error::Validation(format!(
                "Metadata row count ({}) does not match vector count ({})",
                metadata.batch.num_rows(),
                vectors.len()
            )));
        }

        Ok(Self {
            dimension,
            vectors,
            metadata: Some(metadata),
        })
    }

    pub fn vector_count(&self) -> u64 {
        self.vectors.len() as u64
    }
}

/// Segment writer for SEGv1 format
pub struct SegmentWriter {
    compression: CompressionType,
    checksum_type: ChecksumType,
}

impl SegmentWriter {
    pub fn new(compression: CompressionType, checksum_type: ChecksumType) -> Self {
        Self {
            compression,
            checksum_type,
        }
    }

    /// Serialize segment data to bytes
    pub fn write(&self, data: &SegmentData) -> Result<Vec<u8>> {
        info!(
            "Writing segment with {} vectors, dimension {}, has_metadata: {}",
            data.vector_count(),
            data.dimension,
            data.metadata.is_some()
        );

        let mut buffer = Vec::new();

        // 1. Write header (placeholder, will update offsets later)
        let mut header = SegmentHeader::new(data.dimension, data.vector_count());
        header.checksum_type = self.checksum_type;
        header
            .write_to(&mut buffer)
            .map_err(|e| Error::Storage(format!("Failed to write header: {}", e)))?;

        // 2. Write vector data block
        let vector_offset = buffer.len() as u64;
        self.write_vector_block(&mut buffer, &data.vectors)?;

        // Update header with actual offset
        header.vector_offset = vector_offset;

        // 3. Write metadata block if present
        if let Some(ref metadata) = data.metadata {
            let metadata_offset = buffer.len() as u64;
            self.write_metadata_block(&mut buffer, metadata)?;
            header.metadata_offset = metadata_offset;
            debug!("Metadata block written at offset {}", metadata_offset);
        }

        // 4. Update header at the beginning BEFORE computing checksum
        let mut header_bytes = Vec::new();
        header
            .write_to(&mut header_bytes)
            .map_err(|e| Error::Storage(format!("Failed to write updated header: {}", e)))?;
        buffer[0..HEADER_SIZE].copy_from_slice(&header_bytes);

        // 5. Compute checksum over entire buffer (excluding checksum itself)
        let checksum = self.compute_checksum(&buffer)?;

        // 6. Write checksum footer
        buffer.extend_from_slice(&checksum);

        debug!(
            "Segment written: {} bytes (header: {}, data: {}, checksum: {})",
            buffer.len(),
            HEADER_SIZE,
            buffer.len() - HEADER_SIZE - CHECKSUM_SIZE,
            CHECKSUM_SIZE
        );

        Ok(buffer)
    }

    /// Write compressed vector data block
    fn write_vector_block(&self, buffer: &mut Vec<u8>, vectors: &[Vec<f32>]) -> Result<()> {
        // Flatten all vectors into a single f32 array
        let flat_vectors: Vec<f32> = vectors.iter().flat_map(|v| v.iter().copied()).collect();

        // Convert to bytes
        let vector_bytes: &[u8] = bytemuck::cast_slice(&flat_vectors);
        let uncompressed_size = vector_bytes.len() as u64;

        // Compress if needed
        let (compressed_data, compression_type) = match self.compression {
            CompressionType::None => (vector_bytes.to_vec(), CompressionType::None),
            CompressionType::Zstd => {
                let compressed = zstd::encode_all(vector_bytes, 3)
                    .map_err(|e| Error::Storage(format!("Failed to compress vectors: {}", e)))?;
                (compressed, CompressionType::Zstd)
            }
        };

        let compressed_size = compressed_data.len() as u64;

        // Write block header
        buffer
            .write_u8(compression_type as u8)
            .map_err(|e| Error::Storage(format!("Failed to write compression type: {}", e)))?;
        buffer
            .write_u64::<LittleEndian>(compressed_size)
            .map_err(|e| Error::Storage(format!("Failed to write compressed size: {}", e)))?;
        buffer
            .write_u64::<LittleEndian>(uncompressed_size)
            .map_err(|e| Error::Storage(format!("Failed to write uncompressed size: {}", e)))?;

        // Write compressed data
        buffer.extend_from_slice(&compressed_data);

        debug!(
            "Vector block: uncompressed {} bytes, compressed {} bytes ({:.1}% ratio)",
            uncompressed_size,
            compressed_size,
            (compressed_size as f64 / uncompressed_size as f64) * 100.0
        );

        Ok(())
    }

    /// Write metadata block using Arrow IPC format
    fn write_metadata_block(
        &self,
        buffer: &mut Vec<u8>,
        metadata: &crate::metadata::MetadataBlock,
    ) -> Result<()> {
        // Serialize metadata to Arrow IPC format
        let metadata_bytes = metadata
            .serialize()
            .map_err(|e| Error::Storage(format!("Failed to serialize metadata: {}", e)))?;

        let metadata_size = metadata_bytes.len() as u64;

        // Write metadata size first (for easier reading)
        buffer
            .write_u64::<LittleEndian>(metadata_size)
            .map_err(|e| Error::Storage(format!("Failed to write metadata size: {}", e)))?;

        // Write metadata bytes
        buffer.extend_from_slice(&metadata_bytes);

        debug!(
            "Metadata block: {} bytes ({} rows, {} columns)",
            metadata_size,
            metadata.batch.num_rows(),
            metadata.batch.num_columns()
        );

        Ok(())
    }

    /// Compute checksum for the entire buffer
    fn compute_checksum(&self, data: &[u8]) -> Result<[u8; CHECKSUM_SIZE]> {
        let mut checksum = [0u8; CHECKSUM_SIZE];

        match self.checksum_type {
            ChecksumType::XXH3 => {
                let hash = xxhash_rust::xxh3::xxh3_128(data);
                checksum[0..16].copy_from_slice(&hash.to_le_bytes());
            }
            ChecksumType::CRC32C => {
                let crc = crc32c::crc32c(data);
                checksum[0..4].copy_from_slice(&crc.to_le_bytes());
            }
        }

        Ok(checksum)
    }
}

/// Segment reader for SEGv1 format
pub struct SegmentReader;

impl SegmentReader {
    /// Deserialize segment data from bytes
    pub fn read(data: &[u8]) -> Result<SegmentData> {
        if data.len() < HEADER_SIZE + CHECKSUM_SIZE {
            return Err(Error::Storage(format!(
                "Data too small: {} bytes, expected at least {}",
                data.len(),
                HEADER_SIZE + CHECKSUM_SIZE
            )));
        }

        // 1. Read and parse header
        let mut cursor = Cursor::new(&data[0..HEADER_SIZE]);
        let header = SegmentHeader::read_from(&mut cursor)?;

        debug!(
            "Reading segment: {} vectors, dimension {}",
            header.vector_count, header.dimension
        );

        // 2. Verify checksum
        let data_without_checksum = &data[0..data.len() - CHECKSUM_SIZE];
        let stored_checksum = &data[data.len() - CHECKSUM_SIZE..];
        Self::verify_checksum(data_without_checksum, stored_checksum, header.checksum_type)?;

        // 3. Read vector data block
        // BUGFIX (Bug #30): Validate u64 to usize cast to prevent truncation on 32-bit systems
        let vector_offset_usize = usize::try_from(header.vector_offset).map_err(|_| {
            Error::Storage(format!(
                "Vector offset {} exceeds usize::MAX on this platform ({}). \
                 Cannot read vector data block from segment.",
                header.vector_offset,
                usize::MAX
            ))
        })?;

        let vectors = Self::read_vector_block(
            &data[vector_offset_usize..],
            header.dimension,
            header.vector_count,
        )?;

        // 4. Read metadata block if present
        let metadata = if header.metadata_offset > 0 {
            // Read metadata from offset to end minus checksum
            let metadata_end = data.len() - CHECKSUM_SIZE;

            // BUGFIX (Bug #30): Validate u64 to usize cast to prevent truncation on 32-bit systems
            let metadata_offset_usize = usize::try_from(header.metadata_offset).map_err(|_| {
                Error::Storage(format!(
                    "Metadata offset {} exceeds usize::MAX on this platform ({}). \
                     Cannot read metadata block from segment.",
                    header.metadata_offset,
                    usize::MAX
                ))
            })?;

            Some(Self::read_metadata_block(
                &data[metadata_offset_usize..metadata_end],
            )?)
        } else {
            None
        };

        Ok(SegmentData {
            dimension: header.dimension,
            vectors,
            metadata,
        })
    }

    /// Read and decompress vector data block
    fn read_vector_block(data: &[u8], dimension: u32, vector_count: u64) -> Result<Vec<Vec<f32>>> {
        let mut cursor = Cursor::new(data);

        // Read block header
        let compression_type_byte = cursor
            .read_u8()
            .map_err(|e| Error::Storage(format!("Failed to read compression type: {}", e)))?;
        let compression_type = CompressionType::from_u8(compression_type_byte)?;

        let compressed_size = cursor
            .read_u64::<LittleEndian>()
            .map_err(|e| Error::Storage(format!("Failed to read compressed size: {}", e)))?;

        let uncompressed_size = cursor
            .read_u64::<LittleEndian>()
            .map_err(|e| Error::Storage(format!("Failed to read uncompressed size: {}", e)))?;

        // Read compressed data
        // BUGFIX: Validate u64 to usize cast to prevent truncation on 32-bit systems
        let compressed_size_usize = usize::try_from(compressed_size).map_err(|_| {
            Error::Storage(format!(
                "Compressed size {} exceeds usize::MAX on this platform ({}). \
                 Cannot allocate buffer for reading compressed data.",
                compressed_size,
                usize::MAX
            ))
        })?;

        let mut compressed_data = vec![0u8; compressed_size_usize];
        cursor
            .read_exact(&mut compressed_data)
            .map_err(|e| Error::Storage(format!("Failed to read compressed data: {}", e)))?;

        // Decompress
        let vector_bytes = match compression_type {
            CompressionType::None => compressed_data,
            CompressionType::Zstd => zstd::decode_all(&compressed_data[..])
                .map_err(|e| Error::Storage(format!("Failed to decompress vectors: {}", e)))?,
        };

        // Verify size matches
        // BUGFIX: Validate u64 to usize cast before comparison
        let uncompressed_size_usize = usize::try_from(uncompressed_size).map_err(|_| {
            Error::Storage(format!(
                "Uncompressed size {} exceeds usize::MAX on this platform ({})",
                uncompressed_size,
                usize::MAX
            ))
        })?;

        if vector_bytes.len() != uncompressed_size_usize {
            return Err(Error::Storage(format!(
                "Decompressed size mismatch: expected {}, got {}",
                uncompressed_size,
                vector_bytes.len()
            )));
        }

        // Convert bytes back to f32 array
        let flat_vectors: &[f32] = bytemuck::cast_slice(&vector_bytes);

        // BUGFIX: Validate u64 to usize cast for vector_count (moved before use)
        let vector_count_usize = usize::try_from(vector_count).map_err(|_| {
            Error::Storage(format!(
                "Vector count {} exceeds usize::MAX on this platform ({})",
                vector_count,
                usize::MAX
            ))
        })?;

        // BUGFIX: Validate u32 to usize cast for dimension (unlikely to fail but defensive)
        let dimension_usize = usize::try_from(dimension).map_err(|_| {
            Error::Storage(format!(
                "Dimension {} exceeds usize::MAX on this platform ({})",
                dimension,
                usize::MAX
            ))
        })?;

        // BUGFIX: Check for multiplication overflow when calculating expected size
        let expected_total = dimension_usize.checked_mul(vector_count_usize).ok_or_else(|| {
            Error::Storage(format!(
                "Vector size calculation overflow: {} × {} exceeds usize::MAX",
                dimension_usize, vector_count_usize
            ))
        })?;

        if flat_vectors.len() != expected_total {
            return Err(Error::Storage(format!(
                "Vector data size mismatch: expected {} floats ({}×{}), got {}",
                expected_total, vector_count, dimension, flat_vectors.len()
            )));
        }

        let mut vectors = Vec::with_capacity(vector_count_usize);
        for chunk in flat_vectors.chunks_exact(dimension_usize) {
            vectors.push(chunk.to_vec());
        }

        debug!(
            "Read {} vectors (dimension {}), {} bytes",
            vectors.len(),
            dimension,
            vector_bytes.len()
        );

        Ok(vectors)
    }

    /// Verify checksum matches
    fn verify_checksum(data: &[u8], stored: &[u8], checksum_type: ChecksumType) -> Result<()> {
        let mut computed = [0u8; CHECKSUM_SIZE];

        match checksum_type {
            ChecksumType::XXH3 => {
                let hash = xxhash_rust::xxh3::xxh3_128(data);
                computed[0..16].copy_from_slice(&hash.to_le_bytes());
            }
            ChecksumType::CRC32C => {
                let crc = crc32c::crc32c(data);
                computed[0..4].copy_from_slice(&crc.to_le_bytes());
            }
        }

        if &computed[..] != stored {
            return Err(Error::Storage(
                "Checksum verification failed: data may be corrupted".to_string(),
            ));
        }

        debug!("Checksum verified successfully ({:?})", checksum_type);
        Ok(())
    }

    /// Read metadata block from Arrow IPC format
    fn read_metadata_block(data: &[u8]) -> Result<crate::metadata::MetadataBlock> {
        let mut cursor = Cursor::new(data);

        // Read metadata size
        let metadata_size = cursor
            .read_u64::<LittleEndian>()
            .map_err(|e| Error::Storage(format!("Failed to read metadata size: {}", e)))?;

        // Read metadata bytes
        // BUGFIX: Validate u64 to usize cast to prevent truncation on 32-bit systems
        let metadata_size_usize = usize::try_from(metadata_size).map_err(|_| {
            Error::Storage(format!(
                "Metadata size {} exceeds usize::MAX on this platform ({}). \
                 Cannot allocate buffer for reading metadata.",
                metadata_size,
                usize::MAX
            ))
        })?;

        let mut metadata_bytes = vec![0u8; metadata_size_usize];
        cursor
            .read_exact(&mut metadata_bytes)
            .map_err(|e| Error::Storage(format!("Failed to read metadata bytes: {}", e)))?;

        // Deserialize from Arrow IPC
        let metadata = crate::metadata::MetadataBlock::deserialize(&metadata_bytes)
            .map_err(|e| Error::Storage(format!("Failed to deserialize metadata: {}", e)))?;

        debug!(
            "Read metadata block: {} rows, {} columns",
            metadata.batch.num_rows(),
            metadata.batch.num_columns()
        );

        Ok(metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_serialization() {
        let header = SegmentHeader::new(768, 1000);

        let mut buffer = Vec::new();
        header.write_to(&mut buffer).unwrap();

        assert_eq!(buffer.len(), HEADER_SIZE);

        let mut cursor = Cursor::new(&buffer);
        let deserialized = SegmentHeader::read_from(&mut cursor).unwrap();

        assert_eq!(deserialized.dimension, 768);
        assert_eq!(deserialized.vector_count, 1000);
        assert_eq!(deserialized.version, VERSION);
    }

    #[test]
    fn test_segment_roundtrip_no_compression() {
        let vectors = vec![
            vec![1.0, 2.0, 3.0],
            vec![4.0, 5.0, 6.0],
            vec![7.0, 8.0, 9.0],
        ];

        let data = SegmentData::new(3, vectors.clone()).unwrap();

        let writer = SegmentWriter::new(CompressionType::None, ChecksumType::XXH3);
        let bytes = writer.write(&data).unwrap();

        let recovered = SegmentReader::read(&bytes).unwrap();

        assert_eq!(recovered.dimension, 3);
        assert_eq!(recovered.vectors.len(), 3);
        assert_eq!(recovered.vectors, vectors);
    }

    #[test]
    fn test_segment_roundtrip_with_compression() {
        let vectors = vec![
            vec![1.0, 2.0, 3.0, 4.0],
            vec![5.0, 6.0, 7.0, 8.0],
            vec![9.0, 10.0, 11.0, 12.0],
        ];

        let data = SegmentData::new(4, vectors.clone()).unwrap();

        let writer = SegmentWriter::new(CompressionType::Zstd, ChecksumType::XXH3);
        let bytes = writer.write(&data).unwrap();

        let recovered = SegmentReader::read(&bytes).unwrap();

        assert_eq!(recovered.dimension, 4);
        assert_eq!(recovered.vectors.len(), 3);
        assert_eq!(recovered.vectors, vectors);
    }

    #[test]
    fn test_invalid_dimension() {
        let vectors = vec![
            vec![1.0, 2.0, 3.0],
            vec![4.0, 5.0], // Wrong dimension!
        ];

        let result = SegmentData::new(3, vectors);
        assert!(result.is_err());
    }

    #[test]
    fn test_checksum_corruption_detection() {
        let vectors = vec![vec![1.0, 2.0, 3.0]];
        let data = SegmentData::new(3, vectors).unwrap();

        let writer = SegmentWriter::new(CompressionType::None, ChecksumType::XXH3);
        let mut bytes = writer.write(&data).unwrap();

        // Corrupt some data
        bytes[100] ^= 0xFF;

        let result = SegmentReader::read(&bytes);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Checksum verification failed"));
    }

    #[test]
    fn test_large_segment() {
        // Test with 1000 vectors of dimension 128
        let vectors: Vec<Vec<f32>> = (0..1000)
            .map(|i| (0..128).map(|j| (i * 128 + j) as f32).collect())
            .collect();

        let data = SegmentData::new(128, vectors.clone()).unwrap();

        let writer = SegmentWriter::new(CompressionType::Zstd, ChecksumType::XXH3);
        let bytes = writer.write(&data).unwrap();

        println!("Large segment size: {} bytes", bytes.len());

        let recovered = SegmentReader::read(&bytes).unwrap();

        assert_eq!(recovered.dimension, 128);
        assert_eq!(recovered.vectors.len(), 1000);
        assert_eq!(recovered.vectors, vectors);
    }

    #[test]
    fn test_segment_with_metadata() {
        use serde_json::json;

        // Create vectors
        let vectors = vec![
            vec![1.0, 2.0, 3.0],
            vec![4.0, 5.0, 6.0],
            vec![7.0, 8.0, 9.0],
        ];

        // Create metadata payloads
        let payloads = vec![
            json!({"name": "Vector 1", "category": "A", "score": 0.95}),
            json!({"name": "Vector 2", "category": "B", "score": 0.87}),
            json!({"name": "Vector 3", "category": "A", "score": 0.92}),
        ];

        // Convert to MetadataBlock
        let metadata = crate::metadata::MetadataBlock::from_json(payloads.clone()).unwrap();

        // Create segment data with metadata
        let data = SegmentData::with_metadata(3, vectors.clone(), metadata).unwrap();

        // Write segment
        let writer = SegmentWriter::new(CompressionType::Zstd, ChecksumType::XXH3);
        let bytes = writer.write(&data).unwrap();

        println!("Segment with metadata size: {} bytes", bytes.len());

        // Read segment
        let recovered = SegmentReader::read(&bytes).unwrap();

        // Verify vectors
        assert_eq!(recovered.dimension, 3);
        assert_eq!(recovered.vectors.len(), 3);
        assert_eq!(recovered.vectors, vectors);

        // Verify metadata
        assert!(recovered.metadata.is_some());
        let recovered_metadata = recovered.metadata.unwrap();
        assert_eq!(recovered_metadata.batch.num_rows(), 3);
        assert_eq!(recovered_metadata.batch.num_columns(), 3); // name, category, score

        // Convert back to JSON and verify
        let recovered_payloads = recovered_metadata.to_json().unwrap();
        assert_eq!(recovered_payloads.len(), 3);
        assert_eq!(recovered_payloads[0]["name"], "Vector 1");
        assert_eq!(recovered_payloads[1]["category"], "B");
        assert_eq!(recovered_payloads[2]["score"], 0.92);
    }

    #[test]
    fn test_segment_without_metadata_backward_compatible() {
        // Test that segments without metadata can still be read
        let vectors = vec![vec![1.0, 2.0], vec![3.0, 4.0]];

        let data = SegmentData::new(2, vectors.clone()).unwrap();

        let writer = SegmentWriter::new(CompressionType::None, ChecksumType::XXH3);
        let bytes = writer.write(&data).unwrap();

        let recovered = SegmentReader::read(&bytes).unwrap();

        assert_eq!(recovered.dimension, 2);
        assert_eq!(recovered.vectors, vectors);
        assert!(recovered.metadata.is_none()); // No metadata
    }

    #[test]
    fn test_metadata_row_count_mismatch() {
        use serde_json::json;

        let vectors = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let payloads = vec![
            json!({"name": "Only one"}),
            // Missing second payload!
        ];

        let metadata = crate::metadata::MetadataBlock::from_json(payloads).unwrap();

        // Should fail because metadata has 1 row but vectors have 2
        let result = SegmentData::with_metadata(2, vectors, metadata);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not match"));
    }
}
