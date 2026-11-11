# EDG Rust frontend bridge, native compiler integration and Cranelift reference

**Long-term goal:** use the owned EDG frontend semantics as an input to
[`bingordo/compiler`](https://github.com/bingordo/compiler), with its own
typed SSA CFG, optimizer and native code generator. The optional Cranelift
backend is a **correctness/reference stepping-stone**, not a dependency or
IR format for the main compiler. The native EDG-to-`compiler` adapter is
planned, not yet implemented; see
[the integration contract](docs/EDG_COMPILER_INTEGRATION.md) and
[the clarified M9–M13 roadmap](docs/YARPGEN_CRANELIFT_MILESTONES.md#m9--control-flow-semantics-and-first-compiler-adapter).

This branch includes a **reference code-generation implementation**, built
on the previously working EDG C/C++ frontend bridge. The historical implementation,
including experimental type dependency and interprocedural analysis, remains
on [main](https://github.com/bingundu/edgcpp-rust/tree/main).
The earlier pre-codegen snapshot is preserved on
[codegen-integration](https://github.com/bingundu/edgcpp-rust/tree/codegen-integration).

**Validated reference coverage (November 2, 2026):** the
[100-seed constrained YARPGen cohort](https://github.com/bingordo/compiler-ci/actions/runs/41443794336)
and the
[100-seed bounded-default cohort](https://github.com/bingordo/compiler-ci/actions/runs/41445555459)
both passed **100/100** GCC/Clang differential cases; the subsequent
[1,000-seed bounded-default run](https://github.com/bingordo/compiler-ci/actions/runs/41450045519)
also passed **1,000/1,000**, with zero unsupported or wrong-output cases.
The latter changes only YARPGen's maximum array dimensionality to four;
the completely unconstrained
profile was limited primarily by multi-GiB generated arrays and GCC
small-code-model relocation failures. See
[the measured coverage report](docs/YARPGEN_CRANELIFT_MILESTONES.md#measured-yarpgen-coverage-checkpoint-2026-11-02)
for exact counts and unsupported capabilities. This is validation of the
specified generated-C workload, not a claim of full C compatibility.

**C11 VLA reference support (M11):** On the pinned EDG build, C VLAs
are lowered into runtime dimension calculations and `__vla_alloc`/
`__vla_dealloc` calls. The Cranelift ELF backend accepts the checked
scalar/pointer subset and passes GCC differential cases covering 1D/2D
arrays, runtime `sizeof`, side-effectful dimensions, nested scopes,
early returns and loops. Applications must link
[`runtime/vla_runtime.c`](runtime/vla_runtime.c) or a compatible EDG
runtime. This does **not** provide a native `bingordo/compiler` backend
or full C++ VLA / longjmp support. See the [M11 implementation notes](docs/M11_VLA.md).


**Qualified scalar memory (M12, partial):** The reference backend
distinguishes ordinary, C `volatile` and C11 `_Atomic` accesses to
naturally aligned 8/16/32/64-bit RAM globals. The 0.121 Cranelift backend
uses conservative, effectful sequentially consistent operations for
qualified scalar reads/writes. This also supports naturally aligned scalar RAM loads/stores via
`volatile` and `_Atomic` pointer indirection and indexing (M12
follow-up), preserving qualifiers from the pointee rather than the
converted rvalue expression. It does **not** implement MMIO volatile,
qualified local storage, source-visible compare-exchange APIs, or
explicit C11 memory orders. Direct and indirect C11 integer atomic
add/sub/bitwise RMW and CAS-loop multiply/divide/modulo/shifts are
supported and concurrent native retry tests are included; unsupported
cases reject rather than becoming ordinary loads/stores. See
[the M12 qualified memory contract](docs/M12_QUALIFIED_MEMORY.md).

**M14 scalar FP loop CFG/SSA (loop slice complete, 2026-11-05):**
the optional Cranelift reference backend supports source-verified C
`float`/`double` loops (`while`, `for`, `do/while`), nested branches
and loops, structured `break`/`continue`, and early returns.
Typed F32/F64 loop-carried SSA values use unsealed loop headers until
backedges are constructed. Path-sensitive definite-assignment checks
reject invalid zero-iteration reads, unsafe loop tests and
continue/break-dependent uninitialized values. Native EDG/GCC
differential and full CI passed in
[run #42037880876](https://github.com/bingordo/edgcpp-rust/actions/runs/42037880876).
See [the M14 loop/SSA contract](docs/M14_FP_LOOP_SSA.md) for scope and
limitations. **This does not complete the broader mixed INTEGER/SSE ABI,
FP memory or independent compiler-adapter milestones.**

**M14 source-checked mixed INTEGER/SSE scalar arguments (first slice, November 5, 2026):**
The Cranelift reference path now accepts native EDG FP-returning routines
with interleaved standard 32/64-bit signed/unsigned integer and
`float`/`double` formals, including independent GPR/XMM exhaustion
and stack spills. Only *explicit native EDG cast nodes from direct
integer parameters* can form integer-to-FP source expressions; unmodeled
integer arithmetic, integer results, mixed calls and FP memory still
reject. The existing F32/F64 SSA and loop safety checks remain active.
Native GCC differential and full CI
[run #42040987090](https://github.com/bingordo/edgcpp-rust/actions/runs/42040987090)
passed. See the
[M14 mixed-scalar ABI contract](docs/M14_MIXED_SCALAR_ABI.md).

**M14 scalar integer-return and checked in-module direct-call slices (November 5, 2026):**
The native EDG/Cranelift reference emitter now supports explicit,
source-verified FP→signed/unsigned 32/64-bit INTEGER returns
([PR #64](https://github.com/bingordo/edgcpp-rust/pull/64),
[green CI](https://github.com/bingordo/edgcpp-rust/actions/runs/42065227613))
and EDG-resolved direct calls to functions **defined in the same module**
with F32/F64 returns, mixed INTEGER/SSE arguments, forward and nested
calls, and calls inside checked SSA loops
([PR #65](https://github.com/bingordo/edgcpp-rust/pull/65),
[green CI](https://github.com/bingordo/edgcpp-rust/actions/runs/42067347502)).
See [the direct-call safety contract](docs/M14_DIRECT_SCALAR_CALLS.md).
Unrestricted external/indirect calls, general integer expression trees, native FP
memory, arbitrary INTEGER-valued call expressions and the production compiler
adapter remain unsupported and fail closed.

**M14 native EDG function-pointer formal calls (November 5, 2026):**
The Cranelift reference frontend now supports **source-verified typed
indirect F32/F64 calls** through a C function-pointer formal, using
EDG-authored pointee prototypes and a typed SysV `call_indirect`.
The tests cover mixed INTEGER/SSE arguments, a stack-passed pointer,
dynamic F32/F64 callbacks, and callbacks in SSA loops. Arbitrary
pointer expressions, globals, pointer locals, casts, varargs and
arbitrary external imports remain deliberately unsupported. The full native
GCC differential suite [passed in run #42147086671](https://github.com/bingordo/edgcpp-rust/actions/runs/42147086671).
See [M14 indirect-call ABI safety contract](docs/M14_INDIRECT_SCALAR_CALLS.md).

**M14 source-checked external C scalar calls (November 6, 2026):**
[PR #67](https://github.com/bingordo/edgcpp-rust/pull/67)
adds source-verified, typed ELF imports for **declared external C
functions** returning F32/F64 and accepting F32/F64 and native
signed/unsigned 32/64-bit INTEGER arguments. It checks complete
nonvariadic EDG prototypes, source identifiers and conflicting imports
before object emission; GCC implementations in a separate translation
unit link and run against the generated wrappers, including 7-GPR/9-XMM
argument overflow. The full
[EDG/GCC CI run #42264148108](https://github.com/bingordo/edgcpp-rust/actions/runs/42264148108)
passed. See the
[external import contract](docs/M14_EXTERNAL_SCALAR_IMPORTS.md).
This does not add external data symbols, pointer-valued parameters,
varargs, C++ ABI, or general mixed scalar expression support.

**M14 INTEGER-returning typed indirect callbacks (November 6, 2026):**
[PR #68](https://github.com/bingordo/edgcpp-rust/pull/68)
adds source-checked function-pointer calls that return signed/unsigned
32/64-bit C INTEGER values directly in EAX/RAX, including
callbacks in branches and FP SSA loops. The implementation requires an
exact whole-return callback expression and an EDG-verified function-pointer
formal; it does not interpret arbitrary INTEGER expressions or load pointer
values from memory. The full
[native GCC CI run #42269395175](https://github.com/bingordo/edgcpp-rust/actions/runs/42269395175)
passed. See the
[INTEGER-result indirect-call ABI contract](docs/M14_INDIRECT_INTEGER_RESULTS.md).

**M14 checked automatic callable-pointer locals (November 6, 2026):**
[PR #69](https://github.com/bingordo/edgcpp-rust/pull/69)
adds nonescaping, source-verified **automatic function-pointer
locals** initialized or reassigned from checked pointer formals or
other definitely initialized pointer locals. Cranelift represents
these as typed SSA values; FP and INTEGER-returning indirect calls
can use them across branches and FP loops. The compiler compares
complete native callable prototypes even when EDG uses different
type IDs for equivalent pointer types. All Rust, native EDG and GCC
differential tests
[passed in workflow #42295385457](https://github.com/bingordo/edgcpp-rust/actions/runs/42295385457).
See the [pointer-local SSA safety contract](docs/M14_AUTOMATIC_POINTER_LOCALS.md).
Address-taken/memory-backed pointer storage, pointer globals/fields,
aliases and computed call targets remain unsupported.

**M14 shared scalar SysV passing contract (November 6, 2026):**
`src/scalar_abi.rs` now exposes a **Cranelift-independent** source-order
argument/return planner for verified C signed/unsigned 32/64-bit INTEGER,
F32/F64 SSE and callable pointer values on Linux x86-64. Both the
Cranelift FP reference path and the INTEGER CFG's eligible scalar
subset cross-check against this common plan; pure Rust tests verify
independent GPR/XMM exhaustion and source-order stack spills.
See [the shared ABI contract and completion audit](docs/M14_SHARED_SCALAR_ABI.md).
This does **not** claim that aggregate SSE/MEMORY ABI, general FP storage,
qualified memory effects or the native `bingordo/compiler` adapter
are implemented.

**M14 native FP ternary expressions (November 6, 2026):**
[PR #74](https://github.com/bingordo/edgcpp-rust/pull/74)
adds EDG-verified C conditional-value expressions returning F32/F64
with lazily evaluated branches and correctly typed SSA merges. The
native GCC differential tests check NaN, signed zero, nested ternaries
and external calls that must execute on exactly one arm. Full pinned
[CI #42317136600](https://github.com/bingordo/edgcpp-rust/actions/runs/42317136600)
passed. See [M14 FP conditional-value contract](docs/M14_FP_CONDITIONAL_VALUES.md).
INTEGER-valued ternaries, general memory-lvalue semantics and
unmodeled side effects still fail closed.

**M14 source-verified FP pointer memory (November 6, 2026):**
[PR #75](https://github.com/bingordo/edgcpp-rust/pull/75) adds
ordinary F32/F64 loads and stores through native C `float*`/`double*`
function formals, including visible pointer aliasing, branch and
loop updates, and stack-passed pointers in mixed SysV ABI signatures.
Pointer type, unqualified pointee layout, source symbol and dereference
provenance are checked using EDG native metadata; these are real
memory operations, not SSA rewrites. Full pinned
[CI #42336164590](https://github.com/bingordo/edgcpp-rust/actions/runs/42336164590)
passed, with native GCC differential and rejection fixtures. See the
[FP pointer-memory contract](docs/M14_FP_POINTER_MEMORY.md).
Address-taken automatic locals, pointer arithmetic and qualified
pointees remain unsupported.

**M14 source-checked C INTEGER literals in FP expressions and calls
(November 7, 2026):** [PR #76](https://github.com/bingordo/edgcpp-rust/pull/76)
adds native EDG constant INTEGER (signed/unsigned I32/I64) conversions to
F32/F64 and exact typed INTEGER literal arguments for direct, indirect
and imported C calls. Full-width U64 images are preserved, not
accidentally reinterpreted as signed integers. The native GCC/Cranelift
differential, pure Rust validation and unsupported-construct rejection
matrix passed in pinned
[CI #42384598213](https://github.com/bingordo/edgcpp-rust/actions/runs/42384598213).
See [the integer literal conversion contract](docs/M14_INTEGER_LITERAL_CONVERSIONS.md).
General INTEGER arithmetic, automatic integer locals and mixed scalar
side-effect sequencing are **not** supported by this slice; [M14 #60](https://github.com/bingordo/edgcpp-rust/issues/60)
remains open.

**M14 exact IEEE file-scope FP initializer data (November 7, 2026):**
[PR #77](https://github.com/bingordo/edgcpp-rust/pull/77) adds
native EDG-verified constant IEEE images for C `float`/`double`
global definitions. Explicit initialization produces byte-exact
aligned ELF `.data`; zero/tentative definitions remain real `.bss`.
Externally observable unreferenced globals are retained, and ordinary
source-verified loads/stores remain true memory accesses. GCC driver
checks cover signed zero, exact F32 last-bit values, static globals,
independent GCC global reads/writes and exported unused data symbols.
The full pinned Rust/EDG/Cranelift/GCC
[CI #42385599855](https://github.com/bingordo/edgcpp-rust/actions/runs/42385599855)
passed. See [the initialized FP global contract](docs/M14_FP_INITIALIZED_GLOBALS.md).
This does **not** support dynamic initializers, general FP
lvalues/aliasing, volatile/atomic/TLS storage or a native compiler
adapter. [M14 #60](https://github.com/bingordo/edgcpp-rust/issues/60)
remains open.

**M14 source-checked nonescaping FP data-pointer SSA locals (November 7, 2026):**
[PR #78](https://github.com/bingordo/edgcpp-rust/pull/78)
extends the verified `float*`/`double*` formal-memory slice to ordinary
automatic data-pointer locals copied from exact native pointer formals
or initialized locals. The reference CFG validates definite pointer
initialization across branches/loops/joins, and dereferences remain
real typed memory loads and stores that preserve source-visible
aliasing. Native GCC driver cases cover float/double widths, signed
zero, branch-selected targets, pointer reassignments and loop memory
effects. Negative cases reject uninitialized/partly initialized
pointer locals, pointer arithmetic and address-of automatic FP
objects without an ELF output. Full pinned
[CI #42386505956](https://github.com/bingordo/edgcpp-rust/actions/runs/42386505956)
passed. See [M14 FP data-pointer SSA local contract](docs/M14_FP_DATA_POINTER_LOCALS.md).
This does **not** add general address-taken FP locals, escaping pointers
or complete C memory/lifetime semantics; [M14 #60](https://github.com/bingordo/edgcpp-rust/issues/60)
remains open.

**M14 source-checked FP address-taking and indexed memory (November 7, 2026):**
[PR #79](https://github.com/bingordo/edgcpp-rust/pull/79)
adds real stack storage for native EDG `&local` F32/F64 objects, so
writes through a checked pointer are visible in direct local reads.
[PR #80](https://github.com/bingordo/edgcpp-rust/pull/80)
adds signed/unsigned 32/64-bit **native C `p[i]` and `i[p]`**
subscripts on source-checked ordinary F32/F64 pointer formals,
pointer SSA locals and addressed stack locals. Real scaled memory
loads/stores preserve aliases; computed integer indices, pointer
casts, raw pointer arithmetic and qualified pointees still fail closed.
Full pinned native EDG/Rust/Cranelift/GCC
[CI #42392410145](https://github.com/bingordo/edgcpp-rust/actions/runs/42392410145)
for #79 and
[CI #42393335605](https://github.com/bingordo/edgcpp-rust/actions/runs/42393335605)
for #80 passed. See
[address-taken local contract](docs/M14_FP_ADDRESS_TAKEN_LOCALS.md)
and [indexed memory contract](docs/M14_FP_INDEXED_MEMORY.md).
General FP arrays/aggregates, pointer escapes, dynamic lifetimes,
qualified memory and the backend-neutral source projection remain
under [M14 #60](https://github.com/bingordo/edgcpp-rust/issues/60).

**M14 exact INTEGER automatic SSA locals in FP code (November 7, 2026):**
[PR #81](https://github.com/bingordo/edgcpp-rust/pull/81)
adds checked nonqualified signed/unsigned I32/I64 automatic variables
initialized or reassigned from exact native EDG constants, matching
formals or already initialized locals. Numeric conversions to F32/F64
use the correct signed/unsigned instructions, and integer locals can
supply native typed C call arguments or indices for the earlier
`p[i]` FP memory path. Source-checked definite initialization and
negative rejection remain enforced; arbitrary integer arithmetic,
promotions, address-taking and narrowing remain unavailable.
The full pinned
[CI #42394399311](https://github.com/bingordo/edgcpp-rust/actions/runs/42394399311)
passed native EDG/Cranelift/GCC and pure Rust checks. See the
[INTEGER SSA source contract](docs/M14_INTEGER_SSA_LOCALS.md).
M14 [issue #60](https://github.com/bingordo/edgcpp-rust/issues/60)
remains open for general scalar expression/lvalue semantics, fenv and
backend-independent source projection.

## Reused, not reimplemented

- `cpp/bridge.cpp`: EDG IL traversal and lifetime-safe event extraction.
- `src/lib.rs`: owned, typed Rust IL records and native invocation.
- `src/graph.rs`: existing tree/parent/child navigation; **not** its
  dependency-invalidation or scheduling utilities.
- `build.rs` and `tools/build_bridge.py`: exact EDG compile-command
  flags, ABI compatibility and native linking.
- `.github/workflows/build.yml`: pinned, checksum-verified prebuilt EDG
  archive, generated headers only, incremental Cargo cache, docs-only skips,
  and cancellation of superseded runs.

The existing frontend structures are retained to avoid destabilizing the
working native interface. They are **not** a commitment to use or extend
incremental analysis.

### EDG wire record schema

Wire protocol discriminants, named event IDs and all record field counts are
defined in `schema/wire_schema.json`. The language-specific views are
checked-in generated files: `src/wire_schema_generated.rs` and
`cpp/wire_schema_generated.inc`. To change the wire protocol:

```sh
python3 tools/gen_wire_schema.py
python3 tools/gen_wire_schema.py --check
python3 tools/check_wire_schema.py
```

CI checks that generated outputs match the schema, then independently
verifies that every native C++ `emit_record` call supplies the expected
number of fields. The `record_field_tags` table additionally defines the
allowed tags (including signedness and text-vs-number distinction) for the
first 13 fully typed record kinds. Both native emission and Rust decoding
reject wrong tags for these records; `|` in a descriptor denotes an allowed
alternative such as `UNSIGNED|ABSENT`.

The remaining record kinds retain the previous decoder-side validation
while their per-field descriptors are migrated. Record-specific value
constraints, optionality and conditional variants not yet specified in the
schema are tracked in issue #87.

## Milestone 1: EDG → Cranelift → ELF

Build a new, deliberately small lowering module and optional codegen binary:

1. Read a compiled EDG IL snapshot and identify defined, supported routines.
2. Map integer function signatures, parameters and locals to Cranelift types
   and frontend variables.
3. Lower constants, loads, assignments, integer arithmetic and return.
4. Add comparisons, if/else and while using Cranelift blocks.
5. Emit Linux x86-64 ELF objects using `cranelift-object`.
6. Link with the system C compiler; run differential tests against GCC.

Initially reject unsupported constructs with actionable diagnostics. No
optimizer, inliner, incremental analysis, call graph or whole-program framework
is needed. Cranelift codegen should be isolated behind an optional Cargo
feature so native bridge tests do not build it.

## Next integration milestone: EDG → bingordo/compiler

**Current priority:** first make EDG → Cranelift a usable reference compiler
with a measured 100-seed controlled YARPGen cohort, targeted GCC/Clang
differentials and clear unsupported-feature diagnostics. The existing
ten-seed passing smoke is insufficient to establish broad usability.
Only then start the separate EDG-to-`compiler-ir` adapter; the long-term
goal is unchanged. Build a checked, Cranelift-independent source
semantics interface over `Il`/`graph::Graph`, then use the same semantic
input with either Cranelift's CFG or `compiler`'s own typed SSA CFG.

First acceptance case: a defined-behavior integer C function compiled
through EDG + `bingordo/compiler`, producing native ELF linked with a
GCC driver and matching GCC/Cranelift results. Keep the native compiler
optimizer, IR and backends independent of both EDG and Cranelift; isolate
EDG-specific lowering in a separate driver. Later expand through M9–M13
as described in the [integration plan](docs/EDG_COMPILER_INTEGRATION.md).

## CI and CPU budget

- Never compile EDG from source in Rust CI. Use the pinned release archive,
  verify its SHA-256 and source revision, and build only the small header tool.
- Cache Cargo registry and incremental target files by Rust toolchain, EDG
  source revision and bridge configuration; retain a restore prefix across
  ordinary Rust source edits.
- Keep the Cranelift dependency closure out of the normal bridge test path.
  Run targeted codegen tests on codegen changes; run full differential suites
  at milestones or by manual dispatch.
- Avoid duplicate PR/push builds and documentation-only rebuilds.

**CI:** [bingordo/edgcpp-rust Actions](https://github.com/bingordo/edgcpp-rust/actions/workflows/build.yml).


## EDG-to-Cranelift smoke test

The optional `codegen` feature exposes `examples/codegen.rs`, which runs the
EDG frontend, lowers the currently supported signed-64-bit pure-expression subset, and
writes a relocatable x86-64 ELF object:

```sh
cargo build --features codegen --example codegen
# Run from the configured EDG build directory after sourcing environment.sh:
path/to/examples/codegen output.o --c++20 input.cpp
```

The verified subset includes constants, parameters, add/subtract/multiply,
unary negation, integer bitwise operations, signed comparisons and structured
`if/else` where both branches return a pure expression. The comparison
results are canonical 0/1 integers. Boolean-to-integer casts of comparison
results are supported in the legacy expression-only API. The newer CFG
backend also handles typed C conversions and unsigned comparisons.

**ABI guard:** the bridge records EDG-native scalar integer width and
signedness, and validates routine return and parameter types before emitting
machine code. The CFG backend accepts `void` functions, native `bool`, and
8/16/32/64-bit signed/unsigned C ABI signatures, including typed locals,
side-effect-free arithmetic, comparisons, conversions and structured control
flow. Floating-point/pointer ABIs, missing metadata, variadics, implicit C++
object parameters and complex C++ features are rejected. The earlier standalone
pure-expression API remains signed-i64 only; C++ name mangling is not implemented.

Local assignments, structured if/else and pretest while loops are supported
by the newer CFG emitter (see below). Calls, global memory and general C++
features are still unsupported. Typed operations use EDG's conversion nodes;
effectful subexpressions and unsupported operators are rejected explicitly.

The differential CI test compiles `tests/fixtures/codegen_simple.cpp` through
both EDG/Cranelift and GCC, links each with the same test driver, and compares
execution results. Unsupported EDG constructs are rejected rather than lowered
incorrectly. This is a reference backend, not a general C++ compiler.

**CI is run on [bingordo/edgcpp-rust](https://github.com/bingordo/edgcpp-rust/actions/workflows/build.yml).**
The `bingundu` mirror uses a manual-only workflow to avoid duplicate CPU usage.

The negative cases verify rejection of division, shifts, floating-point
returns and pointer parameters without creating object files. Ordinary
8/16/32/64-bit scalar integer functions, local-variable assignments, loops,
branches and bool conversions are tested against GCC. Unit tests cover
missing/cyclic type metadata and mismatched signatures.

## Milestone: signed-i64 local variables

The reference code generator now handles a straight-line sequence of local
signed-i64 declarations, dynamic initializers, simple assignments and
reassignments followed by an unconditional return or a pure returning if/else.
The bridge retains initializer expressions as primary expression trees as well
as their existing role-4 control snapshot. Each assignment becomes an ordered
SSA value update (no explicit memory slot), with rejected uninitialized reads.

The historical expression-only implementation still has narrow limitations.
The current CFG emitter also supports constant initializer metadata and
signed/unsigned narrow locals in conditionals and loops. Globals, static locals,
parameter writes, pointer/array assignments, qualified/address-taken locals,
and side-effecting expression trees remain unsupported.

CI performs native execution with two sequential locals plus reassignment,
checks a local value across returning if/else branches, and compares outputs
with GCC over a signed input grid. Standalone ELF tests additionally check
multiple writes to one local and rejection of uninitialized local reads.

## Structured control-flow milestone

The ELF-emitting EDG reference backend uses real Cranelift basic blocks and SSA
variables for mutable, typed integer locals in if/else (with optional else) and
pretest while loops. Loop headers remain unsealed until the backedge is built.
A path-sensitive definite-initialization check rejects reads of locals set
only in a potentially untaken branch or loop. Every reachable path must return.
The former lower_edg function remains a narrow pure-expression API;
emit_edg_object uses structured CFG lowering. Calls, pointers, globals,
complex C++ expressions and arbitrary goto control flow remain unsupported.

CI compares generated ELF with GCC for mutable branches and loop-carried
variables over the existing signed input grid.

## For / do-while control flow

The typed C integer CFG emitter supports source-level `for` (with
assignment-style initializer/increment and a condition) and `do/while` loops.
For increments retain both EDG's original control expression and a bounded
synthetic statement carrying the complete executable expression tree. This is
needed to preserve numeric constants in increments. Generated basic blocks
handle forward/backward edges and Cranelift SSA merges. Differential tests
compare output to GCC across the existing signed input grid. At present,
source-level `break` / `continue` are accepted only when EDG's native
label flags identify a compiler-generated loop jump. Arbitrary goto and
switch-break targets are still rejected. Other unsupported operators, calls,
pointers and effectful/unsupported expression operations remain errors.

Constant signed-i64 dynamic initializers (including zero initialization)
are now carried as explicit native wire events, because EDG represents their
values without expression nodes. No default zero is invented for unknown
initializer forms.

## Structured loop exits

EDG represents source-level `break` and `continue` as gotos targeting
compiler-generated labels. The bridge now carries EDG's native label flags,
including separate switch-break classification. CFG lowering handles only
confirmed loop-break/continue targets, and rejects user gotos or switch breaks.
The compiler routes `continue` to a while header, a for increment, or a
do-while test as appropriate. Loop exit blocks remain unsealed until all break
edges are built, and loop header blocks until all continue/backedges are built.

A path-sensitive precheck rejects jumps outside a loop and conservatively
disallows reads of variables only initialized on some loop paths. Native
differential tests cover loop breaks and continues across for, while, and
do-while.

## YARPGen expansion plan

A staged plan to make the EDG/Cranelift reference backend reasonably complete
for YARPGen's generated C programs is in
[docs/YARPGEN_CRANELIFT_MILESTONES.md](docs/YARPGEN_CRANELIFT_MILESTONES.md).
It covers pinned YARPGen releases, a differential-test harness, scalar
semantics, C ABI, memory/arrays, calls, control flow, coverage gates and
CI CPU budgets. The plan is not a claim that these features are implemented.

### YARPGen M0/M1 integration

The Linux x86-64 generator is pinned to the permanent release
[yarpgen-binary-20261028-1](https://github.com/bingundu/yarpgen/releases/tag/yarpgen-binary-20261028-1).
The separate `.github/workflows/yarpgen-smoke.yml` job downloads the
verified archive by its exact SHA-256 and runs deterministic GCC-O0/GCC-O2
and Clang-O2 baseline comparisons using `tools/yarpgen_smoke.py`.

The harness also supports an optional EDG/Cranelift codegen binary, which
compiles `func.c` separately and links it with GCC-built `driver.c`.
Known unsupported C constructs are reported explicitly and never counted
as tests passed. Passing test files are removed, but all failures retain
their generated source, commands, logs and JSON manifest. The M1 baseline
is deliberately distinct from claiming YARPGen-compatible codegen.

M1 verification: [100 fixed YARPGen C seeds passed their GCC/Clang oracle
baselines](https://github.com/bingordo/edgcpp-rust/actions/runs/40648943692)
(seeds 12345–12444, GCC -O0/-O2 and Clang -O2). These are
**baseline-only**, not Cranelift passes. The harness explicitly reports
unsupported constructs and mismatches rather than counting them as passes.

### M2 initial C ABI support

The CFG emitter preserves EDG-native integer width and signedness
(8, 16, 32, 64 bits) for function parameters and return values, and
supports `void` functions and `return;`. Supported *pure integer*
expressions now normalize every intermediate result according to its EDG
type, including C integer casts, sign/zero extension, truncation, unsigned
wraparound and unsigned relational comparisons. EDG's explicit conversion
nodes are checked rather than silently guessing the common arithmetic
type. Full 64-bit unsigned constants retain their bit pattern.

**M2 typed-local checkpoint (2026-10-28):** The CFG lowering now tracks
the declared type of each C scalar local, normalizes assignment values to
the correct width/sign and keeps normalized I64 values in Cranelift SSA.
This works across `if/else`, `while`, `for` and `do/while`, with
path-sensitive definite-initialization checks intact. A common normalization
helper also handles EDG-native `bool` (C `_Bool`/C++ `bool`), converting
nonzero values to canonical `1`, including local stores and casts.
All return paths honor the declared integer result width, not only simple
one-statement returns.

[Native CI #40687818445](https://github.com/bingordo/edgcpp-rust/actions/runs/40687818445)
passed the expanded GCC differential fixtures, 93 bridge tests, targeted
Cranelift tests and negative-construct checks. Coverage includes
unsigned-byte truncation, short conditional merges, 32-bit wrapping loops,
16-bit for loops, bool conversions, and the previous ABI boundary cases.

**M2 remains partial:** qualified/volatile or address-taken locals, additional
C scalar corner cases and general side-effecting expression trees are still
unsupported. Pointers, globals, arrays and calls are later milestones.
The legacy `lower_edg()` expression-only API remains signed-i64 only.

### M3 scalar divide, remainder and shifts (2026-10-28)

The CFG reference now lowers EDG-native C operators for signed and unsigned
division and remainder, plus left shift, logical right shift and arithmetic
right shift. These are represented by typed integer operations in the shared
expression IR and use one generic Cranelift instruction mapping, rather than
a post-codegen peephole. Signedness comes from EDG's left-hand operand type;
shifts correctly permit an independently promoted right-hand operand type.
Intermediate arithmetic and local stores retain C's 8/16/32/64-bit
normalization semantics.

[Native CI #40690504958](https://github.com/bingordo/edgcpp-rust/actions/runs/40690504958)
compared the resulting ELF against GCC with fixed signed/unsigned division,
negative remainders, 32/64-bit boundary values, differing shift operand types,
signed negative right shifts and local reassignment examples. Four negative
construct tests (global reads, calls, floating-point and pointers) still
reject as expected. As in C, division by zero, signed overflow such as
INT_MIN / -1 and invalid shift counts are **undefined behavior** and are not
accepted as correctness test vectors.

**M3 completed for ordinary scalar C integer expressions (2026-10-28).**
The CFG backend now supports lazy `&&`, `||`, `?:`, comma sequencing,
unary `!`/`~`, prefix/postfix `++`/`--`, nested assignments,
and compound `+=`, `-=`, `*=`, `/=`, `%=`, `<<=`, `>>=`, `&=`,
`|=`, and `^=`. It uses Cranelift control-flow blocks to ensure
unselected branches are not evaluated and typed SSA variable slots to
preserve side effects and update values. Writable parameters and ordinary
nonvolatile local variables share that implementation.

Compound operations apply integer promotions and usual arithmetic conversions
before evaluation; a mixed unsigned 32-bit / negative signed 32-bit divisor
now uses the correct 32-bit unsigned representation.

[Native CI #40701834739](https://github.com/bingordo/edgcpp-rust/actions/runs/40701834739)
passed the GCC differential C++-linkage tests and **C11-mode differential
tests** (including prefix/postfix and parameter writes), 93 bridge tests,
the targeted Cranelift and ELF tests, and four unsupported-feature guards.
The generated test binaries are linked with the GCC-built driver; this is
an executable comparison, not just a compilation test.

**M3 scope boundary:** field/array/pointer lvalues, address-taken or
volatile/qualified locals, external globals, calls, and floating-point
expressions require M4–M6. Undefined C behavior is not compared.
This milestone is not a claim of full C or C++ support, nor of passing
YARPGen-generated `func.c`; that is gated separately.

## M4 scalar global memory and ELF linkage (2026-10-29)

**M4's ordinary scalar global-memory gate is complete.** The EDG bridge now
exports native global storage/linkage metadata and typed variable references.
The CFG emitter preserves scalar global lvalues as loads and stores, including
compound assignments, increments, sign/zero extension and `_Bool`. The
ELF backend resolves imports and produces initialized data, BSS and
internal/external definitions.

The [external-global differential CI #40794318473](https://github.com/bingordo/edgcpp-rust/actions/runs/40794318473)
and [defined-global data/BSS CI #40794819868](https://github.com/bingordo/edgcpp-rust/actions/runs/40794819868)
both passed against GCC-built reference executables. See
[implementation details and limits](docs/M4_SCALAR_MEMORY.md).

**Still unsupported:** pointers/arrays and address-taken stack locals
(M5), calls (M6), volatile/atomic objects, TLS and dynamic global
initialization. These are rejected, not accepted as successful compilation.
Generic YARPGen `func.c` remains a future M5–M7 gate.

## M5 pointer, fixed-array and generated-C validation (2026-10-29)

**M5's scoped pointer/array gate is complete.** The EDG/Cranelift reference
now lowers pointer parameters/returns, fixed-size local and global arrays,
1-D/2-D indexing, scaled pointer arithmetic and pointer differences,
null comparisons, indirect scalar writes, aliases and address-taken
stack locals. Data-pointer casts, byte stores and C11 zero-argument function
definitions are covered by the executable GCC differential tests.

[CI #40833609330](https://github.com/bingordo/edgcpp-rust/actions/runs/40833609330)
passed the complete existing targeted suite and checked eight **genuine**
pinned YARPGen programs against independent GCC and Clang references.
Three seeds (**12378, 12383, 12409**) compiled and ran through
EDG/Cranelift with matching checksums. Five seeds were rejected as
unsupported; no wrong output was observed. These eight deliberately small
seeds are an acceptance smoke test, **not** a coverage percentage for
YARPGen as a whole.

**Next M6** must handle generic GNU `({...})` statement-expression
blocks, direct calls and broader source semantics. Brace aggregate
initializers, variable-length arrays, volatile/atomic/TLS storage and
general C++ are not claimed as M5 support. See the
[consolidated milestone plan](docs/YARPGEN_CRANELIFT_MILESTONES.md).
