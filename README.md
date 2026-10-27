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

Local assignments, loops, calls, global memory and general C++ features are
still not implemented. The signed-i64 restriction is intentional until
properly typed signatures and expressions are implemented.

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
