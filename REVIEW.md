# Code review — EDG Rust bridge

Scope: `cpp/bridge.cpp`, `src/lib.rs`, `src/graph.rs`,
`build.rs`, `tools/build_bridge.py`, the example, fixtures, and CI.

## Correctness issues fixed in the review

1. **Variadic integration test was failing for the wrong reason.** An unused
   declaration never entered the partial EDG snapshot. Made the variadic
   fixture a definition, and explicitly test its signature.
2. **Overloaded routines shared the same graph identity.** The bridge stripped
   parameter lists from all demangled C++ names, so `f(int)` and `f(long)`
   had identical statement keys. When a scope contains overloads, keep
   demangled parameter signatures. Keep the former short names for
   non-overloaded routines.
3. **Namespace parsing misinterpreted qualified parameter types.**
   `outer::f(other::Widget)` could create spurious namespace nodes.
   Namespace splitting now stops at the beginning of the parameter list.
4. **Rust unwinding into C++.** Callback processing is wrapped in
   `catch_unwind` and compilation reports `CallbackPanicked`. This does
   not recover from allocation aborts or native fatal errors.
5. **Malformed C callback records were silently discarded.** Recognized
   callback kinds are checked for an inserted record; incomplete or
   unparseable records now cause `MalformedCallback`.
6. **Signed/unsigned constant conversion.** Unsigned integer constants larger
   than `i64::MAX` were at risk of becoming negative values. The bridge
   checks signedness and range, with regression coverage for `u64::MAX`.
7. **Inconsistent type edges.** A depth-truncated/missing child could be
   advertised as a valid parent-child relationship. Missing targets now
   clear the relation; graph construction checks that relation roles and
   child IDs are either both present or both absent.
8. **Weak graph parent validation.** A matching path prefix was enough to
   accept skipped levels. Statement and expression edges now require
   immediate, well-formed children.
9. **Empty argument lists.** Reject zero-argument compiler invocation rather
   than passing an invalid argv to EDG.
10. **Build quality checks.** Added Clippy and installed its rustup component
    in CI, plus unit/integration regression tests.

## Remaining significant limitations

- **Incomplete IL coverage:** only selected statement structures and
  operation operands are traversed. Non-operation subtrees, initializers,
  complete control flow, class methods, base classes, and other C++ features
  still need implementation. This must not be used as a complete compiler IR.
- **Identity of routine references:** overloaded routine *declarations* and
  statement graphs are disambiguated, but expression-level routine references
  still carry the legacy short name and cannot always identify one overload.
  Symbol identities should be made explicit, distinct from display names.
- **Lossy text-based callback protocol:** fields are tab-separated strings
  and C-string names. Although malformed records now fail, tabs embedded
  within names and embedded NULs still require an escaped/length-delimited
  structured ABI.
- **Bounded traversal:** the native visitor limits recursion to depth 64,
  statement lists to 10,000, and operand lists to 10,000. Truncation should
  become explicit, not silent, before relying on completeness.
- **Type representation:** typedef/reference qualifiers use native EDG
  encoding; function parameter flags, ABI lowering, field layout subtleties,
  array dynamic bounds and record inheritance aren't fully represented.
- **Diagnostics/repeated invocation:** EDG's diagnostics still go to its
  native reporting infrastructure; repeated compilation and error recovery
  need separate integration tests.
- **Native toolchain coupling:** bridge binary layout depends on *exactly*
  matching EDG headers, compile definitions and compiler ABI, as noted in
  the README and enforced through compilation flags.
- **Performance:** several `Graph` lookups scan vectors. A cached type
  index would help larger translation units. This is not yet performance
  critical for the prototype.

## Review criteria for the next milestone

- CI green on Rust tests, Clippy, and at least one actual EDG parse.
- Fixture includes overloads, both reference categories, a bit-field,
  a variadic function definition, and an out-of-range unsigned literal.
- Invalid callback records and malformed graph parent/type references
  return explicit errors.
- Scope and symbol identity problems are not obscured by naming shortcuts.
- Before using the snapshot for lowering, implement symbol IDs, a structured
  callback protocol, and explicit completeness/truncation reporting.
