//! Callable EDG front end and an owned, first-level view of its IL.
//!
//! This is a **Linux x86-64** C++ ABI bridge. Both sides must be built from
//! matching EDG sources and configuration (see README).
//!
//! The IL snapshot contains named functions, variables, types, and namespaces. The
//! native IL is far richer; this crate intentionally does not expose dangling
//! raw EDG pointers after `edg_main` returns.
pub mod graph;
pub mod memory_order;
pub mod scalar_abi;
pub mod startup_provenance;
mod limits;
#[cfg(feature = "codegen")]
pub mod codegen;

use std::{
    collections::{HashMap, HashSet},
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
// The tags and record arities are generated from schema/wire_schema.json.
include!("wire_schema_generated.rs");
type Callback = unsafe extern "C" fn(*mut c_void, u32, *const WireField, usize, u32);
// A malformed native text length must not produce an enormous or
// out-of-bounds slice before UTF-8 validation. This is a defensive
// implementation cap, not a C/C++ language token-length restriction.
const MAX_WIRE_TEXT_BYTES: usize = 1024 * 1024;

impl WireField {
    fn string(self) -> Option<String> {
        if self.tag != wire_tags::TEXT || self.data.is_null() ||
            self.length > MAX_WIRE_TEXT_BYTES { return None; }
        std::str::from_utf8(unsafe { std::slice::from_raw_parts(self.data, self.length) })
            .ok().map(str::to_owned)
    }
    fn unsigned(self) -> Option<u64> { (self.tag == wire_tags::UNSIGNED).then_some(self.value) }
    fn u32(self) -> Option<u32> { self.unsigned().and_then(|n| n.try_into().ok()) }
    fn signed(self) -> Option<i64> { (self.tag == wire_tags::SIGNED).then_some(self.value as i64) }
    fn present(self) -> bool { self.tag != wire_tags::ABSENT }
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
    /// EDG-authoritative language dialect (None means missing wire provenance).
    pub cpp_language: Option<bool>,
    /// C++ local object admission after inspecting EDG's complete class
    /// member scope, including methods inlined away from routine bodies.
    pub cxx_record_admissions: Vec<CxxRecordAdmission>,
    /// EDG event 63 discriminant=2: complete tagless record without class
    /// supplement. Strictly storage shape; NOT plain_object or lifetime.
    pub cxx_tagless_storage_proofs: Vec<TypeId>,
    /// Distinct source-certified candidate for a bounded user ctor/dtor.
    /// This never implies ordinary POD or proves path-sensitive cleanup.
    pub cxx_scalar_lifetime_admissions: Vec<CxxScalarLifetimeAdmission>,
    /// Structural statement records captured while function IL is live.
    pub statements: Vec<Statement>,
    /// Independent EDG-derived counts for primary expression trees.
    pub native_expression_child_counts: Vec<NativeExpressionChildCount>,
    pub native_control_child_counts: Vec<NativeControlChildCount>,
    pub native_initializer_child_counts: Vec<NativeInitializerChildCount>,
    pub native_constructor_child_counts: Vec<NativeConstructorChildCount>,
    /// Per-statement EDG-authoritative role mask for control-expression roots.
    pub native_control_root_admissions: Vec<NativeControlRootAdmission>,
    /// Per-constructor initializer native expression-root presence.
    pub native_constructor_root_admissions: Vec<NativeConstructorRootAdmission>,
    /// Independent EDG-derived counts for native statement sibling lists.
    pub native_statement_sibling_counts: Vec<NativeStatementSiblingCount>,
    /// EDG-native count of catch handlers owned by each try statement (wire 86).
    pub native_try_handler_counts: Vec<NativeTryHandlerCount>,
    /// Native flags retained for each emitted statement.
    pub statement_attributes: Vec<StatementAttributes>,
    /// Additional expressions on structured controls (e.g. for increments).
    pub control_expressions: Vec<ControlExpression>,
    /// Original EDG enk_lowered_eh_construct subtype and exact expression
    /// identity in the supported nonportable producer mode. Structural only:
    /// nothing here authorizes Itanium unwind, atexit or ELF emission.
    pub native_retained_eh_constructs: Vec<NativeRetainedEhConstruct>,
    /// References and definitions of native goto labels.
    pub statement_labels: Vec<StatementLabel>,
    /// EDG case/default metadata from native switch statements.
    pub statement_switch_cases: Vec<StatementSwitchCase>,
    /// Native generated-label classification for structured loop transfers.
    pub loop_transfers: Vec<LoopTransfer>,
    /// Variable declaration positions within structured statements.
    pub statement_declarations: Vec<StatementDeclaration>,
    /// Runtime VLA bound evaluation sites; tied to native array type IDs.
    pub vla_dimensions: Vec<VlaDimension>,
    /// Dynamic initialization execution sites with optional expression trees.
    pub statement_initializations: Vec<StatementInitialization>,
    /// Exact scalar i64 dynamic initializer constants at execution sites.
    pub statement_integer_initializers: Vec<StatementIntegerInitializer>,
    /// Function-pointer constants keep native EDG routine symbol identities.
    pub statement_routine_initializers: Vec<StatementRoutineInitializer>,
    /// Constant EDG aggregate images at local initialization statements.
    pub statement_aggregate_initializers: Vec<StatementAggregateInitializer>,
    /// Constant EDG aggregate images for statically defined globals.
    pub global_aggregate_initializers: Vec<GlobalAggregateInitializer>,
    /// Relocatable global data pointers embedded in native aggregate images.
    pub global_aggregate_relocations: Vec<GlobalAggregateRelocation>,
    /// Source-identity-preserving EDG constant objects referenced by static
    /// global pointer initializers. Never treat bytes as a guessed address.
    pub global_literal_objects: Vec<GlobalLiteralObject>,
    /// Pointer-sized linker slots referring to the above immutable objects.
    pub global_literal_relocations: Vec<GlobalLiteralRelocation>,
    /// Source-owned routine function address slots in static C objects.
    pub global_routine_relocations: Vec<GlobalRoutineRelocation>,
    pub statement_sources: Vec<StatementSource>,
    pub symbol_sources: Vec<SymbolSource>,
    pub expression_sources: Vec<ExpressionSource>,
    pub object_lifetimes: Vec<ObjectLifetime>,
    pub scope_lifetimes: Vec<ScopeLifetime>,
    pub initialization_lifetimes: Vec<InitializationLifetime>,
    pub lifetime_destructions: Vec<LifetimeDestruction>,
    /// EDG-native independent destruction count for each emitted lifetime.
    pub native_lifetime_destruction_counts: Vec<NativeLifetimeDestructionCount>,
    /// EDG-owned child-list cardinality for each lifetime, including leaves.
    pub native_lifetime_child_counts: Vec<NativeLifetimeChildCount>,
    /// Defined function-scope lifetime owners, including absent roots.
    pub native_lifetime_root_admissions: Vec<NativeLifetimeRootAdmission>,
    /// Nullable scope-to-lifetime owner witnesses, including empty links.
    pub native_scope_lifetime_admissions: Vec<NativeScopeLifetimeAdmission>,
    /// Native lexical-scope child cardinality, including empty scopes.
    pub native_scope_child_counts: Vec<NativeScopeChildCount>,
    /// Native independent declaration count per lexical scope.
    pub native_scope_local_counts: Vec<NativeScopeLocalCount>,
    /// Native independent explicit/implicit parameter count per routine.
    pub native_function_parameter_counts: Vec<NativeFunctionParameterCount>,
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
    /// Source-keyed exact byte images of narrow EDG C string literals.
    /// These are owned constant objects, not reinterpretations of pointers.
    pub string_literals: Vec<ExpressionStringLiteral>,
    /// Identity-preserving types captured during this compilation.
    pub types: Vec<TypeDef>,
    /// EDG-verified signed, non-enum 64-bit integer ABI types (event 43).
    pub signed_i64_types: Vec<TypeId>,
    /// Native C integer width and sign, independent of host compiler ABI.
    pub c_integer_types: Vec<CIntegerType>,
    /// Source-verified standard C binary FP metadata (native wire event 57).
    pub c_float_types: Vec<CFloatType>,
    /// One EDG-native event 76 per type certifies whether integer ABI,
    /// standard float ABI and signed-i64 provenance records must exist.
    pub native_scalar_proofs: Vec<NativeScalarMetadataProof>,
    /// Native IEEE standard floating-point literal images, event 58.
    pub float_constants: Vec<FloatConstant>,
    /// EDG dik_constant scalar FP initializer images, wire event 59.
    pub float_initializers: Vec<FloatInitializer>,
    /// Byte-exact native IEEE file-scope definition image, event 60.
    pub global_float_initializers: Vec<GlobalFloatInitializer>,
    pub global_variables: Vec<GlobalVariable>,
    /// EDG's nonreal/template/prototype classification of each file-scope variable.
    pub global_materializations: Vec<GlobalMaterialization>,
    /// Diagnostic EDG global-init scheduler mode and completeness, never an ELF admission proof.
    pub file_scope_init_schedule: Option<FileScopeInitSchedule>,
    /// EDG's explicit file-scope init routine list in native order (if configured).
    pub file_scope_init_routines: Vec<FileScopeInitRoutine>,
    /// Native EDG per-variable routine identity, available ONLY when the
    /// callable frontend was built in separate file-scope-init mode.
    /// This association does not prove constructor, cleanup, or ELF ABI.
    pub file_scope_dynamic_init_owners: Vec<FileScopeDynamicInitOwner>,
    /// Exact routine-address link from EDG's lowering-generated __link object.
    /// A startup root alone does not prove cleanup order or an ELF init plan.
    pub file_scope_runtime_links: Vec<FileScopeRuntimeLink>,
    /// Native generated __sti__ routine candidates in the non-patch mode.
    /// EDG's own raw C backend classifier is used, not user source names.
    pub file_scope_lowered_startup_routines: Vec<FileScopeLoweredStartupRoutine>,
    /// Native per-scope dynamic initializers in EDG's own list order.
    pub scope_dynamic_initializers: Vec<ScopeDynamicInitializer>,
    /// Opaque source-owned file/namespace asm declarations. Diagnostic only:
    /// the bridge neither assembles them nor claims complete linker semantics.
    pub native_scope_asm: Vec<NativeScopeAsm>,
    /// EDG ck_address/abk_variable literal: base symbol plus signed byte offset.
    pub constant_addresses: Vec<ConstantAddress>,
    /// Folded source expression constants holding C routine pointers.
    pub constant_routine_addresses: Vec<ConstantRoutineAddress>,
    /// Ordered parameter edges belonging to routine types.
    pub type_parameters: Vec<TypeParameter>,
    /// Known fixed array bounds (in elements).
    pub array_bounds: Vec<ArrayBound>,
    pub record_fields: Vec<RecordField>,
    /// EDG-authored counts of nonstatic record fields and direct bases.
    /// Mandatory for every native struct, class, or union on success.
    pub native_record_shapes: Vec<NativeRecordShape>,
    /// Independent EDG-native event 75 proofs that events 49 (layout) and
    /// 9 (fixed array bound) must be present or absent for each emitted type.
    pub native_type_layout_proofs: Vec<NativeTypeLayoutProof>,
    /// Native EDG object sizes for complete types; not inferred from fields.
    pub type_object_sizes: Vec<TypeObjectSize>,
    /// Direct class bases in declaration order.
    pub base_classes: Vec<BaseClass>,
    /// EDG-authoritative verified direct base-subobject byte offsets.
    pub base_subobject_offsets: Vec<BaseSubobjectOffset>,
    /// Direct-base access specifiers keyed by derived type and position.
    pub base_class_access: Vec<BaseClassAccess>,
    /// Constructor and destructor subobject initialization records.
    pub constructor_initializers: Vec<ConstructorInitializer>,
    /// EDG-native independent constructor initializer list size per defined routine.
    pub native_constructor_initializer_counts: Vec<NativeConstructorInitializerCount>,
    /// Owned expression trees attached to constructor initializer entries.
    pub constructor_initializer_expressions: Vec<ConstructorInitializerExpression>,
    /// Class ownership edges for declared member functions.
    pub member_functions: Vec<MemberFunction>,
    /// Special-function and virtual dispatch attributes of routines.
    pub routine_attributes: Vec<RoutineAttributes>,
    /// EDG logical virtual member number, class virtual-info byte offset,
    /// and optional sharing base. Never a complete Itanium dispatch ABI proof.
    pub native_virtual_members: Vec<NativeVirtualMemberInfo>,
    pub type_details: Vec<TypeDetail>,
    pub function_signatures: Vec<FunctionSignature>,
    /// Independent native event 71: whether tk_routine.extra_info exists
    /// and the authoritative EDG parameter count. Every routine has one.
    pub routine_supplements: Vec<NativeRoutineSupplement>,
    pub symbols: Vec<Symbol>,
    /// EDG's untouched routine linker spellings, distinct from display names.
    /// A false mangled flag does NOT authorize fabricating C++ ABI names.
    pub symbol_link_names: Vec<SymbolLinkName>,
    /// Authoritative routine IDs and their names as used by statement IL.
    pub routine_identities: Vec<RoutineIdentity>,
    /// Type edges for named declarations.
    pub symbol_types: Vec<SymbolType>,
    pub variable_initializers: Vec<VariableInitializer>,
    pub initializer_integers: Vec<InitializerInteger>,
    pub dynamic_initializers: Vec<DynamicInitializer>,
    pub initializer_expressions: Vec<InitializerExpression>,
    /// Call sites with the operand index of the first explicit argument.
    pub calls: Vec<CallSite>,
    /// Source-owned direct/indirect callee provenance, one native event 77
    /// per call. Indirect is intentionally *not* a virtual-call ABI proof.
    pub call_targets: Vec<CallTargetProvenance>,
    /// EDG's source-owned member-call dispatch bit. Plain calls may be
    /// lowered from virtual calls; their zero bit is not a nonvirtual proof.
    pub call_dispatches: Vec<CallDispatchProvenance>,
    /// Legacy optional wire event 80 from a retired pre-lowering EDG hook.
    /// New bridge builds cannot produce this record; it is decoded only for
    /// historical fixtures and never establishes executable virtual-call ABI.
    pub prelower_virtual_origins: Vec<PrelowerVirtualCallOrigin>,
    /// Legacy optional wire event 81 paired with a retired EDG source hook.
    /// New bridge builds do not produce it; it was never an ABI adjustment proof.
    pub virtual_receiver_reuses: Vec<VirtualReceiverReuse>,
    /// Legacy optional wire event 82 from retired EDG source instrumentation.
    /// New bridge builds do not produce it; no physical ABI slot is inferred.
    pub virtual_entry_indexes: Vec<VirtualEntryIndexSelection>,
    /// Legacy optional wire event 85 from retired EDG source instrumentation.
    /// New bridge builds do not produce it; no machine receiver adjustment
    /// or live object layout is inferred.
    pub virtual_vptr_receiver_pairs: Vec<VirtualVptrReceiverPair>,
    /// Native compiler-generated pointer conversion wrappers, retaining the
    /// operand source C type before EDG inserts an implicit cast.
    pub implicit_pointer_casts: Vec<ImplicitPointerCast>,
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

/// Exact EDG-owned linker spelling of a native routine. Unlike
/// `Symbol::name`, this never undergoes namespace qualification,
/// demangling, suffix disambiguation or any inferred Itanium encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolLinkName {
    pub symbol: SymbolId,
    pub link_name: String,
    /// True only when EDG explicitly reports that it already mangled
    /// the source correspondence name. False may denote extern "C"
    /// or a C++ entity for which link-name formation is unfinished.
    pub edg_mangled: bool,
}

/// Stable link within one compilation between a native routine symbol and
/// the exported function-name key used for statement and scope paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutineIdentity {
    pub routine: SymbolId,
    pub function: String,
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

/// The native, complete, non-enum scalar integer type of a C declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CIntegerType {
    pub id: TypeId,
    pub bits: u8,
    pub signed: bool,
    /// EDG's native bool_type bit, distinct from an unsigned byte.
    pub is_bool: bool,
}
/// Independent EDG-authoritative presence proof for scalar ABI records
/// 46 (integer), 57 (standard FP) and 43 (signed i64). All three can be
/// legitimately absent for unsupported/incomplete scalar type categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeScalarMetadataProof {
    pub owner: TypeId,
    pub integer_abi: bool,
    pub standard_float_abi: bool,
    pub signed_i64: bool,
}
/// Exact standard float/double source category, excluding long double, half,
/// extended, imaginary and complex types even if their sizes coincide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CFloatType {
    pub id: TypeId,
    pub bits: u8,
}
/// Exact EDG ck_float payload of a primary expression, keyed by full
/// function/statement/expression path to prevent cross-expression aliasing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloatConstant {
    pub function: String,
    pub statement_path: String,
    pub path: String,
    pub ty: TypeId,
    /// Precisely 32 or 64; only IEEE binary32/binary64 are represented.
    pub width: u8,
    pub bits: u64,
}
/// Constant-initialized FP local without a primary EDG expression tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloatInitializer {
    pub function: String,
    pub statement_path: String,
    pub symbol: SymbolId,
    pub ty: TypeId,
    pub width: u8,
    pub bits: u64,
}

/// Exact EDG ck_float initializer for a native file-scope definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalFloatInitializer {
    pub symbol: SymbolId,
    pub ty: TypeId,
    pub width: u8,
    pub bits: u64,
}

/// EDG-native storage classification of a file-scope variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlobalVariable {
    pub symbol: SymbolId,
    /// 1=extern declaration, 2=internal static, 3=other file-scope.
    pub linkage: u32,
    pub explicit_initializer: bool,
    pub thread_local: bool,
    pub local_to_function: bool,
}

/// The source EDG variable properties that distinguish template metadata from
/// real executable storage; none of these bits alone is a startup/ABI proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalMaterialization {
    pub symbol: SymbolId,
    pub is_nonreal: bool,
    pub is_prototype_instantiation: bool,
    pub is_template_variable: bool,
    pub compiler_generated: bool,
    pub used: bool,
    pub address_taken: bool,
    pub is_class_member: bool,
    /// EDG's *applied* GNU weak flag; storage class alone is not a binding proof.
    pub is_weak: bool,
    /// Weak alias declaration, not an ordinary weak object definition.
    pub is_weakref: bool,
    /// GNU alias declaration, requiring a separately verified alias target.
    pub is_gnu_alias: bool,
    /// An explicit GNU/MS data section is present. Its full section name is
    /// not yet exported, so this flag MUST NOT authorize ELF emission.
    pub has_explicit_section: bool,
    /// Exact native EDG data section name (not an inferred ELF section).
    pub section: Option<String>,
    /// Native EDG an_ELF_visibility_kind: unspecified=0, hidden=1,
    /// protected=2, internal=3, default=4. None means unavailable.
    pub elf_visibility: Option<u32>,
}

/// Whether EDG's separately scheduled file-scope initialization list is available.
/// Even a complete empty list does not certify a program as free of effects
/// unless all other EDG initialization modes are accounted for.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct FileScopeInitSchedule {
    pub available: bool,
    pub complete: bool,
}

/// Source-owned ordered routine ID, not an inferred constructor ABI call.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct FileScopeInitRoutine {
    pub position: u32,
    pub routine: SymbolId,
}

/// Native EDG a_variable::init_routine.dynamic_init_routine. No name or
/// generated call-order reconstruction. This is a source-owned mapping
/// from a file/namespace variable to its separate initialization routine.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct FileScopeDynamicInitOwner {
    pub variable: SymbolId,
    pub routine: SymbolId,
}

/// Lowered EDG compiler-created static __link object containing a native
/// ck_address/abk_routine initializer. Neither object spellings nor mangled
/// routine names are used to determine the routine identity.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct FileScopeRuntimeLink {
    pub link_variable: SymbolId,
    pub startup_routine: SymbolId,
}

/// EDG lowered startup routine defined in the file scope. Priority zero
/// means no explicit GNU init_priority; it does not imply ELF section order.
/// Runtime destructor registration and cross-TU order remain unproven.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct FileScopeLoweredStartupRoutine {
    pub routine: SymbolId,
    pub priority: u32,
}

/// Source-level initialization entries in an EDG file/namespace scope.
/// A scope-local ordinal does not establish a cross-scope ELF init priority.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ScopeDynamicInitializer {
    pub scope: String,
    pub position: u32,
    pub variable: Option<SymbolId>,
    pub destructor: Option<SymbolId>,
    pub kind: u32,
}

/// Source-owned EDG namespace/file-scope asm declaration (wire 101).
/// Raw assembly text is untrusted and never admitted to the compiler backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeScopeAsm {
    pub scope: String,
    pub position: u32,
    pub assembly: String,
    /// EDG GNU asm form/volatile/keyword/inline/goto flags (low five bits).
    pub flags: u32,
}

/// Source-owned EDG ck_address/abk_variable target. Expression snapshots
/// use the exact (function, statement_path, expression_path) identity;
/// for a folded dik_constant pointer stmk_init with no expression tree,
/// path "0" belongs to the native statement-initializer owner (event 34).
/// Never interpret the pointer's runtime bits as a data-symbol identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstantAddress {
    pub function: String,
    pub statement_path: String,
    pub path: String,
    pub symbol: SymbolId,
    pub byte_offset: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstantRoutineAddress {
    pub function: String,
    pub statement_path: String,
    pub path: String,
    pub symbol: SymbolId,
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
    /// Identity of the native EDG field, matching enk_field operands.
    pub symbol: SymbolId,
    pub name: String,
    pub byte_offset: u64,
    pub bit_offset: u32,
    /// Zero for ordinary fields; nonzero for bit-fields.
    pub bit_width: u32,
}

/// Independent count proof for a native struct/class/union field list and
/// direct C++ bases; counts are not reconstructed from wire edge records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeRecordShape {
    pub owner: TypeId,
    pub field_count: u32,
    pub direct_base_count: u32,
}

/// Independent native EDG completeness proof for object layout and array bound.
/// Neither presence flag is inferred from emitted wire records 49 or 9.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeTypeLayoutProof {
    pub owner: TypeId,
    pub has_layout: bool,
    pub has_fixed_array_bound: bool,
}

/// Native byte size of a complete EDG type. Alignment is not included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeObjectSize {
    pub owner: TypeId,
    pub bytes: u64,
    /// Native EDG object alignment in bytes.
    pub alignment: u64,
}

/// EDG class-scope proof for native P0 local object storage. False means
/// the type can carry methods, nontrivial specials or source-initialized
/// subobjects. This does not authorize C++ by-value ABI or member calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CxxRecordAdmission {
    pub owner: TypeId,
    pub plain_object: bool,
}

/// EDG source-layout proof for a nontrivial-dtor record containing
/// exactly one 32-bit scalar field, no bases, virtual dispatch, static
/// members or default field initializers. The class routine list may
/// already be pruned: true does NOT prove the constructor/destructor
/// count or semantics. A separate native TypeId verifier must check
/// retained member identities, special-member ABI and every cleanup path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CxxScalarLifetimeAdmission {
    pub owner: TypeId,
    pub candidate: bool,
}

/// A direct C++ base class, in declaration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseClass {
    pub derived: TypeId,
    pub position: u32,
    pub base: TypeId,
    pub is_virtual: bool,
}

/// Byte offset of a direct nonvirtual base within the derived object.
/// None means EDG has NOT established a layout offset; zero is a real offset.
/// Virtual bases are never admitted through this record.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct BaseSubobjectOffset {
    pub derived: TypeId,
    pub position: u32,
    pub byte_offset: Option<u64>,
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
/// EDG-owned number of constructor/destructor initializer entries (wire 92).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeConstructorInitializerCount {
    pub routine: SymbolId,
    pub count: u32,
}

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
    /// EDG-native compiler-generated function classification; never infer from spelling.
    pub compiler_generated: bool,
}

/// EDG source-owned *logical* virtual method metadata. The native number
/// is not an Itanium table slot/address point, nor does the class offset
/// prove live storage, adjusted receivers or exception-safe dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeVirtualMemberInfo {
    pub routine: SymbolId,
    pub owner: TypeId,
    pub virtual_number: Option<u64>,
    pub virtual_info_offset: Option<u64>,
    pub sharing_base: Option<TypeId>,
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

/// Independent native EDG proof of routine supplement presence and the
/// source parameter-list length, not reconstructed from event 8 edges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeRoutineSupplement {
    pub owner: TypeId,
    pub has_extra_info: bool,
    pub parameter_count: u32,
}

/// A statement node, identified by a stable path within one function.
/// Kinds are EDG's a_statement_kind discriminants, not Rust syntax kinds.
/// EDG-authored child cardinality for one supplemental expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeControlRootAdmission {
    pub function:String, pub statement_path:String, pub mask:u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeConstructorRootAdmission {
    pub routine:SymbolId, pub position:u32, pub present:bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeControlChildCount {
    pub function:String, pub statement_path:String, pub role:u32,
    pub path:String, pub count:u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeInitializerChildCount {
    pub owner:SymbolId, pub path:String, pub count:u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeConstructorChildCount {
    pub routine:SymbolId, pub position:u32, pub path:String, pub count:u32,
}

/// Native EDG count witness, independent of emitted child events (wire 83).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeExpressionChildCount {
    pub function: String,
    pub statement_path: String,
    pub path: String,
    pub count: u32,
}

/// Native EDG sibling-list witness (wire 82). Parent is absent for roots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeStatementSiblingCount {
    pub function: String,
    pub parent: Option<String>,
    pub edge: String,
    pub count: u32,
}

/// Independent EDG-native handler-list cardinality witness for a try block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeTryHandlerCount {
    pub function: String,
    pub statement_path: String,
    pub count: u32,
}

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

/// Original nonportable EDG exception action at an owned expression path
/// (wire 106). A zero role refers to a normal expression, roles 1..=4
/// to independent native control-expression trees. `subkind` is EDG's
/// native leck_* enum, not a target-independent EH operation.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct NativeRetainedEhConstruct {
    pub function:String,
    pub statement_path:String,
    pub role:u32,
    pub path:String,
    pub expression_kind:u32,
    pub subkind:u32,
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

/// Native `stmk_set_vla_size`: the bound is evaluated at this program point,
/// not when the array type is first visited. An expression, if present,
/// resides in the ordinary statement expression tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VlaDimension {
    pub function: String,
    pub statement_path: String,
    pub array_type: TypeId,
    pub has_expression: bool,
    /// EDG's copied dimensions reuse an original dimension's stored value.
    /// Their identity must be resolved before code generation.
    pub copied: bool,
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

/// Constant scalar value of a dynamic stmk_init execution point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementIntegerInitializer {
    pub function: String,
    pub statement_path: String,
    pub value: i64,
}

/// Exact routine address in a folded dynamic initializer (ck_address).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementRoutineInitializer {
    pub function: String,
    pub statement_path: String,
    pub symbol: SymbolId,
}

/// Native constant aggregate data, captured while EDG IL pointers are valid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementAggregateInitializer {
    pub function: String,
    pub statement_path: String,
    pub symbol: SymbolId,
    pub ty: TypeId,
    pub bytes: Vec<u8>,
}
/// Fully sized, native constant object representation of a defined global.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalAggregateInitializer {
    pub symbol: SymbolId,
    pub ty: TypeId,
    pub bytes: Vec<u8>,
}

/// One pointer-sized ELF data relocation within a native aggregate image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalAggregateRelocation {
    pub symbol: SymbolId,
    pub offset: u64,
    pub target: SymbolId,
    pub addend: i64,
}

/// A complete immutable native EDG string-object image. The 1-based ID is
/// unique in this translation unit, not a pointer value or a text hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalLiteralObject {
    pub id: u32,
    pub ty: TypeId,
    pub bytes: Vec<u8>,
}

/// A symbolic pointer in a source-owned file-scope global initializer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalLiteralRelocation {
    pub symbol: SymbolId,
    pub offset: u64,
    pub literal_id: u32,
    pub addend: i64,
}

/// One exact source routine target and absolute 64-bit static pointer slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalRoutineRelocation {
    pub symbol: SymbolId,
    pub offset: u64,
    pub routine: SymbolId,
    pub addend: i64,
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
/// EDG-owned destruction-list cardinality (wire event 93).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeLifetimeDestructionCount {
    pub routine: SymbolId,
    pub lifetime: u32,
    pub count: u32,
}

/// EDG-derived child_lifetime count, independent of exported child nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeLifetimeChildCount {
    pub routine: SymbolId,
    pub lifetime: u32,
    pub count: u32,
    /// True only when reached via the EDG child_lifetime list. A scope
    /// fallback may have a parent pointer but is not a listed child.
    pub native_child_edge: bool,
}

/// Defined-function owner proof independent of lifetime record emission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeLifetimeRootAdmission {
    pub routine: SymbolId,
    pub body: Option<u32>,
    pub local_static: Option<u32>,
}

/// Every native local scope owns one nullable lifetime relationship.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeScopeLifetimeAdmission {
    pub routine: SymbolId,
    pub scope: u32,
    pub lifetime: Option<u32>,
}

/// Independent EDG scope-child linked-list cardinality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeScopeChildCount {
    pub routine: SymbolId,
    pub scope: u32,
    pub count: u32,
}

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

/// Case/default constant associated with a native stmk_switch_case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementSwitchCase {
    pub function: String,
    pub statement_path: String,
    /// 0=default, 1=integer constant, 2=unsupported (GNU range/dependent).
    pub status: u32,
    /// Signed i64 bit-carrier retaining unsigned 64-bit switch values.
    pub value: Option<i64>,
}

/// 1=break goto, 2=continue goto, 3=break label, 4=continue label,
/// 5=switch-break goto, 6=switch-break label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopTransfer {
    pub function: String,
    pub statement_path: String,
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

/// One EDG-owned narrow string literal stored as a complete, NUL-ended
/// constant array image. The source path identifies the expression, while
/// its type ID and bytes establish exact allocation size. No linker name
/// is emitted by EDG for the literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionStringLiteral {
    pub function: String,
    pub statement_path: String,
    pub path: String,
    pub ty: TypeId,
    pub bytes: Vec<u8>,
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

/// Count of EDG's native explicit parameter chain and implicit this owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeFunctionParameterCount {
    pub routine: SymbolId,
    pub explicit: u32,
    pub implicit_this: bool,
}

/// Independently counted emitted-eligible EDG scope locals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeScopeLocalCount {
    pub routine: SymbolId,
    pub scope: u32,
    pub count: u32,
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallSite {
    pub function: String,
    pub statement_path: String,
    pub path: String,
    pub first_argument: u32,
}

/// Callee classification from EDG's original operand-zero IL node.
/// This does not include a vtable ABI, virtual slot, receiver adjustment or
/// permission to execute an indirect call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallTargetForm {
    DirectRoutine,
    ConstantRoutineAddress,
    IndirectUnresolved,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallTargetProvenance {
    pub function:String,
    pub statement_path:String,
    pub path:String,
    pub form:CallTargetForm,
    pub callee_type:Option<TypeId>,
    pub callee_symbol:Option<SymbolId>,
}

/// Original native EDG call opcode classification, before bridge inference.
/// OrdinaryOrLowered is deliberately *not* a nonvirtual certificate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallDispatchClass {
    OrdinaryOrLowered,
    Member,
    PointerToMember,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallDispatchProvenance {
    pub function:String,
    pub statement_path:String,
    pub path:String,
    pub native_operator:u32,
    pub class:CallDispatchClass,
    /// Exact EDG operation.is_virtual_call flag, meaningful only for Member.
    /// This is NOT a vtable slot, receiver adjustment, or live-object proof.
    pub explicitly_virtual:bool,
}

/// Source-owned callbacks from both *before* and *after* EDG rewrites a
/// virtual member call into an ordinary indirect eok_call. The bridge
/// requires a structurally valid rewrite of the exact call and an unchanged
/// callee pointer in the final snapshot. Copying/inlining may still lose
/// the witness, and the original receiver tree may be duplicated/rewritten.
/// This is never a virtual ABI slot, live-object certificate, receiver
/// adjustment, or permission to execute indirect calls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrelowerVirtualCallOrigin {
    pub function:String,
    pub statement_path:String,
    pub path:String,
    pub original_routine:SymbolId,
    pub original_receiver_type:Option<TypeId>,
    pub original_operator:u32,
    /// Whether the original receiver node address equals the post-lowered
    /// call's first object argument node. Address identity only.
    pub receiver_node_retained:bool,
}

/// Event 81: the exact reusable receiver node returned by EDG's
/// get_virtual_function_address is still a unique direct argument of the
/// paired, source-confirmed lowered eok_call. The index is zero-based
/// among call arguments (not including the callee operand).
///
/// Neither EDG's pointer reuse nor its source-to-copy transformation proves
/// the subobject address, vtable address point, thunk, this-adjustment,
/// lifetime, or permission to generate an indirect call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualReceiverReuse {
    pub function:String,
    pub statement_path:String,
    pub path:String,
    pub receiver_type:TypeId,
    pub argument_position:u32,
    pub original_node_reused:bool,
}

/// Event 82: source-owned EDG Itanium-class-ABI vptr index expression.
/// The exact native index constant must still appear in the same
/// source-certified call's final callee subtree. The element width comes
/// from EDG vtbl_entry_size, not a guessed pointer/slot width. These
/// facts are not a proof of any live object's vtable address point or
/// executable virtual dispatch ABI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualEntryIndexSelection {
    pub function:String,
    pub statement_path:String,
    pub path:String,
    pub source_index:u64,
    pub entry_bytes:u64,
}

/// Event 85: EDG native virtual vptr lookup and reusable call-argument
/// linkage. The exact lookup-object node remains in the vptr subtree;
/// its EDG-created reusable receiver remains the same final call's unique
/// direct argument. No identity of runtime subobjects, address point,
/// pointer adjustments, thunk or lifetime semantics is certified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualVptrReceiverPair {
    pub function:String,
    pub statement_path:String,
    pub path:String,
    pub lookup_object_type:TypeId,
    pub argument_position:u32,
}

/// Original source-operand C type for a native compiler-generated pointer
/// cast. Explicit source casts never emit this event. The expression path
/// identifies the cast wrapper, not its child or surrounding call.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImplicitPointerCast {
    pub function: String,
    pub statement_path: String,
    pub path: String,
    pub source_type: TypeId,
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

// Return the components of a demangled qualified name before its
// top-level parameter list. "::" inside template arguments is not a
// namespace boundary of the containing declaration.
fn qualified_scope_components(name: &str) -> (Vec<&str>, usize) {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut i = 0;
    let mut template_depth = 0usize;
    let bytes = name.as_bytes();
    while i < bytes.len() {
        match bytes[i] {
            b'<' => template_depth += 1,
            b'>' => template_depth = template_depth.saturating_sub(1),
            b'(' if template_depth == 0 => break,
            b':' if template_depth == 0 && bytes.get(i + 1) == Some(&b':') => {
                parts.push(&name[start..i]);
                i += 2;
                start = i;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    parts.push(&name[start..i]);
    (parts, i)
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
            // Only split top-level scope separators. C++ class names can
            // contain qualified template arguments (Map<other::Key>), and
            // function signatures can contain qualified parameter types.
            // The caller still treats the portion after a top-level '(' as
            // part of the final declaration name, not a namespace.
            let (parts, prefix_end) = qualified_scope_components(qualified);
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

// Native EDG record identities are unique within one compilation. Indexing
// them during callback collection avoids rescanning all prior records for
// every emitted type, symbol, layout, or C++ admission (quadratic growth).
// The final public Il remains the same ordered Vec-backed snapshot.
#[derive(Default)]
struct WireSeen {
    types: HashSet<TypeId>,
    layouts: HashSet<TypeId>,
    c_integer_types: HashSet<TypeId>,
    c_float_types: HashSet<TypeId>,
    /// Event 58 is unique per primary source expression (not per IEEE value).
    float_constant_sites: HashSet<(String,String,String)>,
    /// Event 59 is unique per local source initialization site and symbol.
    float_initializer_sites: HashSet<(String,String,SymbolId)>,
    /// Event 60 describes at most one exact image per defined global.
    global_float_images: HashSet<SymbolId>,
    signed_i64_types: HashSet<TypeId>,
    symbol_types: HashSet<SymbolId>,
    globals: HashSet<SymbolId>,
    cxx_records: HashSet<TypeId>,
    cxx_lifetimes: HashSet<TypeId>,
    routine_supplements: HashSet<TypeId>,
    native_record_shapes: HashSet<TypeId>,
    native_type_layout_proofs: HashSet<TypeId>,
    native_scalar_proofs: HashSet<TypeId>,
    symbols: HashSet<SymbolId>,
    routine_symbols: HashSet<SymbolId>,
    /// Only source-correspondence routines and variables can carry raw linker names.
    linkable_symbols: HashSet<SymbolId>,
    source_link_names: HashSet<SymbolId>,
    base_subobject_offsets: HashSet<(TypeId,u32)>,
    /// The next EDG-native file/namespace-scope asm ordinal, per exact scope.
    /// Avoid rescanning all earlier directives for each newly emitted record.
    native_scope_asm_next: HashMap<String, u32>,
    /// Next source-owned dynamic initializer ordinal per exact native scope.
    /// Repeated callbacks must not rescan the full translation-unit roster.
    scope_dynamic_initializer_next: HashMap<String, u32>,
    /// Event 72: each native runtime link variable has exactly one startup target.
    file_scope_runtime_link_variables: HashSet<SymbolId>,
    file_scope_dynamic_init_owner_variables: HashSet<SymbolId>,
    native_retained_eh_construct_keys: HashSet<(String,String,u32,String)>,
    /// Event 73: each native lowered startup routine has exactly one priority.
    file_scope_lowered_startup_routines: HashSet<SymbolId>,
}
#[derive(Default)]
struct Collector {
    il: Il,
    panicked: bool,
    malformed: bool,
    seen: WireSeen,
}

/// Never allow a Rust panic to cross the C ABI into EDG.
/// Native callbacks are synchronous and serialized by COMPILER_LOCK.
unsafe extern "C" fn collect(context: *mut c_void, kind: u32,
                              name: *const WireField, length: usize, defined: u32) {
    if context.is_null() { return; }
    let state = &mut *(context as *mut Collector);
    if state.panicked || state.malformed { return; }
    // Retired private EDG lowering hooks have no place in the live back-end
    // protocol. Keep their historical wire shape for fixture migration, but
    // reject these records even if a stale or mixed-version native producer
    // still emits a syntactically valid event.
    if matches!(kind, 80 | 81 | 82 | 85) {
        state.malformed = true;
        return;
    }
    // Reject unknown event IDs, arity mismatches and missing field arrays
    // before constructing a slice from the C++ callback pointer. In
    // particular, a bogus length must never reach from_raw_parts.
    if expected_wire_field_count(kind) != Some(length) ||
        (length != 0 && name.is_null()) {
        state.malformed = true;
        return;
    }
    let Some(before) = callback_record_count(&state.il, kind) else {
        state.malformed = true;
        return;
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        collect_event(&mut state.il, &mut state.seen, kind, name, length, defined);
    }));
    if result.is_err() {
        state.panicked = true;
    } else if callback_record_count(&state.il, kind) == Some(before) {
        state.malformed = true;
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
        71 => Some(il.routine_supplements.len()),
        72 => Some(il.file_scope_runtime_links.len()),
        73 => Some(il.file_scope_lowered_startup_routines.len()),
        74 => Some(il.native_record_shapes.len()),
        75 => Some(il.native_type_layout_proofs.len()),
        76 => Some(il.native_scalar_proofs.len()),
        77 => Some(il.call_targets.len()),
        78 => Some(il.call_dispatches.len()),
        79 => Some(il.native_virtual_members.len()),
        80 => Some(il.prelower_virtual_origins.len()),
        81 => Some(il.virtual_receiver_reuses.len()),
        82 => Some(il.virtual_entry_indexes.len()),
        83 => Some(il.native_expression_child_counts.len()),
        84 => Some(il.native_statement_sibling_counts.len()),
        85 => Some(il.virtual_vptr_receiver_pairs.len()),
        86 => Some(il.native_try_handler_counts.len()),
        87 => Some(il.native_control_child_counts.len()),
        88 => Some(il.native_initializer_child_counts.len()),
        89 => Some(il.native_constructor_child_counts.len()),
        90 => Some(il.native_control_root_admissions.len()),
        91 => Some(il.native_constructor_root_admissions.len()),
        92 => Some(il.native_constructor_initializer_counts.len()),
        93 => Some(il.native_lifetime_destruction_counts.len()),
        94 => Some(il.native_lifetime_child_counts.len()),
        95 => Some(il.native_lifetime_root_admissions.len()),
        96 => Some(il.native_scope_lifetime_admissions.len()),
        97 => Some(il.native_scope_child_counts.len()),
        98 => Some(il.native_scope_local_counts.len()),
        99 => Some(il.native_function_parameter_counts.len()),
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
        56 => Some(il.vla_dimensions.len()),
        34 => Some(il.statement_initializations.len()),
        35 => Some(il.statement_sources.len()),
        36 => Some(il.symbol_sources.len()),
        37 => Some(il.expression_sources.len()),
        38 => Some(il.object_lifetimes.len()),
        39 => Some(il.scope_lifetimes.len()),
        40 => Some(il.initialization_lifetimes.len()),
        41 => Some(il.lifetime_destructions.len()),
        42 => Some(il.routine_identities.len()),
        43 => Some(il.signed_i64_types.len()),
        44 => Some(il.loop_transfers.len()),
        45 => Some(il.statement_integer_initializers.len()),
        54 => Some(il.statement_routine_initializers.len()),
        55 => Some(il.constant_routine_addresses.len()),
        46 => Some(il.c_integer_types.len()),
        57 => Some(il.c_float_types.len()),
        58 => Some(il.float_constants.len()),
        59 => Some(il.float_initializers.len()),
        60 => Some(il.global_float_initializers.len()),
        // Record 61/62 decoding already exists below. Without these counts,
        // rejected native events are silently dropped instead of marking the
        // compilation malformed.
        61 => Some(il.symbol_link_names.len()),
        62 => Some(il.string_literals.len()),
        47 => Some(il.global_variables.len()),
        100 => Some(il.global_materializations.len()),
        101 => Some(il.native_scope_asm.len()),
        48 => Some(il.constant_addresses.len()),
        49 => Some(il.type_object_sizes.len()),
        63 => Some(il.cxx_record_admissions.len()),
        64 => Some(usize::from(il.cpp_language.is_some())),
        65 => Some(il.cxx_scalar_lifetime_admissions.len()),
        66 => Some(usize::from(il.file_scope_init_schedule.is_some())),
        67 => Some(il.file_scope_init_routines.len()),
        104 => Some(il.file_scope_dynamic_init_owners.len()),
        68 => Some(il.scope_dynamic_initializers.len()),
        69 => Some(il.base_subobject_offsets.len()),
        50 => Some(il.statement_aggregate_initializers.len()),
        51 => Some(il.global_aggregate_initializers.len()),
        52 => Some(il.global_aggregate_relocations.len()),
        102 => Some(il.global_literal_objects.len()),
        103 => Some(il.global_literal_relocations.len()),
        105 => Some(il.global_routine_relocations.len()),
        106 => Some(il.native_retained_eh_constructs.len()),
        53 => Some(il.statement_switch_cases.len()),
        18 => Some(il.calls.len()),
        70 => Some(il.implicit_pointer_casts.len()),
        _ => None,
    }
}

// The shared limit mirrors cpp/bridge.cpp::MAX_NATIVE_AGGREGATE_IMAGE_BYTES.
// C++ sends hex images with two ASCII characters per byte.
use crate::limits::MAX_NATIVE_AGGREGATE_IMAGE_BYTES;
const MAX_NATIVE_AGGREGATE_HEX_CHARS: usize = MAX_NATIVE_AGGREGATE_IMAGE_BYTES * 2;

/// Decode a bounded native byte image. Reject arbitrary strings and odd
/// lengths; a malformed wire event must not become a partial initializer.
fn decode_aggregate_hex(input: &str) -> Option<Vec<u8>> {
    let raw=input.as_bytes();
    if raw.is_empty() || raw.len()>MAX_NATIVE_AGGREGATE_HEX_CHARS || !raw.len().is_multiple_of(2) {return None;}
    let hex=|b:u8| ->Option<u8> {
        match b {
            b'0'..=b'9'=>Some(b-b'0'),
            b'a'..=b'f'=>Some(b-b'a'+10),
            _=>None,
        }
    };
    let mut out=Vec::with_capacity(raw.len()/2);
    for pair in raw.as_chunks::<2>().0 {
        out.push((hex(pair[0])? << 4) | hex(pair[1])?);
    }
    Some(out)
}

// Keep the decoder's arity table separately testable from record collection.
// Every kind with a wire arity must also have a destination count below.
unsafe fn collect_event(output: &mut Il, seen: &mut WireSeen, kind: u32,
                        data: *const WireField, length: usize, aux: u32) {
    // This preflight must precede from_raw_parts even for direct callers
    // of collect_event: never construct a slice with an untrusted arity.
    let Some(expected) = expected_wire_field_count(kind) else {return;};
    if length != expected || (length != 0 && data.is_null()) {return;}
    let fields = if length == 0 { &[][..] } else {
        std::slice::from_raw_parts(data, length)
    };
    if fields.iter().any(|f|
        f.tag > wire_tags::MAX ||
        (f.tag == wire_tags::TEXT &&
            (f.data.is_null() || f.length > MAX_WIRE_TEXT_BYTES))) { return; }
    if let Some(masks) = expected_wire_field_tag_masks(kind) {
        if fields.len() != masks.len() || fields.iter().zip(masks).any(|(field, mask)| {
            (mask & (1u8 << field.tag)) == 0
        }) {
            return;
        }
    }
    let t = |i: usize| fields[i].string();
    let n = |i: usize| fields[i].u32();
    match kind {
        83 => {
            let (Some(function),Some(statement_path),Some(path),Some(count))=
                (t(0),t(1),t(2),n(3)) else {return;};
            if function.is_empty() || statement_path.is_empty() ||
               path.is_empty() || count>10000 {return;}
            output.native_expression_child_counts.push(NativeExpressionChildCount {
                function,statement_path,path,count,
            });
        }
        84 => {
            let (Some(function),Some(edge),Some(count))=
                (t(0),t(2),n(3)) else {return;};
            if function.is_empty() || count>10000 {return;}
            let parent=if fields[1].present() {
                let Some(parent)=t(1) else {return;};
                if parent.is_empty() {return;}
                Some(parent)
            } else {None};
            output.native_statement_sibling_counts.push(NativeStatementSiblingCount {
                function,parent,edge,count,
            });
        }
        101 => {
            let (Some(scope),Some(position),Some(assembly),Some(flags))=
                (t(0),n(1),t(2),n(3)) else {return;};
            if scope.is_empty() || scope.len()>1024 || (flags & !31)!=0 {return;}
            // EDG emits a contiguous ordinal sequence independently in each
            // scope. Validate in O(1) expected time per callback, not by
            // repeatedly scanning the entire compiled directive roster.
            let expected=seen.native_scope_asm_next.get(&scope).copied().unwrap_or(0);
            if position!=expected {return;}
            let Some(next)=position.checked_add(1) else {return;};
            seen.native_scope_asm_next.insert(scope.clone(),next);
            output.native_scope_asm.push(NativeScopeAsm {
                scope,position,assembly,flags,
            });
        }
        69 => {
            let (Some(derived),Some(position))=(n(0),n(1)) else {return;};
            if derived==0 || !seen.base_subobject_offsets.insert((TypeId(derived),position)) {return;}
            output.base_subobject_offsets.push(BaseSubobjectOffset {
                derived:TypeId(derived), position,
                byte_offset:fields[2].unsigned(),
            });
        }
        66 => {
            let (Some(available),Some(complete))=(n(0),n(1)) else {return;};
            // A source that reports no separate scheduler cannot also
            // export ordered roots from that scheduler. Record 66 normally
            // follows record 67; check either callback ordering to fail closed.
            if available>1 || complete>1 || (available==0 && complete!=0) ||
               output.file_scope_init_schedule.is_some() ||
               (available==0 && !output.file_scope_init_routines.is_empty()) {return;}
            output.file_scope_init_schedule=Some(FileScopeInitSchedule {
                available:available==1,complete:complete==1,
            });
        }
        68 => {
            let (Some(scope),Some(position),Some(kind))=(t(0),n(1),n(4)) else {return;};
            if scope.len()>1024 {return;}
            let variable=n(2).map(SymbolId);
            let destructor=n(3).map(SymbolId);
            if variable==Some(SymbolId(0)) || destructor==Some(SymbolId(0)) {return;}
            let expected=seen.scope_dynamic_initializer_next
                .get(&scope).copied().unwrap_or(0);
            if position!=expected {return;}
            let Some(next)=position.checked_add(1) else {return;};
            seen.scope_dynamic_initializer_next.insert(scope.clone(),next);
            output.scope_dynamic_initializers.push(ScopeDynamicInitializer {
                scope,position,variable,destructor,kind,
            });
        }
        67 => {
            let (Some(position),Some(routine))=(n(0),n(1)) else {return;};
            if routine==0 ||
               matches!(output.file_scope_init_schedule,
                        Some(FileScopeInitSchedule{available:false,..})) ||
               position as usize != output.file_scope_init_routines.len() {return;}
            output.file_scope_init_routines.push(FileScopeInitRoutine {
                position,routine:SymbolId(routine)
            });
        }
        106 => {
            let (Some(function),Some(statement_path),Some(role),
                 Some(path),Some(expression_kind),Some(subkind))=
                (t(0),t(1),n(2),t(3),n(4),n(5)) else {return;};
            // This record is emitted immediately *after* the original
            // expression event. Reject synthetic, mismatched or duplicate
            // action identities; preserve only the EDG-owned native enum.
            let source_present=if role==0 {
                output.expressions.iter().any(|e|
                    e.function==function &&
                    e.statement_path==statement_path &&
                    e.path==path && e.kind==expression_kind)
            } else {
                output.control_expressions.iter().any(|e|
                    e.function==function &&
                    e.statement_path==statement_path &&
                    e.role==role && e.path==path && e.kind==expression_kind)
            };
            if role>4 || subkind>11 || path.is_empty() ||
               !source_present ||
               !seen.native_retained_eh_construct_keys.insert((
                   function.clone(),statement_path.clone(),role,path.clone())) {
                return;
            }
            output.native_retained_eh_constructs.push(NativeRetainedEhConstruct {
                function,statement_path,role,path,expression_kind,subkind,
            });
        }
        104 => {
            let (Some(variable),Some(routine))=(n(0),n(1)) else {return;};
            let variable=SymbolId(variable);
            let routine=SymbolId(routine);
            if variable.0==0 || routine.0==0 ||
               !seen.file_scope_dynamic_init_owner_variables.insert(variable) {
                return;
            }
            output.file_scope_dynamic_init_owners.push(FileScopeDynamicInitOwner {
                variable,routine,
            });
        }
        72 => {
            let (Some(link_variable),Some(startup_routine))=(n(0),n(1)) else {return;};
            if link_variable==0 || startup_routine==0 ||
               !seen.file_scope_runtime_link_variables.insert(SymbolId(link_variable)) {return;}
            output.file_scope_runtime_links.push(FileScopeRuntimeLink {
                link_variable:SymbolId(link_variable),
                startup_routine:SymbolId(startup_routine),
            });
        }
        73 => {
            let (Some(routine),Some(priority))=(n(0),n(1)) else {return;};
            if routine==0 ||
               !seen.file_scope_lowered_startup_routines.insert(SymbolId(routine)) {return;}
            output.file_scope_lowered_startup_routines.push(FileScopeLoweredStartupRoutine {
                routine:SymbolId(routine),priority,
            });
        }
        1..=4 => {
            let Some(name) = t(0) else { return; };
            // Only routine-definition records carry a boolean auxiliary
            // value; all other entity kinds reserve it as zero.
            if (kind == 1 && aux > 1) || (kind != 1 && aux != 0) { return; }
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
            if id==0 || !seen.types.insert(TypeId(id)) {return;}
            output.types.push(TypeDef { id: TypeId(id), kind,
                related_role: (role != 0).then_some(role),
                related: (child != 0).then_some(TypeId(child)) });
        }
        43 => {
            let Some(id) = n(0) else { return; };
            if id == 0 || !seen.signed_i64_types.insert(TypeId(id)) { return; }
            output.signed_i64_types.push(TypeId(id));
        }
        44 => {
            let (Some(function), Some(statement_path), Some(kind)) =
                (t(0), t(1), n(2)) else { return; };
            if !(1..=6).contains(&kind) { return; }
            output.loop_transfers.push(LoopTransfer { function, statement_path, kind });
        }
        53 => {
            let (Some(function),Some(statement_path),Some(status)) =
                (t(0),t(1),n(2)) else {return;};
            if status>2 || (status==1)!=fields[3].signed().is_some() {
                return;
            }
            output.statement_switch_cases.push(StatementSwitchCase {
                function,statement_path,status,value:fields[3].signed(),
            });
        }
        102 => {
            let (Some(id),Some(ty),Some(hex))=(n(0),n(1),t(2)) else {return;};
            // The bridge assigns IDs by first observed original EDG constant
            // identity, starting at one. Incomplete or duplicate pools fail.
            if id==0 || id>256 || ty==0 ||
                id as usize!=output.global_literal_objects.len()+1 {
                return;
            }
            let Some(bytes)=decode_aggregate_hex(&hex) else {return;};
            if bytes.is_empty() || bytes.len()>MAX_NATIVE_AGGREGATE_IMAGE_BYTES ||
                bytes.last()!=Some(&0) {return;}
            output.global_literal_objects.push(GlobalLiteralObject {
                id,ty:TypeId(ty),bytes,
            });
        }
        105 => {
            let (Some(symbol),Some(offset),Some(routine),Some(addend))=
                (n(0),fields[1].unsigned(),n(2),fields[3].signed())
                else {return;};
            let owner=SymbolId(symbol);
            let target=SymbolId(routine);
            let Some(image)=output.global_aggregate_initializers.iter()
                .find(|g|g.symbol==owner) else {return;};
            let Ok(start)=usize::try_from(offset) else {return;};
            if symbol==0 || routine==0 || offset%8!=0 || addend!=0 ||
                !output.symbols.iter().any(|s|s.id==target && s.kind==1) ||
                output.global_routine_relocations.len()>=4096 ||
                start.checked_add(8).is_none_or(|end|end>image.bytes.len()) ||
                image.bytes[start..start+8].iter().any(|&b|b!=0) ||
                output.global_routine_relocations.iter().any(|p|
                    p.symbol==owner && p.offset==offset) ||
                output.global_literal_relocations.iter().any(|p|
                    p.symbol==owner && p.offset==offset) ||
                output.global_aggregate_relocations.iter().any(|p|
                    p.symbol==owner && p.offset==offset) {
                return;
            }
            output.global_routine_relocations.push(GlobalRoutineRelocation {
                symbol:owner,offset,routine:target,addend,
            });
        }
        103 => {
            let (Some(symbol),Some(offset),Some(literal_id),Some(addend))=
                (n(0),fields[1].unsigned(),n(2),fields[3].signed())
                else {return;};
            // Event 51 is emitted before source-owned relocation events.
            // Reject forged, dangling, out-of-bounds, nonzero, or duplicated
            // slots before a later consumer could turn them into linker data.
            let owner=SymbolId(symbol);
            let Some(image)=output.global_aggregate_initializers.iter()
                .find(|g|g.symbol==owner) else {return;};
            let Ok(start)=usize::try_from(offset) else {return;};
            if symbol==0 || literal_id==0 || literal_id>256 ||
                !output.global_literal_objects.iter().any(|v|v.id==literal_id) ||
                offset%8!=0 || output.global_literal_relocations.len()>=4096 ||
                start.checked_add(8).is_none_or(|end|end>image.bytes.len()) ||
                image.bytes[start..start+8].iter().any(|&b|b!=0) ||
                output.global_literal_relocations.iter().any(|p|
                    p.symbol==owner && p.offset==offset) ||
                output.global_aggregate_relocations.iter().any(|p|
                    p.symbol==owner && p.offset==offset) ||
                output.global_routine_relocations.iter().any(|p|
                    p.symbol==owner && p.offset==offset) {
                return;
            }
            output.global_literal_relocations.push(GlobalLiteralRelocation {
                symbol:SymbolId(symbol),offset,literal_id,addend,
            });
        }
        52 => {
            let (Some(symbol),Some(offset),Some(target),Some(addend)) =
                (n(0),fields[1].unsigned(),n(2),fields[3].signed())
                else {return;};
            if symbol==0 || target==0 ||
                output.global_literal_relocations.iter().any(|p|
                    p.symbol==SymbolId(symbol) && p.offset==offset) ||
                output.global_routine_relocations.iter().any(|p|
                    p.symbol==SymbolId(symbol) && p.offset==offset) {return;}
            output.global_aggregate_relocations.push(GlobalAggregateRelocation {
                symbol:SymbolId(symbol),offset,target:SymbolId(target),addend
            });
        }
        63 => {
            let (Some(owner),Some(plain)) = (n(0),n(1)) else {return;};
            if owner==0 || plain>2 ||
                !seen.cxx_records.insert(TypeId(owner)) {
                return;
            }
            output.cxx_record_admissions.push(CxxRecordAdmission {
                owner:TypeId(owner),plain_object:plain==1,
            });
            if plain==2 {
                output.cxx_tagless_storage_proofs.push(TypeId(owner));
            }
        }
        65 => {
            let (Some(owner),Some(proven)) = (n(0),n(1)) else {return;};
            if owner==0 || proven>1 ||
                !seen.cxx_lifetimes.insert(TypeId(owner)) {return;}
            output.cxx_scalar_lifetime_admissions.push(CxxScalarLifetimeAdmission {
                owner:TypeId(owner),candidate:proven==1,
            });
        }
        64 => {
            let Some(cpp) = n(0) else {return;};
            if cpp>1 || output.cpp_language.is_some() {return;}
            output.cpp_language=Some(cpp==1);
        }
        62 => {
            let (Some(function), Some(statement_path), Some(path),
                 Some(ty), Some(hex)) =
                (t(0), t(1), t(2), n(3), t(4)) else {return;};
            if function.is_empty() || statement_path.is_empty() ||
                path.is_empty() || ty==0 {return;}
            let Some(bytes)=decode_aggregate_hex(&hex) else {return;};
            if bytes.len()>MAX_NATIVE_AGGREGATE_IMAGE_BYTES || bytes.last()!=Some(&0) {return;}
            output.string_literals.push(ExpressionStringLiteral {
                function, statement_path, path, ty:TypeId(ty), bytes,
            });
        }
        50 => {
            let (Some(function),Some(statement_path),Some(symbol),Some(ty),Some(hex)) =
                (t(0),t(1),n(2),n(3),t(4)) else {return;};
            if function.is_empty() || statement_path.is_empty() ||
               symbol==0 || ty==0 {return;}
            let Some(bytes)=decode_aggregate_hex(&hex) else {return;};
            output.statement_aggregate_initializers.push(StatementAggregateInitializer {
                function,statement_path,symbol:SymbolId(symbol),ty:TypeId(ty),bytes
            });
        }
        51 => {
            let (Some(symbol),Some(ty),Some(hex))=(n(0),n(1),t(2))
                else {return;};
            if symbol==0 || ty==0 {return;}
            let Some(bytes)=decode_aggregate_hex(&hex) else {return;};
            output.global_aggregate_initializers.push(GlobalAggregateInitializer {
                symbol:SymbolId(symbol),ty:TypeId(ty),bytes
            });
        }
        49 => {
            let (Some(owner), Some(bytes), Some(alignment)) =
                (n(0), fields[1].unsigned(), fields[2].unsigned()) else { return; };
            if owner == 0 || bytes == 0 || alignment == 0 ||
                !seen.layouts.insert(TypeId(owner)) {
                return;
            }
            output.type_object_sizes.push(TypeObjectSize { owner: TypeId(owner), bytes, alignment });
        }
        48 => {
            let (Some(function),Some(statement_path),Some(path),
                 Some(symbol),Some(byte_offset)) =
                (t(0),t(1),t(2),n(3),fields[4].signed()) else {return;};
            if symbol==0 || function.is_empty() || statement_path.is_empty() || path.is_empty() {
                return;
            }
            output.constant_addresses.push(ConstantAddress {
                function,statement_path,path,symbol:SymbolId(symbol),byte_offset,
            });
        }
        100 => {
            let (Some(symbol),Some(flags))=(n(0),n(1)) else {return;};
            let section=t(2);
            let visibility=n(3);
            if symbol==0 || flags>2047 ||
                (section.is_some() != (flags&1024!=0)) ||
                section.as_ref().is_some_and(|s|s.is_empty() || s.contains('\0')) ||
                visibility.is_some_and(|v|v>4) ||
                !seen.globals.contains(&SymbolId(symbol)) ||
                output.global_materializations.iter().any(|m|m.symbol==SymbolId(symbol)) {
                return;
            }
            output.global_materializations.push(GlobalMaterialization {
                symbol:SymbolId(symbol),
                is_nonreal: flags&1!=0,
                is_prototype_instantiation: flags&2!=0,
                is_template_variable: flags&4!=0,
                compiler_generated: flags&8!=0,
                used: flags&16!=0,
                address_taken: flags&32!=0,
                is_class_member: flags&64!=0,
                is_weak: flags&128!=0,
                is_weakref: flags&256!=0,
                is_gnu_alias: flags&512!=0,
                has_explicit_section: flags&1024!=0,
                section,
                elf_visibility: visibility,
            });
        }
        47 => {
            let (Some(symbol), Some(linkage), Some(init), Some(tls), Some(local)) =
                (n(0), n(1), n(2), n(3), n(4)) else { return; };
            if symbol == 0 || !(1..=3).contains(&linkage) || init > 1 || tls > 1 || local > 1 ||
                !seen.globals.insert(SymbolId(symbol)) {
                // Even identical duplicate metadata would make this source
                // identity ambiguous; collect() treats a rejected event as
                // malformed instead of silently trusting whichever came first.
                return;
            }
            output.global_variables.push(GlobalVariable {
                symbol: SymbolId(symbol), linkage,
                explicit_initializer: init == 1,
                thread_local: tls == 1, local_to_function: local == 1,
            });
        }
        46 => {
            let (Some(id), Some(bits), Some(signed), Some(boolean)) =
                (n(0), n(1), n(2), n(3)) else { return; };
            if id == 0 || !matches!(bits, 8 | 16 | 32 | 64) ||
               signed > 1 || boolean > 1 ||
               (boolean == 1 && (bits != 8 || signed != 0)) ||
               !seen.c_integer_types.insert(TypeId(id)) {
                return;
            }
            output.c_integer_types.push(CIntegerType {
                id: TypeId(id), bits: bits as u8, signed: signed == 1,
                is_bool: boolean == 1,
            });
        }
        57 => {
            let (Some(id),Some(bits))=(n(0),n(1)) else {return;};
            if id==0 || !matches!(bits,32|64) ||
                !seen.c_float_types.insert(TypeId(id)) {return;}
            output.c_float_types.push(CFloatType{id:TypeId(id),bits:bits as u8});
        }
        58 => {
            let (Some(function),Some(statement_path),Some(path),
                 Some(ty),Some(width),Some(bits)) =
                (t(0),t(1),t(2),n(3),n(4),fields[5].unsigned())
                else {return;};
            if function.is_empty() || statement_path.is_empty() ||
                path.is_empty() || ty==0 || !matches!(width,32|64) ||
                (width==32 && bits>u64::from(u32::MAX)) ||
                !seen.float_constant_sites.insert((
                    function.clone(),statement_path.clone(),path.clone())) {
                return;
            }
            output.float_constants.push(FloatConstant {
                function,statement_path,path,ty:TypeId(ty),
                width:width as u8,bits,
            });
        }
        60 => {
            let (Some(symbol),Some(ty),Some(width),Some(bits)) =
                (n(0),n(1),n(2),fields[3].unsigned())
                else {return;};
            if symbol==0 || ty==0 || !matches!(width,32|64) ||
                (width==32 && bits>u64::from(u32::MAX)) ||
                !seen.global_float_images.insert(SymbolId(symbol)) {
                return;
            }
            output.global_float_initializers.push(GlobalFloatInitializer {
                symbol:SymbolId(symbol),ty:TypeId(ty),width:width as u8,bits,
            });
        }
        59 => {
            let (Some(function),Some(statement_path),Some(symbol),
                 Some(ty),Some(width),Some(bits)) =
                (t(0),t(1),n(2),n(3),n(4),fields[5].unsigned())
                else {return;};
            if function.is_empty() || statement_path.is_empty() ||
                symbol==0 || ty==0 || !matches!(width,32|64) ||
                (width==32 && bits>u64::from(u32::MAX)) ||
                !seen.float_initializer_sites.insert((
                    function.clone(),statement_path.clone(),SymbolId(symbol))) {
                return;
            }
            output.float_initializers.push(FloatInitializer {
                function,statement_path,symbol:SymbolId(symbol),ty:TypeId(ty),
                width:width as u8,bits,
            });
        }
        45 => {
            let (Some(function), Some(statement_path), Some(value)) =
                (t(0), t(1), fields[2].signed()) else { return; };
            output.statement_integer_initializers.push(StatementIntegerInitializer {
                function, statement_path, value,
            });
        }
        54 => {
            let (Some(function),Some(statement_path),Some(symbol)) =
                (t(0),t(1),n(2)) else {return;};
            if function.is_empty() || statement_path.is_empty() || symbol==0 {return;}
            output.statement_routine_initializers.push(StatementRoutineInitializer {
                function,statement_path,symbol:SymbolId(symbol),
            });
        }
        55 => {
            let (Some(function),Some(statement_path),Some(path),Some(symbol)) =
                (t(0),t(1),t(2),n(3)) else {return;};
            if function.is_empty() || statement_path.is_empty() ||
                path.is_empty() || symbol==0 {return;}
            output.constant_routine_addresses.push(ConstantRoutineAddress {
                function,statement_path,path,symbol:SymbolId(symbol),
            });
        }
        8 => {
            let (Some(owner), Some(position), Some(ty)) = (n(0), n(1), n(2)) else { return; };
            output.type_parameters.push(TypeParameter { owner: TypeId(owner), position, ty: TypeId(ty) });
        }
        9 => {
            let (Some(id), Some(elements)) = (n(0), fields[1].unsigned()) else { return; };
            output.array_bounds.push(ArrayBound { array: TypeId(id), elements });
        }
        76 => {
            let (Some(owner),Some(flags))=(n(0),n(1)) else {return;};
            if owner==0 || flags & !7 != 0 ||
               (flags & 4 != 0 && flags & 1 == 0) ||
               (flags & 3 == 3) ||
               !seen.native_scalar_proofs.insert(TypeId(owner)) {return;}
            output.native_scalar_proofs.push(NativeScalarMetadataProof {
                owner:TypeId(owner),integer_abi:flags&1!=0,
                standard_float_abi:flags&2!=0,signed_i64:flags&4!=0,
            });
        }
        75 => {
            let (Some(owner),Some(has_layout),Some(has_fixed_array_bound)) =
                (n(0),n(1),n(2)) else {return;};
            if owner==0 || has_layout>1 || has_fixed_array_bound>1 ||
               !seen.native_type_layout_proofs.insert(TypeId(owner)) {return;}
            output.native_type_layout_proofs.push(NativeTypeLayoutProof {
                owner:TypeId(owner), has_layout:has_layout==1,
                has_fixed_array_bound:has_fixed_array_bound==1,
            });
        }
        74 => {
            let (Some(owner), Some(field_count), Some(direct_base_count)) =
                (n(0),n(1),n(2)) else { return; };
            if owner==0 || field_count>10000 || direct_base_count>10000 ||
                !seen.native_record_shapes.insert(TypeId(owner)) {return;}
            output.native_record_shapes.push(NativeRecordShape {
                owner:TypeId(owner),field_count,direct_base_count,
            });
        }
        10 => {
            let (Some(owner), Some(position), Some(ty), Some(name),
                 Some(byte_offset), Some(bit_offset), Some(bit_width), Some(symbol)) =
                (n(0), n(1), n(2), t(3), fields[4].unsigned(), n(5), n(6), n(7)) else { return; };
            if symbol == 0 { return; }
            output.record_fields.push(RecordField {
                owner: TypeId(owner), position, ty: TypeId(ty), name,
                symbol: SymbolId(symbol), byte_offset, bit_offset, bit_width });
        }
        11 => {
            let (Some(owner), Some(flags)) = (n(0), n(1)) else { return; };
            // Native EDG only emits reference/rvalue-reference bits 0 and 1.
            // Unknown bits must not be silently ignored.
            if owner == 0 || flags & !3 != 0 { return; }
            output.type_details.push(TypeDetail {
                owner: TypeId(owner), is_reference: flags & 1 != 0,
                is_rvalue_reference: flags & 2 != 0, qualifiers: n(2) });
        }
        71 => {
            let (Some(owner), Some(present), Some(parameter_count)) =
                (n(0),n(1),n(2)) else {return;};
            if owner==0 || present>1 || parameter_count>10000 ||
                (present==0 && parameter_count!=0) ||
                !seen.routine_supplements.insert(TypeId(owner)) {return;}
            output.routine_supplements.push(NativeRoutineSupplement {
                owner:TypeId(owner),has_extra_info:present==1,parameter_count,
            });
        }
        12 => {
            let (Some(owner), Some(prototyped), Some(variadic)) = (n(0), n(1), n(2)) else { return; };
            // Neither field is a truthy integer: the wire schema specifies
            // native boolean flags 0/1, and a malformed value changes ABI
            // prototype/variadic interpretation.
            if owner == 0 || prototyped > 1 || variadic > 1 { return; }
            output.function_signatures.push(FunctionSignature {
                owner: TypeId(owner), prototyped: prototyped != 0, variadic: variadic != 0 });
        }
        42 => {
            let (Some(routine), Some(function)) = (n(0), t(1)) else { return; };
            if routine == 0 || function.is_empty() { return; }
            output.routine_identities.push(RoutineIdentity {
                routine: SymbolId(routine), function,
            });
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
        56 => {
            let (Some(function),Some(statement_path),Some(ty),
                 Some(has_expr),Some(copied)) =
                (t(0),t(1),n(2),n(3),n(4)) else {return;};
            if function.is_empty() || statement_path.is_empty() || ty==0 ||
                has_expr>1 || copied>1 || (copied==1 && has_expr==1) {return;}
            output.vla_dimensions.push(VlaDimension {
                function,statement_path,array_type:TypeId(ty),
                has_expression:has_expr==1,copied:copied==1,
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
            if symbol == 0 || ty == 0 ||
                !seen.symbol_types.insert(SymbolId(symbol)) {
                return;
            }
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
        80 => {
            let (Some(function),Some(statement_path),Some(path),
                 Some(original_routine),Some(original_operator),
                 Some(receiver_retained)) =
                (t(0),t(1),t(2),n(3),n(5),n(6)) else {return;};
            if function.is_empty() || statement_path.is_empty() ||
               path.is_empty() || original_routine==0 ||
               original_operator==0 || receiver_retained>1 ||
               n(4)==Some(0) {return;}
            output.prelower_virtual_origins.push(PrelowerVirtualCallOrigin {
                function,statement_path,path,
                original_routine:SymbolId(original_routine),
                original_receiver_type:n(4).map(TypeId),
                original_operator,
                receiver_node_retained:receiver_retained==1,
            });
        }
        81 => {
            let (Some(function),Some(statement_path),Some(path),
                 Some(ty),Some(position),Some(same_node)) =
                (t(0),t(1),t(2),n(3),n(4),n(5)) else {return;};
            if function.is_empty() || statement_path.is_empty() ||
               path.is_empty() || ty==0 || position>=16 || same_node>1 {
                return;
            }
            output.virtual_receiver_reuses.push(VirtualReceiverReuse {
                function,statement_path,path,
                receiver_type:TypeId(ty),
                argument_position:position,
                original_node_reused:same_node==1,
            });
        }
        82 => {
            let (Some(function),Some(statement_path),Some(path),
                 Some(source_index),Some(entry_bytes)) =
                (t(0),t(1),t(2),n(3),n(4)) else {return;};
            if function.is_empty() || statement_path.is_empty() ||
               path.is_empty() || entry_bytes==0 || entry_bytes>32 ||
               !entry_bytes.is_power_of_two() ||
               u64::from(source_index)
                   .checked_mul(u64::from(entry_bytes)).is_none() {
                return;
            }
            output.virtual_entry_indexes.push(VirtualEntryIndexSelection {
                function,statement_path,path,
                source_index:u64::from(source_index),
                entry_bytes:u64::from(entry_bytes),
            });
        }
        90 => {
            let (Some(function),Some(statement_path),Some(mask))=
                (t(0),t(1),n(2)) else {return;};
            if function.is_empty() || statement_path.is_empty() || mask & !15!=0 {return;}
            output.native_control_root_admissions.push(NativeControlRootAdmission {
                function,statement_path,mask,
            });
        }
        93 => {
            let (Some(routine),Some(lifetime),Some(count))=(n(0),n(1),n(2))
                else {return;};
            if routine==0 || count>10000 {return;}
            output.native_lifetime_destruction_counts.push(NativeLifetimeDestructionCount {
                routine:SymbolId(routine),lifetime,count,
            });
        }
        94 => {
            let (Some(routine),Some(lifetime),Some(count),Some(edge))=
                (n(0),n(1),n(2),n(3)) else {return;};
            if routine==0 || count>10000 || edge>1 {return;}
            output.native_lifetime_child_counts.push(NativeLifetimeChildCount {
                routine:SymbolId(routine),lifetime,count,native_child_edge:edge==1,
            });
        }
        95 => {
            let Some(routine)=n(0) else {return;};
            if routine==0 {return;}
            output.native_lifetime_root_admissions.push(NativeLifetimeRootAdmission {
                routine:SymbolId(routine),body:n(1),local_static:n(2),
            });
        }
        96 => {
            let (Some(routine),Some(scope))=(n(0),n(1)) else {return;};
            if routine==0 {return;}
            output.native_scope_lifetime_admissions.push(NativeScopeLifetimeAdmission {
                routine:SymbolId(routine),scope,lifetime:n(2),
            });
        }
        97 => {
            let (Some(routine),Some(scope),Some(count))=
                (n(0),n(1),n(2)) else {return;};
            if routine==0 || count>10000 {return;}
            output.native_scope_child_counts.push(NativeScopeChildCount {
                routine:SymbolId(routine),scope,count,
            });
        }
        98 => {
            let (Some(routine),Some(scope),Some(count))=
                (n(0),n(1),n(2)) else {return;};
            if routine==0 || count>10000 {return;}
            output.native_scope_local_counts.push(NativeScopeLocalCount {
                routine:SymbolId(routine),scope,count,
            });
        }
        99 => {
            let (Some(routine),Some(explicit),Some(implicit_this))=
                (n(0),n(1),n(2)) else {return;};
            if routine==0 || explicit>10000 || implicit_this>1 {return;}
            output.native_function_parameter_counts.push(NativeFunctionParameterCount {
                routine:SymbolId(routine),explicit,implicit_this:implicit_this==1,
            });
        }
        92 => {
            let (Some(routine),Some(count))=(n(0),n(1)) else {return;};
            if routine==0 || count>10000 {return;}
            output.native_constructor_initializer_counts.push(NativeConstructorInitializerCount {
                routine:SymbolId(routine),count,
            });
        }
        91 => {
            let (Some(routine),Some(position),Some(present))=
                (n(0),n(1),n(2)) else {return;};
            if routine==0 || present>1 {return;}
            output.native_constructor_root_admissions.push(NativeConstructorRootAdmission {
                routine:SymbolId(routine),position,present:present==1,
            });
        }
        87 => {
            let (Some(function),Some(statement_path),Some(role),Some(path),Some(count))=
                (t(0),t(1),n(2),t(3),n(4)) else {return;};
            if function.is_empty() || statement_path.is_empty() || path.is_empty() ||
                count>10000 || role==0 {return;}
            output.native_control_child_counts.push(NativeControlChildCount {
                function,statement_path,role,path,count,
            });
        }
        88 => {
            let (Some(owner),Some(path),Some(count))=
                (n(0),t(1),n(2)) else {return;};
            if owner==0 || path.is_empty() || count>10000 {return;}
            output.native_initializer_child_counts.push(NativeInitializerChildCount {
                owner:SymbolId(owner),path,count,
            });
        }
        89 => {
            let (Some(routine),Some(position),Some(path),Some(count))=
                (n(0),n(1),t(2),n(3)) else {return;};
            if routine==0 || path.is_empty() || count>10000 {return;}
            output.native_constructor_child_counts.push(NativeConstructorChildCount {
                routine:SymbolId(routine),position,path,count,
            });
        }
        86 => {
            let (Some(function),Some(statement_path),Some(count))=
                (t(0),t(1),n(2)) else {return;};
            if function.is_empty() || statement_path.is_empty() ||
                count>10000 {return;}
            output.native_try_handler_counts.push(NativeTryHandlerCount {
                function,statement_path,count,
            });
        }
        85 => {
            let (Some(function),Some(statement_path),Some(path),
                 Some(lookup_ty),Some(position)) =
                (t(0),t(1),t(2),n(3),n(4)) else {return;};
            if function.is_empty() || statement_path.is_empty() ||
               path.is_empty() || lookup_ty==0 || position>=16 {
                return;
            }
            output.virtual_vptr_receiver_pairs.push(VirtualVptrReceiverPair {
                function,statement_path,path,
                lookup_object_type:TypeId(lookup_ty),
                argument_position:position,
            });
        }
        79 => {
            let (Some(routine),Some(owner))=(n(0),n(1)) else {return;};
            if routine==0 || owner==0 {return;}
            if n(4).is_some_and(|base|base==0) {return;}
            output.native_virtual_members.push(NativeVirtualMemberInfo {
                routine: SymbolId(routine),owner:TypeId(owner),
                virtual_number:fields[2].unsigned(),
                virtual_info_offset:fields[3].unsigned(),
                sharing_base:n(4).map(TypeId),
            });
        }
        22 => {
            let (Some(routine), Some(special_kind), Some(flags)) =
                (n(0), n(1), n(2)) else { return; };
            if flags & !2047 != 0 { return; }
            output.routine_attributes.push(RoutineAttributes {
                routine: SymbolId(routine), special_kind,
                is_virtual: flags & 1 != 0, pure_virtual: flags & 2 != 0,
                is_override: flags & 4 != 0, is_final: flags & 8 != 0,
                overrides_base_member: flags & 16 != 0,
                is_inline: flags & 32 != 0, is_constexpr: flags & 64 != 0,
                defined: flags & 128 != 0,
                explicit_constructor: flags & 256 != 0,
                has_this_parameter: flags & 512 != 0,
                compiler_generated: flags & 1024 != 0,
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
        70 => {
            let (Some(function),Some(statement_path),Some(path),Some(source)) =
                (t(0),t(1),t(2),n(3)) else {return;};
            if function.is_empty() || statement_path.is_empty() ||
                path.is_empty() || source==0 {return;}
            output.implicit_pointer_casts.push(ImplicitPointerCast {
                function,statement_path,path,source_type:TypeId(source),
            });
        }
        18 => {
            let (Some(function), Some(statement_path), Some(path), Some(first_argument)) =
                (t(0), t(1), t(2), n(3)) else { return; };
            if first_argument != 1 && first_argument != 2 { return; }
            output.calls.push(CallSite { function, statement_path, path, first_argument });
        }
        78 => {
            let (Some(function),Some(statement_path),Some(path),
                 Some(native_operator),Some(class),Some(virtual_flag)) =
                (t(0),t(1),t(2),n(3),n(4),n(5)) else {return;};
            if function.is_empty() || statement_path.is_empty() ||
                path.is_empty() || virtual_flag>1 {return;}
            let class=match class {
                0=>CallDispatchClass::OrdinaryOrLowered,
                1=>CallDispatchClass::Member,
                2=>CallDispatchClass::PointerToMember,
                _=>return,
            };
            // The native EDG virtual-call bit is defined ONLY on direct
            // member-call operators; never infer it for an arbitrary
            // lowered or pointer-to-member call.
            if virtual_flag==1 && class!=CallDispatchClass::Member {return;}
            output.call_dispatches.push(CallDispatchProvenance {
                function,statement_path,path,native_operator,class,
                explicitly_virtual:virtual_flag==1,
            });
        }
        77 => {
            let (Some(function),Some(statement_path),Some(path),Some(form)) =
                (t(0),t(1),t(2),n(3)) else {return;};
            if function.is_empty() || statement_path.is_empty() ||
                path.is_empty() {return;}
            let form=match form {
                1=>CallTargetForm::DirectRoutine,
                2=>CallTargetForm::ConstantRoutineAddress,
                3=>CallTargetForm::IndirectUnresolved,
                _=>return,
            };
            let callee_symbol=n(5).map(SymbolId);
            if matches!(form,CallTargetForm::IndirectUnresolved) {
                if callee_symbol.is_some() {return;}
            } else if callee_symbol.is_none_or(|s|s.0==0) {return;}
            let callee_type=n(4).map(TypeId);
            if callee_type.is_some_and(|ty|ty.0==0) {return;}
            output.call_targets.push(CallTargetProvenance {
                function,statement_path,path,form,callee_type,callee_symbol,
            });
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
            if symbol == 0 || explicit > 1 || uninitialized > 1 { return; }
            output.variable_initializers.push(VariableInitializer {
                symbol: SymbolId(symbol), kind, explicit: explicit != 0,
                uninitialized: uninitialized != 0,
            });
        }
        13 => {
            let (Some(id), Some(kind), Some(name)) = (n(0), n(1), t(2)) else { return; };
            // The native bridge's pointer-to-ID table emits this mapping
            // once; duplicate IDs make type, global and call references
            // ambiguous. Never allow a zero or non-symbol kind either.
            if id==0 || !(1..=4).contains(&kind) ||
                !seen.symbols.insert(SymbolId(id)) {return;}
            if kind==1 {seen.routine_symbols.insert(SymbolId(id));}
            if matches!(kind,1|2) {seen.linkable_symbols.insert(SymbolId(id));}
            output.symbols.push(Symbol { id: SymbolId(id), kind, name });
        }
        61 => {
            let (Some(symbol),Some(link_name),Some(mangled)) =
                (n(0),t(1),n(2)) else {return;};
            if symbol==0 || link_name.is_empty() || link_name.len()>4096 ||
                link_name.contains('\0') || mangled>1 ||
                !seen.linkable_symbols.contains(&SymbolId(symbol)) ||
                !seen.source_link_names.insert(SymbolId(symbol)) {
                return;
            }
            output.symbol_link_names.push(SymbolLinkName {
                symbol:SymbolId(symbol),link_name,edg_mangled:mangled==1,
            });
        }
        _ => {}
    }
}

/// Event 68 describes EDG's source-owned *scope-local* dynamic initializer
/// and optional destructor association. This is not registration with
/// __cxa_atexit or a portable process-shutdown execution schedule.
/// Independently check the successfully decoded snapshot too: the public Il
/// can be constructed or altered after callback wire validation.
fn validate_native_scope_dynamic_initializer_coverage(il:&Il)->Result<(),&'static str> {
    let variable_symbols:HashSet<_>=il.symbols.iter()
        .filter(|symbol|symbol.kind==2).map(|symbol|symbol.id).collect();
    let routine_symbols:HashSet<_>=il.symbols.iter()
        .filter(|symbol|symbol.kind==1).map(|symbol|symbol.id).collect();
    let mut next_position:HashMap<&str,u32>=HashMap::new();
    for init in &il.scope_dynamic_initializers {
        if init.scope.is_empty() || init.scope.len()>1024 {
            return Err("native dynamic initializer lacks valid scope identity");
        }
        let expected=next_position.entry(init.scope.as_str()).or_insert(0);
        if init.position!=*expected {
            return Err("native dynamic initializer has missing or duplicate scope ordinal");
        }
        *expected=expected.checked_add(1)
            .ok_or("native dynamic initializer scope ordinal overflow")?;
        if init.variable.is_some_and(|id|!variable_symbols.contains(&id)) {
            return Err("native dynamic initializer has no source variable identity");
        }
        if init.destructor.is_some_and(|id|!routine_symbols.contains(&id)) {
            return Err("native dynamic initializer has no source destructor identity");
        }
    }
    Ok(())
}

#[cfg(test)]
mod native_scope_dynamic_initializer_coverage_tests {
    use super::*;

    #[test]
    fn scoped_destructor_owners_require_exact_native_symbol_kinds() {
        let mut il=Il::default();
        il.symbols.extend([
            Symbol{id:SymbolId(10),kind:2,name:"global".into()},
            Symbol{id:SymbolId(20),kind:1,name:"destructor".into()},
        ]);
        il.scope_dynamic_initializers.push(ScopeDynamicInitializer {
            scope:"1:".into(),position:0,variable:Some(SymbolId(10)),
            destructor:Some(SymbolId(20)),kind:1,
        });
        assert!(validate_native_scope_dynamic_initializer_coverage(&il).is_ok());
        let mut wrong=il.clone();
        wrong.scope_dynamic_initializers[0].variable=Some(SymbolId(20));
        assert!(validate_native_scope_dynamic_initializer_coverage(&wrong).is_err());
        let mut wrong=il.clone();
        wrong.scope_dynamic_initializers[0].destructor=Some(SymbolId(10));
        assert!(validate_native_scope_dynamic_initializer_coverage(&wrong).is_err());
        let mut wrong=il.clone();
        wrong.scope_dynamic_initializers[0].destructor=Some(SymbolId(999));
        assert!(validate_native_scope_dynamic_initializer_coverage(&wrong).is_err());
        // An absent EDG destructor association remains unknown, not proof
        // of no shutdown effects.
        il.scope_dynamic_initializers[0].destructor=None;
        assert!(validate_native_scope_dynamic_initializer_coverage(&il).is_ok());
    }

    #[test]
    fn large_interleaved_scope_initializers_require_independent_gapless_order() {
        let mut il=Il::default();
        il.symbols.extend([
            Symbol{id:SymbolId(10),kind:2,name:"global".into()},
            Symbol{id:SymbolId(20),kind:1,name:"destructor".into()},
        ]);
        for position in 0..2048 {
            for scope in ["1:","2:ns::"] {
                il.scope_dynamic_initializers.push(ScopeDynamicInitializer {
                    scope:scope.into(),position,variable:Some(SymbolId(10)),
                    destructor:Some(SymbolId(20)),kind:1,
                });
            }
        }
        assert_eq!(il.scope_dynamic_initializers.len(),4096);
        assert!(validate_native_scope_dynamic_initializer_coverage(&il).is_ok());
        let mut wrong=il.clone();
        wrong.scope_dynamic_initializers[2048].position=2047;
        assert!(validate_native_scope_dynamic_initializer_coverage(&wrong).is_err());
        let mut wrong=il.clone();
        wrong.scope_dynamic_initializers[4095].scope.clear();
        assert!(validate_native_scope_dynamic_initializer_coverage(&wrong).is_err());
    }
}

/// The native bridge exports a separate callee classification for every
/// source-owned call. A dropped, duplicated or cross-site record must not
/// silently convert an unresolved pointer/vtable call into a direct call.
fn validate_native_call_target_coverage(il:&Il)->Result<(),&'static str> {
    if il.call_targets.len()!=il.calls.len() {
        return Err("native calls lack complete callee provenance");
    }
    // All these native IL records belong to one immutable compilation.
    // Index once: a distinct native call must not rescan the full call,
    // expression, type, or symbol roster for every source call target.
    let call_sites: HashSet<_> = il.calls.iter().map(|site| (
        site.function.as_str(),site.statement_path.as_str(),site.path.as_str(),
    )).collect();
    let routine_symbols: HashSet<_> = il.symbols.iter()
        .filter(|sym| sym.kind==1).map(|sym|sym.id).collect();
    let known_types: HashSet<_> = il.types.iter().map(|ty|ty.id).collect();
    // Preserve the old first-matching expression behavior. The later graph
    // completeness check independently rejects duplicate expression keys.
    let mut expressions=HashMap::with_capacity(il.expressions.len());
    for expression in &il.expressions {
        expressions.entry((expression.function.as_str(),
            expression.statement_path.as_str(),expression.path.as_str()))
            .or_insert(expression);
    }
    // Event 55 is emitted independently from the original ck_address /
    // abk_routine callee expression, not inferred from call event 77.
    // A purported constant routine target must agree with that source proof.
    let mut addresses=std::collections::HashMap::with_capacity(
        il.constant_routine_addresses.len());
    for address in &il.constant_routine_addresses {
        let key=(address.function.as_str(),address.statement_path.as_str(),
                 address.path.as_str());
        if addresses.insert(key,address.symbol).is_some() {
            return Err("duplicate native constant routine address");
        }
    }
    let mut unique=HashSet::new();
    for proof in &il.call_targets {
        let key=(proof.function.as_str(),proof.statement_path.as_str(),
                 proof.path.as_str());
        if !unique.insert(key) || !call_sites.contains(&key) {
            return Err("native callee provenance is ambiguous or orphaned");
        }
        if let Some(symbol)=proof.callee_symbol {
            if !routine_symbols.contains(&symbol) {
                return Err("native callee's direct routine identity is missing");
            }
        }
        if let Some(ty)=proof.callee_type {
            if !known_types.contains(&ty) {
                return Err("native callee's source type is missing");
            }
        }
        let callee_path=format!("{}.0",proof.path);
        let expression=expressions.get(&(
            proof.function.as_str(),proof.statement_path.as_str(),
            callee_path.as_str(),
        )).ok_or("native callee lacks original expression node")?;
        if expression.type_id!=proof.callee_type {
            return Err("native callee type disagrees with its EDG expression");
        }
        let source_address=addresses.get(&(
            proof.function.as_str(),proof.statement_path.as_str(),
            callee_path.as_str())).copied();
        match proof.form {
            CallTargetForm::DirectRoutine => {
                if proof.callee_symbol.is_none() ||
                   expression.symbol_id!=proof.callee_symbol ||
                   source_address.is_some() {
                    return Err("native direct callee disagrees with EDG expression");
                }
            }
            CallTargetForm::ConstantRoutineAddress => {
                // None==None is not a positive EDG source-address witness.
                if proof.callee_symbol.is_none() ||
                   source_address!=proof.callee_symbol {
                    return Err("native constant callee lacks matching EDG address proof");
                }
            }
            CallTargetForm::IndirectUnresolved => {
                if proof.callee_symbol.is_some() || source_address.is_some() {
                    return Err("native indirect callee contradicts EDG address proof");
                }
            }
        }
    }
    Ok(())
}

/// A source-identified virtual routine can survive EDG lowering only via the
/// optional paired native pre-lowering and *post-rewrite* observers.
/// Require unique observed sites, a native virtual member declaration, and
/// a source-consistent indirect eok_call. Bridge event 80 can now be
/// emitted only if EDG explicitly confirmed the rewritten call node and its
/// callee pointer survived to the snapshot. Absent witnesses are still legal:
/// subsequent EDG copying/inlining can destroy physical node identity.
/// No consumer may treat this as complete virtual ABI or call permission.
fn validate_prelower_virtual_call_origins(il:&Il)->Result<(),&'static str> {
    if il.cpp_language!=Some(true) && !il.prelower_virtual_origins.is_empty() {
        return Err("prelower virtual call source proof in non-C++ IL");
    }
    let mut unique=HashSet::new();
    for proof in &il.prelower_virtual_origins {
        let key=(proof.function.as_str(),proof.statement_path.as_str(),
                 proof.path.as_str());
        if !unique.insert(key) {
            return Err("duplicate prelower virtual call origin");
        }
        let source=il.call_targets.iter().find(|v|
            v.function==proof.function &&
            v.statement_path==proof.statement_path && v.path==proof.path);
        if !matches!(source,Some(CallTargetProvenance {
                form:CallTargetForm::IndirectUnresolved,..})) {
            return Err("prelower virtual origin not an unresolved indirect call");
        }
        if !il.calls.iter().any(|v|
            v.function==proof.function &&
            v.statement_path==proof.statement_path && v.path==proof.path) ||
           !il.call_dispatches.iter().any(|v|
            v.function==proof.function &&
            v.statement_path==proof.statement_path && v.path==proof.path &&
            v.class==CallDispatchClass::OrdinaryOrLowered &&
            !v.explicitly_virtual) {
            return Err("prelower virtual origin lacks matching lowered call");
        }
        if !il.symbols.iter().any(|s|
            s.id==proof.original_routine && s.kind==1) {
            return Err("prelower virtual routine identity missing");
        }
        // Historical source-hook payloads must be validated independently;
        // A routine ID in a namespace, nonvirtual method, or other unrelated
        // symbol can never be substituted for that original routine.
        let origin=il.native_virtual_members.iter().find(|v|
            v.routine==proof.original_routine)
            .ok_or("prelower routine lacks native virtual-member proof")?;
        if !il.routine_attributes.iter().any(|v|
            v.routine==proof.original_routine &&
            v.is_virtual && v.has_this_parameter) ||
           !il.member_functions.iter().any(|v|
            v.routine==proof.original_routine && v.owner==origin.owner) {
            return Err("prelower routine does not match a virtual class member");
        }
        if let Some(ty)=proof.original_receiver_type {
            if !il.types.iter().any(|t|t.id==ty) {
                return Err("prelower receiver native type missing");
            }
        }
    }
    Ok(())
}

/// Event 81 only certifies that EDG's source-generated reusable receiver
/// expression survived into exactly one *direct* argument of the same
/// final indirect call previously certified by paired event 80.
/// This is an expression-node/evaluation provenance proof, not the
/// original receiver's machine address or virtual dispatch authorization.
fn validate_virtual_receiver_reuses(il:&Il)->Result<(),&'static str> {
    if il.cpp_language!=Some(true) && !il.virtual_receiver_reuses.is_empty() {
        return Err("virtual receiver reuse in non-C++ IL");
    }
    let mut seen=HashSet::new();
    for receiver in &il.virtual_receiver_reuses {
        let key=(receiver.function.as_str(),receiver.statement_path.as_str(),
                 receiver.path.as_str());
        if !seen.insert(key) {
            return Err("duplicate virtual receiver reuse");
        }
        if !il.prelower_virtual_origins.iter().any(|v|
            v.function==receiver.function &&
            v.statement_path==receiver.statement_path &&
            v.path==receiver.path) {
            return Err("receiver reuse lacks paired source virtual-call origin");
        }
        let Some(call)=il.calls.iter().find(|v|
            v.function==receiver.function &&
            v.statement_path==receiver.statement_path &&
            v.path==receiver.path) else {
            return Err("receiver reuse missing ordinary call");
        };
        if call.first_argument!=1 || receiver.argument_position>=16 {
            return Err("receiver reuse has invalid ordinary-call argument index");
        }
        if !il.types.iter().any(|t|t.id==receiver.receiver_type) {
            return Err("reused virtual receiver has missing EDG type");
        }
        let path=format!("{}.{}",receiver.path,receiver.argument_position+1);
        if !il.expressions.iter().any(|v|
            v.function==receiver.function &&
            v.statement_path==receiver.statement_path &&
            v.path==path &&
            v.type_id==Some(receiver.receiver_type)) {
            return Err("receiver reuse type/path not present in final call IL");
        }
    }
    Ok(())
}

/// Event 82 must identify the same original *virtual* source routine
/// already established by paired EDG pre-/post-lowering observer event 80,
/// and its exact EDG logical number from independent event 79. Its index
/// expression pointer appeared uniquely in the final callee subtree.
/// Byte displacement is only the EDG-native eok_padd scaling, not the
/// physical machine slot or vtable address point.
fn validate_virtual_entry_index_selections(il:&Il)->Result<(),&'static str> {
    if il.cpp_language!=Some(true) && !il.virtual_entry_indexes.is_empty() {
        return Err("virtual table index in non-C++ IL");
    }
    let mut seen=HashSet::new();
    for entry in &il.virtual_entry_indexes {
        let key=(entry.function.as_str(),entry.statement_path.as_str(),
                 entry.path.as_str());
        if !seen.insert(key) {
            return Err("duplicate native virtual table entry witness");
        }
        if entry.entry_bytes==0 || entry.entry_bytes>32 ||
           !entry.entry_bytes.is_power_of_two() ||
           entry.source_index.checked_mul(entry.entry_bytes).is_none() {
            return Err("invalid or overflowing EDG native table index");
        }
        let origin=il.prelower_virtual_origins.iter().find(|v|
            v.function==entry.function &&
            v.statement_path==entry.statement_path &&
            v.path==entry.path)
            .ok_or("virtual table index lacks paired source call proof")?;
        let native=il.native_virtual_members.iter().find(|v|
            v.routine==origin.original_routine)
            .ok_or("virtual table index lacks original EDG member declaration")?;
        if native.virtual_number!=Some(entry.source_index) ||
           !il.routine_attributes.iter().any(|v|
               v.routine==origin.original_routine &&
               v.is_virtual && v.has_this_parameter) {
            return Err("virtual table index contradicts EDG routine numbering");
        }
    }
    Ok(())
}

/// Event 85 must be a strict subset of the paired source-event 80,
/// exact virtual lookup event 82, and EDG reused receiver event 81.
/// It binds two expression *origins* in native IL, not physical
/// subobject offsets or executable virtual call ABI.
fn validate_virtual_vptr_receiver_pairs(il:&Il)->Result<(),&'static str> {
    if il.cpp_language!=Some(true) && !il.virtual_vptr_receiver_pairs.is_empty() {
        return Err("vptr receiver pair in non-C++ IL");
    }
    let mut seen=HashSet::new();
    for pair in &il.virtual_vptr_receiver_pairs {
        let key=(pair.function.as_str(),pair.statement_path.as_str(),
                 pair.path.as_str());
        if !seen.insert(key) {
            return Err("duplicate virtual vptr receiver pair");
        }
        if pair.argument_position>=16 ||
           !il.types.iter().any(|t|t.id==pair.lookup_object_type) {
            return Err("invalid EDG vptr lookup object type or argument");
        }
        if !il.prelower_virtual_origins.iter().any(|v|
            v.function==pair.function &&
            v.statement_path==pair.statement_path &&
            v.path==pair.path) ||
           !il.virtual_entry_indexes.iter().any(|v|
            v.function==pair.function &&
            v.statement_path==pair.statement_path &&
            v.path==pair.path) {
            return Err("vptr receiver pair lacks exact source virtual lookup");
        }
        if !il.virtual_receiver_reuses.iter().any(|v|
            v.function==pair.function &&
            v.statement_path==pair.statement_path &&
            v.path==pair.path &&
            v.argument_position==pair.argument_position) {
            return Err("vptr receiver pair contradicts reusable call argument");
        }
    }
    Ok(())
}

/// Native event 79 belongs to each exported virtual member. Validate
/// original EDG routine/class identity, not a guessed ABI dispatch slot.
/// This boundary does NOT grant codegen permission for indirect calls.
fn validate_native_virtual_member_coverage(il:&Il)->Result<(),&'static str> {
    let mut virtuals=HashSet::new();
    for attr in &il.routine_attributes {
        if attr.is_virtual
            && (!attr.has_this_parameter || !virtuals.insert(attr.routine)) {
            return Err("native virtual method lacks unique this ABI metadata");
        }
    }
    if il.cpp_language!=Some(true) && !il.native_virtual_members.is_empty() {
        return Err("virtual member metadata in non-C++ native compilation");
    }
    if virtuals.len()!=il.native_virtual_members.len() {
        return Err("native virtual members lack complete EDG metadata");
    }
    let mut seen=HashSet::new();
    for proof in &il.native_virtual_members {
        if !seen.insert(proof.routine) || !virtuals.contains(&proof.routine) {
            return Err("duplicate or nonvirtual native method proof");
        }
        if !il.symbols.iter().any(|s|s.id==proof.routine && s.kind==1) ||
           !il.types.iter().any(|t|t.id==proof.owner) {
            return Err("virtual member proof has missing source identity");
        }
        if il.member_functions.iter().filter(|m|
            m.routine==proof.routine && m.owner==proof.owner).count()!=1 {
            return Err("virtual member proof disagrees with source class owner");
        }
        if let Some(base)=proof.sharing_base {
            if !il.types.iter().any(|t|t.id==base) ||
               proof.virtual_info_offset.is_none() {
                return Err("invalid shared virtual-info base proof");
            }
        }
        if let Some(offset)=proof.virtual_info_offset {
            if let Some(layout)=il.type_object_sizes.iter().find(|v|
                v.owner==proof.owner) {
                if offset>=layout.bytes {
                    return Err("virtual-info offset exceeds EDG class object layout");
                }
            }
        }
    }
    Ok(())
}

/// A mandatory second witness for every EDG call. Match the exact source
/// call expression's opcode, not a textual routine name or guessed C++ ABI.
fn validate_native_call_dispatch_coverage(il:&Il)->Result<(),&'static str> {
    if il.call_dispatches.len()!=il.calls.len() {
        return Err("native calls lack complete source dispatch provenance");
    }
    let mut unique=HashSet::new();
    for proof in &il.call_dispatches {
        let key=(proof.function.as_str(),proof.statement_path.as_str(),
                 proof.path.as_str());
        if !unique.insert(key) {return Err("duplicate native call dispatch");}
        let site=il.calls.iter().find(|c|
            c.function==proof.function &&
            c.statement_path==proof.statement_path && c.path==proof.path)
            .ok_or("orphaned native call dispatch")?;
        let call=il.expressions.iter().find(|e|
            e.function==proof.function &&
            e.statement_path==proof.statement_path && e.path==proof.path)
            .ok_or("native dispatch lacks exact call expression")?;
        if call.operator!=Some(proof.native_operator) {
            return Err("EDG call dispatch opcode disagrees with expression");
        }
        let expected=match proof.class {
            CallDispatchClass::OrdinaryOrLowered=>1,
            CallDispatchClass::Member|CallDispatchClass::PointerToMember=>2,
        };
        if site.first_argument!=expected ||
            (proof.explicitly_virtual && proof.class!=CallDispatchClass::Member) {
            return Err("native dispatch disagrees with operand/virtual semantics");
        }
    }
    Ok(())
}

/// A successful native back_end() unconditionally exports event 64 with
/// EDG's actual C/C++ mode. A lost event is a truncated snapshot, not a
/// license to guess language semantics from source names or flags. This
/// boundary also covers required type details and relationship edges.
// Successful native compilations must retain exactly one attributes event
// for each statement, including compiler-synthesized for increments.
fn validate_native_statement_attribute_coverage(il:&Il) -> Result<(),Error> {
    let statements: HashSet<_> = il.statements.iter()
        .map(|s| (s.function.as_str(), s.path.as_str())).collect();
    let mut attributes = HashSet::with_capacity(il.statement_attributes.len());
    for attr in &il.statement_attributes {
        let key = (attr.function.as_str(), attr.statement_path.as_str());
        if !statements.contains(&key) || !attributes.insert(key) {
            return Err(Error::MalformedCallback);
        }
    }
    if attributes.len() != il.statements.len() {
        return Err(Error::MalformedCallback);
    }
    Ok(())
}

// Native count proofs must be independent of the events whose completeness
// they establish; never infer the expected length from the received tail.
fn validate_native_tree_count_coverage(il:&Il)->Result<(),Error> {
    use std::collections::HashMap;
    let mut expression_keys=HashSet::with_capacity(il.expressions.len());
    let mut child_counts:HashMap<(String,String,String),u32>=HashMap::new();
    for expr in &il.expressions {
        let key=(expr.function.clone(),expr.statement_path.clone(),expr.path.clone());
        if !expression_keys.insert(key) {return Err(Error::MalformedCallback);}
        if let Some(parent)=&expr.parent {
            let parent_key=(expr.function.clone(),expr.statement_path.clone(),parent.clone());
            *child_counts.entry(parent_key).or_default()+=1;
        }
    }
    let mut expr_proofs=HashSet::new();
    for proof in &il.native_expression_child_counts {
        let key=(proof.function.clone(),proof.statement_path.clone(),proof.path.clone());
        if proof.count>10000 || !expression_keys.contains(&key) ||
           !expr_proofs.insert(key.clone()) ||
           child_counts.get(&key).copied().unwrap_or(0)!=proof.count {
            return Err(Error::MalformedCallback);
        }
    }
    if expr_proofs.len()!=expression_keys.len() {return Err(Error::MalformedCallback);}

    type ListKey=(String,Option<String>,String);
    let mut groups:HashMap<ListKey,Vec<u32>>=HashMap::new();
    let mut statements=HashSet::new();
    let statement_kinds:HashMap<(&str,&str),u32>=il.statements.iter()
        .map(|stmt| ((stmt.function.as_str(),stmt.path.as_str()),
                      graph::native_statement_kind(stmt.kind))).collect();
    for stmt in &il.statements {
        statements.insert((stmt.function.as_str(),stmt.path.as_str()));
        let (edge,index)=if let Some(parent)=&stmt.parent {
            let Some(rest)=stmt.path.strip_prefix(&format!("{parent}.")) else {
                return Err(Error::MalformedCallback);
            };
            let Some((edge,index))=rest.rsplit_once('.') else {
                return Err(Error::MalformedCallback);
            };
            (edge,index)
        } else {("",stmt.path.as_str())};
        let Ok(index)=index.parse::<u32>() else {return Err(Error::MalformedCallback);};
        // The synthetic for-increment statement has a separately emitted
        // expression and does not arise from emit_statements' EDG list.
        if edge=="increment" && index==0 && stmt.parent.is_some() {
            // The bridge synthesizes this expression statement ONLY for
            // the increment of a native for-loop. Arbitrary user-provided
            // "increment.0" paths cannot bypass sibling-list proofs.
            let parent=stmt.parent.as_deref().expect("checked above");
            if statement_kinds.get(&(stmt.function.as_str(),parent))!=Some(&9)
                || graph::native_statement_kind(stmt.kind)!=6 {
                return Err(Error::MalformedCallback);
            }
            continue;
        }
        groups.entry((stmt.function.clone(),stmt.parent.clone(),edge.into()))
            .or_default().push(index);
    }
    let mut seen=HashSet::new();
    for proof in &il.native_statement_sibling_counts {
        let key=(proof.function.clone(),proof.parent.clone(),proof.edge.clone());
        if proof.count>10000 || !seen.insert(key.clone()) ||
            proof.parent.as_ref().is_some_and(|parent|
                !statements.contains(&(proof.function.as_str(),parent.as_str()))) {
            return Err(Error::MalformedCallback);
        }
        let mut positions=groups.remove(&key).unwrap_or_default();
        positions.sort_unstable();
        if positions.len()!=proof.count as usize ||
           positions.iter().enumerate().any(|(i,pos)| *pos!=i as u32) {
            return Err(Error::MalformedCallback);
        }
    }
    if !groups.is_empty() {return Err(Error::MalformedCallback);}
    // Record 86 certifies the complete EDG handler list. The catchN
    // sibling-list witnesses alone could lose a final handler undetected.
    let mut handlers=HashMap::new();
    for proof in &il.native_try_handler_counts {
        let key=(proof.function.as_str(),proof.statement_path.as_str());
        if proof.count>10000 ||
            statement_kinds.get(&key)!=Some(&19) ||
            handlers.insert(key,proof.count).is_some() {
            return Err(Error::MalformedCallback);
        }
    }
    for stmt in &il.statements {
        if graph::native_statement_kind(stmt.kind)!=19 {continue;}
        let Some(&count)=handlers.get(&(stmt.function.as_str(),stmt.path.as_str()))
            else {return Err(Error::MalformedCallback);};
        for i in 0..count {
            if !seen.contains(&(stmt.function.clone(),Some(stmt.path.clone()),
                                format!("catch{i}"))) {
                return Err(Error::MalformedCallback);
            }
        }
    }
    // Reject extra or malformed catch edges beyond the independently
    // recorded count, not just missing ones.
    for proof in &il.native_statement_sibling_counts {
        if !proof.edge.starts_with("catch") {continue;}
        let Some(parent)=proof.parent.as_deref() else {
            return Err(Error::MalformedCallback);
        };
        let key=(proof.function.as_str(),parent);
        let Some(&count)=handlers.get(&key) else {
            return Err(Error::MalformedCallback);
        };
        let Some(index)=proof.edge.strip_prefix("catch")
            .and_then(|n|n.parse::<u32>().ok()) else {
            return Err(Error::MalformedCallback);
        };
        if proof.edge!=format!("catch{index}") || index>=count {
            return Err(Error::MalformedCallback);
        }
    }
    // Event 1 independently marks EDG routines with native definitions.
    // Every defined routine emits event 42 identifying its function key,
    // then emit_statements emits a root count even for an empty body.
    // Requiring the root count closes the case where *all* root statements
    // and their event-84 proof are lost together.
    let mut defined_names=HashSet::new();
    for entity in &il.entities {
        if let Entity::Routine { name, has_definition:true }=entity {
            if !defined_names.insert(name.as_str()) {
                return Err(Error::MalformedCallback);
            }
        }
    }
    let mut defined_identities=HashSet::new();
    for identity in &il.routine_identities {
        if defined_names.contains(identity.function.as_str()) {
            if !defined_identities.insert(identity.function.as_str()) {
                return Err(Error::MalformedCallback);
            }
            if !seen.contains(&(identity.function.clone(),None,String::new())) {
                return Err(Error::MalformedCallback);
            }
        }
    }
    if defined_identities.len()!=defined_names.len() {
        return Err(Error::MalformedCallback);
    }
    // A zero-length list contains no statement records. Its absence is only
    // visible when the native owner's required outgoing edges are checked.
    // Use EDG's normalized enum, never guess numeric statement discriminants.
    for stmt in &il.statements {
        let kind=graph::native_statement_kind(stmt.kind);
        let required:&[&str]=match kind {
            2 => &["block"],
            3 | 17 | 20 | 21 => &["then","else"],
            7 | 8 | 9 | 13 => &["body"],
            18 => &["init","body"],
            19 => &["try"],
            _ => &[],
        };
        for edge in required {
            if !seen.contains(&(stmt.function.clone(),Some(stmt.path.clone()),
                                (*edge).to_owned())) {
                return Err(Error::MalformedCallback);
            }
        }
    }
    // A GNU statement expression owns an embedded statement subtree named
    // "gnu" plus the underscore-separated expression path. Even an empty
    // embedded statement requires a zero-count witness from the bridge.
    for expr in &il.expressions {
        if graph::native_expression_kind(expr.kind)!=4 {continue;}
        let edge=format!("gnu{}",expr.path.replace('.',"_"));
        if !seen.contains(&(expr.function.clone(),Some(expr.statement_path.clone()),edge)) {
            return Err(Error::MalformedCallback);
        }
    }
    Ok(())
}

// Independently certify the three supplemental expression tree families.
// This is deliberately generic: all use the same dense native child-path
// construction but distinct owner identities.
fn validate_native_child_witnesses<K:Eq+std::hash::Hash+Clone>(
    nodes:impl IntoIterator<Item=(K,Option<K>)>,
    proofs:impl IntoIterator<Item=(K,u32)>,
)->Result<(),Error> {
    use std::collections::HashMap;
    let mut keys=HashSet::new();
    let mut counts:HashMap<K,u32>=HashMap::new();
    for (key,parent) in nodes {
        if !keys.insert(key) {return Err(Error::MalformedCallback);}
        if let Some(parent)=parent {
            *counts.entry(parent).or_default()+=1;
        }
    }
    if counts.keys().any(|key|!keys.contains(key)) {
        return Err(Error::MalformedCallback);
    }
    let mut seen=HashSet::new();
    for (key,count) in proofs {
        if count>10000 || !keys.contains(&key) || !seen.insert(key.clone()) ||
            counts.get(&key).copied().unwrap_or(0)!=count {
            return Err(Error::MalformedCallback);
        }
    }
    if seen.len()!=keys.len() {return Err(Error::MalformedCallback);}
    Ok(())
}

fn validate_native_supplemental_child_counts(il:&Il)->Result<(),Error> {
    validate_native_child_witnesses(
        il.control_expressions.iter().map(|e|{
            let identity=|p:&str| (e.function.clone(),e.statement_path.clone(),e.role,p.to_owned());
            (identity(&e.path),e.parent.as_deref().map(identity))
        }),
        il.native_control_child_counts.iter().map(|p|
            ((p.function.clone(),p.statement_path.clone(),p.role,p.path.clone()),p.count))
    )?;
    validate_native_child_witnesses(
        il.initializer_expressions.iter().map(|e|{
            let identity=|p:&str|(e.owner,p.to_owned());
            (identity(&e.path),e.parent.as_deref().map(identity))
        }),
        il.native_initializer_child_counts.iter().map(|p|
            ((p.owner,p.path.clone()),p.count))
    )?;
    validate_native_child_witnesses(
        il.constructor_initializer_expressions.iter().map(|e|{
            let identity=|p:&str|(e.routine,e.position,p.to_owned());
            (identity(&e.path),e.parent.as_deref().map(identity))
        }),
        il.native_constructor_child_counts.iter().map(|p|
            ((p.routine,p.position,p.path.clone()),p.count))
    )
}

// Root admission must be certified by the owning EDG statement or
// initializer record; per-node child counts cannot prove that a whole tree
// was not lost together with all its child-count records.
fn validate_native_supplemental_root_admission(il:&Il)->Result<(),Error> {
    use std::collections::{HashMap,HashSet};
    let statements:HashSet<_>=il.statements.iter()
        .map(|s|(s.function.as_str(),s.path.as_str())).collect();
    let mut control_masks=HashMap::new();
    for admission in &il.native_control_root_admissions {
        let key=(admission.function.as_str(),admission.statement_path.as_str());
        if !statements.contains(&key) ||
           control_masks.insert(key,admission.mask).is_some() {
            return Err(Error::MalformedCallback);
        }
    }
    if control_masks.len()!=statements.len() {return Err(Error::MalformedCallback);}
    let mut actual_roles:HashMap<(&str,&str),u32>=HashMap::new();
    for expr in &il.control_expressions {
        if expr.parent.is_some() {continue;}
        if expr.path!="0" || !(1..=4).contains(&expr.role) {
            return Err(Error::MalformedCallback);
        }
        let mask=actual_roles.entry((&expr.function,&expr.statement_path)).or_default();
        let bit=1_u32 << (expr.role-1);
        if *mask & bit!=0 {return Err(Error::MalformedCallback);}
        *mask|=bit;
    }
    for (key,mask) in &control_masks {
        if actual_roles.get(key).copied().unwrap_or(0)!=*mask {
            return Err(Error::MalformedCallback);
        }
    }
    // Event 16 already carries native expression_kind iff the exported
    // dynamic variable initializer owns an expression. Demand root "0".
    let mut dynamic_roots=HashSet::new();
    for expr in &il.initializer_expressions {
        if expr.parent.is_none() &&
            (expr.path!="0" || !dynamic_roots.insert(expr.owner)) {
            return Err(Error::MalformedCallback);
        }
    }
    let mut dynamic_owners=HashSet::new();
    for init in &il.dynamic_initializers {
        if !dynamic_owners.insert(init.symbol) ||
           dynamic_roots.contains(&init.symbol)!=init.expression_kind.is_some() {
            return Err(Error::MalformedCallback);
        }
    }
    if dynamic_roots.len()!=il.dynamic_initializers.iter()
        .filter(|i|i.expression_kind.is_some()).count() {
        return Err(Error::MalformedCallback);
    }
    // Event 14 independently identifies dynamic-init variables. Therefore
    // dropping both event 16 and the complete expression tree still fails.
    let expected_dynamic:HashSet<_>=il.variable_initializers.iter()
        .filter(|v|graph::native_variable_dynamic_init(v.kind))
        .map(|v|v.symbol).collect();
    if expected_dynamic.len()!=il.variable_initializers.iter()
        .filter(|v|graph::native_variable_dynamic_init(v.kind)).count() ||
       expected_dynamic!=dynamic_owners {
        return Err(Error::MalformedCallback);
    }
    let mut ctor_roots=HashSet::new();
    for expr in &il.constructor_initializer_expressions {
        if expr.parent.is_none() &&
            (expr.path!="0" || !ctor_roots.insert((expr.routine,expr.position))) {
            return Err(Error::MalformedCallback);
        }
    }
    let mut ctor_admissions=HashMap::new();
    for a in &il.native_constructor_root_admissions {
        if ctor_admissions.insert((a.routine,a.position),a.present).is_some() {
            return Err(Error::MalformedCallback);
        }
    }
    let mut ctor_owners=HashSet::new();
    for init in &il.constructor_initializers {
        let key=(init.routine,init.position);
        if !ctor_owners.insert(key) ||
           ctor_admissions.get(&key).copied()!=Some(ctor_roots.contains(&key)) {
            return Err(Error::MalformedCallback);
        }
    }
    if ctor_admissions.len()!=ctor_owners.len() || ctor_roots.len()>
        ctor_admissions.len() {return Err(Error::MalformedCallback);}
    Ok(())
}

// EDG constructor-initializer list cardinality is independent of the
// initializer entry emissions: a missing last event-20/91 pair is not
// detectable from dense observed positions alone.
fn validate_native_constructor_initializer_lists(il:&Il)->Result<(),Error> {
    use std::collections::{HashMap,HashSet};
    let defined:HashSet<&str>=il.entities.iter().filter_map(|entity|
        match entity {
            Entity::Routine{name,has_definition:true} => Some(name.as_str()),
            _=>None,
        }).collect();
    let mut defined_routines=HashSet::new();
    for identity in &il.routine_identities {
        if defined.contains(identity.function.as_str()) &&
            !defined_routines.insert(identity.routine) {
            return Err(Error::MalformedCallback);
        }
    }
    // Defined-name/identity coverage is also checked by
    // validate_native_tree_count_coverage.
    if defined_routines.len()!=defined.len() {return Err(Error::MalformedCallback);}
    let mut proofs=HashMap::new();
    for witness in &il.native_constructor_initializer_counts {
        if witness.count>10000 || !defined_routines.contains(&witness.routine) ||
            proofs.insert(witness.routine,witness.count).is_some() {
            return Err(Error::MalformedCallback);
        }
    }
    if proofs.len()!=defined_routines.len() {return Err(Error::MalformedCallback);}
    let mut positions:HashMap<SymbolId,HashSet<u32>>=HashMap::new();
    for init in &il.constructor_initializers {
        let Some(&count)=proofs.get(&init.routine) else {
            return Err(Error::MalformedCallback);
        };
        if init.position>=count ||
            !positions.entry(init.routine).or_default().insert(init.position) {
            return Err(Error::MalformedCallback);
        }
    }
    for (routine,count) in proofs {
        if positions.get(&routine).map_or(0,HashSet::len)!=count as usize {
            return Err(Error::MalformedCallback);
        }
    }
    Ok(())
}

// An emitted native lifetime always owns a destruction list (possibly
// empty). Check independent EDG counts rather than accepting any dense
// prefix of the observed event-41 positions.
fn validate_native_lifetime_destruction_counts(il:&Il)->Result<(),Error> {
    use std::collections::{HashMap,HashSet};
    let mut lifetimes=HashSet::new();
    for lifetime in &il.object_lifetimes {
        if !lifetimes.insert((lifetime.routine,lifetime.id)) {
            return Err(Error::MalformedCallback);
        }
    }
    let mut witnesses=HashMap::new();
    for proof in &il.native_lifetime_destruction_counts {
        let key=(proof.routine,proof.lifetime);
        if proof.count>10000 || !lifetimes.contains(&key) ||
            witnesses.insert(key,proof.count).is_some() {
            return Err(Error::MalformedCallback);
        }
    }
    if witnesses.len()!=lifetimes.len() {return Err(Error::MalformedCallback);}
    let mut positions:HashMap<_,HashSet<u32>>=HashMap::new();
    for entry in &il.lifetime_destructions {
        let key=(entry.routine,entry.lifetime);
        let Some(&count)=witnesses.get(&key) else {
            return Err(Error::MalformedCallback);
        };
        if entry.position>=count ||
            !positions.entry(key).or_default().insert(entry.position) {
            return Err(Error::MalformedCallback);
        }
    }
    for (key,count) in witnesses {
        if positions.get(&key).map_or(0,HashSet::len)!=count as usize {
            return Err(Error::MalformedCallback);
        }
    }
    Ok(())
}

// A lifetime child-tree witness is counted by native EDG before the
// individual child nodes are emitted. A dense observed prefix is not proof
// that the final native child or an entire tail subtree was retained.
fn validate_native_lifetime_child_counts(il:&Il)->Result<(),Error> {
    use std::collections::{HashMap,HashSet};
    let mut lifetimes=HashSet::new();
    for region in &il.object_lifetimes {
        if !lifetimes.insert((region.routine,region.id)) {
            return Err(Error::MalformedCallback);
        }
    }
    let mut witnessed=HashMap::new();
    for proof in &il.native_lifetime_child_counts {
        let owner=(proof.routine,proof.lifetime);
        if proof.count>10000 || !lifetimes.contains(&owner) ||
            witnessed.insert(owner,(proof.count,proof.native_child_edge)).is_some() {
            return Err(Error::MalformedCallback);
        }
    }
    if witnessed.len()!=lifetimes.len() {return Err(Error::MalformedCallback);}
    let mut actual:HashMap<(SymbolId,u32),HashSet<u32>>=HashMap::new();
    for region in &il.object_lifetimes {
        let key=(region.routine,region.id);
        let Some(&(_,native_child_edge))=witnessed.get(&key) else {
            return Err(Error::MalformedCallback);
        };
        if native_child_edge {
            let Some(parent)=region.parent else {
                return Err(Error::MalformedCallback);
            };
            let owner=(region.routine,parent);
            if parent>=region.id || !lifetimes.contains(&owner) ||
                !actual.entry(owner).or_default().insert(region.id) {
                return Err(Error::MalformedCallback);
            }
        }
    }
    for (owner,(count,_)) in witnessed {
        if actual.get(&owner).map_or(0,HashSet::len)!=count as usize {
            return Err(Error::MalformedCallback);
        }
    }
    Ok(())
}

// Defined functions own nullable body/static lifetime roots. Native scope
// nodes independently own nullable lifetime associations and child lists.
// This closes the correlated-loss cases that a lifetime-only witness misses.
fn validate_native_lifetime_scope_owners(il:&Il)->Result<(),Error> {
    use std::collections::{HashMap,HashSet};
    let defined:HashSet<&str>=il.entities.iter().filter_map(|entity|
        match entity {
            Entity::Routine{name,has_definition:true}=>Some(name.as_str()),
            _=>None,
        }).collect();
    let mut routines=HashSet::new();
    for identity in &il.routine_identities {
        if defined.contains(identity.function.as_str()) &&
            !routines.insert(identity.routine) {
            return Err(Error::MalformedCallback);
        }
    }
    if routines.len()!=defined.len() {return Err(Error::MalformedCallback);}
    let lifetimes:HashSet<(SymbolId,u32)>=il.object_lifetimes.iter()
        .map(|item|(item.routine,item.id)).collect();
    if lifetimes.len()!=il.object_lifetimes.len() {
        return Err(Error::MalformedCallback);
    }
    let mut roots=HashMap::new();
    let mut admitted=HashSet::new();
    for entry in &il.native_lifetime_root_admissions {
        if !routines.contains(&entry.routine) ||
            roots.insert(entry.routine,(entry.body,entry.local_static)).is_some() {
            return Err(Error::MalformedCallback);
        }
        for id in [entry.body,entry.local_static].into_iter().flatten() {
            let key=(entry.routine,id);
            if !lifetimes.contains(&key) {return Err(Error::MalformedCallback);}
            admitted.insert(key);
        }
    }
    if roots.len()!=routines.len() {return Err(Error::MalformedCallback);}

    let scope_keys:HashSet<(SymbolId,u32)>=il.local_scopes.iter()
        .map(|s|(s.routine,s.id)).collect();
    if scope_keys.len()!=il.local_scopes.len() {
        return Err(Error::MalformedCallback);
    }
    let mut root_scopes=HashSet::new();
    for scope in &il.local_scopes {
        if !routines.contains(&scope.routine) ||
            (scope.id==0 && (scope.parent.is_some() ||
                             !root_scopes.insert(scope.routine))) ||
            (scope.id!=0 && scope.parent.is_none()) {
            return Err(Error::MalformedCallback);
        }
    }
    if root_scopes!=routines {return Err(Error::MalformedCallback);}

    let mut scope_admission=HashMap::new();
    for witness in &il.native_scope_lifetime_admissions {
        let key=(witness.routine,witness.scope);
        if !scope_keys.contains(&key) ||
            witness.lifetime.is_some_and(|id|
                !lifetimes.contains(&(witness.routine,id))) ||
            scope_admission.contains_key(&key) {
            return Err(Error::MalformedCallback);
        }
        scope_admission.insert(key,witness.lifetime);
        if let Some(id)=witness.lifetime {
            admitted.insert((witness.routine,id));
        }
    }
    if scope_admission.len()!=scope_keys.len() {
        return Err(Error::MalformedCallback);
    }
    let mut links=HashMap::new();
    for edge in &il.scope_lifetimes {
        let key=(edge.routine,edge.scope);
        if !scope_keys.contains(&key) ||
            links.insert(key,edge.lifetime).is_some() {
            return Err(Error::MalformedCallback);
        }
    }
    for (key,admission) in &scope_admission {
        if links.get(key).copied()!=*admission {
            return Err(Error::MalformedCallback);
        }
    }

    // Every non-child-list lifetime was introduced by a root or a scope
    // fallback. Referencing a child-only descendant cannot justify an
    // unrelated orphan lifetime node.
    for witness in &il.native_lifetime_child_counts {
        if !witness.native_child_edge &&
            !admitted.contains(&(witness.routine,witness.lifetime)) {
            return Err(Error::MalformedCallback);
        }
    }

    let mut child_counts=HashMap::new();
    for proof in &il.native_scope_child_counts {
        let key=(proof.routine,proof.scope);
        if proof.count>10000 || !scope_keys.contains(&key) ||
            child_counts.insert(key,proof.count).is_some() {
            return Err(Error::MalformedCallback);
        }
    }
    if child_counts.len()!=scope_keys.len() {return Err(Error::MalformedCallback);}
    let mut observed:HashMap<(SymbolId,u32),u32>=HashMap::new();
    for scope in &il.local_scopes {
        if let Some(parent)=scope.parent {
            if parent>=scope.id || !scope_keys.contains(&(scope.routine,parent)) {
                return Err(Error::MalformedCallback);
            }
            *observed.entry((scope.routine,parent)).or_default()+=1;
        }
    }
    for (key,count) in child_counts {
        if observed.get(&key).copied().unwrap_or(0)!=count {
            return Err(Error::MalformedCallback);
        }
    }
    Ok(())
}

// Compare EDG-source counts against every event-29 parameter and event-31
// local declaration; a dense observed prefix alone cannot prove completeness.
fn validate_native_function_declaration_counts(il:&Il)->Result<(),Error> {
    use std::collections::{HashMap,HashSet};
    let defined:HashSet<&str>=il.entities.iter().filter_map(|entity|
        match entity {
            Entity::Routine{name,has_definition:true}=>Some(name.as_str()),
            _=>None,
        }).collect();
    let mut routines=HashSet::new();
    for identity in &il.routine_identities {
        if defined.contains(identity.function.as_str()) &&
            !routines.insert(identity.routine) {
            return Err(Error::MalformedCallback);
        }
    }
    if routines.len()!=defined.len() {return Err(Error::MalformedCallback);}

    let mut parameter_proofs=HashMap::new();
    for proof in &il.native_function_parameter_counts {
        if proof.explicit>10000 || !routines.contains(&proof.routine) ||
            parameter_proofs.insert(proof.routine,
                (proof.explicit,proof.implicit_this)).is_some() {
            return Err(Error::MalformedCallback);
        }
    }
    if parameter_proofs.len()!=routines.len() {
        return Err(Error::MalformedCallback);
    }
    let mut parameter_positions:HashMap<SymbolId,HashSet<u32>>=HashMap::new();
    let mut implicit=HashSet::new();
    let mut parameter_symbols=HashSet::new();
    for param in &il.function_parameters {
        if !parameter_proofs.contains_key(&param.routine) ||
            !parameter_symbols.insert(param.symbol) {
            return Err(Error::MalformedCallback);
        }
        if param.implicit_this {
            if param.position.is_some() || !implicit.insert(param.routine) {
                return Err(Error::MalformedCallback);
            }
        } else {
            let Some(position)=param.position else {
                return Err(Error::MalformedCallback);
            };
            if !parameter_positions.entry(param.routine).or_default().insert(position) {
                return Err(Error::MalformedCallback);
            }
        }
    }
    for (routine,(expected,has_this)) in parameter_proofs {
        let positions=parameter_positions.get(&routine);
        if positions.map_or(0,HashSet::len)!=expected as usize ||
            positions.is_some_and(|positions|
                positions.iter().any(|&position|position>=expected)) ||
            implicit.contains(&routine)!=has_this {
            return Err(Error::MalformedCallback);
        }
    }

    let scopes:HashSet<(SymbolId,u32)>=il.local_scopes.iter()
        .map(|scope|(scope.routine,scope.id)).collect();
    if scopes.len()!=il.local_scopes.len() {return Err(Error::MalformedCallback);}
    let mut local_proofs=HashMap::new();
    for proof in &il.native_scope_local_counts {
        let key=(proof.routine,proof.scope);
        if proof.count>10000 || !scopes.contains(&key) ||
            local_proofs.insert(key,proof.count).is_some() {
            return Err(Error::MalformedCallback);
        }
    }
    if local_proofs.len()!=scopes.len() {return Err(Error::MalformedCallback);}
    let mut local_count:HashMap<(SymbolId,u32),u32>=HashMap::new();
    let mut local_symbols=HashSet::new();
    for local in &il.local_variables {
        let key=(local.routine,local.scope);
        if !local_proofs.contains_key(&key) ||
            !local_symbols.insert(local.symbol) {
            return Err(Error::MalformedCallback);
        }
        *local_count.entry(key).or_default()+=1;
    }
    for (key,count) in local_proofs {
        if local_count.get(&key).copied().unwrap_or(0)!=count {
            return Err(Error::MalformedCallback);
        }
    }
    Ok(())
}

/// Native event 100 is mandatory once event 47 has identified a file-scope
/// variable. A missing final provenance record must never make a template
/// variable look like ordinary executable storage or a harmless import.
/// This checks exact one-to-one ownership; the collector separately rejects
/// malformed fields and out-of-order records.
fn validate_native_global_materialization_coverage(il:&Il)->Result<(),&'static str> {
    if il.global_materializations.len()!=il.global_variables.len() {
        return Err("native file-scope variables lack complete materialization provenance");
    }
    let mut globals=HashSet::with_capacity(il.global_variables.len());
    for global in &il.global_variables {
        if !globals.insert(global.symbol) {
            return Err("duplicate native file-scope variable identity");
        }
    }
    let mut witnessed=HashSet::with_capacity(il.global_materializations.len());
    for materialization in &il.global_materializations {
        if !globals.contains(&materialization.symbol) {
            return Err("native materialization provenance has no file-scope owner");
        }
        if !witnessed.insert(materialization.symbol) {
            return Err("duplicate native materialization provenance");
        }
    }
    Ok(())
}

// The C/C++ language mode is an explicit native EDG fact (event 64).
// A C struct has ordinary field/layout records, but no C++ POD, tagless
// class-storage, constructor/destructor, or inheritance certificate.
// Reject a mismatched callback even if the rest of the record graph is
// structurally complete. C++ inputs retain the existing proof semantics.
fn validate_native_c_language_separation(il: &Il) -> Result<(), Error> {
    if il.cpp_language != Some(false) {
        return Ok(());
    }
    if il.cxx_record_admissions.iter().any(|proof| proof.plain_object)
        || !il.cxx_tagless_storage_proofs.is_empty()
        || il.cxx_scalar_lifetime_admissions.iter().any(|proof| proof.candidate)
        || il.native_record_shapes.iter().any(|shape| shape.direct_base_count != 0)
        || !il.base_classes.is_empty()
        || !il.base_subobject_offsets.is_empty()
        || !il.base_class_access.is_empty()
        || !il.member_functions.is_empty()
        || !il.native_virtual_members.is_empty()
        || !il.constructor_initializers.is_empty()
        || !il.constructor_initializer_expressions.is_empty()
        || !il.native_constructor_root_admissions.is_empty()
        || !il.native_constructor_child_counts.is_empty()
        || il.native_constructor_initializer_counts.iter().any(|proof| proof.count != 0)
    {
        return Err(Error::MalformedCallback);
    }
    Ok(())
}

fn validate_complete_native_snapshot(il:&Il) -> Result<(),Error> {
    if il.cpp_language.is_none() ||
       validate_native_c_language_separation(il).is_err() ||
       validate_native_global_materialization_coverage(il).is_err() ||
       graph::Graph::validate_native_type_detail_coverage(il).is_err() ||
       graph::Graph::validate_native_type_edge_coverage(il).is_err() ||
       graph::Graph::validate_native_routine_signature_coverage(il).is_err() ||
       graph::Graph::validate_native_class_admission_coverage(il).is_err() ||
       graph::Graph::validate_native_record_shape_coverage(il).is_err() ||
       graph::Graph::validate_native_type_layout_coverage(il).is_err() ||
       graph::Graph::validate_native_scalar_metadata_coverage(il).is_err() ||
       validate_native_scope_dynamic_initializer_coverage(il).is_err() ||
       validate_native_call_target_coverage(il).is_err() ||
       validate_native_call_dispatch_coverage(il).is_err() ||
       validate_native_virtual_member_coverage(il).is_err() ||
       validate_prelower_virtual_call_origins(il).is_err() ||
       validate_virtual_receiver_reuses(il).is_err() ||
       validate_virtual_entry_index_selections(il).is_err() ||
       validate_virtual_vptr_receiver_pairs(il).is_err() ||
       validate_native_statement_attribute_coverage(il).is_err() ||
       validate_native_tree_count_coverage(il).is_err() ||
       validate_native_supplemental_child_counts(il).is_err() ||
       validate_native_supplemental_root_admission(il).is_err() ||
       validate_native_constructor_initializer_lists(il).is_err() ||
       validate_native_lifetime_destruction_counts(il).is_err() ||
       validate_native_lifetime_child_counts(il).is_err() ||
       validate_native_lifetime_scope_owners(il).is_err() ||
       validate_native_function_declaration_counts(il).is_err() ||
       graph::Graph::from_il(il).is_err() {
        return Err(Error::MalformedCallback);
    }
    Ok(())
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
    let mut collector = Collector::default();
    let code = unsafe {
        edgcpp_compile(cstrings.len() as c_int, argv.as_mut_ptr(),
                       Some(collect), &mut collector as *mut Collector as *mut c_void)
    };
    if collector.panicked { return Err(Error::CallbackPanicked); }
    if collector.malformed { return Err(Error::MalformedCallback); }
    if code < 0 {
        return Err(Error::NativeFailure);
    }
    // Failed compilations may provide partial diagnostic snapshots; on
    // success, require every native-mandatory provenance record.
    if code == 0 {
        validate_complete_native_snapshot(&collector.il)?;
    }
    Ok(Compilation { status: code, il: collector.il })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_entry_rejects_null_callback_and_argument_vector() {
        let valid=std::ffi::CString::new("edgcpfe").unwrap();
        let mut argv=[valid.as_ptr() as *mut c_char,std::ptr::null_mut()];
        let rejected_callback=unsafe {
            edgcpp_compile(1,argv.as_mut_ptr(),None,std::ptr::null_mut())
        };
        assert_eq!(rejected_callback,-1,"a null callback must not SIGSEGV");
        let rejected_argv=unsafe {
            edgcpp_compile(1,std::ptr::null_mut(),Some(collect),std::ptr::null_mut())
        };
        assert_eq!(rejected_argv,-1,"missing argv must not reach EDG");
        let rejected_argc=unsafe {
            edgcpp_compile(0,argv.as_mut_ptr(),Some(collect),std::ptr::null_mut())
        };
        assert_eq!(rejected_argc,-1,"empty argv must not reach EDG");
    }

    #[test]
    fn native_success_requires_language_mode_provenance() {
        let mut il=Il::default();
        assert!(matches!(validate_complete_native_snapshot(&il),
            Err(Error::MalformedCallback)),
            "missing native event 64 must fail even for empty translation units");
        il.cpp_language=Some(false);
        validate_complete_native_snapshot(&il)
            .expect("complete empty C translation unit");
        il.cpp_language=Some(true);
        validate_complete_native_snapshot(&il)
            .expect("complete empty C++ translation unit");
        il.cpp_language=None;
        il.types.push(TypeDef {
            id:TypeId(1),kind:0,related_role:None,related:None,
        });
        assert!(matches!(validate_complete_native_snapshot(&il),
            Err(Error::MalformedCallback)),
            "unrelated records do not replace a language mode event");
    }

    #[test]
    fn c_mode_never_accepts_cpp_object_or_inheritance_certificates() {
        let mut il=Il { cpp_language:Some(false), ..Il::default() };
        validate_native_c_language_separation(&il)
            .expect("ordinary C aggregates need no C++ class proof");
        il.cxx_record_admissions.push(CxxRecordAdmission {
            owner:TypeId(1),plain_object:false,
        });
        il.cxx_scalar_lifetime_admissions.push(CxxScalarLifetimeAdmission {
            owner:TypeId(1),candidate:false,
        });
        validate_native_c_language_separation(&il)
            .expect("explicit zero certificates are legitimate in C");
        il.cxx_record_admissions[0].plain_object=true;
        assert!(validate_native_c_language_separation(&il).is_err());
        il.cxx_record_admissions[0].plain_object=false;
        il.cxx_scalar_lifetime_admissions[0].candidate=true;
        assert!(validate_native_c_language_separation(&il).is_err());
        il.cxx_scalar_lifetime_admissions[0].candidate=false;
        il.cxx_tagless_storage_proofs.push(TypeId(1));
        assert!(validate_native_c_language_separation(&il).is_err());
        il.cxx_tagless_storage_proofs.clear();
        il.native_record_shapes.push(NativeRecordShape {
            owner:TypeId(1),field_count:0,direct_base_count:1,
        });
        assert!(validate_native_c_language_separation(&il).is_err());
        il.native_record_shapes[0].direct_base_count=0;
        il.member_functions.push(MemberFunction{
            owner:TypeId(1),routine:SymbolId(2),
        });
        assert!(validate_native_c_language_separation(&il).is_err());
        il.member_functions.clear();
        il.native_constructor_initializer_counts.push(
            NativeConstructorInitializerCount{routine:SymbolId(2),count:1});
        assert!(validate_native_c_language_separation(&il).is_err());
        il.native_constructor_initializer_counts[0].count=0;
        validate_native_c_language_separation(&il)
            .expect("zero constructor list witnesses are valid in C");
        // The exact same native class certificates remain eligible in C++.
        il.cpp_language=Some(true);
        il.cxx_record_admissions[0].plain_object=true;
        il.cxx_scalar_lifetime_admissions[0].candidate=true;
        validate_native_c_language_separation(&il)
            .expect("C++ admission is checked by the existing full validators");
    }

    #[test]
    fn native_statement_attributes_prove_exact_statement_coverage() {
        let mut il=Il { cpp_language:Some(false), ..Il::default() };
        let statement=|path:&str| Statement {
            function:"f".into(),path:path.into(),parent:None,kind:0,
        };
        let attributes=|path:&str| StatementAttributes {
            function:"f".into(),statement_path:path.into(),
            compiler_generated:false,initialization_guard:false,
            fallthrough:false,likely:false,unlikely:false,
            lowering_generated:false,lowering_boilerplate:false,
        };
        il.statements.extend([statement("0"),statement("1")]);
        for path in ["0","1"] {
            il.native_control_root_admissions.push(NativeControlRootAdmission {
                function:"f".into(),statement_path:path.into(),mask:0,
            });
        }
        il.native_statement_sibling_counts.push(NativeStatementSiblingCount {
            function:"f".into(),parent:None,edge:String::new(),count:2,
        });
        il.statement_attributes.push(attributes("0"));
        assert!(matches!(validate_complete_native_snapshot(&il),
            Err(Error::MalformedCallback)), "missing tail metadata must fail");
        il.statement_attributes.push(attributes("1"));
        validate_complete_native_snapshot(&il).expect("complete metadata");
        il.statement_attributes.push(attributes("1"));
        assert!(matches!(validate_complete_native_snapshot(&il),
            Err(Error::MalformedCallback)), "duplicate metadata must fail");
        il.statement_attributes.pop();
        il.statement_attributes[1].statement_path="2".into();
        assert!(matches!(validate_complete_native_snapshot(&il),
            Err(Error::MalformedCallback)), "orphan metadata must fail");
    }

    #[test]
    fn defined_native_routine_must_have_independent_root_witness() {
        let mut il=Il::default();
        il.entities.push(Entity::Routine {
            name:"f".into(),has_definition:true,
        });
        il.routine_identities.push(RoutineIdentity {
            routine:SymbolId(1),function:"f".into(),
        });
        assert!(matches!(validate_native_tree_count_coverage(&il),
            Err(Error::MalformedCallback)),
            "defined function cannot lose entire root proof");
        il.native_statement_sibling_counts.push(NativeStatementSiblingCount {
            function:"f".into(),parent:None,edge:String::new(),count:0,
        });
        validate_native_tree_count_coverage(&il)
            .expect("defined native function with empty body has root witness");
        il.routine_identities.clear();
        assert!(matches!(validate_native_tree_count_coverage(&il),
            Err(Error::MalformedCallback)),
            "missing independent identity cannot hide a defined function");
        il.routine_identities.push(RoutineIdentity {
            routine:SymbolId(1),function:"f".into(),
        });
        il.native_statement_sibling_counts[0].function="g".into();
        assert!(matches!(validate_native_tree_count_coverage(&il),
            Err(Error::MalformedCallback)),
            "wrong function's proof cannot certify a defined routine");
    }

    #[test]
    fn declared_only_routine_does_not_require_root_statement_proof() {
        let mut il=Il::default();
        il.entities.push(Entity::Routine {
            name:"declaration".into(),has_definition:false,
        });
        validate_native_tree_count_coverage(&il)
            .expect("a declaration alone has no native statement list");
    }

    #[test]
    fn native_empty_statement_list_owner_requires_witness() {
        // The native bridge always traverses a block's contained list, even
        // when that list is empty. Losing its zero-count proof must fail.
        let block_kind=(0..256).find(|&kind|
            graph::native_statement_kind(kind)==2)
            .expect("EDG exposes its native block kind");
        let mut il=Il::default();
        il.statements.push(Statement {
            function:"f".into(),path:"0".into(),parent:None,kind:block_kind,
        });
        il.native_statement_sibling_counts.push(NativeStatementSiblingCount {
            function:"f".into(),parent:None,edge:String::new(),count:1,
        });
        assert!(matches!(validate_native_tree_count_coverage(&il),
            Err(Error::MalformedCallback)), "dropped empty block proof");
        il.native_statement_sibling_counts.push(NativeStatementSiblingCount {
            function:"f".into(),parent:Some("0".into()),edge:"block".into(),count:0,
        });
        validate_native_tree_count_coverage(&il)
            .expect("native empty block is certified by zero-count witness");
        il.native_statement_sibling_counts.pop();
        il.native_statement_sibling_counts.push(NativeStatementSiblingCount {
            function:"f".into(),parent:Some("0".into()),edge:"then".into(),count:0,
        });
        assert!(matches!(validate_native_tree_count_coverage(&il),
            Err(Error::MalformedCallback)), "wrong edge cannot substitute");
    }

    #[test]
    fn native_try_handler_cardinality_rejects_missing_tail_and_extra_catch() {
        let kind=(0..256).find(|&k|graph::native_statement_kind(k)==19)
            .expect("EDG try block kind");
        let mut il=Il::default();
        il.statements.push(Statement {
            function:"f".into(),path:"0".into(),parent:None,kind,
        });
        il.native_statement_sibling_counts.extend([
            NativeStatementSiblingCount {
                function:"f".into(),parent:None,edge:String::new(),count:1,
            },
            NativeStatementSiblingCount {
                function:"f".into(),parent:Some("0".into()),
                edge:"try".into(),count:0,
            },
            NativeStatementSiblingCount {
                function:"f".into(),parent:Some("0".into()),
                edge:"catch0".into(),count:0,
            },
            NativeStatementSiblingCount {
                function:"f".into(),parent:Some("0".into()),
                edge:"catch1".into(),count:0,
            },
        ]);
        assert!(matches!(validate_native_tree_count_coverage(&il),
            Err(Error::MalformedCallback)), "missing handler-count witness");
        il.native_try_handler_counts.push(NativeTryHandlerCount {
            function:"f".into(),statement_path:"0".into(),count:2,
        });
        validate_native_tree_count_coverage(&il).expect("two native handlers certified");
        il.native_statement_sibling_counts.pop();
        assert!(matches!(validate_native_tree_count_coverage(&il),
            Err(Error::MalformedCallback)), "missing final catch list");
        il.native_statement_sibling_counts.push(NativeStatementSiblingCount {
            function:"f".into(),parent:Some("0".into()),
            edge:"catch1".into(),count:0,
        });
        il.native_try_handler_counts[0].count=1;
        assert!(matches!(validate_native_tree_count_coverage(&il),
            Err(Error::MalformedCallback)), "extra catch beyond EDG count");
        il.native_try_handler_counts[0].count=3;
        assert!(matches!(validate_native_tree_count_coverage(&il),
            Err(Error::MalformedCallback)), "missing catch2");
        il.native_try_handler_counts[0].count=2;
        il.native_try_handler_counts.push(il.native_try_handler_counts[0].clone());
        assert!(matches!(validate_native_tree_count_coverage(&il),
            Err(Error::MalformedCallback)), "duplicate owner count");
    }

    #[test]
    fn specialized_native_statement_owners_require_complete_empty_proofs() {
        // These lists are EDG-owned; do not derive their presence from
        // observed child records or hard-code configuration-dependent kinds.
        for (normalized,edges) in [
            (17,&["then","else"][..]),
            (18,&["init","body"][..]),
            (19,&["try"][..]),
            (20,&["then","else"][..]),
            (21,&["then","else"][..]),
        ] {
            let kind=(0..256).find(|&k|graph::native_statement_kind(k)==normalized)
                .expect("EDG specialized statement enum");
            let mut il=Il::default();
            il.statements.push(Statement {
                function:"f".into(),path:"0".into(),parent:None,kind,
            });
            if normalized==19 {
                il.native_try_handler_counts.push(NativeTryHandlerCount {
                    function:"f".into(),statement_path:"0".into(),count:0,
                });
            }
            il.native_statement_sibling_counts.push(NativeStatementSiblingCount {
                function:"f".into(),parent:None,edge:String::new(),count:1,
            });
            for edge in edges {
                assert!(matches!(validate_native_tree_count_coverage(&il),
                    Err(Error::MalformedCallback)),
                    "missing edge {edge} on owner {normalized}");
                il.native_statement_sibling_counts.push(NativeStatementSiblingCount {
                    function:"f".into(),parent:Some("0".into()),
                    edge:(*edge).into(),count:0,
                });
            }
            validate_native_tree_count_coverage(&il)
                .expect("all specialized native owner edges witnessed");
            il.native_statement_sibling_counts.pop();
            assert!(matches!(validate_native_tree_count_coverage(&il),
                Err(Error::MalformedCallback)),
                "dropped final empty edge on owner {normalized}");
        }
    }

    #[test]
    fn gnu_statement_expression_requires_even_empty_subtree_witness() {
        let kind=(0..256).find(|&k|graph::native_expression_kind(k)==4)
            .expect("EDG GNU statement expression kind");
        let mut il=Il::default();
        il.expressions.push(Expression {
            function:"f".into(),statement_path:"0".into(),path:"0.1".into(),
            parent:None,kind,operator:None,type_kind:None,reference:None,
            type_size:None,type_alignment:None,constant_kind:None,
            constant_integer:None,related_type_role:None,
            related_type_kind:None,type_id:None,symbol_id:None,
        });
        il.native_expression_child_counts.push(NativeExpressionChildCount {
            function:"f".into(),statement_path:"0".into(),path:"0.1".into(),count:0,
        });
        assert!(matches!(validate_native_tree_count_coverage(&il),
            Err(Error::MalformedCallback)),
            "empty GNU statement subtree must be witnessed");
        il.native_statement_sibling_counts.push(NativeStatementSiblingCount {
            function:"f".into(),parent:Some("0".into()),edge:"gnu0_1".into(),count:0,
        });
        // Native graph fixtures normally also supply the owning statement.
        il.statements.push(Statement {
            function:"f".into(),path:"0".into(),parent:None,kind:0,
        });
        il.native_statement_sibling_counts.push(NativeStatementSiblingCount {
            function:"f".into(),parent:None,edge:String::new(),count:1,
        });
        validate_native_tree_count_coverage(&il)
            .expect("GNU empty statement witness is retained");
        il.native_statement_sibling_counts[0].edge="gnu0_2".into();
        assert!(matches!(validate_native_tree_count_coverage(&il),
            Err(Error::MalformedCallback)),
            "unrelated GNU subtree must not substitute");
    }

    #[test]
    fn native_if_requires_both_empty_branch_proofs() {
        let if_kind=(0..256).find(|&kind|
            graph::native_statement_kind(kind)==3)
            .expect("EDG exposes its native if kind");
        let mut il=Il::default();
        il.statements.push(Statement {
            function:"f".into(),path:"0".into(),parent:None,kind:if_kind,
        });
        for (parent,edge,count) in [(None,"",1),(Some("0"),"then",0)] {
            il.native_statement_sibling_counts.push(NativeStatementSiblingCount {
                function:"f".into(),parent:parent.map(str::to_owned),
                edge:edge.into(),count,
            });
        }
        assert!(matches!(validate_native_tree_count_coverage(&il),
            Err(Error::MalformedCallback)), "missing empty else branch witness");
        il.native_statement_sibling_counts.push(NativeStatementSiblingCount {
            function:"f".into(),parent:Some("0".into()),edge:"else".into(),count:0,
        });
        validate_native_tree_count_coverage(&il).expect("both branches witnessed");
    }

    #[test]
    fn native_count_proofs_reject_first_and_middle_record_deletions() {
        let mut il=Il::default();
        il.statements.extend((0..3).map(|i| Statement {
            function:"f".into(),path:i.to_string(),parent:None,kind:0,
        }));
        il.native_statement_sibling_counts.push(NativeStatementSiblingCount {
            function:"f".into(),parent:None,edge:String::new(),count:3,
        });
        validate_native_tree_count_coverage(&il).expect("complete sibling list");
        for removed in [0,1] {
            let mut truncated=il.clone();
            truncated.statements.remove(removed);
            assert!(matches!(validate_native_tree_count_coverage(&truncated),
                Err(Error::MalformedCallback)),
                "missing first/middle statement at {removed}");
        }
        il.expressions.extend((0..3).map(|i| Expression {
            function:"f".into(),statement_path:"0".into(),
            path:if i==0 {"0".into()} else {format!("0.{}",i-1)},
            parent:if i==0 {None} else {Some("0".into())},
            kind:1,operator:None,type_kind:None,reference:None,
            type_size:None,type_alignment:None,constant_kind:None,
            constant_integer:None,related_type_role:None,
            related_type_kind:None,type_id:None,symbol_id:None,
        }));
        il.native_expression_child_counts.extend([
            NativeExpressionChildCount {
                function:"f".into(),statement_path:"0".into(),path:"0".into(),count:2,
            },
            NativeExpressionChildCount {
                function:"f".into(),statement_path:"0".into(),path:"0.0".into(),count:0,
            },
            NativeExpressionChildCount {
                function:"f".into(),statement_path:"0".into(),path:"0.1".into(),count:0,
            },
        ]);
        validate_native_tree_count_coverage(&il).expect("complete expression tree");
        for removed in [1,2] {
            let mut truncated=il.clone();
            truncated.expressions.remove(removed);
            truncated.native_expression_child_counts.remove(removed);
            assert!(matches!(validate_native_tree_count_coverage(&truncated),
                Err(Error::MalformedCallback)),
                "missing expression at {removed}");
        }
    }

    #[test]
    fn only_native_for_increment_may_bypass_sibling_counts() {
        let for_kind=(0..256).find(|&k|graph::native_statement_kind(k)==9)
            .expect("EDG for kind");
        let expr_kind=(0..256).find(|&k|graph::native_statement_kind(k)==6)
            .expect("EDG expression kind");
        let mut il=Il::default();
        il.statements.extend([
            Statement {function:"f".into(),path:"0".into(),parent:None,kind:for_kind},
            Statement {function:"f".into(),path:"0.increment.0".into(),
                       parent:Some("0".into()),kind:expr_kind},
        ]);
        il.native_statement_sibling_counts.extend([
            NativeStatementSiblingCount {
                function:"f".into(),parent:None,edge:String::new(),count:1,
            },
            NativeStatementSiblingCount {
                function:"f".into(),parent:Some("0".into()),edge:"body".into(),count:0,
            },
        ]);
        validate_native_tree_count_coverage(&il).expect("synthetic increment");
        il.statements[0].kind=expr_kind;
        assert!(matches!(validate_native_tree_count_coverage(&il),
            Err(Error::MalformedCallback)),
            "non-for parent must not bypass its list proof");
        il.statements[0].kind=for_kind;
        il.statements[1].kind=for_kind;
        assert!(matches!(validate_native_tree_count_coverage(&il),
            Err(Error::MalformedCallback)),
            "non-expression synthetic child must fail closed");
    }

    #[test]
    fn supplemental_tree_counts_reject_missing_final_child_and_proof() {
        // Common verifier covers all three owner identities, including zero
        // children, duplicate proofs, and missing final child events.
        let nodes=vec![(0_u32,None),(1,Some(0)),(2,Some(0))];
        let proofs=vec![(0,2),(1,0),(2,0)];
        validate_native_child_witnesses(nodes.clone(),proofs.clone())
            .expect("complete supplemental tree");
        for removed in [1,2] {
            let mut missing=nodes.clone();
            missing.remove(removed);
            let mut short_proofs=proofs.clone();
            short_proofs.remove(removed);
            assert!(matches!(validate_native_child_witnesses(missing,short_proofs),
                Err(Error::MalformedCallback)));
        }
        let mut missing=proofs.clone();
        missing.pop();
        assert!(matches!(validate_native_child_witnesses(nodes.clone(),missing),
            Err(Error::MalformedCallback)));
        let mut duplicated=proofs.clone();
        duplicated.push((2,0));
        assert!(matches!(validate_native_child_witnesses(nodes,duplicated),
            Err(Error::MalformedCallback)));
    }

    #[test]
    fn parent_owned_control_roots_reject_lost_entire_trees() {
        let mut il=Il::default();
        il.statements.push(Statement {
            function:"f".into(),path:"0".into(),parent:None,kind:0,
        });
        assert!(validate_native_supplemental_root_admission(&il).is_err(),
            "a native statement must itself have a control-role admission");
        il.native_control_root_admissions.push(NativeControlRootAdmission {
            function:"f".into(),statement_path:"0".into(),mask:1,
        });
        assert!(validate_native_supplemental_root_admission(&il).is_err(),
            "missing complete role-1 tree");
        il.control_expressions.push(ControlExpression {
            function:"f".into(),statement_path:"0".into(),role:1,
            path:"0".into(),parent:None,kind:0,operator:None,
            type_id:None,symbol_id:None,
        });
        validate_native_supplemental_root_admission(&il).expect("matched role-1 root");
        il.control_expressions.clear();
        assert!(validate_native_supplemental_root_admission(&il).is_err());
        il.native_control_root_admissions[0].mask=0;
        validate_native_supplemental_root_admission(&il).expect("genuinely absent root");
        il.native_control_root_admissions.push(il.native_control_root_admissions[0].clone());
        assert!(validate_native_supplemental_root_admission(&il).is_err(),
            "duplicate parent admissions fail closed");
    }

    #[test]
    fn native_declaration_counts_reject_lost_parameter_and_local_tail_records() {
        let mut il=Il::default();
        let routine=SymbolId(7);
        il.entities.push(Entity::Routine{name:"f".into(),has_definition:true});
        il.routine_identities.push(RoutineIdentity{routine,function:"f".into()});
        let parameter=|position,symbol,implicit_this|FunctionParameter{
            routine,position,symbol:SymbolId(symbol),ty:TypeId(9),
            implicit_this,compiler_generated:false,
        };
        il.function_parameters.extend([
            parameter(Some(0),10,false),
            parameter(Some(1),11,false),
            parameter(None,12,true),
        ]);
        il.native_function_parameter_counts.push(NativeFunctionParameterCount{
            routine,explicit:2,implicit_this:true,
        });
        let scope=|id,parent|LocalScope{
            routine,id,parent,kind:0,statement_path:None,
        };
        il.local_scopes.extend([scope(0,None),scope(1,Some(0))]);
        let local=|scope,symbol|LocalVariable{
            routine,scope,symbol:SymbolId(symbol),ty:TypeId(9),
            storage_class:0,compiler_generated:false,static_storage:false,
            structured_binding:false,handler_parameter:false,
        };
        il.local_variables.extend([local(0,13),local(0,14),local(1,15)]);
        il.native_scope_local_counts.extend([
            NativeScopeLocalCount{routine,scope:0,count:2},
            NativeScopeLocalCount{routine,scope:1,count:1},
        ]);
        validate_native_function_declaration_counts(&il)
            .expect("complete declarations, including this and both scopes");
        let mut lost_last_param=il.clone();
        lost_last_param.function_parameters.retain(|p|p.position!=Some(1));
        assert!(validate_native_function_declaration_counts(&lost_last_param).is_err(),
            "dense but truncated explicit parameter prefix must fail");
        let mut lost_this=il.clone();
        lost_this.function_parameters.retain(|p|!p.implicit_this);
        assert!(validate_native_function_declaration_counts(&lost_this).is_err());
        let mut extra_this=il.clone();
        extra_this.function_parameters.push(parameter(None,21,true));
        assert!(validate_native_function_declaration_counts(&extra_this).is_err());
        let mut gap=il.clone();
        gap.function_parameters[1].position=Some(3);
        assert!(validate_native_function_declaration_counts(&gap).is_err());
        let mut duplicate_params=il.clone();
        duplicate_params.native_function_parameter_counts.push(
            duplicate_params.native_function_parameter_counts[0].clone());
        assert!(validate_native_function_declaration_counts(&duplicate_params).is_err());
        let mut no_param_witness=il.clone();
        no_param_witness.native_function_parameter_counts.clear();
        assert!(validate_native_function_declaration_counts(&no_param_witness).is_err());
        let mut mismatched_this=il.clone();
        mismatched_this.native_function_parameter_counts[0].implicit_this=false;
        assert!(validate_native_function_declaration_counts(&mismatched_this).is_err());

        let mut last_local=il.clone();
        last_local.local_variables.retain(|v|v.symbol!=SymbolId(15));
        assert!(validate_native_function_declaration_counts(&last_local).is_err(),
            "a missing final variable in the last nested scope must fail");
        let mut last_same_scope=il.clone();
        last_same_scope.local_variables.retain(|v|v.symbol!=SymbolId(14));
        assert!(validate_native_function_declaration_counts(&last_same_scope).is_err());
        let mut no_local_witness=il.clone();
        no_local_witness.native_scope_local_counts.remove(1);
        assert!(validate_native_function_declaration_counts(&no_local_witness).is_err());
        let mut dup_scope_witness=il.clone();
        dup_scope_witness.native_scope_local_counts.push(
            NativeScopeLocalCount{routine,scope:1,count:1});
        assert!(validate_native_function_declaration_counts(&dup_scope_witness).is_err());
        let mut wrong_scope=il.clone();
        wrong_scope.local_variables[2].scope=0;
        assert!(validate_native_function_declaration_counts(&wrong_scope).is_err());
        let mut bogus_scope=il.clone();
        bogus_scope.native_scope_local_counts[1].scope=200;
        assert!(validate_native_function_declaration_counts(&bogus_scope).is_err());
        let mut duplicate_local=il.clone();
        duplicate_local.local_variables.push(local(1,15));
        assert!(validate_native_function_declaration_counts(&duplicate_local).is_err());

        let mut empty=il.clone();
        empty.function_parameters.clear();
        empty.local_variables.clear();
        empty.native_function_parameter_counts[0]=NativeFunctionParameterCount{
            routine,explicit:0,implicit_this:false,
        };
        empty.native_scope_local_counts[0].count=0;
        empty.native_scope_local_counts[1].count=0;
        validate_native_function_declaration_counts(&empty)
            .expect("all-zero native declaration proofs are valid");
        empty.native_scope_local_counts.remove(1);
        assert!(validate_native_function_declaration_counts(&empty).is_err(),
            "even empty native scopes require a declaration count proof");
    }

    #[test]
    fn native_lifetime_owners_reject_missing_roots_scope_links_and_tail_scopes() {
        let mut il=Il::default();
        let routine=SymbolId(7);
        il.entities.push(Entity::Routine{name:"f".into(),has_definition:true});
        il.routine_identities.push(RoutineIdentity{routine,function:"f".into()});
        il.object_lifetimes.extend([
            ObjectLifetime{routine,id:0,parent:None,kind:0},
            ObjectLifetime{routine,id:1,parent:Some(0),kind:0},
            // EDG scope fallback with a parent pointer, but not a native
            // child-list traversal edge.
            ObjectLifetime{routine,id:2,parent:Some(0),kind:0},
        ]);
        il.native_lifetime_child_counts.extend([
            NativeLifetimeChildCount{routine,lifetime:0,count:1,native_child_edge:false},
            NativeLifetimeChildCount{routine,lifetime:1,count:0,native_child_edge:true},
            NativeLifetimeChildCount{routine,lifetime:2,count:0,native_child_edge:false},
        ]);
        let scope=|id,parent|LocalScope{
            routine,id,parent,kind:0,statement_path:None,
        };
        il.local_scopes.extend([
            scope(0,None),scope(1,Some(0)),scope(2,Some(0)),
            scope(3,Some(1)),
        ]);
        il.scope_lifetimes.extend([
            ScopeLifetime{routine,scope:0,lifetime:0},
            ScopeLifetime{routine,scope:1,lifetime:1},
            ScopeLifetime{routine,scope:2,lifetime:2},
        ]);
        il.native_scope_lifetime_admissions.extend([
            NativeScopeLifetimeAdmission{routine,scope:0,lifetime:Some(0)},
            NativeScopeLifetimeAdmission{routine,scope:1,lifetime:Some(1)},
            NativeScopeLifetimeAdmission{routine,scope:2,lifetime:Some(2)},
            NativeScopeLifetimeAdmission{routine,scope:3,lifetime:None},
        ]);
        il.native_scope_child_counts.extend([
            NativeScopeChildCount{routine,scope:0,count:2},
            NativeScopeChildCount{routine,scope:1,count:1},
            NativeScopeChildCount{routine,scope:2,count:0},
            NativeScopeChildCount{routine,scope:3,count:0},
        ]);
        assert!(validate_native_lifetime_scope_owners(&il).is_err(),
            "missing function roots even if scopes own the same regions");
        il.native_lifetime_root_admissions.push(NativeLifetimeRootAdmission {
            routine,body:Some(0),local_static:Some(0),
        });
        validate_native_lifetime_scope_owners(&il)
            .expect("aliasing roots, nullable scope, and fallback parent accepted");

        let mut missing_root=il.clone();
        missing_root.native_lifetime_root_admissions.clear();
        assert!(validate_native_lifetime_scope_owners(&missing_root).is_err());
        let mut missing_root_node=il.clone();
        missing_root_node.object_lifetimes.retain(|region|region.id!=0);
        assert!(validate_native_lifetime_scope_owners(&missing_root_node).is_err());
        let mut unknown_root=il.clone();
        unknown_root.native_lifetime_root_admissions[0].body=Some(99);
        assert!(validate_native_lifetime_scope_owners(&unknown_root).is_err());
        let mut duplicate_root=il.clone();
        duplicate_root.native_lifetime_root_admissions.push(
            duplicate_root.native_lifetime_root_admissions[0].clone());
        assert!(validate_native_lifetime_scope_owners(&duplicate_root).is_err());

        let mut absent_scope=il.clone();
        absent_scope.native_scope_lifetime_admissions.remove(3);
        assert!(validate_native_lifetime_scope_owners(&absent_scope).is_err(),
            "even no-lifetime scopes need nullable admission");
        let mut duplicate_absent=il.clone();
        duplicate_absent.native_scope_lifetime_admissions.push(
            NativeScopeLifetimeAdmission{routine,scope:3,lifetime:None});
        assert!(validate_native_lifetime_scope_owners(&duplicate_absent).is_err(),
            "duplicate absent scope witness must fail");
        let mut lost_link=il.clone();
        lost_link.scope_lifetimes.retain(|e|e.scope!=2);
        assert!(validate_native_lifetime_scope_owners(&lost_link).is_err());
        let mut forged_link=il.clone();
        forged_link.scope_lifetimes.push(ScopeLifetime{routine,scope:3,lifetime:2});
        assert!(validate_native_lifetime_scope_owners(&forged_link).is_err());
        let mut wrong_link=il.clone();
        wrong_link.native_scope_lifetime_admissions[1].lifetime=Some(2);
        assert!(validate_native_lifetime_scope_owners(&wrong_link).is_err());

        // Lose the whole last scope, its nullable link and child-count
        // proof: the parent's independently counted native children remain.
        let mut lost_subtree=il.clone();
        lost_subtree.local_scopes.retain(|s|s.id!=2);
        lost_subtree.native_scope_lifetime_admissions.retain(|s|s.scope!=2);
        lost_subtree.scope_lifetimes.retain(|s|s.scope!=2);
        lost_subtree.native_scope_child_counts.retain(|s|s.scope!=2);
        assert!(validate_native_lifetime_scope_owners(&lost_subtree).is_err());
        let mut lost_child_count=il.clone();
        lost_child_count.native_scope_child_counts.retain(|w|w.scope!=3);
        assert!(validate_native_lifetime_scope_owners(&lost_child_count).is_err());
        let mut extra_child_count=il.clone();
        extra_child_count.native_scope_child_counts[0].count=3;
        assert!(validate_native_lifetime_scope_owners(&extra_child_count).is_err());
        let mut no_root_scope=il.clone();
        no_root_scope.local_scopes.retain(|s|s.id!=0);
        no_root_scope.native_scope_lifetime_admissions.retain(|s|s.scope!=0);
        no_root_scope.native_scope_child_counts.retain(|s|s.scope!=0);
        no_root_scope.scope_lifetimes.retain(|s|s.scope!=0);
        assert!(validate_native_lifetime_scope_owners(&no_root_scope).is_err());

        let mut orphan=il.clone();
        orphan.object_lifetimes.push(ObjectLifetime {
            routine,id:3,parent:None,kind:0,
        });
        orphan.native_lifetime_child_counts.push(NativeLifetimeChildCount{
            routine,lifetime:3,count:0,native_child_edge:false,
        });
        assert!(validate_native_lifetime_scope_owners(&orphan).is_err(),
            "unadmitted standalone region fails even with its own count");
    }

    #[test]
    fn lifetime_child_counts_reject_dropped_subtrees_and_mismatched_owners() {
        let mut il=Il::default();
        let routine=SymbolId(7);
        let region=|id,parent|ObjectLifetime{routine,id,parent,kind:0};
        let witness=|lifetime,count,native_child_edge|NativeLifetimeChildCount{
            routine,lifetime,count,native_child_edge,
        };
        // Three siblings and a grandchild; the unrelated root exercises
        // the legitimate multiple-root case.
        il.object_lifetimes.extend([
            region(0,None),region(1,Some(0)),region(2,Some(0)),
            region(3,Some(0)),region(4,Some(3)),region(5,None),
            // This is a scope fallback: EDG reports a known parent,
            // but the lifetime was not on that parent's child list.
            region(6,Some(0)),
        ]);
        assert!(validate_native_lifetime_child_counts(&il).is_err(),
            "even empty root/leaf lists must be certified");
        il.native_lifetime_child_counts.extend([
            witness(0,3,false),witness(1,0,true),witness(2,0,true),
            witness(3,1,true),witness(4,0,true),witness(5,0,false),
            witness(6,0,false),
        ]);
        validate_native_lifetime_child_counts(&il)
            .expect("complete child lists and scope-fallback parent");
        for removed in [1,2,3] {
            let mut dropped=il.clone();
            dropped.object_lifetimes.retain(|r|r.id!=removed &&
                !(removed==3 && r.id==4));
            dropped.native_lifetime_child_counts.retain(|w|w.lifetime!=removed &&
                !(removed==3 && w.lifetime==4));
            assert!(validate_native_lifetime_child_counts(&dropped).is_err(),
                "dropped entire first/middle/tail subtree: {removed}");
        }
        let mut missing=il.clone();
        missing.native_lifetime_child_counts.retain(|w|w.lifetime!=5);
        assert!(validate_native_lifetime_child_counts(&missing).is_err(),
            "missing zero-child witness");
        let mut orphan=il.clone();
        orphan.native_lifetime_child_counts[0].lifetime=90;
        assert!(validate_native_lifetime_child_counts(&orphan).is_err(),
            "orphan witness");
        let mut duplicate=il.clone();
        duplicate.native_lifetime_child_counts.push(witness(0,3,false));
        assert!(validate_native_lifetime_child_counts(&duplicate).is_err(),
            "duplicate witness");
        let mut extra=il.clone();
        extra.native_lifetime_child_counts[0].count=4;
        assert!(validate_native_lifetime_child_counts(&extra).is_err(),
            "wrong native count");
        let mut missing_native_edge=il.clone();
        missing_native_edge.native_lifetime_child_counts[1].native_child_edge=false;
        assert!(validate_native_lifetime_child_counts(&missing_native_edge).is_err(),
            "a child-list edge cannot silently become a fallback");
        let mut false_root_edge=il.clone();
        false_root_edge.native_lifetime_child_counts[0].native_child_edge=true;
        assert!(validate_native_lifetime_child_counts(&false_root_edge).is_err(),
            "native child-list edge requires a concrete parent");
        let mut wrong_parent=il.clone();
        wrong_parent.object_lifetimes[2].parent=Some(3);
        assert!(validate_native_lifetime_child_counts(&wrong_parent).is_err(),
            "contradictory parent edge");
        let mut dangling_parent=il.clone();
        dangling_parent.object_lifetimes[2].parent=Some(99);
        assert!(validate_native_lifetime_child_counts(&dangling_parent).is_err(),
            "missing parent owner");
        let mut duplicate_region=il.clone();
        duplicate_region.object_lifetimes.push(region(2,Some(0)));
        assert!(validate_native_lifetime_child_counts(&duplicate_region).is_err(),
            "duplicate region identity");
    }

    #[test]
    fn lifetime_destruction_counts_reject_missing_first_middle_and_tail() {
        let mut il=Il::default();
        il.object_lifetimes.push(ObjectLifetime {
            routine:SymbolId(7),id:0,parent:None,kind:0,
        });
        assert!(validate_native_lifetime_destruction_counts(&il).is_err(),
            "missing zero-length list witness");
        il.native_lifetime_destruction_counts.push(NativeLifetimeDestructionCount {
            routine:SymbolId(7),lifetime:0,count:0,
        });
        validate_native_lifetime_destruction_counts(&il)
            .expect("native empty destruction list");
        let item=|position|LifetimeDestruction {
            routine:SymbolId(7),lifetime:0,position,
            variable:None,destructor:None,
        };
        il.lifetime_destructions.extend((0..3).map(item));
        il.native_lifetime_destruction_counts[0].count=3;
        validate_native_lifetime_destruction_counts(&il)
            .expect("complete native destruction list");
        for removed in 0..3 {
            let mut truncated=il.clone();
            truncated.lifetime_destructions.remove(removed);
            assert!(validate_native_lifetime_destruction_counts(&truncated).is_err(),
                "missing destruction {removed}");
        }
        let mut duplicated=il.clone();
        duplicated.lifetime_destructions.push(item(2));
        assert!(validate_native_lifetime_destruction_counts(&duplicated).is_err(),
            "duplicate destruction position");
        let mut extra=il.clone();
        extra.lifetime_destructions.push(item(3));
        assert!(validate_native_lifetime_destruction_counts(&extra).is_err(),
            "extra destruction beyond native count");
        let mut dropped=il.clone();
        dropped.native_lifetime_destruction_counts.clear();
        assert!(validate_native_lifetime_destruction_counts(&dropped).is_err(),
            "lost list witness");
        let mut orphan=il.clone();
        orphan.native_lifetime_destruction_counts[0].lifetime=99;
        assert!(validate_native_lifetime_destruction_counts(&orphan).is_err(),
            "orphan witness");
        il.native_lifetime_destruction_counts.push(
            il.native_lifetime_destruction_counts[0].clone());
        assert!(validate_native_lifetime_destruction_counts(&il).is_err(),
            "duplicate native witness");
    }

    #[test]
    fn all_lifetime_regions_require_independent_zero_count_witnesses() {
        let mut il=Il::default();
        il.object_lifetimes.extend((0..2).map(|id|ObjectLifetime{
            routine:SymbolId(9),id,parent:None,kind:0,
        }));
        il.native_lifetime_destruction_counts.push(NativeLifetimeDestructionCount{
            routine:SymbolId(9),lifetime:0,count:0,
        });
        assert!(validate_native_lifetime_destruction_counts(&il).is_err());
        il.native_lifetime_destruction_counts.push(NativeLifetimeDestructionCount{
            routine:SymbolId(9),lifetime:1,count:0,
        });
        validate_native_lifetime_destruction_counts(&il).expect("two empty regions");
    }

    #[test]
    fn constructor_initializer_list_proofs_reject_missing_first_middle_and_tail() {
        let mut il=Il::default();
        il.entities.push(Entity::Routine{name:"ctor".into(),has_definition:true});
        il.routine_identities.push(RoutineIdentity{
            routine:SymbolId(7),function:"ctor".into(),
        });
        let item=|position|ConstructorInitializer{
            routine:SymbolId(7),position,kind:3,target_type:None,
            target_field:None,compiler_generated:false,braced:false,
            use_field_initializer:false,pack_expansion:false,
        };
        assert!(validate_native_constructor_initializer_lists(&il).is_err(),
            "even an empty native list must carry an independent witness");
        il.native_constructor_initializer_counts.push(
            NativeConstructorInitializerCount{routine:SymbolId(7),count:0});
        validate_native_constructor_initializer_lists(&il)
            .expect("empty native list certified");
        il.constructor_initializers.extend((0..3).map(item));
        il.native_constructor_initializer_counts[0].count=3;
        validate_native_constructor_initializer_lists(&il)
            .expect("complete native list");
        for removed in 0..3 {
            let mut truncated=il.clone();
            truncated.constructor_initializers.remove(removed);
            assert!(validate_native_constructor_initializer_lists(&truncated).is_err(),
                "dropping item {removed} must fail");
        }
        let mut duplicated=il.clone();
        duplicated.constructor_initializers.push(item(2));
        assert!(validate_native_constructor_initializer_lists(&duplicated).is_err(),
            "duplicate initializer position must fail");
        let mut extra=il.clone();
        extra.constructor_initializers.push(item(3));
        assert!(validate_native_constructor_initializer_lists(&extra).is_err(),
            "unexpected extra initializer must fail");
        let mut orphan=il.clone();
        orphan.native_constructor_initializer_counts[0].routine=SymbolId(99);
        assert!(validate_native_constructor_initializer_lists(&orphan).is_err(),
            "orphan list count must fail");
        let mut no_proof=il.clone();
        no_proof.native_constructor_initializer_counts.clear();
        assert!(validate_native_constructor_initializer_lists(&no_proof).is_err(),
            "entire list witness cannot disappear");
        let mut two_proofs=il.clone();
        two_proofs.native_constructor_initializer_counts.push(
            two_proofs.native_constructor_initializer_counts[0].clone());
        assert!(validate_native_constructor_initializer_lists(&two_proofs).is_err(),
            "duplicate native list witness must fail");
    }

    #[test]
    fn constructor_list_count_is_per_defined_routine() {
        let mut il=Il::default();
        for (name,id) in [("one",7),("two",8)] {
            il.entities.push(Entity::Routine{name:name.into(),has_definition:true});
            il.routine_identities.push(RoutineIdentity{
                routine:SymbolId(id),function:name.into(),
            });
        }
        il.native_constructor_initializer_counts.push(
            NativeConstructorInitializerCount{routine:SymbolId(7),count:0});
        assert!(validate_native_constructor_initializer_lists(&il).is_err(),
            "second defined routine also needs a zero count");
        il.native_constructor_initializer_counts.push(
            NativeConstructorInitializerCount{routine:SymbolId(8),count:0});
        validate_native_constructor_initializer_lists(&il)
            .expect("both empty lists independently certified");
        il.entities.push(Entity::Routine{name:"declaration".into(),has_definition:false});
        validate_native_constructor_initializer_lists(&il)
            .expect("declarations do not own a native initializer list");
    }

    #[test]
    fn dynamic_and_constructor_roots_must_match_native_owner_admission() {
        let mut il=Il::default();
        let dynamic_kind=(0..256).find(|&k|
            graph::native_variable_dynamic_init(k)).expect("EDG dynamic init kind");
        il.variable_initializers.push(VariableInitializer {
            symbol:SymbolId(7),kind:dynamic_kind,explicit:true,uninitialized:false,
        });
        il.dynamic_initializers.push(DynamicInitializer {
            symbol:SymbolId(7),kind:1,expression_kind:Some(3),operator:None,
        });
        assert!(validate_native_supplemental_root_admission(&il).is_err(),
            "event 16 mandates a dynamic initializer tree");
        il.initializer_expressions.push(InitializerExpression {
            owner:SymbolId(7),path:"0".into(),parent:None,kind:3,
            operator:None,type_id:None,symbol_id:None,
        });
        validate_native_supplemental_root_admission(&il).expect("dynamic root retained");
        let saved=il.dynamic_initializers.pop().expect("native dynamic initializer");
        assert!(validate_native_supplemental_root_admission(&il).is_err(),
            "losing both event16 and its root must fail event14 coverage");
        il.dynamic_initializers.push(saved);
        il.constructor_initializers.push(ConstructorInitializer {
            routine:SymbolId(8),position:0,kind:3,target_type:None,target_field:None,
            compiler_generated:false,braced:false,use_field_initializer:false,
            pack_expansion:false,
        });
        assert!(validate_native_supplemental_root_admission(&il).is_err(),
            "event 20 requires an independent constructor-root admission");
        il.native_constructor_root_admissions.push(NativeConstructorRootAdmission {
            routine:SymbolId(8),position:0,present:true,
        });
        assert!(validate_native_supplemental_root_admission(&il).is_err(),
            "lost whole constructor expression must fail");
        il.constructor_initializer_expressions.push(ConstructorInitializerExpression {
            routine:SymbolId(8),position:0,path:"0".into(),parent:None,kind:3,
            operator:None,type_id:None,symbol_id:None,
        });
        validate_native_supplemental_root_admission(&il)
            .expect("all native expression roots retained");
        il.native_constructor_root_admissions[0].present=false;
        assert!(validate_native_supplemental_root_admission(&il).is_err(),
            "contradictory native constructor admission");
    }

    #[test]
    fn supplemental_count_witnesses_are_required_for_all_three_families() {
        let mut il=Il::default();
        il.control_expressions.push(ControlExpression {
            function:"f".into(),statement_path:"0".into(),role:1,path:"0".into(),
            parent:None,kind:0,operator:None,type_id:None,symbol_id:None,
        });
        il.initializer_expressions.push(InitializerExpression {
            owner:SymbolId(7),path:"0".into(),parent:None,kind:0,
            operator:None,type_id:None,symbol_id:None,
        });
        il.constructor_initializer_expressions.push(ConstructorInitializerExpression {
            routine:SymbolId(8),position:0,path:"0".into(),parent:None,kind:0,
            operator:None,type_id:None,symbol_id:None,
        });
        assert!(validate_native_supplemental_child_counts(&il).is_err());
        il.native_control_child_counts.push(NativeControlChildCount {
            function:"f".into(),statement_path:"0".into(),role:1,path:"0".into(),count:0,
        });
        assert!(validate_native_supplemental_child_counts(&il).is_err());
        il.native_initializer_child_counts.push(NativeInitializerChildCount {
            owner:SymbolId(7),path:"0".into(),count:0,
        });
        assert!(validate_native_supplemental_child_counts(&il).is_err());
        il.native_constructor_child_counts.push(NativeConstructorChildCount {
            routine:SymbolId(8),position:0,path:"0".into(),count:0,
        });
        validate_native_supplemental_child_counts(&il).expect("all native witnesses");
        il.native_constructor_child_counts[0].count=1;
        assert!(validate_native_supplemental_child_counts(&il).is_err());
    }

    #[test]
    fn native_tree_counts_reject_dropped_final_child_and_sibling() {
        let mut il=Il::default();
        let expr=|path:&str,parent:Option<&str>| Expression {
            function:"f".into(),statement_path:"0".into(),
            path:path.into(),parent:parent.map(str::to_owned),kind:1,
            operator:None,type_kind:None,reference:None,type_size:None,
            type_alignment:None,constant_kind:None,constant_integer:None,
            related_type_role:None,related_type_kind:None,type_id:None,symbol_id:None,
        };
        il.expressions.extend([expr("0",None),expr("0.0",Some("0")),
                               expr("0.1",Some("0"))]);
        for (path,count) in [("0",2),("0.0",0),("0.1",0)] {
            il.native_expression_child_counts.push(NativeExpressionChildCount {
                function:"f".into(),statement_path:"0".into(),path:path.into(),count,
            });
        }
        validate_native_tree_count_coverage(&il).expect("complete operands");
        il.expressions.pop();
        il.native_expression_child_counts.pop();
        assert!(matches!(validate_native_tree_count_coverage(&il),
            Err(Error::MalformedCallback)), "missing last child must not shrink arity");
        il.expressions.push(expr("0.1",Some("0")));
        il.native_expression_child_counts.push(NativeExpressionChildCount {
            function:"f".into(),statement_path:"0".into(),path:"0.1".into(),count:0,
        });
        il.statements.extend([
            Statement {function:"f".into(),path:"0".into(),parent:None,kind:0},
            Statement {function:"f".into(),path:"1".into(),parent:None,kind:0},
        ]);
        il.native_statement_sibling_counts.push(NativeStatementSiblingCount {
            function:"f".into(),parent:None,edge:String::new(),count:2,
        });
        validate_native_tree_count_coverage(&il).expect("complete siblings");
        il.statements.pop();
        assert!(matches!(validate_native_tree_count_coverage(&il),
            Err(Error::MalformedCallback)), "missing final sibling must fail");
        il.statements.push(Statement {
            function:"f".into(),path:"1".into(),parent:None,kind:0,
        });
        il.native_statement_sibling_counts.push(il.native_statement_sibling_counts[0].clone());
        assert!(matches!(validate_native_tree_count_coverage(&il),
            Err(Error::MalformedCallback)), "duplicate proof must fail");
    }

    #[test]
    fn native_success_rejects_duplicate_symbol_identity() {
        let mut il = Il { cpp_language: Some(false), ..Il::default() };
        il.symbols.extend([
            Symbol { id: SymbolId(1), kind: 1, name: "first".into() },
            Symbol { id: SymbolId(1), kind: 2, name: "second".into() },
        ]);
        assert!(matches!(graph::Graph::from_il(&il),
            Err(graph::GraphError::DuplicateSymbol(SymbolId(1)))));
        assert!(matches!(validate_complete_native_snapshot(&il),
            Err(Error::MalformedCallback)));
    }

    #[test]
    fn native_success_accepts_valid_symbol_identity() {
        let mut il = Il { cpp_language: Some(true), ..Il::default() };
        il.symbols.push(Symbol { id: SymbolId(1), kind: 1, name: "routine".into() });
        validate_complete_native_snapshot(&il)
            .expect("well-formed symbol must be accepted");
    }

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
    fn scope_tree_does_not_split_qualified_template_arguments() {
        let mut il = Il::default();
        il.entities.extend([
            Entity::Variable {
                name: "outer::Map<types::Key, std::vector<types::Value>>::member".into(),
            },
            Entity::Routine {
                name: "outer::Map<types::Key, std::vector<types::Value>>::read(arg::Token)".into(),
                has_definition: true,
            },
            Entity::Routine {
                name: "outer::Functor<other::Fn(int)>::apply(arg::Token)".into(),
                has_definition: true,
            },
        ]);
        let tree = il.scope_tree();
        let outer = tree.namespace(&["outer"]).expect("outer namespace");
        let map = outer.namespace(&["Map<types::Key, std::vector<types::Value>>"])
            .expect("templated class scope");
        assert_eq!(map.declarations, vec![
            Entity::Variable { name: "member".into() },
            Entity::Routine { name: "read(arg::Token)".into(), has_definition: true },
        ]);
        let functor = outer.namespace(&["Functor<other::Fn(int)>"])
            .expect("function-type template argument");
        assert_eq!(functor.declarations, vec![
            Entity::Routine { name: "apply(arg::Token)".into(), has_definition: true },
        ]);
        assert!(outer.namespace(&["Map<types"]).is_none());
        assert!(outer.namespace(&["Functor<other"]).is_none());
    }

    #[test]
    fn qualified_scope_parser_keeps_operator_names_and_plain_declarations() {
        assert_eq!(qualified_scope_components("outer::operator<(other::Token)"),
            (vec!["outer", "operator<(other::Token)"], "outer::operator<(other::Token)".len()));
        assert_eq!(qualified_scope_components("plain"),
            (vec!["plain"], 5));
        assert_eq!(qualified_scope_components("ns::f(other::X)"),
            (vec!["ns", "f"], "ns::f".len()));
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
        let mut state = Collector::default();
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
        let mut state = Collector::default();
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
        let mut state = Collector::default();
        let attributes = [number(3), number(1), number(1 | 4 | 8 | 512 | 1024)];
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
        assert!(attributes.compiler_generated);
        assert!(!attributes.is_constexpr);
        let expr = &state.il.constructor_initializer_expressions[0];
        assert_eq!((expr.routine, expr.position), (SymbolId(3), 2));
        assert_eq!(expr.path, "0");
        assert_eq!(expr.type_id, Some(TypeId(9)));
        let invalid_attributes = [number(3), number(1), number(2048)];
        unsafe {
            collect((&mut state as *mut Collector).cast(), 22,
                    invalid_attributes.as_ptr(), invalid_attributes.len(), 0);
        }
        assert!(state.malformed);
    }

    #[test]
    fn tagless_storage_certificate_never_upgrades_class_to_pod() {
        let field=|value| WireField {
            data:std::ptr::null(),length:0,value,tag:2,
        };
        let mut state=Collector::default();
        let tagless=[field(733),field(2)];
        unsafe {
            collect((&mut state as *mut Collector).cast(),63,
                tagless.as_ptr(),tagless.len(),0);
        }
        assert!(!state.malformed);
        assert_eq!(state.il.cxx_tagless_storage_proofs,vec![TypeId(733)]);
        assert_eq!(state.il.cxx_record_admissions,vec![
            CxxRecordAdmission{owner:TypeId(733),plain_object:false}
        ]);
        // A forged, unknown semantic class discriminant must fail closed.
        let bad=[field(734),field(3)];
        unsafe {
            collect((&mut state as *mut Collector).cast(),63,
                bad.as_ptr(),bad.len(),0);
        }
        assert!(state.malformed);
    }

    #[test]
    fn decodes_cpp_dialect_and_immutable_record_triviality_proofs() {
        let number = |value| WireField {
            data:std::ptr::null(),length:0,value,tag:2,
        };
        let mut state=Collector::default();
        let cpp=[number(1)];
        let triv=[number(42),number(1)];
        let ctor=[number(43),number(0)];
        let lifetime_yes=[number(43),number(1)];
        let lifetime_no=[number(42),number(0)];
        unsafe {
            let data=(&mut state as *mut Collector).cast();
            collect(data,64,cpp.as_ptr(),cpp.len(),0);
            collect(data,63,triv.as_ptr(),triv.len(),0);
            collect(data,63,ctor.as_ptr(),ctor.len(),0);
            collect(data,65,lifetime_yes.as_ptr(),lifetime_yes.len(),0);
            collect(data,65,lifetime_no.as_ptr(),lifetime_no.len(),0);
        }
        assert!(!state.malformed);
        assert_eq!(state.il.cpp_language,Some(true));
        assert_eq!(state.il.cxx_record_admissions,vec![
            CxxRecordAdmission {owner:TypeId(42),plain_object:true},
            CxxRecordAdmission {owner:TypeId(43),plain_object:false},
        ]);
        assert_eq!(state.il.cxx_scalar_lifetime_admissions,vec![
            CxxScalarLifetimeAdmission{owner:TypeId(43),candidate:true},
            CxxScalarLifetimeAdmission{owner:TypeId(42),candidate:false},
        ]);
        unsafe {
            let data=(&mut state as *mut Collector).cast();
            collect(data,63,triv.as_ptr(),triv.len(),0);
        }
        assert!(state.malformed, "duplicate class proof must fail closed");
    }

    #[test]
    fn decodes_exact_distinct_edg_cxx_link_names_without_demangling() {
        let string = |bytes: &[u8]| WireField {
            data: bytes.as_ptr(),length: bytes.len(),value:0,tag:1,
        };
        let number = |value| WireField {
            data:std::ptr::null(),length:0,value,tag:2,
        };
        let mut state=Collector::default();
        let first=[number(10),number(1),string(b"ns::f")];
        let second=[number(11),number(1),string(b"ns::f")];
        let c=[number(12),number(1),string(b"extern_c")];
        let mangled_i=[number(10),string(b"_ZN2ns1fEi"),number(1)];
        let mangled_l=[number(11),string(b"_ZN2ns1fEl"),number(1)];
        let plain_c=[number(12),string(b"extern_c"),number(0)];
        unsafe {
            let userdata=(&mut state as *mut Collector).cast();
            for fields in [&first[..],&second[..],&c[..]] {
                collect(userdata,13,fields.as_ptr(),fields.len(),0);
            }
            for fields in [&mangled_i[..],&mangled_l[..],&plain_c[..]] {
                collect(userdata,61,fields.as_ptr(),fields.len(),0);
            }
        }
        assert!(!state.malformed);
        assert_eq!(state.il.symbols[0].name,state.il.symbols[1].name);
        assert_eq!(state.il.symbol_link_names,vec![
            SymbolLinkName {symbol:SymbolId(10),link_name:"_ZN2ns1fEi".into(),
                edg_mangled:true},
            SymbolLinkName {symbol:SymbolId(11),link_name:"_ZN2ns1fEl".into(),
                edg_mangled:true},
            SymbolLinkName {symbol:SymbolId(12),link_name:"extern_c".into(),
                edg_mangled:false},
        ]);
    }

    #[test]
    fn decodes_unique_native_scalar_abi_presence_proof() {
        let number=|v|WireField {
            data:std::ptr::null(),length:0,value:v,tag:wire_tags::UNSIGNED,
        };
        let mut state=Collector::default();
        let fields=[number(11),number(5)];
        unsafe {
            collect((&mut state as *mut Collector).cast(),76,
                    fields.as_ptr(),fields.len(),0);
        }
        assert!(!state.malformed);
        assert_eq!(state.il.native_scalar_proofs,vec![
            NativeScalarMetadataProof{owner:TypeId(11),integer_abi:true,
                standard_float_abi:false,signed_i64:true},
        ]);
        unsafe {
            collect((&mut state as *mut Collector).cast(),76,
                    fields.as_ptr(),fields.len(),0);
        }
        assert!(state.malformed,"duplicate scalar presence proof must fail");
        for values in [[0,0],[8,8],[8,3],[8,4],[8,7]] {
            let mut bad=Collector::default();
            let fields=values.map(number);
            unsafe {
                collect((&mut bad as *mut Collector).cast(),76,
                        fields.as_ptr(),fields.len(),0);
            }
            assert!(bad.malformed,"invalid scalar proof {values:?}");
        }
    }

    #[test]
    fn decodes_unique_native_type_layout_proofs_and_boolean_flags() {
        let number=|v|WireField {
            data:std::ptr::null(),length:0,value:v,tag:wire_tags::UNSIGNED,
        };
        let fields=[number(11),number(1),number(0)];
        let mut state=Collector::default();
        unsafe {
            collect((&mut state as *mut Collector).cast(),75,
                    fields.as_ptr(),fields.len(),0);
        }
        assert!(!state.malformed);
        assert_eq!(state.il.native_type_layout_proofs,vec![
            NativeTypeLayoutProof{owner:TypeId(11),has_layout:true,
                has_fixed_array_bound:false},
        ]);
        unsafe {
            collect((&mut state as *mut Collector).cast(),75,
                    fields.as_ptr(),fields.len(),0);
        }
        assert!(state.malformed,"duplicate native type layout proof");
        for values in [[0,0,0],[8,2,0],[8,0,2]] {
            let mut bad=Collector::default();
            let fields=values.map(number);
            unsafe {
                collect((&mut bad as *mut Collector).cast(),75,
                        fields.as_ptr(),fields.len(),0);
            }
            assert!(bad.malformed,"invalid native type layout proof {values:?}");
        }
    }

    #[test]
    fn decodes_native_record_shape_proof_with_unique_bounded_counts() {
        let number=|value|WireField {
            data:std::ptr::null(),length:0,value,tag:wire_tags::UNSIGNED,
        };
        let mut state=Collector::default();
        let fields=[number(11),number(3),number(1)];
        unsafe {
            collect((&mut state as *mut Collector).cast(),74,
                    fields.as_ptr(),fields.len(),0);
        }
        assert!(!state.malformed);
        assert_eq!(state.il.native_record_shapes,vec![
            NativeRecordShape{owner:TypeId(11),field_count:3,direct_base_count:1},
        ]);
        unsafe {
            collect((&mut state as *mut Collector).cast(),74,
                    fields.as_ptr(),fields.len(),0);
        }
        assert!(state.malformed,"duplicate record shape proof must fail closed");
        for values in [[0,0,0],[7,10001,0],[7,0,10001]] {
            let mut bad=Collector::default();
            let fields=values.map(number);
            unsafe {
                collect((&mut bad as *mut Collector).cast(),74,
                        fields.as_ptr(),fields.len(),0);
            }
            assert!(bad.malformed,"invalid native record shape {values:?}");
        }
    }

    #[test]
    fn decodes_routine_supplement_proof_with_bounded_native_flags() {
        let number=|value|WireField {
            data:std::ptr::null(),length:0,value,tag:2,
        };
        let mut state=Collector::default();
        let proof=[number(7),number(1),number(3)];
        unsafe {
            collect((&mut state as *mut Collector).cast(),71,
                    proof.as_ptr(),proof.len(),0);
        }
        assert!(!state.malformed);
        assert_eq!(state.il.routine_supplements,vec![
            NativeRoutineSupplement {owner:TypeId(7),has_extra_info:true,parameter_count:3},
        ]);
        // Event 71 is unique per native TypeId; duplicate events cannot
        // silently override the first proof.
        unsafe {
            collect((&mut state as *mut Collector).cast(),71,
                    proof.as_ptr(),proof.len(),0);
        }
        assert!(state.malformed);
        let mut state=Collector::default();
        let proof=[number(8),number(0),number(0)];
        unsafe {
            collect((&mut state as *mut Collector).cast(),71,
                    proof.as_ptr(),proof.len(),0);
        }
        assert_eq!(state.il.routine_supplements,vec![
            NativeRoutineSupplement {owner:TypeId(8),has_extra_info:false,parameter_count:0},
        ]);
        assert!(!state.malformed);
        for fields in [
            [number(9),number(2),number(0)],
            [number(0),number(1),number(0)],
            [number(9),number(0),number(1)],
            [number(9),number(1),number(10001)],
        ] {
            let mut bad=Collector::default();
            unsafe {
                collect((&mut bad as *mut Collector).cast(),71,
                        fields.as_ptr(),fields.len(),0);
            }
            assert!(bad.malformed,"native proof needs nonzero owner and boolean flag");
        }
    }

    #[test]
    fn decodes_authoritative_routine_identity_records() {
        let string = |bytes: &[u8]| WireField {
            data: bytes.as_ptr(), length: bytes.len(), value: 0, tag: 1,
        };
        let number = |value| WireField {
            data: std::ptr::null(), length: 0, value, tag: 2,
        };
        let mut state = Collector::default();
        let fields = [number(5), string(b"ns::overloaded")];
        unsafe {
            collect((&mut state as *mut Collector).cast(), 42, fields.as_ptr(), fields.len(), 0);
        }
        assert!(!state.malformed);
        assert_eq!(state.il.routine_identities,
            vec![RoutineIdentity { routine: SymbolId(5), function: "ns::overloaded".into() }]);
    }

    #[test]
    fn decodes_source_positions_and_lifetime_records() {
        let string = |bytes: &[u8]| WireField {
            data: bytes.as_ptr(), length: bytes.len(), value: 0, tag: 1,
        };
        let number = |value| WireField {
            data: std::ptr::null(), length: 0, value, tag: 2,
        };
        let absent = WireField {
            data: std::ptr::null(), length: 0, value: 0, tag: 0,
        };
        let mut state = Collector::default();
        let statement = [string(b"f"), string(b"0"), number(10),
                         number(3), string(b"test.cpp"), number(4)];
        let symbol = [number(2), number(9), number(1),
                      string(b"test.cpp"), number(3)];
        let expression = [string(b"f"), string(b"0"), number(0),
                          string(b"0"), number(10), number(7),
                          string(b"test.cpp"), number(4)];
        let lifetime = [number(1), number(0), absent, number(1)];
        let scope = [number(1), number(0), number(0)];
        let initialization = [string(b"f"), string(b"0"), number(0), number(3)];
        let destruction = [number(1), number(0), number(0), number(2), number(3)];
        unsafe {
            collect((&mut state as *mut Collector).cast(), 35,
                    statement.as_ptr(), statement.len(), 0);
            collect((&mut state as *mut Collector).cast(), 36,
                    symbol.as_ptr(), symbol.len(), 0);
            collect((&mut state as *mut Collector).cast(), 37,
                    expression.as_ptr(), expression.len(), 0);
            collect((&mut state as *mut Collector).cast(), 38,
                    lifetime.as_ptr(), lifetime.len(), 0);
            collect((&mut state as *mut Collector).cast(), 39,
                    scope.as_ptr(), scope.len(), 0);
            collect((&mut state as *mut Collector).cast(), 40,
                    initialization.as_ptr(), initialization.len(), 0);
            collect((&mut state as *mut Collector).cast(), 41,
                    destruction.as_ptr(), destruction.len(), 0);
        }
        assert!(!state.malformed);
        assert_eq!(state.il.statement_sources[0].location.line, 4);
        assert_eq!(state.il.symbol_sources[0].location.column, 1);
        assert_eq!(state.il.expression_sources[0].role, 0);
        assert_eq!(state.il.object_lifetimes[0].parent, None);
        assert_eq!(state.il.scope_lifetimes[0].lifetime, 0);
        assert_eq!(state.il.initialization_lifetimes[0].destructor, Some(SymbolId(3)));
        assert_eq!(state.il.lifetime_destructions[0].variable, Some(SymbolId(2)));
        let bad_location = [string(b"f"), string(b"0"), number(0),
                            number(3), string(b"test.cpp"), number(4)];
        unsafe {
            collect((&mut state as *mut Collector).cast(), 35,
                    bad_location.as_ptr(), bad_location.len(), 0);
        }
        assert!(state.malformed);
    }

    #[test]
    fn m14_standard_float_wire_is_additive_and_strict() {
        let number = |value| WireField {
            data: std::ptr::null(), length:0, value, tag:2,
        };
        let mut state=Collector::default();
        let valid=[number(7),number(32)];
        unsafe { collect((&mut state as *mut Collector).cast(),57,
            valid.as_ptr(),valid.len(),0); }
        assert!(!state.malformed);
        assert_eq!(state.il.c_float_types,vec![CFloatType{id:TypeId(7),bits:32}]);
        let invalid=[number(8),number(80)];
        unsafe { collect((&mut state as *mut Collector).cast(),57,
            invalid.as_ptr(),invalid.len(),0); }
        assert!(state.malformed);
        assert_eq!(state.il.c_float_types.len(),1);
    }
    #[test]
    fn m14_ieee_constant_wire_preserves_full_bits_and_rejects_bad_width() {
        let string = |bytes: &[u8]| WireField {
            data:bytes.as_ptr(),length:bytes.len(),value:0,tag:1,
        };
        let number = |value| WireField {
            data:std::ptr::null(),length:0,value,tag:2,
        };
        let mut state=Collector::default();
        let negative_zero=[
            string(b"fp"),string(b"0.block.1"),string(b"0.1"),
            number(44),number(32),number(0x8000_0000),
        ];
        unsafe {collect((&mut state as *mut Collector).cast(),58,
            negative_zero.as_ptr(),negative_zero.len(),0);}
        assert!(!state.malformed);
        assert_eq!(state.il.float_constants,vec![FloatConstant {
            function:"fp".into(),statement_path:"0.block.1".into(),
            path:"0.1".into(),ty:TypeId(44),width:32,bits:0x8000_0000,
        }]);
        let quiet_nan=[
            string(b"fp"),string(b"0.block.2"),string(b"0"),
            number(45),number(64),number(0x7ff8_1234_abcd_0001),
        ];
        unsafe {collect((&mut state as *mut Collector).cast(),58,
            quiet_nan.as_ptr(),quiet_nan.len(),0);}
        assert!(!state.malformed);
        assert_eq!(state.il.float_constants[1].bits,0x7ff8_1234_abcd_0001);
        let invalid=[
            string(b"fp"),string(b"0.block.3"),string(b"0"),
            number(44),number(32),number(0x1_0000_0000),
        ];
        unsafe {collect((&mut state as *mut Collector).cast(),58,
            invalid.as_ptr(),invalid.len(),0);}
        assert!(state.malformed);
        assert_eq!(state.il.float_constants.len(),2);
    }
    #[test]
    fn m14_constant_fp_initializer_wire_preserves_bits_and_rejects_bad_sizes() {
        let string = |bytes: &[u8]| WireField {
            data:bytes.as_ptr(),length:bytes.len(),value:0,tag:1,
        };
        let number = |value| WireField {
            data:std::ptr::null(),length:0,value,tag:2,
        };
        let mut state=Collector::default();
        let valid=[
            string(b"m14_test"),string(b"0.block.1"),number(17),
            number(23),number(64),number(0x8000_0000_0000_0000),
        ];
        unsafe {collect((&mut state as *mut Collector).cast(),59,
            valid.as_ptr(),valid.len(),0);}
        assert!(!state.malformed);
        assert_eq!(state.il.float_initializers,vec![FloatInitializer {
            function:"m14_test".into(),statement_path:"0.block.1".into(),
            symbol:SymbolId(17),ty:TypeId(23),width:64,
            bits:0x8000_0000_0000_0000,
        }]);
        let invalid=[
            string(b"m14_test"),string(b"0.block.2"),number(17),
            number(23),number(32),number(0x1_0000_0000),
        ];
        unsafe {collect((&mut state as *mut Collector).cast(),59,
            invalid.as_ptr(),invalid.len(),0);}
        assert!(state.malformed);
        assert_eq!(state.il.float_initializers.len(),1);
    }
    #[test]
    fn m14_global_fp_image_wire_preserves_ieee_bits_and_rejects_bad_width() {
        let number=|value| WireField {
            data:std::ptr::null(),length:0,value,tag:2,
        };
        let mut state=Collector::default();
        let valid=[number(17),number(23),number(64),
                   number(0x8000_0000_0000_0000)];
        unsafe {collect((&mut state as *mut Collector).cast(),60,
                        valid.as_ptr(),valid.len(),0);}
        assert!(!state.malformed);
        assert_eq!(state.il.global_float_initializers,vec![
            GlobalFloatInitializer {
                symbol:SymbolId(17),ty:TypeId(23),width:64,
                bits:0x8000_0000_0000_0000,
            }
        ]);
        let bad=[number(18),number(24),number(32),number(0x1_0000_0000)];
        unsafe {collect((&mut state as *mut Collector).cast(),60,
                        bad.as_ptr(),bad.len(),0);}
        assert!(state.malformed);
        assert_eq!(state.il.global_float_initializers.len(),1);
        let mut malformed=Collector::default();
        unsafe {collect((&mut malformed as *mut Collector).cast(),60,
                        valid.as_ptr(),3,0);}
        assert!(malformed.malformed);
        assert!(malformed.il.global_float_initializers.is_empty());
    }

    #[test]
    fn m14_float_expression_images_reject_duplicate_source_path() {
        let text = |v:&'static [u8]| WireField {
            data:v.as_ptr(),length:v.len(),value:0,tag:wire_tags::TEXT,
        };
        let num = |value| WireField {
            data:std::ptr::null(),length:0,value,tag:wire_tags::UNSIGNED,
        };
        let first=[text(b"source"),text(b"0.block.1"),text(b"0.1"),
                   num(44),num(32),num(0x8000_0000)];
        let other=[text(b"source"),text(b"0.block.1"),text(b"0.2"),
                   num(44),num(32),num(7)];
        let duplicate=[text(b"source"),text(b"0.block.1"),text(b"0.1"),
                       num(45),num(64),num(0x7ff8_0000_0000_0001)];
        let mut state=Collector::default();
        for fields in [&first,&other] {
            unsafe {collect((&mut state as *mut Collector).cast(),58,
                            fields.as_ptr(),fields.len(),0);}
            assert!(!state.malformed);
        }
        assert_eq!(state.il.float_constants.len(),2);
        unsafe {collect((&mut state as *mut Collector).cast(),58,
                        duplicate.as_ptr(),duplicate.len(),0);}
        assert!(state.malformed);
        assert_eq!(state.il.float_constants.len(),2);
    }

    #[test]
    fn m14_float_local_images_reject_duplicate_execution_site_and_symbol() {
        let text = |v:&'static [u8]| WireField {
            data:v.as_ptr(),length:v.len(),value:0,tag:wire_tags::TEXT,
        };
        let num = |value| WireField {
            data:std::ptr::null(),length:0,value,tag:wire_tags::UNSIGNED,
        };
        let first=[text(b"local"),text(b"0.block.1"),
                   num(17),num(23),num(64),num(1)];
        let distinct_symbol=[text(b"local"),text(b"0.block.1"),
                             num(18),num(23),num(64),num(1)];
        let duplicate=[text(b"local"),text(b"0.block.1"),
                       num(17),num(23),num(64),num(2)];
        let mut state=Collector::default();
        for fields in [&first,&distinct_symbol] {
            unsafe {collect((&mut state as *mut Collector).cast(),59,
                            fields.as_ptr(),fields.len(),0);}
            assert!(!state.malformed);
        }
        assert_eq!(state.il.float_initializers.len(),2);
        unsafe {collect((&mut state as *mut Collector).cast(),59,
                        duplicate.as_ptr(),duplicate.len(),0);}
        assert!(state.malformed);
        assert_eq!(state.il.float_initializers.len(),2);
    }

    #[test]
    fn m14_float_global_images_reject_duplicated_symbol_identity() {
        let num = |value| WireField {
            data:std::ptr::null(),length:0,value,tag:wire_tags::UNSIGNED,
        };
        let first=[num(17),num(23),num(64),num(0x8000_0000_0000_0000)];
        let second=[num(18),num(23),num(32),num(0x7fc0_0000)];
        let duplicate=[num(17),num(24),num(64),num(0)];
        let mut state=Collector::default();
        for fields in [&first,&second] {
            unsafe {collect((&mut state as *mut Collector).cast(),60,
                            fields.as_ptr(),fields.len(),0);}
            assert!(!state.malformed);
        }
        assert_eq!(state.il.global_float_initializers.len(),2);
        unsafe {collect((&mut state as *mut Collector).cast(),60,
                        duplicate.as_ptr(),duplicate.len(),0);}
        assert!(state.malformed);
        assert_eq!(state.il.global_float_initializers.len(),2);
    }

    #[test]
    fn m11_vla_dimension_wire_requires_valid_runtime_expression_shape() {
        let string = |bytes: &[u8]| WireField {
            data: bytes.as_ptr(), length: bytes.len(), value: 0, tag: 1,
        };
        let number = |value| WireField {
            data: std::ptr::null(), length: 0, value, tag: 2,
        };
        let mut state = Collector::default();
        let valid=[string(b"f"),string(b"0.block.1"),
                   number(42),number(1),number(0)];
        unsafe {
            collect((&mut state as *mut Collector).cast(),56,
                    valid.as_ptr(),valid.len(),0);
        }
        assert!(!state.malformed);
        assert_eq!(state.il.vla_dimensions.len(),1);
        assert_eq!(state.il.vla_dimensions[0].array_type,TypeId(42));
        assert!(state.il.vla_dimensions[0].has_expression);
        assert!(!state.il.vla_dimensions[0].copied);
        let invalid=[string(b"f"),string(b"0.block.2"),
                     number(42),number(1),number(1)];
        unsafe {
            collect((&mut state as *mut Collector).cast(),56,
                    invalid.as_ptr(),invalid.len(),0);
        }
        assert!(state.malformed);
        assert_eq!(state.il.vla_dimensions.len(),1);
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
        let mut state = Collector::default();
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
        let mut state = Collector::default();
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
        let mut state = Collector::default();
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
        let mut state = Collector::default();
        unsafe { collect((&mut state as *mut Collector).cast(), 1, fields.as_ptr(), 1, 0); }
        assert!(state.malformed);
    }

    #[test]
    fn malformed_callback_is_reported() {
        let mut state = Collector::default();
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


#[cfg(test)]
mod wire_field_regression_tests {
    use super::WireField;

    #[test]
    fn string_fields_require_utf8_and_string_tag() {
        let bytes = b"hello";
        let field = WireField {
            data: bytes.as_ptr(),
            length: bytes.len(),
            value: 0,
            tag: 1,
        };
        assert_eq!(field.string().as_deref(), Some("hello"));
        assert_eq!(WireField { tag: 2, ..field }.string(), None);
        assert_eq!(WireField { data: std::ptr::null(), ..field }.string(), None);
        let invalid = [0xff];
        assert_eq!(WireField {
            data: invalid.as_ptr(),
            length: invalid.len(),
            ..field
        }.string(), None);
    }

    #[test]
    fn numeric_fields_check_tags_and_widths() {
        let field = WireField {
            data: std::ptr::null(),
            length: 0,
            value: u64::MAX,
            tag: 2,
        };
        assert_eq!(field.unsigned(), Some(u64::MAX));
        assert_eq!(field.u32(), None);
        assert_eq!(WireField { value: u32::MAX as u64, ..field }.u32(), Some(u32::MAX));
        assert_eq!(WireField { tag: 3, ..field }.signed(), Some(-1));
        assert_eq!(WireField { tag: 3, ..field }.unsigned(), None);
        assert!(!WireField { tag: 0, ..field }.present());
    }
}

#[cfg(test)]
mod wire_tag_audit_tests {
    use super::*;
    #[test]
    fn wire_tag_values_and_decoding_are_stable() {
        assert_eq!([wire_tags::ABSENT,wire_tags::TEXT,wire_tags::UNSIGNED,wire_tags::SIGNED],[0,1,2,3]);
        let number=WireField{data:std::ptr::null(),length:0,value:42,tag:wire_tags::UNSIGNED};
        assert_eq!(number.unsigned(),Some(42));
        assert_eq!(number.signed(),None);
        let signed=WireField{tag:wire_tags::SIGNED,value:(-7i64) as u64,..number};
        assert_eq!(signed.signed(),Some(-7));
        let absent=WireField{tag:wire_tags::ABSENT,..number};
        assert!(!absent.present());
        let bytes=b"hello";
        let text=WireField{data:bytes.as_ptr(),length:bytes.len(),value:0,tag:wire_tags::TEXT};
        assert_eq!(text.string().as_deref(),Some("hello"));
        assert_eq!(WireField{data:std::ptr::null(),..text}.string(),None);
    }
}

#[cfg(test)]
mod audit_wire_record_count_tests {
    use super::*;

    #[test]
    fn invalid_link_names_and_string_literals_fail_closed() {
        let number = |value: u64| WireField {
            data:std::ptr::null(),length:0,value,tag:wire_tags::UNSIGNED,
        };
        let text = |bytes: &'static [u8]| WireField {
            data:bytes.as_ptr(),length:bytes.len(),value:0,tag:wire_tags::TEXT,
        };
        let mut state=Collector::default();
        let symbol=[number(42),number(1),text(b"f")];
        let name=[number(42),text(b"_Z1fv"),number(1)];
        let string=[text(b"f"),text(b"0"),text(b"0"),number(1),text(b"6100")];
        let bad_string=[text(b"f"),text(b"0"),text(b"0"),number(1),text(b"abc")];
        unsafe {
            let userdata=(&mut state as *mut Collector).cast();
            collect(userdata,13,symbol.as_ptr(),symbol.len(),0);
            collect(userdata,61,name.as_ptr(),name.len(),0);
            collect(userdata,62,string.as_ptr(),string.len(),0);
        }
        assert!(!state.malformed);
        assert_eq!(state.il.symbol_link_names.len(),1);
        assert_eq!(state.il.string_literals.len(),1);

        // Rejected duplicate linker-name records must not be ignored.
        unsafe {
            let userdata=(&mut state as *mut Collector).cast();
            collect(userdata,61,name.as_ptr(),name.len(),0);
        }
        assert!(state.malformed);
        state.malformed=false;

        // Odd-length byte images are rejected by the decoder. The callback
        // must also report malformed input, not simply drop the event.
        unsafe {
            let userdata=(&mut state as *mut Collector).cast();
            collect(userdata,62,bad_string.as_ptr(),bad_string.len(),0);
        }
        assert!(state.malformed);
    }
}

#[cfg(test)]
mod audit_wire_schema_consistency_tests {
    use super::*;

    #[test]
    fn wire_field_arity_and_collection_accounting_cover_the_same_kinds() {
        let il=Il::default();
        for kind in 0..=256 {
            assert_eq!(
                expected_wire_field_count(kind).is_some(),
                callback_record_count(&il,kind).is_some(),
                "wire record kind {kind} must be decoded and accounted together"
            );
            if (1..=64).contains(&kind) {
                assert!(expected_wire_field_count(kind).is_some(),
                        "known wire record kind {kind} is missing");
            }
        }
    }
}

#[cfg(test)]
mod audit_wire_preflight_tests {
    use super::*;

    fn callback_rejects(kind:u32, ptr:*const WireField, count:usize) {
        let mut state=Collector::default();
        unsafe {
            collect((&mut state as *mut Collector).cast(),kind,ptr,count,0);
        }
        assert!(state.malformed, "event kind {kind} length {count} must be rejected");
        assert!(!state.panicked);
        assert!(state.il.entities.is_empty());
    }

    #[test]
    fn rejects_unknown_records_and_missing_field_arrays() {
        callback_rejects(0,std::ptr::null(),0);
        callback_rejects(65,std::ptr::null(),0);
        callback_rejects(u32::MAX,std::ptr::null(),0);
        callback_rejects(13,std::ptr::null(),3);
    }

    #[test]
    fn rejects_out_of_range_native_type_flags_and_signature_booleans() {
        let number = |value:u64| WireField {
            data:std::ptr::null(),length:0,value,
            tag:wire_tags::UNSIGNED,
        };
        let absent = WireField {
            data:std::ptr::null(),length:0,value:0,
            tag:wire_tags::ABSENT,
        };
        // Valid reference flags and prototype booleans still decode.
        for flags in 0..=3 {
            let fields=[number(1),number(flags),absent];
            let mut state=Collector::default();
            unsafe {collect((&mut state as *mut Collector).cast(),
                11,fields.as_ptr(),fields.len(),0);}
            assert!(!state.malformed);
            assert_eq!(state.il.type_details.len(),1);
        }
        for (prototype,variadic) in [(0,0),(1,0),(1,1)] {
            let fields=[number(1),number(prototype),number(variadic)];
            let mut state=Collector::default();
            unsafe {collect((&mut state as *mut Collector).cast(),
                12,fields.as_ptr(),fields.len(),0);}
            assert!(!state.malformed);
            assert_eq!(state.il.function_signatures.len(),1);
        }
        for flags in [4,u64::MAX] {
            let fields=[number(1),number(flags),absent];
            callback_rejects(11,fields.as_ptr(),fields.len());
        }
        for (owner,prototype,variadic) in [
            (0,1,0),(1,2,0),(1,1,2),(1,u64::MAX,0),
        ] {
            let fields=[number(owner),number(prototype),number(variadic)];
            callback_rejects(12,fields.as_ptr(),fields.len());
        }
        for (explicit,uninitialized) in [(2,0),(1,2),(u64::MAX,0)] {
            let fields=[number(1),number(0),number(explicit),number(uninitialized)];
            callback_rejects(14,fields.as_ptr(),fields.len());
        }
        let name=WireField {
            data:b"f".as_ptr(),length:1,value:0,tag:wire_tags::TEXT,
        };
        let mut state=Collector::default();
        unsafe {collect((&mut state as *mut Collector).cast(),
            1,&name,1,1);}
        assert_eq!(state.il.entities,
            vec![Entity::Routine{name:"f".into(),has_definition:true}]);
        for (kind,aux) in [(1,2),(2,1),(3,1),(4,1)] {
            let mut state=Collector::default();
            unsafe {collect((&mut state as *mut Collector).cast(),
                kind,&name,1,aux);}
            assert!(state.malformed,"entity event {kind} aux {aux}");
        }
    }

    #[test]
    fn rejects_incorrect_arity_before_reading_native_memory() {
        let field=WireField {
            data:std::ptr::null(),length:0,value:1,
            tag:wire_tags::UNSIGNED,
        };
        // Deliberately pass a pointer to only one actual field. Both
        // wrong lengths must be rejected without forming a native slice.
        callback_rejects(13,&field,1);
        callback_rejects(13,&field,usize::MAX);
        callback_rejects(13,&field,0);
    }
}

#[cfg(test)]
mod audit_native_aggregate_image_cap_tests {
    use super::*;

    #[test]
    fn hex_images_accept_exact_limit_and_reject_oversized_or_malformed_data() {
        let good="7f".repeat(MAX_NATIVE_AGGREGATE_IMAGE_BYTES);
        let bytes=decode_aggregate_hex(&good).expect("4 KiB native image");
        assert_eq!(bytes.len(),MAX_NATIVE_AGGREGATE_IMAGE_BYTES);
        assert_eq!(bytes[0],0x7f);
        assert_eq!(bytes[MAX_NATIVE_AGGREGATE_IMAGE_BYTES-1],0x7f);
        assert!(decode_aggregate_hex(&format!("{good}7f")).is_none());
        assert!(decode_aggregate_hex("").is_none());
        assert!(decode_aggregate_hex("0").is_none());
        assert!(decode_aggregate_hex("gg").is_none());
        assert!(decode_aggregate_hex("AA").is_none());
    }
}

#[cfg(test)]
mod audit_unique_global_storage_wire_tests {
    use super::*;

    fn field(value:u64)->WireField {
        WireField {
            data:std::ptr::null(),length:0,value,tag:wire_tags::UNSIGNED,
        }
    }
    fn send(state:&mut Collector, values:[u64;5]) {
        let fields=values.map(field);
        unsafe {
            collect((state as *mut Collector).cast(),47,fields.as_ptr(),fields.len(),0);
        }
    }
    #[test]
    fn distinct_global_symbols_are_accepted() {
        let mut state=Collector::default();
        send(&mut state,[12,3,1,0,0]);
        send(&mut state,[13,1,0,0,0]);
        assert!(!state.malformed);
        assert!(!state.panicked);
        assert_eq!(state.il.global_variables.len(),2);
        assert!(state.il.global_variables[0].explicit_initializer);
        assert!(!state.il.global_variables[1].explicit_initializer);
    }
    #[test]
    fn duplicate_or_conflicting_global_metadata_fails_closed() {
        for duplicate in [[12,3,1,0,0],[12,1,0,0,0],[12,2,1,1,0]] {
            let mut state=Collector::default();
            send(&mut state,[12,3,1,0,0]);
            assert!(!state.malformed);
            send(&mut state,duplicate);
            assert!(state.malformed,"duplicate metadata {duplicate:?} accepted");
            assert!(!state.panicked);
            assert_eq!(state.il.global_variables.len(),1);
            send(&mut state,[13,3,0,0,0]);
            // Once malformed, no further records may be added.
            assert_eq!(state.il.global_variables.len(),1);
        }
    }
}

#[cfg(test)]
mod audit_unique_native_type_identity_tests {
    use super::*;

    fn field(v:u64)->WireField {
        WireField {data:std::ptr::null(),length:0,value:v,tag:wire_tags::UNSIGNED}
    }
    fn send<const N:usize>(state:&mut Collector,kind:u32,values:[u64;N]) {
        let fields=values.map(field);
        unsafe {collect((state as *mut Collector).cast(),kind,fields.as_ptr(),N,0);}
    }
    #[test]
    fn different_native_type_and_symbol_identities_are_valid() {
        let mut state=Collector::default();
        send(&mut state,7,[1,1,0,0]);
        send(&mut state,7,[2,1,0,0]);
        send(&mut state,49,[1,8,8]);
        send(&mut state,49,[2,4,4]);
        send(&mut state,25,[21,1]);
        send(&mut state,25,[22,2]);
        assert!(!state.malformed);
        assert_eq!(state.il.types.len(),2);
        assert_eq!(state.il.type_object_sizes.len(),2);
        assert_eq!(state.il.symbol_types.len(),2);
    }
    #[test]
    fn duplicate_type_definitions_are_malformed() {
        for duplicate in [[1,1,0,0],[1,5,1,2]] {
            let mut state=Collector::default();
            send(&mut state,7,[1,1,0,0]);
            send(&mut state,7,duplicate);
            assert!(state.malformed);
            assert!(!state.panicked);
            assert_eq!(state.il.types.len(),1);
        }
    }
    #[test]
    fn duplicate_type_layouts_are_malformed() {
        for duplicate in [[1,8,8],[1,16,8]] {
            let mut state=Collector::default();
            send(&mut state,49,[1,8,8]);
            send(&mut state,49,duplicate);
            assert!(state.malformed);
            assert!(!state.panicked);
            assert_eq!(state.il.type_object_sizes.len(),1);
        }
    }
    #[test]
    fn duplicate_symbol_type_bindings_are_malformed() {
        for duplicate in [[21,1],[21,2]] {
            let mut state=Collector::default();
            send(&mut state,25,[21,1]);
            send(&mut state,25,duplicate);
            assert!(state.malformed);
            assert!(!state.panicked);
            assert_eq!(state.il.symbol_types.len(),1);
        }
    }
}

#[cfg(test)]
mod audit_wire_symbol_identity_tests {
    use super::*;

    fn send(state:&mut Collector,id:u64,kind:u64,name:&'static [u8]) {
        let fields=[
            WireField {data:std::ptr::null(),length:0,value:id,tag:wire_tags::UNSIGNED},
            WireField {data:std::ptr::null(),length:0,value:kind,tag:wire_tags::UNSIGNED},
            WireField {data:name.as_ptr(),length:name.len(),value:0,tag:wire_tags::TEXT},
        ];
        unsafe {collect((state as *mut Collector).cast(),13,fields.as_ptr(),3,0);}
    }

    #[test]
    fn accepts_distinct_native_symbol_identities() {
        let mut state=Collector::default();
        for (id,kind) in [(1,1),(2,2),(3,3),(4,4)] {
            send(&mut state,id,kind,b"src");
        }
        assert!(!state.malformed);
        assert_eq!(state.il.symbols.len(),4);
        assert_eq!(state.seen.symbols.len(),4);
        assert_eq!(state.seen.routine_symbols.len(),1);
    }

    #[test]
    fn zero_unknown_and_duplicate_symbol_ids_are_malformed() {
        for (id,kind) in [(0,1),(8,0),(8,5)] {
            let mut state=Collector::default();
            send(&mut state,id,kind,b"f");
            assert!(state.malformed,"invalid id={id} kind={kind} accepted");
            assert!(state.il.symbols.is_empty());
        }
        for kind in [1,2,3,4] {
            let mut state=Collector::default();
            send(&mut state,8,1,b"f");
            send(&mut state,8,kind,b"duplicate");
            assert!(state.malformed,"duplicate symbol with kind={kind} accepted");
            assert_eq!(state.il.symbols.len(),1);
            assert_eq!(state.seen.symbols.len(),1);
        }
    }
}

#[cfg(test)]
mod audit_wire_identity_index_tests {
    use super::*;

    fn number(v: u64) -> WireField {
        WireField { data:std::ptr::null(),length:0,value:v,tag:wire_tags::UNSIGNED }
    }
    fn send<const N: usize>(state:&mut Collector,kind:u32,values:[u64;N]) {
        let fields=values.map(number);
        unsafe {
            collect((state as *mut Collector).cast(),kind,fields.as_ptr(),fields.len(),0);
        }
    }

    fn text(value:&'static [u8])->WireField {
        WireField {data:value.as_ptr(),length:value.len(),value:0,tag:wire_tags::TEXT}
    }
    fn send_routine(state:&mut Collector,id:u64) {
        let fields=[number(id),number(1),text(b"f")];
        unsafe {collect((state as *mut Collector).cast(),13,fields.as_ptr(),3,0);}
    }
    fn send_link_name(state:&mut Collector,id:u64) {
        let fields=[number(id),text(b"_Z1fv"),number(1)];
        unsafe {collect((state as *mut Collector).cast(),61,fields.as_ptr(),3,0);}
    }
    fn send_global_materialization(state:&mut Collector, symbol:u64, flags:u64,
                                   section:Option<&'static [u8]>, visibility:Option<u64>) {
        let absent=WireField {
            data:std::ptr::null(),length:0,value:0,tag:wire_tags::ABSENT,
        };
        let fields=[
            number(symbol),number(flags),
            section.map(text).unwrap_or(absent),
            visibility.map(number).unwrap_or(absent),
        ];
        unsafe { collect((state as *mut Collector).cast(),100,
                          fields.as_ptr(),fields.len(),0); }
    }
    #[test]
    fn native_global_attribute_flags_preserve_weak_alias_and_section_presence() {
        // The existing mandatory per-global record 100 owns these facts.
        // Neither linker binding nor section text is inferred from linkage=3.
        for (flags, weak, weakref, alias, section) in [
            (0_u64, false, false, false, false),
            (128, true, false, false, false),
            (128 | 256, true, true, false, false),
            (512, false, false, true, false),
            (1024, false, false, false, true),
            (128 | 1024, true, false, false, true),
        ] {
            let mut state = Collector::default();
            send(&mut state, 47, [2, 3, 1, 0, 0]);
            send_global_materialization(&mut state,2,flags,
                if flags&1024!=0 {Some(b".data.edg_probe")} else {None},Some(0));
            assert!(!state.malformed, "unexpected flag rejection: {flags}");
            assert_eq!(state.il.global_materializations.len(), 1);
            let m = &state.il.global_materializations[0];
            assert_eq!((m.is_weak, m.is_weakref, m.is_gnu_alias,
                        m.has_explicit_section), (weak, weakref, alias, section));
            assert_eq!(m.section.as_deref(),
                       section.then_some(".data.edg_probe"));
            assert_eq!(m.elf_visibility,Some(0));
            assert!(validate_native_global_materialization_coverage(&state.il).is_ok());
            send_global_materialization(&mut state,2,flags,
                if flags&1024!=0 {Some(b".data.edg_probe")} else {None},Some(0));
            assert!(state.malformed, "duplicate attribute provenance admitted");
        }
        for flags in [2048_u64, 4095, u64::MAX] {
            let mut state = Collector::default();
            send(&mut state, 47, [2, 3, 1, 0, 0]);
            send_global_materialization(&mut state,2,flags,
                if flags&1024!=0 {Some(b".data.edg_probe")} else {None},Some(0));
            assert!(state.malformed, "unknown source attribute bits admitted");
        }
        // Presence must agree with the actual section string, and visibility
        // must be a native EDG enum, never a silently invented default.
        for (flags, section, visibility) in [
            (0_u64, Some(b".data.edg_probe".as_slice()), Some(3_u64)),
            (1024, None, Some(3)),
            (1024, Some(b"".as_slice()), Some(3)),
            (0, None, Some(5)),
        ] {
            let mut state=Collector::default();
            send(&mut state,47,[2,3,1,0,0]);
            send_global_materialization(&mut state,2,flags,section,visibility);
            assert!(state.malformed,"bad global section/visibility admitted: {flags}");
        }
        {
            let mut state=Collector::default();
            send(&mut state,47,[2,3,1,0,0]);
            send_global_materialization(&mut state,2,0,None,None);
            assert!(!state.malformed);
            assert_eq!(state.il.global_materializations[0].elf_visibility,None);
        }
        let mut missing = Collector::default();
        send(&mut missing, 47, [2, 3, 1, 0, 0]);
        assert!(validate_native_global_materialization_coverage(&missing.il).is_err());
    }

    #[test]
    fn accepts_only_source_owned_global_data_link_names() {
        let mut state=Collector::default();
        let fields=[number(17),number(2),text(b"std::cout")];
        unsafe {collect((&mut state as *mut Collector).cast(),13,fields.as_ptr(),3,0);}
        let linked=[number(17),text(b"_ZSt4cout"),number(1)];
        unsafe {collect((&mut state as *mut Collector).cast(),61,linked.as_ptr(),3,0);}
        assert!(!state.malformed);
        assert_eq!(state.il.symbol_link_names,vec![SymbolLinkName{
            symbol:SymbolId(17),link_name:"_ZSt4cout".into(),edg_mangled:true
        }]);
        unsafe {collect((&mut state as *mut Collector).cast(),61,linked.as_ptr(),3,0);}
        assert!(state.malformed,"duplicate global data linker names are ambiguous");

        let mut state=Collector::default();
        let fields=[number(18),number(3),text(b"member")];
        unsafe {collect((&mut state as *mut Collector).cast(),13,fields.as_ptr(),3,0);}
        let linked=[number(18),text(b"_ZN6memberE"),number(1)];
        unsafe {collect((&mut state as *mut Collector).cast(),61,linked.as_ptr(),3,0);}
        assert!(state.malformed,"record fields cannot masquerade as linker data");
    }

    #[test]
    fn unknown_or_duplicate_routine_link_names_fail_closed() {
        let mut state=Collector::default();
        send_link_name(&mut state,1);
        assert!(state.malformed);
        assert!(state.il.symbol_link_names.is_empty());

        let mut state=Collector::default();
        send_routine(&mut state,1);
        send_link_name(&mut state,1);
        assert!(!state.malformed);
        send_link_name(&mut state,1);
        assert!(state.malformed);
        assert_eq!(state.il.symbol_link_names.len(),1);
    }

    #[test]
    fn indexed_identity_decoding_scales_to_many_distinct_types() {
        let mut state=Collector::default();
        // Emission order is preserved and all six identity sets grow only
        // once per native record, instead of searching every prior Vec.
        for id in 1..=8192_u64 {
            send(&mut state,7,[id,1,0,0]);
            send(&mut state,49,[id,8,8]);
            send(&mut state,25,[id, id]);
            send(&mut state,47,[id,3,0,0,0]);
            send(&mut state,63,[id,1]);
            send(&mut state,65,[id,1]);
            send_routine(&mut state,id+8192);
            send_link_name(&mut state,id+8192);
        }
        assert!(!state.malformed);
        assert!(!state.panicked);
        assert_eq!(state.il.types.len(),8192);
        assert_eq!(state.il.type_object_sizes.len(),8192);
        assert_eq!(state.il.symbol_types.len(),8192);
        assert_eq!(state.il.global_variables.len(),8192);
        assert_eq!(state.il.cxx_record_admissions.len(),8192);
        assert_eq!(state.il.cxx_scalar_lifetime_admissions.len(),8192);
        assert_eq!(state.il.symbol_link_names.len(),8192);
        assert_eq!(state.seen.types.len(),8192);
        assert_eq!(state.seen.layouts.len(),8192);
        assert_eq!(state.seen.symbol_types.len(),8192);
        assert_eq!(state.seen.globals.len(),8192);
        assert_eq!(state.seen.cxx_records.len(),8192);
        assert_eq!(state.seen.cxx_lifetimes.len(),8192);
        assert_eq!(state.seen.routine_symbols.len(),8192);
        assert_eq!(state.seen.source_link_names.len(),8192);
        assert_eq!(state.il.types[8191].id,TypeId(8192));
        // A duplicate after a large valid stream must still fail closed.
        send(&mut state,25,[8192,1]);
        assert!(state.malformed);
        assert!(!state.panicked);
        assert_eq!(state.il.symbol_types.len(),8192);
    }
}


#[cfg(test)]
mod audit_typed_wire_field_tests {
    use super::*;

    fn number(value: u64) -> WireField {
        WireField {data:std::ptr::null(),length:0,value,tag:wire_tags::UNSIGNED}
    }
    fn signed(value: i64) -> WireField {
        WireField {data:std::ptr::null(),length:0,value:value as u64,tag:wire_tags::SIGNED}
    }
    fn text(value:&'static [u8]) -> WireField {
        WireField {data:value.as_ptr(),length:value.len(),value:0,tag:wire_tags::TEXT}
    }
    fn send(state:&mut Collector,kind:u32,fields:&[WireField]) {
        unsafe {collect((state as *mut Collector).cast(),kind,fields.as_ptr(),fields.len(),0);}
    }

    #[test]
    fn every_typed_record_rejects_absent_fields() {
        // All currently typed records have mandatory fields. The generated
        // validation must reject a correctly sized event with invalid tags
        // *before* any decoder-specific logic can accept a default value.
        for kind in [7,13,15,25,43,47,49,51,52,61,63,64,65] {
            let len=expected_wire_field_count(kind).unwrap();
            assert_eq!(expected_wire_field_tag_masks(kind).unwrap().len(),len);
            let invalid=vec![WireField {
                data:std::ptr::null(),length:0,value:0,tag:wire_tags::ABSENT,
            };len];
            let mut state=Collector::default();
            send(&mut state,kind,&invalid);
            assert!(state.malformed,"record {kind} accepted absent fields");
            assert!(!state.panicked);
        }
    }

    #[test]
    fn rejects_wrong_numeric_signedness_and_text_tags() {
        let mut state=Collector::default();
        send(&mut state,15,&[number(1),number(42)]);
        assert!(state.malformed);
        assert!(state.il.initializer_integers.is_empty());

        let mut state=Collector::default();
        send(&mut state,52,&[number(1),number(0),number(2),number(0)]);
        assert!(state.malformed);
        assert!(state.il.global_aggregate_relocations.is_empty());

        let mut state=Collector::default();
        send(&mut state,13,&[number(1),number(1),number(99)]);
        assert!(state.malformed);
        assert!(state.il.symbols.is_empty());
    }

    #[test]
    fn accepts_correctly_typed_numeric_and_text_records() {
        let mut state=Collector::default();
        send(&mut state,13,&[number(1),number(2),text(b"global")]);
        send(&mut state,15,&[number(1),signed(-9)]);
        send(&mut state,47,&[number(1),number(3),number(1),number(0),number(0)]);
        assert!(!state.malformed);
        assert!(!state.panicked);
        assert_eq!(state.il.symbols.len(),1);
        assert_eq!(state.il.initializer_integers[0].value,-9);
        assert_eq!(state.il.global_variables.len(),1);
    }
}


#[cfg(test)]
mod audit_complete_wire_field_tags_tests {
    use super::*;

    fn wire_field(tag:u32)->WireField {
        let sample=b"x";
        WireField {
            data:if tag==wire_tags::TEXT {sample.as_ptr()} else {std::ptr::null()},
            length:if tag==wire_tags::TEXT {sample.len()} else {0},
            value:1,tag,
        }
    }

    #[test]
    fn every_record_field_rejects_an_incorrect_but_defined_tag() {
        let mut total=0;
        let mut optional=0;
        for kind in 1..=106_u32 {
            let expected=expected_wire_field_tag_masks(kind)
                .expect("all native wire record kinds have typed fields");
            assert_eq!(Some(expected.len()),expected_wire_field_count(kind));
            for field_index in 0..expected.len() {
                let allowed=expected[field_index];
                assert!(allowed!=0 && allowed & !0x0f==0);
                if allowed & 1!=0 {optional+=1;}
                let baseline:Vec<_>=expected.iter().map(|mask|{
                    wire_field(mask.trailing_zeros())
                }).collect();
                let bad_tag=(0..=wire_tags::MAX).find(|tag|{
                    allowed & (1u8 << tag)==0
                }).expect("must have a disallowed defined tag");
                let mut bad=baseline;
                bad[field_index]=wire_field(bad_tag);
                let mut state=Collector::default();
                unsafe {
                    collect((&mut state as *mut Collector).cast(),
                            kind,bad.as_ptr(),bad.len(),0);
                }
                assert!(state.malformed,
                    "record {kind} field {field_index} accepted illegal tag {bad_tag}");
                assert!(!state.panicked);
                total+=1;
            }
        }
        // Compare against the generated schema rather than a manually
        // maintained total that becomes stale when records gain fields.
        let schema_fields:usize=(1..=106_u32)
            .map(|kind|expected_wire_field_count(kind).unwrap()).sum();
        assert_eq!(expected_wire_field_count(100),Some(4));
        assert_eq!(total,schema_fields);
        assert!(optional>0);
        assert!(expected_wire_field_tag_masks(0).is_none());
        assert!(expected_wire_field_tag_masks(106).is_none());
    }

    #[test]
    fn optional_field_accepts_absent_or_present_text() {
        let mut state=Collector::default();
        let sample=b"f";
        let path=b"0";
        let text=|s:&'static [u8]|WireField{
            data:s.as_ptr(),length:s.len(),value:0,tag:wire_tags::TEXT
        };
        let absent=WireField{
            data:std::ptr::null(),length:0,value:0,tag:wire_tags::ABSENT
        };
        let event=[text(sample),text(path),absent];
        unsafe {
            collect((&mut state as *mut Collector).cast(),
                    5,event.as_ptr(),event.len(),0);
        }
        assert!(!state.malformed);
        assert_eq!(state.il.statements.len(),1);

        let event=[text(sample),text(b"1"),text(path)];
        unsafe {
            collect((&mut state as *mut Collector).cast(),
                    5,event.as_ptr(),event.len(),0);
        }
        assert!(!state.malformed);
        assert_eq!(state.il.statements.len(),2);
    }
}

#[cfg(test)]
mod audit_wire_text_length_tests {
    use super::*;

    #[test]
    fn oversized_native_text_length_is_rejected_before_slice_creation() {
        let one = b"x";
        let bad = WireField {
            data:one.as_ptr(),
            length:usize::MAX,
            value:0,
            tag:wire_tags::TEXT,
        };
        // The pointer designates only one byte: dereferencing the claimed
        // length would be invalid, even for UTF-8 validation.
        assert!(bad.string().is_none());
        let bad = WireField {
            length:MAX_WIRE_TEXT_BYTES + 1, ..bad
        };
        assert!(bad.string().is_none());
    }

    #[test]
    fn text_length_limit_is_enforced_for_native_callbacks() {
        let one=b"x";
        let field=WireField {
            data:one.as_ptr(),
            length:MAX_WIRE_TEXT_BYTES + 1,
            value:0,tag:wire_tags::TEXT,
        };
        let mut collector=Collector::default();
        unsafe {
            collect((&mut collector as *mut Collector).cast(),
                    4,&field,1,0);
        }
        assert!(collector.malformed);
        assert!(!collector.panicked);
        assert!(collector.il.entities.is_empty());

        let ordinary=WireField {length:1,..field};
        let mut valid=Collector::default();
        unsafe {
            collect((&mut valid as *mut Collector).cast(),
                    4,&ordinary,1,0);
        }
        assert!(!valid.malformed);
        assert!(matches!(valid.il.entities.as_slice(),
            [Entity::Namespace { name }] if name=="x"));
    }
}

#[cfg(test)]
mod audit_scalar_type_identity_tests {
    use super::*;

    fn number(value:u64) -> WireField {
        WireField {
            data:std::ptr::null(),length:0,value,tag:wire_tags::UNSIGNED,
        }
    }

    fn send(state:&mut Collector,kind:u32,fields:&[WireField]) {
        unsafe {
            collect((state as *mut Collector).cast(),
                    kind,fields.as_ptr(),fields.len(),0);
        }
    }

    #[test]
    fn rejects_duplicate_or_conflicting_c_integer_type_metadata() {
        let mut state=Collector::default();
        send(&mut state,46,&[number(7),number(32),number(1),number(0)]);
        assert!(!state.malformed);
        assert_eq!(state.il.c_integer_types.len(),1);
        send(&mut state,46,&[number(7),number(64),number(0),number(0)]);
        assert!(state.malformed);
        assert_eq!(state.il.c_integer_types.len(),1);
    }

    #[test]
    fn rejects_duplicate_float_width_and_signed_i64_proofs() {
        let mut state=Collector::default();
        send(&mut state,57,&[number(8),number(32)]);
        assert!(!state.malformed);
        send(&mut state,57,&[number(8),number(64)]);
        assert!(state.malformed);
        assert_eq!(state.il.c_float_types.len(),1);

        let mut state=Collector::default();
        send(&mut state,43,&[number(9)]);
        assert!(!state.malformed);
        send(&mut state,43,&[number(9)]);
        assert!(state.malformed);
        assert_eq!(state.il.signed_i64_types.len(),1);
    }

    #[test]
    fn distinct_scalar_type_proofs_remain_accepted() {
        let mut state=Collector::default();
        send(&mut state,46,&[number(7),number(32),number(1),number(0)]);
        send(&mut state,46,&[number(8),number(64),number(0),number(0)]);
        send(&mut state,57,&[number(9),number(32)]);
        send(&mut state,57,&[number(10),number(64)]);
        send(&mut state,43,&[number(8)]);
        assert!(!state.malformed);
        assert_eq!(state.il.c_integer_types.len(),2);
        assert_eq!(state.il.c_float_types.len(),2);
        assert_eq!(state.il.signed_i64_types.len(),1);
    }
}


#[cfg(test)]
mod file_scope_lowered_startup_wire_tests {
    use super::*;
    fn send(state: &mut Collector, routine: u32, priority: u32) {
        let fields=[routine,priority].map(|value| WireField {
            data: std::ptr::null(), length: 0, value:u64::from(value),
            tag:wire_tags::UNSIGNED,
        });
        unsafe { collect((state as *mut Collector).cast(),73,
                         fields.as_ptr(),fields.len(),0); }
    }
    #[test]
    fn native_startup_routines_retain_priorities_without_inventing_elf_order() {
        let mut state=Collector::default();
        send(&mut state,42,0);
        send(&mut state,43,200);
        assert!(!state.malformed);
        assert_eq!(state.il.file_scope_lowered_startup_routines,
            vec![FileScopeLoweredStartupRoutine{routine:SymbolId(42),priority:0},
                 FileScopeLoweredStartupRoutine{routine:SymbolId(43),priority:200}]);
        assert!(state.il.file_scope_runtime_links.is_empty());
        send(&mut state,43,201);
        assert!(state.malformed,"duplicate startup definition accepted");
    }
    #[test]
    fn many_startup_routine_witnesses_keep_stable_order_and_reject_duplicates() {
        let mut state=Collector::default();
        for routine in 1..=4096 {
            send(&mut state,routine,routine % 256);
        }
        assert!(!state.malformed);
        assert_eq!(state.il.file_scope_lowered_startup_routines.len(),4096);
        assert_eq!(state.seen.file_scope_lowered_startup_routines.len(),4096);
        assert_eq!(state.il.file_scope_lowered_startup_routines[0].routine,SymbolId(1));
        assert_eq!(state.il.file_scope_lowered_startup_routines[4095].routine,SymbolId(4096));
        send(&mut state,2048,9);
        assert!(state.malformed,"duplicate within a large roster was accepted");
        assert_eq!(state.il.file_scope_lowered_startup_routines.len(),4096);
    }
    #[test]
    fn rejects_missing_startup_routine_identity() {
        let mut state=Collector::default();
        send(&mut state,0,0);
        assert!(state.malformed);
    }
}

#[cfg(test)]
mod file_scope_runtime_link_wire_tests {
    use super::*;
    fn send(state: &mut Collector, variable: u32, routine: u32) {
        let fields = [variable, routine].map(|value| WireField {
            data: std::ptr::null(), length: 0, value: u64::from(value),
            tag: wire_tags::UNSIGNED,
        });
        unsafe { collect((state as *mut Collector).cast(), 72,
                         fields.as_ptr(), fields.len(), 0); }
    }
    #[test]
    fn retains_source_owned_startup_pointer_but_not_a_shutdown_certificate() {
        let mut state = Collector::default();
        send(&mut state, 25, 42);
        assert!(!state.malformed);
        assert_eq!(state.il.file_scope_runtime_links,
                   vec![FileScopeRuntimeLink {
                       link_variable: SymbolId(25),
                       startup_routine: SymbolId(42),
                   }]);
        assert!(state.il.file_scope_init_routines.is_empty());
        assert!(state.il.scope_dynamic_initializers.is_empty());
        send(&mut state, 25, 43);
        assert!(state.malformed, "duplicate link object was silently accepted");
    }
    #[test]
    fn many_runtime_links_keep_stable_order_and_reject_duplicates() {
        let mut state=Collector::default();
        for variable in 1..=4096 {
            send(&mut state,variable,variable + 10_000);
        }
        assert!(!state.malformed);
        assert_eq!(state.il.file_scope_runtime_links.len(),4096);
        assert_eq!(state.seen.file_scope_runtime_link_variables.len(),4096);
        assert_eq!(state.il.file_scope_runtime_links[0].link_variable,SymbolId(1));
        assert_eq!(state.il.file_scope_runtime_links[4095].link_variable,SymbolId(4096));
        send(&mut state,2048,99);
        assert!(state.malformed,"duplicate link within a large roster was accepted");
        assert_eq!(state.il.file_scope_runtime_links.len(),4096);
    }
    #[test]
    fn missing_native_identity_is_rejected() {
        for (variable, routine) in [(0, 42), (25, 0)] {
            let mut state = Collector::default();
            send(&mut state, variable, routine);
            assert!(state.malformed);
            assert!(state.il.file_scope_runtime_links.is_empty());
        }
    }
}

#[cfg(test)]
mod native_scope_asm_wire_tests {
    use super::*;

    fn send(state:&mut Collector,scope:&str,position:u32,assembly:&str,flags:u32) {
        let fields=[
            WireField{data:scope.as_ptr(),length:scope.len(),value:0,tag:wire_tags::TEXT},
            WireField{data:std::ptr::null(),length:0,value:u64::from(position),
                      tag:wire_tags::UNSIGNED},
            WireField{data:assembly.as_ptr(),length:assembly.len(),value:0,tag:wire_tags::TEXT},
            WireField{data:std::ptr::null(),length:0,value:u64::from(flags),
                      tag:wire_tags::UNSIGNED},
        ];
        unsafe {
            collect((state as *mut Collector).cast(),101,
                    fields.as_ptr(),fields.len(),0);
        }
    }

    #[test]
    fn opaque_native_asm_preserves_gnu_linker_anchor_and_scope_order() {
        let mut state=Collector::default();
        let raw=".globl _ZSt21ios_base_library_initv";
        send(&mut state,"1:",0,raw,2);
        send(&mut state,"2:std::",0,"",0);
        assert!(!state.malformed);
        assert_eq!(state.il.native_scope_asm,vec![
            NativeScopeAsm{scope:"1:".into(),position:0,
                           assembly:raw.into(),flags:2},
            NativeScopeAsm{scope:"2:std::".into(),position:0,
                           assembly:String::new(),flags:0},
        ]);
    }

    #[test]
    fn native_asm_invalid_flags_and_noncontiguous_ordinals_fail_closed() {
        let mut state=Collector::default();
        send(&mut state,"1:",0,".globl ext",0);
        send(&mut state,"1:",2,"nop",0);
        assert!(state.malformed);
        let mut invalid=Collector::default();
        send(&mut invalid,"1:",0,"nop",32);
        assert!(invalid.malformed);
        let mut missing=Collector::default();
        send(&mut missing,"",0,"nop",0);
        assert!(missing.malformed);
    }

    #[test]
    fn interleaved_scopes_keep_independent_native_asm_ordinals() {
        let mut state=Collector::default();
        for position in 0..2048_u32 {
            send(&mut state,"1:",position,".globl exact_linker_anchor",2);
            send(&mut state,"2:std::",position,"",0);
        }
        assert!(!state.malformed);
        assert_eq!(state.il.native_scope_asm.len(),4096);
        assert_eq!(state.seen.native_scope_asm_next.get("1:"),Some(&2048));
        assert_eq!(state.seen.native_scope_asm_next.get("2:std::"),Some(&2048));
        assert_eq!(state.il.native_scope_asm[4094].position,2047);
        assert_eq!(state.il.native_scope_asm[4095].scope,"2:std::");

        // A missing or duplicated ordinal is rejected even if other scopes
        // have emitted arbitrary numbers of correctly ordered directives.
        let mut missing=Collector::default();
        send(&mut missing,"1:",0,"a",0);
        send(&mut missing,"2:std::",0,"b",0);
        send(&mut missing,"2:std::",2,"c",0);
        assert!(missing.malformed);
        assert_eq!(missing.il.native_scope_asm.len(),2);
        let mut duplicate=Collector::default();
        send(&mut duplicate,"1:",0,"a",0);
        send(&mut duplicate,"2:std::",0,"b",0);
        send(&mut duplicate,"1:",0,"duplicate",0);
        assert!(duplicate.malformed);
        assert_eq!(duplicate.il.native_scope_asm.len(),2);
    }
}

#[cfg(test)]
mod file_scope_init_schedule_wire_tests {
    use super::*;
    fn send(state:&mut Collector,kind:u32,values:[u32;2]) {
        let fields=values.map(|value|WireField {
            data:std::ptr::null(),length:0,value:u64::from(value),
            tag:wire_tags::UNSIGNED,
        });
        unsafe {
            collect((state as *mut Collector).cast(),
                    kind,fields.as_ptr(),fields.len(),0);
        }
    }
    #[test]
    fn ordered_roots_preserve_native_sequence_and_config_proof() {
        let mut state=Collector::default();
        send(&mut state,67,[0,100]);
        send(&mut state,67,[1,42]);
        send(&mut state,66,[1,1]);
        assert!(!state.malformed);
        assert!(!state.panicked);
        assert_eq!(state.il.file_scope_init_schedule,
                   Some(FileScopeInitSchedule{available:true,complete:true}));
        assert_eq!(state.il.file_scope_init_routines,
                   vec![FileScopeInitRoutine{position:0,routine:SymbolId(100)},
                        FileScopeInitRoutine{position:1,routine:SymbolId(42)}]);
        send(&mut state,67,[1,43]); // cannot duplicate or skip an ordinal
        assert!(state.malformed);
    }
    #[test]
    fn scope_initializers_keep_destructor_identity_and_order() {
        let mut state=Collector::default();
        let key=b"1:std::";
        let mk=|position:u64,variable:u64,destructor:u64|{
            [
                WireField{data:key.as_ptr(),length:key.len(),value:0,tag:wire_tags::TEXT},
                WireField{data:std::ptr::null(),length:0,value:position,tag:wire_tags::UNSIGNED},
                WireField{data:std::ptr::null(),length:0,value:variable,tag:wire_tags::UNSIGNED},
                WireField{data:std::ptr::null(),length:0,value:destructor,tag:wire_tags::UNSIGNED},
                WireField{data:std::ptr::null(),length:0,value:0,tag:wire_tags::UNSIGNED},
            ]
        };
        for (position,variable,destructor) in [(0,11,31),(1,12,32)] {
            let fields=mk(position,variable,destructor);
            unsafe {collect((&mut state as *mut Collector).cast(),68,
                            fields.as_ptr(),fields.len(),0);}
        }
        assert!(!state.malformed);
        assert_eq!(state.il.scope_dynamic_initializers.len(),2);
        assert_eq!(state.il.scope_dynamic_initializers[1].destructor,Some(SymbolId(32)));
        let fields=mk(1,13,33);
        unsafe {collect((&mut state as *mut Collector).cast(),68,
                        fields.as_ptr(),fields.len(),0);}
        assert!(state.malformed,"duplicate per-scope initializer ordinal accepted");
    }
    /// Send a native scope-local initializer with independent ordinal and owners.
    fn send_scope(state:&mut Collector,scope:&str,position:u32,
                  variable:u32,destructor:u32) {
        let fields=[
            WireField{data:scope.as_ptr(),length:scope.len(),value:0,tag:wire_tags::TEXT},
            WireField{data:std::ptr::null(),length:0,value:u64::from(position),
                      tag:wire_tags::UNSIGNED},
            WireField{data:std::ptr::null(),length:0,value:u64::from(variable),
                      tag:wire_tags::UNSIGNED},
            WireField{data:std::ptr::null(),length:0,value:u64::from(destructor),
                      tag:wire_tags::UNSIGNED},
            WireField{data:std::ptr::null(),length:0,value:0,tag:wire_tags::UNSIGNED},
        ];
        unsafe {
            collect((state as *mut Collector).cast(),68,
                    fields.as_ptr(),fields.len(),0);
        }
    }

    #[test]
    fn interleaved_scope_initializers_have_linear_independent_ordinals() {
        let mut state=Collector::default();
        for position in 0..2048_u32 {
            send_scope(&mut state,"1:",position,11,31);
            send_scope(&mut state,"2:std::",position,12,32);
        }
        assert!(!state.malformed);
        assert!(!state.panicked);
        assert_eq!(state.il.scope_dynamic_initializers.len(),4096);
        assert_eq!(state.seen.scope_dynamic_initializer_next.get("1:"),Some(&2048));
        assert_eq!(state.seen.scope_dynamic_initializer_next.get("2:std::"),Some(&2048));
        assert_eq!(state.il.scope_dynamic_initializers[4094].position,2047);
        assert_eq!(state.il.scope_dynamic_initializers[4095].scope,"2:std::");
        assert_eq!(state.il.scope_dynamic_initializers[4095].destructor,
                   Some(SymbolId(32)));

        let mut missing=Collector::default();
        send_scope(&mut missing,"1:",0,11,31);
        send_scope(&mut missing,"2:std::",0,12,32);
        send_scope(&mut missing,"2:std::",2,12,32);
        assert!(missing.malformed,"missing per-scope ordinal must fail closed");
        assert_eq!(missing.il.scope_dynamic_initializers.len(),2);

        let mut duplicate=Collector::default();
        send_scope(&mut duplicate,"1:",0,11,31);
        send_scope(&mut duplicate,"2:std::",0,12,32);
        send_scope(&mut duplicate,"1:",0,11,31);
        assert!(duplicate.malformed,"duplicate per-scope ordinal must fail closed");
        assert_eq!(duplicate.il.scope_dynamic_initializers.len(),2);

        let mut overflow=Collector::default();
        send_scope(&mut overflow,"1:",u32::MAX,11,31);
        assert!(overflow.malformed,"nonzero first ordinal must fail closed");
        assert!(overflow.il.scope_dynamic_initializers.is_empty());
    }

    #[test]
    fn unavailable_scheduler_and_ordered_roots_are_mutually_exclusive() {
        // EDG normally emits roots before the availability witness. The
        // decoder must reject contradictory records in either arrival order,
        // including synthetic or version-mismatched native streams.
        for roots_first in [false, true] {
            let mut state=Collector::default();
            if roots_first {
                send(&mut state,67,[0,42]);
                assert!(!state.malformed);
                send(&mut state,66,[0,0]);
                assert!(state.malformed);
                assert!(state.il.file_scope_init_schedule.is_none());
            } else {
                send(&mut state,66,[0,0]);
                assert!(!state.malformed);
                send(&mut state,67,[0,42]);
                assert!(state.malformed);
                assert!(state.il.file_scope_init_routines.is_empty());
            }
        }
    }

    #[test]
    fn unavailable_scheduler_is_not_a_no_effects_certificate() {
        let mut state=Collector::default();
        send(&mut state,66,[0,0]);
        assert_eq!(state.il.file_scope_init_schedule,
                   Some(FileScopeInitSchedule{available:false,complete:false}));
        assert!(state.il.file_scope_init_routines.is_empty());
        assert!(!state.malformed);
        send(&mut state,66,[1,1]); // exactly one availability record
        assert!(state.malformed);
    }
}


#[cfg(test)]
mod cxx5_base_subobject_offset_wire_tests {
    use super::*;

    fn send(state:&mut Collector,derived:u32,position:u32,offset:Option<u64>) {
        let fields=[
            WireField{data:std::ptr::null(),length:0,value:u64::from(derived),
                      tag:wire_tags::UNSIGNED},
            WireField{data:std::ptr::null(),length:0,value:u64::from(position),
                      tag:wire_tags::UNSIGNED},
            WireField{data:std::ptr::null(),length:0,value:offset.unwrap_or_default(),
                      tag:if offset.is_some() {wire_tags::UNSIGNED} else {wire_tags::ABSENT}},
        ];
        unsafe {
            collect((state as *mut Collector).cast(),69,
                    fields.as_ptr(),fields.len(),0);
        }
    }
    #[test]
    fn distinguishes_known_zero_and_unknown_offset() {
        let mut state=Collector::default();
        send(&mut state,118,0,Some(0));
        send(&mut state,118,1,Some(64));
        send(&mut state,119,0,None);
        assert!(!state.malformed);
        assert_eq!(state.il.base_subobject_offsets,vec![
            BaseSubobjectOffset{derived:TypeId(118),position:0,byte_offset:Some(0)},
            BaseSubobjectOffset{derived:TypeId(118),position:1,byte_offset:Some(64)},
            BaseSubobjectOffset{derived:TypeId(119),position:0,byte_offset:None},
        ]);
        send(&mut state,118,0,Some(8));
        assert!(state.malformed,"duplicate base position must fail closed");
    }
    #[test]
    fn reject_zero_owner() {
        let mut state=Collector::default();
        send(&mut state,0,0,Some(0));
        assert!(state.malformed);
        assert!(state.il.base_subobject_offsets.is_empty());
    }
}

#[cfg(test)]
mod cxx5_call_target_wire_tests {
    use super::*;
    fn wire(state:&mut Collector, form:u64, callee_type:Option<u64>,
            callee_symbol:Option<u64>) {
        let function=b"main";
        let statement=b"0.block.0";
        let path=b"0";
        let mk_text=|bytes:&[u8]|WireField {
            data:bytes.as_ptr(), length:bytes.len(),value:0,tag:wire_tags::TEXT,
        };
        let mk_num=|value:Option<u64>|WireField {
            data:std::ptr::null(),length:0,value:value.unwrap_or_default(),
            tag:if value.is_some() {wire_tags::UNSIGNED} else {wire_tags::ABSENT},
        };
        let fields=[
            mk_text(function),mk_text(statement),mk_text(path),
            mk_num(Some(form)),mk_num(callee_type),mk_num(callee_symbol),
        ];
        unsafe {
            collect((state as *mut Collector).cast(),77,
                fields.as_ptr(),fields.len(),0);
        }
    }
    fn source() -> Il {
        let mut il=Il::default();
        il.calls.push(CallSite {
            function:"main".into(), statement_path:"0.block.0".into(),
            path:"0".into(), first_argument:1,
        });
        il.expressions.push(Expression {
            function:"main".into(),statement_path:"0.block.0".into(),
            path:"0.0".into(),parent:Some("0".into()),kind:3,
            operator:None,type_kind:None,reference:Some("f".into()),
            type_size:None,type_alignment:None,constant_kind:None,
            constant_integer:None,related_type_role:None,
            related_type_kind:None,type_id:Some(TypeId(70)),
            symbol_id:Some(SymbolId(9)),
        });
        il.types.push(TypeDef {id:TypeId(70),kind:2,
            related_role:None,related:None});
        il.symbols.push(Symbol{id:SymbolId(9),kind:1,name:"f".into()});
        il.call_targets.push(CallTargetProvenance {
            function:"main".into(),statement_path:"0.block.0".into(),
            path:"0".into(),form:CallTargetForm::DirectRoutine,
            callee_type:Some(TypeId(70)),callee_symbol:Some(SymbolId(9)),
        });
        il
    }
    #[test]
    fn direct_indirect_and_missing_symbol_are_distinct_on_wire() {
        let mut direct=Collector::default();
        wire(&mut direct,1,Some(70),Some(9));
        assert!(!direct.malformed);
        assert_eq!(direct.il.call_targets[0].form,CallTargetForm::DirectRoutine);
        assert_eq!(direct.il.call_targets[0].callee_symbol,Some(SymbolId(9)));
        let mut indirect=Collector::default();
        wire(&mut indirect,3,Some(70),None);
        assert!(!indirect.malformed);
        assert_eq!(indirect.il.call_targets[0].form,CallTargetForm::IndirectUnresolved);
        assert_eq!(indirect.il.call_targets[0].callee_symbol,None);
        let mut invalid=Collector::default();
        wire(&mut invalid,3,Some(70),Some(9));
        assert!(invalid.malformed,"indirect callee must never invent routine identity");
        let mut invalid=Collector::default();
        wire(&mut invalid,1,Some(70),None);
        assert!(invalid.malformed,"direct callee must own a routine source symbol");
        let mut invalid=Collector::default();
        wire(&mut invalid,4,Some(70),None);
        assert!(invalid.malformed,"unrecognized callee form cannot be silently direct");
    }
    #[test]
    fn constant_address_callee_requires_independent_matching_source_proof() {
        let mut il=source();
        il.expressions[0].symbol_id=None;
        il.call_targets[0].form=CallTargetForm::ConstantRoutineAddress;
        assert!(validate_native_call_target_coverage(&il).is_err(),
            "event 77 alone cannot invent a source constant routine target");

        il.constant_routine_addresses.push(ConstantRoutineAddress {
            function:"main".into(),statement_path:"0.block.0".into(),
            path:"0.0".into(),symbol:SymbolId(9),
        });
        assert!(validate_native_call_target_coverage(&il).is_ok());

        let mut wrong=il.clone();
        wrong.constant_routine_addresses[0].symbol=SymbolId(8);
        assert!(validate_native_call_target_coverage(&wrong).is_err(),
            "source target must match the claimed callee routine");

        let mut wrong=il.clone();
        wrong.constant_routine_addresses[0].path="0.1".into();
        assert!(validate_native_call_target_coverage(&wrong).is_err(),
            "unrelated constant addresses do not certify a call");

        let mut wrong=il.clone();
        wrong.constant_routine_addresses.push(
            wrong.constant_routine_addresses[0].clone());
        assert!(validate_native_call_target_coverage(&wrong).is_err(),
            "duplicate source address proof is ambiguous");

        let mut wrong=il.clone();
        wrong.call_targets[0].form=CallTargetForm::IndirectUnresolved;
        wrong.call_targets[0].callee_symbol=None;
        assert!(validate_native_call_target_coverage(&wrong).is_err(),
            "verified constant target cannot silently become unresolved");

        let mut wrong=il;
        wrong.call_targets[0].form=CallTargetForm::DirectRoutine;
        wrong.expressions[0].symbol_id=Some(SymbolId(9));
        assert!(validate_native_call_target_coverage(&wrong).is_err(),
            "the native direct-routine form cannot claim a constant address");
    }
    #[test]
    fn native_direct_and_constant_callees_require_positive_symbol_witnesses() {
        let mut il=source();
        il.call_targets[0].callee_symbol=None;
        il.expressions[0].symbol_id=None;
        assert!(validate_native_call_target_coverage(&il).is_err(),
            "a missing direct symbol is not a valid source routine witness");
        il.call_targets[0].form=CallTargetForm::ConstantRoutineAddress;
        assert!(validate_native_call_target_coverage(&il).is_err(),
            "two absent values cannot certify a constant routine address");
    }

    #[test]
    fn many_native_callee_records_keep_exact_one_to_one_coverage() {
        let sample=source();
        let mut il=sample.clone();
        il.calls.clear();
        il.call_targets.clear();
        il.expressions.clear();
        for index in 0..4096 {
            let statement=format!("0.block.{index}");
            let mut site=sample.calls[0].clone();
            site.statement_path=statement.clone();
            il.calls.push(site);
            let mut target=sample.call_targets[0].clone();
            target.statement_path=statement.clone();
            il.call_targets.push(target);
            let mut expression=sample.expressions[0].clone();
            expression.statement_path=statement;
            il.expressions.push(expression);
        }
        validate_native_call_target_coverage(&il)
            .expect("4096 matching calls and source callee expressions");
        let mut wrong=il.clone();
        wrong.call_targets[2048].callee_symbol=Some(SymbolId(999));
        assert!(validate_native_call_target_coverage(&wrong).is_err(),
            "an inconsistent callee cannot borrow another site's identity");
        il.expressions.pop();
        assert!(validate_native_call_target_coverage(&il).is_err(),
            "missing final source expression must reject the snapshot");
    }

    #[test]
    fn successful_snapshot_needs_exact_one_to_one_call_callee_identity() {
        let mut il=source();
        assert!(validate_native_call_target_coverage(&il).is_ok());
        il.call_targets.clear();
        assert!(validate_native_call_target_coverage(&il).is_err());
        let mut il=source();
        il.call_targets.push(il.call_targets[0].clone());
        assert!(validate_native_call_target_coverage(&il).is_err());
        let mut il=source();
        il.call_targets[0].callee_type=Some(TypeId(99));
        assert!(validate_native_call_target_coverage(&il).is_err());
        let mut il=source();
        il.call_targets[0].callee_symbol=Some(SymbolId(8));
        assert!(validate_native_call_target_coverage(&il).is_err());
        let mut il=source();
        il.call_targets[0].path="0.1".into();
        assert!(validate_native_call_target_coverage(&il).is_err());
        let mut il=source();
        il.call_targets[0].form=CallTargetForm::IndirectUnresolved;
        il.call_targets[0].callee_symbol=None;
        assert!(validate_native_call_target_coverage(&il).is_ok(),
            "unresolved source calls remain a distinct safe, non-authorized class");
    }
}

// Historical source-hook events are decoded only in these unit fixtures.
// The live EDG callback deliberately rejects them before record parsing.
#[cfg(test)]
fn collect_retired_wire_fixture(state: &mut Collector, kind: u32, fields: &[WireField]) {
    assert!(matches!(kind, 80 | 81 | 82 | 85));
    let before = callback_record_count(&state.il, kind);
    if expected_wire_field_count(kind) != Some(fields.len()) {
        state.malformed = true;
        return;
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        collect_event(&mut state.il, &mut state.seen, kind, fields.as_ptr(), fields.len(), 0);
    }));
    if result.is_err() {
        state.panicked = true;
        state.malformed = true;
    } else if callback_record_count(&state.il, kind) == before {
        state.malformed = true;
    }
}

#[cfg(test)]
mod cxx5_virtual_receiver_reuse_tests {
    use super::*;

    fn fixture() -> Il {
        let mut il=Il { cpp_language:Some(true), ..Il::default() };
        il.types.push(TypeDef {
            id:TypeId(4),kind:11,related_role:None,related:None,
        });
        il.calls.push(CallSite {
            function:"caller".into(),statement_path:"0.block.2".into(),
            path:"0".into(),first_argument:1,
        });
        il.prelower_virtual_origins.push(PrelowerVirtualCallOrigin {
            function:"caller".into(),statement_path:"0.block.2".into(),
            path:"0".into(),original_routine:SymbolId(9),
            original_receiver_type:Some(TypeId(4)),
            original_operator:107,receiver_node_retained:false,
        });
        // The lowered ordinary eok_call's callee is at 0.0, so the
        // EDG-reused receiver is its first direct argument at 0.1.
        il.expressions.push(Expression {
            function:"caller".into(),statement_path:"0.block.2".into(),
            path:"0.1".into(),parent:Some("0".into()),
            kind:3,operator:None,type_kind:None,
            reference:None,type_size:None,type_alignment:None,
            constant_kind:None,constant_integer:None,
            related_type_role:None,related_type_kind:None,
            type_id:Some(TypeId(4)),symbol_id:None,
        });
        il.virtual_receiver_reuses.push(VirtualReceiverReuse {
            function:"caller".into(),statement_path:"0.block.2".into(),
            path:"0".into(),receiver_type:TypeId(4),
            argument_position:0,original_node_reused:false,
        });
        il
    }

    #[test]
    fn accept_exact_source_reused_receiver_copy_but_not_execution() {
        let mut il=fixture();
        assert!(validate_virtual_receiver_reuses(&il).is_ok());
        assert!(!il.virtual_receiver_reuses[0].original_node_reused,
            "EDG make_reusable_copy does not mean source pointer identity");
        // The receiver witness is optional. A missing source pointer
        // witness must not produce a substitute by pattern matching.
        il.virtual_receiver_reuses.clear();
        assert!(validate_virtual_receiver_reuses(&il).is_ok());
    }

    #[test]
    fn receiver_witness_must_have_original_virtual_call_and_exact_arg() {
        let il=fixture();
        let mut bad=il.clone();
        bad.prelower_virtual_origins.clear();
        assert!(validate_virtual_receiver_reuses(&bad).is_err());
        let mut bad=il.clone();
        bad.virtual_receiver_reuses.push(bad.virtual_receiver_reuses[0].clone());
        assert!(validate_virtual_receiver_reuses(&bad).is_err());
        let mut bad=il.clone();
        bad.virtual_receiver_reuses[0].receiver_type=TypeId(999);
        assert!(validate_virtual_receiver_reuses(&bad).is_err());
        let mut bad=il.clone();
        bad.virtual_receiver_reuses[0].argument_position=1;
        assert!(validate_virtual_receiver_reuses(&bad).is_err());
        let mut bad=il.clone();
        bad.expressions[0].type_id=None;
        assert!(validate_virtual_receiver_reuses(&bad).is_err());
        let mut bad=il.clone();
        bad.expressions[0].path="0.2".into();
        assert!(validate_virtual_receiver_reuses(&bad).is_err());
        let mut bad=il.clone();
        bad.calls[0].first_argument=2;
        assert!(validate_virtual_receiver_reuses(&bad).is_err());
        let mut bad=il.clone();
        bad.cpp_language=Some(false);
        assert!(validate_virtual_receiver_reuses(&bad).is_err());
    }

    #[test]
    fn receiver_record_81_has_typed_fields_and_bounded_position() {
        let text=|value:&[u8]| WireField {
            data:value.as_ptr(),length:value.len(),value:0,
            tag:wire_tags::TEXT,
        };
        let number=|value:u64| WireField {
            data:std::ptr::null(),length:0,value,
            tag:wire_tags::UNSIGNED,
        };
        let fields=[text(b"caller"),text(b"0.block.2"),text(b"0"),
                    number(4),number(0),number(0)];
        let mut state=Collector::default();
        collect_retired_wire_fixture(&mut state, 81, &fields);
        assert!(!state.malformed);
        assert_eq!(state.il.virtual_receiver_reuses.len(),1);
        assert_eq!(state.il.virtual_receiver_reuses[0].receiver_type,TypeId(4));
        assert_eq!(state.il.virtual_receiver_reuses[0].argument_position,0);
        assert!(!state.il.virtual_receiver_reuses[0].original_node_reused);
        let bad=[text(b"caller"),text(b"0.block.2"),text(b"0"),
                 number(4),number(16),number(0)];
        let mut invalid=Collector::default();
        collect_retired_wire_fixture(&mut invalid, 81, &bad);
        assert!(invalid.malformed);
        let bad=[text(b"caller"),text(b"0.block.2"),text(b"0"),
                 number(4),number(0),number(2)];
        let mut invalid=Collector::default();
        collect_retired_wire_fixture(&mut invalid, 81, &bad);
        assert!(invalid.malformed);
    }
}

#[cfg(test)]
mod cxx5_virtual_vptr_receiver_pair_tests {
    use super::*;

    fn source() -> Il {
        let mut il=Il {cpp_language:Some(true), ..Il::default()};
        il.types.push(TypeDef {
            id:TypeId(4),kind:11,related_role:None,related:None,
        });
        il.prelower_virtual_origins.push(PrelowerVirtualCallOrigin {
            function:"caller".into(),statement_path:"0.block.2".into(),
            path:"0".into(),original_routine:SymbolId(9),
            original_receiver_type:Some(TypeId(4)),
            original_operator:107,receiver_node_retained:false,
        });
        il.virtual_entry_indexes.push(VirtualEntryIndexSelection {
            function:"caller".into(),statement_path:"0.block.2".into(),
            path:"0".into(),source_index:0,entry_bytes:8,
        });
        il.virtual_receiver_reuses.push(VirtualReceiverReuse {
            function:"caller".into(),statement_path:"0.block.2".into(),
            path:"0".into(),receiver_type:TypeId(4),
            argument_position:0,original_node_reused:false,
        });
        il.virtual_vptr_receiver_pairs.push(VirtualVptrReceiverPair {
            function:"caller".into(),statement_path:"0.block.2".into(),
            path:"0".into(),lookup_object_type:TypeId(4),
            argument_position:0,
        });
        il
    }

    #[test]
    fn source_pair_requires_independent_call_lookup_and_receiver_evidence() {
        let il=source();
        assert!(validate_virtual_vptr_receiver_pairs(&il).is_ok());
        let mut partial=il.clone();
        partial.virtual_vptr_receiver_pairs.clear();
        assert!(validate_virtual_vptr_receiver_pairs(&partial).is_ok(),
            "the new source observer is intentionally optional");
        for which in 0..3 {
            let mut bad=il.clone();
            match which {
                0 => bad.prelower_virtual_origins.clear(),
                1 => bad.virtual_entry_indexes.clear(),
                _ => bad.virtual_receiver_reuses.clear(),
            }
            assert!(validate_virtual_vptr_receiver_pairs(&bad).is_err());
        }
        let mut bad=il.clone();
        bad.virtual_vptr_receiver_pairs.push(
            bad.virtual_vptr_receiver_pairs[0].clone());
        assert!(validate_virtual_vptr_receiver_pairs(&bad).is_err());
        let mut bad=il.clone();
        bad.virtual_vptr_receiver_pairs[0].lookup_object_type=TypeId(999);
        assert!(validate_virtual_vptr_receiver_pairs(&bad).is_err());
        let mut bad=il.clone();
        bad.virtual_vptr_receiver_pairs[0].argument_position=1;
        assert!(validate_virtual_vptr_receiver_pairs(&bad).is_err());
        let mut bad=il.clone();
        bad.virtual_vptr_receiver_pairs[0].path="different".into();
        assert!(validate_virtual_vptr_receiver_pairs(&bad).is_err());
        let mut bad=il.clone();
        bad.cpp_language=Some(false);
        assert!(validate_virtual_vptr_receiver_pairs(&bad).is_err());
    }

    #[test]
    fn typed_source_pair_event_85_rejects_invalid_fields() {
        let text=|value:&[u8]| WireField {
            data:value.as_ptr(),length:value.len(),value:0,
            tag:wire_tags::TEXT,
        };
        let number=|value:u64| WireField {
            data:std::ptr::null(),length:0,value,
            tag:wire_tags::UNSIGNED,
        };
        let fields=[text(b"caller"),text(b"0.block.2"),text(b"0"),
                    number(4),number(0)];
        let mut state=Collector::default();
        collect_retired_wire_fixture(&mut state, 85, &fields);
        assert!(!state.malformed);
        assert_eq!(state.il.virtual_vptr_receiver_pairs.len(),1);
        assert_eq!(state.il.virtual_vptr_receiver_pairs[0]
                   .lookup_object_type,TypeId(4));
        assert_eq!(state.il.virtual_vptr_receiver_pairs[0]
                   .argument_position,0);
        for (ty,pos) in [(0,0),(4,16)] {
            let bad=[text(b"caller"),text(b"0.block.2"),text(b"0"),
                     number(ty),number(pos)];
            let mut collector=Collector::default();
            collect_retired_wire_fixture(&mut collector, 85, &bad);
            assert!(collector.malformed);
        }
        let malformed_tags=[text(b"caller"),text(b"0.block.2"),
                            text(b"0"),text(b"not_unsigned"),number(0)];
        let mut collector=Collector::default();
        collect_retired_wire_fixture(&mut collector, 85, &malformed_tags);
        assert!(collector.malformed);
    }
}

#[cfg(test)]
mod cxx5_virtual_entry_index_selection_tests {
    use super::*;

    fn source(index:u64) -> Il {
        let mut il=Il {cpp_language:Some(true), ..Il::default()};
        il.symbols.push(Symbol {
            id:SymbolId(9),kind:1,name:"Base::f".into(),
        });
        il.routine_attributes.push(RoutineAttributes {
            routine:SymbolId(9),special_kind:0,is_virtual:true,
            pure_virtual:false,is_override:false,is_final:false,
            overrides_base_member:false,is_inline:false,is_constexpr:false,
            defined:true,explicit_constructor:false,has_this_parameter:true,
            compiler_generated:false,
        });
        il.native_virtual_members.push(NativeVirtualMemberInfo {
            routine:SymbolId(9),owner:TypeId(4),
            virtual_number:Some(index),virtual_info_offset:Some(0),
            sharing_base:None,
        });
        il.prelower_virtual_origins.push(PrelowerVirtualCallOrigin {
            function:"caller".into(),statement_path:"0.block.2".into(),
            path:"0".into(),original_routine:SymbolId(9),
            original_receiver_type:Some(TypeId(4)),
            original_operator:107,receiver_node_retained:false,
        });
        il.virtual_entry_indexes.push(VirtualEntryIndexSelection {
            function:"caller".into(),statement_path:"0.block.2".into(),
            path:"0".into(),source_index:index,entry_bytes:8,
        });
        il
    }

    #[test]
    fn source_index_must_match_original_virtual_method_and_safe_width() {
        let mut il=source(0);
        assert!(validate_virtual_entry_index_selections(&il).is_ok(),
            "zero-based Itanium native virtual index must be preserved");
        il=source(6);
        assert!(validate_virtual_entry_index_selections(&il).is_ok(),
            "source index six is still not an independently proven machine slot");
        il.virtual_entry_indexes.clear();
        assert!(validate_virtual_entry_index_selections(&il).is_ok(),
            "copied index nodes may lose pointer identity without guessed ABI");
    }

    #[test]
    fn source_index_rejects_missing_ambiguous_or_wrong_method_evidence() {
        let il=source(3);
        let mut bad=il.clone();
        bad.virtual_entry_indexes.push(bad.virtual_entry_indexes[0].clone());
        assert!(validate_virtual_entry_index_selections(&bad).is_err());
        let mut bad=il.clone();
        bad.native_virtual_members[0].virtual_number=Some(4);
        assert!(validate_virtual_entry_index_selections(&bad).is_err());
        let mut bad=il.clone();
        bad.routine_attributes[0].is_virtual=false;
        assert!(validate_virtual_entry_index_selections(&bad).is_err());
        let mut bad=il.clone();
        bad.prelower_virtual_origins.clear();
        assert!(validate_virtual_entry_index_selections(&bad).is_err());
        let mut bad=il.clone();
        bad.virtual_entry_indexes[0].path="different".into();
        assert!(validate_virtual_entry_index_selections(&bad).is_err());
        let mut bad=il.clone();
        bad.cpp_language=Some(false);
        assert!(validate_virtual_entry_index_selections(&bad).is_err());
    }

    #[test]
    fn native_element_width_is_not_a_guessed_machine_slot() {
        let il=source(3);
        for invalid_width in [0,3,5,64] {
            let mut bad=il.clone();
            bad.virtual_entry_indexes[0].entry_bytes=invalid_width;
            assert!(validate_virtual_entry_index_selections(&bad).is_err());
        }
        let mut bad=source(u64::MAX);
        bad.virtual_entry_indexes[0].entry_bytes=8;
        assert!(validate_virtual_entry_index_selections(&bad).is_err());
    }

    #[test]
    fn wire_record_is_typed_and_overflow_is_rejected() {
        let text=|value:&[u8]| WireField {
            data:value.as_ptr(),length:value.len(),value:0,
            tag:wire_tags::TEXT,
        };
        let number=|value:u64| WireField {
            data:std::ptr::null(),length:0,value,
            tag:wire_tags::UNSIGNED,
        };
        let fields=[text(b"caller"),text(b"0.block.2"),text(b"0"),
                    number(0),number(8)];
        let mut collector=Collector::default();
        collect_retired_wire_fixture(&mut collector, 82, &fields);
        assert!(!collector.malformed);
        let got=&collector.il.virtual_entry_indexes[0];
        assert_eq!(got.source_index,0);
        assert_eq!(got.entry_bytes,8);
        let overflow=[text(b"caller"),text(b"0.block.2"),text(b"0"),
                      number(u64::MAX),number(8)];
        let mut collector=Collector::default();
        collect_retired_wire_fixture(&mut collector, 82, &overflow);
        assert!(collector.malformed);
    }
}

#[cfg(test)]
mod cxx5_prelower_virtual_call_origin_tests {
    use super::*;

    fn source() -> Il {
        let mut il=Il {cpp_language:Some(true), ..Il::default()};
        il.symbols.push(Symbol {
            id:SymbolId(9),kind:1,name:"Base::f".into(),
        });
        il.types.push(TypeDef {id:TypeId(4),kind:11,
            related_role:None,related:None});
        il.member_functions.push(MemberFunction {
            owner:TypeId(4),routine:SymbolId(9),
        });
        il.routine_attributes.push(RoutineAttributes {
            routine:SymbolId(9),special_kind:0,is_virtual:true,
            pure_virtual:false,is_override:false,is_final:false,
            overrides_base_member:false,is_inline:false,is_constexpr:false,
            defined:true,explicit_constructor:false,has_this_parameter:true,
            compiler_generated:false,
        });
        il.native_virtual_members.push(NativeVirtualMemberInfo {
            routine:SymbolId(9),owner:TypeId(4),virtual_number:Some(0),
            virtual_info_offset:Some(0),sharing_base:None,
        });
        il.calls.push(CallSite {
            function:"caller".into(),statement_path:"0.block.2".into(),
            path:"0".into(),first_argument:1,
        });
        il.call_targets.push(CallTargetProvenance {
            function:"caller".into(),statement_path:"0.block.2".into(),
            path:"0".into(),form:CallTargetForm::IndirectUnresolved,
            callee_type:None,callee_symbol:None,
        });
        il.call_dispatches.push(CallDispatchProvenance {
            function:"caller".into(),statement_path:"0.block.2".into(),
            path:"0".into(),native_operator:106,
            class:CallDispatchClass::OrdinaryOrLowered,
            explicitly_virtual:false,
        });
        il.prelower_virtual_origins.push(PrelowerVirtualCallOrigin {
            function:"caller".into(),statement_path:"0.block.2".into(),
            path:"0".into(),original_routine:SymbolId(9),
            original_receiver_type:Some(TypeId(4)),
            original_operator:107,receiver_node_retained:false,
        });
        il
    }

    #[test]
    fn source_proof_is_optional_and_distinct_from_indirect_call_permission() {
        let mut il=source();
        assert!(validate_prelower_virtual_call_origins(&il).is_ok());
        il.prelower_virtual_origins.clear();
        assert!(validate_prelower_virtual_call_origins(&il).is_ok(),
            "lost EDG pointer-carrier must not invent a virtual ABI");
    }

    #[test]
    fn duplicate_or_foreign_or_direct_source_proofs_are_rejected() {
        let il=source();
        let mut wrong=il.clone();
        wrong.prelower_virtual_origins.push(il.prelower_virtual_origins[0].clone());
        assert!(validate_prelower_virtual_call_origins(&wrong).is_err());
        let mut wrong=il.clone();
        wrong.prelower_virtual_origins[0].original_routine=SymbolId(999);
        assert!(validate_prelower_virtual_call_origins(&wrong).is_err());
        let mut wrong=il.clone();
        wrong.prelower_virtual_origins[0].original_receiver_type=Some(TypeId(999));
        assert!(validate_prelower_virtual_call_origins(&wrong).is_err());
        let mut wrong=il.clone();
        wrong.native_virtual_members.clear();
        assert!(validate_prelower_virtual_call_origins(&wrong).is_err(),
            "a routine name alone cannot prove virtual dispatch");
        let mut wrong=il.clone();
        wrong.routine_attributes[0].is_virtual=false;
        assert!(validate_prelower_virtual_call_origins(&wrong).is_err(),
            "original routine must remain an actual EDG virtual method");
        let mut wrong=il.clone();
        wrong.member_functions[0].owner=TypeId(999);
        assert!(validate_prelower_virtual_call_origins(&wrong).is_err(),
            "original virtual owner must match its class-member edge");
        let mut wrong=il.clone();
        wrong.call_targets[0].form=CallTargetForm::DirectRoutine;
        assert!(validate_prelower_virtual_call_origins(&wrong).is_err());
        let mut wrong=il.clone();
        wrong.call_dispatches[0].class=CallDispatchClass::Member;
        assert!(validate_prelower_virtual_call_origins(&wrong).is_err());
        let mut wrong=il.clone();
        wrong.cpp_language=Some(false);
        assert!(validate_prelower_virtual_call_origins(&wrong).is_err());
    }

    #[test]
    fn wire_event_preserves_source_routine_and_receiver_without_slot_inference() {
        let mut collector=Collector::default();
        let text=|s:&[u8]| WireField {
            data:s.as_ptr(),length:s.len(),value:0,tag:wire_tags::TEXT,
        };
        let num=|n:u64| WireField {
            data:std::ptr::null(),length:0,value:n,tag:wire_tags::UNSIGNED,
        };
        let absent=WireField {
            data:std::ptr::null(),length:0,value:0,tag:wire_tags::ABSENT,
        };
        let fields=[text(b"caller"),text(b"0.block.2"),text(b"0"),
            num(9),num(4),num(107),num(1)];
        collect_retired_wire_fixture(&mut collector, 80, &fields);
        assert!(!collector.malformed);
        assert_eq!(collector.il.prelower_virtual_origins.len(),1);
        let got=&collector.il.prelower_virtual_origins[0];
        assert_eq!(got.original_routine,SymbolId(9));
        assert_eq!(got.original_receiver_type,Some(TypeId(4)));
        assert_eq!(got.original_operator,107);
        assert!(got.receiver_node_retained);
        let fields=[text(b"caller"),text(b"0.block.2"),text(b"0"),
            num(9),absent,num(107),num(0)];
        let mut absent_owner=Collector::default();
        collect_retired_wire_fixture(&mut absent_owner, 80, &fields);
        assert!(!absent_owner.malformed);
        assert_eq!(absent_owner.il.prelower_virtual_origins[0].original_receiver_type,None);
        let fields=[text(b"caller"),text(b"0.block.2"),text(b"0"),
            num(9),absent,num(107),num(2)];
        let mut invalid=Collector::default();
        collect_retired_wire_fixture(&mut invalid, 80, &fields);
        assert!(invalid.malformed);
    }
}

#[cfg(test)]
mod cxx5_virtual_member_info_wire_tests {
    use super::*;

    fn source() -> Il {
        let mut il=Il { cpp_language:Some(true), ..Il::default() };
        il.symbols.push(Symbol {id:SymbolId(4),kind:1,name:"Base::f".into()});
        il.types.push(TypeDef {id:TypeId(20),kind:11,
            related_role:None,related:None});
        il.member_functions.push(MemberFunction {
            owner:TypeId(20),routine:SymbolId(4),
        });
        il.routine_attributes.push(RoutineAttributes {
            routine:SymbolId(4),special_kind:0,is_virtual:true,
            pure_virtual:false,is_override:false,is_final:false,
            overrides_base_member:false,is_inline:false,is_constexpr:false,
            defined:true,explicit_constructor:false,has_this_parameter:true,
            compiler_generated:false,
        });
        il.type_object_sizes.push(TypeObjectSize {
            owner:TypeId(20),bytes:24,alignment:8,
        });
        il.native_virtual_members.push(NativeVirtualMemberInfo {
            routine:SymbolId(4),owner:TypeId(20),virtual_number:Some(1),
            virtual_info_offset:Some(0),sharing_base:None,
        });
        il
    }

    fn wire(state:&mut Collector,routine:u64,owner:u64,
            virtual_number:Option<u64>,offset:Option<u64>,base:Option<u64>) {
        let field=|value:Option<u64>| WireField {
            data:std::ptr::null(),length:0,value:value.unwrap_or(0),
            tag:if value.is_some() {wire_tags::UNSIGNED}
                else {wire_tags::ABSENT},
        };
        let fields=[field(Some(routine)),field(Some(owner)),
            field(virtual_number),field(offset),field(base)];
        unsafe {
            collect((state as *mut Collector).cast(),79,
                    fields.as_ptr(),fields.len(),0);
        }
    }

    #[test]
    fn virtual_member_wire_is_typed_and_keeps_zero_byte_offsets() {
        let mut collector=Collector::default();
        wire(&mut collector,4,20,Some(0),Some(0),None);
        assert!(!collector.malformed);
        let info=collector.il.native_virtual_members[0];
        assert_eq!(info.routine,SymbolId(4));
        assert_eq!(info.owner,TypeId(20));
        assert_eq!(info.virtual_number,Some(0),
            "IA64_ABI virtual numbering may start at zero");
        assert_eq!(info.virtual_info_offset,Some(0));
        assert_eq!(info.sharing_base,None);
        let mut broken=Collector::default();
        wire(&mut broken,0,20,Some(1),Some(0),None);
        assert!(broken.malformed,"virtual method symbol must be nonzero");
        let mut broken=Collector::default();
        wire(&mut broken,4,0,Some(1),Some(0),None);
        assert!(broken.malformed,"virtual class TypeId must be nonzero");
        let mut broken=Collector::default();
        wire(&mut broken,4,20,None,None,Some(0));
        assert!(broken.malformed,"sharing base TypeId must be nonzero");
    }

    #[test]
    fn reject_missing_duplicate_wrong_class_and_nonvirtual_source_proofs() {
        let il=source();
        assert!(validate_native_virtual_member_coverage(&il).is_ok());
        let mut missing=il.clone();
        missing.native_virtual_members.clear();
        assert!(validate_native_virtual_member_coverage(&missing).is_err());
        let mut duplicate=il.clone();
        duplicate.native_virtual_members.push(duplicate.native_virtual_members[0]);
        assert!(validate_native_virtual_member_coverage(&duplicate).is_err());
        let mut wrong=il.clone();
        wrong.native_virtual_members[0].owner=TypeId(999);
        assert!(validate_native_virtual_member_coverage(&wrong).is_err());
        let mut wrong=il.clone();
        wrong.routine_attributes[0].is_virtual=false;
        assert!(validate_native_virtual_member_coverage(&wrong).is_err());
        let mut wrong=il.clone();
        wrong.routine_attributes[0].has_this_parameter=false;
        assert!(validate_native_virtual_member_coverage(&wrong).is_err());
        let mut wrong=il.clone();
        wrong.native_virtual_members[0].virtual_info_offset=Some(24);
        assert!(validate_native_virtual_member_coverage(&wrong).is_err());
        let mut wrong=il.clone();
        wrong.native_virtual_members[0].sharing_base=Some(TypeId(200));
        assert!(validate_native_virtual_member_coverage(&wrong).is_err());
        let mut wrong=il.clone();
        wrong.cpp_language=Some(false);
        assert!(validate_native_virtual_member_coverage(&wrong).is_err());
    }

    #[test]
    fn incomplete_virtual_number_is_not_a_slot_or_dispatch_certificate() {
        let mut il=source();
        il.native_virtual_members[0].virtual_number=None;
        il.native_virtual_members[0].virtual_info_offset=None;
        assert!(validate_native_virtual_member_coverage(&il).is_ok(),
            "unknown EDG metadata must remain unknown and non-executable");
        il.native_virtual_members[0].sharing_base=Some(TypeId(20));
        assert!(validate_native_virtual_member_coverage(&il).is_err(),
            "cannot claim shared vptr information without its byte location");
    }
}

#[cfg(test)]
mod cxx5_native_call_dispatch_wire_tests {
    use super::*;

    fn send(state:&mut Collector,kind:u64,virtual_flag:u64) {
        let function=b"std::locale::facet";
        let statement=b"0.block.2";
        let path=b"0";
        let txt_field=|bytes:&[u8]| WireField {
            data:bytes.as_ptr(),length:bytes.len(),value:0,tag:wire_tags::TEXT,
        };
        let unsigned=|value| WireField {
            data:std::ptr::null(),length:0,value,tag:wire_tags::UNSIGNED,
        };
        let fields=[
            txt_field(function),txt_field(statement),txt_field(path),
            unsigned(144),unsigned(kind),unsigned(virtual_flag),
        ];
        unsafe {
            collect((state as *mut Collector).cast(),78,
                    fields.as_ptr(),fields.len(),0);
        }
    }

    fn source()->Il {
        let mut il=Il::default();
        il.calls.push(CallSite {
            function:"std::locale::facet".into(),
            statement_path:"0.block.2".into(),path:"0".into(),
            first_argument:2,
        });
        il.expressions.push(Expression {
            function:"std::locale::facet".into(),
            statement_path:"0.block.2".into(),path:"0".into(),
            parent:None,kind:3,operator:Some(144),
            type_kind:None,reference:None,type_size:None,
            type_alignment:None,constant_kind:None,constant_integer:None,
            related_type_role:None,related_type_kind:None,
            type_id:None,symbol_id:None,
        });
        il.call_dispatches.push(CallDispatchProvenance {
            function:"std::locale::facet".into(),
            statement_path:"0.block.2".into(),path:"0".into(),
            native_operator:144,class:CallDispatchClass::Member,
            explicitly_virtual:true,
        });
        il
    }

    #[test]
    fn native_virtual_bit_is_only_meaningful_for_explicit_member_calls() {
        let mut v=Collector::default();
        send(&mut v,1,1);
        assert!(!v.malformed);
        assert_eq!(v.il.call_dispatches.len(),1);
        assert_eq!(v.il.call_dispatches[0].class,CallDispatchClass::Member);
        assert!(v.il.call_dispatches[0].explicitly_virtual);
        let mut ordinary=Collector::default();
        send(&mut ordinary,0,0);
        assert!(!ordinary.malformed);
        assert_eq!(ordinary.il.call_dispatches[0].class,
                   CallDispatchClass::OrdinaryOrLowered);
        assert!(!ordinary.il.call_dispatches[0].explicitly_virtual);
        for (kind,flag) in [(0,1),(2,1),(3,0),(1,2)] {
            let mut bad=Collector::default();
            send(&mut bad,kind,flag);
            assert!(bad.malformed,"unsafe call dispatch ({kind},{flag}) admitted");
        }
    }

    #[test]
    fn duplicate_or_mismatched_call_opcodes_are_rejected_at_success_boundary() {
        let il=source();
        validate_native_call_dispatch_coverage(&il).unwrap();
        let mut missing=il.clone();
        missing.call_dispatches.clear();
        assert!(validate_native_call_dispatch_coverage(&missing).is_err());
        let mut duplicate=il.clone();
        duplicate.call_dispatches.push(duplicate.call_dispatches[0].clone());
        assert!(validate_native_call_dispatch_coverage(&duplicate).is_err());
        let mut wrong_op=il.clone();
        wrong_op.call_dispatches[0].native_operator=133;
        assert!(validate_native_call_dispatch_coverage(&wrong_op).is_err());
        let mut wrong_shape=il.clone();
        wrong_shape.call_dispatches[0].class=CallDispatchClass::OrdinaryOrLowered;
        wrong_shape.call_dispatches[0].explicitly_virtual=false;
        assert!(validate_native_call_dispatch_coverage(&wrong_shape).is_err());
        let mut incomplete=il.clone();
        incomplete.expressions.clear();
        assert!(validate_native_call_dispatch_coverage(&incomplete).is_err());
        let mut not_proven=il.clone();
        not_proven.call_dispatches[0].class=CallDispatchClass::Member;
        not_proven.call_dispatches[0].explicitly_virtual=false;
        assert!(validate_native_call_dispatch_coverage(&not_proven).is_ok(),
            "false source virtual bit cannot authorize or forbid lowered dispatch");
    }
}

#[cfg(test)]
mod retired_source_hook_wire_tests {
    use super::*;

    #[test]
    fn live_callback_rejects_all_retired_virtual_source_events() {
        // The legacy arities are still defined by the schema, so this
        // proves rejection is due to unsupported provenance, not bad arity.
        for kind in [80_u32, 81, 82, 85] {
            let arity = expected_wire_field_count(kind)
                .expect("historical wire kind must remain reserved");
            let values = vec![WireField {
                data: std::ptr::null(),
                length: 0,
                value: 0,
                tag: wire_tags::ABSENT,
            }; arity];
            let mut state = Collector::default();
            unsafe {
                collect(
                    (&mut state as *mut Collector).cast::<c_void>(),
                    kind,
                    values.as_ptr(),
                    values.len(),
                    0,
                );
            }
            assert!(state.malformed, "retired wire event {kind} was accepted");
            assert!(!state.panicked, "retired wire event {kind} panicked");
            assert!(state.il.prelower_virtual_origins.is_empty());
            assert!(state.il.virtual_receiver_reuses.is_empty());
            assert!(state.il.virtual_entry_indexes.is_empty());
            assert!(state.il.virtual_vptr_receiver_pairs.is_empty());
        }
    }
}

#[cfg(test)]
#[path="global_materialization_tests.rs"]
mod global_materialization_tests;

#[cfg(test)]
mod retained_native_eh_wire_tests {
    use super::*;
    fn text_field(s:&str)->WireField {
        WireField{data:s.as_ptr(),length:s.len(),value:0,tag:wire_tags::TEXT}
    }
    fn number(n:u32)->WireField {
        WireField{data:std::ptr::null(),length:0,value:u64::from(n),tag:wire_tags::UNSIGNED}
    }
    fn send(state:&mut Collector,role:u32,kind:u32,subkind:u32) {
        let fields=[
            text_field("function"),text_field("stmt"),number(role),
            text_field("0"),number(kind),number(subkind),
        ];
        unsafe {collect((state as *mut Collector).cast(),
                        106,fields.as_ptr(),fields.len(),0);}
    }
    fn expression()->Expression {
        Expression{
            function:"function".into(),statement_path:"stmt".into(),
            path:"0".into(),parent:None,kind:89,operator:None,
            type_kind:None,reference:None,type_size:None,
            type_alignment:None,constant_kind:None,
            constant_integer:None,related_type_role:None,
            related_type_kind:None,type_id:None,symbol_id:None,
        }
    }
    #[test]
    fn owned_nonportable_eh_action_preserves_native_subkind() {
        let mut state=Collector::default();
        state.il.expressions.push(expression());
        send(&mut state,0,89,11);
        assert!(!state.malformed && !state.panicked);
        assert_eq!(state.il.native_retained_eh_constructs,vec![
            NativeRetainedEhConstruct{
                function:"function".into(),statement_path:"stmt".into(),
                role:0,path:"0".into(),expression_kind:89,subkind:11,
            }
        ]);
        let mut control=Collector::default();
        control.il.control_expressions.push(ControlExpression{
            function:"function".into(),statement_path:"stmt".into(),
            role:2,path:"0".into(),parent:None,kind:89,
            operator:None,type_id:None,symbol_id:None,
        });
        send(&mut control,2,89,4);
        assert_eq!(control.il.native_retained_eh_constructs.len(),1);
        assert!(!control.malformed);
    }
    #[test]
    fn synthetic_duplicate_and_unrecognized_eh_action_fail_closed() {
        let mut missing=Collector::default();
        send(&mut missing,0,89,11);
        assert!(missing.malformed && !missing.panicked);
        let mut wrong=Collector::default();
        wrong.il.expressions.push(expression());
        send(&mut wrong,0,88,11);
        assert!(wrong.malformed && !wrong.panicked);
        let mut wrong=Collector::default();
        wrong.il.expressions.push(expression());
        send(&mut wrong,0,89,12);
        assert!(wrong.malformed && !wrong.panicked);
        let mut wrong=Collector::default();
        wrong.il.expressions.push(expression());
        send(&mut wrong,5,89,1);
        assert!(wrong.malformed && !wrong.panicked);
        let mut duplicate=Collector::default();
        duplicate.il.expressions.push(expression());
        send(&mut duplicate,0,89,11);
        send(&mut duplicate,0,89,11);
        assert!(duplicate.malformed && !duplicate.panicked);
    }
}

#[cfg(test)]
mod separate_file_scope_initializer_owner_wire_tests {
    use super::*;

    fn number(n:u64)->WireField {
        WireField{data:std::ptr::null(),length:0,value:n,tag:wire_tags::UNSIGNED}
    }
    fn send(state:&mut Collector,fields:&[WireField]) {
        unsafe{collect((state as *mut Collector).cast(),104,
                       fields.as_ptr(),fields.len(),0);}
    }
    #[test]
    fn native_owner_preserves_exact_variable_and_routine_ids() {
        let mut state=Collector::default();
        send(&mut state,&[number(1),number(50)]);
        send(&mut state,&[number(6),number(51)]);
        assert!(!state.panicked && !state.malformed);
        assert_eq!(state.il.file_scope_dynamic_init_owners,vec![
            FileScopeDynamicInitOwner{variable:SymbolId(1),routine:SymbolId(50)},
            FileScopeDynamicInitOwner{variable:SymbolId(6),routine:SymbolId(51)},
        ]);
    }
    #[test]
    fn native_owner_rejects_null_or_duplicate_source_variable() {
        for fields in [[0,5],[5,0],[0,0]] {
            let mut state=Collector::default();
            send(&mut state,&[number(fields[0]),number(fields[1])]);
            assert!(state.malformed && !state.panicked);
        }
        let mut state=Collector::default();
        send(&mut state,&[number(5),number(50)]);
        send(&mut state,&[number(5),number(51)]);
        assert!(state.malformed && !state.panicked);
    }
}

#[cfg(test)]
mod bootstrap_literal_global_wire_tests {
    use super::*;
    fn number(n:u64)->WireField {
        WireField{data:std::ptr::null(),length:0,value:n,tag:wire_tags::UNSIGNED}
    }
    fn signed(n:i64)->WireField {
        WireField{data:std::ptr::null(),length:0,value:n as u64,tag:wire_tags::SIGNED}
    }
    fn text(bytes:&[u8])->WireField {
        WireField{data:bytes.as_ptr(),length:bytes.len(),value:0,tag:wire_tags::TEXT}
    }
    fn send(state:&mut Collector,kind:u32,fields:&[WireField]) {
        unsafe{collect((state as *mut Collector).cast(),
                       kind,fields.as_ptr(),fields.len(),0);}
    }
    fn global_image(state:&mut Collector,symbol:u64,bytes:&[u8]) {
        let hex: String=bytes.iter().map(|b|format!("{b:02x}")).collect();
        send(state,51,&[number(symbol),number(60),text(hex.as_bytes())]);
    }
    #[test]
    fn decodes_complete_source_literal_and_distinct_relocatable_global_slots() {
        let mut state=Collector::default();
        // Source C: a static union containing pointers into original
        // constant objects. Equal identity is retained by the same pool ID.
        global_image(&mut state,7,&[0;24]);
        send(&mut state,102,&[number(1),number(44),text(b"486900")]);
        send(&mut state,102,&[number(2),number(45),text(b"42796500")]);
        send(&mut state,103,&[number(7),number(0),number(1),signed(0)]);
        send(&mut state,103,&[number(7),number(8),number(2),signed(0)]);
        send(&mut state,103,&[number(7),number(16),number(1),signed(0)]);
        assert!(!state.malformed && !state.panicked);
        assert_eq!(state.il.global_literal_objects[0],GlobalLiteralObject {
            id:1,ty:TypeId(44),bytes:b"Hi\0".to_vec(),
        });
        assert_eq!(state.il.global_literal_objects[1].bytes.as_slice(),b"Bye\0");
        assert_eq!(state.il.global_literal_relocations.len(),3);
        assert_eq!(state.il.global_literal_relocations[0],GlobalLiteralRelocation {
            symbol:SymbolId(7),offset:0,literal_id:1,addend:0,
        });
        assert_eq!(state.il.global_literal_relocations[2].literal_id,1);
    }
    #[test]
    fn duplicate_missing_or_malformed_source_literal_rejects_without_panic() {
        for image in [b"abcd".as_slice(),b"0".as_slice(),b"gg00".as_slice()] {
            let mut state=Collector::default();
            send(&mut state,102,&[number(1),number(44),text(image)]);
            assert!(state.malformed && !state.panicked);
        }
        let mut state=Collector::default();
        send(&mut state,102,&[number(2),number(44),text(b"6100")]);
        assert!(state.malformed);
        let mut state=Collector::default();
        send(&mut state,102,&[number(1),number(44),text(b"6100")]);
        send(&mut state,102,&[number(1),number(44),text(b"6200")]);
        assert!(state.malformed && !state.panicked);
        let mut state=Collector::default();
        global_image(&mut state,7,&[0;24]);
        send(&mut state,102,&[number(1),number(44),text(b"6100")]);
        send(&mut state,103,&[number(7),number(4),number(1),signed(0)]);
        assert!(state.malformed);
        let mut state=Collector::default();
        global_image(&mut state,7,&[0;24]);
        send(&mut state,102,&[number(1),number(44),text(b"6100")]);
        send(&mut state,103,&[number(7),number(8),number(1),signed(0)]);
        send(&mut state,103,&[number(7),number(8),number(1),signed(0)]);
        assert!(state.malformed && !state.panicked);
    }
    #[test]
    fn literal_relocation_must_own_existing_zeroed_aggregate_slot() {
        for scenario in 0..5 {
            let mut state=Collector::default();
            if scenario!=0 {
                let mut bytes=[0u8;16];
                if scenario==3 {bytes[8]=1;}
                global_image(&mut state,7,&bytes);
            }
            if scenario!=1 {
                send(&mut state,102,&[number(1),number(44),text(b"6100")]);
            }
            let (owner,offset)=match scenario {
                2 => (7,16), // beyond source image
                4 => (9,8), // no source-owned aggregate for this symbol
                _ => (7,8),
            };
            send(&mut state,103,&[
                number(owner),number(offset),number(1),signed(0)]);
            assert!(state.malformed && !state.panicked,"scenario {scenario}");
        }
    }
    #[test]
    fn literal_and_variable_relocations_cannot_share_one_slot() {
        for literal_first in [false,true] {
            let mut state=Collector::default();
            global_image(&mut state,7,&[0;16]);
            send(&mut state,102,&[number(1),number(44),text(b"6100")]);
            let literal=[number(7),number(8),number(1),signed(0)];
            let variable=[number(7),number(8),number(8),signed(0)];
            if literal_first {
                send(&mut state,103,&literal);
                send(&mut state,52,&variable);
            } else {
                send(&mut state,52,&variable);
                send(&mut state,103,&literal);
            }
            assert!(state.malformed && !state.panicked);
        }
    }
}

#[cfg(test)]
mod bootstrap_static_routine_relocation_wire_tests {
    use super::*;
    fn n(n:u64)->WireField {
        WireField {data:std::ptr::null(),length:0,value:n,tag:wire_tags::UNSIGNED}
    }
    fn signed(v:i64)->WireField {
        WireField {data:std::ptr::null(),length:0,value:v as u64,tag:wire_tags::SIGNED}
    }
    fn t(v:&[u8])->WireField {
        WireField {data:v.as_ptr(),length:v.len(),value:0,tag:wire_tags::TEXT}
    }
    fn send(state:&mut Collector,kind:u32,fields:&[WireField]){
        unsafe {collect((state as *mut Collector).cast(),
                        kind,fields.as_ptr(),fields.len(),0);}
    }
    fn setup()->Collector {
        let mut state=Collector::default();
        send(&mut state,13,&[n(9),n(1),t(b"bootstrap_routine_target")]);
        let hex=vec![b'0';64];
        send(&mut state,51,&[n(7),n(70),t(&hex)]);
        assert!(!state.malformed && !state.panicked);
        state
    }
    #[test]
    fn source_routine_relocation_preserves_target_identity_and_zeroed_slot() {
        let mut state=setup();
        send(&mut state,105,&[n(7),n(0),n(9),signed(0)]);
        assert!(!state.malformed && !state.panicked);
        assert_eq!(state.il.global_routine_relocations,
                   vec![GlobalRoutineRelocation {
                       symbol:SymbolId(7),offset:0,
                       routine:SymbolId(9),addend:0,
                   }]);
    }
    #[test]
    fn unknown_duplicate_unaligned_nonzero_and_conflicting_slots_reject() {
        for scenario in 0..6 {
            let mut state=setup();
            match scenario {
                0=>send(&mut state,105,&[n(7),n(0),n(111),signed(0)]),
                1=>send(&mut state,105,&[n(7),n(1),n(9),signed(0)]),
                2=>send(&mut state,105,&[n(7),n(32),n(9),signed(0)]),
                3=>{
                    send(&mut state,105,&[n(7),n(0),n(9),signed(0)]);
                    send(&mut state,105,&[n(7),n(0),n(9),signed(0)]);
                },
                4=>{
                    send(&mut state,105,&[n(7),n(0),n(9),signed(0)]);
                    send(&mut state,52,&[n(7),n(0),n(12),signed(0)]);
                },
                5=>{
                    send(&mut state,105,&[n(7),n(0),n(9),signed(0)]);
                    send(&mut state,103,&[n(7),n(0),n(1),signed(0)]);
                },
                _=>unreachable!(),
            }
            assert!(state.malformed && !state.panicked,
                    "source function relocation scenario {scenario} must reject");
        }
    }
}
