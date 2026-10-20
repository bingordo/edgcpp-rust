# Review: declaration, expression and type dependency APIs

Date: 2026-10-20
Repository: bingundu/edgcpp-rust, branch `main`
Original review range: `4f64f5fc334f39b3906773ee1809e8b56bb356f0..50aca13a076a6bd1beda1d6fadf9f8a31b3150d4` (42 commits).

## Scope and method

Inspected the 42 commit diffs and the cumulative changes to `src/graph.rs`
and `README.md`. Cross-checked introduced types against `src/lib.rs`
and the CI workflow. The review covered borrowed declaration views, ordered
expression visitors and statistics, direct/transitive type dependencies,
reverse traversal, SCC condensation, invalidation scheduling and parallel
waves, mutable indexed adjacency and batched differential invalidation.

This is a source review plus newly added regression coverage; it is not a
claim that local EDG builds or integration tests ran. Push CI runs
`cargo test --all-targets` and `cargo clippy --all-targets -- -D warnings`
in the configured native EDG environment.

## Issues found and fixes

1. **Correctness: wrong symbol kind accepted by borrowed declaration
   views.** `routine_view()` returned a routine view for variable symbols
   and `variable_view()` returned variable views for routine symbols.
   Enforce EDG symbol kind 1 (routine) or 2 (variable), respectively, and
   test both rejection paths. Fix: `bf00407`; tests: `f44f236`.

2. **Performance: quadratic initial index construction.** Constructing
   `TypeDependencyIndex` previously invoked `type_dependencies()` for
   every type, rescanning the entire type, base, field and type-parameter
   tables, plus per-symbol type lookups. Build the forward index in one
   pass over each IL table; build reverse adjacency with per-source edge
   deduplication. Added 32-type mixed-category graph/index equivalence
   coverage. Fix: `c51370d`; tests: `7f94dca`.

3. **Performance and maintenance: duplicate SCC implementations and
   quadratic member rank lookup.** `Graph` built SCCs from repeated
   graph scans while `TypeDependencyIndex` had its own SCC scheduler.
   Consolidated `Graph::type_strongly_connected_components()` onto the
   indexed condensation and replaced repeated linear searches with a
   node-rank map. Added tests asserting dependency-first schedule and wave
   invariants for chains and cycles. Fixes: `c51370d`, `e719bac`;
   tests: `7f94dca`.

4. **Performance: batch mutation reverse-edge cross-product.** Updating
   a batch checked each changed source against every touched target.
   Accumulate new reverse edges once per changed source, then refresh only
   touched destinations. Fix: `cd2ab7e`. Existing batch-vs-sequential
   equivalence tests exercise the affected result semantics.

5. **Completeness: missing explicit API documentation.** Documented
   declaration kind validation, index snapshot invalidation, ordered batch
   edits and membership-oriented before/after diff behavior. Fixes:
   `4a91869`, `4c9ed0f`.

## Remaining design constraints

- Type and symbol identities are compilation-local.
- The index is an owned snapshot. Changing `Graph` does not automatically
  update it; callers must apply matching changes or rebuild.
- Graph edges are captured structural relationships, not a complete
  C++ semantic invalidation model or an ABI layout.
- Diff results describe closure membership changes; retained members with
  changed shortest distance appear in `before_types` and `after_types`,
  not in `newly_affected_types`.
- The scheduled SCC components are dependency-first, but only captured
  type dependencies are included; downstream passes own additional ordering
  and synchronization requirements.

## Verification

New tests compare the indexed builder with the original graph queries across
mixed type-edge categories and verify SCC schedule and wave ordering, cycles,
type removals, symbol reassignment, batched-vs-sequential equivalence and
newly/no-longer affected membership.

CI: https://github.com/bingundu/edgcpp-rust/actions/workflows/build.yml

CI result for these review fixes must be checked separately. No local
EDG build or Rust integration test execution is claimed here.
