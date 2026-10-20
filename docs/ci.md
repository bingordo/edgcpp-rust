# Efficient CI for edgcpp-rust

The workflow at [.github/workflows/build.yml](../.github/workflows/build.yml)
uses an upstream *binary EDG release* rather than compiling the EDG compiler.

## Upstream EDG version (pinned)

- Source repository: `bingundu/edgcpp`
- EDG source commit: `799603e0b5ab272d9418e2d44a53a95bc73dfdc4`
- GitHub release tag: `edgcpfe-ci-799603e0b5ab`
- Release asset SHA-256: `fa769825e70766fbefe017806816b2338acd03daa93bb5562369c293972ce64c`
- Asset: `edgcpfe-ci-x86_64-callable-release-linux-x86_64.tar.gz`
- Permanent release URL:
  https://github.com/bingundu/edgcpp/releases/tag/edgcpfe-ci-799603e0b5ab
- Original successful EDG build:
  https://github.com/bingundu/edgcpp/actions/runs/39259142410

The package contains a PIC-capable `libedgcpfe.a` and its `BUILD-INFO.txt`.
A lightweight upstream *publisher* workflow promoted that previously tested
artifact into a permanent GitHub Release; it did not recompile EDG.
The publisher is now manual-only, and it verifies the existing asset instead
of overwriting the same release tag:

https://github.com/bingundu/edgcpp/blob/ci/x86_64-callable-release/.github/workflows/publish-callable-artifact.yml

CI checks out the **exact source commit** matching that archive because
`cpp/bridge.cpp` includes internal EDG IL headers. It performs a **CMake
configure only**, which generates `compile_commands.json` and the runtime
environment, and generates EDG's error-message headers with the small `error-info` target
(`mk_errinfo`), **without building the EDG front end**. All links use the
upstream library binary. The archive
metadata is checked against the pinned EDG source SHA before testing.
CI also checks the complete archive against the pinned SHA-256 digest **before
extracting it**, and verifies that the actual checkout HEAD matches the
binary's source revision.

### Updating EDG

When the upstream EDG version changes:

1. Build and test the callable EDG archive in `bingundu/edgcpp`.
2. Publish that successful archive under a **new**, version-specific
   GitHub Release with a matching `BUILD-INFO.txt` commit SHA, and record its
   SHA-256 digest. Avoid overwriting an existing release asset.
3. Update the SHA under the EDG `checkout` step and the
   `EDG_SOURCE_SHA`/`EDG_RELEASE_TAG`/`EDG_RELEASE_SHA256` environment variables
   in `build.yml` **together**.
4. Confirm that the Rust bridge still compiles and the IL integration tests
   pass. Never switch only the library without its corresponding headers,
   macro configuration and compilation database.

There is no fallback EDG build inside this workflow: failure to obtain a
matching binary should fail fast rather than silently spend CPU compiling
a different release.

## Incremental Rust caching

The workflow restores `edgcpp-rust/target` and Cargo registry/git caches
with `actions/cache/restore` before the Rust build. On a successful
`main` push, it saves the updated snapshot once using
`actions/cache/save`. Pull requests restore but do not upload their own
large target snapshots. Cache keys contain the source commit SHA; restore
prefixes reuse the latest compatible prior incremental build. Cache keys
include both the pinned EDG source revision and Rust toolchain version;
artifacts compiled against different EDG IL layouts cannot be restored
through these prefixes.

The build script also observes `EDGCPP_EDG_REVISION` in CI so updating the
revision invalidates the native bridge even when Cargo has a warm cache. Local
builds without this variable instead watch EDG source/header inputs and the
native archive for changes. CI uses Rust toolchain `1.99.1` to make incremental
outputs reproducible across runs.

This intentionally favors reduced compilation CPU over accumulating
unbounded caches from every PR. The cache is purely an optimization:
removing caches must never change correctness.

`CARGO_PROFILE_DEV_DEBUG=0` and `CARGO_PROFILE_TEST_DEBUG=0` reduce
debug information produced by CI; the tests are still executed.

## Run policy and verification

- Pushes: `main` only.
- Pull requests: against `main` only, no duplicate branch-push run.
- Documentation-only changes do not trigger expensive compiler builds.
- `workflow_dispatch` provides explicit manual full verification.
- Concurrency cancellation stops superseded runs on the same branch or PR.
- The single build job executes focused Cargo tests, builds the runnable
  `parse` example, runs Clippy, then runs the native EDG IL integration
  fixture and assertions. It intentionally avoids running the same
  expensive native archive build for each Rust commit.

CI: https://github.com/bingundu/edgcpp-rust/actions/workflows/build.yml

## Limitations

The EDG release binary is Linux x86-64 GCC ABI-specific and must match the
toolchain/macro configuration recorded in `compile_commands.json`. GitHub
Release assets can technically be replaced by a maintainer, but the pinned
SHA-256 checksum ensures such a replacement fails this workflow. A new EDG
archive should be published with a new versioned tag; do not mutate the
pinned artifact in-place. Rust cache reuse does not imply EDG binary
compatibility across upstream revisions.
