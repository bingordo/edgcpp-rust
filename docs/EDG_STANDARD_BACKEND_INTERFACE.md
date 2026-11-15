# EDG native C-backend integration contract (no source hooks)

The EDG Rust bridge is a **back end**, not a second frontend and not a patch
to EDG's lowering implementation. Its native `edg::back_end()` entry point is
invoked by the normal EDG driver after `fe_wrapup()`. The bridge snapshots
EDG-owned native IL synchronously and copies values into the checked Rust
wire representation before EDG cleans up its compilation state.

## Source of truth

- Match EDG's C backend when `IL_SHOULD_BE_WRITTEN_TO_FILE` is enabled:
  read `f_il_output` via `il_read` unless `skip_il_read` is set, then restore
  `primary_source_file_name` from `il_header.primary_source_file`. Reject a
  missing native source-file entry or missing primary scope rather than
  returning an apparently valid empty snapshot.
- Traverse `il_header.primary_scope` for declarations, native types and
  layouts, routine scopes, statements, expressions and ordinary call targets.
- Read `il_header.file_scope_dynamic_init_routines` (when
  `SEPARATE_ROUTINES_FOR_FILE_SCOPE_DYNAMIC_INITS` is enabled) for EDG's
  ordered generated startup routine **roster**. Preserve EDG's identities
  and ordering; never reconstruct it from names.
- Use normal post-lowering `eok_call` callee/argument IL to describe direct
  and indirect calls. A remaining indirect call is **not**, by itself,
  evidence of an originating virtual member call or a vtable slot.
- Keep ABI decisions in the downstream backend. Native EDG layout metadata
  and a syntactically valid expression do not prove object lifetime, a
  physical vtable slot, a required thunk, or the executable C++ ABI.

## Retired private instrumentation

The bridge no longer defines or calls any `edgcpp_capture_*` callback.
It does not inspect pre-lowering nodes or pair private EDG rewrite pointers.
In particular, wire events 80, 81, 82 and 85 (pre-lowering virtual origin,
receiver reuse, vtable index selection and vptr/receiver pairing) are
**not produced** by this bridge. Their historical wire definitions and Rust
validators remain temporarily for compatibility with existing fixtures.
The live Rust callback **rejects all four retired record IDs**, even when
syntactically well-formed; the old validators can only be exercised by
fixture/unit tests. Neither they nor the schema are current evidence or
a promise to reintroduce source hooks.

The existing post-lowered IL records (notably 66/67 for the generated
initialization roster, 77 for call targets, 78 for native call forms and
79 for virtual-member declaration metadata) remain available. None
establishes a complete translation-unit init/shutdown plan or proves that
an indirect call is a virtual dispatch. Missing evidence must fail closed;
do not synthesize legacy proof records from heuristics.

The separate experimental hook-backed file-scope initializer *source slices*
in [bridge PR #255](https://github.com/bingordo/edgcpp-rust/pull/255)
and [EDG PR #7](https://github.com/bingundu/edgcpp/pull/7)
are not part of this contract. Rework their diagnostics from ordinary
post-lowering IL if possible; do not merge hook dependencies merely to
preserve them. The EDG fork's `main` is the exact unmodified upstream
`edgcpp/compiler` commit `177765bb773156a7f3d77fa989f11bc9e1715cfe`,
so there are **no private source observer sites** in the active upstream
lowering. The old fork with its historical hooks is preserved at
`archive/pre-upstream-sync-with-edg-hooks-20261114`, not used for releases.

## Validation

- `tests/native_backend_contract.rs` checks that the C++ bridge has the
  standard backend entry point, reads ordinary IL and declares no private
  source-hook symbols.
- Build and run Rust/native tests and GCC/Clang differential tests using
  **compiler-ci only**. Verify against a consistent, version-pinned EDG
  archive/configuration; do not rebuild EDG routinely for bridge changes.
- In particular check ordinary C calls, indirect function pointers,
  basic C++ virtual-call *negative* cases, generated global initializer
  routine discovery, and C++ global-initializer/iostream fail-closed cases.
- Do not treat a static source guard as native compile/link or runtime
  validation. The CI run and exact EDG revision must be recorded before
  merging.

## Verified source and release (November 2026)

The validated EDG callable-library release is
[`edgcpfe-v2026.11.14-upstream-177765bb7731`](https://github.com/bingundu/edgcpp/releases/tag/edgcpfe-v2026.11.14-upstream-177765bb7731),
from package commit `b195677be8b9b56c8337afad6be8064d21a54082`.
The source lowering is exactly the unmodified upstream tree. The archive
SHA-256 is `1ce495b629924f31382ecccc891a7a72caae1014ddfa2ba7dd39b44d8b0a9526`.
A full native/Rust validation passed in
[compiler-ci run #43658741984](https://github.com/bingordo/compiler-ci/actions/runs/43658741984)
and [CI PR #854](https://github.com/bingordo/compiler-ci/pull/854).
Only `compiler-ci` runs routine automated tests; source workflows are
manual-only.
