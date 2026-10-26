//! Compile a small EDG translation unit into an x86-64 ELF object.
//! Usage: codegen output.o [EDG options] source.cpp
use edgcpp::{codegen::emit_edg_object, compile};
use std::{error::Error, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        return Err("usage: codegen output.o [EDG options] source.cpp".into());
    }
    let mut edg_args = vec!["edgcpfe".to_string()];
    edg_args.extend(args[2..].iter().cloned());
    let compiled = compile(&edg_args).map_err(|e| format!("EDG bridge: {e:?}"))?;
    if compiled.status != 0 {
        return Err(format!("EDG compilation failed: status {}", compiled.status).into());
    }
    let object = emit_edg_object(&compiled.il).map_err(|e| {
        let functions: Vec<&str> = compiled.il.entities.iter().filter_map(|entity| {
            if let edgcpp::Entity::Routine { name, has_definition: true } = entity {
                Some(name.as_str())
            } else {
                None
            }
        }).collect();
        format!("EDG IL lowering failed: {e:?}; defined routines: {functions:?}")
    })?;
    fs::write(&args[1], object)?;
    Ok(())
}
