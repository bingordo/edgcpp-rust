//! System V AMD64 C ABI subset. No AArch64 ABI rules belong here.
use super::*;
pub(in crate::codegen) struct SysV;

type Value = cranelift_codegen::ir::Value;

/// Little-endian SysV integer-eightbyte packing. Never touch object padding
/// beyond the native size, and evaluate the operand address only once.
fn pack_bytes(fb:&mut FunctionBuilder<'_>, address:Value,
              start:u8, count:u8) -> Value {
    let mut packed=fb.ins().iconst(types::I64,0);
    for i in 0..count {
        let offset=i64::from(start)+i64::from(i);
        let loc=if offset==0 {address} else {fb.ins().iadd_imm(address,offset)};
        let byte=fb.ins().load(types::I8,cranelift_codegen::ir::MemFlags::new(),loc,0);
        let wide=fb.ins().uextend(types::I64,byte);
        let shifted=if i==0 {wide} else {
            let shift=fb.ins().iconst(types::I64,i64::from(i)*8);
            fb.ins().ishl(wide,shift)
        };
        packed=fb.ins().bor(packed,shifted);
    }
    packed
}

impl TargetAbi for SysV {
    fn isa_name(&self) -> &'static str { "x86_64" }

    /// Plain integer-only aggregates in one or two integer eightbytes.
    /// Packed, SSE, MEMORY and other unsupported cases fail closed.
    /// Conservative SysV AMD64 INTEGER-class classification. Plain, naturally
    /// aligned C structs of at most two eightbytes can use GPR arguments.
    /// Floating, packed, nested, bitfield, union and MEMORY cases fail closed.
    /// Two-GPR results remain deliberately unsupported until multi-value return
    /// lowering is implemented and GCC-differential-tested.
    fn classify_aggregate(&self, il:&Il, ty:crate::TypeId) ->Result<Option<u8>,CodegenError> {
        let ty=canonical_abi_type(il,ty)?;
        let Some(def)=il.types.iter().find(|t|t.id==ty) else {
            return Err(CodegenError::InvalidFunction("missing aggregate ABI type".into()));
        };
        if semantic(4,def.kind)!=7 {return Ok(None);}
        let Some(size)=il.type_object_sizes.iter().find(|t|t.owner==ty) else {
            return Err(CodegenError::InvalidFunction("missing native aggregate size/alignment".into()));
        };
        if size.bytes==0 || size.bytes>16 || size.alignment==0 ||
           !size.alignment.is_power_of_two() || size.alignment>8 {
            return Err(CodegenError::InvalidFunction(
                "unsupported SysV aggregate width/alignment".into()));
        }
        let mut fields:Vec<_>=il.record_fields.iter().filter(|f|f.owner==ty).collect();
        fields.sort_by_key(|f|f.position);
        if fields.is_empty(){return Err(CodegenError::InvalidFunction(
            "empty aggregate ABI unsupported".into()));}
        let mut end=0u64;
        let mut max_align=1u64;
        for (i,field) in fields.iter().enumerate() {
            if field.position!=i as u32 || field.name.is_empty() ||
                field.bit_width!=0 || field.bit_offset!=0 {
                return Err(CodegenError::InvalidFunction(
                    "bitfield/anonymous aggregate ABI unsupported".into()));
            }
            let ty=canonical_abi_type(il,field.ty)?;
            let def=il.types.iter().find(|t|t.id==ty).ok_or_else(||
                CodegenError::InvalidFunction("aggregate field missing type".into()))?;
            let width=match semantic(4,def.kind) {
                1 => {
                    let info=il.c_integer_types.iter().find(|t|t.id==ty)
                        .ok_or_else(||CodegenError::InvalidFunction(
                            "aggregate integer field lacks width".into()))?;
                    if !matches!(info.bits,8|16|32|64) {
                        return Err(CodegenError::InvalidFunction(
                            "unsupported aggregate integer width".into()));
                    }
                    u64::from(info.bits/8)
                }
                5 => 8,
                _ => return Err(CodegenError::InvalidFunction(
                    "non-INTEGER SysV aggregate member unsupported".into())),
            };
            if field.byte_offset%width!=0 || field.byte_offset<end {
                return Err(CodegenError::InvalidFunction(
                    "unaligned/overlapping aggregate ABI member".into()));
            }
            max_align=max_align.max(width);
            end=field.byte_offset.checked_add(width)
                .ok_or_else(||CodegenError::InvalidFunction(
                    "aggregate field offset overflow".into()))?;
            if end>size.bytes {
                return Err(CodegenError::InvalidFunction(
                    "aggregate member exceeds native size".into()));
            }
        }
        if size.alignment!=max_align || size.bytes%size.alignment!=0 {
            return Err(CodegenError::InvalidFunction(
                "aggregate ABI native alignment mismatch".into()));
        }
        Ok(Some(size.bytes as u8))
    }

    fn passing_plan(&self, abi: &CAbi) -> Result<PassingPlan, CodegenError> {
        if abi.parameters.len()!=abi.aggregate_parameters.len() {
            return Err(CodegenError::InvalidFunction(
                "source argument and aggregate ABI counts differ".into()));
        }
        let mut gpr=0u8;
        let mut stack_slot=0u32;
        let mut parameters=Vec::with_capacity(abi.parameters.len());
        let integer=|index| PieceLocation::Register {
            class:RegisterClass::Integer,index
        };
        for (source_index,(&ty,&aggregate)) in abi.parameters.iter()
            .zip(&abi.aggregate_parameters).enumerate() {
            let (mode,pieces)=if let Some(bytes)=aggregate {
                if bytes==0 || bytes>16 || ty.bits!=64 {
                    return Err(CodegenError::InvalidFunction(
                        "invalid SysV INTEGER aggregate descriptor".into()));
                }
                if bytes>8 {
                    // SysV never splits one aggregate between GPR and stack.
                    // A full stack-by-value lowering is not yet implemented.
                    if gpr>4 {
                        return Err(CodegenError::InvalidFunction(
                            "two-GPR aggregate with exhausted SysV registers unsupported".into()));
                    }
                    let first=MachinePiece {
                        ty,object_offset:0,object_bytes:8,location:integer(gpr)
                    };
                    let second=MachinePiece {
                        ty,object_offset:8,object_bytes:bytes-8,
                        location:integer(gpr+1)
                    };
                    gpr+=2;
                    (PassingMode::AggregateByValue{bytes},vec![first,second])
                } else {
                    let location=if gpr<6 {
                        let reg=integer(gpr);
                        gpr+=1;
                        reg
                    } else {
                        let slot=stack_slot;
                        stack_slot+=1;
                        PieceLocation::Stack{slot}
                    };
                    (PassingMode::AggregateByValue{bytes},vec![MachinePiece {
                        ty,object_offset:0,object_bytes:bytes,location
                    }])
                }
            } else {
                if !matches!(ty.bits,8|16|32|64) {
                    return Err(CodegenError::InvalidFunction(
                        "unsupported scalar machine width".into()));
                }
                let location=if gpr<6 {
                    let reg=integer(gpr);
                    gpr+=1;
                    reg
                } else {
                    let slot=stack_slot;
                    stack_slot+=1;
                    PieceLocation::Stack{slot}
                };
                (PassingMode::Scalar,vec![MachinePiece {
                    ty,object_offset:0,object_bytes:ty.bits/8,location
                }])
            };
            parameters.push(ParameterPlan{source_index,mode,pieces});
        }
        self.validate_aggregate_return(abi.aggregate_result)?;
        let result=if let Some(ty)=abi.result {
            vec![MachinePiece{
                ty,object_offset:0,
                object_bytes:abi.aggregate_result.unwrap_or(ty.bits/8),
                location:integer(0)
            }]
        } else {Vec::new()};
        Ok(PassingPlan{parameters,result})
    }

    fn validate_aggregate_return(&self, aggregate: Option<u8>) -> Result<(), CodegenError> {
        if aggregate.is_some_and(|bytes| bytes > 8) {
            return Err(CodegenError::InvalidFunction(
                "two-GPR aggregate returns not yet supported".into()));
        }
        Ok(())
    }

    fn lower_aggregate_argument(&self, size:u8, address:Expr) -> Expr {
        if size > 8 {address} else {Expr::PackAggregate(size,Box::new(address))}
    }

    fn pack_argument(
        &self, fb:&mut FunctionBuilder<'_>, address:Value, size:u8,
    ) -> Result<Vec<Value>,CodegenError> {
        if size==0 || size>16 {
            return Err(CodegenError::InvalidFunction(
                "invalid SysV aggregate argument width".into()));
        }
        let mut values=vec![pack_bytes(fb,address,0,size.min(8))];
        if size>8 {values.push(pack_bytes(fb,address,8,size-8));}
        Ok(values)
    }

    fn pack_result(
        &self, fb:&mut FunctionBuilder<'_>, address:Value, size:u8,
    ) -> Result<Value,CodegenError> {
        if size==0 || size>8 {
            return Err(CodegenError::InvalidFunction(
                "invalid one-register aggregate return width".into()));
        }
        Ok(pack_bytes(fb,address,0,size))
    }

    fn decode_parameters(
        &self, fb:&mut FunctionBuilder<'_>, abi:&CAbi, machine_values:&[Value],
    ) -> Result<(Vec<Value>,Vec<Option<Value>>),CodegenError> {
        let mut physical=machine_values.iter().copied();
        let mut params=Vec::with_capacity(abi.parameters.len());
        let mut second_parts=Vec::with_capacity(abi.parameters.len());
        for (ty,aggregate) in abi.parameters.iter().zip(&abi.aggregate_parameters) {
            let value=physical.next().ok_or_else(||
                CodegenError::InvalidFunction("missing machine argument".into()))?;
            params.push(if ty.bits==64 {value}
                else if ty.signed {fb.ins().sextend(types::I64,value)}
                else {fb.ins().uextend(types::I64,value)});
            second_parts.push(if aggregate.is_some_and(|bytes|bytes>8) {
                Some(physical.next().ok_or_else(||
                    CodegenError::InvalidFunction("missing second aggregate GPR".into()))?)
            } else {None});
        }
        if physical.next().is_some() {
            return Err(CodegenError::InvalidFunction("extra machine arguments".into()));
        }
        Ok((params,second_parts))
    }

    fn materialize_aggregate(
        &self, fb:&mut FunctionBuilder<'_>, address:Value, size:u8,
        first:Value, second:Option<Value>,
    ) -> Result<(),CodegenError> {
        if size==0 || size>16 {
            return Err(CodegenError::InvalidFunction(
                "invalid SysV aggregate parameter width".into()));
        }
        for offset in 0..size {
            let register=if offset<8 {first} else {
                second.ok_or_else(||CodegenError::InvalidFunction(
                    "missing second aggregate GPR".into()))?
            };
            let shift_bytes=offset%8;
            let part=if shift_bytes==0 {register} else {
                let shift=fb.ins().iconst(types::I64,i64::from(shift_bytes)*8);
                fb.ins().ushr(register,shift)
            };
            let byte=fb.ins().ireduce(types::I8,part);
            let loc=if offset==0 {address} else {
                fb.ins().iadd_imm(address,i64::from(offset))
            };
            fb.ins().store(cranelift_codegen::ir::MemFlags::new(),byte,loc,0);
        }
        Ok(())
    }
}
