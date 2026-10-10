fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("PROTOC", protoc_bin_vendored::protoc_bin_path()?);
    let public = [
        "proto/public/arena360/v1/api.proto",
        "proto/public/arena360/v1/realtime.proto",
    ];
    tonic_prost_build::configure()
        .build_client(false)
        .compile_protos(&public, &["proto/public"])?;
    tonic_prost_build::configure().compile_protos(&["proto/storage.proto"], &["proto"])?;
    for path in public.iter().chain(std::iter::once(&"proto/storage.proto")) {
        println!("cargo:rerun-if-changed={path}");
    }
    Ok(())
}
