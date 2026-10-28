//! EDG structured statements -> Cranelift CFG with typed C integer SSA locals.
use super::*;
use std::collections::HashSet;
use std::cell::Cell;
use cranelift_codegen::ir::Block;
use cranelift_frontend::Variable;

#[derive(Debug, Clone)]
enum Stmt {
    Store(usize, Expr),
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
    let info = il.c_integer_types.iter().find(|t| t.id == ty)
        .ok_or_else(|| invalid(format!("local {} has unsupported scalar type",local.symbol.0)))?;
    Ok(CAbiInt { bits: info.bits, signed: info.signed })
}
fn cfg_checked_local(
    il: &Il, locals: &[&crate::LocalVariable], symbol: crate::SymbolId,
) -> Result<usize, CodegenError> {
    let index = locals.iter().position(|v| v.symbol == symbol)
        .ok_or_else(|| invalid(format!("unsupported/nonlocal variable {}",symbol.0)))?;
    cfg_local_type(il, locals[index])?;
    Ok(index)
}
fn normalize_c_value(
    fb: &mut FunctionBuilder<'_>, value: cranelift_codegen::ir::Value, ty: CAbiInt,
) -> cranelift_codegen::ir::Value {
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
            let root = node.expression.ok_or_else(|| invalid("missing assignment"))?;
            let op = graph.expression(root).ok_or_else(|| invalid("missing assignment root"))?;
            if semantic(2, op.data.kind) != 3 ||
                op.data.operator.map(|o| semantic(3, o)) != Some(18) ||
                op.operands.len() != 2 { return Err(invalid("only simple local assignment supported")); }
            let lhs = graph.expression(op.operands[0])
                .ok_or_else(|| invalid("missing assignment destination"))?;
            if semantic(2, lhs.data.kind) != 2 || !lhs.operands.is_empty() {
                return Err(invalid("assignment target must be scalar local"));
            }
            let symbol = lhs.data.symbol_id.ok_or_else(|| invalid("unresolved assignment target"))?;
            let i = cfg_checked_local(il, locals, symbol)?;
            Ok(vec![Stmt::Store(i, expr(op.operands[1])?)])
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
fn check_expr(e: &Expr, vars: &HashSet<usize>, params: usize) -> Result<(), CodegenError> {
    match e {
        Expr::Constant(_) => Ok(()),
        Expr::Parameter(i) => if *i < params { Ok(()) } else { Err(invalid("invalid parameter")) },
        Expr::Local(i) => if vars.contains(i) { Ok(()) }
            else { Err(invalid(format!("local {i} read before initialization"))) },
        Expr::Neg(a) | Expr::Normalize(_, a) => check_expr(a, vars, params),
        Expr::Add(a,b) | Expr::Sub(a,b) | Expr::Mul(a,b) |
        Expr::BitAnd(a,b) | Expr::BitOr(a,b) | Expr::BitXor(a,b) |
        Expr::Compare(_,a,b) | Expr::UnsignedCompare(_,a,b) => {
            check_expr(a, vars, params)?;
            check_expr(b, vars, params)
        }
        Expr::Select(a,b,c) => {
            check_expr(a, vars, params)?;
            check_expr(b, vars, params)?;
            check_expr(c, vars, params)
        }
        Expr::StoreLocal(..) => Err(invalid("side effects in expression unsupported")),
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
    let ty = il.c_integer_types.iter().find(|v| v.id == canonical)
        .ok_or_else(|| invalid(format!(
            "unsupported C expression type at {}:{}:{}",
            node.data.function, node.data.statement_path, node.data.path)))?;
    Ok(CAbiInt { bits: ty.bits, signed: ty.signed })
}

/// Every EDG operation has a result type, and each operand already carries
/// explicit integer promotions and usual arithmetic conversions. Track
/// those conversions rather than evaluating all expressions as signed i64.
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
                Expr::Parameter(i)
            } else {
                Expr::Local(cfg_checked_local(il, locals, symbol)?)
            }
        }
        3 => {
            let op = node.data.operator.map(|v| semantic(3, v)).unwrap_or(0);
            let operands = &node.operands;
            if matches!(op, 4 | 11 | 12 | 16 | 17) {
                if operands.len() != 1 {
                    return Err(invalid("unary expression requires exactly one operand"));
                }
                let a = lower_c_expression(graph, operands[0], params, locals, il)?;
                match op {
                    4 => Expr::Neg(Box::new(a)),
                    11 | 12 | 16 => a,
                    17 => Expr::Compare(CompareOp::NotEqual,
                        Box::new(a), Box::new(Expr::Constant(0))),
                    _ => unreachable!(),
                }
            } else {
                // Reject unknown operators before enforcing the usual
                // arithmetic-conversion rules. Shifts and division do not
                // necessarily have equal operand types, so applying binary
                // arithmetic assumptions first gives misleading diagnostics.
                if !matches!(op, 1 | 2 | 3 | 5..=10 | 13 | 14 | 15) {
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
                if lhs_ty != rhs_ty {
                    return Err(invalid(format!(
                        "unconverted operand types at {}:{}:{} ({lhs_ty:?}, {rhs_ty:?})",
                        node.data.function, node.data.statement_path, node.data.path)));
                }
                if matches!(op, 1 | 2 | 3 | 13 | 14 | 15) && output != lhs_ty {
                    return Err(invalid("arithmetic result/operand types disagree"));
                }
                let a = Box::new(lower_c_expression(graph, operands[0], params, locals, il)?);
                let b = Box::new(lower_c_expression(graph, operands[1], params, locals, il)?);
                match op {
                    1 => Expr::Add(a,b),
                    2 => Expr::Sub(a,b),
                    3 => Expr::Mul(a,b),
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
        let local_types = locals.iter().map(|v| cfg_local_type(il,v))
            .collect::<Result<Vec<_>,_>>()?;
        let body = lower(&graph, roots[0], &ids, &locals, il)?;
        let has_fallthrough = check(&body, &mut HashSet::new(), ids.len(), locals.len(), 0)?;
        if has_fallthrough && abi.result.is_some() {
            return Err(invalid(format!("{name}: some path has no return")));
        }
        out.push(Routine { name: name.clone(), parameters: ids.len(),
            locals: locals.len(), body, abi: Some(abi),
            local_types: Some(local_types) });
    }
    Ok(out)
}
fn emit_expr(
    e: &Expr, fb: &mut FunctionBuilder<'_>,
    params: &[cranelift_codegen::ir::Value], vars: &[Variable],
) -> Result<cranelift_codegen::ir::Value, CodegenError> {
    Ok(match e {
        Expr::Constant(v) => fb.ins().iconst(types::I64, *v),
        Expr::Parameter(i) => *params.get(*i).ok_or_else(|| invalid("invalid parameter"))?,
        Expr::Local(i) => fb.use_var(*vars.get(*i).ok_or_else(|| invalid("invalid local"))?),
        Expr::StoreLocal(..) => return Err(invalid("nested assignment unsupported")),
        Expr::Normalize(ty, a) => {
            let value = emit_expr(a, fb, params, vars)?;
            normalize_c_value(fb, value, *ty)
        }
        Expr::Neg(a) => {
            let v = emit_expr(a, fb, params, vars)?;
            fb.ins().ineg(v)
        }
        Expr::Add(a,b) | Expr::Sub(a,b) | Expr::Mul(a,b) |
        Expr::BitAnd(a,b) | Expr::BitOr(a,b) | Expr::BitXor(a,b) => {
            let a = emit_expr(a, fb, params, vars)?;
            let b = emit_expr(b, fb, params, vars)?;
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
            let a = emit_expr(a, fb, params, vars)?;
            let b = emit_expr(b, fb, params, vars)?;
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
            let test = emit_expr(c, fb, params, vars)?;
            let a = emit_expr(a, fb, params, vars)?;
            let b = emit_expr(b, fb, params, vars)?;
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
                let v = emit_expr(e, fb, args, vars)?;
                let ty = *local_types.get(*i).ok_or_else(|| invalid("bad local type"))?;
                let v = normalize_c_value(fb, v, ty);
                fb.def_var(*vars.get(*i).ok_or_else(|| invalid("bad local"))?, v);
            }
            Stmt::Return(e) => {
                let ty = result.ok_or_else(|| invalid("value returned from void routine"))?;
                let v = emit_expr(e, fb, args, vars)?;
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
                let c = emit_expr(cond, fb, args, vars)?;
                let flag = fb.ins().icmp_imm(IntCC::NotEqual,c,0);
                let t = fb.create_block();
                let f = fb.create_block();
                fb.ins().brif(flag,t,&[],f,&[]);
                fb.seal_block(t);
                fb.seal_block(f);
                fb.switch_to_block(t);
                let t_live = emit_body(yes, fb, args, vars, outer, local_types, result)?;
                let mut merge = None;
                if t_live {
                    let m = fb.create_block();
                    fb.ins().jump(m,&[]);
                    merge = Some(m);
                }
                fb.switch_to_block(f);
                let f_live = emit_body(no, fb, args, vars, outer, local_types, result)?;
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
                let c = emit_expr(cond, fb, args, vars)?;
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
                if emit_body(body,fb,args,vars,Some(targets),local_types,result)? {
                    fb.ins().jump(head,&[]);
                }
                // Backedges and break edges are complete.
                fb.seal_block(head);
                fb.seal_block(exit);
                fb.switch_to_block(exit);
            }
            Stmt::For { init, condition, increment, body } => {
                if !emit_body(init, fb, args, vars, outer, local_types, result)? {
                    live = false;
                    continue;
                }
                let head = fb.create_block();
                let body_block = fb.create_block();
                let step = fb.create_block();
                let exit = fb.create_block();
                fb.ins().jump(head, &[]);
                fb.switch_to_block(head);
                let c = emit_expr(condition, fb, args, vars)?;
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
                let body_live = emit_body(body, fb, args, vars, Some(targets), local_types, result)?;
                if body_live { fb.ins().jump(step, &[]); }
                fb.seal_block(step);
                if body_live || saw_continue.get() {
                    fb.switch_to_block(step);
                    if emit_body(increment, fb, args, vars, Some(targets), local_types, result)? {
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
                let body_live = emit_body(body, fb, args, vars, Some(targets), local_types, result)?;
                if body_live { fb.ins().jump(test_block, &[]); }
                fb.seal_block(test_block);
                if body_live || saw_continue.get() {
                    fb.switch_to_block(test_block);
                    let test = emit_expr(cond, fb, args, vars)?;
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
fn emit_routines(functions: &[Routine]) -> Result<Vec<u8>, CodegenError> {
    let mut flags = settings::builder();
    flags.set("opt_level","speed").map_err(|e| CodegenError::Backend(e.to_string()))?;
    let isa = cranelift_codegen::isa::lookup_by_name("x86_64")
        .map_err(|e| CodegenError::UnsupportedTarget(e.to_string()))?
        .finish(settings::Flags::new(flags))
        .map_err(|e| CodegenError::UnsupportedTarget(e.to_string()))?;
    let builder = ObjectBuilder::new(isa,"edgcpp",cranelift_module::default_libcall_names())
        .map_err(|e| CodegenError::Backend(e.to_string()))?;
    let mut module = ObjectModule::new(builder);
    for func in functions {
        if func.name.is_empty() || func.name.as_bytes().contains(&0) {
            return Err(invalid("invalid export name"));
        }
        let mut context = module.make_context();
        let abi = func.abi.clone().unwrap_or_else(|| CAbi {
            parameters: vec![CAbiInt { bits: 64, signed: true }; func.parameters],
            result: Some(CAbiInt { bits: 64, signed: true }),
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
            let vars: Vec<_> = (0..func.locals).map(|i| {
                let v = Variable::from_u32(i as u32);
                fb.declare_var(v,types::I64);
                v
            }).collect();
            let local_types = func.local_types.clone().unwrap_or_else(|| {
                vec![CAbiInt { bits: 64, signed: true }; func.locals]
            });
            if local_types.len() != func.locals {
                return Err(invalid("local type count mismatch"));
            }
            let body = emit_body(
                &func.body, &mut fb, &params, &vars, None, &local_types, abi.result)?;
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
    emit_routines(&compile_il(il)?)
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
