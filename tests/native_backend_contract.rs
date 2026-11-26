//! Checks that the Rust bridge uses EDG's public back-end callback surface.
//! These are source-level regression guards; native and differential tests
//! must still run using the version-pinned EDG archive on compiler-ci.

const BRIDGE: &str = include_str!("../cpp/bridge.cpp");

#[test]
fn entry_point_consumes_normal_post_lowering_edg_il() {
    assert!(BRIDGE.contains("void back_end(void)"));
    assert!(BRIDGE.contains("collect_scope(il_header.primary_scope, \"\");"));
    assert!(BRIDGE.contains("il_header.file_scope_dynamic_init_routines"));
    assert!(BRIDGE.contains("#if IL_SHOULD_BE_WRITTEN_TO_FILE"));
    assert!(BRIDGE.contains("if (!skip_il_read)"));
    assert!(BRIDGE.contains("il_read(f_il_output)"));
    assert!(BRIDGE.contains("primary_source_file_name = il_header.primary_source_file->file_name;"));
    assert!(BRIDGE.contains("if (il_header.primary_scope == nullptr)"));
}

#[test]
fn bridge_does_not_provide_edg_lowering_observers() {
    for forbidden in [
        "edgcpp_capture_",
        "prelower_virtual_calls",
        "postlower_virtual_calls",
        "native_virtual_entry_selections",
        "native_virtual_receiver_pairs",
        "callee_contains_exact_native_padd",
        "subtree_contains_unique_native_node",
    ] {
        assert!(
            !BRIDGE.contains(forbidden),
            "EDG native bridge reintroduced a private lowering dependency: {forbidden}"
        );
    }
}

#[test]
fn native_operand_lists_are_selected_in_all_expression_contexts() {
    // These are distinct linked-list owners in upstream traverse_expr,
    // not ordinary operation operands. A count of zero from the fallback
    // would silently hide all descendants from Rust.
    let selector=BRIDGE.split("static expression_children children_of(")
        .nth(1).expect("native child selector")
        .split("static unsigned call_argument_offset(").next().unwrap();
    for (kind,member) in [
        ("enk_builtin_operation","builtin_operation.operands"),
        ("enk_builtin_choose_expr","builtin_choose_expr.operands"),
        ("enk_fold","fold.operands"),
    ] {
        let branch=format!("if (node->kind == {kind})");
        let tail=selector.split(&branch).nth(1).expect("native kind selected");
        assert!(tail.trim_start().starts_with(
            &format!("return {{node->variant.{member}, true}};")),
            "{kind} must preserve its original linked operands");
    }
    assert!(BRIDGE.contains("const auto children = children_of(node);"));
    assert!(BRIDGE.contains("const auto children = children_of(expr);"));
}

#[test]
fn expression_owned_dynamic_initialization_keeps_native_edges() {
    let selector=BRIDGE.split("static expression_children dynamic_init_expression_children(")
        .nth(1).expect("dynamic initializer child selector")
        .split("static unsigned call_argument_offset(").next().unwrap();
    for edge in [
        "return {init->variant.expression, false};",
        "return {init->variant.constructor.args, true};",
        "return {init->variant.bitwise_copy.source, false};",
        "return dynamic_init_expression_children(node->variant.init.dynamic_init);",
        "return dynamic_init_expression_children(node->variant.initializer.dyn_init);",
    ] {
        assert!(selector.contains(edge), "missing original initializer edge: {edge}");
    }
}

#[test]
fn builtin_choose_expr_has_independent_native_arity_guard() {
    let emitter = BRIDGE.split("static void emit_expression_variant(")
        .nth(1).expect("native variant emitter")
        .split("static void emit_expr(").next().unwrap();
    let choose = emitter.split("case enk_builtin_choose_expr:")
        .nth(1).expect("choose expression")
        .split("case enk_type_operand:").next().unwrap();
    assert!(choose.contains("node->variant.builtin_choose_expr.operands"));
    assert!(choose.contains("third == nullptr || third->next != nullptr"));
    assert!(choose.find("report_incomplete_snapshot()").unwrap() <
            choose.find("variant = 2;").unwrap());
}

#[test]
fn supplemental_statement_expression_cannot_silently_lose_its_block() {
    let emitter=BRIDGE.split("static void emit_expression_variant(")
        .nth(1).expect("native variant emitter")
        .split("static void emit_expr(").next().unwrap();
    assert!(emitter.contains("context != 0 && node->kind == enk_statement"));
    assert!(emitter.contains("emit_supplemental_block(node->variant.statement"));
    assert!(BRIDGE.contains("RECORD_NATIVE_SUPPLEMENTAL_BLOCK_NODE"));
    assert!(emitter.contains("node->variant.c11_generic.result"));
    assert!(emitter.contains("child == result"));
    assert!(BRIDGE.contains("category == 9"));
}
