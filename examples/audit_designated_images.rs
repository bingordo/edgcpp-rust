//! Native EDG source oracle for bounded deep designated initializer images.
//! Invoke from compiler-ci with EDG's configured working directory.
//! This asserts copied IL bytes, not Cranelift object emission.
use edgcpp::compile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args=vec!["edgcpfe".to_owned()];
    args.extend(std::env::args().skip(1));
    if args.len()<3 {
        return Err("usage: cargo run --example audit_designated_images -- --c11 source.c".into());
    }
    let output=compile(&args).map_err(|error|format!("EDG compilation: {error:?}"))?;
    if output.status!=0 {
        return Err(format!("EDG parser returned {}",output.status).into());
    }
    let (name,expected)=match std::env::var("EDG_DESIGNATED_CASE").as_deref() {
        Ok("array")=>("deep_designated_array",37u8),
        Ok("record")=>("deep_designated_record",53u8),
        _=>return Err("set EDG_DESIGNATED_CASE=array or record".into()),
    };
    let symbol=output.il.symbols.iter().find(|item|item.name==name)
        .ok_or_else(||format!("missing native symbol {name}"))?.id;
    let image=output.il.global_aggregate_initializers.iter()
        .find(|entry|entry.symbol==symbol)
        .ok_or_else(||format!("missing native image for {name}"))?;
    if image.bytes.as_slice()!=[expected,0,0,0] {
        return Err(format!(
            "incorrect native image for {name}: {:?} (expected four bytes)",
            image.bytes).into());
    }
    println!("EDG_DEEP_DESIGNATOR_IMAGE name={name} bytes=4 value={expected}");
    Ok(())
}
