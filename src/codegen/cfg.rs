//! EDG structured statements -> Cranelift CFG with SSA for signed-i64 locals.
use super::*;
use std::collections::HashSet;
use cranelift_frontend::Variable;

#[derive(Debug, Clone)]
enum Stmt {
    Store(usize, Expr),
    Return(Expr),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
}
struct Routine { name: String, parameters: usize, locals: usize, body: Vec<Stmt> }
fn invalid(msg: impl Into<String>) -> CodegenError { CodegenError::InvalidFunction(msg.into()) }

fn lower(
    graph: &Graph, id: crate::graph::StatementId,
    params: &[crate::SymbolId], locals: &[&crate::LocalVariable], il: &Il,
) -> Result<Vec<Stmt>, CodegenError> {
    let node = graph.statement(id).ok_or_else(|| invalid("missing statement"))?;
    let path = &node.data.path;
    let function = &node.data.function;
    let expr = |id| lower_expression(graph, id, params, locals, il);
    match semantic(1, node.data.kind) {
        1 => {
            let id = node.expression.ok_or_else(|| invalid("missing return expression"))?;
            if !node.children.is_empty() { return Err(invalid("return with nested statements")); }
            Ok(vec![Stmt::Return(expr(id)?)])
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
                checked_local(il, locals, decl.symbol)?;
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
            let id = node.expression.ok_or_else(|| invalid("initializer has no integer expression"))?;
            Ok(vec![Stmt::Store(checked_local(il, locals, symbol)?, expr(id)?)])
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
            let i = checked_local(il, locals, symbol)?;
            Ok(vec![Stmt::Store(i, expr(op.operands[1])?)])
        }
        7 => {
            let condition = expr(node.expression.ok_or_else(|| invalid("while has no condition"))?)?;
            if node.children.len() != 1 { return Err(invalid("while body not unique")); }
            let child = node.children[0];
            let body = graph.statement(child).ok_or_else(|| invalid("missing while body"))?;
            if !body.data.path.starts_with(&format!("{path}.body.")) {
                return Err(invalid("unexpected while body"));
            }
            Ok(vec![Stmt::While(condition, lower(graph, child, params, locals, il)?)])
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
        Expr::Neg(a) => check_expr(a, vars, params),
        Expr::Add(a,b) | Expr::Sub(a,b) | Expr::Mul(a,b) |
        Expr::BitAnd(a,b) | Expr::BitOr(a,b) | Expr::BitXor(a,b) |
        Expr::Compare(_,a,b) => {
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
    body: &[Stmt], vars: &mut HashSet<usize>, params: usize, locals: usize,
) -> Result<bool, CodegenError> {
    let mut live = true;
    for stmt in body {
        if !live { return Err(invalid("unreachable statement")); }
        match stmt {
            Stmt::Store(i,e) => {
                if *i >= locals { return Err(invalid("invalid local index")); }
                check_expr(e, vars, params)?;
                vars.insert(*i);
            }
            Stmt::Return(e) => { check_expr(e, vars, params)?; live = false; }
            Stmt::If(e, yes, no) => {
                check_expr(e, vars, params)?;
                let mut a = vars.clone();
                let mut b = vars.clone();
                let ra = check(yes, &mut a, params, locals)?;
                let rb = check(no, &mut b, params, locals)?;
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
                check(inner, &mut nested, params, locals)?;
                // Zero iterations are possible: don't propagate loop-only definitions.
            }
        }
    }
    Ok(live)
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
        validate_routine_abi(il, name, routine, &args)?;
        let ids: Vec<_> = args.iter().map(|p| p.symbol).collect();
        let locals: Vec<_> = il.local_variables.iter().filter(|v| v.routine == routine).collect();
        if locals.len() > 10_000 { return Err(invalid("too many locals")); }
        let body = lower(&graph, roots[0], &ids, &locals, il)?;
        if check(&body, &mut HashSet::new(), ids.len(), locals.len())? {
            return Err(invalid(format!("{name}: some path has no return")));
        }
        out.push(Routine { name: name.clone(), parameters: ids.len(),
            locals: locals.len(), body });
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
        Expr::Compare(op,a,b) => {
            let a = emit_expr(a, fb, params, vars)?;
            let b = emit_expr(b, fb, params, vars)?;
            let cc = match op {
                CompareOp::Equal => IntCC::Equal, CompareOp::NotEqual => IntCC::NotEqual,
                CompareOp::Less => IntCC::SignedLessThan,
                CompareOp::LessEqual => IntCC::SignedLessThanOrEqual,
                CompareOp::Greater => IntCC::SignedGreaterThan,
                CompareOp::GreaterEqual => IntCC::SignedGreaterThanOrEqual,
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
fn emit_body(
    stmts: &[Stmt], fb: &mut FunctionBuilder<'_>,
    args: &[cranelift_codegen::ir::Value], vars: &[Variable],
) -> Result<bool, CodegenError> {
    let mut live = true;
    for stmt in stmts {
        if !live { return Err(invalid("unreachable statement")); }
        match stmt {
            Stmt::Store(i,e) => {
                let v = emit_expr(e, fb, args, vars)?;
                fb.def_var(*vars.get(*i).ok_or_else(|| invalid("bad local"))?, v);
            }
            Stmt::Return(e) => {
                let v = emit_expr(e, fb, args, vars)?;
                fb.ins().return_(&[v]);
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
                let t_live = emit_body(yes, fb, args, vars)?;
                let mut merge = None;
                if t_live {
                    let m = fb.create_block();
                    fb.ins().jump(m,&[]);
                    merge = Some(m);
                }
                fb.switch_to_block(f);
                let f_live = emit_body(no, fb, args, vars)?;
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
                fb.seal_block(exit);
                fb.switch_to_block(body_block);
                if emit_body(body,fb,args,vars)? { fb.ins().jump(head,&[]); }
                // Seal only after the backedge, allowing loop-carried SSA values.
                fb.seal_block(head);
                fb.switch_to_block(exit);
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
        for _ in 0..func.parameters { context.func.signature.params.push(AbiParam::new(types::I64)); }
        context.func.signature.returns.push(AbiParam::new(types::I64));
        let id = module.declare_function(&func.name,Linkage::Export,&context.func.signature)
            .map_err(|e| CodegenError::Backend(e.to_string()))?;
        let mut fb_context = FunctionBuilderContext::new();
        {
            let mut fb = FunctionBuilder::new(&mut context.func,&mut fb_context);
            let entry = fb.create_block();
            fb.append_block_params_for_function_params(entry);
            fb.switch_to_block(entry);
            fb.seal_block(entry);
            let params = fb.block_params(entry).to_vec();
            let vars: Vec<_> = (0..func.locals).map(|i| {
                let v = Variable::from_u32(i as u32);
                fb.declare_var(v,types::I64);
                v
            }).collect();
            if emit_body(&func.body,&mut fb,&params,&vars)? {
                return Err(invalid("fallthrough in code generation"));
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
        assert!(!check(&body,&mut HashSet::new(),2,1).unwrap());
        let object = emit_routines(&[Routine {
            name: "cfg_unit".into(), parameters: 2, locals: 1, body,
        }]).unwrap();
        assert_eq!(&object[..4],b"\x7fELF");
    }
    #[test]
    fn rejects_nondefinite_local_initialization() {
        let mut vars = HashSet::new();
        let body = vec![
            Stmt::If(Expr::Parameter(0),
                vec![Stmt::Store(0,Expr::Constant(1))],vec![]),
            Stmt::Return(Expr::Local(0)),
        ];
        assert!(check(&body,&mut vars,1,1).is_err());
    }
}
