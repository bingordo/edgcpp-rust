//! Callable EDG front end and an owned, first-level view of its IL.
//!
//! This is a **Linux x86-64** C++ ABI bridge. Both sides must be built from
//! matching EDG sources and configuration (see README).
//!
//! The IL snapshot contains named functions, variables, types, and namespaces. The
//! native IL is far richer; this crate intentionally does not expose dangling
//! raw EDG pointers after `edg_main` returns.
pub mod graph;

use std::{
    ffi::{c_char, c_int, c_void, CStr, CString},
    sync::Mutex,
};

static COMPILER_LOCK: Mutex<()> = Mutex::new(());

type Callback = unsafe extern "C" fn(*mut c_void, u32, *const c_char, u32);

extern "C" {
    fn edgcpp_compile(
        argc: c_int,
        argv: *mut *mut c_char,
        callback: Option<Callback>,
        context: *mut c_void,
    ) -> c_int;
}

/// A copied IL entity. It remains valid after the native compiler cleans up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entity {
    Routine { name: String, has_definition: bool },
    Variable { name: String },
    Type { name: String },
    Namespace { name: String },
}

/// Owned snapshot from a compilation. Names use C++ namespace qualification.
#[derive(Debug, Clone, Default)]
pub struct Il {
    pub entities: Vec<Entity>,
    /// Structural statement records captured while function IL is live.
    pub statements: Vec<Statement>,
    /// Owned expression nodes, including operation operand relationships.
    pub expressions: Vec<Expression>,
}

/// A statement node, identified by a stable path within one function.
/// Kinds are EDG's a_statement_kind discriminants, not Rust syntax kinds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    pub function: String,
    pub path: String,
    pub parent: Option<String>,
    pub kind: u32,
}

/// A snapshot of an EDG expression and its ordered operand tree.
/// Discriminants are native EDG enums; consumers must use the matching EDG build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expression {
    pub function: String,
    pub statement_path: String,
    pub path: String,
    pub parent: Option<String>,
    pub kind: u32,
    pub operator: Option<u32>,
    pub type_kind: Option<u32>,
    /// Qualified source name for variable/routine references or field name.
    pub reference: Option<String>,
    /// Target layout in bytes, omitted for incomplete or zero-sized types.
    pub type_size: Option<u64>,
    pub type_alignment: Option<u64>,
    /// EDG constant representation kind, when this is a constant expression.
    pub constant_kind: Option<u32>,
    /// Signed integer value if representable as i64 (otherwise absent).
    pub constant_integer: Option<i64>,
}

/// Owned namespace tree. Declarations are grouped by the namespace in
/// which they occur; all strings and nodes are independent of EDG memory.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scope {
    pub name: String,
    pub declarations: Vec<Entity>,
    pub namespaces: Vec<Scope>,
}

impl Scope {
    /// Look up a nested namespace by its components.
    pub fn namespace(&self, path: &[&str]) -> Option<&Scope> {
        let mut scope = self;
        for part in path {
            scope = scope.namespaces.iter().find(|n| n.name == *part)?;
        }
        Some(scope)
    }
}

impl Il {
    /// Reconstruct a structured namespace tree from the snapshot.
    ///
    /// Names are currently qualified by C++ namespace, not by classes.
    pub fn scope_tree(&self) -> Scope {
        let mut root = Scope::default();
        for entity in &self.entities {
            let (qualified, is_namespace) = match entity {
                Entity::Routine { name, .. } | Entity::Variable { name }
                | Entity::Type { name } => (name.as_str(), false),
                Entity::Namespace { name } => (name.as_str(), true),
            };
            let parts: Vec<&str> = qualified.split("::").collect();
            let mut scope = &mut root;
            let limit = if is_namespace { parts.len() } else { parts.len().saturating_sub(1) };
            for part in &parts[..limit] {
                let index = match scope.namespaces.iter().position(|n| n.name == *part) {
                    Some(index) => index,
                    None => {
                        scope.namespaces.push(Scope { name: (*part).into(), ..Scope::default() });
                        scope.namespaces.len() - 1
                    }
                };
                scope = &mut scope.namespaces[index];
            }
            if !is_namespace {
                let local = parts.last().copied().unwrap_or("");
                let record = match entity {
                    Entity::Routine { has_definition, .. } => Entity::Routine {
                        name: local.into(), has_definition: *has_definition,
                    },
                    Entity::Variable { .. } => Entity::Variable { name: local.into() },
                    Entity::Type { .. } => Entity::Type { name: local.into() },
                    Entity::Namespace { .. } => unreachable!(),
                };
                scope.declarations.push(record);
            }
        }
        root
    }
}

/// Result of invoking the EDG front end.
#[derive(Debug, Clone)]
pub struct Compilation {
    /// EDG exit status; nonzero normally represents diagnostics or failure.
    pub status: i32,
    /// Populated when EDG reaches its back-end callback.
    pub il: Il,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    NulArgument,
    TooManyArguments,
    NativeFailure,
}

unsafe extern "C" fn collect(context: *mut c_void, kind: u32,
                              name: *const c_char, defined: u32) {
    // The callback is invoked synchronously from the bridge.
    if context.is_null() || name.is_null() {
        return;
    }
    let output = &mut *(context as *mut Il);
    let name = CStr::from_ptr(name).to_string_lossy().into_owned();
    match kind {
        1 => output.entities.push(Entity::Routine {
            name, has_definition: defined != 0,
        }),
        2 => output.entities.push(Entity::Variable { name }),
        3 => output.entities.push(Entity::Type { name }),
        4 => output.entities.push(Entity::Namespace { name }),
        6 => {
            let fields: Vec<_> = name.splitn(11, '\t').collect();
            if fields.len() == 11 {
                output.expressions.push(Expression {
                    function: fields[0].into(),
                    statement_path: fields[1].into(),
                    path: fields[2].into(),
                    parent: if fields[3].is_empty() { None } else { Some(fields[3].into()) },
                    kind: defined,
                    operator: fields[4].parse().ok().filter(|_| defined == 1),
                    type_kind: fields[5].parse().ok(),
                    reference: if fields[6].is_empty() { None } else { Some(fields[6].into()) },
                    type_size: fields[7].parse().ok(),
                    type_alignment: fields[8].parse().ok(),
                    constant_kind: fields[9].parse().ok(),
                    constant_integer: fields[10].parse().ok(),
                });
            }
        },
        5 => {
            let mut parts = name.splitn(3, '\t');
            if let (Some(function), Some(path), Some(parent)) =
                (parts.next(), parts.next(), parts.next()) {
                output.statements.push(Statement {
                    function: function.into(), path: path.into(),
                    parent: if parent.is_empty() { None } else { Some(parent.into()) },
                    kind: defined,
                });
            }
        },
        _ => {}
    }
}

/// Execute the front end with the supplied command-line arguments.
///
/// Include argv[0] (usually `"edgcpfe"`) as the first string, then EDG
/// options and the source filename. The call is serialized because EDG's
/// compilation state is global. File-scope IL is copied before EDG cleanup.
///
/// The caller must ensure that the EDG target configuration and input paths
/// are appropriate for the selected EDG command-line options.
pub fn compile<S: AsRef<str>>(args: &[S]) -> Result<Compilation, Error> {
    let _lock = COMPILER_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    if args.len() > c_int::MAX as usize {
        return Err(Error::TooManyArguments);
    }
    let cstrings: Vec<CString> = args.iter()
        .map(|v| CString::new(v.as_ref()).map_err(|_| Error::NulArgument))
        .collect::<Result<_, _>>()?;
    let mut argv: Vec<*mut c_char> =
        cstrings.iter().map(|s| s.as_ptr() as *mut c_char).collect();
    argv.push(std::ptr::null_mut());
    let mut il = Il::default();
    let code = unsafe {
        edgcpp_compile(cstrings.len() as c_int, argv.as_mut_ptr(),
                       Some(collect), &mut il as *mut Il as *mut c_void)
    };
    if code < 0 {
        return Err(Error::NativeFailure);
    }
    Ok(Compilation { status: code, il })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn builds_owned_namespace_tree() {
        let il = Il { entities: vec![
            Entity::Namespace { name: "outer".into() },
            Entity::Namespace { name: "outer::inner".into() },
            Entity::Variable { name: "outer::inner::value".into() },
            Entity::Routine { name: "outer::compute".into(), has_definition: true },
        ], statements: vec![], expressions: vec![] };
        let root = il.scope_tree();
        let outer = root.namespace(&["outer"]).unwrap();
        assert!(outer.declarations.contains(&Entity::Routine {
            name: "compute".into(), has_definition: true,
        }));
        let inner = root.namespace(&["outer", "inner"]).unwrap();
        assert_eq!(inner.declarations, vec![Entity::Variable { name: "value".into() }]);
        assert!(root.namespace(&["absent"]).is_none());
    }

    #[test]
    fn expression_operands_are_ordered_and_owned() {
        let mut il = Il::default();
        il.expressions.push(Expression {
            function: "f".into(), statement_path: "0".into(),
            path: "0".into(), parent: None, kind: 1,
            operator: Some(12), type_kind: Some(2), reference: None, type_size: Some(4), type_alignment: Some(4), constant_kind: None, constant_integer: None,
        });
        il.expressions.push(Expression {
            function: "f".into(), statement_path: "0".into(),
            path: "0.0".into(), parent: Some("0".into()), kind: 2,
            operator: None, type_kind: Some(2), reference: Some("x".into()), type_size: Some(4), type_alignment: Some(4), constant_kind: None,
        });
        assert_eq!(il.expressions[1].parent.as_deref(), Some("0"));
        assert_eq!(il.expressions[1].path, "0.0");
    }

    #[test]
    fn statement_paths_preserve_parent_relation() {
        let mut il = Il::default();
        il.statements.push(Statement {
            function: "f".into(), path: "0.0".into(),
            parent: Some("0".into()), kind: 0,
        });
        assert_eq!(il.statements[0].parent.as_deref(), Some("0"));
    }

    #[test]
    fn rejects_interior_nul() {
        assert_eq!(compile(&["edgcpfe", "a\0b"]).unwrap_err(), Error::NulArgument);
    }
}
