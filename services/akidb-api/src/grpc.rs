use tonic::transport::Server;

/// Builds a tonic gRPC server configured for AkiDB services.
pub fn build_grpc_server() -> Server {
    Server::builder()
}
