//! Indexed, owned view of the EDG statement and expression snapshots.
//! IDs are stable within one `Graph`, not across compilations.
use std::collections::HashMap;
use crate::{Il, Statement, Expression, TypeDef, TypeId, TypeParameter, ArrayBound, RecordField, TypeDetail, FunctionSignature, Symbol, SymbolId, VariableInitializer, InitializerInteger, DynamicInitializer};

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
    DuplicateTypeDetail(TypeId),
    DuplicateFunctionSignature(TypeId),
    InvalidTypeRelation(TypeId),
    DuplicateSymbol(SymbolId),
    MissingSymbol(SymbolId),
    DuplicateVariableInitializer(SymbolId),
    DuplicateInitializerInteger(SymbolId),
    DuplicateDynamicInitializer(SymbolId),
}

/// A dense-ID graph retaining EDG's native kinds and the original paths.
#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub statements: Vec<StatementNode>,
    pub expressions: Vec<ExpressionNode>,
    pub types: Vec<TypeDef>,
    pub type_parameters: Vec<TypeParameter>,
    pub array_bounds: Vec<ArrayBound>,
    pub record_fields: Vec<RecordField>,
    pub type_details: Vec<TypeDetail>,
    pub function_signatures: Vec<FunctionSignature>,
    pub symbols: Vec<Symbol>,
    pub variable_initializers: Vec<VariableInitializer>,
    pub initializer_integers: Vec<InitializerInteger>,
    pub dynamic_initializers: Vec<DynamicInitializer>,
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
        Ok(graph)
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
    pub fn fields(&self, owner: TypeId) -> impl Iterator<Item = &RecordField> {
        self.record_fields.iter().filter(move |f| f.owner == owner)
    }
    pub fn array_bound(&self, array: TypeId) -> Option<u64> {
        self.array_bounds.iter().find(|b| b.array == array).map(|b| b.elements)
    }
    pub fn parameters(&self, owner: TypeId) -> impl Iterator<Item = &TypeParameter> {
        self.type_parameters.iter().filter(move |p| p.owner == owner)
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
