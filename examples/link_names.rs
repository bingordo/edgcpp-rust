//! Inspect EDG-authored link names separately from demangled display keys.
 //! This is an IL/provenance probe, not a native C++ code generator.
use edgcpp::compile;
use std::error::Error;
fn main()->Result<(),Box<dyn Error>> {
    let args:Vec<_>=std::env::args().skip(1).collect();
    if args.is_empty() {
        return Err("usage: cargo run --example link_names -- [EDG options] source.cpp".into());
    }
    let mut cmd=vec!["edgcpfe".to_string()];
    cmd.extend(args);
    let unit=compile(&cmd).map_err(|e|format!("native EDG: {e:?}"))?;
    if unit.status!=0 {
        return Err(format!("EDG compilation status {}",unit.status).into());
    }
    for name in &unit.il.symbol_link_names {
        let display=unit.il.symbols.iter().find(|s|s.id==name.symbol)
            .map(|s|s.name.as_str()).unwrap_or("");
        println!("LINK\t{}\t{}\t{}\t{}",
            name.symbol.0,if name.edg_mangled {1} else {0},name.link_name,display);
    }
    Ok(())
}
