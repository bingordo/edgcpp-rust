//! M14: first checked SysV x86-64 scalar floating-point ABI slice.
//!
//! This independent reference path does not reinterpret floating values as
//! C integer machine pieces. F32/F64 use real Cranelift FP types, so SysV
//! assigns them to XMM registers (or overflow stack slots) in C ABI order.
//! The eventual full EDG adapter must share a typed scalar passing plan with
//! the integer CFG; do not weaken the older integer ABI checks in the meantime.
use super::*;
use cranelift_codegen::ir::{condcodes::FloatCC,immediates::{Ieee32,Ieee64},Block,FuncRef,Value};
use std::collections::HashMap;
use std::cell::Cell;
use cranelift_frontend::Variable;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum FloatType { F32,F64 }
impl FloatType {
    fn machine(self)->cranelift_codegen::ir::Type {
        match self {Self::F32=>types::F32,Self::F64=>types::F64}
    }
}
/// Native scalar argument ABI classes; GPR and SSE registers are allocated
/// independently by Cranelift's x86-64 SysV call-convention lowering.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum ScalarType {
    Float(FloatType),
    Int { bits:u8,signed:bool },
}
impl ScalarType {
    fn machine(self)->cranelift_codegen::ir::Type {
        match self {
            Self::Float(t)=>t.machine(),
            Self::Int {bits:32,..}=>types::I32,
            Self::Int {bits:64,..}=>types::I64,
            _=>unreachable!("integer width was checked at source boundary"),
        }
    }
}
#[derive(Debug,Clone,PartialEq,Eq)]
struct ScalarFloatBody {
    name:String,
    parameters:Vec<ScalarType>,
    result:ScalarType,
    locals:Vec<FloatType>,
    statements:Vec<FloatStmt>,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum ScalarCallArg {
    Float(FloatExpr),
    /// A native EDG direct INTEGER formal passed to the same scalar class.
    IntegerParameter(usize),
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum FloatExpr {
    Parameter(usize),
    /// Exact EDG integer formal converted to FP by a native cast node.
    /// Never reinterpret GPR argument bits as an XMM value.
    IntegerParameterCast {parameter:usize,to:FloatType},
    /// An EDG-verified direct call to a definition in this compilation unit.
    /// Indirect and external calls require a separate declaration contract.
    Call {
        callee:String,
        parameters:Vec<ScalarType>,
        return_type:FloatType,
        arguments:Vec<ScalarCallArg>,
    },
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
/// Checked FP SSA statements. The same path-sensitive flow checker validates
/// straight-line assignments, branches, and loop-carried scalar SSA values.
/// Source expressions are restricted to checked, side-effect-free FP arithmetic.
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum FloatStmt {
    Assign(usize,FloatExpr),
    Return(FloatExpr),
    /// EDG-verified conversion of an FP expression to a native C INTEGER
    /// return class. Out-of-range/NaN are undefined in C and not normalized.
    ReturnInteger(FloatExpr,ScalarType),
    IfReturn(FloatCondition,FloatExpr,FloatExpr),
    /// Straight-line conditional updates joined before a later return.
    /// Either arm may be empty; at least one must update a local.
    IfAssign(FloatCondition,Vec<(usize,FloatExpr)>,Vec<(usize,FloatExpr)>),
    /// General structured branch, including loop transfers and early returns.
    IfBlock(FloatCondition,Vec<FloatStmt>,Vec<FloatStmt>),
    While(FloatCondition,Vec<FloatStmt>),
    For {
        init:Vec<FloatStmt>,
        condition:Option<FloatCondition>,
        increment:Vec<FloatStmt>,
        body:Vec<FloatStmt>,
    },
    DoWhile(Vec<FloatStmt>,FloatCondition),
    Break,
    Continue,
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
    expr:&FloatExpr,params:&[ScalarType],locals:&[FloatType],depth:usize,
)->Result<FloatType,CodegenError> {
    if depth>64 {return Err(reject("FP expression exceeds recursion limit"));}
    use FloatExpr::*;
    match expr {
        Parameter(n)=>match params.get(*n).copied() {
            Some(ScalarType::Float(t))=>Ok(t),
            _=>Err(reject("FP parameter missing or has integer ABI class")),
        },
        IntegerParameterCast {parameter,to}=>{
            if matches!(params.get(*parameter),Some(ScalarType::Int {..})) {
                Ok(*to)
            } else {
                Err(reject("FP integer cast source is not a checked integer formal"))
            }
        },
        Call {parameters,return_type,arguments,..}=>{
            if arguments.len()!=parameters.len() || arguments.len()>256 {
                return Err(reject("FP direct call argument count does not match prototype"));
            }
            for (arg,target) in arguments.iter().zip(parameters) {
                let source=match arg {
                    ScalarCallArg::Float(e)=>
                        ScalarType::Float(expression_type(e,params,locals,depth+1)?),
                    ScalarCallArg::IntegerParameter(n)=>match params.get(*n) {
                        Some(ScalarType::Int {..})=>params[*n],
                        _=>return Err(reject("FP call's integer source is not a formal")),
                    },
                };
                if source!=*target {
                    return Err(reject("FP direct call argument class/type mismatch"));
                }
            }
            Ok(*return_type)
        }
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
    params_ty:&[ScalarType],
    locals:&[Variable],
    locals_ty:&[FloatType],
    refs:&HashMap<String,FuncRef>,
) -> Result<(Value,FloatType),CodegenError> {
    use FloatExpr::*;
    let (value,ty)=match expr {
        Parameter(n)=>{
            let value=*params.get(*n)
                .ok_or_else(||reject("missing physical FP parameter"))?;
            let ty=match params_ty.get(*n) {
                Some(ScalarType::Float(t))=>*t,
                _=>return Err(reject("FP value uses non-floating parameter")),
            };
            (value,ty)
        }
        IntegerParameterCast {parameter,to}=>{
            let input=match params_ty.get(*parameter) {
                Some(ScalarType::Int {bits:32|64,signed})=>*signed,
                _=>return Err(reject("invalid integer cast formal ABI")),
            };
            let value=*params.get(*parameter)
                .ok_or_else(||reject("missing integer formal ABI value"))?;
            let converted=if input {
                fb.ins().fcvt_from_sint(to.machine(),value)
            } else {
                fb.ins().fcvt_from_uint(to.machine(),value)
            };
            (converted,*to)
        }
        Call {callee,parameters,return_type,arguments}=>{
            if arguments.len()!=parameters.len() {
                return Err(reject("invalid direct call argument count"));
            }
            let reference=*refs.get(callee)
                .ok_or_else(||reject("FP call target has no defined function ref"))?;
            let mut values=Vec::with_capacity(arguments.len());
            for (arg,expected) in arguments.iter().zip(parameters) {
                let value=match (arg,expected) {
                    (ScalarCallArg::Float(e),ScalarType::Float(t))=>{
                        let (v,actual)=emit_expr(e,fb,params,params_ty,
                            locals,locals_ty,refs)?;
                        if actual!=*t {
                            return Err(reject("FP direct call argument is not converted"));
                        }
                        v
                    }
                    (ScalarCallArg::IntegerParameter(n),ScalarType::Int {..})=>{
                        if params_ty.get(*n)!=Some(expected) {
                            return Err(reject("direct call INTEGER formal ABI mismatch"));
                        }
                        *params.get(*n).ok_or_else(||reject("missing direct call formal"))?
                    }
                    _=>return Err(reject("direct call crosses incompatible ABI classes")),
                };
                values.push(value);
            }
            let inst=fb.ins().call(reference,&values);
            let result=*fb.inst_results(inst).first()
                .ok_or_else(||reject("FP direct call has no return value"))?;
            (result,*return_type)
        }
        Local(n)=>{
            let var=*locals.get(*n).ok_or_else(||reject("missing FP local"))?;
            let ty=*locals_ty.get(*n).ok_or_else(||reject("missing FP local type"))?;
            (fb.use_var(var),ty)
        }
        Constant32(bits)=>(fb.ins().f32const(Ieee32::with_bits(*bits)),FloatType::F32),
        Constant64(bits)=>(fb.ins().f64const(Ieee64::with_bits(*bits)),FloatType::F64),
        Neg(e)=>{
            let (v,t)=emit_expr(e,fb,params,params_ty,locals,locals_ty,refs)?;
            (fb.ins().fneg(v),t)
        }
        Convert(to,e)=>{
            let (v,t)=emit_expr(e,fb,params,params_ty,locals,locals_ty,refs)?;
            let value=match (t,*to) {
                (FloatType::F32,FloatType::F64)=>fb.ins().fpromote(types::F64,v),
                (FloatType::F64,FloatType::F32)=>fb.ins().fdemote(types::F32,v),
                _=>v,
            };
            (value,*to)
        }
        Add(a,b)|Sub(a,b)|Mul(a,b)|Div(a,b)=>{
            let (x,t)=emit_expr(a,fb,params,params_ty,locals,locals_ty,refs)?;
            let (y,u)=emit_expr(b,fb,params,params_ty,locals,locals_ty,refs)?;
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
        Local(_)|Parameter(_)|IntegerParameterCast {..}|
        Constant32(_)|Constant64(_)=>Ok(()),
        Call {arguments,..}=>{
            for arg in arguments {
                if let ScalarCallArg::Float(expr)=arg {
                    check_local_reads(expr,defined,depth+1)?;
                }
            }
            Ok(())
        },
        Neg(e)|Convert(_,e)=>check_local_reads(e,defined,depth+1),
        Add(a,b)|Sub(a,b)|Mul(a,b)|Div(a,b)=>{
            check_local_reads(a,defined,depth+1)?;
            check_local_reads(b,defined,depth+1)
        }
    }
}
fn check_condition(
    cond:&FloatCondition,params:&[ScalarType],locals:&[FloatType],
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
    params_ty:&[ScalarType],locals:&[Variable],locals_ty:&[FloatType],
    refs:&HashMap<String,FuncRef>,
)->Result<Value,CodegenError> {
    let (x,t)=emit_expr(&cond.left,fb,params,params_ty,locals,locals_ty,refs)?;
    let (y,u)=emit_expr(&cond.right,fb,params,params_ty,locals,locals_ty,refs)?;
    if t!=u {return Err(reject("unconverted mixed FP comparison"));}
    Ok(fb.ins().fcmp(cond.op.cranelift(),x,y))
}
/// Validate one sequential assignment arm against a copy of the incoming
/// definitely-initialized set. Each RHS sees preceding assignments in its
/// own arm; the other arm's effects are never visible during verification.
fn validate_fp_arm(
    writes:&[(usize,FloatExpr)],f:&ScalarFloatBody,mut defined:Vec<bool>,
)->Result<Vec<bool>,CodegenError> {
    if writes.len()>10000 {
        return Err(reject("FP conditional assignment arm is oversized"));
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


/// A control-flow join intersects only reachable predecessors. Never infer
/// initialization from an unreachable return/break/continue branch.
fn fp_join(a:Option<Vec<bool>>,b:Option<Vec<bool>>)->Option<Vec<bool>> {
    match (a,b) {
        (None,other)|(other,None)=>other,
        (Some(mut a),Some(b))=>{
            for (x,y) in a.iter_mut().zip(b) {*x &= y;}
            Some(a)
        }
    }
}
#[derive(Default)]
struct FpFlow {
    normal:Option<Vec<bool>>,
    breaks:Option<Vec<bool>>,
    continues:Option<Vec<bool>>,
}
/// Loop-header facts are those established before the first iteration.
/// Assignments cannot invalidate them, so they are also sound on backedges.
/// In particular, a while/for test may execute before *any* body assignment.
/// For do-while, the test can use the intersection of every normal/continue
/// path through the first body execution.
fn check_fp_statements(
    stmts:&[FloatStmt],f:&ScalarFloatBody,entry:Vec<bool>,
    loop_depth:usize,depth:usize,
)->Result<FpFlow,CodegenError> {
    if depth>64 || stmts.len()>10000 {
        return Err(reject("FP CFG exceeds statement depth or size limit"));
    }
    let mut flow=FpFlow {normal:Some(entry),..FpFlow::default()};
    for stmt in stmts {
        // Preserve fail-closed rejection of unreachable trailing statements.
        let Some(mut defined)=flow.normal.take() else {
            return Err(reject("unreachable trailing FP statement"));
        };
        let next;
        match stmt {
            FloatStmt::Assign(n,expr)=>{
                let ty=*f.locals.get(*n)
                    .ok_or_else(||reject("FP local index out of range"))?;
                check_local_reads(expr,&defined,0)?;
                if expression_type(expr,&f.parameters,&f.locals,0)?!=ty {
                    return Err(reject("FP assignment has an unconverted type"));
                }
                defined[*n]=true;
                next=Some(defined);
            }
            FloatStmt::Return(expr)=>{
                check_local_reads(expr,&defined,0)?;
                if f.result!=ScalarType::Float(
                    expression_type(expr,&f.parameters,&f.locals,0)?) {
                    return Err(reject("FP return has an unconverted type"));
                }
                next=None;
            }
            FloatStmt::ReturnInteger(expr,to)=>{
                if !matches!(to,ScalarType::Int {bits:32|64,..}) ||
                    *to!=f.result {
                    return Err(reject("FP-to-integer return class/signature mismatch"));
                }
                check_local_reads(expr,&defined,0)?;
                expression_type(expr,&f.parameters,&f.locals,0)?;
                next=None;
            }
            FloatStmt::IfReturn(cond,yes,no)=>{
                check_condition(cond,&f.parameters,&f.locals,&defined)?;
                for expr in [yes,no] {
                    check_local_reads(expr,&defined,0)?;
                    if ScalarType::Float(
                        expression_type(expr,&f.parameters,&f.locals,0)?)!=f.result {
                        return Err(reject("FP conditional return has an unconverted type"));
                    }
                }
                next=None;
            }
            FloatStmt::IfAssign(cond,yes,no)=>{
                if yes.is_empty() && no.is_empty() {
                    return Err(reject("FP conditional has no assignment on either path"));
                }
                check_condition(cond,&f.parameters,&f.locals,&defined)?;
                next=fp_join(
                    Some(validate_fp_arm(yes,f,defined.clone())?),
                    Some(validate_fp_arm(no,f,defined)?),
                );
            }
            FloatStmt::IfBlock(cond,yes,no)=>{
                if yes.is_empty() && no.is_empty() {
                    return Err(reject("FP conditional has no statements"));
                }
                check_condition(cond,&f.parameters,&f.locals,&defined)?;
                let y=check_fp_statements(yes,f,defined.clone(),loop_depth,depth+1)?;
                let n=check_fp_statements(no,f,defined,loop_depth,depth+1)?;
                next=fp_join(y.normal,n.normal);
                flow.breaks=fp_join(flow.breaks,fp_join(y.breaks,n.breaks));
                flow.continues=fp_join(flow.continues,fp_join(y.continues,n.continues));
            }
            FloatStmt::While(cond,body)=>{
                check_condition(cond,&f.parameters,&f.locals,&defined)?;
                let exits=check_fp_statements(body,f,defined.clone(),loop_depth+1,depth+1)?;
                // The header may exit before executing the loop once.
                // Break edges may have additional writes but cannot make
                // anything defined on the zero-iteration path.
                next=fp_join(Some(defined),exits.breaks);
            }
            FloatStmt::For {init,condition,increment,body}=>{
                let init_flow=check_fp_statements(init,f,defined,loop_depth,depth+1)?;
                if init_flow.breaks.is_some()||init_flow.continues.is_some() {
                    return Err(reject("FP for initializer cannot transfer loop control"));
                }
                let Some(at_header)=init_flow.normal else {
                    next=None;
                    flow.normal=next;
                    continue;
                };
                if let Some(c)=condition {
                    check_condition(c,&f.parameters,&f.locals,&at_header)?;
                }
                let body_flow=check_fp_statements(body,f,at_header.clone(),
                    loop_depth+1,depth+1)?;
                let at_increment=fp_join(body_flow.normal,body_flow.continues);
                if let Some(state)=at_increment {
                    let step_flow=check_fp_statements(increment,f,state,
                        loop_depth+1,depth+1)?;
                    if step_flow.breaks.is_some()||step_flow.continues.is_some() {
                        return Err(reject("FP for increment cannot transfer loop control"));
                    }
                }
                // A for(;;) has no test-false exit. Only a break reaches
                // the successor; otherwise it is an infinite/returning loop.
                next=if condition.is_some() {
                    fp_join(Some(at_header),body_flow.breaks)
                } else {body_flow.breaks};
            }
            FloatStmt::DoWhile(body,cond)=>{
                let body_flow=check_fp_statements(body,f,defined,
                    loop_depth+1,depth+1)?;
                let at_test=fp_join(body_flow.normal,body_flow.continues);
                if let Some(state)=&at_test {
                    check_condition(cond,&f.parameters,&f.locals,state)?;
                }
                // First test happens after at least one body execution,
                // unless an early break bypasses it. Every arriving edge
                // participates in the definite-assignment intersection.
                next=fp_join(at_test,body_flow.breaks);
            }
            FloatStmt::Break=>{
                if loop_depth==0 {return Err(reject("FP break outside loop"));}
                flow.breaks=fp_join(flow.breaks,Some(defined));
                next=None;
            }
            FloatStmt::Continue=>{
                if loop_depth==0 {return Err(reject("FP continue outside loop"));}
                flow.continues=fp_join(flow.continues,Some(defined));
                next=None;
            }
        }
        flow.normal=next;
    }
    Ok(flow)
}
#[derive(Clone,Copy)]
struct FpLoopTargets<'a> {
    break_to:Block,
    continue_to:Block,
    saw_break:&'a Cell<bool>,
    saw_continue:&'a Cell<bool>,
}
/// Loops leave their headers unsealed until every continue/backedge exists.
/// The Cranelift SSA builder then inserts the required F32/F64 block params.
#[allow(clippy::too_many_arguments)]
fn emit_fp_statements(
    stmts:&[FloatStmt],fb:&mut FunctionBuilder<'_>,params:&[Value],
    params_ty:&[ScalarType],locals:&[Variable],locals_ty:&[FloatType],
    outer:Option<FpLoopTargets<'_>>,
    refs:&HashMap<String,FuncRef>,depth:usize,
)->Result<bool,CodegenError> {
    if depth>64 {return Err(reject("FP CFG emission exceeds recursion limit"));}
    let mut live=true;
    for stmt in stmts {
        if !live {break;}
        match stmt {
            FloatStmt::Assign(n,expr)=>{
                let (v,_)=emit_expr(expr,fb,params,params_ty,locals,locals_ty,refs)?;
                fb.def_var(locals[*n],v);
            }
            FloatStmt::Return(expr)=>{
                let (v,_)=emit_expr(expr,fb,params,params_ty,locals,locals_ty,refs)?;
                fb.ins().return_(&[v]);
                live=false;
            }
            FloatStmt::ReturnInteger(expr,to)=>{
                let (v,_)=emit_expr(expr,fb,params,params_ty,locals,locals_ty,refs)?;
                let converted=match to {
                    ScalarType::Int {signed:true,..}=>
                        fb.ins().fcvt_to_sint(to.machine(),v),
                    ScalarType::Int {signed:false,..}=>
                        fb.ins().fcvt_to_uint(to.machine(),v),
                    _=>return Err(reject("non-integer FP conversion return")),
                };
                fb.ins().return_(&[converted]);
                live=false;
            }
            FloatStmt::IfReturn(cond,yes,no)=>{
                let flag=emit_condition(cond,fb,params,params_ty,locals,locals_ty,refs)?;
                let t=fb.create_block();
                let f=fb.create_block();
                fb.ins().brif(flag,t,&[],f,&[]);
                fb.seal_block(t);
                fb.seal_block(f);
                fb.switch_to_block(t);
                let (v,_)=emit_expr(yes,fb,params,params_ty,locals,locals_ty,refs)?;
                fb.ins().return_(&[v]);
                fb.switch_to_block(f);
                let (v,_)=emit_expr(no,fb,params,params_ty,locals,locals_ty,refs)?;
                fb.ins().return_(&[v]);
                live=false;
            }
            FloatStmt::IfBlock(cond,yes,no)=>{
                live=emit_fp_if(cond,yes,no,fb,params,params_ty,
                    locals,locals_ty,outer,refs,depth+1)?;
            }
            FloatStmt::IfAssign(cond,yes,no)=>{
                let a:Vec<_>=yes.iter().map(|(n,e)|
                    FloatStmt::Assign(*n,e.clone())).collect();
                let b:Vec<_>=no.iter().map(|(n,e)|
                    FloatStmt::Assign(*n,e.clone())).collect();
                live=emit_fp_if(cond,&a,&b,fb,params,params_ty,
                    locals,locals_ty,outer,refs,depth+1)?;
            }
            FloatStmt::While(cond,body)=>{
                let head=fb.create_block();
                let body_block=fb.create_block();
                let exit=fb.create_block();
                fb.ins().jump(head,&[]);
                fb.switch_to_block(head);
                let test=emit_condition(cond,fb,params,params_ty,locals,locals_ty,refs)?;
                fb.ins().brif(test,body_block,&[],exit,&[]);
                fb.seal_block(body_block);
                fb.switch_to_block(body_block);
                let saw_break=Cell::new(false);
                let saw_continue=Cell::new(false);
                let inner=FpLoopTargets {break_to:exit,continue_to:head,
                    saw_break:&saw_break,saw_continue:&saw_continue};
                if emit_fp_statements(body,fb,params,params_ty,locals,
                    locals_ty,Some(inner),refs,depth+1)? {
                    fb.ins().jump(head,&[]);
                }
                fb.seal_block(head);
                fb.seal_block(exit);
                fb.switch_to_block(exit);
            }
            FloatStmt::For {init,condition,increment,body}=>{
                if !emit_fp_statements(init,fb,params,params_ty,locals,
                    locals_ty,outer,refs,depth+1)? {live=false;continue;}
                let head=fb.create_block();
                let body_block=fb.create_block();
                let step=fb.create_block();
                let exit=fb.create_block();
                fb.ins().jump(head,&[]);
                fb.switch_to_block(head);
                if let Some(cond)=condition {
                    let test=emit_condition(cond,fb,params,params_ty,locals,locals_ty,refs)?;
                    fb.ins().brif(test,body_block,&[],exit,&[]);
                } else {fb.ins().jump(body_block,&[]);}
                fb.seal_block(body_block);
                fb.switch_to_block(body_block);
                let saw_break=Cell::new(false);
                let saw_continue=Cell::new(false);
                let inner=FpLoopTargets {break_to:exit,continue_to:step,
                    saw_break:&saw_break,saw_continue:&saw_continue};
                let body_live=emit_fp_statements(body,fb,params,params_ty,
                    locals,locals_ty,Some(inner),refs,depth+1)?;
                if body_live {fb.ins().jump(step,&[]);}
                fb.seal_block(step);
                if body_live||saw_continue.get() {
                    fb.switch_to_block(step);
                    if emit_fp_statements(increment,fb,params,params_ty,
                        locals,locals_ty,Some(inner),refs,depth+1)? {
                        fb.ins().jump(head,&[]);
                    }
                }
                fb.seal_block(head);
                fb.seal_block(exit);
                if condition.is_some()||saw_break.get() {
                    fb.switch_to_block(exit);
                } else {live=false;}
            }
            FloatStmt::DoWhile(body,cond)=>{
                let body_block=fb.create_block();
                let test_block=fb.create_block();
                let exit=fb.create_block();
                fb.ins().jump(body_block,&[]);
                fb.switch_to_block(body_block);
                let saw_break=Cell::new(false);
                let saw_continue=Cell::new(false);
                let inner=FpLoopTargets {break_to:exit,continue_to:test_block,
                    saw_break:&saw_break,saw_continue:&saw_continue};
                let body_live=emit_fp_statements(body,fb,params,params_ty,
                    locals,locals_ty,Some(inner),refs,depth+1)?;
                if body_live {fb.ins().jump(test_block,&[]);}
                fb.seal_block(test_block);
                if body_live||saw_continue.get() {
                    fb.switch_to_block(test_block);
                    let test=emit_condition(cond,fb,params,params_ty,locals,locals_ty,refs)?;
                    fb.ins().brif(test,body_block,&[],exit,&[]);
                }
                fb.seal_block(body_block);
                fb.seal_block(exit);
                if body_live||saw_continue.get()||saw_break.get() {
                    fb.switch_to_block(exit);
                } else {live=false;}
            }
            FloatStmt::Break=>{
                let target=outer.ok_or_else(||reject("FP break outside loop"))?;
                fb.ins().jump(target.break_to,&[]);
                target.saw_break.set(true);
                live=false;
            }
            FloatStmt::Continue=>{
                let target=outer.ok_or_else(||reject("FP continue outside loop"))?;
                fb.ins().jump(target.continue_to,&[]);
                target.saw_continue.set(true);
                live=false;
            }
        }
    }
    Ok(live)
}
#[allow(clippy::too_many_arguments)]
fn emit_fp_if(
    cond:&FloatCondition,yes:&[FloatStmt],no:&[FloatStmt],
    fb:&mut FunctionBuilder<'_>,params:&[Value],params_ty:&[ScalarType],
    locals:&[Variable],locals_ty:&[FloatType],
    outer:Option<FpLoopTargets<'_>>,
    refs:&HashMap<String,FuncRef>,depth:usize,
)->Result<bool,CodegenError> {
    if depth>64 {return Err(reject("FP nested if exceeds recursion limit"));}
    let test=emit_condition(cond,fb,params,params_ty,locals,locals_ty,refs)?;
    let t=fb.create_block();
    let f=fb.create_block();
    fb.ins().brif(test,t,&[],f,&[]);
    fb.seal_block(t);
    fb.seal_block(f);
    fb.switch_to_block(t);
    let t_live=emit_fp_statements(yes,fb,params,params_ty,
        locals,locals_ty,outer,refs,depth+1)?;
    let join=if t_live {
        let b=fb.create_block();
        fb.ins().jump(b,&[]);
        Some(b)
    } else {None};
    fb.switch_to_block(f);
    let f_live=emit_fp_statements(no,fb,params,params_ty,
        locals,locals_ty,outer,refs,depth+1)?;
    let join=if f_live {
        let b=join.unwrap_or_else(||fb.create_block());
        fb.ins().jump(b,&[]);
        Some(b)
    } else {join};
    if let Some(b)=join {
        fb.seal_block(b);
        fb.switch_to_block(b);
        Ok(true)
    } else {Ok(false)}
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
    let scalar:Vec<_>=functions.iter().map(|f|ScalarFloatBody {
        name:f.name.clone(),
        parameters:f.parameters.iter().copied().map(ScalarType::Float).collect(),
        result:ScalarType::Float(f.result),
        locals:f.locals.clone(),statements:f.statements.clone(),
    }).collect();
    emit_scalar_float_bodies(&scalar)
}

/// Verify that every call resolves to a definition in the same module with
/// the precise native ABI signature. Unsupported external or indirect calls
/// are rejected before any ELF object is produced.
fn validate_fp_calls_expr(
    expr:&FloatExpr,
    declarations:&HashMap<String,(Vec<ScalarType>,ScalarType)>,
    depth:usize,
)->Result<(),CodegenError> {
    if depth>64 {return Err(reject("FP call expression exceeds depth limit"));}
    match expr {
        FloatExpr::Call {callee,parameters,return_type,arguments}=>{
            let declared=declarations.get(callee)
                .ok_or_else(||reject("FP direct call has no defined routine"))?;
            if declared.0!=*parameters ||
                declared.1!=ScalarType::Float(*return_type) ||
                arguments.len()!=parameters.len() {
                return Err(reject("FP direct call signature differs from defined routine"));
            }
            for arg in arguments {
                if let ScalarCallArg::Float(arg)=arg {
                    validate_fp_calls_expr(arg,declarations,depth+1)?;
                }
            }
        }
        FloatExpr::Neg(e)|FloatExpr::Convert(_,e)=>
            validate_fp_calls_expr(e,declarations,depth+1)?,
        FloatExpr::Add(a,b)|FloatExpr::Sub(a,b)|
        FloatExpr::Mul(a,b)|FloatExpr::Div(a,b)=>{
            validate_fp_calls_expr(a,declarations,depth+1)?;
            validate_fp_calls_expr(b,declarations,depth+1)?;
        }
        FloatExpr::Parameter(_)|FloatExpr::IntegerParameterCast {..}|
        FloatExpr::Local(_)|FloatExpr::Constant32(_)|
        FloatExpr::Constant64(_)=>{},
    }
    Ok(())
}
fn validate_fp_calls_cond(
    cond:&FloatCondition,
    declarations:&HashMap<String,(Vec<ScalarType>,ScalarType)>,
    depth:usize,
)->Result<(),CodegenError> {
    validate_fp_calls_expr(&cond.left,declarations,depth)?;
    validate_fp_calls_expr(&cond.right,declarations,depth)
}
fn validate_fp_calls_stmts(
    stmts:&[FloatStmt],
    declarations:&HashMap<String,(Vec<ScalarType>,ScalarType)>,
    depth:usize,
)->Result<(),CodegenError> {
    if depth>64 {return Err(reject("FP call statements exceed depth limit"));}
    for stmt in stmts {
        match stmt {
            FloatStmt::Assign(_,e)|FloatStmt::Return(e)|
            FloatStmt::ReturnInteger(e,_)=>
                validate_fp_calls_expr(e,declarations,depth+1)?,
            FloatStmt::IfReturn(cond,yes,no)=>{
                validate_fp_calls_cond(cond,declarations,depth+1)?;
                validate_fp_calls_expr(yes,declarations,depth+1)?;
                validate_fp_calls_expr(no,declarations,depth+1)?;
            }
            FloatStmt::IfAssign(cond,yes,no)=>{
                validate_fp_calls_cond(cond,declarations,depth+1)?;
                for (_,e) in yes.iter().chain(no) {
                    validate_fp_calls_expr(e,declarations,depth+1)?;
                }
            }
            FloatStmt::IfBlock(cond,yes,no)=>{
                validate_fp_calls_cond(cond,declarations,depth+1)?;
                validate_fp_calls_stmts(yes,declarations,depth+1)?;
                validate_fp_calls_stmts(no,declarations,depth+1)?;
            }
            FloatStmt::While(cond,body)=>{
                validate_fp_calls_cond(cond,declarations,depth+1)?;
                validate_fp_calls_stmts(body,declarations,depth+1)?;
            }
            FloatStmt::DoWhile(body,cond)=>{
                validate_fp_calls_stmts(body,declarations,depth+1)?;
                validate_fp_calls_cond(cond,declarations,depth+1)?;
            }
            FloatStmt::For {init,condition,increment,body}=>{
                validate_fp_calls_stmts(init,declarations,depth+1)?;
                if let Some(cond)=condition {
                    validate_fp_calls_cond(cond,declarations,depth+1)?;
                }
                validate_fp_calls_stmts(body,declarations,depth+1)?;
                validate_fp_calls_stmts(increment,declarations,depth+1)?;
            }
            FloatStmt::Break|FloatStmt::Continue=>{},
        }
    }
    Ok(())
}
fn emit_scalar_float_bodies(functions:&[ScalarFloatBody])
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
    let mut declared=HashMap::new();
    for f in functions {
        if declared.insert(f.name.clone(),
            (f.parameters.clone(),f.result)).is_some() {
            return Err(reject("duplicate scalar routine definition"));
        }
    }
    let mut exports=Vec::new();
    for f in functions {
        validate_fp_calls_stmts(&f.statements,&declared,0)?;
        if f.name.is_empty()||f.name.contains('\0') ||
            f.parameters.len()>256 || f.locals.len()>4096 {
            return Err(reject("invalid FP function name or variable count"));
        }
        if f.statements.is_empty() || f.statements.len()>10000 {
            return Err(reject("FP function body is empty or oversized"));
        }
        let checked=check_fp_statements(&f.statements,f,
            vec![false;f.locals.len()],0,0)?;
        if checked.normal.is_some() {
            return Err(reject("FP function has reachable fallthrough without return"));
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
        // Import every definition before lowering its body; this supports
        // forward calls and checked recursive direct calls.
        let mut refs=HashMap::new();
        for (other,other_id) in functions.iter().zip(&exports) {
            let reference=module.declare_func_in_func(*other_id,&mut ctx.func);
            refs.insert(other.name.clone(),reference);
        }
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
            if emit_fp_statements(&f.statements,&mut fb,&params,&f.parameters,
                &vars,&f.locals,None,&refs,0)? {
                return Err(reject("FP emission ended with a reachable fallthrough"));
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
/// Reject a nonstandard ABI class, unsupported integer width, or synthetic
/// type before generating code. Only standard signed/unsigned 32/64 bit C
/// integer formals and native F32/F64 values enter this first mixed ABI slice.
fn scalar_type_from_edg(il:&Il,id:crate::TypeId)
    ->Result<ScalarType,CodegenError> {
    if let Ok(t)=type_from_edg(il,id) {return Ok(ScalarType::Float(t));}
    let canonical=super::canonical_abi_type(il,id)?;
    let info=il.c_integer_types.iter().find(|x|x.id==canonical)
        .ok_or_else(||reject("unsupported mixed scalar parameter ABI type"))?;
    let layout=il.type_object_sizes.iter().find(|x|x.owner==canonical)
        .ok_or_else(||reject("mixed INTEGER formal lacks native size/alignment"))?;
    if info.is_bool || !matches!(info.bits,32|64) ||
        layout.bytes!=u64::from(info.bits/8) ||
        layout.alignment!=u64::from(info.bits/8) {
        return Err(reject("unsupported mixed integer width or alignment"));
    }
    Ok(ScalarType::Int {bits:info.bits,signed:info.signed})
}
/// Source expressions are checked using exact EDG symbol IDs and widths.
struct FpSourceCtx<'a> {
    il:&'a Il,
    graph:&'a crate::graph::Graph,
    routine:crate::SymbolId,
    params:&'a [crate::SymbolId],
    locals:&'a [crate::SymbolId],
    param_types:&'a [ScalarType],
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
    if let Some(site)=ctx.graph.calls.iter().find(|site|
        site.function==node.data.function &&
        site.statement_path==node.data.statement_path &&
        site.path==node.data.path) {
        if site.first_argument!=1 || node.operands.is_empty() {
            return Err(reject("FP direct call has unsupported implicit/member arguments"));
        }
        let callee=ctx.graph.expression(node.operands[0])
            .ok_or_else(||reject("FP direct call has no callee expression"))?;
        if super::semantic(2,callee.data.kind)!=6 ||
            !callee.operands.is_empty() {
            return Err(reject("FP indirect/function-pointer call unsupported"));
        }
        let symbol=callee.data.symbol_id.ok_or_else(||
            reject("FP direct callee has no native symbol"))?;
        let named=ctx.il.symbols.iter().find(|s|
            s.id==symbol && s.kind==1)
            .ok_or_else(||reject("FP direct call target is not a named routine"))?;
        let callee_name=named.name.clone();
        if !ctx.il.entities.iter().any(|e|matches!(e,
            crate::Entity::Routine{name,has_definition:true}
                if name==&callee_name)) {
            return Err(reject("FP direct call requires a definition in this source module"));
        }
        let callee_ty=ctx.il.symbol_types.iter()
            .find(|p|p.symbol==symbol)
            .ok_or_else(||reject("FP direct callee has no native function type"))?;
        let fn_ty=super::canonical_abi_type(ctx.il,callee_ty.ty)?;
        let func=ctx.il.types.iter().find(|p|p.id==fn_ty &&
            super::semantic(4,p.kind)==2 && p.related_role==Some(3))
            .ok_or_else(||reject("FP direct call needs an exact function prototype"))?;
        let proto=ctx.il.function_signatures.iter()
            .find(|p|p.owner==fn_ty)
            .ok_or_else(||reject("FP direct callee missing prototype metadata"))?;
        if !proto.prototyped || proto.variadic {
            return Err(reject("unprototyped/variadic FP direct call unsupported"));
        }
        let returned=scalar_type_from_edg(ctx.il,func.related
            .ok_or_else(||reject("FP direct callee has no return type"))?)?;
        if returned!=ScalarType::Float(ty) {
            return Err(reject("FP direct-call result type/ABI differs from expression"));
        }
        let mut declared:Vec<_>=ctx.il.type_parameters.iter()
            .filter(|p|p.owner==fn_ty).collect();
        declared.sort_by_key(|p|p.position);
        if declared.iter().enumerate().any(|(i,p)|p.position!=i as u32) ||
            declared.len()!=node.operands.len()-1 {
            return Err(reject("FP direct call argument count/position mismatch"));
        }
        let mut parameters=Vec::with_capacity(declared.len());
        let mut arguments=Vec::with_capacity(declared.len());
        for (param,&operand) in declared.iter().zip(&node.operands[1..]) {
            let target=scalar_type_from_edg(ctx.il,param.ty)?;
            let value=match target {
                ScalarType::Float(t)=>{
                    let e=expression_from_edg(ctx,operand,depth+1)?;
                    if expression_type(&e,ctx.param_types,ctx.local_types,0)?!=t {
                        return Err(reject("FP direct argument has unconverted type"));
                    }
                    ScalarCallArg::Float(e)
                }
                ScalarType::Int {..}=>{
                    let e=ctx.graph.expression(operand).ok_or_else(||
                        reject("FP direct INTEGER arg has no native expression"))?;
                    if super::semantic(2,e.data.kind)!=2 ||
                        e.data.constant_kind.is_some() || !e.operands.is_empty() {
                        return Err(reject("FP direct INTEGER argument must be a formal"));
                    }
                    let id=e.data.symbol_id.ok_or_else(||
                        reject("FP direct INTEGER argument lacks symbol"))?;
                    let index=ctx.params.iter().position(|s|*s==id)
                        .ok_or_else(||reject("FP direct INTEGER argument not a formal"))?;
                    let actual=scalar_type_from_edg(ctx.il,e.data.type_id
                        .ok_or_else(||reject("FP direct INTEGER argument type missing"))?)?;
                    if actual!=target || ctx.param_types[index]!=target {
                        return Err(reject("FP direct INTEGER argument has wrong type"));
                    }
                    ScalarCallArg::IntegerParameter(index)
                }
            };
            parameters.push(target);
            arguments.push(value);
        }
        return Ok(FloatExpr::Call {
            callee:callee_name,parameters,return_type:ty,arguments,
        });
    }
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
            if ctx.param_types[i]!=ScalarType::Float(ty) {
                return Err(reject("FP formal rvalue type mismatch"));
            }
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
    // Only an EDG-authoritative conversion from a direct integer formal is
    // accepted. Integer expressions, local memory and implicit bitcasting
    // are *not* silently accepted as scalar floating point values.
    if node.operands.len()==1 && matches!(op,11|12|16) {
        let source=ctx.graph.expression(node.operands[0])
            .ok_or_else(||reject("missing integer-to-FP cast operand"))?;
        let source_ty=source.data.type_id.ok_or_else(||
            reject("integer-to-FP cast operand has no native type"))?;
        if let ScalarType::Int {..}=scalar_type_from_edg(ctx.il,source_ty)? {
            if super::semantic(2,source.data.kind)!=2 ||
                !source.operands.is_empty() || source.data.constant_kind.is_some() {
                return Err(reject("integer-to-FP cast requires a direct integer formal"));
            }
            let symbol=source.data.symbol_id.ok_or_else(||
                reject("integer-to-FP cast operand lacks native symbol"))?;
            let index=ctx.params.iter().position(|s|*s==symbol)
                .ok_or_else(||reject("integer-to-FP cast does not reference a formal"))?;
            if ctx.param_types[index]!=scalar_type_from_edg(ctx.il,source_ty)? {
                return Err(reject("integer-to-FP formal's native type differs"));
            }
            return Ok(FloatExpr::IntegerParameterCast {parameter:index,to:ty});
        }
    }
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
/// Lower EDG source into checked typed FP CFG. Loop bodies and for increment
/// use EDG-owned source statement identities, including its synthetic
/// increment statement; no condition is fabricated for for(;;).
/// Memory effects, unverified nonloop gotos and mixed ABI remain rejected.
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
        3 if node.expression.is_some() && (1..=2).contains(&node.children.len())=>{
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
                let Some(arm)=arm else {return Ok(Vec::new());};
                let mut statements=Vec::new();
                lower_fp_body(ctx,arm,&mut statements,depth+1)?;
                if statements.len()>10000 {
                    return Err(reject("FP if arm is oversized"));
                }
                Ok(statements)
            };
            // EDG omits a null else statement entirely. It must remain an
            // explicit no-op CFG predecessor, inheriting live-in locals.
            if then_stmt.is_none() {return Err(reject("FP if missing then branch"));}
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
                // Either side may do no work. An absent else and an explicit
                // empty else both keep the incoming values on that CFG edge.
                // No general nested CFG, branch termination or memory effect.
                let assignments=|stmts:Vec<FloatStmt>|->Vec<(usize,FloatExpr)> {
                    stmts.into_iter().map(|s|match s {
                        FloatStmt::Assign(n,e)=>(n,e),
                        _=>unreachable!(),
                    }).collect()
                };
                output.push(FloatStmt::IfAssign(
                    cond,assignments(yes),assignments(no)));
            } else {
                // The general CFG validator checks return/break/continue
                // reachability independently for each branch.
                output.push(FloatStmt::IfBlock(cond,yes,no));
            }
        }
        2 if node.expression.is_none()=>{
            for child in &node.children {
                lower_fp_body(ctx,*child,output,depth+1)?;
            }
        }

        7|8=>{
            let condition=comparison_from_edg(ctx,node.expression
                .ok_or_else(||reject("FP loop missing comparison"))?)?;
            if node.children.len()!=1 {
                return Err(reject("FP while/do body must have one source child"));
            }
            let child=node.children[0];
            let body_node=ctx.graph.statement(child)
                .ok_or_else(||reject("FP loop missing child statement"))?;
            if !body_node.data.path.starts_with(&format!("{path}.body.")) {
                return Err(reject("FP loop body source path mismatch"));
            }
            let mut body=Vec::new();
            lower_fp_body(ctx,child,&mut body,depth+1)?;
            if super::semantic(1,node.data.kind)==8 {
                output.push(FloatStmt::DoWhile(body,condition));
            } else {
                output.push(FloatStmt::While(condition,body));
            }
        }
        9=>{
            let condition=node.expression
                .map(|e|comparison_from_edg(ctx,e)).transpose()?;
            let mut init=None;
            let mut body=None;
            let mut increment=None;
            for &child in &node.children {
                let found=ctx.graph.statement(child)
                    .ok_or_else(||reject("FP for missing child"))?;
                let source=&found.data.path;
                if source.starts_with(&format!("{path}.init.")) {
                    if init.replace(child).is_some() {
                        return Err(reject("duplicate FP for initializer"));
                    }
                } else if source.starts_with(&format!("{path}.body.")) {
                    if body.replace(child).is_some() {
                        return Err(reject("duplicate FP for body"));
                    }
                } else if source==&format!("{path}.increment.0") {
                    if increment.replace(child).is_some() {
                        return Err(reject("duplicate FP for increment"));
                    }
                } else {
                    return Err(reject("unexpected FP for source child"));
                }
            }
            let mut init_stmts=Vec::new();
            if let Some(id)=init {
                lower_fp_body(ctx,id,&mut init_stmts,depth+1)?;
            }
            let mut body_stmts=Vec::new();
            lower_fp_body(ctx,body.ok_or_else(||
                reject("FP for has no body"))?,&mut body_stmts,depth+1)?;
            let mut step_stmts=Vec::new();
            if let Some(id)=increment {
                lower_fp_body(ctx,id,&mut step_stmts,depth+1)?;
            }
            output.push(FloatStmt::For {
                init:init_stmts,condition,increment:step_stmts,
                body:body_stmts,
            });
        }
        10|11=>{
            let transfer:Vec<_>=ctx.il.loop_transfers.iter()
                .filter(|t|t.function==*function && t.statement_path==*path)
                .collect();
            // Only native compiler-classified structured transfers. User
            // gotos and ordinary labels are not guessed from name or path.
            if transfer.len()!=1 || ctx.graph.statement_label(id).is_none() {
                return Err(reject("unsupported FP user goto/label"));
            }
            match (super::semantic(1,node.data.kind),transfer[0].kind) {
                (10,1)=>output.push(FloatStmt::Break),
                (10,2)=>output.push(FloatStmt::Continue),
                (11,3|4|6)=>{},
                _=>return Err(reject("inconsistent FP loop transfer metadata")),
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
            let native=ctx.graph.expression(source)
                .ok_or_else(||reject("missing native FP return expression"))?;
            let ty=native.data.type_id.ok_or_else(||
                reject("FP return expression has no native type"))?;
            match scalar_type_from_edg(ctx.il,ty)? {
                ScalarType::Float(_)=>
                    output.push(FloatStmt::Return(expression_from_edg(ctx,source,0)?)),
                result @ ScalarType::Int {..}=>{
                    // Only a unary, source-recorded cast from a standard FP
                    // expression has a defined lowering here. Never turn
                    // FP comparisons, integer arithmetic or implicit symbol
                    // reinterpretation into a native integer return.
                    if super::semantic(2,native.data.kind)!=3 ||
                        native.operands.len()!=1 ||
                        !matches!(native.data.operator.map(|v|
                            super::semantic(3,v)),Some(11|12|16)) {
                        return Err(reject("integer return requires EDG FP conversion node"));
                    }
                    let operand=native.operands[0];
                    let value=expression_from_edg(ctx,operand,0)?;
                    output.push(FloatStmt::ReturnInteger(value,result));
                }
            }
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
        let ret=scalar_type_from_edg(il,func.related.ok_or_else(||
            reject("missing FP return type"))?)?;
        let mut args:Vec<_>=il.function_parameters.iter()
            .filter(|p|p.routine==routine).collect();
        args.sort_by_key(|p|p.position);
        if args.iter().enumerate().any(|(i,p)|
            p.position!=Some(i as u32) || p.implicit_this || p.compiler_generated) {
            return Err(reject("invalid FP explicit parameter list"));
        }
        let params=args.iter().map(|p|scalar_type_from_edg(il,p.ty))
            .collect::<Result<Vec<_>,_>>()?;
        let declared:Vec<_>=il.type_parameters.iter()
            .filter(|p|p.owner==func_ty).collect();
        if declared.len()!=args.len() {
            return Err(reject("FP prototype/definition arity mismatch"));
        }
        for arg in &args {
            let d=declared.iter().find(|d|Some(d.position)==arg.position)
                .ok_or_else(||reject("FP prototype position missing"))?;
            if scalar_type_from_edg(il,d.ty)?!=scalar_type_from_edg(il,arg.ty)? {
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
        functions.push(ScalarFloatBody {
            name:name.to_string(),parameters:params,result:ret,
            locals:local_types,statements,
        });
    }
    emit_scalar_float_bodies(&functions).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn param(n:usize)->FloatExpr {FloatExpr::Parameter(n)}
    fn bin(a:usize,b:usize)->FloatExpr {
        FloatExpr::Add(Box::new(param(a)),Box::new(param(b)))
    }
    #[test]
    fn mixed_intra_module_direct_calls_check_full_typed_abi() {
        let signature=vec![
            ScalarType::Int {bits:32,signed:true},
            ScalarType::Float(FloatType::F64),
        ];
        let base=ScalarFloatBody {
            name:"m14_call_target".into(),
            parameters:signature.clone(),
            result:ScalarType::Float(FloatType::F64),
            locals:vec![],
            statements:vec![FloatStmt::Return(FloatExpr::Add(
                Box::new(FloatExpr::IntegerParameterCast {
                    parameter:0,to:FloatType::F64,
                }),
                Box::new(FloatExpr::Parameter(1)),
            ))],
        };
        let call=FloatExpr::Call {
            callee:base.name.clone(),
            parameters:signature.clone(),
            return_type:FloatType::F64,
            arguments:vec![
                ScalarCallArg::IntegerParameter(0),
                ScalarCallArg::Float(FloatExpr::Parameter(1)),
            ],
        };
        let caller=ScalarFloatBody {
            name:"m14_call_caller".into(),
            parameters:signature,
            result:ScalarType::Float(FloatType::F64),
            locals:vec![],
            statements:vec![FloatStmt::Return(call.clone())],
        };
        let elf=emit_scalar_float_bodies(&[caller.clone(),base.clone()]).unwrap();
        assert_eq!(&elf[..4],b"\x7fELF");
        let mut bad=caller.clone();
        bad.statements=vec![FloatStmt::Return(FloatExpr::Call {
            callee:"missing".into(),..call.clone()
        })];
        assert!(emit_scalar_float_bodies(&[bad,base.clone()]).is_err());
        let mut bad=caller.clone();
        bad.statements=vec![FloatStmt::Return(FloatExpr::Call {
            parameters:vec![ScalarType::Float(FloatType::F64),
                ScalarType::Int {bits:32,signed:true}],
            ..call.clone()
        })];
        assert!(emit_scalar_float_bodies(&[bad,base.clone()]).is_err());
        let mut bad=caller;
        bad.statements=vec![FloatStmt::Return(FloatExpr::Call {
            arguments:vec![ScalarCallArg::IntegerParameter(0),
                ScalarCallArg::IntegerParameter(0)],..call.clone()
        })];
        assert!(emit_scalar_float_bodies(&[bad,base.clone()]).is_err());
        let mut bad=base.clone();
        bad.result=ScalarType::Int {bits:32,signed:true};
        bad.statements=vec![FloatStmt::ReturnInteger(
            FloatExpr::Parameter(1),bad.result,
        )];
        let caller=ScalarFloatBody {
            name:"caller_int_result".into(),
            parameters:vec![],
            result:ScalarType::Float(FloatType::F64),
            locals:vec![],
            statements:vec![FloatStmt::Return(call)],
        };
        assert!(emit_scalar_float_bodies(&[caller,bad]).is_err());
    }

    #[test]
    fn scalar_fp_to_integer_returns_type_check_and_emit_rax() {
        let types=[
            ScalarType::Int {bits:32,signed:true},
            ScalarType::Int {bits:32,signed:false},
            ScalarType::Int {bits:64,signed:true},
            ScalarType::Int {bits:64,signed:false},
        ];
        for (index,result) in types.iter().copied().enumerate() {
            let f=ScalarFloatBody {
                name:format!("m14_integer_result_{index}"),
                parameters:vec![ScalarType::Int {bits:32,signed:true},
                                ScalarType::Float(FloatType::F64)],
                result,locals:vec![],
                statements:vec![FloatStmt::ReturnInteger(
                    FloatExpr::Add(
                        Box::new(FloatExpr::IntegerParameterCast {
                            parameter:0,to:FloatType::F64,
                        }),
                        Box::new(FloatExpr::Parameter(1))),
                    result,
                )],
            };
            assert!(emit_scalar_float_bodies(&[f.clone()]).is_ok());
            let mut bad=f.clone();
            bad.statements=vec![FloatStmt::Return(FloatExpr::Parameter(1))];
            assert!(emit_scalar_float_bodies(&[bad]).is_err());
            let mut bad=f.clone();
            bad.statements=vec![FloatStmt::ReturnInteger(
                FloatExpr::Parameter(1),ScalarType::Float(FloatType::F64))];
            assert!(emit_scalar_float_bodies(&[bad]).is_err());
            let mut bad=f;
            bad.statements=vec![FloatStmt::ReturnInteger(
                FloatExpr::IntegerParameterCast {
                    parameter:1,to:FloatType::F64,
                },result)];
            assert!(emit_scalar_float_bodies(&[bad]).is_err());
        }
    }

    #[test]
    fn mixed_integer_sse_formals_are_checked_and_explicitly_converted() {
        use ScalarType::*;
        let params=vec![Int {bits:32,signed:true},Float(FloatType::F64),
            Int {bits:64,signed:false},Float(FloatType::F32)];
        let i0=FloatExpr::IntegerParameterCast {
            parameter:0,to:FloatType::F64,
        };
        let i2=FloatExpr::IntegerParameterCast {
            parameter:2,to:FloatType::F64,
        };
        let rhs=FloatExpr::Convert(FloatType::F64,
            Box::new(FloatExpr::Parameter(3)));
        let expr=FloatExpr::Add(Box::new(FloatExpr::Add(
            Box::new(i0.clone()),Box::new(FloatExpr::Parameter(1)))),
            Box::new(FloatExpr::Add(Box::new(i2.clone()),Box::new(rhs))));
        let func=ScalarFloatBody {
            name:"m14_mixed_unit".into(),
            parameters:params.clone(),result:ScalarType::Float(FloatType::F64),
            locals:vec![],statements:vec![FloatStmt::Return(expr)],
        };
        let elf=emit_scalar_float_bodies(&[func.clone()]).unwrap();
        assert_eq!(&elf[..4],b"\x7fELF");
        // Integers may not masquerade as the float arguments with which
        // they share source order, nor can an FP argument be cast as integer.
        let mut bad=func.clone();
        bad.statements=vec![FloatStmt::Return(FloatExpr::Parameter(0))];
        assert!(emit_scalar_float_bodies(&[bad]).is_err());
        let mut bad=func.clone();
        bad.statements=vec![FloatStmt::Return(FloatExpr::IntegerParameterCast {
            parameter:1,to:FloatType::F64,
        })];
        assert!(emit_scalar_float_bodies(&[bad]).is_err());
        let mut bad=func;
        bad.parameters[0]=Float(FloatType::F64);
        bad.statements=vec![FloatStmt::Return(i0)];
        assert!(emit_scalar_float_bodies(&[bad]).is_err());
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
        // Empty false (or true) arms carry a previously defined value.
        let mut no_else=valid.clone();
        no_else.statements=vec![
            FloatStmt::Assign(0,FloatExpr::Parameter(0)),
            FloatStmt::IfAssign(cond.clone(),
                vec![(0,FloatExpr::Parameter(1))],vec![]),
            FloatStmt::Return(FloatExpr::Local(0)),
        ];
        assert!(emit_float_bodies(&[no_else]).is_ok());
        let mut empty_then=valid.clone();
        empty_then.statements=vec![
            FloatStmt::Assign(0,FloatExpr::Parameter(0)),
            FloatStmt::IfAssign(cond.clone(),
                vec![],vec![(0,FloatExpr::Parameter(1))]),
            FloatStmt::Return(FloatExpr::Local(0)),
        ];
        assert!(emit_float_bodies(&[empty_then]).is_ok());
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
        let mut missing_else_uninit=valid.clone();
        missing_else_uninit.statements=vec![
            FloatStmt::IfAssign(cond.clone(),
                vec![(0,FloatExpr::Parameter(0))],vec![]),
            FloatStmt::Return(FloatExpr::Local(0)),
        ];
        assert!(emit_float_bodies(&[missing_else_uninit]).is_err());
        let mut no_effect=valid;
        no_effect.statements=vec![
            FloatStmt::Assign(0,FloatExpr::Parameter(0)),
            FloatStmt::IfAssign(cond,vec![],vec![]),
            FloatStmt::Return(FloatExpr::Local(0)),
        ];
        assert!(emit_float_bodies(&[no_effect]).is_err());
    }

    #[test]
    fn fp_while_for_do_while_loop_phi_and_flow_safety() {
        let lt=FloatCondition {
            op:FloatPredicate::Lt,
            left:FloatExpr::Local(0),
            right:FloatExpr::Parameter(1),
        };
        let plus_one=FloatExpr::Add(Box::new(FloatExpr::Local(0)),
            Box::new(FloatExpr::Constant64(1.0f64.to_bits())));
        let mut f=FloatBodyFunction {
            name:"m14_while_phi".into(),
            parameters:vec![FloatType::F64,FloatType::F64],
            locals:vec![FloatType::F64,FloatType::F64],
            result:FloatType::F64,
            statements:vec![
                FloatStmt::Assign(0,FloatExpr::Parameter(0)),
                FloatStmt::Assign(1,FloatExpr::Constant64(0.0f64.to_bits())),
                FloatStmt::While(lt.clone(),vec![
                    FloatStmt::Assign(1,FloatExpr::Add(
                        Box::new(FloatExpr::Local(1)),
                        Box::new(FloatExpr::Local(0)))),
                    FloatStmt::Assign(0,plus_one.clone()),
                ]),
                FloatStmt::Return(FloatExpr::Local(1)),
            ],
        };
        let object=emit_float_bodies(&[f.clone()]).unwrap();
        assert_eq!(&object[..4],b"\x7fELF");
        f.name="m14_for_phi".into();
        f.statements=vec![
            FloatStmt::Assign(1,FloatExpr::Constant64(0.0f64.to_bits())),
            FloatStmt::For {
                init:vec![FloatStmt::Assign(0,FloatExpr::Parameter(0))],
                condition:Some(lt.clone()),
                increment:vec![FloatStmt::Assign(0,plus_one.clone())],
                body:vec![FloatStmt::IfBlock(FloatCondition {
                    op:FloatPredicate::Lt,
                    left:FloatExpr::Local(0),
                    right:FloatExpr::Constant64(0.0f64.to_bits()),
                },vec![FloatStmt::Continue],vec![
                    FloatStmt::Assign(1,FloatExpr::Add(
                        Box::new(FloatExpr::Local(1)),
                        Box::new(FloatExpr::Local(0)))),
                ])],
            },
            FloatStmt::Return(FloatExpr::Local(1)),
        ];
        assert!(emit_float_bodies(&[f.clone()]).is_ok());
        f.name="m14_do_phi".into();
        f.statements=vec![
            FloatStmt::DoWhile(vec![
                FloatStmt::Assign(0,FloatExpr::Parameter(0)),
                FloatStmt::Assign(1,FloatExpr::Parameter(1)),
            ],FloatCondition {
                op:FloatPredicate::Lt,
                left:FloatExpr::Local(0),
                right:FloatExpr::Local(1),
            }),
            FloatStmt::Return(FloatExpr::Local(0)),
        ];
        assert!(emit_float_bodies(&[f.clone()]).is_ok());
        // A while/for may execute zero times: body-only definitions are
        // *not* live after the normal false edge.
        f.statements=vec![
            FloatStmt::While(FloatCondition {
                op:FloatPredicate::Lt,left:FloatExpr::Parameter(0),
                right:FloatExpr::Parameter(1),
            },vec![FloatStmt::Assign(0,FloatExpr::Parameter(0))]),
            FloatStmt::Return(FloatExpr::Local(0)),
        ];
        assert!(emit_float_bodies(&[f.clone()]).is_err());
        // A loop header is evaluated before its first body execution.
        f.statements=vec![
            FloatStmt::While(lt.clone(),vec![
                FloatStmt::Assign(0,FloatExpr::Parameter(0)),
            ]),
            FloatStmt::Return(FloatExpr::Parameter(0)),
        ];
        assert!(emit_float_bodies(&[f.clone()]).is_err());
        // Continue routes directly to the for increment, bypassing v's
        // assignment on some paths. The increment must fail before codegen.
        f.statements=vec![
            FloatStmt::For {
                init:vec![FloatStmt::Assign(0,FloatExpr::Parameter(0))],
                condition:Some(lt.clone()),
                increment:vec![FloatStmt::Assign(0,FloatExpr::Local(1))],
                body:vec![FloatStmt::IfBlock(lt,
                    vec![FloatStmt::Continue],
                    vec![FloatStmt::Assign(1,plus_one.clone())])],
            },
            FloatStmt::Return(FloatExpr::Local(0)),
        ];
        assert!(emit_float_bodies(&[f.clone()]).is_err());
        // Even do-while's first test is unsafe if a branch can skip the
        // assignment, including by continue.
        f.statements=vec![
            FloatStmt::DoWhile(vec![FloatStmt::IfBlock(FloatCondition {
                op:FloatPredicate::Lt,left:FloatExpr::Parameter(0),
                right:FloatExpr::Parameter(1),
            },vec![FloatStmt::Continue],
              vec![FloatStmt::Assign(0,FloatExpr::Parameter(0))])],
            FloatCondition {
                op:FloatPredicate::Lt,left:FloatExpr::Local(0),
                right:FloatExpr::Parameter(1),
            }),
            FloatStmt::Return(FloatExpr::Parameter(0)),
        ];
        assert!(emit_float_bodies(&[f]).is_err());
    }

    #[test]
    fn fp_loop_break_edges_and_nested_returns() {
        let cond=FloatCondition {
            op:FloatPredicate::Gt,
            left:FloatExpr::Local(0),right:FloatExpr::Parameter(0),
        };
        let mut f=FloatBodyFunction {
            name:"m14_loop_break".into(),
            parameters:vec![FloatType::F64],
            result:FloatType::F64,
            locals:vec![FloatType::F64,FloatType::F64],
            statements:vec![
                FloatStmt::Assign(0,FloatExpr::Parameter(0)),
                FloatStmt::For {init:vec![],condition:None,increment:vec![],
                    body:vec![
                        FloatStmt::IfBlock(cond.clone(),
                            vec![FloatStmt::Break],vec![]),
                        FloatStmt::Assign(0,FloatExpr::Add(
                            Box::new(FloatExpr::Local(0)),
                            Box::new(FloatExpr::Constant64(1.0f64.to_bits())))),
                    ]},
                FloatStmt::Return(FloatExpr::Local(0)),
            ],
        };
        assert!(emit_float_bodies(&[f.clone()]).is_ok());
        // A break from the inner loop targets only its own exit.
        f.statements=vec![
            FloatStmt::Assign(0,FloatExpr::Parameter(0)),
            FloatStmt::While(cond.clone(),vec![
                FloatStmt::For {init:vec![],condition:None,increment:vec![],
                    body:vec![FloatStmt::Break]},
                FloatStmt::Return(FloatExpr::Local(0)),
            ]),
            FloatStmt::Return(FloatExpr::Local(0)),
        ];
        assert!(emit_float_bodies(&[f.clone()]).is_ok());
        // A break route must not promote a one-sided local assignment.
        f.statements=vec![
            FloatStmt::For {init:vec![],condition:None,increment:vec![],
                body:vec![FloatStmt::IfBlock(FloatCondition {
                    op:FloatPredicate::Gt,
                    left:FloatExpr::Parameter(0),
                    right:FloatExpr::Constant64(0.0f64.to_bits()),
                },vec![FloatStmt::Break],vec![
                    FloatStmt::Assign(1,FloatExpr::Parameter(0)),
                    FloatStmt::Break,
                ])]},
            FloatStmt::Return(FloatExpr::Local(1)),
        ];
        assert!(emit_float_bodies(&[f]).is_err());
    }

    #[test]
    fn fp_nested_joins_preserve_live_ins_and_reject_partial_initialization() {
        let outer=FloatCondition {op:FloatPredicate::Gt,
            left:FloatExpr::Parameter(0),right:FloatExpr::Parameter(1)};
        let inner=FloatCondition {op:FloatPredicate::Lt,
            left:FloatExpr::Parameter(0),
            right:FloatExpr::Constant64(0.0f64.to_bits())};
        let mut f=FloatBodyFunction {
            name:"m14_nested_unit".into(),
            parameters:vec![FloatType::F64,FloatType::F64],
            result:FloatType::F64,locals:vec![FloatType::F64],
            statements:vec![
                FloatStmt::Assign(0,FloatExpr::Parameter(1)),
                FloatStmt::IfBlock(outer.clone(),
                    vec![FloatStmt::IfAssign(inner.clone(),
                        vec![(0,FloatExpr::Parameter(0))],vec![])],
                    vec![(FloatStmt::Assign(0,FloatExpr::Parameter(1)))]),
                FloatStmt::Return(FloatExpr::Local(0)),
            ],
        };
        assert!(emit_float_bodies(&[f.clone()]).is_ok());
        // A nested branch may not initialize a previously undefined value
        // on only one of its predecessors, even if the outer else does.
        f.statements.remove(0);
        assert!(emit_float_bodies(&[f.clone()]).is_err());
        // Inner conditions are checked on the active path, not with an
        // overoptimistic union of values from the outer branches.
        f.statements=vec![
            FloatStmt::IfBlock(outer,
                vec![FloatStmt::IfAssign(FloatCondition {
                    op:FloatPredicate::Gt,left:FloatExpr::Local(0),
                    right:FloatExpr::Parameter(0),
                },vec![(0,FloatExpr::Parameter(0))],vec![])],
                vec![FloatStmt::Assign(0,FloatExpr::Parameter(1))]),
            FloatStmt::Return(FloatExpr::Local(0)),
        ];
        assert!(emit_float_bodies(&[f]).is_err());
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
