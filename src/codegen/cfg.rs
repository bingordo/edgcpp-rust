//! EDG structured statements -> Cranelift CFG with typed C integer SSA locals.
use super::*;
use std::collections::{HashSet,HashMap};
use std::cell::Cell;
use cranelift_codegen::ir::Block;
use cranelift_frontend::Variable;

#[derive(Debug, Clone)]
enum Stmt {
    Store(usize, Expr),
    Eval(Expr),
    Return(Expr),
    ReturnVoid,
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    /// Condition is checked before the body; increment runs after the body.
    For { init: Vec<Stmt>, condition: Expr, increment: Vec<Stmt>, body: Vec<Stmt> },
    /// Test is at the bottom, so the body runs at least once.
    DoWhile(Vec<Stmt>, Expr),
    Break,
    Continue,
}
struct Routine {
    name: String, parameters: usize, locals: usize, body: Vec<Stmt>,
    /// None on existing standalone unit-test routines (signed-i64 ABI).
    abi: Option<CAbi>,
    /// None only for legacy signed-i64 unit-test routines.
    local_types: Option<Vec<CAbiInt>>,
}
fn invalid(msg: impl Into<String>) -> CodegenError { CodegenError::InvalidFunction(msg.into()) }

/// CFG-only scalar local validation. Addressable and volatile storage remains
/// deliberately unsupported until the memory backend is implemented.
fn cfg_local_type(il: &Il, local: &crate::LocalVariable) -> Result<CAbiInt, CodegenError> {
    if local.static_storage || local.compiler_generated ||
        local.structured_binding || local.handler_parameter {
        return Err(invalid(format!("unsupported local storage for {}", local.symbol.0)));
    }
    // Typedef chains may hide qualifiers. Conservatively reject them now.
    let mut ty = local.ty;
    for _ in 0..=il.types.len() {
        if let Some(detail) = il.type_details.iter().find(|d| d.owner == ty) {
            if detail.is_reference || detail.is_rvalue_reference ||
               detail.qualifiers.unwrap_or(0) != 0 {
                return Err(invalid("qualified/reference local requires memory semantics"));
            }
        }
        let def = il.types.iter().find(|d| d.id == ty)
            .ok_or_else(|| invalid("missing local type definition"))?;
        if semantic(4, def.kind) != 3 { break; }
        ty = def.related.ok_or_else(|| invalid("unresolved local typeref"))?;
    }
    let ty = canonical_abi_type(il, local.ty)?;
    if il.types.iter().any(|d| d.id == ty && semantic(4,d.kind) == 5) {
        return Ok(CAbiInt { bits:64, signed:false, is_bool:false });
    }
    let info = il.c_integer_types.iter().find(|t| t.id == ty)
        .ok_or_else(|| invalid(format!("local {} has unsupported scalar type",local.symbol.0)))?;
    Ok(CAbiInt { bits: info.bits, signed: info.signed, is_bool: info.is_bool })
}
fn cfg_checked_local(
    il: &Il, locals: &[&crate::LocalVariable], symbol: crate::SymbolId,
) -> Result<usize, CodegenError> {
    let index = locals.iter().position(|v| v.symbol == symbol)
        .ok_or_else(|| invalid(format!("unsupported/nonlocal variable {}",symbol.0)))?;
    cfg_local_type(il, locals[index])?;
    Ok(index)
}
pub(super) fn normalize_c_value(
    fb: &mut FunctionBuilder<'_>, value: cranelift_codegen::ir::Value, ty: CAbiInt,
) -> cranelift_codegen::ir::Value {
    if ty.is_bool {
        // A C _Bool/C++ bool stores 0 or 1, not arbitrary nonzero bytes.
        let condition = fb.ins().icmp_imm(IntCC::NotEqual, value, 0);
        let one = fb.ins().iconst(types::I64, 1);
        let zero = fb.ins().iconst(types::I64, 0);
        return fb.ins().select(condition, one, zero);
    }
    if ty.bits == 64 { return value; }
    let small = fb.ins().ireduce(ty.cranelift(), value);
    if ty.signed { fb.ins().sextend(types::I64, small) }
    else { fb.ins().uextend(types::I64, small) }
}

fn lower(
    graph: &Graph, id: crate::graph::StatementId,
    params: &[crate::SymbolId], locals: &[&crate::LocalVariable], il: &Il,
) -> Result<Vec<Stmt>, CodegenError> {
    let node = graph.statement(id).ok_or_else(|| invalid("missing statement"))?;
    let path = &node.data.path;
    let function = &node.data.function;
    let expr = |id| lower_c_expression(graph, id, params, locals, il);
    match semantic(1, node.data.kind) {
        1 => {
            if !node.children.is_empty() { return Err(invalid("return with nested statements")); }
            if let Some(id) = node.expression {
                Ok(vec![Stmt::Return(expr(id)?)])
            } else {
                Ok(vec![Stmt::ReturnVoid])
            }
        }
        2 => {
            if node.expression.is_some() { return Err(invalid("block has an expression")); }
            let mut body = Vec::new();
            for &child in &node.children { body.extend(lower(graph, child, params, locals, il)?); }
            Ok(body)
        }
        3 => {
            let condition = expr(node.expression.ok_or_else(|| invalid("if has no condition"))?)?;
            let mut yes = None;
            let mut no = None;
            for &child in &node.children {
                let nested = graph.statement(child).ok_or_else(|| invalid("missing if child"))?;
                if nested.data.path.starts_with(&format!("{path}.then.")) {
                    if yes.replace(child).is_some() { return Err(invalid("duplicate then")); }
                } else if nested.data.path.starts_with(&format!("{path}.else.")) {
                    if no.replace(child).is_some() { return Err(invalid("duplicate else")); }
                } else { return Err(invalid("unknown if child")); }
            }
            let yes = yes.ok_or_else(|| invalid("if has no then branch"))?;
            let yes = lower(graph, yes, params, locals, il)?;
            let no = match no { Some(id) => lower(graph, id, params, locals, il)?,
                None => Vec::new() };
            Ok(vec![Stmt::If(condition, yes, no)])
        }
        4 => {
            if node.expression.is_some() || !node.children.is_empty() {
                return Err(invalid("executable declaration unsupported"));
            }
            let mut found = false;
            for decl in graph.declarations_at(function, path) {
                found = true;
                if decl.is_vla { return Err(invalid("VLA unsupported")); }
                cfg_checked_local(il, locals, decl.symbol)?;
            }
            if !found { return Err(invalid("non-variable declaration unsupported")); }
            Ok(Vec::new())
        }
        5 => {
            let init = graph.statement_initialization(function, path)
                .ok_or_else(|| invalid("missing local initializer metadata"))?;
            if init.role != 0 || !node.children.is_empty() {
                return Err(invalid("unsupported initializer"));
            }
            let symbol = init.symbol.ok_or_else(|| invalid("initializer has no target"))?;
            let value = if let Some(id) = node.expression {
                expr(id)?
            } else {
                let scalars: Vec<_> = il.statement_integer_initializers.iter()
                    .filter(|v| v.function == *function && v.statement_path == *path)
                    .collect();
                if scalars.len() != 1 {
                    return Err(invalid(format!(
                        "{function}:{path}: initializer has no supported integer expression or constant")));
                }
                Expr::Constant(scalars[0].value)
            };
            Ok(vec![Stmt::Store(cfg_checked_local(il, locals, symbol)?, value)])
        }
        6 => {
            if !node.children.is_empty() { return Err(invalid("expression statement has children")); }
            let root = node.expression.ok_or_else(|| invalid("missing expression"))?;
            Ok(vec![Stmt::Eval(expr(root)?)])
        }
        7 | 8 => {
            let condition = expr(node.expression.ok_or_else(|| invalid("while has no condition"))?)?;
            if node.children.len() != 1 { return Err(invalid("while body not unique")); }
            let child = node.children[0];
            let body = graph.statement(child).ok_or_else(|| invalid("missing while body"))?;
            if !body.data.path.starts_with(&format!("{path}.body.")) {
                return Err(invalid("unexpected while body"));
            }
            let body = lower(graph, child, params, locals, il)?;
            Ok(vec![if semantic(1, node.data.kind) == 8 {
                Stmt::DoWhile(body, condition)
            } else {
                Stmt::While(condition, body)
            }])
        }
        9 => {
            // An increment is emitted as a synthetic stmk_expr child of the
            // for node, preserving its executable tree (including constants).
            // The original control-role-1 snapshot is also retained.
            let condition = expr(node.expression.ok_or_else(|| invalid(
                "unconditional for loop not yet supported"))?)?;
            let mut init = None;
            let mut body = None;
            let mut increment = None;
            for &child in &node.children {
                let child_node = graph.statement(child).ok_or_else(|| invalid("missing for child"))?;
                let child_path = &child_node.data.path;
                if child_path.starts_with(&format!("{path}.init.")) {
                    if init.replace(child).is_some() { return Err(invalid("multiple for init statements")); }
                } else if child_path.starts_with(&format!("{path}.body.")) {
                    if body.replace(child).is_some() { return Err(invalid("multiple for bodies")); }
                } else if child_path == &format!("{path}.increment.0") {
                    if increment.replace(child).is_some() { return Err(invalid("duplicate for increment")); }
                } else { return Err(invalid("unexpected for child")); }
            }
            let init = match init { Some(id) => lower(graph, id, params, locals, il)?,
                None => Vec::new() };
            let body = lower(graph, body.ok_or_else(|| invalid("for body missing"))?,
                params, locals, il)?;
            let increment = match increment { Some(id) => lower(graph, id, params, locals, il)?,
                None => Vec::new() };
            Ok(vec![Stmt::For { init, condition, increment, body }])
        }
        10 | 11 => {
            // Native EDG records classify only compiler-generated loop
            // break/continue targets. Never interpret arbitrary gotos.
            let sem = semantic(1, node.data.kind);
            let matching: Vec<_> = il.loop_transfers.iter()
                .filter(|t| t.function == *function && t.statement_path == *path)
                .collect();
            if matching.len() != 1 || graph.statement_label(id).is_none() {
                return Err(invalid("unsupported non-loop goto or label"));
            }
            match (sem, matching[0].kind) {
                (10, 1) => Ok(vec![Stmt::Break]),
                (10, 2) => Ok(vec![Stmt::Continue]),
                (11, 3 | 4) => Ok(Vec::new()),
                _ => Err(invalid("inconsistent EDG loop-label classification")),
            }
        }
        _ => Err(invalid(format!("unsupported EDG statement {}", node.data.kind))),
    }
}
fn check_expr(e: &Expr, vars: &mut HashSet<usize>, params: usize) -> Result<(), CodegenError> {
    match e {
        Expr::Constant(_) => Ok(()),
        Expr::Parameter(i) => if *i < params { Ok(()) } else { Err(invalid("invalid parameter")) },
        Expr::Local(i) => if vars.contains(i) { Ok(()) }
            else { Err(invalid(format!("local {i} read before initialization"))) },
        Expr::Global(_) => Ok(()),
        Expr::AssignGlobal(_,_,e) => check_expr(e,vars,params),
        Expr::CompoundGlobal(_,_,_,_,e) => check_expr(e,vars,params),
        Expr::PostGlobal(..) => Ok(()),
        Expr::Neg(a) | Expr::Normalize(_, a) => check_expr(a, vars, params),
        Expr::Add(a,b) | Expr::Sub(a,b) | Expr::Mul(a,b) |
        Expr::BitAnd(a,b) | Expr::BitOr(a,b) | Expr::BitXor(a,b) |
        Expr::Compare(_,a,b) | Expr::UnsignedCompare(_,a,b) |
        Expr::IntegerBinary(_,a,b) => {
            check_expr(a, vars, params)?;
            check_expr(b, vars, params)
        }
        Expr::Sequence(a,b) => {
            check_expr(a, vars, params)?;
            check_expr(b, vars, params)
        }
        Expr::AssignLocal(i,_,e) => {
            check_expr(e, vars, params)?;
            vars.insert(*i);
            Ok(())
        }
        Expr::CompoundLocal(i,_,_,_,e) => {
            if !vars.contains(i) { return Err(invalid("compound use of uninitialized local")); }
            check_expr(e, vars, params)
        }
        Expr::PostLocal(i,..) => {
            if vars.contains(i) { Ok(()) }
            else { Err(invalid("postincrement of uninitialized local")) }
        }
        Expr::Conditional(a,b,c) => {
            check_expr(a, vars, params)?;
            let mut yes = vars.clone();
            let mut no = vars.clone();
            check_expr(b, &mut yes, params)?;
            check_expr(c, &mut no, params)?;
            *vars = yes.intersection(&no).copied().collect();
            Ok(())
        }
        Expr::Select(a,b,c) => {
            check_expr(a, vars, params)?;
            check_expr(b, vars, params)?;
            check_expr(c, vars, params)
        }
        Expr::StoreLocal(..) => Err(invalid("legacy local assignment unsupported in CFG")),
    }
}
// Returns true if execution can reach the end of this sequence.
fn check(
    body: &[Stmt], vars: &mut HashSet<usize>, params: usize,
    locals: usize, loop_depth: usize,
) -> Result<bool, CodegenError> {
    let mut live = true;
    for stmt in body {
        if !live { return Err(invalid("unreachable statement")); }
        match stmt {
            Stmt::Break | Stmt::Continue => {
                if loop_depth == 0 { return Err(invalid("break/continue outside loop")); }
                live = false;
            }
            Stmt::Store(i,e) => {
                if *i >= locals { return Err(invalid("invalid local index")); }
                check_expr(e, vars, params)?;
                vars.insert(*i);
            }
            Stmt::Eval(e) => { check_expr(e, vars, params)?; }
            Stmt::Return(e) => { check_expr(e, vars, params)?; live = false; }
            Stmt::ReturnVoid => { live = false; }
            Stmt::If(e, yes, no) => {
                check_expr(e, vars, params)?;
                let mut a = vars.clone();
                let mut b = vars.clone();
                let ra = check(yes, &mut a, params, locals, loop_depth)?;
                let rb = check(no, &mut b, params, locals, loop_depth)?;
                match (ra, rb) {
                    (true, true) => *vars = a.intersection(&b).copied().collect(),
                    (true, false) => *vars = a,
                    (false, true) => *vars = b,
                    (false, false) => live = false,
                }
            }
            Stmt::While(e, inner) => {
                check_expr(e, vars, params)?;
                let mut nested = vars.clone();
                check(inner, &mut nested, params, locals, loop_depth + 1)?;
                // A while may execute zero times. Do not propagate updates.
            }
            Stmt::For { init, condition, increment, body } => {
                if !check(init, vars, params, locals, loop_depth)? {
                    live = false;
                    continue;
                }
                check_expr(condition, vars, params)?;
                let mut nested = vars.clone();
                check(body, &mut nested, params, locals, loop_depth + 1)?;
                // Continue paths can execute the increment. Require that all
                // its reads are safe even without any body-local writes.
                let mut increment_state = vars.clone();
                check(increment, &mut increment_state, params, locals, loop_depth + 1)?;
            }
            Stmt::DoWhile(body, condition) => {
                let mut nested = vars.clone();
                check(body, &mut nested, params, locals, loop_depth + 1)?;
                // Conservative: a continue path can reach the test without
                // writes made only by other paths of the body.
                check_expr(condition, vars, params)?;
                // A break can reach the successor, so keep only entry facts.
            }
        }
    }
    Ok(live)
}

/// C integer promotions and usual arithmetic conversions for ordinary
/// IL scalar types supported by the reference backend. EDG exposes widths
/// and signedness, so rank is unambiguous for 8/16/32/64-bit builtins on
/// our LP64 target. Values smaller than int always promote to signed int.
fn promoted_integer_type(t: CAbiInt) -> CAbiInt {
    if t.bits < 32 {
        CAbiInt { bits: 32, signed: true, is_bool: false }
    } else {
        CAbiInt { is_bool: false, ..t }
    }
}
fn common_integer_type(lhs: CAbiInt, rhs: CAbiInt) -> CAbiInt {
    let a = promoted_integer_type(lhs);
    let b = promoted_integer_type(rhs);
    let bits = a.bits.max(b.bits);
    let signed = if a.signed == b.signed {
        a.signed
    } else if a.bits == b.bits {
        false
    } else if a.bits > b.bits {
        a.signed
    } else {
        b.signed
    };
    CAbiInt { bits, signed, is_bool: false }
}

/// Retrieve native, non-enum integer type information for a C expression.
/// EDG already inserts the usual arithmetic conversion nodes. Never guess
/// signedness from the result of a comparison (which is boolean).
fn integer_expression_type(il: &Il, graph: &Graph, id: ExpressionId)
    -> Result<CAbiInt, CodegenError>
{
    let node = graph.expression(id).ok_or_else(|| invalid("missing typed expression"))?;
    let ty = node.data.type_id.ok_or_else(|| invalid(format!(
        "missing EDG expression type at {}:{}:{}",
        node.data.function, node.data.statement_path, node.data.path)))?;
    let canonical = canonical_abi_type(il, ty)?;
    if il.types.iter().any(|v| v.id == canonical &&
                             matches!(semantic(4,v.kind),5|6)) {
        return Ok(CAbiInt { bits:64, signed:false, is_bool:false });
    }
    let ty = il.c_integer_types.iter().find(|v| v.id == canonical)
        .ok_or_else(|| invalid(format!(
            "unsupported C expression type at {}:{}:{}",
            node.data.function, node.data.statement_path, node.data.path)))?;
    Ok(CAbiInt { bits: ty.bits, signed: ty.signed, is_bool: ty.is_bool })
}

/// Every EDG operation has a result type, and each operand already carries
/// explicit integer promotions and usual arithmetic conversions. Track
/// those conversions rather than evaluating all expressions as signed i64.
/// Resolve a scalar local lvalue. No address-taken, pointer or field writes.
#[derive(Clone, Copy)]
enum ScalarPlace {
    Ssa(usize),
    Memory(crate::SymbolId),
}

/// Global symbols are resolved by their EDG identities, not guessed from a
/// variable's spelling. Qualified, thread-local, and function-local static
/// declarations require a different storage model and are refused.
fn scalar_global(
    il: &Il, symbol: crate::SymbolId,
) -> Result<(String, CAbiInt), CodegenError> {
    let meta = il.global_variables.iter().find(|g| g.symbol == symbol)
        .ok_or_else(|| invalid(format!("unresolved/nonlocal variable {}",symbol.0)))?;
    if meta.thread_local || meta.local_to_function ||
       il.static_local_owners.iter().any(|o| o.symbol == symbol) {
        return Err(invalid("TLS or function-local static global unsupported"));
    }
    let name = il.symbols.iter().find(|s| s.id == symbol && s.kind == 2)
        .map(|s| s.name.clone())
        .ok_or_else(|| invalid("global has no variable symbol"))?;
    if name.is_empty() ||
       !name.bytes().next().is_some_and(|c| c.is_ascii_alphabetic() || c == b'_') ||
       !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_') {
        return Err(invalid(format!("non-C or ambiguous global linkage name: {name}")));
    }
    let mut ty = il.symbol_types.iter().find(|s| s.symbol == symbol)
        .ok_or_else(|| invalid("global has no declared type"))?.ty;
    for _ in 0..=il.types.len() {
        if let Some(d) = il.type_details.iter().find(|d| d.owner == ty) {
            if d.is_reference || d.is_rvalue_reference || d.qualifiers.unwrap_or(0) != 0 {
                return Err(invalid(format!("qualified global {name} needs memory semantics")));
            }
        }
        let def = il.types.iter().find(|d| d.id == ty)
            .ok_or_else(|| invalid("unresolved global type"))?;
        if semantic(4, def.kind) != 3 { break; }
        ty = def.related.ok_or_else(|| invalid("invalid global typedef"))?;
    }
    let ty = canonical_abi_type(il,ty)?;
    let integer = il.c_integer_types.iter().find(|x| x.id == ty)
        .ok_or_else(|| invalid(format!("unsupported global scalar type: {name}")))?;
    Ok((name,CAbiInt { bits: integer.bits, signed: integer.signed,
                       is_bool: integer.is_bool }))
}
fn scalar_lvalue(
    graph: &Graph, id: ExpressionId, params: &[crate::SymbolId],
    locals: &[&crate::LocalVariable], il: &Il,
) -> Result<(ScalarPlace,CAbiInt), CodegenError> {
    let n = graph.expression(id).ok_or_else(|| invalid("missing scalar lvalue"))?;
    if semantic(2,n.data.kind) != 2 || !n.operands.is_empty() {
        return Err(invalid("assignment target must be direct scalar variable"));
    }
    let symbol = n.data.symbol_id.ok_or_else(|| invalid("missing assignment symbol"))?;
    if let Some(i) = params.iter().position(|p| *p == symbol) {
        return Ok((ScalarPlace::Ssa(locals.len()+i),integer_expression_type(il,graph,id)?));
    }
    if let Some(index) = locals.iter().position(|v| v.symbol == symbol) {
        let ty = cfg_local_type(il,locals[index])?;
        return Ok((ScalarPlace::Ssa(index),ty));
    }
    let (_,ty) = scalar_global(il,symbol)?;
    Ok((ScalarPlace::Memory(symbol),ty))
}
/// Byte size of a complete EDG C type, bounded before multiplication.
/// Aggregate record/union/vector types remain unsupported. An array size
/// must come from the native EDG bound, not inferred from C source tokens.
fn byte_size(il: &Il, ty: crate::TypeId) -> Result<u64,CodegenError> {
    let ty = canonical_abi_type(il,ty)?;
    let def = il.types.iter().find(|t| t.id == ty)
        .ok_or_else(|| invalid("missing memory type"))?;
    match semantic(4,def.kind) {
        1 => {
            let info = il.c_integer_types.iter().find(|t| t.id == ty)
                .ok_or_else(|| invalid("unsupported integer memory layout"))?;
            Ok(u64::from(info.bits/8))
        }
        5 => Ok(8),
        6 => {
            let element = def.related.ok_or_else(|| invalid("array has no element type"))?;
            let count = il.array_bounds.iter().find(|b| b.array == ty)
                .ok_or_else(|| invalid("array has no fixed bound"))?.elements;
            let n = byte_size(il,element)?;
            n.checked_mul(count).filter(|&n| n > 0 && n <= 16*1024*1024)
                .ok_or_else(|| invalid("array object too large"))
        }
        _ => Err(invalid("unsupported aggregate/element size")),
    }
}
fn pointer_stride(il:&Il, ty:crate::TypeId) -> Result<u64,CodegenError> {
    let ty = canonical_abi_type(il,ty)?;
    let def = il.types.iter().find(|t| t.id == ty)
        .ok_or_else(|| invalid("missing pointer type"))?;
    match semantic(4,def.kind) {
        5 | 6 => byte_size(il,def.related.ok_or_else(|| invalid("missing pointed-to type"))?),
        _ => Err(invalid("expected pointer/array type for subscript")),
    }
}
fn expression_type_id(graph:&Graph,id:ExpressionId) -> Result<crate::TypeId,CodegenError> {
    graph.expression(id).and_then(|n| n.data.type_id)
        .ok_or_else(|| invalid("missing typed pointer expression"))
}
fn is_pointer_like(il:&Il,ty:crate::TypeId) -> Result<bool,CodegenError> {
    let ty=canonical_abi_type(il,ty)?;
    Ok(il.types.iter().any(|t|t.id==ty && matches!(semantic(4,t.kind),5|6)))
}
/// Decode an addressable scalar or array designator; no implicit load.
/// For subscript, EDG specifies that pointer and index may appear in either
/// operand position. The stride is the size of the pointer's element type.
fn lower_address(
    graph:&Graph,id:ExpressionId,params:&[crate::SymbolId],
    locals:&[&crate::LocalVariable],il:&Il,
) -> Result<Expr,CodegenError> {
    let node=graph.expression(id).ok_or_else(||invalid("missing lvalue"))?;
    match semantic(2,node.data.kind) {
        2 => {
            let sym=node.data.symbol_id.ok_or_else(||invalid("lvalue without symbol"))?;
            if let Some(i)=params.iter().position(|v|*v==sym) {
                return Ok(Expr::AddressLocal(locals.len()+i));
            }
            if let Some(i)=locals.iter().position(|v|v.symbol==sym) {
                return Ok(Expr::AddressLocal(i));
            }
            // Globals require a validated EDG storage identity.
            let meta=il.global_variables.iter().find(|g|g.symbol==sym)
                .ok_or_else(||invalid("address of non-storage symbol"))?;
            if meta.thread_local || meta.local_to_function {
                return Err(invalid("unsupported TLS/static-local address"));
            }
            Ok(Expr::AddressGlobal(sym))
        }
        3 => {
            let op=node.data.operator.map(|o|semantic(3,o)).unwrap_or(0);
            match op {
                44 if node.operands.len()==1 =>
                    lower_c_expression(graph,node.operands[0],params,locals,il),
                45 if node.operands.len()==2 => {
                    let a=node.operands[0]; let b=node.operands[1];
                    let (pointer,index)=if is_pointer_like(il,expression_type_id(graph,a)?)? {(a,b)}
                        else if is_pointer_like(il,expression_type_id(graph,b)?)? {(b,a)}
                        else { return Err(invalid("subscript has no pointer operand")); };
                    let stride=pointer_stride(il,expression_type_id(graph,pointer)?)?;
                    let base=lower_c_expression(graph,pointer,params,locals,il)?;
                    let index=lower_c_expression(graph,index,params,locals,il)?;
                    Ok(Expr::PointerOffset(Box::new(base),Box::new(index),stride))
                }
                46 if node.operands.len()==1 =>
                    lower_address(graph,node.operands[0],params,locals,il),
                11 if node.operands.len()==1 =>
                    lower_address(graph,node.operands[0],params,locals,il),
                _ => Err(invalid("unsupported addressable expression")),
            }
        }
        _ => Err(invalid("unsupported non-variable lvalue")),
    }
}
fn lower_c_expression(
    graph: &Graph, id: ExpressionId, params: &[crate::SymbolId],
    locals: &[&crate::LocalVariable], il: &Il,
) -> Result<Expr, CodegenError> {
    let node = graph.expression(id).ok_or_else(|| invalid("missing expression"))?;
    let output = integer_expression_type(il, graph, id)?;
    let wrap = |e| Expr::Normalize(output, Box::new(e));
    let inner = match semantic(2, node.data.kind) {
        1 => Expr::Constant(node.data.constant_integer
            .ok_or_else(|| invalid("unsupported/non-i64-representable constant"))?),
        2 => {
            let symbol = node.data.symbol_id.ok_or_else(|| invalid("unresolved scalar variable"))?;
            if let Some(i) = params.iter().position(|p| *p == symbol) {
                Expr::Local(locals.len()+i)
            } else {
                if let Some(i) = locals.iter().position(|v| v.symbol == symbol) {
                    cfg_local_type(il, locals[i])?;
                    Expr::Local(i)
                } else {
                    scalar_global(il,symbol)?;
                    Expr::Global(symbol)
                }
            }
        }
        3 => {
            let op = node.data.operator.map(|v| semantic(3, v)).unwrap_or(0);
            let operands = &node.operands;
            if matches!(op, 4 | 11 | 12 | 16 | 17 | 27 | 28 | 29 | 30 | 41 | 42) {
                if operands.len() != 1 {
                    return Err(invalid("unary expression requires exactly one operand"));
                }
                if matches!(op,29|30|41|42) {
                    let (place,ty) = scalar_lvalue(graph,operands[0],params,locals,il)?;
                    if matches!(op,29|30) {
                        match place {
                            ScalarPlace::Ssa(i) => Expr::PostLocal(i,ty,if op==29 {1} else {-1}),
                            ScalarPlace::Memory(s) => Expr::PostGlobal(s,ty,if op==29 {1} else {-1}),
                        }
                    } else {
                        let op = if op==41 {CCompoundOp::Add} else {CCompoundOp::Sub};
                        match place {
                            ScalarPlace::Ssa(i) => Expr::CompoundLocal(i,ty,promoted_integer_type(ty),
                                op,Box::new(Expr::Constant(1))),
                            ScalarPlace::Memory(s) => Expr::CompoundGlobal(s,ty,promoted_integer_type(ty),
                                op,Box::new(Expr::Constant(1))),
                        }
                    }
                } else {
                let a = lower_c_expression(graph, operands[0], params, locals, il)?;
                match op {
                    4 => Expr::Neg(Box::new(a)),
                    11 | 12 | 16 => a,
                    17 => Expr::Compare(CompareOp::NotEqual,
                        Box::new(a), Box::new(Expr::Constant(0))),
                    27 => Expr::BitXor(Box::new(a), Box::new(Expr::Constant(-1))),
                    28 => Expr::Compare(CompareOp::Equal,
                        Box::new(a), Box::new(Expr::Constant(0))),
                    _ => unreachable!(),
                }
                }
            } else if op == 25 {
                if operands.len() != 3 {
                    return Err(invalid("conditional expression needs three operands"));
                }
                Expr::Conditional(
                    Box::new(lower_c_expression(graph, operands[0], params, locals, il)?),
                    Box::new(lower_c_expression(graph, operands[1], params, locals, il)?),
                    Box::new(lower_c_expression(graph, operands[2], params, locals, il)?),
                )
            } else if op == 18 || (31..=40).contains(&op) {
                if operands.len() != 2 {
                    return Err(invalid("assignment expression requires 2 operands"));
                }
                let (place,ty) = scalar_lvalue(graph,operands[0],params,locals,il)?;
                let rhs = Box::new(lower_c_expression(
                    graph,operands[1],params,locals,il)?);
                if op==18 {
                    match place {
                        ScalarPlace::Ssa(i) => Expr::AssignLocal(i,ty,rhs),
                        ScalarPlace::Memory(s) => Expr::AssignGlobal(s,ty,rhs),
                    }
                } else {
                    let kind = match op {
                        31 => CCompoundOp::Add, 32 => CCompoundOp::Sub,
                        33 => CCompoundOp::Mul, 34 => CCompoundOp::Div,
                        35 => CCompoundOp::Rem, 36 => CCompoundOp::Shl,
                        37 => CCompoundOp::Shr, 38 => CCompoundOp::And,
                        39 => CCompoundOp::Or, 40 => CCompoundOp::Xor,
                        _ => unreachable!(),
                    };
                    let rhs_ty = integer_expression_type(il,graph,operands[1])?;
                    let compute_ty = if matches!(kind,CCompoundOp::Shl|CCompoundOp::Shr) {
                        promoted_integer_type(ty)
                    } else {
                        common_integer_type(ty,rhs_ty)
                    };
                    match place {
                        ScalarPlace::Ssa(i) => Expr::CompoundLocal(i,ty,compute_ty,kind,rhs),
                        ScalarPlace::Memory(s) => Expr::CompoundGlobal(s,ty,compute_ty,kind,rhs),
                    }
                }
            } else if matches!(op, 23 | 24 | 26) {
                if operands.len() != 2 {
                    return Err(invalid("logical/comma expression needs two operands"));
                }
                let a = Box::new(lower_c_expression(graph, operands[0], params, locals, il)?);
                let b = Box::new(lower_c_expression(graph, operands[1], params, locals, il)?);
                match op {
                    23 => Expr::Conditional(a,
                        Box::new(Expr::Compare(CompareOp::NotEqual, b,
                            Box::new(Expr::Constant(0)))),
                        Box::new(Expr::Constant(0))),
                    24 => Expr::Conditional(a,
                        Box::new(Expr::Constant(1)),
                        Box::new(Expr::Compare(CompareOp::NotEqual, b,
                            Box::new(Expr::Constant(0))))),
                    26 => Expr::Sequence(a,b),
                    _ => unreachable!(),
                }
            } else {
                // Reject unknown operators before checking conversions.
                // Shifts promote both operands independently; the right
                // operand's promoted type need not match the left one.
                if !matches!(op, 1 | 2 | 3 | 5..=10 | 13 | 14 | 15 | 19..=22) {
                    return Err(invalid(format!(
                        "unsupported EDG operation at {}:{}:{}",
                        node.data.function, node.data.statement_path, node.data.path)));
                }
                if operands.len() != 2 {
                    return Err(invalid("binary expression requires two operands"));
                }
                let lhs_ty = integer_expression_type(il, graph, operands[0])?;
                let rhs_ty = integer_expression_type(il, graph, operands[1])?;
                // Arithmetic operations and comparisons are defined over a
                // common promoted integer type in EDG's typed expression IL.
                // Refuse missing conversions rather than silently comparing
                // signed and unsigned or using the wrong intermediate width.
                if !matches!(op, 21 | 22) && lhs_ty != rhs_ty {
                    return Err(invalid(format!(
                        "unconverted operand types at {}:{}:{} ({lhs_ty:?}, {rhs_ty:?})",
                        node.data.function, node.data.statement_path, node.data.path)));
                }
                if matches!(op, 1 | 2 | 3 | 13 | 14 | 15 | 19..=22)
                    && output != lhs_ty {
                    return Err(invalid("arithmetic result/operand types disagree"));
                }
                let a = Box::new(lower_c_expression(graph, operands[0], params, locals, il)?);
                let b = Box::new(lower_c_expression(graph, operands[1], params, locals, il)?);
                match op {
                    1 => Expr::Add(a,b),
                    2 => Expr::Sub(a,b),
                    3 => Expr::Mul(a,b),
                    19 => Expr::IntegerBinary(
                        if lhs_ty.signed { CIntBinaryOp::SignedDivide }
                        else { CIntBinaryOp::UnsignedDivide }, a, b),
                    20 => Expr::IntegerBinary(
                        if lhs_ty.signed { CIntBinaryOp::SignedRemainder }
                        else { CIntBinaryOp::UnsignedRemainder }, a, b),
                    21 => Expr::IntegerBinary(CIntBinaryOp::ShiftLeft, a, b),
                    22 => Expr::IntegerBinary(
                        if lhs_ty.signed { CIntBinaryOp::SignedShiftRight }
                        else { CIntBinaryOp::UnsignedShiftRight }, a, b),
                    5..=10 => {
                        let cmp = match op {
                            5 => CompareOp::Equal, 6 => CompareOp::NotEqual,
                            7 => CompareOp::Less, 8 => CompareOp::LessEqual,
                            9 => CompareOp::Greater, 10 => CompareOp::GreaterEqual,
                            _ => unreachable!(),
                        };
                        if lhs_ty.signed { Expr::Compare(cmp,a,b) }
                        else { Expr::UnsignedCompare(cmp,a,b) }
                    }
                    13 => Expr::BitAnd(a,b),
                    14 => Expr::BitOr(a,b),
                    15 => Expr::BitXor(a,b),
                    _ => return Err(invalid(format!(
                        "unsupported EDG operation at {}:{}:{}",
                        node.data.function, node.data.statement_path, node.data.path))),
                }
            }
        }
        _ => return Err(invalid(format!("unsupported typed EDG expression kind {}",
                                        node.data.kind))),
    };
    Ok(wrap(inner))
}

fn compile_il(il: &Il) -> Result<Vec<Routine>, CodegenError> {
    let graph = Graph::from_il(il).map_err(|e| invalid(format!("EDG graph: {e:?}")))?;
    let mut out = Vec::new();
    for entity in &il.entities {
        let crate::Entity::Routine { name, has_definition: true } = entity else { continue };
        let roots: Vec<_> = graph.roots(name).map(|(id,_)| id).collect();
        if roots.len() != 1 { return Err(invalid(format!("{name}: expected one root"))); }
        let routine = graph.routine_for_function(name).ok_or_else(|| invalid("missing routine id"))?;
        if il.function_parameters.iter().any(|p| p.routine == routine && p.implicit_this) {
            return Err(invalid("implicit this parameter unsupported"));
        }
        let mut args: Vec<_> = il.function_parameters.iter()
            .filter(|p| p.routine == routine).collect();
        args.sort_by_key(|p| p.position);
        if args.iter().enumerate().any(|(i,p)| p.position != Some(i as u32)) {
            return Err(invalid("non-contiguous function parameters"));
        }
        let abi = cfg_routine_abi(il, name, routine, &args)?;
        let ids: Vec<_> = args.iter().map(|p| p.symbol).collect();
        let locals: Vec<_> = il.local_variables.iter().filter(|v| v.routine == routine).collect();
        if locals.len() > 10_000 { return Err(invalid("too many locals")); }
        let mut local_types = locals.iter().map(|v| cfg_local_type(il,v))
            .collect::<Result<Vec<_>,_>>()?;
        local_types.extend(abi.parameters.iter().copied());
        let body = lower(&graph, roots[0], &ids, &locals, il)?;
        let initialized: HashSet<usize> =
            (locals.len()..(locals.len()+ids.len())).collect();
        let has_fallthrough = check(&body, &mut initialized.clone(),
            ids.len(), local_types.len(), 0)?;
        if has_fallthrough && abi.result.is_some() {
            return Err(invalid(format!("{name}: some path has no return")));
        }
        out.push(Routine { name: name.clone(), parameters: ids.len(),
            locals: locals.len(), body, abi: Some(abi),
            local_types: Some(local_types) });
    }
    Ok(out)
}
type GlobalValues = HashMap<crate::SymbolId,
    (cranelift_codegen::ir::GlobalValue, CAbiInt)>;
fn global_address(
    fb: &mut FunctionBuilder<'_>, globals: &GlobalValues, symbol: crate::SymbolId,
) -> Result<cranelift_codegen::ir::Value, CodegenError> {
    let (value, _) = globals.get(&symbol).ok_or_else(|| invalid(
        format!("unresolved external data symbol {}",symbol.0)))?;
    Ok(fb.ins().global_value(types::I64, *value))
}

/// Shared C compound operation semantics for SSA or memory-backed lvalues.
fn compound_value(
    fb: &mut FunctionBuilder<'_>, before: cranelift_codegen::ir::Value,
    after: cranelift_codegen::ir::Value, ty: CAbiInt, op: CCompoundOp,
) -> cranelift_codegen::ir::Value {
    match op {
        CCompoundOp::Add => fb.ins().iadd(before,after),
        CCompoundOp::Sub => fb.ins().isub(before,after),
        CCompoundOp::Mul => fb.ins().imul(before,after),
        CCompoundOp::And => fb.ins().band(before,after),
        CCompoundOp::Or => fb.ins().bor(before,after),
        CCompoundOp::Xor => fb.ins().bxor(before,after),
        CCompoundOp::Div => emit_integer_binary(
            if ty.signed {CIntBinaryOp::SignedDivide}
            else {CIntBinaryOp::UnsignedDivide},fb,before,after),
        CCompoundOp::Rem => emit_integer_binary(
            if ty.signed {CIntBinaryOp::SignedRemainder}
            else {CIntBinaryOp::UnsignedRemainder},fb,before,after),
        CCompoundOp::Shl => fb.ins().ishl(before,after),
        CCompoundOp::Shr => emit_integer_binary(
            if ty.signed {CIntBinaryOp::SignedShiftRight}
            else {CIntBinaryOp::UnsignedShiftRight},fb,before,after),
    }
}
fn emit_expr(
    e: &Expr, fb: &mut FunctionBuilder<'_>,
    params: &[cranelift_codegen::ir::Value], vars: &[Variable],
    globals: &GlobalValues,
) -> Result<cranelift_codegen::ir::Value, CodegenError> {
    Ok(match e {
        Expr::Constant(v) => fb.ins().iconst(types::I64, *v),
        Expr::Parameter(i) => *params.get(*i).ok_or_else(|| invalid("invalid parameter"))?,
        Expr::Local(i) => fb.use_var(*vars.get(*i).ok_or_else(|| invalid("invalid local"))?),
        Expr::Global(symbol) => {
            let (_,ty) = globals.get(symbol).ok_or_else(|| invalid("unknown global"))?;
            let address = global_address(fb,globals,*symbol)?;
            super::memory::load_integer(fb,address,*ty)
        }
        Expr::AssignGlobal(symbol,ty,rhs) => {
            let value = emit_expr(rhs,fb,params,vars,globals)?;
            let address = global_address(fb,globals,*symbol)?;
            super::memory::store_integer(fb,address,value,*ty)
        }
        Expr::PostGlobal(symbol,ty,delta) => {
            let address = global_address(fb,globals,*symbol)?;
            let old = super::memory::load_integer(fb,address,*ty);
            let delta = fb.ins().iconst(types::I64,*delta);
            let value = fb.ins().iadd(old,delta);
            super::memory::store_integer(fb,address,value,*ty);
            old
        }
        Expr::CompoundGlobal(symbol,ty,compute_ty,op,rhs) => {
            // Evaluate destination once, then RHS, then perform one store.
            let address = global_address(fb,globals,*symbol)?;
            let old = super::memory::load_integer(fb,address,*ty);
            let after = emit_expr(rhs,fb,params,vars,globals)?;
            let before = normalize_c_value(fb,old,*compute_ty);
            let after = if matches!(op,CCompoundOp::Shl|CCompoundOp::Shr) {
                after
            } else {
                normalize_c_value(fb,after,*compute_ty)
            };
            let result = compound_value(fb,before,after,*compute_ty,*op);
            super::memory::store_integer(fb,address,result,*ty)
        },
        Expr::StoreLocal(..) => return Err(invalid("nested assignment unsupported")),
        Expr::Normalize(ty, a) => {
            let value = emit_expr(a,fb,params,vars,globals)?;
            normalize_c_value(fb, value, *ty)
        }
        Expr::Neg(a) => {
            let v = emit_expr(a,fb,params,vars,globals)?;
            fb.ins().ineg(v)
        }
        Expr::IntegerBinary(op,a,b) => {
            let lhs = emit_expr(a,fb,params,vars,globals)?;
            let rhs = emit_expr(b,fb,params,vars,globals)?;
            emit_integer_binary(*op, fb, lhs, rhs)
        }
        Expr::Sequence(a,b) => {
            let _ = emit_expr(a,fb,params,vars,globals)?;
            emit_expr(b,fb,params,vars,globals)?
        }
        Expr::AssignLocal(index,ty,value) => {
            let raw = emit_expr(value,fb,params,vars,globals)?;
            let normalized = normalize_c_value(fb,raw,*ty);
            fb.def_var(*vars.get(*index).ok_or_else(|| invalid("invalid local assignment"))?,normalized);
            normalized
        }
        Expr::PostLocal(index,ty,delta) => {
            let var = *vars.get(*index).ok_or_else(|| invalid("invalid local increment"))?;
            let old = fb.use_var(var);
            let amount = fb.ins().iconst(types::I64,*delta);
            let next = fb.ins().iadd(old,amount);
            let normalized = normalize_c_value(fb,next,*ty);
            fb.def_var(var,normalized);
            old
        }
        Expr::CompoundLocal(index,ty,compute_ty,op,rhs) => {
            let var = *vars.get(*index).ok_or_else(|| invalid("invalid compound target"))?;
            let before = fb.use_var(var);
            let after = emit_expr(rhs,fb,params,vars,globals)?;
            let before = normalize_c_value(fb,before,*compute_ty);
            let after = if matches!(op,CCompoundOp::Shl|CCompoundOp::Shr) {
                after
            } else {
                normalize_c_value(fb,after,*compute_ty)
            };
            let result = compound_value(fb,before,after,*compute_ty,*op);
            let normalized = normalize_c_value(fb,result,*ty);
            fb.def_var(var,normalized);
            normalized
        }
        Expr::Conditional(c,a,b) => {
            let test = emit_expr(c,fb,params,vars,globals)?;
            let flag = fb.ins().icmp_imm(IntCC::NotEqual,test,0);
            let yes = fb.create_block();
            let no = fb.create_block();
            let merge = fb.create_block();
            fb.append_block_param(merge, types::I64);
            fb.ins().brif(flag, yes, &[], no, &[]);
            fb.seal_block(yes);
            fb.seal_block(no);
            fb.switch_to_block(yes);
            let yes_value = emit_expr(a,fb,params,vars,globals)?;
            fb.ins().jump(merge, &[yes_value.into()]);
            fb.switch_to_block(no);
            let no_value = emit_expr(b,fb,params,vars,globals)?;
            fb.ins().jump(merge, &[no_value.into()]);
            fb.seal_block(merge);
            fb.switch_to_block(merge);
            fb.block_params(merge)[0]
        }
        Expr::Add(a,b) | Expr::Sub(a,b) | Expr::Mul(a,b) |
        Expr::BitAnd(a,b) | Expr::BitOr(a,b) | Expr::BitXor(a,b) => {
            let a = emit_expr(a,fb,params,vars,globals)?;
            let b = emit_expr(b,fb,params,vars,globals)?;
            match e {
                Expr::Add(..) => fb.ins().iadd(a,b),
                Expr::Sub(..) => fb.ins().isub(a,b),
                Expr::Mul(..) => fb.ins().imul(a,b),
                Expr::BitAnd(..) => fb.ins().band(a,b),
                Expr::BitOr(..) => fb.ins().bor(a,b),
                Expr::BitXor(..) => fb.ins().bxor(a,b),
                _ => unreachable!(),
            }
        }
        Expr::Compare(op,a,b) | Expr::UnsignedCompare(op,a,b) => {
            let unsigned = matches!(e, Expr::UnsignedCompare(..));
            let a = emit_expr(a,fb,params,vars,globals)?;
            let b = emit_expr(b,fb,params,vars,globals)?;
            let cc = match (op, unsigned) {
                (CompareOp::Equal, _) => IntCC::Equal,
                (CompareOp::NotEqual, _) => IntCC::NotEqual,
                (CompareOp::Less, false) => IntCC::SignedLessThan,
                (CompareOp::LessEqual, false) => IntCC::SignedLessThanOrEqual,
                (CompareOp::Greater, false) => IntCC::SignedGreaterThan,
                (CompareOp::GreaterEqual, false) => IntCC::SignedGreaterThanOrEqual,
                (CompareOp::Less, true) => IntCC::UnsignedLessThan,
                (CompareOp::LessEqual, true) => IntCC::UnsignedLessThanOrEqual,
                (CompareOp::Greater, true) => IntCC::UnsignedGreaterThan,
                (CompareOp::GreaterEqual, true) => IntCC::UnsignedGreaterThanOrEqual,
            };
            let test = fb.ins().icmp(cc,a,b);
            let one = fb.ins().iconst(types::I64,1);
            let zero = fb.ins().iconst(types::I64,0);
            fb.ins().select(test,one,zero)
        }
        Expr::Select(c,a,b) => {
            let test = emit_expr(c,fb,params,vars,globals)?;
            let a = emit_expr(a,fb,params,vars,globals)?;
            let b = emit_expr(b,fb,params,vars,globals)?;
            let nonzero = fb.ins().icmp_imm(IntCC::NotEqual,test,0);
            fb.ins().select(nonzero,a,b)
        }
    })
}
#[derive(Clone, Copy)]
struct LoopTargets<'a> {
    break_to: Block,
    continue_to: Block,
    saw_break: &'a Cell<bool>,
    saw_continue: &'a Cell<bool>,
}
fn emit_body(
    stmts: &[Stmt], fb: &mut FunctionBuilder<'_>,
    args: &[cranelift_codegen::ir::Value], vars: &[Variable],
    outer: Option<LoopTargets<'_>>,
    local_types: &[CAbiInt], result: Option<CAbiInt>,
    globals: &GlobalValues,
) -> Result<bool, CodegenError> {
    let mut live = true;
    for stmt in stmts {
        if !live { return Err(invalid("unreachable statement")); }
        match stmt {
            Stmt::Break => {
                let dest = outer.ok_or_else(|| invalid("break outside loop"))?;
                fb.ins().jump(dest.break_to, &[]);
                dest.saw_break.set(true);
                live = false;
            }
            Stmt::Continue => {
                let dest = outer.ok_or_else(|| invalid("continue outside loop"))?;
                fb.ins().jump(dest.continue_to, &[]);
                dest.saw_continue.set(true);
                live = false;
            }
            Stmt::Store(i,e) => {
                let v = emit_expr(e,fb,args,vars,globals)?;
                let ty = *local_types.get(*i).ok_or_else(|| invalid("bad local type"))?;
                let v = normalize_c_value(fb, v, ty);
                fb.def_var(*vars.get(*i).ok_or_else(|| invalid("bad local"))?, v);
            }
            Stmt::Eval(e) => { let _ = emit_expr(e,fb,args,vars,globals)?; }
            Stmt::Return(e) => {
                let ty = result.ok_or_else(|| invalid("value returned from void routine"))?;
                let v = emit_expr(e,fb,args,vars,globals)?;
                let v = if ty.bits == 64 { v }
                    else { fb.ins().ireduce(ty.cranelift(), v) };
                fb.ins().return_(&[v]);
                live = false;
            }
            Stmt::ReturnVoid => {
                if result.is_some() { return Err(invalid("missing non-void return value")); }
                fb.ins().return_(&[]);
                live = false;
            }
            Stmt::If(cond, yes, no) => {
                let c = emit_expr(cond,fb,args,vars,globals)?;
                let flag = fb.ins().icmp_imm(IntCC::NotEqual,c,0);
                let t = fb.create_block();
                let f = fb.create_block();
                fb.ins().brif(flag,t,&[],f,&[]);
                fb.seal_block(t);
                fb.seal_block(f);
                fb.switch_to_block(t);
                let t_live = emit_body(yes,fb,args,vars,outer,local_types,result,globals)?;
                let mut merge = None;
                if t_live {
                    let m = fb.create_block();
                    fb.ins().jump(m,&[]);
                    merge = Some(m);
                }
                fb.switch_to_block(f);
                let f_live = emit_body(no,fb,args,vars,outer,local_types,result,globals)?;
                if f_live {
                    let m = if let Some(m) = merge { m } else {
                        let m = fb.create_block();
                        merge = Some(m);
                        m
                    };
                    fb.ins().jump(m,&[]);
                }
                if let Some(m) = merge {
                    fb.seal_block(m);
                    fb.switch_to_block(m);
                } else { live = false; }
            }
            Stmt::While(cond, body) => {
                let head = fb.create_block();
                let body_block = fb.create_block();
                let exit = fb.create_block();
                fb.ins().jump(head,&[]);
                fb.switch_to_block(head);
                let c = emit_expr(cond,fb,args,vars,globals)?;
                let flag = fb.ins().icmp_imm(IntCC::NotEqual,c,0);
                fb.ins().brif(flag,body_block,&[],exit,&[]);
                fb.seal_block(body_block);
                fb.switch_to_block(body_block);
                let saw_break = Cell::new(false);
                let saw_continue = Cell::new(false);
                let targets = LoopTargets {
                    break_to: exit, continue_to: head,
                    saw_break: &saw_break, saw_continue: &saw_continue,
                };
                if emit_body(body,fb,args,vars,Some(targets),local_types,result,globals)? {
                    fb.ins().jump(head,&[]);
                }
                // Backedges and break edges are complete.
                fb.seal_block(head);
                fb.seal_block(exit);
                fb.switch_to_block(exit);
            }
            Stmt::For { init, condition, increment, body } => {
                if !emit_body(init,fb,args,vars,outer,local_types,result,globals)? {
                    live = false;
                    continue;
                }
                let head = fb.create_block();
                let body_block = fb.create_block();
                let step = fb.create_block();
                let exit = fb.create_block();
                fb.ins().jump(head, &[]);
                fb.switch_to_block(head);
                let c = emit_expr(condition,fb,args,vars,globals)?;
                let flag = fb.ins().icmp_imm(IntCC::NotEqual, c, 0);
                fb.ins().brif(flag, body_block, &[], exit, &[]);
                fb.seal_block(body_block);
                fb.switch_to_block(body_block);
                let saw_break = Cell::new(false);
                let saw_continue = Cell::new(false);
                let targets = LoopTargets {
                    break_to: exit, continue_to: step,
                    saw_break: &saw_break, saw_continue: &saw_continue,
                };
                let body_live = emit_body(body,fb,args,vars,Some(targets),local_types,result,globals)?;
                if body_live { fb.ins().jump(step, &[]); }
                fb.seal_block(step);
                if body_live || saw_continue.get() {
                    fb.switch_to_block(step);
                    if emit_body(increment,fb,args,vars,Some(targets),local_types,result,globals)? {
                        fb.ins().jump(head, &[]);
                    }
                }
                fb.seal_block(head);
                fb.seal_block(exit);
                fb.switch_to_block(exit);
            }
            Stmt::DoWhile(body, cond) => {
                let body_block = fb.create_block();
                let test_block = fb.create_block();
                let exit = fb.create_block();
                fb.ins().jump(body_block, &[]);
                fb.switch_to_block(body_block);
                let saw_break = Cell::new(false);
                let saw_continue = Cell::new(false);
                let targets = LoopTargets {
                    break_to: exit, continue_to: test_block,
                    saw_break: &saw_break, saw_continue: &saw_continue,
                };
                let body_live = emit_body(body,fb,args,vars,Some(targets),local_types,result,globals)?;
                if body_live { fb.ins().jump(test_block, &[]); }
                fb.seal_block(test_block);
                if body_live || saw_continue.get() {
                    fb.switch_to_block(test_block);
                    let test = emit_expr(cond,fb,args,vars,globals)?;
                    let flag = fb.ins().icmp_imm(IntCC::NotEqual, test, 0);
                    fb.ins().brif(flag, body_block, &[], exit, &[]);
                }
                fb.seal_block(body_block);
                fb.seal_block(exit);
                if body_live || saw_continue.get() || saw_break.get() {
                    fb.switch_to_block(exit);
                } else {
                    live = false;
                }
            }
        }
    }
    Ok(live)
}
#[derive(Clone)]
struct ScalarGlobal {
    symbol: crate::SymbolId,
    name: String,
    ty: CAbiInt,
    linkage: Linkage,
    initializer: Option<Vec<u8>>,
}

/// Resolve only referenced C globals. Unused declarations of unrelated
/// C++ types cannot make an otherwise scalar routine un-compilable.
fn compile_globals(il: &Il) -> Result<Vec<ScalarGlobal>, CodegenError> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for expr in &il.expressions {
        if semantic(2, expr.kind) != 2 { continue; }
        let Some(id) = expr.symbol_id else { continue };
        if !il.global_variables.iter().any(|g| g.symbol == id) ||
           !seen.insert(id) { continue; }
        let (name,ty) = scalar_global(il,id)?;
        let meta = il.global_variables.iter().find(|g| g.symbol == id)
            .ok_or_else(|| invalid("missing global metadata"))?;
        let defined = meta.linkage != 1 || meta.explicit_initializer;
        let linkage = if !defined { Linkage::Import }
            else if meta.linkage == 2 { Linkage::Local }
            else { Linkage::Export };
        let initializer = if meta.explicit_initializer {
            let entry = il.initializer_integers.iter().find(|v| v.symbol == id)
                .ok_or_else(|| invalid(format!(
                    "global {name} has unsupported non-integer initializer")))?;
            let bytes = entry.value.to_le_bytes();
            Some(bytes[..(ty.bits/8) as usize].to_vec())
        } else { None };
        result.push(ScalarGlobal { symbol:id,name,ty,linkage,initializer });
    }
    Ok(result)
}
#[cfg(test)]
fn emit_routines(functions: &[Routine]) -> Result<Vec<u8>, CodegenError> {
    emit_routines_with_globals(functions,&[])
}
fn emit_routines_with_globals(
    functions: &[Routine], globals: &[ScalarGlobal],
) -> Result<Vec<u8>, CodegenError> {
    let mut flags = settings::builder();
    flags.set("opt_level","speed").map_err(|e| CodegenError::Backend(e.to_string()))?;
    let isa = cranelift_codegen::isa::lookup_by_name("x86_64")
        .map_err(|e| CodegenError::UnsupportedTarget(e.to_string()))?
        .finish(settings::Flags::new(flags))
        .map_err(|e| CodegenError::UnsupportedTarget(e.to_string()))?;
    let builder = ObjectBuilder::new(isa,"edgcpp",cranelift_module::default_libcall_names())
        .map_err(|e| CodegenError::Backend(e.to_string()))?;
    let mut module = ObjectModule::new(builder);
    let mut declared = HashMap::new();
    for g in globals {
        let id = module.declare_data(&g.name,g.linkage,true,false)
            .map_err(|e| CodegenError::Backend(e.to_string()))?;
        if g.linkage != Linkage::Import {
            let mut data = cranelift_module::DataDescription::new();
            if let Some(bytes) = &g.initializer {
                data.define(bytes.clone().into_boxed_slice());
            } else {
                data.define_zeroinit((g.ty.bits/8) as usize);
            }
            module.define_data(id,&data)
                .map_err(|e| CodegenError::Backend(e.to_string()))?;
        }
        if declared.insert(g.symbol,(id,g.ty)).is_some() {
            return Err(invalid("duplicate global identity"));
        }
    }
    for func in functions {
        if func.name.is_empty() || func.name.as_bytes().contains(&0) {
            return Err(invalid("invalid export name"));
        }
        let mut context = module.make_context();
        let abi = func.abi.clone().unwrap_or_else(|| CAbi {
            parameters: vec![CAbiInt { bits: 64, signed: true, is_bool: false }; func.parameters],
            result: Some(CAbiInt { bits: 64, signed: true, is_bool: false }),
        });
        if abi.parameters.len() != func.parameters {
            return Err(invalid("ABI parameter count mismatch"));
        }
        for param in &abi.parameters {
            context.func.signature.params.push(AbiParam::new(param.cranelift()));
        }
        if let Some(result) = abi.result {
            context.func.signature.returns.push(AbiParam::new(result.cranelift()));
        }
        let id = module.declare_function(&func.name,Linkage::Export,&context.func.signature)
            .map_err(|e| CodegenError::Backend(e.to_string()))?;
        let global_values: GlobalValues = declared.iter().map(|(symbol,(id,ty))|
            (*symbol,(module.declare_data_in_func(*id,&mut context.func),*ty))).collect();
        let mut fb_context = FunctionBuilderContext::new();
        {
            let mut fb = FunctionBuilder::new(&mut context.func,&mut fb_context);
            let entry = fb.create_block();
            fb.append_block_params_for_function_params(entry);
            fb.switch_to_block(entry);
            fb.seal_block(entry);
            let params: Vec<_> = fb.block_params(entry).to_vec().into_iter()
                .zip(&abi.parameters).map(|(value, ty)| {
                    if ty.bits == 64 { value }
                    else if ty.signed { fb.ins().sextend(types::I64, value) }
                    else { fb.ins().uextend(types::I64, value) }
                }).collect();
            let local_types = func.local_types.clone().unwrap_or_else(|| {
                vec![CAbiInt { bits: 64, signed: true, is_bool: false }; func.locals]
            });
            if local_types.len() != func.locals + if func.abi.is_some() {
                func.parameters
            } else { 0 } {
                return Err(invalid("local/parameter SSA slot count mismatch"));
            }
            let vars: Vec<_> = (0..local_types.len()).map(|i| {
                let v = Variable::from_u32(i as u32);
                fb.declare_var(v,types::I64);
                v
            }).collect();
            if func.abi.is_some() {
                for (i,value) in params.iter().enumerate() {
                    fb.def_var(vars[func.locals+i],*value);
                }
            }
            let body = emit_body(
                &func.body, &mut fb, &params, &vars, None, &local_types, abi.result,
                &global_values)?;
            if body {
                if abi.result.is_none() {
                    fb.ins().return_(&[]);
                } else {
                    return Err(invalid("fallthrough in code generation"));
                }
            }
            fb.finalize();
        }
        module.define_function(id,&mut context).map_err(|e| CodegenError::Backend(e.to_string()))?;
        module.clear_context(&mut context);
    }
    module.finish().emit().map_err(|e| CodegenError::Backend(e.to_string()))
}
pub(super) fn emit_edg_cfg_object(il: &Il) -> Result<Vec<u8>, CodegenError> {
    let routines = compile_il(il)?;
    let globals = compile_globals(il)?;
    emit_routines_with_globals(&routines,&globals)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn emits_loop_and_merges_mutable_local() {
        let body = vec![
            Stmt::Store(0,Expr::Parameter(0)),
            Stmt::While(
                Expr::Compare(CompareOp::Less,Box::new(Expr::Local(0)),
                    Box::new(Expr::Parameter(1))),
                vec![
                    Stmt::If(Expr::Local(0),
                        vec![Stmt::Store(0,Expr::Add(
                            Box::new(Expr::Local(0)),Box::new(Expr::Constant(1))))],
                        vec![Stmt::Store(0,Expr::Add(
                            Box::new(Expr::Local(0)),Box::new(Expr::Constant(2))))]),
                ],
            ),
            Stmt::Return(Expr::Local(0)),
        ];
        assert!(!check(&body,&mut HashSet::new(),2,1,0).unwrap());
        let object = emit_routines(&[Routine {
            name: "cfg_unit".into(), parameters: 2, locals: 1, body, abi: None, local_types: None,
        }]).unwrap();
        assert_eq!(&object[..4],b"\x7fELF");
    }
    #[test]
    fn emits_pre_and_post_test_loops_with_ssa() {
        let condition = Expr::Compare(
            CompareOp::Less, Box::new(Expr::Local(0)), Box::new(Expr::Parameter(1)));
        let increment = vec![Stmt::Store(0, Expr::Add(
            Box::new(Expr::Local(0)), Box::new(Expr::Constant(1))))];
        let body = vec![
            Stmt::Store(0, Expr::Parameter(0)),
            Stmt::Store(1, Expr::Constant(0)),
            Stmt::For { init: vec![], condition: condition.clone(),
                increment: increment.clone(),
                body: vec![Stmt::Store(1, Expr::Add(
                    Box::new(Expr::Local(1)), Box::new(Expr::Local(0))))] },
            Stmt::DoWhile(vec![Stmt::Store(1, Expr::Add(
                Box::new(Expr::Local(1)), Box::new(Expr::Constant(1))))],
                Expr::Compare(CompareOp::Less, Box::new(Expr::Local(1)),
                    Box::new(Expr::Constant(5)))),
            Stmt::Return(Expr::Local(1)),
        ];
        assert!(!check(&body, &mut HashSet::new(), 2, 2, 0).unwrap());
        assert_eq!(&emit_routines(&[Routine {
            name: "for_and_do".into(), parameters: 2, locals: 2, body, abi: None, local_types: None,
        }]).unwrap()[..4], b"\x7fELF");
    }
    #[test]
    fn rejects_break_outside_loop_and_emits_continue_break() {
        assert!(check(&[Stmt::Break], &mut HashSet::new(), 0, 0, 0).is_err());
        let body = vec![
            Stmt::Store(0, Expr::Parameter(0)),
            Stmt::For {
                init: vec![],
                condition: Expr::Compare(CompareOp::Less,
                    Box::new(Expr::Local(0)), Box::new(Expr::Parameter(1))),
                increment: vec![Stmt::Store(0, Expr::Add(
                    Box::new(Expr::Local(0)), Box::new(Expr::Constant(1))))],
                body: vec![
                    Stmt::If(Expr::Compare(CompareOp::Equal,
                        Box::new(Expr::Local(0)), Box::new(Expr::Constant(0))),
                        vec![Stmt::Continue], vec![]),
                    Stmt::If(Expr::Compare(CompareOp::Greater,
                        Box::new(Expr::Local(0)), Box::new(Expr::Constant(5))),
                        vec![Stmt::Break], vec![]),
                ],
            },
            Stmt::Return(Expr::Local(0)),
        ];
        assert!(!check(&body, &mut HashSet::new(), 2, 1, 0).unwrap());
        assert_eq!(&emit_routines(&[Routine {
            name: "loop_jumps".into(), parameters: 2, locals: 1, body, abi: None, local_types: None,
        }]).unwrap()[..4], b"\x7fELF");
    }
    #[test]
    fn rejects_nondefinite_local_initialization() {
        let mut vars = HashSet::new();
        let body = vec![
            Stmt::If(Expr::Parameter(0),
                vec![Stmt::Store(0,Expr::Constant(1))],vec![]),
            Stmt::Return(Expr::Local(0)),
        ];
        assert!(check(&body,&mut vars,1,1,0).is_err());
    }
}
