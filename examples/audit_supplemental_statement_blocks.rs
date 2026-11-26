//! Audit structural capture of retained supplemental GNU statement expressions.
use edgcpp::{compile_structural,compile};
fn main()->Result<(),Box<dyn std::error::Error>> {
    let args:Vec<String>=std::iter::once("edgcpfe".to_owned())
        .chain(std::env::args().skip(1)).collect();
    let result=compile_structural(&args).map_err(|e|format!("{e:?}"))?;
    if result.status!=0 {return Err(format!("EDG failed: {}",result.status).into());}
    let nodes=&result.il.native_supplemental_block_nodes;
    let roots=nodes.iter().filter(|n|n.path=="0").count();
    let declarations=nodes.iter().filter(|n|n.category==3).count();
    let ordinary=result.il.expressions.iter().filter(|e| {
        unsafe {edgcpp_codegen_kind(2,e.kind)==4}
    }).count();
    // Some EDG lowering configurations remove the source block before the
    // backend callback. Do not confuse a successful parse with retained coverage.
    if roots!=0 && compile(&args).is_ok() {
        return Err("execution admission silently accepted detached blocks".into());
    }
    println!("EDG_SUPPLEMENTAL_BLOCKS roots={roots} nodes={} declarations={declarations} ordinary={ordinary} execution_admitted=false",nodes.len());
    Ok(())
}
extern "C" {fn edgcpp_codegen_kind(category:u32,value:u32)->u32;}
