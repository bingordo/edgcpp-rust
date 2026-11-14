//! Shared defensive bounds for the EDG wire reader and reference backends.
//!
//! These are implementation resource policies, not ISO C/C++ or SysV ABI
//! limits. Keep unrelated resources separate: a function's local count,
//! flattened aggregate elements, and a native object's byte size have
//! different memory and compile-time costs. Do not raise a limit solely to
//! accept a test case without checking allocation and code-generation costs.

/// Maximum number of explicitly supported scalar arguments in one signature.
pub(crate) const MAX_SUPPORTED_PARAMETERS: usize = 256;

/// Maximum decoded native aggregate byte image; mirrors the C++ bridge cap.
pub(crate) const MAX_NATIVE_AGGREGATE_IMAGE_BYTES: usize = 4096;

/// Maximum representable native object size in CFG reference lowering.
pub(crate) const MAX_NATIVE_OBJECT_BYTES: u64 = 16 * 1024 * 1024;

/// Maximum aggregate scalar elements expanded into individual CFG operations.
pub(crate) const MAX_AGGREGATE_SCALAR_ELEMENTS: u64 = 256;

/// Maximum local variables per scalar FP reference function.
pub(crate) const MAX_FP_LOCALS: usize = 4096;

/// Maximum statements within an FP reference function or checked statement arm.
pub(crate) const MAX_FP_STATEMENTS: usize = 10_000;

/// Maximum locals exposed by the integer CFG/reference expression backend.
pub(crate) const MAX_CFG_LOCALS: usize = 10_000;

/// Maximum recursion depth for CFG aggregate layouts and scalar expansion.
pub(crate) const MAX_AGGREGATE_LAYOUT_DEPTH: usize = 32;

/// Return the byte size of a bounded native array, rejecting overflow or zero.
pub(crate) fn bounded_array_bytes(element_bytes: u64, elements: u64) -> Option<u64> {
    element_bytes.checked_mul(elements)
        .filter(|&n| n > 0 && n <= MAX_NATIVE_OBJECT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify the shared caps distinguish their different resources.
    #[test]
    fn distinct_backend_limits_stay_explicit() {
        assert_eq!(MAX_SUPPORTED_PARAMETERS, 256);
        assert_eq!(MAX_FP_LOCALS, 4096);
        assert_eq!(MAX_FP_STATEMENTS, 10_000);
        assert_eq!(MAX_CFG_LOCALS, 10_000);
        assert_eq!(MAX_NATIVE_AGGREGATE_IMAGE_BYTES, 4096);
        assert_eq!(MAX_NATIVE_OBJECT_BYTES, 16 * 1024 * 1024);
        assert_eq!(MAX_AGGREGATE_SCALAR_ELEMENTS, 256);
        assert_eq!(MAX_AGGREGATE_LAYOUT_DEPTH, 32);
    }

    /// Reject multiplication overflow and objects outside the native cap.
    #[test]
    fn array_layout_multiplication_checks_exact_boundaries() {
        assert_eq!(bounded_array_bytes(1, MAX_NATIVE_OBJECT_BYTES),
                   Some(MAX_NATIVE_OBJECT_BYTES));
        assert_eq!(bounded_array_bytes(8, MAX_NATIVE_OBJECT_BYTES / 8),
                   Some(MAX_NATIVE_OBJECT_BYTES));
        assert_eq!(bounded_array_bytes(8, MAX_NATIVE_OBJECT_BYTES / 8 + 1), None);
        assert_eq!(bounded_array_bytes(1, MAX_NATIVE_OBJECT_BYTES + 1), None);
        assert_eq!(bounded_array_bytes(0, 5), None);
        assert_eq!(bounded_array_bytes(8, 0), None);
        assert_eq!(bounded_array_bytes(u64::MAX, 2), None);
    }
}
