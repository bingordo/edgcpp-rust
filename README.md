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
