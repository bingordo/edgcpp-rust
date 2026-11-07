# M14 — source-checked C integer literals in FP conversions and calls

Status: implementation under native GCC/EDG/Cranelift differential review.
This is a bounded expression/ABI improvement and **does not close M14 #60**.

## Source boundary

The ordinary FP reference path previously admitted an INTEGER-to-FP cast
only when its operand was a direct, source-verified 32/64-bit INTEGER
function parameter. It also accepted INTEGER direct-call arguments only
from matching parameters. This slice additionally permits a native EDG
**constant INTEGER expression** as the operand of an actual conversion
node (EDG operator 11/12/16) or as an exact-typed INTEGER call argument.

An accepted literal must have:

- A native EDG integer constant record with no operands or symbol identity.
- A C integer type that resolves to exactly signed/unsigned 32/64 bits,
  with matching EDG native size/alignment; no bool or narrow integer.
- A copied, complete integer value from EDG's `constant_integer` record.
  The signed `i64` wire carrier preserves the **bits** of the full unsigned
  `u64` range; it does **not** imply a signed source type.
- A bit image representable by the source width and signedness. A 32-bit
  signed constant must be sign-extended; a 32-bit unsigned constant must
  be zero-extended. Source type must match the direct/indirect callee's
  checked ABI parameter exactly.

All requirements are checked before Cranelift emits code. The native
integer constant is materialized as I32 or I64. Float/double conversions
use `fcvt_from_sint` or `fcvt_from_uint` according to the **source C
integer signedness**, never bitcasting an integer register as FP.

## Positive and negative gate

- Pure Rust codegen tests cover signed/unsigned 32/64-bit constants in
  F32 and F64 numeric conversions, full unsigned 64-bit bits and
  typed imported calls. They reject forged widths, signedness and ABI
  mismatches before object emission.
- Native `m14_integer_literals.c` and a separately GCC-built driver
  compare exact FP result bits against GCC for boundaries, operations
  with a native FP parameter, and external calls using literal
  32/64-bit INTEGER arguments. Both GCC and EDG/Cranelift objects run
  with the same driver.
- Negative native inputs reject short/narrow INTEGER source arguments,
  nonconstant INTEGER arithmetic under a cast, and INTEGER automatic
  locals requiring unavailable SSA/memory semantics. No rejected
  input may produce an ELF object.

## Explicit limits

This does **not** implement general INTEGER expressions, integer
automatic locals, integer-valued results of arbitrary nested calls,
usual arithmetic conversions, integer/FP comparison in the same
expression tree, pointer casts, memory lvalues, short-circuiting or
floating-environment guarantees. EDG may constant-fold a literal cast
before the bridge captures it; the pure-Rust synthetic lowering tests
exercise the nonfolded conversion path regardless. No silent fallback
from an unverified INTEGER expression to a constant is permitted.

General FP memory/lifetimes, complete scalar expression sequencing,
exceptional-value/fenv evidence and the backend-neutral semantic
projection remain tracked in [M14 #60](https://github.com/bingordo/edgcpp-rust/issues/60).
