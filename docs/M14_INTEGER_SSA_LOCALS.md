# M14 — source-checked INTEGER automatic SSA in the FP reference

Status: merged in [PR #81](https://github.com/bingordo/edgcpp-rust/pull/81),
full pinned [native CI #42394399311](https://github.com/bingordo/edgcpp-rust/actions/runs/42394399311) passed. This is a bounded scalar-expression bridge, not a general
integer frontend or completion of M14 #60.

## Accepted native C source forms

Inside a supported F32/F64 function, an ordinary nonqualified
signed/unsigned 32/64-bit `int`/`long long` automatic variable may
be initialized or reassigned **only** from an unchanged same-type
native INTEGER formal, an already definitely initialized same-type
INTEGER automatic variable or an exact EDG constant bit image.

- For `dik_expression`, the original EDG expression must be an
  actual direct source variable/constant with exact type ID and
  symbol identity; no hidden arithmetic or implicit promotions.
- For `dik_constant`, the original statement must provide exactly
  one captured native EDG signed-64 wire integer image. Its bits
  preserve the complete unsigned-64 range. Target width/sign and
  native layout must match exactly (including sign extension for
  signed I32 and zero extension for unsigned I32).
- The source stays in an I32/I64 Cranelift SSA variable. The same
  path-sensitive definite-initialization checker used for FP locals
  rejects uninitialized integer reads across branches and loops.
- An actual EDG integer-to-F32/F64 conversion node may reference
  such an initialized INTEGER local. The numeric conversion uses
  `fcvt_from_sint`/`fcvt_from_uint` according to the original C
  signedness, not a bitcast.
- An INTEGER local may supply an exact-class direct, indirect or
  external typed C call argument, and may be a 32/64-bit signed or
  unsigned indexed-memory subscript under the separately checked
  C `p[i]` memory contract.

Neither source string spelling nor inferred scalar casts are used.

## Differential gate

The native GCC and EDG/Cranelift objects are linked separately to
the same independent driver. This tests numeric conversions near
signed and unsigned 32/64-bit boundaries, negative integer inputs,
large unsigned integer constants, initializer bit images, exact
integer local copies and reassignment, conditional definite
assignment, cross-object INTEGER/SSE calls and FP array access using
an initialized integer SSA local.

Pure Rust tests verify correct object emission for I32 local copies,
numeric conversion, exact typed external calls, source width/sign
mismatch rejection, local definite initialization and integer-local
subscript validation.

Negative C cases reject uninitialized reads, arbitrary integer
arithmetic, narrow integers, integer address-taking and implicit
integer-to-integer promotions. Rejected sources must leave no ELF
output.

## Not claimed

No general integer operators, integer automatic memory, integer
comparisons, pointer arithmetic, integer-valued call results
nested in FP expressions, usual arithmetic conversions, aggregate
memory, volatile/atomic or floating environment guarantees. M14's
backend-neutral source projection and exceptional behavior are
still tracked in [issue #60](https://github.com/bingordo/edgcpp-rust/issues/60).
