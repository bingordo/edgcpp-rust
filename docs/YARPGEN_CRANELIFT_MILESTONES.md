# Milestone plan: EDG → Cranelift reference compiler with YARPGen validation

Status: **plan only — implementation milestones not yet complete**.
Date: 2026-10-28.
Development and CI: \`bingordo/edgcpp-rust\`, branch \`cranelift-codegen\`.
Source mirror: \`bingundu/edgcpp-rust\`, branch \`cranelift-codegen\`.
Generator: \`bingundu/yarpgen\`, branch \`ci/binary-builds\`.

## Objective and completion definition

Build a **reasonably complete C reference backend for YARPGen's loop generator**
on Linux x86-64, using EDG's frontend and Cranelift for machine code. The output
must link with a GCC-built YARPGen driver, execute, and agree with independent
GCC/Clang reference results. This is NOT a goal to implement all C++.

The target is the generator's standard C output first, using controlled
generation options to isolate front-end/backend features, followed by ordinary
(unconstrained apart from C language and resource limits) generated C.

Suggested release gates (targets, not claims about current coverage):

* **Functional gate:** 1,000 fixed, recorded \`--std=c\` seeds under the
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
* **Interop gate:** \`func.c\` compiled by EDG/Cranelift and \`driver.c\` by GCC
  run in one process. ABI, external data relocations, and linkage are tested.
* **Correctness across Cranelift modes:** test \`opt_level=none\` and \`speed\`
  on a smaller fixed corpus, alongside GCC \`-O0\`/\`-O2\` and Clang where
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

Current blockers: functions returning \`void\`, all narrower/unsigned C
scalar types, normal C promotions/conversions, C expressions with side
effects, global/external data access, arrays/pointers, parameters of varying
types and calls, wider control-flow constructs, and C translation-unit
linkage. The existing \`lower_edg()\` helper is a historical pure-expression
API; \`emit_edg_object()\` uses \`codegen/cfg.rs\`.

### What YARPGen actually emits

\`bingundu/yarpgen\` is the modern loop-generating YARPGen branch, not the
separate historic scalar \`v1\` generator. \`--std=c\` produces \`init.h\`,
\`func.c\`, \`driver.c\`; \`--std=c++\` produces corresponding \`.cpp\` files.
\`func.c\` defines \`void test(...)\`, which reads/writes external scalar
variables or arrays and may accept scalar/array parameters. \`driver.c\`
defines the globals, initializes them, calls \`test\`, hashes globals,
and prints a decimal result. Therefore **support for calls, void, external
data and typed memory is essential**; supporting more i64 operations alone
cannot run typical YARPGen outputs.

YARPGen's advertised correctness excludes undefined behavior but permits
implementation-defined behavior. Capture target/ABI/optimization flags and
validate the GCC/Clang oracle; do not assume all implementation-defined
behavior is identical across configurations.

### Critical distribution finding — no release yet

As inspected on 2026-10-28, \`bingundu/yarpgen\` has **zero published
GitHub Releases**. Workflow
\`.github/workflows/binary-builds.yml\` on \`ci/binary-builds\` successfully
uploads Linux x86-64 (plus aarch64/macOS/Windows) binary archives as
**Actions artifacts**. The latest successful binary workflow checked was
[run 39382815359](https://github.com/bingundu/yarpgen/actions/runs/39382815359).
Its artifacts expire on **2026-11-19**. On a \`v*\` tag push, the same
workflow's \`release\` job publishes permanent versioned GitHub Release
assets. **M0 must create/verify such a release**; do not pin an expiring
Actions artifact as a permanent build dependency. Until then, an Actions
artifact can serve only as a temporary manual bootstrap.

## Milestones and dependency gates

| Milestone | Implementation scope | Acceptance / gate | Relative effort |
|---|---|---|---|
| **M0 — Versioned generator** | Tag an audited commit containing binary-build workflow in \`bingundu/yarpgen\`; publish Linux-x86_64 tarball; pin tag, source SHA and archive SHA-256; verify executable \`--version\` and a sample \`--seed=12345 --std=c\` case. No YARPGen source compilation in compiler CI. | A durable downloadable release; unpacked \`yarpgen\` runs; generated \`init.h\`/\`func.c\`/\`driver.c\` verified. | S |
| **M1 — Test harness and baseline** | Add deterministic \`tools/yarpgen_smoke.py\` (or small Rust equivalent), seed manifests, resource limits, GCC/Clang baseline and GCC-only / Cranelift+GCC linking, structured result manifest, repro bundle and classifier. Initially report unsupported backend cases rather than marking CI failed for lack of coverage. | Run and retain baseline for >=100 generator seeds; 10 fixed fixtures on CI; reproduce a selected failure with one command. No unsupported-as-pass counting. | M |
| **M2 — C scalar types and ABI** | Explicit EDG native metadata: integer rank, width, signedness, qualifiers, typedefs, \`_Bool\`, \`void\`; typed Cranelift signatures, C integer promotions/usual conversions, truncation/extension, comparisons and return; preserve correct x86-64 SysV integer parameter classifications. Native C names and a \`void test(void)\` export. | Direct native tests for 8/16/32/64-bit signed/unsigned, \`char\`, bool, casts, overflow boundaries where defined; intentionally wrong signatures rejected. | L |
| **M3 — Complete scalar expressions** | Division/remainder, shifts and signedness, logical \`&&\`/\`||\` with short-circuit semantics, unary \`!\`/\`~\`, \`?:\`, comma, assignment expressions, prefix/postfix increment, compound assignments, sequencing and side-effect correctness. Avoid eager selects or reordering for effects. | GCC/Clang differential edge tests; no unmodeled side effects; deterministic generated scalar-expression corpus. | L |
| **M4 — Scalar memory, objects and linkage** | Addressable locals as stack slots when needed; global and extern scalar declarations; ELF data/bss, external data relocations; loads/stores with alignment/width/extension; correct linkage/visibility, type/layout validation, linker diagnostics. Distinguish SSA scalars from address-taken storage. | Cranelift-generated \`void test()\` modifies external globals defined by GCC-built \`driver.c\`, with matching observable state and checksum. | L |
| **M5 — Arrays, pointers and indexing** | Pointer types/ABI and decay; address-of/dereference, indexing and pointer arithmetic; multidimensional C arrays and element strides; lvalues, initialization, casts, bounded addressing, aliasing, aligned and unaligned accesses. Don't falsely optimize through potentially aliasing pointers. | Handwritten 1-D/2-D array kernels, parameter arrays and globals, plus a real generated seed with GCC matching output. | XL |
| **M6 — Calls, full CFG and source semantics** | Direct calls and C ABI interop; multiple functions/translation-unit references, void calls, argument conversions; fix loop/branch corner cases, \`switch\` if emitted, labels and generated goto shapes as required. Enforce C evaluation order, side effects and definite initialization without overly conservative false rejection. | Representative generated \`func.c\` cases compile and link; >=100 pinned seeds triaged; no invalid Cranelift verifier or relocation errors. | L–XL |
| **M7 — Coverage-driven YARPGen completion** | Iterate on ranked failure buckets (not features in arbitrary order); enable wider generator profiles, optional pragmas/alignments if semantically relevant. Build reducer and regressions for each correctness bug. Run both Cranelift \`none\` and \`speed\`. | Meet the functional and breadth gates above over pinned 1,000-seed cohorts, including independent GCC/Clang baseline checks. | XL |
| **M8 — Stabilize and reuse** | Freeze fixed seeds plus minimized regressions; verify reproducibility on clean Linux CI; document unsupported cases and cross-module boundaries; prepare shared front-end lowering for \`bingundu/compiler\` without coupling its IR to Cranelift. | 10k-seed monitored stress with useful triage; explicit ready/not-ready report and regression corpus; benchmark resource cost. | M–L |

**Ordering and focus:** M0/M1 first, then M2/M3 to establish correct semantics; M4/M5 unlock actual YARPGen's global-array-heavy programs. M6 resolves interop and CFG gaps found on generated cases. M7 is iterative and may pull M3/M6 subfeatures forward based on empirical unsupported-operation counts. Partial C++ support and full C++ name mangling are explicitly **out of scope** for the baseline.

## Harness contract: three compile paths, one oracle

For each generator seed:

1. Generate a fresh isolated directory with the **pinned executable**,
   seed, standard, and recorded options. Verify all three emitted files.
2. Compile both source files with GCC using fixed language/target flags
   at \`-O0\` and \`-O2\` (two independently produced executables).
   Optionally build Clang \`-O2\` to cross-check the oracle.
3. Compile \`func.c\` through the EDG frontend and Cranelift into a
   **relocatable .o**. Compile \`driver.c\` with GCC at controlled options.
   Link through GCC. Preserve object relocation diagnostics.
4. Run all binaries under CPU/memory/wall-time limits in separate processes;
   compare exit codes, decimal checksum stdout, and any \`ERROR:\` markers.
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

\`--inp-as-args=none\` narrows the signature surface but **increases global
memory dependence**. \`--max-array-dims=2\` limits rank, not total bytes:
enforce memory, per-process time and object-size limits independently.
The generator's \`--simple-loops=all\` (with
\`--vectorizer-target=gcc-clang\`) can define another controlled subprofile,
but should not substitute for testing the ordinary loop generator.

### Case classification and reporting

Keep separate, machine-readable counters for:

| Case outcome | What it means |
|---|---|
| \`baseline_disagree\` | GCC/Clang or controlled GCC builds disagree, so no trusted oracle |
| \`generator_failure\` | Generator crashed, timed out or omitted required files |
| \`frontend_error\` | EDG rejects source or exposes malformed/incomplete IL |
| \`unsupported_construct\` | Explicit backend omission with actionable diagnostic |
| \`codegen_failure\` | Cranelift verifier, ISA lowering or codegen failure |
| \`link_failure\` | Symbol, ABI, external data or relocation failure |
| \`runtime_failure\` | Trap, signal, timeout, crash, missing executable or resource failure |
| \`wrong_output\` | Fully accepted case with differing result from established oracle |
| \`pass\` | Successful compile, link, run and exact reference agreement |

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
* The \`bingordo\` branch runs CI. Mirror verified source/docs into
  \`bingundu\`; its workflow remains manual-only to avoid double expenditure.

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

**Preserve evaluation order.** The existing eager \`Expr::Select\` path is
not safe for effectful expressions. Use true blocks, explicit lvalues,
typed loads/stores and sequencing when representing \`?:\`, \`&&\`, \`||\`,
assignments and calls.

**Make external data real.** YARPGen's \`test\` often reads/writes globals
and arrays allocated in the GCC driver. The reference backend needs correct
external data symbol declarations, relocations, addressing and ABI, not just
standalone local SSA values.

**Keep the reference understandable.** Introduce a small typed, backend-neutral
representation for expressions, lvalues, statement CFG and typed symbols
rather than layering every new feature into the original untyped \`Expr\`
and \`cfg.rs\`. Reuse that decoding and semantic checking for the future
\`bingundu/compiler\` connection. The reference remains a *correctness
oracle*, not a competing general optimizing compiler.

**Testing priority:** wrong-code, invalid ABI and nondeterministic
miscompiles outrank unsupported-but-diagnosed features. Do focused unit
regressions per change, bounded end-to-end corpus per milestone, and infrequent
full seeded runs.

## Immediate next implementation task

**Start M0, then M1**: publish and pin a Linux x86-64 YARPGen release using
the existing workflow; create a deterministic harness that proves the
GCC-only baseline for at least 100 seeds and records which EDG/Cranelift
features prevent compilation. **Do not begin by attempting all generated
programs in the Cranelift path**: the present signed-i64 ABI is known to
reject \`void test\`, and external globals/arrays are not implemented.

Useful references:

* \`bingundu/yarpgen/.github/workflows/binary-builds.yml\` on \`ci/binary-builds\`
  — versioned \`v*\` release job and five archive artifacts.
* \`bingundu/yarpgen/src/program.cpp\` — emits \`init.h\`, \`func.*\`,
  \`driver.*\`, \`void test(...)\`, external variables/arrays and checksum.
* \`bingundu/yarpgen/src/options.cpp\` — documented generator flags.
* \`bingordu/edgcpp-rust/examples/codegen.rs\` and
  \`.github/workflows/build.yml\` — current ELF frontend CLI and pinned EDG
  build/test mechanism.
