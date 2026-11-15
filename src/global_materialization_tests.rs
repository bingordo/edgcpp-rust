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
    let m=state.il.global_materializations[0];
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
    let m=state.il.global_materializations[0];
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
