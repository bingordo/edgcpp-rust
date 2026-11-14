# EDG native C-backend integration contract (no source hooks)

The EDG Rust bridge is a **back end**, not a second frontend and not a patch
to EDG's lowering implementation. Its native `edg::back_end()` entry point is
invoked by the normal EDG driver after `fe_wrapup()`. The bridge snapshots
EDG-owned native IL synchronously and copies values into the checked Rust
wire representation before EDG cleans up its compilation state.

## Source of truth

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
validators remain temporarily for compatibility with existing fixtures,
not as current evidence or a promise to reintroduce source hooks.

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
preserve them. The fork's remaining **weak optional** callback call sites
do not have definitions supplied by this bridge and should be deleted
separately.

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
