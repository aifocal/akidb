/**
 * AkiDB TypeScript SDK
 *
 * Official client library for AkiDB vector database
 *
 * @example
 * ```typescript
 * import { AkiDBClient } from '@akidb/client';
 *
 * const client = new AkiDBClient({
 *   endpoint: 'http://localhost:8080',
 *   apiKey: 'ak_your_api_key',
 *   tenant: 'tenant_acme',
 * });
 *
 * // Create collection
 * await client.collections.create({
 *   name: 'documents',
 *   dimension: 768,
 *   metric: 'cosine',
 * });
 *
 * // Insert vectors
 * await client.vectors.insert('documents', [
 *   { id: 'doc1', vector: [0.1, 0.2, ...], metadata: { title: 'Hello' } },
 * ]);
 *
 * // Search
 * const results = await client.vectors.search('documents', {
 *   query: [0.1, 0.2, ...],
 *   k: 10,
 * });
 * ```
 */

export interface AkiDBConfig {
  /** API endpoint (e.g., 'http://localhost:8080') */
  endpoint: string;
  /** API key for authentication */
  apiKey: string;
  /** Tenant ID */
  tenant: string;
  /** Request timeout in milliseconds (default: 30000) */
  timeout?: number;
  /** Retry configuration */
  retry?: RetryConfig;
}

export interface RetryConfig {
  /** Maximum number of retries (default: 3) */
  maxRetries: number;
  /** Initial retry delay in ms (default: 100) */
  initialDelay: number;
  /** Maximum retry delay in ms (default: 5000) */
  maxDelay: number;
}

/**
 * Main AkiDB client
 */
export class AkiDBClient {
  private config: AkiDBConfig;
  public collections: CollectionsAPI;
  public vectors: VectorsAPI;
  public tenants: TenantsAPI;
  public health: HealthAPI;

  constructor(config: AkiDBConfig) {
    this.config = {
      ...config,
      timeout: config.timeout || 30000,
      retry: config.retry || {
        maxRetries: 3,
        initialDelay: 100,
        maxDelay: 5000,
      },
    };

    this.collections = new CollectionsAPI(this);
    this.vectors = new VectorsAPI(this);
    this.tenants = new TenantsAPI(this);
    this.health = new HealthAPI(this);
  }

  /**
   * Make HTTP request with retry logic
   */
  async request<T>(
    method: string,
    path: string,
    body?: unknown
  ): Promise<T> {
    const url = `${this.config.endpoint}${path}`;
    const headers = {
      'Content-Type': 'application/json',
      'X-API-Key': this.config.apiKey,
      'X-Tenant-ID': this.config.tenant,
    };

    let lastError: Error;
    let delay = this.config.retry!.initialDelay;

    for (let attempt = 0; attempt <= this.config.retry!.maxRetries; attempt++) {
      try {
        const controller = new AbortController();
        const timeout = setTimeout(
          () => controller.abort(),
          this.config.timeout
        );

        const response = await fetch(url, {
          method,
          headers,
          body: body ? JSON.stringify(body) : undefined,
          signal: controller.signal,
        });

        clearTimeout(timeout);

        if (!response.ok) {
          throw new AkiDBError(
            `HTTP ${response.status}: ${response.statusText}`,
            response.status
          );
        }

        return await response.json();
      } catch (error) {
        lastError = error as Error;

        if (attempt < this.config.retry!.maxRetries) {
          await new Promise((resolve) => setTimeout(resolve, delay));
          delay = Math.min(delay * 2, this.config.retry!.maxDelay);
        }
      }
    }

    throw lastError!;
  }
}

/**
 * Collections API
 */
export class CollectionsAPI {
  constructor(private client: AkiDBClient) {}

  /**
   * Create a collection
   */
  async create(params: CreateCollectionRequest): Promise<CollectionResponse> {
    return this.client.request('POST', '/collections', params);
  }

  /**
   * Get collection details
   */
  async get(name: string): Promise<CollectionResponse> {
    return this.client.request('GET', `/collections/${name}`);
  }

  /**
   * List collections
   */
  async list(): Promise<CollectionResponse[]> {
    const response = await this.client.request<{ collections: CollectionResponse[] }>(
      'GET',
      '/collections'
    );
    return response.collections;
  }

  /**
   * Delete a collection
   */
  async delete(name: string): Promise<void> {
    await this.client.request('DELETE', `/collections/${name}`);
  }
}

/**
 * Vectors API
 */
export class VectorsAPI {
  constructor(private client: AkiDBClient) {}

  /**
   * Insert vectors
   */
  async insert(
    collection: string,
    vectors: VectorInput[]
  ): Promise<InsertResponse> {
    return this.client.request('POST', `/collections/${collection}/vectors`, {
      vectors,
    });
  }

  /**
   * Search vectors
   */
  async search(
    collection: string,
    params: SearchRequest
  ): Promise<SearchResponse> {
    return this.client.request(
      'POST',
      `/collections/${collection}/search`,
      params
    );
  }

  /**
   * Batch search
   */
  async batchSearch(
    collection: string,
    queries: SearchRequest[]
  ): Promise<SearchResponse[]> {
    const response = await this.client.request<{ results: SearchResponse[] }>(
      'POST',
      `/collections/${collection}/batch-search`,
      { queries }
    );
    return response.results;
  }
}

/**
 * Tenants API
 */
export class TenantsAPI {
  constructor(private client: AkiDBClient) {}

  /**
   * Create tenant
   */
  async create(params: CreateTenantRequest): Promise<TenantResponse> {
    return this.client.request('POST', '/tenants', params);
  }

  /**
   * Get tenant details
   */
  async get(tenantId: string): Promise<TenantResponse> {
    return this.client.request('GET', `/tenants/${tenantId}`);
  }

  /**
   * List tenants
   */
  async list(params?: ListTenantsParams): Promise<ListTenantsResponse> {
    const query = new URLSearchParams(params as Record<string, string>);
    return this.client.request('GET', `/tenants?${query}`);
  }

  /**
   * Update tenant
   */
  async update(
    tenantId: string,
    params: UpdateTenantRequest
  ): Promise<TenantResponse> {
    return this.client.request('PUT', `/tenants/${tenantId}`, params);
  }

  /**
   * Delete tenant
   */
  async delete(tenantId: string): Promise<void> {
    await this.client.request('DELETE', `/tenants/${tenantId}`);
  }
}

/**
 * Health API
 */
export class HealthAPI {
  constructor(private client: AkiDBClient) {}

  /**
   * Get health status
   */
  async status(): Promise<HealthResponse> {
    return this.client.request('GET', '/health');
  }

  /**
   * Get detailed health
   */
  async detailed(): Promise<DetailedHealthResponse> {
    return this.client.request('GET', '/health/details');
  }

  /**
   * Get metrics
   */
  async metrics(): Promise<string> {
    return this.client.request('GET', '/metrics');
  }
}

// Types

export interface CreateCollectionRequest {
  name: string;
  dimension: number;
  metric: 'cosine' | 'euclidean' | 'dot_product';
  description?: string;
  metadata?: Record<string, string>;
}

export interface CollectionResponse {
  name: string;
  dimension: number;
  metric: string;
  vector_count: number;
  created_at: string;
  metadata?: Record<string, string>;
}

export interface VectorInput {
  id: string;
  vector: number[];
  metadata?: Record<string, unknown>;
}

export interface InsertResponse {
  inserted: number;
  failed: number;
}

export interface SearchRequest {
  query: number[];
  k: number;
  filters?: Record<string, string>;
}

export interface SearchResponse {
  results: SearchResult[];
  took_ms: number;
}

export interface SearchResult {
  id: string;
  distance: number;
  metadata?: Record<string, unknown>;
}

export interface CreateTenantRequest {
  name: string;
  quotas?: TenantQuotas;
  metadata?: Record<string, string>;
}

export interface TenantQuotas {
  max_storage_bytes?: number;
  max_collections?: number;
  max_vectors_per_collection?: number;
  api_rate_limit_per_second?: number;
}

export interface TenantResponse {
  tenant_id: string;
  name: string;
  status: string;
  quotas: TenantQuotas;
  usage: TenantUsage;
  created_at: string;
  api_key?: string;
}

export interface TenantUsage {
  storage_bytes: number;
  collection_count: number;
  total_vectors: number;
}

export interface UpdateTenantRequest {
  name?: string;
  quotas?: TenantQuotas;
  metadata?: Record<string, string>;
}

export interface ListTenantsParams {
  offset?: number;
  limit?: number;
  status?: string;
}

export interface ListTenantsResponse {
  tenants: TenantResponse[];
  total: number;
  offset: number;
  limit: number;
}

export interface HealthResponse {
  status: 'healthy' | 'unhealthy';
  version: string;
  uptime_seconds: number;
}

export interface DetailedHealthResponse extends HealthResponse {
  components: Record<string, ComponentHealth>;
}

export interface ComponentHealth {
  status: 'healthy' | 'degraded' | 'unhealthy';
  message?: string;
}

/**
 * AkiDB Error
 */
export class AkiDBError extends Error {
  constructor(
    message: string,
    public statusCode?: number
  ) {
    super(message);
    this.name = 'AkiDBError';
  }
}

export default AkiDBClient;
