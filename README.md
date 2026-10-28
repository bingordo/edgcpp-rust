# EDG → Cranelift reference code generator

This branch is a fresh **code-generation implementation**, built on the
previously working EDG C/C++ frontend bridge. The historical implementation,
including experimental type dependency and interprocedural analysis, remains
on [main](https://github.com/bingundu/edgcpp-rust/tree/main).
The earlier pre-codegen snapshot is preserved on
[codegen-integration](https://github.com/bingundu/edgcpp-rust/tree/codegen-integration).

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

## Milestone 2: EDG → bingundu/compiler

Once the Cranelift reference path passes executable tests, lower the same
supported EDG constructs to `bingundu/compiler`'s frontend-neutral IR.
Compare results against GCC and the Cranelift reference implementation.
Keep the native compiler backend independent of EDG.

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
results are supported; other width-changing conversions remain unsupported.

**ABI guard:** the bridge records which EDG integer types are verified
signed 64-bit types (excluding enums). The reference backend checks the
routine's declared return type, prototype/variadic status and every declared
and defined parameter type before emitting machine code. It resolves typedef
wrappers but rejects non-i64 widths, unsigned integers, pointers, floating
point, missing metadata, parameter mismatches and implicit C++ object
parameters. Only plain signed-64-bit C-linkage functions are currently
supported; C++ name mangling is not implemented.

Local assignments, structured if/else and pretest while loops are supported
by the newer CFG emitter (see below). Calls, global memory and general C++
features are still unsupported. The signed-i64 restriction remains intentional.

The differential CI test compiles `tests/fixtures/codegen_simple.cpp` through
both EDG/Cranelift and GCC, links each with the same test driver, and compares
execution results. Unsupported EDG constructs are rejected rather than lowered
incorrectly. This is a reference backend, not a general C++ compiler.

**CI is run on [bingordo/edgcpp-rust](https://github.com/bingordo/edgcpp-rust/actions/workflows/build.yml).**
The `bingundu` mirror uses a manual-only workflow to avoid duplicate CPU usage.

The CI negative-ABI fixture verifies that int parameters, unsigned returns,
floating-point returns and pointer parameters are rejected without creating
object files. Unit tests cover missing/cyclic type metadata and mismatched
signatures.

## Milestone: signed-i64 local variables

The reference code generator now handles a straight-line sequence of local
signed-i64 declarations, dynamic initializers, simple assignments and
reassignments followed by an unconditional return or a pure returning if/else.
The bridge retains initializer expressions as primary expression trees as well
as their existing role-4 control snapshot. Each assignment becomes an ordered
SSA value update (no explicit memory slot), with rejected uninitialized reads.

The supported subset still excludes initializer forms without a captured
expression, globals, static locals, parameter writes, pointer and array
assignment, narrow/unsigned locals and side-effecting expressions. The initial
straight-line implementation is preserved for reference; the newer structured
CFG backend supports assignment in conditional branches and while loops.

CI performs native execution with two sequential locals plus reassignment,
checks a local value across returning if/else branches, and compares outputs
with GCC over a signed input grid. Standalone ELF tests additionally check
multiple writes to one local and rejection of uninitialized local reads.

## Structured control-flow milestone

The ELF-emitting EDG reference backend uses real Cranelift basic blocks and SSA
variables for mutable signed-64-bit locals in if/else (with optional else) and
pretest while loops. Loop headers remain unsealed until the backedge is built.
A path-sensitive definite-initialization check rejects reads of locals set
only in a potentially untaken branch or loop. Every reachable path must return.
The former lower_edg function remains a narrow pure-expression API;
emit_edg_object uses structured CFG lowering. Calls, pointers, globals,
complex C++ expressions and arbitrary goto control flow remain unsupported.

CI compares generated ELF with GCC for mutable branches and loop-carried
variables over the existing signed input grid.

## For / do-while control flow

The constrained signed-i64 CFG emitter now supports source-level `for` (with
assignment-style initializer/increment and a condition) and `do/while` loops.
For increments retain both EDG's original control expression and a bounded
synthetic statement carrying the complete executable expression tree. This is
needed to preserve numeric constants in increments. Generated basic blocks
handle forward/backward edges and Cranelift SSA merges. Differential tests
compare output to GCC across the existing signed input grid. At present,
source-level `break` / `continue` are accepted only when EDG's native
label flags identify a compiler-generated loop jump. Arbitrary goto and
switch-break targets are still rejected. Other unsupported operators, calls, pointers and non-i64 types
remain errors.

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

The CFG emitter now preserves EDG-native scalar integer width/sign (8, 16,
32, 64 bits) for function parameters and direct simple return values, and
supports `void` functions and `return;`. Cranelift arguments are extended
to the internal I64 representation and simple return values are narrowed
according to the declared C ABI. Full arithmetic/conversion on mixed-width
or unsigned values remains explicitly rejected until native C promotions,
intermediate truncation and signed/unsigned comparisons are implemented.
The separate historical pure-expression `lower_edg()` remains i64-only.
