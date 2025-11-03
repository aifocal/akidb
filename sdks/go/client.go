// Package akidb provides the official Go client for AkiDB vector database
//
// Example usage:
//
//	client := akidb.NewClient(&akidb.Config{
//		Endpoint: "http://localhost:8080",
//		APIKey:   "ak_your_api_key",
//		Tenant:   "tenant_acme",
//	})
//
//	// Create collection
//	err := client.Collections.Create(ctx, &akidb.CreateCollectionRequest{
//		Name:      "documents",
//		Dimension: 768,
//		Metric:    "cosine",
//	})
//
//	// Insert vectors
//	err = client.Vectors.Insert(ctx, "documents", []akidb.VectorInput{
//		{ID: "doc1", Vector: []float32{0.1, 0.2, ...}, Metadata: map[string]interface{}{"title": "Hello"}},
//	})
//
//	// Search
//	results, err := client.Vectors.Search(ctx, "documents", &akidb.SearchRequest{
//		Query: []float32{0.1, 0.2, ...},
//		K:     10,
//	})
package akidb

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"
)

// Config represents the client configuration
type Config struct {
	// Endpoint is the API endpoint (e.g., "http://localhost:8080")
	Endpoint string
	// APIKey is the API key for authentication
	APIKey string
	// Tenant is the tenant ID
	Tenant string
	// Timeout is the request timeout (default: 30s)
	Timeout time.Duration
	// MaxRetries is the maximum number of retries (default: 3)
	MaxRetries int
	// InitialDelay is the initial retry delay (default: 100ms)
	InitialDelay time.Duration
	// MaxDelay is the maximum retry delay (default: 5s)
	MaxDelay time.Duration
}

// Client is the main AkiDB client
type Client struct {
	config      *Config
	httpClient  *http.Client
	Collections *CollectionsAPI
	Vectors     *VectorsAPI
	Tenants     *TenantsAPI
	Health      *HealthAPI
}

// NewClient creates a new AkiDB client
func NewClient(config *Config) *Client {
	if config.Timeout == 0 {
		config.Timeout = 30 * time.Second
	}
	if config.MaxRetries == 0 {
		config.MaxRetries = 3
	}
	if config.InitialDelay == 0 {
		config.InitialDelay = 100 * time.Millisecond
	}
	if config.MaxDelay == 0 {
		config.MaxDelay = 5 * time.Second
	}

	client := &Client{
		config: config,
		httpClient: &http.Client{
			Timeout: config.Timeout,
		},
	}

	client.Collections = &CollectionsAPI{client: client}
	client.Vectors = &VectorsAPI{client: client}
	client.Tenants = &TenantsAPI{client: client}
	client.Health = &HealthAPI{client: client}

	return client
}

// request makes an HTTP request with retry logic
func (c *Client) request(ctx context.Context, method, path string, body interface{}, result interface{}) error {
	url := fmt.Sprintf("%s%s", c.config.Endpoint, path)

	var lastErr error
	delay := c.config.InitialDelay

	for attempt := 0; attempt <= c.config.MaxRetries; attempt++ {
		var reqBody io.Reader
		if body != nil {
			jsonData, err := json.Marshal(body)
			if err != nil {
				return fmt.Errorf("failed to marshal request: %w", err)
			}
			reqBody = bytes.NewReader(jsonData)
		}

		req, err := http.NewRequestWithContext(ctx, method, url, reqBody)
		if err != nil {
			return fmt.Errorf("failed to create request: %w", err)
		}

		req.Header.Set("Content-Type", "application/json")
		req.Header.Set("X-API-Key", c.config.APIKey)
		req.Header.Set("X-Tenant-ID", c.config.Tenant)

		resp, err := c.httpClient.Do(req)
		if err != nil {
			lastErr = err
			if attempt < c.config.MaxRetries {
				time.Sleep(delay)
				delay *= 2
				if delay > c.config.MaxDelay {
					delay = c.config.MaxDelay
				}
				continue
			}
			return fmt.Errorf("request failed after %d attempts: %w", attempt+1, err)
		}

		defer resp.Body.Close()

		if resp.StatusCode >= 400 {
			bodyBytes, _ := io.ReadAll(resp.Body)
			return &Error{
				StatusCode: resp.StatusCode,
				Message:    fmt.Sprintf("HTTP %d: %s", resp.StatusCode, string(bodyBytes)),
			}
		}

		if resp.StatusCode == http.StatusNoContent {
			return nil
		}

		if result != nil {
			if err := json.NewDecoder(resp.Body).Decode(result); err != nil {
				return fmt.Errorf("failed to decode response: %w", err)
			}
		}

		return nil
	}

	return fmt.Errorf("request failed after %d attempts: %w", c.config.MaxRetries+1, lastErr)
}

// Error represents an AkiDB error
type Error struct {
	StatusCode int
	Message    string
}

func (e *Error) Error() string {
	return e.Message
}

// CollectionsAPI provides collection operations
type CollectionsAPI struct {
	client *Client
}

// CreateCollectionRequest is the request for creating a collection
type CreateCollectionRequest struct {
	Name        string            `json:"name"`
	Dimension   int               `json:"dimension"`
	Metric      string            `json:"metric"`
	Description string            `json:"description,omitempty"`
	Metadata    map[string]string `json:"metadata,omitempty"`
}

// CollectionResponse represents a collection
type CollectionResponse struct {
	Name        string            `json:"name"`
	Dimension   int               `json:"dimension"`
	Metric      string            `json:"metric"`
	VectorCount int64             `json:"vector_count"`
	CreatedAt   string            `json:"created_at"`
	Metadata    map[string]string `json:"metadata,omitempty"`
}

// Create creates a new collection
func (api *CollectionsAPI) Create(ctx context.Context, req *CreateCollectionRequest) error {
	var result CollectionResponse
	return api.client.request(ctx, "POST", "/collections", req, &result)
}

// Get retrieves a collection
func (api *CollectionsAPI) Get(ctx context.Context, name string) (*CollectionResponse, error) {
	var result CollectionResponse
	err := api.client.request(ctx, "GET", fmt.Sprintf("/collections/%s", name), nil, &result)
	return &result, err
}

// List lists all collections
func (api *CollectionsAPI) List(ctx context.Context) ([]*CollectionResponse, error) {
	var result struct {
		Collections []*CollectionResponse `json:"collections"`
	}
	err := api.client.request(ctx, "GET", "/collections", nil, &result)
	return result.Collections, err
}

// Delete deletes a collection
func (api *CollectionsAPI) Delete(ctx context.Context, name string) error {
	return api.client.request(ctx, "DELETE", fmt.Sprintf("/collections/%s", name), nil, nil)
}

// VectorsAPI provides vector operations
type VectorsAPI struct {
	client *Client
}

// VectorInput represents a vector to insert
type VectorInput struct {
	ID       string                 `json:"id"`
	Vector   []float32              `json:"vector"`
	Metadata map[string]interface{} `json:"metadata,omitempty"`
}

// InsertResponse is the response from inserting vectors
type InsertResponse struct {
	Inserted int `json:"inserted"`
	Failed   int `json:"failed"`
}

// Insert inserts vectors into a collection
func (api *VectorsAPI) Insert(ctx context.Context, collection string, vectors []VectorInput) (*InsertResponse, error) {
	req := struct {
		Vectors []VectorInput `json:"vectors"`
	}{
		Vectors: vectors,
	}
	var result InsertResponse
	err := api.client.request(ctx, "POST", fmt.Sprintf("/collections/%s/vectors", collection), req, &result)
	return &result, err
}

// SearchRequest represents a search request
type SearchRequest struct {
	Query   []float32         `json:"query"`
	K       int               `json:"k"`
	Filters map[string]string `json:"filters,omitempty"`
}

// SearchResponse is the response from a search
type SearchResponse struct {
	Results []SearchResult `json:"results"`
	TookMS  int64          `json:"took_ms"`
}

// SearchResult represents a single search result
type SearchResult struct {
	ID       string                 `json:"id"`
	Distance float32                `json:"distance"`
	Metadata map[string]interface{} `json:"metadata,omitempty"`
}

// Search searches for similar vectors
func (api *VectorsAPI) Search(ctx context.Context, collection string, req *SearchRequest) (*SearchResponse, error) {
	var result SearchResponse
	err := api.client.request(ctx, "POST", fmt.Sprintf("/collections/%s/search", collection), req, &result)
	return &result, err
}

// BatchSearch performs multiple searches in one request
func (api *VectorsAPI) BatchSearch(ctx context.Context, collection string, queries []*SearchRequest) ([]*SearchResponse, error) {
	req := struct {
		Queries []*SearchRequest `json:"queries"`
	}{
		Queries: queries,
	}
	var result struct {
		Results []*SearchResponse `json:"results"`
	}
	err := api.client.request(ctx, "POST", fmt.Sprintf("/collections/%s/batch-search", collection), req, &result)
	return result.Results, err
}

// TenantsAPI provides tenant operations
type TenantsAPI struct {
	client *Client
}

// CreateTenantRequest is the request for creating a tenant
type CreateTenantRequest struct {
	Name     string            `json:"name"`
	Quotas   *TenantQuotas     `json:"quotas,omitempty"`
	Metadata map[string]string `json:"metadata,omitempty"`
}

// TenantQuotas represents tenant resource quotas
type TenantQuotas struct {
	MaxStorageBytes          *int64 `json:"max_storage_bytes,omitempty"`
	MaxCollections           *int   `json:"max_collections,omitempty"`
	MaxVectorsPerCollection  *int64 `json:"max_vectors_per_collection,omitempty"`
	APIRateLimitPerSecond    *int   `json:"api_rate_limit_per_second,omitempty"`
}

// TenantResponse represents a tenant
type TenantResponse struct {
	TenantID  string            `json:"tenant_id"`
	Name      string            `json:"name"`
	Status    string            `json:"status"`
	Quotas    TenantQuotas      `json:"quotas"`
	Usage     TenantUsage       `json:"usage"`
	CreatedAt string            `json:"created_at"`
	APIKey    string            `json:"api_key,omitempty"`
	Metadata  map[string]string `json:"metadata,omitempty"`
}

// TenantUsage represents tenant resource usage
type TenantUsage struct {
	StorageBytes    int64 `json:"storage_bytes"`
	CollectionCount int   `json:"collection_count"`
	TotalVectors    int64 `json:"total_vectors"`
}

// Create creates a new tenant
func (api *TenantsAPI) Create(ctx context.Context, req *CreateTenantRequest) (*TenantResponse, error) {
	var result TenantResponse
	err := api.client.request(ctx, "POST", "/tenants", req, &result)
	return &result, err
}

// Get retrieves a tenant
func (api *TenantsAPI) Get(ctx context.Context, tenantID string) (*TenantResponse, error) {
	var result TenantResponse
	err := api.client.request(ctx, "GET", fmt.Sprintf("/tenants/%s", tenantID), nil, &result)
	return &result, err
}

// List lists tenants
func (api *TenantsAPI) List(ctx context.Context, offset, limit int) ([]*TenantResponse, error) {
	path := fmt.Sprintf("/tenants?offset=%d&limit=%d", offset, limit)
	var result struct {
		Tenants []*TenantResponse `json:"tenants"`
		Total   int               `json:"total"`
	}
	err := api.client.request(ctx, "GET", path, nil, &result)
	return result.Tenants, err
}

// Delete deletes a tenant
func (api *TenantsAPI) Delete(ctx context.Context, tenantID string) error {
	return api.client.request(ctx, "DELETE", fmt.Sprintf("/tenants/%s", tenantID), nil, nil)
}

// HealthAPI provides health check operations
type HealthAPI struct {
	client *Client
}

// HealthResponse represents the health status
type HealthResponse struct {
	Status        string `json:"status"`
	Version       string `json:"version"`
	UptimeSeconds int64  `json:"uptime_seconds"`
}

// Status retrieves the health status
func (api *HealthAPI) Status(ctx context.Context) (*HealthResponse, error) {
	var result HealthResponse
	err := api.client.request(ctx, "GET", "/health", nil, &result)
	return &result, err
}

// DetailedHealthResponse represents detailed health information
type DetailedHealthResponse struct {
	HealthResponse
	Components map[string]ComponentHealth `json:"components"`
}

// ComponentHealth represents the health of a component
type ComponentHealth struct {
	Status  string `json:"status"`
	Message string `json:"message,omitempty"`
}

// Detailed retrieves detailed health information
func (api *HealthAPI) Detailed(ctx context.Context) (*DetailedHealthResponse, error) {
	var result DetailedHealthResponse
	err := api.client.request(ctx, "GET", "/health/details", nil, &result)
	return &result, err
}
