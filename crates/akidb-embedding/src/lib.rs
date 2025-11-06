//! Embedding service infrastructure for AkiDB 2.0.
//!
//! This crate provides trait definitions and implementations for text embedding generation.
//! The architecture supports multiple backends (MLX, ONNX, etc.) through the `EmbeddingProvider` trait.

mod mock;
mod provider;
mod types;

pub use mock::MockEmbeddingProvider;
pub use provider::EmbeddingProvider;
pub use types::{
    BatchEmbeddingRequest, BatchEmbeddingResponse, EmbeddingError, EmbeddingResult, ModelInfo,
    Usage,
};
