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
    /// Identity-preserving types captured during this compilation.
    pub types: Vec<TypeDef>,
    /// Ordered parameter edges belonging to routine types.
    pub type_parameters: Vec<TypeParameter>,
    /// Known fixed array bounds (in elements).
    pub array_bounds: Vec<ArrayBound>,
    pub record_fields: Vec<RecordField>,
    pub type_details: Vec<TypeDetail>,
    pub function_signatures: Vec<FunctionSignature>,
}

/// Stable type identifier within one compilation. Zero means no type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(pub u32);

/// A type and its direct child relationship, using native EDG kind tags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDef {
    pub id: TypeId,
    pub kind: u32,
    pub related_role: Option<u32>,
    pub related: Option<TypeId>,
}

/// Parameter edge from a routine type, in declaration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeParameter {
    pub owner: TypeId,
    pub position: u32,
    pub ty: TypeId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayBound {
    pub array: TypeId,
    pub elements: u64,
}

/// Ordered nonstatic field of an EDG record type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordField {
    pub owner: TypeId,
    pub position: u32,
    pub ty: TypeId,
    pub name: String,
    pub byte_offset: u64,
    pub bit_offset: u32,
    /// Zero for ordinary fields; nonzero for bit-fields.
    pub bit_width: u32,
}

/// Pointer reference flags and typeref qualifier bitmask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDetail {
    pub owner: TypeId,
    pub is_reference: bool,
    pub is_rvalue_reference: bool,
    pub qualifiers: Option<u32>,
}

/// Prototype and variadic metadata for a routine type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSignature {
    pub owner: TypeId,
    pub prototyped: bool,
    pub variadic: bool,
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
    /// Relationship to a direct child type (1=pointee, 2=array element, 3=function return).
    pub related_type_role: Option<u32>,
    pub related_type_kind: Option<u32>,
    pub type_id: Option<TypeId>,
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
    CallbackPanicked,
    EmptyArguments,
}

struct Collector {
    il: Il,
    panicked: bool,
}

/// Never allow a Rust panic to cross the C ABI into EDG.
/// Native callbacks are synchronous and serialized by COMPILER_LOCK.
unsafe extern "C" fn collect(context: *mut c_void, kind: u32,
                              name: *const c_char, defined: u32) {
    if context.is_null() || name.is_null() { return; }
    let state = &mut *(context as *mut Collector);
    if state.panicked { return; }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        collect_event(&mut state.il, kind, name, defined);
    }));
    if result.is_err() { state.panicked = true; }
}

unsafe fn collect_event(output: &mut Il, kind: u32,
                        name: *const c_char, defined: u32) {
    let name = CStr::from_ptr(name).to_string_lossy().into_owned();
    match kind {
        1 => output.entities.push(Entity::Routine {
            name, has_definition: defined != 0,
        }),
        2 => output.entities.push(Entity::Variable { name }),
        3 => output.entities.push(Entity::Type { name }),
        4 => output.entities.push(Entity::Namespace { name }),
        6 => {
            let fields: Vec<_> = name.splitn(14, '\t').collect();
            if fields.len() == 14 {
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
                    related_type_role: fields[11].parse().ok().filter(|v| *v != 0),
                    related_type_kind: fields[12].parse().ok(),
                    type_id: fields[13].parse::<u32>().ok().filter(|v| *v != 0).map(TypeId),
                });
            }
        },
        12 => {
            let f: Vec<_> = name.splitn(3, '\t').collect();
            if f.len() == 3 {
                if let (Ok(owner), Ok(prototyped), Ok(variadic)) =
                    (f[0].parse(), f[1].parse::<u32>(), f[2].parse::<u32>()) {
                    output.function_signatures.push(FunctionSignature {
                        owner: TypeId(owner), prototyped: prototyped != 0, variadic: variadic != 0,
                    });
                }
            }
        },
        11 => {
            let f: Vec<_> = name.splitn(3, '\t').collect();
            if f.len() == 3 {
                if let (Ok(id), Ok(flags)) = (f[0].parse(), f[1].parse::<u32>()) {
                    output.type_details.push(TypeDetail {
                        owner: TypeId(id),
                        is_reference: flags & 1 != 0,
                        is_rvalue_reference: flags & 2 != 0,
                        qualifiers: f[2].parse().ok(),
                    });
                }
            }
        },
        10 => {
            let f: Vec<_> = name.splitn(7, '\t').collect();
            if f.len() == 7 {
                if let (Ok(owner), Ok(position), Ok(ty)) =
                    (f[0].parse(), f[1].parse(), f[2].parse()) {
                    if let (Ok(byte_offset), Ok(bit_offset), Ok(bit_width)) =
                        (f[4].parse(), f[5].parse(), f[6].parse()) {
                        output.record_fields.push(RecordField {
                            owner: TypeId(owner), position, ty: TypeId(ty),
                            name: f[3].into(), byte_offset, bit_offset, bit_width,
                        });
                    }
                }
            }
        },
        9 => {
            let mut fields = name.splitn(2, '\t');
            if let (Some(id), Some(count)) = (fields.next(), fields.next()) {
                if let (Ok(id), Ok(elements)) = (id.parse(), count.parse()) {
                    output.array_bounds.push(ArrayBound { array: TypeId(id), elements });
                }
            }
        },
        8 => {
            let f: Vec<_> = name.splitn(3, '\t').collect();
            if f.len() == 3 {
                if let (Ok(owner), Ok(position), Ok(ty)) =
                    (f[0].parse(), f[1].parse(), f[2].parse()) {
                    output.type_parameters.push(TypeParameter {
                        owner: TypeId(owner), position, ty: TypeId(ty),
                    });
                }
            }
        },
        7 => {
            let fields: Vec<_> = name.splitn(4, '\t').collect();
            if fields.len() == 4 {
                if let (Ok(id), Ok(kind), Ok(role), Ok(child)) =
                    (fields[0].parse(), fields[1].parse(), fields[2].parse::<u32>(), fields[3].parse::<u32>()) {
                    output.types.push(TypeDef {
                        id: TypeId(id), kind,
                        related_role: if role == 0 { None } else { Some(role) },
                        related: if child == 0 { None } else { Some(TypeId(child)) },
                    });
                }
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
    if args.is_empty() { return Err(Error::EmptyArguments); }
    if args.len() > c_int::MAX as usize {
        return Err(Error::TooManyArguments);
    }
    let cstrings: Vec<CString> = args.iter()
        .map(|v| CString::new(v.as_ref()).map_err(|_| Error::NulArgument))
        .collect::<Result<_, _>>()?;
    let mut argv: Vec<*mut c_char> =
        cstrings.iter().map(|s| s.as_ptr() as *mut c_char).collect();
    argv.push(std::ptr::null_mut());
    let mut collector = Collector { il: Il::default(), panicked: false };
    let code = unsafe {
        edgcpp_compile(cstrings.len() as c_int, argv.as_mut_ptr(),
                       Some(collect), &mut collector as *mut Collector as *mut c_void)
    };
    if collector.panicked { return Err(Error::CallbackPanicked); }
    if code < 0 {
        return Err(Error::NativeFailure);
    }
    Ok(Compilation { status: code, il: collector.il })
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
        ], statements: vec![], expressions: vec![], types: vec![], type_parameters: vec![], array_bounds: vec![], record_fields: vec![], type_details: vec![], function_signatures: vec![] };
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
            operator: Some(12), type_kind: Some(2), reference: None, type_size: Some(4), type_alignment: Some(4), constant_kind: None, constant_integer: None, related_type_role: None, related_type_kind: None, type_id: None,
        });
        il.expressions.push(Expression {
            function: "f".into(), statement_path: "0".into(),
            path: "0.0".into(), parent: Some("0".into()), kind: 2,
            operator: None, type_kind: Some(2), reference: Some("x".into()), type_size: Some(4), type_alignment: Some(4), constant_kind: None, constant_integer: None, related_type_role: None, related_type_kind: None, type_id: None,
        });
        assert_eq!(il.expressions[1].parent.as_deref(), Some("0"));
        assert_eq!(il.expressions[1].path, "0.0");
    }

    #[test]
    fn type_id_links_are_owned() {
        let mut il = Il::default();
        il.types.push(TypeDef { id: TypeId(1), kind: 5,
            related_role: Some(1), related: Some(TypeId(2)) });
        il.types.push(TypeDef { id: TypeId(2), kind: 2,
            related_role: None, related: None });
        assert_eq!(il.types[0].related, Some(il.types[1].id));
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
    fn rejects_empty_argv() {
        assert_eq!(compile(&[] as &[&str]).unwrap_err(), Error::EmptyArguments);
    }

    #[test]
    fn rejects_interior_nul() {
        assert_eq!(compile(&["edgcpfe", "a\0b"]).unwrap_err(), Error::NulArgument);
    }
}
