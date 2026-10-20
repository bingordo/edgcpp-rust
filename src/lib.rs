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
    /// Native flags retained for each emitted statement.
    pub statement_attributes: Vec<StatementAttributes>,
    /// Additional expressions on structured controls (e.g. for increments).
    pub control_expressions: Vec<ControlExpression>,
    /// References and definitions of native goto labels.
    pub statement_labels: Vec<StatementLabel>,
    /// Variable declaration positions within structured statements.
    pub statement_declarations: Vec<StatementDeclaration>,
    /// Dynamic initialization execution sites with optional expression trees.
    pub statement_initializations: Vec<StatementInitialization>,
    pub statement_sources: Vec<StatementSource>,
    pub symbol_sources: Vec<SymbolSource>,
    pub expression_sources: Vec<ExpressionSource>,
    pub object_lifetimes: Vec<ObjectLifetime>,
    pub scope_lifetimes: Vec<ScopeLifetime>,
    pub initialization_lifetimes: Vec<InitializationLifetime>,
    pub lifetime_destructions: Vec<LifetimeDestruction>,
    /// Defined routine parameters, including an optional implicit this.
    pub function_parameters: Vec<FunctionParameter>,
    /// Lexical scope tree for routine bodies.
    pub local_scopes: Vec<LocalScope>,
    /// Typed variables declared in lexical scopes.
    pub local_variables: Vec<LocalVariable>,
    /// Function ownership recovered for hoisted static local variables.
    pub static_local_owners: Vec<StaticLocalOwner>,
    /// Owned expression nodes, including operation operand relationships.
    pub expressions: Vec<Expression>,
    /// Identity-preserving types captured during this compilation.
    pub types: Vec<TypeDef>,
    /// Ordered parameter edges belonging to routine types.
    pub type_parameters: Vec<TypeParameter>,
    /// Known fixed array bounds (in elements).
    pub array_bounds: Vec<ArrayBound>,
    pub record_fields: Vec<RecordField>,
    /// Direct class bases in declaration order.
    pub base_classes: Vec<BaseClass>,
    /// Direct-base access specifiers keyed by derived type and position.
    pub base_class_access: Vec<BaseClassAccess>,
    /// Constructor and destructor subobject initialization records.
    pub constructor_initializers: Vec<ConstructorInitializer>,
    /// Owned expression trees attached to constructor initializer entries.
    pub constructor_initializer_expressions: Vec<ConstructorInitializerExpression>,
    /// Class ownership edges for declared member functions.
    pub member_functions: Vec<MemberFunction>,
    /// Special-function and virtual dispatch attributes of routines.
    pub routine_attributes: Vec<RoutineAttributes>,
    pub type_details: Vec<TypeDetail>,
    pub function_signatures: Vec<FunctionSignature>,
    pub symbols: Vec<Symbol>,
    /// Type edges for named declarations.
    pub symbol_types: Vec<SymbolType>,
    pub variable_initializers: Vec<VariableInitializer>,
    pub initializer_integers: Vec<InitializerInteger>,
    pub dynamic_initializers: Vec<DynamicInitializer>,
    pub initializer_expressions: Vec<InitializerExpression>,
    /// Call sites with the operand index of the first explicit argument.
    pub calls: Vec<CallSite>,
}

/// Per-compilation identity for a declaration or referenced symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub id: SymbolId,
    /// 1=routine, 2=variable, 3=field, 4=goto label.
    pub kind: u32,
    pub name: String,
}

/// Declared type of a routine, variable, or other symbol.
/// Both IDs are valid only within the current compilation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolType {
    pub symbol: SymbolId,
    pub ty: TypeId,
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

/// EDG dynamic initializer category and root expression classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicInitializer {
    pub symbol: SymbolId,
    pub kind: u32,
    pub expression_kind: Option<u32>,
    pub operator: Option<u32>,
}

/// One expression node in a dynamic initializer's operand tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitializerExpression {
    pub owner: SymbolId,
    pub path: String,
    pub parent: Option<String>,
    pub kind: u32,
    pub operator: Option<u32>,
    pub type_id: Option<TypeId>,
    pub symbol_id: Option<SymbolId>,
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

/// A direct C++ base class, in declaration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseClass {
    pub derived: TypeId,
    pub position: u32,
    pub base: TypeId,
    pub is_virtual: bool,
}

/// Native direct-base access from EDG's selected direct derivation path.
/// Access discriminants are EDG-native; None indicates no path was retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseClassAccess {
    pub derived: TypeId,
    pub position: u32,
    pub access: Option<u32>,
    pub pack_expansion: bool,
}

/// One constructor/destructor subobject initializer in execution order.
///
/// Native EDG kind: 0=virtual base, 1=direct base, 2=field, 3=delegation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructorInitializer {
    pub routine: SymbolId,
    pub position: u32,
    pub kind: u32,
    pub target_type: Option<TypeId>,
    pub target_field: Option<SymbolId>,
    pub compiler_generated: bool,
    pub braced: bool,
    pub use_field_initializer: bool,
    pub pack_expansion: bool,
}

/// Expression tree belonging to one constructor/destructor subobject
/// initializer. Paths are local to (routine, position).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructorInitializerExpression {
    pub routine: SymbolId,
    pub position: u32,
    pub path: String,
    pub parent: Option<String>,
    pub kind: u32,
    pub operator: Option<u32>,
    pub type_id: Option<TypeId>,
    pub symbol_id: Option<SymbolId>,
}

/// Member function belonging to a class, struct or union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberFunction {
    pub owner: TypeId,
    pub routine: SymbolId,
}

/// EDG special-function classification and dispatch metadata for a routine.
/// Special-kind discriminants are native EDG values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutineAttributes {
    pub routine: SymbolId,
    pub special_kind: u32,
    pub is_virtual: bool,
    pub pure_virtual: bool,
    pub is_override: bool,
    pub is_final: bool,
    pub overrides_base_member: bool,
    pub is_inline: bool,
    pub is_constexpr: bool,
    pub defined: bool,
    pub explicit_constructor: bool,
    pub has_this_parameter: bool,
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

/// Properties of a structured statement retained after EDG lowering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementAttributes {
    pub function: String,
    pub statement_path: String,
    pub compiler_generated: bool,
    pub initialization_guard: bool,
    pub fallthrough: bool,
    pub likely: bool,
    pub unlikely: bool,
    pub lowering_generated: bool,
    pub lowering_boilerplate: bool,
}

/// Expressions stored outside a statement's primary `expr` field.
/// Roles: 1=for increment, 2=range-for comparison, 3=range-for increment,
/// 4=dynamic initializer expression executed at a statement.
/// Their paths are local to (function, statement_path, role).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlExpression {
    pub function: String,
    pub statement_path: String,
    pub role: u32,
    pub path: String,
    pub parent: Option<String>,
    pub kind: u32,
    pub operator: Option<u32>,
    pub type_id: Option<TypeId>,
    pub symbol_id: Option<SymbolId>,
}

/// A variable declared by a particular statement. More than one variable
/// can share the same statement, in source-list order. The `is_vla` flag
/// identifies a stmk_vla_decl allocation/declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementDeclaration {
    pub function: String,
    pub statement_path: String,
    pub position: u32,
    pub symbol: SymbolId,
    pub ty: TypeId,
    pub is_vla: bool,
}

/// A dynamic initializer executed at a statement (rather than merely
/// attached to a declaration). Native EDG kind is retained.
/// Role 0=stmk_init, 1=return, 2=statement-expression result.
/// A representable expression is available as control role 4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementInitialization {
    pub function: String,
    pub statement_path: String,
    pub symbol: Option<SymbolId>,
    pub kind: u32,
    pub follows_execution: bool,
    pub braced: bool,
    pub role: u32,
}

/// Physical/diagnostic source location resolved from EDG's sequence table.
/// Sequence and column are compilation-local; file and line reflect
/// EDG's #line/include mapping. Generated locations are omitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    pub sequence: u32,
    pub column: u32,
    pub file: String,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementSource {
    pub function: String,
    pub path: String,
    pub location: SourceLocation,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolSource {
    pub symbol: SymbolId,
    pub location: SourceLocation,
}
/// Primary expression role 0, supplemental control/init roles 1..=4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionSource {
    pub function: String,
    pub statement_path: String,
    pub role: u32,
    pub path: String,
    pub location: SourceLocation,
}

/// Runtime lifetime region from EDG. IDs are dense within a routine.
/// Multiple root regions may exist (e.g. local static storage).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectLifetime {
    pub routine: SymbolId,
    pub id: u32,
    pub parent: Option<u32>,
    pub kind: u32,
}

/// Link between a lexical scope and the lifetime region EDG assigned it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeLifetime {
    pub routine: SymbolId,
    pub scope: u32,
    pub lifetime: u32,
}

/// Optional lifetime/destructor of an executed dynamic initialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitializationLifetime {
    pub function: String,
    pub statement_path: String,
    pub lifetime: Option<u32>,
    pub destructor: Option<SymbolId>,
}

/// Destruction list position in the order EDG requires cleanup to run.
/// A destructor may be absent for generated guards and related cleanup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifetimeDestruction {
    pub routine: SymbolId,
    pub lifetime: u32,
    pub position: u32,
    pub variable: Option<SymbolId>,
    pub destructor: Option<SymbolId>,
}

/// A branch or definition referring to one compilation-local EDG label.
/// The label Symbol has kind 4. References may be compiler generated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementLabel {
    pub function: String,
    pub statement_path: String,
    pub label: SymbolId,
    pub definition: bool,
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

/// A parameter variable owned by one function definition.
/// Explicit parameters have dense zero-based positions; the implicit C++
/// object parameter has `position: None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionParameter {
    pub routine: SymbolId,
    pub position: Option<u32>,
    pub symbol: SymbolId,
    pub ty: TypeId,
    pub implicit_this: bool,
    pub compiler_generated: bool,
}

/// A scope in the owned lexical tree of one function definition.
/// Scope zero is the function scope, with no parent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalScope {
    pub routine: SymbolId,
    pub id: u32,
    pub parent: Option<u32>,
    pub kind: u32,
    /// Associated structured statement if EDG retained that link.
    pub statement_path: Option<String>,
}

/// A local declaration belonging to a lexical scope.
/// `static_storage` distinguishes scope->variables from
/// scope->nonstatic_variables in EDG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalVariable {
    pub routine: SymbolId,
    pub scope: u32,
    pub symbol: SymbolId,
    pub ty: TypeId,
    pub storage_class: u32,
    pub compiler_generated: bool,
    pub static_storage: bool,
    pub structured_binding: bool,
    pub handler_parameter: bool,
}

/// Static or thread-storage variable retained in a file-scope memory
/// region even though its declaring context is a function. EDG cannot
/// safely retain its precise block scope across the memory-region boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticLocalOwner {
    pub routine: SymbolId,
    pub symbol: SymbolId,
}

/// An EDG operation call. Operand zero is the callee; member calls use
/// operand one for the object, so explicit arguments start at index two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallSite {
    pub function: String,
    pub statement_path: String,
    pub path: String,
    pub first_argument: u32,
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
        16 => Some(il.dynamic_initializers.len()),
        17 => Some(il.initializer_expressions.len()),
        19 => Some(il.base_classes.len()),
        20 => Some(il.constructor_initializers.len()),
        21 => Some(il.member_functions.len()),
        22 => Some(il.routine_attributes.len()),
        23 => Some(il.constructor_initializer_expressions.len()),
        24 => Some(il.base_class_access.len()),
        25 => Some(il.symbol_types.len()),
        26 => Some(il.statement_attributes.len()),
        27 => Some(il.control_expressions.len()),
        28 => Some(il.statement_labels.len()),
        29 => Some(il.function_parameters.len()),
        30 => Some(il.local_scopes.len()),
        31 => Some(il.local_variables.len()),
        32 => Some(il.static_local_owners.len()),
        33 => Some(il.statement_declarations.len()),
        34 => Some(il.statement_initializations.len()),
        35 => Some(il.statement_sources.len()),
        36 => Some(il.symbol_sources.len()),
        37 => Some(il.expression_sources.len()),
        38 => Some(il.object_lifetimes.len()),
        39 => Some(il.scope_lifetimes.len()),
        40 => Some(il.initialization_lifetimes.len()),
        41 => Some(il.lifetime_destructions.len()),
        18 => Some(il.calls.len()),
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
        9 => 2, 10 => 7, 11 => 3, 12 => 3, 13 => 3, 14 => 4, 15 => 2, 16 => 4, 17 => 7, 18 => 4, 19 => 4, 20 => 6, 21 => 2, 22 => 3, 23 => 8, 24 => 4, 25 => 2, 26 => 3, 27 => 9, 28 => 4, 29 => 5, 30 => 5, 31 => 6, 32 => 2, 33 => 6, 34 => 6, 35 => 6, 36 => 5, 37 => 8, 38 => 4, 39 => 3, 40 => 4, 41 => 5, _ => return,
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
        41 => {
            let (Some(routine), Some(lifetime), Some(position)) =
                (n(0), n(1), n(2)) else { return; };
            if routine == 0 { return; }
            output.lifetime_destructions.push(LifetimeDestruction {
                routine: SymbolId(routine), lifetime, position,
                variable: n(3).map(SymbolId), destructor: n(4).map(SymbolId),
            });
        }
        40 => {
            let (Some(function), Some(statement_path)) =
                (t(0), t(1)) else { return; };
            if !fields[2].present() && !fields[3].present() { return; }
            output.initialization_lifetimes.push(InitializationLifetime {
                function, statement_path,
                lifetime: n(2), destructor: n(3).map(SymbolId),
            });
        }
        39 => {
            let (Some(routine), Some(scope), Some(lifetime)) =
                (n(0), n(1), n(2)) else { return; };
            if routine == 0 { return; }
            output.scope_lifetimes.push(ScopeLifetime {
                routine: SymbolId(routine), scope, lifetime,
            });
        }
        38 => {
            let (Some(routine), Some(id), Some(kind)) =
                (n(0), n(1), n(3)) else { return; };
            if routine == 0 { return; }
            output.object_lifetimes.push(ObjectLifetime {
                routine: SymbolId(routine), id, parent: n(2), kind,
            });
        }
        37 => {
            let (Some(function), Some(statement_path), Some(role),
                 Some(path), Some(sequence), Some(column), Some(file), Some(line)) =
                (t(0), t(1), n(2), t(3), n(4), n(5), t(6), n(7))
                else { return; };
            if role > 4 || sequence == 0 || line == 0 || file.is_empty() { return; }
            output.expression_sources.push(ExpressionSource {
                function, statement_path, role, path,
                location: SourceLocation { sequence, column, file, line },
            });
        }
        36 => {
            let (Some(symbol), Some(sequence), Some(column), Some(file), Some(line)) =
                (n(0), n(1), n(2), t(3), n(4)) else { return; };
            if symbol == 0 || sequence == 0 || line == 0 || file.is_empty() { return; }
            output.symbol_sources.push(SymbolSource {
                symbol: SymbolId(symbol),
                location: SourceLocation { sequence, column, file, line },
            });
        }
        35 => {
            let (Some(function), Some(path), Some(sequence), Some(column),
                 Some(file), Some(line)) = (t(0), t(1), n(2), n(3), t(4), n(5))
                 else { return; };
            if sequence == 0 || line == 0 || file.is_empty() { return; }
            output.statement_sources.push(StatementSource {
                function, path,
                location: SourceLocation { sequence, column, file, line },
            });
        }
        34 => {
            let (Some(function), Some(statement_path), Some(kind),
                 Some(flags), Some(role)) = (t(0), t(1), n(3), n(4), n(5))
                 else { return; };
            if flags & !3 != 0 || role > 2 { return; }
            output.statement_initializations.push(StatementInitialization {
                function, statement_path, symbol: n(2).map(SymbolId), kind,
                follows_execution: flags & 1 != 0,
                braced: flags & 2 != 0, role,
            });
        }
        33 => {
            let (Some(function), Some(statement_path), Some(position),
                 Some(symbol), Some(ty), Some(is_vla)) =
                (t(0), t(1), n(2), n(3), n(4), n(5)) else { return; };
            if symbol == 0 || ty == 0 || is_vla > 1 { return; }
            output.statement_declarations.push(StatementDeclaration {
                function, statement_path, position,
                symbol: SymbolId(symbol), ty: TypeId(ty),
                is_vla: is_vla != 0,
            });
        }
        32 => {
            let (Some(routine), Some(symbol)) = (n(0), n(1)) else { return; };
            if routine == 0 || symbol == 0 { return; }
            output.static_local_owners.push(StaticLocalOwner {
                routine: SymbolId(routine), symbol: SymbolId(symbol),
            });
        }
        31 => {
            let (Some(routine), Some(scope), Some(symbol), Some(ty),
                 Some(storage_class), Some(flags)) =
                (n(0), n(1), n(2), n(3), n(4), n(5)) else { return; };
            if routine == 0 || symbol == 0 || ty == 0 || flags & !15 != 0 { return; }
            output.local_variables.push(LocalVariable {
                routine: SymbolId(routine), scope, symbol: SymbolId(symbol),
                ty: TypeId(ty), storage_class,
                compiler_generated: flags & 1 != 0,
                static_storage: flags & 2 != 0,
                structured_binding: flags & 4 != 0,
                handler_parameter: flags & 8 != 0,
            });
        }
        30 => {
            let (Some(routine), Some(id), Some(kind)) = (n(0), n(1), n(3))
                else { return; };
            if routine == 0 { return; }
            let statement_path = if fields[4].present() {
                let Some(path) = t(4) else { return; };
                Some(path)
            } else { None };
            output.local_scopes.push(LocalScope {
                routine: SymbolId(routine), id, parent: n(2),
                kind, statement_path,
            });
        }
        29 => {
            let (Some(routine), Some(symbol), Some(ty), Some(flags)) =
                (n(0), n(2), n(3), n(4)) else { return; };
            if routine == 0 || symbol == 0 || ty == 0 || flags & !3 != 0
                || ((flags & 1 != 0) != !fields[1].present()) { return; }
            output.function_parameters.push(FunctionParameter {
                routine: SymbolId(routine), position: n(1),
                symbol: SymbolId(symbol), ty: TypeId(ty),
                implicit_this: flags & 1 != 0,
                compiler_generated: flags & 2 != 0,
            });
        }
        28 => {
            let (Some(function), Some(statement_path), Some(label), Some(definition)) =
                (t(0), t(1), n(2), n(3)) else { return; };
            if label == 0 || definition > 1 { return; }
            output.statement_labels.push(StatementLabel {
                function, statement_path,
                label: SymbolId(label), definition: definition == 1,
            });
        }
        27 => {
            let (Some(function), Some(statement_path), Some(role),
                 Some(path), Some(kind)) = (t(0), t(1), n(2), t(3), n(5)) else { return; };
            if !(1..=4).contains(&role) { return; }
            let parent = if fields[4].present() {
                let Some(parent) = t(4) else { return; };
                Some(parent)
            } else { None };
            output.control_expressions.push(ControlExpression {
                function, statement_path, role, path, parent, kind,
                operator: n(6), type_id: n(7).map(TypeId),
                symbol_id: n(8).map(SymbolId),
            });
        }
        26 => {
            let (Some(function), Some(statement_path), Some(flags)) =
                (t(0), t(1), n(2)) else { return; };
            if flags & !127 != 0 { return; }
            output.statement_attributes.push(StatementAttributes {
                function, statement_path,
                compiler_generated: flags & 1 != 0,
                initialization_guard: flags & 2 != 0,
                fallthrough: flags & 4 != 0,
                likely: flags & 8 != 0,
                unlikely: flags & 16 != 0,
                lowering_generated: flags & 32 != 0,
                lowering_boilerplate: flags & 64 != 0,
            });
        }
        25 => {
            let (Some(symbol), Some(ty)) = (n(0), n(1)) else { return; };
            if symbol == 0 || ty == 0 { return; }
            output.symbol_types.push(SymbolType { symbol: SymbolId(symbol), ty: TypeId(ty) });
        }
        24 => {
            let (Some(derived), Some(position), Some(expansion)) =
                (n(0), n(1), n(3)) else { return; };
            if expansion > 1 { return; }
            output.base_class_access.push(BaseClassAccess {
                derived: TypeId(derived), position,
                access: n(2), pack_expansion: expansion == 1,
            });
        }
        23 => {
            let (Some(routine), Some(position), Some(path), Some(kind)) =
                (n(0), n(1), t(2), n(4)) else { return; };
            let parent = if fields[3].present() {
                let Some(parent) = t(3) else { return; };
                Some(parent)
            } else { None };
            output.constructor_initializer_expressions.push(ConstructorInitializerExpression {
                routine: SymbolId(routine), position, path, parent, kind,
                operator: n(5), type_id: n(6).map(TypeId),
                symbol_id: n(7).map(SymbolId),
            });
        }
        22 => {
            let (Some(routine), Some(special_kind), Some(flags)) =
                (n(0), n(1), n(2)) else { return; };
            if flags & !1023 != 0 { return; }
            output.routine_attributes.push(RoutineAttributes {
                routine: SymbolId(routine), special_kind,
                is_virtual: flags & 1 != 0, pure_virtual: flags & 2 != 0,
                is_override: flags & 4 != 0, is_final: flags & 8 != 0,
                overrides_base_member: flags & 16 != 0,
                is_inline: flags & 32 != 0, is_constexpr: flags & 64 != 0,
                defined: flags & 128 != 0,
                explicit_constructor: flags & 256 != 0,
                has_this_parameter: flags & 512 != 0,
            });
        }
        21 => {
            let (Some(owner), Some(routine)) = (n(0), n(1)) else { return; };
            output.member_functions.push(MemberFunction {
                owner: TypeId(owner), routine: SymbolId(routine),
            });
        }
        20 => {
            let (Some(routine), Some(position), Some(kind), Some(flags)) =
                (n(0), n(1), n(2), n(5)) else { return; };
            if kind > 3 || flags & !15 != 0 { return; }
            let target_type = n(3).map(TypeId);
            let target_field = n(4).map(SymbolId);
            if (kind <= 1 && (target_type.is_none() || target_field.is_some()))
                || (kind == 2 && (target_type.is_some() || target_field.is_none()))
                || (kind == 3 && (target_type.is_some() || target_field.is_some()))
            { return; }
            output.constructor_initializers.push(ConstructorInitializer {
                routine: SymbolId(routine), position, kind,
                target_type, target_field,
                compiler_generated: flags & 1 != 0,
                braced: flags & 2 != 0,
                use_field_initializer: flags & 4 != 0,
                pack_expansion: flags & 8 != 0,
            });
        }
        19 => {
            let (Some(derived), Some(position), Some(base), Some(virtual_flag)) =
                (n(0), n(1), n(2), n(3)) else { return; };
            if virtual_flag > 1 { return; }
            output.base_classes.push(BaseClass {
                derived: TypeId(derived), position, base: TypeId(base),
                is_virtual: virtual_flag == 1,
            });
        }
        18 => {
            let (Some(function), Some(statement_path), Some(path), Some(first_argument)) =
                (t(0), t(1), t(2), n(3)) else { return; };
            if first_argument != 1 && first_argument != 2 { return; }
            output.calls.push(CallSite { function, statement_path, path, first_argument });
        }
        17 => {
            let (Some(owner), Some(path), Some(kind)) = (n(0), t(1), n(3)) else { return; };
            let parent = if fields[2].present() {
                let Some(parent) = t(2) else { return; };
                Some(parent)
            } else { None };
            output.initializer_expressions.push(InitializerExpression {
                owner: SymbolId(owner), path, parent, kind,
                operator: n(4), type_id: n(5).map(TypeId),
                symbol_id: n(6).map(SymbolId),
            });
        }
        16 => {
            let (Some(symbol), Some(kind)) = (n(0), n(1)) else { return; };
            output.dynamic_initializers.push(DynamicInitializer {
                symbol: SymbolId(symbol), kind,
                expression_kind: n(2), operator: n(3),
            });
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
        ], statements: vec![], expressions: vec![], types: vec![], type_parameters: vec![], array_bounds: vec![], record_fields: vec![], type_details: vec![], function_signatures: vec![], symbols: vec![], variable_initializers: vec![], initializer_integers: vec![], dynamic_initializers: vec![], initializer_expressions: vec![], ..Il::default() };
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
    fn decodes_routine_attributes_and_constructor_expression_events() {
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
        let attributes = [number(3), number(1), number(1 | 4 | 8 | 512)];
        unsafe {
            collect((&mut state as *mut Collector).cast(), 22,
                    attributes.as_ptr(), attributes.len(), 0);
        }
        let expression = [
            number(3), number(2), string(b"0"), absent,
            number(4), number(10), number(9), absent,
        ];
        unsafe {
            collect((&mut state as *mut Collector).cast(), 23,
                    expression.as_ptr(), expression.len(), 0);
        }
        assert!(!state.malformed);
        assert_eq!(state.il.routine_attributes.len(), 1);
        let attributes = &state.il.routine_attributes[0];
        assert_eq!(attributes.routine, SymbolId(3));
        assert_eq!(attributes.special_kind, 1);
        assert!(attributes.is_virtual && attributes.is_override && attributes.is_final);
        assert!(attributes.has_this_parameter);
        assert!(!attributes.is_constexpr);
        let expr = &state.il.constructor_initializer_expressions[0];
        assert_eq!((expr.routine, expr.position), (SymbolId(3), 2));
        assert_eq!(expr.path, "0");
        assert_eq!(expr.type_id, Some(TypeId(9)));
        let invalid_attributes = [number(3), number(1), number(1024)];
        unsafe {
            collect((&mut state as *mut Collector).cast(), 22,
                    invalid_attributes.as_ptr(), invalid_attributes.len(), 0);
        }
        assert!(state.malformed);
    }

    #[test]
    fn decodes_declaration_and_initializer_site_events() {
        let string = |bytes: &[u8]| WireField {
            data: bytes.as_ptr(), length: bytes.len(), value: 0, tag: 1,
        };
        let number = |value| WireField {
            data: std::ptr::null(), length: 0, value, tag: 2,
        };
        let absent = WireField {
            data: std::ptr::null(), length: 0, value: 0, tag: 0,
        };
        let mut state = Collector { il: Il::default(), panicked: false, malformed: false };
        let decl = [string(b"f"), string(b"0"), number(0),
                    number(11), number(12), number(1)];
        let init = [string(b"f"), string(b"1"), number(11),
                    number(2), number(3), number(0)];
        unsafe {
            collect((&mut state as *mut Collector).cast(), 33,
                    decl.as_ptr(), decl.len(), 0);
            collect((&mut state as *mut Collector).cast(), 34,
                    init.as_ptr(), init.len(), 0);
        }
        assert!(!state.malformed);
        let decl = &state.il.statement_declarations[0];
        assert_eq!((decl.symbol, decl.ty, decl.position),
                   (SymbolId(11), TypeId(12), 0));
        assert!(decl.is_vla);
        let init = &state.il.statement_initializations[0];
        assert_eq!(init.symbol, Some(SymbolId(11)));
        assert!(init.follows_execution && init.braced);
        assert_eq!(init.role, 0);

        // Owned statement initializer expressions use supplemental role 4.
        let initializer_expr = [
            string(b"f"), string(b"1"), number(4), string(b"0"), absent,
            number(1), absent, number(12), number(11),
        ];
        unsafe {
            collect((&mut state as *mut Collector).cast(), 27,
                    initializer_expr.as_ptr(), initializer_expr.len(), 0);
        }
        assert!(!state.malformed);
        assert_eq!(state.il.control_expressions[0].role, 4);

        let ret = [string(b"f"), string(b"2"), absent,
                   number(3), number(0), number(1)];
        unsafe {
            collect((&mut state as *mut Collector).cast(), 34,
                    ret.as_ptr(), ret.len(), 0);
        }
        assert!(!state.malformed);
        assert_eq!(state.il.statement_initializations[1].symbol, None);
        assert_eq!(state.il.statement_initializations[1].role, 1);

        let invalid = [string(b"f"), string(b"0"), number(0),
                       number(11), number(12), number(2)];
        unsafe {
            collect((&mut state as *mut Collector).cast(), 33,
                    invalid.as_ptr(), invalid.len(), 0);
        }
        assert!(state.malformed);
    }

    #[test]
    fn decodes_function_parameters_lexical_scopes_and_locals() {
        let string = |bytes: &[u8]| WireField {
            data: bytes.as_ptr(), length: bytes.len(), value: 0, tag: 1,
        };
        let number = |value| WireField {
            data: std::ptr::null(), length: 0, value, tag: 2,
        };
        let absent = WireField {
            data: std::ptr::null(), length: 0, value: 0, tag: 0,
        };
        let mut state = Collector { il: Il::default(), panicked: false, malformed: false };
        let parameter = [number(1), number(0), number(2), number(3), number(0)];
        let implicit = [number(1), absent, number(4), number(5), number(3)];
        let root = [number(1), number(0), absent, number(17), string(b"0")];
        let nested = [number(1), number(1), number(0), number(2), absent];
        let local = [number(1), number(1), number(6), number(3), number(0), number(6)];
        unsafe {
            for values in [parameter, implicit] {
                collect((&mut state as *mut Collector).cast(), 29,
                        values.as_ptr(), values.len(), 0);
            }
            for values in [root, nested] {
                collect((&mut state as *mut Collector).cast(), 30,
                        values.as_ptr(), values.len(), 0);
            }
            collect((&mut state as *mut Collector).cast(), 31,
                    local.as_ptr(), local.len(), 0);
        }
        assert!(!state.malformed);
        assert_eq!(state.il.function_parameters.len(), 2);
        assert_eq!(state.il.function_parameters[0].position, Some(0));
        assert!(state.il.function_parameters[1].implicit_this);
        assert_eq!(state.il.function_parameters[1].position, None);
        assert_eq!(state.il.local_scopes[0].statement_path.as_deref(), Some("0"));
        assert_eq!(state.il.local_scopes[1].parent, Some(0));
        assert!(state.il.local_variables[0].static_storage);
        assert!(state.il.local_variables[0].structured_binding);
        let malformed = [number(1), number(0), number(2), number(3), number(1)];
        unsafe {
            collect((&mut state as *mut Collector).cast(), 29,
                    malformed.as_ptr(), malformed.len(), 0);
        }
        assert!(state.malformed);
    }

    #[test]
    fn decodes_statement_flags_loop_operands_and_label_edges() {
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
        let attrs = [string(b"f"), string(b"0"), number(1 | 8 | 32)];
        let control = [
            string(b"f"), string(b"0"), number(1), string(b"0"), absent,
            number(1), number(5), number(4), absent,
        ];
        let label = [string(b"f"), string(b"1"), number(17), number(1)];
        unsafe {
            collect((&mut state as *mut Collector).cast(), 26, attrs.as_ptr(), attrs.len(), 0);
            collect((&mut state as *mut Collector).cast(), 27, control.as_ptr(), control.len(), 0);
            collect((&mut state as *mut Collector).cast(), 28, label.as_ptr(), label.len(), 0);
        }
        assert!(!state.malformed);
        assert_eq!(state.il.statement_attributes.len(), 1);
        assert!(state.il.statement_attributes[0].likely);
        assert!(state.il.statement_attributes[0].lowering_generated);
        assert_eq!(state.il.control_expressions[0].role, 1);
        assert_eq!(state.il.control_expressions[0].type_id, Some(TypeId(4)));
        assert_eq!(state.il.statement_labels[0].label, SymbolId(17));
        assert!(state.il.statement_labels[0].definition);
        let invalid_control = [
            string(b"f"), string(b"0"), number(9), string(b"0"), absent,
            number(1), absent, absent, absent,
        ];
        unsafe {
            collect((&mut state as *mut Collector).cast(), 27,
                    invalid_control.as_ptr(), invalid_control.len(), 0);
        }
        assert!(state.malformed);
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
