#!/usr/bin/env python3
"""
ONNX Runtime embedding server with CoreML Execution Provider.
Communicates via JSON over stdin/stdout for IPC with Rust.

Performance: ~10ms P95 with CoreML EP on Apple Silicon (validated Day 2)
Protocol: JSON-RPC style request/response
"""

import sys
import json
import numpy as np
from typing import List, Dict, Any
from pathlib import Path
import logging

# Configure logging to stderr (stdout is reserved for JSON responses)
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s [%(levelname)s] %(message)s',
    stream=sys.stderr
)
logger = logging.getLogger(__name__)

try:
    import onnxruntime as ort
    from transformers import AutoTokenizer
    from sentence_transformers import SentenceTransformer
except ImportError as e:
    logger.error(f"Missing dependency: {e}")
    logger.error("Install with: pip install onnxruntime transformers sentence-transformers")
    sys.exit(1)


class ONNXEmbeddingServer:
    """ONNX Runtime embedding server with CoreML acceleration."""

    def __init__(self):
        """Initialize with no models loaded (lazy loading)."""
        self.sessions: Dict[str, ort.InferenceSession] = {}
        self.tokenizers: Dict[str, Any] = {}
        self.dimensions: Dict[str, int] = {}
        self.pytorch_models: Dict[str, SentenceTransformer] = {}  # Fallback to PyTorch
        logger.info("ONNX embedding server initialized (with PyTorch fallback)")

    def load_model(self, model_name: str, cache_dir: str = "~/.cache/akidb/models") -> Dict[str, Any]:
        """
        Load ONNX model and tokenizer (if not already loaded).

        Args:
            model_name: HuggingFace model name (e.g., "sentence-transformers/all-MiniLM-L6-v2")
            cache_dir: Directory to cache models

        Returns:
            Response dict with status and dimension
        """
        if model_name in self.sessions:
            return {
                "status": "ok",
                "message": "Model already loaded",
                "dimension": self.dimensions[model_name]
            }

        try:
            cache_path = Path(cache_dir).expanduser()
            cache_path.mkdir(parents=True, exist_ok=True)

            # Load tokenizer
            logger.info(f"Loading tokenizer: {model_name}")
            tokenizer = AutoTokenizer.from_pretrained(
                model_name,
                cache_dir=str(cache_path)
            )
            self.tokenizers[model_name] = tokenizer

            # Construct ONNX model path
            model_safe_name = model_name.replace("/", "_")
            onnx_path = cache_path / f"{model_safe_name}.onnx"

            if not onnx_path.exists():
                # Fallback to PyTorch/SentenceTransformer when ONNX not available
                logger.warning(f"ONNX model not found: {onnx_path}")
                logger.info(f"Falling back to PyTorch SentenceTransformer: {model_name}")

                model = SentenceTransformer(model_name, cache_folder=str(cache_path))
                self.pytorch_models[model_name] = model
                dimension = model.get_sentence_embedding_dimension()
                self.dimensions[model_name] = dimension

                logger.info(f"PyTorch model loaded: {model_name}")
                logger.info(f"Dimension: {dimension}")
                logger.info(f"Provider: PyTorch (CPU/MPS)")

                return {
                    "status": "ok",
                    "message": "Model loaded with PyTorch fallback (CPU/MPS)",
                    "dimension": dimension,
                    "providers": ["PyTorchExecutionProvider"]
                }

            # Create ONNX Runtime session with CoreML EP
            logger.info(f"Loading ONNX model: {onnx_path}")

            # Configure CoreML Execution Provider (Apple Silicon acceleration)
            providers = [
                ('CoreMLExecutionProvider', {
                    'MLComputeUnits': 'ALL',  # Use CPU + GPU + ANE
                }),
                'CPUExecutionProvider'  # Fallback
            ]

            session_opts = ort.SessionOptions()
            session_opts.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
            session_opts.intra_op_num_threads = 1  # CoreML handles parallelism

            session = ort.InferenceSession(
                str(onnx_path),
                sess_options=session_opts,
                providers=providers
            )

            # Get embedding dimension from model output
            output_shape = session.get_outputs()[0].shape
            dimension = output_shape[-1]  # Last dimension is embedding size

            self.sessions[model_name] = session
            self.dimensions[model_name] = dimension

            # Check which provider is actually being used
            active_providers = session.get_providers()
            logger.info(f"Model loaded: {model_name}")
            logger.info(f"Dimension: {dimension}")
            logger.info(f"Active providers: {active_providers}")

            return {
                "status": "ok",
                "message": "Model loaded successfully",
                "dimension": dimension,
                "providers": active_providers
            }

        except Exception as e:
            logger.error(f"Failed to load model {model_name}: {e}")
            return {
                "status": "error",
                "message": str(e)
            }

    def embed_batch(self, model_name: str, inputs: List[str], normalize: bool = True) -> Dict[str, Any]:
        """
        Generate embeddings for a batch of texts.

        Args:
            model_name: Model to use
            inputs: List of texts to embed
            normalize: Whether to L2-normalize embeddings

        Returns:
            Response dict with embeddings or error
        """
        # Check if model is loaded (either ONNX or PyTorch)
        if model_name not in self.sessions and model_name not in self.pytorch_models:
            return {
                "status": "error",
                "message": f"Model not loaded: {model_name}. Call load_model first."
            }

        try:
            # Use PyTorch model if available (fallback path)
            if model_name in self.pytorch_models:
                model = self.pytorch_models[model_name]
                embeddings = model.encode(inputs, normalize_embeddings=normalize)
                return {
                    "status": "ok",
                    "embeddings": embeddings.tolist(),
                    "count": len(inputs)
                }

            # Otherwise use ONNX Runtime (optimized path)
            session = self.sessions[model_name]
            tokenizer = self.tokenizers[model_name]

            # Tokenize inputs
            encoded = tokenizer(
                inputs,
                padding=True,
                truncation=True,
                max_length=512,
                return_tensors="np"
            )

            # Prepare inputs for ONNX
            ort_inputs = {
                "input_ids": encoded["input_ids"].astype(np.int64),
                "attention_mask": encoded["attention_mask"].astype(np.int64),
            }

            # Add token_type_ids if model expects it
            input_names = [inp.name for inp in session.get_inputs()]
            if "token_type_ids" in input_names:
                ort_inputs["token_type_ids"] = encoded.get(
                    "token_type_ids",
                    np.zeros_like(encoded["input_ids"])
                ).astype(np.int64)

            # Run inference
            outputs = session.run(None, ort_inputs)
            hidden_states = outputs[0]  # (batch_size, seq_len, hidden_size)

            # Mean pooling with attention mask
            attention_mask = encoded["attention_mask"]
            attention_mask_expanded = np.expand_dims(attention_mask, axis=-1)

            sum_embeddings = np.sum(hidden_states * attention_mask_expanded, axis=1)
            sum_mask = np.clip(attention_mask.sum(axis=1, keepdims=True), a_min=1e-9, a_max=None)
            embeddings = sum_embeddings / sum_mask

            # L2 normalization
            if normalize:
                norms = np.linalg.norm(embeddings, axis=1, keepdims=True)
                norms = np.clip(norms, a_min=1e-12, a_max=None)
                embeddings = embeddings / norms

            return {
                "status": "ok",
                "embeddings": embeddings.tolist(),
                "count": len(inputs)
            }

        except Exception as e:
            logger.error(f"Embedding failed: {e}")
            return {
                "status": "error",
                "message": str(e)
            }

    def process_request(self, request: Dict[str, Any]) -> Dict[str, Any]:
        """
        Process a single JSON-RPC style request.

        Request format:
        {
            "method": "load_model" | "embed_batch" | "ping",
            "params": {...}
        }
        """
        method = request.get("method")

        if method == "ping":
            return {"status": "ok", "message": "pong"}

        elif method == "load_model":
            params = request.get("params", {})
            model_name = params.get("model")
            cache_dir = params.get("cache_dir", "~/.cache/akidb/models")
            return self.load_model(model_name, cache_dir)

        elif method == "embed_batch":
            params = request.get("params", {})
            model_name = params.get("model")
            inputs = params.get("inputs", [])
            normalize = params.get("normalize", True)
            return self.embed_batch(model_name, inputs, normalize)

        else:
            return {
                "status": "error",
                "message": f"Unknown method: {method}"
            }

    def run(self):
        """Main server loop: read JSON from stdin, write JSON to stdout."""
        logger.info("Server ready. Waiting for requests on stdin...")

        for line in sys.stdin:
            line = line.strip()
            if not line:
                continue

            try:
                request = json.loads(line)
                response = self.process_request(request)

                # Write response as single line JSON to stdout
                print(json.dumps(response), flush=True)

            except json.JSONDecodeError as e:
                error_response = {
                    "status": "error",
                    "message": f"Invalid JSON: {e}"
                }
                print(json.dumps(error_response), flush=True)

            except Exception as e:
                logger.error(f"Unexpected error: {e}", exc_info=True)
                error_response = {
                    "status": "error",
                    "message": f"Internal error: {e}"
                }
                print(json.dumps(error_response), flush=True)


def main():
    """Entry point."""
    server = ONNXEmbeddingServer()

    try:
        server.run()
    except KeyboardInterrupt:
        logger.info("Server shutting down (KeyboardInterrupt)")
    except Exception as e:
        logger.error(f"Fatal error: {e}", exc_info=True)
        sys.exit(1)


if __name__ == "__main__":
    main()
