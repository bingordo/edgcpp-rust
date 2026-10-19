//! Indexed, owned view of the EDG statement and expression snapshots.
//! IDs are stable within one `Graph`, not across compilations.
use std::collections::HashMap;
use crate::{Il, Statement, Expression};

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
}

/// A dense-ID graph retaining EDG's native kinds and the original paths.
#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub statements: Vec<StatementNode>,
    pub expressions: Vec<ExpressionNode>,
}

impl Graph {
    pub fn from_il(il: &Il) -> Result<Self, GraphError> {
        let mut graph = Self::default();
        let mut smap = HashMap::new();
        let mut emap = HashMap::new();
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
                    .ok_or(GraphError::MissingStatement { function, path })?;
                graph.statements[idx].parent = Some(id);
                graph.statements[id.0].children.push(StatementId(idx));
            }
        }
        for idx in 0..graph.expressions.len() {
            let item = &graph.expressions[idx].data;
            if let Some(ref path) = item.parent {
                let key = (item.function.clone(), item.statement_path.clone(), path.clone());
                let parent = *emap.get(&key)
                    .ok_or_else(|| GraphError::MissingExpression {
                        function: key.0, statement: key.1, path: key.2,
                    })?;
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
                kind:1, operator:Some(1), type_kind:Some(2),
            });
        }
        let graph = Graph::from_il(&il).unwrap();
        assert_eq!(graph.statements[0].children, vec![StatementId(1)]);
        let root = graph.statements[1].expression.unwrap();
        assert_eq!(graph.expressions[root.0].operands, vec![ExpressionId(2), ExpressionId(1)]);
        assert_eq!(graph.expressions[2].parent, Some(root));
    }
    #[test]
    fn rejects_dangling_statement() {
        let mut il = Il::default();
        il.statements.push(Statement { function:"f".into(), path:"0".into(), parent:Some("missing".into()), kind:1 });
        assert!(matches!(Graph::from_il(&il), Err(GraphError::MissingStatement { .. })));
    }
}
