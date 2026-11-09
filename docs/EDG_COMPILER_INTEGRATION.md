# EDG frontend integration into `bingordo/compiler`

Status: **direct C integer SSA, CFG, source-typed 32/64-bit conversions and native direct integer calls merged and GCC-verified**.
Full EDG C/C++ coverage and native FP lowering are *not* claimed.
The production path is **direct checked lowering into the existing
`compiler-ir`**, not a new semantic-projection IR or independent CFG.

## Immediate delivery objective: useful C, then useful C++ (2026-11-08)

The delivery target is **a usable native C compiler as quickly as possible, then a usable native C++ compiler**—not full C99/C11/C++ standard completion before an end-to-end deliverable. Keep the native path on `compiler-ir` and x86 ELF, and keep optional Cranelift as an independent reference.

Track priority and acceptance in [compiler C usefulness issue #604](https://github.com/bingordo/compiler/issues/604) and [C++ bootstrap issue #617](https://github.com/bingordo/compiler/issues/617). Work first on source-checked typed pointer/array memory, ordinary `char`/small integer types, common external/void calls, linking and simple struct fields so *unmodified* string/array/struct/multi-translation-unit programs can execute. Fill remaining C CFG/FP/varargs features only where they block these concrete workloads. Native C `do while` is merged in [compiler PR #623](https://github.com/bingordo/compiler/pull/623) with [green full CI](https://github.com/bingordo/compiler-ci/actions/runs/42538046752).

At the first accepted C-basic executable matrix, focus the main workstream on Itanium C++ linker names, overloaded free functions, and then nonvirtual class fields, methods, constructors/destructors and lifetime. C++ linker-symbol metadata may be developed independently before that handoff. Complex exception/unwind, polymorphic RTTI, broad templates and full C99/C11 completion are **not** prerequisites for the first practical milestones. Every accepted feature needs native source + GCC/Clang link/run oracles; otherwise reject unsupported code without writing a partial object.

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
[PR #556](https://github.com/bingordo/compiler/pull/556),
[PR #563](https://github.com/bingordo/compiler/pull/563) and
[PR #572](https://github.com/bingordo/compiler/pull/572).
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

### D3c1: checked integer conditional and logical values

[Compiler PR #572](https://github.com/bingordo/compiler/pull/572)
extends native source expression lowering for the C `?:` operator,
integer-valued comparisons, `&&`, `||`, and `!`. Every
value-producing conditional is implemented using **real compiler IR
branches and typed SSA block-argument joins**, not an eagerly
evaluated `Select`. C comparisons and logical expressions return a
proper signed I32 0/1 result. The accepted C11 subset includes nested
conditional values in arithmetic, source-checked locals, direct calls
and `while` expressions. Source-typed selected arms must agree or
carry explicit EDG conversion nodes.

An independent GCC-linked native differential explicitly checks that
nonselected external calls are not executed and nonselected
potentially invalid divisions cannot trap, as well as high-bit U64,
cast and relational results. Assignment-valued expression arms,
side-effecting assignment operands, pointer lvalues and complete
ISO C expression sequencing remain unsupported. The adapter
continues to reject them before object output.

The FP IR work remains separate: existing
[compiler PR #508](https://github.com/bingordo/compiler/pull/508)
develops the generic F32/F64 SSA and verifier foundation; no second
frontend IR or Cranelift production dependency is created by D3c1.

## Build option: keep Cranelift supported

**Retain the existing opt-in `codegen` Cargo feature and Cranelift
object backend**, independently of the new native `bingordo/compiler`
backend. This is a supported alternative/reference code generator, not
a transitional implementation to delete when native compiler
coverage improves.

- `cargo build` builds the EDG binding without a code generator or
  Cranelift dependency.
- `cargo build --features codegen` builds the existing optional
  Cranelift implementation; its `codegen` example remains available.
- The production native compiler integration stays in the separate
  `bingordo/compiler/tools/edg-driver` Cargo workspace so that
  `edgcpp-rust` does **not** acquire a private compiler repository
  dependency, even through a disabled optional Cargo feature.
- Keep Cranelift CI, diagnostics and codegen fixtures working as a
  regression/reference oracle even as native compiler lowering
  achieves parity. Where useful, compare both against GCC and each
  other; no shared intermediate backend IR is required.
- A future unified frontend CLI may offer `--backend=cranelift` and
  `--backend=native`, but this must be an orchestration switch, not
  removal of the existing feature or a new core frontend dependency.

### Practical C99 scalar loops and a useful frontend gate

The native compiler integration now lowers EDG source-verified C99
`for` statements to ordinary production compiler SSA
preheader/header/body/increment/exit blocks. This includes bounded
loop-initializer scalar assignments, source-declared loop counters,
nested loops, early returns and lexical shadowing. Direct ordinary
32/64-bit scalar/formal `++`/`--` and compound assignments
write successive SSA values. A direct nonvoid integer call can be a
standalone expression statement with its result discarded and
its observable call retained.

The native ELF/GCC differential is maintained in
[compiler PR #601](https://github.com/bingordo/compiler/pull/601)
and [compiler-ci PR #164](https://github.com/bingordo/compiler-ci/pull/164).
These are *bounded* features: `break`/`continue`,
`do while`, `switch`, `goto`, arbitrary pointer lvalues,
qualified/atomic effects and using postincrement as an expression
value are not accepted by the native EDG adapter.

An explicit, non-percentage readiness matrix now lives in
[compiler issue #604](https://github.com/bingordo/compiler/issues/604).
It distinguishes a handpicked scalar-function subset from
**generally useful unmodified C99 programs**: pointer/array loops,
small integer types, structs, FP, libc and multi-file linking.
The existing Cranelift `--features codegen` backend remains
supported independently for broader reference behavior; the
EDG-to-compiler path does not remove it.

### C-fast P0: native integer pointer memory (merged 2026-11-08)

[Compiler PR #629](https://github.com/bingordo/compiler/pull/629) adds
a first production **real memory** slice to the isolated
`tools/edg-driver` adapter. It uses EDG-owned pointer/pointee
`TypeId` relationships and native target layout to admit exact
unqualified C `int*`, `unsigned*`, `long long*` and
`unsigned long long*` function parameters. The existing
`compiler-ir` `Ptr64`, `PtrOffset`, ordered scalar load/store,
SSA verifier and x86 ELF backend do all actual production lowering;
no new IR, machine memory model or Cranelift dependency was created.

Real C `*p`, `p[i]` and `*(p+i)` loads, ordinary stores,
pointer-formal aliases and loop-carried scalar results passed the
independent pinned [native/GCC CI #42539807672](https://github.com/bingordo/compiler-ci/actions/runs/42539807672),
including a negative signed index through an interior pointer.
**Indices are restricted to signed C 32-bit `int`**: the existing
legacy `PtrOffset` explicitly sign-extends its I32 index, so
unchecked `unsigned` / `size_t` indices must reject until
backend/IR semantics can preserve their zero-extension. This is
not general C memory support: byte/short accesses, qualified
pointees, pointer-local storage and escapes, address-of local arrays,
pointer-valued calls, struct lvalues and general array objects remain
unimplemented. All are future native production milestones, not
Cranelift-reference blockers.

**Next C readiness task:** source-checked `char*` and
`unsigned char*` byte load/store, integer promotion, and a
GCC/Clang differential `strlen`/byte-checksum/byte-copy program.
Then typed source array/local storage and pointer calls. See the
[C-fast readiness issue #604](https://github.com/bingordo/compiler/issues/604)
and [C++ bootstrap issue #617](https://github.com/bingordo/compiler/issues/617).

### C-fast P0b: executable C byte/string kernels (merged)

The isolated native compiler adapter now accepts source-checked C
`char*`, `signed char*`, `unsigned char*` and
`const char*` parameters, exact byte `*p`/`p[i]` loads and
stores, sign/zero extension and explicit narrowing-cast
normalization using existing `LoadNarrow`, `StoreNarrow` and
integer SSA ops. EDG pointee typeref qualifiers are checked:
`const` prohibits writes; volatile/atomic/restrict effects are
rejected rather than silently erased. Source indexing remains
limited to signed C `int`, because the current legacy
`PtrOffset` sign-extends its 32-bit index.

[Compiler PR #646](https://github.com/bingordo/compiler/pull/646)
merged with unchanged-source C string-length, copy/NUL,
byte checksum, high-bit char and alias tests passing the full
[compiler-ci native x86 ELF/GCC run #42541101380](https://github.com/bingordo/compiler-ci/actions/runs/42541101380).
Permanent regression tests were merged in
[compiler-ci PR #183](https://github.com/bingordo/compiler-ci/pull/183)
and passed the main-tracking
[run #42541335253](https://github.com/bingordo/compiler-ci/actions/runs/42541335253).

Remaining C-basic blockers: `size_t`/unsigned-index semantics,
source arrays and pointer locals, typed external pointer calls,
simple struct objects, real `main`/libc linking and headers.
[Compiler PR #649](https://github.com/bingordo/compiler/pull/649)
is the direct-pointer-call follow-up. No additional EDG or
Cranelift intermediate representation was introduced.

### C-fast P1a: real external C pointer calls, including libc (merged)

[Compiler PR #649](https://github.com/bingordo/compiler/pull/649)
adds exact EDG-source-typed pointer arguments to direct nonvariadic
C calls in the native adapter. An existing `Ptr64` value may cross
the ordinary SysV ABI to an external C function **only** when the
EDG function prototype and argument pointer/pointee semantics match.
The native x86 ELF implementation and compiler IR call operations
were already sufficient; this did not introduce an alternative IR
or extend the independent Cranelift reference path.

Independent pinned [compiler-ci #42541884721](https://github.com/bingordo/compiler-ci/actions/runs/42541884721)
and permanent/main [CI #42542100391](https://github.com/bingordo/compiler-ci/actions/runs/42542100391)
passed. The unchanged C source now calls real libc
`puts(const char*)` and separately linked C functions taking
`const unsigned char*` and `int*`, including two calls in one
expression and observable pointer writes. Regression retained by
[compiler-ci PR #184](https://github.com/bingordo/compiler-ci/pull/184).
Pointer-valued return/calls, `void*` conversions, qualifier-erasing
casts, nested call arguments requiring unmodeled sequencing, array
decay and variadic/indirect ABI are still not accepted.

**Critical remaining C-basic work:** source arrays/local storage,
pointer return values and correct C pointer/size_t conversions,
struct/field objects, libc/header compatibility and a standalone
C `main`/two-TU executable matrix. Pursue initial C++ mangled
free-function bootstrap in parallel where it does not block the
C-basic source acceptance matrix.

### C-fast P2a: fixed local scalar arrays (merged and verified)

[Compiler PR #650](https://github.com/bingordo/compiler/pull/650)
adds bounded C fixed-size automatic arrays directly to existing
production `compiler-ir` stack-memory operations:
`LocalSizedArrayAddr` roots and scaled native pointer addressing
with ordered 8/32/64-bit loads/stores. Native EDG fixed-array bounds,
element identity, byte size and alignment are verified before allocating
independently non-overlapping local stack extents. Source scopes and
shadowing remain distinct. Only source-proven local arrays may participate
in EDG's implicit array-to-qualified-pointer cast, with exact canonical
element type and no const removal; arbitrary pointer casts are not
admitted.

Original unchanged C arrays and a separately linked
`const int*` function passed pinned
[compiler-ci #42545440464](https://github.com/bingordo/compiler-ci/actions/runs/42545440464);
permanent [CI PR #185](https://github.com/bingordo/compiler-ci/pull/185)
was merged following green main-tracking
[run #42545689934](https://github.com/bingordo/compiler-ci/actions/runs/42545689934).
The accepted scope covers byte, I32 and I64 locals, indexing,
multiple independent arrays and lexical shadowing. Variable-length
arrays, aggregate initializers, short/record elements, pointer locals,
unverified pointer escape and pointer-valued return remain unsupported
and reject before native object output.

**Next C readiness target:** exact-layout C struct storage/fields
([#652](https://github.com/bingordo/compiler/issues/652)), plus
standalone C `main`, common libc/header and pointer-local acceptance
in [#604](https://github.com/bingordo/compiler/issues/604).
C++ free-function mangling and class-bootstrap [#617](https://github.com/bingordo/compiler/issues/617)
follow the C-basic gate; do not wait for exhaustive ISO conformance.

### G7a: verified standalone native C `main` and system CRT link

The native EDG adapter now has an **independent executable acceptance
test**, not just helper functions linked into a GCC-owned driver.
Unchanged source `int main(void)` plus a same-translation-unit typed
pointer helper and local arrays is translated through the existing
compiler SSA and native ELF. GCC links the resulting object with the
normal C runtime startup; the standalone executable returns the same
zero exit as independently compiled GCC. A separate source `main`
returning a nonzero value confirms exact exit-status 7 parity.

[Compiler PR #653](https://github.com/bingordo/compiler/pull/653)
and permanent [compiler-ci PR #187](https://github.com/bingordo/compiler-ci/pull/187)
are merged. Both pinned [CI #42546093378](https://github.com/bingordo/compiler-ci/actions/runs/42546093378)
and unpinned/main [CI #42546303566](https://github.com/bingordo/compiler-ci/actions/runs/42546303566)
passed. No new driver-specific IR, backend or Cranelift dependency was
introduced. Header-heavy C, `printf` source variadics, aggregate
initializers, typed pointer locals and broad records still require
separate accepted tests.

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
  `if`/`while`/`for`/`do while` SSA lowering to `break`,
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
