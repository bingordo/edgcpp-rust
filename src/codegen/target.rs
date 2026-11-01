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
}
