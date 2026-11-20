//! Diagnostic EDG-owned startup roots and their **immediate** native call edges.
//!
//! No invocation order is inferred from routine names, callback order, or
//! source-file iteration.  In the common non-separate EDG configuration a
//! generated `__sti__` routine may own initialization statements after
//! EDG has drained the original file-scope dynamic-initializer list.
//!
//! These records do not prove a complete startup/shutdown schedule, the
//! effects of transitive calls, constructor/destructor ABI identities,
//! registration order, or ELF .init_array admission.

use std::collections::{HashMap, HashSet, VecDeque};
use crate::{CallTargetForm, FileScopeInitSchedule, Il, SymbolId, TypeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupRootSource {
    /// Native EDG header's ordered list; the ordinal belongs only to this list.
    SeparateNativeScheduler,
    /// Native address from EDG's lowering-generated static __link object.
    RuntimeLink,
    /// Defined compiler-generated lowered routine; no ELF order is known.
    LoweredGenerated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupRootEvidence {
    pub routine: SymbolId,
    pub function: String,
    pub source: StartupRootSource,
    pub ordinal: Option<u32>,
    pub priority: Option<u32>,
    pub link_variable: Option<SymbolId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupCallEvidence {
    pub root: SymbolId,
    /// Only the immediate call in the root's own body is represented.
    pub function: String,
    pub statement_path: String,
    pub expression_path: String,
    pub form: CallTargetForm,
    pub callee_symbol: Option<SymbolId>,
    pub callee_type: Option<TypeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupEvidence {
    /// Always diagnostic. Even Some { complete:true } applies only to the
    /// optional separate EDG list, not the whole C++ runtime lifecycle.
    pub scheduler: Option<FileScopeInitSchedule>,
    pub roots: Vec<StartupRootEvidence>,
    pub immediate_calls: Vec<StartupCallEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartupEvidenceError {
    MissingRoutine(SymbolId),
    MissingDefinition(SymbolId),
    NotGenerated(SymbolId),
    MissingLinkVariable(SymbolId),
    ContradictoryScheduler,
    DuplicateCallTarget,
    MissingCallTarget,
    OrphanedCallTarget,
    /// A native routine symbol must have one unique function-body identity.
    DuplicateRoutineIdentity(SymbolId),
    /// Different native routine symbols cannot share one body key when
    /// attributing startup calls: this would silently select one root.
    AmbiguousRootFunction(String),
    /// An ordered native scheduler roster requires the mode witness.
    MissingScheduler,
    /// Native ordered roster must start at zero and remain gap-free.
    InvalidSchedulerOrder(u32),
    /// A reachable function body cannot be attributed to two native routines.
    AmbiguousReachableFunction(String),
    /// The diagnostic has a finite analysis budget; never silently truncate.
    ReachabilityLimit,
}

/// Build a *diagnostic* candidate-root / direct-call view from source-owned
/// native IL identities. Does not authorize object construction or codegen.
pub fn collect_startup_evidence(il: &Il) -> Result<StartupEvidence, StartupEvidenceError> {
    let mut roots = Vec::new();
    if !il.file_scope_init_routines.is_empty() {
        match il.file_scope_init_schedule {
            None => return Err(StartupEvidenceError::MissingScheduler),
            Some(FileScopeInitSchedule{available:false,..}) =>
                return Err(StartupEvidenceError::ContradictoryScheduler),
            Some(_) => {}
        }
    }
    let mut next_ordinal = 0u32;
    for entry in &il.file_scope_init_routines {
        if entry.position != next_ordinal {
            return Err(StartupEvidenceError::InvalidSchedulerOrder(entry.position));
        }
        next_ordinal = next_ordinal.checked_add(1)
            .ok_or(StartupEvidenceError::InvalidSchedulerOrder(entry.position))?;
        roots.push(StartupRootEvidence {
            routine: entry.routine, function: String::new(),
            source: StartupRootSource::SeparateNativeScheduler,
            ordinal: Some(entry.position), priority: None,
            link_variable: None,
        });
    }
    for entry in &il.file_scope_runtime_links {
        if !il.symbols.iter().any(|s| s.id == entry.link_variable && s.kind == 2) {
            return Err(StartupEvidenceError::MissingLinkVariable(entry.link_variable));
        }
        roots.push(StartupRootEvidence {
            routine: entry.startup_routine, function: String::new(),
            source: StartupRootSource::RuntimeLink,
            ordinal: None, priority: None,
            link_variable: Some(entry.link_variable),
        });
    }
    for entry in &il.file_scope_lowered_startup_routines {
        roots.push(StartupRootEvidence {
            routine: entry.routine, function: String::new(),
            source: StartupRootSource::LoweredGenerated,
            ordinal: None, priority: Some(entry.priority),
            link_variable: None,
        });
    }
    // The wire collector already checks identity uniqueness. Recheck at the
    // diagnostic API boundary because callers can build/modify Il directly.
    // Silently overwriting a duplicate lets a different function body claim
    // a native initializer routine's identity.
    let mut by_id = HashMap::new();
    for entry in &il.routine_identities {
        if by_id.insert(entry.routine, entry.function.as_str()).is_some() {
            return Err(StartupEvidenceError::DuplicateRoutineIdentity(entry.routine));
        }
    }
    for root in &mut roots {
        if !il.symbols.iter().any(|s| s.id == root.routine && s.kind == 1) {
            return Err(StartupEvidenceError::MissingRoutine(root.routine));
        }
        let Some(function) = by_id.get(&root.routine) else {
            return Err(StartupEvidenceError::MissingRoutine(root.routine));
        };
        let Some(attributes) = il.routine_attributes.iter()
            .find(|r| r.routine == root.routine) else {
                return Err(StartupEvidenceError::MissingDefinition(root.routine));
            };
        if !attributes.defined ||
            !il.entities.iter().any(|e| matches!(e, crate::Entity::Routine {
                name, has_definition:true,
            } if name.as_str() == *function))
        {
            return Err(StartupEvidenceError::MissingDefinition(root.routine));
        }
        if root.source == StartupRootSource::LoweredGenerated && !attributes.compiler_generated {
            return Err(StartupEvidenceError::NotGenerated(root.routine));
        }
        root.function = (*function).to_owned();
    }

    // The same native routine can have more than one independently emitted
    // root witness. Do not pretend this establishes two runtime invocations.
    // Check ALL body identities, not only those on the startup-root roster:
    // calls in an unrelated routine with a colliding function key would
    // otherwise be misattributed to a generated startup body.
    let mut unique_body_owner: HashMap<&str, Option<SymbolId>> = HashMap::new();
    for entry in &il.routine_identities {
        let owner = unique_body_owner.entry(entry.function.as_str())
            .or_insert(Some(entry.routine));
        if *owner != Some(entry.routine) { *owner = None; }
    }
    let mut root_by_function: HashMap<&str, SymbolId> = HashMap::new();
    for root in &roots {
        if unique_body_owner.get(root.function.as_str()) != Some(&Some(root.routine)) {
            return Err(StartupEvidenceError::AmbiguousRootFunction(root.function.clone()));
        }
        // Repeated witnesses for the same exact routine remain valid.
        root_by_function.insert(root.function.as_str(),root.routine);
    }

    let mut targets = HashMap::new();
    for target in &il.call_targets {
        if !root_by_function.contains_key(target.function.as_str()) { continue; }
        let key = (target.function.as_str(), target.statement_path.as_str(),
                   target.path.as_str());
        if targets.insert(key, target).is_some() {
            return Err(StartupEvidenceError::DuplicateCallTarget);
        }
    }
    let mut consumed = HashSet::new();
    let mut immediate_calls = Vec::new();
    for site in &il.calls {
        let Some(&root) = root_by_function.get(site.function.as_str()) else { continue; };
        let key = (site.function.as_str(), site.statement_path.as_str(),
                   site.path.as_str());
        let Some(target) = targets.get(&key) else {
            return Err(StartupEvidenceError::MissingCallTarget);
        };
        if !consumed.insert(key) { return Err(StartupEvidenceError::DuplicateCallTarget); }
        immediate_calls.push(StartupCallEvidence {
            root, function: site.function.clone(),
            statement_path: site.statement_path.clone(),
            expression_path: site.path.clone(),
            form: target.form,
            callee_symbol: target.callee_symbol,
            callee_type: target.callee_type,
        });
    }
    if targets.len() != consumed.len() {
        return Err(StartupEvidenceError::OrphanedCallTarget);
    }
    Ok(StartupEvidence {
        scheduler: il.file_scope_init_schedule,
        roots,
        immediate_calls,
    })
}


/// A single EDG-owned call occurrence reached from a source-identified
/// startup root through locally defined routine bodies. The same occurrence
/// may appear under multiple roots; no actual invocation count or order is
/// inferred from this diagnostic graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupReachableCall {
    pub root: SymbolId,
    pub caller: SymbolId,
    pub function: String,
    pub statement_path: String,
    pub expression_path: String,
    pub form: CallTargetForm,
    pub callee_symbol: Option<SymbolId>,
    pub callee_type: Option<TypeId>,
    /// True only when the exact native callee SymbolId identifies a defined
    /// local routine body. Does not prove C++ calling ABI or object lifetime.
    pub local_body: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupCallTrace {
    pub edges: Vec<StartupReachableCall>,
    /// Indirect call without a proven local callee; may hide side effects.
    pub unresolved_indirect: usize,
    /// Direct known callee without a defined body in this translation unit.
    /// It can execute external constructors, destructors or runtime helpers.
    pub external_calls: usize,
    /// A body reached again from the same root (shared helper or recursion).
    /// A revisitation is not an executable cycle or ordering proof.
    pub revisited_local_bodies: usize,
}

// Hard caps protect diagnostics from pathological or corrupt IL snapshots.
// Exceeding either budget is an error, never a seemingly complete trace.
const MAX_TRACE_BODIES_PER_ROOT: usize = 4096;
const MAX_TRACE_EDGES: usize = 100_000;

/// Trace source-owned *syntactic* direct call chains from all startup roots.
/// This adds transitive helper visibility beyond `immediate_calls` without
/// attempting to execute any C++ initialization, reconstruct call order,
/// follow indirect functions, or infer destructor registration.
pub fn trace_startup_calls(
    il: &Il, proof: &StartupEvidence,
) -> Result<StartupCallTrace, StartupEvidenceError> {
    let mut by_symbol = HashMap::new();
    let mut by_function: HashMap<&str, Option<SymbolId>> = HashMap::new();
    for id in &il.routine_identities {
        if by_symbol.insert(id.routine, id.function.as_str()).is_some() {
            return Err(StartupEvidenceError::DuplicateRoutineIdentity(id.routine));
        }
        let previous = by_function.entry(id.function.as_str())
            .or_insert(Some(id.routine));
        if *previous != Some(id.routine) { *previous = None; }
    }

    let defined: HashSet<SymbolId> = il.routine_attributes.iter()
        .filter(|attr| attr.defined)
        .map(|attr| attr.routine).collect();
    let routine_symbols: HashSet<SymbolId> = il.symbols.iter()
        .filter(|sym| sym.kind == 1).map(|sym| sym.id).collect();
    // A routine attribute's 'defined' flag is not itself proof that EDG
    // emitted a callable body. Event 1 independently records the body.
    let defined_body_functions: HashSet<&str> = il.entities.iter()
        .filter_map(|entity| match entity {
            crate::Entity::Routine { name, has_definition: true } => Some(name.as_str()),
            _ => None,
        }).collect();
    // Build a single index over all native calls and their independent
    // target-evidence events. Do not repeatedly scan large <iostream> IL.
    let mut sites: HashMap<&str,Vec<&crate::CallSite>> = HashMap::new();
    for site in &il.calls {
        sites.entry(site.function.as_str()).or_default().push(site);
    }
    let mut targets: HashMap<&str,Vec<&crate::CallTargetProvenance>> = HashMap::new();
    for target in &il.call_targets {
        targets.entry(target.function.as_str()).or_default().push(target);
    }

    let mut edges = Vec::new();
    let mut unresolved_indirect = 0;
    let mut external_calls = 0;
    let mut revisited_local_bodies = 0;
    let mut seen_roots = HashSet::new();
    for root in &proof.roots {
        // One native root can have multiple scheduling/link witnesses.
        if !seen_roots.insert(root.routine) { continue; }
        if by_symbol.get(&root.routine) != Some(&root.function.as_str()) ||
           !defined.contains(&root.routine) ||
           !routine_symbols.contains(&root.routine) ||
           !defined_body_functions.contains(root.function.as_str()) {
            return Err(StartupEvidenceError::MissingDefinition(root.routine));
        }
        let mut queue = VecDeque::from([root.routine]);
        let mut visited = HashSet::new();
        while let Some(caller) = queue.pop_front() {
            if !visited.insert(caller) {
                revisited_local_bodies += 1;
                continue;
            }
            if visited.len() > MAX_TRACE_BODIES_PER_ROOT {
                return Err(StartupEvidenceError::ReachabilityLimit);
            }
            let function = by_symbol[&caller];
            if by_function.get(function) != Some(&Some(caller)) {
                return Err(StartupEvidenceError::AmbiguousReachableFunction(
                    function.to_owned()));
            }
            let mut matching_targets = HashMap::new();
            for target in targets.get(function).into_iter().flatten() {
                let key=(target.statement_path.as_str(),target.path.as_str());
                if matching_targets.insert(key,*target).is_some() {
                    return Err(StartupEvidenceError::DuplicateCallTarget);
                }
            }
            let mut consumed = HashSet::new();
            for site in sites.get(function).into_iter().flatten() {
                let key=(site.statement_path.as_str(),site.path.as_str());
                let Some(target) = matching_targets.get(&key) else {
                    return Err(StartupEvidenceError::MissingCallTarget);
                };
                if !consumed.insert(key) {
                    return Err(StartupEvidenceError::DuplicateCallTarget);
                }
                if edges.len() >= MAX_TRACE_EDGES {
                    return Err(StartupEvidenceError::ReachabilityLimit);
                }
                let direct = matches!(target.form,CallTargetForm::DirectRoutine |
                                                CallTargetForm::ConstantRoutineAddress);
                let local_body = direct && target.callee_symbol.is_some_and(|id|
                    defined.contains(&id) &&
                    routine_symbols.contains(&id) &&
                    by_symbol.get(&id).is_some_and(|function|
                        defined_body_functions.contains(*function)));
                if local_body {
                    queue.push_back(target.callee_symbol.expect("checked above"));
                } else if !direct || target.callee_symbol.is_none() {
                    unresolved_indirect += 1;
                } else {
                    external_calls += 1;
                }
                edges.push(StartupReachableCall {
                    root:root.routine,caller,function:function.into(),
                    statement_path:site.statement_path.clone(),
                    expression_path:site.path.clone(),
                    form:target.form,callee_symbol:target.callee_symbol,
                    callee_type:target.callee_type,local_body,
                });
            }
            if matching_targets.len() != consumed.len() {
                return Err(StartupEvidenceError::OrphanedCallTarget);
            }
        }
    }
    Ok(StartupCallTrace {
        edges,unresolved_indirect,external_calls,revisited_local_bodies,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CallSite, CallTargetProvenance, Entity, FileScopeInitRoutine,
        FileScopeLoweredStartupRoutine, FileScopeRuntimeLink, RoutineAttributes,
        RoutineIdentity, Symbol};

    fn generated(il: &mut Il, id: u32, name: &str, compiler_generated: bool) {
        let routine = SymbolId(id);
        il.symbols.push(Symbol { id:routine, kind:1, name:name.into() });
        il.routine_identities.push(RoutineIdentity {
            routine, function:name.into(),
        });
        il.entities.push(Entity::Routine {
            name:name.into(), has_definition:true,
        });
        il.routine_attributes.push(RoutineAttributes {
            routine, special_kind:0, is_virtual:false, pure_virtual:false,
            is_override:false, is_final:false, overrides_base_member:false,
            is_inline:false, is_constexpr:false, defined:true,
            explicit_constructor:false, has_this_parameter:false,
            compiler_generated,
        });
    }


    fn add_call(il:&mut Il, function:&str, statement:&str,
                form:CallTargetForm, callee:Option<SymbolId>) {
        il.calls.push(CallSite {
            function:function.into(),statement_path:statement.into(),
            path:"0".into(),first_argument:1,
        });
        il.call_targets.push(CallTargetProvenance {
            function:function.into(),statement_path:statement.into(),
            path:"0".into(),form,callee_symbol:callee,callee_type:None,
        });
    }

    #[test]
    fn trace_follows_owning_startup_helper_calls_without_inventing_elf_order() {
        let mut il=Il::default();
        generated(&mut il,42,"__sti__root",true);
        generated(&mut il,43,"local_helper",false);
        generated(&mut il,44,"local_ctor",false);
        il.file_scope_lowered_startup_routines.push(
            FileScopeLoweredStartupRoutine{routine:SymbolId(42),priority:200});
        // Independent link witness for the same exact generated routine.
        il.symbols.push(Symbol{id:SymbolId(17),kind:2,name:"link".into()});
        il.file_scope_runtime_links.push(FileScopeRuntimeLink{
            link_variable:SymbolId(17),startup_routine:SymbolId(42),
        });
        add_call(&mut il,"__sti__root","0",CallTargetForm::DirectRoutine,Some(SymbolId(43)));
        add_call(&mut il,"local_helper","0",CallTargetForm::DirectRoutine,Some(SymbolId(44)));
        add_call(&mut il,"local_helper","1",CallTargetForm::IndirectUnresolved,None);
        add_call(&mut il,"local_ctor","0",CallTargetForm::DirectRoutine,Some(SymbolId(43)));
        add_call(&mut il,"local_ctor","1",CallTargetForm::DirectRoutine,Some(SymbolId(99)));

        let proof=collect_startup_evidence(&il).unwrap();
        assert_eq!(proof.roots.len(),2);
        assert_eq!(proof.immediate_calls.len(),1);
        let trace=trace_startup_calls(&il,&proof).unwrap();
        assert_eq!(trace.edges.len(),5); // one root, no duplicate traversal
        assert_eq!(trace.unresolved_indirect,1);
        assert_eq!(trace.external_calls,1);
        assert_eq!(trace.revisited_local_bodies,1);
        assert_eq!(trace.edges.iter().filter(|e|e.local_body).count(),3);
        assert!(trace.edges.iter().any(|edge| edge.caller==SymbolId(43) &&
            edge.callee_symbol==Some(SymbolId(44))));
        assert!(trace.edges.iter().all(|e|e.root==SymbolId(42)));
    }

    #[test]
    fn trace_does_not_invent_a_local_body_from_routine_attributes() {
        let mut il=Il::default();
        generated(&mut il,42,"startup",true);
        generated(&mut il,43,"helper",false);
        il.file_scope_lowered_startup_routines.push(
            FileScopeLoweredStartupRoutine{routine:SymbolId(42),priority:0});
        add_call(&mut il,"startup","0",CallTargetForm::DirectRoutine,Some(SymbolId(43)));
        add_call(&mut il,"helper","0",CallTargetForm::DirectRoutine,Some(SymbolId(99)));
        // The routine declaration still claims 'defined', but the independent
        // native event-1 body witness says no function body was emitted.
        if let Entity::Routine { has_definition, .. } = &mut il.entities[1] {
            *has_definition = false;
        }
        let proof=collect_startup_evidence(&il).unwrap();
        let trace=trace_startup_calls(&il,&proof).unwrap();
        assert_eq!(trace.edges.len(),1);
        assert_eq!(trace.external_calls,1);
        assert_eq!(trace.unresolved_indirect,0);
        assert!(!trace.edges[0].local_body);
        assert_eq!(trace.edges[0].callee_symbol,Some(SymbolId(43)));
    }

    #[test]
    fn trace_rejects_root_missing_its_native_body_witness() {
        let mut il=Il::default();
        generated(&mut il,42,"startup",true);
        il.file_scope_lowered_startup_routines.push(
            FileScopeLoweredStartupRoutine{routine:SymbolId(42),priority:0});
        let proof=collect_startup_evidence(&il).unwrap();
        if let Entity::Routine { has_definition, .. } = &mut il.entities[0] {
            *has_definition = false;
        }
        assert_eq!(trace_startup_calls(&il,&proof),
                   Err(StartupEvidenceError::MissingDefinition(SymbolId(42))));
    }

    #[test]
    fn trace_rejects_colliding_nonroot_helper_bodies() {
        let mut il=Il::default();
        generated(&mut il,42,"__sti__root",true);
        generated(&mut il,43,"helper",false);
        generated(&mut il,44,"helper",false);
        il.file_scope_lowered_startup_routines.push(
            FileScopeLoweredStartupRoutine{routine:SymbolId(42),priority:0});
        add_call(&mut il,"__sti__root","0",CallTargetForm::DirectRoutine,Some(SymbolId(43)));
        let proof=collect_startup_evidence(&il).unwrap();
        assert_eq!(trace_startup_calls(&il,&proof),
                   Err(StartupEvidenceError::AmbiguousReachableFunction("helper".into())));
    }

    #[test]
    fn trace_rejects_missing_and_orphaned_helper_target_events() {
        let mut il=Il::default();
        generated(&mut il,42,"__sti__root",true);
        generated(&mut il,43,"helper",false);
        il.file_scope_lowered_startup_routines.push(
            FileScopeLoweredStartupRoutine{routine:SymbolId(42),priority:0});
        add_call(&mut il,"__sti__root","0",CallTargetForm::DirectRoutine,Some(SymbolId(43)));
        il.calls.push(CallSite{
            function:"helper".into(),statement_path:"1".into(),
            path:"0".into(),first_argument:1,
        });
        let proof=collect_startup_evidence(&il).unwrap();
        assert_eq!(trace_startup_calls(&il,&proof),
                   Err(StartupEvidenceError::MissingCallTarget));
        il.calls.pop();
        il.call_targets.push(CallTargetProvenance {
            function:"helper".into(),statement_path:"1".into(),path:"0".into(),
            form:CallTargetForm::IndirectUnresolved,callee_type:None,callee_symbol:None,
        });
        assert_eq!(trace_startup_calls(&il,&proof),
                   Err(StartupEvidenceError::OrphanedCallTarget));
    }

    #[test]
    fn absent_native_schedule_is_not_an_executable_empty_plan() {
        let il = Il::default();
        let report = collect_startup_evidence(&il).unwrap();
        assert!(report.scheduler.is_none());
        assert!(report.roots.is_empty());
        assert!(report.immediate_calls.is_empty());
    }

    #[test]
    fn lowered_generated_root_preserves_direct_and_unknown_call_provenance() {
        let mut il = Il::default();
        generated(&mut il, 42, "__sti__example", true);
        il.file_scope_lowered_startup_routines.push(
            FileScopeLoweredStartupRoutine {routine:SymbolId(42),priority:200});
        for (statement,form,callee) in [
            ("0",CallTargetForm::DirectRoutine,Some(SymbolId(99))),
            ("1",CallTargetForm::IndirectUnresolved,None),
        ] {
            il.calls.push(CallSite {
                function:"__sti__example".into(),
                statement_path:statement.into(), path:"0".into(),
                first_argument:1,
            });
            il.call_targets.push(CallTargetProvenance {
                function:"__sti__example".into(),
                statement_path:statement.into(), path:"0".into(),
                form, callee_type:None, callee_symbol:callee,
            });
        }
        let report = collect_startup_evidence(&il).unwrap();
        assert_eq!(report.roots.len(),1);
        assert_eq!(report.roots[0].priority,Some(200));
        assert_eq!(report.roots[0].ordinal,None);
        assert_eq!(report.immediate_calls.len(),2);
        assert_eq!(report.immediate_calls[0].callee_symbol,Some(SymbolId(99)));
        assert_eq!(report.immediate_calls[1].form,CallTargetForm::IndirectUnresolved);
        assert_eq!(report.immediate_calls[1].callee_symbol,None);
    }

    #[test]
    fn separate_order_and_link_root_do_not_create_invented_cross_scope_order() {
        let mut il = Il::default();
        generated(&mut il,42,"startup-a",true);
        generated(&mut il,43,"startup-b",true);
        il.file_scope_init_schedule=Some(FileScopeInitSchedule {
            available:true, complete:true,
        });
        il.file_scope_init_routines.push(FileScopeInitRoutine {
            position:0,routine:SymbolId(43),
        });
        il.file_scope_init_routines.push(FileScopeInitRoutine {
            position:1,routine:SymbolId(42),
        });
        il.symbols.push(Symbol {
            id:SymbolId(17),kind:2,name:"link".into(),
        });
        il.file_scope_runtime_links.push(FileScopeRuntimeLink {
            link_variable:SymbolId(17),startup_routine:SymbolId(42),
        });
        let report=collect_startup_evidence(&il).unwrap();
        assert_eq!(report.roots.len(),3);
        assert_eq!(report.roots[0].routine,SymbolId(43));
        assert_eq!(report.roots[0].ordinal,Some(0));
        assert_eq!(report.roots[1].ordinal,Some(1));
        assert_eq!(report.roots[2].source,StartupRootSource::RuntimeLink);
        assert_eq!(report.roots[2].ordinal,None);
    }

    #[test]
    fn missing_or_user_routine_cannot_masquerade_as_generated_startup() {
        let mut il=Il::default();
        il.file_scope_lowered_startup_routines.push(
            FileScopeLoweredStartupRoutine {routine:SymbolId(42),priority:0});
        assert_eq!(collect_startup_evidence(&il),
                   Err(StartupEvidenceError::MissingRoutine(SymbolId(42))));
        generated(&mut il,42,"user_code",false);
        assert_eq!(collect_startup_evidence(&il),
                   Err(StartupEvidenceError::NotGenerated(SymbolId(42))));
    }


    #[test]
    fn reject_startup_roster_without_native_scheduler_or_gap_free_order() {
        let mut il=Il::default();
        generated(&mut il,42,"__sti__example",true);
        il.file_scope_init_routines.push(FileScopeInitRoutine {
            position:0,routine:SymbolId(42),
        });
        assert_eq!(collect_startup_evidence(&il),
                   Err(StartupEvidenceError::MissingScheduler));
        il.file_scope_init_schedule=Some(FileScopeInitSchedule {
            available:true,complete:true,
        });
        il.file_scope_init_routines[0].position=1;
        assert_eq!(collect_startup_evidence(&il),
                   Err(StartupEvidenceError::InvalidSchedulerOrder(1)));
        il.file_scope_init_routines[0].position=0;
        il.file_scope_init_routines.push(FileScopeInitRoutine {
            position:2,routine:SymbolId(42),
        });
        assert_eq!(collect_startup_evidence(&il),
                   Err(StartupEvidenceError::InvalidSchedulerOrder(2)));
    }

    #[test]
    fn reject_duplicate_native_routine_identity_without_overwriting_its_body() {
        let mut il=Il::default();
        generated(&mut il,42,"__sti__example",true);
        il.file_scope_lowered_startup_routines.push(
            FileScopeLoweredStartupRoutine {routine:SymbolId(42),priority:0});
        il.routine_identities.push(RoutineIdentity {
            routine:SymbolId(42),function:"unrelated-body".into(),
        });
        assert_eq!(collect_startup_evidence(&il),
                   Err(StartupEvidenceError::DuplicateRoutineIdentity(SymbolId(42))));
    }

    #[test]
    fn reject_nonstartup_routine_aliasing_startup_function_body_key() {
        let mut il=Il::default();
        generated(&mut il,42,"__sti__shared",true);
        generated(&mut il,43,"__sti__shared",false);
        il.file_scope_lowered_startup_routines.push(
            FileScopeLoweredStartupRoutine {routine:SymbolId(42),priority:0});
        // Routine 43 is not a startup root. Its body must not be mistaken
        // for routine 42's body merely because it has the same string key.
        assert_eq!(collect_startup_evidence(&il),
                   Err(StartupEvidenceError::AmbiguousRootFunction("__sti__shared".into())));
    }

    #[test]
    fn reject_different_startup_routines_with_same_function_body_key() {
        let mut il=Il::default();
        generated(&mut il,42,"__sti__shared",true);
        generated(&mut il,43,"__sti__shared",true);
        il.file_scope_lowered_startup_routines.extend([
            FileScopeLoweredStartupRoutine {routine:SymbolId(42),priority:0},
            FileScopeLoweredStartupRoutine {routine:SymbolId(43),priority:0},
        ]);
        assert_eq!(collect_startup_evidence(&il),
                   Err(StartupEvidenceError::AmbiguousRootFunction("__sti__shared".into())));
    }

    #[test]
    fn missing_call_target_and_contradictory_scheduler_fail_closed() {
        let mut il=Il::default();
        generated(&mut il,42,"__sti__example",true);
        il.file_scope_lowered_startup_routines.push(
            FileScopeLoweredStartupRoutine {routine:SymbolId(42),priority:0});
        il.calls.push(CallSite {
            function:"__sti__example".into(),statement_path:"0".into(),
            path:"0".into(),first_argument:1,
        });
        assert_eq!(collect_startup_evidence(&il),
                   Err(StartupEvidenceError::MissingCallTarget));
        il.file_scope_init_schedule=Some(FileScopeInitSchedule {
            available:false,complete:false,
        });
        il.file_scope_init_routines.push(FileScopeInitRoutine {
            position:0,routine:SymbolId(42),
        });
        assert_eq!(collect_startup_evidence(&il),
                   Err(StartupEvidenceError::ContradictoryScheduler));
    }
}
