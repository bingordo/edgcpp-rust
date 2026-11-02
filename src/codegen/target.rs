//! Target-specific C ABI boundary. The EDG C semantics and CFG stay generic.
//! Extend here for new targets; never copy AMD64 register rules into cfg.rs.
use super::*;
mod x86_64;
pub(super) const ABI: x86_64::SysV = x86_64::SysV;

/// Machine pieces retain their source-parameter identity independently of
/// the number of physical values in a Cranelift signature. This is the common
/// contract for direct calls, function entry and future target backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RegisterClass {
    Integer,
    // The current lowering rejects floating arguments. Reserve the class in
    // the plan so a future target does not encode FP as an integer register.
    Floating,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PieceLocation {
    Register { class: RegisterClass, index: u8 },
    // Stack slots are *ABI planning metadata*: Cranelift performs final
    // placement from the target calling convention's signature.
    Stack { slot: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MachinePiece {
    pub ty: CAbiInt,
    pub object_offset: u8,
    pub object_bytes: u8,
    pub location: PieceLocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PassingMode {
    Scalar,
    AggregateByValue { bytes: u8 },
    // An explicit future extension point. No target may select this until
    // indirect argument/result materialization is implemented.
    Indirect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParameterPlan {
    pub source_index: usize,
    pub mode: PassingMode,
    pub pieces: Vec<MachinePiece>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PassingPlan {
    pub parameters: Vec<ParameterPlan>,
    pub result: Vec<MachinePiece>,
}

impl PassingPlan {
    pub fn machine_parameters(&self) -> Vec<CAbiInt> {
        self.parameters.iter()
            .flat_map(|param| param.pieces.iter().map(|piece| piece.ty))
            .collect()
    }
    pub fn machine_returns(&self) -> Vec<CAbiInt> {
        self.result.iter().map(|piece|piece.ty).collect()
    }
}

/// Validated platform layout and machine-level ABI mapping.
/// The current IR encodes an integer-only aggregate subset; HFA/FP/MEMORY
/// support will require richer argument/result descriptors before AArch64.
pub(super) trait TargetAbi {
    fn isa_name(&self) -> &'static str;
    fn classify_aggregate(&self, il:&Il, ty:crate::TypeId) -> Result<Option<u8>,CodegenError>;
    /// Build the authoritative source-to-machine argument and result mapping.
    /// Unsupported placement is rejected here, before any object is emitted.
    fn passing_plan(&self, abi:&CAbi) -> Result<PassingPlan,CodegenError>;
    fn validate_aggregate_return(&self, aggregate:Option<u8>) -> Result<(),CodegenError>;
    /// Produce the checked representation of a source aggregate argument.
    fn lower_aggregate_argument(&self, size:u8, address:Expr) -> Expr;
    /// Pack a call-site argument into its physical ABI registers.
    fn pack_argument(
        &self, fb:&mut FunctionBuilder<'_>,
        address:cranelift_codegen::ir::Value, size:u8,
    ) -> Result<Vec<cranelift_codegen::ir::Value>,CodegenError>;
    /// Pack a one-register aggregate result (the current supported subset).
    fn pack_result(
        &self, fb:&mut FunctionBuilder<'_>,
        address:cranelift_codegen::ir::Value, size:u8,
    ) -> Result<cranelift_codegen::ir::Value,CodegenError>;
    /// Convert incoming physical ABI values into one logical C parameter each.
    /// The optional second register is used only for aggregate materialization.
    fn decode_parameters(
        &self, fb:&mut FunctionBuilder<'_>, abi:&CAbi,
        machine_values:&[cranelift_codegen::ir::Value],
    ) -> Result<(Vec<cranelift_codegen::ir::Value>,
                 Vec<Option<cranelift_codegen::ir::Value>>),CodegenError>;
    /// Store exactly the native object bytes from the incoming register(s).
    fn materialize_aggregate(
        &self, fb:&mut FunctionBuilder<'_>,
        address:cranelift_codegen::ir::Value, size:u8,
        first:cranelift_codegen::ir::Value,
        second:Option<cranelift_codegen::ir::Value>,
    ) -> Result<(),CodegenError>;

}
