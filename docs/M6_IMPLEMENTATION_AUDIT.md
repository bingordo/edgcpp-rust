# M6 implementation audit and next gates

This document records **observed source state**, not a claim of new M6 completion.

## Already implemented on the Cranelift branch

- `src/codegen/cfg.rs:95` contains `lower_gnu_block` for a **restricted**, straight-line GNU statement expression. It translates scalar stores/evaluations into `Expr::BlockValue`, and requires a final scalar result.
- `src/codegen/cfg.rs:315` checks block-value expression effects in order; `src/codegen/cfg.rs:1302` emits them in order.
- `src/codegen/cfg.rs:319` validates direct-call arity, and `src/codegen/cfg.rs:1308` emits direct Cranelift calls through declared function references with typed arguments/results.

These facilities must be **extended**, not duplicated. The older M5 README statement that GNU blocks and calls are entirely unsupported is now too broad; full support is still unverified.

## Next engineering gate: statement expressions

1. Add native differential fixtures for GNU blocks with declaration initialization, nested blocks, mutation of enclosing locals, and a final scalar value. Include a negative fixture for unsupported control flow.
2. Audit EDG's statement tree for declarations and synthetic `.gnu` children. Do not assume every declaration is non-executable; explicitly model initializers and destructors/lifetimes where supported, reject otherwise.
3. Replace the expression-only effect vector with a generic scoped block representation when needed for branching and nested statement control flow. Ensure result evaluation occurs after prior effects, once only, and that local identities are not accidentally shared across scopes.
4. Extend definite-initialization and reachability checking to the block's CFG. A branch that initializes only one path must not establish definite initialization on both.
5. Add GCC differential tests for nested side effects and short-circuit expressions; do not count a compile-only test as passing.

## Next engineering gate: calls

1. Differential-test internal and external direct C calls, void returns, scalar argument conversions, nested calls, and mutations of globals.
2. Verify EDG symbol linkage maps to exactly one ELF symbol, including forward declarations and definitions.
3. Verify the C ABI at 8/16/32/64-bit integer widths, pointer arguments, and return normalization.
4. Reject unsupported varargs, indirect calls and incompatible declarations before producing an object.

## Validation

Run targeted codegen unit tests and native GCC comparisons first. Then run the fixed eight-seed YARPGen smoke suite, followed by the 100-seed manually dispatched cohort. Record separate counters for pass, unsupported, codegen failure, link failure, and wrong output. Keep CI on bingordo only and reuse the pinned EDG/YARPGen binaries.

**Status:** source audit and next-gate plan only; no new runtime or CI result is asserted here.
