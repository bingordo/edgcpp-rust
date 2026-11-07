# M14 — source-verified nonescaping FP data-pointer automatic SSA locals

Status: checked implementation and native GCC/EDG/Cranelift differential
under CI; strictly bounded; M14 issue #60 stays open.

## Source-provenance contract

A C `float *` or `double *` automatic local may only be initialized
or reassigned by copying an **unchanged** pointer formal or an already
definitely initialized pointer local, both with the exact native
unqualified F32/F64 pointee type and 64-bit SysV ABI layout. The
source must be an EDG-owned direct variable reference with native
symbol and type metadata. The existing FP pointer-formal
verification is retained for direct-formal dereferences.

Pointers have SSA registers in the FP reference CFG and use regular
definite-assignment facts across sequences, branches, loops and
reachable joins. A dereference is an actual typed Cranelift memory
`load`/`store` in source statement order. No synthetic pointer
values, address-of derivation or rewrites to FP scalar locals are
allowed. The same physical pointer can be copied to several locals,
and aliasing at the pointed-to address is real observable memory.

## Validation gate

- Pure Rust codegen checks a positive store/load through a copied
  pointer, and rejects uninitialized reads/stores, self-copy before
  definition, wrong local class or incompatible F32/F64 pointees.
- Native C/GCC differential uses a separate driver to compare real
  loads, stores, signed-zero images, parameter/SSA pointer
  reassignment, merged branch targets, loops, F32 width and
  deliberately aliased formals.
- Negative native sources reject uninitialized or partly initialized
  pointer locals, pointer arithmetic and taking the address of an FP
  automatic local; no rejected input may leave an ELF output.

## Limits

This is **not** general address-taken FP storage, alias analysis or
C pointer semantics. Pointer arithmetic, casts, null-pointer
literals, globals containing pointers, address-of locals, escaping
pointer locals, arrays, volatile/atomic/const pointees and general
memory lifetime effects remain unsupported. General mixed scalar C
expressions, FP floating environment evidence and the backend-neutral
source projection are still on
[M14 #60](https://github.com/bingordo/edgcpp-rust/issues/60).
