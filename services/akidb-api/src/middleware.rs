use tower::layer::Layer;

/// Placeholder API layer providing a hook for shared middleware.
#[derive(Debug, Clone, Default)]
pub struct ApiLayer;

impl<S> Layer<S> for ApiLayer {
    type Service = S;

    fn layer(&self, service: S) -> Self::Service {
        service
    }
}
