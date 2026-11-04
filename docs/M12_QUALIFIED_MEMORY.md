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

**M12 indirect RMW follow-up:** C11 integer pointer pre/post increment
and integer add/sub/AND/OR/XOR compound updates now execute as indivisible
native sequentially consistent `atomic_rmw` operations. The lvalue
address and RHS are each evaluated exactly once. For RAM `volatile`
pointees, compound updates perform an observable read followed by an
observable store, respecting the existing C integer promotion rules;
they are not C atomic transactions. The GCC/Cranelift differential
checks old/new return values, narrower integers and 40,000 increments
across four threads through an `_Atomic int*`.

**M12 CAS arithmetic follow-up:** Atomic integer multiply, divide, modulo
and left/right shifts use a sequentially consistent native compare-exchange
retry loop, sharing one implementation for globals and pointer lvalues.
The address and RHS are evaluated once outside the retry loop. Each failed
CAS retries only the memory read, arithmetic and compare-exchange.
Both GCC and Cranelift-generated objects are tested for expression
results, including a concurrent test mixing increments with atomic
multiply-by-one to force possible CAS collisions.

Atomic pointer objects, source-visible compare-exchange APIs, explicit
memory orders other than the default sequentially consistent ordering,
and fences remain unsupported. These must not silently lower to
non-atomic memory accesses. Qualified local
storage, struct/union fields and bitfields, mixed/unknown qualifiers,
MMIO, arbitrary pointer-object atomics, misaligned targets and non-scalar
pointees also remain unsupported. A valid typed pointer must reference
suitably aligned storage.

`tests/fixtures/m12_qualified_globals.c` and the GCC-built driver check
C11 linkage, signed 8/16/32/64-bit volatile accesses, repeated reads,
ordered writes and sequentially consistent atomic scalar reads/writes.
`m12_atomic_rejected.c` guards unsupported CAS-loop arithmetic;
`m12_qualified_indirect_rejected.c` and
`m12_atomic_rejected.c` guard unsupported atomic pointer objects;
`m12_unsupported_indirect.c` guards volatile bitfields.

Full M12 completion requires native target and `bingordo/compiler` support
for precise ordering, true volatile load/store primitives, and concurrent
atomic RMW litmus/stress tests.

## Source memory-order validation groundwork (2026-11-03)

The public `memory_order` Rust module now defines all six C11 orders, operation
categories, and checked load/store/RMW/fence and compare-exchange ordering
contracts, including the failure-order constraints. Unit tests enumerate all
36 success/failure combinations. This is a **semantic validation API only**:
there is currently no EDG builtin extraction, `_explicit` lowering, native
compare-exchange expected-value writeback, or backend mapping of those orders.
Do not infer runtime support from the presence of this module. The seq-cst-only
reference subset described above is unchanged, and source-visible explicit
orders must still be rejected until properly decoded and emitted. M12 remains
open under issue #58.

## M12 follow-up checkpoint — qualified automatic storage and builtin boundary (2026-11-03)

**Passing end-to-end CI:** [run 41752951126](https://github.com/bingordo/edgcpp-rust/actions/runs/41752951126).

The x86-64 Cranelift reference now accepts naturally aligned **scalar integer**
volatile and C11 atomic automatic locals and formal parameters, preserving
the declared qualifiers rather than silently sinking them to ordinary SSA.
Locals are addressable; qualified formals receive an effectful entry store.
Direct reads, assignments, increments and compound arithmetic use qualified
memory operations, including initialization in GNU statement expressions.
Tests compare generated ELF with GCC for locals/formals, indirect addressing,
and both signed and unsigned 1/2/4/8-byte scalars. Unsupported atomic pointer
objects remain rejected before ELF emission.

The runtime seq-cst **store-buffering** litmus has a two-thread GCC and
generated-ELF differential and an x86 machine-effect assertion accepting
either an exchange/LOCK sequence or an ordering fence. The standalone
backend also has a checked native compare-exchange expression with
conditional expected-value update only on failure, validated by executing a
native object from a GCC caller. Success/failure orders are checked with the
public `memory_order` module; this reference backend currently strengthens
accepted orders to seq-cst. This compare-exchange expression is **not yet
extracted from EDG's source-level C11 builtins** and is **not** a claim of a
working source-visible compare-exchange API.

**Native EDG builtin probe:**
- The pinned native EDG configuration cannot find `stdatomic.h`; a normal
  source-visible C11 API fixture therefore fails at header resolution.
- Header-independent GCC `__atomic_*_n` expressions parse in GNU mode,
  but EDG rewrites them to external calls such as `__atomic_load_4`,
  `__atomic_compare_exchange_4`, and `__atomic_thread_fence`.
- The old codegen path emitted an ELF with undefined atomic references.
  This is now an explicit **fail-closed error** instead of an accidental
  claim of complete source memory-order semantics. CI verifies no object
  exists after the rejection. It is not a linkable implementation of those
  builtins.

**Still blocking M12 completion (issue #58):** a validated C11 header and
native builtin-to-semantic decoding, explicit source-order propagation across
all atomic operations, source-visible exchange/fetch/CAS/fence with failure
readback, qualified aggregates/field and bitfield semantics, genuine MMIO
volatile lowering or a documented runtime/target alternative, and the
portable `bingordo/compiler` adapter carrying those memory effects to its
SSA/backend. Continue to fail closed until these are verified. This checkpoint
is intentionally not a declaration that M12 is complete.

## M12 continuation — source atomic operations, qualified fields (2026-11-04)

**Latest full green run:** [41781275228](https://github.com/bingordo/edgcpp-rust/actions/runs/41781275228).
This supersedes the earlier *unresolved builtin* status for the checked
integer subset; it does not supersede the remaining completion gates.

**Implemented, source-visible in EDG GNU mode:** The native EDG bridge
projects width-suffixed `__atomic_load_N`, `__atomic_store_N`,
`__atomic_exchange_N`, `__atomic_fetch_{add,sub,and,or,xor}_N`,
`__atomic_compare_exchange_N`, `__atomic_thread_fence` and
`__atomic_signal_fence` (N=1,2,4,8). A dedicated projection follows
source expression identities through EDG's void-pointer conversion nodes,
recovers the original qualified integer declaration, validates layout,
literal source orders, compare-exchange success/failure combinations and
weak flags, then emits native atomic instructions. This backend conservatively
strengthens all accepted operations to sequential consistency; its weak
compare-exchange implementation is strong, which C11 permits. Exchange/fetch
return the old value, CAS returns success and writes `*expected` only when
unsuccessful. Explicit invalid/dynamic order, wrong/misaligned source pointer,
noninteger atomics, and unsupported builtin arities fail closed **before ELF**.
`compile_calls` never emits unresolved `__atomic_*` references for this subset.

The pinned EDG configuration still does not locate a system `stdatomic.h`.
The test-only `m12_atomic_compat.h` maps C11 `*_explicit` spellings to
those native GNU builtins; it is not an installed standard library header
and does not claim complete C11 `stdatomic.h` API support.

**Qualified fields:** Source-qualified volatile and atomic integer members
of ordinary struct/union object lvalues use the same effectful scalar access
path. EDG field identities and native offsets/alignments are mandatory;
packed/misaligned atomic members and qualified bitfields are rejected. This
does not yet allow volatile-qualified aggregate bases, atomic aggregate copies,
or volatile bitfield storage-unit semantics. Existing array, ordinary member
and scalar behavior remains covered by the full regression suite.

**Checks:** Native/GCC differential for explicit load/store/exchange/fetch,
strong/weak CAS and expected-value updates, relaxed/acquire/release/acq_rel/SC
orders, thread/signal fences, cross-thread release/acquire publication,
failure-order violations and dynamic order rejection. Separate tests cover
1/2/4/8-byte signed/unsigned qualified scalar RAM, qualified locals/formals,
records, concurrency and store buffering. ELF is scanned to prevent unresolved
`__atomic_*` imports in the implemented subset.

**M12 issue #58 is still OPEN:** (1) a proper system C11 atomic-header and
full native API integration, including other types/runtime orders;
(2) a real volatile-MMIO primitive or verified runtime fallback, preserving
device access count/width without treating device registers as cache-coherent
RAM atomics; (3) qualified bitfield and aggregate operations, and inherited
qualifiers where legal; (4) a complete EDG→`bingordo/compiler` SSA producer
and backend legalization. The `compiler` repository now has a checked
`crates/ir/src/source_memory.rs` contract with conservative target preflight
([compiler CI 41781136545](https://github.com/bingordo/compiler/actions/runs/41781136545)),
but that semantic contract is not a functioning adapter. None of these
remaining items should be marked finished without native verification.
