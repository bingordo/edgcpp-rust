# Native compiler IR integration pilot — EDG C11 integer expressions

Status: implementation under full pinned EDG/IR/native-x86/GCC CI.
This first slice builds **the existing compiler IR directly**, with
no intermediate semantic projection tree, custom CFG, or dependence
on Cranelift.

## Architecture

```text
EDG C11 -> owned Il -> graph::Graph -> checked compiler-ir::Function
                               -> compiler-ir::verify
                               -> compiler-backend-x86 -> ELF .o -> GCC linker
```

The **optional** Cargo feature `native-compiler` pins `compiler-ir`,
`compiler-backend` and `compiler-backend-x86` to the same audited
`bingordo/compiler` commit. It does not enable the `codegen`
Cranelift feature. The compiler's core, optimizer, and normal
Oberon/mini-C drivers remain independent of EDG binaries.

Pilot source contract: complete, nonvariadic prototyped unmangled C
functions with direct nonqualified source-standard signed/unsigned
32/64-bit integer formals and returns; only nested side-effect-free
return expressions of exact same native integer type. Supported
expressions are direct formals, losslessly captured EDG integer
constants, unary plus, parenthesized expressions, unary minus and
binary add/subtract/multiply. The verifier runs on the constructed
native typed SSA; `IntBin` preserves the native 32/64-bit modular
machine operations (signed C overflow remains undefined). Each type
is sourced from exact EDG type IDs, signedness, width, and target
size/alignment. Native EDG kind mapping is delegated to the existing
bridge, not private enum numeric guesses.

Unsupported (explicitly rejected before generating output): C
automatics/lifetimes, user/global object memory, incomplete metadata,
implicit or mixed-width arithmetic conversions, pointer expressions,
calls, branches/loops, divisions, FP, C++ ABI names and side-effect
expressions. A C literal too wide for its exact EDG bit image is also
rejected. This is not a general-purpose C frontend yet.

The `native_compiler` example compiles EDG input and writes a native
ELF object **only after** successful IR verification and backend
code emission:

```shell
cargo run --features native-compiler --example native_compiler -- out.o --c11 source.c
```

The pinned CI builds this example, links the resulting ELF object into
an independent GCC-built driver and compares the full set of signed
and unsigned 32/64-bit functions against a GCC-built source object.
Separate negative fixtures reject unsupported C without producing
any partial object. Compiler IR remains the single production IR.
Next expand source-checked locals/branches/loops using compiler IR's
existing block arguments and verifier, then add native FP types and
memory operations through deliberate changes to the shared compiler
IR rather than inventing another IR.
