# Code review — EDG Rust bridge, 20 October 2026

## Scope

Reviewed the 42 commits after `9a5dc712b5ea`, up to `243b3e118f64`,
then reviewed and applied corrective commits through `9e11ce1340b1`.
The review covers `cpp/bridge.cpp`, `src/lib.rs`, `src/graph.rs`,
`examples/parse.rs`, C++ fixtures, GitHub Actions, and README.

Changes under review: explicit/implicit parameters, lexical/local scopes,
static locals hoisted across EDG memory regions, declaration/initializer
statement sites, source file/line/column metadata, object-lifetime trees,
and ordered destructor cleanups.

## Method

- Inspected the GitHub commit comparison, current bridge/decoder/graph
  implementations, and the corresponding definitions in EDG `il_def.h`,
  `basics.h`, and `il.h`.
- Checked event payload lengths, nullable fields, IDs, callback ownership,
  graph validation, parent/child relationships, and cleanup list ordering.
- Inspected actual GitHub Actions integration logs, including #215.
- Added regression tests for routine-name/symbol-ID mismatch and malformed
  lifetime metadata; extended native assertions for lifetime roots, scope
  associations, and destructor order.

## Findings resolved

### 1. CI source filename mismatch — verification blocker

CI searched for `namespaces.cpp` when the fixture was copied and parsed
as `il_fixture.cpp`. The actual bridge emitted the correct filename.
The run #215 built EDG, passed 59 Rust unit tests and Clippy, then failed
the assertion. Fixed in `9427976c` and verified that the pattern matches
the observed native output.

### 2. Stale lexical-scope pointer map across routines — correctness risk

The map from EDG scope pointers to statement paths was cleared only per
compilation. EDG function memory regions can be reused; addresses should
not be treated as globally unique in this map. Clear the map before
exporting each function body, while retaining scope ownership during that
body's traversal. Fixed in `1346d109`.

### 3. Routine ownership inferred from display names — correctness risk

Graph lifetime validation and scope-path linking used symbol display names
as function identity. First-encounter symbol spelling can differ from the
function-name key exported by the bridge (for example a full signature
versus a shortened unambiguous name, overloads, or destructor variants).

Native event 42 now provides an authoritative
`RoutineIdentity { routine: SymbolId, function: String }` relation.
The graph checks unique routine and function keys and prefers this mapping
over display-name inference. Queries `routine_function` and
`routine_for_function` expose it. A unit test deliberately uses
mismatching names and verifies ownership. Fixed in
`2847a1a5`, `43e4ebfc`, `2b8871d0`, and `1660ddf0`.

### 4. Invalid empty initialization-lifetime records — graph invariant

Hand-constructed IL could attach a lifetime record to a nonexistent routine
when it carried only a destructor reference, or accept a record with neither
lifetime nor destructor. Both are now rejected, with negative test coverage.
Fixed in `9e11ce13`.

### 5. Documentation drift — user-facing correctness

README incorrectly described class scopes as untraversed, source locations
as unavailable, a 13-event callback protocol, and missing type/parameter
relationships that were already implemented. It also contained literal
backslash-n escapes in prose. Updated in `0a2f6149`; authoritative
routine identity documented in `015388ce`.

## Verified behavior and caveats

- Source locations are resolved through EDG's sequence-to-file map. Unknown,
  generated, and unmappable positions are intentionally omitted.
- Source positions identify location starts, not complete spans or macro
  expansion-stack metadata.
- Object-lifetime regions are not exact live intervals. Destruction
  records follow EDG's destructor list, which may include cleanup entries
  without explicit variable/destructor symbols.
- C++ static locals may be hoisted into a different EDG memory region;
  ownership is recovered via EDG pointers or, when absent, only an
  unambiguous fully qualified function-signature match.
- The data remains a partial, compilation-local EDG IL snapshot, not a
  standalone or stable serialization.

## Remaining risks and follow-ups

1. **Pointer identity across recycled EDG memory regions**: source-symbol
   and type IDs are keyed by native addresses over one compilation.
   Review whether EDG ever recycles addresses across function-region loads
   on very large translation units. A multi-region stress fixture should
   verify identity uniqueness before widening the visitor further.
2. **Graph build complexity**: many type/symbol checks perform repeated
   linear scans of `graph.symbols` and may become quadratic for large IL
   snapshots. Build symbol/type-kind maps once and measure with larger inputs.
3. **Lifetime completeness**: source-level lifetime/destructor coverage
   depends on EDG lowering and which lifetime regions remain reachable
   from the function's scope tree. Exception cleanup and block-after-label
   stress tests should assert ordering and parent relations independently.
4. **Source resolution overhead**: file/line resolution is currently
   performed per emitted source position; benchmark before caching by
   sequence number.

## Verification record

- Last fully successful pre-review milestone: GitHub Actions #202,
  commit `95dac105`, EDG native build, 56 Rust tests, Clippy and native
  integration all passed.
- GitHub Actions #215, commit `243b3e118f64`: EDG build, 59 Rust tests,
  Clippy and native output succeeded; the job failed the mismatched
  source-filename assertion.
- Post-review integration run: inspect the newest GitHub Actions push
  build for commits after `9e11ce13`; do not mark it passing until
  the workflow reports success.
