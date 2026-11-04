//! M14: first checked SysV x86-64 scalar floating-point ABI slice.
//!
//! This independent reference path does not reinterpret floating values as
//! C integer machine pieces. F32/F64 use real Cranelift FP types, so SysV
//! assigns them to XMM registers (or overflow stack slots) in C ABI order.
//! The eventual full EDG adapter must share a typed scalar passing plan with
//! the integer CFG; do not weaken the older integer ABI checks in the meantime.
use super::*;
use cranelift_codegen::ir::{immediates::{Ieee32,Ieee64}, Value};

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
fn reject(message:impl Into<String>)->CodegenError {
    CodegenError::InvalidFunction(message.into())
}
fn expression_type(
    expr:&FloatExpr,params:&[FloatType],depth:usize,
)->Result<FloatType,CodegenError> {
    if depth>64 {return Err(reject("FP expression exceeds recursion limit"));}
    use FloatExpr::*;
    match expr {
        Parameter(n)=>params.get(*n).copied()
            .ok_or_else(||reject("FP parameter out of range")),
        Constant32(_)=>Ok(FloatType::F32),
        Constant64(_)=>Ok(FloatType::F64),
        Neg(e)=>expression_type(e,params,depth+1),
        Add(a,b)|Sub(a,b)|Mul(a,b)|Div(a,b)=>{
            let a=expression_type(a,params,depth+1)?;
            let b=expression_type(b,params,depth+1)?;
            if a!=b {return Err(reject("FP binary operation operands differ in type"));}
            Ok(a)
        }
        Convert(to,e)=>{
            let _=expression_type(e,params,depth+1)?;
            Ok(*to)
        }
    }
}
fn emit_expr(
    expr:&FloatExpr,
    fb:&mut FunctionBuilder<'_>,
    params:&[Value],
    params_ty:&[FloatType],
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
        Constant32(bits)=>(fb.ins().f32const(Ieee32::with_bits(*bits)),FloatType::F32),
        Constant64(bits)=>(fb.ins().f64const(Ieee64::with_bits(*bits)),FloatType::F64),
        Neg(e)=>{
            let (v,t)=emit_expr(e,fb,params,params_ty)?;
            (fb.ins().fneg(v),t)
        }
        Convert(to,e)=>{
            let (v,t)=emit_expr(e,fb,params,params_ty)?;
            let value=match (t,*to) {
                (FloatType::F32,FloatType::F64)=>fb.ins().fpromote(types::F64,v),
                (FloatType::F64,FloatType::F32)=>fb.ins().fdemote(types::F32,v),
                _=>v,
            };
            (value,*to)
        }
        Add(a,b)|Sub(a,b)|Mul(a,b)|Div(a,b)=>{
            let (x,t)=emit_expr(a,fb,params,params_ty)?;
            let (y,u)=emit_expr(b,fb,params,params_ty)?;
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
/// Produce a directly GCC-linkable x86-64 ELF. A complete source module can
/// also contain integer functions, but this initial path deliberately rejects
/// unsupported mixed machine-class functions rather than guessing register
/// assignments. F32/F64 function signatures use separate SSE registers.
pub fn emit_float_routines(functions:&[FloatFunction])
    ->Result<Vec<u8>,CodegenError> {
    if functions.is_empty() {return Err(reject("empty FP function set"));}
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
            f.parameters.len()>256 {
            return Err(reject("invalid FP function name or argument count"));
        }
        if expression_type(&f.expression,&f.parameters,0)?!=f.result {
            return Err(reject("FP source expression/return type mismatch"));
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
            let (value,ty)=emit_expr(&f.expression,&mut fb,&params,&f.parameters)?;
            if ty!=f.result {
                return Err(reject("unconverted FP return type"));
            }
            fb.ins().return_(&[value]);
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
fn expression_from_edg(
    il:&Il,graph:&crate::graph::Graph,id:crate::graph::ExpressionId,
    params:&[crate::SymbolId],depth:usize,
)->Result<FloatExpr,CodegenError> {
    if depth>64 {return Err(reject("native EDG FP expression too deep"));}
    let node=graph.expression(id).ok_or_else(||reject("missing EDG FP expression"))?;
    let ty=type_from_edg(il,node.data.type_id.ok_or_else(||
        reject("EDG FP expression has no type"))?)?;
    let kind=super::semantic(2,node.data.kind);
    if kind==2 && node.operands.is_empty() {
        let symbol=node.data.symbol_id
            .ok_or_else(||reject("FP variable missing native identity"))?;
        let i=params.iter().position(|p|*p==symbol)
            .ok_or_else(||reject("FP local/global rvalue requires memory lowering"))?;
        return Ok(FloatExpr::Parameter(i));
    }
    if kind!=3 {return Err(reject("unsupported FP source expression form"));}
    let op=node.data.operator.map(|v|super::semantic(3,v))
        .ok_or_else(||reject("missing native FP operation"))?;
    let arg=|i:usize|->Result<FloatExpr,CodegenError> {
        expression_from_edg(il,graph,*node.operands.get(i)
            .ok_or_else(||reject("missing native FP operand"))?,params,depth+1)
    };
    let out=if node.operands.len()==1 {
        match op {
            4=>FloatExpr::Neg(Box::new(arg(0)?)),
            // A native EDG cast/implicit conversion is the sole place
            // where f32->f64 and f64->f32 may change machine class.
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
    let checked=expression_type(&out,&params.iter().map(|sym|{
        let type_id=il.function_parameters.iter().find(|p|p.symbol==*sym)
            .map(|p|p.ty).ok_or_else(||reject("missing native FP formal"))?;
        type_from_edg(il,type_id)
    }).collect::<Result<Vec<_>,_>>()?,0)?;
    if checked!=ty {return Err(reject("EDG FP expression result type differs from operands"));}
    Ok(out)
}
fn return_expression(
    il:&Il,graph:&crate::graph::Graph,
    root:crate::graph::StatementId,params:&[crate::SymbolId],
)->Result<FloatExpr,CodegenError> {
    let node=graph.statement(root).ok_or_else(||reject("missing FP statement"))?;
    match super::semantic(1,node.data.kind) {
        1 if node.children.is_empty()=>{
            let expr=node.expression.ok_or_else(||reject("FP return has no value"))?;
            expression_from_edg(il,graph,expr,params,0)
        },
        2 if node.expression.is_none() && node.children.len()==1=>
            return_expression(il,graph,node.children[0],params),
        _=>Err(reject("FP reference slice needs a single return statement")),
    }
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
        if il.local_variables.iter().any(|v|v.routine==routine) {
            return Err(reject("FP locals require typed scalar CFG integration"));
        }
        let roots:Vec<_>=graph.roots(name).map(|(id,_)|id).collect();
        if roots.len()!=1 {return Err(reject("FP function must have one root"));}
        let params_syms=args.iter().map(|p|p.symbol).collect::<Vec<_>>();
        let expression=return_expression(il,&graph,roots[0],&params_syms)?;
        if expression_type(&expression,&params,0)?!=ret {
            return Err(reject("FP return value missing native conversion"));
        }
        functions.push(FloatFunction {
            name:name.to_string(),parameters:params,result:ret,expression,
        });
    }
    emit_float_routines(&functions).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn param(n:usize)->FloatExpr {FloatExpr::Parameter(n)}
    fn bin(a:usize,b:usize)->FloatExpr {
        FloatExpr::Add(Box::new(param(a)),Box::new(param(b)))
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
