//! EDG's owned GCC-mode C11 atomic builtin projection.
//! Explicit orders are taken from constant EDG expressions, never source text.
//! This reference x86-64 lowering legally strengthens accepted orders to SC.
use super::*;

// Stable normalized semantic IDs from cpp/bridge.cpp::edgcpp_codegen_kind.
// These are NOT EDG's private/native enum discriminants.
const SEMANTIC_EXPRESSION: u32 = 2;
const SEMANTIC_OPERATOR: u32 = 3;
const SEMANTIC_TYPE: u32 = 4;
const EXPR_KIND_CONSTANT: u32 = 1;
const EXPR_KIND_OPERATION: u32 = 3;
const TYPE_KIND_POINTER: u32 = 5;
const OP_PARENS: u32 = 11;
const OP_UNARY_PLUS: u32 = 12;
const OP_CAST: u32 = 16;

fn literal(graph:&Graph,id:ExpressionId) -> Result<i64,CodegenError> {
    // Conversion chains may be arbitrarily long. Use an iterative walk so
    // malformed or deep graphs cannot exhaust the Rust stack.
    let mut current=id;
    let mut visited=std::collections::HashSet::new();
    loop {
        if !visited.insert(current) {
            return Err(invalid("cyclic atomic literal conversion chain"));
        }
        let n=graph.expression(current).ok_or_else(||invalid("missing atomic literal"))?;
        if let Some(value)=n.data.constant_integer {return Ok(value);}
        if n.operands.len()!=1 || semantic(SEMANTIC_EXPRESSION,n.data.kind)!=EXPR_KIND_OPERATION {
            break;
        }
        let op=n.data.operator.map(|x|semantic(SEMANTIC_OPERATOR,x)).unwrap_or(0);
        if !matches!(op,OP_PARENS|OP_UNARY_PLUS|OP_CAST) {break;}
        current=n.operands[0];
    }
    Err(invalid("atomic order/weak operand must be literal integer"))
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
            return Err(invalid("cyclic atomic pointer conversion chain"));
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
                semantic(SEMANTIC_TYPE,t.kind)==TYPE_KIND_POINTER) else {continue};
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
        if semantic(SEMANTIC_EXPRESSION,node.data.kind)!=EXPR_KIND_OPERATION || node.operands.len()!=1 {
            break;
        }
        // A native unary conversion or address-of is fine; a binary
        // expression requires a separately checked pointer/alias analysis.
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
mod audit_atomic_literal_tests {
    use super::*;
    use crate::graph::{ExpressionNode,StatementId};

    fn node(kind:u32, operator:Option<u32>, constant_integer:Option<i64>,
            operand:Option<ExpressionId>) -> ExpressionNode {
        ExpressionNode {
            data:crate::Expression{
                function:"f".into(), statement_path:"0".into(),path:"0".into(),
                parent:None, kind,operator,type_kind:None,reference:None,
                type_size:None,type_alignment:None,constant_kind:None,
                constant_integer,related_type_role:None,related_type_kind:None,
                type_id:None,symbol_id:None,
            },
            parent:None,operands:operand.into_iter().collect(),
            statement:StatementId(0),
        }
    }

    #[test]
    fn deeply_nested_atomic_order_conversions_and_cycles() {
        let literal_kind=(0..128).find(|&k|semantic(SEMANTIC_EXPRESSION,k)==EXPR_KIND_CONSTANT)
            .expect("EDG constant expression kind");
        let operator_kind=(0..128).find(|&k|semantic(SEMANTIC_EXPRESSION,k)==EXPR_KIND_OPERATION)
            .expect("EDG operator expression kind");
        let conversion=(0..128).find(|&k|semantic(SEMANTIC_OPERATOR,k)==OP_PARENS)
            .expect("EDG identity conversion operator");
        let mut graph=Graph::default();
        graph.expressions.push(node(literal_kind,None,Some(7),None));
        for index in 1..128 {
            graph.expressions.push(node(operator_kind,Some(conversion),None,
                Some(ExpressionId(index-1))));
        }
        assert_eq!(literal(&graph,ExpressionId(127)).unwrap(),7);
        // An invalid input graph must fail rather than recurse indefinitely.
        graph.expressions[1].operands=vec![ExpressionId(1)];
        assert!(literal(&graph,ExpressionId(127)).is_err());
        assert!(literal(&graph,ExpressionId(128)).is_err());
    }
}
