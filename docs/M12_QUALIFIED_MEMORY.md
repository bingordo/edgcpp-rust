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
closed: they require CAS-loop or explicit ordering semantics.

**M12 qualified indirect follow-up (PR #55):** Reads and simple stores
through explicitly typed `volatile` and `_Atomic` scalar pointers, plus
scalar subscripting, now carry a distinct access mode through the
reference IR. Access classification uses EDG's **pointed-to type**
rather than its converted rvalue result: integer pointees of 1/2/4/8
bytes with sufficient native alignment are supported. On the target,
each qualified indirect load/store uses the same conservative effectful
RAM operations as qualified globals. GCC differential fixtures test
pointer parameters, repeated volatile reads, scalar writes, 16-bit
accesses, indexing, and C11 atomic loads/stores.

Qualified indirect compound and pre/post increment are **rejected** until
volatile RMW or indivisible C11 atomic indirect RMW is supported; they
must not silently become split non-atomic accesses. Qualified local
storage, struct/union fields and bitfields, mixed/unknown qualifiers,
MMIO, arbitrary pointer-object atomics, misaligned targets and non-scalar
pointees also remain unsupported. A valid C typed pointer must reference
suitably aligned storage.

`tests/fixtures/m12_qualified_globals.c` and the GCC-built driver check
C11 linkage, signed 8/16/32/64-bit volatile accesses, repeated reads,
ordered writes and sequentially consistent atomic scalar reads/writes.
`m12_atomic_rejected.c` guards unsupported CAS-loop arithmetic;
`m12_qualified_indirect_rejected.c` and
`m12_unsupported_indirect.c` guard unsupported indirect atomic and
volatile RMW respectively.

Full M12 completion requires native target and `bingordo/compiler` support
for precise ordering, true volatile load/store primitives, and concurrent
atomic RMW litmus/stress tests.
