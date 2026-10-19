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
    ffi::{c_char, c_int, c_void, CString},
    sync::Mutex,
};

static COMPILER_LOCK: Mutex<()> = Mutex::new(());

#[repr(C)]
#[derive(Clone, Copy)]
struct WireField {
    data: *const u8,
    length: usize,
    value: u64,
    tag: u32,
}
type Callback = unsafe extern "C" fn(*mut c_void, u32, *const WireField, usize, u32);
impl WireField {
    fn string(self) -> Option<String> {
        if self.tag != 1 || self.data.is_null() { return None; }
        std::str::from_utf8(unsafe { std::slice::from_raw_parts(self.data, self.length) })
            .ok().map(str::to_owned)
    }
    fn unsigned(self) -> Option<u64> { (self.tag == 2).then_some(self.value) }
    fn u32(self) -> Option<u32> { self.unsigned().and_then(|n| n.try_into().ok()) }
    fn signed(self) -> Option<i64> { (self.tag == 3).then_some(self.value as i64) }
    fn present(self) -> bool { self.tag != 0 }
}

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
    pub symbols: Vec<Symbol>,
    pub variable_initializers: Vec<VariableInitializer>,
    pub initializer_integers: Vec<InitializerInteger>,
}

/// Per-compilation identity for a declaration or referenced symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub id: SymbolId,
    /// 1=routine, 2=variable, 3=field.
    pub kind: u32,
    pub name: String,
}

/// Native EDG initialization classification for a variable declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableInitializer {
    pub symbol: SymbolId,
    pub kind: u32,
    pub explicit: bool,
    pub uninitialized: bool,
}

/// A scalar static-initializer integer that fits in i64.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitializerInteger {
    pub symbol: SymbolId,
    pub value: i64,
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
    pub symbol_id: Option<SymbolId>,
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
            // Ignore namespace delimiters inside function parameter types.
            let prefix_end = qualified.find('(').unwrap_or(qualified.len());
            let prefix = &qualified[..prefix_end];
            let parts: Vec<&str> = prefix.split("::").collect();
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
                let local_prefix = parts.last().copied().unwrap_or("");
                let local = &qualified[prefix_end - local_prefix.len()..];
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
    MalformedCallback,
}

struct Collector {
    il: Il,
    panicked: bool,
    malformed: bool,
}

/// Never allow a Rust panic to cross the C ABI into EDG.
/// Native callbacks are synchronous and serialized by COMPILER_LOCK.
unsafe extern "C" fn collect(context: *mut c_void, kind: u32,
                              name: *const WireField, length: usize, defined: u32) {
    if context.is_null() { return; }
    if name.is_null() && length != 0 { return; }
    let state = &mut *(context as *mut Collector);
    if state.panicked || state.malformed { return; }
    let before = callback_record_count(&state.il, kind);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        collect_event(&mut state.il, kind, name, length, defined);
    }));
    if result.is_err() {
        state.panicked = true;
    } else if let Some(before) = before {
        if callback_record_count(&state.il, kind) == Some(before) {
            state.malformed = true;
        }
    }
}

fn callback_record_count(il: &Il, kind: u32) -> Option<usize> {
    match kind {
        1..=4 => Some(il.entities.len()),
        5 => Some(il.statements.len()),
        6 => Some(il.expressions.len()),
        7 => Some(il.types.len()),
        8 => Some(il.type_parameters.len()),
        9 => Some(il.array_bounds.len()),
        10 => Some(il.record_fields.len()),
        11 => Some(il.type_details.len()),
        12 => Some(il.function_signatures.len()),
        13 => Some(il.symbols.len()),
        14 => Some(il.variable_initializers.len()),
        15 => Some(il.initializer_integers.len()),
        _ => None,
    }
}

unsafe fn collect_event(output: &mut Il, kind: u32,
                        data: *const WireField, length: usize, aux: u32) {
    let fields = if length == 0 { &[][..] } else {
        if data.is_null() { return; }
        std::slice::from_raw_parts(data, length)
    };
    let expected = match kind {
        1..=4 => 1, 5 => 3, 6 => 15, 7 => 4, 8 => 3,
        9 => 2, 10 => 7, 11 => 3, 12 => 3, 13 => 3, 14 => 4, 15 => 2, _ => return,
    };
    if length != expected || fields.iter().any(|f|
        f.tag > 3 || (f.tag == 1 && f.data.is_null())) { return; }
    let t = |i: usize| fields[i].string();
    let n = |i: usize| fields[i].u32();
    match kind {
        1..=4 => {
            let Some(name) = t(0) else { return; };
            output.entities.push(match kind {
                1 => Entity::Routine { name, has_definition: aux != 0 },
                2 => Entity::Variable { name },
                3 => Entity::Type { name },
                _ => Entity::Namespace { name },
            });
        }
        5 => {
            let (Some(function), Some(path)) = (t(0), t(1)) else { return; };
            let parent = if fields[2].present() { let Some(v) = t(2) else { return; }; Some(v) } else { None };
            output.statements.push(Statement { function, path, parent, kind: aux });
        }
        6 => {
            let (Some(function), Some(statement_path), Some(path)) = (t(0), t(1), t(2)) else { return; };
            let opt_text = |i: usize| if fields[i].present() { t(i) } else { Some(String::new()) };
            let (Some(parent), Some(reference)) = (opt_text(3), opt_text(6)) else { return; };
            output.expressions.push(Expression {
                function, statement_path, path,
                parent: fields[3].present().then_some(parent),
                kind: aux, operator: n(4), type_kind: n(5),
                reference: fields[6].present().then_some(reference),
                type_size: fields[7].unsigned(),
                type_alignment: fields[8].unsigned(),
                constant_kind: n(9), constant_integer: fields[10].signed(),
                related_type_role: n(11), related_type_kind: n(12),
                type_id: n(13).map(TypeId), symbol_id: n(14).map(SymbolId),
            });
        }
        7 => {
            let (Some(id), Some(kind), Some(role), Some(child)) = (n(0), n(1), n(2), n(3)) else { return; };
            output.types.push(TypeDef { id: TypeId(id), kind,
                related_role: (role != 0).then_some(role),
                related: (child != 0).then_some(TypeId(child)) });
        }
        8 => {
            let (Some(owner), Some(position), Some(ty)) = (n(0), n(1), n(2)) else { return; };
            output.type_parameters.push(TypeParameter { owner: TypeId(owner), position, ty: TypeId(ty) });
        }
        9 => {
            let (Some(id), Some(elements)) = (n(0), fields[1].unsigned()) else { return; };
            output.array_bounds.push(ArrayBound { array: TypeId(id), elements });
        }
        10 => {
            let (Some(owner), Some(position), Some(ty), Some(name),
                 Some(byte_offset), Some(bit_offset), Some(bit_width)) =
                (n(0), n(1), n(2), t(3), fields[4].unsigned(), n(5), n(6)) else { return; };
            output.record_fields.push(RecordField {
                owner: TypeId(owner), position, ty: TypeId(ty), name,
                byte_offset, bit_offset, bit_width });
        }
        11 => {
            let (Some(owner), Some(flags)) = (n(0), n(1)) else { return; };
            output.type_details.push(TypeDetail {
                owner: TypeId(owner), is_reference: flags & 1 != 0,
                is_rvalue_reference: flags & 2 != 0, qualifiers: n(2) });
        }
        12 => {
            let (Some(owner), Some(prototyped), Some(variadic)) = (n(0), n(1), n(2)) else { return; };
            output.function_signatures.push(FunctionSignature {
                owner: TypeId(owner), prototyped: prototyped != 0, variadic: variadic != 0 });
        }
        15 => {
            let (Some(symbol), Some(value)) = (n(0), fields[1].signed()) else { return; };
            output.initializer_integers.push(InitializerInteger {
                symbol: SymbolId(symbol), value,
            });
        }
        14 => {
            let (Some(symbol), Some(kind), Some(explicit), Some(uninitialized)) =
                (n(0), n(1), n(2), n(3)) else { return; };
            output.variable_initializers.push(VariableInitializer {
                symbol: SymbolId(symbol), kind, explicit: explicit != 0,
                uninitialized: uninitialized != 0,
            });
        }
        13 => {
            let (Some(id), Some(kind), Some(name)) = (n(0), n(1), t(2)) else { return; };
            output.symbols.push(Symbol { id: SymbolId(id), kind, name });
        }
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
    let mut collector = Collector { il: Il::default(), panicked: false, malformed: false };
    let code = unsafe {
        edgcpp_compile(cstrings.len() as c_int, argv.as_mut_ptr(),
                       Some(collect), &mut collector as *mut Collector as *mut c_void)
    };
    if collector.panicked { return Err(Error::CallbackPanicked); }
    if collector.malformed { return Err(Error::MalformedCallback); }
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
        ], statements: vec![], expressions: vec![], types: vec![], type_parameters: vec![], array_bounds: vec![], record_fields: vec![], type_details: vec![], function_signatures: vec![], symbols: vec![], variable_initializers: vec![], initializer_integers: vec![] };
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
    fn qualified_parameters_do_not_create_namespaces() {
        let mut il = Il::default();
        il.entities.push(Entity::Routine {
            name: "outer::f(other::Widget)".into(), has_definition: true,
        });
        let tree = il.scope_tree();
        let outer = tree.namespace(&["outer"]).unwrap();
        assert!(outer.declarations.contains(&Entity::Routine {
            name: "f(other::Widget)".into(), has_definition: true,
        }));
        assert!(tree.namespace(&["outer", "f(other"]).is_none());
    }

    #[test]
    fn expression_operands_are_ordered_and_owned() {
        let mut il = Il::default();
        il.expressions.push(Expression {
            function: "f".into(), statement_path: "0".into(),
            path: "0".into(), parent: None, kind: 1,
            operator: Some(12), type_kind: Some(2), reference: None, type_size: Some(4), type_alignment: Some(4), constant_kind: None, constant_integer: None, related_type_role: None, related_type_kind: None, type_id: None, symbol_id: None,
        });
        il.expressions.push(Expression {
            function: "f".into(), statement_path: "0".into(),
            path: "0.0".into(), parent: Some("0".into()), kind: 2,
            operator: None, type_kind: Some(2), reference: Some("x".into()), type_size: Some(4), type_alignment: Some(4), constant_kind: None, constant_integer: None, related_type_role: None, related_type_kind: None, type_id: None, symbol_id: None,
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
    fn wire_field_layout_matches_native() {
        assert_eq!(std::mem::size_of::<WireField>(), 32);
        assert_eq!(std::mem::align_of::<WireField>(), 8);
    }

    #[test]
    fn typed_callback_preserves_tabs_and_nul_bytes() {
        let name = b"outer::a\tb\0c";
        let fields = [WireField { data: name.as_ptr(), length: name.len(), value: 0, tag: 1 }];
        let mut state = Collector { il: Il::default(), panicked: false, malformed: false };
        unsafe { collect((&mut state as *mut Collector).cast(), 2, fields.as_ptr(), 1, 0); }
        assert_eq!(state.il.entities, vec![Entity::Variable {
            name: String::from_utf8(name.to_vec()).unwrap()
        }]);
        assert!(!state.malformed);
    }

    #[test]
    fn typed_callback_decodes_numeric_and_optional_fields() {
        let text = b"f";
        let path = b"0";
        let string = |bytes: &[u8]| WireField {
            data: bytes.as_ptr(), length: bytes.len(), value: 0, tag: 1,
        };
        let number = |value| WireField {
            data: std::ptr::null(), length: 0, value, tag: 2,
        };
        let absent = WireField {
            data: std::ptr::null(), length: 0, value: 0, tag: 0,
        };
        let mut state = Collector {
            il: Il::default(), panicked: false, malformed: false,
        };
        let statement = [string(text), string(path), absent];
        unsafe {
            collect((&mut state as *mut Collector).cast(), 5,
                    statement.as_ptr(), statement.len(), 8);
        }
        let expression = [
            string(text), string(path), string(path), absent,
            absent, number(2), absent, number(4), number(4),
            number(1), WireField {
                data: std::ptr::null(), length: 0,
                value: (-9_i64) as u64, tag: 3,
            },
            absent, absent, absent, absent,
        ];
        unsafe {
            collect((&mut state as *mut Collector).cast(), 6,
                    expression.as_ptr(), expression.len(), 2);
        }
        assert!(!state.malformed);
        assert_eq!(state.il.statements.len(), 1);
        assert_eq!(state.il.statements[0].parent, None);
        assert_eq!(state.il.expressions.len(), 1);
        assert_eq!(state.il.expressions[0].constant_integer, Some(-9));
        assert_eq!(state.il.expressions[0].type_size, Some(4));
        assert_eq!(state.il.expressions[0].symbol_id, None);
    }

    #[test]
    fn invalid_utf8_callback_is_reported() {
        let bad = [0xff_u8];
        let fields = [WireField { data: bad.as_ptr(), length: 1, value: 0, tag: 1 }];
        let mut state = Collector { il: Il::default(), panicked: false, malformed: false };
        unsafe { collect((&mut state as *mut Collector).cast(), 1, fields.as_ptr(), 1, 0); }
        assert!(state.malformed);
    }

    #[test]
    fn malformed_callback_is_reported() {
        let mut state = Collector { il: Il::default(), panicked: false, malformed: false };
        unsafe { collect((&mut state as *mut Collector).cast(), 7, std::ptr::null(), 0, 0); }
        assert!(state.malformed);
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
