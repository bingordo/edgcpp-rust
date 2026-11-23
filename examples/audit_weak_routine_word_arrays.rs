//! Independent EDG original C11 control for 40/48/64-byte weak
//! union static data with an explicitly cast routine address. The source
//! fixture is public; missing native image, weak binding or target identity
//! makes this diagnostic fail rather than invent bytes or symbol values.
use edgcpp::{compile, Il, SymbolId};

/// Check one original source weak global and its one symbolic routine slot.
fn check_owner(il: &Il, name: &str, size: usize, target: SymbolId)
    -> Result<(), Box<dyn std::error::Error>>
{
    let sources: Vec<_> = il.symbols.iter()
        .filter(|s| s.kind == 2 && s.name == name).collect();
    if sources.len() != 1 { return Err("missing/duplicate original union symbol".into()); }
    let owner = sources[0].id;
    let materializations: Vec<_> = il.global_materializations.iter()
        .filter(|m| m.symbol == owner).collect();
    if materializations.len() != 1 || !materializations[0].is_weak
        || materializations[0].is_weakref || materializations[0].is_gnu_alias {
        return Err("original GNU weak definition not preserved".into());
    }
    let images: Vec<_> = il.global_aggregate_initializers.iter()
        .filter(|i| i.symbol == owner).collect();
    if images.len() != 1 || images[0].bytes.len() != size
        || images[0].bytes.iter().any(|byte| *byte != 0) {
        return Err("original weak union lacks exact zero placeholder image".into());
    }
    let relocations: Vec<_> = il.global_routine_relocations.iter()
        .filter(|r| r.symbol == owner).collect();
    if relocations.len() != 1 || relocations[0].routine != target
        || relocations[0].offset != 0 || relocations[0].addend != 0 {
        return Err("original weak union lacks exact routine relocation".into());
    }
    if il.global_literal_relocations.iter().any(|r| r.symbol == owner)
        || il.global_aggregate_relocations.iter().any(|r| r.symbol == owner) {
        return Err("weak routine relocation overlaps another symbol class".into());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = vec![String::from("edgcpfe")];
    args.extend(std::env::args().skip(1));
    if args.len() < 3 { return Err("expected original GNU C11 fixture path".into()); }
    let result = compile(&args).map_err(|error| format!("EDG compilation: {error:?}"))?;
    if result.status != 0 || result.il.cpp_language != Some(false) {
        return Err("original EDG did not accept the public GNU C fixture".into());
    }
    let il = &result.il;
    let targets: Vec<_> = il.symbols.iter()
        .filter(|s| s.kind == 1 && s.name == "bootstrap_weak_word_target").collect();
    if targets.len() != 1 { return Err("missing/duplicate source routine".into()); }
    for (name, size) in [
        ("bootstrap_weak_word5", 40),
        ("bootstrap_weak_word6", 48),
        ("bootstrap_weak_word8", 64),
    ] {
        check_owner(il, name, size, targets[0].id)?;
    }
    println!("EDG_WEAK_ROUTINE_WORD_C11_IMAGES=3 RELOCATIONS=3");
    Ok(())
}
