# EDG frontend integration into `bingordo/compiler`

Status: **direct C integer SSA, CFG, source-typed 32/64-bit conversions and native direct integer calls merged and GCC-verified**.
Full EDG C/C++ coverage and native FP lowering are *not* claimed.
The production path is **direct checked lowering into the existing
`compiler-ir`**, not a new semantic-projection IR or independent CFG.

## Implemented boundary (7–8 November 2026)

```text
   EDG C11 frontend
          |
    owned edgcpp::Il
          |
   edgcpp::graph::Graph (source identities and relationships)
          |
   tools/edg-driver source checks and direct SSA construction
          |
     compiler-ir::Function + existing verifier
          |
     compiler-backend-x86 native ELF object
          |
     independent GCC linked differential tests
```

The adapter is in
[`bingordo/compiler/tools/edg-driver`](https://github.com/bingordo/compiler/tree/main/tools/edg-driver),
merged across
[compiler PR #505](https://github.com/bingordo/compiler/pull/505),
[PR #517](https://github.com/bingordo/compiler/pull/517),
[PR #556](https://github.com/bingordo/compiler/pull/556) and
[PR #563](https://github.com/bingordo/compiler/pull/563).
It is a separate opt-in Cargo workspace: regular compiler core,
optimizer, Oberon and Mini-C builds never acquire native EDG or
Cranelift dependencies. It pins the EDG Rust binding revision and
uses the owned IL/Graph API. The adapter itself understands EDG source
kinds, ABI type identities and native layout, then creates ordinary
`compiler_ir::Op` nodes immediately. The frontend never creates an
extra IR, and the native compiler backend never sees EDG types.

The first supported C11 slice is **plain signed/unsigned I32/I64
parameters and matching result**, exact source literals and
side-effect-free unary minus/plus and add/subtract/multiply return
expressions. The source type, signedness, expression tree and target
layout are checked at each lowering step. The adapter runs
`compiler_ir::verify` and `verify_module` before calling the
production x86 ELF encoder.

The integrated native GCC differential, SSA-verifier build and
negative rejected-source tests passed in
[compiler-ci #42401198825](https://github.com/bingordo/compiler-ci/actions/runs/42401198825)
and the second
[merged-main validation #42401419335](https://github.com/bingordo/compiler-ci/actions/runs/42401419335).
The maintained [compiler-ci workflow](https://github.com/bingordo/compiler-ci/blob/main/.github/workflows/edg-direct-ssa-pilot.yml)
was merged in [compiler-ci PR #119](https://github.com/bingordo/compiler-ci/pull/119).
It uses `compiler-ci`'s existing scoped checkout key for the private
compiler repository, checks out the matching EDG source/configuration,
and validates the checksum-pinned EDG callable archive. It never adds
a private compiler Git dependency to the EDG Rust repository.

The initial D0 adapter deliberately rejected automatic locals and
control flow. The merged D1/D2 iterations now accept native
same-type I32/U32/I64/U64 declarations and SSA assignments, lexical
shadowing, `if`/else, early returns, source integer comparisons,
`while` with loop-carried phis, and lazy `&&`/`||`/`!` tests
using real CFG branches. Definite initialization is checked across
joins and zero-iteration loop paths; the verifier validates dominance,
types and branch arguments. The full pinned native source/ELF/GCC
differential and negative matrix passed in
[compiler-ci run #42488285258](https://github.com/bingordo/compiler-ci/actions/runs/42488285258).
An isolated `compiler-ci` workflow continues the acceptance gating.

The merged D3a/b extension adds source-verified I32/U32/I64/U64
integer conversions (signed/unsigned reinterpretation, 32-to-64
sign/zero extension and 64-to-32 truncation), bitwise operations,
signed/unsigned division and remainder, integer shifts and high-bit
unsigned literal bit images, **without inferring missing promotions**.
It also handles source-prototype-verified direct nonvariadic integer
calls to internal and external functions through the compiler's native
SysV ABI (including seven GPR arguments). The pinned GCC/ELF tests
passed in [compiler-ci #42489753577](https://github.com/bingordo/compiler-ci/actions/runs/42489753577)
and [merged-main #42489978105](https://github.com/bingordo/compiler-ci/actions/runs/42489978105);
[compiler-ci PR #145](https://github.com/bingordo/compiler-ci/pull/145)
adds these cases to the maintained daily workflow.

This is **bounded 32/64-bit scalar support**, not full ISO C integer
semantics. Calls through function pointers, `void` and variadic
calls, nested call arguments, small-integer/`_Bool`/enum promotions,
typedef/qualified types, pointer/global/qualified memory, effects
inside logical conditions, `for`/`do while`/`switch`/`goto`,
native float/double and C++ ABI remain unimplemented in this direct
adapter. These are future compiler-IR and frontend integration steps,
not implied acceptance. No output object is written for an
unsupported translation unit.

## Architecture decision: no separate projection IR

The immutable `Il` and `graph::Graph` remain the **source interface**:
they expose native EDG types, symbol IDs, expression/statement paths,
source positions, layout, scopes, labels and lifetime metadata.
They are not a backend-specific control-flow graph.

For each supported C construct, the direct adapter:

1. Verifies the native EDG operation, type, signedness, promotions and
   source object identity, using checked bridge accessors.
2. Determines source evaluation order, side effects, lvalue address
   and memory/qualifier behavior before selecting target-independent
   `compiler-ir` operations.
3. Emits the appropriate existing SSA instructions, block arguments
   and terminators directly; it never copies them through a parallel
   general-purpose C expression IR.
4. Invokes the existing compiler verifier and rejects any unsupported
   semantics. Where the existing compiler IR cannot preserve necessary
   C behavior, add minimal reusable operations or metadata **there**.

The optional EDG/Cranelift codegen remains a separate, thoroughly
tested *reference* for ABI and language-semantics comparisons.
It does not become a production compiler layer. The two paths may
reuse small type/semantic checking utilities on the owned EDG data,
but **must not share a second optimizer IR, SSA graph, instruction
selector or machine ABI placement plan**. Machine ABI realization
belongs to the chosen backend.

## Next implementation steps

- **Broader checked source CFG:** extend the implemented native
  `if`/`while` SSA lowering to `for`/`do while`, `break`,
  `continue`, `switch` and source `goto` where required.
  Preserve side-effect sequencing, scope/lifetimes, and fail-closed
  behavior. Retain lazy `&&`/`||` decisions rather than eager selects.
- **Remaining C type and effect semantics:** extend the merged native
  I32/U32/I64/U64 cast/arithmetic and direct-call subset to precise
  small-integer/enum/`_Bool` promotions, qualified types,
  sequencing-sensitive side effects, indirect calls and typed
  pointer/lvalue memory. Reject missing native EDG semantics rather
  than inferring an operation from source spellings.
- **Native FP:** `compiler-ir` currently has no general F32/F64 SSA
  value types or arithmetic. Add them directly to the shared compiler
  type/op/ABI and x86 backend interfaces with focused verifier and
  differential tests. Use existing M14 Cranelift F32/F64 behavior as
  a useful reference, not as the required native code path.
- **Memory and ABI:** typed lvalues, ordered/qualified effects,
  pointer aliases and object lifetime; verified C calls, global
  definitions, aggregates, stack alignment, FP exceptional/fenv
  semantics and C++ ABI as separate bounded increments.

Every increment requires independent native GCC/Clang oracles,
positive and negative fixtures, compiler SSA verification, and
no silent wrong-code for the accepted C subset. Production features
should be usable by other compiler frontends where possible.

## CI, scope and portability

Use `bingordo/compiler-ci` for native EDG/production compiler
interoperability checks. Avoid adding a Git dependency on the
private compiler repository in the public EDG Rust library's default
Cargo resolution: this was tried in the exploratory
[EDG PR #82](https://github.com/bingordo/edgcpp-rust/pull/82)
and closed because the public CI token could not authenticate.
No credentials are embedded in the adapter.

A C11 x86-64 source/layout snapshot is not automatically valid on
AArch64: pin and verify target-specific EDG configuration and
ABI/layout assumptions before enabling another machine backend.
The [M14 reference work tracker](https://github.com/bingordo/edgcpp-rust/issues/60)
continues to track full FP/C semantics; the native adapter has
separate, incremental implementation milestones.
