//! System V AMD64 C ABI subset. No AArch64 ABI rules belong here.
use super::*;
pub(super) struct SysV;

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

    fn machine_parameters(&self, abi: &CAbi) -> Vec<CAbiInt> {
        let mut params = Vec::new();
        for (ty, aggregate) in abi.parameters.iter().zip(&abi.aggregate_parameters) {
            params.push(*ty);
            if aggregate.is_some_and(|bytes| bytes > 8) {
                params.push(CAbiInt { bits:64, signed:false, is_bool:false });
            }
        }
        params
    }

    fn validate_register_budget(&self, abi: &CAbi) -> Result<(), CodegenError> {
        let mut consumed = 0usize;
        for aggregate in &abi.aggregate_parameters {
            let registers = if aggregate.is_some_and(|bytes| bytes > 8) {2} else {1};
            if registers == 2 && consumed > 4 {
                return Err(CodegenError::InvalidFunction(
                    "two-GPR aggregate with exhausted SysV registers unsupported".into()));
            }
            consumed += registers;
        }
        Ok(())
    }

    fn validate_aggregate_return(&self, aggregate: Option<u8>) -> Result<(), CodegenError> {
        if aggregate.is_some_and(|bytes| bytes > 8) {
            return Err(CodegenError::InvalidFunction(
                "two-GPR aggregate returns not yet supported".into()));
        }
        Ok(())
    }
}
