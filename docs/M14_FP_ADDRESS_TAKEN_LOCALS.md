# M14 — source-addressed FP automatic scalar objects

Status: merged in [PR #79](https://github.com/bingordo/edgcpp-rust/pull/79),
full pinned [native CI #42392410145](https://github.com/bingordo/edgcpp-rust/actions/runs/42392410145) green. This is a bounded **address-taken local storage** improvement,
not completion of the general C memory/lifetime model in M14 #60.

## Source-checked stack selection

Only a native EDG `&local` operation (operator 43) over a direct,
symbol-owned, ordinary automatic `float` or `double` variable selects
real stack storage. The FP source scan matches the **same native symbol
inside the same routine**; unrelated expressions and globals never
change local storage. The local's declared source type must be a
nonqualified standard F32/F64 type with exact target size/alignment.
Direct FP locals *without* an address-of remain regular SSA values.

The internal `ScalarType::StackFloat` class is allowed only for FP
automatic locals, never argument or result ABI slots. Such a local has
a fixed, native 4/8-byte, suitably aligned Cranelift stack slot,
created at function entry. Its internal SSA variable is the **stack
address**, not an FP value. Every ordinary direct local initializer,
assignment, read, branch or loop update uses a real typed memory
store/load. This is crucial: a write through `double *p=&x` must be
visible when the C source subsequently reads `x` directly. No stale
SSA copy of `x` can survive such an alias.

## Address formation and lifetime boundary

`PointerSource::AddressedLocal` is derived only from the native
`&local` node, its direct child symbol, exactly typed C `float*`/
`double*` expression and a checked stack-backed local. An existing
data-pointer SSA local may be initialized or reassigned from that
address. Its later `*p` loads/stores remain **actual ordinary memory**
operations, including between branches and inside loops.

A direct `*(&local)` reads/writes that same stack object. The existing
definite-initialization checker requires the addressed automatic to
have been initialized before exposing its address. This is
intentionally narrower than all legal C programs. The reference path
does not permit returning an automatic's address, passing pointer
values to external/indirect calls or converting the address to integer.

## Native gate

The GCC-vs-EDG/Cranelift driver checks a direct load, pointer store
followed by direct local read (no stale SSA), direct and aliasing
writes, pointer reassignment among two separate stack objects,
conditional selection, loop loads/stores, F32 negative-zero bits and
`*(&local)`. Rust unit checks require stack-backed identity,
definite initialization and exact FP pointer widths.

Negative C fixtures reject uninitialized addressed locals, pointer
arithmetic, unsupported escaping calls and qualified/volatile
objects; none may emit a partial ELF.

## Not implemented

General pointer expressions/arithmetic, address-of parameters,
arrays, aggregates, pointer escapes, qualified/atomic memory,
dynamic lifetime boundaries, automatic integer storage, full C
sequence points and fenv exceptions remain outside this slice.
The broader [M14 issue #60](https://github.com/bingordo/edgcpp-rust/issues/60)
must remain open. C11 atomics remain deferred in #59.
