#!/usr/bin/env python3
"""
AkiDB 2.0 Data Import Script

This script imports vector data into an AkiDB collection from JSON or CSV files.

Usage Examples:
    # Import from JSON file
    python import-data.py --collection-id abc123 --file vectors.json

    # Import from CSV file (comma-separated floats)
    python import-data.py --collection-id abc123 --file vectors.csv --dimension 128

    # Custom batch size and host
    python import-data.py --collection-id abc123 --file data.json --batch-size 50 --host localhost:8080

JSON Format:
    [
        {
            "vector": [0.1, 0.2, 0.3, ...],
            "external_id": "doc1",  // optional
            "metadata": {"key": "value"}  // optional
        },
        ...
    ]

CSV Format:
    Each line contains comma-separated float values representing one vector.
    Example:
        0.1,0.2,0.3,0.4
        0.5,0.6,0.7,0.8
"""

import argparse
import csv
import json
import sys
import time
from pathlib import Path
from typing import Dict, List, Any, Optional

try:
    import requests
except ImportError:
    print("Error: requests library not found. Install with: pip install requests", file=sys.stderr)
    sys.exit(1)

try:
    from tqdm import tqdm
except ImportError:
    # Fallback if tqdm not available
    def tqdm(iterable, **kwargs):
        return iterable


class AkiDBImporter:
    """Handles batch import of vectors into AkiDB collections."""

    def __init__(self, host: str, collection_id: str, batch_size: int = 100):
        self.host = host.rstrip('/')
        self.collection_id = collection_id
        self.batch_size = batch_size
        self.base_url = f"http://{self.host}/api/v1/collections/{self.collection_id}"

    def validate_dimension(self, vectors: List[List[float]], expected_dim: Optional[int] = None) -> int:
        """Validate that all vectors have the same dimension."""
        if not vectors:
            raise ValueError("No vectors to import")

        dimensions = set(len(v) for v in vectors)
        if len(dimensions) > 1:
            raise ValueError(f"Inconsistent vector dimensions: {dimensions}")

        actual_dim = dimensions.pop()
        if expected_dim is not None and actual_dim != expected_dim:
            raise ValueError(f"Dimension mismatch: expected {expected_dim}, got {actual_dim}")

        if not (16 <= actual_dim <= 4096):
            raise ValueError(f"Dimension {actual_dim} outside valid range [16, 4096]")

        return actual_dim

    def insert_batch(self, documents: List[Dict[str, Any]], attempt: int = 1) -> bool:
        """Insert a batch of documents with retry logic."""
        max_attempts = 3
        url = f"{self.base_url}/insert"

        try:
            response = requests.post(url, json={"documents": documents}, timeout=30)
            response.raise_for_status()
            return True
        except requests.exceptions.RequestException as e:
            if attempt < max_attempts:
                time.sleep(0.5 * attempt)  # Exponential backoff
                return self.insert_batch(documents, attempt + 1)
            else:
                print(f"\nError inserting batch after {max_attempts} attempts: {e}", file=sys.stderr)
                return False

    def load_json(self, file_path: Path) -> List[Dict[str, Any]]:
        """Load vectors from JSON file."""
        with open(file_path, 'r') as f:
            data = json.load(f)

        if not isinstance(data, list):
            raise ValueError("JSON file must contain an array of objects")

        documents = []
        for idx, item in enumerate(data):
            if not isinstance(item, dict) or 'vector' not in item:
                raise ValueError(f"Invalid document at index {idx}: missing 'vector' field")

            doc = {"vector": item["vector"]}
            if "external_id" in item:
                doc["external_id"] = str(item["external_id"])
            if "metadata" in item:
                doc["metadata"] = item["metadata"]

            documents.append(doc)

        return documents

    def load_csv(self, file_path: Path, dimension: int) -> List[Dict[str, Any]]:
        """Load vectors from CSV file (comma-separated floats)."""
        documents = []

        with open(file_path, 'r') as f:
            reader = csv.reader(f)
            for line_num, row in enumerate(reader, 1):
                try:
                    vector = [float(x.strip()) for x in row if x.strip()]
                    if len(vector) != dimension:
                        raise ValueError(f"Line {line_num}: expected {dimension} values, got {len(vector)}")
                    documents.append({"vector": vector})
                except ValueError as e:
                    raise ValueError(f"Line {line_num}: {e}")

        return documents

    def import_data(self, file_path: Path, dimension: Optional[int] = None) -> Dict[str, Any]:
        """Import data from file and return statistics."""
        start_time = time.time()

        # Load documents
        file_ext = file_path.suffix.lower()
        if file_ext == '.json':
            documents = self.load_json(file_path)
        elif file_ext == '.csv':
            if dimension is None:
                raise ValueError("--dimension is required for CSV files")
            documents = self.load_csv(file_path, dimension)
        else:
            raise ValueError(f"Unsupported file format: {file_ext} (use .json or .csv)")

        # Validate dimensions
        vectors = [doc["vector"] for doc in documents]
        actual_dim = self.validate_dimension(vectors, dimension)

        print(f"Loaded {len(documents)} vectors (dimension: {actual_dim})")
        print(f"Batch size: {self.batch_size}")
        print(f"Target: {self.base_url}\n")

        # Insert in batches
        total = len(documents)
        succeeded = 0
        failed = 0

        for i in tqdm(range(0, total, self.batch_size), desc="Importing", unit="batch"):
            batch = documents[i:i + self.batch_size]
            if self.insert_batch(batch):
                succeeded += len(batch)
            else:
                failed += len(batch)

        elapsed = time.time() - start_time
        throughput = total / elapsed if elapsed > 0 else 0

        return {
            "total": total,
            "succeeded": succeeded,
            "failed": failed,
            "dimension": actual_dim,
            "elapsed_seconds": elapsed,
            "throughput_per_sec": throughput
        }


def main():
    parser = argparse.ArgumentParser(
        description="Import vector data into AkiDB collection",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__
    )

    parser.add_argument(
        '--collection-id',
        required=True,
        help='Target collection ID (UUID)'
    )
    parser.add_argument(
        '--file',
        required=True,
        type=Path,
        help='Input file path (JSON or CSV)'
    )
    parser.add_argument(
        '--dimension',
        type=int,
        help='Vector dimension (required for CSV, auto-detected for JSON)'
    )
    parser.add_argument(
        '--batch-size',
        type=int,
        default=100,
        help='Batch size for insertion (default: 100)'
    )
    parser.add_argument(
        '--host',
        default='localhost:8080',
        help='AkiDB host:port (default: localhost:8080)'
    )

    args = parser.parse_args()

    if not args.file.exists():
        print(f"Error: File not found: {args.file}", file=sys.stderr)
        sys.exit(1)

    try:
        importer = AkiDBImporter(args.host, args.collection_id, args.batch_size)
        stats = importer.import_data(args.file, args.dimension)

        print("\n" + "=" * 50)
        print("Import Summary")
        print("=" * 50)
        print(f"Total vectors:     {stats['total']}")
        print(f"Succeeded:         {stats['succeeded']}")
        print(f"Failed:            {stats['failed']}")
        print(f"Dimension:         {stats['dimension']}")
        print(f"Elapsed time:      {stats['elapsed_seconds']:.2f}s")
        print(f"Throughput:        {stats['throughput_per_sec']:.1f} vectors/sec")
        print("=" * 50)

        if stats['failed'] > 0:
            sys.exit(1)

    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)


if __name__ == '__main__':
    main()
