//! Independent malformed-wire and native variable classification regressions.
//! Bit witnesses only describe EDG metadata; they never grant ELF startup.

use super::*;

fn send<const N:usize>(state:&mut Collector,kind:u32,values:[u64;N]){
    let fields=values.map(|value|WireField {
        data:std::ptr::null(),length:0,value,tag:wire_tags::UNSIGNED,
    });
    unsafe {collect((state as *mut Collector).cast(),kind,fields.as_ptr(),N,0);}
}

#[test]
fn native_global_flags_preserve_each_source_owned_property() {
    let mut state=Collector::default();
    send(&mut state,47,[91,2,1,0,0]);
    send(&mut state,100,[91,127]);
    assert!(!state.malformed);
    assert!(!state.panicked);
    assert_eq!(state.il.global_materializations.len(),1);
    let m=&state.il.global_materializations[0];
    assert_eq!(m.symbol,SymbolId(91));
    assert!(m.is_nonreal);
    assert!(m.is_prototype_instantiation);
    assert!(m.is_template_variable);
    assert!(m.compiler_generated);
    assert!(m.used);
    assert!(m.address_taken);
    assert!(m.is_class_member);
}

#[test]
fn concrete_variable_is_distinct_from_uninstantiated_template() {
    let mut state=Collector::default();
    send(&mut state,47,[91,3,0,0,0]);
    send(&mut state,100,[91,0]);
    assert!(!state.malformed);
    let m=&state.il.global_materializations[0];
    assert!(!m.is_nonreal && !m.is_template_variable &&
            !m.is_prototype_instantiation && !m.compiler_generated);
}

#[test]
fn orphan_duplicate_and_unknown_native_property_bits_fail_closed() {
    for bad in [0u64,128u64] {
        let mut state=Collector::default();
        send(&mut state,47,[91,3,0,0,0]);
        send(&mut state,100,[91,bad]);
        if bad==0 {
            assert!(!state.malformed);
            send(&mut state,100,[91,0]);
            assert!(state.malformed,"duplicate flag event accepted");
        } else {
            assert!(state.malformed,"unknown high bit accepted");
        }
    }
    let mut orphan=Collector::default();
    send(&mut orphan,100,[91,1]);
    assert!(orphan.malformed,"missing global owner accepted");
}

#[test]
fn complete_native_snapshot_requires_one_materialization_per_global() {
    assert!(validate_native_global_materialization_coverage(&Il::default()).is_ok(),
        "an empty translation unit has no file-scope owners");
    let mut state=Collector::default();
    send(&mut state,47,[91,3,0,0,0]);
    send(&mut state,100,[91,1]);
    send(&mut state,47,[92,1,0,0,0]);
    send(&mut state,100,[92,0]);
    assert!(!state.malformed);
    let complete=state.il;
    validate_native_global_materialization_coverage(&complete)
        .expect("exact pair of independent native variable/provenance records");

    let mut dropped=complete.clone();
    dropped.global_materializations.pop();
    assert!(validate_native_global_materialization_coverage(&dropped).is_err(),
        "dropping the final event 100 must reject a successful native snapshot");

    let mut missing_all=complete.clone();
    missing_all.global_materializations.clear();
    assert!(validate_native_global_materialization_coverage(&missing_all).is_err());

    let mut orphan=complete.clone();
    orphan.global_materializations[1].symbol=SymbolId(99);
    assert!(validate_native_global_materialization_coverage(&orphan).is_err(),
        "equal counts do not prove exact owner identity");

    let mut duplicate=complete.clone();
    duplicate.global_materializations[1].symbol=SymbolId(91);
    assert!(validate_native_global_materialization_coverage(&duplicate).is_err(),
        "same owner cannot be covered twice");

    let mut double_global=complete.clone();
    double_global.global_variables[1].symbol=SymbolId(91);
    assert!(validate_native_global_materialization_coverage(&double_global).is_err(),
        "duplicate source-owned file-scope variables are ambiguous");
}
