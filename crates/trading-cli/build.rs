// crates/trading-cli/build.rs

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Arahkan ke file proto yang sama
    tonic_build::compile_protos("../../proto/trading.proto")?;
    Ok(())
}