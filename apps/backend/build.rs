fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    std::env::set_var("PROTOC", protoc);

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(false)
        .compile_protos(
            &[
                "proto/arena360/v1/api.proto",
                "proto/arena360/v1/realtime.proto",
            ],
            &["proto"],
        )?;

    println!("cargo:rerun-if-changed=proto/arena360/v1/api.proto");
    println!("cargo:rerun-if-changed=proto/arena360/v1/realtime.proto");
    Ok(())
}
