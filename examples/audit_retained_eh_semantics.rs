//! Audit the original EDG retained-EH expression and action kinds.
//!
//! Compile the exact C++ TU through the source-built custom EDG backend.
//! This proves semantic identities, not an executable C++ unwind ABI.
use edgcpp::compile;
use std::collections::HashSet;

extern "C" {
    fn edgcpp_codegen_kind(category: u32, raw: u32) -> u32;
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = vec!["edgcpfe".to_string()];
    args.extend(std::env::args().skip(1));
    if args.len() < 2 {
        return Err("usage: audit_retained_eh_semantics [EDG options] source.cpp".into());
    }
    let result = compile(&args).map_err(|e| format!("{e:?}"))?;
    if result.status != 0 {
        return Err(format!("EDG compilation failed: {}", result.status).into());
    }
    edgcpp::retained_eh::validate_native_catch_handlers(&result.il)
        .map_err(|e| format!("invalid native catch roster: {e:?}"))?;
    let handlers = &result.il.native_catch_handlers;
    let typed = handlers.iter().filter(|h| h.parameter.is_some()).count();
    let catch_all = handlers.iter().filter(|h| h.parameter.is_none()).count();
    let typed_with_rtti = handlers.iter()
        .filter(|h| h.parameter.is_some() && h.typeinfo.is_some()).count();
    println!("EDG_NATIVE_CATCH_METADATA typed={typed} catch_all={catch_all} typed_with_rtti={typed_with_rtti}");
    use edgcpp::retained_eh::CleanupReference;
    let owned: HashSet<_> = result.il.native_dynamic_init_identities.iter()
        .map(|d| (d.routine,d.id)).collect();
    let mut null=0;
    let mut joined=0;
    let mut unresolved=0;
    let mut regions=0;
    for state in &result.il.native_eh_cleanup_states {
        match state.reference {
            CleanupReference::DynamicInit(None)=>null+=1,
            CleanupReference::DynamicInit(Some(id)) if owned.contains(&(state.routine,id))=>joined+=1,
            CleanupReference::DynamicInit(Some(_))=>unresolved+=1,
            CleanupReference::Region(_)=>regions+=1,
        }
    }
    if null+joined+unresolved+regions==0 {
        return Err("original EH fixture has no cleanup payloads".into());
    }
    println!("EDG_NATIVE_EH_CLEANUP null={null} joined={joined} unresolved={unresolved} regions={regions}");
    let coverage=edgcpp::retained_eh::validate_native_eh_action_targets(&result.il)
        .map_err(|e| format!("invalid native EH action targets: {e:?}"))?;
    println!("EDG_NATIVE_EH_TARGETS handlers={} tries={} routines={} unresolved={}",
             coverage.handlers,coverage.tries,coverage.routines,coverage.unresolved);
    let dispatch=edgcpp::retained_eh::checked_catch_dispatch(&result.il)
        .map_err(|e|format!("invalid source catch dispatch: {e:?}"))?;
    let clauses=dispatch.iter().map(|r|r.clauses.len()).sum::<usize>();
    if dispatch.is_empty() || clauses!=handlers.len() {
        return Err("native try/catch body inventory incomplete".into());
    }
    println!("EDG_NATIVE_CATCH_DISPATCH regions={} clauses={clauses}",dispatch.len());
    let actions = &result.il.native_retained_eh_constructs;
    if actions.is_empty() {
        return Err("original EDG translation unit exported no retained EH actions".into());
    }
    let mut action_kinds = HashSet::new();
    let mut internal_try_count = 0;
    for action in actions {
        let expr_kind = unsafe { edgcpp_codegen_kind(8, action.expression_kind) };
        if expr_kind != 1 {
            return Err(format!("native EH action has wrong expression kind: {action:?}").into());
        }
        let normalized = unsafe { edgcpp_codegen_kind(7, action.subkind) };
        if !(1..=12).contains(&normalized) {
            return Err(format!("unknown source EH action kind: {action:?}").into());
        }
        action_kinds.insert(normalized);
        if normalized == 12 {
            internal_try_count += 1;
            // EDG's original internal_try has exactly two owned expression
            // children (normal work and exceptional cleanup), in that order.
            if action.role != 0 {
                return Err("internal_try control-expression role unsupported by audit".into());
            }
            let counts: Vec<_> = result.il.native_expression_child_counts.iter()
                .filter(|count| count.function == action.function &&
                    count.statement_path == action.statement_path &&
                    count.path == action.path)
                .collect();
            if counts.len() != 1 || counts[0].count != 2 {
                return Err(format!("native internal_try lacks 2 original child expressions: {action:?}").into());
            }
            for suffix in [".0", ".1"] {
                let child = format!("{}{}", action.path, suffix);
                if !result.il.expressions.iter().any(|e| {
                    e.function == action.function &&
                    e.statement_path == action.statement_path &&
                    e.parent.as_deref() == Some(action.path.as_str()) &&
                    e.path == child
                }) {
                    return Err(format!("native internal_try lacks original child {child}").into());
                }
            }
        }
    }
    // A real throw/catch+destructor fixture must carry actual source-owned
    // function prologue, cleanup progression, caught object and catch entry.
    for required in [1, 3, 5, 9] {
        if !action_kinds.contains(&required) {
            return Err(format!("native fixture lacks EH action semantic {required}").into());
        }
    }
    let mut ordered: Vec<_> = action_kinds.into_iter().collect();
    ordered.sort_unstable();
    println!("EDG_RETAINED_EH_SEMANTICS actions={} kinds={ordered:?} internal_try={} itanium_abi_admitted=false",
             actions.len(),internal_try_count);
    Ok(())
}
