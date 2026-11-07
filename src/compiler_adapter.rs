//! Direct EDG -> bingordo/compiler SSA integration pilot.
//!
//! This is intentionally an *adapter*, not a new source-projection IR.
//! Native EDG identities/types are checked at their point of use and each
//! accepted expression immediately constructs compiler_ir::Op values.
//! The production compiler and its IR do not depend on EDG or Cranelift.
use crate::{graph::{ExpressionId,Graph,StatementId},Entity,Il,SymbolId,TypeId};
use compiler_ir::{
    Block,BlockId,Function,Instruction,IntBinOp,Op,Parameter,Terminator,
    Type,ValueId,
};
use compiler_backend::Backend;
use compiler_backend_x86::X86_64;

extern "C" {
    fn edgcpp_codegen_kind(category:u32,value:u32)->u32;
}
fn semantic(category:u32,value:u32)->u32 {
    // The bridge owns the mapping from EDG's native enum discriminants.
    unsafe {edgcpp_codegen_kind(category,value)}
}
fn reject(message:impl Into<String>)->String {message.into()}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
struct CInteger {ty:Type,bits:u8,signed:bool}

fn source_integer(il:&Il,id:TypeId)->Result<CInteger,String> {
    // Do not erase source qualifiers, typerefs or enum ABI classes in this
    // initial pilot; a later checked C type normalizer can broaden this.
    let def=il.types.iter().find(|d|d.id==id)
        .ok_or_else(||reject("EDG integer type definition missing"))?;
    if semantic(4,def.kind)!=1 {
        return Err(reject("EDG integer source type is not a direct native integer"));
    }
    let info=il.c_integer_types.iter().find(|t|t.id==id)
        .ok_or_else(||reject("EDG native signedness/width metadata missing"))?;
    if info.is_bool || !matches!(info.bits,32|64) {
        return Err(reject("EDG compiler adapter supports only plain I32/I64 integer types"));
    }
    let layout=il.type_object_sizes.iter().find(|v|v.owner==id)
        .ok_or_else(||reject("EDG integer has no target data layout"))?;
    if layout.bytes!=u64::from(info.bits/8) ||
        layout.alignment!=u64::from(info.bits/8) {
        return Err(reject("EDG integer layout differs from pinned x86-64 target"));
    }
    Ok(CInteger {
        ty:if info.bits==32 {Type::I32} else {Type::I64},
        bits:info.bits,signed:info.signed,
    })
}
struct Builder<'a> {
    il:&'a Il,
    graph:&'a Graph,
    params:Vec<(SymbolId,CInteger,ValueId)>,
    result:CInteger,
    instructions:Vec<Instruction>,
    next:u32,
}
impl Builder<'_> {
    fn alloc(&mut self,op:Op)->Result<ValueId,String> {
        if self.instructions.len()>=4096 {return Err(reject("EDG native IR pilot instruction budget exceeded"));}
        let id=ValueId(self.next);
        self.next=self.next.checked_add(1).ok_or("SSA value ID overflow")?;
        let ty=op.result_type();
        self.instructions.push(Instruction {id,ty,op});
        Ok(id)
    }
    fn expression(&mut self,id:ExpressionId,depth:usize)
        ->Result<(ValueId,CInteger),String> {
        if depth>64 {return Err(reject("EDG integer expression recursion limit"));}
        let node=self.graph.expression(id)
            .ok_or_else(||reject("missing native EDG expression"))?;
        let data=&node.data;
        let native=source_integer(self.il,data.type_id
            .ok_or("native expression missing exact C type ID")?)?;
        let path=format!("{}:{}:{}",data.function,data.statement_path,data.path);
        match semantic(2,data.kind) {
            1 if node.operands.is_empty() && data.symbol_id.is_none() => {
                let n=data.constant_integer.ok_or_else(||
                    reject(format!("{path}: no lossless signed 64-bit integer source image")))?;
                match (native.bits,native.signed) {
                    (32,true) if (i32::MIN as i64..=i32::MAX as i64).contains(&n)=>{},
                    (32,false) if (0..=u32::MAX as i64).contains(&n)=>{},
                    (64,true)=>{},
                    // Large U64 constants may not be representable in the
                    // signed EDG integer carrier. Reject, don't truncate.
                    (64,false) if n>=0=>{},
                    _=>return Err(reject(format!("{path}: integer literal image exceeds native type"))),
                }
                let op=if native.bits==32 {Op::ConstI32(n as i32)}
                    else {Op::ConstI64(n)};
                Ok((self.alloc(op)?,native))
            }
            2 if node.operands.is_empty() && data.constant_kind.is_none() => {
                let symbol=data.symbol_id.ok_or_else(||
                    reject(format!("{path}: unbound C variable")))?;
                let (_,expected,id)=self.params.iter().find(|(s,_,_)|*s==symbol)
                    .ok_or_else(||reject(format!("{path}: automatic/global C variable needs storage lowering")))?;
                if native!=*expected {
                    return Err(reject(format!("{path}: C parameter type changed without explicit conversion")));
                }
                Ok((*id,native))
            }
            3 => {
                let op=data.operator.map(|v|semantic(3,v)).unwrap_or(0);
                if matches!(op,11|12) && node.operands.len()==1 {
                    let (v,ty)=self.expression(node.operands[0],depth+1)?;
                    if ty!=native {return Err(reject(format!("{path}: integer promotion or conversion not modeled")));}
                    return Ok((v,ty));
                }
                if op==4 && node.operands.len()==1 {
                    let (v,ty)=self.expression(node.operands[0],depth+1)?;
                    if ty!=native {return Err(reject(format!("{path}: integer unary promotion not modeled")));}
                    let zero=if native.bits==32 {Op::ConstI32(0)} else {Op::ConstI64(0)};
                    let z=self.alloc(zero)?;
                    return Ok((self.alloc(Op::IntBin {
                        kind:IntBinOp::Sub,bits:native.bits,lhs:z,rhs:v,
                    })?,native));
                }
                if matches!(op,1..=3) && node.operands.len()==2 {
                    let (lhs,lt)=self.expression(node.operands[0],depth+1)?;
                    let (rhs,rt)=self.expression(node.operands[1],depth+1)?;
                    // Usual arithmetic conversions beyond identical
                    // native C integer types need explicit source semantics.
                    if lt!=native || rt!=native {
                        return Err(reject(format!("{path}: mixed-type C arithmetic needs checked conversion")));
                    }
                    let kind=match op {1=>IntBinOp::Add,2=>IntBinOp::Sub,
                        3=>IntBinOp::Mul,_=>unreachable!()};
                    return Ok((self.alloc(Op::IntBin {
                        kind,bits:native.bits,lhs,rhs,
                    })?,native));
                }
                Err(reject(format!("{path}: unsupported EDG C operation {op}; no guessed SSA semantics")))
            }
            _=>Err(reject(format!("{path}: unsupported C expression kind; no code emitted"))),
        }
    }
}
fn sole_return(graph:&Graph,id:StatementId,depth:usize)->Result<ExpressionId,String>{
    if depth>32 {return Err(reject("native C statement nesting limit"));}
    let node=graph.statement(id).ok_or("missing native C statement")?;
    match semantic(1,node.data.kind) {
        1 if node.children.is_empty()=>node.expression.ok_or_else(||
            reject("native pilot requires a value return")),
        2 if node.expression.is_none() && node.children.len()==1=>
            sole_return(graph,node.children[0],depth+1),
        _=>Err(reject(format!("{}:{}: only a side-effect-free return is supported by the initial native pilot",
            node.data.function,node.data.path))),
    }
}
fn function(il:&Il,graph:&Graph,name:&str)->Result<Function,String> {
    if name.is_empty() || name.len()>1024 ||
        !name.bytes().next().is_some_and(|b|b.is_ascii_alphabetic()||b==b'_') ||
        !name.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'_') {
        return Err(reject("native pilot requires an unmangled C identifier"));
    }
    let routine=graph.routine_for_function(name)
        .ok_or_else(||reject(format!("{name}: missing EDG routine identity")))?;
    if il.local_variables.iter().any(|v|v.routine==routine) {
        return Err(reject(format!("{name}: automatic locals need subsequent checked SSA lowering")));
    }
    let ty=il.symbol_types.iter().find(|s|s.symbol==routine)
        .ok_or_else(||reject(format!("{name}: no source function type")))?;
    let def=il.types.iter().find(|d|d.id==ty.ty)
        .ok_or_else(||reject(format!("{name}: missing function type")))?;
    if semantic(4,def.kind)!=2 || def.related_role!=Some(3) {
        return Err(reject(format!("{name}: not a direct C function signature")));
    }
    let sig=il.function_signatures.iter().find(|s|s.owner==ty.ty)
        .ok_or_else(||reject(format!("{name}: missing native C prototype")))?;
    if !sig.prototyped || sig.variadic {
        return Err(reject(format!("{name}: variadic/unprototyped C ABI unsupported")));
    }
    let result=source_integer(il,def.related.ok_or("routine has no return type")?)?;
    let mut formals:Vec<_>=il.function_parameters.iter()
        .filter(|p|p.routine==routine).collect();
    formals.sort_by_key(|p|p.position);
    let mut declared:Vec<_>=il.type_parameters.iter()
        .filter(|p|p.owner==ty.ty).collect();
    declared.sort_by_key(|p|p.position);
    if declared.len()!=formals.len() || formals.len()>16 {
        return Err(reject(format!("{name}: C formal count/ABI mismatch")));
    }
    let mut params=Vec::with_capacity(formals.len());
    for (i,(actual,formal)) in declared.iter().zip(formals).enumerate() {
        if actual.position!=i as u32 || formal.position!=Some(i as u32) ||
            actual.ty!=formal.ty || formal.compiler_generated || formal.implicit_this {
            return Err(reject(format!("{name}: C parameter source type/order mismatch")));
        }
        params.push((formal.symbol,source_integer(il,actual.ty)?,ValueId(i as u32)));
    }
    let roots:Vec<_>=graph.roots(name).map(|(id,_)|id).collect();
    if roots.len()!=1 {
        return Err(reject(format!("{name}: expected one native EDG source root")));
    }
    let returned=sole_return(graph,roots[0],0)?;
    let mut lower=Builder {
        il,graph,params,result,instructions:Vec::new(),next:declared.len() as u32,
    };
    let (value,ty)=lower.expression(returned,0)?;
    if ty!=lower.result {
        return Err(reject(format!("{name}: C return requires unsupported implicit conversion")));
    }
    let f=Function {
        name:name.to_owned(),
        blocks:vec![Block {
            id:BlockId(0),
            params:lower.params.iter().map(|(_,ty,id)|
                Parameter {id:*id,ty:ty.ty}).collect(),
            instructions:lower.instructions,
            end:Terminator::Return(value),
        }],
    };
    compiler_ir::verify(&f).map_err(|e|format!("{name}: compiler-ir verifier: {e}"))?;
    Ok(f)
}
/// Validate and lower directly into the production compiler's existing
/// SSA representation. No projection IR, Cranelift types or extra CFG.
pub fn lower_edg_to_compiler_ir(il:&Il)->Result<Vec<Function>,String> {
    let graph=Graph::from_il(il).map_err(|e|format!("EDG owned graph: {e:?}"))?;
    if !il.calls.is_empty() || !il.global_variables.is_empty() {
        return Err(reject("native pilot has no general C call/global semantics yet"));
    }
    let definitions:Vec<_>=il.entities.iter().filter_map(|e|match e {
        Entity::Routine{name,has_definition:true}=>Some(name.as_str()),
        _=>None,
    }).collect();
    if definitions.is_empty() {return Err(reject("no defined EDG C functions"));}
    if definitions.len()>256 {return Err(reject("native pilot function count limit"));}
    let mut functions=Vec::with_capacity(definitions.len());
    for name in definitions { functions.push(function(il,&graph,name)?); }
    compiler_ir::verify_module(&functions)
        .map_err(|e|format!("native compiler-ir module verifier: {e}"))?;
    Ok(functions)
}
/// The production x86 backend emits a native object from *compiler IR*.
/// There is no Cranelift invocation at any point in this adapter.
pub fn emit_edg_native_object(il:&Il)->Result<Vec<u8>,String>{
    let functions=lower_edg_to_compiler_ir(il)?;
    X86_64.emit_object(&functions)
        .map_err(|e|format!("compiler x86 backend: {e}"))
}
