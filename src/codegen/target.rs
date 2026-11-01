//! Target-specific C ABI boundary. The EDG C semantics and CFG stay generic.
//! Extend here for new targets; never copy AMD64 register rules into cfg.rs.
use super::*;
mod x86_64;
pub(super) const ABI: x86_64::SysV = x86_64::SysV;

/// Validated platform layout and machine-level ABI mapping.
/// The current IR encodes an integer-only aggregate subset; HFA/FP/MEMORY
/// support will require richer argument/result descriptors before AArch64.
pub(super) trait TargetAbi {
    fn isa_name(&self) -> &'static str;
    fn classify_aggregate(&self, il:&Il, ty:crate::TypeId) -> Result<Option<u8>,CodegenError>;
    fn machine_parameters(&self, abi:&CAbi) -> Vec<CAbiInt>;
    fn validate_register_budget(&self, abi:&CAbi) -> Result<(),CodegenError>;
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
