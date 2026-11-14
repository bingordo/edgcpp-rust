//! Indexed, owned view of the EDG statement and expression snapshots.
//! IDs are stable within one `Graph`, not across compilations.
use std::collections::HashMap;
// The bridge normalizes configuration-dependent EDG operation numbers.
// Record 18 must describe the native call operator, not an arbitrary tree.
extern "C" {
    fn edgcpp_codegen_kind(category: u32, value: u32) -> u32;
}

/// Normalize EDG's configuration-dependent statement enum. Used to ensure
/// that native list-owner statements cannot lose an empty-list witness.
pub(crate) fn native_statement_kind(kind: u32) -> u32 {
    unsafe { edgcpp_codegen_kind(1, kind) }
}

/// EDG-native expression kind normalization, including GNU statement expressions.
pub(crate) fn native_expression_kind(kind: u32) -> u32 {
    unsafe { edgcpp_codegen_kind(2, kind) }
}

/// EDG-native variable initialization category: 1 means initk_dynamic.
pub(crate) fn native_variable_dynamic_init(kind: u32) -> bool {
    unsafe { edgcpp_codegen_kind(6, kind) == 1 }
}

fn native_call_argument_offset(expr: &Expression) -> Option<u32> {
    let op = expr.operator?;
    match unsafe { edgcpp_codegen_kind(3, op) } {
        54 if unsafe { edgcpp_codegen_kind(2, expr.kind) } == 3 => Some(1),
        55 if unsafe { edgcpp_codegen_kind(2, expr.kind) } == 3 => Some(2),
        _ => None,
    }
}

use crate::{VlaDimension, Il, Statement, StatementAttributes, ControlExpression, StatementLabel, StatementDeclaration, StatementInitialization, SourceLocation, StatementSource, StatementSwitchCase, SymbolSource, ExpressionSource, RoutineIdentity, ObjectLifetime, ScopeLifetime, InitializationLifetime, LifetimeDestruction, FunctionParameter, LocalScope, LocalVariable, StaticLocalOwner, Expression, TypeDef, TypeId, TypeParameter, ArrayBound, RecordField, BaseClass, BaseClassAccess, ConstructorInitializer, ConstructorInitializerExpression, MemberFunction, RoutineAttributes, TypeDetail, FunctionSignature, Symbol, SymbolId, SymbolType, VariableInitializer, InitializerInteger, DynamicInitializer, InitializerExpression, CallSite};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StatementId(pub usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExpressionId(pub usize);

#[derive(Clone, Debug)]
pub struct StatementNode {
    pub data: Statement,
    pub parent: Option<StatementId>,
    pub children: Vec<StatementId>,
    pub expression: Option<ExpressionId>,
}

#[derive(Clone, Debug)]
pub struct ExpressionNode {
    pub data: Expression,
    pub parent: Option<ExpressionId>,
    pub operands: Vec<ExpressionId>,
    pub statement: StatementId,
}

/// A lowered indirect callee's *observed* expression-tree reference to a
/// field named "__vptr", verified against its EDG record-field identity.
/// Neither a class ABI certificate nor proof the source call was virtual.
/// Literal values here are un-interpreted EDG expression constants, NOT
/// virtual slots, byte displacements, vtable address points or this offsets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalleeVptrFieldReference {
    pub field: SymbolId,
    pub owner: TypeId,
    pub field_path: String,
    pub field_byte_offset: u64,
    pub raw_integer_literals: Vec<(String, i64)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GraphError {
    DuplicateStatement { function: String, path: String },
    InvalidStatementLabel { function: String, path: String },
    DuplicateStatementLabel { function: String, path: String },
    DuplicateLabelDefinition(SymbolId),
    DuplicateStatementAttributes { function: String, path: String },
    InvalidStatementDeclaration { function: String, path: String },
    DuplicateStatementDeclaration { function: String, path: String, position: u32 },
    InvalidStatementDeclarationPosition { function: String, path: String },
    InvalidVlaDimension { function: String, path: String },
    DuplicateVlaDimension { function: String, path: String },
    InvalidStatementInitialization { function: String, path: String },
    InvalidSourceLocation { function: String, path: String },
    DuplicateSourceLocation { function: String, path: String },
    InvalidSymbolSource(SymbolId),
    DuplicateSymbolSource(SymbolId),
    InvalidLifetime { routine: SymbolId, id: u32 },
    DuplicateLifetime { routine: SymbolId, id: u32 },
    InvalidScopeLifetime { routine: SymbolId, scope: u32 },
    DuplicateScopeLifetime { routine: SymbolId, scope: u32 },
    InvalidInitializationLifetime { function: String, path: String },
    DuplicateInitializationLifetime { function: String, path: String },
    InvalidLifetimeDestruction { routine: SymbolId, lifetime: u32 },
    DuplicateLifetimeDestruction { routine: SymbolId, lifetime: u32, position: u32 },
    InvalidLifetimeDestructionPosition { routine: SymbolId, lifetime: u32 },
    DuplicateStatementInitialization { function: String, path: String },
    InvalidStatementAttributes { function: String, path: String },
    DuplicateControlExpression { function: String, statement: String, role: u32, path: String },
    InvalidControlExpression { function: String, statement: String, role: u32, path: String },
    MissingControlExpressionRoot { function: String, statement: String, role: u32 },
    InvalidControlExpressionOperandPath { function: String, statement: String, role: u32, path: String },
    MissingStatement { function: String, path: String },
    DuplicateExpression { function: String, statement: String, path: String },
    MissingExpression { function: String, statement: String, path: String },
    MultipleStatementExpressions { function: String, path: String },
    InvalidStatementParent { function: String, path: String },
    InvalidExpressionParent { function: String, statement: String, path: String },
    DuplicateType(TypeId),
    MissingType(TypeId),
    InvalidTypeId(TypeId),
    DuplicateParameter { owner: TypeId, position: u32 },
    InvalidTypeParameterPosition(TypeId),
    InvalidTypeParameterOwner(TypeId),
    InvalidFunctionSignatureOwner(TypeId),
    MissingFunctionSignature(TypeId),
    MissingRoutineSupplement(TypeId),
    InvalidRoutineParameterCount(TypeId),
    MissingNativeRecordAdmission(TypeId),
    MissingNativeRecordShape(TypeId),
    MissingNativeTypeLayoutProof(TypeId),
    MissingNativeScalarMetadataProof(TypeId),
    InvalidNativeScalarMetadataProof(TypeId),
    DuplicateNativeScalarMetadataProof(TypeId),
    InvalidNativeTypeLayoutProof(TypeId),
    DuplicateNativeTypeLayoutProof(TypeId),
    InvalidNativeRecordShape(TypeId),
    DuplicateNativeRecordShape(TypeId),
    MissingNativeLifetimeAdmission(TypeId),
    InvalidNativeRecordAdmission(TypeId),
    InvalidNativeLifetimeAdmission(TypeId),
    InvalidRoutineSupplement(TypeId),
    DuplicateRoutineSupplement(TypeId),
    DuplicateArrayBound(TypeId),
    InvalidArrayBoundOwner(TypeId),
    InvalidTypeDetailOwner(TypeId),
    InvalidTypeDetailFlags(TypeId),
    MissingTypeDetail(TypeId),
    MissingNativeTypeRelation(TypeId),
    DuplicateRecordField { owner: TypeId, position: u32 },
    InvalidRecordFieldOwner(TypeId),
    DuplicateRecordFieldSymbol { owner: TypeId, symbol: SymbolId },
    InvalidRecordFieldBitOffset { owner: TypeId, position: u32 },
    InvalidRecordFieldPosition(TypeId),
    InvalidRecordFieldSymbol { owner: TypeId, symbol: SymbolId },
    DuplicateBaseClass { derived: TypeId, position: u32 },
    InvalidBaseClassAccess { derived: TypeId, position: u32 },
    DuplicateBaseClassAccess { derived: TypeId, position: u32 },
    InvalidBaseClass { derived: TypeId, base: TypeId },
    InvalidBaseClassPosition(TypeId),
    CyclicInheritance(TypeId),
    InvalidConstructorInitializer { routine: SymbolId, position: u32 },
    DuplicateConstructorExpression { routine: SymbolId, position: u32, path: String },
    InvalidConstructorExpression { routine: SymbolId, position: u32, path: String },
    MissingConstructorExpressionRoot { routine: SymbolId, position: u32 },
    InvalidConstructorExpressionOperandPath { routine: SymbolId, position: u32, path: String },
    InvalidMemberFunction { owner: TypeId, routine: SymbolId },
    DuplicateMemberFunction(SymbolId),
    InvalidRoutineAttributes(SymbolId),
    DuplicateRoutineAttributes(SymbolId),
    DuplicateTypeDetail(TypeId),
    DuplicateFunctionSignature(TypeId),
    InvalidTypeRelation(TypeId),
    DuplicateSymbol(SymbolId),
    InvalidRoutineIdentity(SymbolId),
    DuplicateRoutineIdentity(SymbolId),
    DuplicateRoutineFunction(String),
    DuplicateSymbolType(SymbolId),
    InvalidFunctionParameter { routine: SymbolId, symbol: SymbolId },
    DuplicateFunctionParameter { routine: SymbolId, position: Option<u32> },
    InvalidFunctionParameterPosition(SymbolId),
    DuplicateLocalScope { routine: SymbolId, scope: u32 },
    InvalidLocalScope { routine: SymbolId, scope: u32 },
    DuplicateLocalVariable(SymbolId),
    InvalidLocalVariable(SymbolId),
    InvalidStaticLocalOwner(SymbolId),
    DuplicateStaticLocalOwner(SymbolId),
    InvalidSymbolType(SymbolId),
    MissingSymbol(SymbolId),
    InvalidInitializerSymbol(SymbolId),
    DuplicateVariableInitializer(SymbolId),
    DuplicateInitializerInteger(SymbolId),
    DuplicateDynamicInitializer(SymbolId),
    DuplicateInitializerExpression { owner: SymbolId, path: String },
    InvalidInitializerParent { owner: SymbolId, path: String },
    MissingInitializerRoot(SymbolId),
    InvalidInitializerOperandPath { owner: SymbolId, path: String },
    MissingCallExpression { function: String, statement: String, path: String },
    MissingCallMetadata { function: String, statement: String, path: String },
    DuplicateCallExpression { function: String, statement: String, path: String },
    InvalidCallOperands { function: String, statement: String, path: String },
    InvalidCallOperandPath { function: String, statement: String, path: String },
    InvalidExpressionOperandPath { function: String, statement: String, path: String },
    InvalidImplicitPointerCast { function: String, statement: String, path: String },
}

/// A dense-ID graph retaining EDG's native kinds and the original paths.
#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub statements: Vec<StatementNode>,
    pub statement_attributes: Vec<StatementAttributes>,
    pub control_expressions: Vec<ControlExpression>,
    pub statement_labels: Vec<StatementLabel>,
    pub statement_switch_cases: Vec<StatementSwitchCase>,
    pub statement_declarations: Vec<StatementDeclaration>,
    pub vla_dimensions: Vec<VlaDimension>,
    pub statement_initializations: Vec<StatementInitialization>,
    pub statement_sources: Vec<StatementSource>,
    pub symbol_sources: Vec<SymbolSource>,
    pub expression_sources: Vec<ExpressionSource>,
    pub object_lifetimes: Vec<ObjectLifetime>,
    pub scope_lifetimes: Vec<ScopeLifetime>,
    pub initialization_lifetimes: Vec<InitializationLifetime>,
    pub lifetime_destructions: Vec<LifetimeDestruction>,
    pub function_parameters: Vec<FunctionParameter>,
    pub local_scopes: Vec<LocalScope>,
    pub local_variables: Vec<LocalVariable>,
    pub static_local_owners: Vec<StaticLocalOwner>,
    pub expressions: Vec<ExpressionNode>,
    pub types: Vec<TypeDef>,
    pub type_parameters: Vec<TypeParameter>,
    pub array_bounds: Vec<ArrayBound>,
    pub record_fields: Vec<RecordField>,
    pub base_classes: Vec<BaseClass>,
    pub base_class_access: Vec<BaseClassAccess>,
    pub constructor_initializers: Vec<ConstructorInitializer>,
    pub constructor_initializer_expressions: Vec<ConstructorInitializerExpression>,
    pub member_functions: Vec<MemberFunction>,
    pub routine_attributes: Vec<RoutineAttributes>,
    pub type_details: Vec<TypeDetail>,
    pub function_signatures: Vec<FunctionSignature>,
    pub symbols: Vec<Symbol>,
    pub routine_identities: Vec<RoutineIdentity>,
    pub symbol_types: Vec<SymbolType>,
    pub variable_initializers: Vec<VariableInitializer>,
    pub initializer_integers: Vec<InitializerInteger>,
    pub dynamic_initializers: Vec<DynamicInitializer>,
    pub initializer_expressions: Vec<InitializerExpression>,
    pub calls: Vec<CallSite>,
    implicit_pointer_cast_source_by_expression: HashMap<ExpressionId, TypeId>,
    routine_by_function: HashMap<String, SymbolId>,
    routine_function_by_id: HashMap<SymbolId, String>,
    label_definitions: HashMap<SymbolId, StatementId>,
    statement_label_index: HashMap<StatementId, usize>,
    lifetime_index: HashMap<(SymbolId, u32), usize>,
    scope_lifetime_index: HashMap<(SymbolId, u32), usize>,
    initialization_lifetime_index: HashMap<(String, String), usize>,
    lifetime_destruction_index: HashMap<(SymbolId, u32), Vec<usize>>,
    statement_source_index: HashMap<(String, String), usize>,
    symbol_source_index: HashMap<SymbolId, usize>,
    expression_source_index: HashMap<(String, String, u32, String), usize>,
    control_expr_index: HashMap<(String, String, u32, String), usize>,
    control_expr_children: HashMap<(String, String, u32, String), Vec<usize>>,
    control_expr_by_statement: HashMap<(String, String), Vec<usize>>,
    constructor_expr_index: HashMap<(SymbolId, u32, String), usize>,
    constructor_expr_children: HashMap<(SymbolId, u32, String), Vec<usize>>,
    initializer_index: HashMap<(SymbolId, String), usize>,
    initializer_children: HashMap<(SymbolId, String), Vec<usize>>,
    // Resolve a native call's precise owner expression without a linear
    // scan of all call-site records. Keep borrowed/equal CallSite queries.
    call_expression_by_site: HashMap<CallSite, ExpressionId>,
    // Dense ExpressionId -> call record index. Avoid O(expressions * calls)
    // string-path scans during source expression lowering.
    call_index_by_expression: Vec<Option<usize>>,
}

/// Borrowed expression captured within a function body.
#[derive(Debug, Clone, Copy)]
pub enum FunctionExpressionRef<'a> {
    Primary(&'a ExpressionNode),
    Control(&'a ControlExpression),
}

/// Borrowed expression across function bodies and initializer trees.
#[derive(Debug, Clone, Copy)]
pub enum AnyExpressionRef<'a> {
    Primary(&'a ExpressionNode),
    Control(&'a ControlExpression),
    VariableInitializer(&'a InitializerExpression),
    ConstructorInitializer(&'a ConstructorInitializerExpression),
}

impl AnyExpressionRef<'_> {
    /// Referenced declaration, if this expression names one.
    pub fn symbol_id(self) -> Option<SymbolId> {
        match self {
            Self::Primary(node) => node.data.symbol_id,
            Self::Control(node) => node.symbol_id,
            Self::VariableInitializer(node) => node.symbol_id,
            Self::ConstructorInitializer(node) => node.symbol_id,
        }
    }

    /// Expression's EDG kind discriminator.
    pub fn kind(self) -> u32 {
        match self {
            Self::Primary(node) => node.data.kind,
            Self::Control(node) => node.kind,
            Self::VariableInitializer(node) => node.kind,
            Self::ConstructorInitializer(node) => node.kind,
        }
    }

    /// Operator discriminator, when supplied by EDG.
    pub fn operator(self) -> Option<u32> {
        match self {
            Self::Primary(node) => node.data.operator,
            Self::Control(node) => node.operator,
            Self::VariableInitializer(node) => node.operator,
            Self::ConstructorInitializer(node) => node.operator,
        }
    }

    /// Type associated with the expression, if available.
    pub fn type_id(self) -> Option<TypeId> {
        match self {
            Self::Primary(node) => node.data.type_id,
            Self::Control(node) => node.type_id,
            Self::VariableInitializer(node) => node.type_id,
            Self::ConstructorInitializer(node) => node.type_id,
        }
    }
}

/// Counts of captured expression kinds in a routine or initializer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExpressionStats {
    pub primary: usize,
    pub control: usize,
    pub variable_initializers: usize,
    pub constructor_initializers: usize,
    /// Number of expression nodes with a symbol reference (including repeats).
    pub symbol_uses: usize,
    /// Number of distinct referenced symbols.
    pub distinct_symbols: usize,
}

impl ExpressionStats {
    pub fn total(&self) -> usize {
        self.primary + self.control + self.variable_initializers + self.constructor_initializers
    }
}

/// A symbol and its number of occurrences, ordered by first use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SymbolUseCount {
    pub symbol: SymbolId,
    pub count: usize,
}

/// Borrowed summary of a type's captured EDG metadata.
#[derive(Clone, Copy, Debug)]
pub struct TypeView<'a> {
    pub definition: &'a TypeDef,
    pub detail: Option<&'a TypeDetail>,
    pub signature: Option<&'a FunctionSignature>,
    pub array_elements: Option<u64>,
}

/// A record's captured direct base or nonstatic field.
#[derive(Clone, Copy, Debug)]
pub enum RecordLayoutItem<'a> {
    Base(&'a BaseClass),
    Field(&'a RecordField),
}

/// Borrowed view of an EDG routine declaration.
#[derive(Clone, Copy, Debug)]
pub struct RoutineView<'a> {
    pub symbol: &'a Symbol,
    pub function: Option<&'a str>,
    pub ty: Option<TypeView<'a>>,
    pub implicit_this: Option<&'a FunctionParameter>,
}

/// Borrowed view of an EDG variable declaration.
#[derive(Clone, Copy, Debug)]
pub struct VariableView<'a> {
    pub symbol: &'a Symbol,
    pub ty: Option<TypeView<'a>>,
    pub initializer: Option<&'a VariableInitializer>,
}

/// Direct captured type relationship. These are structural references,
/// not a statement about ABI layout or ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeDependencyKind {
    Related,
    Base,
    Field,
    Parameter,
}

/// One direct edge in the captured type graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypeDependency {
    pub target: TypeId,
    pub kind: TypeDependencyKind,
}

/// A type reachable through captured type relationships, with its shortest
/// discovered edge distance from the starting type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReachableType {
    pub ty: TypeId,
    pub depth: usize,
}

/// One ordered mutation of a reusable type dependency index.
/// Within a batch, later changes to the same source or symbol win.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeIndexChange {
    /// Set the complete outgoing dependency list of a type.
    ReplaceType { ty: TypeId, dependencies: Vec<TypeDependency> },
    /// Delete the type definition's outgoing edges; incoming edges remain.
    RemoveType { ty: TypeId },
    /// Change or remove the declared type of a symbol.
    SetSymbolType { symbol: SymbolId, ty: Option<TypeId> },
}

/// Changes in membership of the conservative invalidation closure.
/// Depths in before/after and newly/no-longer affected types are the
/// shortest reverse-edge distances in the corresponding index state.
/// Types present on both sides remain affected even if their depth changes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TypeInvalidationDiff {
    pub before_types: Vec<ReachableType>,
    pub after_types: Vec<ReachableType>,
    pub newly_affected_types: Vec<ReachableType>,
    pub no_longer_affected_types: Vec<ReachableType>,
    pub newly_affected_symbols: Vec<SymbolId>,
    pub no_longer_affected_symbols: Vec<SymbolId>,
}

/// Reusable snapshot of forward/reverse type edges and symbol ownership.
/// Construct after the Graph is populated; rebuild after mutating its public
/// collections. Queries never rescan Graph's type relationships.
#[derive(Clone, Debug, Default)]
pub struct TypeDependencyIndex {
    forward: HashMap<TypeId, Vec<TypeDependency>>,
    reverse: HashMap<TypeId, Vec<TypeId>>,
    symbols_by_type: HashMap<TypeId, Vec<SymbolId>>,
    symbol_order: Vec<(SymbolId, TypeId)>,
    type_order: Vec<TypeId>,
}

impl TypeDependencyIndex {
    /// Replace the outgoing relationships for one type.
    /// Duplicate forward edges are retained, but reverse edges are unique.
    pub fn replace_type_dependencies(&mut self, ty: TypeId, edges: Vec<TypeDependency>) {
        self.apply_changes(&[TypeIndexChange::ReplaceType { ty, dependencies: edges }]);
    }

    /// Remove a captured type's outgoing edges. Incoming references remain.
    /// Returns false if this source type is not present.
    pub fn remove_type(&mut self, ty: TypeId) -> bool {
        if !self.forward.contains_key(&ty) { return false; }
        self.apply_changes(&[TypeIndexChange::RemoveType { ty }]);
        true
    }

    /// Update a symbol's declared type. Existing symbols retain their
    /// captured order; newly associated symbols are appended.
    pub fn set_symbol_type(&mut self, symbol: SymbolId, ty: Option<TypeId>) {
        self.apply_changes(&[TypeIndexChange::SetSymbolType { symbol, ty }]);
    }

    /// Apply an ordered batch of mutations with a single reverse-edge
    /// refresh. Only reverse adjacency lists touched by a changed type are
    /// rebuilt; other reverse lists and outgoing edge lists are unchanged.
    /// The index is updated, not the Graph that originally supplied it.
    pub fn apply_changes(&mut self, changes: &[TypeIndexChange]) {
        let mut changed_sources = std::collections::HashSet::new();
        let mut touched_targets = std::collections::HashSet::new();
        let mut touched_symbol_types = std::collections::HashSet::new();

        // Index only multi-edit batches. Single edits keep the existing
        // low-allocation path; a large batch must not linearly search or
        // shift the ordered vectors once per changed symbol/type.
        let index_symbols = changes.iter().filter(|change|
            matches!(change, TypeIndexChange::SetSymbolType { .. })).count() > 1;
        let index_types = changes.len() > 1 && changes.iter().any(|change|
            matches!(change, TypeIndexChange::RemoveType { .. }));
        let mut symbol_slots: Vec<Option<(SymbolId, TypeId)>> = Vec::new();
        let mut symbol_positions: HashMap<SymbolId, usize> = HashMap::new();
        if index_symbols {
            symbol_slots = std::mem::take(&mut self.symbol_order)
                .into_iter().map(Some).collect();
            symbol_positions.extend(symbol_slots.iter().enumerate()
                .filter_map(|(i, slot)| slot.map(|(symbol, _)| (symbol, i))));
        }
        let mut type_slots: Vec<Option<TypeId>> = Vec::new();
        let mut type_positions: HashMap<TypeId, usize> = HashMap::new();
        if index_types {
            type_slots = std::mem::take(&mut self.type_order)
                .into_iter().map(Some).collect();
            type_positions.extend(type_slots.iter().enumerate()
                .filter_map(|(i, slot)| slot.map(|ty| (ty, i))));
        }
        for change in changes {
            match change {
                TypeIndexChange::ReplaceType { ty, dependencies } => {
                    if !self.forward.contains_key(ty) {
                        if index_types {
                            type_positions.insert(*ty, type_slots.len());
                            type_slots.push(Some(*ty));
                        } else {
                            self.type_order.push(*ty);
                        }
                    }
                    if let Some(previous) = self.forward.insert(*ty, dependencies.clone()) {
                        touched_targets.extend(previous.into_iter().map(|edge| edge.target));
                    }
                    touched_targets.extend(dependencies.iter().map(|edge| edge.target));
                    changed_sources.insert(*ty);
                }
                TypeIndexChange::RemoveType { ty } => {
                    if let Some(previous) = self.forward.remove(ty) {
                        if index_types {
                            if let Some(position) = type_positions.remove(ty) {
                                type_slots[position] = None;
                            }
                        } else {
                            self.type_order.retain(|source| source != ty);
                        }
                        touched_targets.extend(previous.into_iter().map(|edge| edge.target));
                        changed_sources.insert(*ty);
                    }
                }
                TypeIndexChange::SetSymbolType { symbol, ty } => {
                    if index_symbols {
                        if let Some(&position) = symbol_positions.get(symbol) {
                            let (_, old_ty) = symbol_slots[position]
                                .expect("live symbol position");
                            touched_symbol_types.insert(old_ty);
                            if let Some(new_ty) = ty {
                                symbol_slots[position] = Some((*symbol, *new_ty));
                            } else {
                                symbol_slots[position] = None;
                                symbol_positions.remove(symbol);
                            }
                        } else if let Some(new_ty) = ty {
                            symbol_positions.insert(*symbol, symbol_slots.len());
                            symbol_slots.push(Some((*symbol, *new_ty)));
                        }
                    } else if let Some(position) = self.symbol_order.iter()
                        .position(|(id, _)| id == symbol)
                    {
                        touched_symbol_types.insert(self.symbol_order[position].1);
                        if let Some(new_ty) = ty {
                            self.symbol_order[position].1 = *new_ty;
                        } else {
                            self.symbol_order.remove(position);
                        }
                    } else if let Some(new_ty) = ty {
                        self.symbol_order.push((*symbol, *new_ty));
                    }
                    if let Some(new_ty) = ty {
                        touched_symbol_types.insert(*new_ty);
                    }
                }
            }
        }
        if index_symbols {
            self.symbol_order = symbol_slots.into_iter().flatten().collect();
        }
        if index_types {
            self.type_order = type_slots.into_iter().flatten().collect();
        }
        if !changed_sources.is_empty() {
            let ranks: HashMap<TypeId, usize> = self.type_order.iter()
                .copied().enumerate().map(|(rank, ty)| (ty, rank)).collect();
            // Index the new outgoing edges once rather than checking every
            // changed source against every touched reverse destination.
            let mut new_reverse: HashMap<TypeId, Vec<TypeId>> = HashMap::new();
            for &source in &changed_sources {
                let mut emitted = std::collections::HashSet::new();
                for edge in self.dependencies(source) {
                    if emitted.insert(edge.target) {
                        new_reverse.entry(edge.target).or_default().push(source);
                    }
                }
            }
            for target in touched_targets {
                let mut sources = self.reverse.remove(&target).unwrap_or_default();
                sources.retain(|source| !changed_sources.contains(source));
                if let Some(additions) = new_reverse.get(&target) {
                    sources.extend(additions.iter().copied());
                }
                sources.sort_by_key(|source| ranks.get(source).copied().unwrap_or(usize::MAX));
                sources.dedup();
                if !sources.is_empty() {
                    self.reverse.insert(target, sources);
                }
            }
        }
        // Refresh only buckets whose symbol/type associations changed.
        if !touched_symbol_types.is_empty() {
            for &ty in &touched_symbol_types {
                self.symbols_by_type.remove(&ty);
            }
            for &(symbol, ty) in &self.symbol_order {
                if touched_symbol_types.contains(&ty) {
                    self.symbols_by_type.entry(ty).or_default().push(symbol);
                }
            }
        }
    }

    /// Apply a batch and report changes to invalidation membership from
    /// the same seed IDs, comparing the index before and after the batch.
    /// Seeds are explicit: edits alone do not infer which types changed.
    /// Retained members with changed distances are not "newly affected".
    pub fn apply_changes_with_diff(
        &mut self, changes: &[TypeIndexChange], changed: &[TypeId],
    ) -> TypeInvalidationDiff {
        let before_types = self.affected_types(changed);
        let before_symbols = self.affected_symbols(changed);
        self.apply_changes(changes);
        let after_types = self.affected_types(changed);
        let after_symbols = self.affected_symbols(changed);

        let old_set: std::collections::HashSet<TypeId> =
            before_types.iter().map(|entry| entry.ty).collect();
        let new_set: std::collections::HashSet<TypeId> =
            after_types.iter().map(|entry| entry.ty).collect();
        let old_symbols: std::collections::HashSet<SymbolId> =
            before_symbols.iter().copied().collect();
        let new_symbols: std::collections::HashSet<SymbolId> =
            after_symbols.iter().copied().collect();

        let newly_affected_types = after_types.iter().copied()
            .filter(|entry| !old_set.contains(&entry.ty)).collect();
        let no_longer_affected_types = before_types.iter().copied()
            .filter(|entry| !new_set.contains(&entry.ty)).collect();
        let newly_affected_symbols = after_symbols.into_iter()
            .filter(|symbol| !old_symbols.contains(symbol)).collect();
        let no_longer_affected_symbols = before_symbols.into_iter()
            .filter(|symbol| !new_symbols.contains(symbol)).collect();

        TypeInvalidationDiff {
            before_types, after_types, newly_affected_types,
            no_longer_affected_types, newly_affected_symbols,
            no_longer_affected_symbols,
        }
    }

    /// Symbols with the requested declared type, in captured symbol order.
    pub fn symbols_with_type(&self, ty: TypeId) -> &[SymbolId] {
        self.symbols_by_type.get(&ty).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Direct captured dependencies in the same order as Graph queries.
    pub fn dependencies(&self, ty: TypeId) -> &[TypeDependency] {
        self.forward.get(&ty).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Direct dependent type IDs, deduplicated in captured definition order.
    pub fn dependents(&self, ty: TypeId) -> &[TypeId] {
        self.reverse.get(&ty).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Reverse dependency closure from multiple seeds. Shortest distances
    /// and first-encounter ordering are preserved.
    pub fn affected_types(&self, changed: &[TypeId]) -> Vec<ReachableType> {
        let mut seen = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        let mut result = Vec::new();
        for &ty in changed {
            if seen.insert(ty) {
                queue.push_back(ReachableType { ty, depth: 0 });
            }
        }
        while let Some(node) = queue.pop_front() {
            for &ty in self.dependents(node.ty) {
                if seen.insert(ty) {
                    queue.push_back(ReachableType { ty, depth: node.depth + 1 });
                }
            }
            result.push(node);
        }
        result
    }

    /// Compute dependency-first invalidation batches using SCC condensation.
    /// Each batch contains a strongly connected component; components are
    /// emitted in topological order, with dependencies before dependents.
    /// Uncaptured changed IDs are treated as singleton components.
    pub fn invalidation_schedule(&self, changed: &[TypeId]) -> Vec<Vec<TypeId>> {
        let affected = self.affected_types(changed);
        if affected.is_empty() { return Vec::new(); }
        let nodes: Vec<TypeId> = affected.iter().map(|entry| entry.ty).collect();
        let included: std::collections::HashSet<TypeId> = nodes.iter().copied().collect();

        // Iterative Kosaraju pass one: genuine DFS postorder.
        let mut seen = std::collections::HashSet::new();
        let mut finish = Vec::new();
        for &root in &nodes {
            if !seen.insert(root) { continue; }
            let mut stack = vec![(root, 0usize)];
            while let Some((node, next)) = stack.last_mut() {
                let edges = self.dependencies(*node);
                if *next < edges.len() {
                    let target = edges[*next].target;
                    *next += 1;
                    if included.contains(&target) && seen.insert(target) {
                        stack.push((target, 0));
                    }
                } else {
                    finish.push(*node);
                    stack.pop();
                }
            }
        }

        let mut component_of = HashMap::new();
        let mut components: Vec<Vec<TypeId>> = Vec::new();
        seen.clear();
        for &root in finish.iter().rev() {
            if !seen.insert(root) { continue; }
            let component_id = components.len();
            let mut members = Vec::new();
            let mut stack = vec![root];
            while let Some(node) = stack.pop() {
                component_of.insert(node, component_id);
                members.push(node);
                for &parent in self.dependents(node) {
                    if included.contains(&parent) && seen.insert(parent) {
                        stack.push(parent);
                    }
                }
            }
            components.push(members);
        }

        // Build condensed dependency -> dependent DAG.
        let mut successors = vec![std::collections::HashSet::new(); components.len()];
        let mut indegree = vec![0usize; components.len()];
        for &node in &nodes {
            let dependent = component_of[&node];
            for edge in self.dependencies(node) {
                if let Some(&dependency) = component_of.get(&edge.target) {
                    if dependency != dependent && successors[dependency].insert(dependent) {
                        indegree[dependent] += 1;
                    }
                }
            }
        }

        // Stable tie-breaking by initial affected-type discovery order.
        let mut rank = vec![usize::MAX; components.len()];
        let mut node_rank = HashMap::new();
        for (position, &node) in nodes.iter().enumerate() {
            rank[component_of[&node]] = rank[component_of[&node]].min(position);
            node_rank.insert(node, position);
        }
        let mut ready: std::collections::BTreeSet<(usize, usize)> = indegree.iter()
            .enumerate()
            .filter(|(_, degree)| **degree == 0)
            .map(|(id, _)| (rank[id], id))
            .collect();
        let mut schedule = Vec::new();
        while let Some(&(priority, id)) = ready.iter().next() {
            ready.remove(&(priority, id));
            let mut members = components[id].clone();
            members.sort_by_key(|ty| node_rank.get(ty).copied().unwrap_or(usize::MAX));
            schedule.push(members);
            for &next in &successors[id] {
                indegree[next] -= 1;
                if indegree[next] == 0 {
                    ready.insert((rank[next], next));
                }
            }
        }
        schedule
    }

    /// Compute parallel-ready waves of SCC batches. All batches in one wave
    /// have no dependencies on each other within the affected subgraph.
    /// A later wave depends only on preceding waves. The inner Vec contains
    /// mutually dependent types, which must be handled together.
    pub fn invalidation_waves(&self, changed: &[TypeId]) -> Vec<Vec<Vec<TypeId>>> {
        let schedule = self.invalidation_schedule(changed);
        if schedule.is_empty() { return Vec::new(); }
        let mut component_of = HashMap::new();
        for (id, component) in schedule.iter().enumerate() {
            for &ty in component {
                component_of.insert(ty, id);
            }
        }
        let mut levels = vec![0usize; schedule.len()];
        for (id, component) in schedule.iter().enumerate() {
            for &ty in component {
                for edge in self.dependencies(ty) {
                    if let Some(&dependency) = component_of.get(&edge.target) {
                        if dependency != id {
                            levels[id] = levels[id].max(levels[dependency] + 1);
                        }
                    }
                }
            }
        }
        let mut waves: Vec<Vec<Vec<TypeId>>> = Vec::new();
        for (id, component) in schedule.into_iter().enumerate() {
            let level = levels[id];
            if waves.len() <= level {
                waves.resize_with(level + 1, Vec::new);
            }
            waves[level].push(component);
        }
        waves
    }

    /// All affected symbols grouped by the wave of their declared type.
    /// Symbol ordering within a wave follows captured symbol order.
    pub fn invalidation_symbol_waves(&self, changed: &[TypeId]) -> Vec<Vec<SymbolId>> {
        self.invalidation_waves(changed).into_iter().map(|wave| {
            let types: std::collections::HashSet<TypeId> =
                wave.into_iter().flatten().collect();
            self.symbol_order.iter()
                .filter(|(_, ty)| types.contains(ty))
                .map(|(symbol, _)| *symbol)
                .collect()
        }).collect()
    }

    /// Affected captured symbols in overall captured symbol order.
    pub fn affected_symbols(&self, changed: &[TypeId]) -> Vec<SymbolId> {
        let affected: std::collections::HashSet<TypeId> =
            self.affected_types(changed).into_iter().map(|node| node.ty).collect();
        self.symbol_order.iter()
            .filter(|(_, ty)| affected.contains(ty))
            .map(|(symbol, _)| *symbol)
            .collect()
    }
}

impl Graph {
    /// cpp/bridge.cpp::emit_type unconditionally emits BOTH event 63
    /// (plain-struct/C++ class admission) and event 65 (bounded scalar
    /// lifetime candidate) for every tk_struct or tk_class. Their absence
    /// on a successful compilation means a truncated snapshot; a missing
    /// positive admission must never look like a valid negative proof.
    /// Keep partial hand-built Graph fixtures independently supported.
    pub fn validate_native_class_admission_coverage(il:&Il) -> Result<(),GraphError> {
        let mut kinds=HashMap::with_capacity(il.types.len());
        for ty in &il.types {
            if kinds.insert(ty.id,unsafe {edgcpp_codegen_kind(4,ty.kind)}).is_some() {
                return Err(GraphError::DuplicateType(ty.id));
            }
        }
        let mut admitted=std::collections::HashSet::new();
        for record in &il.cxx_record_admissions {
            let kind=kinds.get(&record.owner)
                .ok_or(GraphError::MissingType(record.owner))?;
            if !matches!(*kind,7|8) || !admitted.insert(record.owner) {
                return Err(GraphError::InvalidNativeRecordAdmission(record.owner));
            }
        }
        let mut lifetime=std::collections::HashSet::new();
        for record in &il.cxx_scalar_lifetime_admissions {
            let kind=kinds.get(&record.owner)
                .ok_or(GraphError::MissingType(record.owner))?;
            if !matches!(*kind,7|8) || !lifetime.insert(record.owner) {
                return Err(GraphError::InvalidNativeLifetimeAdmission(record.owner));
            }
        }
        // Iterate source type order for deterministic first-error identity.
        for ty in &il.types {
            let id=ty.id;
            let kind=kinds[&id];
            if !matches!(kind,7|8) {continue;}
            if !admitted.contains(&id) {
                return Err(GraphError::MissingNativeRecordAdmission(id));
            }
            if !lifetime.contains(&id) {
                return Err(GraphError::MissingNativeLifetimeAdmission(id));
            }
        }
        Ok(())
    }

    /// The native bridge emits event 74 for every tk_struct/class/union,
    /// independently counting its EDG field_list and direct_base_classes.
    /// Without this a lost *final* field/base could silently shorten a
    /// seemingly contiguous layout, changing source ABI and memory access.
    /// Also require each base's event 24 access and event 69 offset proof
    /// (the offset value may legitimately be absent).
    pub fn validate_native_record_shape_coverage(il:&Il) -> Result<(),GraphError> {
        let mut kinds=HashMap::with_capacity(il.types.len());
        for ty in &il.types {
            if kinds.insert(ty.id,unsafe {edgcpp_codegen_kind(4,ty.kind)}).is_some() {
                return Err(GraphError::DuplicateType(ty.id));
            }
        }
        let mut proofs=HashMap::with_capacity(il.native_record_shapes.len());
        for proof in &il.native_record_shapes {
            let kind=kinds.get(&proof.owner).copied()
                .ok_or(GraphError::MissingType(proof.owner))?;
            if !matches!(kind,7..=9) || proof.field_count>10000 ||
               proof.direct_base_count>10000 ||
               (kind==9 && proof.direct_base_count!=0) {
                return Err(GraphError::InvalidNativeRecordShape(proof.owner));
            }
            if proofs.insert(proof.owner,(proof.field_count,proof.direct_base_count)).is_some() {
                return Err(GraphError::DuplicateNativeRecordShape(proof.owner));
            }
        }
        let mut fields:HashMap<TypeId,Vec<u32>>=HashMap::new();
        for field in &il.record_fields {
            if !kinds.get(&field.owner).is_some_and(|kind| matches!(*kind,7..=9)) ||
               !kinds.contains_key(&field.ty) || field.symbol.0==0 {
                return Err(GraphError::InvalidNativeRecordShape(field.owner));
            }
            fields.entry(field.owner).or_default().push(field.position);
        }
        let mut bases:HashMap<TypeId,Vec<u32>>=HashMap::new();
        for base in &il.base_classes {
            if !kinds.get(&base.derived).is_some_and(|kind| matches!(*kind,7|8)) ||
               !kinds.contains_key(&base.base) || base.derived==base.base {
                return Err(GraphError::InvalidNativeRecordShape(base.derived));
            }
            bases.entry(base.derived).or_default().push(base.position);
        }
        let mut accesses:HashMap<TypeId,Vec<u32>>=HashMap::new();
        for access in &il.base_class_access {
            accesses.entry(access.derived).or_default().push(access.position);
        }
        let mut offsets:HashMap<TypeId,Vec<u32>>=HashMap::new();
        for offset in &il.base_subobject_offsets {
            offsets.entry(offset.derived).or_default().push(offset.position);
        }
        fn complete(mut positions:Vec<u32>,count:u32)->bool {
            if positions.len()!=count as usize {return false;}
            positions.sort_unstable();
            positions.iter().enumerate().all(|(index,&position)|position==index as u32)
        }
        for ty in &il.types {
            if !matches!(kinds[&ty.id],7..=9) {continue;}
            let (field_count,base_count)=proofs.get(&ty.id).copied()
                .ok_or(GraphError::MissingNativeRecordShape(ty.id))?;
            if !complete(fields.remove(&ty.id).unwrap_or_default(),field_count) ||
               !complete(bases.remove(&ty.id).unwrap_or_default(),base_count) ||
               !complete(accesses.remove(&ty.id).unwrap_or_default(),base_count) ||
               !complete(offsets.remove(&ty.id).unwrap_or_default(),base_count) {
                return Err(GraphError::InvalidNativeRecordShape(ty.id));
            }
        }
        // A stray access/offset proof cannot belong to any native record.
        if let Some((&id,_))=accesses.iter().next().or_else(||offsets.iter().next()) {
            return Err(GraphError::InvalidNativeRecordShape(id));
        }
        Ok(())
    }

    /// Event 71 independently certifies both extra_info presence and the
    /// native EDG parameter-list length. A lost final event 8 must never
    /// change the inferred signature of a successful compilation.
    pub fn validate_native_routine_signature_coverage(il:&Il) -> Result<(),GraphError> {
        let mut kinds=HashMap::with_capacity(il.types.len());
        for ty in &il.types {
            if kinds.insert(ty.id,unsafe {edgcpp_codegen_kind(4,ty.kind)}).is_some() {
                return Err(GraphError::DuplicateType(ty.id));
            }
        }
        let mut proofs=HashMap::with_capacity(il.routine_supplements.len());
        for proof in &il.routine_supplements {
            match kinds.get(&proof.owner) {
                None=>return Err(GraphError::MissingType(proof.owner)),
                Some(&2)=>{},
                _=>return Err(GraphError::InvalidRoutineSupplement(proof.owner)),
            }
            if proof.parameter_count>10000 ||
               (!proof.has_extra_info && proof.parameter_count!=0) {
                return Err(GraphError::InvalidRoutineParameterCount(proof.owner));
            }
            if proofs.insert(proof.owner,(proof.has_extra_info,proof.parameter_count)).is_some() {
                return Err(GraphError::DuplicateRoutineSupplement(proof.owner));
            }
        }
        let mut signatures=std::collections::HashSet::new();
        for sig in &il.function_signatures {
            if kinds.get(&sig.owner)!=Some(&2) {
                return Err(GraphError::InvalidFunctionSignatureOwner(sig.owner));
            }
            if !signatures.insert(sig.owner) {
                return Err(GraphError::DuplicateFunctionSignature(sig.owner));
            }
        }
        for (&owner,&kind) in &kinds {
            if kind!=2 {continue;}
            let (present,_) = *proofs.get(&owner)
                .ok_or(GraphError::MissingRoutineSupplement(owner))?;
            if present && !signatures.contains(&owner) {
                return Err(GraphError::MissingFunctionSignature(owner));
            }
            if !present && signatures.contains(&owner) {
                return Err(GraphError::InvalidRoutineSupplement(owner));
            }
        }
        let mut positions: HashMap<TypeId,Vec<u32>>=HashMap::new();
        for param in &il.type_parameters {
            if !signatures.contains(&param.owner) {
                return Err(GraphError::MissingFunctionSignature(param.owner));
            }
            if !kinds.contains_key(&param.ty) {
                return Err(GraphError::MissingType(param.ty));
            }
            positions.entry(param.owner).or_default().push(param.position);
        }
        for (&owner, &(_,expected_count)) in &proofs {
            let observed=positions.entry(owner).or_default();
            if observed.len()!=expected_count as usize {
                return Err(GraphError::InvalidRoutineParameterCount(owner));
            }
            observed.sort_unstable();
            if observed.iter().enumerate().any(|(i,&position)| position!=i as u32) {
                return Err(GraphError::InvalidTypeParameterPosition(owner));
            }
        }
        Ok(())
    }

    /// A successful native EDG type emits exactly one event 75 describing
    /// whether EDG promises event 49 (complete object size/alignment) and
    /// event 9 (complete fixed array bound). An absent required layout/bound
    /// is truncated metadata, not proof that a complete object is incomplete
    /// or that a fixed array has no bound. Synthetic partial Il fixtures
    /// remain supported by Graph::from_il.
    pub fn validate_native_type_layout_coverage(il:&Il) -> Result<(),GraphError> {
        let mut kinds=HashMap::with_capacity(il.types.len());
        for ty in &il.types {
            if ty.id.0==0 {
                return Err(GraphError::InvalidTypeId(ty.id));
            }
            if kinds.insert(ty.id,unsafe {edgcpp_codegen_kind(4,ty.kind)}).is_some() {
                return Err(GraphError::DuplicateType(ty.id));
            }
        }
        let mut proofs=HashMap::with_capacity(il.native_type_layout_proofs.len());
        for proof in &il.native_type_layout_proofs {
            let Some(&kind)=kinds.get(&proof.owner) else {
                return Err(GraphError::MissingType(proof.owner));
            };
            if proof.has_fixed_array_bound && kind!=6 {
                return Err(GraphError::InvalidNativeTypeLayoutProof(proof.owner));
            }
            if proofs.insert(proof.owner,
                (proof.has_layout,proof.has_fixed_array_bound)).is_some() {
                return Err(GraphError::DuplicateNativeTypeLayoutProof(proof.owner));
            }
        }
        let mut layouts=std::collections::HashSet::new();
        for layout in &il.type_object_sizes {
            if !kinds.contains_key(&layout.owner) ||
               layout.bytes==0 || layout.alignment==0 ||
               !layouts.insert(layout.owner) {
                return Err(GraphError::InvalidNativeTypeLayoutProof(layout.owner));
            }
        }
        let mut bounds=std::collections::HashSet::new();
        for bound in &il.array_bounds {
            if kinds.get(&bound.array)!=Some(&6) ||
               !bounds.insert(bound.array) {
                return Err(GraphError::InvalidNativeTypeLayoutProof(bound.array));
            }
        }
        for ty in &il.types {
            let (expected_layout,expected_bound)=proofs.get(&ty.id).copied()
                .ok_or(GraphError::MissingNativeTypeLayoutProof(ty.id))?;
            if layouts.contains(&ty.id)!=expected_layout ||
               bounds.contains(&ty.id)!=expected_bound {
                return Err(GraphError::InvalidNativeTypeLayoutProof(ty.id));
            }
        }
        Ok(())
    }

    /// Native event 76 certifies each EDG type's supported scalar ABI
    /// records independently from the emitted records 43/46/57. A lost
    /// scalar width/sign classification must invalidate the compilation
    /// snapshot rather than appear as a legitimate unsupported type.
    /// Keep hand-built partial Graph::from_il fixtures independent.
    pub fn validate_native_scalar_metadata_coverage(il:&Il) -> Result<(),GraphError> {
        let mut kinds=HashMap::with_capacity(il.types.len());
        for ty in &il.types {
            if ty.id.0==0 {return Err(GraphError::InvalidTypeId(ty.id));}
            if kinds.insert(ty.id,unsafe {edgcpp_codegen_kind(4,ty.kind)}).is_some() {
                return Err(GraphError::DuplicateType(ty.id));
            }
        }
        let mut proofs=HashMap::with_capacity(il.native_scalar_proofs.len());
        for proof in &il.native_scalar_proofs {
            let Some(&kind)=kinds.get(&proof.owner) else {
                return Err(GraphError::MissingType(proof.owner));
            };
            if (proof.integer_abi && proof.standard_float_abi) ||
               (proof.signed_i64 && !proof.integer_abi) ||
               ((proof.integer_abi || proof.signed_i64) && kind!=1) ||
               (proof.standard_float_abi && kind!=0) {
                return Err(GraphError::InvalidNativeScalarMetadataProof(proof.owner));
            }
            if proofs.insert(proof.owner,
                (proof.integer_abi,proof.standard_float_abi,proof.signed_i64)).is_some() {
                return Err(GraphError::DuplicateNativeScalarMetadataProof(proof.owner));
            }
        }
        let mut integers=HashMap::with_capacity(il.c_integer_types.len());
        for info in &il.c_integer_types {
            if kinds.get(&info.id)!=Some(&1) ||
               !matches!(info.bits,8|16|32|64) ||
               (info.is_bool && (info.bits!=8 || info.signed)) ||
               integers.insert(info.id,info).is_some() {
                return Err(GraphError::InvalidNativeScalarMetadataProof(info.id));
            }
        }
        let mut floats=HashMap::with_capacity(il.c_float_types.len());
        for info in &il.c_float_types {
            // The current EDG semantic-kind mapper leaves tk_float as 0.
            if kinds.get(&info.id)!=Some(&0) ||
               !matches!(info.bits,32|64) ||
               floats.insert(info.id,info).is_some() {
                return Err(GraphError::InvalidNativeScalarMetadataProof(info.id));
            }
        }
        let mut signed=std::collections::HashSet::new();
        for &id in &il.signed_i64_types {
            if kinds.get(&id)!=Some(&1) || !signed.insert(id) {
                return Err(GraphError::InvalidNativeScalarMetadataProof(id));
            }
        }
        let mut layouts=HashMap::with_capacity(il.type_object_sizes.len());
        for layout in &il.type_object_sizes {
            if layouts.insert(layout.owner,layout.bytes).is_some() {
                return Err(GraphError::InvalidNativeScalarMetadataProof(layout.owner));
            }
        }
        for ty in &il.types {
            let (want_integer,want_float,want_i64)=proofs.get(&ty.id).copied()
                .ok_or(GraphError::MissingNativeScalarMetadataProof(ty.id))?;
            if integers.contains_key(&ty.id)!=want_integer ||
               floats.contains_key(&ty.id)!=want_float ||
               signed.contains(&ty.id)!=want_i64 {
                return Err(GraphError::InvalidNativeScalarMetadataProof(ty.id));
            }
            if let Some(integer)=integers.get(&ty.id) {
                if layouts.get(&ty.id).is_some_and(|&bytes|
                    bytes != u64::from(integer.bits)/8) ||
                   (want_i64 && (!integer.signed || integer.bits!=64 ||
                                  integer.is_bool)) {
                    return Err(GraphError::InvalidNativeScalarMetadataProof(ty.id));
                }
            }
            if let Some(float)=floats.get(&ty.id) {
                if layouts.get(&ty.id).is_some_and(|&bytes|
                    bytes != u64::from(float.bits)/8) {
                    return Err(GraphError::InvalidNativeScalarMetadataProof(ty.id));
                }
            }
        }
        Ok(())
    }

    /// A successful native EDG snapshot must retain pointee/element/underlying
    /// edges for pointer, array and typeref nodes. Those native owner types
    /// always have a child type; a zero relationship is an incomplete export,
    /// even if its TypeDef record itself is otherwise well-formed.
    ///
    /// Do not enforce this in from_il, which also accepts deliberately
    /// partial synthetic fixtures. In proven native C mode, every routine
    /// also has a return type (including void). C++ special member functions
    /// may lack a normal return type, so retain their existing exception.
    pub fn validate_native_type_edge_coverage(il:&Il) -> Result<(),GraphError> {
        let ids: std::collections::HashSet<TypeId> =
            il.types.iter().map(|ty|ty.id).collect();
        for ty in &il.types {
            let expected=match unsafe {edgcpp_codegen_kind(4,ty.kind)} {
                5=>1, 6=>2, 3=>4,
                2 if il.cpp_language==Some(false)=>3,
                _=>continue,
            };
            if ty.related_role!=Some(expected) ||
               !ty.related.is_some_and(|id| id.0!=0 && ids.contains(&id)) {
                return Err(GraphError::MissingNativeTypeRelation(ty.id));
            }
        }
        Ok(())
    }

    /// A successful *native EDG* snapshot contains one event 11 for every
    /// emitted tk_pointer or tk_typeref, including aliases with qualifiers=0.
    /// The general from_il constructor intentionally accepts partial hand-
    /// built fixtures, so enforce completeness at the native compile boundary.
    pub fn validate_native_type_detail_coverage(il:&Il) -> Result<(),GraphError> {
        let kinds: HashMap<TypeId,u32> = il.types.iter()
            .map(|ty|(ty.id,unsafe {edgcpp_codegen_kind(4,ty.kind)})).collect();
        let mut detailed=std::collections::HashSet::with_capacity(il.type_details.len());
        for detail in &il.type_details {
            let owner=*kinds.get(&detail.owner)
                .ok_or(GraphError::MissingType(detail.owner))?;
            if !matches!(owner,3|5) {
                return Err(GraphError::InvalidTypeDetailOwner(detail.owner));
            }
            // Check native shape as well as completeness at the callback
            // boundary, before a consumer decides whether to build a Graph.
            if (owner==5 && (detail.qualifiers.is_some() ||
                  (detail.is_rvalue_reference && !detail.is_reference))) ||
               (owner==3 && (detail.qualifiers.is_none() ||
                  detail.is_reference || detail.is_rvalue_reference)) {
                return Err(GraphError::InvalidTypeDetailFlags(detail.owner));
            }
            if !detailed.insert(detail.owner) {
                return Err(GraphError::DuplicateTypeDetail(detail.owner));
            }
        }
        for ty in &il.types {
            if matches!(unsafe {edgcpp_codegen_kind(4,ty.kind)},3|5)
                && !detailed.contains(&ty.id) {
                return Err(GraphError::MissingTypeDetail(ty.id));
            }
        }
        Ok(())
    }

    pub fn from_il(il: &Il) -> Result<Self, GraphError> {
        let mut graph = Self::default();
        for (index, expr) in il.control_expressions.iter().enumerate() {
            graph.control_expr_by_statement
                .entry((expr.function.clone(), expr.statement_path.clone()))
                .or_default()
                .push(index);
        }
        for indices in graph.control_expr_by_statement.values_mut() {
            indices.sort_by(|&a, &b| {
                let left = &il.control_expressions[a];
                let right = &il.control_expressions[b];
                left.role.cmp(&right.role)
                    .then_with(|| compare_tree_paths(&left.path, &right.path))
            });
        }

        let mut smap = HashMap::new();
        let mut emap = HashMap::new();
        let mut type_map = HashMap::new();
        for ty in &il.types {
            if ty.id.0 == 0 { return Err(GraphError::InvalidTypeId(ty.id)); }
            if type_map.insert(ty.id, graph.types.len()).is_some() {
                return Err(GraphError::DuplicateType(ty.id));
            }
            graph.types.push(ty.clone());
        }
        for ty in &graph.types {
            if ty.related.is_some() != ty.related_role.is_some() {
                return Err(GraphError::InvalidTypeRelation(ty.id));
            }
            // Native role numbers are pinned by cpp/bridge.cpp::emit_type:
            // 1 pointer pointee, 2 array element, 3 routine return,
            // 4 typedef/qualified underlying type. Other owner kinds
            // cannot have relationships in the exported wire format.
            if let Some(role)=ty.related_role {
                let expected=match unsafe { edgcpp_codegen_kind(4,ty.kind) } {
                    5 => 1, 6 => 2, 2 => 3, 3 => 4, _ => 0,
                };
                if role != expected {
                    return Err(GraphError::InvalidTypeRelation(ty.id));
                }
            }
            if let Some(child) = ty.related {
                if !type_map.contains_key(&child) { return Err(GraphError::MissingType(child)); }
            }
        }
        // The native bridge emits signature and parameter records only for
        // tk_routine. Decode native kind through the pinned semantic table,
        // not a hardcoded EDG enum discriminant.
        let routine_types: std::collections::HashSet<TypeId> = graph.types.iter()
            .filter(|ty| unsafe { edgcpp_codegen_kind(4, ty.kind) } == 2)
            .map(|ty| ty.id).collect();
        let mut parameters = std::collections::HashSet::new();
        let mut parameter_positions: HashMap<TypeId, Vec<u32>> = HashMap::new();
        for param in &il.type_parameters {
            for id in [param.owner, param.ty] {
                if !type_map.contains_key(&id) {
                    return Err(GraphError::MissingType(id));
                }
            }
            if !routine_types.contains(&param.owner) {
                return Err(GraphError::InvalidTypeParameterOwner(param.owner));
            }
            if !parameters.insert((param.owner, param.position)) {
                return Err(GraphError::DuplicateParameter {
                    owner: param.owner, position: param.position,
                });
            }
            parameter_positions.entry(param.owner).or_default().push(param.position);
            graph.type_parameters.push(param.clone());
        }
        // Native EDG emits each function type's parameters at 0..N in
        // declaration order. Gaps indicate a missing/truncated wire record.
        for (owner, positions) in &mut parameter_positions {
            positions.sort_unstable();
            if positions.iter().enumerate().any(|(index, &position)|
                position != index as u32)
            {
                return Err(GraphError::InvalidTypeParameterPosition(*owner));
            }
        }
        graph.type_parameters.sort_by_key(|p| (p.owner.0, p.position));
        let mut bounded = std::collections::HashSet::new();
        for bound in &il.array_bounds {
            let Some(&index)=type_map.get(&bound.array) else {
                return Err(GraphError::MissingType(bound.array));
            };
            // Event 9 only describes a fixed, complete EDG array type.
            if unsafe { edgcpp_codegen_kind(4, graph.types[index].kind) } != 6 {
                return Err(GraphError::InvalidArrayBoundOwner(bound.array));
            }
            if !bounded.insert(bound.array) {
                return Err(GraphError::DuplicateArrayBound(bound.array));
            }
            graph.array_bounds.push(bound.clone());
        }
        let mut field_positions = std::collections::HashSet::new();
        let mut field_symbol_positions = std::collections::HashSet::new();
        for field in &il.record_fields {
            for id in [field.owner, field.ty] {
                if !type_map.contains_key(&id) {
                    return Err(GraphError::MissingType(id));
                }
            }
            // Event 10 is emitted only for tk_struct, tk_class or tk_union.
            // An integer/array/pointer cannot own a native field layout.
            let owner=&graph.types[type_map[&field.owner]];
            if !matches!(unsafe {edgcpp_codegen_kind(4, owner.kind)},7..=9) {
                return Err(GraphError::InvalidRecordFieldOwner(field.owner));
            }
            // EDG's offset_bit_remainder is the sub-byte remainder of
            // an 8-bit byte offset; only 0..7 are representable.
            if field.bit_offset >= 8 {
                return Err(GraphError::InvalidRecordFieldBitOffset {
                    owner: field.owner, position: field.position,
                });
            }
            if !field_symbol_positions.insert((field.owner, field.symbol)) {
                return Err(GraphError::DuplicateRecordFieldSymbol {
                    owner: field.owner, symbol: field.symbol,
                });
            }
            if !field_positions.insert((field.owner, field.position)) {
                return Err(GraphError::DuplicateRecordField {
                    owner: field.owner, position: field.position,
                });
            }
            graph.record_fields.push(field.clone());
        }
        graph.record_fields.sort_by_key(|f| (f.owner.0, f.position));
        // The C++ emitter indexes each record's fields densely from zero;
        // gaps imply that native metadata was lost, even if the surviving
        // field has a plausible offset and field symbol.
        let mut field_counts: HashMap<TypeId, usize> = HashMap::new();
        for field in &graph.record_fields {
            let expected = field_counts.entry(field.owner).or_default();
            if field.position as usize != *expected {
                return Err(GraphError::InvalidRecordFieldPosition(field.owner));
            }
            *expected += 1;
        }
        let mut base_positions = std::collections::HashSet::new();
        for edge in &il.base_classes {
            if !type_map.contains_key(&edge.derived) {
                return Err(GraphError::MissingType(edge.derived));
            }
            if !type_map.contains_key(&edge.base) {
                return Err(GraphError::MissingType(edge.base));
            }
            if edge.derived == edge.base {
                return Err(GraphError::InvalidBaseClass {
                    derived: edge.derived, base: edge.base,
                });
            }
            if !base_positions.insert((edge.derived, edge.position)) {
                return Err(GraphError::DuplicateBaseClass {
                    derived: edge.derived, position: edge.position,
                });
            }
            graph.base_classes.push(edge.clone());
        }
        graph.base_classes.sort_by_key(|edge| (edge.derived.0, edge.position));
        let mut seen_access = std::collections::HashSet::new();
        for access in &il.base_class_access {
            if !base_positions.contains(&(access.derived, access.position)) {
                return Err(GraphError::InvalidBaseClassAccess {
                    derived: access.derived, position: access.position,
                });
            }
            if !seen_access.insert((access.derived, access.position)) {
                return Err(GraphError::DuplicateBaseClassAccess {
                    derived: access.derived, position: access.position,
                });
            }
            graph.base_class_access.push(access.clone());
        }
        graph.base_class_access.sort_by_key(|edge| (edge.derived.0, edge.position));

        // Direct bases are a source-order list, not a sparse index space.
        // Reject missing entries and cycles before exposing inheritance APIs.
        let mut grouped: HashMap<TypeId, Vec<TypeId>> = HashMap::new();
        for edge in &graph.base_classes {
            let entry = grouped.entry(edge.derived).or_default();
            if edge.position as usize != entry.len() {
                return Err(GraphError::InvalidBaseClassPosition(edge.derived));
            }
            entry.push(edge.base);
        }
        // Iterative DFS avoids stack overflow on malformed deep inheritance.
        let mut visited = std::collections::HashSet::new();
        let mut active = std::collections::HashSet::new();
        for &root in grouped.keys() {
            if visited.contains(&root) { continue; }
            let mut stack = vec![(root, false)];
            while let Some((node, exiting)) = stack.pop() {
                if exiting {
                    active.remove(&node);
                    visited.insert(node);
                    continue;
                }
                if visited.contains(&node) { continue; }
                if !active.insert(node) {
                    return Err(GraphError::CyclicInheritance(node));
                }
                stack.push((node, true));
                if let Some(bases) = grouped.get(&node) {
                    for &base in bases.iter().rev() {
                        if active.contains(&base) {
                            return Err(GraphError::CyclicInheritance(base));
                        }
                        if !visited.contains(&base) { stack.push((base, false)); }
                    }
                }
            }
        }

        let mut detailed = std::collections::HashSet::new();
        for detail in &il.type_details {
            let Some(&index)=type_map.get(&detail.owner) else {
                return Err(GraphError::MissingType(detail.owner));
            };
            // The bridge emits event 11 only for pointer and typeref
            // nodes; it cannot describe a record, array or integer.
            let owner_kind=unsafe { edgcpp_codegen_kind(4, graph.types[index].kind) };
            if !matches!(owner_kind, 3|5) {
                return Err(GraphError::InvalidTypeDetailOwner(detail.owner));
            }
            // The native pointer emits reference bits but no qualifiers.
            // The native typeref emits qualifiers (including zero) but never
            // reference bits. Lost/shifted field tags must fail closed.
            // EDG il_def.h defines is_reference as encompassing both
            // lvalue *and* rvalue references. An rvalue-reference flag
            // without the encompassing reference bit cannot be native IL.
            if (owner_kind==5 &&
                (detail.qualifiers.is_some() ||
                 (detail.is_rvalue_reference && !detail.is_reference)))
                || (owner_kind==3 &&
                    (detail.qualifiers.is_none() ||
                     detail.is_reference || detail.is_rvalue_reference))
            {
                return Err(GraphError::InvalidTypeDetailFlags(detail.owner));
            }
            if !detailed.insert(detail.owner) {
                return Err(GraphError::DuplicateTypeDetail(detail.owner));
            }
            graph.type_details.push(detail.clone());
        }
        let mut signatures = std::collections::HashSet::new();
        for sig in &il.function_signatures {
            if !type_map.contains_key(&sig.owner) {
                return Err(GraphError::MissingType(sig.owner));
            }
            if !routine_types.contains(&sig.owner) {
                return Err(GraphError::InvalidFunctionSignatureOwner(sig.owner));
            }
            if !signatures.insert(sig.owner) {
                return Err(GraphError::DuplicateFunctionSignature(sig.owner));
            }
            graph.function_signatures.push(sig.clone());
        }
        // Explicit native proof records are optional for hand-built graph
        // fixtures, but when present they must agree with event 12 even if
        // there are no parameter records to cross-check.
        let mut proven=std::collections::HashSet::new();
        for proof in &il.routine_supplements {
            if !type_map.contains_key(&proof.owner) {
                return Err(GraphError::MissingType(proof.owner));
            }
            if !routine_types.contains(&proof.owner) {
                return Err(GraphError::InvalidRoutineSupplement(proof.owner));
            }
            if !proven.insert(proof.owner) {
                return Err(GraphError::DuplicateRoutineSupplement(proof.owner));
            }
            if proof.has_extra_info && !signatures.contains(&proof.owner) {
                return Err(GraphError::MissingFunctionSignature(proof.owner));
            }
            if !proof.has_extra_info && signatures.contains(&proof.owner) {
                return Err(GraphError::InvalidRoutineSupplement(proof.owner));
            }
            if (!proof.has_extra_info && proof.parameter_count!=0) ||
               proof.parameter_count>10000 ||
               parameter_positions.get(&proof.owner).map_or(0,Vec::len)
                    != proof.parameter_count as usize {
                return Err(GraphError::InvalidRoutineParameterCount(proof.owner));
            }
        }
        // Native parameter edges require event 12, even in a partial
        // synthetic graph without supplement-presence proof.
        for owner in parameter_positions.keys() {
            if !signatures.contains(owner) {
                return Err(GraphError::MissingFunctionSignature(*owner));
            }
        }
        // Native symbol identities are unique across all declaration kinds.
        // Retain their kind and position once: validating every parameter,
        // local, label or lifetime must not rescan the full symbol vector.
        let mut symbol_index: HashMap<SymbolId, (u32, usize)> = HashMap::new();
        for symbol in &il.symbols {
            if symbol.id.0 == 0 ||
               symbol_index.insert(symbol.id, (symbol.kind, graph.symbols.len())).is_some() {
                return Err(GraphError::DuplicateSymbol(symbol.id));
            }
            graph.symbols.push(symbol.clone());
        }
        let has_kind = |id: SymbolId, kind: u32| symbol_index.get(&id)
            .is_some_and(|&(actual, _)| actual == kind);
        // Native record field metadata carries the exact field symbol
        // emitted alongside the record; it cannot reference an unrelated
        // variable, a zero ID or an absent symbol.
        for field in &graph.record_fields {
            if !has_kind(field.symbol, 3) {
                return Err(GraphError::InvalidRecordFieldSymbol {
                    owner: field.owner, symbol: field.symbol,
                });
            }
        }
        for identity in &il.routine_identities {
            if !has_kind(identity.routine, 1)
                || identity.function.is_empty() {
                return Err(GraphError::InvalidRoutineIdentity(identity.routine));
            }
            if graph.routine_function_by_id.insert(
                identity.routine, identity.function.clone()).is_some()
            {
                return Err(GraphError::DuplicateRoutineIdentity(identity.routine));
            }
            if graph.routine_by_function.insert(
                identity.function.clone(), identity.routine).is_some()
            {
                return Err(GraphError::DuplicateRoutineFunction(identity.function.clone()));
            }
            graph.routine_identities.push(identity.clone());
        }
        let mut typed_symbols = std::collections::HashSet::new();
        for edge in &il.symbol_types {
            // The bridge exports these type edges only for routine and
            // variable declarations. Record fields have explicit field-type
            // metadata; labels are untyped control-flow identities.
            if !(has_kind(edge.symbol, 1) || has_kind(edge.symbol, 2))
                || !type_map.contains_key(&edge.ty) {
                return Err(GraphError::InvalidSymbolType(edge.symbol));
            }
            if !typed_symbols.insert(edge.symbol) {
                return Err(GraphError::DuplicateSymbolType(edge.symbol));
            }
            graph.symbol_types.push(edge.clone());
        }
        // The native bridge emits symbol types and parameter/local metadata
        // from the same EDG variable declaration. Their identities must not
        // disagree, even when the prototype would independently be accepted.
        let symbol_type_by_id: HashMap<SymbolId, TypeId> = graph.symbol_types.iter()
            .map(|edge| (edge.symbol, edge.ty)).collect();
        let mut annotated_routines = std::collections::HashSet::new();
        for attributes in &il.routine_attributes {
            if !has_kind(attributes.routine, 1)
                || (attributes.pure_virtual && !attributes.is_virtual)
                || (attributes.is_override && !attributes.is_virtual)
                || (attributes.is_final && !attributes.is_virtual)
                || (attributes.overrides_base_member && !attributes.is_virtual)
            {
                return Err(GraphError::InvalidRoutineAttributes(attributes.routine));
            }
            if !annotated_routines.insert(attributes.routine) {
                return Err(GraphError::DuplicateRoutineAttributes(attributes.routine));
            }
            graph.routine_attributes.push(attributes.clone());
        }
        let mut member_owners = std::collections::HashSet::new();
        for member in &il.member_functions {
            if !type_map.contains_key(&member.owner) ||
                !has_kind(member.routine, 1) {
                return Err(GraphError::InvalidMemberFunction {
                    owner: member.owner, routine: member.routine,
                });
            }
            if !member_owners.insert(member.routine) {
                return Err(GraphError::DuplicateMemberFunction(member.routine));
            }
            graph.member_functions.push(member.clone());
        }
        // Cross-check field ownership only when both the member routine
        // and the native record field identities have been exported.
        let constructor_owner: HashMap<SymbolId, TypeId> = graph.member_functions.iter()
            .map(|member| (member.routine, member.owner)).collect();
        let mut field_owners: HashMap<SymbolId, std::collections::HashSet<TypeId>> =
            HashMap::new();
        for field in &graph.record_fields {
            field_owners.entry(field.symbol).or_default().insert(field.owner);
        }
        // Constructor initializers refer to routine symbols, field symbols,
        // or record types. Check the complete snapshot before indexing them.
        let mut ctor_positions = std::collections::HashSet::new();
        for init in &il.constructor_initializers {
            let valid_routine = has_kind(init.routine, 1);
            let valid_target = match init.kind {
                0 | 1 => init.target_type.is_some_and(|ty| type_map.contains_key(&ty))
                    && init.target_field.is_none(),
                2 => init.target_field.is_some_and(|field| {
                        has_kind(field, 3) &&
                        constructor_owner.get(&init.routine).is_none_or(|owner| {
                            field_owners.get(&field).is_none_or(|owners|
                                owners.contains(owner))
                        })
                    })
                    && init.target_type.is_none(),
                3 => init.target_type.is_none() && init.target_field.is_none(),
                _ => false,
            };
            if !valid_routine || !valid_target ||
               !ctor_positions.insert((init.routine, init.position)) {
                return Err(GraphError::InvalidConstructorInitializer {
                    routine: init.routine, position: init.position,
                });
            }
            graph.constructor_initializers.push(init.clone());
        }
        graph.constructor_initializers.sort_by_key(|v| (v.routine.0, v.position));
        for window in graph.constructor_initializers.windows(2) {
            if window[0].routine == window[1].routine
                && window[1].position != window[0].position + 1 {
                return Err(GraphError::InvalidConstructorInitializer {
                    routine: window[1].routine, position: window[1].position,
                });
            }
        }
        for init in &graph.constructor_initializers {
            if init.position == 0 { continue; }
            if !ctor_positions.contains(&(init.routine, 0)) {
                return Err(GraphError::InvalidConstructorInitializer {
                    routine: init.routine, position: init.position,
                });
            }
        }
        // Each constructor initializer can carry a separately owned
        // expression tree. Paths are local to (routine, position).
        let mut ctor_expr_paths = std::collections::HashSet::new();
        for expr in &il.constructor_initializer_expressions {
            let key = (expr.routine, expr.position, expr.path.clone());
            if !ctor_positions.contains(&(expr.routine, expr.position))
                || expr.symbol_id.is_some_and(|id| !symbol_index.contains_key(&id))
                || expr.type_id.is_some_and(|id| !type_map.contains_key(&id))
            {
                return Err(GraphError::InvalidConstructorExpression {
                    routine: expr.routine, position: expr.position,
                    path: expr.path.clone(),
                });
            }
            if !ctor_expr_paths.insert(key.clone()) {
                return Err(GraphError::DuplicateConstructorExpression {
                    routine: expr.routine, position: expr.position,
                    path: expr.path.clone(),
                });
            }
            graph.constructor_expr_index.insert(key, graph.constructor_initializer_expressions.len());
            graph.constructor_initializer_expressions.push(expr.clone());
        }
        for &(routine, position, _) in &ctor_expr_paths {
            if !ctor_expr_paths.contains(&(routine, position, String::from("0"))) {
                return Err(GraphError::MissingConstructorExpressionRoot { routine, position });
            }
        }
        for (idx, node) in graph.constructor_initializer_expressions.iter().enumerate() {
            let valid = match &node.parent {
                None => node.path == "0",
                Some(parent) => node.path.strip_prefix(&(parent.clone() + "."))
                    .is_some_and(|suffix| !suffix.is_empty() && !suffix.contains('.')
                        && suffix.parse::<u32>().is_ok())
                    && ctor_expr_paths.contains(&(node.routine, node.position, parent.clone())),
            };
            if !valid {
                return Err(GraphError::InvalidConstructorExpression {
                    routine: node.routine, position: node.position,
                    path: node.path.clone(),
                });
            }
            if let Some(parent) = &node.parent {
                graph.constructor_expr_children.entry((node.routine, node.position, parent.clone()))
                    .or_default().push(idx);
            }
        }
        for children in graph.constructor_expr_children.values_mut() {
            children.sort_by_key(|&idx|
                graph_path_index(&graph.constructor_initializer_expressions[idx].path));
        }
        for ((routine, position, parent), children) in &graph.constructor_expr_children {
            for (index, &child) in children.iter().enumerate() {
                if graph.constructor_initializer_expressions[child].path
                    != format!("{}.{}", parent, index)
                {
                    return Err(GraphError::InvalidConstructorExpressionOperandPath {
                        routine: *routine, position: *position, path: parent.clone(),
                    });
                }
            }
        }
        let mut initialized = std::collections::HashSet::new();
        for init in &il.variable_initializers {
            if !symbol_index.contains_key(&init.symbol) {
                return Err(GraphError::MissingSymbol(init.symbol));
            }
            if !has_kind(init.symbol, 2) {
                return Err(GraphError::InvalidInitializerSymbol(init.symbol));
            }
            if !initialized.insert(init.symbol) {
                return Err(GraphError::DuplicateVariableInitializer(init.symbol));
            }
            graph.variable_initializers.push(init.clone());
        }
        let mut integer_inits = std::collections::HashSet::new();
        for val in &il.initializer_integers {
            if !symbol_index.contains_key(&val.symbol) {
                return Err(GraphError::MissingSymbol(val.symbol));
            }
            if !has_kind(val.symbol, 2) {
                return Err(GraphError::InvalidInitializerSymbol(val.symbol));
            }
            if !integer_inits.insert(val.symbol) {
                return Err(GraphError::DuplicateInitializerInteger(val.symbol));
            }
            graph.initializer_integers.push(val.clone());
        }
        let mut dynamic_inits = std::collections::HashSet::new();
        for init in &il.dynamic_initializers {
            if !symbol_index.contains_key(&init.symbol) { return Err(GraphError::MissingSymbol(init.symbol)); }
            if !has_kind(init.symbol, 2) {
                return Err(GraphError::InvalidInitializerSymbol(init.symbol));
            }
            if !dynamic_inits.insert(init.symbol) {
                return Err(GraphError::DuplicateDynamicInitializer(init.symbol));
            }
            graph.dynamic_initializers.push(init.clone());
        }
        let mut init_paths = std::collections::HashSet::new();
        for node in &il.initializer_expressions {
            if !symbol_index.contains_key(&node.owner) {
                return Err(GraphError::MissingSymbol(node.owner));
            }
            if !has_kind(node.owner, 2) {
                return Err(GraphError::InvalidInitializerSymbol(node.owner));
            }
            if let Some(id) = node.symbol_id {
                if !symbol_index.contains_key(&id) { return Err(GraphError::MissingSymbol(id)); }
            }
            if let Some(id) = node.type_id {
                if !type_map.contains_key(&id) { return Err(GraphError::MissingType(id)); }
            }
            if !init_paths.insert((node.owner, node.path.clone())) {
                return Err(GraphError::DuplicateInitializerExpression {
                    owner: node.owner, path: node.path.clone(),
                });
            }
            graph.initializer_expressions.push(node.clone());
            graph.initializer_index.insert((node.owner, node.path.clone()), graph.initializer_expressions.len() - 1);
        }
        // Every captured initializer tree must have its root. Otherwise
        // navigation could silently return None for a malformed snapshot.
        let owners: std::collections::HashSet<_> = graph.initializer_expressions
            .iter().map(|node| node.owner).collect();
        for owner in owners {
            if !init_paths.contains(&(owner, String::from("0"))) {
                return Err(GraphError::MissingInitializerRoot(owner));
            }
        }
        for node in &graph.initializer_expressions {
            let valid = match &node.parent {
                None => node.path == "0",
                Some(parent) => node.path.strip_prefix(&(parent.clone() + "."))
                    .is_some_and(|suffix| !suffix.is_empty() && !suffix.contains('.')
                        && suffix.parse::<u32>().is_ok())
                    && init_paths.contains(&(node.owner, parent.clone())),
            };
            if !valid {
                return Err(GraphError::InvalidInitializerParent {
                    owner: node.owner, path: node.path.clone(),
                });
            }
        }
        for (idx, node) in graph.initializer_expressions.iter().enumerate() {
            if let Some(parent) = &node.parent {
                graph.initializer_children.entry((node.owner, parent.clone()))
                    .or_default().push(idx);
            }
        }
        for children in graph.initializer_children.values_mut() {
            children.sort_by_key(|&idx| graph_path_index(&graph.initializer_expressions[idx].path));
        }
        // Initializer operands use the same positional path convention as
        // statement expressions. Missing indices must not be interpreted as
        // a different operand position by downstream compiler passes.
        for ((owner, parent), children) in &graph.initializer_children {
            for (index, &child) in children.iter().enumerate() {
                if graph.initializer_expressions[child].path
                    != format!("{}.{}", parent, index)
                {
                    return Err(GraphError::InvalidInitializerOperandPath {
                        owner: *owner, path: parent.clone(),
                    });
                }
            }
        }
        for expr in &il.expressions {
            if let Some(id) = expr.symbol_id {
                if !symbol_index.contains_key(&id) { return Err(GraphError::MissingSymbol(id)); }
            }
        }
        for expr in &il.expressions {
            if let Some(id) = expr.type_id {
                if !type_map.contains_key(&id) { return Err(GraphError::MissingType(id)); }
            }
        }
        for item in &il.statements {
            let key = (item.function.clone(), item.path.clone());
            if smap.insert(key.clone(), StatementId(graph.statements.len())).is_some() {
                return Err(GraphError::DuplicateStatement { function: key.0, path: key.1 });
            }
            graph.statements.push(StatementNode {
                data: item.clone(), parent: None, children: Vec::new(), expression: None,
            });
        }
        // Scope IDs are dense per routine in the native snapshot and
        // parents precede their children. Validate after collecting all
        // statements and symbols, since EDG emits these independently.
        let is_routine = |id: SymbolId| has_kind(id, 1);
        let is_variable = |id: SymbolId| has_kind(id, 2);
        let mut scope_keys = std::collections::HashSet::new();
        for scope in &il.local_scopes {
            if !is_routine(scope.routine)
                || !scope_keys.insert((scope.routine, scope.id)) {
                if is_routine(scope.routine) {
                    return Err(GraphError::DuplicateLocalScope {
                        routine: scope.routine, scope: scope.id,
                    });
                }
                return Err(GraphError::InvalidLocalScope {
                    routine: scope.routine, scope: scope.id,
                });
            }
            graph.local_scopes.push(scope.clone());
        }
        let mut scope_groups: HashMap<SymbolId, Vec<u32>> = HashMap::new();
        for scope in &graph.local_scopes {
            let valid_parent = if scope.id == 0 {
                scope.parent.is_none()
            } else {
                scope.parent.is_some_and(|parent|
                    parent < scope.id && scope_keys.contains(&(scope.routine, parent)))
            };
            let valid_statement = scope.statement_path.as_ref().is_none_or(|path| {
                let name = graph.routine_function_by_id.get(&scope.routine)
                    .map(String::as_str)
                    .or_else(|| symbol_index.get(&scope.routine)
                        .map(|&(_, index)| graph.symbols[index].name.as_str()));
                name.is_some_and(|function|
                    smap.contains_key(&(function.to_owned(), path.clone())))
            });
            if !valid_parent || !valid_statement {
                return Err(GraphError::InvalidLocalScope {
                    routine: scope.routine, scope: scope.id,
                });
            }
            scope_groups.entry(scope.routine).or_default().push(scope.id);
        }
        for (routine, mut ids) in scope_groups {
            ids.sort_unstable();
            if ids.iter().enumerate().any(|(index, &id)| index as u32 != id) {
                return Err(GraphError::InvalidLocalScope {
                    routine, scope: ids[0],
                });
            }
        }

        let mut parameter_positions: HashMap<SymbolId, std::collections::HashSet<Option<u32>>> =
            HashMap::new();
        let mut parameter_symbols = std::collections::HashSet::new();
        for param in &il.function_parameters {
            if !is_routine(param.routine) || !is_variable(param.symbol)
                || !type_map.contains_key(&param.ty)
                || param.implicit_this != param.position.is_none()
                || symbol_type_by_id.get(&param.symbol)
                    .is_some_and(|&ty| ty != param.ty)
            {
                return Err(GraphError::InvalidFunctionParameter {
                    routine: param.routine, symbol: param.symbol,
                });
            }
            if !parameter_symbols.insert(param.symbol)
                || !parameter_positions.entry(param.routine).or_default().insert(param.position)
            {
                return Err(GraphError::DuplicateFunctionParameter {
                    routine: param.routine, position: param.position,
                });
            }
            graph.function_parameters.push(param.clone());
        }
        for (routine, positions) in &parameter_positions {
            let mut indices: Vec<_> = positions.iter().flatten().copied().collect();
            indices.sort_unstable();
            if indices.iter().enumerate().any(|(index, &position)|
                index as u32 != position)
            {
                return Err(GraphError::InvalidFunctionParameterPosition(*routine));
            }
        }
        graph.function_parameters.sort_by_key(|p|
            (p.routine.0, p.position.unwrap_or(u32::MAX)));

        let mut declared_locals = std::collections::HashSet::new();
        for var in &il.local_variables {
            if !is_routine(var.routine) || !is_variable(var.symbol)
                || !type_map.contains_key(&var.ty)
                || !scope_keys.contains(&(var.routine, var.scope))
                || parameter_symbols.contains(&var.symbol)
                || symbol_type_by_id.get(&var.symbol)
                    .is_some_and(|&ty| ty != var.ty)
            {
                return Err(GraphError::InvalidLocalVariable(var.symbol));
            }
            if !declared_locals.insert(var.symbol) {
                return Err(GraphError::DuplicateLocalVariable(var.symbol));
            }
            graph.local_variables.push(var.clone());
        }
        graph.local_scopes.sort_by_key(|scope| (scope.routine.0, scope.id));
        graph.local_variables.sort_by_key(|var| (var.routine.0, var.scope, var.symbol.0));
        let locals_by_symbol: HashMap<SymbolId, (SymbolId, bool)> =
            graph.local_variables.iter().map(|local|
                (local.symbol, (local.routine, local.static_storage))).collect();
        let mut static_owner_symbols = std::collections::HashSet::new();
        for entry in &il.static_local_owners {
            if !is_routine(entry.routine) || !is_variable(entry.symbol) ||
                locals_by_symbol.get(&entry.symbol).is_some_and(|&(routine, is_static)|
                    routine != entry.routine || !is_static)
            {
                return Err(GraphError::InvalidStaticLocalOwner(entry.symbol));
            }
            if !static_owner_symbols.insert(entry.symbol) {
                return Err(GraphError::DuplicateStaticLocalOwner(entry.symbol));
            }
            graph.static_local_owners.push(entry.clone());
        }
        graph.static_local_owners.sort_by_key(|entry| (entry.routine.0, entry.symbol.0));


        let mut switch_case_keys=std::collections::HashSet::new();
        for case in &il.statement_switch_cases {
            let key=(case.function.clone(),case.statement_path.clone());
            if !smap.contains_key(&key) || !switch_case_keys.insert(key.clone()) ||
                case.status>2 || (case.status==1)!=case.value.is_some() {
                return Err(GraphError::InvalidStatementLabel {
                    function:key.0,path:key.1,
                });
            }
            graph.statement_switch_cases.push(case.clone());
        }
        // C/C++ labels are function-scoped, even when every native label
        // identity is unique across the full translation unit. A corrupted
        // snapshot must not resolve a goto to a different function's label.
        let mut label_function: HashMap<SymbolId, String> = HashMap::new();
        for label in &il.statement_labels {
            let key = (label.function.clone(), label.statement_path.clone());
            if let Some(owner) = label_function.get(&label.label) {
                if owner != &label.function {
                    return Err(GraphError::InvalidStatementLabel {
                        function: label.function.clone(), path: label.statement_path.clone(),
                    });
                }
            } else {
                label_function.insert(label.label, label.function.clone());
            }
            let Some(&statement) = smap.get(&key) else {
                return Err(GraphError::InvalidStatementLabel {
                    function: label.function.clone(), path: label.statement_path.clone(),
                });
            };
            if !has_kind(label.label, 4)
            {
                return Err(GraphError::InvalidStatementLabel {
                    function: label.function.clone(), path: label.statement_path.clone(),
                });
            }
            if graph.statement_label_index.insert(statement, graph.statement_labels.len()).is_some() {
                return Err(GraphError::DuplicateStatementLabel {
                    function: label.function.clone(), path: label.statement_path.clone(),
                });
            }
            if label.definition &&
                graph.label_definitions.insert(label.label, statement).is_some()
            {
                return Err(GraphError::DuplicateLabelDefinition(label.label));
            }
            graph.statement_labels.push(label.clone());
        }

        let mut decl_keys = std::collections::HashSet::new();
        let mut decl_positions: HashMap<(String, String), Vec<u32>> = HashMap::new();
        for decl in &il.statement_declarations {
            let key = (decl.function.clone(), decl.statement_path.clone());
            if !smap.contains_key(&key)
                || !has_kind(decl.symbol, 2)
                || !type_map.contains_key(&decl.ty)
                || symbol_type_by_id.get(&decl.symbol)
                    .is_some_and(|&symbol_ty| symbol_ty != decl.ty)
            {
                return Err(GraphError::InvalidStatementDeclaration {
                    function: decl.function.clone(), path: decl.statement_path.clone(),
                });
            }
            if !decl_keys.insert((key.clone(), decl.position)) {
                return Err(GraphError::DuplicateStatementDeclaration {
                    function: decl.function.clone(), path: decl.statement_path.clone(),
                    position: decl.position,
                });
            }
            decl_positions.entry(key).or_default().push(decl.position);
            graph.statement_declarations.push(decl.clone());
        }
        for ((function, path), mut positions) in decl_positions {
            positions.sort_unstable();
            if positions.iter().enumerate().any(|(i, &position)| i as u32 != position) {
                return Err(GraphError::InvalidStatementDeclarationPosition { function, path });
            }
        }
        graph.statement_declarations.sort_by_key(|d|
            (d.function.clone(), d.statement_path.clone(), d.position));

        // A VLA dimension belongs to a particular *execution statement*,
        // not to a declaration's source spelling or global type lifetime.
        // VLA dimension validation needs only the existence of a native
        // root expression at each statement, not repeated scans of every
        // expression in the entire translation unit.
        let source_expression_roots: std::collections::HashSet<(String, String)> =
            il.expressions.iter().filter(|e| e.path == "0" && e.parent.is_none())
            .map(|e| (e.function.clone(), e.statement_path.clone())).collect();
        let mut vla_sites=std::collections::HashSet::new();
        for dim in &il.vla_dimensions {
            let key=(dim.function.clone(),dim.statement_path.clone());
            if !smap.contains_key(&key) || !type_map.contains_key(&dim.array_type) ||
                (dim.copied && dim.has_expression) ||
                (dim.has_expression && !source_expression_roots.contains(&key))
            {
                return Err(GraphError::InvalidVlaDimension {
                    function:key.0,path:key.1,
                });
            }
            if !vla_sites.insert(key.clone()) {
                return Err(GraphError::DuplicateVlaDimension {
                    function:key.0,path:key.1,
                });
            }
            graph.vla_dimensions.push(dim.clone());
        }

        let mut init_keys = std::collections::HashSet::new();
        for init in &il.statement_initializations {
            let key = (init.function.clone(), init.statement_path.clone());
            if !smap.contains_key(&key) || init.role > 2 ||
               init.symbol.is_some_and(|symbol| !has_kind(symbol, 2)) {
                return Err(GraphError::InvalidStatementInitialization {
                    function: key.0, path: key.1,
                });
            }
            if !init_keys.insert(key.clone()) {
                return Err(GraphError::DuplicateStatementInitialization {
                    function: key.0, path: key.1,
                });
            }
            graph.statement_initializations.push(init.clone());
        }

        let mut statement_metadata = std::collections::HashSet::new();
        for attributes in &il.statement_attributes {
            let key = (attributes.function.clone(), attributes.statement_path.clone());
            if !smap.contains_key(&key)
                || (attributes.likely && attributes.unlikely)
            {
                return Err(GraphError::InvalidStatementAttributes {
                    function: key.0, path: key.1,
                });
            }
            if !statement_metadata.insert(key.clone()) {
                return Err(GraphError::DuplicateStatementAttributes {
                    function: key.0, path: key.1,
                });
            }
            graph.statement_attributes.push(attributes.clone());
        }

        let mut control_paths = std::collections::HashSet::new();
        for expr in &il.control_expressions {
            let key = (expr.function.clone(), expr.statement_path.clone(),
                       expr.role, expr.path.clone());
            if !(1..=4).contains(&expr.role)
                || (expr.role == 4 && !init_keys.contains(&(
                    expr.function.clone(), expr.statement_path.clone())))
                || !smap.contains_key(&(expr.function.clone(), expr.statement_path.clone()))
                || expr.type_id.is_some_and(|id| !type_map.contains_key(&id))
                || expr.symbol_id.is_some_and(|id| !symbol_index.contains_key(&id))
            {
                return Err(GraphError::InvalidControlExpression {
                    function: expr.function.clone(), statement: expr.statement_path.clone(),
                    role: expr.role, path: expr.path.clone(),
                });
            }
            if !control_paths.insert(key.clone()) {
                return Err(GraphError::DuplicateControlExpression {
                    function: expr.function.clone(), statement: expr.statement_path.clone(),
                    role: expr.role, path: expr.path.clone(),
                });
            }
            graph.control_expr_index.insert(key, graph.control_expressions.len());
            graph.control_expressions.push(expr.clone());
        }
        for expr in &graph.control_expressions {
            if !control_paths.contains(&(expr.function.clone(), expr.statement_path.clone(),
                                          expr.role, String::from("0"))) {
                return Err(GraphError::MissingControlExpressionRoot {
                    function: expr.function.clone(), statement: expr.statement_path.clone(),
                    role: expr.role,
                });
            }
        }
        for (idx, expr) in graph.control_expressions.iter().enumerate() {
            let valid = match &expr.parent {
                None => expr.path == "0",
                Some(parent) => expr.path.strip_prefix(&(parent.clone() + "."))
                    .is_some_and(|suffix| !suffix.is_empty() && !suffix.contains('.')
                        && suffix.parse::<u32>().is_ok())
                    && control_paths.contains(&(expr.function.clone(), expr.statement_path.clone(),
                                                expr.role, parent.clone())),
            };
            if !valid {
                return Err(GraphError::InvalidControlExpression {
                    function: expr.function.clone(), statement: expr.statement_path.clone(),
                    role: expr.role, path: expr.path.clone(),
                });
            }
            if let Some(parent) = &expr.parent {
                graph.control_expr_children.entry((
                    expr.function.clone(), expr.statement_path.clone(),
                    expr.role, parent.clone(),
                )).or_default().push(idx);
            }
        }
        for children in graph.control_expr_children.values_mut() {
            children.sort_by_key(|&idx| graph_path_index(&graph.control_expressions[idx].path));
        }
        for ((function, statement, role, parent), children) in &graph.control_expr_children {
            for (index, &idx) in children.iter().enumerate() {
                if graph.control_expressions[idx].path != format!("{}.{}", parent, index) {
                    return Err(GraphError::InvalidControlExpressionOperandPath {
                        function: function.clone(), statement: statement.clone(),
                        role: *role, path: parent.clone(),
                    });
                }
            }
        }
        for item in &il.expressions {
            let key = (item.function.clone(), item.statement_path.clone(), item.path.clone());
            if emap.insert(key.clone(), ExpressionId(graph.expressions.len())).is_some() {
                return Err(GraphError::DuplicateExpression {
                    function: key.0, statement: key.1, path: key.2,
                });
            }
            let statement = *smap.get(&(item.function.clone(), item.statement_path.clone()))
                .ok_or_else(|| GraphError::MissingStatement {
                    function: item.function.clone(), path: item.statement_path.clone(),
                })?;
            graph.expressions.push(ExpressionNode {
                data: item.clone(), parent: None, operands: Vec::new(), statement,
            });
        }
        graph.call_index_by_expression.resize(graph.expressions.len(), None);
        let mut call_paths = std::collections::HashSet::new();
        for call in &il.calls {
            let key = (call.function.clone(), call.statement_path.clone(), call.path.clone());
            if !call_paths.insert(key.clone()) {
                return Err(GraphError::DuplicateCallExpression {
                    function: key.0, statement: key.1, path: key.2,
                });
            }
            if !emap.contains_key(&key) {
                return Err(GraphError::MissingCallExpression {
                    function: key.0, statement: key.1, path: key.2,
                });
            }
            let expression = emap[&key];
            graph.call_index_by_expression[expression.0] = Some(graph.calls.len());
            graph.call_expression_by_site.insert(call.clone(), expression);
            graph.calls.push(call.clone());
        }
        for idx in 0..graph.statements.len() {
            let parent = graph.statements[idx].data.parent.clone();
            if let Some(path) = parent {
                let function = graph.statements[idx].data.function.clone();
                let id = *smap.get(&(function.clone(), path.clone()))
                    .ok_or(GraphError::MissingStatement { function, path: path.clone() })?;
                let rest = graph.statements[idx].data.path
                    .strip_prefix(&(path.clone() + "."));
                // Statement child paths: parent.edge.index.
                let direct = rest.and_then(|r| r.split_once('.'))
                    .is_some_and(|(edge, index)| {
                        !edge.is_empty() && !index.is_empty() &&
                        !index.contains('.') && index.parse::<usize>().is_ok()
                    });
                if id.0 == idx || !direct {
                    return Err(GraphError::InvalidStatementParent {
                        function: graph.statements[idx].data.function.clone(),
                        path: graph.statements[idx].data.path.clone(),
                    });
                }
                graph.statements[idx].parent = Some(id);
                graph.statements[id.0].children.push(StatementId(idx));
            }
        }
        // Native statement paths use dense sequence indices per edge:
        // root statement indexes are 0,1,... and every parent.edge child
        // sequence independently starts at 0. A missing sibling can change
        // execution order or silently remove a statement from the snapshot.
        let mut root_positions: HashMap<String, Vec<usize>> = HashMap::new();
        let mut child_positions: HashMap<(String, String, String), Vec<usize>> =
            HashMap::new();
        for node in &graph.statements {
            let item = &node.data;
            let parsed = if let Some(parent) = &item.parent {
                let rest = item.path.strip_prefix(&(parent.clone() + "."));
                rest.and_then(|r| r.split_once('.')).and_then(|(edge, index)| {
                    let position = index.parse::<usize>().ok()?;
                    if edge.is_empty() || position.to_string() != index {
                        return None;
                    }
                    child_positions.entry((
                        item.function.clone(), parent.clone(), edge.to_owned(),
                    )).or_default().push(position);
                    Some(())
                })
            } else {
                item.path.parse::<usize>().ok().and_then(|position| {
                    if position.to_string() != item.path { return None; }
                    root_positions.entry(item.function.clone())
                        .or_default().push(position);
                    Some(())
                })
            };
            if parsed.is_none() {
                return Err(GraphError::InvalidStatementParent {
                    function: item.function.clone(), path: item.path.clone(),
                });
            }
        }
        let dense = |positions: &mut Vec<usize>| {
            positions.sort_unstable();
            positions.iter().enumerate().all(|(expected, actual)|
                expected == *actual)
        };
        for (function, positions) in &mut root_positions {
            if !dense(positions) {
                return Err(GraphError::InvalidStatementParent {
                    function: function.clone(), path: format!("roots {positions:?}"),
                });
            }
        }
        for ((function, parent, edge), positions) in &mut child_positions {
            if !dense(positions) {
                return Err(GraphError::InvalidStatementParent {
                    function: function.clone(),
                    path: format!("{parent}.{edge} {positions:?}"),
                });
            }
        }
        for idx in 0..graph.expressions.len() {
            let item = &graph.expressions[idx].data;
            if let Some(ref path) = item.parent {
                let key = (item.function.clone(), item.statement_path.clone(), path.clone());
                let parent = *emap.get(&key)
                    .ok_or(GraphError::MissingExpression {
                        function: key.0, statement: key.1, path: key.2,
                    })?;
                let rest = graph.expressions[idx].data.path
                    .strip_prefix(&(path.clone() + "."));
                // Expression child paths: parent.index, with no skipped levels.
                let direct = rest.is_some_and(|r| !r.is_empty() &&
                    !r.contains('.') && r.parse::<usize>().is_ok());
                if parent.0 == idx || !direct {
                    return Err(GraphError::InvalidExpressionParent {
                        function: graph.expressions[idx].data.function.clone(),
                        statement: graph.expressions[idx].data.statement_path.clone(),
                        path: graph.expressions[idx].data.path.clone(),
                    });
                }
                graph.expressions[idx].parent = Some(parent);
                graph.expressions[parent.0].operands.push(ExpressionId(idx));
            } else {
                // Native emit_expr always assigns path "0" to the root of
                // each statement expression. An orphan with another path is
                // a truncated/malformed graph, not an independent root.
                if item.path != "0" {
                    return Err(GraphError::InvalidExpressionParent {
                        function: item.function.clone(),
                        statement: item.statement_path.clone(),
                        path: item.path.clone(),
                    });
                }
                let stmt = graph.expressions[idx].statement;
                if graph.statements[stmt.0].expression.replace(ExpressionId(idx)).is_some() {
                    return Err(GraphError::MultipleStatementExpressions {
                        function: graph.statements[stmt.0].data.function.clone(),
                        path: graph.statements[stmt.0].data.path.clone(),
                    });
                }
            }
        }
        // Preserve operand order from sequence-indexed paths (not callback order).
        let expr_order: Vec<usize> = graph.expressions.iter()
            .map(|n| graph_path_index(&n.data.path)).collect();
        // Structured statements use parent.edge.index paths. Sorting only
        // the last component interleaves different branches (e.g. 0.1.0
        // before 0.0.1), depending on native callback arrival order.
        let stmt_paths: Vec<String> = graph.statements.iter()
            .map(|n| n.data.path.clone()).collect();
        for node in &mut graph.expressions {
            node.operands.sort_by_key(|id| expr_order[id.0]);
        }
        for node in &mut graph.statements {
            node.children.sort_by(|a,b|
                compare_tree_paths(&stmt_paths[a.0],&stmt_paths[b.0]));
        }
        // Native event 70 names a compiler-generated eok_cast wrapper and
        // its *pre-conversion* pointer source type. Do not infer this from a
        // syntactically similar user-written cast: explicit C casts permit
        // conversions that are forbidden as implicit call arguments.
        for implicit in &il.implicit_pointer_casts {
            let bad=||GraphError::InvalidImplicitPointerCast {
                function:implicit.function.clone(),
                statement:implicit.statement_path.clone(),path:implicit.path.clone(),
            };
            let key=(implicit.function.clone(),implicit.statement_path.clone(),
                implicit.path.clone());
            let id=*emap.get(&key).ok_or_else(bad)?;
            let expr=&graph.expressions[id.0];
            if unsafe {edgcpp_codegen_kind(2,expr.data.kind)}!=3 ||
                expr.data.operator.map(|op|unsafe {edgcpp_codegen_kind(3,op)})!=Some(16) ||
                expr.operands.len()!=1 ||
                expr.data.type_id.is_none() ||
                expr.operands.first().and_then(|child|
                    graph.expressions[child.0].data.type_id)!=Some(implicit.source_type) ||
                !type_map.contains_key(&implicit.source_type) ||
                graph.implicit_pointer_cast_source_by_expression
                    .insert(id,implicit.source_type).is_some()
            {return Err(bad());}
        }
        // Every call expression must carry its semantic operand offset.
        // A lost call-site event must not silently turn an ordinary function
        // call into an unrelated expression tree.
        for node in &graph.expressions {
            if native_call_argument_offset(&node.data).is_some()
                && !call_paths.contains(&(
                    node.data.function.clone(),
                    node.data.statement_path.clone(),
                    node.data.path.clone(),
                ))
            {
                return Err(GraphError::MissingCallMetadata {
                    function: node.data.function.clone(),
                    statement: node.data.statement_path.clone(),
                    path: node.data.path.clone(),
                });
            }
        }
        // A call's first operand is its callee; member calls additionally
        // carry an object operand. Missing required operands indicate a
        // truncated native snapshot rather than a valid zero-argument call.
        for call in &graph.calls {
            let key = (call.function.clone(), call.statement_path.clone(), call.path.clone());
            let id = emap[&key];
            if native_call_argument_offset(&graph.expressions[id.0].data)
                != Some(call.first_argument)
                || graph.expressions[id.0].operands.len() < call.first_argument as usize
            {
                return Err(GraphError::InvalidCallOperands {
                    function: call.function.clone(),
                    statement: call.statement_path.clone(),
                    path: call.path.clone(),
                });
            }
        }
        // EDG call operands have semantic positions. A count alone is not
        // sufficient: gaps (0.0, 0.2) would shift callee/object/arguments.
        for call in &graph.calls {
            let key = (call.function.clone(), call.statement_path.clone(), call.path.clone());
            let id = emap[&key];
            for (index, operand) in graph.expressions[id.0].operands.iter().enumerate() {
                if graph.expressions[operand.0].data.path
                    != format!("{}.{}", call.path, index)
                {
                    return Err(GraphError::InvalidCallOperandPath {
                        function: call.function.clone(),
                        statement: call.statement_path.clone(),
                        path: call.path.clone(),
                    });
                }
            }
        }
        // Every expression's child indices must be dense. Otherwise a
        // truncated native snapshot can silently change operand semantics,
        // even when the parent is not a function call.
        for node in &graph.expressions {
            for (index, child) in node.operands.iter().enumerate() {
                if graph.expressions[child.0].data.path
                    != format!("{}.{}", node.data.path, index)
                {
                    return Err(GraphError::InvalidExpressionOperandPath {
                        function: node.data.function.clone(),
                        statement: node.data.statement_path.clone(),
                        path: node.data.path.clone(),
                    });
                }
            }
        }
        // The native snapshot assigns lifetime IDs in preorder *within*
        // each function. Parent regions always appear before their children.
        let mut lifetime_positions: HashMap<SymbolId, Vec<u32>> = HashMap::new();
        for region in &il.object_lifetimes {
            if !has_kind(region.routine, 1) ||
                region.parent.is_some_and(|parent| parent >= region.id)
            {
                return Err(GraphError::InvalidLifetime {
                    routine: region.routine, id: region.id,
                });
            }
            if graph.lifetime_index.insert((region.routine, region.id),
                                            graph.object_lifetimes.len()).is_some() {
                return Err(GraphError::DuplicateLifetime {
                    routine: region.routine, id: region.id,
                });
            }
            lifetime_positions.entry(region.routine).or_default().push(region.id);
            graph.object_lifetimes.push(region.clone());
        }
        for region in &graph.object_lifetimes {
            if region.parent.is_some_and(|parent|
                !graph.lifetime_index.contains_key(&(region.routine, parent)))
            {
                return Err(GraphError::InvalidLifetime {
                    routine: region.routine, id: region.id,
                });
            }
        }
        for (routine, mut ids) in lifetime_positions {
            ids.sort_unstable();
            if ids.iter().enumerate().any(|(index, &id)| id != index as u32) {
                return Err(GraphError::InvalidLifetime { routine, id: ids[0] });
            }
        }
        for item in &il.scope_lifetimes {
            if !scope_keys.contains(&(item.routine, item.scope))
                || !graph.lifetime_index.contains_key(&(item.routine, item.lifetime))
            {
                return Err(GraphError::InvalidScopeLifetime {
                    routine: item.routine, scope: item.scope,
                });
            }
            if graph.scope_lifetime_index.insert(
                (item.routine, item.scope), graph.scope_lifetimes.len()).is_some()
            {
                return Err(GraphError::DuplicateScopeLifetime {
                    routine: item.routine, scope: item.scope,
                });
            }
            graph.scope_lifetimes.push(item.clone());
        }
        let routine_names: HashMap<String, SymbolId> =
            if graph.routine_by_function.is_empty() {
                graph.symbols.iter().filter(|s| s.kind == 1)
                    .map(|s| (s.name.clone(), s.id)).collect()
            } else {
                graph.routine_by_function.clone()
            };
        for item in &il.initialization_lifetimes {
            let key = (item.function.clone(), item.statement_path.clone());
            let owner = routine_names.get(&item.function);
            if owner.is_none()
                || (item.lifetime.is_none() && item.destructor.is_none())
                || !init_keys.contains(&key)
                || item.lifetime.is_some_and(|id|
                    owner.is_none_or(|routine| !graph.lifetime_index.contains_key(&(*routine, id))))
                || item.destructor.is_some_and(|symbol|
                    !has_kind(symbol, 1))
            {
                return Err(GraphError::InvalidInitializationLifetime {
                    function: key.0, path: key.1,
                });
            }
            if graph.initialization_lifetime_index.insert(
                key.clone(), graph.initialization_lifetimes.len()).is_some()
            {
                return Err(GraphError::DuplicateInitializationLifetime {
                    function: key.0, path: key.1,
                });
            }
            graph.initialization_lifetimes.push(item.clone());
        }
        let mut destruction_positions: HashMap<(SymbolId, u32), Vec<u32>> = HashMap::new();
        let mut seen_destructions = std::collections::HashSet::new();
        for item in &il.lifetime_destructions {
            let key = (item.routine, item.lifetime);
            if !graph.lifetime_index.contains_key(&key)
                || item.variable.is_some_and(|symbol|
                    !has_kind(symbol, 2))
                || item.destructor.is_some_and(|symbol|
                    !has_kind(symbol, 1))
            {
                return Err(GraphError::InvalidLifetimeDestruction {
                    routine: item.routine, lifetime: item.lifetime,
                });
            }
            if !seen_destructions.insert((item.routine, item.lifetime, item.position)) {
                return Err(GraphError::DuplicateLifetimeDestruction {
                    routine: item.routine, lifetime: item.lifetime,
                    position: item.position,
                });
            }
            destruction_positions.entry(key).or_default().push(item.position);
            graph.lifetime_destruction_index.entry(key).or_default()
                .push(graph.lifetime_destructions.len());
            graph.lifetime_destructions.push(item.clone());
        }
        for ((routine, lifetime), mut positions) in destruction_positions {
            positions.sort_unstable();
            if positions.iter().enumerate().any(|(i, &pos)| pos != i as u32) {
                return Err(GraphError::InvalidLifetimeDestructionPosition { routine, lifetime });
            }
        }
        for indices in graph.lifetime_destruction_index.values_mut() {
            indices.sort_by_key(|&i| graph.lifetime_destructions[i].position);
        }
        let valid_position = |loc: &SourceLocation|
            loc.sequence != 0 && loc.line != 0 && !loc.file.is_empty();
        for record in &il.statement_sources {
            let key = (record.function.clone(), record.path.clone());
            if !valid_position(&record.location) || !smap.contains_key(&key) {
                return Err(GraphError::InvalidSourceLocation {
                    function: key.0, path: key.1,
                });
            }
            if graph.statement_source_index.insert(key.clone(), graph.statement_sources.len()).is_some() {
                return Err(GraphError::DuplicateSourceLocation {
                    function: key.0, path: key.1,
                });
            }
            graph.statement_sources.push(record.clone());
        }
        for record in &il.symbol_sources {
            if !valid_position(&record.location) || !symbol_index.contains_key(&record.symbol) {
                return Err(GraphError::InvalidSymbolSource(record.symbol));
            }
            if graph.symbol_source_index.insert(record.symbol, graph.symbol_sources.len()).is_some() {
                return Err(GraphError::DuplicateSymbolSource(record.symbol));
            }
            graph.symbol_sources.push(record.clone());
        }
        for record in &il.expression_sources {
            let key = (record.function.clone(), record.statement_path.clone(),
                       record.role, record.path.clone());
            let valid_expr = if record.role == 0 {
                emap.contains_key(&(record.function.clone(), record.statement_path.clone(),
                                    record.path.clone()))
            } else {
                graph.control_expr_index.contains_key(&key)
            };
            if !valid_position(&record.location) || !valid_expr {
                return Err(GraphError::InvalidSourceLocation {
                    function: record.function.clone(), path: record.path.clone(),
                });
            }
            if graph.expression_source_index.insert(key, graph.expression_sources.len()).is_some() {
                return Err(GraphError::DuplicateSourceLocation {
                    function: record.function.clone(), path: record.path.clone(),
                });
            }
            graph.expression_sources.push(record.clone());
        }
        Ok(graph)
    }
    /// Branch/label record belonging to a statement, if present.
    pub fn statement_label(&self, statement: StatementId) -> Option<&StatementLabel> {
        self.statement_label_index.get(&statement).map(|&idx| &self.statement_labels[idx])
    }

    /// Resolve a label definition included in the current snapshot.
    pub fn label_target(&self, label: SymbolId) -> Option<StatementId> {
        self.label_definitions.get(&label).copied()
    }

    /// Resolve a captured goto statement to its target statement, if captured.
    pub fn goto_target(&self, statement: StatementId) -> Option<StatementId> {
        let info = self.statement_label(statement)?;
        (!info.definition).then(|| self.label_target(info.label)).flatten()
    }

    /// Explicit parameters of a defined routine, in declaration order.
    pub fn function_parameters(&self, routine: SymbolId)
        -> impl Iterator<Item = &FunctionParameter> {
        self.function_parameters.iter().filter(move |p|
            p.routine == routine && !p.implicit_this)
    }

    /// The implicit C++ `this` parameter, if EDG retained it.
    pub fn implicit_this(&self, routine: SymbolId) -> Option<&FunctionParameter> {
        self.function_parameters.iter().find(|p| p.routine == routine && p.implicit_this)
    }

    /// Lexical scopes belonging to a function, including the function root.
    pub fn local_scopes(&self, routine: SymbolId) -> impl Iterator<Item = &LocalScope> {
        self.local_scopes.iter().filter(move |scope| scope.routine == routine)
    }

    /// Look up a function's lexical scope by its compilation-local ID.
    pub fn local_scope(&self, routine: SymbolId, id: u32) -> Option<&LocalScope> {
        self.local_scopes.iter().find(|scope|
            scope.routine == routine && scope.id == id)
    }

    /// Locals declared in this scope (not in its descendants).
    pub fn locals_in_scope(&self, routine: SymbolId, scope: u32)
        -> impl Iterator<Item = &LocalVariable> {
        self.local_variables.iter().filter(move |var|
            var.routine == routine && var.scope == scope)
    }

    /// Static locals belonging to a routine, including variables that EDG
    /// hoisted into file-scope memory. Their exact lexical scope may be unknown.
    pub fn static_locals(&self, routine: SymbolId)
        -> impl Iterator<Item = &StaticLocalOwner> {
        self.static_local_owners.iter().filter(move |entry| entry.routine == routine)
    }

    pub fn static_local_owner(&self, symbol: SymbolId) -> Option<SymbolId> {
        self.static_local_owners.iter().find(|entry| entry.symbol == symbol)
            .map(|entry| entry.routine)
    }

    /// The exact declaring lexical scope of a local variable.
    pub fn local_variable_scope(&self, symbol: SymbolId) -> Option<&LocalScope> {
        let local = self.local_variables.iter().find(|var| var.symbol == symbol)?;
        self.local_scope(local.routine, local.scope)
    }

    /// EDG object-lifetime region identified within a routine.
    pub fn object_lifetime(&self, routine: SymbolId, id: u32)
        -> Option<&ObjectLifetime> {
        self.lifetime_index.get(&(routine, id)).map(|&i| &self.object_lifetimes[i])
    }

    /// Associated runtime lifetime region, if EDG created one for the scope.
    pub fn scope_lifetime(&self, routine: SymbolId, scope: u32)
        -> Option<&ObjectLifetime> {
        let link = self.scope_lifetime_index.get(&(routine, scope))?;
        let item = &self.scope_lifetimes[*link];
        self.object_lifetime(routine, item.lifetime)
    }

    /// Lifetime/destructor of an executed initializer, if retained.
    pub fn initialization_lifetime(&self, function: &str, path: &str)
        -> Option<&InitializationLifetime> {
        self.initialization_lifetime_index.get(&(function.to_owned(), path.to_owned()))
            .map(|&i| &self.initialization_lifetimes[i])
    }

    /// Destruction entries in EDG cleanup order for a lifetime region.
    pub fn lifetime_destructions(&self, routine: SymbolId, lifetime: u32)
        -> Vec<&LifetimeDestruction> {
        self.lifetime_destruction_index.get(&(routine, lifetime))
            .map(|indices| indices.iter().map(|&i| &self.lifetime_destructions[i]).collect())
            .unwrap_or_default()
    }

    /// Source file, line and column of a captured statement.
    pub fn statement_source(&self, function: &str, path: &str)
        -> Option<&SourceLocation> {
        self.statement_source_index.get(&(function.to_owned(), path.to_owned()))
            .map(|&idx| &self.statement_sources[idx].location)
    }

    /// Source location of a variable, routine, or field declaration.
    pub fn symbol_source(&self, symbol: SymbolId) -> Option<&SourceLocation> {
        self.symbol_source_index.get(&symbol)
            .map(|&idx| &self.symbol_sources[idx].location)
    }

    /// Source location of a primary or supplemental expression node.
    /// Role 0 is the primary expression; roles 1..=4 are supplemental trees.
    pub fn expression_source(&self, function: &str, statement: &str,
                             role: u32, path: &str) -> Option<&SourceLocation> {
        self.expression_source_index.get(&(function.to_owned(), statement.to_owned(),
                                            role, path.to_owned()))
            .map(|&idx| &self.expression_sources[idx].location)
    }

    /// Runtime VLA dimension evaluated at a specific statement execution site.
    /// This is intentionally separate from the array's declaration.
    pub fn vla_dimension_at(&self, function:&str, path:&str) -> Option<&VlaDimension> {
        self.vla_dimensions.iter().find(|dim|
            dim.function==function && dim.statement_path==path)
    }

    /// Variable declarations belonging to a statement, in EDG list order.
    pub fn declarations_at<'a>(&'a self, function: &'a str, path: &'a str)
        -> impl Iterator<Item = &'a StatementDeclaration> + 'a {
        self.statement_declarations.iter().filter(move |decl|
            decl.function == function && decl.statement_path == path)
    }

    /// Captured declaration sites of one variable. The same symbol can
    /// appear at more than one source/IL statement site.
    pub fn declaration_sites(&self, symbol: SymbolId)
        -> impl Iterator<Item = &StatementDeclaration> {
        self.statement_declarations.iter().filter(move |decl| decl.symbol == symbol)
    }

    /// The dynamic initializer executed by a given statement, if present.
    pub fn statement_initialization(
        &self, function: &str, path: &str,
    ) -> Option<&StatementInitialization> {
        self.statement_initializations.iter().find(|init|
            init.function == function && init.statement_path == path)
    }

    /// All initializer execution sites associated with a variable.
    pub fn variable_initialization_sites(&self, symbol: SymbolId)
        -> impl Iterator<Item = &StatementInitialization> {
        self.statement_initializations.iter().filter(move |init| init.symbol == Some(symbol))
    }

    /// Root expression of a statement initializer, when EDG exposes one.
    pub fn statement_initializer_expression(
        &self, function: &str, path: &str,
    ) -> Option<&ControlExpression> {
        self.control_expression_root(function, path, 4)
    }

    /// Flags supplied by EDG for a statement in a function.
    pub fn statement_attributes(&self, function: &str, path: &str)
        -> Option<&StatementAttributes> {
        self.statement_attributes.iter().find(|a|
            a.function == function && a.statement_path == path)
    }

    /// Root of a supplemental control-expression tree.
    pub fn control_expression_root(
        &self, function: &str, statement: &str, role: u32,
    ) -> Option<&ControlExpression> {
        self.control_expression(function, statement, role, "0")
    }

    /// Resolve one expression in a supplemental control-expression tree.
    pub fn control_expression(
        &self, function: &str, statement: &str, role: u32, path: &str,
    ) -> Option<&ControlExpression> {
        self.control_expr_index.get(&(function.to_owned(), statement.to_owned(),
                                      role, path.to_owned()))
            .map(|&idx| &self.control_expressions[idx])
    }

    /// Immediate operands of a supplemental control expression in EDG order.
    pub fn control_operands<'a>(&'a self, expr: &ControlExpression)
        -> Vec<&'a ControlExpression> {
        self.control_expr_children.get(&(expr.function.clone(), expr.statement_path.clone(),
                                         expr.role, expr.path.clone()))
            .map(|children| children.iter().map(|&idx| &self.control_expressions[idx]).collect())
            .unwrap_or_default()
    }

    /// Root statements belonging to the specified function.
    pub fn roots<'a>(&'a self, function: &'a str)
        -> impl Iterator<Item = (StatementId, &'a StatementNode)> + 'a {
        // A native callback can deliver complete statement records out of
        // order. In particular, top-level roots have no parent to sort them.
        let mut roots: Vec<_> = self.statements.iter().enumerate()
            .filter(move |(_, n)| n.parent.is_none() && n.data.function == function)
            .map(|(i, n)| (StatementId(i), n)).collect();
        roots.sort_by(|a,b|compare_tree_paths(&a.1.data.path,&b.1.data.path));
        roots.into_iter()
    }
    /// Visit structured statements in depth-first pre-order, retaining the
    /// original child order. The callback can stop traversal early by returning
    /// false; this avoids allocating a flattened statement list.
    pub fn walk_statements(
        &self,
        function: &str,
        mut visit: impl FnMut(StatementId, &StatementNode) -> bool,
    ) {
        let mut stack: Vec<_> = self.roots(function).map(|(id, _)| id).collect();
        stack.reverse();
        while let Some(id) = stack.pop() {
            let Some(node) = self.statement(id) else { continue };
            if !visit(id, node) { break; }
            stack.extend(node.children.iter().rev().copied());
        }
    }

    /// Visit an expression tree in pre-order, following operand order.
    /// Invalid IDs are ignored; graphs created by from_il have valid edges.
    pub fn walk_expression(
        &self,
        root: ExpressionId,
        mut visit: impl FnMut(ExpressionId, &ExpressionNode) -> bool,
    ) {
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let Some(node) = self.expression(id) else { continue };
            if !visit(id, node) { break; }
            stack.extend(node.operands.iter().rev().copied());
        }
    }

    /// Visit statements in post-order: children before their parent.
    /// Returning false terminates the entire traversal.
    pub fn walk_statements_postorder(
        &self,
        function: &str,
        mut visit: impl FnMut(StatementId, &StatementNode) -> bool,
    ) {
        let mut stack: Vec<_> = self.roots(function)
            .map(|(id, _)| (id, false)).collect();
        stack.reverse();
        while let Some((id, expanded)) = stack.pop() {
            let Some(node) = self.statement(id) else { continue };
            if expanded {
                if !visit(id, node) { break; }
            } else {
                stack.push((id, true));
                stack.extend(node.children.iter().rev().map(|id| (*id, false)));
            }
        }
    }

    /// Visit an expression tree in post-order, with operands before users.
    /// Returning false terminates the entire traversal.
    pub fn walk_expression_postorder(
        &self,
        root: ExpressionId,
        mut visit: impl FnMut(ExpressionId, &ExpressionNode) -> bool,
    ) {
        let mut stack = vec![(root, false)];
        while let Some((id, expanded)) = stack.pop() {
            let Some(node) = self.expression(id) else { continue };
            if expanded {
                if !visit(id, node) { break; }
            } else {
                stack.push((id, true));
                stack.extend(node.operands.iter().rev().map(|id| (*id, false)));
            }
        }
    }

    /// Visit primary expression trees of a function in post-order.
    /// Statements are visited in statement post-order as well.
    pub fn walk_function_expressions_postorder(
        &self,
        function: &str,
        mut visit: impl FnMut(StatementId, ExpressionId, &ExpressionNode) -> bool,
    ) {
        let mut continuing = true;
        self.walk_statements_postorder(function, |statement_id, statement| {
            if let Some(root) = statement.expression {
                self.walk_expression_postorder(root, |expression_id, expression| {
                    continuing = visit(statement_id, expression_id, expression);
                    continuing
                });
            }
            continuing
        });
    }

    /// Visit all primary expression trees attached to a function's statements.
    /// Statements and expression operands are visited in their IL order.
    /// Returning false stops the entire traversal.
    pub fn walk_function_expressions(
        &self,
        function: &str,
        mut visit: impl FnMut(StatementId, ExpressionId, &ExpressionNode) -> bool,
    ) {
        let mut continuing = true;
        self.walk_statements(function, |statement_id, statement| {
            if let Some(root) = statement.expression {
                self.walk_expression(root, |expression_id, node| {
                    continuing = visit(statement_id, expression_id, node);
                    continuing
                });
            }
            continuing
        });
    }

    /// Walk primary and supplemental control expressions together in statement
    /// order. Primary expression nodes precede supplemental controls for each
    /// statement. Returning false stops all subsequent traversal.
    pub fn walk_function_all_expressions(
        &self,
        function: &str,
        mut visit: impl FnMut(StatementId, FunctionExpressionRef<'_>) -> bool,
    ) {
        let mut continuing = true;
        self.walk_statements(function, |statement_id, statement| {
            if let Some(root) = statement.expression {
                self.walk_expression(root, |_, node| {
                    continuing = visit(statement_id, FunctionExpressionRef::Primary(node));
                    continuing
                });
            }
            if !continuing { return false; }
            if let Some(indices) = self.control_expr_by_statement
                .get(&(function.to_owned(), statement.data.path.clone()))
            {
                for &index in indices {
                    if !visit(statement_id,
                        FunctionExpressionRef::Control(&self.control_expressions[index]))
                    {
                        return false;
                    }
                }
            }
            true
        });
    }

    /// Collect all referenced symbol IDs from primary expression trees in
    /// first-encounter order, without duplicates. Supplemental control and
    /// initializer expressions are not included.
    pub fn referenced_symbols(&self, function: &str) -> Vec<SymbolId> {
        let mut seen = std::collections::HashSet::new();
        let mut symbols = Vec::new();
        self.walk_function_expressions(function, |_, _, node| {
            if let Some(id) = node.data.symbol_id {
                if seen.insert(id) { symbols.push(id); }
            }
            true
        });
        symbols
    }

    /// All symbol references captured for a function, including supplemental
    /// control-expression trees. Deduplicated in first-encounter order.
    /// Variable and constructor initializer trees belong to symbols/routines
    /// and are deliberately not attributed to a function here.
    pub fn referenced_symbols_including_controls(&self, function: &str) -> Vec<SymbolId> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        self.walk_function_all_expressions(function, |_, expression| {
            let id = match expression {
                FunctionExpressionRef::Primary(node) => node.data.symbol_id,
                FunctionExpressionRef::Control(node) => node.symbol_id,
            };
            if let Some(id) = id {
                if seen.insert(id) { result.push(id); }
            }
            true
        });
        result
    }

    /// Visit supplemental control expressions in statement order, then role and
    /// numeric tree-path order. Returning false stops the entire walk.
    pub fn walk_function_control_expressions(
        &self, function: &str,
        mut visit: impl FnMut(StatementId, &ControlExpression) -> bool,
    ) {
        let mut continuing = true;
        self.walk_statements(function, |statement_id, statement| {
            if let Some(indices) = self.control_expr_by_statement
                .get(&(function.to_owned(), statement.data.path.clone()))
            {
                for &index in indices {
                    if !visit(statement_id, &self.control_expressions[index]) {
                        continuing = false;
                        break;
                    }
                }
            }
            continuing
        });
    }

    /// Visit a variable's initializer expression tree in numeric path order.
    /// Returning false stops traversal. Missing owners produce no callbacks.
    pub fn walk_initializer_expressions(
        &self, owner: SymbolId,
        mut visit: impl FnMut(&InitializerExpression) -> bool,
    ) {
        let mut nodes: Vec<_> = self.initializer_expressions(owner).collect();
        nodes.sort_by(|a, b| compare_tree_paths(&a.path, &b.path));
        for node in nodes {
            if !visit(node) { break; }
        }
    }

    /// Visit constructor initializer expressions in initializer-position and
    /// numeric tree-path order, stopping when the callback returns false.
    pub fn walk_constructor_expressions(
        &self, routine: SymbolId,
        mut visit: impl FnMut(&ConstructorInitializerExpression) -> bool,
    ) {
        let mut nodes: Vec<_> = self.constructor_initializer_expressions.iter()
            .filter(|node| node.routine == routine).collect();
        nodes.sort_by(|a, b| a.position.cmp(&b.position)
            .then_with(|| compare_tree_paths(&a.path, &b.path)));
        for node in nodes {
            if !visit(node) { break; }
        }
    }

    /// Visit every expression associated with a function, including constructor
    /// initializer expressions when the routine symbol is supplied.
    /// Function-body expressions precede constructor initializers.
    pub fn walk_routine_expressions(
        &self,
        function: &str,
        routine: SymbolId,
        mut visit: impl FnMut(AnyExpressionRef<'_>) -> bool,
    ) {
        let mut continuing = true;
        self.walk_function_all_expressions(function, |_, expression| {
            continuing = visit(match expression {
                FunctionExpressionRef::Primary(node) => AnyExpressionRef::Primary(node),
                FunctionExpressionRef::Control(node) => AnyExpressionRef::Control(node),
            });
            continuing
        });
        if !continuing { return; }
        self.walk_constructor_expressions(routine, |node| {
            visit(AnyExpressionRef::ConstructorInitializer(node))
        });
    }

    /// Visit expressions in a variable initializer through the same borrowed
    /// expression representation used by routine traversal.
    pub fn walk_variable_expressions(
        &self,
        owner: SymbolId,
        mut visit: impl FnMut(AnyExpressionRef<'_>) -> bool,
    ) {
        self.walk_initializer_expressions(owner, |node| {
            visit(AnyExpressionRef::VariableInitializer(node))
        });
    }

    /// Count all expression nodes associated with a routine, including
    /// supplemental controls and constructor initializers.
    pub fn routine_expression_count(&self, function: &str, routine: SymbolId) -> usize {
        let mut count = 0;
        self.walk_routine_expressions(function, routine, |_| {
            count += 1;
            true
        });
        count
    }

    /// Count all expression nodes associated with a variable initializer.
    pub fn variable_expression_count(&self, owner: SymbolId) -> usize {
        let mut count = 0;
        self.walk_variable_expressions(owner, |_| {
            count += 1;
            true
        });
        count
    }

    /// Visit expressions referencing a particular symbol in a routine.
    /// Returning false stops traversal immediately.
    pub fn walk_routine_symbol_uses(
        &self,
        function: &str,
        routine: SymbolId,
        symbol: SymbolId,
        mut visit: impl FnMut(AnyExpressionRef<'_>) -> bool,
    ) {
        self.walk_routine_expressions(function, routine, |expression| {
            expression.symbol_id() != Some(symbol) || visit(expression)
        });
    }

    /// Visit expressions referencing a particular symbol in a variable
    /// initializer. Returning false stops traversal immediately.
    pub fn walk_variable_symbol_uses(
        &self,
        owner: SymbolId,
        symbol: SymbolId,
        mut visit: impl FnMut(AnyExpressionRef<'_>) -> bool,
    ) {
        self.walk_variable_expressions(owner, |expression| {
            expression.symbol_id() != Some(symbol) || visit(expression)
        });
    }

    /// Count references to each symbol in a routine, in first-use order.
    /// Repeated references in the same expression tree are counted separately.
    pub fn routine_symbol_use_counts(
        &self,
        function: &str,
        routine: SymbolId,
    ) -> Vec<SymbolUseCount> {
        let mut positions: HashMap<SymbolId, usize> = HashMap::new();
        let mut counts: Vec<SymbolUseCount> = Vec::new();
        self.walk_routine_expressions(function, routine, |expression| {
            if let Some(symbol) = expression.symbol_id() {
                if let Some(&index) = positions.get(&symbol) {
                    counts[index].count += 1;
                } else {
                    positions.insert(symbol, counts.len());
                    counts.push(SymbolUseCount { symbol, count: 1 });
                }
            }
            true
        });
        counts
    }

    /// Count references to each symbol in a variable initializer.
    pub fn variable_symbol_use_counts(&self, owner: SymbolId) -> Vec<SymbolUseCount> {
        let mut positions: HashMap<SymbolId, usize> = HashMap::new();
        let mut counts: Vec<SymbolUseCount> = Vec::new();
        self.walk_variable_expressions(owner, |expression| {
            if let Some(symbol) = expression.symbol_id() {
                if let Some(&index) = positions.get(&symbol) {
                    counts[index].count += 1;
                } else {
                    positions.insert(symbol, counts.len());
                    counts.push(SymbolUseCount { symbol, count: 1 });
                }
            }
            true
        });
        counts
    }

    /// Summarize all captured expression categories in a routine.
    pub fn routine_expression_stats(&self, function: &str, routine: SymbolId) -> ExpressionStats {
        let mut stats = ExpressionStats::default();
        let mut seen = std::collections::HashSet::new();
        self.walk_routine_expressions(function, routine, |expression| {
            match expression {
                AnyExpressionRef::Primary(_) => stats.primary += 1,
                AnyExpressionRef::Control(_) => stats.control += 1,
                AnyExpressionRef::VariableInitializer(_) => stats.variable_initializers += 1,
                AnyExpressionRef::ConstructorInitializer(_) => stats.constructor_initializers += 1,
            }
            if let Some(symbol) = expression.symbol_id() {
                stats.symbol_uses += 1;
                if seen.insert(symbol) { stats.distinct_symbols += 1; }
            }
            true
        });
        stats
    }

    /// Summarize the captured initializer tree for a variable.
    pub fn variable_expression_stats(&self, owner: SymbolId) -> ExpressionStats {
        let mut stats = ExpressionStats::default();
        let mut seen = std::collections::HashSet::new();
        self.walk_variable_expressions(owner, |expression| {
            stats.variable_initializers += 1;
            if let Some(symbol) = expression.symbol_id() {
                stats.symbol_uses += 1;
                if seen.insert(symbol) { stats.distinct_symbols += 1; }
            }
            true
        });
        stats
    }

    /// Collect unique symbol references across function-body and constructor
    /// initializer expressions, preserving first-encounter order.
    pub fn routine_referenced_symbols(&self, function: &str, routine: SymbolId) -> Vec<SymbolId> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        self.walk_routine_expressions(function, routine, |expression| {
            let symbol = match expression {
                AnyExpressionRef::Primary(node) => node.data.symbol_id,
                AnyExpressionRef::Control(node) => node.symbol_id,
                AnyExpressionRef::VariableInitializer(node) => node.symbol_id,
                AnyExpressionRef::ConstructorInitializer(node) => node.symbol_id,
            };
            if let Some(symbol) = symbol {
                if seen.insert(symbol) { result.push(symbol); }
            }
            true
        });
        result
    }

    /// Collect unique variable-initializer symbol references in numeric
    /// expression-path order.
    pub fn variable_referenced_symbols(&self, owner: SymbolId) -> Vec<SymbolId> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        self.walk_variable_expressions(owner, |expression| {
            if let AnyExpressionRef::VariableInitializer(node) = expression {
                if let Some(symbol) = node.symbol_id {
                    if seen.insert(symbol) { result.push(symbol); }
                }
            }
            true
        });
        result
    }

    /// Symbol references from a variable's initializer expression tree.
    /// Returns unique symbols in numeric tree-path order.
    pub fn initializer_referenced_symbols(&self, owner: SymbolId) -> Vec<SymbolId> {
        let mut nodes: Vec<_> = self.initializer_expressions(owner).collect();
        nodes.sort_by(|a, b| compare_tree_paths(&a.path, &b.path));
        let mut seen = std::collections::HashSet::new();
        nodes.into_iter().filter_map(|node| node.symbol_id)
            .filter(|id| seen.insert(*id)).collect()
    }

    /// Symbol references from constructor initializer expressions, ordered by
    /// initializer position and then numeric tree path.
    pub fn constructor_referenced_symbols(&self, routine: SymbolId) -> Vec<SymbolId> {
        let mut nodes: Vec<_> = self.constructor_initializer_expressions.iter()
            .filter(|node| node.routine == routine).collect();
        nodes.sort_by(|a, b| a.position.cmp(&b.position)
            .then_with(|| compare_tree_paths(&a.path, &b.path)));
        let mut seen = std::collections::HashSet::new();
        nodes.into_iter().filter_map(|node| node.symbol_id)
            .filter(|id| seen.insert(*id)).collect()
    }

    /// Expression operands in the original IL order.
    pub fn operands(&self, id: ExpressionId) -> impl Iterator<Item = &ExpressionNode> {
        self.expression(id).into_iter()
            .flat_map(move |n| n.operands.iter().filter_map(move |child| self.expression(*child)))
    }
    pub fn initializer_expressions(&self, id: SymbolId) -> impl Iterator<Item = &InitializerExpression> {
        self.initializer_expressions.iter().filter(move |e| e.owner == id)
    }
    /// Root expression of a variable's dynamic initializer, if captured.
    pub fn initializer_root(&self, owner: SymbolId) -> Option<&InitializerExpression> {
        self.initializer_expression(owner, "0")
    }

    /// Look up an initializer expression by its compilation-local tree path.
    pub fn initializer_expression(&self, owner: SymbolId, path: &str)
        -> Option<&InitializerExpression> {
        self.initializer_index.get(&(owner, path.to_owned()))
            .map(|&idx| &self.initializer_expressions[idx])
    }

    /// Direct operands in source traversal order. Kind and operator values remain EDG-native.
    pub fn initializer_operands<'a>(&'a self, node: &InitializerExpression)
        -> Vec<&'a InitializerExpression> {
        self.initializer_children.get(&(node.owner, node.path.clone()))
            .map(|children| children.iter()
                .map(|&idx| &self.initializer_expressions[idx]).collect())
            .unwrap_or_default()
    }

    /// Resolve a call record by its compilation-local expression identity.
    pub fn call_at(&self, id: ExpressionId) -> Option<&CallSite> {
        self.call_index_by_expression.get(id.0).copied().flatten()
            .and_then(|index| self.calls.get(index))
    }

    /// Resolve the expression associated with a call-site record.
    ///
    /// Call-site references should come from this graph's `calls` slice.
    pub fn call_expression(&self, call: &CallSite) -> Option<ExpressionId> {
        self.call_expression_by_site.get(call).copied()
    }

    /// Callee expression (operand zero) of an EDG call.
    pub fn call_callee(&self, call: &CallSite) -> Option<ExpressionId> {
        let id = self.call_expression(call)?;
        self.expressions[id.0].operands.first().copied()
    }

    /// Inspect a *callee-only* tree for an exact EDG record-field reference
    /// named "__vptr". No source member-call classification survives by
    /// implication: the caller must independently require the event-77
    /// IndirectUnresolved record, and this evidence never authorizes codegen.
    ///
    /// A fixed bound and rejection of multiple candidates avoid silently
    /// choosing a vptr field in large/ambiguous lowered expressions. Raw
    /// constants are reported with original expression paths, but never
    /// interpreted as an Itanium slot or pointer arithmetic unit.
    pub fn callee_vptr_field_reference(
        &self, call: &CallSite
    ) -> Option<CalleeVptrFieldReference> {
        let callee=self.call_callee(call)?;
        let mut stack=vec![callee];
        let mut seen=std::collections::HashSet::new();
        let mut field=None;
        let mut integers=Vec::new();
        while let Some(id)=stack.pop() {
            if !seen.insert(id) || seen.len()>512 {return None;}
            let node=self.expressions.get(id.0)?;
            if node.data.function!=call.function ||
               node.data.statement_path!=call.statement_path {
                return None;
            }
            if unsafe {edgcpp_codegen_kind(2,node.data.kind)}==5 {
                if let Some(symbol)=node.data.symbol_id {
                    if let Some(record)=self.record_fields.iter().find(|f|
                        f.symbol==symbol && f.name=="__vptr") {
                        if node.data.reference.as_deref()!=Some("__vptr") ||
                           field.is_some() || record.bit_width!=0 {
                            return None;
                        }
                        field=Some((symbol,record.owner,node.data.path.clone(),
                                    record.byte_offset));
                    }
                }
            }
            if unsafe {edgcpp_codegen_kind(2,node.data.kind)}==1 {
                if let Some(value)=node.data.constant_integer {
                    if integers.len()>=16 {return None;}
                    integers.push((node.data.path.clone(),value));
                }
            }
            stack.extend(node.operands.iter().copied());
        }
        let (field,owner,field_path,field_byte_offset)=field?;
        integers.sort_by(|a,b|compare_tree_paths(&a.0,&b.0));
        Some(CalleeVptrFieldReference {
            field,owner,field_path,field_byte_offset,
            raw_integer_literals:integers,
        })
    }

    /// Implicit object operand for member calls; absent for ordinary calls.
    pub fn call_object(&self, call: &CallSite) -> Option<ExpressionId> {
        if call.first_argument != 2 { return None; }
        let id = self.call_expression(call)?;
        self.expressions[id.0].operands.get(1).copied()
    }

    /// Explicit argument expressions in EDG operand order.
    pub fn call_arguments(&self, call: &CallSite) -> Option<&[ExpressionId]> {
        let id = self.call_expression(call)?;
        self.expressions[id.0].operands.get(call.first_argument as usize..)
    }

    pub fn dynamic_initializer(&self, id: SymbolId) -> Option<&DynamicInitializer> {
        self.dynamic_initializers.iter().find(|v| v.symbol == id)
    }
    pub fn initializer_integer(&self, id: SymbolId) -> Option<i64> {
        self.initializer_integers.iter().find(|v| v.symbol == id).map(|v| v.value)
    }
    pub fn variable_initializer(&self, id: SymbolId) -> Option<&VariableInitializer> {
        self.variable_initializers.iter().find(|v| v.symbol == id)
    }
    /// Canonical exported IL function name for an emitted routine.
    /// Unlike Symbol.name, this is independent of the first reference's spelling.
    pub fn routine_function(&self, id: SymbolId) -> Option<&str> {
        self.routine_function_by_id.get(&id).map(String::as_str)
    }

    /// Resolve the function key of a statement body to its native routine.
    pub fn routine_for_function(&self, function: &str) -> Option<SymbolId> {
        self.routine_by_function.get(function).copied()
    }

    pub fn symbol(&self, id: SymbolId) -> Option<&Symbol> {
        self.symbols.iter().find(|s| s.id == id)
    }
    pub fn signature(&self, id: TypeId) -> Option<&FunctionSignature> {
        self.function_signatures.iter().find(|s| s.owner == id)
    }
    pub fn implicit_pointer_cast_source(&self,id:ExpressionId) -> Option<TypeId> {
        self.implicit_pointer_cast_source_by_expression.get(&id).copied()
    }
    pub fn type_detail(&self, id: TypeId) -> Option<&TypeDetail> {
        self.type_details.iter().find(|d| d.owner == id)
    }
    /// EDG special-function and dispatch metadata for this routine.
    pub fn routine_attributes(&self, routine: SymbolId) -> Option<&RoutineAttributes> {
        self.routine_attributes.iter().find(|attributes| attributes.routine == routine)
    }

    /// Declared member routines belonging to this class.
    pub fn member_functions(&self, owner: TypeId)
        -> impl Iterator<Item = &MemberFunction> {
        self.member_functions.iter().filter(move |m| m.owner == owner)
    }

    /// Owning class of a member routine, if any.
    pub fn member_function_owner(&self, routine: SymbolId) -> Option<TypeId> {
        self.member_functions.iter().find(|m| m.routine == routine).map(|m| m.owner)
    }

    /// Expression tree root for a constructor/destructor subobject initializer,
    /// if EDG captured a representable expression.
    pub fn constructor_initializer_root(
        &self, routine: SymbolId, position: u32,
    ) -> Option<&ConstructorInitializerExpression> {
        self.constructor_initializer_expression(routine, position, "0")
    }

    pub fn constructor_initializer_expression(
        &self, routine: SymbolId, position: u32, path: &str,
    ) -> Option<&ConstructorInitializerExpression> {
        self.constructor_expr_index.get(&(routine, position, path.to_owned()))
            .map(|&idx| &self.constructor_initializer_expressions[idx])
    }

    /// Direct initializer-expression operands in original EDG order.
    pub fn constructor_initializer_operands<'a>(
        &'a self, node: &ConstructorInitializerExpression,
    ) -> Vec<&'a ConstructorInitializerExpression> {
        self.constructor_expr_children.get(&(node.routine, node.position, node.path.clone()))
            .map(|children| children.iter()
                .map(|&idx| &self.constructor_initializer_expressions[idx]).collect())
            .unwrap_or_default()
    }

    /// Subobject initialization/destruction entries in EDG execution order.
    pub fn constructor_initializers(&self, routine: SymbolId)
        -> impl Iterator<Item = &ConstructorInitializer> {
        self.constructor_initializers.iter().filter(move |v| v.routine == routine)
    }

    /// Direct base access and pack-expansion details if available.
    pub fn base_access(&self, derived: TypeId, position: u32) -> Option<&BaseClassAccess> {
        self.base_class_access.iter().find(|access|
            access.derived == derived && access.position == position)
    }

    /// Direct bases in source declaration order, including virtual bases.
    pub fn bases(&self, derived: TypeId) -> impl Iterator<Item = &BaseClass> {
        self.base_classes.iter().filter(move |edge| edge.derived == derived)
    }
    pub fn fields(&self, owner: TypeId) -> impl Iterator<Item = &RecordField> {
        self.record_fields.iter().filter(move |f| f.owner == owner)
    }
    pub fn array_bound(&self, array: TypeId) -> Option<u64> {
        self.array_bounds.iter().find(|b| b.array == array).map(|b| b.elements)
    }
    pub fn parameters(&self, owner: TypeId) -> impl Iterator<Item = &TypeParameter> {
        self.type_parameters.iter().filter(move |p| p.owner == owner)
    }
    /// Declared EDG type of a routine or variable symbol, if captured.
    pub fn symbol_type(&self, symbol: SymbolId) -> Option<TypeId> {
        self.symbol_types.iter().find(|edge| edge.symbol == symbol).map(|edge| edge.ty)
    }

    /// Borrow a routine's symbol, declared type, signature and parameters.
    /// Missing optional metadata is represented by None or empty iterators.
    pub fn routine_view(&self, routine: SymbolId) -> Option<RoutineView<'_>> {
        let symbol = self.symbol(routine)?;
        if symbol.kind != 1 { return None; }
        Some(RoutineView {
            symbol,
            function: self.routine_function(routine),
            ty: self.symbol_type_view(routine),
            implicit_this: self.implicit_this(routine),
        })
    }

    /// Borrow a variable's symbol, declared type and initializer metadata.
    pub fn variable_view(&self, variable: SymbolId) -> Option<VariableView<'_>> {
        let symbol = self.symbol(variable)?;
        if symbol.kind != 2 { return None; }
        Some(VariableView {
            symbol,
            ty: self.symbol_type_view(variable),
            initializer: self.variable_initializer(variable),
        })
    }

    /// Visit explicitly declared parameters, excluding implicit C++ this.
    /// The order is the captured EDG parameter order.
    pub fn walk_routine_parameters(
        &self, routine: SymbolId, mut visit: impl FnMut(&FunctionParameter) -> bool,
    ) {
        for parameter in self.function_parameters(routine) {
            if !visit(parameter) { break; }
        }
    }

    /// Collect record field IDs' declared types, in captured field order.
    /// This is a direct dependency query, not a transitive type walk.
    pub fn record_field_types(&self, record: TypeId) -> Vec<TypeId> {
        self.fields(record).map(|field| field.ty).collect()
    }

    /// Collect direct base types in captured base declaration order.
    pub fn direct_base_types(&self, record: TypeId) -> Vec<TypeId> {
        self.bases(record).map(|base| base.base).collect()
    }

    /// Collect explicit routine parameter types in captured order.
    /// The implicit C++ this parameter is excluded.
    pub fn routine_parameter_types(&self, routine: SymbolId) -> Vec<TypeId> {
        self.function_parameters(routine).map(|parameter| parameter.ty).collect()
    }

    /// Visit direct type relationships in a stable category order:
    /// related type, direct bases, fields, then function-type parameters.
    /// Missing referenced types are still reported as IDs.
    pub fn walk_type_dependencies(
        &self, ty: TypeId, mut visit: impl FnMut(TypeDependency) -> bool,
    ) {
        if let Some(related) = self.type_def(ty).and_then(|def| def.related) {
            if !visit(TypeDependency { target: related, kind: TypeDependencyKind::Related }) {
                return;
            }
        }
        for base in self.bases(ty) {
            if !visit(TypeDependency { target: base.base, kind: TypeDependencyKind::Base }) {
                return;
            }
        }
        for field in self.fields(ty) {
            if !visit(TypeDependency { target: field.ty, kind: TypeDependencyKind::Field }) {
                return;
            }
        }
        for parameter in self.parameters(ty) {
            if !visit(TypeDependency {
                target: parameter.ty, kind: TypeDependencyKind::Parameter,
            }) {
                return;
            }
        }
    }

    /// Collect all direct type relationships, retaining repeated edges.
    pub fn type_dependencies(&self, ty: TypeId) -> Vec<TypeDependency> {
        let mut result = Vec::new();
        self.walk_type_dependencies(ty, |dependency| {
            result.push(dependency);
            true
        });
        result
    }

    /// Breadth-first, cycle-safe traversal of the captured type graph.
    /// Includes the starting type at depth zero. References to uncaptured
    /// type IDs remain visible as leaves. Returning false stops traversal.
    pub fn walk_reachable_types(
        &self, root: TypeId, mut visit: impl FnMut(ReachableType) -> bool,
    ) {
        let mut seen = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        seen.insert(root);
        queue.push_back(ReachableType { ty: root, depth: 0 });
        while let Some(node) = queue.pop_front() {
            if !visit(node) { break; }
            self.walk_type_dependencies(node.ty, |dependency| {
                if seen.insert(dependency.target) {
                    queue.push_back(ReachableType {
                        ty: dependency.target, depth: node.depth + 1,
                    });
                }
                true
            });
        }
    }

    /// Collect all reachable type IDs once, in breadth-first order.
    pub fn reachable_types(&self, root: TypeId) -> Vec<ReachableType> {
        let mut result = Vec::new();
        self.walk_reachable_types(root, |node| {
            result.push(node);
            true
        });
        result
    }

    /// Find captured types that directly reference the target type.
    /// The result is deduplicated and follows type-definition order.
    /// Only captured type definitions can act as sources.
    pub fn direct_type_dependents(&self, target: TypeId) -> Vec<TypeId> {
        let mut result = Vec::new();
        for ty in &self.types {
            let mut found = false;
            self.walk_type_dependencies(ty.id, |edge| {
                if edge.target == target {
                    found = true;
                    false
                } else {
                    true
                }
            });
            if found && !result.contains(&ty.id) {
                result.push(ty.id);
            }
        }
        result
    }

    /// Cycle-safe breadth-first traversal of reverse type dependencies.
    /// Includes the requested type at depth zero. A dependent type is
    /// reported at its shortest reverse-edge distance from the root.
    pub fn walk_type_dependents(
        &self, root: TypeId, mut visit: impl FnMut(ReachableType) -> bool,
    ) {
        let mut seen = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        seen.insert(root);
        queue.push_back(ReachableType { ty: root, depth: 0 });
        while let Some(node) = queue.pop_front() {
            if !visit(node) { break; }
            for ty in self.direct_type_dependents(node.ty) {
                if seen.insert(ty) {
                    queue.push_back(ReachableType {
                        ty, depth: node.depth + 1,
                    });
                }
            }
        }
    }

    /// Collect reverse-reachable types in breadth-first order.
    pub fn type_dependents(&self, root: TypeId) -> Vec<ReachableType> {
        let mut result = Vec::new();
        self.walk_type_dependents(root, |node| {
            result.push(node);
            true
        });
        result
    }

    /// Partition captured types into strongly connected components.
    /// Each type appears exactly once. Edges to uncaptured types are ignored
    /// for component membership. Components are emitted dependency-first.
    pub fn type_strongly_connected_components(&self) -> Vec<Vec<TypeId>> {
        // Scheduling all captured definitions visits exactly the captured
        // type graph. The schedule already condenses cycles, so reuse the
        // indexed SCC implementation rather than running a separate,
        // repeatedly scanning graph algorithm.
        let all: Vec<TypeId> = self.types.iter().map(|ty| ty.id).collect();
        self.type_dependency_index().invalidation_schedule(&all)
    }

    /// Build a reusable dependency index for repeated invalidation queries.
    /// This is a snapshot: rebuild it after changing public Graph collections.
    pub fn type_dependency_index(&self) -> TypeDependencyIndex {
        let mut index = TypeDependencyIndex::default();

        // Populate the forward index in category order with one pass over
        // each IL collection. Querying each type through type_dependencies()
        // would repeatedly scan the complete field/base/parameter tables.
        for ty in &self.types {
            if index.forward.contains_key(&ty.id) {
                continue;
            }
            index.type_order.push(ty.id);
            let mut dependencies = Vec::new();
            if let Some(target) = ty.related {
                dependencies.push(TypeDependency {
                    target, kind: TypeDependencyKind::Related,
                });
            }
            index.forward.insert(ty.id, dependencies);
        }
        for base in &self.base_classes {
            if let Some(edges) = index.forward.get_mut(&base.derived) {
                edges.push(TypeDependency {
                    target: base.base, kind: TypeDependencyKind::Base,
                });
            }
        }
        for field in &self.record_fields {
            if let Some(edges) = index.forward.get_mut(&field.owner) {
                edges.push(TypeDependency {
                    target: field.ty, kind: TypeDependencyKind::Field,
                });
            }
        }
        for parameter in &self.type_parameters {
            if let Some(edges) = index.forward.get_mut(&parameter.owner) {
                edges.push(TypeDependency {
                    target: parameter.ty, kind: TypeDependencyKind::Parameter,
                });
            }
        }

        // Reverse edges are unique even if several relationships reference
        // the same target. Keep dependent order equal to captured type order.
        for &source in &index.type_order {
            let mut emitted = std::collections::HashSet::new();
            for edge in index.forward.get(&source).map(Vec::as_slice).unwrap_or(&[]) {
                if emitted.insert(edge.target) {
                    index.reverse.entry(edge.target).or_default().push(source);
                }
            }
        }

        // A symbol may have no declared type. In a malformed synthetic graph
        // multiple edges can exist; match symbol_type()'s first-edge rule.
        let mut symbol_types = HashMap::new();
        for edge in &self.symbol_types {
            symbol_types.entry(edge.symbol).or_insert(edge.ty);
        }
        for symbol in &self.symbols {
            if let Some(&ty) = symbol_types.get(&symbol.id) {
                index.symbols_by_type.entry(ty).or_default().push(symbol.id);
                index.symbol_order.push((symbol.id, ty));
            }
        }
        index
    }

    /// Produce dependency-first batches of affected types. Types in a cycle
    /// form one batch; independent batches at the same level can be analyzed
    /// together. Uncaptured seed IDs are retained as singleton batches.
    pub fn type_invalidation_schedule(&self, changed: &[TypeId]) -> Vec<Vec<TypeId>> {
        self.type_dependency_index().invalidation_schedule(changed)
    }

    /// Build dependency-level waves for parallel incremental analysis.
    pub fn type_invalidation_waves(&self, changed: &[TypeId]) -> Vec<Vec<Vec<TypeId>>> {
        self.type_dependency_index().invalidation_waves(changed)
    }

    /// Find the conservative set of captured types affected by changes to
    /// one or more type IDs. Seeds are retained, including uncaptured IDs.
    /// Each type is returned once, with the shortest reverse dependency
    /// distance from any seed. Seed order breaks equal-distance ties.
    pub fn affected_types(&self, changed: &[TypeId]) -> Vec<ReachableType> {
        let mut seen = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        let mut result = Vec::new();
        for &ty in changed {
            if seen.insert(ty) {
                queue.push_back(ReachableType { ty, depth: 0 });
            }
        }
        while let Some(node) = queue.pop_front() {
            for ty in self.direct_type_dependents(node.ty) {
                if seen.insert(ty) {
                    queue.push_back(ReachableType { ty, depth: node.depth + 1 });
                }
            }
            result.push(node);
        }
        result
    }

    /// Find captured symbols whose declared types may be affected by
    /// changed type definitions. Symbols appear once, in captured symbol
    /// order. This is conservative and includes indirect dependencies.
    pub fn affected_symbols(&self, changed: &[TypeId]) -> Vec<SymbolId> {
        let affected: std::collections::HashSet<TypeId> =
            self.affected_types(changed).into_iter().map(|node| node.ty).collect();
        self.symbols.iter()
            .filter(|symbol| self.symbol_type(symbol.id).is_some_and(|ty| affected.contains(&ty)))
            .map(|symbol| symbol.id)
            .collect()
    }

    /// Group the affected captured types by their strongly connected
    /// components. The groups are ordered by first affected-type encounter.
    /// Uncaptured type IDs are returned as singleton groups.
    pub fn affected_type_groups(&self, changed: &[TypeId]) -> Vec<Vec<TypeId>> {
        let affected = self.affected_types(changed);
        let components = self.type_strongly_connected_components();
        let mut membership = HashMap::new();
        for (index, component) in components.iter().enumerate() {
            for &ty in component {
                membership.insert(ty, index);
            }
        }
        let mut emitted = std::collections::HashSet::new();
        let mut groups = Vec::new();
        for node in affected {
            if let Some(&index) = membership.get(&node.ty) {
                if emitted.insert(index) {
                    groups.push(components[index].clone());
                }
            } else {
                groups.push(vec![node.ty]);
            }
        }
        groups
    }

    /// Traverse the type graph starting from a symbol's declared type.
    /// Returns None if the symbol has no captured type edge.
    pub fn symbol_reachable_types(&self, symbol: SymbolId) -> Option<Vec<ReachableType>> {
        Some(self.reachable_types(self.symbol_type(symbol)?))
    }

    /// Visit the fields and direct bases of a record as one declaration stream.
    /// Bases are emitted before fields; order within each category is IL order.
    pub fn walk_record_layout(
        &self, record: TypeId, mut visit: impl FnMut(RecordLayoutItem<'_>) -> bool,
    ) {
        for base in self.bases(record) {
            if !visit(RecordLayoutItem::Base(base)) { return; }
        }
        for field in self.fields(record) {
            if !visit(RecordLayoutItem::Field(field)) { return; }
        }
    }

    /// Borrow a complete type view without copying the underlying IL records.
    /// Missing type IDs return None; absent optional metadata remains None.
    pub fn type_view(&self, id: TypeId) -> Option<TypeView<'_>> {
        Some(TypeView {
            definition: self.type_def(id)?,
            detail: self.type_detail(id),
            signature: self.signature(id),
            array_elements: self.array_bound(id),
        })
    }

    /// Borrow the type view of a symbol's declared type, if present.
    pub fn symbol_type_view(&self, symbol: SymbolId) -> Option<TypeView<'_>> {
        self.type_view(self.symbol_type(symbol)?)
    }

    /// Visit the fields of a record in captured IL order, with early exit.
    pub fn walk_record_fields(
        &self, owner: TypeId, mut visit: impl FnMut(&RecordField) -> bool,
    ) {
        for field in self.fields(owner) {
            if !visit(field) { break; }
        }
    }

    /// Visit direct base-class edges in captured IL order, with early exit.
    pub fn walk_base_classes(
        &self, derived: TypeId, mut visit: impl FnMut(&BaseClass) -> bool,
    ) {
        for base in self.bases(derived) {
            if !visit(base) { break; }
        }
    }

    /// Visit function-type parameters in captured IL order, with early exit.
    pub fn walk_type_parameters(
        &self, owner: TypeId, mut visit: impl FnMut(&TypeParameter) -> bool,
    ) {
        for parameter in self.parameters(owner) {
            if !visit(parameter) { break; }
        }
    }

    /// Visit member functions of a record in captured IL order, with early exit.
    pub fn walk_member_functions(
        &self, owner: TypeId, mut visit: impl FnMut(&MemberFunction) -> bool,
    ) {
        for member in self.member_functions(owner) {
            if !visit(member) { break; }
        }
    }

    pub fn type_def(&self, id: TypeId) -> Option<&TypeDef> {
        self.types.iter().find(|t| t.id == id)
    }
    pub fn statement(&self, id: StatementId) -> Option<&StatementNode> {
        self.statements.get(id.0)
    }
    pub fn expression(&self, id: ExpressionId) -> Option<&ExpressionNode> {
        self.expressions.get(id.0)
    }
}
fn compare_tree_paths(a: &str, b: &str) -> std::cmp::Ordering {
    let mut left = a.split('.');
    let mut right = b.split('.');
    loop {
        match (left.next(), right.next()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) => {
                let order = match (x.parse::<u32>(), y.parse::<u32>()) {
                    (Ok(x), Ok(y)) => x.cmp(&y),
                    _ => x.cmp(y),
                };
                if order != std::cmp::Ordering::Equal { return order; }
            }
        }
    }
}

fn graph_path_index(path: &str) -> usize {
    path.rsplit('.').next().and_then(|n| n.parse().ok()).unwrap_or(usize::MAX)
}

#[cfg(test)]
mod callee_vptr_field_tests {
    use super::*;

    fn kind(semantic:u32)->u32 {
        (0..256).find(|&value|
            unsafe {edgcpp_codegen_kind(2,value)}==semantic)
            .expect("native EDG expression kind")
    }
    fn expr(path:&str,semantic:u32,symbol:Option<SymbolId>,
            name:Option<&str>,integer:Option<i64>)->Expression {
        Expression {
            function:"target".into(),statement_path:"0.block.2".into(),
            path:path.into(),
            parent:if path=="0" {None}
                   else {Some(path.rsplit_once('.').unwrap().0.into())},
            kind:kind(semantic),operator:None,type_kind:None,
            reference:name.map(str::to_owned),
            type_size:None,type_alignment:None,constant_kind:None,
            constant_integer:integer,related_type_role:None,
            related_type_kind:None,type_id:None,symbol_id:symbol,
        }
    }
    fn node(data:Expression,operands:Vec<usize>)->ExpressionNode {
        ExpressionNode {
            data,parent:None,
            operands:operands.into_iter().map(ExpressionId).collect(),
            statement:StatementId(0),
        }
    }
    fn fixture()->(Graph,CallSite) {
        let call=CallSite {
            function:"target".into(),statement_path:"0.block.2".into(),
            path:"0".into(),first_argument:1,
        };
        let mut graph=Graph {
            expressions:vec![
            node(expr("0",3,None,None,None),vec![1,4]),
            node(expr("0.0",3,None,None,None),vec![2,3]),
            node(expr("0.0.0",5,Some(SymbolId(106)),
                      Some("__vptr"),None),vec![]),
            node(expr("0.0.1",1,None,None,Some(6)),vec![]),
            node(expr("0.1",2,None,Some("this"),None),vec![]),
            ],
            ..Default::default()
        };
        graph.record_fields.push(RecordField {
            owner:TypeId(145),position:0,ty:TypeId(117),
            symbol:SymbolId(106),name:"__vptr".into(),
            byte_offset:0,bit_offset:0,bit_width:0,
        });
        graph.calls.push(call.clone());
        graph.call_expression_by_site.insert(call.clone(),ExpressionId(0));
        (graph,call)
    }

    #[test]
    fn reports_exact_callee_field_identity_and_uninterpreted_literal() {
        let (graph,call)=fixture();
        let evidence=graph.callee_vptr_field_reference(&call).unwrap();
        assert_eq!(evidence.field,SymbolId(106));
        assert_eq!(evidence.owner,TypeId(145));
        assert_eq!(evidence.field_path,"0.0.0");
        assert_eq!(evidence.field_byte_offset,0);
        assert_eq!(evidence.raw_integer_literals,
                   vec![("0.0.1".into(),6)]);
        // An ordinary pointer argument must not contribute evidence.
        assert!(!evidence.raw_integer_literals.iter()
                 .any(|(p,_)|p=="0.1"));
    }

    #[test]
    fn fails_closed_for_forged_or_non_callee_field_references() {
        let (mut graph,call)=fixture();
        graph.record_fields[0].name="user_field".into();
        assert!(graph.callee_vptr_field_reference(&call).is_none(),
            "source names alone are insufficient without an EDG field record");
        let (mut graph,call)=fixture();
        graph.expressions[2].data.symbol_id=Some(SymbolId(999));
        assert!(graph.callee_vptr_field_reference(&call).is_none(),
            "field identity must match its independently exported record");
        let (mut graph,call)=fixture();
        graph.expressions[2].data.reference=Some("different".into());
        assert!(graph.callee_vptr_field_reference(&call).is_none(),
            "field expression must carry the same native name");
        let (mut graph,call)=fixture();
        graph.expressions[1].operands=vec![ExpressionId(3)];
        graph.expressions[0].operands.push(ExpressionId(2));
        assert!(graph.callee_vptr_field_reference(&call).is_none(),
            "vptr mentioned only as an argument is not a callee-chain proof");
        let (mut graph,call)=fixture();
        graph.expressions[1].operands.push(ExpressionId(2));
        assert!(graph.callee_vptr_field_reference(&call).is_none(),
            "ambiguous/repeated expression paths must not be guessed");
        let (mut graph,call)=fixture();
        graph.record_fields[0].bit_width=1;
        assert!(graph.callee_vptr_field_reference(&call).is_none(),
            "a bit-field is not a vptr-sized source field");
    }

    #[test]
    fn bounded_tree_cannot_recognize_unbounded_or_cyclic_input() {
        let (mut graph,call)=fixture();
        graph.expressions[1].operands.push(ExpressionId(1));
        assert!(graph.callee_vptr_field_reference(&call).is_none());
        let (mut graph,call)=fixture();
        for id in 0..514 {
            let path=format!("0.0.{}",id+2);
            graph.expressions.push(node(expr(&path,1,None,None,Some(id)),
                                        vec![]));
            let last=ExpressionId(graph.expressions.len()-1);
            graph.expressions[1].operands.push(last);
        }
        assert!(graph.callee_vptr_field_reference(&call).is_none(),
            "oversized callee trees must not produce misleading partial proof");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn native_routine_type_kind() -> u32 {
        (0..4096).find(|&kind| unsafe { edgcpp_codegen_kind(4,kind) } == 2)
            .expect("native EDG routine type kind")
    }
    fn native_type_kind_for_semantic(semantic:u32) -> u32 {
        (0..4096).find(|&kind| unsafe { edgcpp_codegen_kind(4,kind) } == semantic)
            .expect("pinned native EDG type kind")
    }
    fn native_call_kinds(member: bool) -> (u32, u32) {
        let native_kind=(0..4096).find(|&kind|
            unsafe { edgcpp_codegen_kind(2,kind) }==3
        ).expect("native EDG operation expression kind");
        let expected=if member {55} else {54};
        let native_operator=(0..4096).find(|&op|
            unsafe { edgcpp_codegen_kind(3,op) }==expected
        ).expect("native EDG call operator");
        (native_kind,native_operator)
    }
    #[test]
    fn constructor_symbol_references_are_ordered_and_unique() {
        let mut il = Il::default();
        il.symbols.extend([
            Symbol { id: SymbolId(1), kind: 1, name: "ctor".into() },
            Symbol { id: SymbolId(2), kind: 2, name: "a".into() },
            Symbol { id: SymbolId(3), kind: 2, name: "b".into() },
        ]);
        // Expression order in the raw IL need not match initializer order.
        for (position, path, parent, symbol) in [
            (1, "0", None, Some(SymbolId(2))),
            (0, "0", None, Some(SymbolId(3))),
            (0, "0.0", Some("0"), Some(SymbolId(2))),
        ] {
            il.constructor_initializer_expressions.push(ConstructorInitializerExpression {
                routine: SymbolId(1), position, path: path.into(),
                parent: parent.map(str::to_owned), kind: 1,
                operator: None, type_id: None, symbol_id: symbol,
            });
        }
        // Constructor expression validation also requires initializer records;
        // this test exercises the query on an owned graph without inventing
        // unrelated EDG initializer metadata.
        let graph = Graph {
            constructor_initializer_expressions: il.constructor_initializer_expressions,
            ..Graph::default()
        };
        assert_eq!(graph.constructor_referenced_symbols(SymbolId(1)),
            vec![SymbolId(3), SymbolId(2)]);
    }

    #[test]
    fn initializer_visitor_orders_numeric_paths_and_stops() {
        let mut graph = Graph::default();
        for path in ["0.10", "0", "0.2", "0.1"] {
            graph.initializer_expressions.push(InitializerExpression {
                owner: SymbolId(1), path: path.into(), parent: None,
                kind: 1, operator: None, type_id: None, symbol_id: None,
            });
        }
        let mut paths = Vec::new();
        graph.walk_initializer_expressions(SymbolId(1), |expr| {
            paths.push(expr.path.clone());
            paths.len() < 3
        });
        assert_eq!(paths, ["0", "0.1", "0.2"]);
        let mut missing = 0;
        graph.walk_initializer_expressions(SymbolId(2), |_| {
            missing += 1;
            true
        });
        assert_eq!(missing, 0);
    }

    #[test]
    fn constructor_visitor_orders_positions_and_stops() {
        let mut graph = Graph::default();
        for (position, path) in [(1, "0"), (0, "0.10"), (0, "0"), (0, "0.2")] {
            graph.constructor_initializer_expressions.push(ConstructorInitializerExpression {
                routine: SymbolId(1), position, path: path.into(),
                parent: None, kind: 1, operator: None, type_id: None,
                symbol_id: None,
            });
        }
        let mut paths = Vec::new();
        graph.walk_constructor_expressions(SymbolId(1), |expr| {
            paths.push((expr.position, expr.path.clone()));
            paths.len() < 3
        });
        assert_eq!(paths, [(0, "0".into()), (0, "0.2".into()), (0, "0.10".into())]);
        let mut missing = 0;
        graph.walk_constructor_expressions(SymbolId(2), |_| {
            missing += 1;
            true
        });
        assert_eq!(missing, 0);
    }

    #[test]
    fn variable_analysis_reports_counts_and_first_use_order() {
        let mut graph = Graph::default();
        for (path, symbol) in [
            ("0.10", Some(SymbolId(2))),
            ("0", None),
            ("0.2", Some(SymbolId(1))),
            ("0.1", Some(SymbolId(2))),
        ] {
            graph.initializer_expressions.push(InitializerExpression {
                owner: SymbolId(7), path: path.into(), parent: None,
                kind: 1, operator: None, type_id: None, symbol_id: symbol,
            });
        }
        assert_eq!(graph.variable_symbol_use_counts(SymbolId(7)), vec![
            SymbolUseCount { symbol: SymbolId(2), count: 2 },
            SymbolUseCount { symbol: SymbolId(1), count: 1 },
        ]);
        assert_eq!(graph.variable_expression_stats(SymbolId(7)), ExpressionStats {
            variable_initializers: 4, symbol_uses: 3, distinct_symbols: 2,
            ..ExpressionStats::default()
        });
        assert_eq!(graph.variable_expression_stats(SymbolId(7)).total(), 4);
        assert_eq!(graph.variable_expression_stats(SymbolId(99)), ExpressionStats::default());
    }

    #[test]
    fn routine_analysis_includes_constructor_initializers() {
        let mut graph = Graph::default();
        for (position, symbol) in [(1, SymbolId(1)), (0, SymbolId(2)), (2, SymbolId(2))] {
            graph.constructor_initializer_expressions.push(ConstructorInitializerExpression {
                routine: SymbolId(7), position, path: "0".into(),
                parent: None, kind: 1, operator: None, type_id: None,
                symbol_id: Some(symbol),
            });
        }
        assert_eq!(graph.routine_symbol_use_counts("absent", SymbolId(7)), vec![
            SymbolUseCount { symbol: SymbolId(2), count: 2 },
            SymbolUseCount { symbol: SymbolId(1), count: 1 },
        ]);
        assert_eq!(graph.routine_expression_stats("absent", SymbolId(7)), ExpressionStats {
            constructor_initializers: 3, symbol_uses: 3, distinct_symbols: 2,
            ..ExpressionStats::default()
        });
        assert_eq!(graph.routine_expression_count("absent", SymbolId(7)), 3);
    }

    #[test]
    fn borrowed_type_views_and_record_visitors() {
        let mut graph = Graph::default();
        graph.types.push(TypeDef {
            id: TypeId(1), kind: 1, related_role: None, related: None,
        });
        graph.array_bounds.push(ArrayBound { array: TypeId(1), elements: 17 });
        graph.record_fields.extend([
            RecordField { symbol: SymbolId(0), owner: TypeId(1), position: 0, ty: TypeId(2),
                name: "first".into(), byte_offset: 0, bit_offset: 0, bit_width: 0 },
            RecordField { symbol: SymbolId(0), owner: TypeId(1), position: 1, ty: TypeId(2),
                name: "second".into(), byte_offset: 8, bit_offset: 0, bit_width: 0 },
        ]);
        graph.base_classes.push(BaseClass {
            derived: TypeId(1), position: 0, base: TypeId(3), is_virtual: false,
        });
        graph.type_parameters.push(TypeParameter {
            owner: TypeId(1), position: 0, ty: TypeId(2),
        });
        graph.member_functions.push(MemberFunction {
            owner: TypeId(1), routine: SymbolId(4),
        });
        let view = graph.type_view(TypeId(1)).unwrap();
        assert_eq!(view.definition.id, TypeId(1));
        assert_eq!(view.array_elements, Some(17));
        assert!(view.detail.is_none());
        assert!(view.signature.is_none());
        assert!(graph.type_view(TypeId(99)).is_none());
        assert!(graph.symbol_type_view(SymbolId(99)).is_none());
        let mut fields = Vec::new();
        graph.walk_record_fields(TypeId(1), |field| {
            fields.push(field.name.clone());
            false
        });
        assert_eq!(fields, ["first"]);
        let mut bases = Vec::new();
        graph.walk_base_classes(TypeId(1), |base| {
            bases.push(base.base);
            true
        });
        assert_eq!(bases, [TypeId(3)]);
        let mut parameters = Vec::new();
        graph.walk_type_parameters(TypeId(1), |parameter| {
            parameters.push(parameter.ty);
            true
        });
        assert_eq!(parameters, [TypeId(2)]);
        let mut members = Vec::new();
        graph.walk_member_functions(TypeId(1), |member| {
            members.push(member.routine);
            true
        });
        assert_eq!(members, [SymbolId(4)]);
    }

    #[test]
    fn declaration_views_handle_partial_metadata_and_missing_symbols() {
        let mut graph = Graph::default();
        graph.symbols.extend([
            Symbol { id: SymbolId(1), kind: 1, name: "run".into() },
            Symbol { id: SymbolId(2), kind: 2, name: "value".into() },
        ]);
        graph.types.push(TypeDef {
            id: TypeId(3), kind: 1, related_role: None, related: None,
        });
        graph.symbol_types.push(SymbolType { symbol: SymbolId(1), ty: TypeId(3) });
        graph.symbol_types.push(SymbolType { symbol: SymbolId(2), ty: TypeId(99) });
        assert!(graph.routine_view(SymbolId(99)).is_none());
        assert!(graph.variable_view(SymbolId(99)).is_none());
        // A captured symbol with the wrong declaration kind is not a
        // routine/variable view even if it happens to have type metadata.
        assert!(graph.routine_view(SymbolId(2)).is_none());
        assert!(graph.variable_view(SymbolId(1)).is_none());
        let routine = graph.routine_view(SymbolId(1)).unwrap();
        assert_eq!(routine.symbol.name, "run");
        assert_eq!(routine.ty.unwrap().definition.id, TypeId(3));
        assert!(routine.implicit_this.is_none());
        let variable = graph.variable_view(SymbolId(2)).unwrap();
        assert_eq!(variable.symbol.name, "value");
        assert!(variable.ty.is_none());
        assert!(variable.initializer.is_none());
        let mut calls = 0;
        graph.walk_routine_parameters(SymbolId(1), |_| {
            calls += 1;
            true
        });
        assert_eq!(calls, 0);
    }

    #[test]
    fn record_layout_stream_places_bases_before_fields_and_stops() {
        let mut graph = Graph::default();
        graph.base_classes.push(BaseClass {
            derived: TypeId(1), position: 0, base: TypeId(2), is_virtual: false,
        });
        graph.record_fields.push(RecordField {
            symbol: SymbolId(0), owner: TypeId(1), position: 0, ty: TypeId(3),
            name: "x".into(), byte_offset: 0, bit_offset: 0, bit_width: 0,
        });
        let mut kinds = Vec::new();
        graph.walk_record_layout(TypeId(1), |item| {
            kinds.push(match item {
                RecordLayoutItem::Base(_) => "base",
                RecordLayoutItem::Field(_) => "field",
            });
            true
        });
        assert_eq!(kinds, ["base", "field"]);
        kinds.clear();
        graph.walk_record_layout(TypeId(1), |item| {
            kinds.push(match item {
                RecordLayoutItem::Base(_) => "base",
                RecordLayoutItem::Field(_) => "field",
            });
            false
        });
        assert_eq!(kinds, ["base"]);
    }

    #[test]
    fn type_dependency_graph_is_cycle_safe_and_breadth_first() {
        let mut graph = Graph::default();
        graph.types.extend([
            TypeDef { id: TypeId(1), kind: 1, related_role: Some(1), related: Some(TypeId(2)) },
            TypeDef { id: TypeId(2), kind: 1, related_role: Some(1), related: Some(TypeId(1)) },
            TypeDef { id: TypeId(3), kind: 1, related_role: None, related: None },
        ]);
        graph.base_classes.push(BaseClass {
            derived: TypeId(1), position: 0, base: TypeId(3), is_virtual: false,
        });
        graph.record_fields.push(RecordField {
            symbol: SymbolId(0), owner: TypeId(1), position: 0, ty: TypeId(4),
            name: "missing".into(), byte_offset: 0, bit_offset: 0, bit_width: 0,
        });
        graph.type_parameters.push(TypeParameter {
            owner: TypeId(2), position: 0, ty: TypeId(3),
        });
        graph.symbol_types.push(SymbolType { symbol: SymbolId(8), ty: TypeId(1) });
        assert_eq!(graph.type_dependencies(TypeId(1)), vec![
            TypeDependency { target: TypeId(2), kind: TypeDependencyKind::Related },
            TypeDependency { target: TypeId(3), kind: TypeDependencyKind::Base },
            TypeDependency { target: TypeId(4), kind: TypeDependencyKind::Field },
        ]);
        let expected = vec![
            ReachableType { ty: TypeId(1), depth: 0 },
            ReachableType { ty: TypeId(2), depth: 1 },
            ReachableType { ty: TypeId(3), depth: 1 },
            ReachableType { ty: TypeId(4), depth: 1 },
        ];
        assert_eq!(graph.reachable_types(TypeId(1)), expected);
        assert_eq!(graph.symbol_reachable_types(SymbolId(8)), Some(expected));
        assert!(graph.symbol_reachable_types(SymbolId(99)).is_none());
        let mut stopped = Vec::new();
        graph.walk_reachable_types(TypeId(1), |node| {
            stopped.push(node.ty);
            stopped.len() < 2
        });
        assert_eq!(stopped, [TypeId(1), TypeId(2)]);
        let mut direct = Vec::new();
        graph.walk_type_dependencies(TypeId(1), |edge| {
            direct.push(edge.target);
            false
        });
        assert_eq!(direct, [TypeId(2)]);
    }

    #[test]
    fn reverse_type_dependencies_and_sccs_handle_cycles() {
        let mut graph = Graph::default();
        graph.types.extend([
            TypeDef { id: TypeId(1), kind: 1, related_role: None, related: Some(TypeId(2)) },
            TypeDef { id: TypeId(2), kind: 1, related_role: None, related: Some(TypeId(1)) },
            TypeDef { id: TypeId(3), kind: 1, related_role: None, related: Some(TypeId(2)) },
            TypeDef { id: TypeId(4), kind: 1, related_role: None, related: Some(TypeId(99)) },
        ]);
        assert_eq!(graph.direct_type_dependents(TypeId(2)), [TypeId(1), TypeId(3)]);
        assert_eq!(graph.type_dependents(TypeId(2)), vec![
            ReachableType { ty: TypeId(2), depth: 0 },
            ReachableType { ty: TypeId(1), depth: 1 },
            ReachableType { ty: TypeId(3), depth: 1 },
        ]);
        let mut components: Vec<Vec<TypeId>> = graph.type_strongly_connected_components();
        for component in &mut components {
            component.sort_by_key(|id| id.0);
        }
        components.sort_by_key(|component| component[0].0);
        assert_eq!(components, vec![
            vec![TypeId(1), TypeId(2)], vec![TypeId(3)], vec![TypeId(4)],
        ]);
        let mut stopped = Vec::new();
        graph.walk_type_dependents(TypeId(2), |node| {
            stopped.push(node.ty);
            false
        });
        assert_eq!(stopped, [TypeId(2)]);
        assert_eq!(graph.direct_type_dependents(TypeId(99)), [TypeId(4)]);
    }

    #[test]
    fn incremental_invalidation_merges_seeds_and_expands_cycles() {
        let mut graph = Graph::default();
        graph.types.extend([
            TypeDef { id: TypeId(1), kind: 1, related_role: None, related: Some(TypeId(2)) },
            TypeDef { id: TypeId(2), kind: 1, related_role: None, related: Some(TypeId(1)) },
            TypeDef { id: TypeId(3), kind: 1, related_role: None, related: Some(TypeId(2)) },
            TypeDef { id: TypeId(4), kind: 1, related_role: None, related: None },
        ]);
        graph.symbols.extend([
            Symbol { id: SymbolId(10), kind: 1, name: "a".into() },
            Symbol { id: SymbolId(11), kind: 1, name: "b".into() },
            Symbol { id: SymbolId(12), kind: 1, name: "c".into() },
        ]);
        graph.symbol_types.extend([
            SymbolType { symbol: SymbolId(10), ty: TypeId(3) },
            SymbolType { symbol: SymbolId(11), ty: TypeId(4) },
            SymbolType { symbol: SymbolId(12), ty: TypeId(1) },
        ]);
        assert_eq!(graph.affected_types(&[TypeId(2), TypeId(2)]), vec![
            ReachableType { ty: TypeId(2), depth: 0 },
            ReachableType { ty: TypeId(1), depth: 1 },
            ReachableType { ty: TypeId(3), depth: 1 },
        ]);
        assert_eq!(graph.affected_symbols(&[TypeId(2)]), [SymbolId(10), SymbolId(12)]);
        let mut groups = graph.affected_type_groups(&[TypeId(2)]);
        for group in &mut groups { group.sort_by_key(|ty| ty.0); }
        assert_eq!(groups, vec![vec![TypeId(1), TypeId(2)], vec![TypeId(3)]]);
        assert_eq!(graph.affected_types(&[]), Vec::<ReachableType>::new());
        assert_eq!(graph.affected_type_groups(&[TypeId(99)]), vec![vec![TypeId(99)]]);
    }

    #[test]
    fn indexed_type_queries_match_graph_and_remain_snapshots() {
        let mut graph = Graph::default();
        graph.types.extend([
            TypeDef { id: TypeId(1), kind: 1, related_role: None, related: Some(TypeId(2)) },
            TypeDef { id: TypeId(2), kind: 1, related_role: None, related: Some(TypeId(1)) },
            TypeDef { id: TypeId(3), kind: 1, related_role: None, related: Some(TypeId(2)) },
        ]);
        graph.symbols.push(Symbol { id: SymbolId(7), kind: 1, name: "x".into() });
        graph.symbol_types.push(SymbolType { symbol: SymbolId(7), ty: TypeId(3) });
        let index = graph.type_dependency_index();
        assert_eq!(index.dependencies(TypeId(1)), graph.type_dependencies(TypeId(1)));
        assert_eq!(index.dependents(TypeId(2)), graph.direct_type_dependents(TypeId(2)));
        assert_eq!(index.affected_types(&[TypeId(2)]), graph.affected_types(&[TypeId(2)]));
        assert_eq!(index.affected_symbols(&[TypeId(2)]), graph.affected_symbols(&[TypeId(2)]));
        assert!(index.dependencies(TypeId(99)).is_empty());
        assert!(index.dependents(TypeId(99)).is_empty());
        graph.types.push(TypeDef {
            id: TypeId(4), kind: 1, related_role: None, related: Some(TypeId(2)),
        });
        assert_eq!(index.dependents(TypeId(2)), [TypeId(1), TypeId(3)]);
        assert_eq!(graph.type_dependency_index().dependents(TypeId(2)),
            [TypeId(1), TypeId(3), TypeId(4)]);
    }

    #[test]
    fn invalidation_schedule_orders_dependencies_and_collapses_cycles() {
        let mut graph = Graph::default();
        graph.types.extend([
            TypeDef { id: TypeId(1), kind: 1, related_role: None, related: Some(TypeId(2)) },
            TypeDef { id: TypeId(2), kind: 1, related_role: None, related: Some(TypeId(1)) },
            TypeDef { id: TypeId(3), kind: 1, related_role: None, related: Some(TypeId(2)) },
            TypeDef { id: TypeId(4), kind: 1, related_role: None, related: Some(TypeId(3)) },
            TypeDef { id: TypeId(5), kind: 1, related_role: None, related: None },
        ]);
        let index = graph.type_dependency_index();
        let schedule = index.invalidation_schedule(&[TypeId(2)]);
        assert_eq!(schedule.len(), 3);
        let mut cycle = schedule[0].clone();
        cycle.sort_by_key(|ty| ty.0);
        assert_eq!(cycle, [TypeId(1), TypeId(2)]);
        assert_eq!(schedule[1], [TypeId(3)]);
        assert_eq!(schedule[2], [TypeId(4)]);
        assert_eq!(graph.type_invalidation_schedule(&[TypeId(2)]), schedule);
        assert_eq!(index.invalidation_schedule(&[]), Vec::<Vec<TypeId>>::new());
        assert_eq!(index.invalidation_schedule(&[TypeId(99)]), vec![vec![TypeId(99)]]);
        assert_eq!(index.invalidation_schedule(&[TypeId(5)]), vec![vec![TypeId(5)]]);
    }

    #[test]
    fn invalidation_waves_separate_dependencies_and_parallel_branches() {
        let mut graph = Graph::default();
        graph.types.extend([
            TypeDef { id: TypeId(1), kind: 1, related_role: None, related: Some(TypeId(2)) },
            TypeDef { id: TypeId(2), kind: 1, related_role: None, related: Some(TypeId(1)) },
            TypeDef { id: TypeId(3), kind: 1, related_role: None, related: Some(TypeId(2)) },
            TypeDef { id: TypeId(4), kind: 1, related_role: None, related: Some(TypeId(2)) },
            TypeDef { id: TypeId(5), kind: 1, related_role: None, related: Some(TypeId(3)) },
        ]);
        graph.symbols.extend([
            Symbol { id: SymbolId(9), kind: 1, name: "later".into() },
            Symbol { id: SymbolId(8), kind: 1, name: "earlier".into() },
        ]);
        graph.symbol_types.extend([
            SymbolType { symbol: SymbolId(9), ty: TypeId(5) },
            SymbolType { symbol: SymbolId(8), ty: TypeId(4) },
        ]);
        let index = graph.type_dependency_index();
        let waves = index.invalidation_waves(&[TypeId(2)]);
        assert_eq!(waves.len(), 3);
        assert_eq!(waves[0].len(), 1);
        let mut cycle = waves[0][0].clone();
        cycle.sort_by_key(|ty| ty.0);
        assert_eq!(cycle, [TypeId(1), TypeId(2)]);
        let mut parallel: Vec<TypeId> = waves[1].iter().flatten().copied().collect();
        parallel.sort_by_key(|ty| ty.0);
        assert_eq!(parallel, [TypeId(3), TypeId(4)]);
        assert_eq!(waves[2], vec![vec![TypeId(5)]]);
        assert_eq!(index.invalidation_symbol_waves(&[TypeId(2)]),
            vec![vec![], vec![SymbolId(8)], vec![SymbolId(9)]]);
        assert_eq!(graph.type_invalidation_waves(&[TypeId(2)]), waves);
        assert!(index.invalidation_waves(&[]).is_empty());
    }

    #[test]
    fn mutable_type_index_updates_reverse_edges_without_losing_order() {
        let mut graph = Graph::default();
        graph.types.extend([
            TypeDef { id: TypeId(1), kind: 1, related_role: None, related: Some(TypeId(3)) },
            TypeDef { id: TypeId(2), kind: 1, related_role: None, related: Some(TypeId(3)) },
            TypeDef { id: TypeId(3), kind: 1, related_role: None, related: None },
        ]);
        let mut index = graph.type_dependency_index();
        assert_eq!(index.dependents(TypeId(3)), [TypeId(1), TypeId(2)]);
        index.replace_type_dependencies(TypeId(1), vec![
            TypeDependency { target: TypeId(4), kind: TypeDependencyKind::Related },
            TypeDependency { target: TypeId(4), kind: TypeDependencyKind::Field },
        ]);
        assert_eq!(index.dependents(TypeId(3)), [TypeId(2)]);
        assert_eq!(index.dependents(TypeId(4)), [TypeId(1)]);
        index.replace_type_dependencies(TypeId(1), vec![
            TypeDependency { target: TypeId(3), kind: TypeDependencyKind::Related },
        ]);
        assert_eq!(index.dependents(TypeId(3)), [TypeId(1), TypeId(2)]);
        assert!(index.remove_type(TypeId(1)));
        assert!(!index.remove_type(TypeId(1)));
        assert_eq!(index.dependents(TypeId(3)), [TypeId(2)]);
        index.replace_type_dependencies(TypeId(5), vec![
            TypeDependency { target: TypeId(3), kind: TypeDependencyKind::Related },
        ]);
        assert_eq!(index.dependents(TypeId(3)), [TypeId(2), TypeId(5)]);
        index.set_symbol_type(SymbolId(9), Some(TypeId(5)));
        assert_eq!(index.affected_symbols(&[TypeId(3)]), [SymbolId(9)]);
        index.set_symbol_type(SymbolId(9), None);
        assert!(index.affected_symbols(&[TypeId(3)]).is_empty());
    }

    #[test]
    fn batched_type_updates_match_sequential_edits_and_keep_reverse_order() {
        let mut graph = Graph::default();
        graph.types.extend([
            TypeDef { id: TypeId(1), kind: 1, related_role: None, related: Some(TypeId(3)) },
            TypeDef { id: TypeId(2), kind: 1, related_role: None, related: Some(TypeId(3)) },
            TypeDef { id: TypeId(3), kind: 1, related_role: None, related: None },
            TypeDef { id: TypeId(4), kind: 1, related_role: None, related: Some(TypeId(3)) },
        ]);
        graph.symbols.extend([
            Symbol { id: SymbolId(10), kind: 1, name: "first".into() },
            Symbol { id: SymbolId(11), kind: 1, name: "second".into() },
        ]);
        graph.symbol_types.extend([
            SymbolType { symbol: SymbolId(10), ty: TypeId(1) },
            SymbolType { symbol: SymbolId(11), ty: TypeId(2) },
        ]);
        let mut batch = graph.type_dependency_index();
        let mut sequential = batch.clone();
        let changes = vec![
            TypeIndexChange::ReplaceType {
                ty: TypeId(1),
                dependencies: vec![
                    TypeDependency { target: TypeId(5), kind: TypeDependencyKind::Field },
                    TypeDependency { target: TypeId(5), kind: TypeDependencyKind::Related },
                ],
            },
            TypeIndexChange::RemoveType { ty: TypeId(4) },
            TypeIndexChange::ReplaceType {
                ty: TypeId(1),
                dependencies: vec![
                    TypeDependency { target: TypeId(3), kind: TypeDependencyKind::Base },
                    TypeDependency { target: TypeId(3), kind: TypeDependencyKind::Field },
                ],
            },
            TypeIndexChange::ReplaceType {
                ty: TypeId(6),
                dependencies: vec![TypeDependency {
                    target: TypeId(3), kind: TypeDependencyKind::Related,
                }],
            },
            TypeIndexChange::SetSymbolType { symbol: SymbolId(10), ty: Some(TypeId(6)) },
            TypeIndexChange::SetSymbolType { symbol: SymbolId(10), ty: Some(TypeId(3)) },
            TypeIndexChange::SetSymbolType { symbol: SymbolId(12), ty: Some(TypeId(6)) },
        ];
        batch.apply_changes(&changes);
        for change in changes {
            match change {
                TypeIndexChange::ReplaceType { ty, dependencies } => {
                    sequential.replace_type_dependencies(ty, dependencies);
                }
                TypeIndexChange::RemoveType { ty } => {
                    assert!(sequential.remove_type(ty));
                }
                TypeIndexChange::SetSymbolType { symbol, ty } => {
                    sequential.set_symbol_type(symbol, ty);
                }
            }
        }
        for target in [TypeId(3), TypeId(4), TypeId(5), TypeId(6)] {
            assert_eq!(batch.dependents(target), sequential.dependents(target));
        }
        assert_eq!(batch.dependents(TypeId(3)),
            [TypeId(1), TypeId(2), TypeId(6)]);
        assert_eq!(batch.dependencies(TypeId(1)).len(), 2);
        assert!(batch.dependents(TypeId(5)).is_empty());
        assert!(batch.dependencies(TypeId(4)).is_empty());
        assert_eq!(batch.affected_types(&[TypeId(3)]),
            sequential.affected_types(&[TypeId(3)]));
        assert_eq!(batch.invalidation_schedule(&[TypeId(3)]),
            sequential.invalidation_schedule(&[TypeId(3)]));
        assert_eq!(batch.affected_symbols(&[TypeId(3)]),
            [SymbolId(10), SymbolId(11), SymbolId(12)]);
        assert_eq!(batch.affected_symbols(&[TypeId(3)]),
            sequential.affected_symbols(&[TypeId(3)]));
    }

    #[test]
    fn audit_indexed_mutation_batches_match_sequential_order_and_reverse_edges() {
        let mut batch=TypeDependencyIndex::default();
        for n in 1..=96 {
            batch.replace_type_dependencies(TypeId(n),vec![TypeDependency {
                target:TypeId(n%96+1),kind:TypeDependencyKind::Related,
            }]);
        }
        for n in 1..=192 {
            batch.set_symbol_type(SymbolId(n),Some(TypeId(n%96+1)));
        }
        let mut sequential=batch.clone();
        let mut seed=0x9e3779b97f4a7c15u64;
        let mut changes=Vec::new();
        for i in 0..2400 {
            seed=seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let symbol=SymbolId(((seed>>16)%192+1) as u32);
            let ty=TypeId(((seed>>32)%96+1) as u32);
            let target=TypeId(((seed>>48)%96+1) as u32);
            changes.push(match i%8 {
                0|1|6 => TypeIndexChange::SetSymbolType {symbol,ty:Some(ty)},
                2 => TypeIndexChange::SetSymbolType {symbol,ty:None},
                3|7 => TypeIndexChange::RemoveType {ty},
                _ => TypeIndexChange::ReplaceType {
                    ty, dependencies:vec![TypeDependency {
                        target,kind:TypeDependencyKind::Field,
                    }],
                },
            });
        }
        batch.apply_changes(&changes);
        for change in &changes {
            // Deliberately exercise the single-edit path as an independent
            // ordering oracle for the indexed multi-edit implementation.
            sequential.apply_changes(std::slice::from_ref(change));
        }
        assert_eq!(batch.type_order,sequential.type_order);
        assert_eq!(batch.symbol_order,sequential.symbol_order);
        assert_eq!(batch.forward,sequential.forward);
        assert_eq!(batch.reverse,sequential.reverse);
        assert_eq!(batch.symbols_by_type,sequential.symbols_by_type);
        for ty in 1..=96 {
            assert_eq!(batch.affected_types(&[TypeId(ty)]),
                sequential.affected_types(&[TypeId(ty)]));
            assert_eq!(batch.affected_symbols(&[TypeId(ty)]),
                sequential.affected_symbols(&[TypeId(ty)]));
        }
    }

    #[test]
    fn audit_mutation_batch_delete_and_readd_moves_symbols_and_types_to_end() {
        let mut index=TypeDependencyIndex::default();
        for id in 1..=3 {
            index.replace_type_dependencies(TypeId(id),vec![]);
            index.set_symbol_type(SymbolId(id),Some(TypeId(id)));
        }
        index.apply_changes(&[
            TypeIndexChange::RemoveType {ty:TypeId(2)},
            TypeIndexChange::ReplaceType {ty:TypeId(2),dependencies:vec![]},
            TypeIndexChange::SetSymbolType {symbol:SymbolId(1),ty:None},
            TypeIndexChange::SetSymbolType {symbol:SymbolId(1),ty:Some(TypeId(3))},
            TypeIndexChange::SetSymbolType {symbol:SymbolId(2),ty:Some(TypeId(1))},
            TypeIndexChange::RemoveType {ty:TypeId(1)},
            TypeIndexChange::ReplaceType {ty:TypeId(1),dependencies:vec![]},
        ]);
        assert_eq!(index.type_order,[TypeId(3),TypeId(2),TypeId(1)]);
        assert_eq!(index.symbol_order,[
            (SymbolId(2),TypeId(1)),
            (SymbolId(3),TypeId(3)),
            (SymbolId(1),TypeId(3)),
        ]);
        assert_eq!(index.symbols_with_type(TypeId(3)),[SymbolId(3),SymbolId(1)]);
    }

    /// Run manually with:
    /// cargo test --lib --features codegen audit_type_index_batch_scaling -- --ignored --nocapture
    /// Old path is a frozen reproduction of the pre-#174 scan-based symbol
    /// mutation logic. Keep the same input and runner for both measurements.
    #[test]
    #[ignore = "profiling-only; run explicitly with --ignored --nocapture"]
    fn audit_type_index_batch_scaling() {
        fn legacy_symbol_batch(index:&mut TypeDependencyIndex,changes:&[TypeIndexChange]) {
            let mut touched=std::collections::HashSet::new();
            for change in changes {
                let TypeIndexChange::SetSymbolType {symbol,ty}=change else {
                    panic!("benchmark accepts symbol edits only");
                };
                if let Some(old_ty)=index.symbol_order.iter()
                    .find(|(id,_)|id==symbol).map(|(_,old_ty)|*old_ty)
                {
                    touched.insert(old_ty);
                }
                if let Some(new_ty)=ty {touched.insert(*new_ty);}
                if let Some(position)=index.symbol_order.iter()
                    .position(|(id,_)|id==symbol)
                {
                    if let Some(new_ty)=ty {
                        index.symbol_order[position].1 = *new_ty;
                    } else {
                        index.symbol_order.remove(position);
                    }
                } else if let Some(new_ty)=ty {
                    index.symbol_order.push((*symbol,*new_ty));
                }
            }
            for &ty in &touched {index.symbols_by_type.remove(&ty);}
            for &(symbol,ty) in &index.symbol_order {
                if touched.contains(&ty) {
                    index.symbols_by_type.entry(ty).or_default().push(symbol);
                }
            }
        }

        for count in [1024_u32,4096,16384] {
            let mut initial=TypeDependencyIndex {
                symbol_order:(1..=count)
                    .map(|id|(SymbolId(id),TypeId(id%32+1))).collect(),
                ..Default::default()
            };
            for &(symbol,ty) in &initial.symbol_order {
                initial.symbols_by_type.entry(ty).or_default().push(symbol);
            }
            let updates:Vec<_>=(1..=count).rev().map(|id|
                TypeIndexChange::SetSymbolType {
                    symbol:SymbolId(id),
                    ty:Some(TypeId((id.wrapping_mul(7))%32+1)),
                }).collect();
            let mut old_times=Vec::new();
            let mut new_times=Vec::new();
            for _ in 0..3 {
                let mut old=initial.clone();
                let mut new=initial.clone();
                let started=std::time::Instant::now();
                legacy_symbol_batch(&mut old,&updates);
                old_times.push(started.elapsed());
                let started=std::time::Instant::now();
                new.apply_changes(&updates);
                new_times.push(started.elapsed());
                std::hint::black_box(&old);
                std::hint::black_box(&new);
                assert_eq!(old.symbol_order,new.symbol_order);
                assert_eq!(old.symbols_by_type,new.symbols_by_type);
            }
            old_times.sort_unstable();
            new_times.sort_unstable();
            let old=old_times[1].as_secs_f64()*1000.0;
            let new=new_times[1].as_secs_f64()*1000.0;
            println!("type-index-batch count={count} old_ms={old:.3} indexed_ms={new:.3} speedup={:.2}x",old/new);
        }

        // Type removals previously filtered the full ordered type vector
        // once per deletion. Isolate that algorithm using edge-free types;
        // no reverse-edge processing should distort either measurement.
        for count in [1024_u32,4096,16384] {
            let initial=TypeDependencyIndex {
                type_order:(1..=count).map(TypeId).collect(),
                forward:(1..=count).map(|id|(TypeId(id),Vec::new())).collect(),
                ..Default::default()
            };
            let updates:Vec<_>=(1..=count).rev()
                .map(|id|TypeIndexChange::RemoveType {ty:TypeId(id)}).collect();
            let mut old_times=Vec::new();
            let mut new_times=Vec::new();
            for _ in 0..3 {
                let mut old=initial.clone();
                let mut new=initial.clone();
                let started=std::time::Instant::now();
                for update in &updates {
                    let TypeIndexChange::RemoveType {ty}=update else {
                        unreachable!("type removal benchmark");
                    };
                    if old.forward.remove(ty).is_some() {
                        old.type_order.retain(|source|source!=ty);
                    }
                }
                old_times.push(started.elapsed());
                let started=std::time::Instant::now();
                new.apply_changes(&updates);
                new_times.push(started.elapsed());
                std::hint::black_box(&old);
                std::hint::black_box(&new);
                assert_eq!(old.type_order,new.type_order);
                assert_eq!(old.forward,new.forward);
                assert_eq!(old.reverse,new.reverse);
            }
            old_times.sort_unstable();
            new_times.sort_unstable();
            let old=old_times[1].as_secs_f64()*1000.0;
            let new=new_times[1].as_secs_f64()*1000.0;
            println!("type-index-remove count={count} old_ms={old:.3} indexed_ms={new:.3} speedup={:.2}x",old/new);
        }
    }

    #[test]
    fn differential_invalidation_reports_additions_removals_and_shortest_depths() {
        let mut graph = Graph::default();
        graph.types.extend([
            TypeDef { id: TypeId(1), kind: 1, related_role: None, related: None },
            TypeDef { id: TypeId(2), kind: 1, related_role: None, related: Some(TypeId(1)) },
            TypeDef { id: TypeId(3), kind: 1, related_role: None, related: Some(TypeId(2)) },
            TypeDef { id: TypeId(4), kind: 1, related_role: None, related: None },
            TypeDef { id: TypeId(5), kind: 1, related_role: None, related: None },
        ]);
        graph.symbols.extend([
            Symbol { id: SymbolId(4), kind: 1, name: "added".into() },
            Symbol { id: SymbolId(3), kind: 1, name: "removed".into() },
            Symbol { id: SymbolId(1), kind: 1, name: "retained".into() },
        ]);
        graph.symbol_types.extend([
            SymbolType { symbol: SymbolId(4), ty: TypeId(4) },
            SymbolType { symbol: SymbolId(3), ty: TypeId(3) },
            SymbolType { symbol: SymbolId(1), ty: TypeId(1) },
        ]);
        let mut index = graph.type_dependency_index();
        let delta = index.apply_changes_with_diff(&[
            TypeIndexChange::ReplaceType {
                ty: TypeId(2),
                dependencies: vec![TypeDependency {
                    target: TypeId(6), kind: TypeDependencyKind::Related,
                }],
            },
            TypeIndexChange::ReplaceType {
                ty: TypeId(4),
                dependencies: vec![TypeDependency {
                    target: TypeId(1), kind: TypeDependencyKind::Related,
                }],
            },
            TypeIndexChange::ReplaceType {
                ty: TypeId(5),
                dependencies: vec![TypeDependency {
                    target: TypeId(4), kind: TypeDependencyKind::Related,
                }],
            },
        ], &[TypeId(1)]);
        assert_eq!(delta.before_types, vec![
            ReachableType { ty: TypeId(1), depth: 0 },
            ReachableType { ty: TypeId(2), depth: 1 },
            ReachableType { ty: TypeId(3), depth: 2 },
        ]);
        assert_eq!(delta.after_types, vec![
            ReachableType { ty: TypeId(1), depth: 0 },
            ReachableType { ty: TypeId(4), depth: 1 },
            ReachableType { ty: TypeId(5), depth: 2 },
        ]);
        assert_eq!(delta.newly_affected_types, vec![
            ReachableType { ty: TypeId(4), depth: 1 },
            ReachableType { ty: TypeId(5), depth: 2 },
        ]);
        assert_eq!(delta.no_longer_affected_types, vec![
            ReachableType { ty: TypeId(2), depth: 1 },
            ReachableType { ty: TypeId(3), depth: 2 },
        ]);
        assert_eq!(delta.newly_affected_symbols, [SymbolId(4)]);
        assert_eq!(delta.no_longer_affected_symbols, [SymbolId(3)]);
        let unchanged = index.apply_changes_with_diff(&[], &[TypeId(1)]);
        assert!(unchanged.newly_affected_types.is_empty());
        assert!(unchanged.no_longer_affected_types.is_empty());
        assert!(unchanged.newly_affected_symbols.is_empty());
        assert!(unchanged.no_longer_affected_symbols.is_empty());
    }

    #[test]
    fn batch_can_delete_and_recreate_types_or_remove_symbol_links() {
        let mut index = TypeDependencyIndex::default();
        let changes = [
            TypeIndexChange::ReplaceType {
                ty: TypeId(7),
                dependencies: vec![TypeDependency {
                    target: TypeId(8), kind: TypeDependencyKind::Field,
                }],
            },
            TypeIndexChange::RemoveType { ty: TypeId(7) },
            TypeIndexChange::ReplaceType {
                ty: TypeId(7),
                dependencies: vec![TypeDependency {
                    target: TypeId(9), kind: TypeDependencyKind::Related,
                }],
            },
            TypeIndexChange::SetSymbolType { symbol: SymbolId(1), ty: Some(TypeId(7)) },
            TypeIndexChange::SetSymbolType { symbol: SymbolId(2), ty: Some(TypeId(7)) },
            TypeIndexChange::SetSymbolType { symbol: SymbolId(1), ty: None },
        ];
        let delta = index.apply_changes_with_diff(&changes, &[TypeId(9)]);
        assert_eq!(index.dependents(TypeId(9)), [TypeId(7)]);
        assert!(index.dependents(TypeId(8)).is_empty());
        assert!(delta.before_types.contains(&ReachableType { ty: TypeId(9), depth: 0 }));
        assert_eq!(delta.newly_affected_types, vec![
            ReachableType { ty: TypeId(7), depth: 1 },
        ]);
        assert_eq!(delta.newly_affected_symbols, [SymbolId(2)]);
        assert_eq!(index.affected_symbols(&[TypeId(9)]), [SymbolId(2)]);
    }

    #[test]
    fn indexed_type_construction_matches_all_direct_graph_queries() {
        let mut graph = Graph::default();
        for i in 1..=32u32 {
            graph.types.push(TypeDef {
                id: TypeId(i), kind: 1, related_role: Some(1),
                related: Some(TypeId((i % 32) + 1)),
            });
            graph.base_classes.push(BaseClass {
                derived: TypeId(i), position: 0,
                base: TypeId(((i + 1) % 32) + 1), is_virtual: false,
            });
            graph.record_fields.push(RecordField {
                symbol: SymbolId(0), owner: TypeId(i), position: 0, ty: TypeId(99),
                name: format!("field_{i}"),
                byte_offset: 0, bit_offset: 0, bit_width: 0,
            });
            graph.type_parameters.push(TypeParameter {
                owner: TypeId(i), position: 0, ty: TypeId(((i + 2) % 32) + 1),
            });
            graph.symbols.push(Symbol {
                id: SymbolId(i), kind: 1, name: format!("symbol_{i}"),
            });
            graph.symbol_types.push(SymbolType {
                symbol: SymbolId(i), ty: TypeId(i),
            });
        }
        let index = graph.type_dependency_index();
        for i in 1..=32u32 {
            let ty = TypeId(i);
            assert_eq!(index.dependencies(ty), graph.type_dependencies(ty));
            assert_eq!(index.dependents(ty), graph.direct_type_dependents(ty));
            assert_eq!(index.symbols_with_type(ty), [SymbolId(i)]);
        }
        assert_eq!(index.dependents(TypeId(99)),
            graph.direct_type_dependents(TypeId(99)));
        assert!(index.dependencies(TypeId(99)).is_empty());
        assert_eq!(index.affected_types(&[TypeId(7)]),
            graph.affected_types(&[TypeId(7)]));
        assert_eq!(index.affected_symbols(&[TypeId(7)]),
            graph.affected_symbols(&[TypeId(7)]));
    }

    #[test]
    fn type_invalidation_schedule_and_waves_respect_every_condensed_edge() {
        let mut graph = Graph::default();
        for i in 1..=24u32 {
            graph.types.push(TypeDef {
                id: TypeId(i), kind: 1, related_role: Some(1),
                related: Some(TypeId(if i <= 2 { 3 - i } else { i - 1 })),
            });
            if i > 3 {
                graph.record_fields.push(RecordField {
                    symbol: SymbolId(0), owner: TypeId(i), position: 0,
                    ty: TypeId(i - 3), name: String::new(),
                    byte_offset: 0, bit_offset: 0, bit_width: 0,
                });
            }
        }
        let index = graph.type_dependency_index();
        let schedule = index.invalidation_schedule(&[TypeId(1)]);
        let mut order = HashMap::new();
        for (step, component) in schedule.iter().enumerate() {
            for &ty in component {
                assert!(order.insert(ty, step).is_none());
            }
        }
        assert_eq!(order.len(), 24);
        for (&ty, &step) in &order {
            for edge in index.dependencies(ty) {
                if let Some(&prerequisite) = order.get(&edge.target) {
                    assert!(prerequisite <= step,
                        "type {:?} was scheduled before dependency {:?}", ty, edge.target);
                }
            }
        }
        let waves = index.invalidation_waves(&[TypeId(1)]);
        let mut levels = HashMap::new();
        for (level, wave) in waves.iter().enumerate() {
            for component in wave {
                for &ty in component {
                    assert!(levels.insert(ty, level).is_none());
                }
            }
        }
        assert_eq!(levels.len(), order.len());
        for (&ty, &level) in &levels {
            for edge in index.dependencies(ty) {
                if let Some(&prerequisite) = levels.get(&edge.target) {
                    if order[&ty] != order[&edge.target] {
                        assert!(prerequisite < level);
                    }
                }
            }
        }
    }

    #[test]
    fn initializer_symbol_references_follow_numeric_tree_order() {
        let mut il = Il::default();
        il.symbols.extend([
            Symbol { id: SymbolId(1), kind: 2, name: "target".into() },
            Symbol { id: SymbolId(2), kind: 2, name: "a".into() },
            Symbol { id: SymbolId(3), kind: 2, name: "b".into() },
        ]);
        for (path, parent, symbol) in [
            ("0", None, None),
            ("0.1", Some("0"), Some(SymbolId(2))),
            ("0.0", Some("0"), Some(SymbolId(3))),
        ] {
            il.initializer_expressions.push(InitializerExpression {
                owner: SymbolId(1), path: path.into(),
                parent: parent.map(str::to_owned), kind: 1,
                operator: None, type_id: None, symbol_id: symbol,
            });
        }
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(graph.initializer_referenced_symbols(SymbolId(1)),
            vec![SymbolId(3), SymbolId(2)]);
        assert!(graph.constructor_referenced_symbols(SymbolId(1)).is_empty());
    }


    #[test]
    fn unified_symbol_collection_preserves_primary_then_control_order() {
        let mut il = Il::default();
        for id in 1..=3 {
            il.symbols.push(Symbol {
                id: SymbolId(id), kind: 2, name: format!("s{id}"),
            });
        }
        il.statements.push(Statement {
            function: "f".into(), path: "0".into(), parent: None, kind: 1,
        });
        il.expressions.push(Expression {
            function: "f".into(), statement_path: "0".into(),
            path: "0".into(), parent: None, kind: 1,
            operator: None, type_kind: None, reference: None,
            type_size: None, type_alignment: None, constant_kind: None,
            constant_integer: None, related_type_role: None,
            related_type_kind: None, type_id: None, symbol_id: Some(SymbolId(2)),
        });
        for (path, parent, symbol) in [
            ("0", None, SymbolId(1)),
            ("0.0", Some("0"), SymbolId(2)),
            ("0.1", Some("0"), SymbolId(3)),
        ] {
            il.control_expressions.push(ControlExpression {
                function: "f".into(), statement_path: "0".into(), role: 1,
                path: path.into(), parent: parent.map(str::to_owned),
                kind: 1, operator: None, type_id: None, symbol_id: Some(symbol),
            });
        }
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(graph.referenced_symbols("f"), vec![SymbolId(2)]);
        assert_eq!(graph.referenced_symbols_including_controls("f"),
            vec![SymbolId(2), SymbolId(1), SymbolId(3)]);
    }

    #[test]
    fn control_symbol_references_are_indexed_and_deduplicated() {
        let mut il = Il::default();
        il.symbols.push(Symbol { id: SymbolId(1), kind: 2, name: "a".into() });
        il.symbols.push(Symbol { id: SymbolId(2), kind: 2, name: "b".into() });
        il.statements.push(Statement {
            function: "f".into(), path: "0".into(), parent: None, kind: 1,
        });
        for (path, parent, symbol) in [
            ("0", None, Some(SymbolId(1))),
            ("0.0", Some("0"), Some(SymbolId(2))),
            ("0.1", Some("0"), Some(SymbolId(1))),
        ] {
            il.control_expressions.push(ControlExpression {
                function: "f".into(), statement_path: "0".into(), role: 1,
                path: path.into(), parent: parent.map(str::to_owned),
                kind: 1, operator: None, type_id: None, symbol_id: symbol,
            });
        }
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(
            graph.referenced_symbols_including_controls("f"),
            vec![SymbolId(1), SymbolId(2)]
        );
        assert!(graph.referenced_symbols_including_controls("other").is_empty());
    }


    #[test]
    fn numeric_tree_paths_sort_by_operand_index() {
        let mut paths = vec!["0.10", "0.2", "0.1", "0.2.0", "0"];
        paths.sort_by(|a, b| compare_tree_paths(a, b));
        assert_eq!(paths, ["0", "0.1", "0.2", "0.2.0", "0.10"]);
    }


    #[test]
    fn function_expression_walk_and_symbol_collection() {
        let mut il = Il::default();
        il.symbols.push(Symbol { id: SymbolId(1), kind: 2, name: "x".into() });
        il.statements.push(Statement {
            function: "f".into(), path: "0".into(), parent: None, kind: 1,
        });
        for (path, parent) in [("0", None), ("0.0", Some("0"))] {
            il.expressions.push(Expression {
                function: "f".into(), statement_path: "0".into(),
                path: path.into(), parent: parent.map(str::to_owned),
                kind: 1, operator: None, type_kind: None, reference: None,
                type_size: None, type_alignment: None, constant_kind: None,
                constant_integer: None, related_type_role: None,
                related_type_kind: None, type_id: None,
                symbol_id: Some(SymbolId(1)),
            });
        }
        let graph = Graph::from_il(&il).unwrap();
        let mut paths = Vec::new();
        graph.walk_function_expressions("f", |_, _, node| {
            paths.push(node.data.path.clone());
            true
        });
        assert_eq!(paths, ["0", "0.0"]);
        assert_eq!(graph.referenced_symbols("f"), vec![SymbolId(1)]);
        let mut count = 0;
        graph.walk_function_expressions("f", |_, _, _| {
            count += 1;
            false
        });
        assert_eq!(count, 1);
        assert!(graph.referenced_symbols("missing").is_empty());
    }


    #[test]
    fn unified_expression_walk_orders_kinds_and_honors_early_exit() {
        let mut il = Il::default();
        il.symbols.push(Symbol { id: SymbolId(1), kind: 2, name: "a".into() });
        il.statements.push(Statement {
            function: "f".into(), path: "0".into(), parent: None, kind: 1,
        });
        il.expressions.push(Expression {
            function: "f".into(), statement_path: "0".into(),
            path: "0".into(), parent: None, kind: 1,
            operator: None, type_kind: None, reference: None,
            type_size: None, type_alignment: None, constant_kind: None,
            constant_integer: None, related_type_role: None,
            related_type_kind: None, type_id: None,
            symbol_id: Some(SymbolId(1)),
        });
        il.control_expressions.push(ControlExpression {
            function: "f".into(), statement_path: "0".into(), role: 1,
            path: "0".into(), parent: None, kind: 1,
            operator: None, type_id: None, symbol_id: Some(SymbolId(1)),
        });
        let graph = Graph::from_il(&il).unwrap();
        let mut kinds = Vec::new();
        graph.walk_function_all_expressions("f", |_, expr| {
            kinds.push(match expr {
                FunctionExpressionRef::Primary(_) => "primary",
                FunctionExpressionRef::Control(_) => "control",
            });
            true
        });
        assert_eq!(kinds, ["primary", "control"]);
        kinds.clear();
        graph.walk_function_all_expressions("f", |_, expr| {
            kinds.push(match expr {
                FunctionExpressionRef::Primary(_) => "primary",
                FunctionExpressionRef::Control(_) => "control",
            });
            false
        });
        assert_eq!(kinds, ["primary"]);
        let mut absent = 0;
        graph.walk_function_all_expressions("missing", |_, _| {
            absent += 1;
            true
        });
        assert_eq!(absent, 0);
    }

    #[test]
    fn walks_control_expressions_in_index_order_and_stops_early() {
        let mut il = Il::default();
        il.symbols.extend([
            Symbol { id: SymbolId(1), kind: 2, name: "a".into() },
            Symbol { id: SymbolId(2), kind: 2, name: "b".into() },
        ]);
        il.statements.push(Statement {
            function: "f".into(), path: "0".into(),
            parent: None, kind: 1,
        });
        for (role, path, parent, symbol) in [
            (2, "0", None, Some(SymbolId(2))),
            (1, "0", None, Some(SymbolId(1))),
            (1, "0.1", Some("0"), Some(SymbolId(2))),
            (1, "0.0", Some("0"), None),
        ] {
            il.control_expressions.push(ControlExpression {
                function: "f".into(), statement_path: "0".into(), role,
                path: path.into(), parent: parent.map(str::to_owned),
                kind: 1, operator: None, type_id: None, symbol_id: symbol,
            });
        }
        let graph = Graph::from_il(&il).unwrap();
        let mut visited = Vec::new();
        graph.walk_function_control_expressions("f", |statement, expr| {
            visited.push((statement, expr.role, expr.path.clone()));
            true
        });
        assert_eq!(visited.iter().map(|(_, role, path)| (*role, path.as_str()))
            .collect::<Vec<_>>(),
            vec![(1, "0"), (1, "0.0"), (1, "0.1"), (2, "0")]);
        let mut count = 0;
        graph.walk_function_control_expressions("f", |_, _| {
            count += 1;
            count < 2
        });
        assert_eq!(count, 2);
    }

    #[test]
    fn postorder_statement_walk_is_bottom_up_and_stoppable() {
        let mut il = Il::default();
        for (path, parent) in [
            ("0", None), ("0.0.0", Some("0")),
            ("0.0.0.0.0", Some("0.0.0")),
            ("0.0.1", Some("0")), ("1", None),
        ] {
            il.statements.push(Statement {
                function: "f".into(), path: path.into(),
                parent: parent.map(str::to_owned), kind: 1,
            });
        }
        let graph = Graph::from_il(&il).unwrap();
        let mut paths = Vec::new();
        graph.walk_statements_postorder("f", |_, node| {
            paths.push(node.data.path.clone());
            true
        });
        assert_eq!(paths, ["0.0.0.0.0", "0.0.0", "0.0.1", "0", "1"]);
        let mut stopped = Vec::new();
        graph.walk_statements_postorder("f", |_, node| {
            stopped.push(node.data.path.clone());
            stopped.len() < 2
        });
        assert_eq!(stopped, ["0.0.0.0.0", "0.0.0"]);
    }

    #[test]
    fn postorder_expression_walk_visits_operands_first() {
        let mut il = Il::default();
        il.statements.push(Statement {
            function: "f".into(), path: "0".into(), parent: None, kind: 1,
        });
        for (path, parent) in [
            ("0", None), ("0.0", Some("0")), ("0.1", Some("0")),
            ("0.0.0", Some("0.0")),
        ] {
            il.expressions.push(Expression {
                function: "f".into(), statement_path: "0".into(),
                path: path.into(), parent: parent.map(str::to_owned),
                kind: 1, operator: None, type_kind: None, reference: None,
                type_size: None, type_alignment: None, constant_kind: None,
                constant_integer: None, related_type_role: None,
                related_type_kind: None, type_id: None, symbol_id: None,
            });
        }
        let graph = Graph::from_il(&il).unwrap();
        let mut paths = Vec::new();
        graph.walk_function_expressions_postorder("f", |_, _, node| {
            paths.push(node.data.path.clone());
            true
        });
        assert_eq!(paths, ["0.0.0", "0.0", "0.1", "0"]);
        let mut stopped = Vec::new();
        graph.walk_function_expressions_postorder("f", |_, _, node| {
            stopped.push(node.data.path.clone());
            stopped.len() < 2
        });
        assert_eq!(stopped, ["0.0.0", "0.0"]);
        let root = graph.statements[0].expression.unwrap();
        let mut direct = Vec::new();
        graph.walk_expression_postorder(root, |_, node| {
            direct.push(node.data.path.clone());
            true
        });
        assert_eq!(direct, paths);
    }

    #[test]
    fn depth_first_walk_preserves_order_and_supports_early_exit() {
        let mut il = Il::default();
        for (path, parent) in [
            ("0", None), ("0.0.0", Some("0")), ("0.0.0.0.0", Some("0.0.0")),
            ("0.0.1", Some("0")), ("1", None),
        ] {
            il.statements.push(Statement {
                function: "f".into(), path: path.into(),
                parent: parent.map(str::to_owned), kind: 1,
            });
        }
        let graph = Graph::from_il(&il).unwrap();
        let mut paths = Vec::new();
        graph.walk_statements("f", |_, node| {
            paths.push(node.data.path.clone());
            true
        });
        assert_eq!(paths, ["0", "0.0.0", "0.0.0.0.0", "0.0.1", "1"]);
        let mut stopped = Vec::new();
        graph.walk_statements("f", |_, node| {
            stopped.push(node.data.path.clone());
            stopped.len() < 3
        });
        assert_eq!(stopped, ["0", "0.0.0", "0.0.0.0.0"]);
        let mut missing = 0;
        graph.walk_statements("other", |_, _| { missing += 1; true });
        assert_eq!(missing, 0);
    }


    #[test]
    fn preserves_hoisted_static_local_ownership_without_inventing_scopes() {
        let mut il = Il::default();
        il.symbols.push(Symbol { id: SymbolId(1), kind: 1, name: "f".into() });
        il.symbols.push(Symbol { id: SymbolId(2), kind: 2, name: "f()::counter".into() });
        il.static_local_owners.push(StaticLocalOwner {
            routine: SymbolId(1), symbol: SymbolId(2),
        });
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(graph.static_local_owner(SymbolId(2)), Some(SymbolId(1)));
        assert_eq!(graph.static_locals(SymbolId(1)).count(), 1);
        assert_eq!(graph.local_variable_scope(SymbolId(2)), None);
        il.static_local_owners.push(il.static_local_owners[0].clone());
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::DuplicateStaticLocalOwner(SymbolId(2)))));
        il.static_local_owners.pop();
        il.static_local_owners[0].routine = SymbolId(9);
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidStaticLocalOwner(SymbolId(2)))));
    }

    #[test]
    fn indexed_function_locals_preserve_scope_and_shadowing() {
        let mut il = Il::default();
        il.symbols.push(Symbol { id: SymbolId(1), kind: 1, name: "f".into() });
        for symbol in 2..=5 {
            il.symbols.push(Symbol {
                id: SymbolId(symbol), kind: 2, name: "x".into(),
            });
        }
        il.types.push(TypeDef {
            id: TypeId(1), kind: 2, related_role: None, related: None,
        });
        il.statements.push(Statement {
            function: "f".into(), path: "0".into(), parent: None, kind: 11,
        });
        il.local_scopes.push(LocalScope {
            routine: SymbolId(1), id: 1, parent: Some(0),
            kind: 2, statement_path: None,
        });
        il.local_scopes.push(LocalScope {
            routine: SymbolId(1), id: 0, parent: None,
            kind: 17, statement_path: Some("0".into()),
        });
        il.function_parameters.push(FunctionParameter {
            routine: SymbolId(1), position: Some(0), symbol: SymbolId(2),
            ty: TypeId(1), implicit_this: false, compiler_generated: false,
        });
        il.function_parameters.push(FunctionParameter {
            routine: SymbolId(1), position: None, symbol: SymbolId(3),
            ty: TypeId(1), implicit_this: true, compiler_generated: true,
        });
        for (scope, symbol) in [(0, SymbolId(4)), (1, SymbolId(5))] {
            il.local_variables.push(LocalVariable {
                routine: SymbolId(1), scope, symbol, ty: TypeId(1),
                storage_class: 0, compiler_generated: false,
                static_storage: false, structured_binding: false,
                handler_parameter: false,
            });
        }
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(graph.function_parameters(SymbolId(1)).count(), 1);
        assert_eq!(graph.implicit_this(SymbolId(1)).unwrap().symbol, SymbolId(3));
        assert_eq!(graph.locals_in_scope(SymbolId(1), 0).count(), 1);
        assert_eq!(graph.locals_in_scope(SymbolId(1), 1).count(), 1);
        assert_eq!(graph.local_variable_scope(SymbolId(5)).unwrap().parent, Some(0));
        il.local_scopes[0].parent = Some(1);
        assert!(matches!(Graph::from_il(&il), Err(GraphError::InvalidLocalScope { .. })));
        il.local_scopes[0].parent = Some(0);
        il.function_parameters[0].position = Some(2);
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidFunctionParameterPosition(SymbolId(1)))));
        il.function_parameters[0].position = Some(0);
        il.local_variables.push(il.local_variables[0].clone());
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::DuplicateLocalVariable(SymbolId(4)))));
    }

    #[test]
    fn audit_symbol_index_validates_many_parameter_and_local_kinds() {
        let mut il = Il::default();
        il.symbols.push(Symbol {
            id: SymbolId(1), kind: 1, name: "f".into(),
        });
        il.types.push(TypeDef {
            id: TypeId(1), kind: 1, related_role: None, related: None,
        });
        il.local_scopes.push(LocalScope {
            routine: SymbolId(1), id: 0, parent: None, kind: 17,
            statement_path: None,
        });
        for position in 0..128 {
            let symbol = SymbolId(position + 2);
            il.symbols.push(Symbol {
                id: symbol, kind: 2, name: format!("arg_{position}"),
            });
            il.function_parameters.push(FunctionParameter {
                routine: SymbolId(1), position: Some(position),
                symbol, ty: TypeId(1), implicit_this: false,
                compiler_generated: false,
            });
        }
        for index in 0..128 {
            let symbol = SymbolId(index + 130);
            il.symbols.push(Symbol {
                id: symbol, kind: 2, name: format!("local_{index}"),
            });
            il.local_variables.push(LocalVariable {
                routine: SymbolId(1), scope: 0, symbol, ty: TypeId(1),
                storage_class: 0, compiler_generated: false,
                static_storage: false, structured_binding: false,
                handler_parameter: false,
            });
        }
        let graph = Graph::from_il(&il).expect("many valid native declarations");
        assert_eq!(graph.function_parameters.len(), 128);
        assert_eq!(graph.local_variables.len(), 128);
        il.symbols[129].kind = 3;
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::InvalidLocalVariable(SymbolId(130)));
    }

    #[test]
    fn audit_indexed_scope_and_lifetime_validation_on_many_records() {
        let mut il = Il::default();
        il.symbols.push(Symbol { id: SymbolId(1), kind: 1, name: "f".into() });
        il.types.push(TypeDef {
            id: TypeId(1), kind: 1, related_role: None, related: None,
        });
        for i in 0..128u32 {
            let statement = i.to_string();
            let symbol = SymbolId(i + 2);
            il.symbols.push(Symbol {
                id: symbol, kind: 2, name: format!("static_{i}"),
            });
            il.local_scopes.push(LocalScope {
                routine: SymbolId(1), id: i,
                parent: if i == 0 { None } else { Some(i - 1) },
                kind: 17, statement_path: None,
            });
            il.local_variables.push(LocalVariable {
                routine: SymbolId(1), scope: i, symbol, ty: TypeId(1),
                storage_class: 0, compiler_generated: false,
                static_storage: true, structured_binding: false,
                handler_parameter: false,
            });
            il.static_local_owners.push(StaticLocalOwner {
                routine: SymbolId(1), symbol,
            });
            il.object_lifetimes.push(ObjectLifetime {
                routine: SymbolId(1), id: i,
                parent: if i == 0 { None } else { Some(i - 1) }, kind: 0,
            });
            il.scope_lifetimes.push(ScopeLifetime {
                routine: SymbolId(1), scope: i, lifetime: i,
            });
            il.lifetime_destructions.push(LifetimeDestruction {
                routine: SymbolId(1), lifetime: i, position: 0,
                variable: Some(symbol), destructor: None,
            });
            il.statements.push(Statement {
                function: "f".into(), path: statement.clone(),
                parent: None, kind: 1,
            });
            il.statement_initializations.push(StatementInitialization {
                function: "f".into(), statement_path: statement.clone(),
                symbol: Some(symbol), kind: 1, follows_execution: false,
                braced: false, role: 0,
            });
            il.initialization_lifetimes.push(InitializationLifetime {
                function: "f".into(), statement_path: statement.clone(),
                lifetime: Some(i), destructor: None,
            });
            il.vla_dimensions.push(VlaDimension {
                function: "f".into(), statement_path: statement,
                array_type: TypeId(1), has_expression: false, copied: false,
            });
        }
        let graph = Graph::from_il(&il).expect("valid indexed native metadata");
        assert_eq!(graph.scope_lifetimes.len(), 128);
        assert_eq!(graph.initialization_lifetimes.len(), 128);
        assert_eq!(graph.static_local_owners.len(), 128);
        il.lifetime_destructions.push(il.lifetime_destructions[0].clone());
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::DuplicateLifetimeDestruction {
                routine: SymbolId(1), lifetime: 0, position: 0,
            });
    }

    #[test]
    fn audit_parameter_and_local_types_match_native_symbol_types() {
        let mut il = Il::default();
        il.symbols.extend([
            Symbol { id: SymbolId(1), kind: 1, name: "f".into() },
            Symbol { id: SymbolId(2), kind: 2, name: "argument".into() },
            Symbol { id: SymbolId(3), kind: 2, name: "local".into() },
        ]);
        for id in [1, 2] {
            il.types.push(TypeDef {
                id: TypeId(id), kind: 2, related_role: None, related: None,
            });
        }
        il.function_parameters.push(FunctionParameter {
            routine: SymbolId(1), position: Some(0), symbol: SymbolId(2),
            ty: TypeId(1), implicit_this: false, compiler_generated: false,
        });
        il.local_scopes.push(LocalScope {
            routine: SymbolId(1), id: 0, parent: None,
            kind: 17, statement_path: None,
        });
        il.local_variables.push(LocalVariable {
            routine: SymbolId(1), scope: 0, symbol: SymbolId(3),
            ty: TypeId(1), storage_class: 0, compiler_generated: false,
            static_storage: false, structured_binding: false,
            handler_parameter: false,
        });
        il.symbol_types.extend([
            SymbolType { symbol: SymbolId(2), ty: TypeId(1) },
            SymbolType { symbol: SymbolId(3), ty: TypeId(1) },
        ]);
        Graph::from_il(&il).expect("matching native variable type records");

        il.symbol_types[0].ty = TypeId(2);
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidFunctionParameter {
                routine: SymbolId(1), symbol: SymbolId(2),
            })));
        il.symbol_types[0].ty = TypeId(1);
        il.symbol_types[1].ty = TypeId(2);
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::InvalidLocalVariable(SymbolId(3)));
    }

    #[test]
    fn audit_function_type_parameters_require_dense_positions() {
        let mut il = Il::default();
        for id in [1, 2] {
            il.types.push(TypeDef {
                id: TypeId(id), kind: if id == 1 { native_routine_type_kind() } else { 2 },
                related_role: None, related: None,
            });
        }
        il.type_parameters.push(TypeParameter {
            owner: TypeId(1), position: 1, ty: TypeId(2),
        });
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::InvalidTypeParameterPosition(TypeId(1)));
        il.type_parameters[0].position = 0;
        il.function_signatures.push(FunctionSignature {
            owner:TypeId(1),prototyped:true,variadic:false,
        });
        Graph::from_il(&il).expect("valid one-parameter native function type");

        il.type_parameters.push(TypeParameter {
            owner: TypeId(1), position: 2, ty: TypeId(2),
        });
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::InvalidTypeParameterPosition(TypeId(1)));
        il.type_parameters[1].position = 1;
        il.type_parameters.swap(0, 1);
        Graph::from_il(&il).expect("complete out-of-order wire records");
    }

    #[test]
    fn resolves_goto_labels_and_rejects_duplicate_definitions() {
        let mut il = Il::default();
        il.symbols.push(Symbol { id: SymbolId(4), kind: 4, name: "done".into() });
        for index in 0..=1 {
            il.statements.push(Statement {
                function: "jump".into(), path: index.to_string(),
                parent: None, kind: if index == 0 { 6 } else { 7 },
            });
            il.statement_labels.push(StatementLabel {
                function: "jump".into(), statement_path: index.to_string(),
                label: SymbolId(4), definition: index == 1,
            });
        }
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(graph.goto_target(StatementId(0)), Some(StatementId(1)));
        assert_eq!(graph.label_target(SymbolId(4)), Some(StatementId(1)));
        assert_eq!(graph.goto_target(StatementId(1)), None);
        il.statement_labels.push(il.statement_labels[1].clone());
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::DuplicateStatementLabel { .. })));
        il.statement_labels.pop();
        il.statement_labels[1].statement_path = "0".into();
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::DuplicateStatementLabel { .. })));
        il.statement_labels[1].statement_path = "1".into();
        il.symbols[0].kind = 1;
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidStatementLabel { .. })));
    }

    #[test]
    fn audit_rejects_cross_function_goto_label_identity() {
        let mut il = Il::default();
        il.symbols.push(Symbol {
            id: SymbolId(4), kind: 4, name: "same_name".into(),
        });
        il.symbols.push(Symbol {
            id: SymbolId(5), kind: 4, name: "same_name".into(),
        });
        for function in ["f", "g"] {
            il.statements.push(Statement {
                function: function.into(), path: "0".into(),
                parent: None, kind: 7,
            });
            il.statement_labels.push(StatementLabel {
                function: function.into(), statement_path: "0".into(),
                label: SymbolId(4), definition: function == "g",
            });
        }
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::InvalidStatementLabel {
                function: "g".into(), path: "0".into(),
            });
        // Equal source spellings are legal across functions, but only
        // when their native label symbols have distinct identities.
        il.statement_labels[1].label = SymbolId(5);
        let graph = Graph::from_il(&il).expect("separate function-local labels");
        assert_eq!(graph.label_target(SymbolId(5)), Some(StatementId(1)));
        assert_eq!(graph.goto_target(StatementId(0)), None);
    }

    #[test]
    fn routine_identity_is_not_inferred_from_symbol_display_name() {
        let mut il = Il::default();
        il.symbols.push(Symbol {
            id: SymbolId(1), kind: 1,
            name: "ns::overloaded(int)".into(),
        });
        il.routine_identities.push(RoutineIdentity {
            routine: SymbolId(1), function: "ns::overloaded".into(),
        });
        il.statements.push(Statement {
            function: "ns::overloaded".into(), path: "0".into(),
            parent: None, kind: 1,
        });
        il.local_scopes.push(LocalScope {
            routine: SymbolId(1), id: 0, parent: None, kind: 17,
            statement_path: Some("0".into()),
        });
        il.statement_initializations.push(StatementInitialization {
            function: "ns::overloaded".into(), statement_path: "0".into(),
            symbol: None, kind: 1, follows_execution: false,
            braced: false, role: 1,
        });
        il.object_lifetimes.push(ObjectLifetime {
            routine: SymbolId(1), id: 0, parent: None, kind: 1,
        });
        il.initialization_lifetimes.push(InitializationLifetime {
            function: "ns::overloaded".into(), statement_path: "0".into(),
            lifetime: Some(0), destructor: None,
        });
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(graph.routine_for_function("ns::overloaded"), Some(SymbolId(1)));
        assert_eq!(graph.routine_function(SymbolId(1)), Some("ns::overloaded"));
        assert_eq!(graph.initialization_lifetime("ns::overloaded", "0")
            .unwrap().lifetime, Some(0));
        il.routine_identities.push(RoutineIdentity {
            routine: SymbolId(1), function: "other".into(),
        });
        assert_eq!(Graph::from_il(&il).unwrap_err(),
                   GraphError::DuplicateRoutineIdentity(SymbolId(1)));
    }

    #[test]
    fn lifetime_tree_preserves_scope_links_and_cleanup_order() {
        let mut il = Il::default();
        il.symbols.extend([
            Symbol { id: SymbolId(1), kind: 1, name: "f".into() },
            Symbol { id: SymbolId(2), kind: 2, name: "object".into() },
            Symbol { id: SymbolId(3), kind: 1, name: "Object::~Object".into() },
        ]);
        il.local_scopes.push(LocalScope {
            routine: SymbolId(1), id: 0, parent: None,
            kind: 17, statement_path: None,
        });
        il.statements.push(Statement {
            function: "f".into(), path: "0".into(), parent: None, kind: 18,
        });
        il.statement_initializations.push(StatementInitialization {
            function: "f".into(), statement_path: "0".into(),
            symbol: Some(SymbolId(2)), kind: 3,
            follows_execution: false, braced: false, role: 0,
        });
        il.object_lifetimes.extend([
            ObjectLifetime { routine: SymbolId(1), id: 0, parent: None, kind: 1 },
            ObjectLifetime { routine: SymbolId(1), id: 1, parent: Some(0), kind: 2 },
        ]);
        il.scope_lifetimes.push(ScopeLifetime {
            routine: SymbolId(1), scope: 0, lifetime: 0,
        });
        il.initialization_lifetimes.push(InitializationLifetime {
            function: "f".into(), statement_path: "0".into(),
            lifetime: Some(1), destructor: Some(SymbolId(3)),
        });
        il.lifetime_destructions.push(LifetimeDestruction {
            routine: SymbolId(1), lifetime: 1, position: 0,
            variable: Some(SymbolId(2)), destructor: Some(SymbolId(3)),
        });
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(graph.object_lifetime(SymbolId(1), 1).unwrap().parent, Some(0));
        assert_eq!(graph.scope_lifetime(SymbolId(1), 0).unwrap().id, 0);
        assert_eq!(graph.initialization_lifetime("f", "0").unwrap().lifetime, Some(1));
        assert_eq!(graph.lifetime_destructions(SymbolId(1), 1).len(), 1);
        il.object_lifetimes[1].parent = Some(1);
        assert!(matches!(Graph::from_il(&il), Err(GraphError::InvalidLifetime { .. })));
        il.object_lifetimes[1].parent = Some(0);
        il.lifetime_destructions[0].position = 2;
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidLifetimeDestructionPosition { .. })));
        il.lifetime_destructions[0].position = 0;
        il.initialization_lifetimes[0].lifetime = None;
        il.initialization_lifetimes[0].destructor = None;
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidInitializationLifetime { .. })));
        il.initialization_lifetimes[0].lifetime = Some(1);
        il.initialization_lifetimes[0].destructor = Some(SymbolId(3));
        il.scope_lifetimes[0].lifetime = 99;
        assert!(matches!(Graph::from_il(&il), Err(GraphError::InvalidScopeLifetime { .. })));
    }

    #[test]
    fn source_locations_link_only_to_existing_il_nodes() {
        let mut il = Il::default();
        il.symbols.push(Symbol { id: SymbolId(2), kind: 2, name: "variable".into() });
        il.statements.push(Statement { function: "f".into(), path: "0".into(),
                                       parent: None, kind: 0 });
        il.expressions.push(Expression {
            function: "f".into(), statement_path: "0".into(),
            path: "0".into(), parent: None, kind: 1,
            operator: None, type_kind: None, reference: None,
            type_size: None, type_alignment: None, constant_kind: None,
            constant_integer: None, related_type_role: None,
            related_type_kind: None, type_id: None, symbol_id: None,
        });
        let location = SourceLocation {
            sequence: 12, column: 7, file: "example.cpp".into(), line: 4,
        };
        il.statement_sources.push(StatementSource {
            function: "f".into(), path: "0".into(), location: location.clone(),
        });
        il.symbol_sources.push(SymbolSource {
            symbol: SymbolId(2), location: location.clone(),
        });
        il.expression_sources.push(ExpressionSource {
            function: "f".into(), statement_path: "0".into(), role: 0,
            path: "0".into(), location: location.clone(),
        });
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(graph.statement_source("f", "0"), Some(&location));
        assert_eq!(graph.symbol_source(SymbolId(2)), Some(&location));
        assert_eq!(graph.expression_source("f", "0", 0, "0"), Some(&location));
        il.expression_sources[0].path = "0.1".into();
        assert!(matches!(Graph::from_il(&il), Err(GraphError::InvalidSourceLocation { .. })));
        il.expression_sources[0].path = "0".into();
        il.statement_sources.push(il.statement_sources[0].clone());
        assert!(matches!(Graph::from_il(&il), Err(GraphError::DuplicateSourceLocation { .. })));
        il.statement_sources.pop();
        il.symbol_sources[0].symbol = SymbolId(99);
        assert_eq!(Graph::from_il(&il).unwrap_err(), GraphError::InvalidSymbolSource(SymbolId(99)));
    }

    #[test]
    fn m11_vla_dimensions_keep_execution_identity_and_reject_bad_links() {
        let mut il=Il::default();
        il.statements.push(Statement {
            function:"vla".into(),path:"0".into(),parent:None,kind:1,
        });
        il.types.push(TypeDef {
            id:TypeId(7),kind:6,related_role:None,related:None,
        });
        il.expressions.push(Expression {
            function:"vla".into(),statement_path:"0".into(),
            path:"0".into(),parent:None,kind:1,operator:None,
            type_kind:None,reference:None,type_size:None,
            type_alignment:None,constant_kind:None,
            constant_integer:Some(12),related_type_role:None,
            related_type_kind:None,type_id:None,symbol_id:None,
        });
        il.vla_dimensions.push(VlaDimension {
            function:"vla".into(),statement_path:"0".into(),
            array_type:TypeId(7),has_expression:true,copied:false,
        });
        let graph=Graph::from_il(&il).unwrap();
        assert_eq!(graph.vla_dimension_at("vla","0").unwrap().array_type,TypeId(7));
        assert!(graph.vla_dimension_at("vla","1").is_none());
        il.vla_dimensions.push(il.vla_dimensions[0].clone());
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::DuplicateVlaDimension {..})));
        il.vla_dimensions.pop();
        il.vla_dimensions[0].array_type=TypeId(999);
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidVlaDimension {..})));
        il.vla_dimensions[0].array_type=TypeId(7);
        il.expressions.clear();
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidVlaDimension {..})));
        il.vla_dimensions[0].has_expression=false;
        il.vla_dimensions[0].copied=true;
        assert!(Graph::from_il(&il).is_ok());
    }

    #[test]
    fn declaration_sites_and_initializer_execution_are_distinct() {
        let mut il = Il::default();
        for path in ["0", "1"] {
            il.statements.push(Statement {
                function: "f".into(), path: path.into(), parent: None, kind: 1,
            });
        }
        for id in 1..=2 {
            il.symbols.push(Symbol {
                id: SymbolId(id), kind: 2, name: format!("var{id}"),
            });
        }
        il.types.push(TypeDef {
            id: TypeId(7), kind: 2, related_role: None, related: None,
        });
        for (position, symbol) in [(1, SymbolId(2)), (0, SymbolId(1))] {
            il.statement_declarations.push(StatementDeclaration {
                function: "f".into(), statement_path: "0".into(),
                position, symbol, ty: TypeId(7), is_vla: false,
            });
        }
        il.statement_initializations.push(StatementInitialization {
            function: "f".into(), statement_path: "1".into(),
            symbol: Some(SymbolId(1)), kind: 1,
            follows_execution: true, braced: false, role: 0,
        });
        il.control_expressions.push(ControlExpression {
            function: "f".into(), statement_path: "1".into(), role: 4,
            path: "0".into(), parent: None, kind: 1, operator: None,
            type_id: Some(TypeId(7)), symbol_id: None,
        });
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(graph.declarations_at("f", "0").map(|d| d.symbol)
            .collect::<Vec<_>>(), vec![SymbolId(1), SymbolId(2)]);
        assert_eq!(graph.declaration_sites(SymbolId(1)).count(), 1);
        assert_eq!(graph.variable_initialization_sites(SymbolId(1)).count(), 1);
        assert!(graph.statement_initialization("f", "1").unwrap().follows_execution);
        assert_eq!(graph.statement_initializer_expression("f", "1").unwrap().path, "0");
        assert!(graph.statement_initializer_expression("f", "0").is_none());

        il.statement_declarations[0].position = 2;
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidStatementDeclarationPosition { .. })));
        il.statement_declarations[0].position = 1;
        il.statement_declarations.push(il.statement_declarations[0].clone());
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::DuplicateStatementDeclaration { .. })));
        il.statement_declarations.pop();
        il.statement_initializations.clear();
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidControlExpression { .. })));
        il.control_expressions.clear();
        il.statement_declarations[0].symbol = SymbolId(999);
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidStatementDeclaration { .. })));
    }

    #[test]
    fn audit_statement_declaration_type_must_match_symbol_type() {
        let mut il=Il::default();
        il.statements.push(Statement {
            function:"f".into(),path:"0".into(),parent:None,kind:1,
        });
        il.symbols.push(Symbol {
            id:SymbolId(3),kind:2,name:"local".into(),
        });
        for id in 1..=2 {
            il.types.push(TypeDef {
                id:TypeId(id),kind:2,related_role:None,related:None,
            });
        }
        il.symbol_types.push(SymbolType {
            symbol:SymbolId(3),ty:TypeId(1),
        });
        il.statement_declarations.push(StatementDeclaration {
            function:"f".into(),statement_path:"0".into(),
            position:0,symbol:SymbolId(3),ty:TypeId(1),is_vla:false,
        });
        Graph::from_il(&il).expect("native declaration agrees with symbol type");
        il.statement_declarations[0].ty=TypeId(2);
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::InvalidStatementDeclaration {
                function:"f".into(),path:"0".into(),
            });
    }

    #[test]
    fn validates_statement_flags_and_auxiliary_control_expressions() {
        let mut il = Il::default();
        il.statements.push(Statement {
            function: "loop".into(), path: "0".into(), parent: None, kind: 14,
        });
        il.statement_attributes.push(StatementAttributes {
            function: "loop".into(), statement_path: "0".into(),
            compiler_generated: false, initialization_guard: false,
            fallthrough: false, likely: true, unlikely: false,
            lowering_generated: false, lowering_boilerplate: false,
        });
        for (path, parent) in [("0.1", Some("0")), ("0", None), ("0.0", Some("0"))] {
            il.control_expressions.push(ControlExpression {
                function: "loop".into(), statement_path: "0".into(),
                role: 1, path: path.into(), parent: parent.map(str::to_owned),
                kind: 1, operator: None, type_id: None, symbol_id: None,
            });
        }
        let graph = Graph::from_il(&il).unwrap();
        assert!(graph.statement_attributes("loop", "0").unwrap().likely);
        let root = graph.control_expression_root("loop", "0", 1).unwrap();
        assert_eq!(graph.control_operands(root).iter()
                   .map(|node| node.path.as_str()).collect::<Vec<_>>(),
                   vec!["0.0", "0.1"]);
        il.control_expressions[0].path = "0.2".into();
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidControlExpressionOperandPath { .. })));
        il.control_expressions[0].path = "0.1".into();
        il.control_expressions.push(il.control_expressions[0].clone());
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::DuplicateControlExpression { .. })));
        il.control_expressions.pop();
        il.statement_attributes[0].unlikely = true;
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidStatementAttributes { .. })));
    }

    #[test]
    fn constructor_records_are_independent_of_symbol_callback_order() {
        let mut il = Il::default();
        il.constructor_initializers.push(ConstructorInitializer {
            routine: SymbolId(10), position: 0, kind: 3,
            target_type: None, target_field: None,
            compiler_generated: false, braced: false,
            use_field_initializer: false, pack_expansion: false,
        });
        il.member_functions.push(MemberFunction {
            owner: TypeId(20), routine: SymbolId(10),
        });
        il.symbols.push(Symbol {
            id: SymbolId(10), kind: 1, name: "Widget::Widget".into(),
        });
        il.types.push(TypeDef {
            id: TypeId(20), kind: 1, related_role: None, related: None,
        });
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(graph.member_function_owner(SymbolId(10)), Some(TypeId(20)));
        assert_eq!(graph.constructor_initializers(SymbolId(10)).count(), 1);
    }

    #[test]
    fn indexes_constructor_expression_operands_in_path_order() {
        let mut il = Il::default();
        il.symbols.push(Symbol { id: SymbolId(1), kind: 1, name: "Widget::Widget".into() });
        il.constructor_initializers.push(ConstructorInitializer {
            routine: SymbolId(1), position: 0, kind: 3,
            target_type: None, target_field: None,
            compiler_generated: false, braced: false,
            use_field_initializer: false, pack_expansion: false,
        });
        for (path, parent) in [
            ("0.1", Some("0")), ("0", None), ("0.0", Some("0")),
        ] {
            il.constructor_initializer_expressions.push(ConstructorInitializerExpression {
                routine: SymbolId(1), position: 0,
                path: path.into(), parent: parent.map(str::to_owned),
                kind: 1, operator: None, type_id: None, symbol_id: None,
            });
        }
        let graph = Graph::from_il(&il).unwrap();
        let root = graph.constructor_initializer_root(SymbolId(1), 0).unwrap();
        assert_eq!(root.path, "0");
        let operands = graph.constructor_initializer_operands(root);
        assert_eq!(operands.iter().map(|node| node.path.as_str()).collect::<Vec<_>>(),
                   vec!["0.0", "0.1"]);
        il.constructor_initializer_expressions.push(il.constructor_initializer_expressions[0].clone());
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::DuplicateConstructorExpression { .. })));
        il.constructor_initializer_expressions.pop();
        il.constructor_initializer_expressions[2].path = "0.2".into();
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidConstructorExpressionOperandPath { .. })));
        il.constructor_initializer_expressions.remove(1);
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::MissingConstructorExpressionRoot { .. })));
    }

    #[test]
    fn validates_routine_attributes_and_duplicate_records() {
        let mut il = Il::default();
        il.symbols.push(Symbol {
            id: SymbolId(1), kind: 1, name: "Widget::method".into(),
        });
        let attributes = RoutineAttributes {
            routine: SymbolId(1), special_kind: 0,
            is_virtual: true, pure_virtual: false,
            is_override: true, is_final: true,
            overrides_base_member: true,
            is_inline: false, is_constexpr: false,
            defined: true, explicit_constructor: false,
            has_this_parameter: true, compiler_generated: false,
        };
        il.routine_attributes.push(attributes.clone());
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(graph.routine_attributes(SymbolId(1)), Some(&attributes));
        il.routine_attributes.push(attributes);
        assert_eq!(Graph::from_il(&il).unwrap_err(),
                   GraphError::DuplicateRoutineAttributes(SymbolId(1)));
        il.routine_attributes.pop();
        il.routine_attributes[0].is_virtual = false;
        assert_eq!(Graph::from_il(&il).unwrap_err(),
                   GraphError::InvalidRoutineAttributes(SymbolId(1)));
    }

    #[test]
    fn validates_member_function_ownership() {
        let mut il = Il::default();
        il.types.push(TypeDef {
            id: TypeId(1), kind: 1, related_role: None, related: None,
        });
        il.symbols.push(Symbol {
            id: SymbolId(1), kind: 1, name: "C::f".into(),
        });
        il.member_functions.push(MemberFunction {
            owner: TypeId(1), routine: SymbolId(1),
        });
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(graph.member_function_owner(SymbolId(1)), Some(TypeId(1)));
        assert_eq!(graph.member_functions(TypeId(1)).count(), 1);
        il.member_functions.push(il.member_functions[0].clone());
        assert_eq!(Graph::from_il(&il).unwrap_err(),
                   GraphError::DuplicateMemberFunction(SymbolId(1)));
    }

    #[test]
    fn audit_constructor_initializer_rejects_foreign_record_field() {
        let mut il = Il::default();
        for id in 1..=3 {
            il.types.push(TypeDef {
                id: TypeId(id),
                kind: if id < 3 {native_type_kind_for_semantic(7)} else {1},
                related_role: None, related: None,
            });
        }
        for (id, kind, name) in [
            (1, 1, "Owner::Owner"),
            (2, 3, "member"),
            (3, 3, "foreign_member"),
        ] {
            il.symbols.push(Symbol {
                id: SymbolId(id), kind, name: name.into(),
            });
        }
        il.member_functions.push(MemberFunction {
            owner: TypeId(1), routine: SymbolId(1),
        });
        for (owner, symbol) in [(1, 2), (2, 3)] {
            il.record_fields.push(RecordField {
                owner: TypeId(owner), position: 0, ty: TypeId(3),
                symbol: SymbolId(symbol), name: format!("field_{symbol}"),
                byte_offset: 0, bit_offset: 0, bit_width: 0,
            });
        }
        il.constructor_initializers.push(ConstructorInitializer {
            routine: SymbolId(1), position: 0, kind: 2,
            target_type: None, target_field: Some(SymbolId(3)),
            compiler_generated: false, braced: false,
            use_field_initializer: false, pack_expansion: false,
        });
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::InvalidConstructorInitializer {
                routine: SymbolId(1), position: 0,
            });
        il.constructor_initializers[0].target_field = Some(SymbolId(2));
        Graph::from_il(&il).expect("constructor initializes own member");
    }

    #[test]
    fn validates_constructor_initializer_targets_and_order() {
        let mut il = Il::default();
        il.symbols.push(Symbol { id: SymbolId(1), kind: 1, name: "C::C".into() });
        il.symbols.push(Symbol { id: SymbolId(2), kind: 3, name: "member".into() });
        il.types.push(TypeDef {
            id: TypeId(1), kind: 1, related_role: None, related: None,
        });
        il.constructor_initializers.push(ConstructorInitializer {
            routine: SymbolId(1), position: 1, kind: 2,
            target_type: None, target_field: Some(SymbolId(2)),
            compiler_generated: false, braced: true,
            use_field_initializer: false, pack_expansion: false,
        });
        il.constructor_initializers.push(ConstructorInitializer {
            routine: SymbolId(1), position: 0, kind: 1,
            target_type: Some(TypeId(1)), target_field: None,
            compiler_generated: true, braced: false,
            use_field_initializer: false, pack_expansion: false,
        });
        let graph = Graph::from_il(&il).unwrap();
        let entries: Vec<_> = graph.constructor_initializers(SymbolId(1)).collect();
        assert_eq!(entries.iter().map(|v| v.kind).collect::<Vec<_>>(), vec![1, 2]);
        assert!(entries[1].braced);
        il.constructor_initializers[0].position = 3;
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidConstructorInitializer { .. })));
    }

    #[test]
    fn indexes_symbol_types_and_rejects_duplicates() {
        let mut il = Il::default();
        il.symbols.push(Symbol {
            id: SymbolId(1), kind: 1, name: "f".into(),
        });
        il.types.push(TypeDef {
            id: TypeId(2), kind: 8, related_role: None, related: None,
        });
        il.symbol_types.push(SymbolType { symbol: SymbolId(1), ty: TypeId(2) });
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(graph.symbol_type(SymbolId(1)), Some(TypeId(2)));
        assert_eq!(graph.symbol_type(SymbolId(99)), None);
        il.symbol_types.push(il.symbol_types[0].clone());
        assert_eq!(Graph::from_il(&il).unwrap_err(),
                   GraphError::DuplicateSymbolType(SymbolId(1)));
        il.symbol_types.pop();
        il.symbol_types[0].ty = TypeId(100);
        assert_eq!(Graph::from_il(&il).unwrap_err(),
                   GraphError::InvalidSymbolType(SymbolId(1)));
    }

    #[test]
    fn audit_symbol_type_edges_cannot_attach_to_fields_or_labels() {
        let mut il=Il::default();
        il.types.push(TypeDef {
            id:TypeId(1),kind:2,related_role:None,related:None,
        });
        il.symbols.push(Symbol {
            id:SymbolId(1),kind:2,name:"variable".into(),
        });
        il.symbol_types.push(SymbolType {
            symbol:SymbolId(1),ty:TypeId(1),
        });
        Graph::from_il(&il).expect("native variable type edge");
        for invalid_kind in [3,4] {
            il.symbols[0].kind=invalid_kind;
            assert_eq!(Graph::from_il(&il).unwrap_err(),
                GraphError::InvalidSymbolType(SymbolId(1)));
        }
        il.symbols[0].kind=1;
        Graph::from_il(&il).expect("native routine type edge");
    }

    #[test]
    fn validates_direct_base_access_edges() {
        let mut il = Il::default();
        for id in 1..=2 {
            il.types.push(TypeDef {
                id: TypeId(id), kind: 1, related_role: None, related: None,
            });
        }
        il.base_classes.push(BaseClass {
            derived: TypeId(1), position: 0, base: TypeId(2), is_virtual: false,
        });
        il.base_class_access.push(BaseClassAccess {
            derived: TypeId(1), position: 0, access: Some(1),
            pack_expansion: false,
        });
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(graph.base_access(TypeId(1), 0), il.base_class_access.first());
        il.base_class_access.push(il.base_class_access[0].clone());
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::DuplicateBaseClassAccess { .. })));
        il.base_class_access.pop();
        il.base_class_access[0].position = 1;
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidBaseClassAccess { .. })));
    }

    #[test]
    fn orders_direct_base_classes_and_rejects_duplicates() {
        let mut il = Il::default();
        for id in 1..=3 {
            il.types.push(TypeDef {
                id: TypeId(id), kind: 1, related_role: None, related: None,
            });
        }
        il.base_classes.push(BaseClass {
            derived: TypeId(1), position: 1, base: TypeId(3), is_virtual: true,
        });
        il.base_classes.push(BaseClass {
            derived: TypeId(1), position: 0, base: TypeId(2), is_virtual: false,
        });
        let graph = Graph::from_il(&il).unwrap();
        let bases: Vec<_> = graph.bases(TypeId(1)).collect();
        assert_eq!(bases.iter().map(|b| b.base).collect::<Vec<_>>(),
                   vec![TypeId(2), TypeId(3)]);
        assert!(!bases[0].is_virtual);
        assert!(bases[1].is_virtual);
        il.base_classes.push(il.base_classes[0].clone());
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::DuplicateBaseClass { .. })));
    }

    #[test]
    fn rejects_sparse_and_cyclic_inheritance() {
        let mut il = Il::default();
        for id in 1..=3 {
            il.types.push(TypeDef {
                id: TypeId(id), kind: 1, related_role: None, related: None,
            });
        }
        il.base_classes.push(BaseClass {
            derived: TypeId(1), position: 1, base: TypeId(2), is_virtual: false,
        });
        assert_eq!(Graph::from_il(&il).unwrap_err(),
                   GraphError::InvalidBaseClassPosition(TypeId(1)));
        il.base_classes[0].position = 0;
        il.base_classes.push(BaseClass {
            derived: TypeId(2), position: 0, base: TypeId(3), is_virtual: false,
        });
        il.base_classes.push(BaseClass {
            derived: TypeId(3), position: 0, base: TypeId(1), is_virtual: false,
        });
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::CyclicInheritance(_))));
    }

    #[test]
    fn rejects_initializer_tree_without_root() {
        let mut il = Il::default();
        il.symbols.push(Symbol { id: SymbolId(1), kind: 2, name: "x".into() });
        il.initializer_expressions.push(InitializerExpression {
            owner: SymbolId(1), path: "0.0".into(),
            parent: Some("0".into()), kind: 1, operator: None,
            type_id: None, symbol_id: None,
        });
        assert_eq!(Graph::from_il(&il).unwrap_err(),
                   GraphError::MissingInitializerRoot(SymbolId(1)));
    }

    #[test]
    fn initializer_tree_queries_preserve_operand_order() {
        let mut il = Il::default();
        il.symbols.push(Symbol { id: SymbolId(1), kind: 2, name: "value".into() });
        for (path, parent) in [
            ("0.2", Some("0")), ("0", None), ("0.1", Some("0")),
            ("0.0", Some("0")),
        ] {
            il.initializer_expressions.push(InitializerExpression {
                owner: SymbolId(1), path: path.into(),
                parent: parent.map(str::to_owned), kind: 1,
                operator: None, type_id: None, symbol_id: None,
            });
        }
        let graph = Graph::from_il(&il).unwrap();
        let root = graph.initializer_root(SymbolId(1)).unwrap();
        assert_eq!(root.path, "0");
        let paths: Vec<_> = graph.initializer_operands(root)
            .iter().map(|node| node.path.as_str()).collect();
        assert_eq!(paths, ["0.0", "0.1", "0.2"]);
        assert_eq!(graph.initializer_expression(SymbolId(1), "0.1").unwrap().path, "0.1");
        assert!(graph.initializer_expression(SymbolId(1), "0.9").is_none());
        assert!(graph.initializer_root(SymbolId(99)).is_none());
    }

    #[test]
    fn links_statement_and_ordered_operands() {
        let mut il = Il::default();
        il.statements.push(Statement { function:"f".into(), path:"0".into(), parent:None, kind:11 });
        il.statements.push(Statement { function:"f".into(), path:"0.block.0".into(), parent:Some("0".into()), kind:8 });
        for (path,parent) in [("0",None),("0.1",Some("0")),("0.0",Some("0"))] {
            il.expressions.push(Expression {
                function:"f".into(), statement_path:"0.block.0".into(),
                path:path.into(), parent:parent.map(str::to_string),
                kind:1, operator:Some(1), type_kind:Some(2), reference:None, type_size:Some(4), type_alignment:Some(4), constant_kind:None, constant_integer:None, related_type_role:None, related_type_kind:None, type_id:None, symbol_id:None,
            });
        }
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(graph.statements[0].children, vec![StatementId(1)]);
        let root = graph.statements[1].expression.unwrap();
        assert_eq!(graph.expressions[root.0].operands, vec![ExpressionId(2), ExpressionId(1)]);
        assert_eq!(graph.expressions[2].parent, Some(root));
    }
    #[test]
    fn resolves_call_callee_object_and_arguments() {
        let mut il = Il::default();
        il.statements.push(Statement {
            function: "f".into(), path: "0".into(), parent: None, kind: 1,
        });
        for (path, parent) in [
            ("0", None), ("0.3", Some("0")), ("0.1", Some("0")),
            ("0.0", Some("0")), ("0.2", Some("0")),
        ] {
            il.expressions.push(Expression {
                function: "f".into(), statement_path: "0".into(),
                path: path.into(), parent: parent.map(str::to_owned),
                kind: if path=="0" { native_call_kinds(true).0 } else {1},
                operator: (path=="0").then(||native_call_kinds(true).1),
                type_kind: None, reference: None,
                type_size: None, type_alignment: None, constant_kind: None,
                constant_integer: None, related_type_role: None,
                related_type_kind: None, type_id: None, symbol_id: None,
            });
        }
        il.calls.push(CallSite {
            function: "f".into(), statement_path: "0".into(),
            path: "0".into(), first_argument: 2,
        });
        let graph = Graph::from_il(&il).unwrap();
        let call = &graph.calls[0];
        assert_eq!(graph.call_callee(call), Some(ExpressionId(3)));
        assert_eq!(graph.call_object(call), Some(ExpressionId(2)));
        assert_eq!(graph.call_arguments(call), Some(&[ExpressionId(4), ExpressionId(1)][..]));
    }

    #[test]
    fn validated_implicit_pointer_casts_retain_source_type_by_expression_id() {
        let op=(0..4096).find(|&v|unsafe {edgcpp_codegen_kind(3,v)}==16)
            .expect("native EDG eok_cast");
        let expr_kind=(0..4096).find(|&v|unsafe {edgcpp_codegen_kind(2,v)}==3)
            .expect("native EDG expression operation");
        let mut il=Il::default();
        let integer=native_type_kind_for_semantic(1);
        let pointer=native_type_kind_for_semantic(5);
        il.types.extend([
            TypeDef{id:TypeId(1),kind:integer,related_role:None,related:None},
            TypeDef{id:TypeId(2),kind:pointer,related_role:Some(1),related:Some(TypeId(1))},
            TypeDef{id:TypeId(3),kind:pointer,related_role:Some(1),related:Some(TypeId(1))},
        ]);
        il.statements.push(Statement{
            function:"f".into(),path:"0".into(),parent:None,kind:1,
        });
        for (path,parent,kind,operator,ty) in [
            ("0",None,expr_kind,Some(op),TypeId(3)),
            ("0.0",Some("0"),1,None,TypeId(2)),
        ] {
            il.expressions.push(Expression {
                function:"f".into(),statement_path:"0".into(),
                path:path.into(),parent:parent.map(str::to_owned),
                kind,operator,type_id:Some(ty),
                type_kind:None,reference:None,type_size:None,
                type_alignment:None,constant_kind:None,constant_integer:None,
                related_type_role:None,related_type_kind:None,symbol_id:None,
            });
        }
        let record=crate::ImplicitPointerCast {
            function:"f".into(),statement_path:"0".into(),
            path:"0".into(),source_type:TypeId(2),
        };
        il.implicit_pointer_casts.push(record.clone());
        let graph=Graph::from_il(&il).expect("native cast source provenance");
        assert_eq!(graph.implicit_pointer_cast_source(ExpressionId(0)),Some(TypeId(2)));
        assert_eq!(graph.implicit_pointer_cast_source(ExpressionId(1)),None);
        il.implicit_pointer_casts[0].source_type=TypeId(3);
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidImplicitPointerCast{..})));
        il.implicit_pointer_casts[0]=record.clone();
        il.implicit_pointer_casts.push(record.clone());
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidImplicitPointerCast{..})));
        il.implicit_pointer_casts.pop();
        il.implicit_pointer_casts[0].path="0.99".into();
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidImplicitPointerCast{..})));
    }

    #[test]
    fn resolves_regular_call_with_no_object() {
        let mut il = Il::default();
        il.statements.push(Statement {
            function: "f".into(), path: "0".into(), parent: None, kind: 1,
        });
        for (path, parent) in [("0.1", Some("0")), ("0", None), ("0.0", Some("0"))] {
            il.expressions.push(Expression {
                function: "f".into(), statement_path: "0".into(),
                path: path.into(), parent: parent.map(str::to_owned),
                kind: if path=="0" {native_call_kinds(false).0} else {1},
                operator: (path=="0").then(||native_call_kinds(false).1),
                type_kind: None, reference: None,
                type_size: None, type_alignment: None, constant_kind: None,
                constant_integer: None, related_type_role: None,
                related_type_kind: None, type_id: None, symbol_id: None,
            });
        }
        il.calls.push(CallSite {
            function: "f".into(), statement_path: "0".into(),
            path: "0".into(), first_argument: 1,
        });
        let graph = Graph::from_il(&il).unwrap();
        let call = &graph.calls[0];
        let root = graph.call_expression(call).unwrap();
        assert_eq!(root, ExpressionId(1));
        assert_eq!(graph.call_at(root), Some(call));
        // Equal detached records are intentionally accepted; the complete
        // identity (including semantic operand offset) must still match.
        let cloned=call.clone();
        assert_eq!(graph.call_expression(&cloned), Some(root));
        let mut incompatible=cloned.clone();
        incompatible.first_argument=2;
        assert_eq!(graph.call_expression(&incompatible), None);
        assert_eq!(graph.call_callee(call), Some(ExpressionId(2)));
        assert_eq!(graph.call_object(call), None);
        assert_eq!(graph.call_arguments(call), Some(&[ExpressionId(0)][..]));
    }

    #[test]
    fn audit_indexed_call_lookups_preserve_identity_with_many_calls() {
        let mut il = Il::default();
        let (call_kind, call_operator) = native_call_kinds(false);
        // Interleave calls with non-calls and deliver call records in the
        // reverse order, independent of expression callback ordering.
        for index in 0..256 {
            let statement = index.to_string();
            let is_call = index % 3 == 0;
            il.statements.push(Statement {
                function: "f".into(), path: statement.clone(),
                parent: None, kind: 1,
            });
            il.expressions.push(Expression {
                function: "f".into(), statement_path: statement.clone(),
                path: "0".into(), parent: None,
                kind: if is_call { call_kind } else { 1 },
                operator: if is_call { Some(call_operator) } else { None },
                type_kind: None, reference: None, type_size: None,
                type_alignment: None, constant_kind: None,
                constant_integer: None, related_type_role: None,
                related_type_kind: None, type_id: None, symbol_id: None,
            });
            if is_call {
                il.expressions.push(Expression {
                    function: "f".into(), statement_path: statement.clone(),
                    path: "0.0".into(), parent: Some("0".into()),
                    kind: 1, operator: None, type_kind: None, reference: None,
                    type_size: None, type_alignment: None, constant_kind: None,
                    constant_integer: None, related_type_role: None,
                    related_type_kind: None, type_id: None, symbol_id: None,
                });
                il.calls.push(CallSite {
                    function: "f".into(), statement_path: statement,
                    path: "0".into(), first_argument: 1,
                });
            }
        }
        il.calls.reverse();
        let graph = Graph::from_il(&il).expect("valid interleaved call records");
        assert_eq!(graph.call_index_by_expression.len(), graph.expressions.len());
        for (index, node) in graph.expressions.iter().enumerate() {
            let indexed = graph.call_at(ExpressionId(index));
            if node.data.path == "0" && node.data.statement_path.parse::<usize>()
                .is_ok_and(|position| position % 3 == 0)
            {
                let call = indexed.expect("every native call has metadata");
                assert_eq!(call.statement_path, node.data.statement_path);
                assert_eq!(graph.call_expression(call), Some(ExpressionId(index)));
                assert_eq!(graph.call_arguments(call), Some(&[][..]));
            } else {
                assert!(indexed.is_none(), "non-call expressions have no call record");
            }
        }
        assert!(graph.call_at(ExpressionId(graph.expressions.len())).is_none());
    }

    /// Same-runner measurement of the pre-#141/#157 linear scans versus
    /// the current call-expression and call-site indexes. Run manually with
    /// --ignored --nocapture; this is a lookup microbenchmark, not full CFG.
    #[test]
    #[ignore = "opt-in call lookup scaling measurement"]
    fn audit_call_lookup_scaling() {
        for count in [512_usize, 2048, 4096] {
            let mut graph=Graph {
                call_index_by_expression:vec![None;count*2],
                ..Default::default()
            };
            let mut keys=Vec::with_capacity(count*2);
            for i in 0..count {
                let statement=i.to_string();
                let call=CallSite {
                    function:"benchmark".into(), statement_path:statement.clone(),
                    path:"0".into(), first_argument:1,
                };
                let index=graph.calls.len();
                graph.call_index_by_expression[i*2]=Some(index);
                graph.call_expression_by_site.insert(call.clone(),ExpressionId(i*2));
                graph.calls.push(call);
                keys.push(("benchmark".to_owned(),statement,"0".to_owned()));
                keys.push(("benchmark".to_owned(),format!("missing_{i}"),"0".to_owned()));
            }
            let mut old_forward=Vec::new();
            let mut indexed_forward=Vec::new();
            let mut old_reverse=Vec::new();
            let mut indexed_reverse=Vec::new();
            for _ in 0..3 {
                let started=std::time::Instant::now();
                let legacy:Vec<_>=keys.iter().map(|(function,statement,path)|
                    graph.calls.iter().find(|call|
                        call.function==*function && call.statement_path==*statement &&
                        call.path==*path)).collect();
                old_forward.push(started.elapsed());
                let started=std::time::Instant::now();
                let indexed:Vec<_>=(0..keys.len())
                    .map(|i|graph.call_at(ExpressionId(i))).collect();
                indexed_forward.push(started.elapsed());
                assert_eq!(legacy,indexed);
                std::hint::black_box(&legacy);
                std::hint::black_box(&indexed);

                let started=std::time::Instant::now();
                let legacy:Vec<_>=graph.calls.iter().map(|call|
                    graph.calls.iter().position(|candidate|candidate==call)
                        .map(|position|ExpressionId(position*2))).collect();
                old_reverse.push(started.elapsed());
                let started=std::time::Instant::now();
                let indexed:Vec<_>=graph.calls.iter()
                    .map(|call|graph.call_expression(call)).collect();
                indexed_reverse.push(started.elapsed());
                assert_eq!(legacy,indexed);
                std::hint::black_box(&legacy);
                std::hint::black_box(&indexed);
            }
            for (label,old,new) in [
                ("forward", &mut old_forward, &mut indexed_forward),
                ("reverse", &mut old_reverse, &mut indexed_reverse),
            ] {
                old.sort_unstable();new.sort_unstable();
                let before=old[1].as_secs_f64()*1000.0;
                let after=new[1].as_secs_f64()*1000.0;
                println!("call-lookup kind={label} calls={count} old_ms={before:.3} indexed_ms={after:.3} speedup={:.2}x",before/after);
            }
            // No-call control: the indexed path should also handle ordinary
            // expression IDs whose call slot is empty.
            let empty=Graph {
                call_index_by_expression:vec![None;count*2],
                ..Default::default()
            };
            for i in 0..count*2 {
                assert!(empty.call_at(ExpressionId(i)).is_none());
            }
        }
    }

    #[test]
    fn rejects_initializer_operand_gaps() {
        let mut il = Il::default();
        il.symbols.push(Symbol {
            id: SymbolId(1), kind: 2, name: "x".into(),
        });
        for (path, parent) in [("0", None), ("0.1", Some("0"))] {
            il.initializer_expressions.push(InitializerExpression {
                owner: SymbolId(1), path: path.into(),
                parent: parent.map(str::to_owned), kind: 1,
                operator: None, type_id: None, symbol_id: None,
            });
        }
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidInitializerOperandPath { .. })));
    }

    #[test]
    fn rejects_non_call_expression_operand_gaps() {
        let mut il = Il::default();
        il.statements.push(Statement {
            function: "f".into(), path: "0".into(), parent: None, kind: 1,
        });
        for (path, parent) in [("0", None), ("0.1", Some("0"))] {
            il.expressions.push(Expression {
                function: "f".into(), statement_path: "0".into(),
                path: path.into(), parent: parent.map(str::to_owned),
                kind: 1, operator: None, type_kind: None, reference: None,
                type_size: None, type_alignment: None, constant_kind: None,
                constant_integer: None, related_type_role: None,
                related_type_kind: None, type_id: None, symbol_id: None,
            });
        }
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidExpressionOperandPath { .. })));
    }

    #[test]
    fn rejects_call_operand_gaps() {
        let mut il = Il::default();
        il.statements.push(Statement {
            function: "f".into(), path: "0".into(), parent: None, kind: 1,
        });
        for (path, parent) in [("0", None), ("0.0", Some("0")), ("0.2", Some("0"))] {
            il.expressions.push(Expression {
                function: "f".into(), statement_path: "0".into(),
                path: path.into(), parent: parent.map(str::to_owned),
                kind: if path=="0" {native_call_kinds(false).0} else {1},
                operator: (path=="0").then(||native_call_kinds(false).1),
                type_kind: None, reference: None,
                type_size: None, type_alignment: None, constant_kind: None,
                constant_integer: None, related_type_role: None,
                related_type_kind: None, type_id: None, symbol_id: None,
            });
        }
        il.calls.push(CallSite {
            function: "f".into(), statement_path: "0".into(),
            path: "0".into(), first_argument: 1,
        });
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidCallOperandPath { .. })));
    }

    #[test]
    fn rejects_truncated_call_operands() {
        let mut il = Il::default();
        il.statements.push(Statement {
            function: "f".into(), path: "0".into(), parent: None, kind: 1,
        });
        il.expressions.push(Expression {
            function: "f".into(), statement_path: "0".into(),
            path: "0".into(), parent: None,
            kind: native_call_kinds(false).0,
            operator: Some(native_call_kinds(false).1),
            type_kind: None, reference: None,
            type_size: None, type_alignment: None, constant_kind: None,
            constant_integer: None, related_type_role: None,
            related_type_kind: None, type_id: None, symbol_id: None,
        });
        il.calls.push(CallSite {
            function: "f".into(), statement_path: "0".into(),
            path: "0".into(), first_argument: 1,
        });
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidCallOperands { .. })));
    }


    fn call_snapshot(member:bool) -> Il {
        let mut il=Il::default();
        il.statements.push(Statement {
            function:"f".into(),path:"0".into(),parent:None,kind:1,
        });
        let (kind,op)=native_call_kinds(member);
        for (path,parent) in [
            ("0",None),("0.0",Some("0")),("0.1",Some("0")),
        ] {
            il.expressions.push(Expression {
                function:"f".into(),statement_path:"0".into(),
                path:path.into(),parent:parent.map(str::to_owned),
                kind:if path=="0" {kind} else {1},
                operator:if path=="0" {Some(op)} else {None},
                type_kind:None,reference:None,type_size:None,
                type_alignment:None,constant_kind:None,
                constant_integer:None,related_type_role:None,
                related_type_kind:None,type_id:None,symbol_id:None,
            });
        }
        il.calls.push(CallSite {
            function:"f".into(),statement_path:"0".into(),
            path:"0".into(),first_argument:if member {2} else {1},
        });
        il
    }

    #[test]
    fn audit_call_metadata_rejects_missing_event() {
        for member in [false,true] {
            let mut il=call_snapshot(member);
            il.calls.clear();
            assert!(matches!(Graph::from_il(&il),
                Err(GraphError::MissingCallMetadata {..})));
        }
    }

    #[test]
    fn audit_call_metadata_checks_operator_and_offset() {
        let mut il=call_snapshot(false);
        il.calls[0].first_argument=2; // two operands are present
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidCallOperands {..})));
        let mut il=call_snapshot(true);
        il.calls[0].first_argument=1;
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidCallOperands {..})));
        let mut il=call_snapshot(false);
        // A non-call operation with sufficient operands cannot borrow a
        // call event and silently change callee / argument interpretation.
        let add=(0..4096).find(|&op|
            unsafe {edgcpp_codegen_kind(3,op)}==1
        ).unwrap();
        il.expressions[0].operator=Some(add);
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidCallOperands {..})));
        let mut il=call_snapshot(false);
        let constant=(0..4096).find(|&kind|
            unsafe {edgcpp_codegen_kind(2,kind)}==1
        ).expect("native EDG constant expression kind");
        il.expressions[0].kind=constant; // a constant is not an operation
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidCallOperands {..})));
    }

    #[test]
    fn audit_call_metadata_allows_zero_argument_call() {
        let mut il=call_snapshot(false);
        il.expressions.pop(); // remove 0.1, retaining the callee at 0.0
        let graph=Graph::from_il(&il).expect("valid zero-argument call");
        let call=&graph.calls[0];
        assert_eq!(graph.call_arguments(call),Some(&[][..]));
        assert_eq!(graph.call_callee(call),Some(ExpressionId(1)));
        assert_eq!(graph.call_object(call),None);
    }

    #[test]
    fn rejects_non_descendant_parents() {
        let mut il = Il::default();
        il.statements.push(Statement { function:"f".into(), path:"0".into(), parent:Some("0".into()), kind:1 });
        assert!(matches!(Graph::from_il(&il), Err(GraphError::InvalidStatementParent { .. })));
    }
    #[test]
    fn rejects_missing_type_references() {
        let mut il = Il::default();
        il.types.push(TypeDef {
            id: TypeId(1), kind: (0..4096).find(|&kind|
                unsafe {edgcpp_codegen_kind(4,kind)}==5
            ).expect("native pointer kind"), related_role: Some(1),
            related: Some(TypeId(2)),
        });
        assert_eq!(Graph::from_il(&il).unwrap_err(), GraphError::MissingType(TypeId(2)));
        il.types.push(TypeDef {
            id: TypeId(2), kind: 2, related_role: None, related: None,
        });
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(graph.type_def(TypeId(1)).unwrap().related, Some(TypeId(2)));
    }

    #[test]
    fn rejects_non_routine_owners_for_function_metadata() {
        let routine=native_routine_type_kind();
        let nonroutine=(0..4096).find(|&kind|
            unsafe {edgcpp_codegen_kind(4,kind)} == 1
        ).expect("native EDG integer type kind");
        let mut il=Il::default();
        il.types.extend([
            TypeDef { id:TypeId(1), kind:nonroutine, related_role:None, related:None },
            TypeDef { id:TypeId(2), kind:nonroutine, related_role:None, related:None },
        ]);
        il.type_parameters.push(TypeParameter {
            owner:TypeId(1), position:0, ty:TypeId(2),
        });
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::InvalidTypeParameterOwner(TypeId(1)));
        il.type_parameters.clear();
        il.function_signatures.push(FunctionSignature {
            owner:TypeId(1), prototyped:true, variadic:false,
        });
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::InvalidFunctionSignatureOwner(TypeId(1)));
        il.types[0].kind=routine;
        Graph::from_il(&il).expect("routine signature metadata");
        il.type_parameters.push(TypeParameter {
            owner:TypeId(1), position:0, ty:TypeId(2),
        });
        Graph::from_il(&il).expect("routine parameter edges");
    }

    #[test]
    fn rejects_type_parameters_missing_native_function_signature() {
        let mut il=Il::default();
        il.types.extend([
            TypeDef { id:TypeId(1),kind:native_routine_type_kind(),
                related_role:None,related:None },
            TypeDef { id:TypeId(2),kind:2,
                related_role:None,related:None },
        ]);
        il.type_parameters.push(TypeParameter {
            owner:TypeId(1),position:0,ty:TypeId(2),
        });
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::MissingFunctionSignature(TypeId(1)));
        il.function_signatures.push(FunctionSignature {
            owner:TypeId(1),prototyped:true,variadic:false,
        });
        Graph::from_il(&il).expect("matching native signature and parameters");
    }

    #[test]
    fn validates_parameter_edges() {
        let mut il = Il::default();
        il.types.push(TypeDef { id: TypeId(1), kind: 0,
            related_role: None, related: None });
        il.type_parameters.push(TypeParameter {
            owner: TypeId(1), position: 0, ty: TypeId(2),
        });
        assert_eq!(Graph::from_il(&il).unwrap_err(), GraphError::MissingType(TypeId(2)));
        il.types.push(TypeDef { id: TypeId(2), kind: 0,
            related_role: None, related: None });
        il.types[0].kind = native_routine_type_kind();
        il.function_signatures.push(FunctionSignature {
            owner:TypeId(1),prototyped:true,variadic:false,
        });
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(graph.parameters(TypeId(1)).next().unwrap().ty, TypeId(2));
    }

    #[test]
    fn rejects_record_fields_attached_to_non_record_types() {
        let mut il=Il::default();
        il.symbols.push(Symbol {
            id:SymbolId(9),kind:3,name:"field".into(),
        });
        il.types.extend([
            TypeDef { id:TypeId(1),kind:native_type_kind_for_semantic(1),
                related_role:None,related:None },
            TypeDef { id:TypeId(2),kind:native_type_kind_for_semantic(1),
                related_role:None,related:None },
        ]);
        il.record_fields.push(RecordField {
            symbol:SymbolId(9),owner:TypeId(1),position:0,ty:TypeId(2),
            name:"field".into(),byte_offset:0,bit_offset:0,bit_width:0,
        });
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::InvalidRecordFieldOwner(TypeId(1)));
        for semantic in [7,8,9] {
            il.types[0].kind=native_type_kind_for_semantic(semantic);
            Graph::from_il(&il).expect("struct, class and union field owner");
        }
    }

    #[test]
    fn record_fields_are_linked_and_ordered() {
        let mut il = Il::default();
        il.types.push(TypeDef { id: TypeId(1), kind: native_type_kind_for_semantic(7), related_role: None, related: None });
        il.types.push(TypeDef { id: TypeId(2), kind: 0, related_role: None, related: None });
        il.symbols.extend([
            Symbol { id: SymbolId(10), kind: 3, name: "a".into() },
            Symbol { id: SymbolId(11), kind: 3, name: "b".into() },
        ]);
        il.record_fields.push(RecordField { symbol: SymbolId(11), owner: TypeId(1), position: 1, ty: TypeId(2), name: "b".into(), byte_offset: 4, bit_offset: 0, bit_width: 0 });
        il.record_fields.push(RecordField { symbol: SymbolId(10), owner: TypeId(1), position: 0, ty: TypeId(2), name: "a".into(), byte_offset: 0, bit_offset: 0, bit_width: 0 });
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(graph.fields(TypeId(1)).map(|f| f.name.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);

        il.record_fields[0].symbol = SymbolId(0);
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::InvalidRecordFieldSymbol { owner: TypeId(1), symbol: SymbolId(0) });
        il.record_fields[0].symbol = SymbolId(99);
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::InvalidRecordFieldSymbol { owner: TypeId(1), symbol: SymbolId(99) });
        il.record_fields[0].symbol = SymbolId(11);
        il.record_fields[1].symbol = SymbolId(11);
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::DuplicateRecordFieldSymbol {
                owner: TypeId(1), symbol: SymbolId(11),
            });
        il.record_fields[1].symbol = SymbolId(10);
        il.record_fields[0].bit_offset = 8;
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::InvalidRecordFieldBitOffset {
                owner: TypeId(1), position: 1,
            });
        il.record_fields[0].bit_offset = 0;
        il.symbols[1].kind = 2;
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::InvalidRecordFieldSymbol { owner: TypeId(1), symbol: SymbolId(11) });
        il.symbols[1].kind = 3;
        il.record_fields.remove(1); // missing position zero
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::InvalidRecordFieldPosition(TypeId(1)));
    }

    #[test]
    fn native_class_admission_coverage_rejects_missing_or_invalid_proofs() {
        let mut il=Il::default();
        il.types.extend([
            TypeDef{id:TypeId(1),kind:native_type_kind_for_semantic(7),
                related_role:None,related:None},
            TypeDef{id:TypeId(2),kind:native_type_kind_for_semantic(8),
                related_role:None,related:None},
            TypeDef{id:TypeId(3),kind:native_type_kind_for_semantic(9),
                related_role:None,related:None},
        ]);
        // Graph's ordinary constructor is intentionally permissive: the
        // completeness guarantee belongs to successful native callbacks.
        Graph::from_il(&il).expect("partial hand-built graph fixture");
        assert_eq!(Graph::validate_native_class_admission_coverage(&il),
            Err(GraphError::MissingNativeRecordAdmission(TypeId(1))));
        for id in [1,2] {
            il.cxx_record_admissions.push(crate::CxxRecordAdmission{
                owner:TypeId(id),plain_object:id==1,
            });
        }
        assert_eq!(Graph::validate_native_class_admission_coverage(&il),
            Err(GraphError::MissingNativeLifetimeAdmission(TypeId(1))));
        for id in [1,2] {
            il.cxx_scalar_lifetime_admissions.push(crate::CxxScalarLifetimeAdmission{
                owner:TypeId(id),candidate:id==2,
            });
        }
        Graph::validate_native_class_admission_coverage(&il)
            .expect("all struct and class records have both native proofs");
        il.cxx_record_admissions.push(il.cxx_record_admissions[0]);
        assert_eq!(Graph::validate_native_class_admission_coverage(&il),
            Err(GraphError::InvalidNativeRecordAdmission(TypeId(1))));
        il.cxx_record_admissions.pop();
        il.cxx_scalar_lifetime_admissions.push(il.cxx_scalar_lifetime_admissions[0]);
        assert_eq!(Graph::validate_native_class_admission_coverage(&il),
            Err(GraphError::InvalidNativeLifetimeAdmission(TypeId(1))));
        il.cxx_scalar_lifetime_admissions.pop();
        il.cxx_record_admissions[0].owner=TypeId(3);
        assert_eq!(Graph::validate_native_class_admission_coverage(&il),
            Err(GraphError::InvalidNativeRecordAdmission(TypeId(3))),
            "unions do not receive native class admission events");
        il.cxx_record_admissions[0].owner=TypeId(999);
        assert_eq!(Graph::validate_native_class_admission_coverage(&il),
            Err(GraphError::MissingType(TypeId(999))));
    }

    #[test]
    fn native_type_edge_coverage_rejects_truncated_pointer_array_typeref() {
        let mut il=Il::default();
        il.types.extend([
            TypeDef{id:TypeId(1),kind:native_type_kind_for_semantic(1),
                related_role:None,related:None},
            TypeDef{id:TypeId(2),kind:native_type_kind_for_semantic(5),
                related_role:Some(1),related:Some(TypeId(1))},
            TypeDef{id:TypeId(3),kind:native_type_kind_for_semantic(6),
                related_role:Some(2),related:Some(TypeId(1))},
            TypeDef{id:TypeId(4),kind:native_type_kind_for_semantic(3),
                related_role:Some(4),related:Some(TypeId(1))},
        ]);
        Graph::validate_native_type_edge_coverage(&il)
            .expect("all required native type edges exported");
        for index in [1usize,2,3] {
            let original=il.types[index].clone();
            il.types[index].related=None;
            il.types[index].related_role=None;
            // Partial fixtures remain useful for Graph visitors; only the
            // successful native callback is required to supply these edges.
            Graph::from_il(&il).expect("partial graph fixture still accepted");
            assert_eq!(Graph::validate_native_type_edge_coverage(&il),
                Err(GraphError::MissingNativeTypeRelation(original.id)));
            il.types[index]=original.clone();
            il.types[index].related_role=Some(4);
            if index!=3 {
                assert_eq!(Graph::validate_native_type_edge_coverage(&il),
                    Err(GraphError::MissingNativeTypeRelation(original.id)));
            }
            il.types[index]=original.clone();
            il.types[index].related=Some(TypeId(999));
            assert_eq!(Graph::validate_native_type_edge_coverage(&il),
                Err(GraphError::MissingNativeTypeRelation(original.id)));
            il.types[index]=original;
        }
        Graph::validate_native_type_edge_coverage(&il)
            .expect("complete edges remain valid after each mutation");
        // A routine without a return relation can represent a C++ special
        // member and is not proof of truncation in this narrow check.
        il.types.push(TypeDef{id:TypeId(5),kind:native_routine_type_kind(),
            related_role:None,related:None});
        Graph::validate_native_type_edge_coverage(&il)
            .expect("routine return relationship is not assumed mandatory without mode proof");
        il.cpp_language=Some(true);
        Graph::validate_native_type_edge_coverage(&il)
            .expect("C++ special member may lack normal return relation");
        il.cpp_language=Some(false);
        assert_eq!(Graph::validate_native_type_edge_coverage(&il),
            Err(GraphError::MissingNativeTypeRelation(TypeId(5))));
        il.types[4].related_role=Some(3);
        il.types[4].related=Some(TypeId(1));
        Graph::validate_native_type_edge_coverage(&il)
            .expect("C function return type relation is complete");
        il.types[4].related=Some(TypeId(999));
        assert_eq!(Graph::validate_native_type_edge_coverage(&il),
            Err(GraphError::MissingNativeTypeRelation(TypeId(5))));
    }

    #[test]
    fn native_detail_coverage_rejects_missing_pointer_or_typeref_event() {
        let mut il=Il::default();
        il.types.extend([
            TypeDef{id:TypeId(1),kind:native_type_kind_for_semantic(1),
                related_role:None,related:None},
            TypeDef{id:TypeId(2),kind:native_type_kind_for_semantic(5),
                related_role:Some(1),related:Some(TypeId(1))},
            TypeDef{id:TypeId(3),kind:native_type_kind_for_semantic(3),
                related_role:Some(4),related:Some(TypeId(1))},
        ]);
        // Incomplete synthetic test graphs may still be constructed. A real
        // successful native callback must not drop either required event 11.
        Graph::from_il(&il).expect("partial hand-built graph fixture");
        assert_eq!(Graph::validate_native_type_detail_coverage(&il),
            Err(GraphError::MissingTypeDetail(TypeId(2))));
        il.type_details.push(TypeDetail {
            owner:TypeId(2),is_reference:false,
            is_rvalue_reference:false,qualifiers:None,
        });
        assert_eq!(Graph::validate_native_type_detail_coverage(&il),
            Err(GraphError::MissingTypeDetail(TypeId(3))));
        il.type_details.push(TypeDetail {
            owner:TypeId(3),is_reference:false,
            is_rvalue_reference:false,qualifiers:Some(0),
        });
        Graph::validate_native_type_detail_coverage(&il)
            .expect("native pointer and plain typedef details are complete");
        Graph::from_il(&il).expect("complete graph type details");
        il.type_details[1].qualifiers=Some(1);
        Graph::validate_native_type_detail_coverage(&il)
            .expect("qualified typedef also supplies event 11");
        il.type_details.push(il.type_details[1].clone());
        assert_eq!(Graph::validate_native_type_detail_coverage(&il),
            Err(GraphError::DuplicateTypeDetail(TypeId(3))));
        il.type_details.pop();
        il.type_details[0].is_rvalue_reference=true;
        assert_eq!(Graph::validate_native_type_detail_coverage(&il),
            Err(GraphError::InvalidTypeDetailFlags(TypeId(2))));
        il.type_details[0].is_reference=true;
        Graph::validate_native_type_detail_coverage(&il)
            .expect("well-formed native rvalue reference includes reference bit");
        il.type_details[0].qualifiers=Some(0);
        assert_eq!(Graph::validate_native_type_detail_coverage(&il),
            Err(GraphError::InvalidTypeDetailFlags(TypeId(2))));
        il.type_details[0].qualifiers=None;
        il.type_details[1].is_reference=true;
        assert_eq!(Graph::validate_native_type_detail_coverage(&il),
            Err(GraphError::InvalidTypeDetailFlags(TypeId(3))));
        il.type_details[1].is_reference=false;
        il.type_details[0].is_reference=false;
        il.type_details[0].is_rvalue_reference=false;
        il.type_details[0].owner=TypeId(1);
        assert_eq!(Graph::validate_native_type_detail_coverage(&il),
            Err(GraphError::InvalidTypeDetailOwner(TypeId(1))));
    }

    #[test]
    fn resolves_reference_details() {
        let mut il = Il::default();
        il.types.push(TypeDef {
            id: TypeId(1), kind: native_type_kind_for_semantic(5),
            related_role: None, related: None,
        });
        il.type_details.push(TypeDetail { owner: TypeId(1), is_reference: true,
            is_rvalue_reference: false, qualifiers: None });
        let graph = Graph::from_il(&il).unwrap();
        assert!(graph.type_detail(TypeId(1)).unwrap().is_reference);
    }

    #[test]
    fn audit_native_zero_parameter_routine_supplement_proof() {
        let mut il=Il::default();
        il.types.push(TypeDef {id:TypeId(1),kind:native_routine_type_kind(),
            related_role:None,related:None});
        // A no-parameter function has no event 8 edges; this was the
        // ambiguous case before the independent native event 71.
        il.function_signatures.push(FunctionSignature {
            owner:TypeId(1),prototyped:true,variadic:false,
        });
        Graph::from_il(&il).expect("partial synthetic graph without event 71");
        assert_eq!(Graph::validate_native_routine_signature_coverage(&il),
            Err(GraphError::MissingRoutineSupplement(TypeId(1))));
        il.routine_supplements.push(crate::NativeRoutineSupplement {
            owner:TypeId(1),has_extra_info:true,parameter_count:0,
        });
        Graph::validate_native_routine_signature_coverage(&il)
            .expect("zero-parameter prototype with EDG extra_info proof");
        Graph::from_il(&il).expect("native provenance agrees with signature");
        il.function_signatures.clear();
        assert_eq!(Graph::validate_native_routine_signature_coverage(&il),
            Err(GraphError::MissingFunctionSignature(TypeId(1))));
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::MissingFunctionSignature(TypeId(1)));
        il.routine_supplements[0].has_extra_info=false;
        Graph::validate_native_routine_signature_coverage(&il)
            .expect("a genuinely null extra_info need not have event 12");
        Graph::from_il(&il).expect("matching no-supplement proof");
        il.function_signatures.push(FunctionSignature {
            owner:TypeId(1),prototyped:true,variadic:false,
        });
        assert_eq!(Graph::validate_native_routine_signature_coverage(&il),
            Err(GraphError::InvalidRoutineSupplement(TypeId(1))));
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::InvalidRoutineSupplement(TypeId(1)));
        il.function_signatures.clear();
        il.routine_supplements[0].has_extra_info=true;
        il.function_signatures.push(FunctionSignature {
            owner:TypeId(1),prototyped:true,variadic:false,
        });
        il.routine_supplements.push(il.routine_supplements[0].clone());
        assert_eq!(Graph::validate_native_routine_signature_coverage(&il),
            Err(GraphError::DuplicateRoutineSupplement(TypeId(1))));
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::DuplicateRoutineSupplement(TypeId(1)));
        il.routine_supplements.pop();
        il.types.push(TypeDef {id:TypeId(2),
            kind:native_type_kind_for_semantic(1),
            related_role:None,related:None});
        il.routine_supplements[0].owner=TypeId(2);
        assert_eq!(Graph::validate_native_routine_signature_coverage(&il),
            Err(GraphError::InvalidRoutineSupplement(TypeId(2))));
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::InvalidRoutineSupplement(TypeId(2)));
        il.routine_supplements[0].owner=TypeId(999);
        assert_eq!(Graph::validate_native_routine_signature_coverage(&il),
            Err(GraphError::MissingType(TypeId(999))));
        il.routine_supplements[0].owner=TypeId(1);
        il.type_parameters.push(TypeParameter {
            owner:TypeId(1),position:0,ty:TypeId(2),
        });
        il.routine_supplements[0].parameter_count=1;
        Graph::validate_native_routine_signature_coverage(&il)
            .expect("parameter edges plus signature and proof");
        il.function_signatures.clear();
        assert_eq!(Graph::validate_native_routine_signature_coverage(&il),
            Err(GraphError::MissingFunctionSignature(TypeId(1))));
    }

    #[test]
    fn audit_native_scalar_metadata_proves_width_sign_and_standard_fp_presence() {
        let mut il=Il::default();
        for (id,semantic) in [(1,1),(2,1),(3,0),(4,5)] {
            il.types.push(TypeDef {
                id:TypeId(id),kind:if semantic==0 {u32::MAX} else {
                    native_type_kind_for_semantic(semantic)
                },related_role:None,related:None,
            });
        }
        il.native_scalar_proofs.extend([
            crate::NativeScalarMetadataProof{owner:TypeId(1),integer_abi:true,
                standard_float_abi:false,signed_i64:true},
            crate::NativeScalarMetadataProof{owner:TypeId(2),integer_abi:true,
                standard_float_abi:false,signed_i64:false},
            crate::NativeScalarMetadataProof{owner:TypeId(3),integer_abi:false,
                standard_float_abi:true,signed_i64:false},
            crate::NativeScalarMetadataProof{owner:TypeId(4),integer_abi:false,
                standard_float_abi:false,signed_i64:false},
        ]);
        il.c_integer_types.extend([
            crate::CIntegerType{id:TypeId(1),bits:64,signed:true,is_bool:false},
            crate::CIntegerType{id:TypeId(2),bits:32,signed:false,is_bool:false},
        ]);
        il.c_float_types.push(crate::CFloatType{id:TypeId(3),bits:32});
        il.signed_i64_types.push(TypeId(1));
        il.type_object_sizes.extend([
            crate::TypeObjectSize{owner:TypeId(1),bytes:8,alignment:8},
            crate::TypeObjectSize{owner:TypeId(2),bytes:4,alignment:4},
            crate::TypeObjectSize{owner:TypeId(3),bytes:4,alignment:4},
        ]);
        Graph::validate_native_scalar_metadata_coverage(&il)
            .expect("native signed 64, unsigned 32 and standard float metadata");
        let mut lost_signed=il.clone();
        lost_signed.signed_i64_types.clear();
        assert_eq!(Graph::validate_native_scalar_metadata_coverage(&lost_signed),
            Err(GraphError::InvalidNativeScalarMetadataProof(TypeId(1))));
        for index in 0..2 {
            let mut lost=il.clone();
            let id=lost.c_integer_types.remove(index).id;
            assert_eq!(Graph::validate_native_scalar_metadata_coverage(&lost),
                Err(GraphError::InvalidNativeScalarMetadataProof(id)));
        }
        let mut lost_float=il.clone();
        lost_float.c_float_types.clear();
        assert_eq!(Graph::validate_native_scalar_metadata_coverage(&lost_float),
            Err(GraphError::InvalidNativeScalarMetadataProof(TypeId(3))));
        let mut invalid_width=il.clone();
        invalid_width.c_integer_types[0].bits=32;
        assert_eq!(Graph::validate_native_scalar_metadata_coverage(&invalid_width),
            Err(GraphError::InvalidNativeScalarMetadataProof(TypeId(1))));
        let mut invalid_fp_width=il.clone();
        invalid_fp_width.c_float_types[0].bits=64;
        assert_eq!(Graph::validate_native_scalar_metadata_coverage(&invalid_fp_width),
            Err(GraphError::InvalidNativeScalarMetadataProof(TypeId(3))));
        let mut duplicate=il.clone();
        duplicate.native_scalar_proofs.push(duplicate.native_scalar_proofs[0]);
        assert_eq!(Graph::validate_native_scalar_metadata_coverage(&duplicate),
            Err(GraphError::DuplicateNativeScalarMetadataProof(TypeId(1))));
        let mut missing_proof=il.clone();
        missing_proof.native_scalar_proofs.pop();
        assert_eq!(Graph::validate_native_scalar_metadata_coverage(&missing_proof),
            Err(GraphError::MissingNativeScalarMetadataProof(TypeId(4))));
        let mut invalid_category=il.clone();
        invalid_category.native_scalar_proofs[3].integer_abi=true;
        assert_eq!(Graph::validate_native_scalar_metadata_coverage(&invalid_category),
            Err(GraphError::InvalidNativeScalarMetadataProof(TypeId(4))));
        let mut invalid_ambiguous=il.clone();
        invalid_ambiguous.native_scalar_proofs[0].standard_float_abi=true;
        assert_eq!(Graph::validate_native_scalar_metadata_coverage(&invalid_ambiguous),
            Err(GraphError::InvalidNativeScalarMetadataProof(TypeId(1))));
        let partial=Il { types:il.types, ..Il::default() };
        Graph::from_il(&partial).expect("partial synthetic graphs need no native proof");
    }

    #[test]
    fn audit_native_layout_proof_rejects_missing_bounds_and_object_layouts() {
        let mut il=Il::default();
        for (id,kind) in [(1,6),(2,7),(3,5)] {
            il.types.push(TypeDef{id:TypeId(id),
                kind:native_type_kind_for_semantic(kind),
                related_role:None,related:None});
        }
        il.native_type_layout_proofs.extend([
            crate::NativeTypeLayoutProof{owner:TypeId(1),
                has_layout:true,has_fixed_array_bound:true},
            crate::NativeTypeLayoutProof{owner:TypeId(2),
                has_layout:true,has_fixed_array_bound:false},
            crate::NativeTypeLayoutProof{owner:TypeId(3),
                has_layout:false,has_fixed_array_bound:false},
        ]);
        il.type_object_sizes.extend([
            crate::TypeObjectSize{owner:TypeId(1),bytes:16,alignment:4},
            crate::TypeObjectSize{owner:TypeId(2),bytes:8,alignment:4},
        ]);
        il.array_bounds.push(crate::ArrayBound{array:TypeId(1),elements:4});
        Graph::validate_native_type_layout_coverage(&il)
            .expect("complete type, fixed array, incomplete type");
        let mut lost_fixed_bound=il.clone();
        lost_fixed_bound.array_bounds.clear();
        assert_eq!(Graph::validate_native_type_layout_coverage(&lost_fixed_bound),
            Err(GraphError::InvalidNativeTypeLayoutProof(TypeId(1))));
        for lost_index in 0..2 {
            let mut truncated=il.clone();
            truncated.type_object_sizes.remove(lost_index);
            assert_eq!(Graph::validate_native_type_layout_coverage(&truncated),
                Err(GraphError::InvalidNativeTypeLayoutProof(TypeId(lost_index as u32+1))));
        }
        let mut missing_proof=il.clone();
        missing_proof.native_type_layout_proofs.remove(2);
        assert_eq!(Graph::validate_native_type_layout_coverage(&missing_proof),
            Err(GraphError::MissingNativeTypeLayoutProof(TypeId(3))));
        let mut duplicate=il.clone();
        duplicate.native_type_layout_proofs.push(duplicate.native_type_layout_proofs[0]);
        assert_eq!(Graph::validate_native_type_layout_coverage(&duplicate),
            Err(GraphError::DuplicateNativeTypeLayoutProof(TypeId(1))));
        let mut wrong_kind=il.clone();
        wrong_kind.native_type_layout_proofs[1].has_fixed_array_bound=true;
        assert_eq!(Graph::validate_native_type_layout_coverage(&wrong_kind),
            Err(GraphError::InvalidNativeTypeLayoutProof(TypeId(2))));
        let mut unexpected_bound=il.clone();
        unexpected_bound.array_bounds.push(crate::ArrayBound{
            array:TypeId(2),elements:9});
        assert_eq!(Graph::validate_native_type_layout_coverage(&unexpected_bound),
            Err(GraphError::InvalidNativeTypeLayoutProof(TypeId(2))));
        let mut unexpected_layout=il.clone();
        unexpected_layout.type_object_sizes.push(crate::TypeObjectSize{
            owner:TypeId(3),bytes:8,alignment:8});
        assert_eq!(Graph::validate_native_type_layout_coverage(&unexpected_layout),
            Err(GraphError::InvalidNativeTypeLayoutProof(TypeId(3))));
        let mut incomplete=Il::default();
        incomplete.types.push(TypeDef{id:TypeId(3),
            kind:native_type_kind_for_semantic(5),
            related_role:None,related:None});
        Graph::from_il(&incomplete).expect("partial synthetic IL remains supported");
        incomplete.native_type_layout_proofs.push(crate::NativeTypeLayoutProof{
            owner:TypeId(3),has_layout:false,has_fixed_array_bound:false});
        Graph::validate_native_type_layout_coverage(&incomplete)
            .expect("incomplete native type may lack object layout");
    }

    #[test]
    fn audit_native_record_shape_proves_all_fields_and_direct_base_metadata() {
        let mut il=Il::default();
        for (id,semantic) in [(1,7),(2,8),(3,9),(4,1)] {
            il.types.push(TypeDef{id:TypeId(id),kind:native_type_kind_for_semantic(semantic),
                related_role:None,related:None});
        }
        il.native_record_shapes.extend([
            crate::NativeRecordShape{owner:TypeId(1),field_count:3,direct_base_count:0},
            crate::NativeRecordShape{owner:TypeId(2),field_count:1,direct_base_count:1},
            crate::NativeRecordShape{owner:TypeId(3),field_count:2,direct_base_count:0},
        ]);
        for (owner,count) in [(1,3),(2,1),(3,2)] {
            for position in 0..count {
                il.record_fields.push(RecordField {
                    owner:TypeId(owner),position,ty:TypeId(4),
                    symbol:SymbolId(100+owner*10+position),
                    name:format!("field_{position}"),byte_offset:0,
                    bit_offset:0,bit_width:0,
                });
            }
        }
        il.base_classes.push(BaseClass{derived:TypeId(2),position:0,
            base:TypeId(1),is_virtual:false});
        il.base_class_access.push(BaseClassAccess{derived:TypeId(2),
            position:0,access:Some(0),pack_expansion:false});
        il.base_subobject_offsets.push(crate::BaseSubobjectOffset{
            derived:TypeId(2),position:0,byte_offset:None});
        Graph::validate_native_record_shape_coverage(&il)
            .expect("complete native struct/class/union shape metadata");
        for index in 0..3 {
            let mut truncated=il.clone();
            truncated.record_fields.remove(index);
            assert_eq!(Graph::validate_native_record_shape_coverage(&truncated),
                Err(GraphError::InvalidNativeRecordShape(TypeId(1))));
        }
        let mut lost_union_tail=il.clone();
        lost_union_tail.record_fields.pop();
        assert_eq!(Graph::validate_native_record_shape_coverage(&lost_union_tail),
            Err(GraphError::InvalidNativeRecordShape(TypeId(3))));
        for omitted in 0..3 {
            let mut truncated=il.clone();
            match omitted {
                0=>{truncated.base_classes.clear();},
                1=>{truncated.base_class_access.clear();},
                _=>{truncated.base_subobject_offsets.clear();},
            }
            assert_eq!(Graph::validate_native_record_shape_coverage(&truncated),
                Err(GraphError::InvalidNativeRecordShape(TypeId(2))));
        }
        let mut missing=il.clone();
        missing.native_record_shapes.remove(2);
        assert_eq!(Graph::validate_native_record_shape_coverage(&missing),
            Err(GraphError::MissingNativeRecordShape(TypeId(3))));
        let mut duplicate=il.clone();
        duplicate.native_record_shapes.push(duplicate.native_record_shapes[0]);
        assert_eq!(Graph::validate_native_record_shape_coverage(&duplicate),
            Err(GraphError::DuplicateNativeRecordShape(TypeId(1))));
        let mut union_base=il.clone();
        union_base.native_record_shapes[2].direct_base_count=1;
        assert_eq!(Graph::validate_native_record_shape_coverage(&union_base),
            Err(GraphError::InvalidNativeRecordShape(TypeId(3))));
        let mut zero=Il::default();
        zero.types.push(TypeDef{id:TypeId(1),
            kind:native_type_kind_for_semantic(7),related_role:None,related:None});
        assert_eq!(Graph::validate_native_record_shape_coverage(&zero),
            Err(GraphError::MissingNativeRecordShape(TypeId(1))));
        Graph::from_il(&zero).expect("partial synthetic graph remains supported");
        zero.native_record_shapes.push(crate::NativeRecordShape{
            owner:TypeId(1),field_count:0,direct_base_count:0});
        Graph::validate_native_record_shape_coverage(&zero)
            .expect("empty record with explicit native shape");
    }

    #[test]
    fn audit_native_parameter_count_rejects_dropped_tail_and_other_edges() {
        let mut il=Il::default();
        il.types.push(TypeDef {id:TypeId(1),kind:native_routine_type_kind(),
            related_role:None,related:None});
        il.types.push(TypeDef {id:TypeId(2),kind:native_type_kind_for_semantic(1),
            related_role:None,related:None});
        il.function_signatures.push(FunctionSignature {
            owner:TypeId(1),prototyped:true,variadic:false,
        });
        il.routine_supplements.push(crate::NativeRoutineSupplement {
            owner:TypeId(1),has_extra_info:true,parameter_count:3,
        });
        for position in 0..3 {
            il.type_parameters.push(TypeParameter {
                owner:TypeId(1),position,ty:TypeId(2),
            });
        }
        Graph::validate_native_routine_signature_coverage(&il)
            .expect("complete three-parameter proof");
        Graph::from_il(&il).expect("complete three-parameter graph");
        for missing in 0..3 {
            let mut truncated=il.clone();
            truncated.type_parameters.remove(missing);
            assert_eq!(Graph::validate_native_routine_signature_coverage(&truncated),
                Err(GraphError::InvalidRoutineParameterCount(TypeId(1))));
            assert!(Graph::from_il(&truncated).is_err(),
                "dropped parameter position {missing} must fail closed");
        }
        let mut repeated=il.clone();
        repeated.type_parameters[2].position=1;
        assert_eq!(Graph::validate_native_routine_signature_coverage(&repeated),
            Err(GraphError::InvalidTypeParameterPosition(TypeId(1))));

        let mut old_style=il.clone();
        old_style.function_signatures[0].prototyped=false;
        old_style.function_signatures[0].variadic=false;
        Graph::validate_native_routine_signature_coverage(&old_style)
            .expect("old-style supplement with preserved parameter list");

        let mut variadic=il.clone();
        variadic.function_signatures[0].variadic=true;
        Graph::validate_native_routine_signature_coverage(&variadic)
            .expect("variadic parameter list includes only declared fixed parameters");

        let mut empty=il.clone();
        empty.type_parameters.clear();
        empty.routine_supplements[0].parameter_count=0;
        Graph::validate_native_routine_signature_coverage(&empty)
            .expect("zero-parameter prototype");
        empty.routine_supplements[0].has_extra_info=false;
        empty.function_signatures.clear();
        Graph::validate_native_routine_signature_coverage(&empty)
            .expect("native null supplement");
        empty.routine_supplements[0].parameter_count=1;
        assert_eq!(Graph::validate_native_routine_signature_coverage(&empty),
            Err(GraphError::InvalidRoutineParameterCount(TypeId(1))));
    }

    #[test]
    fn routine_signature_metadata() {
        let mut il = Il::default();
        il.types.push(TypeDef { id: TypeId(1), kind: native_routine_type_kind(), related_role: None, related: None });
        il.function_signatures.push(FunctionSignature { owner: TypeId(1),
            prototyped: true, variadic: true });
        assert!(Graph::from_il(&il).unwrap().signature(TypeId(1)).unwrap().variadic);
    }

    #[test]
    fn rejects_native_type_shape_metadata_on_wrong_kind() {
        let mut il=Il::default();
        il.types.push(TypeDef {
            id:TypeId(1), kind:native_type_kind_for_semantic(1),
            related_role:None, related:None,
        });
        il.array_bounds.push(ArrayBound {array:TypeId(1),elements:4});
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::InvalidArrayBoundOwner(TypeId(1)));
        il.array_bounds.clear();
        il.type_details.push(TypeDetail {
            owner:TypeId(1), is_reference:false,
            is_rvalue_reference:false, qualifiers:None,
        });
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::InvalidTypeDetailOwner(TypeId(1)));
        il.types[0].kind=native_type_kind_for_semantic(3);
        il.type_details[0].qualifiers=Some(0);
        Graph::from_il(&il).expect("typeref detail");
        il.types[0].kind=native_type_kind_for_semantic(5);
        il.type_details[0].qualifiers=None;
        Graph::from_il(&il).expect("pointer detail");
    }

    #[test]
    fn rejects_impossible_native_type_detail_flag_combinations() {
        let mut il=Il::default();
        il.types.push(TypeDef {
            id:TypeId(1),kind:native_type_kind_for_semantic(5),
            related_role:None,related:None,
        });
        il.type_details.push(TypeDetail {
            owner:TypeId(1),is_reference:true,
            is_rvalue_reference:false,qualifiers:Some(0),
        });
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::InvalidTypeDetailFlags(TypeId(1)));
        il.type_details[0].qualifiers=None;
        Graph::from_il(&il).expect("native pointer reference detail");
        il.type_details[0].is_reference=false;
        il.type_details[0].is_rvalue_reference=true;
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::InvalidTypeDetailFlags(TypeId(1)));
        il.type_details[0].is_reference=true;
        Graph::from_il(&il).expect("rvalue reference retains its enclosing reference bit");
        il.type_details[0].is_rvalue_reference=false;
        il.types[0].kind=native_type_kind_for_semantic(3);
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::InvalidTypeDetailFlags(TypeId(1)));
        il.type_details[0].is_reference=false;
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::InvalidTypeDetailFlags(TypeId(1)));
        il.type_details[0].qualifiers=Some(0);
        Graph::from_il(&il).expect("native typeref qualifiers");
        il.type_details[0].is_rvalue_reference=true;
        assert_eq!(Graph::from_il(&il).unwrap_err(),
            GraphError::InvalidTypeDetailFlags(TypeId(1)));
    }

    #[test]
    fn fixed_array_bound_is_owned() {
        let mut il = Il::default();
        il.types.push(TypeDef {
            id: TypeId(1), kind: native_type_kind_for_semantic(6),
            related_role: None, related: None,
        });
        il.array_bounds.push(ArrayBound { array: TypeId(1), elements: 12 });
        assert_eq!(Graph::from_il(&il).unwrap().array_bound(TypeId(1)), Some(12));
    }

    #[test]
    fn audit_type_relationship_roles_match_native_owner_kind() {
        let native=|semantic| (0..4096).find(|&kind|
            unsafe {edgcpp_codegen_kind(4,kind)}==semantic
        ).expect("pinned native type kind");
        for (semantic, role) in [(5,1), (6,2), (2,3), (3,4)] {
            let mut il=Il::default();
            il.types.push(TypeDef {
                id:TypeId(1), kind:native(semantic), related_role:Some(role),
                related:Some(TypeId(2)),
            });
            il.types.push(TypeDef {
                id:TypeId(2), kind:native(1), related_role:None,related:None,
            });
            Graph::from_il(&il).expect("matching EDG native relationship");
            il.types[0].related_role=Some((role%4)+1);
            assert_eq!(Graph::from_il(&il).unwrap_err(),
                GraphError::InvalidTypeRelation(TypeId(1)));
            il.types[0].kind=native(1);
            assert_eq!(Graph::from_il(&il).unwrap_err(),
                GraphError::InvalidTypeRelation(TypeId(1)));
        }
    }

    #[test]
    fn rejects_inconsistent_type_edge() {
        let mut il = Il::default();
        il.types.push(TypeDef {
            id: TypeId(1), kind: 7, related_role: Some(1), related: None,
        });
        assert_eq!(Graph::from_il(&il).unwrap_err(), GraphError::InvalidTypeRelation(TypeId(1)));
    }

    #[test]
    fn symbol_identity_is_validated() {
        let mut il = Il::default();
        il.symbols.push(Symbol { id: SymbolId(1), kind: 1, name: "f(int)".into() });
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(graph.symbol(SymbolId(1)).unwrap().name, "f(int)");
        il.symbols.push(il.symbols[0].clone());
        assert_eq!(Graph::from_il(&il).unwrap_err(), GraphError::DuplicateSymbol(SymbolId(1)));
    }

    #[test]
    fn rejects_dangling_expression_symbol() {
        let mut il = Il::default();
        il.statements.push(Statement {
            function: "f".into(), path: "0".into(), parent: None, kind: 8,
        });
        il.expressions.push(Expression {
            function: "f".into(), statement_path: "0".into(),
            path: "0".into(), parent: None, kind: 3, operator: None,
            type_kind: None, reference: Some("x".into()),
            type_size: None, type_alignment: None, constant_kind: None,
            constant_integer: None, related_type_role: None,
            related_type_kind: None, type_id: None, symbol_id: Some(SymbolId(7)),
        });
        assert_eq!(Graph::from_il(&il).unwrap_err(),
                   GraphError::MissingSymbol(SymbolId(7)));
        il.symbols.push(Symbol { id: SymbolId(7), kind: 2, name: "x".into() });
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(graph.symbol(SymbolId(7)).unwrap().name, "x");
    }

    #[test]
    fn rejects_wrong_symbol_kinds_for_native_initializer_owners() {
        // Only a variable (symbol kind 2) may own EDG initializer
        // metadata. Existing routine and field symbols are not valid.
        for wrong_kind in [1, 3, 4] {
            let mut il = Il::default();
            il.symbols.push(Symbol {
                id: SymbolId(7), kind: wrong_kind, name: "not_a_variable".into(),
            });
            il.variable_initializers.push(VariableInitializer {
                symbol: SymbolId(7), kind: 0,
                explicit: true, uninitialized: false,
            });
            assert_eq!(Graph::from_il(&il).unwrap_err(),
                GraphError::InvalidInitializerSymbol(SymbolId(7)));
            il.variable_initializers.clear();
            il.initializer_integers.push(InitializerInteger {
                symbol: SymbolId(7), value: 1,
            });
            assert_eq!(Graph::from_il(&il).unwrap_err(),
                GraphError::InvalidInitializerSymbol(SymbolId(7)));
            il.initializer_integers.clear();
            il.dynamic_initializers.push(DynamicInitializer {
                symbol: SymbolId(7), kind: 3,
                expression_kind: None, operator: None,
            });
            assert_eq!(Graph::from_il(&il).unwrap_err(),
                GraphError::InvalidInitializerSymbol(SymbolId(7)));
            il.dynamic_initializers.clear();
            il.initializer_expressions.push(InitializerExpression {
                owner: SymbolId(7), path: "0".into(),
                parent: None, kind: 1, operator: None,
                type_id: None, symbol_id: None,
            });
            assert_eq!(Graph::from_il(&il).unwrap_err(),
                GraphError::InvalidInitializerSymbol(SymbolId(7)));
        }
    }

    #[test]
    fn validates_variable_initializers() {
        let mut il = Il::default();
        il.variable_initializers.push(VariableInitializer {
            symbol: SymbolId(5), kind: 0, explicit: true, uninitialized: false,
        });
        assert_eq!(Graph::from_il(&il).unwrap_err(), GraphError::MissingSymbol(SymbolId(5)));
        il.symbols.push(Symbol { id: SymbolId(5), kind: 2, name: "x".into() });
        let graph = Graph::from_il(&il).unwrap();
        assert!(graph.variable_initializer(SymbolId(5)).unwrap().explicit);
    }

    #[test]
    fn validates_integer_initializer_values() {
        let mut il = Il::default();
        il.initializer_integers.push(InitializerInteger {
            symbol: SymbolId(3), value: -17,
        });
        assert_eq!(Graph::from_il(&il).unwrap_err(), GraphError::MissingSymbol(SymbolId(3)));
        il.symbols.push(Symbol { id: SymbolId(3), kind: 2, name: "x".into() });
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(graph.initializer_integer(SymbolId(3)), Some(-17));
        il.initializer_integers.push(il.initializer_integers[0].clone());
        assert_eq!(Graph::from_il(&il).unwrap_err(),
                   GraphError::DuplicateInitializerInteger(SymbolId(3)));
    }

    #[test]
    fn validates_dynamic_initializers() {
        let mut il = Il::default();
        il.dynamic_initializers.push(DynamicInitializer {
            symbol: SymbolId(4), kind: 3, expression_kind: Some(1), operator: Some(40),
        });
        assert_eq!(Graph::from_il(&il).unwrap_err(), GraphError::MissingSymbol(SymbolId(4)));
        il.symbols.push(Symbol { id: SymbolId(4), kind: 2, name: "x".into() });
        assert_eq!(Graph::from_il(&il).unwrap().dynamic_initializer(SymbolId(4)).unwrap().kind, 3);
        il.dynamic_initializers.push(il.dynamic_initializers[0].clone());
        assert_eq!(Graph::from_il(&il).unwrap_err(), GraphError::DuplicateDynamicInitializer(SymbolId(4)));
    }

    #[test]
    fn validates_initializer_expression_tree() {
        let mut il = Il::default();
        il.symbols.push(Symbol { id: SymbolId(1), kind: 2, name: "x".into() });
        il.initializer_expressions.push(InitializerExpression {
            owner: SymbolId(1), path: "0".into(), parent: None,
            kind: 1, operator: Some(4), type_id: None, symbol_id: None,
        });
        il.initializer_expressions.push(InitializerExpression {
            owner: SymbolId(1), path: "0.0".into(), parent: Some("0".into()),
            kind: 3, operator: None, type_id: None, symbol_id: Some(SymbolId(1)),
        });
        assert_eq!(Graph::from_il(&il).unwrap().initializer_expressions(SymbolId(1)).count(), 2);
        il.initializer_expressions[1].parent = Some("missing".into());
        assert!(matches!(Graph::from_il(&il),
            Err(GraphError::InvalidInitializerParent { .. })));
    }

    #[test]
    fn rejects_duplicate_type_ids() {
        let mut il = Il::default();
        let ty = TypeDef { id: TypeId(1), kind: 2, related_role: None, related: None };
        il.types.extend([ty.clone(), ty]);
        assert_eq!(Graph::from_il(&il).unwrap_err(), GraphError::DuplicateType(TypeId(1)));
    }

    #[test]
    fn rejects_skipped_statement_level() {
        let mut il = Il::default();
        il.statements.push(Statement {
            function: "f".into(), path: "0".into(), parent: None, kind: 11,
        });
        il.statements.push(Statement {
            function: "f".into(), path: "0.block.0.block.0".into(),
            parent: Some("0".into()), kind: 8,
        });
        assert!(matches!(Graph::from_il(&il),
                         Err(GraphError::InvalidStatementParent { .. })));
    }

    #[test]
    fn rejects_dangling_statement() {
        let mut il = Il::default();
        il.statements.push(Statement { function:"f".into(), path:"0".into(), parent:Some("missing".into()), kind:1 });
        assert!(matches!(Graph::from_il(&il), Err(GraphError::MissingStatement { .. })));
    }
}

#[cfg(test)]
mod audit_expression_root_tests {
    use super::*;

    fn graph_with_root(path: &str) -> Result<Graph, GraphError> {
        let mut il=Il::default();
        il.statements.push(Statement {
            function:"f".into(),path:"0".into(),parent:None,kind:1,
        });
        il.expressions.push(Expression {
            function:"f".into(),statement_path:"0".into(),
            path:path.into(),parent:None,kind:1,
            operator:None,type_kind:None,reference:None,
            type_size:None,type_alignment:None,constant_kind:None,
            constant_integer:Some(3),related_type_role:None,
            related_type_kind:None,type_id:None,symbol_id:None,
        });
        Graph::from_il(&il)
    }

    #[test]
    fn only_canonical_statement_expression_root_is_accepted() {
        assert!(graph_with_root("0").is_ok());
        for invalid in ["1","0.0","00",""] {
            assert!(matches!(
                graph_with_root(invalid),
                Err(GraphError::InvalidExpressionParent { .. })
            ), "noncanonical orphan expression root {invalid:?}");
        }
    }
}

#[cfg(test)]
mod audit_statement_source_order_tests {
    use super::*;

    fn shuffled_source_graph() -> Graph {
        let mut il=Il::default();
        // Deliberately scramble callback delivery. Edge 0 has two
        // children and edge 1 has two, all sharing terminal indices.
        for (path,parent) in [
            ("1",None),("0.1.1",Some("0")),
            ("0.1.0",Some("0")),("0.0.1",Some("0")),
            ("0.0.0.0.0",Some("0.0.0")),
            ("0",None),("0.0.0",Some("0")),
        ] {
            il.statements.push(Statement{
                function:"f".into(),path:path.into(),
                parent:parent.map(str::to_owned),kind:1,
            });
        }
        Graph::from_il(&il).expect("well-formed EDG source paths")
    }

    #[test]
    fn roots_and_preorder_use_full_numeric_source_paths() {
        let graph=shuffled_source_graph();
        let roots:Vec<_>=graph.roots("f")
            .map(|(_,s)|s.data.path.as_str()).collect();
        assert_eq!(roots,["0","1"]);
        let mut visited=Vec::new();
        graph.walk_statements("f",|_,s|{
            visited.push(s.data.path.clone());
            true
        });
        assert_eq!(visited,[
            "0","0.0.0","0.0.0.0.0","0.0.1",
            "0.1.0","0.1.1","1"
        ]);
        let mut early=Vec::new();
        graph.walk_statements("f",|_,s|{
            early.push(s.data.path.clone());
            early.len()<4
        });
        assert_eq!(early,["0","0.0.0","0.0.0.0.0","0.0.1"]);
    }

    #[test]
    fn postorder_preserves_branch_and_root_sequence() {
        let graph=shuffled_source_graph();
        let mut visited=Vec::new();
        graph.walk_statements_postorder("f",|_,s|{
            visited.push(s.data.path.clone());
            true
        });
        assert_eq!(visited,[
            "0.0.0.0.0","0.0.0","0.0.1",
            "0.1.0","0.1.1","0","1"
        ]);
    }

    #[test]
    fn numeric_source_order_handles_double_digit_siblings() {
        let mut il=Il::default();
        // Include every native sibling. This tests double-digit numeric
        // ordering without supplying a truncated/sparse statement sequence.
        for path in ["10","2","0","1","9","6","3","8","7","4","5"] {
            il.statements.push(Statement {
                function:"f".into(),path:path.into(),parent:None,kind:1,
            });
        }
        let graph=Graph::from_il(&il).unwrap();
        let paths:Vec<_>=graph.roots("f")
            .map(|(_,s)|s.data.path.as_str()).collect();
        assert_eq!(paths,["0","1","2","3","4","5","6","7","8","9","10"]);
    }
}

#[cfg(test)]
mod audit_statement_sequence_tests {
    use super::*;

    fn from_paths(items: &[(&str, Option<&str>)]) -> Result<Graph, GraphError> {
        let mut il = Il::default();
        for &(path, parent) in items {
            il.statements.push(Statement {
                function: "f".into(),
                path: path.into(),
                parent: parent.map(str::to_owned),
                kind: 1,
            });
        }
        Graph::from_il(&il)
    }

    #[test]
    fn accepts_dense_roots_and_independently_numbered_statement_edges() {
        assert!(from_paths(&[
            ("1",None), ("0",None),
            ("0.then.0",Some("0")), ("0.then.1",Some("0")),
            ("0.else.0",Some("0")), ("1.block.0",Some("1")),
        ]).is_ok());
    }

    #[test]
    fn rejects_missing_first_or_middle_native_statement() {
        for paths in [
            vec![("1",None)],
            vec![("0",None),("2",None)],
            vec![("0",None),("0.block.1",Some("0"))],
            vec![("0",None),("0.block.0",Some("0")),
                 ("0.block.2",Some("0"))],
            vec![("0",None),("0.block.00",Some("0"))],
            vec![("00",None)],
        ] {
            assert!(matches!(from_paths(&paths),
                Err(GraphError::InvalidStatementParent { .. })),
                "malformed statement path sequence was accepted: {paths:?}");
        }
    }
}
