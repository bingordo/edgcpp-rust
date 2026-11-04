//! C11 memory-order contract shared by future EDG semantic projections and
//! backend adapters.  This module does not decode EDG's native atomic builtins:
//! callers must retain the source order and reject unknown native operations.
//! Do not interpret these values as Cranelift instruction flags.
use core::fmt;

/// The six ISO C11 memory orders. Consume must not be silently treated as
/// relaxed; a backend may strengthen it to acquire when dependencies cannot
/// be represented.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MemoryOrder {
    Relaxed,
    Consume,
    Acquire,
    Release,
    AcqRel,
    SeqCst,
}

/// The GCC/Clang atomic builtin order encoding. Never accept unknown
/// values, dynamic order expressions, or backend-specific modifier bits.
pub fn from_gnu_builtin_constant(raw:i64) -> Result<MemoryOrder,String> {
    match raw {
        0=>Ok(MemoryOrder::Relaxed),
        1=>Ok(MemoryOrder::Consume),
        2=>Ok(MemoryOrder::Acquire),
        3=>Ok(MemoryOrder::Release),
        4=>Ok(MemoryOrder::AcqRel),
        5=>Ok(MemoryOrder::SeqCst),
        _=>Err(format!("invalid/dynamic GNU C11 atomic memory order {raw}")),
    }
}

/// A C11 atomic operation's ordering constraints, independent of the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AtomicOperation {
    Load,
    Store,
    ReadModifyWrite,
    Fence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryOrderError {
    InvalidOrder { operation: AtomicOperation, order: MemoryOrder },
    InvalidFailureOrder { success: MemoryOrder, failure: MemoryOrder },
}
impl fmt::Display for MemoryOrderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::InvalidOrder { operation, order } =>
                write!(f, "{order:?} is not a valid {operation:?} order"),
            Self::InvalidFailureOrder { success, failure } =>
                write!(f, "{failure:?} is not a valid compare-exchange failure order for {success:?} success"),
        }
    }
}
impl std::error::Error for MemoryOrderError {}

/// Validate *source* ordering, before attempting backend legalization.
pub fn validate_order(op: AtomicOperation, order: MemoryOrder) -> Result<(), MemoryOrderError> {
    use AtomicOperation::*;
    use MemoryOrder::*;
    let valid = match op {
        Load => matches!(order, Relaxed | Consume | Acquire | SeqCst),
        Store => matches!(order, Relaxed | Release | SeqCst),
        ReadModifyWrite | Fence => true,
    };
    if valid { Ok(()) } else { Err(MemoryOrderError::InvalidOrder { operation: op, order }) }
}

/// C11 compare-exchange failure semantics: failure is a load, never a
/// release; it must not be stronger than the success order. This explicit
/// table avoids treating AcqRel/Release as a simple total ordering.
pub fn validate_compare_exchange(
    success: MemoryOrder,
    failure: MemoryOrder,
) -> Result<(), MemoryOrderError> {
    use MemoryOrder::*;
    let allowed = match success {
        Relaxed => matches!(failure, Relaxed),
        Consume => matches!(failure, Relaxed | Consume),
        Acquire => matches!(failure, Relaxed | Consume | Acquire),
        Release => matches!(failure, Relaxed),
        AcqRel => matches!(failure, Relaxed | Consume | Acquire),
        SeqCst => matches!(failure, Relaxed | Consume | Acquire | SeqCst),
    };
    if allowed { Ok(()) }
    else { Err(MemoryOrderError::InvalidFailureOrder { success, failure }) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use MemoryOrder::*;
    const ORDERS: [MemoryOrder; 6] = [Relaxed, Consume, Acquire, Release, AcqRel, SeqCst];

    #[test]
    fn gnu_order_literals_are_checked_without_masking_unknown_bits() {
        assert_eq!(from_gnu_builtin_constant(0),Ok(Relaxed));
        assert_eq!(from_gnu_builtin_constant(1),Ok(Consume));
        assert_eq!(from_gnu_builtin_constant(2),Ok(Acquire));
        assert_eq!(from_gnu_builtin_constant(3),Ok(Release));
        assert_eq!(from_gnu_builtin_constant(4),Ok(AcqRel));
        assert_eq!(from_gnu_builtin_constant(5),Ok(SeqCst));
        for raw in [-1,6,7,0x100,0x2000,i64::MAX] {
            assert!(from_gnu_builtin_constant(raw).is_err(),"{raw}");
        }
    }
    #[test]
    fn operation_table_is_exhaustive() {
        for order in ORDERS {
            assert_eq!(validate_order(AtomicOperation::Load, order).is_ok(),
                matches!(order, Relaxed | Consume | Acquire | SeqCst));
            assert_eq!(validate_order(AtomicOperation::Store, order).is_ok(),
                matches!(order, Relaxed | Release | SeqCst));
            assert!(validate_order(AtomicOperation::ReadModifyWrite, order).is_ok());
            assert!(validate_order(AtomicOperation::Fence, order).is_ok());
        }
    }

    #[test]
    fn compare_exchange_all_36_pairs() {
        let allowed = [
            [true, false, false, false, false, false],
            [true, true, false, false, false, false],
            [true, true, true, false, false, false],
            [true, false, false, false, false, false],
            [true, true, true, false, false, false],
            [true, true, true, false, false, true],
        ];
        for (i, success) in ORDERS.iter().copied().enumerate() {
            for (j, failure) in ORDERS.iter().copied().enumerate() {
                assert_eq!(validate_compare_exchange(success, failure).is_ok(),
                    allowed[i][j], "success={success:?} failure={failure:?}");
            }
        }
    }

    #[test]
    fn errors_preserve_order_diagnostics() {
        assert_eq!(
            validate_order(AtomicOperation::Store, Acquire),
            Err(MemoryOrderError::InvalidOrder {
                operation: AtomicOperation::Store, order: Acquire,
            })
        );
        assert_eq!(
            validate_compare_exchange(Release, Acquire),
            Err(MemoryOrderError::InvalidFailureOrder {
                success: Release, failure: Acquire,
            })
        );
    }
}
