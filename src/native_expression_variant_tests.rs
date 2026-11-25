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
