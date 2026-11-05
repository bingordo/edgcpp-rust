# M14 scalar floating-point loops and SSA correctness

**Status:** implemented and validated on the Cranelift reference backend
(2026-11-05), via [PR #62](https://github.com/bingordo/edgcpp-rust/pull/62).
The native EDG/GCC differential and full regression gate passed in
[workflow run #42037880876](https://github.com/bingordo/edgcpp-rust/actions/runs/42037880876).

This completes the **scalar floating-point loop/SSA slice**, not the whole
[M14 floating-point integration roadmap](https://github.com/bingordo/edgcpp-rust/issues/60).

## Supported EDG source control flow

- `while`, `for` (including missing condition), and `do/while` with
  checked F32/F64 comparisons and automatic FP scalar local assignments.
- Nested loops and conditional assignments, with source-classified
  `break`/`continue` targets; `continue` reaches the while header,
  for increment, or do/while test as appropriate.
- Early FP returns, including returns nested inside loop branches.
- F32 and F64 loop-carried values. The reference emitter uses Cranelift SSA
  variables and block parameters, not untyped integer bitcasts.
- This code supports pure FP operands and source-verified FP local types.
  It does not claim general mixed-integer/FP expressions or signatures.

## SSA and definite assignment invariants

1. Every FP local starts uninitialized unless its source initializer executes.
   Reads in expressions and comparisons are checked before code emission.
2. The verifier carries separate reachable normal, break and continue paths.
   A value is definitely initialized at a merge only if it is initialized
   on **every reachable incoming predecessor**; terminated paths do not
   contaminate a normal-edge join.
3. A `while` or `for` condition is checked using facts true on the first
   (possibly zero-iteration) entry. A body-only write is not inferred at its
   header or at the test-false exit.
4. `for` increments are checked with the intersection of body fallthrough
   and continue predecessors; a continue bypassing an assignment cannot
   manufacture a value for the increment.
5. The first `do/while` test uses the intersection of body fallthrough and
   continue paths after one body execution. Break bypasses that test and
   contributes only to the loop exit.
6. Cranelift loop headers stay **unsealed** until every backedge and
   continue path has been emitted. Exit and join blocks are sealed after
   their known incoming edges, allowing the SSA builder to insert typed
   F32/F64 merge parameters.
7. Unsupported native statements, unverified gotos/labels, and unreachable
   trailing executable statements fail closed before object emission.

These checks are conservative: some legal but unsupported C control-flow
forms may still be rejected. That is preferable to silently emitting an ELF
object with undefined local reads or missing control effects.

## Validation

- Rust `src/codegen/fp.rs` unit tests cover nested FP SSA joins,
  zero-iteration loop tests, loop-carried variables, nested early returns,
  break/continue, and unsafe incoming paths.
- Native fixture `tests/fixtures/m14_fp_loops.c` plus
  `m14_fp_loops_driver.c` compares emitted ELF against GCC with F32/F64
  loops, nested loops, NaNs, signed zero, and conditional exits.
- Negative fixtures `m14_fp_loops_bad_*.c` require a diagnostic and no
  emitted object for uninitialized zero-iteration results, first-header
  reads, bypassed `for` increment initialization, unsafe do/while tests,
  and partially initialized break exits.
- The pinned EDG + Cargo + existing codegen/GCC differential suite passed in
  [run #42037880876](https://github.com/bingordo/edgcpp-rust/actions/runs/42037880876).

## Remaining broader M14 work

The FP reference path is intentionally separate from the integer CFG.
Unifying their typed control-flow and ABI machinery, mixed INTEGER/SSE
signatures and calls, FP globals/address-taken locals, pointer memory,
more general boolean/side-effecting conditions, and the independent
`bingordo/compiler` adapter remain open. Do not call the entire M14
language or ABI milestone complete. Atomic expansion is separately deferred
under [issue #59](https://github.com/bingordo/edgcpp-rust/issues/59).
