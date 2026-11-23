//! Public GNU C11 differential fixture for weak static unions containing
//! a *routine* address and a separate *data-variable* address cast explicitly
//! into 64-bit word slots. All images must be exact zero-placeholder bytes.
//! No private generated C is used; the target source IDs come from EDG.
use edgcpp::{compile,Il,SymbolId};

/// Check the complete source-owned weak definition, exact byte extent and
/// two disjoint relocation records pointing at original named source IDs.
fn verify_owner(il:&Il,name:&str,bytes:usize,data_offset:u64,
    routine:SymbolId,variable:SymbolId)->Result<(),Box<dyn std::error::Error>> {
    let sources:Vec<_>=il.symbols.iter().filter(|s|s.kind==2 && s.name==name).collect();
    if sources.len()!=1 { return Err("missing or duplicate original weak union".into()); }
    let owner=sources[0].id;
    let mats:Vec<_>=il.global_materializations.iter()
        .filter(|m|m.symbol==owner).collect();
    if mats.len()!=1 || !mats[0].is_weak ||
       mats[0].is_weakref || mats[0].is_gnu_alias ||
       mats[0].is_nonreal ||
       !il.global_variables.iter().any(|g|
           g.symbol==owner && !g.thread_local && !g.local_to_function &&
           g.explicit_initializer) {
        return Err("weak union lacks original real file-scope storage".into());
    }
    let imgs:Vec<_>=il.global_aggregate_initializers.iter()
        .filter(|img|img.symbol==owner).collect();
    if imgs.len()!=1 || imgs[0].bytes.len()!=bytes ||
       imgs[0].bytes.iter().any(|x|*x!=0) {
        return Err("missing complete source-owned zero-placeholder image".into());
    }
    let routine_relocs:Vec<_>=il.global_routine_relocations.iter()
        .filter(|r|r.symbol==owner).collect();
    if routine_relocs.len()!=1 || routine_relocs[0].routine!=routine ||
       routine_relocs[0].offset!=0 || routine_relocs[0].addend!=0 {
        return Err("missing original routine relocation at word zero".into());
    }
    let data_relocs:Vec<_>=il.global_aggregate_relocations.iter()
        .filter(|r|r.symbol==owner).collect();
    if data_relocs.len()!=1 || data_relocs[0].target!=variable ||
       data_relocs[0].offset!=data_offset || data_relocs[0].addend!=0 {
        return Err("missing original data-address relocation at designated word".into());
    }
    if il.global_literal_relocations.iter().any(|r|r.symbol==owner) {
        return Err("unexpected overlapping literal identity".into());
    }
    Ok(())
}

fn main()->Result<(),Box<dyn std::error::Error>>{
    let mut args=vec![String::from("edgcpfe")];
    args.extend(std::env::args().skip(1));
    if args.len()<3 {return Err("expected original GNU C11 fixture".into());}
    let result=compile(&args).map_err(|e|format!("EDG compile: {e:?}"))?;
    if result.status!=0 || result.il.cpp_language!=Some(false) {
        return Err("EDG rejected public GNU C11 source".into());
    }
    let il=&result.il;
    let r:Vec<_>=il.symbols.iter().filter(|s|
        s.kind==1 && s.name=="bootstrap_weak_word_target").collect();
    let v:Vec<_>=il.symbols.iter().filter(|s|
        s.kind==2 && s.name=="bootstrap_weak_data_target").collect();
    if r.len()!=1 || v.len()!=1 {return Err("missing original source targets".into());}
    for (name,size,data_offset) in [
        ("bootstrap_weak_word5",40,32),
        ("bootstrap_weak_word6",48,40),
        ("bootstrap_weak_word8",64,48),
    ] {
        verify_owner(il,name,size,data_offset,r[0].id,v[0].id)?;
    }
    println!("EDG_WEAK_WORD_C11_IMAGES=3 ROUTINE_RELOCS=3 DATA_RELOCS=3");
    Ok(())
}
