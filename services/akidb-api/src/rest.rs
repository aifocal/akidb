use crate::{
    handlers::{
        batch_search_vectors, create_collection, delete_collection, detailed_health_handler,
        get_collection, insert_vectors, list_collections, liveness_handler, metrics_handler,
        readiness_handler, search_vectors,
    },
    middleware::{auth_middleware, track_metrics, AuthConfig},
    state::AppState,
};
use axum::{
    extract::Request,
    middleware,
    response::Response,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::{classify::ServerErrorsFailureClass, trace::TraceLayer};
use tracing::{info_span, Span};
use uuid::Uuid;

/// Builds the Axum router with custom auth config (for testing).
///
/// CRITICAL FIX (Bug #49): Separate public and protected routes to prevent
/// auth middleware from blocking health/metrics endpoints needed by
/// monitoring systems (Prometheus, Kubernetes).
pub fn build_router_with_auth(state: AppState, auth_config: AuthConfig) -> Router {
    let auth_config = Arc::new(auth_config);

    // PUBLIC ROUTES: Health checks and metrics (no authentication required)
    // These must remain accessible for Prometheus scraping and K8s probes
    let public_routes = Router::new()
        .route("/health", get(detailed_health_handler))
        .route("/health/live", get(liveness_handler))
        .route("/health/ready", get(readiness_handler))
        .route("/metrics", get(metrics_handler));

    // PROTECTED ROUTES: All API endpoints requiring authentication
    let protected_routes = Router::new()
        // Collection management
        .route(
            "/collections",
            get(list_collections).post(create_collection),
        )
        .route(
            "/collections/:name",
            get(get_collection).delete(delete_collection),
        )
        // Vector operations
        .route("/collections/:name/vectors", post(insert_vectors))
        // Search
        .route("/collections/:name/search", post(search_vectors))
        .route(
            "/collections/:name/batch-search",
            post(batch_search_vectors),
        )
        // Add authentication middleware ONLY to protected routes
        .layer(middleware::from_fn(move |req, next| {
            let config = auth_config.clone();
            auth_middleware(config, req, next)
        }));

    // Combine public and protected routes
    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        // Add state (shared across all routes)
        .with_state(state)
        // Add metrics middleware (tracks ALL requests including public routes)
        .layer(middleware::from_fn(track_metrics))
        // Add logging layer (outermost - logs ALL requests)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request| {
                    let request_id = Uuid::new_v4();
                    info_span!(
                        "http_request",
                        request_id = %request_id,
                        method = %request.method(),
                        uri = %request.uri(),
                        version = ?request.version(),
                    )
                })
                .on_request(|_request: &Request, _span: &Span| {
                    tracing::debug!("started processing request");
                })
                .on_response(|response: &Response, latency: std::time::Duration, _span: &Span| {
                    let status = response.status();
                    let latency_ms = latency.as_millis();

                    if status.is_server_error() {
                        tracing::error!(status = %status, latency_ms = latency_ms, "request failed with server error");
                    } else if status.is_client_error() {
                        tracing::warn!(status = %status, latency_ms = latency_ms, "request failed with client error");
                    } else {
                        tracing::info!(status = %status, latency_ms = latency_ms, "request completed");
                    }
                })
                .on_failure(|failure_class: ServerErrorsFailureClass, latency: std::time::Duration, _span: &Span| {
                    tracing::error!(failure_class = ?failure_class, latency_ms = latency.as_millis(), "request failed");
                }),
        )
}

/// Builds the Axum router hosting the REST facade for AkiDB (production mode).
/// Uses auth config from environment variables.
pub fn build_router(state: AppState) -> Router {
    build_router_with_auth(state, AuthConfig::from_env())
}
