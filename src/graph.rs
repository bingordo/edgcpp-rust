//! Indexed, owned view of the EDG statement and expression snapshots.
//! IDs are stable within one `Graph`, not across compilations.
use std::collections::HashMap;
use crate::{Il, Statement, StatementAttributes, ControlExpression, StatementLabel, StatementDeclaration, StatementInitialization, SourceLocation, StatementSource, SymbolSource, ExpressionSource, RoutineIdentity, ObjectLifetime, ScopeLifetime, InitializationLifetime, LifetimeDestruction, FunctionParameter, LocalScope, LocalVariable, StaticLocalOwner, Expression, TypeDef, TypeId, TypeParameter, ArrayBound, RecordField, BaseClass, BaseClassAccess, ConstructorInitializer, ConstructorInitializerExpression, MemberFunction, RoutineAttributes, TypeDetail, FunctionSignature, Symbol, SymbolId, SymbolType, VariableInitializer, InitializerInteger, DynamicInitializer, InitializerExpression, CallSite};

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
    DuplicateArrayBound(TypeId),
    DuplicateRecordField { owner: TypeId, position: u32 },
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
    DuplicateVariableInitializer(SymbolId),
    DuplicateInitializerInteger(SymbolId),
    DuplicateDynamicInitializer(SymbolId),
    DuplicateInitializerExpression { owner: SymbolId, path: String },
    InvalidInitializerParent { owner: SymbolId, path: String },
    MissingInitializerRoot(SymbolId),
    InvalidInitializerOperandPath { owner: SymbolId, path: String },
    MissingCallExpression { function: String, statement: String, path: String },
    DuplicateCallExpression { function: String, statement: String, path: String },
    InvalidCallOperands { function: String, statement: String, path: String },
    InvalidCallOperandPath { function: String, statement: String, path: String },
    InvalidExpressionOperandPath { function: String, statement: String, path: String },
}

/// A dense-ID graph retaining EDG's native kinds and the original paths.
#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub statements: Vec<StatementNode>,
    pub statement_attributes: Vec<StatementAttributes>,
    pub control_expressions: Vec<ControlExpression>,
    pub statement_labels: Vec<StatementLabel>,
    pub statement_declarations: Vec<StatementDeclaration>,
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
    call_expression_ids: Vec<ExpressionId>,
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

impl Graph {
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
            if let Some(child) = ty.related {
                if !type_map.contains_key(&child) { return Err(GraphError::MissingType(child)); }
            }
        }
        let mut parameters = std::collections::HashSet::new();
        for param in &il.type_parameters {
            for id in [param.owner, param.ty] {
                if !type_map.contains_key(&id) {
                    return Err(GraphError::MissingType(id));
                }
            }
            if !parameters.insert((param.owner, param.position)) {
                return Err(GraphError::DuplicateParameter {
                    owner: param.owner, position: param.position,
                });
            }
            graph.type_parameters.push(param.clone());
        }
        graph.type_parameters.sort_by_key(|p| (p.owner.0, p.position));
        let mut bounded = std::collections::HashSet::new();
        for bound in &il.array_bounds {
            if !type_map.contains_key(&bound.array) {
                return Err(GraphError::MissingType(bound.array));
            }
            if !bounded.insert(bound.array) {
                return Err(GraphError::DuplicateArrayBound(bound.array));
            }
            graph.array_bounds.push(bound.clone());
        }
        let mut field_positions = std::collections::HashSet::new();
        for field in &il.record_fields {
            for id in [field.owner, field.ty] {
                if !type_map.contains_key(&id) {
                    return Err(GraphError::MissingType(id));
                }
            }
            if !field_positions.insert((field.owner, field.position)) {
                return Err(GraphError::DuplicateRecordField {
                    owner: field.owner, position: field.position,
                });
            }
            graph.record_fields.push(field.clone());
        }
        graph.record_fields.sort_by_key(|f| (f.owner.0, f.position));
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
            if !type_map.contains_key(&detail.owner) {
                return Err(GraphError::MissingType(detail.owner));
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
            if !signatures.insert(sig.owner) {
                return Err(GraphError::DuplicateFunctionSignature(sig.owner));
            }
            graph.function_signatures.push(sig.clone());
        }
        let mut symbol_ids = std::collections::HashSet::new();
        for symbol in &il.symbols {
            if symbol.id.0 == 0 || !symbol_ids.insert(symbol.id) {
                return Err(GraphError::DuplicateSymbol(symbol.id));
            }
            graph.symbols.push(symbol.clone());
        }
        for identity in &il.routine_identities {
            if !graph.symbols.iter().any(|sym|
                    sym.id == identity.routine && sym.kind == 1)
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
            if !symbol_ids.contains(&edge.symbol) || !type_map.contains_key(&edge.ty) {
                return Err(GraphError::InvalidSymbolType(edge.symbol));
            }
            if !typed_symbols.insert(edge.symbol) {
                return Err(GraphError::DuplicateSymbolType(edge.symbol));
            }
            graph.symbol_types.push(edge.clone());
        }
        let mut annotated_routines = std::collections::HashSet::new();
        for attributes in &il.routine_attributes {
            if !graph.symbols.iter().any(|symbol|
                symbol.id == attributes.routine && symbol.kind == 1)
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
                !graph.symbols.iter().any(|s| s.id == member.routine && s.kind == 1) {
                return Err(GraphError::InvalidMemberFunction {
                    owner: member.owner, routine: member.routine,
                });
            }
            if !member_owners.insert(member.routine) {
                return Err(GraphError::DuplicateMemberFunction(member.routine));
            }
            graph.member_functions.push(member.clone());
        }
        // Constructor initializers refer to routine symbols, field symbols,
        // or record types. Check the complete snapshot before indexing them.
        let mut ctor_positions = std::collections::HashSet::new();
        for init in &il.constructor_initializers {
            let valid_routine = graph.symbols.iter()
                .any(|s| s.id == init.routine && s.kind == 1);
            let valid_target = match init.kind {
                0 | 1 => init.target_type.is_some_and(|ty| type_map.contains_key(&ty))
                    && init.target_field.is_none(),
                2 => init.target_field.is_some_and(|field| graph.symbols.iter()
                    .any(|s| s.id == field && s.kind == 3))
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
                || expr.symbol_id.is_some_and(|id| !symbol_ids.contains(&id))
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
            if !symbol_ids.contains(&init.symbol) {
                return Err(GraphError::MissingSymbol(init.symbol));
            }
            if !initialized.insert(init.symbol) {
                return Err(GraphError::DuplicateVariableInitializer(init.symbol));
            }
            graph.variable_initializers.push(init.clone());
        }
        let mut integer_inits = std::collections::HashSet::new();
        for val in &il.initializer_integers {
            if !symbol_ids.contains(&val.symbol) {
                return Err(GraphError::MissingSymbol(val.symbol));
            }
            if !integer_inits.insert(val.symbol) {
                return Err(GraphError::DuplicateInitializerInteger(val.symbol));
            }
            graph.initializer_integers.push(val.clone());
        }
        let mut dynamic_inits = std::collections::HashSet::new();
        for init in &il.dynamic_initializers {
            if !symbol_ids.contains(&init.symbol) { return Err(GraphError::MissingSymbol(init.symbol)); }
            if !dynamic_inits.insert(init.symbol) {
                return Err(GraphError::DuplicateDynamicInitializer(init.symbol));
            }
            graph.dynamic_initializers.push(init.clone());
        }
        let mut init_paths = std::collections::HashSet::new();
        for node in &il.initializer_expressions {
            if !symbol_ids.contains(&node.owner) {
                return Err(GraphError::MissingSymbol(node.owner));
            }
            if let Some(id) = node.symbol_id {
                if !symbol_ids.contains(&id) { return Err(GraphError::MissingSymbol(id)); }
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
                if !symbol_ids.contains(&id) { return Err(GraphError::MissingSymbol(id)); }
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
        let is_routine = |id: SymbolId| graph.symbols.iter()
            .any(|s| s.id == id && s.kind == 1);
        let is_variable = |id: SymbolId| graph.symbols.iter()
            .any(|s| s.id == id && s.kind == 2);
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
                    .or_else(|| graph.symbols.iter()
                        .find(|sym| sym.id == scope.routine)
                        .map(|sym| sym.name.as_str()));
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
        let mut static_owner_symbols = std::collections::HashSet::new();
        for entry in &il.static_local_owners {
            if !is_routine(entry.routine) || !is_variable(entry.symbol) ||
                graph.local_variables.iter().any(|local| {
                    local.symbol == entry.symbol &&
                    (local.routine != entry.routine || !local.static_storage)
                })
            {
                return Err(GraphError::InvalidStaticLocalOwner(entry.symbol));
            }
            if !static_owner_symbols.insert(entry.symbol) {
                return Err(GraphError::DuplicateStaticLocalOwner(entry.symbol));
            }
            graph.static_local_owners.push(entry.clone());
        }
        graph.static_local_owners.sort_by_key(|entry| (entry.routine.0, entry.symbol.0));


        for label in &il.statement_labels {
            let key = (label.function.clone(), label.statement_path.clone());
            let Some(&statement) = smap.get(&key) else {
                return Err(GraphError::InvalidStatementLabel {
                    function: label.function.clone(), path: label.statement_path.clone(),
                });
            };
            if !graph.symbols.iter().any(|symbol|
                symbol.id == label.label && symbol.kind == 4)
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
                || !graph.symbols.iter().any(|sym| sym.id == decl.symbol && sym.kind == 2)
                || !type_map.contains_key(&decl.ty)
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

        let mut init_keys = std::collections::HashSet::new();
        for init in &il.statement_initializations {
            let key = (init.function.clone(), init.statement_path.clone());
            if !smap.contains_key(&key) || init.role > 2 ||
               init.symbol.is_some_and(|symbol| !graph.symbols.iter()
                   .any(|sym| sym.id == symbol && sym.kind == 2)) {
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
                || expr.symbol_id.is_some_and(|id| !symbol_ids.contains(&id))
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
            graph.call_expression_ids.push(emap[&key]);
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
        let stmt_order: Vec<usize> = graph.statements.iter()
            .map(|n| graph_path_index(&n.data.path)).collect();
        for node in &mut graph.expressions {
            node.operands.sort_by_key(|id| expr_order[id.0]);
        }
        for node in &mut graph.statements {
            node.children.sort_by_key(|id| stmt_order[id.0]);
        }
        // A call's first operand is its callee; member calls additionally
        // carry an object operand. Missing required operands indicate a
        // truncated native snapshot rather than a valid zero-argument call.
        for call in &graph.calls {
            let key = (call.function.clone(), call.statement_path.clone(), call.path.clone());
            let id = emap[&key];
            if !(call.first_argument == 1 || call.first_argument == 2)
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
            if !graph.symbols.iter().any(|symbol|
                    symbol.id == region.routine && symbol.kind == 1) ||
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
            if !graph.local_scopes.iter().any(|scope|
                 scope.routine == item.routine && scope.id == item.scope)
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
                || !graph.statement_initializations.iter().any(|init|
                    init.function == item.function &&
                    init.statement_path == item.statement_path)
                || item.lifetime.is_some_and(|id|
                    owner.is_none_or(|routine| !graph.lifetime_index.contains_key(&(*routine, id))))
                || item.destructor.is_some_and(|symbol|
                    !graph.symbols.iter().any(|s| s.id == symbol && s.kind == 1))
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
        for item in &il.lifetime_destructions {
            let key = (item.routine, item.lifetime);
            if !graph.lifetime_index.contains_key(&key)
                || item.variable.is_some_and(|symbol|
                    !graph.symbols.iter().any(|s| s.id == symbol && s.kind == 2))
                || item.destructor.is_some_and(|symbol|
                    !graph.symbols.iter().any(|s| s.id == symbol && s.kind == 1))
            {
                return Err(GraphError::InvalidLifetimeDestruction {
                    routine: item.routine, lifetime: item.lifetime,
                });
            }
            let positions = destruction_positions.entry(key).or_default();
            if positions.contains(&item.position) {
                return Err(GraphError::DuplicateLifetimeDestruction {
                    routine: item.routine, lifetime: item.lifetime,
                    position: item.position,
                });
            }
            positions.push(item.position);
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
            if !valid_position(&record.location) || !symbol_ids.contains(&record.symbol) {
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
        self.statements.iter().enumerate()
            .filter(move |(_, n)| n.parent.is_none() && n.data.function == function)
            .map(|(i, n)| (StatementId(i), n))
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
        self.call_expression_ids.iter().position(|&candidate| candidate == id)
            .map(|index| &self.calls[index])
    }

    /// Resolve the expression associated with a call-site record.
    ///
    /// Call-site references should come from this graph's `calls` slice.
    pub fn call_expression(&self, call: &CallSite) -> Option<ExpressionId> {
        self.calls.iter().position(|candidate| candidate == call)
            .map(|index| self.call_expression_ids[index])
    }

    /// Callee expression (operand zero) of an EDG call.
    pub fn call_callee(&self, call: &CallSite) -> Option<ExpressionId> {
        let id = self.call_expression(call)?;
        self.expressions[id.0].operands.first().copied()
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
        Some(RoutineView {
            symbol: self.symbol(routine)?,
            function: self.routine_function(routine),
            ty: self.symbol_type_view(routine),
            implicit_this: self.implicit_this(routine),
        })
    }

    /// Borrow a variable's symbol, declared type and initializer metadata.
    pub fn variable_view(&self, variable: SymbolId) -> Option<VariableView<'_>> {
        Some(VariableView {
            symbol: self.symbol(variable)?,
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
mod tests {
    use super::*;
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
            RecordField { owner: TypeId(1), position: 0, ty: TypeId(2),
                name: "first".into(), byte_offset: 0, bit_offset: 0, bit_width: 0 },
            RecordField { owner: TypeId(1), position: 1, ty: TypeId(2),
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
            has_this_parameter: true,
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
                kind: 1, operator: None, type_kind: None, reference: None,
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
    fn resolves_regular_call_with_no_object() {
        let mut il = Il::default();
        il.statements.push(Statement {
            function: "f".into(), path: "0".into(), parent: None, kind: 1,
        });
        for (path, parent) in [("0.1", Some("0")), ("0", None), ("0.0", Some("0"))] {
            il.expressions.push(Expression {
                function: "f".into(), statement_path: "0".into(),
                path: path.into(), parent: parent.map(str::to_owned),
                kind: 1, operator: None, type_kind: None, reference: None,
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
        assert_eq!(graph.call_callee(call), Some(ExpressionId(2)));
        assert_eq!(graph.call_object(call), None);
        assert_eq!(graph.call_arguments(call), Some(&[ExpressionId(0)][..]));
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
                kind: 1, operator: None, type_kind: None, reference: None,
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
            kind: 1, operator: None, type_kind: None, reference: None,
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
            id: TypeId(1), kind: 5, related_role: Some(1),
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
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(graph.parameters(TypeId(1)).next().unwrap().ty, TypeId(2));
    }

    #[test]
    fn record_fields_are_linked_and_ordered() {
        let mut il = Il::default();
        il.types.push(TypeDef { id: TypeId(1), kind: 0, related_role: None, related: None });
        il.types.push(TypeDef { id: TypeId(2), kind: 0, related_role: None, related: None });
        il.record_fields.push(RecordField { owner: TypeId(1), position: 1, ty: TypeId(2), name: "b".into(), byte_offset: 4, bit_offset: 0, bit_width: 0 });
        il.record_fields.push(RecordField { owner: TypeId(1), position: 0, ty: TypeId(2), name: "a".into(), byte_offset: 0, bit_offset: 0, bit_width: 0 });
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(graph.fields(TypeId(1)).map(|f| f.name.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
    }

    #[test]
    fn resolves_reference_details() {
        let mut il = Il::default();
        il.types.push(TypeDef { id: TypeId(1), kind: 0, related_role: None, related: None });
        il.type_details.push(TypeDetail { owner: TypeId(1), is_reference: true,
            is_rvalue_reference: false, qualifiers: None });
        let graph = Graph::from_il(&il).unwrap();
        assert!(graph.type_detail(TypeId(1)).unwrap().is_reference);
    }

    #[test]
    fn routine_signature_metadata() {
        let mut il = Il::default();
        il.types.push(TypeDef { id: TypeId(1), kind: 0, related_role: None, related: None });
        il.function_signatures.push(FunctionSignature { owner: TypeId(1),
            prototyped: true, variadic: true });
        assert!(Graph::from_il(&il).unwrap().signature(TypeId(1)).unwrap().variadic);
    }

    #[test]
    fn fixed_array_bound_is_owned() {
        let mut il = Il::default();
        il.types.push(TypeDef { id: TypeId(1), kind: 0, related_role: None, related: None });
        il.array_bounds.push(ArrayBound { array: TypeId(1), elements: 12 });
        assert_eq!(Graph::from_il(&il).unwrap().array_bound(TypeId(1)), Some(12));
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
