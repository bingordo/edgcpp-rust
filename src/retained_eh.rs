//! Checked source-owned nonportable EDG exception actions.
//!
//! This is a view of the existing snapshot, not a second compiler IR or an
//! executable unwind plan. Native discriminants are normalized by the C++
//! bridge built against the same EDG configuration.

use crate::{Error, Il, NativeRetainedEhConstruct};
use std::collections::HashMap;

extern "C" {
    fn edgcpp_codegen_kind(category: u32, value: u32) -> u32;
}

/// Stable meanings of retained EDG EH expressions. These describe source IL
/// actions only; in particular cleanup-state changes are not destructor calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetainedEhKind {
    CaughtObjectAddress,
    ThrownObjectAddress,
    CleanupState,
    UnreachableCleanupState,
    FunctionPrologue,
    FunctionEpilogue,
    CatchEpilogue,
    TryEpilogue,
    ExceptionCaught,
    ExceptionStarted,
    InitializationCompleted,
    InternalTry,
}

/// A validated action with its original expression identity. The identity
/// includes the control role; identical ordinal paths in different trees
/// must never be merged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetainedEhAction<'a> {
    pub source: &'a NativeRetainedEhConstruct,
    pub kind: RetainedEhKind,
}

impl RetainedEhKind {
    /// Normalize through the configured native enum, not a Rust ordinal.
    fn from_native(raw: u32) -> Option<Self> {
        Some(match unsafe { edgcpp_codegen_kind(7, raw) } {
            1 => Self::CaughtObjectAddress,
            2 => Self::ThrownObjectAddress,
            3 => Self::CleanupState,
            4 => Self::UnreachableCleanupState,
            5 => Self::FunctionPrologue,
            6 => Self::FunctionEpilogue,
            7 => Self::CatchEpilogue,
            8 => Self::TryEpilogue,
            9 => Self::ExceptionCaught,
            10 => Self::ExceptionStarted,
            11 => Self::InitializationCompleted,
            12 => Self::InternalTry,
            _ => return None,
        })
    }
}

type Key<'a> = (&'a str, &'a str, u32, &'a str);

fn is_retained_eh(raw: u32) -> bool {
    unsafe { edgcpp_codegen_kind(8, raw) == 1 }
}

/// Require exactly one known action per original retained expression in
/// primary and control trees. InternalTry owns normal work and exceptional
/// cleanup in that order. This checks structural evidence, not type matching,
/// cleanup-state payloads, exception object ownership or Itanium admission.
///
/// Runtime is linear in snapshot nodes plus the length of their path strings.
/// No recursion, fixed depth or number-of-actions limit is introduced.
pub fn checked_retained_eh_actions(il: &Il) -> Result<Vec<RetainedEhAction<'_>>, Error> {
    let mut expected: HashMap<Key<'_>, u32> = HashMap::new();
    for e in &il.expressions {
        if is_retained_eh(e.kind)
            && expected.insert((&e.function, &e.statement_path, 0, &e.path), e.kind).is_some()
        {
            return Err(Error::MalformedCallback);
        }
    }
    for e in &il.control_expressions {
        if is_retained_eh(e.kind)
            && expected.insert((&e.function, &e.statement_path, e.role, &e.path), e.kind).is_some()
        {
            return Err(Error::MalformedCallback);
        }
    }
    let mut actions = Vec::with_capacity(il.native_retained_eh_constructs.len());
    let mut internal: HashMap<Key<'_>, u8> = HashMap::new();
    for source in &il.native_retained_eh_constructs {
        let key = (source.function.as_str(), source.statement_path.as_str(),
                   source.role, source.path.as_str());
        if source.role > 4 || expected.remove(&key) != Some(source.expression_kind) {
            return Err(Error::MalformedCallback);
        }
        let kind = RetainedEhKind::from_native(source.subkind).ok_or(Error::MalformedCallback)?;
        if kind == RetainedEhKind::InternalTry {
            internal.insert(key, 0);
        }
        actions.push(RetainedEhAction { source, kind });
    }
    if !expected.is_empty() {
        return Err(Error::MalformedCallback);
    }
    if internal.is_empty() {
        return Ok(actions);
    }

    // Each internal try must own both distinct children, with canonical
    // ordinal identities. Look up parents by the full expression owner.
    fn child<'a>(internal: &mut HashMap<Key<'a>, u8>, key: Key<'a>, path: &str)
        -> Result<(), Error>
    {
        if let Some(mask) = internal.get_mut(&key) {
            let suffix = path.strip_prefix(key.3).ok_or(Error::MalformedCallback)?;
            let bit = match suffix {
                ".0" => 1,
                ".1" => 2,
                _ => return Err(Error::MalformedCallback),
            };
            if *mask & bit != 0 {
                return Err(Error::MalformedCallback);
            }
            *mask |= bit;
        }
        Ok(())
    }

    for e in &il.expressions {
        if let Some(parent) = e.parent.as_deref() {
            child(&mut internal, (&e.function, &e.statement_path, 0, parent), &e.path)?;
        }
    }
    for e in &il.control_expressions {
        if let Some(parent) = e.parent.as_deref() {
            child(&mut internal, (&e.function, &e.statement_path, e.role, parent), &e.path)?;
        }
    }
    if internal.values().any(|&mask| mask != 3) {
        return Err(Error::MalformedCallback);
    }
    Ok(actions)
}

/// One native catch clause, keyed by its owning try statement and ordinal.
/// A missing parameter denotes an ellipsis catch, not a missing record.
/// Typeinfo is an EDG variable identity, not a proven Itanium RTTI symbol.
/// Initialization presence does not prove the copy or destruction plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCatchHandler {
    pub function: String,
    pub statement_path: String,
    pub position: u32,
    pub parameter: Option<crate::SymbolId>,
    pub parameter_type: Option<crate::TypeId>,
    pub typeinfo: Option<crate::SymbolId>,
    pub has_initialization: bool,
    pub function_try: bool,
}

/// Join the declaration roster to the independently counted try-handler
/// lists. Reject gaps, duplicates, foreign owners and inconsistent types.
/// Normal and exceptional cleanup schedules still require separate proof.
pub fn validate_native_catch_handlers(il: &Il) -> Result<(), Error> {
    use std::collections::HashSet;
    let mut owners = HashMap::new();
    for proof in &il.native_try_handler_counts {
        if owners.insert((proof.function.as_str(), proof.statement_path.as_str()),
                         (proof.count, 0u32, None)).is_some() {
            return Err(Error::MalformedCallback);
        }
    }
    let symbols: HashMap<_, _> = il.symbols.iter().map(|s| (s.id, s.kind)).collect();
    let types: HashSet<_> = il.types.iter().map(|t| t.id).collect();
    let declarations: HashMap<_, _> = il.symbol_types.iter().map(|d| (d.symbol, d.ty)).collect();
    let mut seen = HashSet::new();
    for h in &il.native_catch_handlers {
        let key = (h.function.as_str(), h.statement_path.as_str());
        let (count, observed, function_try) = owners.get_mut(&key)
            .ok_or(Error::MalformedCallback)?;
        if h.position >= *count || !seen.insert((key, h.position)) ||
           function_try.is_some_and(|flag| flag != h.function_try) {
            return Err(Error::MalformedCallback);
        }
        *function_try = Some(h.function_try);
        match (h.parameter, h.parameter_type) {
            (Some(symbol), Some(ty)) if symbols.get(&symbol) == Some(&2) &&
                types.contains(&ty) && declarations.get(&symbol) == Some(&ty) => {}
            (None, None) if h.position == *count - 1 && !h.has_initialization &&
                h.typeinfo.is_none() => {}
            _ => return Err(Error::MalformedCallback),
        }
        if h.typeinfo.is_some_and(|id| symbols.get(&id) != Some(&2) ||
            !declarations.get(&id).is_some_and(|ty| types.contains(ty))) {
            return Err(Error::MalformedCallback);
        }
        *observed += 1;
    }
    if owners.values().any(|(count, observed, _)| count != observed) {
        return Err(Error::MalformedCallback);
    }
    Ok(())
}

/// Exact native cleanup union value. Dynamic-init IDs share the routine-local
/// identity namespace; a non-null ID may lack an independently exported owner.
/// Region numbers are EDG table-mode values, not generic lifetime IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupReference {
    DynamicInit(Option<u32>),
    Region(u64),
}

/// Cleanup-state or initialization-completed payload keyed by the original
/// retained action occurrence. No destructor plan is implied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeEhCleanupState {
    pub function: String,
    pub statement_path: String,
    pub role: u32,
    pub path: String,
    pub routine: crate::SymbolId,
    pub reference: CleanupReference,
}

/// Require exactly one payload per cleanup action and the correct routine
/// owner. Equal dynamic-init IDs in different routines are unrelated.
pub fn validate_native_eh_cleanup_states(il: &Il) -> Result<(), Error> {
    let names: HashMap<_, _> = il.routine_identities.iter()
        .map(|r| (r.function.as_str(), r.routine)).collect();
    let mut expected = HashMap::new();
    for a in &il.native_retained_eh_constructs {
        let kind = RetainedEhKind::from_native(a.subkind).ok_or(Error::MalformedCallback)?;
        if !matches!(kind, RetainedEhKind::CleanupState |
            RetainedEhKind::UnreachableCleanupState | RetainedEhKind::InitializationCompleted) {
            continue;
        }
        let key = (a.function.as_str(), a.statement_path.as_str(), a.role, a.path.as_str());
        if expected.insert(key, kind).is_some() {return Err(Error::MalformedCallback);}
    }
    for state in &il.native_eh_cleanup_states {
        let key = (state.function.as_str(), state.statement_path.as_str(),
                   state.role, state.path.as_str());
        let kind = expected.remove(&key).ok_or(Error::MalformedCallback)?;
        if names.get(state.function.as_str()) != Some(&state.routine) ||
            matches!(state.reference, CleanupReference::DynamicInit(Some(0))) ||
            (kind == RetainedEhKind::InitializationCompleted &&
                !matches!(state.reference, CleanupReference::DynamicInit(Some(_)))) {
            return Err(Error::MalformedCallback);
        }
    }
    if !expected.is_empty() {return Err(Error::MalformedCallback);}
    Ok(())
}

/// Original native try supplement (position=None) or handler (position=Some).
/// IDs are routine-local and disjoint from the dynamic-init namespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeEhOwner {
    pub function: String,
    pub statement_path: String,
    pub routine: crate::SymbolId,
    pub position: Option<u32>,
    pub id: u32,
    pub initialization: Option<u32>,
}

/// Original EH action union pointer. For function entry/exit the target is
/// a routine symbol; for catch/try actions it is a routine-local owner ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeEhActionTarget {
    pub function: String,
    pub statement_path: String,
    pub role: u32,
    pub path: String,
    pub routine: crate::SymbolId,
    pub target: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EhTargetCoverage {
    pub handlers: usize,
    pub tries: usize,
    pub routines: usize,
    /// A real native pointer without an exported primary-statement owner.
    /// This must not authorize executable lowering.
    pub unresolved: usize,
}

/// Cross-check owners against independent try/handler records and require
/// exactly one correctly typed target for each pointer-bearing action.
/// A missing owner is retained as unresolved, never inferred lexically.
pub fn validate_native_eh_action_targets(il: &Il) -> Result<EhTargetCoverage, Error> {
    let names: HashMap<_, _> = il.routine_identities.iter()
        .map(|r| (r.function.as_str(), r.routine)).collect();
    let symbols: HashMap<_, _> = il.symbols.iter().map(|s| (s.id, s.kind)).collect();
    let mut tries: HashMap<_, _> = il.native_try_handler_counts.iter()
        .map(|t| ((t.function.as_str(), t.statement_path.as_str()), t.count)).collect();
    let mut handlers: HashMap<_, _> = il.native_catch_handlers.iter()
        .map(|h| ((h.function.as_str(),h.statement_path.as_str(),h.position),h.has_initialization)).collect();
    let mut owners = HashMap::new();
    for o in &il.native_eh_owners {
        let key=(o.function.as_str(),o.statement_path.as_str());
        if names.get(key.0)!=Some(&o.routine) || o.id==0 ||
            o.initialization==Some(0) ||
            owners.insert((o.routine,o.id),o.position.is_some()).is_some() {
            return Err(Error::MalformedCallback);
        }
        match o.position {
            Some(position) if handlers.remove(&(key.0,key.1,position))==Some(o.initialization.is_some()) => {}
            None if tries.remove(&key).is_some() && o.initialization.is_none() => {}
            _=>return Err(Error::MalformedCallback),
        }
    }
    if !handlers.is_empty() || tries.values().any(|&count|count!=0) {
        return Err(Error::MalformedCallback);
    }
    let mut expected=HashMap::new();
    for a in &il.native_retained_eh_constructs {
        let kind=RetainedEhKind::from_native(a.subkind).ok_or(Error::MalformedCallback)?;
        if matches!(kind,RetainedEhKind::CaughtObjectAddress | RetainedEhKind::CatchEpilogue |
            RetainedEhKind::TryEpilogue | RetainedEhKind::FunctionPrologue | RetainedEhKind::FunctionEpilogue) &&
            expected.insert((a.function.as_str(),a.statement_path.as_str(),a.role,a.path.as_str()),kind).is_some() {
            return Err(Error::MalformedCallback);
        }
    }
    let mut coverage=EhTargetCoverage::default();
    for t in &il.native_eh_action_targets {
        let key=(t.function.as_str(),t.statement_path.as_str(),t.role,t.path.as_str());
        let kind=expected.remove(&key).ok_or(Error::MalformedCallback)?;
        if names.get(key.0)!=Some(&t.routine) || t.target==0 {return Err(Error::MalformedCallback);}
        match kind {
            RetainedEhKind::FunctionPrologue | RetainedEhKind::FunctionEpilogue => {
                if symbols.get(&crate::SymbolId(t.target))!=Some(&1) {return Err(Error::MalformedCallback);}
                coverage.routines+=1;
            }
            _=>{
                let handler=kind!=RetainedEhKind::TryEpilogue;
                match owners.get(&(t.routine,t.target)) {
                    Some(&actual) if actual!=handler=>return Err(Error::MalformedCallback),
                    Some(_) if handler=>coverage.handlers+=1,
                    Some(_)=>coverage.tries+=1,
                    None=>coverage.unresolved+=1,
                }
            }
        }
    }
    if !expected.is_empty() {return Err(Error::MalformedCallback);}
    Ok(coverage)
}

/** Source-owned match identity, not an ABI selector or a guessed RTTI name. */
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatchMatch {
    Typed { parameter: crate::SymbolId, ty: crate::TypeId, typeinfo: Option<crate::SymbolId> },
    Any,
}

/// Borrow the original handler body and owner. This is a checked frontend
/// view to drive canonical CFG lowering; it is not a second executable IR.
#[derive(Debug, Clone)]
pub struct CatchDispatchClause<'a> {
    pub declaration: &'a NativeCatchHandler,
    pub owner: &'a NativeEhOwner,
    pub body: &'a crate::Statement,
    pub matcher: CatchMatch,
}

#[derive(Debug, Clone)]
pub struct CatchDispatchRegion<'a> {
    pub owner: &'a NativeEhOwner,
    pub protected_body: &'a crate::Statement,
    /// Original EDG first-match order, including catch-all last.
    pub clauses: Vec<CatchDispatchClause<'a>>,
}

/// Join source try/catch metadata to the actual native child statement roots.
/// Never reconstruct a handler from a lexical search or a type name. The
/// bridge's explicit sibling-list witnesses certify both protected and
/// handler body edges. A missing root, wrong owner or duplicate is an error.
/// Unresolved RTTI and initializer identities are retained, not admitted
/// for target exception lowering. Work is linear in snapshot size.
pub fn checked_catch_dispatch(il: &Il) -> Result<Vec<CatchDispatchRegion<'_>>, Error> {
    validate_native_catch_handlers(il)?;
    validate_native_eh_action_targets(il)?;
    if il.native_try_handler_counts.is_empty() {return Ok(Vec::new());}
    type OwnerKey<'a> = (&'a str, &'a str, Option<u32>);
    let mut owners = HashMap::<OwnerKey<'_>, &NativeEhOwner>::new();
    let mut clauses = HashMap::new();
    for owner in &il.native_eh_owners {
        if owners.insert((&owner.function,&owner.statement_path,owner.position),owner).is_some() {
            return Err(Error::MalformedCallback);
        }
    }
    for clause in &il.native_catch_handlers {
        if clauses.insert((clause.function.as_str(),clause.statement_path.as_str(),clause.position),clause).is_some() {
            return Err(Error::MalformedCallback);
        }
    }
    let mut statements = HashMap::new();
    for statement in &il.statements {
        if statements.insert((statement.function.as_str(),statement.path.as_str()),statement).is_some() {
            return Err(Error::MalformedCallback);
        }
    }
    let mut lists = HashMap::new();
    for list in &il.native_statement_sibling_counts {
        if let Some(parent)=list.parent.as_deref() {
            if list.edge=="try" || list.edge.starts_with("catch") {
                if lists.insert((list.function.as_str(),parent,list.edge.as_str()),list.count).is_some() {
                    return Err(Error::MalformedCallback);
                }
            }
        }
    }
    let body = |function: &str, parent: &str, edge: &str| -> Result<&crate::Statement, Error> {
        if lists.get(&(function,parent,edge))!=Some(&1) {
            return Err(Error::MalformedCallback);
        }
        let path=format!("{parent}.{edge}.0");
        let statement=statements.get(&(function,path.as_str())).copied().ok_or(Error::MalformedCallback)?;
        if statement.parent.as_deref()!=Some(parent) {return Err(Error::MalformedCallback);}
        Ok(statement)
    };
    let mut regions=Vec::with_capacity(il.native_try_handler_counts.len());
    for count in &il.native_try_handler_counts {
        let function=count.function.as_str();
        let path=count.statement_path.as_str();
        if !statements.contains_key(&(function,path)) || count.count==0 {
            return Err(Error::MalformedCallback);
        }
        let owner=owners.get(&(function,path,None)).copied().ok_or(Error::MalformedCallback)?;
        let protected_body=body(function,path,"try")?;
        let mut ordered=Vec::new();
        for ordinal in 0..count.count {
            let declaration=clauses.get(&(function,path,ordinal)).copied().ok_or(Error::MalformedCallback)?;
            let clause_owner=owners.get(&(function,path,Some(ordinal))).copied().ok_or(Error::MalformedCallback)?;
            let matcher=match (declaration.parameter,declaration.parameter_type) {
                (Some(parameter),Some(ty))=>CatchMatch::Typed{parameter,ty,typeinfo:declaration.typeinfo},
                (None,None)=>CatchMatch::Any,
                _=>return Err(Error::MalformedCallback),
            };
            ordered.push(CatchDispatchClause{
                declaration,owner:clause_owner,body:body(function,path,&format!("catch{ordinal}"))?,matcher,
            });
        }
        regions.push(CatchDispatchRegion{owner,protected_body,clauses:ordered});
    }
    Ok(regions)
}
