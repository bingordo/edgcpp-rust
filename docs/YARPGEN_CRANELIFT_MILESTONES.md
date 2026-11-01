# Milestone plan: EDG → Cranelift reference compiler with YARPGen validation

Status: **M0–M5 completed with explicit scope limits; M5 pointer/array and first real-YARPGen execution gates CI-verified (2026-10-29). Next M6 calls, GNU block-valued expressions and remaining source semantics.**

M0: [versioned YARPGen binary](https://github.com/bingundu/yarpgen/releases/tag/yarpgen-binary-20261028-1),
source SHA `251c5a148ade11d039d5bbbb1e5917b89bd5a785`,
Linux archive SHA-256 `fc3c7c80c10c67782c03f9d8b70c426f942332e78701397a14a5edff81cbc2e8`.

M1: deterministic Python harness, negative classification tests, 10-seed
push CI and manual cohorts up to 100 seeds.
[100-seed GCC/Clang baseline run](https://github.com/bingordo/edgcpp-rust/actions/runs/40648943692):
seeds **12345–12444**, GCC -O0, GCC -O2, Clang -O2, all **100 pass**,
no generator failures and no oracle disagreements. This is the
**GCC/Clang oracle baseline only**: the M5 reference backend now passes **3 of 8 deliberately small, pinned**
YARPGen programs, but this 100-seed GCC-only reference result is **not**
100 EDG/Cranelift passes. Broad generated-C compatibility is still unproven.

Status updated: 2026-10-29 (original plan 2026-10-28).
Development and CI: `bingordo/edgcpp-rust`, branch `cranelift-codegen`.
Source mirror: `bingundu/edgcpp-rust`, branch `cranelift-codegen`.
Generator: `bingundu/yarpgen`, branch `ci/binary-builds`.

## Objective and completion definition

Build a **reasonably complete C reference backend for YARPGen's loop generator**
on Linux x86-64, using EDG's frontend and Cranelift for machine code. The output
must link with a GCC-built YARPGen driver, execute, and agree with independent
GCC/Clang reference results. This is NOT a goal to implement all C++.

The target is the generator's standard C output first, using controlled
generation options to isolate front-end/backend features, followed by ordinary
(unconstrained apart from C language and resource limits) generated C.

Suggested release gates (targets, not claims about current coverage):

* **Functional gate:** 1,000 fixed, recorded `--std=c` seeds under the
  constrained profile: >=95% compile+link+execute with matching results,
  no unexplained wrong-code among accepted programs, and every failure
  categorized. Verify on two independent runs.
* **Breadth gate:** 1,000 fixed seeds with default C generation options,
  except explicitly recorded resource limits: >=90% compile+execute+match,
  zero unexplained mismatches. This is a stretch goal if optional pragmas
  exercise unsupported directives.
* **Stress gate:** 10,000 deterministic or persisted random C seeds on a
  scheduled/manual job with failure bucketing and reproduction, not on every
  push. Do not relabel unsupported cases as passes.
* **Safety gate:** Unsupported/ambiguous language forms fail with diagnostics
  without emitting an object that could be silently wrong.
* **Interop gate:** `func.c` compiled by EDG/Cranelift and `driver.c` by GCC
  run in one process. ABI, external data relocations, and linkage are tested.
* **Correctness across Cranelift modes:** test `opt_level=none` and `speed`
  on a smaller fixed corpus, alongside GCC `-O0`/`-O2` and Clang where
  available.

Do not call it "95% C compliant": the gate concerns a *pinned YARPGen
distribution* under stated flags and seeds, not all C programs.

## Verified starting point

The current reference backend already supports a deliberately narrow
**signed-i64** C-linkage function ABI, integer constants, locals,
add/subtract/multiply/negate, comparisons, bitwise AND/OR/XOR, if/else,
for/while/do loops, compiler-generated loop break/continue, SSA joins and
ELF object emission. GCC differential CI, bridge tests and negative ABI
checks currently pass. This is not yet sufficient to link generated YARPGen.

Current blockers: functions returning `void`, all narrower/unsigned C
scalar types, normal C promotions/conversions, C expressions with side
effects, global/external data access, arrays/pointers, parameters of varying
types and calls, wider control-flow constructs, and C translation-unit
linkage. The existing `lower_edg()` helper is a historical pure-expression
API; `emit_edg_object()` uses `codegen/cfg.rs`.

### What YARPGen actually emits

`bingundu/yarpgen` is the modern loop-generating YARPGen branch, not the
separate historic scalar `v1` generator. `--std=c` produces `init.h`,
`func.c`, `driver.c`; `--std=c++` produces corresponding `.cpp` files.
`func.c` defines `void test(...)`, which reads/writes external scalar
variables or arrays and may accept scalar/array parameters. `driver.c`
defines the globals, initializes them, calls `test`, hashes globals,
and prints a decimal result. Therefore **support for calls, void, external
data and typed memory is essential**; supporting more i64 operations alone
cannot run typical YARPGen outputs.

YARPGen's advertised correctness excludes undefined behavior but permits
implementation-defined behavior. Capture target/ABI/optimization flags and
validate the GCC/Clang oracle; do not assume all implementation-defined
behavior is identical across configurations.

### Critical distribution finding — no release yet

As inspected on 2026-10-28, `bingundu/yarpgen` has **zero published
GitHub Releases**. Workflow
`.github/workflows/binary-builds.yml` on `ci/binary-builds` successfully
uploads Linux x86-64 (plus aarch64/macOS/Windows) binary archives as
**Actions artifacts**. The latest successful binary workflow checked was
[run 39382815359](https://github.com/bingundu/yarpgen/actions/runs/39382815359).
Its artifacts expire on **2026-11-19**. On a `v*` tag push, the same
workflow's `release` job publishes permanent versioned GitHub Release
assets. **M0 must create/verify such a release**; do not pin an expiring
Actions artifact as a permanent build dependency. Until then, an Actions
artifact can serve only as a temporary manual bootstrap.

## M2 implementation checkpoint (2026-10-28)

The first M2 implementation landed in
[bingordo/edgcpp-rust](https://github.com/bingordo/edgcpp-rust/tree/cranelift-codegen)
and passed [CI #40654173606](https://github.com/bingordo/edgcpp-rust/actions/runs/40654173606).

* EDG bridge exports ordinary 8/16/32/64-bit integer widths and signedness.
* CFG emitter supports `void` return signatures and `return;`, including
  implicit fallthrough returns from void functions.
* Cranelift signatures use actual parameter and result widths for safe
  direct parameter/constant returns, with proper parameter widening and
  return narrowing. Signed 8-bit, unsigned 32-bit and 16-bit direct returns
  have GCC native execution checks.
* **Completed next checkpoint** in
  [CI #40661637692](https://github.com/bingordo/edgcpp-rust/actions/runs/40661637692):
  signed/unsigned 8–64-bit pure-return expressions carry EDG's native types
  through the CFG lowering. Integer conversions and intermediate truncation
  are explicit; unsigned arithmetic wraps at the source width; signedness
  determines signed vs unsigned relational comparison predicates. Unsigned
  64-bit constants preserve all bits. GCC differential execution passed
  expanded edge-case fixtures, along with 93 bridge tests, six targeted
  codegen tests, native ELF execution and negative ABI checks.
* **Typed local SSA checkpoint:** [CI #40687389836](https://github.com/bingordo/edgcpp-rust/actions/runs/40687389836)
  passed GCC differential tests for narrow signed/unsigned locals, local
  assignments, 8/16/32-bit truncation and wrapping, conditionals, loops
  and return narrowing. Each store normalizes to the local's declared
  C width/sign and stores the canonical value in an I64 SSA variable;
  no special-purpose peephole pass was introduced. Any return site
  now honors the routine's ABI result width.
* **Native boolean checkpoint:** [CI #40687818445](https://github.com/bingordo/edgcpp-rust/actions/runs/40687818445)
  passed after adding EDG's native bool_type metadata, distinct from plain
  unsigned 8-bit integers. C _Bool/C++ bool conversions, parameter/result
  ABIs and local stores canonicalize nonzero to 1; native differential
  fixtures test zero, positive, negative, and branch-dependent assignments.
* **Still unsupported (later milestones):** qualified, volatile or
  address-taken locals, global memory, pointers/aggregates, calls, and
  floating-point types. The CFG emitter now supports sequenced scalar
  side effects and short-circuit logic. The legacy expression-only
  API remains signed-i64.
* **M2 scalar checkpoint:** ordinary integer/`_Bool` parameters, returns,
  typedef-aware width/sign and local SSA are validated, including C11.
  Volatile/addressable/qualified object semantics require M4 memory and
  will not be silently emulated as pure SSA scalars.

## M4 completed: scalar global memory and ELF linkage (2026-10-29)

The reference backend now emits typed global scalar loads and stores, linked
external data relocations, file-scope initialized data and zero-initialized BSS,
including internal and exported storage. The EDG bridge records native
storage/linkage metadata and resolves variables by symbol identity. M3
sequencing, conversions and compound assignments are reused unchanged.

[CI #40794318473](https://github.com/bingordo/edgcpp-rust/actions/runs/40794318473)
passes GCC differential execution with four globals defined in the C driver.
[CI #40794819868](https://github.com/bingordo/edgcpp-rust/actions/runs/40794819868)
passes additional object-owned data/BSS/internal-linkage execution and the
existing bridge, Cranelift, scalar C11 and negative tests.
See [M4 implementation and acceptance](M4_SCALAR_MEMORY.md).

**Scope adjustment:** The original M4 plan also proposed address-taken local
stack storage. Correct observation of address-taking requires the pointer
lvalue/address semantics introduced in M5; those linked changes move together
to M5 instead of introducing untestable stack-slot code. M4 completion is
therefore specifically the independently testable ordinary-scalar
memory/linkage gate, not full pointer/qualified-object support.

## M3 complete: ordinary scalar C expressions (2026-10-28)

**Implementation:**

* EDG's native operator semantics are exported through stable C/Rust semantic
  IDs. Rust does not depend on the private EDG enumeration layout.
* The shared integer IR and Cranelift emitter implement signed/unsigned
  division/remainder, shift operations, signedness-aware comparisons,
  unary logical/bitwise operators, lazy logical AND/OR, ternary `?:`,
  comma evaluation, simple and nested assignments, all ten supported
  arithmetic/bitwise compound assignments, prefix and postfix increments
  and decrements.
* `&&`, `||`, and `?:` lower into real CFG with SSA merges. Skipped
  operands are not evaluated, including mutations of local variables.
  Sequenced comma operations evaluate the left side before the right.
* Ordinary typed local variables and writable function parameters use
  unified Cranelift SSA slots. Values normalize at C stores and respect
  declared widths and signedness. Prefix returns the updated value,
  postfix returns the previous one. Integer promotions and usual
  conversions are applied for compound arithmetic, including mixed
  unsigned/signed 32-bit divisors.
* No one-off peephole or optimization pass was added.

**Verification:**

* [CI #40690504958](https://github.com/bingordo/edgcpp-rust/actions/runs/40690504958):
  signed and unsigned 32/64-bit division/remainder, shifts and negative
  shift/right-boundary GCC comparisons.
* [CI #40699609390](https://github.com/bingordo/edgcpp-rust/actions/runs/40699609390):
  lazy expression/assignment/side-effect GCC comparisons.
* [CI #40700242021](https://github.com/bingordo/edgcpp-rust/actions/runs/40700242021):
  C usual arithmetic conversion regression cases for compound operators.
* [CI #40701242410](https://github.com/bingordo/edgcpp-rust/actions/runs/40701242410):
  EDG C11 frontend and GCC differential compilation/execution, prefix
  increment/decrement and `_Bool`.
* [CI #40701834739](https://github.com/bingordo/edgcpp-rust/actions/runs/40701834739):
  final C11 tests including writable parameters, plus C++-linkage
  differential tests, 93 bridge tests, targeted ELF/codegen tests and
  deliberate rejection of unsupported global/pointer/call/float cases.

**Completion boundary:** M3 covers ordinary defined integer/boolean
expressions with scalar SSA locals and writable integer parameters. C
undefined behavior (zero divisors, invalid shift counts, signed overflow)
does not have an expected comparison result. Address-taken/volatile or
qualified locals, aggregate/pointer lvalues and external memory move to
M4/M5; calls move to M6. Higher-order or target-specific expression cases
surfaced by YARPGen belong to the M7 corpus-debugging loop.

This does **not** mean YARPGen `func.c` currently compiles or passes. M1's
100/100 refers only to GCC/Clang oracle baselines. The next main gate is
M4: correct external/global scalar memory and ELF data relocations.

## Milestones and dependency gates

| Milestone | Implementation scope | Acceptance / gate | Relative effort |
|---|---|---|---|
| **M0 — Versioned generator** | Tag an audited commit containing binary-build workflow in `bingundu/yarpgen`; publish Linux-x86_64 tarball; pin tag, source SHA and archive SHA-256; verify executable `--version` and a sample `--seed=12345 --std=c` case. No YARPGen source compilation in compiler CI. | A durable downloadable release; unpacked `yarpgen` runs; generated `init.h`/`func.c`/`driver.c` verified. | S |
| **M1 — Test harness and baseline** | Add deterministic `tools/yarpgen_smoke.py` (or small Rust equivalent), seed manifests, resource limits, GCC/Clang baseline and GCC-only / Cranelift+GCC linking, structured result manifest, repro bundle and classifier. Initially report unsupported backend cases rather than marking CI failed for lack of coverage. | Run and retain baseline for >=100 generator seeds; 10 fixed fixtures on CI; reproduce a selected failure with one command. No unsupported-as-pass counting. | M |
| **M2 — C scalar types and ABI** | Explicit EDG native metadata: integer rank, width, signedness, qualifiers, typedefs, `_Bool`, `void`; typed Cranelift signatures, C integer promotions/usual conversions, truncation/extension, comparisons and return; preserve correct x86-64 SysV integer parameter classifications. Native C names and a `void test(void)` export. | Direct native tests for 8/16/32/64-bit signed/unsigned, `char`, bool, casts, overflow boundaries where defined; intentionally wrong signatures rejected. | L |
| **M3 — Complete scalar expressions** | Division/remainder, shifts and signedness, logical `&&`/`||` with short-circuit semantics, unary `!`/`~`, `?:`, comma, assignment expressions, prefix/postfix increment, compound assignments, sequencing and side-effect correctness. Avoid eager selects or reordering for effects. | GCC/Clang differential edge tests; no unmodeled side effects; deterministic generated scalar-expression corpus. | L |
| **M4 — Scalar memory, objects and linkage** | Addressable locals as stack slots when needed; global and extern scalar declarations; ELF data/bss, external data relocations; loads/stores with alignment/width/extension; correct linkage/visibility, type/layout validation, linker diagnostics. Distinguish SSA scalars from address-taken storage. | Cranelift-generated `void test()` modifies external globals defined by GCC-built `driver.c`, with matching observable state and checksum. | L |
| **M5 — Arrays, pointers and indexing** | Address-taken local stack slots, pointer types/ABI and decay; address-of/dereference, indexing and pointer arithmetic; multidimensional C arrays and element strides; lvalues, explicit scalar-element initialization and casts, bounded addressing, aliasing, aligned and unaligned accesses. Aggregate brace initializers move to M6 with generic source-value lowering. Don't falsely optimize through potentially aliasing pointers. | Handwritten 1-D/2-D array kernels, parameter arrays and globals, plus a real generated seed with GCC matching output. | XL |
| **M6 — Calls, full CFG and source semantics** | Direct calls and C ABI interop; multiple functions/translation-unit references, void calls, argument conversions; fix loop/branch corner cases, `switch` if emitted, labels and generated goto shapes as required. Enforce C evaluation order, side effects and definite initialization without overly conservative false rejection. | Representative generated `func.c` cases compile and link; >=100 pinned seeds triaged; no invalid Cranelift verifier or relocation errors. | L–XL |
| **M7 — Coverage-driven YARPGen completion** | Iterate on ranked failure buckets (not features in arbitrary order); enable wider generator profiles, optional pragmas/alignments if semantically relevant. Build reducer and regressions for each correctness bug. Run both Cranelift `none` and `speed`. | Meet the functional and breadth gates above over pinned 1,000-seed cohorts, including independent GCC/Clang baseline checks. | XL |
| **M8 — Stabilize and reuse** | Freeze fixed seeds plus minimized regressions; verify reproducibility on clean Linux CI; document unsupported cases and cross-module boundaries; prepare shared front-end lowering for `bingundu/compiler` without coupling its IR to Cranelift. | 10k-seed monitored stress with useful triage; explicit ready/not-ready report and regression corpus; benchmark resource cost. | M–L |

**Ordering and focus:** M0/M1 first, then M2/M3 to establish correct semantics; M4/M5 unlock actual YARPGen's global-array-heavy programs. M6 resolves interop and CFG gaps found on generated cases. M7 is iterative and may pull M3/M6 subfeatures forward based on empirical unsupported-operation counts. Partial C++ support and full C++ name mangling are explicitly **out of scope** for the baseline.

## Harness contract: three compile paths, one oracle

For each generator seed:

1. Generate a fresh isolated directory with the **pinned executable**,
   seed, standard, and recorded options. Verify all three emitted files.
2. Compile both source files with GCC using fixed language/target flags
   at `-O0` and `-O2` (two independently produced executables).
   Optionally build Clang `-O2` to cross-check the oracle.
3. Compile `func.c` through the EDG frontend and Cranelift into a
   **relocatable .o**. Compile `driver.c` with GCC at controlled options.
   Link through GCC. Preserve object relocation diagnostics.
4. Run all binaries under CPU/memory/wall-time limits in separate processes;
   compare exit codes, decimal checksum stdout, and any `ERROR:` markers.
   Require the reference compilers to agree before reporting a wrong-code
   case against EDG/Cranelift; classify reference disagreements separately.
5. Record a JSON case manifest (seed, mutation seed, generator SHA/tag/hash,
   profile, C standard, target triplet, compiler versions and flags,
   source hashes, build/run command lines, outcome classification).
6. Keep full generated source, log and reproduction script only for failures;
   do not upload every passing generated program as an artifact. Reduce
   stable wrong-code failures with C-Reduce or a comparable minimizer.

An initial *illustrative* generator invocation:

~~~sh
mkdir -p case-12345
yarpgen --seed=12345 --std=c --out-dir=case-12345 \
  --inp-as-args=none --emit-pragmas=none --emit-align-attr=none \
  --omp-simd=off --max-array-dims=2
~~~

`--inp-as-args=none` narrows the signature surface but **increases global
memory dependence**. `--max-array-dims=2` limits rank, not total bytes:
enforce memory, per-process time and object-size limits independently.
The generator's `--simple-loops=all` (with
`--vectorizer-target=gcc-clang`) can define another controlled subprofile,
but should not substitute for testing the ordinary loop generator.

### Case classification and reporting

Keep separate, machine-readable counters for:

| Case outcome | What it means |
|---|---|
| `baseline_disagree` | GCC/Clang or controlled GCC builds disagree, so no trusted oracle |
| `generator_failure` | Generator crashed, timed out or omitted required files |
| `frontend_error` | EDG rejects source or exposes malformed/incomplete IL |
| `unsupported_construct` | Explicit backend omission with actionable diagnostic |
| `codegen_failure` | Cranelift verifier, ISA lowering or codegen failure |
| `link_failure` | Symbol, ABI, external data or relocation failure |
| `runtime_failure` | Trap, signal, timeout, crash, missing executable or resource failure |
| `wrong_output` | Fully accepted case with differing result from established oracle |
| `pass` | Successful compile, link, run and exact reference agreement |

Use denominators that distinguish **generated**, **oracle-valid**,
**backend-accepted**, **compiled**, **executed**, and **passing**.
Never improve a reported success percentage by silently discarding
unsupported cases. Archive mismatches until disproven; a test timed out
at one optimization level is not a value comparison.

## CI execution and cost control

* Keep existing EDG source SHA and SHA-256-verified prebuilt EDG library;
  no repeated EDG native build.
* Download one pinned Linux x86-64 YARPGen **Release** tarball and verify
  SHA-256; do not rebuild YARPGen in this repo. Use repository-scoped
  read credentials if the release is private, never print tokens.
* Keep Rust/Cargo incremental cache and Cranelift codegen feature isolation;
  build the codegen binary **once per workflow**, not once per seed.
* Run 10 cheap fixed seeds on relevant codegen pushes after M1 is ready.
  Until first real seeds pass, those CI tests should focus on harness
  classification and on fixed unsupported behavior without falsely
  greenwashing unsupported generated code.
* Increase to ~20 fixed seeds for ordinary branch validation as compiler
  coverage grows. Run 100 seeds nightly and 1,000 weekly only after M5/M6
  make this productive. Make 10,000 a manual or infrequent stress workload.
* Prefer bounded parallel seed workers and CPU-time quotas to unconstrained
  parallel Cranelift/GCC builds. Cancel superseded CI runs and skip
  documentation-only changes as the existing workflow already does.
* Save only actionable failure bundles, aggregate CSV/JSON reports and
  minimized regressions. Cache toolchains/dependencies, not random source.
* The `bingordo` branch runs CI. Mirror verified source/docs into
  `bingundu`; its workflow remains manual-only to avoid double expenditure.

**Coverage metrics:** status totals by feature bucket, wrong-output count,
pass fraction among all oracle-valid seeds, crash/hang count, median/p95
compile+run time, generator seed profile, and a reproducibility score for
failures. Record EDG/Cranelift/GCC/Clang versions for every cohort.

## Engineering principles and critical risks

**Model C semantics before adding instructions.** C integer promotions,
unsigned overflow, signed right shifts, narrowing, operation order, volatile
and aliasing cannot safely be inferred from every EDG expression's result
type alone. The bridge must capture explicit native metadata or reject
what it cannot classify. Do not overfit by editing YARPGen outputs into an
easy subset and still claim default-generator support.

**Preserve evaluation order.** The existing eager `Expr::Select` path is
not safe for effectful expressions. Use true blocks, explicit lvalues,
typed loads/stores and sequencing when representing `?:`, `&&`, `||`,
assignments and calls.

**Make external data real.** YARPGen's `test` often reads/writes globals
and arrays allocated in the GCC driver. The reference backend needs correct
external data symbol declarations, relocations, addressing and ABI, not just
standalone local SSA values.

**Keep the reference understandable.** Introduce a small typed, backend-neutral
representation for expressions, lvalues, statement CFG and typed symbols
rather than layering every new feature into the original untyped `Expr`
and `cfg.rs`. Reuse that decoding and semantic checking for the future
`bingundu/compiler` connection. The reference remains a *correctness
oracle*, not a competing general optimizing compiler.

**Testing priority:** wrong-code, invalid ABI and nondeterministic
miscompiles outrank unsupported-but-diagnosed features. Do focused unit
regressions per change, bounded end-to-end corpus per milestone, and infrequent
full seeded runs.

## M5 completed — pointer/array memory and initial generated-C acceptance (2026-10-29)

**Implementation (verified):** Stable, native-derived EDG operator/type IDs;
SysV x86-64 data-pointer parameters and returns; fixed-dimension arrays,
1-D/2-D indexing and element strides; signed pointer differences, scaled
addition/subtraction, pointer comparison and null-pointer constants; typed
dereference, indirect assignment, compound assignment and prefix/postfix;
address-taken scalar locals and fixed-size stack arrays with real stack slots;
alias-safe memory loads/stores and byte-width operations; ordinary pointer
casts and GCC-owned initialized array data. C11 `void test()` definitions
with no parameters are also supported.

**GCC differential:** `tests/fixtures/m5_pointer_arrays.c` covers a
GCC-owned global array and grid, pointer parameters, local array storage,
indirect mutations, aliasing, pointer differences/comparisons, `void*`
roundtrips and byte loads/stores. The test is a real linked execution, not
only parsing. The expanded fixture, M0–M4 regressions, 93 bridge tests and
negative guards passed in
[CI #40833609330](https://github.com/bingordo/edgcpp-rust/actions/runs/40833609330).

**Real YARPGen acceptance:** The same green run used the pinned YARPGen
binary and GCC/Clang independent reference. Of eight recorded, intentionally
small seeds selected by compile cost from the prior 100-seed oracle cohort,
**12378, 12383 and 12409 passed EDG/Cranelift compile, link and checksum**;
five were explicitly classified `unsupported_construct`; there were
**zero wrong outputs, verifier errors, crashes or link failures**. This
meets M5's acceptance requirement of at least one genuine generated seed.
It is **not** representative breadth/coverage: M6/M7 must test larger,
unselected 100-/1,000-seed cohorts.

**Explicit limits:** GNU `enk_statement` block-valued expressions
(`({...})`, used by YARPGen `min`/`max`), direct calls, field/aggregate
lvalues, variable-length arrays, dynamic initialization, general brace
aggregate array initializers, volatile/atomic/TLS semantics and full C++
ABI remain unsupported. Rejections must occur before an ELF object is
created. Generic statement-expression and aggregate-initializer semantics
belong in the generic M6 source/lvalue framework, not special-case macros.

## Immediate next implementation task

**M6: calls, full CFG and source semantics**. First add a generic block-valued
expression IR for EDG `enk_statement`, preserving nested scopes, temporaries,
sequencing, resulting value and control-flow exits. Reuse that framework for
aggregate/compound initialization rather than adding one-off `min`/`max`
passes. Then implement direct C calls, function references, ABI argument
conversion, remaining switch/goto cases, and reduction tests. Run 100 pinned
YARPGen cases in a manually triggered CI cohort only after the targeted
regressions are green; keep failures classified by root cause.

Useful references:

* `bingundu/yarpgen/.github/workflows/binary-builds.yml` on `ci/binary-builds`
  — versioned `v*` release job and five archive artifacts.
* `bingundu/yarpgen/src/program.cpp` — emits `init.h`, `func.*`,
  `driver.*`, `void test(...)`, external variables/arrays and checksum.
* `bingundu/yarpgen/src/options.cpp` — documented generator flags.
* `bingordu/edgcpp-rust/examples/codegen.rs` and
  `.github/workflows/build.yml` — current ELF frontend CLI and pinned EDG
  build/test mechanism.

## M7 progress — failure-driven generated-C coverage (2026-10-30)

The M7 harness now records normalized per-seed EDG/Cranelift diagnostics and
aggregates nonpassing outcomes by classification and diagnostic. The separate
`bingordo/compiler-ci` workflow publishes the most frequent failures and
coverage counts in its Actions summary, with a 15-minute change-detection
schedule. A green workflow proves only the configured cohort and acceptance
threshold; it is not evidence that the unsupported-language backlog is done.

**Implementation check:** Current `src/codegen/cfg.rs` already contains
straight-line GNU block-value expression lowering and direct-call lowering.
The earlier M5 section above is historical, not a current inventory; do not
reimplement those capabilities without first identifying a failing case.
Arbitrary non-loop goto/labels and non-straight-line GNU statement expressions
still have explicit guards. Their frequency in unselected YARPGen cohorts
has not yet been established.

**Next codegen gate:** inspect the ranked diagnostic groups from a completed
unselected cohort, select the most frequent *supported-oracle* unsupported
construct, add a reduced C differential fixture, then extend generic EDG
lowering and rerun the same pinned seeds. Record generated/oracle-valid,
accepted, passing, unsupported and severe-failure counts before and after.
Do not increase the automatic cohort size until this produces measurable
acceptance improvement.


## M8–M13 — C coverage roadmap (planned, 2026-10-30)

**Scope and sequencing.** Defer any further GNU statement-expression work:
the existing straight-line scalar support remains, but general nested
control flow, nontrivial lifetimes and GNU block-value extensions are
explicitly *out of scope* until this roadmap is complete. Do not regress
existing M6 behavior. Use a small typed/lvalue framework and EDG metadata
rather than adding source-pattern-specific rewrites to `cfg.rs`.

### M8 — aggregate objects and initializers (start here)

* M8.1: inventory EDG record/union/field and initializer metadata; document
  size, alignment, layout, bitfield and relocation requirements; add negative
  tests proving unknown layouts are rejected before ELF emission.
* M8.2: add fixed-size record/union storage and field lvalues, including
  address-of, loads/stores and nested member/array access; support only
  layouts independently validated against the native EDG/GCC ABI.
* M8.3: support zero-filled and constant brace initializers for fixed arrays
  and records, including padding and relocatable references when representable.
* M8.4: implement local aggregate copy/initialization with correct ordering
  and alias behavior; extend SysV aggregate ABI only in a separately tested
  step. **Gate:** GCC differential fixtures (nested records, arrays, unions,
  alignment, global and local initialization) and explicit rejection tests.

### M9 — general statement CFG

* Introduce per-function label/block discovery, branch target validation and
  reachability; lower `switch`/case/default, fallthrough and arbitrary
  non-loop goto/labels without abusing break/continue recognition.
* Reject illegal jumps into scopes with unsupported initialization/lifetime
  constraints. **Gate:** GCC differential fixtures with loops, fallthrough,
  forward/backward jumps, nested scopes and unreachable blocks.

### M10 — call and ABI completeness

* Extend typed function references to indirect calls through function
  pointers, preserving prototypes, return conversions and symbol linkage.
* Support unprototyped and variadic C calls only with verified SysV x86-64
  default promotions and register/stack rules; handle aggregate arguments
  after M8's ABI representation is validated.
* **Gate:** separate C caller/callee compilation, GCC interop in both
  directions, negative mismatched-prototype tests.

### M11 — VLAs and dynamic automatic storage

* Model runtime dimensions, checked size calculations, dynamic stack
  allocation, nested-scope lifetimes and early exits. Preserve `sizeof`
  semantics and reject unsupported dynamic initializers explicitly.
* **Gate:** GCC differential for VLA indexing, nested scopes, bounds-related
  size expressions and control-flow exits; stack-stress cases.

### M12 — volatile and atomics

* Track volatile qualifiers in EDG memory operations; ensure observable
  accesses are not eliminated, combined or reordered across sequencing.
* Treat C11 atomics as a separate subphase with explicit ordering,
  alignment, target instruction selection and fallback semantics. Do not
  silently lower atomics to ordinary loads/stores.
* **Gate:** IR/assembly assertions plus differential single-thread tests;
  targeted concurrency litmus tests only after a sound memory model.

### M13 — semantic correctness audit and broader coverage

* Target integer promotions/narrowing, signed/unsigned operations, pointer
  provenance and aliasing, evaluation sequencing, casts and ABI boundaries.
* Add focused GCC/Clang differential regressions and property-based
  generation. Keep 10-seed automatic YARPGen and manually run unselected
  100/1000-seed cohorts after targeted checks pass; record accepted/passed,
  unsupported and severe failures separately. No performance benchmarking
  gate yet.

### M8 implementation checkpoint (2026-11-01)

**Implemented and GCC-differential-tested (PRs #5–#24):**

* Native EDG object size and alignment, field identities and offsets, and
  recursive bounded layout validation for C structs, nested structs, fixed
  scalar arrays and ordinary unions. Exact layouts come from EDG; no guessed
  field positions or ABI padding.
* Aligned local/global storage, scalar `.` and `->` member access, array
  members, aggregate copies (fieldwise for structs; bounded bytewise for
  unions), and zero-initialization for explicitly represented full-zero
  aggregate objects.
* Bounded local and global constant aggregate object images from EDG's
  `ck_aggregate`/`ck_designator` tree, including partially initialized
  and designated arrays, records and unions. Some native repeated constants
  are supported; unsupported dynamic entries fail closed.
* Runtime nonconstant braced initialization *as lowered by EDG* for the
  tested scalar leaf expressions, arrays, nested records and unions. The
  GCC differential fixture also verifies a side-effectful call occurs once,
  unspecified fields read as zero, and nested/multidimensional designated
  initialization works; this is not a claim of full C99 initializer coverage.
* Standalone aggregate copies are bounded and reject unsupported aggregate
  value uses. Existing codegen and regression tests stay enabled.

**Validation:** `tests/fixtures/m8_struct_fields.c` and
`m8_struct_driver.c` compile through both EDG/Cranelift and GCC with
matching results. The pinned EDG archive and matching source are also
tested by `bingordo/compiler-ci`'s
`.github/workflows/test-edgcpp-m8.yml`. In particular, run
`41326462848` passed the side-effect, nested, union and designated
initializer differential tests on the `feature/m8-init-semantics-regressions`
commit, subsequently merged as PR #24. This supplementary CI workflow
is a verification route, not a separate compiler implementation.

**Remaining before declaring M8 complete:**

1. Relocatable static pointer/address initializers for aggregates and
   validation of ELF relocations against GCC (not merely integer images).
2. Bitfield layout, read/modify/write and constant initialization, using
   native EDG bit offsets/widths with packed/union/overlap diagnostics.
3. General nonconstant aggregate initializer trees and source-order
   evaluation in cases where EDG does not already lower them into supported
   executable statements. Add negative controls and dynamic-call cases.
4. SysV x86-64 aggregate parameter/return classification, register/stack
   passing, hidden-result storage and interoperability tests with GCC.
5. Coverage expansion for flexible members, packed/overaligned layouts,
   qualifiers, unsupported pointer/union corner cases, compound literals,
   and aggregate-as-expression uses.

**Boundaries:** GNU statement expressions remain deferred as agreed. The
current bitfield, aggregate ABI, relocatable initializer and extended
lifetime cases must continue to reject unsupported semantics rather than
silently emitting scalar or incorrectly aligned code.
