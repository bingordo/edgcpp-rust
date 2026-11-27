//! Mutation tests for the checked source-owned retained EH view.
use edgcpp::{retained_eh::{checked_retained_eh_actions, RetainedEhKind}, Expression,
             Il, NativeRetainedEhConstruct, ControlExpression};

extern "C" {
    fn edgcpp_codegen_kind(category: u32, value: u32) -> u32;
}

fn raw(category: u32, semantic: u32) -> Option<u32> {
    (0..256).find(|&v| unsafe { edgcpp_codegen_kind(category, v) == semantic })
}

fn expr(path: &str, parent: Option<&str>, kind: u32) -> Expression {
    Expression {
        function: "f".into(), statement_path: "s".into(), path: path.into(),
        parent: parent.map(str::to_owned), kind, operator: None,
        type_kind: None, reference: None, type_size: None, type_alignment: None,
        constant_kind: None, constant_integer: None, related_type_role: None,
        related_type_kind: None, type_id: None, symbol_id: None,
    }
}

fn action(kind: u32, subkind: u32, role: u32) -> NativeRetainedEhConstruct {
    NativeRetainedEhConstruct {
        function: "f".into(), statement_path: "s".into(), role,
        path: "0".into(), expression_kind: kind, subkind,
    }
}

#[test]
fn no_eh_and_orphan_action() {
    let mut il = Il::default();
    assert!(checked_retained_eh_actions(&il).unwrap().is_empty());
    il.native_retained_eh_constructs.push(action(0, 0, 0));
    assert!(checked_retained_eh_actions(&il).is_err());
}

#[test]
fn independent_kind_coverage_and_control_identity() {
    // Portable EDG builds do not contain the retained-EH enum.
    let Some(kind) = raw(8, 1) else { return };
    let subkind = raw(7, 3).expect("native cleanup state");
    let mut il = Il::default();
    il.expressions.push(expr("0", None, kind));
    il.control_expressions.push(ControlExpression {
        function: "f".into(), statement_path: "s".into(), role: 2,
        path: "0".into(), parent: None, kind, operator: None,
        type_id: None, symbol_id: None,
    });
    assert!(checked_retained_eh_actions(&il).is_err());
    il.native_retained_eh_constructs.push(action(kind, subkind, 0));
    assert!(checked_retained_eh_actions(&il).is_err());
    il.native_retained_eh_constructs.push(action(kind, subkind, 2));
    assert_eq!(checked_retained_eh_actions(&il).unwrap().iter()
        .map(|a| a.kind).collect::<Vec<_>>(), vec![RetainedEhKind::CleanupState; 2]);
    il.native_retained_eh_constructs.push(action(kind, subkind, 2));
    assert!(checked_retained_eh_actions(&il).is_err());
    il.native_retained_eh_constructs.pop();
    il.native_retained_eh_constructs[0].subkind = u32::MAX;
    assert!(checked_retained_eh_actions(&il).is_err());
}

#[test]
fn internal_try_requires_both_ordered_children() {
    let Some(kind) = raw(8, 1) else { return };
    let subkind = raw(7, 12).expect("native internal try");
    let ordinary = raw(2, 1).expect("native constant");
    let mut il = Il::default();
    il.expressions.push(expr("0", None, kind));
    il.native_retained_eh_constructs.push(action(kind, subkind, 0));
    il.expressions.push(expr("0.0", Some("0"), ordinary));
    assert!(checked_retained_eh_actions(&il).is_err());
    il.expressions.push(expr("0.1", Some("0"), ordinary));
    assert_eq!(checked_retained_eh_actions(&il).unwrap()[0].kind, RetainedEhKind::InternalTry);
    il.expressions[2].path = "0.2".into();
    assert!(checked_retained_eh_actions(&il).is_err());
}

#[test]
fn catch_roster_preserves_order_and_rejects_missing_and_invalid_metadata() {
    use edgcpp::{NativeTryHandlerCount, Symbol, SymbolId, SymbolType, TypeDef, TypeId,
        retained_eh::{NativeCatchHandler, validate_native_catch_handlers}};
    let mut il = Il::default();
    il.native_try_handler_counts.push(NativeTryHandlerCount {
        function: "f".into(), statement_path: "s".into(), count: 2,
    });
    il.types.push(TypeDef { id: TypeId(1), kind: 0, related_role: None, related: None });
    for id in [SymbolId(1), SymbolId(2)] {
        il.symbols.push(Symbol { id, kind: 2, name: "v".into() });
        il.symbol_types.push(SymbolType { symbol: id, ty: TypeId(1) });
    }
    let typed = NativeCatchHandler {
        function: "f".into(), statement_path: "s".into(), position: 0,
        parameter: Some(SymbolId(1)), parameter_type: Some(TypeId(1)),
        typeinfo: Some(SymbolId(2)), has_initialization: true, function_try: false,
    };
    let all = NativeCatchHandler {
        position: 1, parameter: None, parameter_type: None, typeinfo: None,
        has_initialization: false, ..typed.clone()
    };
    assert!(validate_native_catch_handlers(&il).is_err());
    il.native_catch_handlers.push(typed.clone());
    assert!(validate_native_catch_handlers(&il).is_err());
    il.native_catch_handlers.push(all.clone());
    validate_native_catch_handlers(&il).unwrap();
    il.native_catch_handlers.push(all.clone());
    assert!(validate_native_catch_handlers(&il).is_err());
    il.native_catch_handlers.pop();
    il.native_catch_handlers[0].parameter_type = Some(TypeId(2));
    assert!(validate_native_catch_handlers(&il).is_err());
    il.native_catch_handlers[0] = typed.clone();
    il.native_catch_handlers[0].typeinfo = Some(SymbolId(99));
    assert!(validate_native_catch_handlers(&il).is_err());
    il.native_catch_handlers[0] = typed;
    il.native_catch_handlers[1] = NativeCatchHandler { position: 0, ..all };
    assert!(validate_native_catch_handlers(&il).is_err());
}

#[test]
fn cleanup_payload_is_required_and_routine_scoped() {
    use edgcpp::{RoutineIdentity, SymbolId,
        retained_eh::{CleanupReference, NativeEhCleanupState, validate_native_eh_cleanup_states}};
    let Some(kind) = raw(8, 1) else { return };
    let subkind = raw(7, 3).expect("native cleanup state");
    let mut il = Il::default();
    il.routine_identities.push(RoutineIdentity { routine: SymbolId(1), function: "f".into() });
    il.native_retained_eh_constructs.push(action(kind, subkind, 0));
    assert!(validate_native_eh_cleanup_states(&il).is_err());
    let state = NativeEhCleanupState {
        function: "f".into(), statement_path: "s".into(), role: 0, path: "0".into(),
        routine: SymbolId(1), reference: CleanupReference::DynamicInit(None),
    };
    il.native_eh_cleanup_states.push(state.clone());
    validate_native_eh_cleanup_states(&il).unwrap();
    // Non-null but not independently exported is not null or executable.
    il.native_eh_cleanup_states[0].reference = CleanupReference::DynamicInit(Some(42));
    validate_native_eh_cleanup_states(&il).unwrap();
    il.native_eh_cleanup_states[0].routine = SymbolId(2);
    assert!(validate_native_eh_cleanup_states(&il).is_err());
    il.native_eh_cleanup_states[0] = state.clone();
    il.native_eh_cleanup_states.push(state);
    assert!(validate_native_eh_cleanup_states(&il).is_err());
}

#[test]
fn action_targets_join_native_owners_not_lexical_paths() {
    use edgcpp::{RoutineIdentity, SymbolId, NativeTryHandlerCount,
        retained_eh::{NativeCatchHandler, NativeEhOwner, NativeEhActionTarget,
            validate_native_eh_action_targets}};
    let Some(kind)=raw(8,1) else {return};
    let subkind=raw(7,1).expect("caught object");
    let mut il=Il::default();
    il.routine_identities.push(RoutineIdentity {routine:SymbolId(1),function:"f".into()});
    il.native_try_handler_counts.push(NativeTryHandlerCount {
        function:"f".into(),statement_path:"try-owner".into(),count:1,
    });
    il.native_catch_handlers.push(NativeCatchHandler {
        function:"f".into(),statement_path:"try-owner".into(),position:0,
        parameter:None,parameter_type:None,typeinfo:None,has_initialization:false,function_try:false,
    });
    let owner=NativeEhOwner {function:"f".into(),statement_path:"try-owner".into(),
        routine:SymbolId(1),position:None,id:1,initialization:None};
    il.native_eh_owners.push(owner.clone());
    il.native_eh_owners.push(NativeEhOwner {position:Some(0),id:2,..owner});
    il.native_retained_eh_constructs.push(action(kind,subkind,0));
    assert!(validate_native_eh_action_targets(&il).is_err());
    let target=NativeEhActionTarget {function:"f".into(),statement_path:"s".into(),role:0,
        path:"0".into(),routine:SymbolId(1),target:2};
    il.native_eh_action_targets.push(target.clone());
    assert_eq!(validate_native_eh_action_targets(&il).unwrap().handlers,1);
    il.native_eh_action_targets[0].target=1;
    assert!(validate_native_eh_action_targets(&il).is_err());
    il.native_eh_action_targets[0].target=99;
    assert_eq!(validate_native_eh_action_targets(&il).unwrap().unresolved,1);
    il.native_eh_action_targets[0]=target.clone();
    il.native_eh_action_targets.push(target);
    assert!(validate_native_eh_action_targets(&il).is_err());
}

#[test]
fn ordered_catch_dispatch_binds_native_protected_and_handler_bodies() {
    use edgcpp::{RoutineIdentity,Symbol,SymbolId,SymbolType,TypeDef,TypeId,
        NativeTryHandlerCount,NativeStatementSiblingCount,Statement,
        retained_eh::{NativeCatchHandler,NativeEhOwner,CatchMatch,checked_catch_dispatch}};
    let mut il=Il::default();
    il.routine_identities.push(RoutineIdentity{routine:SymbolId(1),function:"f".into()});
    il.types.push(TypeDef{id:TypeId(1),kind:0,related_role:None,related:None});
    for id in [SymbolId(2),SymbolId(3)] {
        il.symbols.push(Symbol{id,kind:2,name:"v".into()});
        il.symbol_types.push(SymbolType{symbol:id,ty:TypeId(1)});
    }
    il.native_try_handler_counts.push(NativeTryHandlerCount{function:"f".into(),statement_path:"0".into(),count:2});
    il.statements.push(Statement{function:"f".into(),path:"0".into(),parent:None,kind:0});
    for edge in ["try","catch0","catch1"] {
        il.native_statement_sibling_counts.push(NativeStatementSiblingCount{
            function:"f".into(),parent:Some("0".into()),edge:edge.into(),count:1,
        });
        il.statements.push(Statement{
            function:"f".into(),path:format!("0.{edge}.0"),parent:Some("0".into()),kind:0,
        });
    }
    for (position,id,initialization) in [(None,1,None),(Some(0),2,Some(9)),(Some(1),3,None)] {
        il.native_eh_owners.push(NativeEhOwner{function:"f".into(),statement_path:"0".into(),
            routine:SymbolId(1),position,id,initialization});
    }
    let typed=NativeCatchHandler{function:"f".into(),statement_path:"0".into(),position:0,
        parameter:Some(SymbolId(2)),parameter_type:Some(TypeId(1)),typeinfo:Some(SymbolId(3)),
        has_initialization:true,function_try:false};
    let any=NativeCatchHandler{position:1,parameter:None,parameter_type:None,typeinfo:None,
        has_initialization:false,..typed.clone()};
    // Wire collection order is not the dispatch order.
    il.native_catch_handlers.extend([any,typed]);
    il.native_eh_owners.reverse();
    {
        let regions=checked_catch_dispatch(&il).unwrap();
        assert_eq!(regions.len(),1);
        let r=&regions[0];
        assert_eq!(r.protected_body.path,"0.try.0");
        assert_eq!(r.clauses[0].body.path,"0.catch0.0");
        assert_eq!(r.clauses[0].owner.initialization,Some(9));
        assert_eq!(r.clauses[0].matcher,CatchMatch::Typed{
            parameter:SymbolId(2),ty:TypeId(1),typeinfo:Some(SymbolId(3))});
        assert_eq!(r.clauses[1].matcher,CatchMatch::Any);
        assert_eq!(r.clauses[1].body.path,"0.catch1.0");
    }
    let mut bad=il.clone();
    bad.statements.pop();
    assert!(checked_catch_dispatch(&bad).is_err());
    let mut bad=il.clone();
    bad.native_statement_sibling_counts[1].count=0;
    assert!(checked_catch_dispatch(&bad).is_err());
    let mut bad=il.clone();
    bad.statements[2].parent=Some("other".into());
    assert!(checked_catch_dispatch(&bad).is_err());
    let mut bad=il.clone();
    bad.native_eh_owners.pop();
    assert!(checked_catch_dispatch(&bad).is_err());
}
