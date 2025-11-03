"""
AkiDB Python SDK

Official Python client library for AkiDB vector database

Example:
    >>> from akidb import AkiDBClient
    >>>
    >>> client = AkiDBClient(
    ...     endpoint='http://localhost:8080',
    ...     api_key='ak_your_api_key',
    ...     tenant='tenant_acme'
    ... )
    >>>
    >>> # Create collection
    >>> client.collections.create(
    ...     name='documents',
    ...     dimension=768,
    ...     metric='cosine'
    ... )
    >>>
    >>> # Insert vectors
    >>> client.vectors.insert('documents', [
    ...     {'id': 'doc1', 'vector': [0.1, 0.2, ...], 'metadata': {'title': 'Hello'}},
    ... ])
    >>>
    >>> # Search
    >>> results = client.vectors.search('documents', query=[0.1, 0.2, ...], k=10)
"""

from typing import Dict, List, Optional, Any
import requests
import time
from dataclasses import dataclass


@dataclass
class AkiDBConfig:
    """AkiDB client configuration"""

    endpoint: str
    api_key: str
    tenant: str
    timeout: int = 30
    max_retries: int = 3
    initial_delay: float = 0.1
    max_delay: float = 5.0


class AkiDBError(Exception):
    """AkiDB client error"""

    def __init__(self, message: str, status_code: Optional[int] = None):
        super().__init__(message)
        self.status_code = status_code


class AkiDBClient:
    """Main AkiDB client"""

    def __init__(
        self,
        endpoint: str,
        api_key: str,
        tenant: str,
        timeout: int = 30,
        max_retries: int = 3,
    ):
        """
        Initialize AkiDB client

        Args:
            endpoint: API endpoint (e.g., 'http://localhost:8080')
            api_key: API key for authentication
            tenant: Tenant ID
            timeout: Request timeout in seconds (default: 30)
            max_retries: Maximum number of retries (default: 3)
        """
        self.config = AkiDBConfig(
            endpoint=endpoint,
            api_key=api_key,
            tenant=tenant,
            timeout=timeout,
            max_retries=max_retries,
        )

        self.collections = CollectionsAPI(self)
        self.vectors = VectorsAPI(self)
        self.tenants = TenantsAPI(self)
        self.health = HealthAPI(self)

    def _request(
        self,
        method: str,
        path: str,
        json: Optional[Dict[str, Any]] = None,
        params: Optional[Dict[str, Any]] = None,
    ) -> Any:
        """Make HTTP request with retry logic"""
        url = f"{self.config.endpoint}{path}"
        headers = {
            "Content-Type": "application/json",
            "X-API-Key": self.config.api_key,
            "X-Tenant-ID": self.config.tenant,
        }

        last_error = None
        delay = self.config.initial_delay

        for attempt in range(self.config.max_retries + 1):
            try:
                response = requests.request(
                    method=method,
                    url=url,
                    headers=headers,
                    json=json,
                    params=params,
                    timeout=self.config.timeout,
                )

                if not response.ok:
                    raise AkiDBError(
                        f"HTTP {response.status_code}: {response.text}",
                        response.status_code,
                    )

                if response.status_code == 204:
                    return None

                return response.json()

            except Exception as e:
                last_error = e

                if attempt < self.config.max_retries:
                    time.sleep(delay)
                    delay = min(delay * 2, self.config.max_delay)

        raise last_error  # type: ignore


class CollectionsAPI:
    """Collections API"""

    def __init__(self, client: AkiDBClient):
        self.client = client

    def create(
        self,
        name: str,
        dimension: int,
        metric: str = "cosine",
        description: Optional[str] = None,
        metadata: Optional[Dict[str, str]] = None,
    ) -> Dict[str, Any]:
        """
        Create a collection

        Args:
            name: Collection name
            dimension: Vector dimension
            metric: Distance metric ('cosine', 'euclidean', 'dot_product')
            description: Optional description
            metadata: Optional metadata

        Returns:
            Collection response
        """
        return self.client._request(
            "POST",
            "/collections",
            json={
                "name": name,
                "dimension": dimension,
                "metric": metric,
                "description": description,
                "metadata": metadata,
            },
        )

    def get(self, name: str) -> Dict[str, Any]:
        """Get collection details"""
        return self.client._request("GET", f"/collections/{name}")

    def list(self) -> List[Dict[str, Any]]:
        """List all collections"""
        response = self.client._request("GET", "/collections")
        return response.get("collections", [])

    def delete(self, name: str) -> None:
        """Delete a collection"""
        self.client._request("DELETE", f"/collections/{name}")


class VectorsAPI:
    """Vectors API"""

    def __init__(self, client: AkiDBClient):
        self.client = client

    def insert(
        self, collection: str, vectors: List[Dict[str, Any]]
    ) -> Dict[str, Any]:
        """
        Insert vectors into a collection

        Args:
            collection: Collection name
            vectors: List of vectors with 'id', 'vector', and optional 'metadata'

        Returns:
            Insert response with counts
        """
        return self.client._request(
            "POST", f"/collections/{collection}/vectors", json={"vectors": vectors}
        )

    def search(
        self,
        collection: str,
        query: List[float],
        k: int = 10,
        filters: Optional[Dict[str, str]] = None,
    ) -> Dict[str, Any]:
        """
        Search for similar vectors

        Args:
            collection: Collection name
            query: Query vector
            k: Number of results
            filters: Optional metadata filters

        Returns:
            Search response with results
        """
        return self.client._request(
            "POST",
            f"/collections/{collection}/search",
            json={"query": query, "k": k, "filters": filters},
        )

    def batch_search(
        self, collection: str, queries: List[Dict[str, Any]]
    ) -> List[Dict[str, Any]]:
        """
        Batch search multiple queries

        Args:
            collection: Collection name
            queries: List of search requests

        Returns:
            List of search responses
        """
        response = self.client._request(
            "POST",
            f"/collections/{collection}/batch-search",
            json={"queries": queries},
        )
        return response.get("results", [])


class TenantsAPI:
    """Tenants API"""

    def __init__(self, client: AkiDBClient):
        self.client = client

    def create(
        self,
        name: str,
        quotas: Optional[Dict[str, Any]] = None,
        metadata: Optional[Dict[str, str]] = None,
    ) -> Dict[str, Any]:
        """Create a new tenant"""
        return self.client._request(
            "POST", "/tenants", json={"name": name, "quotas": quotas, "metadata": metadata}
        )

    def get(self, tenant_id: str) -> Dict[str, Any]:
        """Get tenant details"""
        return self.client._request("GET", f"/tenants/{tenant_id}")

    def list(
        self, offset: int = 0, limit: int = 20, status: Optional[str] = None
    ) -> Dict[str, Any]:
        """List tenants"""
        return self.client._request(
            "GET",
            "/tenants",
            params={"offset": offset, "limit": limit, "status": status},
        )

    def update(
        self,
        tenant_id: str,
        name: Optional[str] = None,
        quotas: Optional[Dict[str, Any]] = None,
        metadata: Optional[Dict[str, str]] = None,
    ) -> Dict[str, Any]:
        """Update tenant"""
        return self.client._request(
            "PUT",
            f"/tenants/{tenant_id}",
            json={"name": name, "quotas": quotas, "metadata": metadata},
        )

    def delete(self, tenant_id: str) -> None:
        """Delete tenant"""
        self.client._request("DELETE", f"/tenants/{tenant_id}")


class HealthAPI:
    """Health API"""

    def __init__(self, client: AkiDBClient):
        self.client = client

    def status(self) -> Dict[str, Any]:
        """Get health status"""
        return self.client._request("GET", "/health")

    def detailed(self) -> Dict[str, Any]:
        """Get detailed health information"""
        return self.client._request("GET", "/health/details")

    def metrics(self) -> str:
        """Get Prometheus metrics"""
        return self.client._request("GET", "/metrics")


__version__ = "0.1.0"
__all__ = ["AkiDBClient", "AkiDBError", "AkiDBConfig"]
