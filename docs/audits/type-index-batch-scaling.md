# Type dependency index batch-update audit

Measured on **2026-11-11** for [audit issue #171](https://github.com/bingordo/edgcpp-rust/issues/171).
The algorithmic change is [PR #174](https://github.com/bingordo/edgcpp-rust/pull/174);
the reproducible ignored benchmark is in [PR #176](https://github.com/bingordo/edgcpp-rust/pull/176).

## Test method

- GitHub Actions `ubuntu-latest` runner and the pinned EDG x86-64 container,
  using Rust 1.99.1's **unoptimized test profile**.
- In the **same process and on the same runner**, compare a test-only frozen
  reproduction of the original O(batch × vector-length) linear-scan logic
  against the production indexed `TypeDependencyIndex::apply_changes`.
- Three repetitions per implementation and size; report **median elapsed
  milliseconds**. Results below are from the full passing native CI
  [run 43051101205](https://github.com/bingordo/edgcpp-rust/actions/runs/43051101205).
- The symbol workload updates 1,024 / 4,096 / 16,384 existing symbols in
  descending symbol-ID order, with types partitioned across 32 buckets.
  Both implementations must produce identical ordered symbol entries
  and type-to-symbol lookup buckets.
- The type workload removes 1,024 / 4,096 / 16,384 existing types in descending
  ID order. **Dependency lists are empty** to isolate ordered-vector deletion
  overhead. Both implementations must produce identical ordered type lists
  and forward/reverse type edge maps.

## Symbol-type batch updates

| Symbol updates | Original linear scans (ms) | Indexed batch (ms) | Speedup |
| ---: | ---: | ---: | ---: |
| 1,024 | 5.230 | 1.274 | 4.10× |
| 4,096 | 70.985 | 5.058 | 14.03× |
| 16,384 | 1,091.486 | 20.234 | 53.94× |

## Type-removal batches

| Types removed | Original per-removal vector filtering (ms) | Indexed batch (ms) | Speedup |
| ---: | ---: | ---: | ---: |
| 1,024 | 3.598 | 1.382 | 2.60× |
| 4,096 | 56.706 | 5.672 | 10.00× |
| 16,384 | 882.828 | 25.129 | 35.13× |

## Reproduction

The test is deliberately ignored by normal `cargo test` runs:

```sh
cargo test --lib --features codegen audit_type_index_batch_scaling -- --ignored --nocapture
```

Run it in the pinned native EDG build environment used by CI.
Alternatively use **Actions → Rust EDG bridge (Linux x86-64) → Run workflow**
on `cranelift-codegen`, selecting the optional
`audit_type_index_benchmark` input.
Ordinary pull requests and pushes do **not** run these slower microbenchmarks.

## Interpretation and limits

These are **synthetic worst-case mutation batches**, not end-to-end compiler
measurements. The large speedups establish that the repeated linear-order
scans/removals have been eliminated for these workloads; they do not imply
equivalent improvements to EDG parsing or C/C++ compile time. Different
runners, CPU contention, optimizations and dependency graph densities may
produce different timings. Small/single edits intentionally retain a
low-allocation path. The ordinary test suite separately compares thousands of
mixed indexed batch changes against equivalent sequential updates, including
remove-and-readd ordering, reverse dependencies, and affected-symbol sets.
