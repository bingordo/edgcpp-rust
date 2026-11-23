//! Original EDG C11 proof for an explicit function-address-to-64bit-word
//! static union. Validate source identity, exact image and relocation.
use edgcpp::compile;
fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut args=vec![String::from("edgcpfe")];
    args.extend(std::env::args().skip(1));
    if args.len()<3 {return Err("expected original C11 fixture path".into());}
    let unit=compile(&args).map_err(|e|format!("EDG compilation: {e:?}"))?;
    if unit.status!=0 || unit.il.cpp_language!=Some(false) {
        return Err("original EDG did not accept native C11 source".into());
    }
    let il=&unit.il;
    let owner=il.symbols.iter().find(|s|s.name=="bootstrap_routine_union")
        .ok_or("missing original EDG source union")?.id;
    let target=il.symbols.iter().find(|s|
        s.kind==1 && s.name=="bootstrap_routine_target")
        .ok_or("missing original EDG target routine")?.id;
    let images:Vec<_>=il.global_aggregate_initializers.iter()
        .filter(|i|i.symbol==owner).collect();
    if images.len()!=1 || images[0].bytes.len()!=32 ||
        images[0].bytes.iter().any(|v|*v!=0) {
        return Err("source union lacks exact 32-byte relocation placeholder".into());
    }
    let relocs:Vec<_>=il.global_routine_relocations.iter()
        .filter(|r|r.symbol==owner).collect();
    if relocs.len()!=1 || relocs[0].routine!=target ||
        relocs[0].offset!=0 || relocs[0].addend!=0 {
        return Err("source routine symbol relocation is incomplete".into());
    }
    if il.global_literal_relocations.iter().any(|r|r.symbol==owner) ||
        il.global_aggregate_relocations.iter().any(|r|r.symbol==owner) {
        return Err("routine relocation must not alias other target types".into());
    }
    println!("EDG_STATIC_ROUTINE_UNION_SYMBOLIC=1 BYTES=32 SLOTS=1");
    Ok(())
}
