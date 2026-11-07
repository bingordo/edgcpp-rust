# M14 — source-checked C indexed F32/F64 memory

Status: merged in [PR #80](https://github.com/bingordo/edgcpp-rust/pull/80),
full pinned [native CI #42393335605](https://github.com/bingordo/edgcpp-rust/actions/runs/42393335605) green. This is a bounded extension of the existing
pointer/formal, automatic pointer SSA and stack-backed local model.
[M14 #60](https://github.com/bingordo/edgcpp-rust/issues/60) remains open.

## Source contract

A native EDG `eok_subscript` (semantic operation 45) on a standard C
`float` or `double` lvalue may be lowered to a real indexed memory
access. The original EDG expression must have **exactly two operands**,
one an unqualified native `float *`/`double *` pointer with the same
pointee as the result, the other a native signed/unsigned I32/I64 index.

The pointer is accepted only when it is the unchanged, source-verified
pointer formal, definitely initialized automatic data-pointer SSA
local or an already initialized, true address-taken F32/F64 stack
object. The integer index is accepted only if it is an unchanged native
I32/I64 formal, a definitely initialized exact-type INTEGER automatic
SSA local (merged PR #81), or an EDG-owned integer literal with exact
signedness, width and 64-bit carrier bits. No source string interpretation, cast,
computed index tree, arbitrary address, invalid INTEGER local or pointer
arithmetic is inferred. Both `p[i]` and the C-equivalent `i[p]`
operand order are checked.

The index uses native signed or unsigned extension for 32-bit values
to 64 bits, then a scale of exactly four (F32) or eight (F64) bytes,
followed by a real ordinary typed Cranelift load or store. An indexed
store is visible through another pointer to the same backing memory;
an addressed FP local is still stack-backed, so direct local reads
cannot return a stale SSA value. Neither the index nor the pointer
operand may have side effects in this subset.

The caller remains responsible for ordinary C pointer bounds and
lifetime rules. Access outside an actual backing object has undefined
behavior in C and is not made valid by this implementation.

## Validation

- Pure Rust tests verify true FP indexed object emission, correct
  source ABI classes, checked pointer provenance and local definite
  assignment, refusal of forged I32 constant images, invalid I16
  indices, incorrect pointee widths and absent source arguments.
- Native EDG/Cranelift-versus-GCC differential uses the same independent
  driver and compares actual F32/F64 memory, signed negative and
  unsigned indices (including verified INTEGER SSA locals in PR #81), C reversed `i[p]`, pointer SSA aliases, fixed
  literal indices, and signed-zero writes observed by the C caller.
- Negative native inputs reject computed integer index arithmetic,
  pointer reinterpret casts, general `*(p+i)` arithmetic,
  volatile-qualified pointees and narrow INTEGER index promotions.
  Rejected input must leave no ELF file.

## Still not supported

This is **not** general C arrays or all pointer arithmetic. Array
objects, pointer addition/subtraction outside a checked subscript,
signed/unsigned integer expression trees or local integer indexes,
arbitrary pointer casts, structured fields, volatile/atomic memory,
escaping pointers, dynamic object lifetimes, full integer/FP mixed
expression and floating environment behavior are separate steps.
