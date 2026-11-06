# M14 — checked nonescaping automatic callable-pointer SSA locals

**Status:** implemented and squash-merged on November 6, 2026 in
[PR #69](https://github.com/bingordo/edgcpp-rust/pull/69), merge
commit `97bd0c36def2eeb8234c45fc5d7fd96abd32e95d`.
The full pinned EDG/Rust/Cranelift/GCC differential suite passed in
[workflow #42295385457](https://github.com/bingordo/edgcpp-rust/actions/runs/42295385457).

This is a restricted **nonescaping, automatic pointer-local** path.
It does **not** implement arbitrary C memory-backed function pointers,
pointer-to-pointer aliasing, heap objects, globals, fields or
address-taken locals.

## Accepted examples

```c
double forward(double (*fn)(double), double x) {
    double (*local)(double) = fn;
    return local(x);
}
double choose(double (*a)(double), double (*b)(double),
              double x, double flag) {
    double (*selected)(double) = a;
    if (flag > 0.0) selected = b;
    return selected(x);
}
long forward_integer(long (*fn)(double), double x) {
    long (*stored)(double) = fn;
    return stored(x);
}
```

- A native EDG automatic C function-pointer local can be initialized
  or assigned from a verified function-pointer formal or another
  definitely initialized automatic local.
- Only exact, direct native symbol-valued pointer reads and assignments
  are accepted. No pointer cast, hidden address computation, implicit
  data-pointer coercion, external pointer storage or pointer constants.
- The source declaration must have a real function-pointer type with
  native 8-byte SysV pointer layout and a complete, nonvariadic,
  source-checked C callable prototype. Its argument/result classes must
  be supported native INTEGER/F32/F64 types.
- EDG can assign distinct native type IDs to structurally equivalent
  pointer-to-function prototypes. The compiler canonicalizes only
  **complete, checked, matching callable signatures**. It does not
  equate arbitrary same-sized pointers.
- Cranelift keeps these nonescaping automatic locals as typed
  64-bit **SSA values**. No stack slot or unverified memory load is
  introduced when the address never escapes. The existing FP SSA CFG
  verifies that each pointer-local read is definitely initialized,
  intersecting predecessor facts at branch joins and respecting
  zero-iteration loop paths.
- Indirect F32/F64 expressions and the narrowly supported
  INTEGER-result `return local(...);` statements may use these locals.
  The full source-derived SysV INTEGER/SSE argument signature is
  required for every call.

## Differential and negative tests

- [Positive C source](../tests/fixtures/m14_pointer_locals.c) and
  [GCC driver](../tests/fixtures/m14_pointer_locals_driver.c) cover
  initialized callback locals, copying one local into another,
  local reassignment, selection via branches, callback invocation
  inside a floating-point SSA loop and direct INTEGER return
  from a callback-local call.
- [Uninitialized pointer](../tests/fixtures/m14_pointer_locals_bad_uninit.c)
  and [one-branch-only initialization](../tests/fixtures/m14_pointer_locals_bad_branch.c)
  must fail before emitting an object.
- [Function-pointer casts](../tests/fixtures/m14_pointer_locals_bad_cast.c)
  and [address escapes](../tests/fixtures/m14_pointer_locals_bad_escape.c)
  must also fail without producing a partial ELF.
- Older indirect-FP and indirect-INTEGER negative fixtures have been
  updated to test **address-taken aliases**, rather than the now-legal
  nonescaping local-pointer case.
- Rust tests check the exact pointer types, source-value checks,
  uninitialized and self-initializing pointer reads, mismatched ABI
  identities, and both valid and invalid branch initialization joins.
  All earlier M14 regression suites passed in the same native run.

## Remaining integration work

- **Memory-backed callable pointers:** address-taken automatic locals,
  pointer-to-pointer aliases, globals, aggregate fields, arrays, and
  pointer values loaded/stored through general memory SSA. This
  requires real ownership, aliasing, memory effects and lifetime
  rules; do not implicitly reinterpret them as SSA-only locals.
- General computed/selected function-pointer expressions, function
  pointer return values, pointer-to-pointer call parameters,
  generalized INTEGER-returning call expressions, and source-derived
  integer arithmetic/promotions.
- Broader aggregate/SSEUP/varargs ABI, complete C++ member-call ABI,
  generalized FP memory effects, and a shared typed INTEGER/FP CFG and
  ABI pipeline for the production `bingordo/compiler` adapter.

This follows the
[typed indirect FP call slice](M14_INDIRECT_SCALAR_CALLS.md) and the
[INTEGER-result indirect call slice](M14_INDIRECT_INTEGER_RESULTS.md).
The broader [M14 milestone #60](https://github.com/bingordo/edgcpp-rust/issues/60)
remains open; [atomic operations #59](https://github.com/bingordo/edgcpp-rust/issues/59)
remain separately deferred.
