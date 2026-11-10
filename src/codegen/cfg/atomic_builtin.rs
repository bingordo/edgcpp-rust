//! EDG's owned GCC-mode C11 atomic builtin projection.
//! Explicit orders are taken from constant EDG expressions, never source text.
//! This reference x86-64 lowering legally strengthens accepted orders to SC.
use super::*;

fn literal(graph:&Graph,id:ExpressionId) -> Result<i64,CodegenError> {
    let mut current=id;
    let mut visited=std::collections::HashSet::new();
    loop {
        if !visited.insert(current) {
            return Err(invalid("cyclic atomic literal expression"));
        }
        let n=graph.expression(current).ok_or_else(||invalid("missing atomic literal"))?;
        if let Some(value)=n.data.constant_integer {return Ok(value);}
        if n.operands.len()==1 && semantic(2,n.data.kind)==3 {
            let op=n.data.operator.map(|x|semantic(3,x)).unwrap_or(0);
            if matches!(op,11|12|16) {
                current=n.operands[0];
                continue;
            }
        }
        return Err(invalid("atomic order/weak operand must be literal integer"));
    }
}
fn order(
    graph:&Graph,id:ExpressionId,
    op:crate::memory_order::AtomicOperation,
) -> Result<crate::memory_order::MemoryOrder,CodegenError> {
    let raw=literal(graph,id)?;
    let order=crate::memory_order::from_gnu_builtin_constant(raw).map_err(invalid)?;
    crate::memory_order::validate_order(op,order)
        .map_err(|e|invalid(format!("invalid source atomic order: {e}")))?;
    Ok(order)
}
/// EDG's generated __atomic_N declarations accept void*/const void*.
/// Those argument conversion nodes deliberately erase the C pointee type.
/// Walk ONLY the owned unary conversion chain, consulting original symbol
/// declarations and typed expressions. Never infer the type from width alone.
fn source_pointee(
    graph:&Graph,id:ExpressionId,il:&Il,
    access:MemoryAccess,bytes:usize,
) -> Result<crate::TypeId,CodegenError> {
    let mut current=id;
    let mut visited=std::collections::HashSet::new();
    loop {
        if !visited.insert(current) {
            return Err(invalid("cyclic native atomic pointer expression"));
        }
        let node=graph.expression(current)
            .ok_or_else(||invalid("missing native atomic pointer operand"))?;
        let mut candidates=Vec::new();
        if let Some(sym)=node.data.symbol_id {
            if let Some(declared)=il.symbol_types.iter().find(|t|t.symbol==sym) {
                candidates.push(declared.ty);
            }
        }
        if let Some(ty)=node.data.type_id {candidates.push(ty);}
        for candidate in candidates {
            let Ok(base)=canonical_abi_type(il,candidate) else {continue};
            let Some(def)=il.types.iter().find(|t|t.id==base &&
                semantic(4,t.kind)==5) else {continue};
            let Some(element)=def.related else {continue};
            if scalar_memory_access(il,element).ok()!=Some(access) {continue;}
            if byte_size(il,element).ok()!=Some(bytes as u64) {continue;}
            if storage_alignment(il,element).ok().is_none_or(|a|a<bytes as u64) {
                continue;
            }
            let Ok(ty)=call_type_abi(il,element) else {continue};
            if ty.is_bool || usize::from(ty.bits)!=8*bytes {continue;}
            return Ok(element);
        }
        if semantic(2,node.data.kind)!=3 || node.operands.len()!=1 {
            break;
        }
        // Only transparent conversions and address formation may preserve
        // the original pointer's provenance. Other unary operations (e.g.
        // dereference or negation) must not confer an unrelated pointee type.
        let op=node.data.operator.map(|x|semantic(3,x)).unwrap_or(0);
        if !matches!(op,11|16|43|46) {
            break;
        }
        current=node.operands[0];
    }
    Err(invalid("cannot recover typed, aligned atomic builtin source pointer"))
}
pub(super) fn lower(
    graph:&Graph,name:&str,args:&[ExpressionId],
    params:&[crate::SymbolId],locals:&[&crate::LocalVariable],il:&Il,
) -> Result<Expr,CodegenError> {
    use crate::memory_order::{AtomicOperation,validate_compare_exchange};
    let arg=|i:usize| -> Result<ExpressionId,CodegenError> {
        args.get(i).copied().ok_or_else(||invalid("source atomic builtin argument missing"))
    };
    if name=="__atomic_thread_fence" {
        if args.len()!=1 {return Err(invalid("wrong atomic_thread_fence arity"));}
        return Ok(Expr::AtomicFence(order(graph,arg(0)?,AtomicOperation::Fence)?));
    }
    // Cranelift's effectful fence is a compiler barrier and a full hardware
    // fence. The extra runtime ordering is a legal strengthening of C11
    // signal-fence semantics on this x86-64 reference backend.
    if name=="__atomic_signal_fence" {
        if args.len()!=1 {return Err(invalid("wrong atomic_signal_fence arity"));}
        return Ok(Expr::AtomicFence(order(graph,arg(0)?,AtomicOperation::Fence)?));
    }
    let (family,width)=name.rsplit_once('_')
        .ok_or_else(||invalid(format!("unknown atomic builtin {name}")))?;
    let bytes:usize=width.parse()
        .map_err(|_|invalid(format!("unknown atomic builtin {name}")))?;
    if !matches!(bytes,1|2|4|8) {
        return Err(invalid("native atomic builtin width must be 1/2/4/8 bytes"));
    }
    let object=arg(0)?;
    let pointee=source_pointee(graph,object,il,MemoryAccess::Atomic,bytes)?;
    let ty=call_type_abi(il,pointee)?;
    if ty.is_bool || usize::from(ty.bits)!=bytes*8 {
        return Err(invalid("source atomic builtin pointee width/type mismatch"));
    }
    let lower=|id|lower_c_expression(graph,id,params,locals,il).map(Box::new);
    match family {
        "__atomic_load" if args.len()==2 => Ok(Expr::AtomicLoad(
            ty,order(graph,arg(1)?,AtomicOperation::Load)?,lower(object)?)),
        "__atomic_store" if args.len()==3 => Ok(Expr::AtomicStore(
            ty,order(graph,arg(2)?,AtomicOperation::Store)?,
            lower(object)?,lower(arg(1)?)?)),
        "__atomic_exchange" if args.len()==3 => Ok(Expr::AtomicFetch(
            ty,order(graph,arg(2)?,AtomicOperation::ReadModifyWrite)?,
            AtomicFetchOp::Exchange,lower(object)?,lower(arg(1)?)?)),
        "__atomic_fetch_add"|"__atomic_fetch_sub"|
        "__atomic_fetch_and"|"__atomic_fetch_or"|
        "__atomic_fetch_xor" if args.len()==3 => {
            let op=match family {
                "__atomic_fetch_add"=>AtomicFetchOp::Add,
                "__atomic_fetch_sub"=>AtomicFetchOp::Sub,
                "__atomic_fetch_and"=>AtomicFetchOp::And,
                "__atomic_fetch_or"=>AtomicFetchOp::Or,
                "__atomic_fetch_xor"=>AtomicFetchOp::Xor,
                _=>unreachable!(),
            };
            Ok(Expr::AtomicFetch(ty,
                order(graph,arg(2)?,AtomicOperation::ReadModifyWrite)?,
                op,lower(object)?,lower(arg(1)?)?))
        }
        "__atomic_compare_exchange" if args.len()==6 => {
            let weak=match literal(graph,arg(3)?)? {
                0=>false,1=>true,
                _=>return Err(invalid("atomic weak flag must be 0 or 1")),
            };
            let success=order(graph,arg(4)?,AtomicOperation::ReadModifyWrite)?;
            let failure=order(graph,arg(5)?,AtomicOperation::Load)?;
            validate_compare_exchange(success,failure)
                .map_err(|e|invalid(format!("invalid atomic compare-exchange order pair: {e}")))?;
            let expected=arg(1)?;
            let expected_ty=source_pointee(graph,expected,il,
                MemoryAccess::Ordinary,bytes)?;
            if call_type_abi(il,expected_ty)?!=ty {
                return Err(invalid("atomic expected pointer has wrong scalar type"));
            }
            Ok(Expr::AtomicCompareExchange {ty,success,failure,weak,
                object:lower(object)?,expected:lower(expected)?,desired:lower(arg(2)?)?})
        }
        _=>Err(invalid(format!(
            "unsupported source atomic builtin {name} with {} arguments",args.len()))),
    }
}

#[cfg(test)]
mod traversal_tests {
    use super::*;

    fn expression(value:Option<i64>) -> crate::Expression {
        crate::Expression {
            function:String::new(),statement_path:String::new(),
            path:String::new(),parent:None,kind:0,operator:None,
            type_kind:None,reference:None,type_size:None,
            type_alignment:None,constant_kind:None,
            constant_integer:value,related_type_role:None,
            related_type_kind:None,type_id:None,symbol_id:None,
        }
    }

    #[test]
    fn literal_reads_exact_constant_and_rejects_missing_node() {
        let mut graph=Graph::default();
        graph.expressions.push(crate::graph::ExpressionNode {
            data:expression(Some(42)),parent:None,operands:vec![],
            statement:crate::graph::StatementId(0),
        });
        assert_eq!(literal(&graph,ExpressionId(0)).unwrap(),42);
        assert!(literal(&graph,ExpressionId(1)).is_err());
    }

    // Resolve EDG's private enum values through the bridge, never hard-code
    // their numeric representation in Rust tests.
    fn edg_value(category:u32,expected:u32) -> u32 {
        (0..4096).find(|&value|semantic(category,value)==expected)
            .expect("EDG bridge must expose the tested expression kind/operator")
    }

    fn unary_graph(depth:usize,cyclic:bool) -> Graph {
        let mut graph=Graph::default();
        let kind=edg_value(2,3); // operation
        let op=edg_value(3,11); // parentheses
        for index in 0..depth {
            let mut data=expression(None);
            data.kind=kind;
            data.operator=Some(op);
            graph.expressions.push(crate::graph::ExpressionNode {
                data,parent:None,
                operands:vec![ExpressionId(if index+1==depth {
                    if cyclic {0} else {depth}
                } else {index+1})],
                statement:crate::graph::StatementId(0),
            });
        }
        if !cyclic {
            graph.expressions.push(crate::graph::ExpressionNode {
                data:expression(Some(73)),parent:None,operands:vec![],
                statement:crate::graph::StatementId(0),
            });
        }
        graph
    }

    #[test]
    fn literal_accepts_deep_unary_chain() {
        let graph=unary_graph(128,false);
        assert_eq!(literal(&graph,ExpressionId(0)).unwrap(),73);
    }

    #[test]
    fn literal_rejects_cyclic_unary_chain() {
        let graph=unary_graph(128,true);
        assert!(literal(&graph,ExpressionId(0)).is_err());
    }

    #[test]
    fn pointer_walk_rejects_cyclic_chain_without_hanging() {
        let graph=unary_graph(128,true);
        assert!(source_pointee(
            &graph,ExpressionId(0),&Il::default(),
            MemoryAccess::Atomic,4,
        ).is_err());
    }

    #[test]
    fn pointer_walk_does_not_follow_unrelated_unary_operation() {
        let mut graph=unary_graph(2,false);
        // A dereference is not a transparent conversion.
        graph.expressions[0].data.operator=Some(edg_value(3,44));
        assert!(source_pointee(
            &graph,ExpressionId(0),&Il::default(),
            MemoryAccess::Atomic,4,
        ).is_err());
    }

    #[test]
    fn pointer_walk_rejects_missing_node() {
        let graph=Graph::default();
        assert!(source_pointee(
            &graph,ExpressionId(0),&Il::default(),
            MemoryAccess::Atomic,4,
        ).is_err());
    }
}
