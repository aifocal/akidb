//! Embedding service infrastructure for AkiDB 2.0.
//!
//! This crate provides trait definitions and implementations for text embedding generation.
//! The architecture supports multiple backends (MLX, ONNX, etc.) through the `EmbeddingProvider` trait.
//!
//! # Bug Fix #5: Feature-gated MLX
//!
//! The MLX embedding provider is now behind the "mlx" feature flag to improve portability.
//! Enable with: `cargo build --features mlx` (enabled by default)
//! Disable for Python-free builds: `cargo build --no-default-features`

#[cfg(feature = "mlx")]
mod mlx;
#[cfg(feature = "candle")]
mod candle;
#[cfg(feature = "onnx")]
mod onnx;
mod mock;
mod provider;
mod types;

#[cfg(feature = "mlx")]
pub use mlx::MlxEmbeddingProvider;
#[cfg(feature = "candle")]
pub use candle::CandleEmbeddingProvider;
#[cfg(feature = "onnx")]
pub use onnx::OnnxEmbeddingProvider;
pub use mock::MockEmbeddingProvider;
pub use provider::EmbeddingProvider;
pub use types::{
    BatchEmbeddingRequest, BatchEmbeddingResponse, EmbeddingError, EmbeddingResult, ModelInfo,
    Usage,
};
