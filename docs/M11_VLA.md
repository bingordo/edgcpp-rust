# M11 — Variable-length arrays and dynamic automatic storage

**Status:** Working, GCC-differential-tested **EDG-lowered C11 VLA subset** in
the Cranelift reference backend (x86-64 SysV). The optional C VLA runtime ships
as `runtime/vla_runtime.c`. This is **not** a native `bingordo/compiler`
backend implementation, and not full C/C++ VLA support.

## Two authoritative representations

The EDG frontend may expose an *unlowered* VLA as
`stmk_set_vla_size` (evaluate a dimension exactly at this execution site)
and `stmk_vla_decl` (allocate a true VLA object; a variably modified pointer
declaration does not itself allocate storage). The native bridge and shared
Rust `Il`/`Graph` preserve the native array `TypeId`, declaration identity,
the optional bound expression, the copied-dimension flag, and the original
execution site. The graph rejects missing or duplicate dimension links.
This representation is tested by Rust wire/graph tests; **the pinned native
EDG binary does not currently emit these nodes.** No backend should assume
that presence/absence of these events proves the source did/did not use VLAs.

The pinned callable EDG build, source SHA
`799603e0b5ab272d9418e2d44a53a95bc73dfdc4`, instead lowers C11 VLA
syntax into ordinary typed IL before the Rust callback. EDG's
`src/lower_c99.c` specifies its portable lowering. For `int a[n][m]`, it
evaluates the dimensions at their source positions into generated scalar
temporaries, lowers `a` into a pointer, computes byte lengths and indexed
strides, calls `__vla_alloc((void *)&a, bytes)` to store the allocated
pointer, and emits `__vla_dealloc((void *)&a)` on the relevant lexical exit
edges. Nested scopes and loop re-entry are represented by separate call
execution points. Do not synthesize additional lifetime edges from
declaration spelling or substitute Cranelift's scalable-vector dynamic slots.

## Cranelift reference backend contract

- `cpp/bridge.cpp` retains typed routine/prototype records even when an
  EDG-inserted helper is not in the source namespace's routine list.
- Only a function with both lowered VLA allocation and deallocation helpers
  may use compiler-generated **integer/pointer** temporaries on the scalar
  reference lowering path. Static, TLS, reference, qualified, aggregate or
  unrelated generated storage remains rejected. This does not generally enable
  C++ generated temporaries.
- `Expr::VlaAlloc` wraps only a checked direct
  `__vla_alloc(&automatic_pointer_slot, size)` call. The function prototype,
  number of arguments and slot address are validated. The wrapper lets
  definite-initialization analysis know that this exact local pointer is
  initialized through the helper, while Cranelift still emits the original
  checked call **once**.
- The existing target-owned ABI passing plan handles the helper calls. EDG
  owns the runtime dimension evaluation and cleanup call placement, not the
  shared Rust graph or a second frontend CFG.
- Generated ELF objects import `__vla_alloc` and `__vla_dealloc`. Link
  `runtime/vla_runtime.c` or a compatible EDG-supplied runtime into the
  application. The included optional runtime uses one `malloc`/`free`
  pair per allocation rather than EDG's example pooled allocation strategy.
  It has no global pool and requires only fundamental C alignment.

## Executable regression gate

`tests/fixtures/m11_vla_semantics.c` and `m11_vla_driver.c` compile
the C11 source with GCC and separately through EDG→Cranelift. The generated
object is linked with the same GCC-built test driver plus the reference
runtime, then executes the same assertions. The CI step rebuilds
`target/debug/examples/codegen` before the test so a cached executable
cannot silently test a previous commit.

The gate exercises:

- Positive 1D bounds, sequential indexing and a side-effectful `++count`.
- Nested VLA scopes and multiple live allocations.
- Two-dimensional VLA row strides with runtime bounds.
- Runtime `sizeof VLA` with the original dimension evaluated once.
- Cleanup across early returns and nested `break`/`continue` paths.
- Loop re-entry and hundreds of repeated allocation/deallocation iterations.

First 1D/nested end-to-end pass:
[41579999195](https://github.com/bingordo/edgcpp-rust/actions/runs/41579999195).
Expanded multidimensional, `sizeof`, return, loop and stress pass:
[41580672223](https://github.com/bingordo/edgcpp-rust/actions/runs/41580672223).

The pinned EDG parse-only fixture also checks that lowered declarations
and runtime helper references are visible. Rust tests independently check
unlowered `VlaDimension` metadata validation.

## Deliberate limits and follow-up work

- **Native `compiler` adapter:** not implemented yet. It must choose its
  own allocation/cleanup lowering; do not copy Cranelift's ABI machinery.
- **Unlowered VLA code generation:** not enabled; a backend consuming native
  `stmk_set_vla_size`/ `stmk_vla_decl` must implement bound storage,
  dependency order and per-edge cleanup before accepting those nodes.
- **C++ VLAs and non-local control transfers:** not claimed. `longjmp`
  bypasses generated cleanup in the simple malloc/free runtime; EDG's sample
  pooled runtime has special dead-frame recovery that is not reproduced.
  Exceptions/coroutines, cleanup on exceptional edges, and jumps into VLA
  lifetimes need their own tests and semantics.
- **Extended alignment / extreme sizes:** `malloc` supports fundamental
  alignment, not arbitrary extended alignments. The runtime rejects negative
  sizes or allocation failure. C code with invalid bounds/overflow is not a
  supported defined-behavior test case.
- **Complex signatures and storage:** floats, unsupported aggregates,
  qualified/volatile accesses and other M10/M12 ABI features still fail closed
  under their existing validation gates.
- **Future verification:** add multidimensional overflow/stride stress, deep
  scope exits, sanitizers and negative provenance/cleanup cases. Prove that
  unsupported EDG IL never emits a misleading runnable object.

The implementation deliberately relies on **EDG's explicit lowered control
flow and typed runtime calls**; it does not infer VLA storage from source
names or treat a fixed stack slot as a runtime-sized allocation.
