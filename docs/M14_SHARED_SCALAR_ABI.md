# M14 — backend-neutral scalar C ABI contract and remaining completion gates

**Scope:** Linux x86-64 SysV scalar C ABI. The Rust module
[`src/scalar_abi.rs`](../src/scalar_abi.rs) does not import Cranelift and
is exposed without the `codegen` Cargo feature. It is intended as a
source-to-machine mapping shared with the future `bingordo/compiler` adapter.

## Source-owned preconditions

Before a source compiler builds `scalar_abi::Signature`, it must prove:

- The source type is a native EDG C function type with a complete,
  nonvariadic prototype, no unaccounted member/implicit parameters,
  and an exact ordered argument list. No unsourced symbol name or
  guessed type is sufficient.
- Every parameter and result has a supported native C scalar class:
  signed/unsigned 32/64-bit INTEGER, 32/64-bit IEEE FLOAT, or a
  native pointer **already proved to point to a compatible callable**.
  The pointer class *does not* grant permission to cast data pointers.
- All type width/alignment, signedness, native source function-pointer
  signatures, local initialization and source operand conversions are
  checked by the frontend independently of the physical passing plan.
  Narrow integer/bool, `long double`, complex, SIMD aggregates,
  varargs, pointers needing memory provenance and any other unhandled
  class **must fail closed** rather than pretending to use this plan.
- An unsourced/unprototyped external import, unsupported call expression,
  source ambiguity or nonlocal memory effect must not produce partial ELF.

## Machine plan

The pure-Rust source-order map uses one value per supported C scalar
argument. It does not consume an INTEGER register for an FP argument or
an XMM register for an INTEGER/pointer argument.

| Scalar kind | ABI class | Source bytes | Registers before overflow | Return |
|---|---|---:|---|---|
| C signed/unsigned 32-bit | INTEGER | 4 | RDI, RSI, RDX, RCX, R8, R9 | EAX |
| C signed/unsigned 64-bit | INTEGER | 8 | RDI, RSI, RDX, RCX, R8, R9 | RAX |
| Verified native callable pointer | INTEGER | 8 | RDI, RSI, RDX, RCX, R8, R9 | RAX |
| C `float` | SSE | 4 | XMM0–XMM7 | XMM0 |
| C `double` | SSE | 8 | XMM0–XMM7 | XMM0 |

Overflow arguments consume consecutive **8-byte stack argument slots
in source spill order**, regardless of their INTEGER/SSE class. Slots
are relative to the outgoing argument area, **not absolute offsets
from RSP/RBP**; physical frame lowering stays target-owned.
A plan explicitly includes `source_index`, `source_type`, and
`location` per parameter, plus register and spill counts.

The pure planner tests both classes under heavy interleaving
(seven INTEGER plus nine SSE parameters), result return classes,
pointer GPR consumption, 10 FP or INTEGER arguments, empty
signatures, and rejection of oversized, variadic, and unprototyped
signatures. Signedness is retained in the source type even where
machine register allocation is identical.

## Integration boundaries

- `src/codegen/fp.rs` translates only **source-checked** EDG
  `ScalarType` metadata to this planner. It validates every function
  definition and ordinary direct/indirect FP-returning call as well
  as INTEGER-result indirect calls before backend emission.
- `src/codegen.rs::CAbi::passing_plan` cross-checks its existing
  target INTEGER CFG passing plan against the common contract for
  eligible native 32/64-bit scalar signatures. The older target ABI
  remains authoritative for narrow INTEGER, booleans and aggregates;
  no working integer feature is reinterpreted as SSE.
- The new module does **not** change Cranelift's final frame layout,
  pick register encodings in the generic AST, define memory
  side effects, or implement the `bingordo/compiler` native adapter.
  This is a **shared contract and verified plan**, not a complete
  target-independent code generation framework.

The cross-compiler ABI validation already in the pinned CI includes
the real EDG-to-Cranelift object vs GCC suites for mixed 7-GPR/9-XMM
signatures, direct/indirect/external C calls, stack-passed pointer
formals, F32/F64/INTEGER results and FP SSA loops. The new unit
tests run with and without the optional `codegen` feature.

## M14 completion audit

The previously merged M14 slices have **verified** native IEEE
F32/F64 arguments/returns; exact literal bits; arithmetic/conversion;
FP comparisons with NaN-aware predicates; SSA locals,
nested conditions and loop control; mixed 32/64-bit INTEGER/SSE
formals; signed/unsigned FP-to-INTEGER conversion returns; checked
same-module/external named FP-returning direct calls; indirect
FP/INTEGER-result calls through source-verified function pointers;
and nonescaping pointer SSA locals.

The following **do not** follow from the passing-plan work and
must not be claimed completed:

1. General FP memory effects: address-taken locals, FP globals,
   pointer aliasing and observable qualified/volatile storage.
2. Full source C truthiness, conditional expressions and
   general INTEGER↔FP expression casts/promotions/side effects.
3. Complete SysV aggregate SSE/SSEUP/MEMORY classes, varargs,
   narrow/extended FP and complex ABI; target-specific out-of-scope
   classes need explicit separate milestones if deliberately deferred.
4. End-to-end fenv/rounding-mode and exceptional-value semantics
   beyond the existing finite/NaN/signed-zero differential cases.
5. An independent native `bingordo/compiler` adapter and
   executable backend differential proof.

The milestone must remain open unless these requirements are
implemented, explicitly deferred into follow-on issues, or revised
through an acknowledged, independently verified acceptance decision.
C11 atomic expansion remains deferred separately under
[issue #59](https://github.com/bingordo/edgcpp-rust/issues/59).
The [M14 tracking issue #60](https://github.com/bingordo/edgcpp-rust/issues/60)
is the source of truth for overall completion.
