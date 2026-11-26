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
