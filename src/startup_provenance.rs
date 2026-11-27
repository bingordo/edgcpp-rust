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
    /// Two event-22 attribute witnesses for one native routine conflict.
    DuplicateRoutineAttributes(SymbolId),
    /// A trace may only consume evidence from this exact current IL snapshot.
    MismatchedEvidence,
    /// Different native routine symbols cannot share one body key when
    /// attributing startup calls: this would silently select one root.
    AmbiguousRootFunction(String),
    /// An ordered native scheduler roster requires the mode witness.
    MissingScheduler,
    /// Native ordered roster must start at zero and remain gap-free.
    InvalidSchedulerOrder(u32),
    /// A reachable function body cannot be attributed to two native routines.
    AmbiguousReachableFunction(String),
    /// Missing or invalid original EDG evidence for one cleanup operand.
    MissingCleanupWitness,
    /// Two different paths/owners claim one source-owned cleanup operand.
    AmbiguousCleanupWitness,
    /// A runtime call has an unsupported argument shape; never guess an ABI.
    UnsupportedCleanupCall,
    /// The source-derived diagnostic could not represent or allocate its
    /// complete result; never silently truncate a proven call graph.
    ReachabilityLimit,
    /// EDG marked its optional native scheduler traversal incomplete.
    IncompleteScheduler,
    /// One native scheduled routine appears more than once.
    DuplicateScheduledInitializer(SymbolId),
    /// Event 104 refers to an object without a supported source file-scope
    /// variable declaration, or a local/TLS object.
    InvalidInitializerVariable(SymbolId),
    /// Event 104 names a routine absent from the native ordered schedule.
    UnscheduledInitializerOwner(SymbolId),
    /// Two event-104 owner records claim the same variable.
    DuplicateInitializerVariable(SymbolId),
    /// Two native globals claim the same scheduled routine.
    DuplicateInitializerRoutine(SymbolId),
    /// The native scheduler has a routine with no exact event-104 owner.
    MissingInitializerOwner(SymbolId),
    /// An independently certified native atexit call was found under a
    /// startup root that is not on the checked separate scheduler.
    UnscheduledCleanupRoot(SymbolId),
    /// A native atexit call under an initializer routine addresses a
    /// different object than that routine's source-owned global.
    MisownedCleanupRegistration { root: SymbolId, object: SymbolId },
    /// Multiple certified atexit calls claim the same initializer root.
    DuplicateCleanupRegistration(SymbolId),
    /// The requested all-registered subset has no certified direct atexit
    /// call for an initializer root. Does not mean no cleanup happens.
    MissingDirectCleanupRegistration(SymbolId),
}

/// Build a *diagnostic* candidate-root / direct-call view from source-owned
/// native IL identities. Does not authorize object construction or codegen.
pub fn collect_startup_evidence(il: &Il) -> Result<StartupEvidence, StartupEvidenceError> {
    let mut roots = Vec::new();
    // All four source-owned lookup tables are compilation-local. Index once
    // rather than scanning the entire IL for every generated startup root.
    // Reject duplicate routine attributes here as well as at the native
    // compile boundary: the tracer must not union contradictory 'defined'
    // flags and invent a locally callable body.
    let routine_symbols: HashSet<SymbolId> = il.symbols.iter()
        .filter(|s| s.kind == 1).map(|s| s.id).collect();
    let variable_symbols: HashSet<SymbolId> = il.symbols.iter()
        .filter(|s| s.kind == 2).map(|s| s.id).collect();
    let mut attributes_by_routine = HashMap::new();
    for attr in &il.routine_attributes {
        if attributes_by_routine.insert(attr.routine,attr).is_some() {
            return Err(StartupEvidenceError::DuplicateRoutineAttributes(attr.routine));
        }
    }
    let defined_functions: HashSet<&str> = il.entities.iter()
        .filter_map(|entity| match entity {
            crate::Entity::Routine { name, has_definition:true } => Some(name.as_str()),
            _ => None,
        }).collect();
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
        if !variable_symbols.contains(&entry.link_variable) {
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
        if !routine_symbols.contains(&root.routine) {
            return Err(StartupEvidenceError::MissingRoutine(root.routine));
        }
        let Some(function) = by_id.get(&root.routine) else {
            return Err(StartupEvidenceError::MissingRoutine(root.routine));
        };
        let Some(attributes) = attributes_by_routine.get(&root.routine) else {
                return Err(StartupEvidenceError::MissingDefinition(root.routine));
            };
        if !attributes.defined || !defined_functions.contains(*function) {
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


/// Exact file/namespace source-owned object mapped to its EDG separate-mode
/// initializer routine. The ordinal comes ONLY from native event 67; it is
/// not inferred from declarations or statement/call ordering.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct CheckedSeparateInitializer {
    pub variable: SymbolId,
    pub routine: SymbolId,
    pub function: String,
    pub scheduler_position: u32,
}

/// An internally consistent subset of the EDG *separate-mode* initializer
/// scheduler. Even when event 66 says complete, that is only completeness of
/// EDG's own scheduler list. It does NOT establish the complete C++ lifecycle,
/// effects of imported helpers, a destructor/atexit schedule, inter-TU order,
/// or executable ELF `.init_array` admission.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct CheckedSeparateInitializerSchedule {
    pub entries: Vec<CheckedSeparateInitializer>,
}

/// Join native event-104 variable→routine pointers to ordered event-67
/// routine identities, requiring a bijection among the represented entries.
/// This deliberately fails closed for missing entries, unscheduled ownership,
/// duplicate identities, TLS/local/non-file-scope variables, and unknown
/// function bodies. It NEVER authorizes emitting executable initialization.
pub fn collect_checked_separate_initializer_schedule(
    il:&Il,
) -> Result<CheckedSeparateInitializerSchedule,StartupEvidenceError> {
    let Some(schedule)=il.file_scope_init_schedule else {
        return Err(StartupEvidenceError::MissingScheduler);
    };
    if !schedule.available {
        return Err(StartupEvidenceError::ContradictoryScheduler);
    }
    if !schedule.complete {
        return Err(StartupEvidenceError::IncompleteScheduler);
    }
    // This independently checks source-owned routine symbols, body identity,
    // definition status and dense native scheduler order.
    let proof=collect_startup_evidence(il)?;
    let roots:Vec<_>=proof.roots.iter().filter(|root|
        root.source==StartupRootSource::SeparateNativeScheduler).collect();

    let mut scheduled=HashSet::with_capacity(roots.len());
    for root in &roots {
        if !scheduled.insert(root.routine) {
            return Err(StartupEvidenceError::DuplicateScheduledInitializer(root.routine));
        }
    }

    let variables:HashSet<_>=il.symbols.iter().filter(|s|s.kind==2)
        .map(|s|s.id).collect();
    let mut global=HashMap::with_capacity(il.global_variables.len());
    for entry in &il.global_variables {
        if global.insert(entry.symbol,entry).is_some() {
            return Err(StartupEvidenceError::DuplicateInitializerVariable(entry.symbol));
        }
    }
    let mut seen_variables=HashSet::with_capacity(il.file_scope_dynamic_init_owners.len());
    let mut by_routine=HashMap::with_capacity(il.file_scope_dynamic_init_owners.len());
    for owner in &il.file_scope_dynamic_init_owners {
        if !seen_variables.insert(owner.variable) {
            return Err(StartupEvidenceError::DuplicateInitializerVariable(owner.variable));
        }
        let Some(variable)=global.get(&owner.variable) else {
            return Err(StartupEvidenceError::InvalidInitializerVariable(owner.variable));
        };
        if !variables.contains(&owner.variable) ||
           variable.thread_local || variable.local_to_function {
            return Err(StartupEvidenceError::InvalidInitializerVariable(owner.variable));
        }
        if !scheduled.contains(&owner.routine) {
            return Err(StartupEvidenceError::UnscheduledInitializerOwner(owner.routine));
        }
        if by_routine.insert(owner.routine,owner.variable).is_some() {
            return Err(StartupEvidenceError::DuplicateInitializerRoutine(owner.routine));
        }
    }
    let mut entries=Vec::with_capacity(roots.len());
    for root in roots {
        let Some(&variable)=by_routine.get(&root.routine) else {
            return Err(StartupEvidenceError::MissingInitializerOwner(root.routine));
        };
        entries.push(CheckedSeparateInitializer {
            variable,routine:root.routine,function:root.function.clone(),
            scheduler_position:root.ordinal.expect("native scheduler root carries ordinal"),
        });
    }
    Ok(CheckedSeparateInitializerSchedule{entries})
}

/// Exact per-scheduled-object match for native immediate `__cxa_atexit`
/// operands. A None registration is UNKNOWN, not proof the object is trivial
/// or that its destructor is never registered transitively.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct CheckedSeparateCleanupAssociation {
    pub initializer: CheckedSeparateInitializer,
    pub direct_registration: Option<CleanupCallCandidate>,
}

/// Retains all source-certified direct cleanup registrations and marks
/// initializer roots without such an immediate call as explicitly unknown.
/// No runtime invocation, constructor/destructor ABI, cross-TU ordering or
/// linker startup section is established by this diagnostic.
pub fn collect_checked_separate_cleanup_associations(
    il:&Il,
) -> Result<Vec<CheckedSeparateCleanupAssociation>,StartupEvidenceError> {
    let scheduled=collect_checked_separate_initializer_schedule(il)?;
    let candidates=collect_cleanup_call_candidates(il)?;
    let mut by_root=HashMap::with_capacity(scheduled.entries.len());
    for entry in &scheduled.entries {
        if by_root.insert(entry.routine,entry.variable).is_some() {
            return Err(StartupEvidenceError::DuplicateScheduledInitializer(entry.routine));
        }
    }
    let mut registrations=HashMap::with_capacity(candidates.len());
    for call in candidates {
        let Some(&object)=by_root.get(&call.root) else {
            return Err(StartupEvidenceError::UnscheduledCleanupRoot(call.root));
        };
        if object!=call.object {
            return Err(StartupEvidenceError::MisownedCleanupRegistration {
                root:call.root,object:call.object,
            });
        }
        let root=call.root;
        if registrations.insert(root,call).is_some() {
            return Err(StartupEvidenceError::DuplicateCleanupRegistration(root));
        }
    }
    Ok(scheduled.entries.into_iter().map(|initializer| {
        let direct_registration=registrations.remove(&initializer.routine);
        CheckedSeparateCleanupAssociation {initializer,direct_registration}
    }).collect())
}

/// Stricter opt-in diagnostic for a caller that independently knows every
/// checked initializer in THIS subset should register a destructor directly.
/// Absence is an error; the result still does not certify a complete C++
/// lifecycle or that any call actually executes.
pub fn collect_checked_separate_direct_cleanup_registrations(
    il:&Il,
) -> Result<Vec<(CheckedSeparateInitializer,CleanupCallCandidate)>,
            StartupEvidenceError> {
    collect_checked_separate_cleanup_associations(il)?.into_iter()
        .map(|association| {
            let call=association.direct_registration.ok_or(
                StartupEvidenceError::MissingDirectCleanupRegistration(
                    association.initializer.routine))?;
            Ok((association.initializer,call))
        }).collect()
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

// Trace size follows the finite, source-owned IL graph. Revisited bodies are
// eliminated by identity; allocation or representability failures are errors,
// never incomplete results labelled as complete.

/// Trace source-owned *syntactic* direct call chains from all startup roots.
/// This adds transitive helper visibility beyond `immediate_calls` without
/// attempting to execute any C++ initialization, reconstruct call order,
/// follow indirect functions, or infer destructor registration.
pub fn trace_startup_calls(
    il: &Il, proof: &StartupEvidence,
) -> Result<StartupCallTrace, StartupEvidenceError> {
    // A proof can outlive its source snapshot or be built by a caller. Do not
    // trace an invented root or stale call target as if EDG attested to it.
    if collect_startup_evidence(il)? != *proof {
        return Err(StartupEvidenceError::MismatchedEvidence);
    }
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
            if visited.contains(&caller) {
                revisited_local_bodies += 1;
                continue;
            }
            if visited.len()==visited.capacity() {
                visited.try_reserve(1)
                    .map_err(|_|StartupEvidenceError::ReachabilityLimit)?;
            }
            visited.insert(caller);
            // The local-body filter enqueues only defined source-owned
            // symbols, so this worklist terminates after their distinct IDs.
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
                if edges.len()==edges.capacity() {
                    edges.try_reserve(1)
                        .map_err(|_|StartupEvidenceError::ReachabilityLimit)?;
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

/// A checked association of native source-owned address identities in a
/// generated startup call, NOT proof of registration execution or C++ ABI.
/// Even a nonempty result does not prove a complete ctor/dtor lifecycle.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct CleanupRegistrationWitness {
    pub root: SymbolId,
    pub function: String,
    pub statement_path: String,
    pub expression_path: String,
    pub scope: String,
    pub scope_position: u32,
    pub destructor: SymbolId,
    pub object: SymbolId,
    pub dso_handle: SymbolId,
}

/// Source-owned direct `__cxa_atexit` operand identities at one call site.
/// This is only a call-local candidate. It deliberately carries NO native
/// scope initializer ordinal, completeness claim, runtime execution proof,
/// destruction ABI proof, or executable startup admission.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct CleanupCallCandidate {
    pub root: SymbolId,
    pub function: String,
    pub statement_path: String,
    pub expression_path: String,
    pub destructor: SymbolId,
    pub object: SymbolId,
    pub dso_handle: SymbolId,
}

/// Walk only the exact expression children owned by one original EDG
/// argument. Independent event-55/48 target IDs are resolved separately.
fn cleanup_argument_nodes<'a>(
    function:&str,statement:&str,path:&str,
    expressions:&HashMap<(&'a str,&'a str,&'a str),&'a crate::Expression>,
    children:&HashMap<(&'a str,&'a str,&'a str),Vec<&'a crate::Expression>>,
) -> Result<Vec<&'a crate::Expression>,StartupEvidenceError> {
    let Some(&root)=expressions.get(&(function,statement,path)) else {
        return Err(StartupEvidenceError::MissingCleanupWitness);
    };
    let mut stack=vec![root];
    let mut seen=HashSet::new();
    let mut result=Vec::new();
    while let Some(node)=stack.pop() {
        if !seen.insert(node.path.as_str()) {
            return Err(StartupEvidenceError::AmbiguousCleanupWitness);
        }
        // All admitted __cxa_atexit operands have at most two original
        // expression nodes (bare value or one cast/address-of wrapper).
        // A larger tree is unsupported by shape, not by a depth ceiling.
        if result.len()>=2 {
            return Err(StartupEvidenceError::UnsupportedCleanupCall);
        }
        result.push(node);
        if let Some(nested)=children.get(&(function,statement,node.path.as_str())) {
            stack.extend(nested.iter().copied());
        }
    }
    Ok(result)
}

/// Source-checked, *call-local* address diagnostic for immediate native
/// startup-root calls to __cxa_atexit. Unlike the strict scoped registration
/// join, this does not require the post-lowering event-68 list, which EDG may
/// drain. Event 55 certifies the destructor address, event 48 the zero-offset
/// object address, and the native expression identifies __dso_handle.
/// Missing or contradictory operand evidence always fails closed.
/// This NEVER proves executed registration, a full cleanup roster, ABI,
/// ordering, or linker .init_array/.fini_array support.
pub fn collect_cleanup_call_candidates(
    il:&Il,
) -> Result<Vec<CleanupCallCandidate>,StartupEvidenceError> {
    let proof=collect_startup_evidence(il)?;
    // All indexes are compilation-local; avoid full-IL scans per call.
    let mut kinds=HashMap::new();
    for symbol in &il.symbols {
        if kinds.insert(symbol.id,symbol.kind).is_some() {
            return Err(StartupEvidenceError::AmbiguousCleanupWitness);
        }
    }
    let mut links=HashMap::new();
    for link in &il.symbol_link_names {
        if links.insert(link.symbol,link.link_name.as_str()).is_some() {
            return Err(StartupEvidenceError::AmbiguousCleanupWitness);
        }
    }
    let mut sites=HashMap::new();
    for site in &il.calls {
        let key=(site.function.as_str(),site.statement_path.as_str(),site.path.as_str());
        if sites.insert(key,site).is_some() {
            return Err(StartupEvidenceError::AmbiguousCleanupWitness);
        }
    }
    let mut expressions=HashMap::new();
    let mut children:HashMap<(&str,&str,&str),Vec<&crate::Expression>>=HashMap::new();
    for expr in &il.expressions {
        let key=(expr.function.as_str(),expr.statement_path.as_str(),expr.path.as_str());
        if expressions.insert(key,expr).is_some() {
            return Err(StartupEvidenceError::AmbiguousCleanupWitness);
        }
        if let Some(parent)=expr.parent.as_deref() {
            children.entry((expr.function.as_str(),expr.statement_path.as_str(),parent))
                .or_default().push(expr);
        }
    }
    let mut routines=HashMap::new();
    for event in &il.constant_routine_addresses {
        let key=(event.function.as_str(),event.statement_path.as_str(),event.path.as_str());
        if routines.insert(key,event.symbol).is_some() {
            return Err(StartupEvidenceError::AmbiguousCleanupWitness);
        }
    }
    let mut variables=HashMap::new();
    for event in &il.constant_addresses {
        let key=(event.function.as_str(),event.statement_path.as_str(),event.path.as_str());
        if variables.insert(key,(event.symbol,event.byte_offset)).is_some() {
            return Err(StartupEvidenceError::AmbiguousCleanupWitness);
        }
    }
    let mut result=Vec::new();
    for call in &proof.immediate_calls {
        let Some(callee)=call.callee_symbol else {continue;};
        if links.get(&callee)!=Some(&"__cxa_atexit") {continue;}
        if call.form!=CallTargetForm::DirectRoutine ||
           kinds.get(&callee)!=Some(&1) {
            return Err(StartupEvidenceError::UnsupportedCleanupCall);
        }
        let function=call.function.as_str();
        let stmt=call.statement_path.as_str();
        let path=call.expression_path.as_str();
        let Some(site)=sites.get(&(function,stmt,path)) else {
            return Err(StartupEvidenceError::MissingCleanupWitness);
        };
        if site.first_argument!=1 {
            return Err(StartupEvidenceError::UnsupportedCleanupCall);
        }
        // Only four original EDG eok_call children are legal: one callee
        // plus three arguments. Validate independent parent ownership as
        // well as ordinal paths; a detached node or extra .5 can otherwise
        // pass the old ".4 absent" check.
        let direct=children.get(&(function,stmt,path))
            .ok_or(StartupEvidenceError::MissingCleanupWitness)?;
        if direct.len()!=4 {
            return Err(StartupEvidenceError::UnsupportedCleanupCall);
        }
        for position in 0..4 {
            let expected=format!("{path}.{position}");
            if !direct.iter().any(|node|node.path==expected) {
                return Err(StartupEvidenceError::UnsupportedCleanupCall);
            }
        }
        // The original callee is native EDG enk_routine (21), not
        // enk_variable (3). Confirmed on real two-global C++17 startup IL.
        // The call-target record and the independently emitted original EDG
        // callee expression must identify the same native routine. This API
        // also accepts hand-built Il snapshots, not just validated compile().
        let callee_path=format!("{path}.0");
        let Some(callee_node)=expressions.get(&(function,stmt,callee_path.as_str())) else {
            return Err(StartupEvidenceError::MissingCleanupWitness);
        };
        if callee_node.parent.as_deref()!=Some(path) ||
           callee_node.kind!=21 || callee_node.symbol_id!=Some(callee) ||
           callee_node.type_id!=call.callee_type {
            return Err(StartupEvidenceError::MissingCleanupWitness);
        }
        let args=[format!("{path}.1"),format!("{path}.2"),format!("{path}.3")];
        let mut witnesses=[None,None];
        for index in 0..2 {
            let nodes=cleanup_argument_nodes(
                function,stmt,&args[index],&expressions,&children,
            )?;
            // Source address events are not proof that an arbitrary
            // expression *evaluates* to the source address. Admit only the
            // observed direct constant or single EDG cast wrapper for the
            // routine, and the bare object constant. Other operators might
            // compute a different address and remain unsupported.
            if index==0 {
                let bare=nodes.len()==1 && nodes[0].kind==2 &&
                    nodes[0].constant_kind==Some(7);
                let cast=nodes.len()==2 && nodes[0].kind==1 &&
                    nodes[0].operator==Some(5) &&
                    nodes[1].parent.as_deref()==Some(args[0].as_str()) &&
                    nodes[1].kind==2 && nodes[1].constant_kind==Some(7);
                if !bare && !cast {
                    return Err(StartupEvidenceError::UnsupportedCleanupCall);
                }
            } else if nodes.len()!=1 || nodes[0].kind!=2 ||
                      nodes[0].constant_kind!=Some(7) {
                return Err(StartupEvidenceError::UnsupportedCleanupCall);
            }
            for node in nodes {
                let key=(function,stmt,node.path.as_str());
                let native=if index==0 {
                    // ck_address has EDG native constant-kind 7 on the
                    // pinned C++ frontend; event 55 is independent proof.
                    if variables.contains_key(&key) {
                        return Err(StartupEvidenceError::AmbiguousCleanupWitness);
                    }
                    routines.get(&key).copied().map(|sym|(sym,0))
                } else {
                    if routines.contains_key(&key) {
                        return Err(StartupEvidenceError::AmbiguousCleanupWitness);
                    }
                    variables.get(&key).copied()
                };
                if let Some((symbol,offset))=native {
                    if node.kind!=2 || node.constant_kind!=Some(7) ||
                       offset!=0 ||
                       kinds.get(&symbol)!=Some(&(if index==0 {1} else {2})) {
                        return Err(StartupEvidenceError::MissingCleanupWitness);
                    }
                    if witnesses[index].replace(symbol).is_some() {
                        return Err(StartupEvidenceError::AmbiguousCleanupWitness);
                    }
                }
            }
        }
        let (Some(dtor),Some(object))=(witnesses[0],witnesses[1]) else {
            return Err(StartupEvidenceError::MissingCleanupWitness);
        };
        let dso_nodes=cleanup_argument_nodes(
            function,stmt,&args[2],&expressions,&children,
        )?;
        // A random nested reference to __dso_handle is not sufficient.
        // The observed source expression is an address-of wrapper with one
        // direct EDG variable child. No arbitrary pointer arithmetic.
        if dso_nodes.len()!=2 || dso_nodes[0].kind!=1 ||
           dso_nodes[0].operator!=Some(0) ||
           dso_nodes[1].parent.as_deref()!=Some(args[2].as_str()) ||
           dso_nodes[1].kind!=3 {
            return Err(StartupEvidenceError::UnsupportedCleanupCall);
        }
        let mut dso=None;
        for node in dso_nodes {
            if let Some(id)=node.symbol_id {
                if kinds.get(&id)==Some(&2) &&
                   links.get(&id)==Some(&"__dso_handle") &&
                   node.kind==3 &&
                   dso.replace(id).is_some() {
                    return Err(StartupEvidenceError::AmbiguousCleanupWitness);
                }
            }
        }
        let Some(dso)=dso else {
            return Err(StartupEvidenceError::MissingCleanupWitness);
        };
        result.push(CleanupCallCandidate {
            root:call.root,function:call.function.clone(),
            statement_path:call.statement_path.clone(),
            expression_path:call.expression_path.clone(),
            destructor:dtor,object,dso_handle:dso,
        });
    }
    Ok(result)
}

/// One independently source-certified call, optionally joined to one
/// retained native scope initializer. Missing post-lowering event 68 means
/// UNKNOWN ownership, NOT an absent global or an absent destructor.
/// Neither a successful join nor the collection proves a complete roster.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct CleanupScopeAssociation {
    pub call: CleanupCallCandidate,
    pub scope: Option<String>,
    pub scope_position: Option<u32>,
}

/// A lossless *partial* association diagnostic. Unlike the strict API this
/// retains source-certified native calls whose event-68 scope owner was
/// drained by EDG. Never infer the missing ordinal from call order.
/// Duplicate native scope pairs or two calls claiming the same ordinal
/// remain contradictory and fail closed.
pub fn collect_partial_cleanup_associations(
    il:&Il,
) -> Result<Vec<CleanupScopeAssociation>,StartupEvidenceError> {
    let candidates=collect_cleanup_call_candidates(il)?;
    let mut scoped=HashMap::new();
    for init in &il.scope_dynamic_initializers {
        if let (Some(dtor),Some(object))=(init.destructor,init.variable) {
            if scoped.insert((dtor,object),init).is_some() {
                return Err(StartupEvidenceError::AmbiguousCleanupWitness);
            }
        }
    }
    let mut consumed=HashSet::new();
    let mut result=Vec::with_capacity(candidates.len());
    for call in candidates {
        let entry=scoped.get(&(call.destructor,call.object));
        if let Some(init)=entry {
            if !consumed.insert((init.scope.as_str(),init.position)) {
                return Err(StartupEvidenceError::AmbiguousCleanupWitness);
            }
        }
        result.push(CleanupScopeAssociation {
            scope:entry.map(|init|init.scope.clone()),
            scope_position:entry.map(|init|init.position),
            call,
        });
    }
    Ok(result)
}

/// Strict source-owned scope *association* diagnostic, not a complete C++
/// cleanup registration schedule or permission to execute any runtime ABI.
/// Post-lowering EDG event 68 is non-exhaustive; missing pairs are UNKNOWN,
/// and two calls must not consume the same native scope initializer entry.
pub fn collect_cleanup_registration_witnesses(
    il:&Il,
) -> Result<Vec<CleanupRegistrationWitness>,StartupEvidenceError> {
    let associations=collect_partial_cleanup_associations(il)?;
    let mut result=Vec::with_capacity(associations.len());
    for association in associations {
        let (Some(scope),Some(scope_position))=
            (association.scope,association.scope_position) else {
            return Err(StartupEvidenceError::MissingCleanupWitness);
        };
        let call=association.call;
        result.push(CleanupRegistrationWitness {
            root:call.root,function:call.function,
            statement_path:call.statement_path,
            expression_path:call.expression_path,
            scope,scope_position,
            destructor:call.destructor,object:call.object,
            dso_handle:call.dso_handle,
        });
    }
    Ok(result)
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

    fn emit_cleanup_test_expr(
        il:&mut Il,statement:&str,path:&str,parent:Option<&str>,
        kind:u32,operator:Option<u32>,constant_kind:Option<u32>,
        symbol_id:Option<SymbolId>,
    ) {
        il.expressions.push(crate::Expression {
            function:"startup".into(),statement_path:statement.into(),
            path:path.into(),parent:parent.map(str::to_owned),
            kind,operator,type_kind:None,reference:None,
            type_size:None,type_alignment:None,constant_kind,
            constant_integer:None,related_type_role:None,related_type_kind:None,
            type_id:None,symbol_id,
        });
    }

    fn separate_init_fixture(count:u32) -> Il {
        let mut il=Il::default();
        il.file_scope_init_schedule=Some(FileScopeInitSchedule{
            available:true,complete:true,
        });
        for position in 0..count {
            let routine=SymbolId(position+10);
            let variable=SymbolId(position+1000);
            generated(&mut il,routine.0,&format!("init_{position}"),true);
            il.symbols.push(Symbol {
                id:variable,kind:2,name:format!("global_{position}"),
            });
            il.global_variables.push(crate::GlobalVariable {
                symbol:variable,linkage:2,explicit_initializer:false,
                thread_local:false,local_to_function:false,
            });
            il.file_scope_init_routines.push(FileScopeInitRoutine{
                position,routine,
            });
            il.file_scope_dynamic_init_owners.push(
                crate::FileScopeDynamicInitOwner{variable,routine});
        }
        il
    }

    /// Scale the same original EDG direct cleanup operand layout across
    /// separate native roots. Unlike a post-lowering scope join, event 68
    /// is absent; the only ownership source is the checked event-104 map.
    fn separate_cleanup_fixture(count:u32) -> Il {
        let mut il=separate_init_fixture(count);
        let mut calls=cleanup_test_fixture(count);
        il.symbols.extend(calls.symbols.drain(..).filter(|s|s.id.0>=2000));
        il.symbol_link_names.append(&mut calls.symbol_link_names);
        let function=|statement:&str| {
            format!("init_{}",statement.rsplit('.').next().expect("fixture index"))
        };
        for site in &mut calls.calls {
            site.function=function(&site.statement_path);
        }
        for target in &mut calls.call_targets {
            target.function=function(&target.statement_path);
        }
        for expression in &mut calls.expressions {
            expression.function=function(&expression.statement_path);
        }
        for addr in &mut calls.constant_addresses {
            addr.function=function(&addr.statement_path);
        }
        for addr in &mut calls.constant_routine_addresses {
            addr.function=function(&addr.statement_path);
        }
        il.calls.append(&mut calls.calls);
        il.call_targets.append(&mut calls.call_targets);
        il.expressions.append(&mut calls.expressions);
        il.constant_addresses.append(&mut calls.constant_addresses);
        il.constant_routine_addresses.append(&mut calls.constant_routine_addresses);
        il
    }

    #[test]
    fn source_owned_direct_cleanup_association_preserves_order_and_scales() {
        let il=separate_cleanup_fixture(128);
        assert!(il.scope_dynamic_initializers.is_empty());
        let pairs=collect_checked_separate_direct_cleanup_registrations(&il).unwrap();
        assert_eq!(pairs.len(),128);
        assert_eq!(pairs[0].0.scheduler_position,0);
        assert_eq!(pairs[0].0.variable,SymbolId(1000));
        assert_eq!(pairs[0].1.root,SymbolId(10));
        assert_eq!(pairs[0].1.object,SymbolId(1000));
        assert_eq!(pairs[0].1.destructor,SymbolId(2000));
        assert_eq!(pairs[127].0.scheduler_position,127);
        assert_eq!(pairs[127].1.root,SymbolId(137));
        assert_eq!(pairs[127].1.object,SymbolId(1127));
        let census=collect_checked_separate_cleanup_associations(&il).unwrap();
        assert_eq!(census.len(),128);
        assert!(census.iter().all(|a|a.direct_registration.is_some()));
    }

    #[test]
    fn missing_direct_cleanup_is_unknown_unless_explicitly_required() {
        let empty=separate_init_fixture(2);
        let census=collect_checked_separate_cleanup_associations(&empty).unwrap();
        assert_eq!(census.len(),2);
        assert!(census.iter().all(|a|a.direct_registration.is_none()));
        assert_eq!(collect_checked_separate_direct_cleanup_registrations(&empty),
                   Err(StartupEvidenceError::MissingDirectCleanupRegistration(SymbolId(10))));
        let mut partial=separate_cleanup_fixture(2);
        partial.calls.pop();
        partial.call_targets.pop();
        let census=collect_checked_separate_cleanup_associations(&partial).unwrap();
        assert!(census[0].direct_registration.is_some());
        assert!(census[1].direct_registration.is_none());
        assert_eq!(collect_checked_separate_direct_cleanup_registrations(&partial),
                   Err(StartupEvidenceError::MissingDirectCleanupRegistration(SymbolId(11))));
    }

    #[test]
    fn direct_cleanup_rejects_wrong_object_or_forged_operand() {
        let mut il=separate_cleanup_fixture(2);
        il.constant_addresses[1].symbol=SymbolId(1000);
        assert_eq!(collect_checked_separate_cleanup_associations(&il),
                   Err(StartupEvidenceError::MisownedCleanupRegistration{
                       root:SymbolId(11),object:SymbolId(1000)}));
        let mut il=separate_cleanup_fixture(2);
        il.constant_routine_addresses.pop();
        assert_eq!(collect_checked_separate_cleanup_associations(&il),
                   Err(StartupEvidenceError::MissingCleanupWitness));
        let mut il=separate_cleanup_fixture(2);
        il.file_scope_dynamic_init_owners[1].routine=SymbolId(10);
        assert_eq!(collect_checked_separate_cleanup_associations(&il),
                   Err(StartupEvidenceError::DuplicateInitializerRoutine(SymbolId(10))));
    }

    #[test]
    fn duplicate_exact_cleanup_registration_under_one_root_fails_closed() {
        let mut il=separate_cleanup_fixture(2);
        let from="0.block.0";
        let to="0.block.9";
        let mut copy=il.calls[0].clone();
        copy.statement_path=to.into();
        il.calls.push(copy);
        let mut copy=il.call_targets[0].clone();
        copy.statement_path=to.into();
        il.call_targets.push(copy);
        let originals:Vec<_>=il.expressions.iter()
            .filter(|e|e.statement_path==from).cloned().collect();
        for mut e in originals {
            e.statement_path=to.into();
            il.expressions.push(e);
        }
        let mut object=il.constant_addresses[0].clone();
        object.statement_path=to.into();
        il.constant_addresses.push(object);
        let mut dtor=il.constant_routine_addresses[0].clone();
        dtor.statement_path=to.into();
        il.constant_routine_addresses.push(dtor);
        assert_eq!(collect_checked_separate_cleanup_associations(&il),
                   Err(StartupEvidenceError::DuplicateCleanupRegistration(SymbolId(10))));
    }

    #[test]
    fn checked_separate_scheduler_scales_with_exact_global_owners() {
        let il=separate_init_fixture(512);
        let plan=collect_checked_separate_initializer_schedule(&il).unwrap();
        assert_eq!(plan.entries.len(),512);
        assert_eq!(plan.entries[0].variable,SymbolId(1000));
        assert_eq!(plan.entries[0].routine,SymbolId(10));
        assert_eq!(plan.entries[0].scheduler_position,0);
        assert_eq!(plan.entries[511].variable,SymbolId(1511));
        assert_eq!(plan.entries[511].routine,SymbolId(521));
        assert_eq!(plan.entries[511].scheduler_position,511);
        // An empty native scheduler must not be presented as a complete
        // C++ runtime lifecycle, even though the source-owned roster joins.
        let empty=separate_init_fixture(0);
        assert!(collect_checked_separate_initializer_schedule(&empty)
            .unwrap().entries.is_empty());
    }

    #[test]
    fn checked_separate_scheduler_rejects_missing_or_contradictory_roster() {
        let il=separate_init_fixture(2);
        let mut wrong=il.clone();
        wrong.file_scope_dynamic_init_owners.pop();
        assert_eq!(collect_checked_separate_initializer_schedule(&wrong),
                   Err(StartupEvidenceError::MissingInitializerOwner(SymbolId(11))));
        let mut wrong=il.clone();
        wrong.file_scope_dynamic_init_owners[1].routine=SymbolId(999);
        assert_eq!(collect_checked_separate_initializer_schedule(&wrong),
                   Err(StartupEvidenceError::UnscheduledInitializerOwner(SymbolId(999))));
        let mut wrong=il.clone();
        wrong.file_scope_dynamic_init_owners[1].routine=SymbolId(10);
        assert_eq!(collect_checked_separate_initializer_schedule(&wrong),
                   Err(StartupEvidenceError::DuplicateInitializerRoutine(SymbolId(10))));
        let mut wrong=il.clone();
        wrong.file_scope_dynamic_init_owners[1].variable=SymbolId(1000);
        assert_eq!(collect_checked_separate_initializer_schedule(&wrong),
                   Err(StartupEvidenceError::DuplicateInitializerVariable(SymbolId(1000))));
        let mut wrong=il.clone();
        wrong.file_scope_init_routines[1].routine=SymbolId(10);
        assert_eq!(collect_checked_separate_initializer_schedule(&wrong),
                   Err(StartupEvidenceError::DuplicateScheduledInitializer(SymbolId(10))));
        let mut wrong=il.clone();
        wrong.file_scope_init_routines[1].position=9;
        assert_eq!(collect_checked_separate_initializer_schedule(&wrong),
                   Err(StartupEvidenceError::InvalidSchedulerOrder(9)));
    }

    #[test]
    fn checked_separate_scheduler_rejects_unproven_global_or_incomplete_mode() {
        let il=separate_init_fixture(2);
        let mut wrong=il.clone();
        wrong.global_variables[0].thread_local=true;
        assert_eq!(collect_checked_separate_initializer_schedule(&wrong),
                   Err(StartupEvidenceError::InvalidInitializerVariable(SymbolId(1000))));
        let mut wrong=il.clone();
        wrong.global_variables[0].local_to_function=true;
        assert_eq!(collect_checked_separate_initializer_schedule(&wrong),
                   Err(StartupEvidenceError::InvalidInitializerVariable(SymbolId(1000))));
        let mut wrong=il.clone();
        wrong.global_variables.remove(0);
        assert_eq!(collect_checked_separate_initializer_schedule(&wrong),
                   Err(StartupEvidenceError::InvalidInitializerVariable(SymbolId(1000))));
        let mut wrong=il.clone();
        wrong.symbols.retain(|s|s.id!=SymbolId(1000));
        assert_eq!(collect_checked_separate_initializer_schedule(&wrong),
                   Err(StartupEvidenceError::InvalidInitializerVariable(SymbolId(1000))));
        let mut wrong=il.clone();
        wrong.file_scope_init_schedule=Some(FileScopeInitSchedule{
            available:true,complete:false,
        });
        assert_eq!(collect_checked_separate_initializer_schedule(&wrong),
                   Err(StartupEvidenceError::IncompleteScheduler));
        let mut wrong=il.clone();
        wrong.file_scope_init_schedule=Some(FileScopeInitSchedule{
            available:false,complete:false,
        });
        assert_eq!(collect_checked_separate_initializer_schedule(&wrong),
                   Err(StartupEvidenceError::ContradictoryScheduler));
        let mut wrong=il.clone();
        wrong.file_scope_init_schedule=None;
        assert_eq!(collect_checked_separate_initializer_schedule(&wrong),
                   Err(StartupEvidenceError::MissingScheduler));
        let mut wrong=il;
        wrong.routine_attributes[0].defined=false;
        assert_eq!(collect_checked_separate_initializer_schedule(&wrong),
                   Err(StartupEvidenceError::MissingDefinition(SymbolId(10))));
    }

    fn cleanup_test_fixture(count:u32) -> Il {
        let mut il=Il::default();
        generated(&mut il,42,"startup",true);
        il.file_scope_lowered_startup_routines.push(
            FileScopeLoweredStartupRoutine{routine:SymbolId(42),priority:0});
        il.symbols.extend([
            Symbol{id:SymbolId(3000),kind:2,name:"__dso_handle".into()},
            Symbol{id:SymbolId(3001),kind:1,name:"__cxa_atexit".into()},
        ]);
        il.symbol_link_names.extend([
            crate::SymbolLinkName{symbol:SymbolId(3000),
                link_name:"__dso_handle".into(),edg_mangled:false},
            crate::SymbolLinkName{symbol:SymbolId(3001),
                link_name:"__cxa_atexit".into(),edg_mangled:false},
        ]);
        for i in 0..count {
            let object=SymbolId(1000+i);
            let dtor=SymbolId(2000+i);
            let statement=format!("0.block.{i}");
            il.symbols.extend([
                Symbol{id:object,kind:2,name:format!("global_{i}")},
                Symbol{id:dtor,kind:1,name:format!("dtor_{i}")},
            ]);
            il.scope_dynamic_initializers.push(crate::ScopeDynamicInitializer {
                scope:"1:".into(),position:i,variable:Some(object),
                destructor:Some(dtor),kind:0,
            });
            add_call(&mut il,"startup",&statement,
                     CallTargetForm::DirectRoutine,Some(SymbolId(3001)));
            emit_cleanup_test_expr(&mut il,&statement,"0",None,1,Some(3),None,None);
            emit_cleanup_test_expr(&mut il,&statement,"0.0",Some("0"),
                21,None,None,Some(SymbolId(3001)));
            emit_cleanup_test_expr(&mut il,&statement,"0.1",Some("0"),
                1,Some(5),None,None);
            emit_cleanup_test_expr(&mut il,&statement,"0.1.0",Some("0.1"),
                2,None,Some(7),None);
            emit_cleanup_test_expr(&mut il,&statement,"0.2",Some("0"),
                2,None,Some(7),None);
            emit_cleanup_test_expr(&mut il,&statement,"0.3",Some("0"),
                1,Some(0),None,None);
            emit_cleanup_test_expr(&mut il,&statement,"0.3.0",Some("0.3"),
                3,None,None,Some(SymbolId(3000)));
            il.constant_routine_addresses.push(crate::ConstantRoutineAddress{
                function:"startup".into(),statement_path:statement.clone(),
                path:"0.1.0".into(),symbol:dtor,
            });
            il.constant_addresses.push(crate::ConstantAddress{
                function:"startup".into(),statement_path:statement,
                path:"0.2".into(),symbol:object,byte_offset:0,
            });
        }
        il
    }

    #[test]
    fn exact_cleanup_address_witnesses_scale_across_many_globals() {
        let il=cleanup_test_fixture(256);
        let witnesses=collect_cleanup_registration_witnesses(&il).unwrap();
        assert_eq!(witnesses.len(),256);
        assert_eq!(witnesses[0].root,SymbolId(42));
        assert_eq!(witnesses[0].destructor,SymbolId(2000));
        assert_eq!(witnesses[0].object,SymbolId(1000));
        assert_eq!(witnesses[0].dso_handle,SymbolId(3000));
        assert_eq!(witnesses[255].scope_position,255);
        assert_eq!(witnesses[255].destructor,SymbolId(2255));
        assert_eq!(witnesses[255].object,SymbolId(1255));
    }

    #[test]
    fn drained_native_scope_roster_still_has_exact_call_local_candidates() {
        let mut il=cleanup_test_fixture(2);
        // Real native EDG lowers two cleanup calls but can drain one scope
        // initializer event 68. Do not invent its scope ordinal.
        il.scope_dynamic_initializers.pop();
        let candidates=collect_cleanup_call_candidates(&il).unwrap();
        assert_eq!(candidates.len(),2);
        assert_eq!(candidates[0].object,SymbolId(1000));
        assert_eq!(candidates[1].object,SymbolId(1001));
        assert_ne!(candidates[0].statement_path,candidates[1].statement_path);
        assert_eq!(collect_cleanup_registration_witnesses(&il),
                   Err(StartupEvidenceError::MissingCleanupWitness));
    }

    #[test]
    fn partial_cleanup_association_preserves_drained_native_entries() {
        let mut il=cleanup_test_fixture(2);
        il.scope_dynamic_initializers.pop();
        let associations=collect_partial_cleanup_associations(&il).unwrap();
        assert_eq!(associations.len(),2);
        assert_eq!(associations[0].call.object,SymbolId(1000));
        assert_eq!(associations[0].scope.as_deref(),Some("1:"));
        assert_eq!(associations[0].scope_position,Some(0));
        assert_eq!(associations[1].call.object,SymbolId(1001));
        assert_eq!(associations[1].scope,None);
        assert_eq!(associations[1].scope_position,None);
        assert_eq!(collect_cleanup_registration_witnesses(&il),
                   Err(StartupEvidenceError::MissingCleanupWitness));
    }

    #[test]
    fn partial_association_rejects_duplicate_native_scope_owners() {
        let mut il=cleanup_test_fixture(2);
        // A duplicated source-owned pair cannot choose a scope ordinal.
        il.scope_dynamic_initializers[1].variable=Some(SymbolId(1000));
        il.scope_dynamic_initializers[1].destructor=Some(SymbolId(2000));
        assert_eq!(collect_partial_cleanup_associations(&il),
                   Err(StartupEvidenceError::AmbiguousCleanupWitness));
    }

    #[test]
    fn partial_association_does_not_claim_complete_empty_roster() {
        let mut il=cleanup_test_fixture(2);
        il.scope_dynamic_initializers.clear();
        let associations=collect_partial_cleanup_associations(&il).unwrap();
        assert_eq!(associations.len(),2);
        assert!(associations.iter().all(|association|
            association.scope.is_none() && association.scope_position.is_none()));
    }

    #[test]
    fn duplicate_cleanup_calls_remain_visible_but_not_scoped_proofs() {
        let mut il=cleanup_test_fixture(2);
        // Source-owned distinct call sites may name one original object.
        // Preserve both diagnostic occurrences; reject duplicate ownership.
        il.constant_routine_addresses[1].symbol=SymbolId(2000);
        il.constant_addresses[1].symbol=SymbolId(1000);
        let candidates=collect_cleanup_call_candidates(&il).unwrap();
        assert_eq!(candidates.len(),2);
        assert_eq!(candidates[0].object,candidates[1].object);
        assert_eq!(collect_cleanup_registration_witnesses(&il),
                   Err(StartupEvidenceError::AmbiguousCleanupWitness));
    }

    #[test]
    fn call_local_candidate_rejects_contradictory_source_callee() {
        let il=cleanup_test_fixture(1);
        let mut wrong=il.clone();
        wrong.expressions[1].symbol_id=Some(SymbolId(2000));
        assert_eq!(collect_cleanup_call_candidates(&wrong),
                   Err(StartupEvidenceError::MissingCleanupWitness));
        let mut wrong=il.clone();
        // An enk_variable (3) cannot stand in for EDG enk_routine (21),
        // even if its symbol and type fields are manually forged.
        wrong.expressions[1].kind=3;
        assert_eq!(collect_cleanup_call_candidates(&wrong),
                   Err(StartupEvidenceError::MissingCleanupWitness));
        let mut wrong=il.clone();
        wrong.expressions[1].parent=Some("0.1".into());
        // Once native direct-child ownership is checked first, a detached
        // callee is classified as an unsupported malformed call shape.
        assert_eq!(collect_cleanup_call_candidates(&wrong),
                   Err(StartupEvidenceError::UnsupportedCleanupCall));
    }

    #[test]
    fn cleanup_call_requires_exact_direct_child_list() {
        let fixture=cleanup_test_fixture(1);
        // A path resembling a direct argument cannot replace EDG parenthood.
        let mut wrong=fixture.clone();
        let arg=wrong.expressions.iter_mut()
            .find(|expr|expr.path=="0.2").unwrap();
        arg.parent=Some("0.1".into());
        assert_eq!(collect_cleanup_call_candidates(&wrong),
                   Err(StartupEvidenceError::UnsupportedCleanupCall));

        // The old code only excluded extra .4; extra .5 was accepted.
        let mut wrong=fixture.clone();
        emit_cleanup_test_expr(&mut wrong,"0.block.0","0.5",Some("0"),
                               2,None,Some(7),None);
        assert_eq!(collect_cleanup_call_candidates(&wrong),
                   Err(StartupEvidenceError::UnsupportedCleanupCall));

        // Missing one direct argument is not repaired by a renumbered child.
        let mut wrong=fixture;
        let arg=wrong.expressions.iter_mut()
            .find(|expr|expr.path=="0.3").unwrap();
        arg.path="0.5".into();
        assert_eq!(collect_cleanup_call_candidates(&wrong),
                   Err(StartupEvidenceError::UnsupportedCleanupCall));
    }

    #[test]
    fn multiple_globals_may_share_one_native_destructor_routine() {
        let mut il=cleanup_test_fixture(2);
        // EDG emits distinct global objects with a common class D1 body.
        il.scope_dynamic_initializers[1].destructor=Some(SymbolId(2000));
        il.constant_routine_addresses[1].symbol=SymbolId(2000);
        let witnesses=collect_cleanup_registration_witnesses(&il).unwrap();
        assert_eq!(witnesses.len(),2);
        assert_eq!(witnesses[0].destructor,witnesses[1].destructor);
        assert_ne!(witnesses[0].object,witnesses[1].object);
        assert_eq!(witnesses[0].scope_position,0);
        assert_eq!(witnesses[1].scope_position,1);
    }

    #[test]
    fn duplicate_native_registration_for_one_scope_initializer_is_ambiguous() {
        let mut il=cleanup_test_fixture(2);
        // A separate original call site reuses the first EDG source-owned
        // destructor/object pair rather than owning the second initializer.
        // The old API emitted TWO positive witnesses for the SAME owner.
        il.constant_routine_addresses[1].symbol=SymbolId(2000);
        il.constant_addresses[1].symbol=SymbolId(1000);
        assert_eq!(collect_cleanup_registration_witnesses(&il),
                   Err(StartupEvidenceError::AmbiguousCleanupWitness));
    }

    #[test]
    fn incomplete_cleanup_operand_witnesses_fail_closed() {
        let il=cleanup_test_fixture(1);
        let mut wrong=il.clone();
        wrong.constant_routine_addresses.clear();
        assert_eq!(collect_cleanup_registration_witnesses(&wrong),
                   Err(StartupEvidenceError::MissingCleanupWitness));
        let mut wrong=il.clone();
        wrong.constant_addresses[0].byte_offset=1;
        assert_eq!(collect_cleanup_registration_witnesses(&wrong),
                   Err(StartupEvidenceError::MissingCleanupWitness));
        let mut wrong=il.clone();
        wrong.constant_addresses[0].symbol=SymbolId(3000);
        assert_eq!(collect_cleanup_registration_witnesses(&wrong),
                   Err(StartupEvidenceError::MissingCleanupWitness));
        let mut wrong=il.clone();
        wrong.constant_routine_addresses[0].path="0.1.1".into();
        assert_eq!(collect_cleanup_registration_witnesses(&wrong),
                   Err(StartupEvidenceError::MissingCleanupWitness));
        let mut wrong=il.clone();
        wrong.scope_dynamic_initializers[0].destructor=Some(SymbolId(42));
        assert_eq!(collect_cleanup_registration_witnesses(&wrong),
                   Err(StartupEvidenceError::MissingCleanupWitness));
        let mut wrong=il.clone();
        wrong.symbol_link_names[0].link_name="other_context".into();
        assert_eq!(collect_cleanup_registration_witnesses(&wrong),
                   Err(StartupEvidenceError::MissingCleanupWitness));
        let mut wrong=il.clone();
        wrong.expressions.last_mut().unwrap().symbol_id=None;
        assert_eq!(collect_cleanup_registration_witnesses(&wrong),
                   Err(StartupEvidenceError::MissingCleanupWitness));
        let mut wrong=il.clone();
        wrong.expressions[3].constant_kind=Some(1);
        assert_eq!(collect_cleanup_registration_witnesses(&wrong),
                   Err(StartupEvidenceError::UnsupportedCleanupCall));
    }

    #[test]
    fn ambiguous_or_unsupported_cleanup_edges_do_not_claim_complete_abi() {
        let il=cleanup_test_fixture(1);
        let mut wrong=il.clone();
        wrong.constant_routine_addresses.push(wrong.constant_routine_addresses[0].clone());
        assert_eq!(collect_cleanup_registration_witnesses(&wrong),
                   Err(StartupEvidenceError::AmbiguousCleanupWitness));
        let mut wrong=il.clone();
        let mut duplicate=wrong.scope_dynamic_initializers[0].clone();
        duplicate.scope="2:".into();
        wrong.scope_dynamic_initializers.push(duplicate);
        assert_eq!(collect_cleanup_registration_witnesses(&wrong),
                   Err(StartupEvidenceError::AmbiguousCleanupWitness));
        let mut wrong=il.clone();
        wrong.calls[0].first_argument=2;
        assert_eq!(collect_cleanup_registration_witnesses(&wrong),
                   Err(StartupEvidenceError::UnsupportedCleanupCall));
        let mut wrong=il.clone();
        wrong.expressions[2].operator=Some(99);
        assert_eq!(collect_cleanup_registration_witnesses(&wrong),
                   Err(StartupEvidenceError::UnsupportedCleanupCall));
        let mut wrong=il.clone();
        wrong.expressions[5].operator=Some(99);
        assert_eq!(collect_cleanup_registration_witnesses(&wrong),
                   Err(StartupEvidenceError::UnsupportedCleanupCall));
        let mut wrong=il.clone();
        emit_cleanup_test_expr(&mut wrong,"0.block.0","0.1.1",Some("0.1"),
            2,None,Some(7),None);
        assert_eq!(collect_cleanup_registration_witnesses(&wrong),
                   Err(StartupEvidenceError::UnsupportedCleanupCall));
        let mut wrong=il;
        emit_cleanup_test_expr(&mut wrong,"0.block.0","0.4",Some("0"),
            2,None,Some(7),None);
        assert_eq!(collect_cleanup_registration_witnesses(&wrong),
                   Err(StartupEvidenceError::UnsupportedCleanupCall));
    }

    #[test]
    fn many_native_startup_roots_keep_source_order_and_failure_modes() {
        let mut il=Il::default();
        for id in 1..=4096 {
            generated(&mut il,id,&format!("generated_startup_{id}"),true);
            il.file_scope_lowered_startup_routines.push(
                FileScopeLoweredStartupRoutine {
                    routine:SymbolId(id),priority:id % 200,
                });
        }
        let proof=collect_startup_evidence(&il).unwrap();
        assert_eq!(proof.roots.len(),4096);
        assert_eq!(proof.roots[0].routine,SymbolId(1));
        assert_eq!(proof.roots[0].function,"generated_startup_1");
        assert_eq!(proof.roots[4095].routine,SymbolId(4096));
        assert_eq!(proof.roots[4095].function,"generated_startup_4096");
        assert!(proof.immediate_calls.is_empty());

        let mut missing=il.clone();
        missing.routine_attributes.pop();
        assert_eq!(collect_startup_evidence(&missing),
                   Err(StartupEvidenceError::MissingDefinition(SymbolId(4096))));
        let mut missing=il.clone();
        missing.symbols.retain(|sym|sym.id!=SymbolId(2048));
        assert_eq!(collect_startup_evidence(&missing),
                   Err(StartupEvidenceError::MissingRoutine(SymbolId(2048))));
    }

    #[test]
    fn many_runtime_link_roots_keep_source_order_and_missing_link_fails() {
        let mut il=Il::default();
        for id in 1..=4096 {
            generated(&mut il,id,&format!("link_target_{id}"),false);
            il.symbols.push(Symbol {
                id:SymbolId(id+10_000),kind:2,name:format!("link_{id}"),
            });
            il.file_scope_runtime_links.push(FileScopeRuntimeLink {
                link_variable:SymbolId(id+10_000),startup_routine:SymbolId(id),
            });
        }
        let proof=collect_startup_evidence(&il).unwrap();
        assert_eq!(proof.roots.len(),4096);
        assert_eq!(proof.roots[0].source,StartupRootSource::RuntimeLink);
        assert_eq!(proof.roots[0].link_variable,Some(SymbolId(10_001)));
        assert_eq!(proof.roots[4095].routine,SymbolId(4096));
        assert_eq!(proof.roots[4095].link_variable,Some(SymbolId(14_096)));
        let mut missing=il.clone();
        missing.symbols.retain(|sym|sym.id!=SymbolId(14_096));
        assert_eq!(collect_startup_evidence(&missing),
                   Err(StartupEvidenceError::MissingLinkVariable(SymbolId(14_096))));
    }

    #[test]
    fn duplicate_routine_attributes_fail_closed_even_in_public_diagnostics() {
        let mut il=Il::default();
        generated(&mut il,42,"startup",true);
        il.file_scope_lowered_startup_routines.push(
            FileScopeLoweredStartupRoutine{routine:SymbolId(42),priority:0});
        let proof=collect_startup_evidence(&il).unwrap();
        let mut contradictory=il.routine_attributes[0].clone();
        contradictory.defined=false;
        il.routine_attributes.push(contradictory);
        assert_eq!(collect_startup_evidence(&il),
                   Err(StartupEvidenceError::DuplicateRoutineAttributes(SymbolId(42))));
        assert_eq!(trace_startup_calls(&il,&proof),
                   Err(StartupEvidenceError::DuplicateRoutineAttributes(SymbolId(42))));
    }

    #[test]
    fn trace_rejects_foreign_or_stale_startup_evidence() {
        let mut il=Il::default();
        generated(&mut il,42,"startup",true);
        generated(&mut il,43,"helper",false);
        il.file_scope_lowered_startup_routines.push(
            FileScopeLoweredStartupRoutine{routine:SymbolId(42),priority:0});
        add_call(&mut il,"startup","0",CallTargetForm::DirectRoutine,Some(SymbolId(43)));
        let proof=collect_startup_evidence(&il).unwrap();
        assert_eq!(trace_startup_calls(&il,&proof).unwrap().edges.len(),1);

        let mut forged=proof.clone();
        forged.roots[0].priority=Some(99);
        assert_eq!(trace_startup_calls(&il,&forged),
                   Err(StartupEvidenceError::MismatchedEvidence));
        let mut forged=proof.clone();
        forged.immediate_calls.clear();
        assert_eq!(trace_startup_calls(&il,&forged),
                   Err(StartupEvidenceError::MismatchedEvidence));

        // A new target witness must invalidate the old source-owned proof,
        // even if the trace could otherwise traverse the same routine body.
        il.call_targets[0].callee_symbol=Some(SymbolId(44));
        assert_eq!(trace_startup_calls(&il,&proof),
                   Err(StartupEvidenceError::MismatchedEvidence));
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
