//! Standalone M4 external-data relocation smoke test.
//! cargo run --features codegen --example m4_data_probe -- /tmp/m4_probe.o
use std::path::PathBuf;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os().nth(1)
        .map(PathBuf::from).ok_or("usage: m4_data_probe <output.o>")?;
    let bytes = edgcpp::codegen::external_u32_increment_object("m4_counter")
        .map_err(|e| format!("{e:?}"))?;
    std::fs::write(output, bytes)?;
    Ok(())
}
