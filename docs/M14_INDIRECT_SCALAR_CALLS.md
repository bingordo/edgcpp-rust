# M14: checked indirect scalar calls through function-pointer formals

**Status:** implemented and merged in [PR #66](https://github.com/bingordo/edgcpp-rust/pull/66), squash commit `a6b65484b210a69924d917ce703d66a1ac829d05`. The full pinned EDG bridge, Cranelift and native GCC differential suite passed in [workflow #42147086671](https://github.com/bingordo/edgcpp-rust/actions/runs/42147086671).

This is a **bounded Cranelift reference slice**, not general C indirect-call lowering or completion of the entire M14 language/ABI milestone.

## Accepted source forms

- Native EDG C function-pointer **formal parameters** with an actual routine pointee, complete nonvariadic prototype, and 8-byte/8-byte-aligned SysV pointer layout.
- A call made directly through such a named formal (`fp(a,b)`) or through a native unary function-pointer dereference (`(*fp)(a,b)`), without any other pointer-producing operation.
- Pointee signatures returning F32 or F64. The pointed-to routine's arguments may be standard signed/unsigned 32-/64-bit C INTEGER and native F32/F64 scalar arguments. Each INTEGER argument is currently a direct verified INTEGER formal in the enclosing routine; FP arguments may use the supported typed FP expression tree.
- Pointer formals passed in GPRs or spilled onto the stack. Call arguments follow the full source order, using the target's independent SysV INTEGER and SSE register allocation. A function-pointer callee value is loaded **once**, before evaluating the call arguments.
- FP calls nested within expressions, conditional branches, and loop-carried SSA updates. An explicitly converted F32/F64 callback result can be returned as a signed/unsigned 32-/64-bit INTEGER result by the enclosing function.

## Source validation and safety

1. Native callable-pointer formals are distinguished from ordinary C data pointers, integer values, and floating values by the EDG type graph: pointer `kind` and `related_role`, routine pointee, verified complete C prototype, and native pointer layout.
2. The EDG callee is resolved to a **direct formal symbol**, optionally underneath one native dereference operation. Pointer locals/globals, pointer casts, computed addresses and arbitrary dereferences are rejected before object emission.
3. The compiler compares the callee designator's **structural native calling signature**, not just numeric EDG type IDs. This handles equivalent duplicated EDG function-type records while still requiring matching ordered source parameter classes, return type, and nonvariadic prototype. Source types missing metadata reject.
4. The reference expression records the native function-pointer formal index and canonical native pointer type identity. The verifier rejects a mismatched or absent pointer, wrong argument count/types, and unsafe FP local reads.
5. Cranelift imports an explicit, typed call signature matching those source classes and uses `call_indirect`; ordinary direct calls retain their existing module definition linkage and do not become unsafe pointer calls.
6. Code emission never silently substitutes an INTEGER GPR value for an SSE argument (or vice versa) or assumes an external symbol or pointer target exists.

## Tests

- [Positive C source](../tests/fixtures/m14_indirect_scalar_calls.c) and [native GCC driver](../tests/fixtures/m14_indirect_scalar_calls_driver.c): dynamically select two different `double` callbacks; call a `float` callback; forward mixed signed/unsigned INTEGER and FP arguments; exhaust **7 INTEGER and 9 SSE** argument registers with the function pointer itself stack-passed; call a callback from within a loop; and convert a callback result for an INTEGER return.
- [Negative C fixtures](../tests/fixtures/m14_indirect_scalar_calls_bad_local.c) reject pointer-local rebindings, [variadic function pointers](../tests/fixtures/m14_indirect_scalar_calls_bad_varargs.c), [cast function-pointer targets](../tests/fixtures/m14_indirect_scalar_calls_bad_cast.c), and [data-pointer casts](../tests/fixtures/m14_indirect_scalar_calls_bad_dataptr.c). Failures must not leave a generated object file.
- Rust unit checks reject raw integer and floating-point values substituted as callees, different function-pointer native type IDs, wrong argument classes and wrong formal indices.
- All prior FP SSA, mixed argument, integer-return and direct-call regression suites passed in [CI #42147086671](https://github.com/bingordo/edgcpp-rust/actions/runs/42147086671).

## Not included

- Function pointers stored in locals, fields, globals or memory, and function-pointer expressions produced by loads, casts, arithmetic, conditional selection or returned by other calls.
- Passing function pointers as arguments to other callbacks, arbitrary external direct imports, indirect callbacks returning C INTEGER values, varargs, narrow integer promotions, aggregate SSE/SSEUP or C++ member ABI rules.
- General integer expression trees, arbitrary FP memory and volatile/atomic effects, and the production `bingordo/compiler` adapter.

The next M14 integration choices are **checked external direct-call imports** and expanded memory-backed function-pointer values, followed by sharing the integer and FP typed ABI/CFG plan. Atomic expansion remains a separately deferred task under [issue #59](https://github.com/bingordo/edgcpp-rust/issues/59).
