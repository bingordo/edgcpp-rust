//! Native EDG C++ reference-formation regression gate.
//!
//! The bridge is expected to normalize EDG's original eok_reference_to to
//! semantic operator 56, distinct from ordinary eok_address_of (43).
//! This only verifies source provenance; it does not authorize reference
//! lowering, C++ object lifetimes, or executable C++ ABI in any backend.
use edgcpp::{compile, graph::Graph};

unsafe extern "C" {
    fn edgcpp_codegen_kind(category: u32, value: u32) -> u32;
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = vec!["edgcpfe".to_owned()];
    args.extend(std::env::args().skip(1));
    if args.len() < 2 {
        return Err("usage: reference_formation [EDG options] fixture.cpp".into());
    }
    let result = compile(&args).map_err(|e| format!("EDG bridge: {e:?}"))?;
    if result.status != 0 {
        return Err(format!("EDG compilation failed: status {}", result.status).into());
    }
    // The normal graph verifier must accept the native, fully copied IL.
    Graph::from_il(&result.il).map_err(|e| format!("invalid EDG graph: {e:?}"))?;

    let mut references = 0usize;
    let mut addresses = 0usize;
    for expr in result.il.expressions.iter().filter(|expr|
        expr.function.contains("reference_formation_probe::exercise")
    ) {
        let Some(native_op) = expr.operator else { continue };
        let normalized = unsafe { edgcpp_codegen_kind(3, native_op) };
        match normalized {
            56 => references += 1,
            43 => addresses += 1,
            _ => {}
        }
    }
    println!("EDG_CXX_REFERENCE_FORMATION reference_to={references} address_of={addresses}");
    if references == 0 {
        return Err("EDG did not export source reference formation (operator 56) in exercise".into());
    }
    // An address-of might be folded by native lowering, so it is diagnostic
    // only; do not invent an address-of event that the producer omitted.
    Ok(())
}
