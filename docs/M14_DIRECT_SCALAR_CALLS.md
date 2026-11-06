# M14 checked scalar direct calls

**Status:** native EDG/Cranelift reference slice merged 2026-11-05 in
[PR #65](https://github.com/bingordo/edgcpp-rust/pull/65)
(commit `d08f9bdb7170adc1cc9e1424c8407f85a296eeeb`).
The full [pinned CI run #42067347502](https://github.com/bingordo/edgcpp-rust/actions/runs/42067347502)
passed.

## Supported

- **Direct** calls to named, defined functions in the **same EDG compilation
  unit**, including forward, nested and recursively linked calls.
- Exact EDG call-site metadata and named routine symbol identity; no
  reconstruction from source spelling, arbitrary pointers, or guessed
  argument positions.
- Nonvariadic, fully prototyped native function signatures with an F32/F64
  return. Formal arguments may interleave standard F32/F64 and signed/unsigned
  32/64-bit integers in source order, retaining independent SysV INTEGER
  (6-GPR) and SSE (8-XMM) allocation and the normal stack overflow ABI.
- Float argument expressions already supported by the typed FP emitter
  (including explicit F32/F64 conversions). An integer argument must be a
  directly referenced, native-verified **integer formal** of the caller,
  not guessed integer arithmetic or memory.
- Integer-returning **caller** functions may explicitly cast an FP-valued
  direct-call result to a signed/unsigned 32/64-bit integer, using the
  separately validated FP-to-INTEGER result ABI introduced in
  [PR #64](https://github.com/bingordo/edgcpp-rust/pull/64).
- Calls occur inside existing FP expressions, conditional branches,
  SSA loops, and source-verified FP local assignments.

## Safety and signature checks

1. Source calls are selected by the native EDG call records' complete
   function/statement/expression identity, and the native callee must be a
   direct routine designator.
2. The call site's function type must have a checked return type,
   nonvariadic prototype, contiguous parameters and exact argument count.
   Each argument is lowered according to the authoritative parameter type.
3. Before code emission, every `FloatExpr::Call` is checked against
   **actual function definitions** in the module, including all parameter
   classes and return type. Unknown, external, mismatched, or indirect
   targets reject without emitting a silently incomplete object.
4. Every function ID is declared before constructing body calls; Cranelift
   imports those function references and emits typed native `call`
   instructions. The existing FP SSA verifier still guards local reads
   across branch and loop predecessors.
5. No arbitrary external symbol, function pointer, indirect call, varargs,
   integer expression tree, side-effecting argument, global/volatile
   memory operation, aggregate SSE/SSEUP, or mixed INTEGER-valued callee
   return is accepted by this new path.

## Differential validation

[`m14_direct_scalar_calls.c`](../tests/fixtures/m14_direct_scalar_calls.c)
and its [GCC driver](../tests/fixtures/m14_direct_scalar_calls_driver.c)
cover forward calls, nested calls, F32 return values, mixed
32/64-bit signed/unsigned integer and FP parameters, **7 GPR / 9 XMM**
caller/callee argument exhaustion and stack spills, a direct FP call in
a loop-carried SSA update, and explicit integer results of an FP call.
They run as both GCC-compiled and EDG/Cranelift objects.
Source negative fixtures reject external-only callees and arithmetic
INTEGER call arguments. Rust tests also require rejection of missing
callees and altered ABI signatures.

## Follow-on typed indirect-call slice

[PR #66](https://github.com/bingordo/edgcpp-rust/pull/66)
(merge commit `a6b65484b210a69924d917ce703d66a1ac829d05`)
added [source-verified native function-pointer-formal indirect
calls](M14_INDIRECT_SCALAR_CALLS.md). The full
[GCC differential workflow](https://github.com/bingordo/edgcpp-rust/actions/runs/42147086671)
passed. Direct and indirect calls share typed scalar argument
emission, but this slice does **not** implement pointer locals,
computed pointer targets or indirect INTEGER-returning callees.

## Remaining M14 integration gates

- Qualified external direct calls with checked imports/linkage/visibility,
  and indirect calls through verified function-pointer values.
- General INTEGER expression trees, narrow integer parameter promotions,
  FP-to-integer and integer-to-FP conversions throughout full source
  expressions, and INTEGER-returning *callees* at mixed call sites.
- Shared, target-independent typed ABI/CFG machinery for the integer and FP
  paths; FP lvalue and global memory semantics, aggregate SSE and SSEUP,
  richer control conditions and exceptional FP environment support.
- A checked, independent EDG source-to-`bingordo/compiler` adapter with
  production-backend tests.

This documents **a checked reference slice**, not full M14 completion.
C11 atomics remain separately deferred in
[issue #59](https://github.com/bingordo/edgcpp-rust/issues/59).
