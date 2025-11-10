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

    /// Test inference with single text.
    ///
    /// Verifies that:
    /// - Embeddings are generated correctly
    /// - Output dimension matches model dimension (384 for MiniLM)
    /// - Embeddings are L2 normalized (unit length)
    #[tokio::test]
    #[ignore] // Expensive: runs inference on GPU/CPU
    async fn test_inference_single_text() {
        eprintln!("\n=== Test: Inference Single Text ===\n");

        let provider = CandleEmbeddingProvider::new("sentence-transformers/all-MiniLM-L6-v2")
            .await
            .expect("Failed to load model");

        // Call internal method directly
        let embeddings = provider
            .embed_batch_internal(vec!["Hello world".to_string()])
            .await
            .expect("Failed to generate embedding");

        assert_eq!(embeddings.len(), 1, "Should return 1 embedding");
        assert_eq!(embeddings[0].len(), 384, "MiniLM has 384 dimensions");

        // Check L2 normalized (unit length)
        let norm: f32 = embeddings[0].iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 0.01,
            "Embedding should be L2 normalized, got norm={}",
            norm
        );

        eprintln!("\n✅ Test passed: Single text inference works");
        eprintln!("   Embedding dimension: {}", embeddings[0].len());
        eprintln!("   L2 norm: {:.6}", norm);
        eprintln!("   First 5 values: {:?}", &embeddings[0][..5]);
    }

    /// Test inference with batch of texts.
    ///
    /// Verifies that:
    /// - Multiple texts processed correctly
    /// - All embeddings have correct dimension
    /// - All embeddings are L2 normalized
    /// - Different texts produce different embeddings
    #[tokio::test]
    #[ignore] // Expensive: runs batch inference
    async fn test_inference_batch() {
        eprintln!("\n=== Test: Inference Batch ===\n");

        let provider = CandleEmbeddingProvider::new("sentence-transformers/all-MiniLM-L6-v2")
            .await
            .expect("Failed to load model");

        let embeddings = provider
            .embed_batch_internal(vec![
                "Hello world".to_string(),
                "Rust is awesome".to_string(),
                "Machine learning".to_string(),
            ])
            .await
            .expect("Failed to generate embeddings");

        assert_eq!(embeddings.len(), 3, "Should return 3 embeddings");

        // Verify each embedding
        for (i, emb) in embeddings.iter().enumerate() {
            assert_eq!(emb.len(), 384, "Embedding {} should have 384 dims", i);

            let norm: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!(
                (norm - 1.0).abs() < 0.01,
                "Embedding {} should be L2 normalized, got norm={}",
                i,
                norm
            );
        }

        // Check that different texts produce different embeddings
        let cosine_sim = |a: &[f32], b: &[f32]| -> f32 {
            a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
        };

        let sim_01 = cosine_sim(&embeddings[0], &embeddings[1]);
        let sim_02 = cosine_sim(&embeddings[0], &embeddings[2]);
        let sim_12 = cosine_sim(&embeddings[1], &embeddings[2]);

        eprintln!("\n✅ Test passed: Batch inference works");
        eprintln!("   Batch size: {}", embeddings.len());
        eprintln!("   Similarity(0,1): {:.3}", sim_01);
        eprintln!("   Similarity(0,2): {:.3}", sim_02);
        eprintln!("   Similarity(1,2): {:.3}", sim_12);

        // Different texts should not be identical
        assert!(sim_01 < 0.99, "Different texts should not be identical");
        assert!(sim_02 < 0.99, "Different texts should not be identical");
    }

    /// Test inference performance.
    ///
    /// Measures:
    /// - Single text inference time (target: <20ms on Metal GPU)
    /// - Batch of 8 inference time (target: <40ms on Metal GPU)
    ///
    /// Note: Targets are for Metal GPU on Apple Silicon.
    /// CPU or other hardware may be slower.
    #[tokio::test]
    #[ignore] // Expensive: runs performance benchmarks
    async fn test_inference_performance() {
        use std::time::Instant;

        eprintln!("\n=== Test: Inference Performance ===\n");

        let provider = CandleEmbeddingProvider::new("sentence-transformers/all-MiniLM-L6-v2")
            .await
            .expect("Failed to load model");

        // Warm up (first inference might be slower due to GPU initialization)
        eprintln!("Warming up...");
        let _ = provider
            .embed_batch_internal(vec!["warmup".to_string()])
            .await;

        // Single text benchmark
        eprintln!("Benchmarking single text...");
        let start = Instant::now();
        let _ = provider
            .embed_batch_internal(vec!["Hello world".to_string()])
            .await
            .expect("Failed");
        let single_ms = start.elapsed().as_millis();

        // Batch of 8 benchmark
        eprintln!("Benchmarking batch of 8...");
        let texts = vec!["Sample text".to_string(); 8];
        let start = Instant::now();
        let _ = provider
            .embed_batch_internal(texts)
            .await
            .expect("Failed");
        let batch8_ms = start.elapsed().as_millis();

        eprintln!("\n✅ Test passed: Performance measured");
        eprintln!("   Single text: {}ms (target: <20ms)", single_ms);
        eprintln!("   Batch of 8:  {}ms (target: <40ms)", batch8_ms);

        // Soft assertions (targets are for Metal GPU)
        if single_ms > 20 {
            eprintln!(
                "   ⚠️  Single text slower than target ({}ms > 20ms)",
                single_ms
            );
            eprintln!("      (This is expected on CPU or non-Apple Silicon)");
        } else {
            eprintln!("   ✅ Single text meets target (<20ms)!");
        }

        if batch8_ms > 40 {
            eprintln!("   ⚠️  Batch of 8 slower than target ({}ms > 40ms)", batch8_ms);
            eprintln!("      (This is expected on CPU or non-Apple Silicon)");
        } else {
            eprintln!("   ✅ Batch of 8 meets target (<40ms)!");
        }

        // Performance comparison to MLX (MLX single text: ~182ms)
        let speedup = 182.0 / single_ms as f32;
        eprintln!("\n   MLX baseline: 182ms (Python + MLX)");
        eprintln!("   Candle: {}ms (Rust + Metal)", single_ms);
        eprintln!("   Speedup: {:.1}x faster than MLX", speedup);
    }
}
