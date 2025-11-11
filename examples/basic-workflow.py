#!/usr/bin/env python3
"""
AkiDB 2.0 Basic Workflow Example

This script demonstrates the complete lifecycle of working with AkiDB's REST API:
1. Create a vector collection with specified dimensions and distance metric
2. Insert multiple vectors with associated metadata
3. Perform similarity search to find nearest neighbors
4. Retrieve specific vectors by document ID
5. Delete individual vectors
6. Clean up by deleting the collection

Prerequisites:
    - AkiDB server running on localhost:8080
    - Python 3.7+ with requests library installed (pip install requests)

Usage:
    python basic-workflow.py

The script includes comprehensive error handling and formatted output for each
operation, making it suitable as a reference implementation for integrating
AkiDB into your applications.
"""

import json
import sys
from typing import Dict, List, Any, Optional

try:
    import requests
except ImportError:
    print("Error: requests library not found. Install with: pip install requests")
    sys.exit(1)


BASE_URL = "http://localhost:8080/api/v1"


def print_section(title: str) -> None:
    """Print a formatted section header."""
    print(f"\n{'=' * 60}")
    print(f"  {title}")
    print(f"{'=' * 60}\n")


def handle_response(response: requests.Response, operation: str) -> Optional[Dict[str, Any]]:
    """Handle API response with error checking."""
    try:
        response.raise_for_status()
        data = response.json()
        print(f"✓ {operation} succeeded")
        return data
    except requests.exceptions.HTTPError as e:
        print(f"✗ {operation} failed: HTTP {response.status_code}")
        try:
            error_detail = response.json()
            print(f"  Error: {json.dumps(error_detail, indent=2)}")
        except ValueError:
            print(f"  Error: {response.text}")
        return None
    except Exception as e:
        print(f"✗ {operation} failed: {str(e)}")
        return None


def create_collection(collection_name: str, dimension: int, metric: str = "cosine") -> Optional[str]:
    """Create a new vector collection."""
    print_section("Step 1: Create Collection")

    url = f"{BASE_URL}/collections"
    payload = {
        "name": collection_name,
        "dimension": dimension,
        "metric": metric
    }

    print(f"Creating collection: {collection_name}")
    print(f"  Dimension: {dimension}, Metric: {metric}")

    response = requests.post(url, json=payload)
    result = handle_response(response, "Create collection")

    if result:
        collection_id = result.get('collection_id')
        print(f"  Collection ID: {collection_id}")
        return collection_id
    return None


def insert_vectors(collection_id: str, vectors: List[Dict[str, Any]]) -> List[str]:
    """Insert multiple vectors into a collection."""
    print_section("Step 2: Insert Vectors")

    url = f"{BASE_URL}/collections/{collection_id}/insert"
    doc_ids = []

    for i, vec_data in enumerate(vectors, 1):
        print(f"Inserting vector {i}/{len(vectors)}...")
        response = requests.post(url, json=vec_data)
        result = handle_response(response, f"Insert vector {i}")

        if result:
            doc_id = result.get('doc_id')
            doc_ids.append(doc_id)
            print(f"  Document ID: {doc_id}")
            if vec_data.get('metadata'):
                print(f"  Metadata: {vec_data['metadata']}")
        else:
            print(f"  Failed to insert vector {i}")

    print(f"\nTotal inserted: {len(doc_ids)}/{len(vectors)}")
    return doc_ids


def search_vectors(collection_id: str, query_vector: List[float], top_k: int = 3) -> None:
    """Search for similar vectors."""
    print_section("Step 3: Search for Similar Vectors")

    url = f"{BASE_URL}/collections/{collection_id}/query"
    payload = {
        "vector": query_vector,
        "k": top_k
    }

    print(f"Searching for top {top_k} similar vectors...")
    response = requests.post(url, json=payload)
    result = handle_response(response, "Search vectors")

    if result and 'results' in result:
        results = result['results']
        print(f"\nFound {len(results)} results:")
        for i, res in enumerate(results, 1):
            print(f"\n  Result {i}:")
            print(f"    Document ID: {res.get('doc_id')}")
            print(f"    Score: {res.get('score', 0):.4f}")
            if res.get('metadata'):
                print(f"    Metadata: {res['metadata']}")


def get_vector(collection_id: str, doc_id: str) -> None:
    """Retrieve a specific vector by document ID."""
    print_section("Step 4: Get Vector by ID")

    url = f"{BASE_URL}/collections/{collection_id}/docs/{doc_id}"

    print(f"Retrieving vector: {doc_id}")
    response = requests.get(url)
    result = handle_response(response, "Get vector")

    if result:
        print(f"  Document ID: {result.get('doc_id')}")
        print(f"  Vector dimension: {len(result.get('vector', []))}")
        if result.get('metadata'):
            print(f"  Metadata: {result['metadata']}")


def delete_vector(collection_id: str, doc_id: str) -> bool:
    """Delete a vector by document ID."""
    print_section("Step 5: Delete Vector")

    url = f"{BASE_URL}/collections/{collection_id}/docs/{doc_id}"

    print(f"Deleting vector: {doc_id}")
    response = requests.delete(url)
    result = handle_response(response, "Delete vector")

    return result is not None


def delete_collection(collection_id: str) -> bool:
    """Delete a collection."""
    print_section("Step 6: Delete Collection")

    url = f"{BASE_URL}/collections/{collection_id}"

    print(f"Deleting collection: {collection_id}")
    response = requests.delete(url)
    result = handle_response(response, "Delete collection")

    return result is not None


def main() -> None:
    """Execute the complete workflow demonstration."""
    print("AkiDB 2.0 Basic Workflow Example")
    print(f"Target: {BASE_URL}")

    collection_name = "demo_collection"
    dimension = 128

    # Step 1: Create collection
    collection_id = create_collection(collection_name, dimension, "cosine")
    if not collection_id:
        print("\n✗ Failed to create collection. Exiting.")
        return

    # Step 2: Insert vectors
    sample_vectors = [
        {
            "vector": [0.1] * dimension,
            "metadata": {"category": "technology", "tag": "ai"}
        },
        {
            "vector": [0.2] * dimension,
            "metadata": {"category": "science", "tag": "research"}
        },
        {
            "vector": [0.3] * dimension,
            "metadata": {"category": "technology", "tag": "cloud"}
        },
        {
            "vector": [0.4] * dimension,
            "metadata": {"category": "business", "tag": "finance"}
        },
        {
            "vector": [0.5] * dimension,
            "metadata": {"category": "science", "tag": "physics"}
        }
    ]

    doc_ids = insert_vectors(collection_id, sample_vectors)

    if not doc_ids:
        print("\n✗ No vectors inserted. Skipping remaining steps.")
        delete_collection(collection_id)
        return

    # Step 3: Search vectors
    query_vector = [0.15] * dimension
    search_vectors(collection_id, query_vector, top_k=3)

    # Step 4: Get vector by ID
    if doc_ids:
        get_vector(collection_id, doc_ids[0])

    # Step 5: Delete vector
    if len(doc_ids) >= 2:
        delete_vector(collection_id, doc_ids[1])

    # Step 6: Clean up - delete collection
    delete_collection(collection_id)

    print_section("Workflow Complete")
    print("All operations completed successfully!")


if __name__ == "__main__":
    main()
