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


#[test]
fn constructor_constant_tree_preserves_repeat_and_rejects_missing_children() {
    let mut state=Collector::default();
    state.il.constructor_initializers.push(ConstructorInitializer {
        routine:SymbolId(9),position:0,kind:3,target_type:None,target_field:None,
        compiler_generated:false,braced:true,use_field_initializer:false,
        pack_expansion:false,
    });
    let root=[num(9),num(0),num(1),num(0),absent(),absent()];
    unsafe {collect((&mut state as *mut Collector).cast(),109,root.as_ptr(),root.len(),0);}
    state.seen.types.insert(TypeId(42));
    let records=[
        [num(9),num(0),text("0"),absent(),num(1),num(42),num(1),num(2),absent(),num(1),num(0),absent(),absent()],
        [num(9),num(0),text("0.0"),text("0"),num(2),absent(),num(3),num(0),num(3),num(0),num(0),absent(),absent()],
        [num(9),num(0),text("0.1"),text("0"),num(3),absent(),num(2),num(1),num(5),num(0),num(0),absent(),absent()],
        [num(9),num(0),text("0.1.0"),text("0.1"),num(4),num(42),num(4),num(0),num(17),num(0),num(0),absent(),absent()],
    ];
    for fields in records {
        unsafe {collect((&mut state as *mut Collector).cast(),110,fields.as_ptr(),fields.len(),0);}
    }
    assert!(!state.malformed);
    validate_ctor_constant_trees(&state.il).unwrap();
    for index in 0..4 {
        let mut missing=state.il.clone();
        missing.native_ctor_constant_nodes.remove(index);
        assert!(validate_ctor_constant_trees(&missing).is_err());
    }
    let mut wrong=state.il.clone();
    wrong.native_ctor_constant_nodes[2].child_count=0;
    assert!(validate_ctor_constant_trees(&wrong).is_err());
    let mut wrong=state.il.clone();
    wrong.native_ctor_constant_nodes[3].parent=Some("0".into());
    assert!(validate_ctor_constant_trees(&wrong).is_err());
    let mut wrong=state.il.clone();
    wrong.native_ctor_constant_roots.clear();
    assert!(validate_ctor_constant_trees(&wrong).is_err());
    let mut wrong=state.il.clone();
    wrong.native_ctor_constant_nodes.push(wrong.native_ctor_constant_nodes[0].clone());
    assert!(validate_ctor_constant_trees(&wrong).is_err());
}


#[test]
fn dynamic_init_identity_joins_ordered_destructions_without_name_matching() {
    let mut il=Il::default();
    let routine=SymbolId(9);
    il.routine_identities.push(RoutineIdentity{routine,function:"f".into()});
    il.object_lifetimes.extend([
        ObjectLifetime{routine,id:0,parent:None,kind:0},
        ObjectLifetime{routine,id:1,parent:Some(0),kind:1},
    ]);
    for position in 0..2 {
        il.native_constructor_init_provenance.push(NativeConstructorInitProvenance {
            routine,position,dynamic_kind:Some(4),executable_root_count:0,
            source_root_present:false,source_is_executable_root:false,selected_root:0,
        });
        il.lifetime_destructions.push(LifetimeDestruction{
            routine,lifetime:0,position,variable:None,destructor:Some(SymbolId(55)),
        });
        il.native_dynamic_init_identities.push(NativeDynamicInitIdentity {
            routine,context:0,position,path:None,destruction_region:None,
            id:position+1,kind:4,flags:7,lifetime:Some(0),expression_lifetime:Some(1),
            has_variable:false,
        });
        il.native_dynamic_init_identities.push(NativeDynamicInitIdentity {
            routine,context:2,position,path:None,destruction_region:Some(0),
            id:2-position,kind:4,flags:7,lifetime:Some(0),expression_lifetime:None,
            has_variable:false,
        });
    }
    validate_dynamic_init_identities(&il).unwrap();
    for i in 0..il.native_dynamic_init_identities.len() {
        let mut missing=il.clone();
        missing.native_dynamic_init_identities.remove(i);
        assert!(validate_dynamic_init_identities(&missing).is_err());
    }
    let mut wrong=il.clone();
    wrong.native_dynamic_init_identities[1].id=1;
    assert!(validate_dynamic_init_identities(&wrong).is_err());
    let mut wrong=il.clone();
    wrong.native_dynamic_init_identities[1].expression_lifetime=Some(0);
    assert!(validate_dynamic_init_identities(&wrong).is_err());
    let mut wrong=il.clone();
    wrong.native_dynamic_init_identities[1].destruction_region=Some(1);
    assert!(validate_dynamic_init_identities(&wrong).is_err());
}


#[test]
fn constant_tree_owners_cannot_alias_equal_paths_across_contexts() {
    let mut il=Il::default();
    let routine=SymbolId(9);
    il.routine_identities.push(RoutineIdentity{routine,function:"f".into()});
    il.statement_initializations.push(StatementInitialization {
        function:"f".into(),statement_path:"0".into(),symbol:None,kind:1,
        follows_execution:false,braced:true,role:0,
    });
    il.dynamic_initializers.push(DynamicInitializer {
        symbol:routine,kind:1,expression_kind:None,operator:None,
    });
    il.native_expression_variants.push(NativeExpressionVariant {
        owner:NativeExpressionOwner::Statement{function:"f".into(),statement_path:"0".into()},
        path:"0".into(),expression_kind:1,
        payload:NativeExpressionPayload::DynamicInitializer{kind:1,flags:0},
    });
    for (context,statement,expression) in [(1,Some("0"),None),(2,None,None),
        (3,Some("0"),Some("0"))] {
        il.native_ctor_constant_roots.push(NativeCtorConstantRoot{
            routine,position:0,present:true,context,
            owner_statement:statement.map(str::to_owned),
            owner_expression:expression.map(str::to_owned),
        });
        il.native_ctor_constant_nodes.push(NativeCtorConstantNode{
            routine,position:0,path:"0".into(),parent:None,kind:1,
            ty:Some(TypeId(4)),family:4,child_count:0,
            payload:Some(NativeCtorConstantPayload::Unsigned(42)),flags:0,context,
            owner_statement:statement.map(str::to_owned),
            owner_expression:expression.map(str::to_owned),
        });
    }
    validate_ctor_constant_trees(&il).unwrap();
    for i in 0..3 {
        let mut missing=il.clone();
        missing.native_ctor_constant_roots.remove(i);
        assert!(validate_ctor_constant_trees(&missing).is_err());
        let mut missing=il.clone();
        missing.native_ctor_constant_nodes.remove(i);
        assert!(validate_ctor_constant_trees(&missing).is_err());
    }
    let mut wrong=il.clone();
    wrong.native_ctor_constant_nodes[2].context=1;
    wrong.native_ctor_constant_nodes[2].owner_expression=None;
    assert!(validate_ctor_constant_trees(&wrong).is_err());
}


#[test]
fn executable_initializer_forest_retains_ordered_constructor_arguments_and_nested_roots() {
    let mut il=Il::default();
    let routine=SymbolId(9);
    il.native_constructor_init_provenance.push(NativeConstructorInitProvenance {
        routine,position:0,dynamic_kind:Some(4),executable_root_count:2,
        source_root_present:true,source_is_executable_root:false,selected_root:2,
    });
    il.native_ctor_constant_roots.push(NativeCtorConstantRoot {
        routine,position:0,present:true,context:0,owner_statement:None,owner_expression:None,
    });
    il.native_ctor_constant_nodes.push(NativeCtorConstantNode {
        routine,position:0,path:"0".into(),parent:None,kind:1,ty:None,
        family:5,child_count:0,payload:Some(NativeCtorConstantPayload::Unsigned(4)),
        flags:1,context:0,owner_statement:None,owner_expression:None,
    });
    let owner=NativeInitializerExecOwner {
        symbol:routine,position:0,context:0,statement:None,expression:None,nested:None,
    };
    let nested=NativeInitializerExecOwner{nested:Some("0".into()),..owner.clone()};
    il.native_initializer_exec_roots.extend([
        NativeInitializerExecRoots{owner:owner.clone(),count:2,constructor:Some(SymbolId(77))},
        NativeInitializerExecRoots{owner:nested.clone(),count:1,constructor:None},
    ]);
    let kind=(0..4096).find(|&k|graph::native_expression_kind(k)!=4).unwrap();
    for (owner,path,parent,count) in [
        (owner.clone(),"0",None,1),(owner.clone(),"0.0",Some("0"),0),
        (owner,"1",None,0),(nested,"0",None,0),
    ] {
        il.native_initializer_exec_nodes.push(NativeInitializerExecNode {
            owner,path:path.into(),parent:parent.map(str::to_owned),kind,
            operator:None,ty:None,symbol:None,child_count:count,
            integer:None,embedded_statement:false,
        });
    }
    validate_initializer_exec_trees(&il).unwrap();
    for i in 0..il.native_initializer_exec_nodes.len() {
        let mut wrong=il.clone();
        wrong.native_initializer_exec_nodes.remove(i);
        assert!(validate_initializer_exec_trees(&wrong).is_err());
    }
    let mut wrong=il.clone();
    wrong.native_initializer_exec_roots[0].count=1;
    assert!(validate_initializer_exec_trees(&wrong).is_err());
    let mut wrong=il.clone();
    wrong.native_initializer_exec_roots[1].owner.nested=Some("1".into());
    assert!(validate_initializer_exec_trees(&wrong).is_err());
    let mut wrong=il.clone();
    wrong.native_initializer_exec_nodes[1].parent=Some("1".into());
    assert!(validate_initializer_exec_trees(&wrong).is_err());
}


#[test]
fn supplemental_statement_graph_requires_original_owner_and_complete_branch_lists() {
    let mut il=Il::default();
    let routine=SymbolId(9);
    il.routine_identities.push(RoutineIdentity{routine,function:"f".into()});
    let stmt_kind=|n| (0..4096).find(|&k|graph::native_statement_kind(k)==n).unwrap();
    let expr_kind=|n| (0..4096).find(|&k|graph::native_expression_kind(k)==n).unwrap();
    il.control_expressions.push(ControlExpression {
        function:"f".into(),statement_path:"0".into(),role:1,path:"0".into(),
        parent:None,kind:expr_kind(4),operator:None,type_id:None,symbol_id:None,
    });
    let owner=NativeSupplementalBlockOwner {
        initializer:NativeInitializerExecOwner{symbol:routine,position:1,context:4,
            statement:Some("0".into()),expression:Some("0".into()),nested:None},
        from_exec:false,expression_path:"0".into(),
    };
    for (path,parent,edge,category,kind,count) in [
        ("0",None,"embedded",0,0,1),
        ("0.0",Some("0"),"item",1,stmt_kind(2),3),
        ("0.0.0",Some("0.0"),"primary",0,0,0),
        ("0.0.1",Some("0.0"),"declarations",0,0,0),
        ("0.0.2",Some("0.0"),"block",0,0,2),
        ("0.0.2.0",Some("0.0.2"),"item",1,stmt_kind(3),4),
        ("0.0.2.0.0",Some("0.0.2.0"),"primary",0,0,1),
        ("0.0.2.0.0.0",Some("0.0.2.0.0"),"item",2,expr_kind(1),1),
        ("0.0.2.0.0.0.0",Some("0.0.2.0.0.0"),"operands",0,0,0),
        ("0.0.2.0.1",Some("0.0.2.0"),"declarations",0,0,0),
        ("0.0.2.0.2",Some("0.0.2.0"),"then",0,0,0),
        ("0.0.2.0.3",Some("0.0.2.0"),"else",0,0,0),
        ("0.0.2.1",Some("0.0.2"),"item",1,stmt_kind(12),2),
        ("0.0.2.1.0",Some("0.0.2.1"),"primary",0,0,0),
        ("0.0.2.1.1",Some("0.0.2.1"),"declarations",0,0,0),
    ] {
        il.native_supplemental_block_nodes.push(NativeSupplementalBlockNode {
            owner:owner.clone(),path:path.into(),parent:parent.map(str::to_owned),
            edge:edge.into(),category,kind,count,ty:None,symbol:None,operator:None,
            integer:None,flags:0,
        });
    }
    validate_supplemental_block_graph(&il).unwrap();
    for i in 0..il.native_supplemental_block_nodes.len() {
        let mut wrong=il.clone();
        wrong.native_supplemental_block_nodes.remove(i);
        assert!(validate_supplemental_block_graph(&wrong).is_err(),"missing node {i}");
    }
    let mut wrong=il.clone();
    wrong.native_supplemental_block_nodes[11].edge="body".into();
    assert!(validate_supplemental_block_graph(&wrong).is_err());
    let mut wrong=il.clone();
    wrong.native_supplemental_block_nodes[0].owner.initializer.position=4;
    assert!(validate_supplemental_block_graph(&wrong).is_err());
    let mut wrong=il.clone();
    wrong.control_expressions.clear();
    assert!(validate_supplemental_block_graph(&wrong).is_err());
    #[cfg(feature="codegen")]
    {
        assert!(codegen::lower_edg(&il).is_err());
        assert!(codegen::emit_edg_object(&il).is_err());
    }
}
