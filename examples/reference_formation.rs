//! Native EDG C++ reference-binding provenance regression gate.
//!
//! On the pinned EDG lowering, an lvalue bound to an int& formal can be
//! materialized with eok_address_of (43), just like an explicit pointer
//! argument. eok_reference_to (56) is a distinct normalized operator, but
//! its absence does NOT prove the absence of source reference semantics.
//! The callee's EDG-owned formal TypeId is the authority for that distinction.
//! No reference ABI, object lifetime, or C++ executable lowering is admitted.
use edgcpp::{compile, graph::Graph};

unsafe extern "C" {
    fn edgcpp_codegen_kind(category: u32, value: u32) -> u32;
}

fn semantic(category: u32, value: u32) -> u32 {
    unsafe { edgcpp_codegen_kind(category, value) }
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
    Graph::from_il(&result.il).map_err(|e| format!("invalid EDG graph: {e:?}"))?;

    let mut reference_formal = false;
    let mut pointer_formal = false;
    for name in ["by_reference", "by_pointer"] {
        let routine = result.il.routine_identities.iter()
            .find(|r| r.function == format!("reference_formation_probe::{name}"))
            .ok_or_else(|| format!("native EDG lost the {name} routine identity"))?;
        let params: Vec<_> = result.il.function_parameters.iter()
            .filter(|p| p.routine == routine.routine && !p.implicit_this).collect();
        if params.len() != 1 || params[0].position != Some(0) {
            return Err(format!("{name} must have exactly one source-owned formal").into());
        }
        let ty = params[0].ty;
        let detail = result.il.type_details.iter().find(|t| t.owner == ty)
            .ok_or_else(|| format!("missing native type detail for {name} formal"))?;
        let type_kind = result.il.types.iter().find(|t| t.id == ty)
            .map(|t| semantic(4, t.kind))
            .ok_or_else(|| format!("missing native type for {name} formal"))?;
        println!("EDG_CXX_FORMAL name={name} type={} kind={} reference={} rvalue={}",
                 ty.0, type_kind, detail.is_reference, detail.is_rvalue_reference);
        match name {
            "by_reference" => reference_formal =
                detail.is_reference && !detail.is_rvalue_reference,
            "by_pointer" => pointer_formal =
                !detail.is_reference && type_kind == 5,
            _ => unreachable!(),
        }
    }

    let mut references = 0usize;
    let mut addresses = 0usize;
    let mut calls = 0usize;
    let mut operators = std::collections::BTreeMap::<String, Vec<(u32, u32)>>::new();
    for expr in &result.il.expressions {
        let Some(native_op) = expr.operator else { continue };
        let normalized = semantic(3, native_op);
        operators.entry(expr.function.clone()).or_default().push((native_op, normalized));
        if expr.function == "reference_formation_probe::exercise" {
            match normalized {
                56 => references += 1,
                43 => addresses += 1,
                54 => calls += 1,
                _ => {}
            }
        }
    }
    for (function, kinds) in &operators {
        println!("EDG_CXX_OPERATOR_TRACE function={function:?} native_and_semantic={kinds:?}");
    }
    println!("EDG_CXX_REFERENCE_FORMATION reference_to={references} address_of={addresses} calls={calls} reference_formal={reference_formal} pointer_formal={pointer_formal}");
    // The fixed fixture contains one call through each *distinct source
    // formal* and two lowered address expressions. Never infer reference
    // semantics from those addresses alone or require operator 56 to appear.
    if !reference_formal || !pointer_formal || calls != 2 || addresses < 2 {
        return Err("EDG C++ source formal/operator provenance is incomplete".into());
    }
    Ok(())
}
