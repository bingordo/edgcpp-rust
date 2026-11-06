# M14 — native INTEGER returns from checked indirect calls

**Status:** implemented and squash-merged in [PR #68](https://github.com/bingordo/edgcpp-rust/pull/68) on November 6, 2026
(commit `079281d6b57cbf53ecdefaf581530ede30ffdb6e`).
The complete pinned EDG/Rust/Cranelift/GCC suite passed in
[workflow #42269395175](https://github.com/bingordo/edgcpp-rust/actions/runs/42269395175).

This is a source-checked **direct return statement** extension of the
existing typed function-pointer formal mechanism, not a general INTEGER
expression-tree implementation.

## Exact supported source/ABI contract

A function whose own result is a standard native C `int`, `unsigned int`,
`long`, or `unsigned long` may now return the value of a direct
function-pointer call:

```c
long forward(long (*fn)(long,double),long a,double x) {
    return fn(a,x);
}
```

The implementation also accepts `(*fn)(...)`, with a verified EDG
function-designator dereference. Native metadata must establish that
`fn` is an **unchanged pointer-valued formal parameter**, not an
arbitrary pointer expression or a memory load. Its canonical pointer
type must have valid x86-64 SysV layout (8-byte size/alignment);
its pointee must be a complete, nonvariadic function prototype whose
return class exactly matches the enclosing function's signedness and
32-/64-bit return width.

The call must be the **entire return expression**. Its parameters may
interleave native F32/F64 with signed/unsigned 32-/64-bit INTEGER.
INTEGER call arguments must still be exact checked INTEGER formals of
the enclosing function; FP arguments use the existing typed FP expression
tree and definite-initialization checks. No implicit narrowing,
integer arithmetic, general pointer argument or varargs is inferred.

The native EDG call site must have a unique owned function, statement,
expression identity, exactly the expected argument positions and count,
and a structurally compatible function-designator prototype where one
is present. The typed statement `ReturnIndirectInteger` is verified
with the surrounding FP CFG and returned through a Cranelift
`call_indirect`. The returned I32/I64 value is forwarded **directly
through EAX/RAX**, with no FP-to-integer conversion or bitcast. Other
paths through FP loops and branches continue using the SSA and
path-sensitive local initialization verification.

## Native differential coverage

[Source](../tests/fixtures/m14_indirect_integer_results.c) and the
[GCC driver](../tests/fixtures/m14_indirect_integer_results_driver.c)
execute both GCC and EDG/Cranelift objects, and test:

- Signed and unsigned 32- and 64-bit callback results.
- Two distinct signed-32 callback targets selected at runtime.
- Unsigned 64-bit callback results above `INT64_MAX` (no reinterpretation).
- Callback returns from both sides of an FP comparison, and following
  an FP SSA loop.
- Independent INTEGER/SSE argument overflow with **7 INTEGER and
  9 SSE formals**, a function pointer passed on the stack, and every
  argument forwarded to the INTEGER-returning callback.

The negative fixtures reject INTEGER arithmetic around a callback,
cast call targets, **address-taken** function-pointer locals, INTEGER-returning external
direct calls (a separate future slice), and non-32/64-bit results.
No failed source may leave a generated object file. Rust tests also
exercise malformed callback pointers, argument mismatches and reads
of uninitialized FP SSA locals.

## Still unsupported

- INTEGER-valued calls used inside larger INTEGER expressions,
  assignments or comparisons, or calls that return pointer/aggregate
  results.
- Function pointers stored in **address-taken** automatic locals, fields, arrays,
  globals or memory; dynamically selected or cast function pointers.
  Source-verified [nonescaping automatic pointer locals](M14_AUTOMATIC_POINTER_LOCALS.md)
  are supported by the later M14 slice.
- Narrow integer/bool promotions, varargs, aggregate SSE/SSEUP ABI,
  general FP memory/global effects and C++ member-call ABI.
- A shared target-independent INTEGER/FP typed ABI and CFG design,
  and a production adapter into `bingordo/compiler`.

This work follows the
[typed FP indirect-call slice](M14_INDIRECT_SCALAR_CALLS.md)
and complements the
[checked external *FP-returning* call imports](M14_EXTERNAL_SCALAR_IMPORTS.md).
The broader [M14 milestone #60](https://github.com/bingordo/edgcpp-rust/issues/60)
remains open. Atomics stay deferred in [issue #59](https://github.com/bingordo/edgcpp-rust/issues/59).
