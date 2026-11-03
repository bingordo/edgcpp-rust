# M11 — Variable-length arrays and dynamic automatic storage

Status: **semantic bridge / graph slice implemented; native allocation not yet implemented**.

## Source facts preserved

A C VLA is not a fixed array with an unknown compile-time constant. EDG
represents its runtime bound at `stmk_set_vla_size`, separately from its
`stmk_vla_decl` storage/declaration site. The bound may have side effects
(`int a[++n];`), and re-entry to that statement must evaluate it again.
The array type is identified by native `TypeId`, not by a variable name or
expression spelling. EDG also distinguishes a true VLA object from a
variably modified pointer declaration; neither implies the other.

The bridge emits a checked `VlaDimension` record (native array type,
evaluation site, expression/copy flags), retains an ordinary typed expression
tree at the size-setting statement when available, and preserves the native
VLA declaration flag. `Graph::from_il` checks for duplicate, dangling, or
missing expression sites. A copied dimension without its own expression
must not evaluate an invented bound. The current projection intentionally
has **no storage allocation semantics**.

## Confirmed pinned-EDG representation (2026-11-02)

The pinned EDG source SHA `799603e0b5ab272d9418e2d44a53a95bc73dfdc4`
and the current prebuilt callable frontend **already lower the VLA syntax**
before the Rust snapshot. The C11 probe in
[run 41577821937](https://github.com/bingordo/edgcpp-rust/actions/runs/41577821937)
showed ordinary `stmk_decl` records with `is_vla=false`, followed by
compiler-generated expression statements including calls. No native
`stmk_set_vla_size` was visible. This is a **real representation difference**,
not evidence that the original C source had fixed-length arrays.

Consequences:
- The new native VLA events (56) and its checked graph model are a future
  *unlowered IL* interface; only their Rust unit tests exercise them with the
  pinned binary. The fixture CI presently tests the lowered shape, and reports
  that the native dimension gate was not exercised.
- Do not infer original VLA lifetimes, per-dimension expression counts or
  deallocation responsibilities from the lowered declaration alone.
- Either capture before EDG VLA lowering (possibly requiring a new pinned
  callable EDG build/configuration) **or** explicitly translate the EDG
  generated allocation, deallocation and runtime stride operations with
  verified semantics. A pattern match on arbitrary `eok_call` spellings is
  not enough, because provenance and cleanup edges matter.
- The workflow deliberately requires that the Cranelift backend reject the
  still-unsupported runtime VLA fixture without creating an object. Parsing
  and rejection are NOT executable VLA support.

## Backend design boundary

Cranelift's `create_dynamic_stack_slot` describes dynamically sized
*vector types* (target vscale), not arbitrary per-execution C allocations.
Do not silently substitute it for a VLA stack allocation.

Before the reference backend can accept one VLA, implement these together:

1. A typed runtime-size expression and result binding evaluated exactly once
   at each native set-size point, with checked integer overflow and native
   alignment/element-stride data. Handle a dependent multidimensional type
   only when all dimension dependencies have been evaluated.
2. An explicit allocation *and deallocation* representation tied to lexical
   scope and control-flow edges. Native EDG can emit VLA deallocation nodes;
   preserve their identity. Branches, returns, loop re-entry, continue,
   break, and jumps must not bypass required cleanup, re-use an expired
   allocation, or jump into a VLA's lifetime. Current `lower()` flattens
   lexical blocks, so allocation must not be added without restoring scope
   information or honoring explicit deallocation.
3. A target-owned storage strategy. A Cranelift reference implementation may
   use a checked runtime allocator with explicit scope cleanup if dynamic
   stack allocation is unavailable; the future native `compiler` backend
   must choose its own stack/ABI strategy. Do not encode SysV stack-pointer
   manipulation in the shared EDG graph.
4. Correct runtime `sizeof`, array-to-pointer decay, indexing and strides,
   and propagation of the evaluated dimension into all dependent expressions.
   Re-reading `++n` during `sizeof` or allocating a fixed-size surrogate
   silently changes C semantics.

## Regression gates

The parse-only fixture `tests/fixtures/m11_vla_semantics.c` exercises a
simple VLA, a side-effectful bound, and nested lexical scopes. Rust unit
tests validate graph and wire facts. These are **not executable VLA passes**.

Before enabling native VLA execution, require a generated-object/GCC-driver
differential covering: positive 1D bounds, nested VLAs, side-effect counts,
pointer indexing, runtime `sizeof`, early returns, loop re-entry, nested
break/continue, and stack/allocation stress. Add negative tests for copied
or missing dimensions, incompatible element layouts, overflows, unsupported
jump/cleanup shapes, and unexpected EDG node kinds. Any unsupported C VLA
must fail closed without emitting a potentially incorrect object.
