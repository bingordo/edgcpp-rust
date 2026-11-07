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
use cranelift_codegen::ir::{GlobalValue,MemFlags};
type FpGlobalRefs=HashMap<crate::SymbolId,(GlobalValue,FloatType)>;
#[derive(Debug,Clone,PartialEq,Eq)]
struct FpScalarGlobal {
    symbol:crate::SymbolId,
    name:String,
    ty:FloatType,
    linkage:Linkage,
    initializer:Option<Vec<u8>>,
}
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
    /// An EDG-verified pointer to a prototyped nonvariadic C function.
    /// Its canonical native identity prevents casting ordinary data pointers.
    FunctionPointer(crate::TypeId),
    /// A source-verified unqualified pointer to standard C float/double.
    /// Its source pointee and native x86-64 layout are checked before use.
    DataPointer(FloatType),
}
impl ScalarType {
    fn machine(self)->cranelift_codegen::ir::Type {
        match self {
            Self::Float(t)=>t.machine(),
            Self::Int {bits:32,..}=>types::I32,
            Self::Int {bits:64,..}=>types::I64,
            Self::FunctionPointer(_)|Self::DataPointer(_)=>types::I64,
            _=>unreachable!("integer width was checked at source boundary"),
        }
    }
}
#[derive(Debug,Clone,PartialEq,Eq)]
struct ScalarFloatBody {
    name:String,
    parameters:Vec<ScalarType>,
    result:ScalarType,
    locals:Vec<ScalarType>,
    statements:Vec<FloatStmt>,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum PointerSource {
    Parameter(usize),
    Local(usize),
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum ScalarCallArg {
    Float(FloatExpr),
    /// A native EDG direct INTEGER formal passed to the same scalar class.
    IntegerParameter(usize),
    /// A literal whose exact C integer width, signedness and bits were
    /// checked against the native EDG expression and callee prototype.
    IntegerConstant { bits:u64, ty:ScalarType },
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum FloatExpr {
    Parameter(usize),
    /// Exact EDG integer formal converted to FP by a native cast node.
    /// Never reinterpret GPR argument bits as an XMM value.
    IntegerParameterCast {parameter:usize,to:FloatType},
    /// A native EDG integer constant converted numerically to float/double.
    /// Bits retain the full U64 range even though EDG's wire carrier is i64.
    IntegerConstantCast {bits:u64,source:ScalarType,to:FloatType},
    /// An EDG-verified direct call to a definition in this compilation unit.
    /// Indirect and external calls require a separate declaration contract.
    Call {
        callee:String,
        parameters:Vec<ScalarType>,
        return_type:FloatType,
        arguments:Vec<ScalarCallArg>,
    },
    /// A source-verified named C declaration with no definition in the
    /// current module. Emit a typed ELF undefined symbol for the linker.
    /// Public FP-only reference entry points must reject this variant.
    CallExternal {
        callee:String,
        parameters:Vec<ScalarType>,
        return_type:FloatType,
        arguments:Vec<ScalarCallArg>,
    },
    /// Direct use of a function-pointer *formal*, never a guessed memory
    /// address or an unchecked C cast. The native pointee prototype is
    /// validated during EDG lowering.
    CallIndirect {
        pointer:PointerSource,
        pointer_type:crate::TypeId,
        parameters:Vec<ScalarType>,
        return_type:FloatType,
        arguments:Vec<ScalarCallArg>,
    },
    Local(usize),
    /// A source-verified, ordinary external C F32/F64 global load. Memory
    /// effects are never folded into an SSA register or constant.
    ExternalGlobal {symbol:crate::SymbolId,ty:FloatType},
    /// Real ordinary memory access through a source-verified FP pointer formal.
    /// This is NOT an SSA alias or a guessed address.
    LoadPointerParameter {parameter:usize,ty:FloatType},
    /// A local scalar pointer copied only from another checked FP data pointer.
    /// Reads use real memory, and the local's pointer SSA value is defined
    /// only along reachable CFG paths.
    LoadPointerLocal {local:usize,ty:FloatType},
    Constant32(u32),
    Constant64(u64),
    Neg(Box<Self>),
    Add(Box<Self>,Box<Self>),
    Sub(Box<Self>,Box<Self>),
    Mul(Box<Self>,Box<Self>),
    Div(Box<Self>,Box<Self>),
    /// EDG-verified C conditional-value expression. Evaluate exactly one
    /// arm; a block-parameter SSA merge preserves signed zero and NaN bits.
    Conditional {
        condition:Box<FloatCondition>,
        yes:Box<Self>,
        no:Box<Self>,
    },
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
    /// Real source-verified ordinary F32/F64 data store (not SSA).
    StoreGlobal {symbol:crate::SymbolId,ty:FloatType,value:FloatExpr},
    /// A checked unqualified *p=FP-value store to memory, never an SSA write.
    StorePointerParameter {parameter:usize,ty:FloatType,value:FloatExpr},
    /// Ordinary source-verified write through a nonescaping pointer SSA local.
    StorePointerLocal {local:usize,ty:FloatType,value:FloatExpr},
    /// Copy a checked data pointer formal/local into an automatic pointer SSA
    /// local, never create a pointer from integers, addresses or casts.
    AssignDataPointer {local:usize,ty:FloatType,source:PointerSource},
    /// A nonescaping, native EDG-verified callable pointer value in a
    /// typed automatic local. Definite assignment is tracked across CFG.
    AssignPointer {
        local:usize,
        pointer_type:crate::TypeId,
        source:PointerSource,
    },
    Return(FloatExpr),
    /// EDG-verified conversion of an FP expression to a native C INTEGER
    /// return class. Out-of-range/NaN are undefined in C and not normalized.
    ReturnInteger(FloatExpr,ScalarType),
    /// A verified direct return of a typed INTEGER-valued function-pointer
    /// call. Unlike FP-to-int conversion this preserves the callee's native
    /// INTEGER return register and exact 32/64-bit width/signature.
    ReturnIndirectInteger {
        pointer:PointerSource,
        pointer_type:crate::TypeId,
        parameters:Vec<ScalarType>,
        return_type:ScalarType,
        arguments:Vec<ScalarCallArg>,
    },
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
/// Resolve exact pointer ABI identity. A local is a register-sized,
/// nonescaping C automatic scalar and cannot be used until initialized.
fn pointer_source_type(
    src:PointerSource,params:&[ScalarType],locals:&[ScalarType],
)->Result<crate::TypeId,CodegenError> {
    match src {
        PointerSource::Parameter(n)=>match params.get(n) {
            Some(ScalarType::FunctionPointer(ty))=>Ok(*ty),
            _=>Err(reject("call target is not a checked pointer formal")),
        },
        PointerSource::Local(n)=>match locals.get(n) {
            Some(ScalarType::FunctionPointer(ty))=>Ok(*ty),
            _=>Err(reject("call target is not a checked pointer local")),
        },
    }
}
fn data_pointer_source_type(
    src:PointerSource,params:&[ScalarType],locals:&[ScalarType],
)->Result<FloatType,CodegenError> {
    match src {
        PointerSource::Parameter(n)=>match params.get(n) {
            Some(ScalarType::DataPointer(t))=>Ok(*t),
            _=>Err(reject("FP data pointer is not an exact typed formal")),
        },
        PointerSource::Local(n)=>match locals.get(n) {
            Some(ScalarType::DataPointer(t))=>Ok(*t),
            _=>Err(reject("FP data pointer is not an exact typed automatic local")),
        },
    }
}
fn check_pointer_source_reads(
    src:PointerSource,defined:&[bool],
)->Result<(),CodegenError> {
    match src {
        PointerSource::Parameter(_)=>Ok(()),
        PointerSource::Local(n) if defined.get(n).copied()==Some(true)=>Ok(()),
        PointerSource::Local(_)=>Err(reject("read of uninitialized callable pointer local")),
    }
}
fn emit_data_pointer_source(
    src:PointerSource,fb:&mut FunctionBuilder<'_>,
    params:&[Value],params_ty:&[ScalarType],locals:&[Variable],
    locals_ty:&[ScalarType],required:FloatType,
)->Result<Value,CodegenError> {
    if data_pointer_source_type(src,params_ty,locals_ty)?!=required {
        return Err(reject("FP pointer SSA source has wrong native pointee"));
    }
    match src {
        PointerSource::Parameter(n)=>params.get(n).copied().ok_or_else(||
            reject("FP data pointer formal has no native ABI value")),
        PointerSource::Local(n)=>locals.get(n).map(|var|fb.use_var(*var))
            .ok_or_else(||reject("FP data pointer local has no SSA slot")),
    }
}
fn emit_pointer_source(
    src:PointerSource,fb:&mut FunctionBuilder<'_>,
    params:&[Value],params_ty:&[ScalarType],locals:&[Variable],
    locals_ty:&[ScalarType],required:crate::TypeId,
)->Result<Value,CodegenError> {
    if pointer_source_type(src,params_ty,locals_ty)?!=required {
        return Err(reject("call pointer source ABI identity mismatch"));
    }
    match src {
        PointerSource::Parameter(n)=>params.get(n).copied().ok_or_else(||
            reject("call pointer formal has no physical ABI value")),
        PointerSource::Local(n)=>locals.get(n).map(|var|fb.use_var(*var))
            .ok_or_else(||reject("call pointer local has no SSA variable")),
    }
}
/// Common source ABI plan shared by the FP Cranelift reference and the
/// integer CFG. All type identity/provenance checks occur before this step.
fn checked_scalar_passing_plan(
    parameters:&[ScalarType],result:ScalarType,
)->Result<crate::scalar_abi::PassingPlan,CodegenError> {
    use crate::scalar_abi::{ScalarKind as K,Signature};
    let convert=|ty:ScalarType|->Result<K,CodegenError> {
        match ty {
            ScalarType::Float(FloatType::F32)=>Ok(K::Float32),
            ScalarType::Float(FloatType::F64)=>Ok(K::Float64),
            ScalarType::Int {bits:32,signed:true}=>Ok(K::Signed32),
            ScalarType::Int {bits:32,signed:false}=>Ok(K::Unsigned32),
            ScalarType::Int {bits:64,signed:true}=>Ok(K::Signed64),
            ScalarType::Int {bits:64,signed:false}=>Ok(K::Unsigned64),
            ScalarType::FunctionPointer(_)=>Ok(K::CallablePointer),
            ScalarType::DataPointer(_)=>Ok(K::DataPointer),
            _=>Err(reject("scalar ABI requires source-verified native 32/64-bit types")),
        }
    };
    let parameters=parameters.iter().copied()
        .map(convert).collect::<Result<Vec<_>,_>>()?;
    Signature {parameters,result:convert(result)?,
        prototyped:true,variadic:false}.plan_sysv_x86_64()
        .map_err(|err|reject(format!("unsupported source-checked scalar ABI: {err:?}")))
}
/// Accept only exact representable bit images for source-verified C integer
/// literals. Reject forged/truncated 32-bit literals, narrow integers and bool.
fn checked_integer_literal(ty:ScalarType,bits:u64)->Result<(),CodegenError> {
    match ty {
        ScalarType::Int {bits:32,signed:true}
            if bits==(bits as i32 as i64 as u64)=>Ok(()),
        ScalarType::Int {bits:32,signed:false}
            if bits==(bits as u32 as u64)=>Ok(()),
        ScalarType::Int {bits:64,..}=>Ok(()),
        _=>Err(reject("integer literal width, sign or native bit image invalid")),
    }
}
fn emit_integer_literal(
    fb:&mut FunctionBuilder<'_>,ty:ScalarType,bits:u64,
)->Result<Value,CodegenError> {
    checked_integer_literal(ty,bits)?;
    let value=match ty {
        ScalarType::Int {bits:32,..}=>bits as u32 as i32 as i64,
        ScalarType::Int {bits:64,..}=>bits as i64,
        _=>return Err(reject("unsupported integer literal ABI class")),
    };
    Ok(fb.ins().iconst(ty.machine(),value))
}
fn check_scalar_call_args(
    arguments:&[ScalarCallArg],target_types:&[ScalarType],
    params:&[ScalarType],locals:&[ScalarType],depth:usize,
)->Result<(),CodegenError> {
    if depth>64 || arguments.len()!=target_types.len() ||
        arguments.len()>256 {
        return Err(reject("scalar call arity/depth invalid"));
    }
    for (arg,target) in arguments.iter().zip(target_types) {
        let source=match arg {
            ScalarCallArg::Float(e)=>
                ScalarType::Float(expression_type(e,params,locals,depth+1)?),
            ScalarCallArg::IntegerParameter(n)=>match params.get(*n) {
                Some(ScalarType::Int {..})=>params[*n],
                _=>return Err(reject("call INTEGER argument is not a native formal")),
            },
            ScalarCallArg::IntegerConstant {bits,ty}=>{
                checked_integer_literal(*ty,*bits)?;
                *ty
            },
        };
        if source!=*target {
            return Err(reject("scalar call argument type/class mismatch"));
        }
    }
    Ok(())
}
fn check_scalar_call_local_reads(
    arguments:&[ScalarCallArg],defined:&[bool],
)->Result<(),CodegenError> {
    for arg in arguments {
        if let ScalarCallArg::Float(expr)=arg {
            check_local_reads(expr,defined,0)?;
        }
    }
    Ok(())
}
fn expression_type(
    expr:&FloatExpr,params:&[ScalarType],locals:&[ScalarType],depth:usize,
)->Result<FloatType,CodegenError> {
    if depth>64 {return Err(reject("FP expression exceeds recursion limit"));}
    use FloatExpr::*;
    match expr {
        Parameter(n)=>match params.get(*n).copied() {
            Some(ScalarType::Float(t))=>Ok(t),
            _=>Err(reject("FP parameter missing or has integer ABI class")),
        },
        IntegerParameterCast {parameter,to}=>{
            if matches!(params.get(*parameter),Some(ScalarType::Int {bits:32|64,..})) {
                Ok(*to)
            } else {
                Err(reject("FP integer cast source is not a checked integer formal"))
            }
        },
        IntegerConstantCast {bits,source,to}=>{
            checked_integer_literal(*source,*bits)?;
            Ok(*to)
        },
        Call {parameters,return_type,arguments,..}|
        CallExternal {parameters,return_type,arguments,..}|
        CallIndirect {parameters,return_type,arguments,..}=>{
            if let CallIndirect {pointer,pointer_type,..}=expr {
                if pointer_source_type(*pointer,params,locals)?!=*pointer_type {
                    return Err(reject("FP indirect call requires exact native callable type"));
                }
            }
            checked_scalar_passing_plan(parameters,ScalarType::Float(*return_type))?;
            check_scalar_call_args(arguments,parameters,params,locals,depth+1)?;
            Ok(*return_type)
        }
        Local(n)=>match locals.get(*n).copied() {
            Some(ScalarType::Float(t))=>Ok(t),
            _=>Err(reject("FP local out of range or is a callable pointer")),
        },
        ExternalGlobal {ty,..}=>Ok(*ty),
        LoadPointerParameter {parameter,ty}=>{
            if params.get(*parameter)!=Some(&ScalarType::DataPointer(*ty)) {
                return Err(reject("FP load through nonmatching pointer formal"));
            }
            Ok(*ty)
        },
        LoadPointerLocal {local,ty}=>{
            if locals.get(*local)!=Some(&ScalarType::DataPointer(*ty)) {
                return Err(reject("FP load through nonmatching data-pointer local"));
            }
            Ok(*ty)
        },
        Constant32(_)=>Ok(FloatType::F32),
        Constant64(_)=>Ok(FloatType::F64),
        Neg(e)=>expression_type(e,params,locals,depth+1),
        Add(a,b)|Sub(a,b)|Mul(a,b)|Div(a,b)=>{
            let a=expression_type(a,params,locals,depth+1)?;
            let b=expression_type(b,params,locals,depth+1)?;
            if a!=b {return Err(reject("FP binary operation operands differ in type"));}
            Ok(a)
        }
        Conditional {condition,yes,no}=>{
            let left=expression_type(&condition.left,params,locals,depth+1)?;
            let right=expression_type(&condition.right,params,locals,depth+1)?;
            if left!=right {
                return Err(reject("FP conditional operands have mismatched types"));
            }
            let y=expression_type(yes,params,locals,depth+1)?;
            let n=expression_type(no,params,locals,depth+1)?;
            if y!=n {
                return Err(reject("C FP conditional value arms lack native conversion"));
            }
            Ok(y)
        }
        Convert(to,e)=>{
            let _=expression_type(e,params,locals,depth+1)?;
            Ok(*to)
        }
    }
}
/// Evaluate typed scalar arguments exactly once and in source order.
/// Both direct and indirect calls use this function and the same ABI classes.
#[allow(clippy::too_many_arguments)]
fn emit_scalar_call_arguments(
    arguments:&[ScalarCallArg],parameters:&[ScalarType],
    fb:&mut FunctionBuilder<'_>,params:&[Value],params_ty:&[ScalarType],
    locals:&[Variable],locals_ty:&[ScalarType],
    refs:&HashMap<String,FuncRef>,global_refs:&FpGlobalRefs,
)->Result<Vec<Value>,CodegenError> {
    if arguments.len()!=parameters.len() || arguments.len()>256 {
        return Err(reject("FP call argument arity invalid"));
    }
    let mut values=Vec::with_capacity(arguments.len());
    for (arg,expected) in arguments.iter().zip(parameters) {
        let value=match (arg,expected) {
            (ScalarCallArg::Float(e),ScalarType::Float(t))=>{
                let (v,actual)=emit_expr(e,fb,params,params_ty,locals,locals_ty,refs,global_refs)?;
                if actual!=*t {return Err(reject("FP call argument needs conversion"));}
                v
            }
            (ScalarCallArg::IntegerParameter(n),ScalarType::Int {..})=>{
                if params_ty.get(*n)!=Some(expected) {
                    return Err(reject("FP call INTEGER formal ABI mismatch"));
                }
                *params.get(*n).ok_or_else(||reject("FP call missing INTEGER formal"))?
            }
            (ScalarCallArg::IntegerConstant {bits,ty},ScalarType::Int {..})=>{
                if ty!=expected {
                    return Err(reject("FP call INTEGER literal source type mismatch"));
                }
                emit_integer_literal(fb,*ty,*bits)?
            }
            _=>return Err(reject("FP call crosses incompatible argument classes")),
        };
        values.push(value);
    }
    Ok(values)
}
fn emit_expr(
    expr:&FloatExpr,
    fb:&mut FunctionBuilder<'_>,
    params:&[Value],
    params_ty:&[ScalarType],
    locals:&[Variable],
    locals_ty:&[ScalarType],
    refs:&HashMap<String,FuncRef>,global_refs:&FpGlobalRefs,
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
        IntegerConstantCast {bits,source,to}=>{
            let value=emit_integer_literal(fb,*source,*bits)?;
            let signed=match source {
                ScalarType::Int {signed,..}=>*signed,
                _=>return Err(reject("invalid integer literal conversion source")),
            };
            let converted=if signed {
                fb.ins().fcvt_from_sint(to.machine(),value)
            } else {
                fb.ins().fcvt_from_uint(to.machine(),value)
            };
            (converted,*to)
        }
        Call {callee,parameters,return_type,arguments}|
        CallExternal {callee,parameters,return_type,arguments}=>{
            if arguments.len()!=parameters.len() {
                return Err(reject("invalid direct call argument count"));
            }
            let reference=*refs.get(callee)
                .ok_or_else(||reject("FP call target has no defined function ref"))?;
            let values=emit_scalar_call_arguments(arguments,parameters,fb,
                params,params_ty,locals,locals_ty,refs,global_refs)?;
            let inst=fb.ins().call(reference,&values);
            let result=*fb.inst_results(inst).first()
                .ok_or_else(||reject("FP direct call has no return value"))?;
            (result,*return_type)
        }
        CallIndirect {pointer,pointer_type,parameters,
                      return_type,arguments}=>{
            let pointer=emit_pointer_source(*pointer,fb,
                params,params_ty,locals,locals_ty,*pointer_type)?;
            // The signature is typed: every INTEGER/SSE piece uses its
            // source class. The SysV call convention chooses the registers.
            let mut signature=cranelift_codegen::ir::Signature::new(
                fb.func.signature.call_conv);
            for ty in parameters {
                if matches!(ty,ScalarType::FunctionPointer(_)) {
                    return Err(reject("passing function pointers to FP callees unavailable"));
                }
                signature.params.push(AbiParam::new(ty.machine()));
            }
            signature.returns.push(AbiParam::new(return_type.machine()));
            let values=emit_scalar_call_arguments(arguments,parameters,fb,
                params,params_ty,locals,locals_ty,refs,global_refs)?;
            let reference=fb.import_signature(signature);
            let call=fb.ins().call_indirect(reference,pointer,&values);
            let result=*fb.inst_results(call).first()
                .ok_or_else(||reject("indirect FP call has no result"))?;
            (result,*return_type)
        }
        Local(n)=>{
            let var=*locals.get(*n).ok_or_else(||reject("missing FP local"))?;
            let ty=match locals_ty.get(*n) {
                Some(ScalarType::Float(t))=>*t,
                _=>return Err(reject("FP local is not a typed FP value")),
            };
            (fb.use_var(var),ty)
        }
        LoadPointerParameter {parameter,ty}=>{
            if params_ty.get(*parameter)!=Some(&ScalarType::DataPointer(*ty)) {
                return Err(reject("FP pointer memory load lacks verified formal"));
            }
            let ptr=*params.get(*parameter).ok_or_else(||
                reject("FP pointer load parameter has no ABI value"))?;
            (fb.ins().load(ty.machine(),MemFlags::new(),ptr,0),*ty)
        }
        LoadPointerLocal {local,ty}=>{
            let ptr=emit_data_pointer_source(PointerSource::Local(*local),fb,
                params,params_ty,locals,locals_ty,*ty)?;
            (fb.ins().load(ty.machine(),MemFlags::new(),ptr,0),*ty)
        }
        ExternalGlobal {symbol,ty}=>{
            let (data,actual)=*global_refs.get(symbol).ok_or_else(||
                reject("FP external global has no verified data import"))?;
            if actual!=*ty {
                return Err(reject("FP external global typed load disagrees with source import"));
            }
            // Always emit a real ordinary memory load. Even nonvolatile
            // extern objects may change between calls and through other code.
            let address=fb.ins().global_value(types::I64,data);
            (fb.ins().load(ty.machine(),MemFlags::new(),address,0),*ty)
        }
        Constant32(bits)=>(fb.ins().f32const(Ieee32::with_bits(*bits)),FloatType::F32),
        Constant64(bits)=>(fb.ins().f64const(Ieee64::with_bits(*bits)),FloatType::F64),
        Neg(e)=>{
            let (v,t)=emit_expr(e,fb,params,params_ty,locals,locals_ty,refs,global_refs)?;
            (fb.ins().fneg(v),t)
        }
        Convert(to,e)=>{
            let (v,t)=emit_expr(e,fb,params,params_ty,locals,locals_ty,refs,global_refs)?;
            let value=match (t,*to) {
                (FloatType::F32,FloatType::F64)=>fb.ins().fpromote(types::F64,v),
                (FloatType::F64,FloatType::F32)=>fb.ins().fdemote(types::F32,v),
                _=>v,
            };
            (value,*to)
        }
        Conditional {condition,yes,no}=>{
            // Do not emit select: only the chosen arm may be evaluated, as
            // either arm can contain an external call or memory load.
            let test=emit_condition(condition,fb,params,params_ty,locals,
                locals_ty,refs,global_refs)?;
            let then_block=fb.create_block();
            let else_block=fb.create_block();
            let merged=fb.create_block();
            let expected=expression_type(yes,params_ty,locals_ty,0)?;
            fb.append_block_param(merged,expected.machine());
            fb.ins().brif(test,then_block,&[],else_block,&[]);
            fb.seal_block(then_block);
            fb.seal_block(else_block);
            fb.switch_to_block(then_block);
            let (selected,actual)=emit_expr(yes,fb,params,params_ty,
                locals,locals_ty,refs,global_refs)?;
            if actual!=expected {return Err(reject("FP conditional then arm type changed"));}
            fb.ins().jump(merged,&[selected.into()]);
            fb.switch_to_block(else_block);
            let (selected,actual)=emit_expr(no,fb,params,params_ty,
                locals,locals_ty,refs,global_refs)?;
            if actual!=expected {return Err(reject("FP conditional else arm type changed"));}
            fb.ins().jump(merged,&[selected.into()]);
            fb.seal_block(merged);
            fb.switch_to_block(merged);
            (fb.block_params(merged)[0],expected)
        }
        Add(a,b)|Sub(a,b)|Mul(a,b)|Div(a,b)=>{
            let (x,t)=emit_expr(a,fb,params,params_ty,locals,locals_ty,refs,global_refs)?;
            let (y,u)=emit_expr(b,fb,params,params_ty,locals,locals_ty,refs,global_refs)?;
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
        LoadPointerLocal {local,..} if !defined.get(*local).copied().unwrap_or(false)=>
            Err(reject("FP load through uninitialized data pointer local")),
        Local(_)|Parameter(_)|IntegerParameterCast {..}|
        IntegerConstantCast {..}|
        ExternalGlobal {..}|LoadPointerParameter {..}|LoadPointerLocal {..}|
        Constant32(_)|Constant64(_)=>Ok(()),
        Call {arguments,..}|CallExternal {arguments,..}=>{
            for arg in arguments {
                if let ScalarCallArg::Float(expr)=arg {
                    check_local_reads(expr,defined,depth+1)?;
                }
            }
            Ok(())
        }
        CallIndirect {pointer,arguments,..}=>{
            check_pointer_source_reads(*pointer,defined)?;
            for arg in arguments {
                if let ScalarCallArg::Float(expr)=arg {
                    check_local_reads(expr,defined,depth+1)?;
                }
            }
            Ok(())
        },
        Conditional {condition,yes,no}=>{
            check_local_reads(&condition.left,defined,depth+1)?;
            check_local_reads(&condition.right,defined,depth+1)?;
            check_local_reads(yes,defined,depth+1)?;
            check_local_reads(no,defined,depth+1)
        }
        Neg(e)|Convert(_,e)=>check_local_reads(e,defined,depth+1),
        Add(a,b)|Sub(a,b)|Mul(a,b)|Div(a,b)=>{
            check_local_reads(a,defined,depth+1)?;
            check_local_reads(b,defined,depth+1)
        }
    }
}
fn check_condition(
    cond:&FloatCondition,params:&[ScalarType],locals:&[ScalarType],
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
    params_ty:&[ScalarType],locals:&[Variable],locals_ty:&[ScalarType],
    refs:&HashMap<String,FuncRef>,global_refs:&FpGlobalRefs,
)->Result<Value,CodegenError> {
    let (x,t)=emit_expr(&cond.left,fb,params,params_ty,locals,locals_ty,refs,global_refs)?;
    let (y,u)=emit_expr(&cond.right,fb,params,params_ty,locals,locals_ty,refs,global_refs)?;
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
        if ScalarType::Float(expression_type(expr,&f.parameters,&f.locals,0)?)!=ty {
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
            FloatStmt::AssignDataPointer {local,ty,source}=>{
                if f.locals.get(*local)!=Some(&ScalarType::DataPointer(*ty)) ||
                    data_pointer_source_type(*source,&f.parameters,&f.locals)?!=*ty {
                    return Err(reject("FP data pointer SSA assignment pointee mismatch"));
                }
                check_pointer_source_reads(*source,&defined)?;
                defined[*local]=true;
                next=Some(defined);
            }
            FloatStmt::StorePointerLocal {local,ty,value}=>{
                if f.locals.get(*local)!=Some(&ScalarType::DataPointer(*ty)) ||
                    !defined.get(*local).copied().unwrap_or(false) {
                    return Err(reject("FP store through uninitialized/wrong data pointer local"));
                }
                check_local_reads(value,&defined,0)?;
                if expression_type(value,&f.parameters,&f.locals,0)?!=*ty {
                    return Err(reject("FP local pointer store RHS source type mismatch"));
                }
                next=Some(defined);
            }
            FloatStmt::AssignPointer {local,pointer_type,source}=>{
                let actual=f.locals.get(*local).ok_or_else(||
                    reject("pointer assignment local out of range"))?;
                if *actual!=ScalarType::FunctionPointer(*pointer_type) ||
                    pointer_source_type(*source,&f.parameters,&f.locals)?!=*pointer_type {
                    return Err(reject("pointer local assignment has incorrect native ABI type"));
                }
                check_pointer_source_reads(*source,&defined)?;
                defined[*local]=true;
                next=Some(defined);
            }
            FloatStmt::StorePointerParameter {parameter,ty,value}=>{
                if f.parameters.get(*parameter)!=Some(&ScalarType::DataPointer(*ty)) {
                    return Err(reject("FP memory store pointer formal type mismatch"));
                }
                check_local_reads(value,&defined,0)?;
                if expression_type(value,&f.parameters,&f.locals,0)?!=*ty {
                    return Err(reject("FP pointer store needs exact FP source type"));
                }
                next=Some(defined);
            }
            FloatStmt::StoreGlobal {ty,value,..}=>{
                check_local_reads(value,&defined,0)?;
                if expression_type(value,&f.parameters,&f.locals,0)?!=*ty {
                    return Err(reject("FP global store RHS native type mismatch"));
                }
                next=Some(defined);
            }
            FloatStmt::Assign(n,expr)=>{
                let ty=*f.locals.get(*n)
                    .ok_or_else(||reject("FP local index out of range"))?;
                check_local_reads(expr,&defined,0)?;
                if ScalarType::Float(expression_type(expr,&f.parameters,&f.locals,0)?)!=ty {
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
            FloatStmt::ReturnIndirectInteger {
                pointer,pointer_type,parameters,return_type,arguments,
            }=>{
                if !matches!(return_type,ScalarType::Int {bits:32|64,..}) ||
                   *return_type!=f.result ||
                   pointer_source_type(*pointer,&f.parameters,&f.locals)?!=*pointer_type ||
                   parameters.iter().any(|t|matches!(t,ScalarType::FunctionPointer(_))) {
                    return Err(reject("indirect INTEGER return ABI/formal mismatch"));
                }
                checked_scalar_passing_plan(parameters,*return_type)?;
                check_scalar_call_args(arguments,parameters,
                    &f.parameters,&f.locals,0)?;
                check_scalar_call_local_reads(arguments,&defined)?;
                check_pointer_source_reads(*pointer,&defined)?;
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
    params_ty:&[ScalarType],locals:&[Variable],locals_ty:&[ScalarType],
    outer:Option<FpLoopTargets<'_>>,
    refs:&HashMap<String,FuncRef>,global_refs:&FpGlobalRefs,depth:usize,
)->Result<bool,CodegenError> {
    if depth>64 {return Err(reject("FP CFG emission exceeds recursion limit"));}
    let mut live=true;
    for stmt in stmts {
        if !live {break;}
        match stmt {
            FloatStmt::AssignDataPointer {local,ty,source}=>{
                let value=emit_data_pointer_source(*source,fb,
                    params,params_ty,locals,locals_ty,*ty)?;
                let var=*locals.get(*local).ok_or_else(||
                    reject("FP data pointer local has no SSA slot"))?;
                fb.def_var(var,value);
            }
            FloatStmt::StorePointerLocal {local,ty,value}=>{
                let pointer=emit_data_pointer_source(PointerSource::Local(*local),fb,
                    params,params_ty,locals,locals_ty,*ty)?;
                let (v,actual)=emit_expr(value,fb,params,params_ty,locals,locals_ty,
                    refs,global_refs)?;
                if actual!=*ty {return Err(reject("FP data pointer store needs conversion"));}
                fb.ins().store(MemFlags::new(),v,pointer,0);
            }
            FloatStmt::AssignPointer {local,pointer_type,source}=>{
                let value=emit_pointer_source(*source,fb,
                    params,params_ty,locals,locals_ty,*pointer_type)?;
                let var=*locals.get(*local).ok_or_else(||
                    reject("pointer local has no SSA slot"))?;
                fb.def_var(var,value);
            }
            FloatStmt::StorePointerParameter {parameter,ty,value}=>{
                if params_ty.get(*parameter)!=Some(&ScalarType::DataPointer(*ty)) {
                    return Err(reject("FP pointer store uses unchecked formal"));
                }
                let pointer=*params.get(*parameter).ok_or_else(||
                    reject("FP pointer store has no native formal value"))?;
                let (v,source)=emit_expr(value,fb,params,params_ty,locals,locals_ty,
                    refs,global_refs)?;
                if source!=*ty {return Err(reject("FP memory store needs explicit conversion"));}
                fb.ins().store(MemFlags::new(),v,pointer,0);
            }
            FloatStmt::StoreGlobal {symbol,ty,value}=>{
                let (data,actual)=*global_refs.get(symbol).ok_or_else(||
                    reject("FP global store lacks verified data symbol"))?;
                if *ty!=actual {return Err(reject("FP global store native type mismatch"));}
                let (v,source)=emit_expr(value,fb,params,params_ty,locals,locals_ty,
                    refs,global_refs)?;
                if source!=*ty {return Err(reject("FP global store RHS type differs"));}
                let address=fb.ins().global_value(types::I64,data);
                fb.ins().store(MemFlags::new(),v,address,0);
            }
            FloatStmt::Assign(n,expr)=>{
                let (v,_)=emit_expr(expr,fb,params,params_ty,locals,locals_ty,refs,global_refs)?;
                fb.def_var(locals[*n],v);
            }
            FloatStmt::Return(expr)=>{
                let (v,_)=emit_expr(expr,fb,params,params_ty,locals,locals_ty,refs,global_refs)?;
                fb.ins().return_(&[v]);
                live=false;
            }
            FloatStmt::ReturnInteger(expr,to)=>{
                let (v,_)=emit_expr(expr,fb,params,params_ty,locals,locals_ty,refs,global_refs)?;
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
            FloatStmt::ReturnIndirectInteger {
                pointer,pointer_type,parameters,return_type,arguments,
            }=>{
                let pointer=emit_pointer_source(*pointer,fb,
                    params,params_ty,locals,locals_ty,*pointer_type)?;
                let mut signature=cranelift_codegen::ir::Signature::new(
                    fb.func.signature.call_conv);
                for ty in parameters {
                    if matches!(ty,ScalarType::FunctionPointer(_)|ScalarType::DataPointer(_)) {
                        return Err(reject("nested pointer argument unsupported"));
                    }
                    signature.params.push(AbiParam::new(ty.machine()));
                }
                signature.returns.push(AbiParam::new(return_type.machine()));
                let values=emit_scalar_call_arguments(arguments,parameters,fb,
                    params,params_ty,locals,locals_ty,refs,global_refs)?;
                let reference=fb.import_signature(signature);
                let call=fb.ins().call_indirect(reference,pointer,&values);
                let returned=*fb.inst_results(call).first().ok_or_else(||
                    reject("INTEGER indirect call has no return register"))?;
                fb.ins().return_(&[returned]);
                live=false;
            }
            FloatStmt::IfReturn(cond,yes,no)=>{
                let flag=emit_condition(cond,fb,params,params_ty,locals,locals_ty,refs,global_refs)?;
                let t=fb.create_block();
                let f=fb.create_block();
                fb.ins().brif(flag,t,&[],f,&[]);
                fb.seal_block(t);
                fb.seal_block(f);
                fb.switch_to_block(t);
                let (v,_)=emit_expr(yes,fb,params,params_ty,locals,locals_ty,refs,global_refs)?;
                fb.ins().return_(&[v]);
                fb.switch_to_block(f);
                let (v,_)=emit_expr(no,fb,params,params_ty,locals,locals_ty,refs,global_refs)?;
                fb.ins().return_(&[v]);
                live=false;
            }
            FloatStmt::IfBlock(cond,yes,no)=>{
                live=emit_fp_if(cond,yes,no,fb,params,params_ty,
                    locals,locals_ty,outer,refs,global_refs,depth+1)?;
            }
            FloatStmt::IfAssign(cond,yes,no)=>{
                let a:Vec<_>=yes.iter().map(|(n,e)|
                    FloatStmt::Assign(*n,e.clone())).collect();
                let b:Vec<_>=no.iter().map(|(n,e)|
                    FloatStmt::Assign(*n,e.clone())).collect();
                live=emit_fp_if(cond,&a,&b,fb,params,params_ty,
                    locals,locals_ty,outer,refs,global_refs,depth+1)?;
            }
            FloatStmt::While(cond,body)=>{
                let head=fb.create_block();
                let body_block=fb.create_block();
                let exit=fb.create_block();
                fb.ins().jump(head,&[]);
                fb.switch_to_block(head);
                let test=emit_condition(cond,fb,params,params_ty,locals,locals_ty,refs,global_refs)?;
                fb.ins().brif(test,body_block,&[],exit,&[]);
                fb.seal_block(body_block);
                fb.switch_to_block(body_block);
                let saw_break=Cell::new(false);
                let saw_continue=Cell::new(false);
                let inner=FpLoopTargets {break_to:exit,continue_to:head,
                    saw_break:&saw_break,saw_continue:&saw_continue};
                if emit_fp_statements(body,fb,params,params_ty,locals,
                    locals_ty,Some(inner),refs,global_refs,depth+1)? {
                    fb.ins().jump(head,&[]);
                }
                fb.seal_block(head);
                fb.seal_block(exit);
                fb.switch_to_block(exit);
            }
            FloatStmt::For {init,condition,increment,body}=>{
                if !emit_fp_statements(init,fb,params,params_ty,locals,
                    locals_ty,outer,refs,global_refs,depth+1)? {live=false;continue;}
                let head=fb.create_block();
                let body_block=fb.create_block();
                let step=fb.create_block();
                let exit=fb.create_block();
                fb.ins().jump(head,&[]);
                fb.switch_to_block(head);
                if let Some(cond)=condition {
                    let test=emit_condition(cond,fb,params,params_ty,locals,locals_ty,refs,global_refs)?;
                    fb.ins().brif(test,body_block,&[],exit,&[]);
                } else {fb.ins().jump(body_block,&[]);}
                fb.seal_block(body_block);
                fb.switch_to_block(body_block);
                let saw_break=Cell::new(false);
                let saw_continue=Cell::new(false);
                let inner=FpLoopTargets {break_to:exit,continue_to:step,
                    saw_break:&saw_break,saw_continue:&saw_continue};
                let body_live=emit_fp_statements(body,fb,params,params_ty,
                    locals,locals_ty,Some(inner),refs,global_refs,depth+1)?;
                if body_live {fb.ins().jump(step,&[]);}
                fb.seal_block(step);
                if body_live||saw_continue.get() {
                    fb.switch_to_block(step);
                    if emit_fp_statements(increment,fb,params,params_ty,
                        locals,locals_ty,Some(inner),refs,global_refs,depth+1)? {
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
                    locals,locals_ty,Some(inner),refs,global_refs,depth+1)?;
                if body_live {fb.ins().jump(test_block,&[]);}
                fb.seal_block(test_block);
                if body_live||saw_continue.get() {
                    fb.switch_to_block(test_block);
                    let test=emit_condition(cond,fb,params,params_ty,locals,locals_ty,refs,global_refs)?;
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
    locals:&[Variable],locals_ty:&[ScalarType],
    outer:Option<FpLoopTargets<'_>>,
    refs:&HashMap<String,FuncRef>,global_refs:&FpGlobalRefs,depth:usize,
)->Result<bool,CodegenError> {
    if depth>64 {return Err(reject("FP nested if exceeds recursion limit"));}
    let test=emit_condition(cond,fb,params,params_ty,locals,locals_ty,refs,global_refs)?;
    let t=fb.create_block();
    let f=fb.create_block();
    fb.ins().brif(test,t,&[],f,&[]);
    fb.seal_block(t);
    fb.seal_block(f);
    fb.switch_to_block(t);
    let t_live=emit_fp_statements(yes,fb,params,params_ty,
        locals,locals_ty,outer,refs,global_refs,depth+1)?;
    let join=if t_live {
        let b=fb.create_block();
        fb.ins().jump(b,&[]);
        Some(b)
    } else {None};
    fb.switch_to_block(f);
    let f_live=emit_fp_statements(no,fb,params,params_ty,
        locals,locals_ty,outer,refs,global_refs,depth+1)?;
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
        locals:f.locals.iter().copied().map(ScalarType::Float).collect(),
        statements:f.statements.clone(),
    }).collect();
    emit_scalar_float_bodies(&scalar)
}

/// Prevent compiler builtin and non-C symbol names from entering the
/// ordinary imported-function ABI path without verified special semantics.
fn safe_scalar_external_name(name:&str)->bool {
    let b=name.as_bytes();
    !b.is_empty() && b.len()<=256 && !name.starts_with("__") &&
    (b[0].is_ascii_alphabetic()||b[0]==b'_') &&
    b[1..].iter().all(|c|c.is_ascii_alphanumeric()||*c==b'_')
}
/// Prevalidate all source-authorized external prototypes before ELF emission.
fn validate_fp_calls_expr(
    expr:&FloatExpr,
    declarations:&HashMap<String,(Vec<ScalarType>,ScalarType)>,
    imports:&mut HashMap<String,(Vec<ScalarType>,ScalarType)>,
    source_imports:bool,
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
                    validate_fp_calls_expr(arg,declarations,imports,source_imports,depth+1)?;
                }
            }
        }
        FloatExpr::CallExternal {callee,parameters,return_type,arguments}=>{
            if !source_imports || !safe_scalar_external_name(callee) ||
                declarations.contains_key(callee) ||
                arguments.len()!=parameters.len() || parameters.len()>256 ||
                parameters.iter().any(|ty|matches!(ty,ScalarType::FunctionPointer(_))) {
                return Err(reject("FP external call lacks an authorized C prototype"));
            }
            let sig=(parameters.clone(),ScalarType::Float(*return_type));
            if let Some(previous)=imports.get(callee) {
                if previous!=&sig {
                    return Err(reject("conflicting native FP external-call prototypes"));
                }
            } else {imports.insert(callee.clone(),sig);}
            for arg in arguments {
                if let ScalarCallArg::Float(e)=arg {
                    validate_fp_calls_expr(e,declarations,imports,source_imports,depth+1)?;
                }
            }
        }
        FloatExpr::CallIndirect {arguments,..}=>{
            for arg in arguments {
                if let ScalarCallArg::Float(arg)=arg {
                    validate_fp_calls_expr(arg,declarations,imports,source_imports,depth+1)?;
                }
            }
        }
        FloatExpr::Conditional {condition,yes,no}=>{
            validate_fp_calls_cond(condition,declarations,imports,source_imports,depth+1)?;
            validate_fp_calls_expr(yes,declarations,imports,source_imports,depth+1)?;
            validate_fp_calls_expr(no,declarations,imports,source_imports,depth+1)?;
        }
        FloatExpr::Neg(e)|FloatExpr::Convert(_,e)=>
            validate_fp_calls_expr(e,declarations,imports,source_imports,depth+1)?,
        FloatExpr::Add(a,b)|FloatExpr::Sub(a,b)|
        FloatExpr::Mul(a,b)|FloatExpr::Div(a,b)=>{
            validate_fp_calls_expr(a,declarations,imports,source_imports,depth+1)?;
            validate_fp_calls_expr(b,declarations,imports,source_imports,depth+1)?;
        }
        FloatExpr::Parameter(_)|FloatExpr::IntegerParameterCast {..}|
        FloatExpr::IntegerConstantCast {..}|
        FloatExpr::Local(_)|FloatExpr::ExternalGlobal {..}|
        FloatExpr::LoadPointerParameter {..}|FloatExpr::LoadPointerLocal {..}|
        FloatExpr::Constant32(_)|FloatExpr::Constant64(_)=>{},
    }
    Ok(())
}
fn validate_fp_calls_cond(
    cond:&FloatCondition,
    declarations:&HashMap<String,(Vec<ScalarType>,ScalarType)>,
    imports:&mut HashMap<String,(Vec<ScalarType>,ScalarType)>,
    source_imports:bool,
    depth:usize,
)->Result<(),CodegenError> {
    validate_fp_calls_expr(&cond.left,declarations,imports,source_imports,depth)?;
    validate_fp_calls_expr(&cond.right,declarations,imports,source_imports,depth)
}
fn validate_fp_calls_stmts(
    stmts:&[FloatStmt],
    declarations:&HashMap<String,(Vec<ScalarType>,ScalarType)>,
    imports:&mut HashMap<String,(Vec<ScalarType>,ScalarType)>,
    source_imports:bool,
    depth:usize,
)->Result<(),CodegenError> {
    if depth>64 {return Err(reject("FP call statements exceed depth limit"));}
    for stmt in stmts {
        match stmt {
            FloatStmt::Assign(_,e)|FloatStmt::Return(e)|
            FloatStmt::ReturnInteger(e,_)=>
                validate_fp_calls_expr(e,declarations,imports,source_imports,depth+1)?,
            FloatStmt::StoreGlobal {value,..}|
            FloatStmt::StorePointerParameter {value,..}|
            FloatStmt::StorePointerLocal {value,..}=>
                validate_fp_calls_expr(value,declarations,imports,source_imports,depth+1)?,
            FloatStmt::ReturnIndirectInteger {arguments,..}=>{
                for arg in arguments {
                    if let ScalarCallArg::Float(e)=arg {
                        validate_fp_calls_expr(e,declarations,imports,
                            source_imports,depth+1)?;
                    }
                }
            }
            FloatStmt::IfReturn(cond,yes,no)=>{
                validate_fp_calls_cond(cond,declarations,imports,source_imports,depth+1)?;
                validate_fp_calls_expr(yes,declarations,imports,source_imports,depth+1)?;
                validate_fp_calls_expr(no,declarations,imports,source_imports,depth+1)?;
            }
            FloatStmt::IfAssign(cond,yes,no)=>{
                validate_fp_calls_cond(cond,declarations,imports,source_imports,depth+1)?;
                for (_,e) in yes.iter().chain(no) {
                    validate_fp_calls_expr(e,declarations,imports,source_imports,depth+1)?;
                }
            }
            FloatStmt::IfBlock(cond,yes,no)=>{
                validate_fp_calls_cond(cond,declarations,imports,source_imports,depth+1)?;
                validate_fp_calls_stmts(yes,declarations,imports,source_imports,depth+1)?;
                validate_fp_calls_stmts(no,declarations,imports,source_imports,depth+1)?;
            }
            FloatStmt::While(cond,body)=>{
                validate_fp_calls_cond(cond,declarations,imports,source_imports,depth+1)?;
                validate_fp_calls_stmts(body,declarations,imports,source_imports,depth+1)?;
            }
            FloatStmt::DoWhile(body,cond)=>{
                validate_fp_calls_stmts(body,declarations,imports,source_imports,depth+1)?;
                validate_fp_calls_cond(cond,declarations,imports,source_imports,depth+1)?;
            }
            FloatStmt::For {init,condition,increment,body}=>{
                validate_fp_calls_stmts(init,declarations,imports,source_imports,depth+1)?;
                if let Some(cond)=condition {
                    validate_fp_calls_cond(cond,declarations,imports,source_imports,depth+1)?;
                }
                validate_fp_calls_stmts(body,declarations,imports,source_imports,depth+1)?;
                validate_fp_calls_stmts(increment,declarations,imports,source_imports,depth+1)?;
            }
            FloatStmt::AssignPointer {..}|FloatStmt::AssignDataPointer {..}|
            FloatStmt::Break|FloatStmt::Continue=>{},
        }
    }
    Ok(())
}
fn emit_scalar_float_bodies(functions:&[ScalarFloatBody])
    ->Result<Vec<u8>,CodegenError> {
    emit_scalar_float_bodies_internal(functions,false)
}
fn emit_scalar_float_bodies_internal(
    functions:&[ScalarFloatBody],source_imports:bool,
)->Result<Vec<u8>,CodegenError> {
    emit_scalar_float_bodies_with_globals(functions,source_imports,&[])
}
fn emit_scalar_float_bodies_with_globals(
    functions:&[ScalarFloatBody],source_imports:bool,
    globals:&[FpScalarGlobal],
)->Result<Vec<u8>,CodegenError> {
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
    if !source_imports && !globals.is_empty() {
        return Err(reject("standalone FP API cannot import unsourced global data"));
    }
    let mut global_ids=HashMap::new();
    let mut global_names=std::collections::HashSet::new();
    for g in globals {
        if !safe_scalar_external_name(&g.name) ||
            !global_names.insert(g.name.clone()) ||
            global_ids.contains_key(&g.symbol) ||
            !matches!(g.linkage,Linkage::Import|Linkage::Local|Linkage::Export) {
            return Err(reject("invalid or duplicate checked FP data symbol"));
        }
        let id=module.declare_data(&g.name,g.linkage,true,false)
            .map_err(|e|CodegenError::Backend(e.to_string()))?;
        global_ids.insert(g.symbol,(id,g.ty));
    }
    // Source-owned definitions have real ELF storage: .data for exact
    // IEEE bit images, .bss for tentative/uninitialized objects.
    for g in globals {
        if g.linkage==Linkage::Import {
            if g.initializer.is_some() {
                return Err(reject("imported FP global cannot own initializer"));
            }
            continue;
        }
        let id=global_ids.get(&g.symbol).ok_or_else(||
            reject("FP data definition was not declared"))?.0;
        let mut data=cranelift_module::DataDescription::new();
        let bytes=match g.ty {FloatType::F32=>4usize,FloatType::F64=>8usize};
        data.set_align(bytes as u64);
        if let Some(image)=&g.initializer {
            if image.len()!=bytes {
                return Err(reject("FP global initializer native width mismatch"));
            }
            data.define(image.clone().into_boxed_slice());
        } else {
            data.define_zeroinit(bytes);
        }
        module.define_data(id,&data)
            .map_err(|e|CodegenError::Backend(e.to_string()))?;
    }
    let mut declared=HashMap::new();
    for f in functions {
        if declared.insert(f.name.clone(),
            (f.parameters.clone(),f.result)).is_some() {
            return Err(reject("duplicate scalar routine definition"));
        }
    }
    let mut imports=HashMap::new();
    for f in functions {
        validate_fp_calls_stmts(&f.statements,&declared,&mut imports,
            source_imports,0)?;
    }
    // Every import has been checked against actual EDG callsites, target
    // classes and local definitions; no unknown extern is silently emitted.
    let mut import_ids=Vec::new();
    for (name,(params,ret)) in &imports {
        let mut sig=module.make_signature();
        for ty in params {sig.params.push(AbiParam::new(ty.machine()));}
        sig.returns.push(AbiParam::new(ret.machine()));
        let id=module.declare_function(name,Linkage::Import,&sig)
            .map_err(|e|CodegenError::Backend(e.to_string()))?;
        import_ids.push((name.clone(),id));
    }
    let mut exports=Vec::new();
    for f in functions {
        checked_scalar_passing_plan(&f.parameters,f.result)?;
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
    for (f,id) in functions.iter().zip(exports.iter().copied()) {
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
        for (name,id) in &import_ids {
            let reference=module.declare_func_in_func(*id,&mut ctx.func);
            refs.insert(name.clone(),reference);
        }
        let global_refs:FpGlobalRefs=global_ids.iter().map(|(symbol,(id,ty))|
            (*symbol,(module.declare_data_in_func(*id,&mut ctx.func),*ty)))
            .collect();
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
                &vars,&f.locals,None,&refs,&global_refs,0)? {
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
/// Only EDG-native *function* pointers are callable. Verify pointer
/// layout, pointee role and actual prototype before accepting the ABI.
fn function_pointer_type_from_edg(il:&Il,id:crate::TypeId)
    ->Result<(crate::TypeId,crate::TypeId),CodegenError> {
    let pointer=super::canonical_abi_type(il,id)?;
    let ptr=il.types.iter().find(|t|t.id==pointer &&
        super::semantic(4,t.kind)==5 && t.related_role==Some(1))
        .ok_or_else(||reject("indirect FP call does not use a function pointer"))?;
    let layout=il.type_object_sizes.iter().find(|x|x.owner==pointer)
        .ok_or_else(||reject("indirect FP function pointer has no layout"))?;
    if layout.bytes!=8 || layout.alignment!=8 {
        return Err(reject("indirect FP function pointer has non-SysV layout"));
    }
    let target=super::canonical_abi_type(il,ptr.related.ok_or_else(||
        reject("FP function pointer has no pointee type"))?)?;
    if !il.types.iter().any(|t|t.id==target &&
        super::semantic(4,t.kind)==2 && t.related_role==Some(3)) {
        return Err(reject("FP pointer does not point to a C routine prototype"));
    }
    let proto=il.function_signatures.iter().find(|p|p.owner==target)
        .ok_or_else(||reject("indirect FP call lacks native prototype"))?;
    if !proto.prototyped || proto.variadic {
        return Err(reject("indirect FP unprototyped/variadic call unsupported"));
    }
    Ok((pointer,target))
}
/// A native 64-bit pointer to **unqualified standard C float/double**.
/// Only a pointer to a scalar is accepted; arrays, function pointers,
/// qualified pointees, void*, integers and pointer arithmetic remain rejected.
fn fp_data_pointer_type_from_edg(
    il:&Il,id:crate::TypeId,
)->Result<FloatType,CodegenError> {
    let pointer=super::canonical_abi_type(il,id)?;
    let native=il.types.iter().find(|t|t.id==pointer &&
        super::semantic(4,t.kind)==5 && t.related_role==Some(1))
        .ok_or_else(||reject("FP memory formal is not a native C pointer"))?;
    let size=il.type_object_sizes.iter().find(|t|t.owner==pointer)
        .ok_or_else(||reject("FP memory pointer has no native layout"))?;
    if size.bytes!=8 || size.alignment!=8 {
        return Err(reject("FP data pointer does not have the x86-64 SysV layout"));
    }
    check_plain_fp_local(il,native.related.ok_or_else(||
        reject("FP data pointer lacks pointee type"))?)?;
    type_from_edg(il,native.related.ok_or_else(||
        reject("FP memory pointer lacks native pointee"))?)
}
/// Resolve a direct checked pointer formal. No computed addresses, local
/// pointer variables, casts or other memory provenance are accepted.
fn source_fp_pointer_formal(
    ctx:&FpSourceCtx<'_>,id:crate::graph::ExpressionId,ty:FloatType,
)->Result<usize,CodegenError> {
    let expr=ctx.graph.expression(id).ok_or_else(||
        reject("FP pointer operand has no owned EDG expression"))?;
    if super::semantic(2,expr.data.kind)!=2 ||
        expr.data.constant_kind.is_some() || !expr.operands.is_empty() {
        return Err(reject("FP memory pointer must be an unchanged formal"));
    }
    let symbol=expr.data.symbol_id.ok_or_else(||
        reject("FP memory pointer formal has no symbol"))?;
    let n=ctx.params.iter().position(|p|*p==symbol).ok_or_else(||
        reject("FP memory load/store pointer not a source formal"))?;
    if ctx.param_types[n]!=ScalarType::DataPointer(ty) ||
        fp_data_pointer_type_from_edg(ctx.il,expr.data.type_id.ok_or_else(||
            reject("FP memory pointer operand missing native type"))?)?!=ty {
        return Err(reject("FP memory pointer source pointee mismatch"));
    }
    Ok(n)
}
/// EDG may assign separate native type identities to a function designator
/// and the pointee of a pointer-to-function, even when they have the same
/// valid C prototype. Compare the *complete* source-verified scalar calling
/// signature, never the numeric type-ID or only the argument count.
fn source_callable_signature(
    il:&Il,original:crate::TypeId,
)->Result<(Vec<ScalarType>,ScalarType),CodegenError> {
    let ty=super::canonical_abi_type(il,original)?;
    let function_ty=if il.types.iter().any(|t|t.id==ty &&
        super::semantic(4,t.kind)==2 && t.related_role==Some(3)) {
        ty
    } else {
        let (_,fn_ty)=function_pointer_type_from_edg(il,ty)?;
        fn_ty
    };
    let def=il.types.iter().find(|t|t.id==function_ty &&
        super::semantic(4,t.kind)==2 && t.related_role==Some(3))
        .ok_or_else(||reject("indirect designator is not a function prototype"))?;
    let prototype=il.function_signatures.iter().find(|p|p.owner==function_ty)
        .ok_or_else(||reject("indirect designator has no prototype metadata"))?;
    if !prototype.prototyped || prototype.variadic {
        return Err(reject("indirect designator is variadic/unprototyped"));
    }
    let ret_ty=def.related.ok_or_else(||
        reject("indirect function has no native return type"))?;
    if function_pointer_type_from_edg(il,ret_ty).is_ok() {
        return Err(reject("nested pointer result is not a supported callable ABI"));
    }
    let returned=scalar_type_from_edg(il,ret_ty)?;
    let mut declared:Vec<_>=il.type_parameters.iter()
        .filter(|p|p.owner==function_ty).collect();
    declared.sort_by_key(|p|p.position);
    if declared.len()>256 || declared.iter().enumerate()
        .any(|(n,p)|p.position!=n as u32) {
        return Err(reject("indirect function prototype has invalid positions"));
    }
    if declared.iter().any(|p|function_pointer_type_from_edg(il,p.ty).is_ok()) {
        return Err(reject("nested function-pointer parameter ABI unsupported"));
    }
    let parameters=declared.iter().map(|p|scalar_type_from_edg(il,p.ty))
        .collect::<Result<Vec<_>,_>>()?;
    Ok((parameters,returned))
}
/// Reject a nonstandard ABI class, unsupported integer width, or synthetic
/// type before generating code. Only standard signed/unsigned 32/64 bit C
/// integer formals and native F32/F64 values enter this first mixed ABI slice.
fn scalar_type_from_edg(il:&Il,id:crate::TypeId)
    ->Result<ScalarType,CodegenError> {
    if let Ok(t)=type_from_edg(il,id) {return Ok(ScalarType::Float(t));}
    let canonical=super::canonical_abi_type(il,id)?;
    if let Ok((pointer,_))=function_pointer_type_from_edg(il,canonical) {
        // EDG can assign distinct native type IDs to structurally
        // equivalent function pointers (e.g. a formal and an automatic
        // local). Normalize *only* complete, nonvariadic, exact scalar
        // callable prototypes so SSA copies/calls use the same identity.
        // A data pointer, nested callback ABI or unknown type never aliases.
        let signature=source_callable_signature(il,pointer)?;
        let mut representative=pointer;
        for ty in &il.types {
            if ty.id.0>=representative.0 ||
               super::semantic(4,ty.kind)!=5 ||
               ty.related_role!=Some(1) {
                continue;
            }
            if function_pointer_type_from_edg(il,ty.id).is_ok() &&
                source_callable_signature(il,ty.id).ok()==Some(signature.clone()) {
                representative=ty.id;
            }
        }
        return Ok(ScalarType::FunctionPointer(representative));
    }
    if let Ok(t)=fp_data_pointer_type_from_edg(il,canonical) {
        return Ok(ScalarType::DataPointer(t));
    }
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
/// Only an unmodified EDG formal or automatic local can provide a callable
/// pointer SSA value; aliases, casts, data-pointer coercions and memory loads
/// require a separate provenance and lifetime model.
fn pointer_source_from_edg(
    ctx:&FpSourceCtx<'_>,id:crate::graph::ExpressionId,
)->Result<(PointerSource,crate::TypeId),CodegenError> {
    let expr=ctx.graph.expression(id)
        .ok_or_else(||reject("callable pointer value missing EDG expression"))?;
    if super::semantic(2,expr.data.kind)!=2 ||
        !expr.operands.is_empty() || expr.data.constant_kind.is_some() {
        return Err(reject("function-pointer value must be direct checked variable"));
    }
    let symbol=expr.data.symbol_id.ok_or_else(||
        reject("function-pointer value has no native EDG symbol"))?;
    let ty=match scalar_type_from_edg(ctx.il,expr.data.type_id
        .ok_or_else(||reject("function-pointer value lacks native type"))?)? {
        ScalarType::FunctionPointer(ty)=>ty,
        _=>return Err(reject("source expression is not callable-pointer typed")),
    };
    let source=if let Some(i)=ctx.params.iter().position(|p|*p==symbol) {
        PointerSource::Parameter(i)
    } else if let Some(i)=ctx.locals.iter().position(|p|*p==symbol) {
        PointerSource::Local(i)
    } else {
        return Err(reject("global, computed or nonlocal function-pointer unsupported"));
    };
    if pointer_source_type(source,ctx.param_types,ctx.local_types)?!=ty {
        return Err(reject("function-pointer value differs from native symbol ABI"));
    }
    Ok((source,ty))
}
/// Source-owned named C FP object import, never an arbitrary memory address.
fn source_fp_global(
    il:&Il,symbol:crate::SymbolId,
)->Result<(String,FloatType,Linkage,Option<Vec<u8>>),CodegenError> {
    let mut metas=il.global_variables.iter().filter(|g|g.symbol==symbol);
    let meta=metas.next().ok_or_else(||
        reject("FP global is not an EDG-classified file-scope object"))?;
    if metas.next().is_some() {
        return Err(reject("FP global has ambiguous native storage metadata"));
    }
    if !matches!(meta.linkage,1|2|3) ||
        meta.thread_local || meta.local_to_function ||
        il.static_local_owners.iter().any(|s|s.symbol==symbol) {
        return Err(reject("FP globals need ordinary C storage and source metadata"));
    }
    let name=il.symbols.iter().find(|s|s.id==symbol && s.kind==2)
        .map(|s|s.name.clone())
        .ok_or_else(||reject("FP global has no native variable identity"))?;
    if !safe_scalar_external_name(&name) {
        return Err(reject("FP external global has unsafe/non-C symbol name"));
    }
    let declared=il.symbol_types.iter().find(|s|s.symbol==symbol)
        .ok_or_else(||reject("FP global declaration has no native source type"))?.ty;
    check_plain_fp_local(il,declared)?;
    let ty=type_from_edg(il,declared)?;
    let initializer=if meta.explicit_initializer {
        let mut images=il.global_float_initializers.iter()
            .filter(|v|v.symbol==symbol);
        let image=images.next().ok_or_else(||
            reject("explicit FP global initializer lacks exact native IEEE image"))?;
        if images.next().is_some() {
            return Err(reject("ambiguous native FP global initializer image"));
        }
        let width=match ty {FloatType::F32=>32,FloatType::F64=>64};
        if image.ty!=declared || u32::from(image.width)!=width {
            return Err(reject("FP global initializer native type/width mismatch"));
        }
        Some(image.bits.to_le_bytes()[..(width/8) as usize].to_vec())
    } else {
        if il.global_float_initializers.iter().any(|v|v.symbol==symbol) {
            return Err(reject("native FP global image without explicit definition"));
        }
        None
    };
    let linkage=match meta.linkage {
        1 if meta.explicit_initializer=>Linkage::Export,
        1=>Linkage::Import,2=>Linkage::Local,3=>Linkage::Export,
        _=>return Err(reject("unknown FP global storage linkage")),
    };
    Ok((name,ty,linkage,initializer))
}
/// Source expressions are checked using exact EDG symbol IDs and widths.
struct FpSourceCtx<'a> {
    il:&'a Il,
    graph:&'a crate::graph::Graph,
    routine:crate::SymbolId,
    params:&'a [crate::SymbolId],
    locals:&'a [crate::SymbolId],
    param_types:&'a [ScalarType],
    local_types:&'a [ScalarType],
}
/// Decode a source-owned integer constant, never a constant inferred
/// from its printed name or a generic C integer expression. The IL stores
/// unsigned U64 as an i64 bit carrier; do not sign-convert its payload.
fn source_integer_constant(
    ctx:&FpSourceCtx<'_>,node:&crate::graph::ExpressionNode,
    expected:ScalarType,
)->Result<Option<u64>,CodegenError> {
    if node.data.constant_kind.is_none() {return Ok(None);}
    if !node.operands.is_empty() || node.data.symbol_id.is_some() {
        return Err(reject("integer constant has operands or variable identity"));
    }
    let actual=scalar_type_from_edg(ctx.il,node.data.type_id.ok_or_else(||
        reject("integer literal has no native EDG type"))?)?;
    if actual!=expected {
        return Err(reject("integer literal native type differs from target ABI"));
    }
    let bits=node.data.constant_integer.ok_or_else(||
        reject("integer constant has no exact 64-bit EDG image"))? as u64;
    checked_integer_literal(actual,bits)?;
    Ok(Some(bits))
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
            .ok_or_else(||reject("FP scalar call has no callee expression"))?;
        let (callee_name,external_callee,pointer_source,pointer_type,fn_ty)=
            if super::semantic(2,callee.data.kind)==6 &&
                callee.operands.is_empty() {
                // Existing exact named routine path, including forward defs.
                let symbol=callee.data.symbol_id.ok_or_else(||
                    reject("FP direct callee has no native symbol"))?;
                let named=ctx.il.symbols.iter().find(|s|
                    s.id==symbol && s.kind==1)
                    .ok_or_else(||reject("FP direct target is not a named routine"))?;
                let name=named.name.clone();
                let in_module=ctx.il.entities.iter().any(|e|matches!(e,
                    crate::Entity::Routine{name,has_definition:true}
                        if name==&named.name));
                if !in_module {
                    // An actual source C routine declaration, not a guessed
                    // linker name, is required before importing its symbol.
                    if !safe_scalar_external_name(&name) ||
                        !ctx.il.entities.iter().any(|e|matches!(e,
                            crate::Entity::Routine{name,has_definition:false}
                                if name==&named.name)) {
                        return Err(reject("FP external call has no verified C declaration"));
                    }
                }
                let callee_ty=ctx.il.symbol_types.iter()
                    .find(|p|p.symbol==symbol)
                    .ok_or_else(||reject("FP direct callee has no native type"))?;
                let fn_ty=super::canonical_abi_type(ctx.il,callee_ty.ty)?;
                (Some(name),!in_module,None,None,fn_ty)
            } else {
                // Exactly a pointer-valued *function formal*, optionally
                // expressed by EDG as a function-designator dereference.
                // Neither pointer locals nor computed addresses are accepted.
                let pointer_expr=if super::semantic(2,callee.data.kind)==3 &&
                    callee.operands.len()==1 &&
                    callee.data.operator.is_some_and(|op|
                        super::semantic(3,op)==44) {
                    ctx.graph.expression(callee.operands[0]).ok_or_else(||
                        reject("FP indirect dereference has no pointer operand"))?
                } else {callee};
                if super::semantic(2,pointer_expr.data.kind)!=2 ||
                    !pointer_expr.operands.is_empty() ||
                    pointer_expr.data.constant_kind.is_some() {
                    return Err(reject("FP indirect target must be a direct function-pointer formal"));
                }
                let (source,pointer_type)=pointer_source_from_edg(
                    ctx,if std::ptr::eq(pointer_expr,callee) {
                        node.operands[0]
                    } else {callee.operands[0]})?;
                let (_,fn_ty)=function_pointer_type_from_edg(ctx.il,pointer_type)?;
                // The EDG expression must refer to the native pointer type,
                // or be an actual function-designator dereference. Neither
                // a cast to function pointer nor data-pointer coercion is safe.
                if !std::ptr::eq(pointer_expr,callee) {
                    let designator=super::canonical_abi_type(ctx.il,
                        callee.data.type_id.ok_or_else(||
                            reject("indirect FP designator lacks type"))?)?;
                    // The native designator can have a distinct type ID.
                    // Its full source prototype must still agree with the
                    // exact pointee carried by the verified pointer formal.
                    if source_callable_signature(ctx.il,designator)? !=
                       source_callable_signature(ctx.il,fn_ty)? {
                        return Err(reject("indirect FP designator prototype mismatch"));
                    }
                }
                (None,false,Some(source),Some(pointer_type),fn_ty)
            };
        let func=ctx.il.types.iter().find(|p|p.id==fn_ty &&
            super::semantic(4,p.kind)==2 && p.related_role==Some(3))
            .ok_or_else(||reject("FP callee requires a native function prototype"))?;
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
                    if let Some(bits)=source_integer_constant(ctx,e,target)? {
                        ScalarCallArg::IntegerConstant {bits,ty:target}
                    } else {
                        if super::semantic(2,e.data.kind)!=2 ||
                            !e.operands.is_empty() {
                            return Err(reject("FP direct INTEGER argument must be a formal/literal"));
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
                }
                ScalarType::FunctionPointer(_)|ScalarType::DataPointer(_)=>return Err(reject(
                    "passing pointer arguments needs a separate typed call contract")),
            };
            parameters.push(target);
            arguments.push(value);
        }
        return if let Some(callee)=callee_name {
            if external_callee {
                Ok(FloatExpr::CallExternal {
                    callee,parameters,return_type:ty,arguments,
                })
            } else {
                Ok(FloatExpr::Call {
                    callee,parameters,return_type:ty,arguments,
                })
            }
        } else {
            Ok(FloatExpr::CallIndirect {
                pointer:pointer_source.ok_or_else(||
                    reject("FP indirect pointer source was not resolved"))?,
                pointer_type:pointer_type.ok_or_else(||
                    reject("FP indirect pointer type was not resolved"))?,
                parameters,return_type:ty,arguments,
            })
        };
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
            if ctx.local_types[i]!=ScalarType::Float(ty) {
                return Err(reject("FP local rvalue type mismatch"));
            }
            return Ok(FloatExpr::Local(i));
        }
        if ctx.il.global_variables.iter().any(|g|g.symbol==symbol) {
            let (_,actual,_,_)=source_fp_global(ctx.il,symbol)?;
            if actual!=ty || scalar_type_from_edg(ctx.il,node.data.type_id.ok_or_else(||
                    reject("FP external global expression lacks native type"))?)?!=
                    ScalarType::Float(actual) {
                return Err(reject("FP external global source expression type mismatch"));
            }
            return Ok(FloatExpr::ExternalGlobal {symbol,ty});
        }
        return Err(reject("FP global/address-taken rvalue requires memory lowering"));
    }
    if kind!=3 {return Err(reject("unsupported FP source expression form"));}
    let op=node.data.operator.map(|v|super::semantic(3,v))
        .ok_or_else(||reject("missing native FP operation"))?;
    // Only an EDG-authoritative conversion from a direct integer formal is
    // accepted. Integer expressions, local memory and implicit bitcasting
    // are *not* silently accepted as scalar floating point values.
    if op==44 && node.operands.len()==1 {
        let n=source_fp_pointer_formal(ctx,node.operands[0],ty)?;
        return Ok(FloatExpr::LoadPointerParameter {parameter:n,ty});
    }
    if node.operands.len()==1 && matches!(op,11|12|16) {
        let source=ctx.graph.expression(node.operands[0])
            .ok_or_else(||reject("missing integer-to-FP cast operand"))?;
        let source_ty=source.data.type_id.ok_or_else(||
            reject("integer-to-FP cast operand has no native type"))?;
        let from=scalar_type_from_edg(ctx.il,source_ty)?;
        if let ScalarType::Int {..}=from {
            if let Some(bits)=source_integer_constant(ctx,source,from)? {
                return Ok(FloatExpr::IntegerConstantCast {bits,source:from,to:ty});
            }
            if super::semantic(2,source.data.kind)!=2 ||
                !source.operands.is_empty() || source.data.constant_kind.is_some() {
                return Err(reject("integer-to-FP cast requires a direct integer formal or literal"));
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
    let out=if op==25 {
        // This is the source EDG conditional operator, not a speculative
        // floating select or the integer backend's own expression kind.
        if node.operands.len()!=3 {
            return Err(reject("C FP conditional expression requires three operands"));
        }
        let condition=comparison_from_edg(ctx,node.operands[0])?;
        FloatExpr::Conditional {
            condition:Box::new(condition),
            yes:Box::new(arg(1)?),
            no:Box::new(arg(2)?),
        }
    } else if node.operands.len()==1 {
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
/// A scalar C floating condition is true exactly when it is *nonzero*.
/// Both +0.0 and -0.0 compare equal to zero; NaN is true because unordered
/// comparisons satisfy C != (and false for the logical negation). Keep the
/// source expression as F32/F64; never interpret a bool-cast as FP bit data.
fn truthiness_from_edg(
    ctx:&FpSourceCtx<'_>,source:crate::graph::ExpressionId,negated:bool,
)->Result<FloatCondition,CodegenError> {
    let left=expression_from_edg(ctx,source,0)?;
    let ty=expression_type(&left,ctx.param_types,ctx.local_types,0)?;
    let zero=match ty {
        FloatType::F32=>FloatExpr::Constant32(0.0f32.to_bits()),
        FloatType::F64=>FloatExpr::Constant64(0.0f64.to_bits()),
    };
    let cond=FloatCondition {
        op:if negated {FloatPredicate::Eq} else {FloatPredicate::Ne},
        left,right:zero,
    };
    check_condition(&cond,ctx.param_types,ctx.local_types,
        &vec![true;ctx.local_types.len()])?;
    Ok(cond)
}
/// A C floating-point comparison has an integer/bool result, but its
/// operands must both have verified native FP widths. Never lower it by
/// first interpreting the result as a floating-point expression.
fn comparison_from_edg(
    ctx:&FpSourceCtx<'_>,id:crate::graph::ExpressionId,
)->Result<FloatCondition,CodegenError> {
    let node=ctx.graph.expression(id).ok_or_else(||
        reject("missing EDG floating comparison"))?;
    // A direct FP-valued source expression is a legal C scalar condition.
    // We also accept EDG's *explicit* bool cast and logical negation
    // applied to an FP value. Reject integer/aggregate truthiness here:
    // those belong to the integer CFG rather than guessing from FP bits.
    let kind=super::semantic(2,node.data.kind);
    let operator=node.data.operator.map(|v|super::semantic(3,v));
    // EDG may preserve the original floating expression (including pure
    // arithmetic, not just a variable) directly as the condition. The
    // strict expression decoder below rejects calls/side effects we do not
    // model, but all verified F32/F64 values have C scalar truthiness.
    if node.data.type_id.is_some_and(|ty|type_from_edg(ctx.il,ty).is_ok()) {
        return truthiness_from_edg(ctx,id,false);
    }
    let boolean_kind=kind==3 &&
        node.data.type_id.and_then(|ty|
            super::canonical_abi_type(ctx.il,ty).ok()).is_some_and(|ty|
            ctx.il.c_integer_types.iter().any(|k|k.id==ty &&
                (k.is_bool || k.bits==32)));
    if boolean_kind && node.data.constant_kind.is_none() &&
        node.operands.len()==1 {
        if operator==Some(17) {
            return truthiness_from_edg(ctx,node.operands[0],false);
        }
        if operator==Some(28) {
            let operand=ctx.graph.expression(node.operands[0]).ok_or_else(||
                reject("logical-not FP condition has no operand"))?;
            let direct=operand.data.type_id
                .is_some_and(|ty|type_from_edg(ctx.il,ty).is_ok());
            if direct && super::semantic(2,operand.data.kind)==2 &&
                operand.operands.is_empty() {
                return truthiness_from_edg(ctx,node.operands[0],true);
            }
            if super::semantic(2,operand.data.kind)==3 &&
                operand.data.operator.is_some_and(|op|
                    super::semantic(3,op)==17) && operand.operands.len()==1 &&
                operand.data.constant_kind.is_none() {
                return truthiness_from_edg(ctx,operand.operands[0],true);
            }
        }
    }
    if kind!=3 || node.operands.len()!=2 ||
        node.data.constant_kind.is_some() {
        return Err(reject("FP condition requires a source-verified comparison or scalar truthiness"));
    }
    let integer=super::canonical_abi_type(ctx.il,
        node.data.type_id.ok_or_else(||
            reject("FP condition has no native result type"))?)?;
    if !ctx.il.c_integer_types.iter().any(|ty|
        ty.id==integer && (ty.is_bool || ty.bits==32)) {
        return Err(reject(format!(
            "FP comparison must produce C int or bool: kind={kind}, operator={operator:?}, expr_type={:?}, canonical={integer:?}, operand_types={:?}",
            node.data.type_id,
            node.operands.iter().map(|id|ctx.graph.expression(*id)
                .and_then(|e|e.data.type_id)).collect::<Vec<_>>()
        )));
    }
    let op=match operator {
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
                let index=ctx.locals.iter().position(|p|*p==decl.symbol)
                    .ok_or_else(||reject("FP declaration local symbol missing"))?;
                if scalar_type_from_edg(ctx.il,decl.ty)?!=ctx.local_types[index] {
                    return Err(reject("FP declaration source ABI type mismatch"));
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
            if let ScalarType::FunctionPointer(pointer_type)=ctx.local_types[local] {
                if init.kind!=3 {
                    return Err(reject("callable pointer initializer requires exact expression"));
                }
                let (source,actual)=pointer_source_from_edg(ctx,node.expression
                    .ok_or_else(||reject("pointer initializer lacks source expression"))?)?;
                if actual!=pointer_type {
                    return Err(reject("pointer initializer native type mismatch"));
                }
                output.push(FloatStmt::AssignPointer {local,pointer_type,source});
                return Ok(());
            }
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
                        (ScalarType::Float(FloatType::F32),32)=>
                            FloatExpr::Constant32(exact.bits as u32),
                        (ScalarType::Float(FloatType::F64),64)=>
                            FloatExpr::Constant64(exact.bits),
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
            // Native C *p = rhs: p must be an unchanged, unqualified
            // source-verified pointer formal to exactly this F32/F64 type.
            if super::semantic(2,left.data.kind)==3 &&
                left.data.operator.is_some_and(|op|super::semantic(3,op)==44) &&
                left.operands.len()==1 {
                let ty=type_from_edg(ctx.il,left.data.type_id.ok_or_else(||
                    reject("FP memory assignment lacks native lvalue type"))?)?;
                let parameter=source_fp_pointer_formal(ctx,left.operands[0],ty)?;
                let value=expression_from_edg(ctx,operation.operands[1],0)?;
                if expression_type(&value,ctx.param_types,ctx.local_types,0)?!=ty {
                    return Err(reject("FP pointer store missing source conversion"));
                }
                output.push(FloatStmt::StorePointerParameter {parameter,ty,value});
                return Ok(());
            }
            if super::semantic(2,left.data.kind)!=2 || !left.operands.is_empty() {
                return Err(reject("FP assignment target is not direct scalar storage"));
            }
            if let Some(symbol)=left.data.symbol_id {
                if ctx.il.global_variables.iter().any(|g|g.symbol==symbol) {
                    let (_,ty,_,_)=source_fp_global(ctx.il,symbol)?;
                    if scalar_type_from_edg(ctx.il,left.data.type_id.ok_or_else(||
                        reject("FP global store lvalue has no native type"))?)?!=
                        ScalarType::Float(ty) {
                        return Err(reject("FP global store ABI type mismatch"));
                    }
                    let value=expression_from_edg(ctx,operation.operands[1],0)?;
                    if expression_type(&value,ctx.param_types,ctx.local_types,0)?!=ty {
                        return Err(reject("FP global store RHS needs conversion"));
                    }
                    output.push(FloatStmt::StoreGlobal {symbol,ty,value});
                    return Ok(());
                }
            }
            let local=ctx.locals.iter().position(|p|Some(*p)==left.data.symbol_id)
                .ok_or_else(||reject("FP assignment target is not a checked local"))?;
            if scalar_type_from_edg(ctx.il,left.data.type_id.ok_or_else(||
                reject("missing native scalar lvalue type"))?)?!=ctx.local_types[local] {
                return Err(reject("FP lvalue type mismatch"));
            }
            if let ScalarType::FunctionPointer(pointer_type)=ctx.local_types[local] {
                let (source,actual)=pointer_source_from_edg(ctx,operation.operands[1])?;
                if actual!=pointer_type {
                    return Err(reject("pointer assignment source ABI differs from target"));
                }
                output.push(FloatStmt::AssignPointer {local,pointer_type,source});
                return Ok(());
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
                    // Exact INTEGER-valued indirect calls preserve RAX/EAX;
                    // they are not FP-to-int numeric conversions.
                    let is_call=ctx.graph.calls.iter().any(|site|
                        site.function==native.data.function &&
                        site.statement_path==native.data.statement_path &&
                        site.path==native.data.path);
                    if is_call {
                        output.push(lower_integer_indirect_return(ctx,source,result)?);
                        return Ok(());
                    }
                    // Only an EDG-authoritative FP-to-INTEGER cast remains
                    // legal in the ordinary expression-return path.
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
                ScalarType::FunctionPointer(_)|ScalarType::DataPointer(_)=>return Err(reject(
                    "FP reference module cannot return pointer values")),
            }
        }
        _=>return Err(reject("unsupported FP control flow or statement form")),
    }
    if output.len()>10000 {return Err(reject("FP statement count exceeds limit"));}
    Ok(())
}
/// Source-checked narrow INTEGER-result indirect call slice. We accept an
/// *exact* return expression `return fp(...);` with an EDG-verified function
/// pointer formal. A separate expression tree is needed for integer arithmetic
/// and casts, so these remain unsupported.
fn lower_integer_indirect_return(
    ctx:&FpSourceCtx<'_>,id:crate::graph::ExpressionId,
    result:ScalarType,
)->Result<FloatStmt,CodegenError> {
    if !matches!(result,ScalarType::Int {bits:32|64,..}) {
        return Err(reject("indirect return is not a supported C INTEGER"));
    }
    let node=ctx.graph.expression(id)
        .ok_or_else(||reject("missing INTEGER indirect-call expression"))?;
    if scalar_type_from_edg(ctx.il,node.data.type_id.ok_or_else(||
        reject("INTEGER indirect-call expression lacks native result type"))?)?!=result {
        return Err(reject("INTEGER indirect-call expression/result type mismatch"));
    }
    let mut calls=ctx.graph.calls.iter().filter(|site|
        site.function==node.data.function &&
        site.statement_path==node.data.statement_path &&
        site.path==node.data.path);
    let site=calls.next().ok_or_else(||
        reject("INTEGER return is not a direct EDG-recorded call"))?;
    if calls.next().is_some() || site.first_argument!=1 ||
        node.operands.is_empty() {
        return Err(reject("INTEGER indirect call has ambiguous/member argument layout"));
    }
    let callee=ctx.graph.expression(node.operands[0])
        .ok_or_else(||reject("INTEGER indirect call has no native callee"))?;
    let pointer_expr=if super::semantic(2,callee.data.kind)==3 &&
        callee.operands.len()==1 &&
        callee.data.operator.is_some_and(|op|super::semantic(3,op)==44) {
        ctx.graph.expression(callee.operands[0]).ok_or_else(||
            reject("INTEGER function designator has no pointer"))?
    } else {callee};
    if super::semantic(2,pointer_expr.data.kind)!=2 ||
        !pointer_expr.operands.is_empty() ||
        pointer_expr.data.constant_kind.is_some() {
        return Err(reject("INTEGER indirect call requires an unmodified pointer formal"));
    }
    let (pointer,pointer_type)=pointer_source_from_edg(
        ctx,if std::ptr::eq(pointer_expr,callee) {
            node.operands[0]
        } else {callee.operands[0]})?;
    let (_,fn_ty)=function_pointer_type_from_edg(ctx.il,pointer_type)?;
    if !std::ptr::eq(pointer_expr,callee) {
        let designator=super::canonical_abi_type(ctx.il,
            callee.data.type_id.ok_or_else(||
                reject("INTEGER indirect designator lacks native type"))?)?;
        if source_callable_signature(ctx.il,designator)?!=
           source_callable_signature(ctx.il,fn_ty)? {
            return Err(reject("INTEGER indirect designator prototype mismatch"));
        }
    }
    let (parameters,returned)=source_callable_signature(ctx.il,fn_ty)?;
    if returned!=result || parameters.len()!=node.operands.len()-1 {
        return Err(reject("INTEGER indirect return signature/arity mismatch"));
    }
    if parameters.iter().any(|ty|matches!(ty,ScalarType::FunctionPointer(_))) {
        return Err(reject("nested function-pointer INTEGER call unsupported"));
    }
    let mut arguments=Vec::with_capacity(parameters.len());
    for (target,&operand) in parameters.iter().zip(&node.operands[1..]) {
        let value=match target {
            ScalarType::Float(t)=>{
                let expr=expression_from_edg(ctx,operand,1)?;
                if expression_type(&expr,ctx.param_types,ctx.local_types,0)?!=*t {
                    return Err(reject("INTEGER indirect FP argument needs source conversion"));
                }
                ScalarCallArg::Float(expr)
            }
            ScalarType::Int {..}=>{
                let expr=ctx.graph.expression(operand).ok_or_else(||
                    reject("INTEGER indirect formal argument missing"))?;
                if let Some(bits)=source_integer_constant(ctx,expr,*target)? {
                    ScalarCallArg::IntegerConstant {bits,ty:*target}
                } else {
                    if super::semantic(2,expr.data.kind)!=2 ||
                        !expr.operands.is_empty() {
                        return Err(reject("INTEGER indirect call INTEGER argument not formal/literal"));
                    }
                    let symbol=expr.data.symbol_id.ok_or_else(||
                        reject("INTEGER indirect argument lacks native symbol"))?;
                    let index=ctx.params.iter().position(|p|*p==symbol).ok_or_else(||
                        reject("INTEGER indirect argument not a checked formal"))?;
                    let ty=scalar_type_from_edg(ctx.il,expr.data.type_id.ok_or_else(||
                        reject("INTEGER indirect argument lacks native type"))?)?;
                    if ty!=*target || ctx.param_types[index]!=*target {
                        return Err(reject("INTEGER indirect argument type mismatch"));
                    }
                    ScalarCallArg::IntegerParameter(index)
                }
            }
            ScalarType::FunctionPointer(_)|ScalarType::DataPointer(_)=>return Err(reject(
                "pointer-valued INTEGER indirect-call argument unsupported")),
        };
        arguments.push(value);
    }
    Ok(FloatStmt::ReturnIndirectInteger {
        pointer,pointer_type,parameters,
        return_type:result,arguments,
    })
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
    // Allow only ordinary, source-classified C file-scope data. Explicit
    // initialization requires a separate native IEEE byte-image event.
    if il.global_variables.iter().any(|g|
        !matches!(g.linkage,1|2|3) ||
        g.thread_local || g.local_to_function) {
        return Err(reject("FP source module has unsupported TLS/local globals"));
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
            let ty=scalar_type_from_edg(il,local.ty)?;
            if !matches!(ty,ScalarType::Float(_)|ScalarType::FunctionPointer(_)|
                ScalarType::DataPointer(_)) {
                return Err(reject("unsupported FP automatic local ABI class"));
            }
            local_types.push(ty);
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
    // Retain ALL owned global definitions, even without a source-local read:
    // external GCC objects can refer to these symbols. Only referenced
    // declarations become imports.
    let mut fp_globals=Vec::new();
    let mut seen=std::collections::HashSet::new();
    for g in &il.global_variables {
        if (g.linkage!=1 || g.explicit_initializer) && seen.insert(g.symbol) {
            let symbol=g.symbol;
            let (name,ty,linkage,initializer)=source_fp_global(il,symbol)?;
            fp_globals.push(FpScalarGlobal {symbol,name,ty,linkage,initializer});
        }
    }
    for expr in &il.expressions {
        if super::semantic(2,expr.kind)!=2 {continue;}
        let Some(symbol)=expr.symbol_id else {continue;};
        if il.global_variables.iter().any(|g|g.symbol==symbol) &&
            seen.insert(symbol) {
            let (name,ty,linkage,initializer)=source_fp_global(il,symbol)?;
            fp_globals.push(FpScalarGlobal {symbol,name,ty,linkage,initializer});
        }
    }
    emit_scalar_float_bodies_with_globals(&functions,true,&fp_globals).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn param(n:usize)->FloatExpr {FloatExpr::Parameter(n)}
    fn bin(a:usize,b:usize)->FloatExpr {
        FloatExpr::Add(Box::new(param(a)),Box::new(param(b)))
    }
    #[test]
    fn m14_ieee_global_data_images_are_exact_and_reject_malformed_definitions() {
        let f=ScalarFloatBody {
            name:"m14_exact_global_test".into(),
            parameters:vec![],
            result:ScalarType::Float(FloatType::F64),
            locals:vec![],
            statements:vec![FloatStmt::Return(FloatExpr::Constant64(0))],
        };
        let bits=0x8000_0000_0000_0000_u64;
        let g=FpScalarGlobal {
            symbol:crate::SymbolId(100),name:"m14_exact_global_data".into(),
            ty:FloatType::F64,linkage:Linkage::Export,
            initializer:Some(bits.to_le_bytes().to_vec()),
        };
        let elf=emit_scalar_float_bodies_with_globals(&[f.clone()],true,&[g.clone()]).unwrap();
        assert_eq!(&elf[..4],b"\x7fELF");
        assert!(elf.windows(8).any(|bytes| bytes==bits.to_le_bytes().as_slice()));
        let mut bad=g.clone();
        bad.initializer=Some(vec![0;4]);
        assert!(emit_scalar_float_bodies_with_globals(&[f.clone()],true,&[bad]).is_err());
        let mut bad=g.clone();
        bad.linkage=Linkage::Import;
        assert!(emit_scalar_float_bodies_with_globals(&[f.clone()],true,&[bad]).is_err());
        let mut bad=g.clone();
        bad.symbol=crate::SymbolId(101);
        assert!(emit_scalar_float_bodies_with_globals(&[f.clone()],true,&[g.clone(),bad]).is_err());
        let zero=FpScalarGlobal {
            symbol:crate::SymbolId(102),name:"m14_exact_zero_global".into(),
            ty:FloatType::F32,linkage:Linkage::Local,initializer:None,
        };
        assert!(emit_scalar_float_bodies_with_globals(&[f],true,&[g,zero]).is_ok());
    }
    #[test]
    fn source_authorized_external_scalar_imports_are_typed_and_restricted() {
        let params=vec![ScalarType::Float(FloatType::F64)];
        let external=FloatExpr::CallExternal {
            callee:"m14_foreign_lib".into(),
            parameters:params.clone(),
            return_type:FloatType::F64,
            arguments:vec![ScalarCallArg::Float(FloatExpr::Parameter(0))],
        };
        let build=|name:&str,expr:FloatExpr|ScalarFloatBody {
            name:name.into(),parameters:params.clone(),
            result:ScalarType::Float(FloatType::F64),
            locals:vec![],statements:vec![FloatStmt::Return(expr)],
        };
        let caller=build("m14_import_caller",external.clone());
        // Public FloatExpr reference API is *never* permission to import.
        assert!(emit_scalar_float_bodies(&[caller.clone()]).is_err());
        let bytes=emit_scalar_float_bodies_internal(&[caller.clone()],true).unwrap();
        assert_eq!(&bytes[..4],b"\x7fELF");
        // Identical multiple call sites share the same exact declaration.
        let same=build("m14_second_caller",external.clone());
        assert!(emit_scalar_float_bodies_internal(&[caller.clone(),same],true).is_ok());
        let mut conflict=external.clone();
        if let FloatExpr::CallExternal {parameters,arguments,..}=&mut conflict {
            *parameters=vec![ScalarType::Float(FloatType::F32)];
            *arguments=vec![ScalarCallArg::Float(FloatExpr::Constant32(0))];
        }
        assert!(emit_scalar_float_bodies_internal(
            &[caller.clone(),build("m14_conflicting_caller",conflict)],true).is_err());
        let defined=build("m14_foreign_lib",FloatExpr::Parameter(0));
        assert!(emit_scalar_float_bodies_internal(
            &[caller.clone(),defined],true).is_err());
        let mut builtin=external.clone();
        if let FloatExpr::CallExternal {callee,..}=&mut builtin {
            *callee="__atomic_fake".into();
        }
        assert!(emit_scalar_float_bodies_internal(
            &[build("m14_builtin_caller",builtin)],true).is_err());
        let mut name=external;
        if let FloatExpr::CallExternal {callee,..}=&mut name {
            *callee="namespace::not_a_C_symbol".into();
        }
        assert!(emit_scalar_float_bodies_internal(
            &[build("m14_non_c_caller",name)],true).is_err());
    }


    #[test]
    fn checked_integer_literals_convert_numerically_and_keep_u64_bits() {
        let cases=[
            (ScalarType::Int {bits:32,signed:true},0x7fff_ffff_u64),
            (ScalarType::Int {bits:32,signed:false},0xffff_ffff_u64),
            (ScalarType::Int {bits:64,signed:true},0x7fff_ffff_ffff_ffff_u64),
            (ScalarType::Int {bits:64,signed:false},u64::MAX),
        ];
        for (index,(source,bits)) in cases.into_iter().enumerate() {
            for to in [FloatType::F32,FloatType::F64] {
                let function=ScalarFloatBody {
                    name:format!("m14_literal_conv_{index}_{to:?}"),
                    parameters:vec![],result:ScalarType::Float(to),
                    locals:vec![],
                    statements:vec![FloatStmt::Return(FloatExpr::IntegerConstantCast {
                        bits,source,to,
                    })],
                };
                let bytes=emit_scalar_float_bodies(&[function.clone()]).unwrap();
                assert_eq!(&bytes[..4],b"\x7fELF");
                let mut bad=function.clone();
                if let FloatStmt::Return(FloatExpr::IntegerConstantCast {source,..})=
                    &mut bad.statements[0] {
                    *source=ScalarType::Float(FloatType::F64);
                }
                assert!(emit_scalar_float_bodies(&[bad]).is_err());
                let mut bad=function;
                if let FloatStmt::Return(FloatExpr::IntegerConstantCast {bits,source,..})=
                    &mut bad.statements[0] {
                    *bits=0x1_0000_0000_u64;
                    *source=ScalarType::Int {bits:32,signed:false};
                }
                assert!(emit_scalar_float_bodies(&[bad]).is_err());
            }
        }
        assert!(checked_integer_literal(
            ScalarType::Int {bits:32,signed:true},0xffff_ffff).is_err());
        assert!(checked_integer_literal(
            ScalarType::Int {bits:8,signed:false},7).is_err());
        let external=ScalarFloatBody {
            name:"m14_integer_literal_external".into(),
            parameters:vec![],result:ScalarType::Float(FloatType::F64),
            locals:vec![],statements:vec![FloatStmt::Return(FloatExpr::CallExternal {
                callee:"m14_import_literal".into(),
                parameters:vec![ScalarType::Int {bits:64,signed:false}],
                return_type:FloatType::F64,
                arguments:vec![ScalarCallArg::IntegerConstant {
                    bits:u64::MAX,ty:ScalarType::Int {bits:64,signed:false},
                }],
            })],
        };
        assert!(emit_scalar_float_bodies_internal(&[external.clone()],true).is_ok());
        assert!(emit_scalar_float_bodies_internal(&[external.clone()],false).is_err());
        let mut bad=external;
        if let FloatStmt::Return(FloatExpr::CallExternal {arguments,..})=
            &mut bad.statements[0] {
            arguments[0]=ScalarCallArg::IntegerConstant {
                bits:u64::MAX,ty:ScalarType::Int {bits:64,signed:true},
            };
        }
        assert!(emit_scalar_float_bodies_internal(&[bad],true).is_err());
    }
    #[test]
    fn indirect_integer_return_keeps_rax_and_rejects_bad_prototypes() {
        let pointer=crate::TypeId(123456);
        let result_types=[
            ScalarType::Int {bits:32,signed:true},
            ScalarType::Int {bits:32,signed:false},
            ScalarType::Int {bits:64,signed:true},
            ScalarType::Int {bits:64,signed:false},
        ];
        for (index,result) in result_types.iter().copied().enumerate() {
            let original=FloatStmt::ReturnIndirectInteger {
                pointer:PointerSource::Parameter(0),
                pointer_type:pointer,
                parameters:vec![ScalarType::Int {bits:32,signed:true},
                                ScalarType::Float(FloatType::F64)],
                return_type:result,
                arguments:vec![ScalarCallArg::IntegerParameter(1),
                               ScalarCallArg::Float(FloatExpr::Parameter(2))],
            };
            let function=ScalarFloatBody {
                name:format!("indirect_integer_result_{index}"),
                parameters:vec![
                    ScalarType::FunctionPointer(pointer),
                    ScalarType::Int {bits:32,signed:true},
                    ScalarType::Float(FloatType::F64),
                ],
                result,locals:vec![],statements:vec![original.clone()],
            };
            let bytes=emit_scalar_float_bodies(&[function.clone()]).unwrap();
            assert_eq!(&bytes[..4],b"\x7fELF");
            let mut bad=function.clone();
            bad.parameters[0]=ScalarType::Int {bits:64,signed:false};
            assert!(emit_scalar_float_bodies(&[bad]).is_err());
            let mut bad=function.clone();
            if let FloatStmt::ReturnIndirectInteger {return_type,..}=
                &mut bad.statements[0] {
                *return_type=ScalarType::Float(FloatType::F64);
            }
            assert!(emit_scalar_float_bodies(&[bad]).is_err());
            let mut bad=function.clone();
            if let FloatStmt::ReturnIndirectInteger {arguments,..}=
                &mut bad.statements[0] {
                *arguments=vec![ScalarCallArg::IntegerParameter(1),
                    ScalarCallArg::Float(FloatExpr::IntegerParameterCast {
                        parameter:0,to:FloatType::F64})];
            }
            assert!(emit_scalar_float_bodies(&[bad]).is_err());
            let mut bad=function.clone();
            if let FloatStmt::ReturnIndirectInteger {pointer,..}=
                &mut bad.statements[0] {
                *pointer=PointerSource::Parameter(1);
            }
            assert!(emit_scalar_float_bodies(&[bad]).is_err());
            let mut bad=function.clone();
            bad.locals.push(ScalarType::Float(FloatType::F64));
            if let FloatStmt::ReturnIndirectInteger {arguments,..}=
                &mut bad.statements[0] {
                arguments[1]=ScalarCallArg::Float(FloatExpr::Local(0));
            }
            assert!(emit_scalar_float_bodies(&[bad]).is_err());
        }
    }

    #[test]
    fn typed_fp_memory_formals_have_exact_abi_and_real_load_store() {
        let ptr=ScalarType::DataPointer(FloatType::F64);
        let load=FloatExpr::LoadPointerParameter {
            parameter:0,ty:FloatType::F64,
        };
        let f=ScalarFloatBody {
            name:"m14_pointer_memory".into(),
            parameters:vec![ptr,ScalarType::Float(FloatType::F64)],
            result:ScalarType::Float(FloatType::F64),locals:vec![],
            statements:vec![
                FloatStmt::StorePointerParameter {
                    parameter:0,ty:FloatType::F64,
                    value:FloatExpr::Parameter(1),
                },
                FloatStmt::Return(load.clone()),
            ],
        };
        let bytes=emit_scalar_float_bodies(&[f.clone()]).unwrap();
        assert_eq!(&bytes[..4],b"\x7fELF");
        let mut wrong=f.clone();
        wrong.parameters[0]=ScalarType::FunctionPointer(crate::TypeId(77));
        assert!(emit_scalar_float_bodies(&[wrong]).is_err());
        let mut wrong=f.clone();
        wrong.parameters[0]=ScalarType::Int{bits:64,signed:false};
        assert!(emit_scalar_float_bodies(&[wrong]).is_err());
        let mut wrong=f.clone();
        wrong.statements=vec![FloatStmt::Return(FloatExpr::LoadPointerParameter {
            parameter:1,ty:FloatType::F64,
        })];
        assert!(emit_scalar_float_bodies(&[wrong]).is_err());
        let mut wrong=f.clone();
        if let FloatStmt::StorePointerParameter {ty,..}=&mut wrong.statements[0] {
            *ty=FloatType::F32;
        }
        assert!(emit_scalar_float_bodies(&[wrong]).is_err());
        let mut wrong=f.clone();
        if let FloatStmt::StorePointerParameter {value,..}=&mut wrong.statements[0] {
            *value=FloatExpr::Constant32(1.0f32.to_bits());
        }
        assert!(emit_scalar_float_bodies(&[wrong]).is_err());
    }

    #[test]
    fn native_fp_ternary_value_is_typed_lazy_and_checks_both_arms() {
        let cond=FloatCondition {
            op:FloatPredicate::Ne,left:FloatExpr::Parameter(0),
            right:FloatExpr::Constant64(0.0f64.to_bits()),
        };
        let choose=FloatExpr::Conditional {
            condition:Box::new(cond.clone()),
            yes:Box::new(FloatExpr::Parameter(1)),
            no:Box::new(FloatExpr::Parameter(2)),
        };
        let build=|expr:FloatExpr|ScalarFloatBody {
            name:"m14_typed_conditional".into(),
            parameters:vec![ScalarType::Float(FloatType::F64);3],
            result:ScalarType::Float(FloatType::F64),
            locals:vec![],statements:vec![FloatStmt::Return(expr)],
        };
        let bytes=emit_scalar_float_bodies(&[build(choose.clone())]).unwrap();
        assert_eq!(&bytes[..4],b"\x7fELF");
        let mut mismatched=choose.clone();
        if let FloatExpr::Conditional {no,..}=&mut mismatched {
            *no=Box::new(FloatExpr::Constant32(0.0f32.to_bits()));
        }
        assert!(emit_scalar_float_bodies(&[build(mismatched)]).is_err());
        let mut bad_condition=choose.clone();
        if let FloatExpr::Conditional {condition,..}=&mut bad_condition {
            condition.right=FloatExpr::Constant32(0.0f32.to_bits());
        }
        assert!(emit_scalar_float_bodies(&[build(bad_condition)]).is_err());
        let mut bad_call=choose;
        if let FloatExpr::Conditional {yes,..}=&mut bad_call {
            *yes=Box::new(FloatExpr::Call {callee:"absent".into(),
                parameters:vec![],return_type:FloatType::F64,arguments:vec![]});
        }
        // Even a dynamically untaken arm must have a verified call target.
        assert!(emit_scalar_float_bodies(&[build(bad_call)]).is_err());
    }

    #[test]
    fn typed_callable_pointer_locals_propagate_ssa_and_validate_definite_assignment() {
        let ptr=crate::TypeId(4096);
        let call=FloatExpr::CallIndirect {
            pointer:PointerSource::Local(0),pointer_type:ptr,
            parameters:vec![ScalarType::Float(FloatType::F64)],
            return_type:FloatType::F64,
            arguments:vec![ScalarCallArg::Float(FloatExpr::Parameter(1))],
        };
        let function=ScalarFloatBody {
            name:"m14_pointer_ssa_local".into(),
            parameters:vec![ScalarType::FunctionPointer(ptr),
                            ScalarType::Float(FloatType::F64)],
            result:ScalarType::Float(FloatType::F64),
            locals:vec![ScalarType::FunctionPointer(ptr)],
            statements:vec![
                FloatStmt::AssignPointer {
                    local:0,pointer_type:ptr,source:PointerSource::Parameter(0),
                },
                FloatStmt::Return(call.clone()),
            ],
        };
        let object=emit_scalar_float_bodies(&[function.clone()]).unwrap();
        assert_eq!(&object[..4],b"\x7fELF");
        let mut bad=function.clone();
        bad.statements=vec![FloatStmt::Return(call.clone())];
        assert!(emit_scalar_float_bodies(&[bad]).is_err());
        let mut bad=function.clone();
        bad.statements=vec![
            FloatStmt::AssignPointer {
                local:0,pointer_type:ptr,source:PointerSource::Local(0),
            },
            FloatStmt::Return(call.clone()),
        ];
        assert!(emit_scalar_float_bodies(&[bad]).is_err());
        let mut bad=function.clone();
        bad.locals[0]=ScalarType::Float(FloatType::F64);
        assert!(emit_scalar_float_bodies(&[bad]).is_err());
        let mut bad=function.clone();
        bad.statements=vec![
            FloatStmt::AssignPointer {
                local:0,pointer_type:crate::TypeId(4097),
                source:PointerSource::Parameter(0),
            },
            FloatStmt::Return(call.clone()),
        ];
        assert!(emit_scalar_float_bodies(&[bad]).is_err());
        let cond=FloatCondition {
            op:FloatPredicate::Gt,
            left:FloatExpr::Parameter(1),
            right:FloatExpr::Constant64(0.0f64.to_bits()),
        };
        let mut bad=function.clone();
        bad.statements=vec![
            FloatStmt::IfBlock(cond.clone(),vec![FloatStmt::AssignPointer {
                local:0,pointer_type:ptr,source:PointerSource::Parameter(0),
            }],vec![]),
            FloatStmt::Return(call.clone()),
        ];
        assert!(emit_scalar_float_bodies(&[bad]).is_err());
        let mut valid=function.clone();
        valid.statements=vec![
            FloatStmt::IfBlock(cond,vec![FloatStmt::AssignPointer {
                local:0,pointer_type:ptr,source:PointerSource::Parameter(0),
            }],vec![FloatStmt::AssignPointer {
                local:0,pointer_type:ptr,source:PointerSource::Parameter(0),
            }]),
            FloatStmt::Return(call),
        ];
        assert!(emit_scalar_float_bodies(&[valid]).is_ok());
    }

    #[test]
    fn function_pointer_formal_indirect_call_checks_abi_classes_and_target_identity() {
        let ptr=crate::TypeId(987);
        let signature=vec![
            ScalarType::FunctionPointer(ptr),
            ScalarType::Int {bits:32,signed:true},
            ScalarType::Float(FloatType::F64),
        ];
        let call=FloatExpr::CallIndirect {
            pointer:PointerSource::Parameter(0),pointer_type:ptr,
            parameters:vec![
                ScalarType::Int {bits:32,signed:true},
                ScalarType::Float(FloatType::F64),
            ],
            return_type:FloatType::F64,
            arguments:vec![
                ScalarCallArg::IntegerParameter(1),
                ScalarCallArg::Float(FloatExpr::Parameter(2)),
            ],
        };
        let good=ScalarFloatBody {
            name:"m14_function_pointer_formal".into(),
            parameters:signature.clone(),result:ScalarType::Float(FloatType::F64),
            locals:vec![],statements:vec![FloatStmt::Return(call.clone())],
        };
        let elf=emit_scalar_float_bodies(&[good.clone()]).unwrap();
        assert_eq!(&elf[..4],b"\x7fELF");
        // A raw integer, another pointer type, an FP parameter, and
        // swapped argument classes may never stand in for this callee.
        let mut bad=good.clone();
        bad.parameters[0]=ScalarType::Int {bits:64,signed:false};
        assert!(emit_scalar_float_bodies(&[bad]).is_err());
        let mut bad=good.clone();
        bad.parameters[0]=ScalarType::FunctionPointer(crate::TypeId(988));
        assert!(emit_scalar_float_bodies(&[bad]).is_err());
        let mut bad=good.clone();
        bad.parameters[0]=ScalarType::Float(FloatType::F64);
        assert!(emit_scalar_float_bodies(&[bad]).is_err());
        let mut bad=good.clone();
        let mut wrong=call.clone();
        if let FloatExpr::CallIndirect {arguments,..}=&mut wrong {
            *arguments=vec![
                ScalarCallArg::Float(FloatExpr::Parameter(2)),
                ScalarCallArg::IntegerParameter(1),
            ];
        }
        bad.statements=vec![FloatStmt::Return(wrong)];
        assert!(emit_scalar_float_bodies(&[bad]).is_err());
        let mut bad=good;
        let mut wrong=call;
        if let FloatExpr::CallIndirect {pointer,..}=&mut wrong {
            *pointer=PointerSource::Parameter(2);
        }
        bad.statements=vec![FloatStmt::Return(wrong)];
        assert!(emit_scalar_float_bodies(&[bad]).is_err());
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
        let mut unknown=call.clone();
        if let FloatExpr::Call {callee,..}=&mut unknown {
            *callee="missing".into();
        }
        bad.statements=vec![FloatStmt::Return(unknown)];
        assert!(emit_scalar_float_bodies(&[bad,base.clone()]).is_err());
        let mut bad=caller.clone();
        let mut swapped=call.clone();
        if let FloatExpr::Call {parameters,..}=&mut swapped {
            *parameters=vec![ScalarType::Float(FloatType::F64),
                ScalarType::Int {bits:32,signed:true}];
        }
        bad.statements=vec![FloatStmt::Return(swapped)];
        assert!(emit_scalar_float_bodies(&[bad,base.clone()]).is_err());
        let mut bad=caller;
        let mut mistyped=call.clone();
        if let FloatExpr::Call {arguments,..}=&mut mistyped {
            *arguments=vec![ScalarCallArg::IntegerParameter(0),
                ScalarCallArg::IntegerParameter(0)];
        }
        bad.statements=vec![FloatStmt::Return(mistyped)];
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
