# Adaptive direct-call declaration indexing

Audit issue [#148](https://github.com/bingordo/edgcpp-rust/issues/148),
implementation [PR #180](https://github.com/bingordo/edgcpp-rust/pull/180),
measured **2026-11-11**.

## Problem and implementation

The prior compilation-local direct-call catalog (PRs #152 and #155)
avoids repeated ABI validation of a **single** routine symbol. Each **distinct**
target still scanned the complete native IL symbol table, symbol-to-type
edges and type definitions before reaching `function_type_abi`. For a
translation unit containing thousands of unique direct-call targets,
symbol lookup was quadratic in the number of symbols.

The new snapshot-local `DirectCallIndex` builds first-matching
`SymbolId -> routine`, `SymbolId -> TypeId`, and `TypeId -> TypeDef`
maps. It memoizes canonical typedef resolution and fully validated C ABI
plans per canonical function `TypeId`. It does **not** skip first-use
`function_type_abi` checks, aggregate classification, target passing-plan
validation or atomic builtin rejection.

Index construction has a fixed cost. Initial measurements showed a
regression for single-target workloads, so the production catalog builder
uses an adaptive strategy:

- Fewer than **64 distinct routines**: original cheap
  `direct_call_decl` path, once per unique symbol.
- **64 or more distinct routines**: construct the local indexes once,
  then validate each distinct target and reuse function-type ABI plans.

The 64-routine threshold is a pragmatic synthetic-workload crossover
heuristic, **not** a universally proven optimum. Its cost/benefit needs to be
checked against real C/C++ translation-unit profiles. The native graph
validation, duplicate-ID handling and reserved `__atomic_`/`__sync_`
checks are unchanged; there is no cross-compilation persistent cache.
Routine designators are still deduplicated and the final call declarations
are sorted by name.

## Same-runner benchmark

The existing ignored audit benchmark compares three paths on identical
synthetic IL: legacy per-occurrence `direct_call_decl`, the prior
deduplicated call catalog using linear IL scans, and the new adaptive
catalog. The new path's measured time **includes indexing and cache
construction**. Each case runs three times; the median is shown.
All declarations' symbol, native name and complete ABI were compared.

Source: full passing native EDG/GCC
[CI run #43074889798](https://github.com/bingordo/edgcpp-rust/actions/runs/43074889798)
on the pinned Linux x86-64 EDG environment, Rust 1.99.1 unoptimized test profile.

### All uses refer to one target

| Call uses | Legacy rescans (ms) | Prior cached (ms) | Adaptive (ms) | Adaptive / prior throughput |
| ---: | ---: | ---: | ---: | ---: |
| 256 | 0.786 | 0.077 | 0.082 | 0.95× |
| 1,024 | 8.984 | 0.294 | 0.305 | 0.97× |
| 4,096 | 133.093 | 1.165 | 1.213 | 0.96× |

The adaptive algorithm intentionally retains the low-overhead
per-unique-declaration path for this workload. Median differences below
0.05 ms at these sizes should not be treated as stable regressions.

### Each use refers to a different target

All symbols in this workload share one native valid function `TypeId`.

| Unique routine targets | Legacy rescans (ms) | Prior cached (ms) | Adaptive (ms) | Adaptive / prior throughput |
| ---: | ---: | ---: | ---: | ---: |
| 256 | 0.526 | 0.666 | 0.557 | 1.19× |
| 1,024 | 5.150 | 5.858 | 2.227 | 2.63× |
| 4,096 | 67.231 | 72.067 | 9.151 | 7.88× |

The largest measured distinct-target case is 7.35× faster than the
original per-occurrence scan and **7.88× faster than the previous
compilation-local catalog**.

## Reproduction

Use a native EDG-configured build or invoke the pinned EDG GitHub Actions
workflow manually and check `audit_call_lookup_benchmark`. To run just
this test:

```sh
cargo test --lib --features codegen audit_direct_call_catalog_scaling -- --ignored --nocapture
```

The normal test suite does not run the slower audit benchmark.
Normal CI always runs strict Clippy, the adaptive 1/63/64/128-target
regression test and the fail-closed native metadata tests.

## Limits and further work

- These are **synthetic declaration-resolution microbenchmarks**, not
  end-to-end compilation speedups. The controlled inputs mostly share
  a function signature and omit source-language parsing, IR optimization,
  final object generation and link costs.
- Calls to many routines with **different function TypeIds** still use
  `function_type_abi` once for each unique type. Its signature/type
  parameter and aggregate checks may become the next bottleneck.
  Optimize those checks only after measuring representative workloads,
  retaining all native and target ABI validation.
- The 64-target index threshold should be revisited using real-world
  call-heavy and no-call compilation fixtures.
