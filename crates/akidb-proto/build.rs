fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile(
            &[
                "proto/akidb/collection/v1/collection.proto",
                "proto/akidb/embedding/v1/embedding.proto",
            ],
            &["proto"],
        )?;
    Ok(())
}
