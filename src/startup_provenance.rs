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

use std::collections::{HashMap, HashSet};
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
}

/// Build a *diagnostic* candidate-root / direct-call view from source-owned
/// native IL identities. Does not authorize object construction or codegen.
pub fn collect_startup_evidence(il: &Il) -> Result<StartupEvidence, StartupEvidenceError> {
    let mut roots = Vec::new();
    if il.file_scope_init_schedule.is_some_and(|s| !s.available)
        && !il.file_scope_init_routines.is_empty()
    {
        return Err(StartupEvidenceError::ContradictoryScheduler);
    }
    for entry in &il.file_scope_init_routines {
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
    let by_id: HashMap<_, _> = il.routine_identities.iter()
        .map(|entry| (entry.routine, entry.function.as_str())).collect();
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
            } if name == function))
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
    let mut root_by_function: HashMap<&str, SymbolId> = HashMap::new();
    for root in &roots {
        root_by_function.entry(root.function.as_str()).or_insert(root.routine);
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
