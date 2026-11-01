# Target architecture and ABI boundary (EDG / Cranelift)

This document describes the **current implementation**, and separates it from
**future AArch64 work**. The current reference backend generates x86-64 ELF
objects using the SysV AMD64 C ABI; it does **not** generate AArch64 objects.

## Design rule

EDG language semantics, validated native type/layout metadata, expression
evaluation order, lvalues, dataflow and statement CFG belong in generic
lowering. The machine calling convention, ABI classifications, register
assignments, byte packing of aggregate arguments/results, and construction of
the Cranelift ISA belong in the selected *target* module.

```text
EDG native IL + validated C types/layouts
        |
        v
Generic C expressions / lvalues / statement CFG
        |
        +--> TargetAbi: aggregate classification + supported ABI gate
        |       +--> target/x86_64.rs (today: SysV AMD64 subset)
        |       +--> target/aarch64.rs (planned: AAPCS64)
        |
        v
Cranelift IR call/return signatures and body
        |
        v
Target ISA -> relocatable ELF object -> GCC/Clang ABI differential tests
```

## Current boundary

- `src/codegen/target.rs`: target ABI interface and the one explicitly
  selected implementation. **Only x86-64 is selected today.**
- `src/codegen/target/x86_64.rs`: SysV aggregate classification, GPR budget,
  Cranelift machine parameter expansion, C struct argument byte packing,
  one-GPR result packing, decoding incoming physical arguments, exact-sized
  materialization of aggregate parameters, and the Cranelift ISA name.
- `src/codegen.rs`: backend-neutral C ABI scalar descriptors and expression
  syntax, plus the current x86-only entry point. It forwards ABI decisions to
  `TargetAbi`; it must not acquire new SysV register rules.
- `src/codegen/cfg.rs`: generic EDG CFG and lvalue sequencing, calls, imports,
  Cranelift emission, native stack slots, and object production; physical
  argument/result layout calls the selected ABI hooks.

The existing one- and two-INTEGER-eightbyte SysV tests remain authoritative:
native C caller/generated callee, generated caller/GCC callee, generated-to-
generated calls, scalar/aggregate mixtures, and the register exhaustion guard.

## Adding AArch64 later

1. Expand the current `Option<u8>` integer-aggregate descriptor into a
   generic *passing plan*: distinguish INTEGER, FP/SIMD, homogeneous floating
   aggregates, stack-by-value, indirect/result pointer and per-argument
   machine locations (including a whole-argument rollback if registers are
   exhausted). Keep the source parameter index independent of machine slots.
2. Implement `target/aarch64.rs` against that plan and add explicit target
   selection. Do **not** copy the x86 `>8`-bytes check or six-GPR budget;
   AAPCS64 has its own x0–x7/v0–v7 rules, HFA handling, indirect results and
   stack placement. The exact ABI variant must be selected (AAPCS64 vs Apple
   arm64/Windows arm64 where applicable).
3. Centralize the remaining layout assumptions before enabling the new
   target: pointer size/alignment, endianness, data relocations, constant
   object images, native bitfield layout, ELF reloc kinds, and hard-coded
   64-bit pointer operations. Native EDG layouts are authoritative **only**
   for the target and ABI configuration that generated them.
4. Build/pin an AArch64-compatible EDG toolchain or validate a trustworthy
   target-specific metadata path. Add native ARM64 CI runners (or QEMU
   smoke tests, with native validation as the ABI acceptance gate).
5. Gate each additional calling-convention class with GCC/Clang
   cross-object tests in **both directions**, including return values,
   nested structures, register exhaustion, and negative unsupported cases.
   Never claim an ISA is supported solely because Cranelift can emit it.

## Invariants

- Unsupported ABI types or mismatched target layout are rejected **before**
  emitting an object. An unclassified aggregate must never silently become a
  scalar or pointer argument.
- Source expressions producing addresses are evaluated once; each aggregate
  value is packed from its actual object, without reading past native size.
- Declared and defined function types must map to identical ABI signatures.
- Signature construction and function-entry argument reconstruction must use
  the **same** target plan as direct-call lowering.
- Endianness and register budget are properties of a concrete target ABI;
  generic CFG code must not infer either from the source record size.
- Existing x86-64 behavior and GCC differential tests are a regression gate
  for any further refactor.

**Limit today:** This patch establishes the target seam for the *currently
supported* integer-only ABI. It does not yet provide the richer passing-plan
IR, an AArch64 build, target-dependent EDG configuration or endian-independent
global data emission. Those are explicit follow-up steps, not implemented
capabilities.
