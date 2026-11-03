# M12 — qualified scalar memory access (first supported subset)

This change introduces an explicit `MemoryAccess` distinction among
ordinary, volatile, and C11 atomic scalar globals. The authoritative
qualifier bits are native EDG `TQ_VOLATILE` and `TQ_C11_ATOMIC`;
typedef chains are inspected rather than discarding those bits during
canonicalization.

The Cranelift reference backend (0.121, x86-64 SysV) has no general
`volatile` flag. To keep every qualified scalar access observable at the
machine-code level, **naturally aligned 1/2/4/8-byte RAM globals** use
Cranelift's effectful, sequentially consistent `atomic_load` and
`atomic_store` operations. For C volatile this is deliberately stronger
ordering than required, not a claim that volatile objects are C atomics.
It is *not* a sound substitute for device/MMIO volatile accesses,
misaligned/packed members, or arbitrary memory-mapped hardware registers.
These remain unsupported. For ordinary memory all previous loads and
stores retain the original non-atomic lowering.

For C11 `_Atomic` integer globals, direct scalar reads and assignments
use sequentially consistent operations. Postincrement/decrement and native
integer compound add/subtract/AND/OR/XOR use indivisible Cranelift
`atomic_rmw` operations, preserving old versus new expression results.
The GCC differential now includes a four-thread concurrent increment
stress test. Atomic multiplication/division, compare-exchange, explicit
weak/non-SC orders, fences, atomic pointers and complex RMW still fail
closed: they require CAS-loop or explicit ordering semantics. Similarly, qualified pointer dereferences,
qualified local storage, qualified aggregate fields, mixed/unknown
qualifiers and non-scalar or insufficiently aligned globals fail closed.

`tests/fixtures/m12_qualified_globals.c` and the GCC-built driver check
C11 linkage, signed 8/16/32/64-bit volatile accesses, repeated reads,
ordered writes and sequentially consistent atomic scalar reads/writes.
`m12_atomic_rejected.c` guards against accidentally accepting a
non-atomic emulation of atomic increment.

Full M12 completion requires native target and `bingordo/compiler` support
for precise ordering, true volatile load/store primitives, and concurrent
atomic RMW litmus/stress tests.
