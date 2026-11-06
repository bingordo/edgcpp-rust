//! Backend-independent source-to-machine plan for the currently supported
//! Linux x86-64 SysV C scalar ABI. This is *not* a C parser, pointer-provenance
//! checker, aggregate classifier or replacement for EDG native type metadata.
//! Callers must verify the complete source C prototype before constructing it.
//!
//! The plan is shared between Cranelift FP lowering and the INTEGER CFG path.
//! It keeps source argument order separate from physical GPR/SSE allocation:
//! the x86-64 SysV target owns six integer registers (RDI..R9), eight SSE
//! registers (XMM0..XMM7), and 8-byte stack slots in source spill order.
#[derive(Debug,Clone,Copy,PartialEq,Eq,Hash)]
pub enum ScalarKind {
    Signed32,
    Unsigned32,
    Signed64,
    Unsigned64,
    Float32,
    Float64,
    /// A native 64-bit pointer whose callable prototype and provenance
    /// have *already* been verified by the source frontend.
    CallablePointer,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Hash)]
pub enum RegisterClass { Integer, Sse }
#[derive(Debug,Clone,Copy,PartialEq,Eq,Hash)]
pub enum Location {
    Register {class:RegisterClass,index:u8},
    /// Zero-based 8-byte slot in the *stack argument area*. This is not
    /// an absolute offset from RSP or RBP: the target backend owns frames.
    Stack {slot:u32},
}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Hash)]
pub struct ParameterLocation {
    pub source_index:usize,
    pub source_type:ScalarKind,
    pub location:Location,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct PassingPlan {
    pub parameters:Vec<ParameterLocation>,
    pub result:Location,
    pub result_type:ScalarKind,
    pub outgoing_stack_slots:u32,
    pub used_integer_registers:u8,
    pub used_sse_registers:u8,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct Signature {
    pub parameters:Vec<ScalarKind>,
    pub result:ScalarKind,
    /// Native EDG metadata is authoritative. A C11 unspecified-argument
    /// prototype is never reinterpreted as the typed function's ABI.
    pub prototyped:bool,
    pub variadic:bool,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum PlanError {
    UnprototypedOrVariadic,
    OversizedSignature,
}
impl ScalarKind {
    pub const fn class(self)->RegisterClass {
        match self {
            Self::Float32|Self::Float64=>RegisterClass::Sse,
            Self::Signed32|Self::Unsigned32|
            Self::Signed64|Self::Unsigned64|Self::CallablePointer=>
                RegisterClass::Integer,
        }
    }
    pub const fn bits(self)->u8 {
        match self {
            Self::Signed32|Self::Unsigned32|Self::Float32=>32,
            _=>64,
        }
    }
}
impl Signature {
    pub fn plan_sysv_x86_64(&self)->Result<PassingPlan,PlanError> {
        if !self.prototyped || self.variadic {
            return Err(PlanError::UnprototypedOrVariadic);
        }
        if self.parameters.len()>256 {
            return Err(PlanError::OversizedSignature);
        }
        let mut gpr=0u8;
        let mut xmm=0u8;
        let mut stack=0u32;
        let mut params=Vec::with_capacity(self.parameters.len());
        for (source_index,source_type) in
            self.parameters.iter().copied().enumerate() {
            let class=source_type.class();
            let location=match class {
                RegisterClass::Integer if gpr<6=>{
                    let index=gpr;gpr+=1;
                    Location::Register{class,index}
                }
                RegisterClass::Sse if xmm<8=>{
                    let index=xmm;xmm+=1;
                    Location::Register{class,index}
                }
                _=>{
                    let slot=stack;stack+=1;
                    Location::Stack{slot}
                }
            };
            params.push(ParameterLocation {source_index,source_type,location});
        }
        Ok(PassingPlan {
            parameters:params,result:Location::Register {
                class:self.result.class(),index:0
            },result_type:self.result,outgoing_stack_slots:stack,
            used_integer_registers:gpr,used_sse_registers:xmm,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_gpr_and_sse_register_banks_and_spill_order() {
        let mut types=Vec::new();
        for _ in 0..7 {
            types.push(ScalarKind::Signed32);
            types.push(ScalarKind::Float64);
        }
        types.push(ScalarKind::Float32);
        types.push(ScalarKind::Float64);
        let plan=Signature{parameters:types,result:ScalarKind::Float64,
            prototyped:true,variadic:false}.plan_sysv_x86_64().unwrap();
        // Registers are counted *independently* even in interleaved C source.
        for i in 0..6 {
            assert_eq!(plan.parameters[2*i].location,Location::Register{
                class:RegisterClass::Integer,index:i as u8});
            assert_eq!(plan.parameters[2*i+1].location,Location::Register{
                class:RegisterClass::Sse,index:i as u8});
        }
        assert_eq!(plan.parameters[12].location,Location::Stack{slot:0});
        assert_eq!(plan.parameters[13].location,Location::Register{
            class:RegisterClass::Sse,index:6});
        assert_eq!(plan.parameters[14].location,Location::Register{
            class:RegisterClass::Sse,index:7});
        assert_eq!(plan.parameters[15].location,Location::Stack{slot:1});
        assert_eq!(plan.outgoing_stack_slots,2);
        assert_eq!(plan.result,Location::Register{
            class:RegisterClass::Sse,index:0});
        assert_eq!(plan.used_integer_registers,6);
        assert_eq!(plan.used_sse_registers,8);
        for (i,arg) in plan.parameters.iter().enumerate() {
            assert_eq!(arg.source_index,i);
        }
    }
    #[test]
    fn pointer_consumes_gpr_not_sse_and_integer_result_returns_in_rax() {
        let s=Signature {
            parameters:vec![ScalarKind::CallablePointer,ScalarKind::Float32,
                ScalarKind::Unsigned64,ScalarKind::Float64],
            result:ScalarKind::Unsigned64,prototyped:true,variadic:false,
        }.plan_sysv_x86_64().unwrap();
        assert_eq!(s.parameters[0].location,Location::Register{
            class:RegisterClass::Integer,index:0});
        assert_eq!(s.parameters[1].location,Location::Register{
            class:RegisterClass::Sse,index:0});
        assert_eq!(s.parameters[2].location,Location::Register{
            class:RegisterClass::Integer,index:1});
        assert_eq!(s.parameters[3].location,Location::Register{
            class:RegisterClass::Sse,index:1});
        assert_eq!(s.result,Location::Register{
            class:RegisterClass::Integer,index:0});
        assert_eq!(s.result_type,ScalarKind::Unsigned64);
    }
    #[test]
    fn overflow_spills_preserve_source_order_and_width() {
        let kinds=[ScalarKind::Signed64,ScalarKind::Unsigned32,
            ScalarKind::Float32,ScalarKind::Float64];
        for kind in kinds {
            let plan=Signature {parameters:vec![kind;10],result:kind,
                prototyped:true,variadic:false}.plan_sysv_x86_64().unwrap();
            let reg_count=if kind.class()==RegisterClass::Integer {6} else {8};
            assert_eq!(plan.outgoing_stack_slots,(10-reg_count) as u32);
            for i in reg_count..10 {
                assert_eq!(plan.parameters[i].location,
                    Location::Stack{slot:(i-reg_count) as u32});
            }
            assert_eq!(plan.result_type,kind);
            assert_eq!(plan.result,Location::Register{
                class:kind.class(),index:0});
        }
    }
    #[test]
    fn exact_prototype_required_and_large_signatures_rejected() {
        for (prototyped,variadic) in [(false,false),(false,true),(true,true)] {
            let sig=Signature {parameters:vec![],result:ScalarKind::Float64,
                prototyped,variadic};
            assert_eq!(sig.plan_sysv_x86_64(),
                Err(PlanError::UnprototypedOrVariadic));
        }
        assert_eq!(Signature{
            parameters:vec![ScalarKind::Float64;257],
            result:ScalarKind::Float64,prototyped:true,variadic:false,
        }.plan_sysv_x86_64(),Err(PlanError::OversizedSignature));
        assert!(Signature{
            parameters:vec![],result:ScalarKind::Float64,
            prototyped:true,variadic:false,
        }.plan_sysv_x86_64().is_ok());
    }
}
