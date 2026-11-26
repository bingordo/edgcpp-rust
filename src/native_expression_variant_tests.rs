//! Wire regression tests for owner-keyed native expression variant metadata.
use super::*;

fn text(s:&str)->WireField {
    WireField{data:s.as_ptr(),length:s.len(),value:0,tag:wire_tags::TEXT}
}
fn num(n:u32)->WireField {
    WireField{data:std::ptr::null(),length:0,value:u64::from(n),tag:wire_tags::UNSIGNED}
}
fn absent()->WireField {
    WireField{data:std::ptr::null(),length:0,value:0,tag:wire_tags::ABSENT}
}
fn send(state:&mut Collector,fields:[WireField;9]) {
    unsafe {collect((state as *mut Collector).cast(),107,fields.as_ptr(),fields.len(),0);}
}
fn statement()->Collector {
    let mut state=Collector::default();
    state.il.expressions.push(Expression{
        function:"f".into(),statement_path:"s".into(),path:"0".into(),
        parent:None,kind:89,operator:None,type_kind:None,reference:None,
        type_size:None,type_alignment:None,constant_kind:None,constant_integer:None,
        related_type_role:None,related_type_kind:None,type_id:None,symbol_id:None,
    });
    state
}
fn statement_fields(variant:u32,payload:WireField)->[WireField;9] {
    [num(0),text("f"),text("s"),absent(),num(0),text("0"),num(89),num(variant),payload]
}

#[test]
fn variant_payloads_preserve_zero_builtin_and_type_operand_identity() {
    let mut state=statement();
    send(&mut state,statement_fields(1,num(0)));
    assert!(!state.malformed);
    assert_eq!(state.il.native_expression_variants[0].payload,
        NativeExpressionPayload::BuiltinOperation(0));
    let mut state=statement();
    state.seen.types.insert(TypeId(42));
    send(&mut state,statement_fields(3,num(42)));
    assert!(!state.malformed);
    assert_eq!(state.il.native_expression_variants[0].payload,
        NativeExpressionPayload::TypeOperand(Some(TypeId(42))));
    let mut state=statement();
    send(&mut state,statement_fields(3,absent()));
    assert!(!state.malformed);
    assert_eq!(state.il.native_expression_variants[0].payload,
        NativeExpressionPayload::TypeOperand(None));
}

#[test]
fn variant_owners_keep_equal_paths_in_separate_trees() {
    let mut state=statement();
    send(&mut state,statement_fields(2,num(1)));
    state.il.control_expressions.push(ControlExpression{
        function:"f".into(),statement_path:"s".into(),role:1,path:"0".into(),
        parent:None,kind:89,operator:None,type_id:None,symbol_id:None,
    });
    send(&mut state,[num(1),text("f"),text("s"),absent(),num(1),text("0"),
        num(89),num(2),num(0)]);
    state.il.initializer_expressions.push(InitializerExpression{
        owner:SymbolId(9),path:"0".into(),parent:None,kind:89,
        operator:None,type_id:None,symbol_id:None,
    });
    send(&mut state,[num(2),absent(),absent(),num(9),num(0),text("0"),
        num(89),num(4),absent()]);
    state.il.constructor_initializer_expressions.push(ConstructorInitializerExpression{
        routine:SymbolId(9),position:0,path:"0".into(),parent:None,kind:89,
        operator:None,type_id:None,symbol_id:None,
    });
    send(&mut state,[num(3),absent(),absent(),num(9),num(0),text("0"),
        num(89),num(4),absent()]);
    assert!(!state.malformed);
    assert_eq!(state.il.native_expression_variants.len(),4);
    assert_eq!(state.il.native_expression_variants[1].payload,
        NativeExpressionPayload::BuiltinChooseFirst(false));
}

#[test]
fn malformed_or_duplicate_variant_metadata_rejects() {
    for fields in [
        statement_fields(2,num(2)),
        statement_fields(3,num(99)),
        statement_fields(1,absent()),
        statement_fields(6,num(0)),
        statement_fields(5,num(128 << 8)),
        statement_fields(5,absent()),
    ] {
        let mut state=statement();
        send(&mut state,fields);
        assert!(state.malformed && !state.panicked);
    }
    let mut state=statement();
    let mut wrong=statement_fields(1,num(0));
    wrong[6]=num(88);
    send(&mut state,wrong);
    assert!(state.malformed);
    let mut state=statement();
    send(&mut state,statement_fields(1,num(0)));
    send(&mut state,statement_fields(1,num(0)));
    assert!(state.malformed);
    let mut state=Collector::default();
    send(&mut state,statement_fields(1,num(0)));
    assert!(state.malformed);
}

#[test]
fn dynamic_initializer_kind_and_presence_are_not_object_lifetime_proof() {
    let mut state=statement();
    send(&mut state,statement_fields(5,num(3 | (19 << 8))));
    assert!(!state.malformed);
    assert_eq!(state.il.native_expression_variants[0].payload,
        NativeExpressionPayload::DynamicInitializer {kind:3,flags:19});
}

fn native_variant_kind(family:u32)->u32 {
    (0..4096).find(|&kind|graph::native_expression_variant_family(kind)==family)
        .expect("configured native variant kind")
}

#[test]
fn independent_metadata_coverage_rejects_missing_wrong_and_extra_records() {
    let mut state=statement();
    let kind=native_variant_kind(1);
    state.il.expressions[0].kind=kind;
    let mut fields=statement_fields(1,num(0));
    fields[6]=num(kind);
    send(&mut state,fields);
    assert!(!state.malformed);
    state.il.control_expressions.push(ControlExpression{
        function:"f".into(),statement_path:"s".into(),role:1,path:"0".into(),
        parent:None,kind,operator:None,type_id:None,symbol_id:None,
    });
    state.il.initializer_expressions.push(InitializerExpression{
        owner:SymbolId(9),path:"0".into(),parent:None,kind,
        operator:None,type_id:None,symbol_id:None,
    });
    state.il.constructor_initializer_expressions.push(ConstructorInitializerExpression{
        routine:SymbolId(9),position:0,path:"0".into(),parent:None,kind,
        operator:None,type_id:None,symbol_id:None,
    });
    assert!(validate_native_expression_variant_coverage(&state.il).is_err());
    send(&mut state,[num(1),text("f"),text("s"),absent(),num(1),text("0"),num(kind),num(1),num(0)]);
    send(&mut state,[num(2),absent(),absent(),num(9),num(0),text("0"),num(kind),num(1),num(0)]);
    send(&mut state,[num(3),absent(),absent(),num(9),num(0),text("0"),num(kind),num(1),num(0)]);
    assert!(!state.malformed);
    validate_native_expression_variant_coverage(&state.il).unwrap();
    for index in 0..4 {
        let mut missing=state.il.clone();
        missing.native_expression_variants.remove(index);
        assert!(validate_native_expression_variant_coverage(&missing).is_err());
    }
    let mut wrong=state.il.clone();
    wrong.native_expression_variants[0].payload=NativeExpressionPayload::TypeOperand(None);
    assert!(validate_native_expression_variant_coverage(&wrong).is_err());
    let mut extra=state.il.clone();
    extra.native_expression_variants.push(extra.native_expression_variants[0].clone());
    assert!(validate_native_expression_variant_coverage(&extra).is_err());
}

#[test]
fn generic_selected_operand_requires_original_direct_value_child() {
    let mut state=statement();
    let kind=native_variant_kind(6);
    state.il.expressions[0].kind=kind;
    let mut fields=statement_fields(6,num(2));
    fields[6]=num(kind);
    send(&mut state,fields);
    assert!(!state.malformed);
    assert!(validate_native_expression_variant_coverage(&state.il).is_err());
    let mut child=state.il.expressions[0].clone();
    child.path="0.2".into();
    child.parent=Some("0".into());
    child.kind=(0..4096).find(|&k|graph::native_expression_kind(k)==1).unwrap();
    state.il.expressions.push(child);
    validate_native_expression_variant_coverage(&state.il).unwrap();
    let mut detached=state.il.clone();
    detached.expressions[1].parent=None;
    assert!(validate_native_expression_variant_coverage(&detached).is_err());
    let mut type_operand=state.il.clone();
    type_operand.expressions[1].kind=native_variant_kind(3);
    assert!(validate_native_expression_variant_coverage(&type_operand).is_err());
    let mut wrong_ordinal=state.il.clone();
    wrong_ordinal.native_expression_variants[0].payload=NativeExpressionPayload::GenericSelectedOperand(0);
    assert!(validate_native_expression_variant_coverage(&wrong_ordinal).is_err());
}


#[test]
fn constructor_provenance_distinguishes_source_fallback_and_executable_roots() {
    let mut state=Collector::default();
    state.il.constructor_initializers.push(ConstructorInitializer {
        routine:SymbolId(9),position:0,kind:3,target_type:None,target_field:None,
        compiler_generated:false,braced:false,use_field_initializer:false,
        pack_expansion:false,
    });
    state.il.native_constructor_root_admissions.push(NativeConstructorRootAdmission {
        routine:SymbolId(9),position:0,present:true,
    });
    let fields=[num(9),num(0),num(4),num(2),num(1),num(0),num(2)];
    assert!(validate_constructor_init_provenance(&state.il).is_err());
    unsafe {collect((&mut state as *mut Collector).cast(),108,fields.as_ptr(),fields.len(),0);}
    assert!(!state.malformed);
    validate_constructor_init_provenance(&state.il).unwrap();
    let p=&state.il.native_constructor_init_provenance[0];
    assert_eq!(p.executable_root_count,2);
    assert_eq!(p.selected_root,2);
    assert!(!p.source_is_executable_root);
    for (index,value) in [(3,num(0)),(4,num(0)),(5,num(2)),(6,num(1))] {
        let mut bad=fields;
        bad[index]=value;
        if index==3 {bad[5]=num(1);}
        let mut other=Collector::default();
        unsafe {collect((&mut other as *mut Collector).cast(),108,bad.as_ptr(),bad.len(),0);}
        assert!(other.malformed);
    }
    let mut wrong=state.il.clone();
    wrong.native_constructor_init_provenance[0].position=1;
    assert!(validate_constructor_init_provenance(&wrong).is_err());
    let mut missing=state.il.clone();
    missing.native_constructor_init_provenance.clear();
    assert!(validate_constructor_init_provenance(&missing).is_err());
    let mut duplicate=state.il.clone();
    duplicate.native_constructor_init_provenance.push(p.clone());
    assert!(validate_constructor_init_provenance(&duplicate).is_err());
}
