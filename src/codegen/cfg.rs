//! EDG structured statements -> Cranelift CFG with typed C integer SSA locals.
use super::*;
use std::collections::{HashSet,HashMap};
use std::cell::Cell;
use cranelift_codegen::ir::{Block,StackSlot,StackSlotData,StackSlotKind,MemFlags};
use cranelift_frontend::Variable;

#[derive(Debug, Clone)]
enum Stmt {
    Store(usize, Expr),
    /// EDG-confirmed complete zero initialization of a fixed-size aggregate.
    ZeroAggregate(usize, u32),
    /// Complete native byte image of a constant local aggregate.
    InitializeAggregate(usize, Vec<u8>),
    Eval(Expr),
    Return(Expr),
    /// Return a native 9-16-byte INTEGER aggregate in RAX/RDX. The argument
    /// evaluates to the address of an existing C object, not its first word.
    ReturnAggregate(u8, Expr),
    ReturnVoid,
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    /// Condition is checked before the body; increment runs after the body.
    For { init: Vec<Stmt>, condition: Option<Expr>, increment: Vec<Stmt>, body: Vec<Stmt> },
    /// Test is at the bottom, so the body runs at least once.
    DoWhile(Vec<Stmt>, Expr),
    Break,
    Continue,
    /// User-visible C goto label, addressed by the owned EDG symbol identity.
    Goto(crate::SymbolId),
    /// A destination in the current function, not a separate frontend CFG.
    Label(crate::SymbolId),
    /// Integer-switch arms use EDG's original C case constants.
    Switch(Expr,Vec<Stmt>),
    Case(Option<i64>),
    BreakSwitch,
}
struct Routine {
    name: String, parameters: usize, locals: usize, body: Vec<Stmt>,
    /// None on existing standalone unit-test routines (signed-i64 ABI).
    abi: Option<CAbi>,
    /// None only for legacy signed-i64 unit-test routines.
    local_types: Option<Vec<CAbiInt>>,
    /// Exact storage sizes for address-taken locals and local arrays.
    local_storage: Option<Vec<u32>>,
    /// Cranelift stack-slot alignment shifts, one per local/parameter.
    local_alignments: Option<Vec<u8>>,
    /// Qualified function parameters have automatic C storage semantics.
    parameter_accesses: Vec<MemoryAccess>,
}
fn invalid(msg: impl Into<String>) -> CodegenError { CodegenError::InvalidFunction(msg.into()) }

/// Allow compiler-generated scalar locals only in the *specific* lowered
/// EDG VLA runtime path, where the source frontend has already inserted
/// allocation and deallocation calls. Do not reinterpret arbitrary C++
/// compiler temporaries as scalar values.
fn has_lowered_vla_runtime(il:&Il,routine:crate::SymbolId) -> bool {
    let Some(function)=il.routine_identities.iter()
        .find(|r|r.routine==routine).map(|r|r.function.as_str()) else {return false;};
    let mut alloc=false;
    let mut dealloc=false;
    for expr in &il.expressions {
        if expr.function!=function {continue;}
        match expr.reference.as_deref() {
            Some("__vla_alloc") => alloc=true,
            Some("__vla_dealloc") => dealloc=true,
            _=>{},
        }
    }
    alloc && dealloc
}
/// CFG-only scalar local validation. Ordinary C++ generated temporaries
/// still need explicit lifetime and value-category handling.
fn cfg_local_type(il: &Il, local: &crate::LocalVariable) -> Result<CAbiInt, CodegenError> {
    let lowered_vla=has_lowered_vla_runtime(il,local.routine);
    if local.static_storage || local.structured_binding || local.handler_parameter ||
        (local.compiler_generated && !lowered_vla) {
        return Err(invalid(format!(
            "unsupported local storage for {} (routine={}, static={}, generated={}, structured={}, handler={}, vla_helpers={})",
            local.symbol.0,local.routine.0,local.static_storage,
            local.compiler_generated,local.structured_binding,
            local.handler_parameter,lowered_vla)));
    }
    // Scalar qualifiers hidden by typedefs are authoritative: volatile and
    // C11 atomic automatic locals must live in addressable storage, never SSA.
    let access=scalar_memory_access(il,local.ty)?;
    let ty = canonical_abi_type(il, local.ty)?;
    if access!=MemoryAccess::Ordinary {
        let kind=il.types.iter().find(|t|t.id==ty)
            .map(|t|semantic(4,t.kind))
            .ok_or_else(||invalid("qualified local has no canonical type"))?;
        let bytes=byte_size(il,local.ty)?;
        let align=storage_alignment(il,local.ty)?;
        if kind!=1 || !matches!(bytes,1|2|4|8) || align<bytes {
            return Err(invalid("qualified automatic local needs naturally aligned integer scalar"));
        }
    }
    // Lowered VLA helpers introduce integer dimension temporaries and pointer
    // storage. Compiler-generated aggregates are never accepted here.
    if local.compiler_generated && !il.types.iter().any(|d| d.id == ty &&
        matches!(semantic(4,d.kind),1|5)) {
        return Err(invalid("unsupported generated VLA temporary type"));
    }
    if il.types.iter().any(|d| d.id == ty && matches!(semantic(4,d.kind),5|6|7|9)) {
        // Records are stack-addressed aggregates, never scalar SSA values.
        // Keep the placeholder ABI type solely for local slot bookkeeping.
        if il.types.iter().any(|d| d.id == ty && matches!(semantic(4,d.kind),7|9)) {
            byte_size(il,ty)?;
        }
        return Ok(CAbiInt { bits:64, signed:false, is_bool:false });
    }
    let info = il.c_integer_types.iter().find(|t| t.id == ty)
        .ok_or_else(|| invalid(format!("local {} has unsupported scalar type",local.symbol.0)))?;
    Ok(CAbiInt { bits: info.bits, signed: info.signed, is_bool: info.is_bool })
}
/// The ABI erases top-level parameter qualifiers, but their automatic
/// object reads/writes must still obey the source declaration.
fn parameter_memory_access(
    il:&Il,symbol:crate::SymbolId,
) -> Result<MemoryAccess,CodegenError> {
    let arg=il.function_parameters.iter().find(|p|p.symbol==symbol)
        .ok_or_else(||invalid("function parameter missing native declaration type"))?;
    let access=scalar_memory_access(il,arg.ty)?;
    if access!=MemoryAccess::Ordinary {
        let base=canonical_abi_type(il,arg.ty)?;
        let kind=il.types.iter().find(|t|t.id==base)
            .map(|t|semantic(4,t.kind))
            .ok_or_else(||invalid("missing qualified parameter type"))?;
        let bytes=byte_size(il,arg.ty)?;
        let alignment=storage_alignment(il,arg.ty)?;
        if kind!=1 || !matches!(bytes,1|2|4|8) || alignment<bytes {
            return Err(invalid("qualified formal requires naturally aligned integer scalar"));
        }
    }
    Ok(access)
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

fn is_gnu_child(graph:&Graph,id:crate::graph::StatementId,parent:&str) -> bool {
    graph.statement(id).is_some_and(|n|n.data.path.starts_with(
        &format!("{parent}.gnu")))
}
/// Validate the EDG statement tree belonging to one GNU ({...}) expression.
/// Only straight-line, scoped scalar statements are accepted here. Branches,
/// jumps, nontrivial lifetimes and nested control flow are intentionally
/// rejected, rather than assuming a source macro has no side effects.
fn lower_gnu_block(
    graph:&Graph, id:crate::graph::StatementId,
    params:&[crate::SymbolId], locals:&[&crate::LocalVariable], il:&Il,
) -> Result<Expr,CodegenError> {
    let root=graph.statement(id).ok_or_else(||invalid("missing GNU block"))?;
    if semantic(1,root.data.kind)!=2 {
        return Err(invalid("GNU expression does not own a statement block"));
    }
    let mut effects:Vec<Expr>=Vec::new();
    let mut result:Option<Expr>=None;
    // EDG may append synthetic .gnu descendants after the result statement.
    // Only semantic children count when enforcing the final-value position.
    let semantic_children:Vec<_>=root.children.iter().copied()
        .filter(|&child|!is_gnu_child(graph,child,&root.data.path)).collect();
    for (position,&child) in semantic_children.iter().enumerate() {
        let node=graph.statement(child).ok_or_else(||invalid("missing GNU statement"))?;
        if result.is_some() {return Err(invalid("GNU block statements after result"));}
        match semantic(1,node.data.kind) {
            4 => {let body=lower(graph,child,params,locals,il)?;
                if !body.is_empty() {return Err(invalid("executable GNU declaration"));}}
            5 => {
                for action in lower(graph,child,params,locals,il)? {
                    match action {
                        Stmt::Store(i,e) => {
                            let ty=if i<locals.len() {cfg_local_type(il,locals[i])?}
                                else {
                                    let index=i-locals.len();
                                    let sym=*params.get(index).ok_or_else(||invalid("GNU param index"))?;
                                    let declared=il.symbol_types.iter().find(|s|s.symbol==sym)
                                        .ok_or_else(||invalid("GNU param has no type"))?.ty;
                                    call_type_abi(il,declared)?
                                };
                            if i<locals.len() {
                                let access=scalar_memory_access(il,locals[i].ty)?;
                                if access!=MemoryAccess::Ordinary {
                                    effects.push(Expr::StoreQualified(ty,access,
                                        Box::new(Expr::AddressLocal(i)),Box::new(e)));
                                    continue;
                                }
                            }
                            effects.push(Expr::AssignLocal(i,ty,Box::new(e)));
                        }
                        _ => return Err(invalid("non-scalar GNU initializer")),
                    }
                }
            }
            6 => {
                if node.children.iter().any(|&c|!is_gnu_child(graph,c,&node.data.path)) {
                    return Err(invalid("GNU expression has unexpected children"));
                }
                let expr=node.expression.ok_or_else(||invalid("GNU evaluation is missing"))?;
                effects.push(lower_c_expression(graph,expr,params,locals,il)?);
            }
            12 => {
                let expr=node.expression.ok_or_else(||invalid(
                    "GNU statement-expression result is not scalar"))?;
                result=Some(lower_c_expression(graph,expr,params,locals,il)?);
                if position+1!=semantic_children.len() {
                    return Err(invalid("GNU result is not last statement"));
                }
            }
            _ => return Err(invalid(format!(
                "GNU block has unsupported statement kind {}",node.data.kind))),
        }
    }
    Ok(Expr::BlockValue(effects,Box::new(result.ok_or_else(||
        invalid("GNU statement-expression missing final value"))?)))
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
            if node.children.iter().any(|&c|!is_gnu_child(graph,c,path)) {
                return Err(invalid("return with nested statements"));
            }
            if let Some(id) = node.expression {
                if let Some(size)=target::ABI.classify_aggregate(il,expression_type_id(graph,id)?)? {
                    let expression=graph.expression(id)
                        .ok_or_else(||invalid("missing returned aggregate expression"))?;
                    let direct_call=graph.calls.iter().any(|site|
                        site.function==expression.data.function &&
                        site.statement_path==expression.data.statement_path &&
                        site.path==expression.data.path);
                    if direct_call {
                        // A small aggregate call yields an INTEGER scalar;
                        // a two-eightbyte call yields a temporary object
                        // address after materializing both return registers.
                        let returned=expr(id)?;
                        if size>8 {
                            Ok(vec![Stmt::ReturnAggregate(size,returned)])
                        } else {
                            Ok(vec![Stmt::Return(returned)])
                        }
                    } else {
                        let address=lower_address(graph,id,params,locals,il)?;
                        if size>8 {
                            Ok(vec![Stmt::ReturnAggregate(size,address)])
                        } else {
                            Ok(vec![Stmt::Return(Expr::PackAggregate(size,Box::new(address)))])
                        }
                    }
                } else {
                    Ok(vec![Stmt::Return(expr(id)?)])
                }
            } else {
                Ok(vec![Stmt::ReturnVoid])
            }
        }
        2 => {
            if node.expression.is_some() { return Err(invalid("block has an expression")); }
            let mut body = Vec::new();
            for &child in &node.children {
                if is_gnu_child(graph,child,path) {continue;}
                body.extend(lower(graph, child, params, locals, il)?);
            }
            Ok(body)
        }
        3 => {
            let condition = expr(node.expression.ok_or_else(|| invalid("if has no condition"))?)?;
            let mut yes = None;
            let mut no = None;
            for &child in &node.children {
                if is_gnu_child(graph,child,path) {continue;}
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
            if node.expression.is_some() ||
               node.children.iter().any(|&c|!is_gnu_child(graph,c,path)) {
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
            if init.role != 0 ||
               node.children.iter().any(|&c|!is_gnu_child(graph,c,path)) {
                return Err(invalid("unsupported initializer"));
            }
            let symbol = init.symbol.ok_or_else(|| invalid("initializer has no target"))?;
            let index=cfg_checked_local(il,locals,symbol)?;
            let local_ty=canonical_abi_type(il,locals[index].ty)?;
            let local_kind=semantic(4,il.types.iter()
                .find(|t|t.id==local_ty).ok_or_else(||
                    invalid("initializer has no local type definition"))?.kind);
            if matches!(local_kind,6|7|9) {
                let matching=il.statement_aggregate_initializers.iter()
                    .filter(|v|v.function==*function &&
                        v.statement_path==*path && v.symbol==symbol)
                    .collect::<Vec<_>>();
                if matching.len()>1 {
                    return Err(invalid("duplicate native aggregate initializer"));
                }
                if let Some(image)=matching.first() {
                    if canonical_abi_type(il,image.ty)?!=local_ty ||
                       image.bytes.len()!=byte_size(il,local_ty)? as usize ||
                       image.bytes.is_empty() || image.bytes.len()>4096 {
                        return Err(invalid("aggregate native image/type size mismatch"));
                    }
                    return Ok(vec![Stmt::InitializeAggregate(index,image.bytes.clone())]);
                }
                // The scalar initialization path must never write just one
                // word into a larger array/struct. Only the exact native
                // full-object zero-init kind is safe without an initializer
                // tree. Partial/braced constant aggregates remain rejected.
                if semantic(5,init.kind)!=1 || node.expression.is_some() {
                    return Err(invalid(format!("{function}:{path}: aggregate initializer requires structured lowering (native kind={}, braced={}, image_present={})", init.kind, init.braced, !matching.is_empty())));
                }
                let bytes=byte_size(il,local_ty)?;
                if bytes==0 || bytes>4096 {
                    return Err(invalid("aggregate zero initializer exceeds 4 KiB limit"));
                }
                return Ok(vec![Stmt::ZeroAggregate(index,bytes as u32)]);
            }
            let value = if let Some(id) = node.expression {
                expr(id)?
            } else if let Some(reference) = {
                let matches:Vec<_>=il.statement_routine_initializers.iter()
                    .filter(|v|v.function==*function && v.statement_path==*path)
                    .collect();
                if matches.len()>1 {
                    return Err(invalid("multiple native routine initializers"));
                }
                matches.into_iter().next()
            } {
                if local_kind != 5 {
                    return Err(invalid("routine address initializer on non-pointer local"));
                }
                let target=direct_call_decl(il,reference.symbol)?;
                let declared=callable_type_abi(il,local_ty)?;
                if target.abi!=declared {
                    return Err(invalid("function pointer initializer has incompatible ABI"));
                }
                Expr::AddressFunction(reference.symbol)
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
            Ok(vec![Stmt::Store(index, value)])
        }
        6 => {
            if node.children.iter().any(|&c|!is_gnu_child(graph,c,path)) {
                return Err(invalid("expression statement has children"));
            }
            let root = node.expression.ok_or_else(|| invalid("missing expression"))?;
            Ok(vec![Stmt::Eval(expr(root)?)])
        }
        7 | 8 => {
            let condition = expr(node.expression.ok_or_else(|| invalid("while has no condition"))?)?;
            let body_children:Vec<_>=node.children.iter().copied()
                .filter(|&child|!is_gnu_child(graph,child,path)).collect();
            if body_children.len()!=1 {return Err(invalid("while body not unique"));}
            let child=body_children[0];
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
            // In C, a missing for-condition is an unconditional true test.
            // Keep the absence explicit so reachability recognizes a loop
            // with no exit, rather than introducing an artificial false edge.
            let condition = node.expression.map(expr).transpose()?;
            let mut init = None;
            let mut body = None;
            let mut increment = None;
            for &child in &node.children {
                if is_gnu_child(graph,child,path) {continue;}
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
        13 => {
            let selector=expr(node.expression.ok_or_else(||
                invalid("switch has no selector expression"))?)?;
            let children:Vec<_>=node.children.iter().copied()
                .filter(|&child|!is_gnu_child(graph,child,path)).collect();
            if children.len()!=1 ||
                !graph.statement(children[0]).is_some_and(|s|
                    s.data.path.starts_with(&format!("{path}.body."))) {
                return Err(invalid("switch body is not unique"));
            }
            let body=lower(graph,children[0],params,locals,il)?;
            // Initial scope: flat case/default labels of a compound body;
            // every other layout must fail clearly until its CFG entry
            // semantics have been checked.
            check_switch_segments(&body)?;
            Ok(vec![Stmt::Switch(selector,body)])
        }
        14 => {
            let cases:Vec<_>=il.statement_switch_cases.iter()
                .filter(|c|c.function==*function && c.statement_path==*path)
                .collect();
            if cases.len()!=1 || cases[0].status==2 {
                return Err(invalid("unsupported switch case value/range"));
            }
            Ok(vec![Stmt::Case(cases[0].value)])
        }
        10 | 11 => {
            let sem = semantic(1, node.data.kind);
            let matching: Vec<_> = il.loop_transfers.iter()
                .filter(|t| t.function == *function && t.statement_path == *path)
                .collect();
            let label=graph.statement_label(id)
                .ok_or_else(||invalid("goto/label lacks EDG target identity"))?;
            if !matching.is_empty() {
                // Keep compiler-generated structured-loop transfers separate
                // from source-level gotos. No guessed target based on spelling.
                if matching.len()!=1 {
                    return Err(invalid("duplicate EDG loop-transfer metadata"));
                }
                return match (sem, matching[0].kind) {
                    (10, 1) => Ok(vec![Stmt::Break]),
                    (10, 2) => Ok(vec![Stmt::Continue]),
                    (10, 5) => Ok(vec![Stmt::BreakSwitch]),
                    (11, 3 | 4 | 6) => Ok(Vec::new()),
                    _ => Err(invalid("inconsistent EDG loop-label classification")),
                };
            }
            if node.expression.is_some() ||
                node.children.iter().any(|&c|!is_gnu_child(graph,c,path)) {
                return Err(invalid("source goto/label has unexpected nested statement"));
            }
            match (sem,label.definition) {
                (10,false) if graph.goto_target(id).is_some() =>
                    Ok(vec![Stmt::Goto(label.label)]),
                (11,true) => Ok(vec![Stmt::Label(label.label)]),
                (10,false) => Err(invalid("goto has unresolved label in function")),
                _ => Err(invalid("invalid source goto/label definition metadata")),
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
        Expr::Global(_) | Expr::AddressGlobal(_) | Expr::AddressFunction(_) | Expr::AddressLocal(_) => Ok(()),
        Expr::BlockValue(effects,result) => {
            for effect in effects {check_expr(effect,vars,params)?;}
            check_expr(result,vars,params)
        }
        Expr::Call(_,abi,args) => {
            if abi.parameters.len()!=args.len() {return Err(invalid("invalid call arity"));}
            for arg in args {check_expr(arg,vars,params)?;}
            Ok(())
        },
        Expr::CallIndirect(callee,abi,args) => {
            if abi.parameters.len()!=args.len() {return Err(invalid("invalid indirect-call arity"));}
            check_expr(callee,vars,params)?;
            for arg in args {check_expr(arg,vars,params)?;}
            Ok(())
        },
        Expr::VlaAlloc(index,call) => {
            // The EDG lowering performs *(&local) = malloc(bytes) through
            // its proven runtime helper. This is a definite write, unlike an
            // arbitrary external call with an address argument.
            check_expr(call,vars,params)?;
            vars.insert(*index);
            Ok(())
        },
        Expr::LoadAddress(_,a) | Expr::LoadQualified(_,_,a) |
        Expr::LoadBitField(_,a) |
        Expr::PackAggregate(_,a) =>
            check_expr(a,vars,params),
        Expr::StoreAddress(_,a,b) | Expr::StoreQualified(_,_,a,b) |
        Expr::CompoundQualified(_,_,_,_,a,b) |
        Expr::StoreBitField(_,a,b) |
        Expr::CompoundBitField(_,_,_,a,b) |
        Expr::CompoundAddress(_,_,_,a,b) |
        Expr::CopyStruct(_,a,b) | Expr::CopyBytes(_,a,b) |
        Expr::PointerOffset(a,b,_) => {
            check_expr(a,vars,params)?;
            check_expr(b,vars,params)
        }
        Expr::PostAddress(_,_,a) | Expr::PostQualified(_,_,_,a) |
        Expr::PostBitField(_,_,a) =>
            check_expr(a,vars,params),
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
/// Extract cases from a flat switch body. Do not accidentally treat case
/// labels within nested if/loops as ordinary sequential statements.
fn check_switch_segments(body:&[Stmt]) -> Result<Vec<(Option<i64>,Vec<Stmt>)>,CodegenError> {
    let mut segments:Vec<(Option<i64>,Vec<Stmt>)>=Vec::new();
    let mut seen=HashSet::new();
    let mut has_default=false;
    for stmt in body {
        match stmt {
            Stmt::Case(value) => {
                if let Some(value)=value {
                    if !seen.insert(*value) {return Err(invalid("duplicate switch case"));}
                } else if has_default {
                    return Err(invalid("duplicate switch default"));
                } else {has_default=true;}
                segments.push((*value,Vec::new()));
            }
            // Nested switches have their own case tables and break targets;
            // their internal case labels must never be confused with the
            // surrounding switch's flat source-order entries.
            Stmt::Switch(..) => {
                if let Some((_,chunk))=segments.last_mut() {
                    chunk.push(stmt.clone());
                } else {
                    return Err(invalid("switch has executable pre-case statements"));
                }
            },
            Stmt::If(_,a,b) if a.iter().any(|s|matches!(s,Stmt::Case(_))) ||
                b.iter().any(|s|matches!(s,Stmt::Case(_))) =>
                    return Err(invalid("case label nested in if unsupported")),
            Stmt::While(_,a) | Stmt::DoWhile(a,_) if
                a.iter().any(|s|matches!(s,Stmt::Case(_))) =>
                    return Err(invalid("case label nested in loop unsupported")),
            Stmt::For{body:a,..} if a.iter().any(|s|matches!(s,Stmt::Case(_))) =>
                return Err(invalid("case label nested in for unsupported")),
            _ => {
                if let Some((_,chunk))=segments.last_mut() {
                    chunk.push(stmt.clone());
                } else {
                    // Statements lexically preceding the first case are not
                    // entered by selection. They must not be executed.
                    // Declarations already lower to no-op; reject effects.
                    return Err(invalid("switch has executable pre-case statements"));
                }
            }
        }
    }
    Ok(segments)
}

/// A break targets this switch only if it is not consumed by an enclosed
/// switch. Branches inside an if can reach it; nested switches cannot.
fn may_break_current_switch(body:&[Stmt]) -> bool {
    body.iter().any(|stmt|match stmt {
        Stmt::BreakSwitch => true,
        Stmt::If(_,yes,no) =>
            may_break_current_switch(yes) || may_break_current_switch(no),
        Stmt::While(_,body) | Stmt::DoWhile(body,_) =>
            may_break_current_switch(body),
        Stmt::For{init,increment,body,..} =>
            may_break_current_switch(init) ||
            may_break_current_switch(increment) ||
            may_break_current_switch(body),
        Stmt::Switch(..) => false,
        _ => false,
    })
}

// Does this body contain a break aimed at the surrounding loop?
// Nested loops consume their own breaks. Over-approximation is safe here:
// this is only used to avoid inventing a fallthrough from for (;;).
fn may_break_current_loop(body: &[Stmt]) -> bool {
    body.iter().any(|stmt| match stmt {
        Stmt::Break => true,
        Stmt::If(_, yes, no) =>
            may_break_current_loop(yes) || may_break_current_loop(no),
        Stmt::While(..) | Stmt::DoWhile(..) | Stmt::For {..} => false,
        _ => false,
    })
}

/// Predeclare source labels and reject unsupported jumps through structured
/// loops. Labels are accepted at block level; a goto in an if-arm may target
/// any such label in the same function. This is a *scope validator*, not a CFG.
fn source_labels(body:&[Stmt]) -> Result<Vec<crate::SymbolId>,CodegenError> {
    let mut definitions=HashSet::new();
    let mut references=HashSet::new();
    fn visit(body:&[Stmt],defs:&mut HashSet<crate::SymbolId>,
             refs:&mut HashSet<crate::SymbolId>, nested:bool,
             in_loop:bool) -> Result<(),CodegenError> {
        for stmt in body {
            match stmt {
                Stmt::Label(label) => {
                    if nested || in_loop {
                        return Err(invalid("label inside structured control unsupported"));
                    }
                    if !defs.insert(*label) {
                        return Err(invalid("duplicate source label"));
                    }
                }
                Stmt::Goto(label) => {
                    // A branch *out* of a loop is legal: the destination
                    // remains unsealed until the complete function is built.
                    // Jumping *into* an interior loop label is still rejected.
                    refs.insert(*label);
                }
                Stmt::If(_,yes,no) => {
                    visit(yes,defs,refs,true,in_loop)?;
                    visit(no,defs,refs,true,in_loop)?;
                }
                Stmt::While(_,body) | Stmt::DoWhile(body,_) =>
                    visit(body,defs,refs,true,true)?,
                Stmt::Switch(_,body) =>
                    visit(body,defs,refs,true,in_loop)?,
                Stmt::For{init,increment,body,..} => {
                    visit(init,defs,refs,true,true)?;
                    visit(increment,defs,refs,true,true)?;
                    visit(body,defs,refs,true,true)?;
                }
                _ => ()
            }
        }
        Ok(())
    }
    visit(body,&mut definitions,&mut references,false,false)?;
    if !references.is_subset(&definitions) {
        return Err(invalid("unresolved user goto target"));
    }
    Ok(definitions.into_iter().collect())
}

// Returns true if execution can reach the end of this sequence.
fn check(
    body: &[Stmt], vars: &mut HashSet<usize>, params: usize,
    locals: usize, loop_depth: usize,
) -> Result<bool, CodegenError> {
    let mut live = true;
    for stmt in body {
        // A label can be entered through a forward jump even if the lexical
        // predecessor returned or jumped away. Avoid phantom fallthrough.
        // C permits arbitrary dead statements after a return or goto.
        // No execution edge enters them unless a source label starts a new
        // block; do not interpret their reads or writes as reachable.
        if !live && !matches!(stmt, Stmt::Label(_)) {continue;}
        match stmt {
            Stmt::Label(_) => live=true,
            Stmt::Goto(_) => live=false,
            Stmt::Case(_) => return Err(invalid("switch case outside switch")),
            Stmt::BreakSwitch => {live=false;}
            Stmt::Switch(selector,body) => {
                check_expr(selector,vars,params)?;
                let segments=check_switch_segments(body)?;
                let has_default=segments.iter().any(|(value,_)|value.is_none());
                let mut last_fallthrough=false;
                for (_,chunk) in &segments {
                    let mut state=vars.clone();
                    last_fallthrough=check(chunk,&mut state,params,locals,loop_depth)?;
                }
                // All dispatch targets terminate when a default is present,
                // no arm can break out, and the last arm cannot fall through.
                // This recognizes C functions whose final switch returns
                // from every case without requiring a synthetic return.
                if has_default && !may_break_current_switch(body) &&
                    !last_fallthrough {
                    live=false;
                }
                // Do not infer definite assignments across switch arms.
            }
            Stmt::Break | Stmt::Continue => {
                if loop_depth == 0 { return Err(invalid("break/continue outside loop")); }
                live = false;
            }
            Stmt::InitializeAggregate(index,bytes) => {
                if *index>=locals || bytes.is_empty() || bytes.len()>4096 {
                    return Err(invalid("invalid aggregate initializer image"));
                }
                vars.insert(*index);
            }
            Stmt::ZeroAggregate(i,bytes) => {
                if *i >= locals || *bytes==0 || *bytes>4096 {
                    return Err(invalid("invalid aggregate zero initialization"));
                }
                vars.insert(*i);
            }
            Stmt::Store(i,e) => {
                if *i >= locals { return Err(invalid("invalid local index")); }
                check_expr(e, vars, params)?;
                vars.insert(*i);
            }
            Stmt::Eval(e) => { check_expr(e, vars, params)?; }
            Stmt::Return(e) => { check_expr(e, vars, params)?; live = false; }
            Stmt::ReturnAggregate(bytes,address) => {
                if !(9..=16).contains(bytes) {
                    return Err(invalid("invalid two-GPR C aggregate return size"));
                }
                check_expr(address,vars,params)?;
                live = false;
            }
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
                if let Some(condition) = condition {
                    check_expr(condition, vars, params)?;
                }
                let mut nested = vars.clone();
                check(body, &mut nested, params, locals, loop_depth + 1)?;
                // Continue paths can execute the increment. Require that all
                // its reads are safe even without any body-local writes.
                let mut increment_state = vars.clone();
                check(increment, &mut increment_state, params, locals, loop_depth + 1)?;
                // A missing condition and no break aimed at this loop means
                // there is no outgoing normal edge, even if the body returns.
                if condition.is_none() && !may_break_current_loop(body) {
                    live = false;
                }
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
/// Access class of a C scalar lvalue. Native EDG qualifier bits are
/// TQ_VOLATILE (2) and TQ_C11_ATOMIC (8); ordinary and qualified memory
/// accesses must never accidentally share non-atomic optimizations.
fn scalar_memory_access(il:&Il,ty:crate::TypeId)
    -> Result<MemoryAccess,CodegenError> {
    let mut current=ty;
    let mut access=MemoryAccess::Ordinary;
    for _ in 0..=il.types.len() {
        if let Some(d)=il.type_details.iter().find(|d|d.owner==current) {
            if d.is_reference || d.is_rvalue_reference {
                return Err(invalid("qualified reference access unsupported"));
            }
            let flags=d.qualifiers.unwrap_or(0);
            // Reject const, restrict, address-space and other qualifiers in
            // this initial M12 subset until their exact semantics are modeled.
            if flags & !10 != 0 || flags == 10 {
                return Err(invalid("unsupported/mixed C scalar qualifiers"));
            }
            let found=match flags {
                0=>MemoryAccess::Ordinary,
                2=>MemoryAccess::Volatile,
                8=>MemoryAccess::Atomic,
                _=>return Err(invalid("unsupported scalar qualifier combination")),
            };
            if found!=MemoryAccess::Ordinary {
                if access!=MemoryAccess::Ordinary && access!=found {
                    return Err(invalid("conflicting scalar memory access qualifiers"));
                }
                access=found;
            }
        }
        let def=il.types.iter().find(|t|t.id==current)
            .ok_or_else(||invalid("missing qualified type"))?;
        if semantic(4,def.kind)!=3 {return Ok(access);}
        current=def.related.ok_or_else(||invalid("unresolved qualified typeref"))?;
    }
    Err(invalid("recursive qualified type"))
}
/// EDG may normalize the *result* type of a dereference to an
/// unqualified rvalue type. The pointee's native qualifier is authoritative
/// for the memory access, so never inspect only the result expression.
fn pointee_memory_access(il:&Il,pointer_ty:crate::TypeId)
    -> Result<MemoryAccess,CodegenError> {
    let canonical=canonical_abi_type(il,pointer_ty)?;
    let def=il.types.iter().find(|t|t.id==canonical)
        .ok_or_else(||invalid("missing pointer/array operand type"))?;
    if !matches!(semantic(4,def.kind),5|6) {
        return Err(invalid("indirect access has non-pointer type"));
    }
    let element=def.related.ok_or_else(||invalid("indirect access has no pointee type"))?;
    let access=scalar_memory_access(il,element)?;
    if access!=MemoryAccess::Ordinary {
        let size=byte_size(il,element)?;
        let align=storage_alignment(il,element)?;
        if !matches!(size,1|2|4|8) || align<size {
            return Err(invalid("qualified indirect access needs naturally aligned scalar storage"));
        }
        let base=canonical_abi_type(il,element)?;
        if !il.types.iter().any(|t|t.id==base && semantic(4,t.kind)==1) {
            return Err(invalid("qualified indirect aggregates/pointers unsupported"));
        }
    }
    Ok(access)
}
/// The native pointee carries qualifiers, not necessarily the EDG rvalue.
fn indirect_lvalue_access(graph:&Graph,id:ExpressionId,il:&Il)
    -> Result<MemoryAccess,CodegenError> {
    let node=graph.expression(id).ok_or_else(||invalid("missing qualified lvalue"))?;
    if semantic(2,node.data.kind)!=3 {return Ok(MemoryAccess::Ordinary);}
    let op=node.data.operator.map(|x|semantic(3,x)).unwrap_or(0);
    match op {
        44 if node.operands.len()==1 =>
            pointee_memory_access(il,expression_type_id(graph,node.operands[0])?),
        45 if node.operands.len()==2 => {
            let a=node.operands[0];let b=node.operands[1];
            let pointer=if is_pointer_like(il,expression_type_id(graph,a)?)? {a}
                else if is_pointer_like(il,expression_type_id(graph,b)?)? {b}
                else {return Err(invalid("subscript has no pointer operand"));};
            pointee_memory_access(il,expression_type_id(graph,pointer)?)
        }
        _=>Ok(MemoryAccess::Ordinary),
    }
}
/// Preserve a direct automatic object's declared qualifiers even when
/// EDG converts its expression type to an unqualified rvalue type.
fn lvalue_memory_access(
    graph:&Graph,id:ExpressionId,locals:&[&crate::LocalVariable],il:&Il,
) -> Result<MemoryAccess,CodegenError> {
    let node=graph.expression(id).ok_or_else(||invalid("missing lvalue access"))?;
    if semantic(2,node.data.kind)==2 {
        if let Some(sym)=node.data.symbol_id {
            if let Some(local)=locals.iter().find(|v|v.symbol==sym) {
                cfg_local_type(il,local)?;
                return scalar_memory_access(il,local.ty);
            }
        }
    }
    if semantic(2,node.data.kind)==2 {
        if let Some(sym)=node.data.symbol_id {
            if il.function_parameters.iter().any(|p|p.symbol==sym) {
                return parameter_memory_access(il,sym);
            }
        }
    }
    indirect_lvalue_access(graph,id,il)
}
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
    let access=scalar_memory_access(il,ty)?;
    let ty = canonical_abi_type(il,ty)?;
    if access==MemoryAccess::Atomic &&
        il.types.iter().any(|d|d.id==ty && semantic(4,d.kind)==5) {
        return Err(invalid("atomic pointer globals need typed pointer RMW semantics"));
    }
    if access!=MemoryAccess::Ordinary {
        let alignment=storage_alignment(il,ty)?;
        let bytes=byte_size(il,ty)?;
        if !matches!(bytes,1|2|4|8) || alignment<bytes {
            return Err(invalid("qualified scalar requires naturally aligned 1/2/4/8-byte storage"));
        }
    }
    if il.types.iter().any(|d|d.id==ty && semantic(4,d.kind)==5) {
        return Ok((name,CAbiInt { bits:64,signed:false,is_bool:false }));
    }
    let integer = il.c_integer_types.iter().find(|x| x.id == ty)
        .ok_or_else(|| invalid(format!("unsupported global scalar type: {name}")))?;
    Ok((name,CAbiInt { bits: integer.bits, signed: integer.signed,
                       is_bool: integer.is_bool }))
}
/// Validate a global's linkage and return either a scalar layout or
/// the size of a fixed array. Array expressions decay to an address rather
/// than loading eight bytes of their first element.
fn global_value_type(il:&Il,symbol:crate::SymbolId)
    -> Result<(String,Option<CAbiInt>),CodegenError> {
    let meta=il.global_variables.iter().find(|g|g.symbol==symbol)
        .ok_or_else(||invalid("unresolved global symbol"))?;
    if meta.thread_local || meta.local_to_function ||
       il.static_local_owners.iter().any(|o|o.symbol==symbol) {
        return Err(invalid("TLS/static function global unsupported"));
    }
    let name=il.symbols.iter().find(|s|s.id==symbol && s.kind==2)
        .map(|s|s.name.clone()).ok_or_else(||invalid("global missing name"))?;
    if name.is_empty() || !name.bytes().next().is_some_and(|c|c.is_ascii_alphabetic()||c==b'_') ||
       !name.bytes().all(|c|c.is_ascii_alphanumeric()||c==b'_') {
        return Err(invalid("global has non-C/ambiguous linkage name"));
    }
    let ty=il.symbol_types.iter().find(|t|t.symbol==symbol)
        .ok_or_else(||invalid("global missing declared type"))?.ty;
    let ty=canonical_abi_type(il,ty)?;
    if il.types.iter().any(|t|t.id==ty && matches!(semantic(4,t.kind),6|7|9)) {
        byte_size(il,ty)?;
        Ok((name,None))
    } else {
        let (name,scalar)=scalar_global(il,symbol)?;
        Ok((name,Some(scalar)))
    }
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
        if parameter_memory_access(il,symbol)?!=MemoryAccess::Ordinary {
            return Err(invalid("qualified formal must use memory lvalue"));
        }
        return Ok((ScalarPlace::Ssa(locals.len()+i),integer_expression_type(il,graph,id)?));
    }
    if let Some(index) = locals.iter().position(|v| v.symbol == symbol) {
        let ty = cfg_local_type(il,locals[index])?;
        if scalar_memory_access(il,locals[index].ty)?!=MemoryAccess::Ordinary {
            // A qualified automatic local is an addressable object. Route
            // writes and RMW through the shared qualified lvalue operations.
            return Err(invalid("qualified automatic local must use memory lvalue"));
        }
        return Ok((ScalarPlace::Ssa(index),ty));
    }
    let (_,ty) = scalar_global(il,symbol)?;
    Ok((ScalarPlace::Memory(symbol),ty))
}
/// Byte size of a complete EDG C type, bounded before multiplication.
/// Aggregate record/union/vector types remain unsupported. An array size
/// must come from the native EDG bound, not inferred from C source tokens.
fn byte_size(il: &Il, ty: crate::TypeId) -> Result<u64,CodegenError> {
    byte_size_depth(il,ty,0)
}
fn byte_size_depth(il: &Il, ty: crate::TypeId, depth: usize) -> Result<u64,CodegenError> {
    if depth > 32 { return Err(invalid("nested aggregate layout exceeds depth limit")); }
    let ty = canonical_abi_type(il,ty)?;
    let def = il.types.iter().find(|t| t.id == ty)
        .ok_or_else(|| invalid("missing memory type"))?;
    match semantic(4,def.kind) {
        1 => {
            let info = il.c_integer_types.iter().find(|t| t.id == ty)
                .ok_or_else(|| invalid("unsupported integer memory layout"))?;
            if info.bits == 0 || info.bits % 8 != 0 {
                return Err(invalid("integer memory layout must be nonzero and byte-aligned"));
            }
            Ok(u64::from(info.bits / 8))
        }
        5 => Ok(8),
        6 => {
            let element = def.related.ok_or_else(|| invalid("array has no element type"))?;
            let count = il.array_bounds.iter().find(|b| b.array == ty)
                .ok_or_else(|| invalid("array has no fixed bound"))?.elements;
            let n = byte_size_depth(il,element,depth+1)?;
            n.checked_mul(count).filter(|&n| n > 0 && n <= 16*1024*1024)
                .ok_or_else(|| invalid("array object too large"))
        }
        // Native EDG size includes the record's internal and trailing padding.
        // Recursive validation rejects unsupported nested layouts, cycles and
        // overlapping/union fields before the allocation is accepted.
        7 | 9 if il.record_fields.iter().any(|f| f.owner == ty) => {
            checked_record_fields_depth(il, ty, depth+1)?;
            let layout = il.type_object_sizes.iter().find(|entry| entry.owner == ty)
                .ok_or_else(|| invalid("record missing native object layout"))?;
            Ok(layout.bytes)
        }
        _ => Err(invalid("unsupported aggregate/element size")),
    }
}
/// Resolve an object's native EDG alignment before exposing its address to
/// Cranelift or the ELF linker. Under-aligned storage is not a valid C object.
fn storage_alignment(il:&Il,ty:crate::TypeId)->Result<u64,CodegenError> {
    let ty=canonical_abi_type(il,ty)?;
    let size=byte_size(il,ty)?;
    let layout=il.type_object_sizes.iter().find(|item|item.owner==ty)
        .ok_or_else(||invalid("object missing native EDG size/alignment"))?;
    if layout.bytes!=size || layout.alignment==0 ||
        !layout.alignment.is_power_of_two() ||
        layout.alignment>16*1024*1024 || size%layout.alignment!=0 {
        return Err(invalid("object size/alignment disagrees with native EDG layout"));
    }
    Ok(layout.alignment)
}
/// Reject qualified and reference types until volatile/atomic/reference
/// memory semantics are implemented (M12). Qualifiers may hide in typerefs.
fn check_plain_storage_type(il:&Il,ty:crate::TypeId)->Result<(),CodegenError> {
    let mut current=ty;
    for _ in 0..=il.types.len() {
        if let Some(detail)=il.type_details.iter().find(|d|d.owner==current) {
            if detail.is_reference || detail.is_rvalue_reference ||
                detail.qualifiers.unwrap_or(0)!=0 {
                return Err(invalid("qualified/reference aggregate field unsupported"));
            }
        }
        let def=il.types.iter().find(|t|t.id==current)
            .ok_or_else(||invalid("missing aggregate field type"))?;
        if semantic(4,def.kind)!=3 {return Ok(());}
        current=def.related.ok_or_else(||invalid("unresolved aggregate field typeref"))?;
    }
    Err(invalid("cyclic aggregate field typeref"))
}
/// Validate the EDG field offsets before any record lvalue is emitted.
/// This deliberately does not infer padding, total size, or union layout.
fn checked_record_fields<'a>(
    il: &'a Il, owner: crate::TypeId,
) -> Result<Vec<&'a crate::RecordField>, CodegenError> {
    checked_record_fields_depth(il, owner, 0)
}
fn checked_record_fields_depth<'a>(
    il: &'a Il, owner: crate::TypeId, depth: usize,
) -> Result<Vec<&'a crate::RecordField>, CodegenError> {
    if depth > 32 { return Err(invalid("nested record layout exceeds depth limit")); }
    let kind=il.types.iter().find(|t|t.id==owner)
        .map(|t|semantic(4,t.kind)).ok_or_else(||invalid("missing record type"))?;
    if !matches!(kind,7|9) {return Err(invalid("unsupported record/union type"));}
    let is_union=kind==9;
    let mut fields:Vec<_> = il.record_fields.iter()
        .filter(|field| field.owner == owner).collect();
    fields.sort_by_key(|field| field.position);
    let native_size = il.type_object_sizes.iter()
        .find(|layout| layout.owner == owner)
        .ok_or_else(|| invalid("record has no native EDG object size"))?.bytes;
    if native_size == 0 || native_size > 16 * 1024 * 1024 {
        return Err(invalid("record has invalid native EDG object size"));
    }
    let native_alignment = il.type_object_sizes.iter()
        .find(|layout| layout.owner == owner)
        .ok_or_else(|| invalid("record has no native EDG alignment"))?.alignment;
    if native_alignment == 0 || !native_alignment.is_power_of_two()
        || native_alignment > 16 * 1024 * 1024 {
        return Err(invalid("record has invalid native EDG alignment"));
    }
    if native_size % native_alignment != 0 {
        return Err(invalid("record size is not a multiple of native alignment"));
    }
    let mut previous_end_bit = 0u64;
    for (index, field) in fields.iter().enumerate() {
        if field.position != index as u32 {
            return Err(invalid("record field positions are not contiguous"));
        }
        check_plain_storage_type(il,field.ty)?;
        let field_type=canonical_abi_type(il,field.ty)?;
        let def=il.types.iter().find(|t|t.id==field_type)
            .ok_or_else(||invalid("record field type missing"))?;
        let field_kind=semantic(4,def.kind);
        let is_bitfield=field.bit_width!=0 || field.bit_offset!=0;
        let (end,bit_start,bit_end)=if is_bitfield {
            if field_kind!=1 {
                return Err(invalid("bitfield must have an integer type"));
            }
            let info=il.c_integer_types.iter().find(|t|t.id==field_type)
                .ok_or_else(||invalid("bitfield lacks EDG integer type"))?;
            if !matches!(info.bits,8|16|32|64) ||
                field.bit_width==0 || field.bit_width>u32::from(info.bits) ||
                field.bit_offset>=8 || field.bit_offset+field.bit_width>64 {
                return Err(invalid("unsupported native bitfield width/position"));
            }
            let byte_count=u64::from((field.bit_offset+field.bit_width).div_ceil(8));
            let end=field.byte_offset.checked_add(byte_count)
                .ok_or_else(||invalid("bitfield byte range overflows"))?;
            let bit_start=field.byte_offset.checked_mul(8)
                .and_then(|n|n.checked_add(u64::from(field.bit_offset)))
                .ok_or_else(||invalid("bitfield bit offset overflows"))?;
            let bit_end=bit_start.checked_add(u64::from(field.bit_width))
                .ok_or_else(||invalid("bitfield bit extent overflows"))?;
            (end,bit_start,bit_end)
        } else {
            if !matches!(field_kind,1|5|6|7|9) {
                return Err(invalid("unsupported record field kind"));
            }
            let width=byte_size_depth(il,field.ty,depth+1)?;
            let end=field.byte_offset.checked_add(width)
                .ok_or_else(||invalid("record field offset overflows"))?;
            let start_bit=field.byte_offset.checked_mul(8)
                .ok_or_else(||invalid("record bit position overflows"))?;
            let end_bit=end.checked_mul(8)
                .ok_or_else(||invalid("record bit extent overflows"))?;
            (end,start_bit,end_bit)
        };
        if is_union {
            // C unions share offset zero, including native bitfield members.
            if field.byte_offset!=0 {
                return Err(invalid("nonzero union member offset unsupported"));
            }
        } else {
            if bit_start<previous_end_bit {
                return Err(invalid("record fields overlap outside union"));
            }
            previous_end_bit=bit_end;
        }
        if end>native_size || end>16*1024*1024 {
            return Err(invalid("record field exceeds native EDG object size"));
        }
        if field.name.is_empty() {
            // Anonymous and zero-width barriers need an explicit wire marker.
            // Keep them unsupported instead of guessing which layout was used.
            return Err(invalid("anonymous/unnamed bitfield layout unsupported"));
        }
    }
    Ok(fields)
}
/// C aggregate assignment may copy padding and bitfield storage units as
/// bytes. Never copy a bitfield by reloading its *declared* integer type:
/// that would clobber adjacent fields and could overread a packed record.
fn has_native_bitfield(il:&Il,ty:crate::TypeId,depth:usize)
    ->Result<bool,CodegenError> {
    if depth>32 {return Err(invalid("recursive native bitfield layout"));}
    let ty=canonical_abi_type(il,ty)?;
    let def=il.types.iter().find(|t|t.id==ty)
        .ok_or_else(||invalid("missing aggregate type"))?;
    match semantic(4,def.kind) {
        6 => has_native_bitfield(il,def.related
            .ok_or_else(||invalid("aggregate array missing element"))?,depth+1),
        7|9 => {
            for field in checked_record_fields_depth(il,ty,depth+1)? {
                if field.bit_width!=0 || field.bit_offset!=0 ||
                    has_native_bitfield(il,field.ty,depth+1)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}
/// Expand a supported value-semantic aggregate into scalar fields at native
/// byte offsets. This copies array elements and nested records without
/// touching indeterminate C struct padding. The bound also keeps generated
/// machine code and Cranelift SSA temporaries predictable.
fn flattened_copy_fields(il:&Il,ty:crate::TypeId)
    ->Result<Vec<(CAbiInt,i64)>,CodegenError> {
    byte_size(il,ty)?;
    let mut out=Vec::new();
    flatten_copy_type(il,ty,0,0,&mut out)?;
    if out.is_empty() {return Err(invalid("aggregate has no copyable fields"));}
    Ok(out)
}
fn flatten_copy_type(
    il:&Il,ty:crate::TypeId,offset:u64,depth:usize,
    out:&mut Vec<(CAbiInt,i64)>,
) -> Result<(),CodegenError> {
    if depth>32 {return Err(invalid("aggregate copy nesting exceeds depth limit"));}
    check_plain_storage_type(il,ty)?;
    let ty=canonical_abi_type(il,ty)?;
    let def=il.types.iter().find(|d|d.id==ty)
        .ok_or_else(||invalid("missing aggregate copy type"))?;
    match semantic(4,def.kind) {
        1|5 => {
            if out.len()>=256 {
                return Err(invalid("aggregate copy has over 256 scalar elements"));
            }
            let field_offset=i64::try_from(offset)
                .map_err(|_|invalid("aggregate copy offset overflow"))?;
            let abi=call_type_abi(il,ty)?;
            if !matches!(abi.bits,8|16|32|64) {
                return Err(invalid("unsupported aggregate copy element width"));
            }
            out.push((abi,field_offset));
        }
        6 => {
            let element=def.related
                .ok_or_else(||invalid("aggregate array has no element type"))?;
            let count=il.array_bounds.iter().find(|b|b.array==ty)
                .ok_or_else(||invalid("aggregate array lacks fixed bound"))?.elements;
            if count==0 || count>256 {
                return Err(invalid("aggregate copy array bound unsupported"));
            }
            let stride=byte_size(il,element)?;
            for i in 0..count {
                let delta=i.checked_mul(stride)
                    .ok_or_else(||invalid("aggregate copy stride overflow"))?;
                let member=offset.checked_add(delta)
                    .ok_or_else(||invalid("aggregate copy address overflow"))?;
                flatten_copy_type(il,element,member,depth+1,out)?;
            }
        }
        7 => {
            for field in checked_record_fields(il,ty)? {
                let member=offset.checked_add(field.byte_offset)
                    .ok_or_else(||invalid("nested record field offset overflow"))?;
                flatten_copy_type(il,field.ty,member,depth+1,out)?;
            }
        }
        _ => return Err(invalid("unsupported aggregate copy element type")),
    }
    Ok(())
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
/// Resolve a native named bitfield by field symbol identity. The address
/// used by the resulting IR is the first occupied byte, not a C address-of
/// operation (C prohibits taking the address of a bitfield).
fn selected_bitfield(
    graph:&Graph,id:ExpressionId,il:&Il,
)->Result<Option<BitFieldLayout>,CodegenError> {
    let node=graph.expression(id).ok_or_else(||invalid("missing bitfield expression"))?;
    if semantic(2,node.data.kind)!=3 {return Ok(None);}
    let op=node.data.operator.map(|v|semantic(3,v)).unwrap_or(0);
    if !matches!(op,52|53) || node.operands.len()!=2 {return Ok(None);}
    let field_node=graph.expression(node.operands[1])
        .ok_or_else(||invalid("missing EDG bitfield identity"))?;
    if semantic(2,field_node.data.kind)!=5 || !field_node.operands.is_empty() {
        return Err(invalid("invalid EDG field designator"));
    }
    let symbol=field_node.data.symbol_id
        .ok_or_else(||invalid("missing EDG bitfield symbol"))?;
    let base_type=canonical_abi_type(il,expression_type_id(graph,node.operands[0])?)?;
    let owner=if op==52 {base_type} else {
        let pointer=il.types.iter().find(|t|t.id==base_type &&
            semantic(4,t.kind)==5).ok_or_else(||invalid("bitfield arrow needs pointer"))?;
        canonical_abi_type(il,pointer.related
            .ok_or_else(||invalid("bitfield pointer missing target"))?)?
    };
    if !il.types.iter().any(|t|t.id==owner &&
        matches!(semantic(4,t.kind),7|9)) {
        return Err(invalid("bitfield owner is not a record/union"));
    }
    let fields=checked_record_fields(il,owner)?;
    let field=fields.into_iter().find(|f|f.symbol==symbol)
        .ok_or_else(||invalid("unknown EDG record field identity"))?;
    if field.bit_width==0 && field.bit_offset==0 {return Ok(None);}
    let ty=canonical_abi_type(il,field.ty)?;
    if ty!=canonical_abi_type(il,expression_type_id(graph,id)?)? {
        return Err(invalid("bitfield expression/field types disagree"));
    }
    let info=il.c_integer_types.iter().find(|t|t.id==ty)
        .ok_or_else(||invalid("bitfield integer type is missing"))?;
    if info.is_bool {return Err(invalid("boolean bitfield unsupported"));}
    let total=field.bit_offset+field.bit_width;
    if total==0 || total>64 || field.bit_width>64 {
        return Err(invalid("native bitfield exceeds supported width"));
    }
    Ok(Some(BitFieldLayout {
        shift:field.bit_offset as u8,
        width:field.bit_width as u8,
        bytes:total.div_ceil(8) as u8,
        signed:info.signed,
    }))
}
/// Narrow unsigned bitfields promote to signed int when the value range
/// fits in int. Using the underlying unsigned C type would give the wrong
/// division/remainder and signed shifts for expressions such as b /= -2.
fn bitfield_promoted_type(layout:BitFieldLayout,base:CAbiInt)->CAbiInt {
    if layout.width<32 {
        CAbiInt {bits:32,signed:true,is_bool:false}
    } else {promoted_integer_type(base)}
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
        6 => {
            let symbol=node.data.symbol_id.ok_or_else(||invalid("routine address without symbol"))?;
            direct_call_decl(il,symbol)?;
            Ok(Expr::AddressFunction(symbol))
        }
        3 => {
            let op=node.data.operator.map(|o|semantic(3,o)).unwrap_or(0);
            match op {
                44 if node.operands.len()==1 => {
                    let _=pointee_memory_access(il,
                        expression_type_id(graph,node.operands[0])?)?;
                    lower_c_expression(graph,node.operands[0],params,locals,il)
                }
                45 if node.operands.len()==2 => {
                    let a=node.operands[0]; let b=node.operands[1];
                    let (pointer,index)=if is_pointer_like(il,expression_type_id(graph,a)?)? {(a,b)}
                        else if is_pointer_like(il,expression_type_id(graph,b)?)? {(b,a)}
                        else { return Err(invalid("subscript has no pointer operand")); };
                    let _=pointee_memory_access(il,expression_type_id(graph,pointer)?)?;
                    let stride=pointer_stride(il,expression_type_id(graph,pointer)?)?;
                    let base=lower_c_expression(graph,pointer,params,locals,il)?;
                    let index=lower_c_expression(graph,index,params,locals,il)?;
                    Ok(Expr::PointerOffset(Box::new(base),Box::new(index),stride))
                }
                46 if node.operands.len()==1 =>
                    lower_address(graph,node.operands[0],params,locals,il),
                52 | 53 if node.operands.len()==2 => {
                    // EDG: dot_field(base, enk_field) or
                    // points_to_field(pointer, enk_field). The field identity
                    // is native and never reconstructed from its spelling.
                    let field_node=graph.expression(node.operands[1])
                        .ok_or_else(||invalid("missing field selection operand"))?;
                    if semantic(2,field_node.data.kind)!=5 || !field_node.operands.is_empty() {
                        return Err(invalid("field selection requires an EDG field reference"));
                    }
                    let symbol=field_node.data.symbol_id
                        .ok_or_else(||invalid("field reference missing native symbol"))?;
                    let base_type=canonical_abi_type(il,
                        expression_type_id(graph,node.operands[0])?)?;
                    let owner=if op==52 {base_type} else {
                        let pointer=il.types.iter().find(|t|t.id==base_type
                            && semantic(4,t.kind)==5)
                            .ok_or_else(||invalid("arrow selection requires struct pointer"))?;
                        canonical_abi_type(il,pointer.related
                            .ok_or_else(||invalid("arrow pointer has no target type"))?)?
                    };
                    if !il.types.iter().any(|t|t.id==owner &&
                            matches!(semantic(4,t.kind),7|9)) {
                        return Err(invalid("only C struct/union field selection supported"));
                    }
                    let fields=checked_record_fields(il,owner)?;
                    let field=fields.into_iter().find(|f|f.symbol==symbol)
                        .ok_or_else(||invalid("field does not belong to selected struct"))?;
                    check_plain_storage_type(il,expression_type_id(graph,id)?)?;
                    if canonical_abi_type(il,field.ty)? !=
                        canonical_abi_type(il,expression_type_id(graph,id)?)? {
                        return Err(invalid("field reference type disagrees with EDG record layout"));
                    }
                    let offset=i64::try_from(field.byte_offset)
                        .map_err(|_|invalid("field offset exceeds address range"))?;
                    let base=if op==52 {
                        lower_address(graph,node.operands[0],params,locals,il)?
                    } else {
                        lower_c_expression(graph,node.operands[0],params,locals,il)?
                    };
                    Ok(if offset==0 {base} else {
                        Expr::Add(Box::new(base),Box::new(Expr::Constant(offset)))
                    })
                }
                11 if node.operands.len()==1 =>
                    lower_address(graph,node.operands[0],params,locals,il),
                _ => Err(invalid("unsupported addressable expression")),
            }
        }
        _ => Err(invalid("unsupported non-variable lvalue")),
    }
}
#[derive(Debug,Clone)]
struct CallDeclaration {
    symbol: crate::SymbolId,
    name: String,
    abi: CAbi,
}
fn call_type_abi(il:&Il, ty:crate::TypeId) -> Result<CAbiInt,CodegenError> {
    let ty=canonical_abi_type(il,ty)?;
    if il.types.iter().any(|t|t.id==ty && semantic(4,t.kind)==5) {
        // SysV LP64 data pointers (not aggregate/function-pointer ABI).
        return Ok(CAbiInt {bits:64,signed:false,is_bool:false});
    }
    let t=il.c_integer_types.iter().find(|t|t.id==ty)
        .ok_or_else(||invalid("unsupported C direct-call scalar/pointer type"))?;
    if !matches!(t.bits,8|16|32|64) {
        return Err(invalid("unsupported direct-call integer width"));
    }
    Ok(CAbiInt {bits:t.bits,signed:t.signed,is_bool:t.is_bool})
}
fn direct_call_decl(il:&Il,symbol:crate::SymbolId) -> Result<CallDeclaration,CodegenError> {
    let sym=il.symbols.iter().find(|s|s.id==symbol && s.kind==1)
        .ok_or_else(||invalid("call callee is not a direct named routine"))?;
    let name=sym.name.clone();
    if name.is_empty() ||
       !name.bytes().next().is_some_and(|c|c.is_ascii_alphabetic()||c==b'_') ||
       !name.bytes().all(|c|c.is_ascii_alphanumeric()||c==b'_') {
        return Err(invalid(format!("non-C/ambiguous direct-call symbol {name}")));
    }
    let ty=il.symbol_types.iter().find(|t|t.symbol==symbol)
        .ok_or_else(||invalid("call has no routine type"))?.ty;
    let ty=canonical_abi_type(il,ty)?;
    let def=il.types.iter().find(|t|t.id==ty)
        .ok_or_else(||invalid("missing called function type"))?;
    if semantic(4,def.kind)!=2 || def.related_role!=Some(3) {
        return Err(invalid("indirect/function-pointer calls not implemented"));
    }
    let abi=function_type_abi(il,ty)?;
    Ok(CallDeclaration {symbol,name,abi})
}

/// Validate a C function prototype independently of whether the callee is a
/// named symbol or a function-pointer expression. ABI placement remains owned
/// by the selected target and is never inferred from the callee's spelling.
fn function_type_abi(il:&Il,ty:crate::TypeId) -> Result<CAbi,CodegenError> {
    let ty=canonical_abi_type(il,ty)?;
    let def=il.types.iter().find(|t|t.id==ty)
        .ok_or_else(||invalid("missing called function type"))?;
    if semantic(4,def.kind)!=2 || def.related_role!=Some(3) {
        return Err(invalid("callee type is not a C function prototype"));
    }
    let proto=il.function_signatures.iter().find(|s|s.owner==ty)
        .ok_or_else(||invalid("called function missing prototype metadata"))?;
    if proto.variadic { return Err(invalid("variadic C calls unsupported")); }
    let mut declared:Vec<_>=il.type_parameters.iter().filter(|p|p.owner==ty).collect();
    declared.sort_by_key(|p|p.position);
    if declared.iter().enumerate().any(|(i,p)|p.position!=i as u32) {
        return Err(invalid("noncontiguous call parameters"));
    }
    if !proto.prototyped && !declared.is_empty() {
        return Err(invalid("unprototyped C calls with arguments unsupported"));
    }
    let mut parameters=Vec::with_capacity(declared.len());
    let mut aggregate_parameters=Vec::with_capacity(declared.len());
    for p in declared {
        let aggregate=target::ABI.classify_aggregate(il,p.ty)?;
        parameters.push(if aggregate.is_some() {
            CAbiInt{bits:64,signed:false,is_bool:false}
        } else {call_type_abi(il,p.ty)?});
        aggregate_parameters.push(aggregate);
    }
    let ret=canonical_abi_type(il,def.related.ok_or_else(||invalid("call return type missing"))?)?;
    let aggregate_result=target::ABI.classify_aggregate(il,ret)?;
    target::ABI.validate_aggregate_return(aggregate_result)?;
    let result=if il.types.iter().any(|t|t.id==ret && semantic(4,t.kind)==4) {
        None
    } else if aggregate_result.is_some() {
        Some(CAbiInt{bits:64,signed:false,is_bool:false})
    } else {Some(call_type_abi(il,ret)?)};
    let abi=CAbi{aggregate_parameters,aggregate_result,parameters,result};
    abi.passing_plan()?;
    Ok(abi)
}
/// Accept only a real function type or a pointer to a function. Do not
/// reinterpret an arbitrary data pointer as a callable C prototype.
fn callable_type_abi(il:&Il,ty:crate::TypeId) -> Result<CAbi,CodegenError> {
    let ty=canonical_abi_type(il,ty)?;
    let def=il.types.iter().find(|t|t.id==ty)
        .ok_or_else(||invalid("missing call target type"))?;
    let function_ty=match semantic(4,def.kind) {
        2 => ty,
        5 if def.related_role==Some(1) => canonical_abi_type(il,
            def.related.ok_or_else(||invalid("function pointer without target"))?)?,
        _ => return Err(invalid("call target is not a function/function pointer")),
    };
    function_type_abi(il,function_ty)
}
fn compile_calls(il:&Il) -> Result<Vec<CallDeclaration>,CodegenError> {
    // Member calls remain a separate C++ ABI/lifetime feature.
    for site in &il.calls {
        if site.first_argument!=1 { return Err(invalid("member call ABI unsupported")); }
    }
    // Include ordinary calls AND function designators whose addresses are
    // taken but never directly called. Both need FuncRefs and ELF relocations.
    let mut found:HashMap<crate::SymbolId,CallDeclaration>=HashMap::new();
    let mut symbols:Vec<_>=il.expressions.iter()
        .filter(|expr|semantic(2,expr.kind)==6)
        .map(|expr|expr.symbol_id.ok_or_else(||
            invalid("routine expression without symbol")))
        .collect::<Result<Vec<_>,_>>()?;
    symbols.extend(il.statement_routine_initializers.iter().map(|r|r.symbol));
    symbols.extend(il.constant_routine_addresses.iter().map(|r|r.symbol));
    for symbol in symbols {
        let decl=direct_call_decl(il,symbol)?;
        if let Some(previous)=found.get(&symbol) {
            if previous.abi!=decl.abi || previous.name!=decl.name {
                return Err(invalid("inconsistent routine reference type"));
            }
        } else {found.insert(symbol,decl);}
    }
    let mut result:Vec<CallDeclaration>=found.into_values().collect();
    result.sort_by(|a,b|a.name.cmp(&b.name));
    Ok(result)
}
/// Accept exactly the address of one stack-backed C pointer local.
/// Arbitrary pointer casts or pointer arithmetic do not prove a written slot.
fn lowered_vla_slot(arg:&Expr) -> Option<usize> {
    match arg {
        Expr::AddressLocal(index) => Some(*index),
        Expr::Normalize(_,inner) => lowered_vla_slot(inner),
        _ => None,
    }
}
fn lower_c_expression(
    graph: &Graph, id: ExpressionId, params: &[crate::SymbolId],
    locals: &[&crate::LocalVariable], il: &Il,
) -> Result<Expr, CodegenError> {
    let node = graph.expression(id).ok_or_else(|| invalid("missing expression"))?;
    if let Some(site)=graph.calls.iter().find(|site|
        site.function==node.data.function &&
        site.statement_path==node.data.statement_path &&
        site.path==node.data.path) {
        if site.first_argument!=1 || node.operands.is_empty() {
            return Err(invalid("member call unsupported or missing callee"));
        }
        let callee=graph.expression(node.operands[0])
            .ok_or_else(||invalid("call missing callee expression"))?;
        let direct=semantic(2,callee.data.kind)==6;
        let (symbol,abi)=if direct {
            let symbol=callee.data.symbol_id
                .ok_or_else(||invalid("direct routine without symbol"))?;
            let decl=direct_call_decl(il,symbol)?;
            (Some(symbol),decl.abi)
        } else {
            (None,callable_type_abi(il,expression_type_id(graph,node.operands[0])?)?)
        };
        if abi.parameters.len()!=node.operands.len()-1 {
            return Err(invalid("call argument count disagrees with EDG prototype"));
        }
        let mut args=Vec::with_capacity(abi.parameters.len());
        for (&operand,aggregate) in node.operands[1..].iter()
            .zip(&abi.aggregate_parameters) {
            if let Some(size)=aggregate {
                let actual=target::ABI.classify_aggregate(il,expression_type_id(graph,operand)?)?;
                if actual!=Some(*size) {
                    return Err(invalid("aggregate call argument ABI type/size mismatch"));
                }
                let address=lower_address(graph,operand,params,locals,il)?;
                args.push(target::ABI.lower_aggregate_argument(*size,address));
            } else {
                args.push(lower_c_expression(graph,operand,params,locals,il)?);
            }
        }
        return Ok(if let Some(symbol)=symbol {
            let decl=direct_call_decl(il,symbol)?;
            if decl.name=="__vla_alloc" {
                let caller=graph.routine_for_function(&node.data.function)
                    .ok_or_else(||invalid("VLA helper call has no owning function"))?;
                if !has_lowered_vla_runtime(il,caller) || abi.result.is_some() ||
                    abi.parameters.len()!=2 || args.len()!=2 {
                    return Err(invalid("unsupported VLA allocation helper signature/lifetime"));
                }
                let target=lowered_vla_slot(&args[0]).ok_or_else(||
                    invalid("VLA allocator must receive address of an automatic pointer"))?;
                Expr::VlaAlloc(target,Box::new(Expr::Call(symbol,abi,args)))
            } else {
                Expr::Call(symbol,abi,args)
            }
        } else {
            // Evaluate the function pointer exactly once at the call site.
            let pointer=lower_c_expression(graph,node.operands[0],params,locals,il)?;
            Expr::CallIndirect(Box::new(pointer),abi,args)
        });
    }
    // A routine designator decays to a function pointer; it is never a load
    // from a scalar data symbol.
    if semantic(2,node.data.kind)==6 {
        let symbol=node.data.symbol_id
            .ok_or_else(||invalid("routine designator has no symbol"))?;
        direct_call_decl(il,symbol)?;
        return Ok(Expr::AddressFunction(symbol));
    }
    // The result of *function_pointer is itself a function designator.
    // There is no load from executable memory: use the pointer value.
    if semantic(2,node.data.kind)==3 && node.operands.len()==1 &&
        node.data.operator.is_some_and(|op|semantic(3,op)==44) {
        let ty=canonical_abi_type(il,expression_type_id(graph,id)?)?;
        if il.types.iter().any(|t|t.id==ty && semantic(4,t.kind)==2) {
            callable_type_abi(il,ty)?;
            return lower_c_expression(graph,node.operands[0],params,locals,il);
        }
    }
    // C struct assignment is an aggregate operation, not an I64 expression.
    // Only a standalone assignment statement is supported at this stage:
    // using its aggregate value in another expression needs value-category IR.
    if semantic(2,node.data.kind)==3 &&
        node.data.operator.is_some_and(|op|semantic(3,op)==18) &&
        node.operands.len()==2 {
        let ty=canonical_abi_type(il,expression_type_id(graph,id)?)?;
        if il.types.iter().any(|def|def.id==ty && matches!(semantic(4,def.kind),7|9)) {
            if node.data.parent.is_some() {
                return Err(invalid("aggregate assignment value use unsupported"));
            }
            for &operand in &node.operands {
                if canonical_abi_type(il,expression_type_id(graph,operand)?)?!=ty {
                    return Err(invalid("struct copy operands have different types"));
                }
            }
            check_plain_storage_type(il,expression_type_id(graph,id)?)?;
            let kind=il.types.iter().find(|d|d.id==ty)
                .map(|d|semantic(4,d.kind)).ok_or_else(||
                    invalid("missing aggregate assignment type"))?;
            let dst=lower_address(graph,node.operands[0],params,locals,il)?;
            let src=lower_address(graph,node.operands[1],params,locals,il)?;
            if kind==9 || has_native_bitfield(il,ty,0)? {
                let bytes=byte_size(il,ty)?;
                if bytes==0 || bytes>512 {
                    return Err(invalid("bitfield/union byte copy exceeds 512-byte limit"));
                }
                return Ok(Expr::CopyBytes(bytes as u32,Box::new(dst),Box::new(src)));
            }
            let layout=flattened_copy_fields(il,ty)?;
            return Ok(Expr::CopyStruct(layout,Box::new(dst),Box::new(src)));
        }
    }
    let output = integer_expression_type(il, graph, id)?;
    let wrap = |e| Expr::Normalize(output, Box::new(e));
    // Intercept bitfield ++/-- and compound writes before generic lvalue
    // lowering. A bitfield is not a separately addressable C scalar.
    if semantic(2,node.data.kind)==3 {
        let op=node.data.operator.map(|v|semantic(3,v)).unwrap_or(0);
        let unary=matches!(op,29|30|41|42) && node.operands.len()==1;
        let compound=(31..=40).contains(&op) && node.operands.len()==2;
        if unary || compound {
            let left=node.operands[0];
            if let Some(layout)=selected_bitfield(graph,left,il)? {
                let address=Box::new(lower_address(graph,left,params,locals,il)?);
                let ty=integer_expression_type(il,graph,left)?;
                let promotion=bitfield_promoted_type(layout,ty);
                let updated=if op==29 || op==30 {
                    Expr::PostBitField(layout,if op==29 {1} else {-1},address)
                } else {
                    let kind=match op {
                        31|41=>CCompoundOp::Add,32|42=>CCompoundOp::Sub,
                        33=>CCompoundOp::Mul,34=>CCompoundOp::Div,
                        35=>CCompoundOp::Rem,36=>CCompoundOp::Shl,
                        37=>CCompoundOp::Shr,38=>CCompoundOp::And,
                        39=>CCompoundOp::Or,40=>CCompoundOp::Xor,
                        _=>return Err(invalid("unsupported native bitfield operator")),
                    };
                    let (compute_ty,amount)=if unary {
                        (promotion,Box::new(Expr::Constant(1)))
                    } else {
                        let rhs=node.operands[1];
                        let rhs_ty=integer_expression_type(il,graph,rhs)?;
                        let compute_ty=if matches!(kind,CCompoundOp::Shl|CCompoundOp::Shr) {
                            promotion
                        } else {common_integer_type(promotion,rhs_ty)};
                        (compute_ty,Box::new(lower_c_expression(graph,rhs,params,locals,il)?))
                    };
                    Expr::CompoundBitField(layout,compute_ty,kind,address,amount)
                };
                return Ok(wrap(updated));
            }
        }
    }
    let inner = match semantic(2, node.data.kind) {
        4 => {
            let path=format!("{}.gnu{}.0",node.data.statement_path,
                             node.data.path.replace('.',"_"));
            let stmt=graph.statements.iter().position(|entry|
                entry.data.function==node.data.function && entry.data.path==path)
                .ok_or_else(||invalid("GNU expression missing native statement block"))?;
            return lower_gnu_block(graph,crate::graph::StatementId(stmt),params,locals,il);
        },
        1 => {
            if let Some(value)=node.data.constant_integer {
                Expr::Constant(value)
            } else if let Some(address)=il.constant_routine_addresses.iter().find(|a|
                a.function==node.data.function &&
                a.statement_path==node.data.statement_path && a.path==node.data.path) {
                let decl=direct_call_decl(il,address.symbol)?;
                let actual=callable_type_abi(il,expression_type_id(graph,id)?)?;
                if actual!=decl.abi {
                    return Err(invalid("folded function-pointer constant type mismatch"));
                }
                Expr::AddressFunction(address.symbol)
            } else if let Some(address)=il.constant_addresses.iter().find(|a|
                a.function==node.data.function &&
                a.statement_path==node.data.statement_path && a.path==node.data.path) {
                let base=if let Some(index)=params.iter().position(|p|*p==address.symbol) {
                    Expr::AddressLocal(locals.len()+index)
                } else if let Some(index)=locals.iter().position(|v|v.symbol==address.symbol) {
                    Expr::AddressLocal(index)
                } else {
                    global_value_type(il,address.symbol)?;
                    Expr::AddressGlobal(address.symbol)
                };
                if address.byte_offset==0 {base} else {
                    Expr::Add(Box::new(base),Box::new(Expr::Constant(address.byte_offset)))
                }
            } else {
                return Err(invalid(format!(
                    "unsupported/non-i64-representable constant: kind={:?}, type_kind={:?}, type_id={:?} at {}:{}:{}",
                    node.data.constant_kind,node.data.type_kind,node.data.type_id,
                    node.data.function,node.data.statement_path,node.data.path)));
            }
        },
        2 => {
            let symbol = node.data.symbol_id.ok_or_else(|| invalid("unresolved scalar variable"))?;
            if let Some(i) = params.iter().position(|p| *p == symbol) {
                let access=parameter_memory_access(il,symbol)?;
                if access==MemoryAccess::Ordinary {Expr::Local(locals.len()+i)}
                else {Expr::LoadQualified(output,access,
                    Box::new(Expr::AddressLocal(locals.len()+i)))}
            } else {
                if let Some(i) = locals.iter().position(|v| v.symbol == symbol) {
                    let local_ty=cfg_local_type(il,locals[i])?;
                    let canonical=canonical_abi_type(il,locals[i].ty)?;
                    if il.types.iter().any(|d|d.id==canonical
                           && semantic(4,d.kind)==6) {
                        Expr::AddressLocal(i)
                    } else {
                        let access=scalar_memory_access(il,locals[i].ty)?;
                        if access==MemoryAccess::Ordinary {Expr::Local(i)}
                        else {Expr::LoadQualified(local_ty,access,
                            Box::new(Expr::AddressLocal(i)))}
                    }
                } else {
                    let (_,ty)=global_value_type(il,symbol)?;
                    if ty.is_none() { Expr::AddressGlobal(symbol) }
                    else { Expr::Global(symbol) }
                }
            }
        }
        3 => {
            let op = node.data.operator.map(|v| semantic(3, v)).unwrap_or(0);
            let operands = &node.operands;
            if matches!(op,43|44|45|46|52|53) {
                match op {
                    52 | 53 if operands.len()==2 => {
                        let address=lower_address(graph,id,params,locals,il)?;
                        let canonical=canonical_abi_type(il,expression_type_id(graph,id)?)?;
                        if let Some(layout)=selected_bitfield(graph,id,il)? {
                            Expr::LoadBitField(layout,Box::new(address))
                        } else if il.types.iter().any(|d|d.id==canonical &&
                                semantic(4,d.kind)==6) {
                            // C array member designators decay to an address.
                            address
                        } else {
                            Expr::LoadAddress(output,Box::new(address))
                        }
                    }
                    43 if operands.len()==1 => {
                        if selected_bitfield(graph,operands[0],il)?.is_some() {
                            return Err(invalid("C bitfield address-of is forbidden"));
                        }
                        lower_address(graph,operands[0],params,locals,il)?
                    },
                    44 if operands.len()==1 => {
                        let address=lower_address(graph,id,params,locals,il)?;
                        let canonical=canonical_abi_type(il,expression_type_id(graph,id)?)?;
                        if il.types.iter().any(|d|d.id==canonical && semantic(4,d.kind)==6) {
                            address
                        } else {
                            let access=indirect_lvalue_access(graph,id,il)?;
                            if access==MemoryAccess::Ordinary {
                                Expr::LoadAddress(output,Box::new(address))
                            } else {
                                Expr::LoadQualified(output,access,Box::new(address))
                            }
                        }
                    }
                    45 if operands.len()==2 => {
                        let address=lower_address(graph,id,params,locals,il)?;
                        let canonical=canonical_abi_type(il,expression_type_id(graph,id)?)?;
                        if il.types.iter().any(|d|d.id==canonical && semantic(4,d.kind)==6) {
                            address
                        } else {
                            let access=indirect_lvalue_access(graph,id,il)?;
                            if access==MemoryAccess::Ordinary {
                                Expr::LoadAddress(output,Box::new(address))
                            } else {
                                Expr::LoadQualified(output,access,Box::new(address))
                            }
                        }
                    }
                    46 if operands.len()==1 => lower_address(
                        graph,operands[0],params,locals,il)?,
                    _ => return Err(invalid("invalid pointer expression arity")),
                }
            } else if matches!(op,47|48|49|50|51) {
                if operands.len()!=2 {return Err(invalid("pointer arithmetic arity"));}
                let a=operands[0];let b=operands[1];
                if matches!(op,47|48|49) {
                    let lhs_ptr=is_pointer_like(il,expression_type_id(graph,a)?)?;
                    let rhs_ptr=is_pointer_like(il,expression_type_id(graph,b)?)?;
                    if op==49 {
                        if !lhs_ptr || !rhs_ptr {return Err(invalid("pointer difference types"));}
                        let stride=pointer_stride(il,expression_type_id(graph,a)?)?;
                        let left=lower_c_expression(graph,a,params,locals,il)?;
                        let right=lower_c_expression(graph,b,params,locals,il)?;
                        Expr::IntegerBinary(CIntBinaryOp::SignedDivide,
                            Box::new(Expr::Sub(Box::new(left),Box::new(right))),
                            Box::new(Expr::Constant(stride as i64)))
                    } else {
                        let (ptr,idx)=if lhs_ptr {(a,b)}
                            else if op==47 && rhs_ptr {(b,a)}
                            else {return Err(invalid("pointer arithmetic pointer missing"));};
                        let stride=pointer_stride(il,expression_type_id(graph,ptr)?)?;
                        let base=lower_c_expression(graph,ptr,params,locals,il)?;
                        let index=lower_c_expression(graph,idx,params,locals,il)?;
                        let index=if op==48 {Expr::Neg(Box::new(index))} else {index};
                        Expr::PointerOffset(Box::new(base),Box::new(index),stride)
                    }
                } else {
                    let (place,ty)=scalar_lvalue(graph,a,params,locals,il)?;
                    if !is_pointer_like(il,expression_type_id(graph,a)?)? {
                        return Err(invalid("pointer compound assignment on nonpointer"));
                    }
                    let stride=pointer_stride(il,expression_type_id(graph,a)?)?;
                    let base=match place {
                        ScalarPlace::Ssa(i)=>Expr::Local(i),
                        ScalarPlace::Memory(s)=>Expr::Global(s),
                    };
                    let idx=lower_c_expression(graph,b,params,locals,il)?;
                    let idx=if op==51 {Expr::Neg(Box::new(idx))} else {idx};
                    let next=Box::new(Expr::PointerOffset(Box::new(base),Box::new(idx),stride));
                    match place {
                        ScalarPlace::Ssa(i)=>Expr::AssignLocal(i,ty,next),
                        ScalarPlace::Memory(s)=>Expr::AssignGlobal(s,ty,next),
                    }
                }
            } else if matches!(op, 4 | 11 | 12 | 16 | 17 | 27 | 28 | 29 | 30 | 41 | 42) {
                if operands.len() != 1 {
                    return Err(invalid("unary expression requires exactly one operand"));
                }
                if matches!(op,29|30|41|42) {
                    let direct=scalar_lvalue(graph,operands[0],params,locals,il).ok();
                    let (place,ty)=match direct {
                        Some((place,ty)) => (Some(place),ty),
                        None => {
                            let ty=integer_expression_type(il,graph,operands[0])?;
                            // The expression must designate addressable storage.
                            let _=lower_address(graph,operands[0],params,locals,il)?;
                            (None,ty)
                        }
                    };
                    let stride=if is_pointer_like(il,expression_type_id(graph,operands[0])?)? {
                        pointer_stride(il,expression_type_id(graph,operands[0])?)? as i64
                    } else {1};
                    let address=if place.is_none() {
                        Some(Box::new(lower_address(graph,operands[0],params,locals,il)?))
                    } else {None};
                    if matches!(op,29|30) {
                        let delta=if op==29 {stride} else {-stride};
                        match place {
                            Some(ScalarPlace::Ssa(i)) => Expr::PostLocal(i,ty,delta),
                            Some(ScalarPlace::Memory(s)) => Expr::PostGlobal(s,ty,delta),
                            None => {
                                let access=lvalue_memory_access(graph,operands[0],locals,il)?;
                                if access==MemoryAccess::Ordinary {
                                    Expr::PostAddress(ty,delta,address.unwrap())
                                } else {
                                    if ty.is_bool {return Err(invalid("qualified bool increment unsupported"));}
                                    Expr::PostQualified(ty,access,delta,address.unwrap())
                                }
                            },
                        }
                    } else {
                        let op=if op==41 {CCompoundOp::Add} else {CCompoundOp::Sub};
                        let amount=Box::new(Expr::Constant(stride));
                        match place {
                            Some(ScalarPlace::Ssa(i)) => Expr::CompoundLocal(i,ty,
                                promoted_integer_type(ty),op,amount),
                            Some(ScalarPlace::Memory(s)) => Expr::CompoundGlobal(s,ty,
                                promoted_integer_type(ty),op,amount),
                            None => {
                                let access=lvalue_memory_access(graph,operands[0],locals,il)?;
                                if access==MemoryAccess::Ordinary {
                                    Expr::CompoundAddress(ty,promoted_integer_type(ty),
                                        op,address.unwrap(),amount)
                                } else if is_pointer_like(il,
                                    expression_type_id(graph,operands[0])?)? {
                                    // Atomics on pointer *objects* require a
                                    // different representation and scaled arithmetic.
                                    return Err(invalid("qualified indirect pointer update unsupported"));
                                } else {
                                    // ++(*p) updates an integer pointee. This is
                                    // not the pointer-arithmetic case above.
                                    Expr::CompoundQualified(ty,promoted_integer_type(ty),
                                        access,op,address.unwrap(),amount)
                                }
                            },
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
                let direct=scalar_lvalue(graph,operands[0],params,locals,il);
                let (place,ty) = match direct {
                    Ok((place,ty)) => (Some(place),ty),
                    Err(_) => {
                        let ty=integer_expression_type(il,graph,operands[0])?;
                        // Require a structurally valid dereference/subscript
                        // rather than turning arbitrary expressions into lvalues.
                        let _=lower_address(graph,operands[0],params,locals,il)?;
                        (None,ty)
                    }
                };
                let rhs = Box::new(lower_c_expression(graph,operands[1],params,locals,il)?);
                let address=if place.is_none() {
                    Some(Box::new(lower_address(graph,operands[0],params,locals,il)?))
                } else {None};
                if op==18 {
                    match place {
                        Some(ScalarPlace::Ssa(i)) => Expr::AssignLocal(i,ty,rhs),
                        Some(ScalarPlace::Memory(s)) => Expr::AssignGlobal(s,ty,rhs),
                        None => {
                            if let Some(layout)=selected_bitfield(graph,operands[0],il)? {
                                Expr::StoreBitField(layout,address.unwrap(),rhs)
                            } else {
                                let access=lvalue_memory_access(graph,operands[0],locals,il)?;
                                if access==MemoryAccess::Ordinary {
                                    Expr::StoreAddress(ty,address.unwrap(),rhs)
                                } else {
                                    Expr::StoreQualified(ty,access,address.unwrap(),rhs)
                                }
                            }
                        }
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
                    let rhs_ty=integer_expression_type(il,graph,operands[1])?;
                    let compute_ty=if matches!(kind,CCompoundOp::Shl|CCompoundOp::Shr) {
                        promoted_integer_type(ty)
                    } else {
                        common_integer_type(ty,rhs_ty)
                    };
                    match place {
                        Some(ScalarPlace::Ssa(i)) => Expr::CompoundLocal(i,ty,compute_ty,kind,rhs),
                        Some(ScalarPlace::Memory(s)) => Expr::CompoundGlobal(s,ty,compute_ty,kind,rhs),
                        None => {
                            let access=lvalue_memory_access(graph,operands[0],locals,il)?;
                            if access==MemoryAccess::Ordinary {
                                Expr::CompoundAddress(ty,compute_ty,kind,address.unwrap(),rhs)
                            } else {
                                if ty.is_bool {return Err(invalid("qualified bool RMW unsupported"));}
                                Expr::CompoundQualified(ty,compute_ty,access,kind,address.unwrap(),rhs)
                            }
                        },
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
                let lhs_ptr=is_pointer_like(il,expression_type_id(graph,operands[0])?)?;
                let rhs_ptr=is_pointer_like(il,expression_type_id(graph,operands[1])?)?;
                if matches!(op,1|2) && (lhs_ptr ^ rhs_ptr) {
                    let (ptr,offset,ptr_ty) = if lhs_ptr {
                        (operands[0],operands[1],expression_type_id(graph,operands[0])?)
                    } else if op==1 {
                        (operands[1],operands[0],expression_type_id(graph,operands[1])?)
                    } else { return Err(invalid("integer minus pointer unsupported")); };
                    let stride=pointer_stride(il,ptr_ty)?;
                    let base=lower_c_expression(graph,ptr,params,locals,il)?;
                    let offset=lower_c_expression(graph,offset,params,locals,il)?;
                    let offset=if op==2 {Expr::Neg(Box::new(offset))} else {offset};
                    return Ok(wrap(Expr::PointerOffset(Box::new(base),
                        Box::new(offset),stride)));
                }
                if lhs_ptr || rhs_ptr {
                    // A null-pointer constant is the only integer we
                    // accept in an equality comparison with a pointer.
                    let literal_zero = |id: ExpressionId| graph.expression(id)
                        .is_some_and(|n| semantic(2,n.data.kind)==1
                            && n.data.constant_integer == Some(0));
                    let null_compare = matches!(op,5|6) &&
                        ((lhs_ptr && literal_zero(operands[1])) ||
                         (rhs_ptr && literal_zero(operands[0])));
                    if (lhs_ptr && rhs_ptr && matches!(op,5..=10)) || null_compare {
                        let cmp=match op {
                            5=>CompareOp::Equal,6=>CompareOp::NotEqual,
                            7=>CompareOp::Less,8=>CompareOp::LessEqual,
                            9=>CompareOp::Greater,10=>CompareOp::GreaterEqual,
                            _=>unreachable!(),
                        };
                        let a=lower_c_expression(graph,operands[0],params,locals,il)?;
                        let b=lower_c_expression(graph,operands[1],params,locals,il)?;
                        return Ok(wrap(Expr::UnsignedCompare(cmp,Box::new(a),Box::new(b))));
                    }
                    return Err(invalid("unsupported pointer operation"));
                }
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

/// Initialization statements are checked with their original local-store
/// definite-assignment semantics, then lowered to effectful qualified memory
/// accesses. This retains observable initialization without treating a
/// volatile/atomic local as an ordinary Cranelift SSA variable.
fn lower_qualified_initializers(
    body:&mut [Stmt],locals:&[&crate::LocalVariable],il:&Il,
) -> Result<(),CodegenError> {
    for stmt in body {
        match stmt {
            Stmt::Store(index,value) if *index<locals.len() => {
                let index=*index;
                let access=scalar_memory_access(il,locals[index].ty)?;
                if access!=MemoryAccess::Ordinary {
                    let ty=cfg_local_type(il,locals[index])?;
                    *stmt=Stmt::Eval(Expr::StoreQualified(
                        ty,access,Box::new(Expr::AddressLocal(index)),
                        Box::new(value.clone())));
                }
            }
            Stmt::If(_,yes,no) => {
                lower_qualified_initializers(yes,locals,il)?;
                lower_qualified_initializers(no,locals,il)?;
            }
            Stmt::While(_,inner) | Stmt::DoWhile(inner,_) |
            Stmt::Switch(_,inner) =>
                lower_qualified_initializers(inner,locals,il)?,
            Stmt::For {init,increment,body,..} => {
                lower_qualified_initializers(init,locals,il)?;
                lower_qualified_initializers(increment,locals,il)?;
                lower_qualified_initializers(body,locals,il)?;
            }
            _ => {}
        }
    }
    Ok(())
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
        let parameter_accesses=ids.iter().map(|sym|
            parameter_memory_access(il,*sym)).collect::<Result<Vec<_>,_>>()?;
        let locals: Vec<_> = il.local_variables.iter().filter(|v| v.routine == routine).collect();
        if locals.len() > 10_000 { return Err(invalid("too many locals")); }
        let mut local_types = locals.iter().map(|v| cfg_local_type(il,v))
            .collect::<Result<Vec<_>,_>>()?;
        let mut local_storage=locals.iter().map(|v| {
            let bytes=byte_size(il,v.ty)?;
            u32::try_from(bytes).map_err(|_|invalid("oversized local storage"))
        }).collect::<Result<Vec<_>,_>>()?;
        local_storage.extend(abi.parameters.iter().zip(&abi.aggregate_parameters)
            .map(|(t,aggregate)|u32::from(aggregate.unwrap_or(t.bits/8))));
        let mut local_alignments=locals.iter().map(|v|
            storage_alignment(il,v.ty).map(|a|a.trailing_zeros() as u8))
            .collect::<Result<Vec<_>,_>>()?;
        local_alignments.extend(args.iter().map(|arg|
            storage_alignment(il,arg.ty).map(|a|a.trailing_zeros() as u8))
            .collect::<Result<Vec<_>,_>>()?);
        local_types.extend(abi.parameters.iter().copied());
        let mut body = lower(&graph, roots[0], &ids, &locals, il)?;
        let labels=source_labels(&body)?;
        let initialized: HashSet<usize> =
            (locals.len()..(locals.len()+ids.len())).collect();
        let mut checked=initialized.clone();
        // Across user gotos, lexical statement order is not a reliable
        // definite-assignment analysis. Conservatively let Cranelift model
        // all scalar locals as addressable storage and validate the selected
        // target labels separately. Reading a skipped C initializer is UB.
        if !labels.is_empty() { checked.extend(0..locals.len()); }
        let has_fallthrough = check(&body, &mut checked,
            ids.len(), local_types.len(), 0)?;
        if has_fallthrough && abi.result.is_some() {
            return Err(invalid(format!("{name}: some path has no return")));
        }
        lower_qualified_initializers(&mut body,&locals,il)?;
        out.push(Routine { name: name.clone(), parameters: ids.len(),
            locals: locals.len(), body, abi: Some(abi),
            local_types: Some(local_types),local_storage:Some(local_storage),
            local_alignments:Some(local_alignments),parameter_accesses });
    }
    Ok(out)
}
/// Gather only variables whose addresses escape into pointer expressions.
/// SSA locals remain cheap virtual registers; addressed locals use stack slots.
fn addressed_expr(expr:&Expr,found:&mut HashSet<usize>) {
    match expr {
        Expr::AddressLocal(i) => {found.insert(*i);}
        Expr::Call(_,_,args) => for arg in args { addressed_expr(arg,found); },
        Expr::VlaAlloc(_,call) => addressed_expr(call,found),
        Expr::CallIndirect(callee,_,args) => {
            addressed_expr(callee,found);
            for arg in args {addressed_expr(arg,found);}
        },
        Expr::BlockValue(effects,result) => {
            for effect in effects {addressed_expr(effect,found);}
            addressed_expr(result,found);
        },
        Expr::StoreAddress(_,a,b) | Expr::StoreQualified(_,_,a,b) |
        Expr::CompoundQualified(_,_,_,_,a,b) |
        Expr::StoreBitField(_,a,b) |
        Expr::CompoundBitField(_,_,_,a,b) |
        Expr::CompoundAddress(_,_,_,a,b) |
        Expr::CopyStruct(_,a,b) | Expr::CopyBytes(_,a,b) |
        Expr::PointerOffset(a,b,_) | Expr::Add(a,b) | Expr::Sub(a,b) |
        Expr::Mul(a,b) | Expr::BitAnd(a,b) | Expr::BitOr(a,b) |
        Expr::BitXor(a,b) | Expr::Compare(_,a,b) | Expr::UnsignedCompare(_,a,b) |
        Expr::IntegerBinary(_,a,b) | Expr::Sequence(a,b) |
        Expr::StoreLocal(_,a,b) => {addressed_expr(a,found);addressed_expr(b,found);}
        Expr::Conditional(a,b,c) | Expr::Select(a,b,c) => {
            addressed_expr(a,found);addressed_expr(b,found);addressed_expr(c,found);
        }
        Expr::Normalize(_,a) | Expr::LoadAddress(_,a) |
        Expr::LoadQualified(_,_,a) |
        Expr::PackAggregate(_,a) |
        Expr::LoadBitField(_,a) | Expr::Neg(a) |
        Expr::AssignLocal(_,_,a) | Expr::CompoundLocal(_,_,_,_,a) |
        Expr::AssignGlobal(_,_,a) | Expr::CompoundGlobal(_,_,_,_,a) |
        Expr::PostAddress(_,_,a) | Expr::PostQualified(_,_,_,a) |
        Expr::PostBitField(_,_,a) =>
            addressed_expr(a,found),
        Expr::PostLocal(..) | Expr::PostGlobal(..) | Expr::Constant(_) |
        Expr::Parameter(_) | Expr::Local(_) | Expr::Global(_) |
        Expr::AddressGlobal(_) | Expr::AddressFunction(_) => {}
    }
}
fn addressed_stmt(body:&[Stmt],found:&mut HashSet<usize>) {
    for statement in body {
        match statement {
            Stmt::ZeroAggregate(index,_) |
            Stmt::InitializeAggregate(index,_) => {found.insert(*index);}
            Stmt::Store(_,e) | Stmt::Eval(e) | Stmt::Return(e) |
            Stmt::ReturnAggregate(_,e) => addressed_expr(e,found),
            Stmt::If(c,a,b) => { addressed_expr(c,found);
                addressed_stmt(a,found); addressed_stmt(b,found); },
            Stmt::While(c,body) | Stmt::DoWhile(body,c) =>
                { addressed_expr(c,found); addressed_stmt(body,found); },
            Stmt::Switch(selector,body) => {
                addressed_expr(selector,found);
                addressed_stmt(body,found);
            }
            Stmt::For{init,condition,increment,body} => {
                addressed_stmt(init,found);
                if let Some(condition)=condition {addressed_expr(condition,found);}
                addressed_stmt(body,found); addressed_stmt(increment,found);
            }
            Stmt::ReturnVoid | Stmt::Break | Stmt::Continue |
            Stmt::Goto(_) | Stmt::Label(_) | Stmt::Case(_) |
            Stmt::BreakSwitch => {}
        }
    }
}
fn local_address(fb:&mut FunctionBuilder<'_>,stack:&HashMap<usize,StackSlot>,
                 index:usize) -> Result<cranelift_codegen::ir::Value,CodegenError> {
    let slot=stack.get(&index).ok_or_else(||invalid("local lacks an addressable stack slot"))?;
    Ok(fb.ins().stack_addr(types::I64,*slot,0))
}
fn local_read(fb:&mut FunctionBuilder<'_>,vars:&[Variable],
              stack:&HashMap<usize,StackSlot>, local_types:&[CAbiInt],
              index:usize) -> Result<cranelift_codegen::ir::Value,CodegenError> {
    if stack.contains_key(&index) {
        let addr=local_address(fb,stack,index)?;
        Ok(super::memory::load_integer(fb,addr,*local_types.get(index)
            .ok_or_else(||invalid("missing addressable local type"))?))
    } else {
        Ok(fb.use_var(*vars.get(index).ok_or_else(||invalid("invalid local index"))?))
    }
}
fn local_write(fb:&mut FunctionBuilder<'_>,vars:&[Variable],
               stack:&HashMap<usize,StackSlot>,local_types:&[CAbiInt],
               index:usize,value:cranelift_codegen::ir::Value
) -> Result<cranelift_codegen::ir::Value,CodegenError> {
    let ty=*local_types.get(index).ok_or_else(||invalid("invalid local type"))?;
    let normalized=normalize_c_value(fb,value,ty);
    if stack.contains_key(&index) {
        let addr=local_address(fb,stack,index)?;
        super::memory::store_integer(fb,addr,normalized,ty);
    } else {
        fb.def_var(*vars.get(index).ok_or_else(||invalid("invalid local"))?,normalized);
    }
    Ok(normalized)
}
type CallValues = HashMap<crate::SymbolId,
    (cranelift_codegen::ir::FuncRef,CAbi)>;
type GlobalValues = HashMap<crate::SymbolId,
    (cranelift_codegen::ir::GlobalValue, CAbiInt, MemoryAccess)>;
fn global_address(
    fb: &mut FunctionBuilder<'_>, globals: &GlobalValues, symbol: crate::SymbolId,
) -> Result<cranelift_codegen::ir::Value, CodegenError> {
    let (value, _, _) = globals.get(&symbol).ok_or_else(|| invalid(
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
/// C qualified scalar increment: an atomic update must be indivisible,
/// whereas a volatile update is one observable load followed by one store.
/// The caller evaluates the lvalue address exactly once.
fn emit_qualified_post(
    fb:&mut FunctionBuilder<'_>,ty:CAbiInt,access:MemoryAccess,
    address:cranelift_codegen::ir::Value,delta:i64,
)->Result<cranelift_codegen::ir::Value,CodegenError> {
    if ty.is_bool {return Err(invalid("qualified bool increment unsupported"));}
    if access==MemoryAccess::Atomic {
        let value=fb.ins().iconst(types::I64,delta);
        let narrow=if ty.bits==64 {value}
            else {fb.ins().ireduce(ty.cranelift(),value)};
        let old=fb.ins().atomic_rmw(
            ty.cranelift(),MemFlags::new(),
            cranelift_codegen::ir::AtomicRmwOp::Add,address,narrow);
        let old=if ty.bits==64 {old}
            else if ty.signed {fb.ins().sextend(types::I64,old)}
            else {fb.ins().uextend(types::I64,old)};
        return Ok(normalize_c_value(fb,old,ty));
    }
    let old=super::memory::load_integer_access(fb,address,ty,access);
    let amount=fb.ins().iconst(types::I64,delta);
    let next=fb.ins().iadd(old,amount);
    super::memory::store_integer_access(fb,address,next,ty,access);
    Ok(old)
}

/// A C11 qualified compound update uses a native RMW for simple integer
/// operators; multiplying or dividing would need an atomic CAS loop.
/// Volatile operators use the same integer promotions as normal C, with
/// each memory effect retained by the backend.
/// C11 compound arithmetic without a native atomic_rmw opcode must
/// retry with compare-exchange. The RHS and lvalue address are evaluated by
/// the caller *before* entering this loop, so a CAS failure never replays
/// user-visible effects. All operations are sequentially consistent.
fn emit_atomic_cas_compound(
    fb:&mut FunctionBuilder<'_>,ty:CAbiInt,compute_ty:CAbiInt,
    op:CCompoundOp,address:cranelift_codegen::ir::Value,
    rhs:cranelift_codegen::ir::Value,
)->Result<cranelift_codegen::ir::Value,CodegenError> {
    if ty.is_bool {return Err(invalid("atomic bool CAS arithmetic unsupported"));}
    if !matches!(op,CCompoundOp::Mul|CCompoundOp::Div|CCompoundOp::Rem|
                 CCompoundOp::Shl|CCompoundOp::Shr) {
        return Err(invalid("unsupported atomic CAS arithmetic operator"));
    }
    let rhs=if matches!(op,CCompoundOp::Shl|CCompoundOp::Shr) {rhs}
        else {normalize_c_value(fb,rhs,compute_ty)};
    let retry=fb.create_block();
    let done=fb.create_block();
    fb.append_block_param(done,types::I64);
    fb.ins().jump(retry,&[]);
    fb.switch_to_block(retry);
    let old_raw=fb.ins().atomic_load(ty.cranelift(),MemFlags::new(),address);
    let old=if ty.bits==64 {old_raw}
        else if ty.signed {fb.ins().sextend(types::I64,old_raw)}
        else {fb.ins().uextend(types::I64,old_raw)};
    let old=normalize_c_value(fb,old,compute_ty);
    let result=compound_value(fb,old,rhs,compute_ty,op);
    let new=normalize_c_value(fb,result,ty);
    let new_raw=if ty.bits==64 {new}
        else {fb.ins().ireduce(ty.cranelift(),new)};
    let observed=fb.ins().atomic_cas(MemFlags::new(),address,old_raw,new_raw);
    let success=fb.ins().icmp(IntCC::Equal,observed,old_raw);
    fb.ins().brif(success,done,&[new.into()],retry,&[]);
    fb.seal_block(retry);
    fb.seal_block(done);
    fb.switch_to_block(done);
    Ok(fb.block_params(done)[0])
}
fn emit_qualified_compound(
    fb:&mut FunctionBuilder<'_>,ty:CAbiInt,compute_ty:CAbiInt,
    access:MemoryAccess,op:CCompoundOp,address:cranelift_codegen::ir::Value,
    rhs:cranelift_codegen::ir::Value,
)->Result<cranelift_codegen::ir::Value,CodegenError> {
    if ty.is_bool && access==MemoryAccess::Atomic {
        return Err(invalid("atomic bool RMW unsupported"));
    }
    if access==MemoryAccess::Atomic {
        use cranelift_codegen::ir::AtomicRmwOp;
        let atomic_op=match op {
            CCompoundOp::Add=>AtomicRmwOp::Add,
            CCompoundOp::Sub=>AtomicRmwOp::Sub,
            CCompoundOp::And=>AtomicRmwOp::And,
            CCompoundOp::Or=>AtomicRmwOp::Or,
            CCompoundOp::Xor=>AtomicRmwOp::Xor,
            _=>return emit_atomic_cas_compound(fb,ty,compute_ty,op,address,rhs),
        };
        let rhs=normalize_c_value(fb,rhs,ty);
        let narrow=if ty.bits==64 {rhs}
            else {fb.ins().ireduce(ty.cranelift(),rhs)};
        let old=fb.ins().atomic_rmw(ty.cranelift(),MemFlags::new(),atomic_op,address,narrow);
        let old=if ty.bits==64 {old}
            else if ty.signed {fb.ins().sextend(types::I64,old)}
            else {fb.ins().uextend(types::I64,old)};
        let result=match op {
            CCompoundOp::Add=>fb.ins().iadd(old,rhs),
            CCompoundOp::Sub=>fb.ins().isub(old,rhs),
            CCompoundOp::And=>fb.ins().band(old,rhs),
            CCompoundOp::Or=>fb.ins().bor(old,rhs),
            CCompoundOp::Xor=>fb.ins().bxor(old,rhs),
            _=>unreachable!(),
        };
        return Ok(normalize_c_value(fb,result,ty));
    }
    let before=super::memory::load_integer_access(fb,address,ty,access);
    let before=normalize_c_value(fb,before,compute_ty);
    let after=if matches!(op,CCompoundOp::Shl|CCompoundOp::Shr) {
        rhs
    } else {normalize_c_value(fb,rhs,compute_ty)};
    let result=compound_value(fb,before,after,compute_ty,op);
    Ok(super::memory::store_integer_access(fb,address,result,ty,access))
}
fn emit_expr(
    e: &Expr, fb: &mut FunctionBuilder<'_>,
    params: &[cranelift_codegen::ir::Value], vars: &[Variable],
    globals: &GlobalValues, stack: &HashMap<usize,StackSlot>,
    local_types: &[CAbiInt], calls: &CallValues,
) -> Result<cranelift_codegen::ir::Value, CodegenError> {
    Ok(match e {
        Expr::Constant(v) => fb.ins().iconst(types::I64, *v),
        Expr::Parameter(i) => *params.get(*i).ok_or_else(|| invalid("invalid parameter"))?,
        Expr::Local(i) => local_read(fb,vars,stack,local_types,*i)?,
        Expr::BlockValue(effects,result) => {
            for effect in effects {
                let _=emit_expr(effect,fb,params,vars,globals,stack,local_types,calls)?;
            }
            emit_expr(result,fb,params,vars,globals,stack,local_types,calls)?
        }
        Expr::VlaAlloc(_,call) => {
            // Lowered EDG call remains the single source of side effects.
            emit_expr(call,fb,params,vars,globals,stack,local_types,calls)?
        }
        Expr::AddressFunction(symbol) => {
            let (reference,_)=calls.get(symbol)
                .ok_or_else(||invalid("routine address has no declared function"))?;
            fb.ins().func_addr(types::I64,*reference)
        }
        Expr::Call(..) | Expr::CallIndirect(..) => {
            let (abi,arguments)=match e {
                Expr::Call(_,abi,args) | Expr::CallIndirect(_,abi,args) => (abi,args),
                _ => unreachable!(),
            };
            if arguments.len()!=abi.parameters.len() {
                return Err(invalid("call argument count/ABI mismatch"));
            }
            // C and C++ require the function designator to be evaluated
            // before entering the called function. Evaluate it once here.
            let indirect_target=if let Expr::CallIndirect(callee,_,_)=e {
                Some(emit_expr(callee,fb,params,vars,globals,stack,local_types,calls)?)
            } else {None};
            let direct_ref=if let Expr::Call(symbol,_,_)=e {
                let (reference,declared)=calls.get(symbol)
                    .ok_or_else(||invalid("direct call has no declared function"))?;
                if abi!=declared {return Err(invalid("direct call ABI mismatch"));}
                Some(*reference)
            } else {None};
            let plan=abi.passing_plan()?;
            let mut values=Vec::with_capacity(plan.machine_parameters().len());
            for (arg,entry) in arguments.iter().zip(&plan.parameters) {
                let value=emit_expr(arg,fb,params,vars,globals,stack,local_types,calls)?;
                match entry.mode {
                    target::PassingMode::AggregateByValue{bytes} if bytes>8 => {
                        // Evaluated once; the target ABI plan owns its pieces.
                        values.extend(target::ABI.pack_argument(fb,value,bytes)?);
                    }
                    target::PassingMode::Indirect =>
                        return Err(invalid("indirect ABI argument lowering unavailable")),
                    _ => {
                        let ty=entry.pieces.first().ok_or_else(||
                            invalid("missing scalar ABI piece"))?.ty;
                        values.push(if ty.bits==64 {value}
                            else {fb.ins().ireduce(ty.cranelift(),value)});
                    }
                }
            }
            let inst=if let Some(target)=indirect_target {
                // Signatures are target-call-convention descriptors. They
                // intentionally share the same passing plan as direct calls.
                let mut signature=cranelift_codegen::ir::Signature::new(
                    fb.func.signature.call_conv);
                for ty in plan.machine_parameters() {
                    signature.params.push(AbiParam::new(ty.cranelift()));
                }
                for ty in plan.machine_returns() {
                    signature.returns.push(AbiParam::new(ty.cranelift()));
                }
                let reference=fb.import_signature(signature);
                fb.ins().call_indirect(reference,target,&values)
            } else {
                fb.ins().call(direct_ref.ok_or_else(||invalid("missing direct callee"))?,&values)
            };
            if let Some(bytes)=abi.aggregate_result.filter(|bytes|*bytes>8) {
                let returned=fb.inst_results(inst).to_vec();
                if returned.len()!=2 {
                    return Err(invalid("two-register aggregate call result mismatch"));
                }
                let slot=fb.create_sized_stack_slot(StackSlotData::new(
                    StackSlotKind::ExplicitSlot,u32::from(bytes),3));
                let address=fb.ins().stack_addr(types::I64,slot,0);
                target::ABI.materialize_aggregate(fb,address,bytes,&returned)?;
                address
            } else if let Some(ty)=abi.result {
                let result=*fb.inst_results(inst).first()
                    .ok_or_else(||invalid("non-void call has no result"))?;
                if ty.bits==64 {result}
                else if ty.signed {fb.ins().sextend(types::I64,result)}
                else {fb.ins().uextend(types::I64,result)}
            } else {fb.ins().iconst(types::I64,0)}
        },
        Expr::AddressLocal(i) => local_address(fb,stack,*i)?,
        Expr::AddressGlobal(symbol) => global_address(fb,globals,*symbol)?,
        Expr::LoadAddress(ty,pointer) => {
            let address=emit_expr(pointer,fb,params,vars,globals,stack,local_types,calls)?;
            super::memory::load_integer(fb,address,*ty)
        }
        Expr::LoadQualified(ty,access,pointer) => {
            let address=emit_expr(pointer,fb,params,vars,globals,stack,local_types,calls)?;
            super::memory::load_integer_access(fb,address,*ty,*access)
        }
        Expr::PackAggregate(bytes,pointer) => {
            let address=emit_expr(pointer,fb,params,vars,globals,stack,local_types,calls)?;
            target::ABI.pack_result(fb,address,*bytes)?
        }
        Expr::LoadBitField(layout,pointer) => {
            let address=emit_expr(pointer,fb,params,vars,globals,stack,local_types,calls)?;
            super::memory::load_bitfield(fb,address,*layout)
        }
        Expr::StoreAddress(ty,pointer,value) => {
            let address=emit_expr(pointer,fb,params,vars,globals,stack,local_types,calls)?;
            let value=emit_expr(value,fb,params,vars,globals,stack,local_types,calls)?;
            super::memory::store_integer(fb,address,value,*ty)
        }
        Expr::StoreQualified(ty,access,pointer,value) => {
            let address=emit_expr(pointer,fb,params,vars,globals,stack,local_types,calls)?;
            let value=emit_expr(value,fb,params,vars,globals,stack,local_types,calls)?;
            super::memory::store_integer_access(fb,address,value,*ty,*access)
        }
        Expr::StoreBitField(layout,pointer,value) => {
            let address=emit_expr(pointer,fb,params,vars,globals,stack,local_types,calls)?;
            let value=emit_expr(value,fb,params,vars,globals,stack,local_types,calls)?;
            super::memory::store_bitfield(fb,address,value,*layout)
        }
        Expr::CompoundBitField(layout,compute_ty,op,pointer,rhs) => {
            let address=emit_expr(pointer,fb,params,vars,globals,stack,local_types,calls)?;
            let before=super::memory::load_bitfield(fb,address,*layout);
            let after=emit_expr(rhs,fb,params,vars,globals,stack,local_types,calls)?;
            let before=normalize_c_value(fb,before,*compute_ty);
            let after=if matches!(op,CCompoundOp::Shl|CCompoundOp::Shr) {
                after
            } else {normalize_c_value(fb,after,*compute_ty)};
            let computed=compound_value(fb,before,after,*compute_ty,*op);
            super::memory::store_bitfield(fb,address,computed,*layout)
        }
        Expr::PostBitField(layout,delta,pointer) => {
            let address=emit_expr(pointer,fb,params,vars,globals,stack,local_types,calls)?;
            let before=super::memory::load_bitfield(fb,address,*layout);
            let delta=fb.ins().iconst(types::I64,*delta);
            let next=fb.ins().iadd(before,delta);
            super::memory::store_bitfield(fb,address,next,*layout);
            before
        }
        Expr::CopyStruct(fields,destination,source) => {
            let dst=emit_expr(destination,fb,params,vars,globals,stack,local_types,calls)?;
            let src=emit_expr(source,fb,params,vars,globals,stack,local_types,calls)?;
            // Materialize all source field values before writing anything.
            // This also handles identical or overlapping source/destination
            // without assuming C struct padding bytes are observable values.
            let mut values=Vec::with_capacity(fields.len());
            for &(ty,offset) in fields {
                let addr=if offset==0 {src} else {fb.ins().iadd_imm(src,offset)};
                values.push(super::memory::load_integer(fb,addr,ty));
            }
            for (&(ty,offset),value) in fields.iter().zip(values) {
                let addr=if offset==0 {dst} else {fb.ins().iadd_imm(dst,offset)};
                super::memory::store_integer(fb,addr,value,ty);
            }
            dst
        }
        Expr::CopyBytes(bytes,destination,source) => {
            if *bytes==0 || *bytes>512 {
                return Err(invalid("union byte copy exceeds supported size"));
            }
            let dst=emit_expr(destination,fb,params,vars,globals,stack,local_types,calls)?;
            let src=emit_expr(source,fb,params,vars,globals,stack,local_types,calls)?;
            // Read the object representation as bytes, not typed inactive
            // union members. Read all bytes before any write to handle self
            // assignments and overlapping regions conservatively.
            let mut values=Vec::with_capacity(*bytes as usize);
            for offset in 0..*bytes {
                let addr=if offset==0 {src} else {
                    fb.ins().iadd_imm(src,i64::from(offset))
                };
                values.push(fb.ins().load(types::I8,MemFlags::new(),addr,0));
            }
            for (offset,value) in values.into_iter().enumerate() {
                let addr=if offset==0 {dst} else {
                    fb.ins().iadd_imm(dst,offset as i64)
                };
                fb.ins().store(MemFlags::new(),value,addr,0);
            }
            dst
        }
        Expr::CompoundAddress(ty,compute_ty,op,pointer,rhs) => {
            let address=emit_expr(pointer,fb,params,vars,globals,stack,local_types,calls)?;
            let before=super::memory::load_integer(fb,address,*ty);
            let after=emit_expr(rhs,fb,params,vars,globals,stack,local_types,calls)?;
            let before=normalize_c_value(fb,before,*compute_ty);
            let after=if matches!(op,CCompoundOp::Shl|CCompoundOp::Shr) {
                after
            } else {
                normalize_c_value(fb,after,*compute_ty)
            };
            let result=compound_value(fb,before,after,*compute_ty,*op);
            super::memory::store_integer(fb,address,result,*ty)
        }
        Expr::PostAddress(ty,delta,pointer) => {
            let address=emit_expr(pointer,fb,params,vars,globals,stack,local_types,calls)?;
            let old=super::memory::load_integer(fb,address,*ty);
            let delta=fb.ins().iconst(types::I64,*delta);
            let value=fb.ins().iadd(old,delta);
            super::memory::store_integer(fb,address,value,*ty);
            old
        }
        Expr::PostQualified(ty,access,delta,pointer) => {
            let address=emit_expr(pointer,fb,params,vars,globals,stack,local_types,calls)?;
            emit_qualified_post(fb,*ty,*access,address,*delta)?
        }
        Expr::CompoundQualified(ty,compute_ty,access,op,pointer,rhs) => {
            let address=emit_expr(pointer,fb,params,vars,globals,stack,local_types,calls)?;
            let value=emit_expr(rhs,fb,params,vars,globals,stack,local_types,calls)?;
            emit_qualified_compound(fb,*ty,*compute_ty,*access,*op,address,value)?
        }
        Expr::PointerOffset(base,index,stride) => {
            let base=emit_expr(base,fb,params,vars,globals,stack,local_types,calls)?;
            let index=emit_expr(index,fb,params,vars,globals,stack,local_types,calls)?;
            let stride=fb.ins().iconst(types::I64,*stride as i64);
            let offset=fb.ins().imul(index,stride);
            fb.ins().iadd(base,offset)
        },
        Expr::Global(symbol) => {
            let (_,ty,access) = globals.get(symbol).ok_or_else(|| invalid("unknown global"))?;
            let address = global_address(fb,globals,*symbol)?;
            super::memory::load_integer_access(fb,address,*ty,*access)
        }
        Expr::AssignGlobal(symbol,ty,rhs) => {
            let value = emit_expr(rhs,fb,params,vars,globals,stack,local_types,calls)?;
            let address = global_address(fb,globals,*symbol)?;
            let (_,_,access)=globals.get(symbol)
                .ok_or_else(||invalid("global assignment access missing"))?;
            super::memory::store_integer_access(fb,address,value,*ty,*access)
        }
        Expr::PostGlobal(symbol,ty,delta) => {
            let (_,_,access)=globals.get(symbol)
                .ok_or_else(||invalid("global increment access missing"))?;
            if *access==MemoryAccess::Atomic {
                if ty.is_bool {return Err(invalid("atomic bool increment unsupported"));}
                let address=global_address(fb,globals,*symbol)?;
                let value=fb.ins().iconst(types::I64,*delta);
                let value=if ty.bits==64 {value}
                    else {fb.ins().ireduce(ty.cranelift(),value)};
                let old=fb.ins().atomic_rmw(
                    ty.cranelift(),MemFlags::new(),
                    cranelift_codegen::ir::AtomicRmwOp::Add,address,value);
                let old=if ty.bits==64 {old}
                    else if ty.signed {fb.ins().sextend(types::I64,old)}
                    else {fb.ins().uextend(types::I64,old)};
                return Ok(normalize_c_value(fb,old,*ty));
            }
            let address = global_address(fb,globals,*symbol)?;
            let old = super::memory::load_integer_access(fb,address,*ty,*access);
            let delta = fb.ins().iconst(types::I64,*delta);
            let value = fb.ins().iadd(old,delta);
            super::memory::store_integer_access(fb,address,value,*ty,*access);
            old
        }
        Expr::CompoundGlobal(symbol,ty,compute_ty,op,rhs) => {
            // Evaluate destination once, then RHS, then perform one store.
            let (_,_,access)=globals.get(symbol)
                .ok_or_else(||invalid("global compound access missing"))?;
            if *access==MemoryAccess::Atomic {
                // Reuse the same integer RMW/CAS implementation for global
                // storage and qualified pointer lvalues, keeping the
                // target-owned memory-order semantics in one place.
                let address=global_address(fb,globals,*symbol)?;
                let operand=emit_expr(rhs,fb,params,vars,globals,stack,local_types,calls)?;
                return emit_qualified_compound(
                    fb,*ty,*compute_ty,*access,*op,address,operand);
            }
            let address = global_address(fb,globals,*symbol)?;
            let old = super::memory::load_integer_access(fb,address,*ty,*access);
            let after = emit_expr(rhs,fb,params,vars,globals,stack,local_types,calls)?;
            let before = normalize_c_value(fb,old,*compute_ty);
            let after = if matches!(op,CCompoundOp::Shl|CCompoundOp::Shr) {
                after
            } else {
                normalize_c_value(fb,after,*compute_ty)
            };
            let result = compound_value(fb,before,after,*compute_ty,*op);
            super::memory::store_integer_access(fb,address,result,*ty,*access)
        },
        Expr::StoreLocal(..) => return Err(invalid("nested assignment unsupported")),
        Expr::Normalize(ty, a) => {
            let value = emit_expr(a,fb,params,vars,globals,stack,local_types,calls)?;
            normalize_c_value(fb, value, *ty)
        }
        Expr::Neg(a) => {
            let v = emit_expr(a,fb,params,vars,globals,stack,local_types,calls)?;
            fb.ins().ineg(v)
        }
        Expr::IntegerBinary(op,a,b) => {
            let lhs = emit_expr(a,fb,params,vars,globals,stack,local_types,calls)?;
            let rhs = emit_expr(b,fb,params,vars,globals,stack,local_types,calls)?;
            emit_integer_binary(*op, fb, lhs, rhs)
        }
        Expr::Sequence(a,b) => {
            let _ = emit_expr(a,fb,params,vars,globals,stack,local_types,calls)?;
            emit_expr(b,fb,params,vars,globals,stack,local_types,calls)?
        }
        Expr::AssignLocal(index,_ty,value) => {
            let raw = emit_expr(value,fb,params,vars,globals,stack,local_types,calls)?;
            local_write(fb,vars,stack,local_types,*index,raw)?
        }
        Expr::PostLocal(index,_ty,delta) => {
            let old = local_read(fb,vars,stack,local_types,*index)?;
            let amount = fb.ins().iconst(types::I64,*delta);
            let next = fb.ins().iadd(old,amount);
            local_write(fb,vars,stack,local_types,*index,next)?;
            old
        }
        Expr::CompoundLocal(index,_ty,compute_ty,op,rhs) => {
            let before = local_read(fb,vars,stack,local_types,*index)?;
            let after = emit_expr(rhs,fb,params,vars,globals,stack,local_types,calls)?;
            let before = normalize_c_value(fb,before,*compute_ty);
            let after = if matches!(op,CCompoundOp::Shl|CCompoundOp::Shr) {
                after
            } else {
                normalize_c_value(fb,after,*compute_ty)
            };
            let result = compound_value(fb,before,after,*compute_ty,*op);
            local_write(fb,vars,stack,local_types,*index,result)?
        }
        Expr::Conditional(c,a,b) => {
            let test = emit_expr(c,fb,params,vars,globals,stack,local_types,calls)?;
            let flag = fb.ins().icmp_imm(IntCC::NotEqual,test,0);
            let yes = fb.create_block();
            let no = fb.create_block();
            let merge = fb.create_block();
            fb.append_block_param(merge, types::I64);
            fb.ins().brif(flag, yes, &[], no, &[]);
            fb.seal_block(yes);
            fb.seal_block(no);
            fb.switch_to_block(yes);
            let yes_value = emit_expr(a,fb,params,vars,globals,stack,local_types,calls)?;
            fb.ins().jump(merge, &[yes_value.into()]);
            fb.switch_to_block(no);
            let no_value = emit_expr(b,fb,params,vars,globals,stack,local_types,calls)?;
            fb.ins().jump(merge, &[no_value.into()]);
            fb.seal_block(merge);
            fb.switch_to_block(merge);
            fb.block_params(merge)[0]
        }
        Expr::Add(a,b) | Expr::Sub(a,b) | Expr::Mul(a,b) |
        Expr::BitAnd(a,b) | Expr::BitOr(a,b) | Expr::BitXor(a,b) => {
            let a = emit_expr(a,fb,params,vars,globals,stack,local_types,calls)?;
            let b = emit_expr(b,fb,params,vars,globals,stack,local_types,calls)?;
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
            let a = emit_expr(a,fb,params,vars,globals,stack,local_types,calls)?;
            let b = emit_expr(b,fb,params,vars,globals,stack,local_types,calls)?;
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
            let test = emit_expr(c,fb,params,vars,globals,stack,local_types,calls)?;
            let a = emit_expr(a,fb,params,vars,globals,stack,local_types,calls)?;
            let b = emit_expr(b,fb,params,vars,globals,stack,local_types,calls)?;
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
    globals: &GlobalValues,stack:&HashMap<usize,StackSlot>,
    calls: &CallValues, labels:&HashMap<crate::SymbolId,Block>,
    switch_break:Option<Block>,
) -> Result<bool, CodegenError> {
    let mut live = true;
    for stmt in stmts {
        if !live && !matches!(stmt,Stmt::Label(_)) {continue;}
        match stmt {
            Stmt::BreakSwitch => {
                let dest=switch_break.ok_or_else(||
                    invalid("switch break outside switch"))?;
                fb.ins().jump(dest,&[]);
                live=false;
            }
            Stmt::Case(_) => return Err(invalid("case label outside switch dispatcher")),
            Stmt::Switch(selector,body) => {
                let cases=check_switch_segments(body)?;
                let exit=fb.create_block();
                let blocks:Vec<Block>=(0..cases.len()).map(|_|
                    fb.create_block()).collect();
                // Evaluate C's switch selector exactly once.
                let selector=emit_expr(selector,fb,args,vars,globals,stack,local_types,calls)?;
                let default=cases.iter().position(|(v,_)|v.is_none());
                for (idx,(value,_)) in cases.iter().enumerate() {
                    let Some(value)=value else {continue;};
                    let next=fb.create_block();
                    let match_case=fb.ins().icmp_imm(IntCC::Equal,selector,*value);
                    fb.ins().brif(match_case,blocks[idx],&[],next,&[]);
                    fb.switch_to_block(next);
                    fb.seal_block(next);
                }
                fb.ins().jump(default.map(|i|blocks[i]).unwrap_or(exit),&[]);
                let mut final_fallthrough=false;
                for (i,(_,chunk)) in cases.iter().enumerate() {
                    fb.switch_to_block(blocks[i]);
                    let arm_live=emit_body(chunk,fb,args,vars,outer,local_types,
                        result,globals,stack,calls,labels,Some(exit))?;
                    if arm_live {
                        let target=blocks.get(i+1).copied().unwrap_or(exit);
                        fb.ins().jump(target,&[]);
                    }
                    final_fallthrough=arm_live;
                }
                for block in blocks {fb.seal_block(block);}
                fb.seal_block(exit);
                if default.is_none() || may_break_current_switch(body) ||
                    final_fallthrough {
                    fb.switch_to_block(exit);
                } else {
                    // No path reaches the switch successor; Cranelift must
                    // not invent an outgoing branch or demand another return.
                    live=false;
                }
            }
            Stmt::Goto(symbol) => {
                let target=*labels.get(symbol).ok_or_else(||
                    invalid("source goto has no predeclared block"))?;
                fb.ins().jump(target,&[]);
                live=false;
            }
            Stmt::Label(symbol) => {
                let target=*labels.get(symbol).ok_or_else(||
                    invalid("source label has no predeclared block"))?;
                if live {fb.ins().jump(target,&[]);}
                fb.switch_to_block(target);
                live=true;
            }
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
            Stmt::InitializeAggregate(index,bytes) => {
                if bytes.is_empty() || bytes.len()>4096 {
                    return Err(invalid("aggregate native image exceeds 4 KiB limit"));
                }
                let base=local_address(fb,stack,*index)?;
                let mut offset=0usize;
                while offset<bytes.len() {
                    let remaining=bytes.len()-offset;
                    let width=if remaining>=8 {8} else if remaining>=4 {4}
                        else if remaining>=2 {2} else {1};
                    let mut raw=[0u8;8];
                    raw[..width].copy_from_slice(&bytes[offset..offset+width]);
                    let value=fb.ins().iconst(types::I64,u64::from_le_bytes(raw) as i64);
                    let address=if offset==0 {base} else {
                        fb.ins().iadd_imm(base,offset as i64)
                    };
                    let ty=CAbiInt{bits:(width*8) as u8,signed:false,is_bool:false};
                    super::memory::store_integer(fb,address,value,ty);
                    offset+=width;
                }
            }
            Stmt::ZeroAggregate(index,bytes) => {
                if *bytes==0 || *bytes>4096 {
                    return Err(invalid("aggregate zero initialization too large"));
                }
                let base=local_address(fb,stack,*index)?;
                let zero=fb.ins().iconst(types::I64,0);
                let mut offset=0u32;
                while offset<*bytes {
                    let remaining=*bytes-offset;
                    let width=if remaining>=8 {8} else if remaining>=4 {4}
                        else if remaining>=2 {2} else {1};
                    let address=if offset==0 {base} else {
                        fb.ins().iadd_imm(base,i64::from(offset))
                    };
                    let ty=CAbiInt{bits:(width*8) as u8,signed:false,is_bool:false};
                    super::memory::store_integer(fb,address,zero,ty);
                    offset+=width;
                }
            }
            Stmt::Store(i,e) => {
                let v = emit_expr(e,fb,args,vars,globals,stack,local_types,calls)?;
                local_write(fb,vars,stack,local_types,*i,v)?;
            }
            Stmt::Eval(e) => { let _ = emit_expr(e,fb,args,vars,globals,stack,local_types,calls)?; }
            Stmt::Return(e) => {
                let ty = result.ok_or_else(|| invalid("value returned from void routine"))?;
                if fb.func.signature.returns.len()!=1 {
                    return Err(invalid("scalar C return incompatible with machine signature"));
                }
                let v = emit_expr(e,fb,args,vars,globals,stack,local_types,calls)?;
                let v = if ty.bits == 64 { v }
                    else { fb.ins().ireduce(ty.cranelift(), v) };
                fb.ins().return_(&[v]);
                live = false;
            }
            Stmt::ReturnAggregate(bytes,address) => {
                if result.is_none() || !(9..=16).contains(bytes) ||
                   fb.func.signature.returns.len()!=2 {
                    return Err(invalid("two-GPR return ABI/signature mismatch"));
                }
                let address=emit_expr(address,fb,args,vars,globals,stack,local_types,calls)?;
                let pieces=target::ABI.pack_argument(fb,address,*bytes)?;
                if pieces.len()!=2 {
                    return Err(invalid("two-GPR return must produce exactly two values"));
                }
                fb.ins().return_(&pieces);
                live=false;
            }
            Stmt::ReturnVoid => {
                if result.is_some() { return Err(invalid("missing non-void return value")); }
                fb.ins().return_(&[]);
                live = false;
            }
            Stmt::If(cond, yes, no) => {
                let c = emit_expr(cond,fb,args,vars,globals,stack,local_types,calls)?;
                let flag = fb.ins().icmp_imm(IntCC::NotEqual,c,0);
                let t = fb.create_block();
                let f = fb.create_block();
                fb.ins().brif(flag,t,&[],f,&[]);
                fb.seal_block(t);
                fb.seal_block(f);
                fb.switch_to_block(t);
                let t_live = emit_body(yes,fb,args,vars,outer,local_types,result,globals,stack,calls,labels,switch_break)?;
                let mut merge = None;
                if t_live {
                    let m = fb.create_block();
                    fb.ins().jump(m,&[]);
                    merge = Some(m);
                }
                fb.switch_to_block(f);
                let f_live = emit_body(no,fb,args,vars,outer,local_types,result,globals,stack,calls,labels,switch_break)?;
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
                let c = emit_expr(cond,fb,args,vars,globals,stack,local_types,calls)?;
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
                if emit_body(body,fb,args,vars,Some(targets),local_types,result,globals,stack,calls,labels,switch_break)? {
                    fb.ins().jump(head,&[]);
                }
                // Backedges and break edges are complete.
                fb.seal_block(head);
                fb.seal_block(exit);
                fb.switch_to_block(exit);
            }
            Stmt::For { init, condition, increment, body } => {
                if !emit_body(init,fb,args,vars,outer,local_types,result,globals,stack,calls,labels,switch_break)? {
                    live = false;
                    continue;
                }
                let head = fb.create_block();
                let body_block = fb.create_block();
                let step = fb.create_block();
                let exit = fb.create_block();
                fb.ins().jump(head, &[]);
                fb.switch_to_block(head);
                if let Some(condition)=condition {
                    let c = emit_expr(condition,fb,args,vars,globals,stack,local_types,calls)?;
                    let flag = fb.ins().icmp_imm(IntCC::NotEqual, c, 0);
                    fb.ins().brif(flag, body_block, &[], exit, &[]);
                } else {
                    fb.ins().jump(body_block, &[]);
                }
                fb.seal_block(body_block);
                fb.switch_to_block(body_block);
                let saw_break = Cell::new(false);
                let saw_continue = Cell::new(false);
                let targets = LoopTargets {
                    break_to: exit, continue_to: step,
                    saw_break: &saw_break, saw_continue: &saw_continue,
                };
                let body_live = emit_body(body,fb,args,vars,Some(targets),local_types,result,globals,stack,calls,labels,switch_break)?;
                if body_live { fb.ins().jump(step, &[]); }
                fb.seal_block(step);
                if body_live || saw_continue.get() {
                    fb.switch_to_block(step);
                    if emit_body(increment,fb,args,vars,Some(targets),local_types,result,globals,stack,calls,labels,switch_break)? {
                        fb.ins().jump(head, &[]);
                    }
                }
                fb.seal_block(head);
                fb.seal_block(exit);
                if condition.is_some() || saw_break.get() {
                    fb.switch_to_block(exit);
                } else {
                    live=false;
                }
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
                let body_live = emit_body(body,fb,args,vars,Some(targets),local_types,result,globals,stack,calls,labels,switch_break)?;
                if body_live { fb.ins().jump(test_block, &[]); }
                fb.seal_block(test_block);
                if body_live || saw_continue.get() {
                    fb.switch_to_block(test_block);
                    let test = emit_expr(cond,fb,args,vars,globals,stack,local_types,calls)?;
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
    access: MemoryAccess,
    linkage: Linkage,
    initializer: Option<Vec<u8>>,
    relocations: Vec<(usize,crate::SymbolId,i64)>,
    storage_bytes: usize,
    alignment: u64,
}

/// Resolve only referenced C globals. Unused declarations of unrelated
/// C++ types cannot make an otherwise scalar routine un-compilable.
fn compile_globals(il: &Il) -> Result<Vec<ScalarGlobal>, CodegenError> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    let references=il.expressions.iter()
        .filter(|expr|semantic(2,expr.kind)==2).filter_map(|expr|expr.symbol_id)
        .chain(il.constant_addresses.iter().map(|a|a.symbol))
        .chain(il.global_aggregate_relocations.iter().flat_map(|r|[r.symbol,r.target]));
    for id in references {
        if !il.global_variables.iter().any(|g|g.symbol==id) ||
           !seen.insert(id) {continue;}
        let (name,scalar) = global_value_type(il,id)?;
        let declared=il.symbol_types.iter().find(|t|t.symbol==id)
            .ok_or_else(||invalid("global missing declared type"))?.ty;
        let alignment=storage_alignment(il,declared)?;
        let ty=scalar.unwrap_or(CAbiInt{bits:64,signed:false,is_bool:false});
        let storage_bytes=if scalar.is_some() {
            let bytes = usize::from(ty.bits / 8);
            if bytes == 0 || ty.bits % 8 != 0 {
                return Err(invalid("global scalar has invalid storage width"));
            }
            bytes
        } else {
                let declared=il.symbol_types.iter().find(|t|t.symbol==id)
                    .ok_or_else(||invalid("array without declared type"))?.ty;
                byte_size(il,declared)? as usize
            };
        let meta = il.global_variables.iter().find(|g| g.symbol == id)
            .ok_or_else(|| invalid("missing global metadata"))?;
        let defined = meta.linkage != 1 || meta.explicit_initializer;
        let linkage = if !defined { Linkage::Import }
            else if meta.linkage == 2 { Linkage::Local }
            else { Linkage::Export };
        let initializer = if meta.explicit_initializer {
            if scalar.is_none() {
                let matches=il.global_aggregate_initializers.iter()
                    .filter(|image|image.symbol==id).collect::<Vec<_>>();
                if matches.len()!=1 {
                    return Err(invalid(format!(
                        "global {name} has no single supported aggregate initializer")));
                }
                let image=matches[0];
                if canonical_abi_type(il,image.ty)?!=
                    canonical_abi_type(il,declared)? ||
                    image.bytes.len()!=storage_bytes {
                    return Err(invalid("global native aggregate image/type size mismatch"));
                }
                Some(image.bytes.clone())
            } else {
                let entry = il.initializer_integers.iter().find(|v| v.symbol == id)
                    .ok_or_else(|| invalid(format!(
                        "global {name} has unsupported non-integer initializer")))?;
                let bytes = entry.value.to_le_bytes();
                Some(bytes[..(ty.bits/8) as usize].to_vec())
            }
        } else { None };
        let mut relocations=Vec::new();
        for reloc in il.global_aggregate_relocations.iter().filter(|r|r.symbol==id) {
            let start=usize::try_from(reloc.offset)
                .map_err(|_|invalid("aggregate relocation offset out of range"))?;
            if scalar.is_some() || !meta.explicit_initializer ||
                start.checked_add(8).is_none_or(|end|end>storage_bytes) ||
                relocations.iter().any(|(offset,_,_)|*offset==start) {
                return Err(invalid("unsupported/overlapping aggregate pointer relocation"));
            }
            relocations.push((start,reloc.target,reloc.addend));
        }
        let access=scalar_memory_access(il,declared)?;
        if access!=MemoryAccess::Ordinary && scalar.is_none() {
            return Err(invalid("qualified aggregate globals unsupported"));
        }
        result.push(ScalarGlobal {
            symbol:id,name,ty,access,linkage,initializer,relocations,storage_bytes,alignment
        });
    }
    Ok(result)
}
#[cfg(test)]
fn emit_routines(functions: &[Routine]) -> Result<Vec<u8>, CodegenError> {
    emit_routines_with_globals(functions,&[],&[])
}
fn emit_routines_with_globals(
    functions: &[Routine], globals: &[ScalarGlobal], calls: &[CallDeclaration],
) -> Result<Vec<u8>, CodegenError> {
    let mut flags = settings::builder();
    flags.set("opt_level","speed").map_err(|e| CodegenError::Backend(e.to_string()))?;
    let isa = cranelift_codegen::isa::lookup_by_name(target::ABI.isa_name())
        .map_err(|e| CodegenError::UnsupportedTarget(e.to_string()))?
        .finish(settings::Flags::new(flags))
        .map_err(|e| CodegenError::UnsupportedTarget(e.to_string()))?;
    let builder = ObjectBuilder::new(isa,"edgcpp",cranelift_module::default_libcall_names())
        .map_err(|e| CodegenError::Backend(e.to_string()))?;
    let mut module = ObjectModule::new(builder);
    // Declare every global before defining any initializer, since data
    // relocations may reference a later-defined variable.
    let mut declared = HashMap::new();
    for g in globals {
        let id = module.declare_data(&g.name,g.linkage,true,false)
            .map_err(|e| CodegenError::Backend(e.to_string()))?;
        if declared.insert(g.symbol,(id,g.ty,g.access)).is_some() {
            return Err(invalid("duplicate global identity"));
        }
    }
    for g in globals {
        if g.linkage == Linkage::Import {continue;}
        let id = declared.get(&g.symbol).ok_or_else(||invalid("missing global identity"))?.0;
        let mut data = cranelift_module::DataDescription::new();
        data.set_align(g.alignment);
        if let Some(bytes) = &g.initializer {
            data.define(bytes.clone().into_boxed_slice());
        } else {
            data.define_zeroinit(g.storage_bytes);
        }
        for &(offset,target,addend) in &g.relocations {
            if offset>u32::MAX as usize {
                return Err(invalid("aggregate relocation offset exceeds ELF bound"));
            }
            let target_id=declared.get(&target)
                .ok_or_else(||invalid("aggregate relocation target not declared"))?.0;
            let global=module.declare_data_in_data(target_id,&mut data);
            data.write_data_addr(offset as u32,global,addend);
        }
        module.define_data(id,&data)
            .map_err(|e| CodegenError::Backend(e.to_string()))?;
    }
    // Predeclare defined and external functions before compiling any body:
    // forward references and mutual recursion use one Module symbol table.
    let mut by_name: HashMap<String,(cranelift_module::FuncId,CAbi)> = HashMap::new();
    for func in functions {
        let abi=func.abi.clone().unwrap_or_else(|| CAbi {
            parameters: vec![CAbiInt{bits:64,signed:true,is_bool:false};func.parameters],
            aggregate_parameters:vec![None;func.parameters],aggregate_result:None,
            result: Some(CAbiInt{bits:64,signed:true,is_bool:false}),
        });
        if abi.parameters.len()!=func.parameters {
            return Err(invalid("defined function parameter count mismatch"));
        }
        let mut signature=module.make_signature();
        let plan=abi.passing_plan()?;
        for ty in plan.machine_parameters() {signature.params.push(AbiParam::new(ty.cranelift()));}
        for ty in plan.machine_returns() {signature.returns.push(AbiParam::new(ty.cranelift()));}
        let id=module.declare_function(&func.name,Linkage::Export,&signature)
            .map_err(|e|CodegenError::Backend(e.to_string()))?;
        if by_name.insert(func.name.clone(),(id,abi)).is_some() {
            return Err(invalid("duplicate function export"));
        }
    }
    for callee in calls {
        if let Some((_,abi))=by_name.get(&callee.name) {
            if abi!=&callee.abi { return Err(invalid("direct-call declaration ABI mismatch")); }
            continue;
        }
        let mut signature=module.make_signature();
        let plan=callee.abi.passing_plan()?;
        for ty in plan.machine_parameters() {
            signature.params.push(AbiParam::new(ty.cranelift()));
        }
        for ty in plan.machine_returns() {
            signature.returns.push(AbiParam::new(ty.cranelift()));
        }
        let id=module.declare_function(&callee.name,Linkage::Import,&signature)
            .map_err(|e|CodegenError::Backend(e.to_string()))?;
        by_name.insert(callee.name.clone(),(id,callee.abi.clone()));
    }
    for func in functions {
        if func.name.is_empty() || func.name.as_bytes().contains(&0) {
            return Err(invalid("invalid export name"));
        }
        let mut context = module.make_context();
        let abi = func.abi.clone().unwrap_or_else(|| CAbi {
            parameters: vec![CAbiInt { bits: 64, signed: true, is_bool: false }; func.parameters],
            aggregate_parameters:vec![None;func.parameters],aggregate_result:None,
            result: Some(CAbiInt { bits: 64, signed: true, is_bool: false }),
        });
        if abi.parameters.len() != func.parameters {
            return Err(invalid("ABI parameter count mismatch"));
        }
        let plan=abi.passing_plan()?;
        for param in plan.machine_parameters() {
            context.func.signature.params.push(AbiParam::new(param.cranelift()));
        }
        for result in plan.machine_returns() {
            context.func.signature.returns.push(AbiParam::new(result.cranelift()));
        }
        let id=by_name.get(&func.name)
            .ok_or_else(||invalid("undeclared function export"))?.0;
        let call_values: CallValues = calls.iter().map(|call| {
            let (id,abi)=by_name.get(&call.name)
                .ok_or_else(||invalid("undeclared direct call target"))?;
            let reference=module.declare_func_in_func(*id,&mut context.func);
            Ok((call.symbol,(reference,abi.clone())))
        }).collect::<Result<HashMap<_,_>,CodegenError>>()?;
        let global_values: GlobalValues = declared.iter().map(|(symbol,(id,ty,access))|
            (*symbol,(module.declare_data_in_func(*id,&mut context.func),*ty,*access))).collect();
        let mut fb_context = FunctionBuilderContext::new();
        {
            let mut fb = FunctionBuilder::new(&mut context.func,&mut fb_context);
            let entry = fb.create_block();
            fb.append_block_params_for_function_params(entry);
            fb.switch_to_block(entry);
            fb.seal_block(entry);
            let machine_values=fb.block_params(entry).to_vec();
            let decoded=target::ABI.decode_parameters(
                &mut fb,&plan,&machine_values)?;
            let params:Vec<_>=decoded.iter().map(|pieces|
                pieces.first().copied().ok_or_else(||
                    invalid("missing decoded source argument")))
                .collect::<Result<_,_>>()?;
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
            let mut addressed=HashSet::new();
            addressed_stmt(&func.body,&mut addressed);
            let label_ids=source_labels(&func.body)?;
            // A source goto may bypass lexical SSA initialization; address
            // all local/parameter slots so stores and reads preserve C memory
            // semantics irrespective of the predecessor edge.
            if !label_ids.is_empty() {addressed.extend(0..local_types.len());}
            let labels:HashMap<_,_>=label_ids.into_iter()
                .map(|id|(id,fb.create_block())).collect();
            // Aggregate formal parameters are addressable C objects,
            // not SSA scalar values. Materialize exactly the native object
            // width from the incoming INTEGER-class register.
            for (index,aggregate) in abi.aggregate_parameters.iter().enumerate() {
                if aggregate.is_some() {addressed.insert(func.locals+index);}
            }
            if !func.parameter_accesses.is_empty() &&
               func.parameter_accesses.len()!=func.parameters {
                return Err(invalid("qualified formal access metadata count mismatch"));
            }
            for (index,access) in func.parameter_accesses.iter().enumerate() {
                if *access!=MemoryAccess::Ordinary {
                    addressed.insert(func.locals+index);
                }
            }
            let storage=func.local_storage.clone().unwrap_or_else(||
                local_types.iter().map(|t|u32::from(t.bits/8)).collect());
            if storage.len()!=local_types.len() {
                return Err(invalid("stack storage/type count mismatch"));
            }
            let alignments=func.local_alignments.clone()
                .unwrap_or_else(||vec![0;local_types.len()]);
            if alignments.len()!=storage.len() {
                return Err(invalid("stack alignment/type count mismatch"));
            }
            let mut stack=HashMap::new();
            for index in addressed {
                let bytes=*storage.get(index).ok_or_else(||invalid("invalid stack local"))?;
                let slot=fb.create_sized_stack_slot(
                    StackSlotData::new(StackSlotKind::ExplicitSlot,bytes,alignments[index]));
                stack.insert(index,slot);
            }
            if func.abi.is_some() {
                for (i,values) in decoded.iter().enumerate() {
                    let first=params[i];
                    if let Some(bytes)=abi.aggregate_parameters[i] {
                        let address=local_address(&mut fb,&stack,func.locals+i)?;
                        target::ABI.materialize_aggregate(
                            &mut fb,address,bytes,values)?;
                    } else {
                        let access=func.parameter_accesses.get(i).copied()
                            .unwrap_or(MemoryAccess::Ordinary);
                        if access==MemoryAccess::Ordinary {
                            local_write(&mut fb,&vars,&stack,&local_types,
                                func.locals+i,first)?;
                        } else {
                            let index=func.locals+i;
                            let address=local_address(&mut fb,&stack,index)?;
                            super::memory::store_integer_access(
                                &mut fb,address,first,local_types[index],access);
                        }
                    }
                }
            }
            let body = emit_body(
                &func.body, &mut fb, &params, &vars, None, &local_types, abi.result,
                &global_values,&stack,&call_values,&labels,None)?;
            if body {
                if abi.result.is_none() {
                    fb.ins().return_(&[]);
                } else {
                    return Err(invalid("fallthrough in code generation"));
                }
            }
            // Source labels can receive forward and backward edges, including
            // branches from inside an if-arm. Seal only after emission is done.
            for block in labels.values() {fb.seal_block(*block);}
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
    let calls = compile_calls(il)?;
    emit_routines_with_globals(&routines,&globals,&calls)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn m10_indirect_callback_object_has_native_call_signature() {
        let word=CAbiInt{bits:64,signed:true,is_bool:false};
        let callback_abi=CAbi {
            parameters:vec![word],result:Some(word),
            aggregate_parameters:vec![None],aggregate_result:None,
        };
        let routine=Routine {
            name:"m10_test_forward".to_string(),parameters:2,locals:0,
            body:vec![Stmt::Return(Expr::CallIndirect(
                Box::new(Expr::Parameter(0)),callback_abi,
                vec![Expr::Parameter(1)]))],
            abi:None,local_types:None,local_storage:None,local_alignments:None,parameter_accesses:vec![],
        };
        assert!(!emit_routines(&[routine]).unwrap().is_empty());
    }
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
            name: "cfg_unit".into(), parameters: 2, locals: 1, body, abi: None, local_types: None, local_storage: None, local_alignments: None,parameter_accesses:vec![],
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
            Stmt::For { init: vec![], condition: Some(condition.clone()),
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
            name: "for_and_do".into(), parameters: 2, locals: 2, body, abi: None, local_types: None, local_storage: None, local_alignments: None,parameter_accesses:vec![],
        }]).unwrap()[..4], b"\x7fELF");
    }
    #[test]
    fn rejects_break_outside_loop_and_emits_continue_break() {
        assert!(check(&[Stmt::Break], &mut HashSet::new(), 0, 0, 0).is_err());
        let body = vec![
            Stmt::Store(0, Expr::Parameter(0)),
            Stmt::For {
                init: vec![],
                condition: Some(Expr::Compare(CompareOp::Less,
                    Box::new(Expr::Local(0)), Box::new(Expr::Parameter(1)))),
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
            name: "loop_jumps".into(), parameters: 2, locals: 1, body, abi: None, local_types: None, local_storage: None, local_alignments: None,parameter_accesses:vec![],
        }]).unwrap()[..4], b"\x7fELF");
    }

    #[test]
    fn emits_complete_zero_initialized_aggregate_slot() {
        let body=vec![Stmt::ZeroAggregate(0,14),Stmt::Return(Expr::Constant(3))];
        assert!(!check(&body,&mut HashSet::new(),0,1,0).unwrap());
        let obj=emit_routines(&[Routine {
            name:"zero_aggregate_unit".into(),parameters:0,locals:1,body,
            abi:None,local_types:None,local_storage:Some(vec![14]),
            local_alignments:Some(vec![1]),parameter_accesses:vec![],
        }]).unwrap();
        assert_eq!(&obj[..4],b"\x7fELF");
        assert!(check(&[Stmt::ZeroAggregate(0,4097)],
            &mut HashSet::new(),0,1,0).is_err());
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
    #[test]
    fn unconditional_for_requires_no_fabricated_exit_edge() {
        let returns = vec![Stmt::For {
            init:vec![], condition:None, increment:vec![],
            body:vec![Stmt::Return(Expr::Parameter(0))],
        }];
        assert!(!check(&returns,&mut HashSet::new(),1,0,0).unwrap());
        let object=emit_routines(&[Routine {
            name:"forever_return_unit".into(),parameters:1,locals:0,body:returns,
            abi:None,local_types:None,local_storage:None,local_alignments:None,parameter_accesses:vec![],
        }]).unwrap();
        assert_eq!(&object[..4],b"\x7fELF");

        let with_break=vec![
            Stmt::For {
                init:vec![],condition:None,increment:vec![],
                body:vec![Stmt::If(Expr::Parameter(0),
                    vec![Stmt::Break],vec![Stmt::Return(Expr::Constant(12))])],
            },
            Stmt::Return(Expr::Constant(77)),
        ];
        assert!(!check(&with_break,&mut HashSet::new(),1,0,0).unwrap());
        let object=emit_routines(&[Routine {
            name:"forever_break_unit".into(),parameters:1,locals:0,
            body:with_break,abi:None,local_types:None,
            local_storage:None,local_alignments:None,parameter_accesses:vec![],
        }]).unwrap();
        assert_eq!(&object[..4],b"\x7fELF");
    }

    #[test]
    fn breaks_inside_nested_loops_do_not_exit_the_outer_unconditional_for() {
        let outer=vec![Stmt::For {
            init:vec![],condition:None,increment:vec![],
            body:vec![Stmt::While(Expr::Parameter(0),vec![Stmt::Break])],
        }];
        if let Stmt::For{body,..}=&outer[0] {
            assert!(!may_break_current_loop(body));
        } else {panic!("expected for loop");}
        assert!(!check(&outer,&mut HashSet::new(),1,0,0).unwrap());
    }

    #[test]
    fn user_label_map_validates_forward_backward_and_scoped_gotos() {
        let label=crate::SymbolId(91);
        let stmts=vec![
            Stmt::If(Expr::Parameter(0),
                vec![Stmt::Goto(label)],vec![]),
            Stmt::Label(label),
            Stmt::Store(0,Expr::Constant(3)),
            Stmt::Goto(label),
        ];
        assert_eq!(source_labels(&stmts).unwrap(),vec![label]);
        let unresolved=vec![Stmt::Goto(crate::SymbolId(92))];
        assert!(source_labels(&unresolved).is_err());
        let duplicate=vec![Stmt::Label(label),Stmt::Label(label)];
        assert!(source_labels(&duplicate).is_err());
        let nested=vec![Stmt::While(Expr::Parameter(0),
            vec![Stmt::Goto(label)]),Stmt::Label(label)];
        assert!(source_labels(&nested).is_ok());
        let interior_label=vec![Stmt::While(Expr::Parameter(0),
            vec![Stmt::Label(label)])];
        assert!(source_labels(&interior_label).is_err());
    }

    #[test]
    fn user_goto_returns_to_a_labeled_block_without_lexical_fallthrough() {
        let label=crate::SymbolId(100);
        let body=vec![
            Stmt::Store(0,Expr::Constant(0)),
            Stmt::Goto(label),
            Stmt::Label(label),
            Stmt::Store(0,Expr::Add(Box::new(Expr::Local(0)),
                Box::new(Expr::Constant(7)))),
            Stmt::Return(Expr::Local(0)),
        ];
        assert!(source_labels(&body).is_ok());
        let object=emit_routines(&[Routine {
            name:"test_source_label_jump".into(),parameters:0,locals:1,
            body,abi:None,local_types:None,
            local_storage:None,local_alignments:None,parameter_accesses:vec![],
        }]).unwrap();
        assert_eq!(&object[..4],b"\x7fELF");
    }

}
