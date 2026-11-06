# M14 — native source-checked external scalar C calls

**Status:** implemented and squash-merged on 2026-11-06 in
[PR #67](https://github.com/bingordo/edgcpp-rust/pull/67)
(commit `a7c8fe5706ce4b1fb1238306a95e8692b9934a2b`).
The entire pinned EDG/Rust/Cranelift/GCC suite passed in
[workflow #42264148108](https://github.com/bingordo/edgcpp-rust/actions/runs/42264148108).

This is a **limited C scalar ELF import path**, not a general library linker,
C++ ABI implementation, or completion of the M14 typed CFG milestone.

## Supported

The native EDG source must contain a direct, named C routine declaration
whose definition is **not** part of this translation unit. The call must
resolve to that native routine symbol and a fully prototyped, nonvariadic
function type. Only calls whose return class is native F32/F64 are supported.
Formal argument classes are native F32/F64 and signed/unsigned 32-/64-bit C
integers. Exactly matching native EDG source expressions must supply each
argument: FP expressions through the checked FP expression tree and
integer arguments through exact same-type integer formals.

After source lowering, `FloatExpr::CallExternal` is explicitly different from
the existing same-module `FloatExpr::Call` and typed pointer-formal
`FloatExpr::CallIndirect`. The public FP-only `emit_float_bodies` reference
API **cannot authorize** external imports; only the native-verified EDG source
entry point can. A pre-emission validation pass checks every external
prototype, rejects mismatches between call sites, forbids collisions with
defined functions and deduplicates identical import signatures. Non-C
symbol names and compiler-reserved `__*` names (including unchecked
atomic builtins) cannot use this ordinary scalar import path.

Cranelift then declares each external function with `Linkage::Import`,
a complete source-derived SysV signature, and normal ELF undefined-symbol
relocations. The caller's floating-point SSA, loop backedges and
path-sensitive local initialization checks remain active, including calls
nested in expressions and calls whose FP results are explicitly converted
to 32/64-bit integer **caller** returns.

**Linking is deliberately external.** Producing the ELF object does not
prove the final symbol is available or that a foreign implementation obeys
the declared C ABI. The test suite supplies a separately GCC-compiled
implementation and links the two together; unsupported native declarations
fail before object emission.

## Validation

- [Caller source](../tests/fixtures/m14_external_scalar_calls.c)
  and [GCC-only definitions](../tests/fixtures/m14_external_scalar_defs.c)
  are separate translation units. The
  [driver](../tests/fixtures/m14_external_scalar_calls_driver.c)
  links and executes both GCC-compiled callers and the EDG/Cranelift
  ELF object against the same external implementations.
- Tests cover F32/F64 results, direct and nested external calls, mixed
  INTEGER/SSE argument classes, 7 INTEGER / 9 SSE argument overflow,
  external calls in loop-carried SSA updates and explicit
  FP-to-INTEGER enclosing-function results.
- The CI job checks that the generated ELF defines the wrapper and retains
  undefined external function symbols (`nm -u`) until linking.
- Negative fixtures reject variadic and unprototyped external functions,
  pointer-valued arguments and external data symbols; they cannot leave
  a partial ELF object. An older negative fixture now tests an actually
  unsupported unprototyped external declaration instead of assuming
  all external functions must be rejected.
- Rust unit tests reject unsourced public reference calls, symbol names
  reserved for compiler/builtins, inconsistent imported prototypes,
  duplicate-definition/import collisions and non-C symbol spellings.

## Remaining M14 work

- General memory-backed function pointers and dynamic pointer selection;
  indirect INTEGER-result calls beyond the [checked direct-return subset](M14_INDIRECT_INTEGER_RESULTS.md);
  function-pointer calls returning pointer or aggregate results.
- Full scalar integer expression trees (instead of direct formal-only
  integer argument values), 8-/16-bit promotions, bool, C conversions
  and general integer/FP mixed expression lowering.
- FP lvalues, globals, address-taken and volatile objects, wider aggregate
  ABI classes (SSE/SSEUP, hidden results), C++ member functions and varargs.
- A single target-independent typed ABI and CFG lowering design shared by
  the integer and FP paths, then a native `bingordo/compiler` backend adapter.

Atomics remain deferred separately under
[issue #59](https://github.com/bingordo/edgcpp-rust/issues/59).
The broader [M14 issue #60](https://github.com/bingordo/edgcpp-rust/issues/60)
remains open.
