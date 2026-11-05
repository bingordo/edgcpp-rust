# M14 mixed INTEGER/SSE scalar argument ABI (first slice)

**Status:** implemented and validated on 2026-11-05 in
[PR #63](https://github.com/bingordo/edgcpp-rust/pull/63),
merge commit `6ed7c4e83258205a54aa247e132dba42560393e7`.
The full pinned EDG/Cargo/native GCC differential
[workflow #42040987090](https://github.com/bingordo/edgcpp-rust/actions/runs/42040987090)
passed.

This is **not** the completion of general mixed FP/integer code generation,
calls, returns, or the native `bingordo/compiler` adapter.

## Implementation contract

- For FP-returning functions, native EDG parameter and prototype types are
  classified as `F32`, `F64`, signed/unsigned 32-bit integer, or
  signed/unsigned 64-bit integer. Native object size and alignment must
  agree with the declared integer width. Other classes reject.
- The existing FP-only public reference emitter still accepts `FloatType`.
  An internal `ScalarType` signature is used for mixed source routines;
  each argument retains its original source position.
- The Cranelift SysV calling convention allocates six INTEGER GPR argument
  slots independently from eight SSE/XMM argument slots; subsequent values
  use the standard stack ABI. No arbitrary positional interleaving is
  interpreted as a homogeneous list of FP machine values.
- The FP expression adapter allows an integer formal only when an
  EDG-authoritative conversion node explicitly converts that *direct*
  native integer parameter into F32/F64. This emits
  `fcvt_from_sint` or `fcvt_from_uint`, not a bitcast or a guessed signed
  conversion. Source mismatches, out-of-range parameters, and FP arguments
  passed through the integer cast form reject before ELF emission.
- All previously supported checked F32/F64 local SSA, source comparisons,
  branches, loops, early returns and definite initialization remain active
  for mixed-signature FP-returning functions.
- FP returns use native XMM results. This slice does **not** support an
  integer return type, integer locals/arithmetic, direct or indirect mixed
  calls, varargs, nonstandard floating types, pointers, or source expressions
  with unmodeled effects.

## Validation

The positive
[`m14_mixed_abi.c`](../tests/fixtures/m14_mixed_abi.c) fixture and
[`m14_mixed_abi_driver.c`](../tests/fixtures/m14_mixed_abi_driver.c)
exercise interleaved signed/unsigned integer and F32/F64 parameters,
signed/unsigned 64-bit conversion endpoints, a 7-GPR/9-XMM parameter
layout with independent register exhaustion and stack spills, FP
conditionals with SSA merges, and loops using integer-derived FP bounds.
The same GCC driver runs against GCC and EDG/Cranelift generated objects.

Rust unit tests reject type/class confusion at the expression API boundary.
Negative EDG fixtures verify that 16-bit formals, nontrivial integer expression
trees, and integer-result FP routines still reject and leave no object.
The prior M14 FP-only and loop differential tests run unchanged.

## Follow-on slices validated after PR #63

- [PR #64](https://github.com/bingordo/edgcpp-rust/pull/64) added
  explicit source-verified F32/F64→signed/unsigned 32/64-bit INTEGER
  **return** conversions, including branches/loops; the native
  [GCC differential run](https://github.com/bingordo/edgcpp-rust/actions/runs/42065227613)
  passed. Arbitrary integer arithmetic and comparisons as return values
  are still rejected.
- [PR #65](https://github.com/bingordo/edgcpp-rust/pull/65) added
  [native in-module scalar direct calls](M14_DIRECT_SCALAR_CALLS.md)
  with verified EDG symbol identities/prototypes and typed INTEGER/SSE
  argument classes. The native
  [GCC differential run](https://github.com/bingordo/edgcpp-rust/actions/runs/42067347502)
  passed. External or indirect calls remain unsupported.

## Next gates

1. General scalar typed expression/ABI plan shared by integer and FP CFG
   backends, with 8/16-bit integer promotions, bool, and integer results.
2. Mixed direct/indirect calls and two-way C/GCC ABI interoperability.
3. FP locals requiring observable memory, globals and pointer lvalues.
4. Complete C scalar conversions (both int→FP and FP→int), boolean
   truthiness, side effects, and exceptional FP behavior.
5. Separate checked EDG source-semantics contract for
   `bingordo/compiler` independent of Cranelift.

C11 atomics remain separately deferred in
[issue #59](https://github.com/bingordo/edgcpp-rust/issues/59).
