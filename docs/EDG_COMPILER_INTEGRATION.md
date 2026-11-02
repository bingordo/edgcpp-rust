# EDG frontend integration into bingordo/compiler

Status: **integration plan, not a claim of an existing EDG compiler adapter**.
The project goal is a reusable EDG frontend for the native
`bingordo/compiler` optimizer and machine backends. Cranelift is an optional
**reference backend** for semantic and ABI differential testing, not an
intermediate representation or required runtime dependency of `compiler`.

## Current state and intended boundaries

```text
         EDG C/C++ frontend (matching native configuration)
                              |
                       owned Il snapshot
                              |
                   graph::Graph indexed view
                              |
              checked C semantics / source interface
                     /                    \
                    v                      v
          Cranelift adapter          compiler adapter
          (reference oracle)         (production goal)
                    |                      |
             Cranelift CFG         compiler-ir CFG/SSA
             x86 SysV ABI          compiler target ABI
                    |                      |
             x86-64 ELF              native backend
                    \                      /
               differential execution against GCC/Clang
```

What is implemented already:

- `src/lib.rs` exports an owned, compilation-lifetime-independent `Il`.
- `src/graph.rs` supplies an indexed graph with symbol/type/statement/
  expression identity, scope, source location, lifetime and label metadata.
- `src/codegen.rs` and `src/codegen/cfg.rs` implement an *optional* Cranelift
  backend with a selected target ABI. Its internal `Expr`, `Stmt` and
  Cranelift `FunctionBuilder` structures are **not** the frontend interface.
- `bingordo/compiler` already owns its block-argument SSA CFG, verifier,
  optimizer, machine backend and a separate Mini-C frontend driver.
  The new EDG integration should follow the driver pattern rather than
  linking the compiler core to Cranelift.

## Stable frontend contract to build next

Introduce a **checked semantic projection** over the owned EDG `Il`/
`graph::Graph`, reusing those IDs where possible and avoiding a second
unconditional copy of the whole AST. The goal is a backend-neutral input to
both lowering adapters, with:

1. **Typed declarations and references:** symbol identity, function
   prototype, linkage, storage class, qualifiers, source locations,
   lexical scope and lifetime events. Preserve source `goto`/label/
   `switch`/case/default identities; do **not** construct a shared CFG.
2. **Explicit expression semantics:** source and promoted/result types,
   value category (lvalue/rvalue), signedness, width, casts, integer
   promotions, usual arithmetic conversions, short-circuiting,
   sequencing and side-effect/volatile/atomic annotations. Keep pointer
   provenance/alias guarantees conservative.
3. **Object and initialization semantics:** native size, alignment, field
   offsets, bitfield ranges, array bounds, global relocations and
   dynamic-initializer execution order. These layouts are only valid for
   the matching EDG target ABI configuration; do not treat x86-native
   layout metadata as universal.
4. **Actionable unsupported diagnostics:** classify missing metadata,
   unsupported language semantics and unsupported *target backend*
   capabilities separately. Never translate an unknown C operation into
   a plausible but unverified SSA or machine operation.
5. **A versioned Rust interface and tests:** use strongly typed IDs and
   checked accessors rather than leaking opaque native EDG enum numbers;
   avoid raw EDG pointers and Cranelift types in the consumer-facing API.
   Public API changes should remain compatible where possible, or use
   deliberate version/feature gates.

This is a semantic *source interface*, not a new optimization IR, generic
three-address code, machine ABI passing plan, frontend SSA, or front-end
CFG. Backends may directly build their own CFG/SSA from it.

## Sequencing decision: stabilize Cranelift reference first

**Current execution order:** make EDG -> Cranelift a usable reference C
compiler before beginning the `bingordo/compiler` adapter. This revises the
earlier proposal to run M9 integration concurrently. The long-term goal
remains `bingordo/compiler`; this is a dependency gate, **not** a decision
to adopt Cranelift IR in the main compiler.

The immediate reference-readiness gate is:

1. Native x86-64 ELF code generation, callable from C via the example
   driver, with no silent wrong-code for the supported subset.
2. A pinned **100-seed** controlled YARPGen cohort, with independent GCC and
   Clang oracles, a published pass/unsupported/failure breakdown and
   replayable failing cases. Do not infer broad coverage from the existing
   ten-seed smoke.
3. Important C language gaps prioritized from the actual failure ranking:
   unconditional `for (;;)` and other control-flow cases, C ABI classes,
   indirect calls, initializer semantics, etc. Do not prioritize features
   without a representative failing case or concrete interoperability need.
4. Preserve two-way GCC caller/callee tests and enforce explicit rejection
   when native EDG semantics, ABI layouts or target capabilities are unknown.
5. Maintain the owned `Il` / `graph::Graph` interface without making
   production compiler code or reference-only Cranelift machinery its API.

Only after the reference cohort and interoperability gates are credible do
we begin the first `compiler-ir` integration pilot described next.
The 1,000-seed constrained/default breadth gates in
[YARPGen roadmap](YARPGEN_CRANELIFT_MILESTONES.md) remain longer-term
acceptance goals; they are not claims of current success.

## First compiler integration slice (after the reference-readiness gate)

Create a separate EDG driver/adapter in `bingordo/compiler`, analogous to
`tools/mini-c-driver`. Pin the EDG bridge revision and build configuration.
Keep the normal compiler, generic IR and optimizer free of EDG, EDG binary
and Cranelift dependencies. The adapter may use the checked projection from
this repository as an ordinary Rust dependency.

1. Start with simple, **defined-behavior integer C**: one `int f(int)`
   style function, constants/parameters, conversions, locals, return,
   comparisons, then `if` and `while`.
2. Translate directly into `compiler-ir` using its existing typed
   block-argument CFG/SSA construction and verifier. The adapter—not
   `compiler-ir`—knows EDG kinds and type metadata. Do not port the
   Cranelift `Stmt` or `Expr` lowering as the public interface.
3. Add labels, arbitrary `goto` and `switch` once their source
   semantics are checked. The reference adapter can use a two-phase
   label-to-Cranelift-block map; the native adapter uses its own
   block map. Neither requires a shared frontend CFG.
4. Extend to memory, globals, pointers, aggregate objects, calls and
   more language coverage **incrementally**, relying on already tested
   `compiler-ir` operations and target ABI hooks. Do not block the
   pilot on full SysV aggregate, floating, variadic, VLA or atomic support.

**Initial acceptance gate:** compile and link a tiny C function with EDG
+ `compiler` native ELF; compare result to GCC and EDG + Cranelift on a
fixed fixture; run the compiler IR verifier. Expand to a branch/loop
fixture. Include one negative fixture that fails cleanly on both
adapters rather than silently compiling unsupported semantics.

## Subsequent integration and ABI ownership

- M10: typed direct/indirect calls, function pointers and verified ABI.
  The semantic projection provides C function types and target-native
  layout metadata. Each concrete backend owns its machine calling
  convention and placement, independently of Cranelift's `PassingPlan`.
  Use GCC/Clang cross-object tests in both call directions.
- M11: VLAs, runtime `sizeof` and lifetime events belong in the source
  contract. Dynamic allocation/stack restoration and CFG edges belong in
  each backend.
- M12: volatile and C11 atomic semantics must be expressed as explicit
  memory effects and orderings. The native compiler implements them in
  `compiler-ir`/legalization/target code; Cranelift reference coverage
  may remain partial and reject unsupported cases.
- M13: broaden the checked projection and native adapter, audit semantics
  and run reproducible compiler-vs-Cranelift-vs-GCC/Clang tests.
  A Cranelift unsupported result is a coverage limit, not a proof of
  compiler failure; a shared bug between two adapters is not proven
  correct without an independent oracle.

**Longer-term success criterion:** the same EDG source and checked semantic
view can drive `compiler` without importing Cranelift; shared C semantic
tests pass, target ABI and layout are independently validated, errors retain
source locations, and frontend integration has no forked semantic rules.

## CI and non-goals

- Test the owned `Il` and checked semantic interface without having to
  compile Cranelift. Keep bridge/native EDG dependency setup isolated.
- Run narrow native compiler and Cranelift reference differential fixtures
  separately; gate broad YARPGen seed cohorts on proven baseline coverage.
- Pin the source/configuration/binary revision of EDG in integration CI;
  respect the repository's existing access and binary artifact restrictions.
- No duplicate general-purpose frontend CFG, optimizer, vectorizer,
  register allocator or machine instruction selector.
- No promise that an x86-64 native EDG snapshot is suitable for AArch64.
  AArch64 requires matching frontend target configuration and layout
  validation in addition to backend code generation.
