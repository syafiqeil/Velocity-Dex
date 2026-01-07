// crates/bench-tool/build.rs

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("../../proto/trading.proto")?;
    Ok(())
}