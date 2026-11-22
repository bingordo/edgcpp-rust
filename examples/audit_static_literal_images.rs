//! Public EDG C11 oracle for static union pointer-to-string literal images.
//! Run with the original pinned EDG frontend and C11/GNU flags. Distinct
//! source constants may share the same string bytes without sharing identity.
use edgcpp::compile;
use std::collections::HashMap;
fn main() -> Result<(),Box<dyn std::error::Error>> {
    let mut args=vec![String::from("edgcpfe")];
    args.extend(std::env::args().skip(1));
    if args.len()<3 {return Err("expected original C fixture path".into());}
    let unit=compile(&args).map_err(|error|format!("EDG compilation: {error:?}"))?;
    if unit.status!=0 {return Err("EDG did not accept public fixture".into());}
    let il=&unit.il;
    let global=il.symbols.iter()
        .find(|s|s.name=="bootstrap_literal_union")
        .ok_or("missing native union symbol")?.id;
    let images:Vec<_>=il.global_aggregate_initializers.iter()
        .filter(|v|v.symbol==global).collect();
    if images.len()!=1 || images[0].bytes.len()!=32 ||
        images[0].bytes.iter().any(|byte|*byte!=0) {
        return Err("global union must have one exact 32-byte pointer image".into());
    }
    let refs:Vec<_>=il.global_literal_relocations.iter()
        .filter(|v|v.symbol==global).collect();
    if refs.len()!=4 {return Err("global union must have four literal slots".into());}
    let targets:HashMap<u32,_>=il.global_literal_objects.iter()
        .map(|object|(object.id,object)).collect();
    if targets.len()!=il.global_literal_objects.len() ||
        targets.keys().any(|key|*key==0) {
        return Err("duplicate/zero literal source identity".into());
    }
    let expected:[&[u8];4]=[
        b"first-bootstrap\0",b"second-bootstrap\0",
        b"first-bootstrap\0",b"fourth\0",
    ];
    for (index,reloc) in refs.into_iter().enumerate() {
        if reloc.offset!=(index*8) as u64 || reloc.addend!=0 {
            return Err("unverified global literal relocation position".into());
        }
        let object=targets.get(&reloc.literal_id)
            .ok_or("missing literal object referenced by global union")?;
        if object.bytes!=expected[index] {
            return Err("literal byte image differs from independent C source".into());
        }
    }
    println!("EDG_STATIC_LITERAL_C11_OBJECTS={} SLOTS=4 IMAGE_BYTES=32",
             il.global_literal_objects.len());
    Ok(())
}
