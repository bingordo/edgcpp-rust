# M14 — verified ordinary F32/F64 pointer-formal memory

**Status: implemented and merged** in [PR #75](https://github.com/bingordo/edgcpp-rust/pull/75)
(squash commit `99299190322d7af802958ede1b5648c41b4180b2`),
with complete pinned EDG/Rust/Cranelift/GCC
[CI #42336164590](https://github.com/bingordo/edgcpp-rust/actions/runs/42336164590)
passing on November 6, 2026.

This is a **source-checked, ordinary memory** subset: real loads/stores
through unchanged C `float*` or `double*` function parameters.
It does **not** treat arbitrary address expressions as checked pointers.

## Supported examples

```c
double read(double *p) { return *p; }
float read_f32(float *p) { return *p; }
double update(double *p, double value) {
    *p = value;
    return *p;
}
double alias(double *p, double *q, double value) {
    *p = value;
    return *q;
}
```

For a checked native EDG pointer type, we require an actual pointer
(type kind 5, pointee role 1), the native x86-64 8-byte size and
alignment and an **unqualified standard C F32/F64 pointee** with the
matching 4/4 or 8/8 native layout. This is distinguished as
`ScalarType::DataPointer(FloatType)` from a callable C pointer,
and as `ScalarKind::DataPointer` in the backend-neutral SysV scalar ABI
plan. Data pointers use the GPR bank, independent of XMM formals.

The source `*p` must be an EDG-native unary dereference (operator 44)
whose sole operand is an exact, direct pointer **formal** with
matching type, symbol and native declaration. Such expressions emit
a real Cranelift ordinary `load`; source `*p=rhs` statement lowering
emits a real `store` after checking the lvalue and converted
F32/F64 value types. The compiler does not fold mutable memory into
SSA or fabricate a memory address from integer values.

The reference backend passes the exact pointer value through the
native SysV ABI, including stack-passed parameters. Aliasing may occur
between two formals (e.g. `p == q`); a store through `p` is observable
through a subsequent load from `q` and by the GCC caller.

## GCC differential and rejection tests

[Native C fixture](../tests/fixtures/m14_fp_pointer_memory.c)
and [GCC driver](../tests/fixtures/m14_fp_pointer_memory_driver.c)
exercise F32/F64 loads/stores and externally visible mutations,
same-pointer aliasing, load-add-store, a conditional update, a
loop using a checked FP SSA local, NaN and signed-zero round trips
and mixed INTEGER/SSE argument placement including stack-passed
pointer/FP arguments. Both GCC-produced and EDG/Cranelift-produced
functions execute against the same native driver.

Negative fixtures reject:
- pointer arithmetic and computed address dereferences;
- `int*` interpreted as `double*`;
- `volatile double*` / qualified pointees without proper memory semantics;
- C data-pointer locals initialized from an address-taken FP local;
- casts that attempt writes through a `const double*`.

The Rust unit test validates exact DataPointer identity, F64 loads/stores
and failures for callable pointers, integer stand-ins, wrong formal
indices, mismatched F32/F64 store classes and implicit RHS conversion.

## Remaining gaps

This is **not** general address-taken FP local or global memory support.
Local data-pointer variables, `&local`, arbitrary pointer arithmetic,
subscript/aggregate field lvalues, qualified/atomic memory,
pointer-to-pointer aliases, pointer values returned/passed through
indirect callees, explicit global initialization images and general
pointer provenance/lifetimes still require distinct implementation.

The broader [M14 completion checklist #60](https://github.com/bingordo/edgcpp-rust/issues/60)
also tracks C INTEGER/FP conversions and side effects, exceptional-value
semantics and a complete backend-independent semantic projection for
`bingordo/compiler`. Deferred C11 atomic expansion remains in
[issue #59](https://github.com/bingordo/edgcpp-rust/issues/59).
