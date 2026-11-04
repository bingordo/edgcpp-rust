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
            std::thread::current().name().unwrap_or("test").replace(' ','_')));
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
