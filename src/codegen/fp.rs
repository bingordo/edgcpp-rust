//! M14: first checked SysV x86-64 scalar floating-point ABI slice.
//!
//! This independent reference path does not reinterpret floating values as
//! C integer machine pieces. F32/F64 use real Cranelift FP types, so SysV
//! assigns them to XMM registers (or overflow stack slots) in C ABI order.
//! The eventual full EDG adapter must share a typed scalar passing plan with
//! the integer CFG; do not weaken the older integer ABI checks in the meantime.
use super::*;
use cranelift_codegen::ir::{condcodes::FloatCC,immediates::{Ieee32,Ieee64}, Value};
use cranelift_frontend::Variable;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum FloatType { F32,F64 }
impl FloatType {
    fn machine(self)->cranelift_codegen::ir::Type {
        match self {Self::F32=>types::F32,Self::F64=>types::F64}
    }
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum FloatExpr {
    Parameter(usize),
    Local(usize),
    Constant32(u32),
    Constant64(u64),
    Neg(Box<Self>),
    Add(Box<Self>,Box<Self>),
    Sub(Box<Self>,Box<Self>),
    Mul(Box<Self>,Box<Self>),
    Div(Box<Self>,Box<Self>),
    /// The conversion is explicit and checked against its source type.
    Convert(FloatType,Box<Self>),
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct FloatFunction {
    pub name:String,
    pub parameters:Vec<FloatType>,
    pub result:FloatType,
    pub expression:FloatExpr,
}
/// C comparison truth tables: all except Ne are ordered. Ne also matches NaN.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum FloatPredicate { Eq,Ne,Lt,Le,Gt,Ge }
impl FloatPredicate {
    fn cranelift(self)->FloatCC {
        match self {
            Self::Eq=>FloatCC::Equal,
            Self::Ne=>FloatCC::NotEqual,
            Self::Lt=>FloatCC::LessThan,
            Self::Le=>FloatCC::LessThanOrEqual,
            Self::Gt=>FloatCC::GreaterThan,
            Self::Ge=>FloatCC::GreaterThanOrEqual,
        }
    }
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct FloatCondition {
    pub op:FloatPredicate,
    pub left:FloatExpr,
    pub right:FloatExpr,
}
/// Checked FP SSA statements. IfAssign branches both fall through to a
/// merge block; the Cranelift SSA builder materializes block parameters for
/// locals whose values differ by predecessor. No loop or nested branch yet.
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum FloatStmt {
    Assign(usize,FloatExpr),
    Return(FloatExpr),
    IfReturn(FloatCondition,FloatExpr,FloatExpr),
    /// Two nonempty straight-line assignment arms, joined before a later return.
    IfAssign(FloatCondition,Vec<(usize,FloatExpr)>,Vec<(usize,FloatExpr)>),
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct FloatBodyFunction {
    pub name:String,
    pub parameters:Vec<FloatType>,
    pub result:FloatType,
    pub locals:Vec<FloatType>,
    pub statements:Vec<FloatStmt>,
}
fn reject(message:impl Into<String>)->CodegenError {
    CodegenError::InvalidFunction(message.into())
}
fn expression_type(
    expr:&FloatExpr,params:&[FloatType],locals:&[FloatType],depth:usize,
)->Result<FloatType,CodegenError> {
    if depth>64 {return Err(reject("FP expression exceeds recursion limit"));}
    use FloatExpr::*;
    match expr {
        Parameter(n)=>params.get(*n).copied()
            .ok_or_else(||reject("FP parameter out of range")),
        Local(n)=>locals.get(*n).copied()
            .ok_or_else(||reject("FP local out of range")),
        Constant32(_)=>Ok(FloatType::F32),
        Constant64(_)=>Ok(FloatType::F64),
        Neg(e)=>expression_type(e,params,locals,depth+1),
        Add(a,b)|Sub(a,b)|Mul(a,b)|Div(a,b)=>{
            let a=expression_type(a,params,locals,depth+1)?;
            let b=expression_type(b,params,locals,depth+1)?;
            if a!=b {return Err(reject("FP binary operation operands differ in type"));}
            Ok(a)
        }
        Convert(to,e)=>{
            let _=expression_type(e,params,locals,depth+1)?;
            Ok(*to)
        }
    }
}
fn emit_expr(
    expr:&FloatExpr,
    fb:&mut FunctionBuilder<'_>,
    params:&[Value],
    params_ty:&[FloatType],
    locals:&[Variable],
    locals_ty:&[FloatType],
) -> Result<(Value,FloatType),CodegenError> {
    use FloatExpr::*;
    let (value,ty)=match expr {
        Parameter(n)=>{
            let value=*params.get(*n)
                .ok_or_else(||reject("missing physical FP parameter"))?;
            let ty=*params_ty.get(*n)
                .ok_or_else(||reject("missing source FP type"))?;
            (value,ty)
        }
        Local(n)=>{
            let var=*locals.get(*n).ok_or_else(||reject("missing FP local"))?;
            let ty=*locals_ty.get(*n).ok_or_else(||reject("missing FP local type"))?;
            (fb.use_var(var),ty)
        }
        Constant32(bits)=>(fb.ins().f32const(Ieee32::with_bits(*bits)),FloatType::F32),
        Constant64(bits)=>(fb.ins().f64const(Ieee64::with_bits(*bits)),FloatType::F64),
        Neg(e)=>{
            let (v,t)=emit_expr(e,fb,params,params_ty,locals,locals_ty)?;
            (fb.ins().fneg(v),t)
        }
        Convert(to,e)=>{
            let (v,t)=emit_expr(e,fb,params,params_ty,locals,locals_ty)?;
            let value=match (t,*to) {
                (FloatType::F32,FloatType::F64)=>fb.ins().fpromote(types::F64,v),
                (FloatType::F64,FloatType::F32)=>fb.ins().fdemote(types::F32,v),
                _=>v,
            };
            (value,*to)
        }
        Add(a,b)|Sub(a,b)|Mul(a,b)|Div(a,b)=>{
            let (x,t)=emit_expr(a,fb,params,params_ty,locals,locals_ty)?;
            let (y,u)=emit_expr(b,fb,params,params_ty,locals,locals_ty)?;
            if t!=u {return Err(reject("unconverted mixed FP arithmetic"));}
            let value=match expr {
                Add(..)=>fb.ins().fadd(x,y),
                Sub(..)=>fb.ins().fsub(x,y),
                Mul(..)=>fb.ins().fmul(x,y),
                Div(..)=>fb.ins().fdiv(x,y),
                _=>unreachable!(),
            };
            (value,t)
        }
    };
    Ok((value,ty))
}
/// Reject uninitialized local reads before passing values to Cranelift.
/// No source expression may observe an undefined local, even speculatively.
fn check_local_reads(expr:&FloatExpr,defined:&[bool],depth:usize)->Result<(),CodegenError> {
    if depth>64 {return Err(reject("FP expression exceeds recursion limit"));}
    use FloatExpr::*;
    match expr {
        Local(i) if !defined.get(*i).copied().unwrap_or(false)=>
            Err(reject("read of uninitialized or invalid FP local")),
        Local(_)|Parameter(_)|Constant32(_)|Constant64(_)=>Ok(()),
        Neg(e)|Convert(_,e)=>check_local_reads(e,defined,depth+1),
        Add(a,b)|Sub(a,b)|Mul(a,b)|Div(a,b)=>{
            check_local_reads(a,defined,depth+1)?;
            check_local_reads(b,defined,depth+1)
        }
    }
}
fn check_condition(
    cond:&FloatCondition,params:&[FloatType],locals:&[FloatType],
    defined:&[bool],
)->Result<(),CodegenError> {
    check_local_reads(&cond.left,defined,0)?;
    check_local_reads(&cond.right,defined,0)?;
    let left=expression_type(&cond.left,params,locals,0)?;
    let right=expression_type(&cond.right,params,locals,0)?;
    if left!=right {return Err(reject("unconverted mixed FP comparison"));}
    Ok(())
}
fn emit_condition(
    cond:&FloatCondition,fb:&mut FunctionBuilder<'_>,params:&[Value],
    params_ty:&[FloatType],locals:&[Variable],locals_ty:&[FloatType],
)->Result<Value,CodegenError> {
    let (x,t)=emit_expr(&cond.left,fb,params,params_ty,locals,locals_ty)?;
    let (y,u)=emit_expr(&cond.right,fb,params,params_ty,locals,locals_ty)?;
    if t!=u {return Err(reject("unconverted mixed FP comparison"));}
    Ok(fb.ins().fcmp(cond.op.cranelift(),x,y))
}
/// Validate one sequential assignment arm against a copy of the incoming
/// definitely-initialized set. Each RHS sees preceding assignments in its
/// own arm; the other arm's effects are never visible during verification.
fn validate_fp_arm(
    writes:&[(usize,FloatExpr)],f:&FloatBodyFunction,mut defined:Vec<bool>,
)->Result<Vec<bool>,CodegenError> {
    if writes.is_empty() || writes.len()>10000 {
        return Err(reject("FP conditional assignment arm is empty or oversized"));
    }
    for (n,expr) in writes {
        let ty=*f.locals.get(*n)
            .ok_or_else(||reject("FP branch local index out of range"))?;
        check_local_reads(expr,&defined,0)?;
        if expression_type(expr,&f.parameters,&f.locals,0)?!=ty {
            return Err(reject("FP branch assignment has an unconverted type"));
        }
        defined[*n]=true;
    }
    Ok(defined)
}
/// Keep the original pure-return reference API, sharing the same typed
/// statement emitter and ABI classification as local-bearing FP functions.
pub fn emit_float_routines(functions:&[FloatFunction])
    ->Result<Vec<u8>,CodegenError> {
    let bodies:Vec<_>=functions.iter().map(|f|FloatBodyFunction {
        name:f.name.clone(),parameters:f.parameters.clone(),result:f.result,
        locals:Vec::new(),statements:vec![FloatStmt::Return(f.expression.clone())],
    }).collect();
    emit_float_bodies(&bodies)
}
/// Emit typed SSE values and ordered straight-line FP local updates.
/// Local arithmetic is evaluated once and kept in explicit Cranelift SSA
/// variables, rather than substituting or repeating the source expressions.
pub fn emit_float_bodies(functions:&[FloatBodyFunction])
    ->Result<Vec<u8>,CodegenError> {
    if functions.is_empty(){return Err(reject("empty FP function set"));}
    let mut flags=settings::builder();
    flags.set("opt_level","speed")
        .map_err(|e|CodegenError::Backend(e.to_string()))?;
    let isa=cranelift_codegen::isa::lookup_by_name(target::ABI.isa_name())
        .map_err(|e|CodegenError::UnsupportedTarget(e.to_string()))?
        .finish(settings::Flags::new(flags))
        .map_err(|e|CodegenError::UnsupportedTarget(e.to_string()))?;
    let obj=ObjectBuilder::new(isa,"edgcpp_float",
        cranelift_module::default_libcall_names())
        .map_err(|e|CodegenError::Backend(e.to_string()))?;
    let mut module=ObjectModule::new(obj);
    let mut exports=Vec::new();
    for f in functions {
        if f.name.is_empty()||f.name.contains('\0') ||
            f.parameters.len()>256 || f.locals.len()>4096 {
            return Err(reject("invalid FP function name or variable count"));
        }
        if f.statements.is_empty() || f.statements.len()>10000 ||
            !matches!(f.statements.last(),
                Some(FloatStmt::Return(_)|FloatStmt::IfReturn(..))) {
            return Err(reject("FP body must end in one return or returning if"));
        }
        let mut defined=vec![false;f.locals.len()];
        for (i,stmt) in f.statements.iter().enumerate() {
            let last=i+1==f.statements.len();
            match stmt {
                FloatStmt::Assign(n,expr)=>{
                    if last {return Err(reject("FP function ends in assignment"));}
                    let ty=*f.locals.get(*n)
                        .ok_or_else(||reject("FP local index out of range"))?;
                    check_local_reads(expr,&defined,0)?;
                    if expression_type(expr,&f.parameters,&f.locals,0)?!=ty {
                        return Err(reject("FP assignment has an unconverted type"));
                    }
                    defined[*n]=true;
                }
                FloatStmt::Return(expr)=>{
                    if !last {return Err(reject("FP return before end of body"));}
                    check_local_reads(expr,&defined,0)?;
                    if expression_type(expr,&f.parameters,&f.locals,0)?!=f.result {
                        return Err(reject("FP return has an unconverted type"));
                    }
                }
                FloatStmt::IfReturn(cond,yes,no)=>{
                    if !last {return Err(reject("returning FP if must terminate the body"));}
                    check_condition(cond,&f.parameters,&f.locals,&defined)?;
                    for expr in [yes,no] {
                        check_local_reads(expr,&defined,0)?;
                        if expression_type(expr,&f.parameters,&f.locals,0)?!=f.result {
                            return Err(reject("FP conditional arm has an unconverted return type"));
                        }
                    }
                }
                FloatStmt::IfAssign(cond,yes,no)=>{
                    if last {return Err(reject("FP merging if must have a later return"));}
                    check_condition(cond,&f.parameters,&f.locals,&defined)?;
                    let yes=validate_fp_arm(yes,f,defined.clone())?;
                    let no=validate_fp_arm(no,f,defined.clone())?;
                    for i in 0..defined.len() {
                        defined[i]=yes[i] && no[i];
                    }
                }
            }
        }
        let mut sig=module.make_signature();
        for ty in &f.parameters {sig.params.push(AbiParam::new(ty.machine()));}
        sig.returns.push(AbiParam::new(f.result.machine()));
        let id=module.declare_function(&f.name,Linkage::Export,&sig)
            .map_err(|e|CodegenError::Backend(e.to_string()))?;
        exports.push(id);
    }
    for (f,id) in functions.iter().zip(exports) {
        let mut ctx=module.make_context();
        for ty in &f.parameters {
            ctx.func.signature.params.push(AbiParam::new(ty.machine()));
        }
        ctx.func.signature.returns.push(AbiParam::new(f.result.machine()));
        let mut fctx=FunctionBuilderContext::new();
        {
            let mut fb=FunctionBuilder::new(&mut ctx.func,&mut fctx);
            let entry=fb.create_block();
            fb.append_block_params_for_function_params(entry);
            fb.switch_to_block(entry);
            fb.seal_block(entry);
            let params=fb.block_params(entry).to_vec();
            let vars:Vec<_>=f.locals.iter().enumerate().map(|(i,ty)|{
                let var=Variable::from_u32(i as u32);
                fb.declare_var(var,ty.machine());
                var
            }).collect();
            for stmt in &f.statements {
                match stmt {
                    FloatStmt::Assign(n,expr)=>{
                        let (v,_) = emit_expr(expr,&mut fb,&params,&f.parameters,
                            &vars,&f.locals)?;
                        fb.def_var(vars[*n],v);
                    }
                    FloatStmt::Return(expr)=>{
                        let (v,_) = emit_expr(expr,&mut fb,&params,&f.parameters,
                            &vars,&f.locals)?;
                        fb.ins().return_(&[v]);
                    }
                    FloatStmt::IfReturn(cond,yes,no)=>{
                        let flag=emit_condition(cond,&mut fb,&params,&f.parameters,
                            &vars,&f.locals)?;
                        let yes_block=fb.create_block();
                        let no_block=fb.create_block();
                        fb.ins().brif(flag,yes_block,&[],no_block,&[]);
                        fb.seal_block(yes_block);
                        fb.seal_block(no_block);
                        fb.switch_to_block(yes_block);
                        let (value,_) = emit_expr(yes,&mut fb,&params,&f.parameters,
                            &vars,&f.locals)?;
                        fb.ins().return_(&[value]);
                        fb.switch_to_block(no_block);
                        let (value,_) = emit_expr(no,&mut fb,&params,&f.parameters,
                            &vars,&f.locals)?;
                        fb.ins().return_(&[value]);
                    }
                    FloatStmt::IfAssign(cond,yes,no)=>{
                        let flag=emit_condition(cond,&mut fb,&params,&f.parameters,
                            &vars,&f.locals)?;
                        let yes_block=fb.create_block();
                        let no_block=fb.create_block();
                        let join_block=fb.create_block();
                        fb.ins().brif(flag,yes_block,&[],no_block,&[]);
                        fb.seal_block(yes_block);
                        fb.seal_block(no_block);
                        fb.switch_to_block(yes_block);
                        for (n,expr) in yes {
                            let (v,_) = emit_expr(expr,&mut fb,&params,&f.parameters,
                                &vars,&f.locals)?;
                            fb.def_var(vars[*n],v);
                        }
                        fb.ins().jump(join_block,&[]);
                        fb.switch_to_block(no_block);
                        for (n,expr) in no {
                            let (v,_) = emit_expr(expr,&mut fb,&params,&f.parameters,
                                &vars,&f.locals)?;
                            fb.def_var(vars[*n],v);
                        }
                        fb.ins().jump(join_block,&[]);
                        // Seal only after both incoming CFG edges are known:
                        // Cranelift then inserts the proper SSA merge parameters.
                        fb.seal_block(join_block);
                        fb.switch_to_block(join_block);
                    }
                }
            }
            fb.finalize();
        }
        module.define_function(id,&mut ctx)
            .map_err(|e|CodegenError::Backend(e.to_string()))?;
        module.clear_context(&mut ctx);
    }
    module.finish().emit().map_err(|e|CodegenError::Backend(e.to_string()))
}

/// EDG's native standard-float metadata is authoritative. A same-sized
/// extended or long-double type must never enter this SSE ABI path.
fn type_from_edg(il:&Il,id:crate::TypeId)->Result<FloatType,CodegenError> {
    let canonical=super::canonical_abi_type(il,id)?;
    let info=il.c_float_types.iter().find(|t|t.id==canonical)
        .ok_or_else(||reject("EDG type is not standard C float/double"))?;
    let layout=il.type_object_sizes.iter().find(|t|t.owner==canonical)
        .ok_or_else(||reject("EDG FP type has no native layout"))?;
    match (info.bits,layout.bytes,layout.alignment) {
        (32,4,4)=>Ok(FloatType::F32),
        (64,8,8)=>Ok(FloatType::F64),
        _=>Err(reject("EDG FP width/alignment does not match x86-64 SysV")),
    }
}
/// Source expressions are checked using exact EDG symbol IDs and widths.
struct FpSourceCtx<'a> {
    il:&'a Il,
    graph:&'a crate::graph::Graph,
    routine:crate::SymbolId,
    params:&'a [crate::SymbolId],
    locals:&'a [crate::SymbolId],
    param_types:&'a [FloatType],
    local_types:&'a [FloatType],
}
fn expression_from_edg(
    ctx:&FpSourceCtx<'_>,id:crate::graph::ExpressionId,depth:usize,
)->Result<FloatExpr,CodegenError> {
    if depth>64 {return Err(reject("native EDG FP expression too deep"));}
    let node=ctx.graph.expression(id)
        .ok_or_else(||reject("missing EDG FP expression"))?;
    let ty=type_from_edg(ctx.il,node.data.type_id.ok_or_else(||
        reject("EDG FP expression has no type"))?)?;
    let kind=super::semantic(2,node.data.kind);
    if node.data.constant_kind.is_some() {
        if !node.operands.is_empty() || node.data.symbol_id.is_some() {
            return Err(reject("FP constant cannot carry operands or symbol identity"));
        }
        let mut literals=ctx.il.float_constants.iter().filter(|c|
            c.function==node.data.function &&
            c.statement_path==node.data.statement_path &&
            c.path==node.data.path);
        let exact=literals.next()
            .ok_or_else(||reject("EDG floating-point constant has no exact bit image"))?;
        if literals.next().is_some() {
            return Err(reject("ambiguous duplicate EDG floating-point constant"));
        }
        if Some(exact.ty)!=node.data.type_id {
            return Err(reject("EDG FP literal native type identity mismatch"));
        }
        return match (ty,exact.width) {
            (FloatType::F32,32)=>
                Ok(FloatExpr::Constant32(exact.bits as u32)),
            (FloatType::F64,64)=>
                Ok(FloatExpr::Constant64(exact.bits)),
            _=>Err(reject("EDG FP literal width/source type mismatch")),
        };
    }
    if kind==2 && node.operands.is_empty() {
        let symbol=node.data.symbol_id
            .ok_or_else(||reject("FP variable missing native identity"))?;
        if let Some(i)=ctx.params.iter().position(|p|*p==symbol) {
            if ctx.param_types[i]!=ty {return Err(reject("FP formal rvalue type mismatch"));}
            return Ok(FloatExpr::Parameter(i));
        }
        if let Some(i)=ctx.locals.iter().position(|p|*p==symbol) {
            if ctx.local_types[i]!=ty {return Err(reject("FP local rvalue type mismatch"));}
            return Ok(FloatExpr::Local(i));
        }
        return Err(reject("FP global/address-taken rvalue requires memory lowering"));
    }
    if kind!=3 {return Err(reject("unsupported FP source expression form"));}
    let op=node.data.operator.map(|v|super::semantic(3,v))
        .ok_or_else(||reject("missing native FP operation"))?;
    let arg=|i:usize|->Result<FloatExpr,CodegenError> {
        expression_from_edg(ctx,*node.operands.get(i)
            .ok_or_else(||reject("missing native FP operand"))?,depth+1)
    };
    let out=if node.operands.len()==1 {
        match op {
            4=>FloatExpr::Neg(Box::new(arg(0)?)),
            // All type changes must be introduced by actual EDG nodes.
            11|12|16=>FloatExpr::Convert(ty,Box::new(arg(0)?)),
            _=>return Err(reject(format!("unsupported unary EDG FP operator {op}"))),
        }
    } else if node.operands.len()==2 {
        let x=Box::new(arg(0)?);
        let y=Box::new(arg(1)?);
        match op {
            1=>FloatExpr::Add(x,y),
            2=>FloatExpr::Sub(x,y),
            3=>FloatExpr::Mul(x,y),
            19=>FloatExpr::Div(x,y),
            _=>return Err(reject(format!("unsupported binary EDG FP operator {op}"))),
        }
    } else {
        return Err(reject("unsupported native EDG FP operation arity"));
    };
    if expression_type(&out,ctx.param_types,ctx.local_types,0)?!=ty {
        return Err(reject("EDG FP expression result type differs from operands"));
    }
    Ok(out)
}
/// A C floating-point comparison has an integer/bool result, but its
/// operands must both have verified native FP widths. Never lower it by
/// first interpreting the result as a floating-point expression.
fn comparison_from_edg(
    ctx:&FpSourceCtx<'_>,id:crate::graph::ExpressionId,
)->Result<FloatCondition,CodegenError> {
    let node=ctx.graph.expression(id).ok_or_else(||
        reject("missing EDG floating comparison"))?;
    if super::semantic(2,node.data.kind)!=3 || node.operands.len()!=2 ||
        node.data.constant_kind.is_some() {
        return Err(reject("FP condition requires a binary comparison"));
    }
    let integer=super::canonical_abi_type(ctx.il,
        node.data.type_id.ok_or_else(||
            reject("FP condition has no native result type"))?)?;
    if !ctx.il.c_integer_types.iter().any(|ty|
        ty.id==integer && (ty.is_bool || ty.bits==32)) {
        return Err(reject("FP comparison must produce C int or bool"));
    }
    let op=match node.data.operator.map(|v|super::semantic(3,v)) {
        Some(5)=>FloatPredicate::Eq,
        Some(6)=>FloatPredicate::Ne,
        Some(7)=>FloatPredicate::Lt,
        Some(8)=>FloatPredicate::Le,
        Some(9)=>FloatPredicate::Gt,
        Some(10)=>FloatPredicate::Ge,
        _=>return Err(reject("unsupported EDG FP comparison predicate")),
    };
    let left=expression_from_edg(ctx,node.operands[0],0)?;
    let right=expression_from_edg(ctx,node.operands[1],0)?;
    let cond=FloatCondition {op,left,right};
    check_condition(&cond,ctx.param_types,ctx.local_types,
        &vec![true;ctx.local_types.len()])?;
    Ok(cond)
}
/// EDG typedef wrappers may conceal qualifiers. No ordinary SSA value may
/// represent an observable volatile or atomic local, or a C++ reference.
fn check_plain_fp_local(il:&Il,mut ty:crate::TypeId)->Result<(),CodegenError> {
    for _ in 0..=il.types.len() {
        if let Some(detail)=il.type_details.iter().find(|d|d.owner==ty) {
            if detail.is_reference || detail.is_rvalue_reference ||
                detail.qualifiers.unwrap_or(0)!=0 {
                return Err(reject("qualified/reference FP local needs memory effects"));
            }
        }
        let t=il.types.iter().find(|t|t.id==ty)
            .ok_or_else(||reject("missing native FP local type"))?;
        if super::semantic(4,t.kind)!=3 {return Ok(());}
        ty=t.related.ok_or_else(||reject("unresolved FP local type alias"))?;
    }
    Err(reject("cyclic FP local type aliases"))
}
/// Accept a narrow ordered C body with typed FP locals, returning if/else,
/// and straight-line conditional FP assignments with later SSA joins.
/// Nested conditions, loops, cleanup and pointer access are rejected.
fn lower_fp_body(
    ctx:&FpSourceCtx<'_>,id:crate::graph::StatementId,
    output:&mut Vec<FloatStmt>,depth:usize,
)->Result<(),CodegenError> {
    if depth>64 {return Err(reject("FP source statement nesting too deep"));}
    let node=ctx.graph.statement(id).ok_or_else(||reject("missing FP statement"))?;
    let path=&node.data.path;
    let function=&node.data.function;
    let simple_children=node.children.is_empty();
    match super::semantic(1,node.data.kind) {
        3 if node.expression.is_some() && node.children.len()==2=>{
            let cond=comparison_from_edg(ctx,node.expression.unwrap())?;
            let mut then_stmt=None;
            let mut else_stmt=None;
            let then_prefix=format!("{path}.then.");
            let else_prefix=format!("{path}.else.");
            for child in &node.children {
                let arm=ctx.graph.statement(*child)
                    .ok_or_else(||reject("missing FP if arm"))?;
                if arm.data.path.starts_with(&then_prefix) {
                    if then_stmt.replace(*child).is_some() {
                        return Err(reject("duplicate FP then branch"));
                    }
                } else if arm.data.path.starts_with(&else_prefix) {
                    if else_stmt.replace(*child).is_some() {
                        return Err(reject("duplicate FP else branch"));
                    }
                } else {
                    return Err(reject("unexpected FP if branch edge"));
                }
            }
            let extract=|arm:Option<crate::graph::StatementId>|->
                Result<Vec<FloatStmt>,CodegenError> {
                let arm=arm.ok_or_else(||reject("FP if requires both branches"))?;
                let mut statements=Vec::new();
                lower_fp_body(ctx,arm,&mut statements,depth+1)?;
                if statements.is_empty() || statements.len()>10000 {
                    return Err(reject("FP if arm must be nonempty and bounded"));
                }
                Ok(statements)
            };
            let yes=extract(then_stmt)?;
            let no=extract(else_stmt)?;
            if yes.len()==1 && no.len()==1 &&
                matches!(yes[0],FloatStmt::Return(_)) &&
                matches!(no[0],FloatStmt::Return(_)) {
                let (FloatStmt::Return(yes),FloatStmt::Return(no))=
                    (yes.into_iter().next().unwrap(),no.into_iter().next().unwrap())
                    else {unreachable!()};
                output.push(FloatStmt::IfReturn(cond,yes,no));
            } else if yes.iter().all(|s|matches!(s,FloatStmt::Assign(..))) &&
                      no.iter().all(|s|matches!(s,FloatStmt::Assign(..))) {
                // Only straight-line assignment branches can join; there is
                // no general nested CFG, branch termination or memory effect.
                let assignments=|stmts:Vec<FloatStmt>|->Vec<(usize,FloatExpr)> {
                    stmts.into_iter().map(|s|match s {
                        FloatStmt::Assign(n,e)=>(n,e),
                        _=>unreachable!(),
                    }).collect()
                };
                output.push(FloatStmt::IfAssign(
                    cond,assignments(yes),assignments(no)));
            } else {
                return Err(reject("FP if arms must both return or both assign locals"));
            }
        }
        2 if node.expression.is_none()=>{
            for child in &node.children {
                lower_fp_body(ctx,*child,output,depth+1)?;
            }
        }
        4 if node.expression.is_none() && simple_children=>{
            let mut found=false;
            for decl in ctx.graph.declarations_at(function,path) {
                found=true;
                if decl.is_vla || !ctx.locals.contains(&decl.symbol) {
                    return Err(reject("FP declaration is not a checked automatic scalar"));
                }
                if type_from_edg(ctx.il,decl.ty).is_err() {
                    return Err(reject("FP declaration has unsupported source type"));
                }
            }
            if !found {return Err(reject("FP declaration missing native symbol"));}
        }
        5 if simple_children=>{
            let init=ctx.graph.statement_initialization(function,path)
                .ok_or_else(||reject("FP initializer missing native statement metadata"))?;
            if init.role!=0 || init.braced {
                return Err(reject("unsupported FP initializer kind"));
            }
            let symbol=init.symbol.ok_or_else(||reject("FP initializer has no local"))?;
            let local=ctx.locals.iter().position(|p|*p==symbol)
                .ok_or_else(||reject("FP initializer target is not an automatic local"))?;
            let value=match init.kind {
                // EDG dik_expression: an actual expression executes at this site.
                3=>{
                    let source=node.expression
                        .ok_or_else(||reject("FP local initializer has no expression"))?;
                    expression_from_edg(ctx,source,0)?
                }
                // EDG dik_constant: no expression exists, so use exactly the
                // original constant's IEEE image, not a decimal conversion.
                2=>{
                    if node.expression.is_some() {
                        return Err(reject("FP constant initializer has unexpected expression"));
                    }
                    let mut matches=ctx.il.float_initializers.iter().filter(|v|
                        v.function==*function && v.statement_path==*path &&
                        v.symbol==symbol);
                    let exact=matches.next().ok_or_else(||
                        reject("FP constant initializer has no exact native image"))?;
                    if matches.next().is_some() {
                        return Err(reject("ambiguous FP constant initializer"));
                    }
                    let source_type=ctx.il.local_variables.iter().find(|v|
                        v.routine==ctx.routine && v.symbol==symbol)
                        .map(|v|v.ty).ok_or_else(||
                            reject("missing FP initializer target type"))?;
                    if exact.ty!=source_type {
                        return Err(reject("FP initializer native target type mismatch"));
                    }
                    match (ctx.local_types[local],exact.width) {
                        (FloatType::F32,32)=>FloatExpr::Constant32(exact.bits as u32),
                        (FloatType::F64,64)=>FloatExpr::Constant64(exact.bits),
                        _=>return Err(reject("FP initializer width/type mismatch")),
                    }
                }
                _=>return Err(reject("unsupported FP initializer kind")),
            };
            output.push(FloatStmt::Assign(local,value));
        }
        6 if simple_children=>{
            let expr=node.expression
                .ok_or_else(||reject("FP expression statement has no expression"))?;
            let operation=ctx.graph.expression(expr)
                .ok_or_else(||reject("missing FP assignment expression"))?;
            if super::semantic(2,operation.data.kind)!=3 ||
                operation.data.operator.map(|v|super::semantic(3,v))!=Some(18) ||
                operation.operands.len()!=2 {
                return Err(reject("FP side-effect expression needs simple local assignment"));
            }
            let left=ctx.graph.expression(operation.operands[0])
                .ok_or_else(||reject("FP assignment has no lvalue"))?;
            if super::semantic(2,left.data.kind)!=2 || !left.operands.is_empty() {
                return Err(reject("FP assignment target is not a direct local"));
            }
            let local=ctx.locals.iter().position(|p|Some(*p)==left.data.symbol_id)
                .ok_or_else(||reject("FP assignment target is not a checked local"))?;
            if type_from_edg(ctx.il,left.data.type_id.ok_or_else(||
                reject("missing native FP lvalue type"))?)?!=ctx.local_types[local] {
                return Err(reject("FP lvalue type mismatch"));
            }
            let value=expression_from_edg(ctx,operation.operands[1],0)?;
            output.push(FloatStmt::Assign(local,value));
        }
        1 if simple_children=>{
            let source=node.expression.ok_or_else(||reject("FP return has no value"))?;
            output.push(FloatStmt::Return(expression_from_edg(ctx,source,0)?));
        }
        _=>return Err(reject("unsupported FP control flow or statement form")),
    }
    if output.len()>10000 {return Err(reject("FP statement count exceeds limit"));}
    Ok(())
}
/// Only a module consisting entirely of simple floating-point routines is
/// handled here. Ordinary integer C follows the established CFG; mixed FP and
/// integer statements/functions fail closed rather than truncating an ELF.
pub(super) fn maybe_emit_edg_float_object(il:&Il)
    ->Result<Option<Vec<u8>>,CodegenError> {
    let definitions:Vec<_>=il.entities.iter().filter_map(|e|match e {
        crate::Entity::Routine{name,has_definition:true}=>Some(name.as_str()),
        _=>None,
    }).collect();
    if definitions.is_empty() {return Ok(None);}
    let graph=crate::graph::Graph::from_il(il)
        .map_err(|e|reject(format!("invalid EDG graph: {e:?}")))?;
    let mut found_float=false;
    for name in &definitions {
        let routine=graph.routine_for_function(name)
            .ok_or_else(||reject("missing EDG routine identity"))?;
        let sym_type=il.symbol_types.iter().find(|s|s.symbol==routine)
            .ok_or_else(||reject("missing native routine type"))?;
        let func_ty=super::canonical_abi_type(il,sym_type.ty)?;
        let func=il.types.iter().find(|t|t.id==func_ty)
            .ok_or_else(||reject("missing native function type"))?;
        if let Some(ret)=func.related {
            if type_from_edg(il,ret).is_ok() {found_float=true;}
        }
        if il.function_parameters.iter().any(|p|
            p.routine==routine && type_from_edg(il,p.ty).is_ok()) {
            found_float=true;
        }
    }
    if !found_float {return Ok(None);}
    // This reference emitter only defines functions. A module that owns
    // global storage or executable initialization must not silently drop
    // its data section when switched from the general integer CFG.
    if il.global_variables.iter().any(|g|
        g.linkage!=1 || g.explicit_initializer || g.thread_local ||
        g.local_to_function) {
        return Err(reject("FP source module defines global/TLS storage without FP data lowering"));
    }
    let mut functions=Vec::with_capacity(definitions.len());
    for name in definitions {
        let routine=graph.routine_for_function(name)
            .ok_or_else(||reject("missing FP routine"))?;
        let sym_type=il.symbol_types.iter().find(|s|s.symbol==routine)
            .ok_or_else(||reject("missing FP function declaration"))?;
        let func_ty=super::canonical_abi_type(il,sym_type.ty)?;
        let func=il.types.iter().find(|t|t.id==func_ty &&
            super::semantic(4,t.kind)==2)
            .ok_or_else(||reject("FP function type not a routine"))?;
        let sig=il.function_signatures.iter().find(|s|s.owner==func_ty)
            .ok_or_else(||reject("missing FP function prototype"))?;
        if sig.variadic || !sig.prototyped {
            return Err(reject("FP variadic/unprototyped ABI unsupported"));
        }
        let ret=type_from_edg(il,func.related.ok_or_else(||
            reject("missing FP return type"))?)?;
        let mut args:Vec<_>=il.function_parameters.iter()
            .filter(|p|p.routine==routine).collect();
        args.sort_by_key(|p|p.position);
        if args.iter().enumerate().any(|(i,p)|
            p.position!=Some(i as u32) || p.implicit_this || p.compiler_generated) {
            return Err(reject("invalid FP explicit parameter list"));
        }
        let params=args.iter().map(|p|type_from_edg(il,p.ty))
            .collect::<Result<Vec<_>,_>>()?;
        let declared:Vec<_>=il.type_parameters.iter()
            .filter(|p|p.owner==func_ty).collect();
        if declared.len()!=args.len() {
            return Err(reject("FP prototype/definition arity mismatch"));
        }
        for arg in &args {
            let d=declared.iter().find(|d|Some(d.position)==arg.position)
                .ok_or_else(||reject("FP prototype position missing"))?;
            if type_from_edg(il,d.ty)?!=type_from_edg(il,arg.ty)? {
                return Err(reject("FP prototype type mismatch"));
            }
        }
        let locals:Vec<_>=il.local_variables.iter()
            .filter(|v|v.routine==routine).collect();
        if locals.len()>4096 {return Err(reject("too many FP locals"));}
        let mut local_syms=Vec::with_capacity(locals.len());
        let mut local_types=Vec::with_capacity(locals.len());
        for local in locals {
            if local.static_storage || local.compiler_generated ||
                local.structured_binding || local.handler_parameter ||
                local_syms.contains(&local.symbol) {
                return Err(reject("unsupported FP local storage class"));
            }
            check_plain_fp_local(il,local.ty)?;
            local_types.push(type_from_edg(il,local.ty)?);
            local_syms.push(local.symbol);
        }
        let roots:Vec<_>=graph.roots(name).map(|(id,_)|id).collect();
        if roots.len()!=1 {return Err(reject("FP function must have one root"));}
        let params_syms=args.iter().map(|p|p.symbol).collect::<Vec<_>>();
        let ctx=FpSourceCtx {
            il,graph:&graph,routine,params:&params_syms,locals:&local_syms,
            param_types:&params,local_types:&local_types,
        };
        let mut statements=Vec::new();
        lower_fp_body(&ctx,roots[0],&mut statements,0)?;
        functions.push(FloatBodyFunction {
            name:name.to_string(),parameters:params,result:ret,
            locals:local_types,statements,
        });
    }
    emit_float_bodies(&functions).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn param(n:usize)->FloatExpr {FloatExpr::Parameter(n)}
    fn bin(a:usize,b:usize)->FloatExpr {
        FloatExpr::Add(Box::new(param(a)),Box::new(param(b)))
    }
    #[test]
    fn fp_if_assignment_merge_is_typed_and_definitely_initialized() {
        let cond=FloatCondition {
            op:FloatPredicate::Lt,
            left:FloatExpr::Parameter(0),
            right:FloatExpr::Parameter(1),
        };
        let valid=FloatBodyFunction {
            name:"m14_merge_unit".into(),
            parameters:vec![FloatType::F64,FloatType::F64],
            result:FloatType::F64,
            locals:vec![FloatType::F64,FloatType::F64],
            statements:vec![
                FloatStmt::Assign(0,FloatExpr::Parameter(0)),
                FloatStmt::IfAssign(cond.clone(),
                    vec![
                        (1,FloatExpr::Add(
                            Box::new(FloatExpr::Local(0)),
                            Box::new(FloatExpr::Parameter(1)))),
                        (0,FloatExpr::Local(1)),
                    ],
                    vec![
                        (1,FloatExpr::Sub(
                            Box::new(FloatExpr::Local(0)),
                            Box::new(FloatExpr::Parameter(1)))),
                        (0,FloatExpr::Local(1)),
                    ],
                ),
                FloatStmt::Return(FloatExpr::Local(0)),
            ],
        };
        let elf=emit_float_bodies(&[valid.clone()]).unwrap();
        assert_eq!(&elf[..4],b"\x7fELF");
        // An initialized local modified in just one branch remains defined
        // because its prior value is live along the other predecessor.
        let mut one_side=valid.clone();
        one_side.statements=vec![
            FloatStmt::Assign(0,FloatExpr::Parameter(0)),
            FloatStmt::IfAssign(cond.clone(),
                vec![(0,FloatExpr::Parameter(1))],
                vec![(1,FloatExpr::Parameter(0))]),
            FloatStmt::Return(FloatExpr::Local(0)),
        ];
        assert!(emit_float_bodies(&[one_side]).is_ok());
        // But a value defined on only one predecessor is not initialized
        // after the merge; another local's assignment cannot fix it.
        let mut uninit=valid.clone();
        uninit.statements=vec![
            FloatStmt::IfAssign(cond.clone(),
                vec![(0,FloatExpr::Parameter(0))],
                vec![(1,FloatExpr::Parameter(1))]),
            FloatStmt::Return(FloatExpr::Local(0)),
        ];
        assert!(emit_float_bodies(&[uninit]).is_err());
        let mut uninit_in_arm=valid.clone();
        uninit_in_arm.statements=vec![
            FloatStmt::IfAssign(cond.clone(),
                vec![(0,FloatExpr::Local(0))],
                vec![(0,FloatExpr::Parameter(1))]),
            FloatStmt::Return(FloatExpr::Local(0)),
        ];
        assert!(emit_float_bodies(&[uninit_in_arm]).is_err());
        let mut wrong_type=valid.clone();
        wrong_type.statements=vec![
            FloatStmt::IfAssign(cond.clone(),
                vec![(0,FloatExpr::Constant32(0))],
                vec![(0,FloatExpr::Parameter(0))]),
            FloatStmt::Return(FloatExpr::Local(0)),
        ];
        assert!(emit_float_bodies(&[wrong_type]).is_err());
        let mut empty_arm=valid;
        empty_arm.statements=vec![
            FloatStmt::IfAssign(cond,
                vec![(0,FloatExpr::Parameter(0))],vec![]),
            FloatStmt::Return(FloatExpr::Local(0)),
        ];
        assert!(emit_float_bodies(&[empty_arm]).is_err());
    }

    #[test]
    fn fp_predicates_check_nan_correctly_and_fail_closed() {
        let condition=FloatCondition {
            op:FloatPredicate::Ne,
            left:FloatExpr::Parameter(0),
            right:FloatExpr::Parameter(1),
        };
        let valid=FloatBodyFunction {
            name:"m14_unit_compare".into(),
            parameters:vec![FloatType::F64,FloatType::F64],
            result:FloatType::F64,locals:vec![],
            statements:vec![FloatStmt::IfReturn(condition.clone(),
                FloatExpr::Constant64(11.0f64.to_bits()),
                FloatExpr::Constant64(22.0f64.to_bits()))],
        };
        let object=emit_float_bodies(&[valid.clone()]).unwrap();
        assert_eq!(&object[..4],b"\x7fELF");
        let mut bad=valid.clone();
        bad.parameters[1]=FloatType::F32;
        assert!(emit_float_bodies(&[bad]).is_err());
        let mut bad=valid.clone();
        bad.statements=vec![FloatStmt::IfReturn(condition,
            FloatExpr::Parameter(0),
            FloatExpr::Constant32(0))];
        assert!(emit_float_bodies(&[bad]).is_err());
        let mut bad=valid;
        bad.statements.push(FloatStmt::Return(FloatExpr::Parameter(0)));
        assert!(emit_float_bodies(&[bad]).is_err());
    }

    #[test]
    fn fp_local_definite_assignment_and_type_checks() {
        let valid=FloatBodyFunction {
            name:"m14_let".into(),
            parameters:vec![FloatType::F32,FloatType::F32],
            result:FloatType::F32,
            locals:vec![FloatType::F32],
            statements:vec![
                FloatStmt::Assign(0,FloatExpr::Parameter(0)),
                FloatStmt::Assign(0,FloatExpr::Add(
                    Box::new(FloatExpr::Local(0)),
                    Box::new(FloatExpr::Parameter(1)))),
                FloatStmt::Return(FloatExpr::Local(0)),
            ],
        };
        let object=emit_float_bodies(&[valid.clone()]).unwrap();
        assert_eq!(&object[..4],b"\x7fELF");
        let mut bad=valid.clone();
        bad.statements=vec![FloatStmt::Return(FloatExpr::Local(0))];
        assert!(emit_float_bodies(&[bad]).is_err());
        let mut bad=valid.clone();
        bad.statements=vec![
            FloatStmt::Assign(0,FloatExpr::Local(0)),
            FloatStmt::Return(FloatExpr::Local(0)),
        ];
        assert!(emit_float_bodies(&[bad]).is_err());
        let mut bad=valid.clone();
        bad.statements=vec![
            FloatStmt::Assign(0,FloatExpr::Constant64(1.5f64.to_bits())),
            FloatStmt::Return(FloatExpr::Local(0)),
        ];
        assert!(emit_float_bodies(&[bad]).is_err());
        let mut bad=valid;
        bad.statements=vec![
            FloatStmt::Return(FloatExpr::Parameter(0)),
            FloatStmt::Return(FloatExpr::Parameter(1)),
        ];
        assert!(emit_float_bodies(&[bad]).is_err());
    }

    #[test]
    fn rejects_mixed_fp_without_explicit_conversion() {
        let f=FloatFunction{name:"bad".into(),parameters:vec![FloatType::F32,FloatType::F64],
            result:FloatType::F64,expression:bin(0,1)};
        assert!(emit_float_routines(&[f]).is_err());
    }
    #[test]
    fn rejects_wrong_return_type_and_invalid_index() {
        let f=FloatFunction{name:"bad".into(),parameters:vec![FloatType::F32],
            result:FloatType::F64,expression:param(0)};
        assert!(emit_float_routines(&[f]).is_err());
        let f=FloatFunction{name:"bad".into(),parameters:vec![FloatType::F32],
            result:FloatType::F32,expression:param(1)};
        assert!(emit_float_routines(&[f]).is_err());
    }
    #[test]
    fn sysv_sse_registers_and_stack_match_gcc() {
        let functions=vec![
            FloatFunction{name:"m14_add_f32".into(),
                parameters:vec![FloatType::F32;2],
                result:FloatType::F32,expression:bin(0,1)},
            FloatFunction{name:"m14_add_f64".into(),
                parameters:vec![FloatType::F64;2],
                result:FloatType::F64,expression:bin(0,1)},
            FloatFunction{name:"m14_mix_promote".into(),
                parameters:vec![FloatType::F32,FloatType::F64],
                result:FloatType::F64,
                expression:FloatExpr::Add(
                    Box::new(FloatExpr::Convert(FloatType::F64,Box::new(param(0)))),
                    Box::new(param(1)))},
            FloatFunction{name:"m14_mix_demote".into(),
                parameters:vec![FloatType::F64,FloatType::F32],
                result:FloatType::F32,
                expression:FloatExpr::Sub(
                    Box::new(FloatExpr::Convert(FloatType::F32,Box::new(param(0)))),
                    Box::new(param(1)))},
            FloatFunction{name:"m14_stack_f64".into(),
                parameters:vec![FloatType::F64;10],
                result:FloatType::F64,expression:bin(8,9)},
            FloatFunction{name:"m14_constant_f64".into(),
                parameters:vec![],result:FloatType::F64,
                expression:FloatExpr::Constant64(1.25f64.to_bits())},
            FloatFunction{name:"m14_neg_f32".into(),
                parameters:vec![FloatType::F32],result:FloatType::F32,
                expression:FloatExpr::Neg(Box::new(param(0)))},
        ];
        let object=emit_float_routines(&functions).unwrap();
        assert_eq!(&object[..4],b"\x7fELF");
        use std::process::Command;
        let root=std::env::temp_dir().join(format!(
            "edg_m14_float_{}_{}",std::process::id(),
            std::thread::current().name().unwrap_or("test").replace(' ',"_")));
        std::fs::create_dir_all(&root).unwrap();
        let obj=root.join("float.o");
        let src=root.join("driver.c");
        let exe=root.join("driver");
        std::fs::write(&obj,object).unwrap();
        std::fs::write(&src,r#"
#include <stdio.h>
#include <stdlib.h>
float m14_add_f32(float,float);
double m14_add_f64(double,double);
double m14_mix_promote(float,double);
float m14_mix_demote(double,float);
double m14_stack_f64(double,double,double,double,double,double,double,double,double,double);
double m14_constant_f64(void);
float m14_neg_f32(float);
static void check(int ok,const char* name) {
  if(!ok){fprintf(stderr,"failed: %s\n",name);exit(1);}
}
int main(void) {
  check(m14_add_f32(2.5f,3.25f)==5.75f,"f32 add");
  check(m14_add_f64(-2.0,4.25)==2.25,"f64 add");
  check(m14_mix_promote(2.5f,3.25)==5.75,"f32 -> f64");
  check(m14_mix_demote(8.5,1.25f)==7.25f,"f64 -> f32");
  check(m14_stack_f64(1,2,3,4,5,6,7,8,9.25,10.5)==19.75,"SSE overflow stack");
  check(m14_constant_f64()==1.25,"f64 constant");
  check(m14_neg_f32(2.25f)==-2.25f,"f32 neg");
  return 0;
}
"#).unwrap();
        let cc=Command::new("gcc").args(["-std=c11","-O2","-Wall","-Wextra","-Werror"])
            .arg(&obj).arg(&src).arg("-o").arg(&exe).output().unwrap();
        assert!(cc.status.success(),"GCC: {}",String::from_utf8_lossy(&cc.stderr));
        let run=Command::new(&exe).output().unwrap();
        assert!(run.status.success(),"FP ABI execution: {}",String::from_utf8_lossy(&run.stderr));
        let _=std::fs::remove_dir_all(root);
    }
}
