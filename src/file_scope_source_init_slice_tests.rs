// Source-owned file-scope initialization slice tests.
// Exercise producer-member counts and ordinal/identity rejection independently
// of C++ class execution, startup ELF registration or destructor ABI.

use super::*;

/// Build a native unsigned wire field without borrowing transient storage.
fn number(value:u32)->WireField {
    WireField {data:std::ptr::null(),length:0,value:u64::from(value),
               tag:wire_tags::UNSIGNED}
}

/// Model a genuinely absent optional native destructor field.
fn absent()->WireField {
    WireField {data:std::ptr::null(),length:0,value:0,tag:wire_tags::ABSENT}
}

/// Submit one native wire record to the production collector.
fn send(state:&mut Collector,kind:u32,fields:&[WireField]) {
    unsafe {collect((state as *mut Collector).cast(),
                    kind,fields.as_ptr(),fields.len(),0);}
}

/// Provide native source routine and variable identities for coverage checks.
fn identities(il:&mut Il) {
    for (id,kind,name) in [(17,1,"generated_init"),(18,2,"object"),
                           (19,2,"object2"),(20,1,"destructor")] {
        il.symbols.push(Symbol{id:SymbolId(id),kind,name:name.into()});
    }
    il.routine_attributes.push(RoutineAttributes {
        routine:SymbolId(17),special_kind:0,is_virtual:false,
        pure_virtual:false,is_override:false,is_final:false,
        overrides_base_member:false,is_inline:false,is_constexpr:false,
        defined:true,explicit_constructor:false,has_this_parameter:false,
        compiler_generated:true,
    });
}

#[test]
fn exact_source_slice_preserves_processed_order_and_optional_destructor() {
    let mut state=Collector::default();
    send(&mut state,94,&[number(17),number(200),number(0),number(2)]);
    send(&mut state,95,&[number(17),number(0),number(18),absent(),number(0)]);
    send(&mut state,95,&[number(17),number(1),number(19),number(20),number(3)]);
    assert!(!state.malformed);
    assert_eq!(state.il.source_file_init_slices,
               vec![SourceFileInitSlice {
                   routine:SymbolId(17),priority:200,thread_local:false,
                   member_count:2,
               }]);
    assert_eq!(state.il.source_file_init_members.len(),2);
    assert_eq!(state.il.source_file_init_members[0].variable,SymbolId(18));
    assert_eq!(state.il.source_file_init_members[1].destructor,
               Some(SymbolId(20)));
    identities(&mut state.il);
    assert!(validate_source_file_init_slices(&state.il).is_ok());
    // Exact ordered member proof is NOT an ELF initialization plan.
    assert!(state.il.file_scope_init_routines.is_empty());
    assert!(state.il.file_scope_runtime_links.is_empty());
}

#[test]
fn missing_terminal_member_or_wrong_source_identity_is_rejected() {
    let mut state=Collector::default();
    send(&mut state,94,&[number(17),number(0),number(0),number(2)]);
    send(&mut state,95,&[number(17),number(0),number(18),absent(),number(0)]);
    identities(&mut state.il);
    assert!(validate_source_file_init_slices(&state.il).is_err());
    state.il.source_file_init_members.push(SourceFileInitMember {
        routine:SymbolId(17),position:1,variable:SymbolId(999),
        destructor:None,kind:0,
    });
    assert!(validate_source_file_init_slices(&state.il).is_err());
    state.il.source_file_init_members.last_mut().unwrap().variable=SymbolId(19);
    assert!(validate_source_file_init_slices(&state.il).is_ok());
    state.il.routine_attributes[0].compiler_generated=false;
    assert!(validate_source_file_init_slices(&state.il).is_err());
}

#[test]
fn duplicate_or_out_of_order_source_wire_is_malformed() {
    let mut state=Collector::default();
    send(&mut state,95,&[number(17),number(0),number(18),absent(),number(0)]);
    assert!(state.malformed, "orphaned member accepted");
    let mut state=Collector::default();
    send(&mut state,94,&[number(17),number(0),number(0),number(2)]);
    send(&mut state,94,&[number(17),number(0),number(0),number(2)]);
    assert!(state.malformed,"duplicate producer slice accepted");
    let mut state=Collector::default();
    send(&mut state,94,&[number(17),number(0),number(0),number(2)]);
    send(&mut state,95,&[number(17),number(1),number(18),absent(),number(0)]);
    assert!(state.malformed,"noncontiguous producer ordinal accepted");
}

#[test]
fn absent_observer_is_unknown_not_a_complete_initialization_certificate() {
    let mut state=Collector::default();
    assert!(validate_source_file_init_slices(&state.il).is_ok());
    assert!(state.il.source_file_init_slices.is_empty());
    for (routine,tls,count) in [(0,0,1),(17,2,1),(17,0,1025)] {
        let mut bad=Collector::default();
        send(&mut bad,94,&[number(routine),number(0),number(tls),number(count)]);
        assert!(bad.malformed);
    }
}
