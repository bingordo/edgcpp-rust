//! Public original EDG C11 oracle for explicit pointer-to-64-bit integer
//! static initializer: byte-exact zero placeholders and symbolic source ID.
use edgcpp::compile;
fn main()->Result<(),Box<dyn std::error::Error>>{
    let mut args=vec![String::from("edgcpfe")];
    args.extend(std::env::args().skip(1));
    if args.len()<3 {return Err("expected C11 fixture pathname".into());}
    let unit=compile(&args).map_err(|e|format!("EDG compilation: {e:?}"))?;
    if unit.status!=0 {return Err("original EDG rejected C11 source".into());}
    let il=&unit.il;
    if il.cpp_language!=Some(false) {return Err("missing original C provenance".into());}
    let mut symbols=il.symbols.iter()
        .filter(|s|s.name=="bootstrap_integer_literal_union");
    let owner=symbols.next().ok_or("missing original union global")?.id;
    if symbols.next().is_some(){return Err("duplicate original union owner".into());}
    let mut images=il.global_aggregate_initializers.iter()
        .filter(|g|g.symbol==owner);
    let image=images.next().ok_or("no native initializer image")?;
    if images.next().is_some() || image.bytes.len()!=32 ||
        image.bytes.iter().any(|v|*v!=0) {
        return Err("C11 integer-address union lacks exact 32-byte zero placeholders".into());
    }
    let relocs:Vec<_>=il.global_literal_relocations.iter()
        .filter(|r|r.symbol==owner).collect();
    if relocs.len()!=1 || relocs[0].offset!=0 || relocs[0].addend!=0 {
        return Err("integer-address image needs one source-symbolic pointer".into());
    }
    let obj=il.global_literal_objects.iter()
        .find(|v|v.id==relocs[0].literal_id)
        .ok_or("missing original source-owned constant identity")?;
    if obj.bytes!=b"static-cast-object\0" {
        return Err("original EDG constant bytes mismatch independent C11 source".into());
    }
    println!("EDG_STATIC_INTEGER_ADDRESS_C11_OBJECTS={} RELOCATIONS=1 IMAGE_BYTES=32",
             il.global_literal_objects.len());
    Ok(())
}
