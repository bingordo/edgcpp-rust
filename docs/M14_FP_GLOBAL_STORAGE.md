# M14 — source-checked native floating-point global storage

**Status:** two separate merged/CI-validated slices on November 6, 2026:
[PR #72](https://github.com/bingordo/edgcpp-rust/pull/72)
(commit `3cb2702d5757276bda9c9198d776d7cc7a9481bc`,
[full CI #42313261217](https://github.com/bingordo/edgcpp-rust/actions/runs/42313261217))
and [PR #73](https://github.com/bingordo/edgcpp-rust/pull/73)
(commit `c05e8591d7468fa80c7604bb19117dbd7978cbb4`,
[full CI #42314149574](https://github.com/bingordo/edgcpp-rust/actions/runs/42314149574)).

This is the first checked native F32/F64 **data-section** functionality
of the independent Cranelift reference emitter. It is not general
memory SSA, pointer aliasing, volatile/atomic semantics or complete
support for initialized FP data objects.

## Supported and checked native source

Only native EDG file-scope variables of exact standard C `float` or
`double` type, with valid 4/8-byte size and alignment, are accepted:

- Ordinary external **declarations** can be read. They become real
  undefined ELF `Linkage::Import` **data** symbols, resolved when the
  object is linked with GCC-owned data definitions.
- File-scope zero-initialized/tentative **definitions** use source-owned
  `Linkage::Export`; translation-unit static globals use
  `Linkage::Local`. Both allocate genuine native aligned ELF zero
  storage (`.bss`).
- Native EDG ordinary direct global rvalues become
  `FloatExpr::ExternalGlobal`, whose emission performs a checked,
  typed Cranelift memory load (`F32` or `F64`). The node name
  remains for compatibility: it also handles verified source-owned
  globals. A read **never** becomes an SSA constant or an integer
  reinterpretation.
- Direct `global = FP_expression;` statements are allowed and must
  be exact source-owned C assignments, with checked F32/F64 source
  and destination types. `FloatStmt::StoreGlobal` runs within the
  existing path-sensitive CFG and emits a real ordinary typed memory
  store, including after branches and loop edges. The RHS is evaluated
  once at the source statement position.
- The native symbol, storage class, nonvolatile qualifiers and
  source type must all match. There is no fabricated data import from
  the public FP-only reference entry point.

## Verification

- `tests/fixtures/m14_fp_external_globals.c` plus the GCC-only
  `m14_fp_external_globals_driver.c` test cross-object ELF
  undefined-data references, two consecutive loads, branch and loop
  conditions, GCC changes observed by generated code, IEEE NaN and
  signed zero.
- `tests/fixtures/m14_fp_zero_owned_globals.c` plus its GCC driver
  verify initial native .bss zeros, F32/F64 definitions,
  translation-unit static/local linkage, direct stores,
  read/modify/write, branch-dependent stores, IEEE signed-zero/NaN
  preservation, and reading changes made by GCC to exported data.
- The two CI stages inspect actual ELF symbols with `nm`,
  compile and link separate GCC/EDG objects, execute both versions,
  and require no partially emitted ELF on negative cases.
- Negative fixtures reject explicit floating-point initializers,
  TLS and function-local static objects, volatile/global qualified
  effects, integer data interpreted as FP, and address-taken pointer
  aliases.

## Remaining memory functionality

- Byte-exact source-owned **explicitly initialized** FP global data
  (`double x=1.5;`) requires a separate EDG-native constant image
  bridge, checked initializers and `.data` emission.
- Address-taken and aliased **FP automatic locals**, FP pointer
  indirections and array/field lvalues, general pointer provenance
  and memory SSA.
- Volatile/TLS/atomic FP memory effects and C fenv interaction with
  storage and native rounding modes.
- Broader C FP conditions/side effects, INTEGER/FP casts,
  SysV aggregates, and a separately executable native
  `bingordo/compiler` adapter.

This is part of the still-open [M14 issue #60](https://github.com/bingordo/edgcpp-rust/issues/60).
C11 atomic expansion is separately deferred in
[issue #59](https://github.com/bingordo/edgcpp-rust/issues/59).
