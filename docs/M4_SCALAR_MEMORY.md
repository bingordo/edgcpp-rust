# M4 — scalar global memory, ELF objects and linkage

Status: **Completed for ordinary, nonvolatile scalar C globals** on Linux
x86-64. Verified in the bingordo CI repository on 2026-10-29.
Pointer-dependent address-taken local storage is an explicit **M5 prerequisite**,
not silently claimed as M4 support.

## Implementation

- The EDG bridge exports global declaration storage/linkage as native-derived
  event 47, and ensures referenced variables have copied symbol/type edges.
- Rust resolves globals by EDG `SymbolId` and validates exact scalar width
  (8/16/32/64), signedness and `_Bool`. It refuses ambiguous symbol names,
  TLS, qualified, address-taken, aggregate and unsupported storage forms.
- The typed expression IR distinguishes SSA locals/parameters from real global
  lvalues. Globals support loads, assignment, compound arithmetic/bitwise
  operations, prefix and postfix increment/decrement, including M3 sequencing
  and C integer conversions.
- `src/codegen/memory.rs` handles exact-width loads/stores with sign/zero
  extension, narrowing and canonical boolean values. Memory operations are
  not replaced with SSA locals.
- `ObjectModule` declares imported variables with data relocations and owns
  local/exported scalar definitions. Constant initializers become data;
  zero-initialized definitions become BSS. Functions and data are emitted in
  the same relocatable ELF object and linked with GCC.

## Acceptance evidence

- [CI #40794318473](https://github.com/bingordo/edgcpp-rust/actions/runs/40794318473):
  EDG-compiled `tests/fixtures/m4_external_globals.c` linked with a GCC
  driver that **owns** four extern globals (u32, i16, bool, u64). Both
  executions matched, covering reads/writes, narrowing and two mutations.
  Standalone imported-data relocation probe and M3 regressions also passed.
- [CI #40794819868](https://github.com/bingordo/edgcpp-rust/actions/runs/40794819868):
  additional GCC differential program for object-owned initialized data,
  zero-initialized exported BSS, internal-linkage storage and repeated writes.
  All other targeted bridge/codegen/native and negative tests passed.

For local execution, run the configured EDG codegen example with C11 input:

```sh
cargo test --features codegen --lib codegen::memory::tests::
bash tools/m4_data_probe.sh
# In the pinned EDG build directory, after sourcing environment.sh:
path/to/codegen m4_external.o --c11 m4_external_globals.c
gcc -std=c11 -O2 m4_external.o m4_external_driver.c -o m4_check
```

The differential GCC-only and Cranelift paths are both retained in
`.github/workflows/build.yml` on `bingordo`; no separate heavy job or
EDG rebuild is needed.

## Scope boundary and next milestone

M4 does **not** implement general pointers, arrays, aliases, field lvalues,
volatile/atomic semantics, TLS, dynamic global initialization, arbitrary C++
linkage, or C object models beyond ordinary scalar data. Unsupported forms
are rejected instead of manufacturing an SSA value or an unsafe relocation.
Address-taken locals depend on pointer/address operations and stack slots:
both are now explicit prerequisites of **M5**. In particular this milestone
does not mean a default YARPGen `func.c` compiles: generated arrays and
pointer parameters still need M5; calls and remaining control patterns need
M6. Do not count M1 GCC-only YARPGen baseline seeds as EDG passes.
