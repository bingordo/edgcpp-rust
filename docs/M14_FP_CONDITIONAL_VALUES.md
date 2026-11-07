# M14 — source-checked lazy C floating conditional values

**Status: merged** in [PR #74](https://github.com/bingordo/edgcpp-rust/pull/74),
squash commit `d45c2beb57a090667098138363afdf7d915e3bce`.
The full pinned EDG/Rust/Cranelift/GCC
[CI run #42317136600](https://github.com/bingordo/edgcpp-rust/actions/runs/42317136600)
passed on November 6, 2026.

## Supported source forms

C conditional-value expressions yielding native `float` or `double`:

```c
double choose(double c,double a,double b) {
    return c ? a : b;
}
double compare(double x,double y,double a,double b) {
    return x < y ? a : b;
}
double nested(double c,double x,double a,double b,double d) {
    return c ? (x ? a : b) : d;
}
```

An owned EDG operator 25 node with *exactly three operands*
identifies the source conditional; no textual parsing or guessed
conditional expression is used. The condition is lowered through the
existing native-verified FP comparison and scalar truthiness path;
nonzero F32/F64 is true, `+0` and `-0` are false, and NaN is
true for C `!=0` truthiness.

Both value arms must typecheck to the same native F32/F64 result
class **after any explicit EDG source conversions**. The verifier
checks the local initialization and all called targets in **both**
branches, including branches that may be dynamically untaken.

The generated Cranelift code uses a real conditional branch, executes
**only the selected arm**, and joins the two control-flow predecessors
through an F32/F64 SSA block parameter. It never speculatively
evaluates an unused arm. That matters for user-defined external
calls, global memory loads, FP exceptions and preserving the
selected arm's exact signed-zero/NaN bit pattern.

## Native differential and fail-closed checks

- [Source fixture](../tests/fixtures/m14_fp_conditional_values.c)
  and [GCC driver](../tests/fixtures/m14_fp_conditional_values_driver.c)
  cover F32/F64 truthiness, FP comparisons, nested expressions,
  arithmetic enclosing a ternary, `+0/-0`, NaN, `+/-infinity`,
  and direct external C calls on the selected arm.
- External calls update counters in the GCC driver: for each test
  case exactly **one** arm is executed.
- Negative source fixtures reject INTEGER-valued conditions,
  pointer-valued truthiness and unmodeled assignment side effects
  rather than silently inferring a general INTEGER or pointer
  expression tree.
- Rust tests reject mismatched result-arm widths, invalid condition
  widths, and missing direct-call targets even inside a branch
  that might be untaken.
- The legacy M14 truthiness negative fixture was updated from
  the now-supported FP ternary to a genuinely unsupported
  INTEGER-valued ternary expression.

## Still missing in the broader M14 milestone

This is **not** general C expression lowering. INTEGER-valued ternaries,
short-circuit logical expressions involving unmodeled INTEGER source
trees, arbitrary FP memory/lvalues, pointer-valued conditions,
unmodeled mutations in a ternary arm and C++ conditional overloads
remain unsupported. Full FP environment and all rounding-mode semantics
need independent evidence. See the maintained
[M14 milestone checklist](https://github.com/bingordo/edgcpp-rust/issues/60).
