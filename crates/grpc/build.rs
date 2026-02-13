//! This tells tonic-build to compile your protobufs when you build your Rust project.
/// While you can configure this build process in a number of ways.
/// More tonic-build documentation details [here](https://github.com/hyperium/tonic/blob/master/tonic-build/README.md)
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .protoc_arg("--experimental_allow_proto3_optional") // for older systems
        .out_dir("src/gen")
        .compile(&["proto/relay.proto"], &["proto"])?;

    Ok(())
}
