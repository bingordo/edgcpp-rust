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
