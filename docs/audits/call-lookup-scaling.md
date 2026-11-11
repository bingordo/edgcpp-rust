# Native call-lookup and direct-call ABI catalog audit

Measured **2026-11-11** for [issue #138](https://github.com/bingordo/edgcpp-rust/issues/138)
and [issue #148](https://github.com/bingordo/edgcpp-rust/issues/148).
The indexed algorithms were introduced in PRs #141, #152, #155 and #157.
This benchmark was added in [PR #179](https://github.com/bingordo/edgcpp-rust/pull/179).

## Method

- Run on a single GitHub Actions Linux x86-64 native EDG runner with the
  pinned Rust 1.99.1 **unoptimized test profile**.
- Each old and new path runs **three repetitions**, reporting the median
  elapsed milliseconds. Both paths execute on the same runner on identical
  synthetic workloads.
- Source of timings: complete passing native
  [CI run #43057376646](https://github.com/bingordo/edgcpp-rust/actions/runs/43057376646).
- Frozen old code in ignored tests performs linear scans; new code calls the
  production indexed methods. Both return values must agree.
- These are targeted lookup and ABI-declaration microbenchmarks; **they do
  not measure whole-program compiler speedups**.

## Graph call-site lookups (issue #138)

For 512, 2,048 and 4,096 distinct call sites, the benchmark uses twice as
many expression lookup IDs, interleaving non-call expressions with calls.
The baseline searches the old call vector by native function, statement,
and expression path; the new path uses `Graph::call_at(ExpressionId)`.
For inverse lookup, the baseline scans the call vector for an equal
`CallSite`, while `Graph::call_expression` uses the indexed map.
A separate zero-call graph verifies that non-call IDs resolve to no call
metadata; this control is **correctness-only**, not a timed workload.

| Calls | Forward scan ms | Forward index ms | Speedup | Reverse scan ms | Reverse index ms | Speedup |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 512 | 4.567 | 0.022 | 203.61× | 1.932 | 0.156 | 12.40× |
| 2,048 | 70.731 | 0.101 | 702.50× | 29.117 | 0.747 | 38.98× |
| 4,096 | 290.070 | 0.185 | 1,569.73× | 120.110 | 1.595 | 75.32× |

## Direct-call declarations and ABI catalog (issue #148)

A synthetic IL contains actual pinned native routine and void type-kind
values, a void-returning function signature, and 256/1,024/4,096 distinct
named routine symbols sharing that signature. The **repeated** workload
uses the last routine symbol for every call occurrence. The **distinct**
control uses each routine symbol once.

The old path runs the complete `direct_call_decl` validation and ABI
planning for every occurrence; the new path creates a compilation-local
`HashMap<SymbolId, CallDeclaration>` containing **one validated declaration
per used target** and performs indexed lookups for occurrences. Catalog
construction and validation are **included in the new-path elapsed time**.
For every occurrence, symbol, name, and full ABI structure must be equal.

| Uses | Repeated old ms | Repeated cached ms | Speedup | Distinct old ms | Distinct cached ms | Speedup |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 256 | 1.607 | 0.230 | 6.99× | 1.064 | 1.477 | 0.72× |
| 1,024 | 9.702 | 0.319 | 30.42× | 5.380 | 6.301 | 0.85× |
| 4,096 | 140.019 | 1.266 | 110.60× | 72.116 | 76.121 | 0.95× |

Repeated direct-call resolution benefits substantially from the shared
catalog; building one entry for every distinct call target does not.
The distinct workload still pays for full native IL symbol/type/prototype
linear scans per unique target, so more work on compilation-local
metadata indexing may be justified **if real-world profiling supports it**.

## Reproducing

The two tests are ignored by ordinary Rust test runs:

```sh
cargo test --lib --features codegen audit_call_lookup_scaling -- --ignored --nocapture
cargo test --lib --features codegen audit_direct_call_catalog_scaling -- --ignored --nocapture
```

Use the native EDG build environment established by the regular CI job, or
run the GitHub Actions `Rust EDG bridge (Linux x86-64)` workflow manually
(`workflow_dispatch`) on `cranelift-codegen` with the
`audit_call_lookup_benchmark` input set to true. Normal pull requests and
pushes skip the benchmark step.

## Interpretation and follow-up

The speedups establish the cost of previously redundant lookups in the
isolated workloads, **not** reductions in end-to-end EDG parsing, CFG
lowering, optimization, code generation, or whole-C/C++ compile time.
Test-profile timings are not optimized-release throughput and microsecond
measurements may be sensitive to runner load. For #138, measure a real
call-heavy and no-call CFG lowering fixture separately; for #148, inspect
whether distinct-target metadata indexing produces a net benefit with a
representative large translation unit. No changes to atomic semantics,
C++ support, or native wire format are involved in this benchmark.
