//! Checks that the Rust bridge uses EDG's public back-end callback surface.
//! These are source-level regression guards; native and differential tests
//! must still run using the version-pinned EDG archive on compiler-ci.

const BRIDGE: &str = include_str!("../cpp/bridge.cpp");

#[test]
fn entry_point_consumes_normal_post_lowering_edg_il() {
    assert!(BRIDGE.contains("void back_end(void)"));
    assert!(BRIDGE.contains("collect_scope(il_header.primary_scope, \"\", 0);"));
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
