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

**CI:** [Build workflow](https://github.com/bingundu/edgcpp-rust/actions/workflows/build.yml).
