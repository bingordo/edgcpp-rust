//! Shared defensive bounds for the EDG wire reader and reference backends.
//!
//! These are implementation resource policies, not ISO C/C++ or SysV ABI
//! limits. Keep unrelated resources separate: a function's local count,
//! flattened aggregate elements, and a native object's byte size have
//! different memory and compile-time costs. Do not raise a limit solely to
//! accept a test case without checking allocation and code-generation costs.

/// Maximum number of explicitly supported scalar arguments in one signature.
pub(crate) const MAX_SUPPORTED_PARAMETERS: usize = 256;

/// Maximum representable native object size in CFG reference lowering.
pub(crate) const MAX_NATIVE_OBJECT_BYTES: u64 = 16 * 1024 * 1024;

/// Conservative upper bound on *explicit* Cranelift stack slots per CFG
/// function, including materialized aggregate call results. This does not
/// account for final register spills or outgoing call frames, which Cranelift
/// chooses later. It is not a C automatic-storage or ABI limit.
pub(crate) const MAX_FUNCTION_EXPLICIT_STACK_BYTES: u64 = 16 * 1024 * 1024;

/// Maximum local variables per scalar FP reference function.
pub(crate) const MAX_FP_LOCALS: usize = 4096;

/// Maximum statements within an FP reference function or checked statement arm.
pub(crate) const MAX_FP_STATEMENTS: usize = 10_000;

/// Maximum locals exposed by the integer CFG/reference expression backend.
pub(crate) const MAX_CFG_LOCALS: usize = 10_000;

/// Return the byte size of a bounded native array, rejecting overflow or zero.
pub(crate) fn bounded_array_bytes(element_bytes: u64, elements: u64) -> Option<u64> {
    element_bytes.checked_mul(elements)
        .filter(|&n| n > 0 && n <= MAX_NATIVE_OBJECT_BYTES)
}

/// Bound the explicit stack slots of a single CFG function before creating
/// any slots in Cranelift. Add worst-case padding for each slot alignment.
/// Temporary call-result bytes are already aligned/padded by their collector.
pub(crate) fn bounded_explicit_stack_bytes(
    slots: impl IntoIterator<Item = (u32, u8)>,
    temporary_bytes: u64,
) -> Option<u64> {
    if temporary_bytes > MAX_FUNCTION_EXPLICIT_STACK_BYTES { return None; }
    let mut total = temporary_bytes;
    for (bytes, log2_alignment) in slots {
        let alignment = 1u64.checked_shl(u32::from(log2_alignment))?;
        if bytes == 0 || alignment > MAX_NATIVE_OBJECT_BYTES
            || u64::from(bytes) > MAX_NATIVE_OBJECT_BYTES {
            return None;
        }
        // Padding by alignment-1 is an upper bound on the physical
        // displacement caused by placing this slot after other slots.
        total = total.checked_add(u64::from(bytes))?
            .checked_add(alignment - 1)?;
        if total > MAX_FUNCTION_EXPLICIT_STACK_BYTES { return None; }
    }
    Some(total)
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
        assert_eq!(MAX_NATIVE_OBJECT_BYTES, 16 * 1024 * 1024);
        assert_eq!(MAX_FUNCTION_EXPLICIT_STACK_BYTES, 16 * 1024 * 1024);
    }

    /// Enforce a budget over the sum, not only over each slot.
    #[test]
    fn explicit_stack_slot_sum_checks_alignment_and_overflow() {
        let max = MAX_FUNCTION_EXPLICIT_STACK_BYTES;
        assert_eq!(bounded_explicit_stack_bytes([(max as u32, 0)], 0), Some(max));
        assert_eq!(bounded_explicit_stack_bytes([(max as u32, 0), (1, 0)], 0), None);
        assert_eq!(bounded_explicit_stack_bytes(
            [((max / 2) as u32, 0), ((max / 2) as u32, 0)], 0), Some(max));
        assert_eq!(bounded_explicit_stack_bytes(
            [((max / 2) as u32, 0), ((max / 2) as u32, 0)], 1), None);
        assert_eq!(bounded_explicit_stack_bytes([(max as u32 - 7, 3)], 0), Some(max));
        assert_eq!(bounded_explicit_stack_bytes([(max as u32 - 6, 3)], 0), None);
        assert_eq!(bounded_explicit_stack_bytes([(8, 3)], max - 15), Some(max));
        assert_eq!(bounded_explicit_stack_bytes([(8, 3)], max - 14), None);
        assert_eq!(bounded_explicit_stack_bytes([(1, 0)], u64::MAX), None);
        assert_eq!(bounded_explicit_stack_bytes([(0, 0)], 0), None);
        assert_eq!(bounded_explicit_stack_bytes([(1, u8::MAX)], 0), None);
        assert_eq!(bounded_explicit_stack_bytes([(1, 25)], 0), None);
        assert_eq!(bounded_explicit_stack_bytes([(u32::MAX, 0)], 0), None);
        assert_eq!(bounded_explicit_stack_bytes(std::iter::empty(), 0), Some(0));
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
