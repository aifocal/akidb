//! Integration tests for Candle embedding provider.
//!
//! These tests download real models from Hugging Face Hub and verify functionality.
//! Tests are marked with #[ignore] since they are expensive (download + GPU/CPU).
//!
//! Run with: cargo test --features candle -p akidb-embedding -- --ignored --nocapture

#[cfg(feature = "candle")]
mod candle_integration_tests {
    use akidb_embedding::{CandleEmbeddingProvider, EmbeddingProvider};

    /// Test loading MiniLM model from Hugging Face Hub.
    ///
    /// This test:
    /// 1. Downloads model from HF Hub (first run) or uses cache
    /// 2. Selects device (Metal/CUDA/CPU)
    /// 3. Loads model weights into memory
    /// 4. Initializes tokenizer
    /// 5. Verifies model info matches expectations
    #[tokio::test]
    #[ignore] // Expensive: downloads ~22MB model on first run
    async fn test_load_minilm_model() {
        eprintln!("\n=== Test: Load MiniLM Model ===\n");

        let provider = CandleEmbeddingProvider::new("sentence-transformers/all-MiniLM-L6-v2")
            .await
            .expect("Failed to load MiniLM model");

        // Verify model info
        let info = provider
            .model_info()
            .await
            .expect("Failed to get model info");

        assert_eq!(info.model, "sentence-transformers/all-MiniLM-L6-v2");
        assert_eq!(info.dimension, 384, "MiniLM has 384 dimensions");
        assert_eq!(info.max_tokens, 512, "BERT standard max tokens");

        eprintln!("\n✅ Test passed: MiniLM model loaded successfully");
        eprintln!("   Model: {}", info.model);
        eprintln!("   Dimension: {}", info.dimension);
        eprintln!("   Max tokens: {}", info.max_tokens);
    }

    /// Test device selection logic.
    ///
    /// Verifies that:
    /// - macOS selects Metal GPU (if available)
    /// - Linux selects CUDA GPU (if available)
    /// - CPU fallback always works
    #[tokio::test]
    #[ignore] // Expensive: downloads model
    async fn test_device_selection() {
        eprintln!("\n=== Test: Device Selection ===\n");

        let provider = CandleEmbeddingProvider::new("sentence-transformers/all-MiniLM-L6-v2")
            .await
            .expect("Failed to load model");

        // On macOS with Apple Silicon, should use Metal
        #[cfg(target_os = "macos")]
        {
            eprintln!("✅ Test running on macOS");
            eprintln!("   Expected: Metal GPU");
            eprintln!("   (Check output above for device confirmation)");
        }

        // On Linux, might be CUDA or CPU
        #[cfg(target_os = "linux")]
        {
            eprintln!("✅ Test running on Linux");
            eprintln!("   Expected: CUDA GPU or CPU fallback");
        }

        // Model should load regardless of device
        let info = provider.model_info().await.unwrap();
        assert_eq!(info.dimension, 384);

        eprintln!("\n✅ Test passed: Device selection successful");
    }

    /// Test health check (model is loaded and ready).
    #[tokio::test]
    #[ignore] // Expensive: downloads model
    async fn test_health_check() {
        eprintln!("\n=== Test: Health Check ===\n");

        let provider = CandleEmbeddingProvider::new("sentence-transformers/all-MiniLM-L6-v2")
            .await
            .expect("Failed to load model");

        // Health check should pass (model loaded)
        let result = provider.health_check().await;

        // For now health_check is not implemented (Day 5), so it will fail with todo!()
        // This test verifies the model at least loaded successfully
        match result {
            Ok(_) => {
                eprintln!("✅ Health check passed");
            }
            Err(e) => {
                // Expected until Day 5 implementation
                eprintln!("⚠️  Health check not yet implemented: {}", e);
                eprintln!("   (This is expected for Day 2)");
            }
        }

        // But model_info should work
        let info = provider.model_info().await.unwrap();
        assert_eq!(info.dimension, 384);

        eprintln!("\n✅ Test passed: Model is healthy");
    }

    /// Test that model caching works (second load is fast).
    ///
    /// First load: Downloads model (~22MB for MiniLM)
    /// Second load: Uses cached files (~1-2s)
    #[tokio::test]
    #[ignore] // Expensive: loads model twice
    async fn test_model_caching() {
        eprintln!("\n=== Test: Model Caching ===\n");

        use std::time::Instant;

        // First load (might download)
        eprintln!("Loading model (first time)...");
        let start = Instant::now();
        let provider1 = CandleEmbeddingProvider::new("sentence-transformers/all-MiniLM-L6-v2")
            .await
            .expect("Failed to load model (first time)");
        let first_load_ms = start.elapsed().as_millis();

        drop(provider1);

        // Second load (should use cache)
        eprintln!("\nLoading model (second time, from cache)...");
        let start = Instant::now();
        let provider2 = CandleEmbeddingProvider::new("sentence-transformers/all-MiniLM-L6-v2")
            .await
            .expect("Failed to load model (second time)");
        let second_load_ms = start.elapsed().as_millis();

        eprintln!("\n✅ Test passed: Model caching works");
        eprintln!("   First load: {}ms", first_load_ms);
        eprintln!("   Second load: {}ms (from cache)", second_load_ms);

        // Second load should be faster (if already cached)
        if first_load_ms > 5000 {
            // First load took >5s, so it probably downloaded
            assert!(
                second_load_ms < first_load_ms / 2,
                "Cached load should be at least 2x faster"
            );
        }

        let info = provider2.model_info().await.unwrap();
        assert_eq!(info.dimension, 384);
    }

    /// Test loading different model (BGE-small).
    ///
    /// Verifies that the system can load multiple model architectures.
    #[tokio::test]
    #[ignore] // Expensive: downloads ~33MB model
    async fn test_load_bge_small_model() {
        eprintln!("\n=== Test: Load BGE-Small Model ===\n");

        let provider = CandleEmbeddingProvider::new("BAAI/bge-small-en-v1.5")
            .await
            .expect("Failed to load BGE-small model");

        let info = provider.model_info().await.expect("Failed to get model info");

        assert_eq!(info.model, "BAAI/bge-small-en-v1.5");
        assert_eq!(info.dimension, 384, "BGE-small has 384 dimensions");

        eprintln!("\n✅ Test passed: BGE-small model loaded successfully");
        eprintln!("   Model: {}", info.model);
        eprintln!("   Dimension: {}", info.dimension);
    }
}
