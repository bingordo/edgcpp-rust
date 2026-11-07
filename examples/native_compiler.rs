//! Compile a bounded EDG C11 integer source directly through
//! bingordo/compiler typed SSA, verifier and native x86 ELF emitter.
//! Usage: cargo run --features native-compiler --example native_compiler -- out.o --c11 source.c
use edgcpp::{compile,compiler_adapter::emit_edg_native_object};
use std::{error::Error,fs};
fn main()->Result<(),Box<dyn Error>> {
    let args:Vec<_>=std::env::args().collect();
    if args.len()<4 {
        return Err("usage: native_compiler output.o [EDG options] source.c".into());
    }
    let mut edg_args=vec!["edgcpfe".to_owned()];
    edg_args.extend(args[2..].iter().cloned());
    let unit=compile(&edg_args).map_err(|e|format!("EDG bridge: {e:?}"))?;
    if unit.status!=0 {
        return Err(format!("EDG compilation failed with status {}",unit.status).into());
    }
    let object=emit_edg_native_object(&unit.il).map_err(|e|
        format!("EDG -> compiler-ir rejected source: {e}"))?;
    // Failed source never leaves a partially generated output object.
    fs::write(&args[1],object)?;
    Ok(())
}
