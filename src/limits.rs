//! Checked representation arithmetic shared by reference backends.

/// The CFG uses signed 64-bit pointer displacements/strides and host
/// slices for materialized object data. Do not conflate layout sizes with
/// Cranelift's narrower stack slot representation.
pub(crate) fn native_object_size_fits(bytes:u64)->bool {
    bytes!=0 && i64::try_from(bytes).is_ok() && usize::try_from(bytes).is_ok()
}

pub(crate) fn bounded_array_bytes(element_bytes:u64,elements:u64)->Option<u64> {
    element_bytes.checked_mul(elements).filter(|&n|native_object_size_fits(n))
}

/// The current x86-64 stack lowering uses signed 32-bit displacements.
/// Check a conservative worst-case explicitly allocated frame extent,
/// including alignment padding and the 16-byte ABI rounding. Cranelift
/// remains responsible for register spills, callee saves and final layout.
/// This is not a language limit or a runtime stack-capacity check.
pub(crate) fn bounded_explicit_stack_bytes(
    slots:impl IntoIterator<Item=(u32,u8)>,
    temporary_bytes:u64,
)->Option<u64> {
    let mut total=temporary_bytes;
    for (bytes,log2_alignment) in slots {
        let alignment=1u64.checked_shl(u32::from(log2_alignment))?;
        if bytes==0 || u32::try_from(alignment).is_err() {return None;}
        total=total.checked_add(u64::from(bytes))?.checked_add(alignment-1)?;
    }
    let rounded=total.checked_add(15)? & !15;
    i32::try_from(rounded).ok()?;
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_stack_slot_sum_checks_alignment_and_overflow() {
        let max=(i32::MAX as u64)&!15;
        assert_eq!(bounded_explicit_stack_bytes([(32*1024*1024,0)],0),Some(32*1024*1024));
        assert_eq!(bounded_explicit_stack_bytes([(max as u32,0)],0),Some(max));
        assert_eq!(bounded_explicit_stack_bytes([(max as u32,0),(1,0)],0),None);
        assert_eq!(bounded_explicit_stack_bytes([(8,3)],max-15),Some(max));
        assert_eq!(bounded_explicit_stack_bytes([(8,3)],max-14),None);
        assert_eq!(bounded_explicit_stack_bytes([(1,0)],u64::MAX),None);
        assert_eq!(bounded_explicit_stack_bytes([(0,0)],0),None);
        assert_eq!(bounded_explicit_stack_bytes([(1,u8::MAX)],0),None);
        assert_eq!(bounded_explicit_stack_bytes([(u32::MAX,0)],0),None);
        assert_eq!(bounded_explicit_stack_bytes(std::iter::empty(),0),Some(0));
    }

    #[test]
    fn array_layout_multiplication_checks_exact_boundaries() {
        let max=(i64::MAX as u64).min(usize::MAX as u64);
        assert_eq!(bounded_array_bytes(1,64*1024*1024),Some(64*1024*1024));
        assert_eq!(bounded_array_bytes(1,max),Some(max));
        assert_eq!(bounded_array_bytes(1,max+1),None);
        assert_eq!(bounded_array_bytes(8,max/8+1),None);
        assert_eq!(bounded_array_bytes(0,5),None);
        assert_eq!(bounded_array_bytes(8,0),None);
        assert_eq!(bounded_array_bytes(u64::MAX,2),None);
    }
}
