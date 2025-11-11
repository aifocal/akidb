//! Protobuf definitions for AkiDB 2.0 gRPC API.

pub mod akidb {
    pub mod collection {
        pub mod v1 {
            tonic::include_proto!("akidb.collection.v1");
        }
    }

    pub mod embedding {
        pub mod v1 {
            tonic::include_proto!("akidb.embedding.v1");
        }
    }
}

pub use akidb::collection::v1::*;
pub use akidb::embedding::v1 as embedding;
