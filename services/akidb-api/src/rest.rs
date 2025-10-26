use crate::{
    handlers::{
        batch_search_vectors, create_collection, delete_collection, get_collection, insert_vectors,
        list_collections, metrics_handler, search_vectors,
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
pub fn build_router_with_auth(state: AppState, auth_config: AuthConfig) -> Router {
    let auth_config = Arc::new(auth_config);

    Router::new()
        // Health check and metrics (no auth required)
        .route("/health", get(health_check))
        .route("/metrics", get(metrics_handler))
        // Collection management
        .route("/collections", get(list_collections).post(create_collection))
        .route(
            "/collections/:name",
            get(get_collection).delete(delete_collection),
        )
        // Vector operations
        .route("/collections/:name/vectors", post(insert_vectors))
        // Search
        .route("/collections/:name/search", post(search_vectors))
        .route("/collections/:name/batch-search", post(batch_search_vectors))
        // Add state
        .with_state(state)
        // Add metrics middleware (tracks ALL requests, applied before auth)
        .layer(middleware::from_fn(track_metrics))
        // Add authentication middleware
        .layer(middleware::from_fn(move |req, next| {
            let config = auth_config.clone();
            auth_middleware(config, req, next)
        }))
        // Add logging layer
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

async fn health_check() -> &'static str {
    "ok"
}
