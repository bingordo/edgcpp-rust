//! Indexed, owned view of the EDG statement and expression snapshots.
//! IDs are stable within one `Graph`, not across compilations.
use std::collections::HashMap;
use crate::{Il, Statement, StatementAttributes, ControlExpression, StatementLabel, StatementDeclaration, StatementInitialization, FunctionParameter, LocalScope, LocalVariable, StaticLocalOwner, Expression, TypeDef, TypeId, TypeParameter, ArrayBound, RecordField, BaseClass, BaseClassAccess, ConstructorInitializer, ConstructorInitializerExpression, MemberFunction, RoutineAttributes, TypeDetail, FunctionSignature, Symbol, SymbolId, SymbolType, VariableInitializer, InitializerInteger, DynamicInitializer, InitializerExpression, CallSite};

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
    pub symbol_types: Vec<SymbolType>,
    pub variable_initializers: Vec<VariableInitializer>,
    pub initializer_integers: Vec<InitializerInteger>,
    pub dynamic_initializers: Vec<DynamicInitializer>,
    pub initializer_expressions: Vec<InitializerExpression>,
    pub calls: Vec<CallSite>,
    label_definitions: HashMap<SymbolId, StatementId>,
    statement_label_index: HashMap<StatementId, usize>,
    control_expr_index: HashMap<(String, String, u32, String), usize>,
    control_expr_children: HashMap<(String, String, u32, String), Vec<usize>>,
    constructor_expr_index: HashMap<(SymbolId, u32, String), usize>,
    constructor_expr_children: HashMap<(SymbolId, u32, String), Vec<usize>>,
    initializer_index: HashMap<(SymbolId, String), usize>,
    initializer_children: HashMap<(SymbolId, String), Vec<usize>>,
    call_expression_ids: Vec<ExpressionId>,
}

impl Graph {
    pub fn from_il(il: &Il) -> Result<Self, GraphError> {
        let mut graph = Self::default();
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
            let valid_statement = scope.statement_path.as_ref().is_none_or(|path|
                smap.contains_key(&(
                    graph.symbols.iter().find(|s| s.id == scope.routine).unwrap().name.clone(),
                    path.clone(),
                )));
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
fn graph_path_index(path: &str) -> usize {
    path.rsplit('.').next().and_then(|n| n.parse().ok()).unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
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
