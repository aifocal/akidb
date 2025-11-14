//! ONNX embedding provider tests.
//!
//! These tests verify the ONNX provider configuration and API surface.

#[cfg(feature = "onnx")]
mod onnx_tests {
    use akidb_embedding::{ExecutionProviderConfig, OnnxConfig};
    use std::path::PathBuf;

    #[test]
    fn test_onnx_config_default() {
        let config = OnnxConfig::default();
        assert_eq!(config.dimension, 384);
        assert_eq!(config.max_length, 512);
        assert_eq!(config.model_name, "sentence-transformers/all-MiniLM-L6-v2");
    }

    #[test]
    fn test_onnx_config_tensorrt() {
        let config = OnnxConfig {
            model_path: PathBuf::from("models/qwen3-4b-fp8.onnx"),
            tokenizer_path: PathBuf::from("models/tokenizer.json"),
            model_name: "Qwen/Qwen2.5-4B".to_string(),
            dimension: 4096,
            max_length: 512,
            execution_provider: ExecutionProviderConfig::TensorRT {
                device_id: 0,
                fp8_enable: true,
                engine_cache_path: Some(PathBuf::from("/tmp/trt_cache")),
            },
        };

        assert_eq!(config.dimension, 4096);
        assert_eq!(config.model_name, "Qwen/Qwen2.5-4B");

        match config.execution_provider {
            ExecutionProviderConfig::TensorRT {
                device_id,
                fp8_enable,
                engine_cache_path,
            } => {
                assert_eq!(device_id, 0);
                assert!(fp8_enable);
                assert!(engine_cache_path.is_some());
            }
            _ => panic!("Expected TensorRT execution provider"),
        }
    }

    #[test]
    fn test_onnx_config_coreml() {
        let config = OnnxConfig {
            execution_provider: ExecutionProviderConfig::CoreML,
            ..Default::default()
        };

        matches!(config.execution_provider, ExecutionProviderConfig::CoreML);
    }

    #[test]
    fn test_onnx_config_cuda() {
        let config = OnnxConfig {
            execution_provider: ExecutionProviderConfig::CUDA { device_id: 0 },
            ..Default::default()
        };

        match config.execution_provider {
            ExecutionProviderConfig::CUDA { device_id } => {
                assert_eq!(device_id, 0);
            }
            _ => panic!("Expected CUDA execution provider"),
        }
    }
}
